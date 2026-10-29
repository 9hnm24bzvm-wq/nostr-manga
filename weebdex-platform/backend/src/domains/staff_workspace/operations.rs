//! Read-only operational overview: no signer paths, payloads, credentials or replay.
use crate::{AppState, ApiError, ApiResult, domains::auth};
use axum::{Json, extract::State, http::HeaderMap};
use serde_json::{Value,json};

pub(super) async fn read(State(s):State<AppState>,h:HeaderMap)->ApiResult<Json<Value>> {
    let mut db=s.db.client().await?;let tx=db.transaction().await?;
    let a=auth::require_native_account_principal_in(&s,&h,&tx,false).await?;
    if !a.is_staff(){return Err(ApiError::forbidden("Staff operations permission required"));}a.require_permission("operations.read")?;
    let publications=tx.query("SELECT command_id,resource_kind,catalog_id,state,platform.catalog_timestamp(updated_at) FROM platform.protocol_publication_commands WHERE state<>'materialized' ORDER BY updated_at,command_id LIMIT 51",&[]).await?;
    let schedules=tx.query("SELECT session_id,state,failure_code,platform.catalog_timestamp(updated_at) FROM platform.scheduled_uploads WHERE state IN ('blocked','starting') ORDER BY updated_at,session_id LIMIT 51",&[]).await?;
    let publication_count:i64=tx.query_one("SELECT count(*) FROM platform.protocol_publication_commands WHERE state<>'materialized'",&[]).await?.get(0);
    let schedule_count:i64=tx.query_one("SELECT count(*) FROM platform.scheduled_uploads WHERE state IN ('blocked','starting')",&[]).await?.get(0);
    // Map internal state to a bounded operational instruction, not arbitrary
    // stored errors or private upload/publisher inputs.
    let pending:Vec<Value>=publications.into_iter().take(50).map(|r|{let state:String=r.get(3);json!({"reference":r.get::<_,String>(0),"kind":r.get::<_,String>(1),"target":r.get::<_,String>(2),"state":state,"updatedAt":r.get::<_,String>(4),"nextStep":if state=="source_accepted"{"Source accepted; reconcile the original command before local materialization. Do not create a replacement."}else{"Outcome may be uncertain. Inspect the original command and source receipt; do not repeat publication."}})}).collect();
    let blocked:Vec<Value>=schedules.into_iter().take(50).map(|r|{let state:String=r.get(1);json!({"reference":r.get::<_,String>(0),"state":state,"updatedAt":r.get::<_,String>(3),"nextStep":if state=="starting"{"A publication may already have started. Check its original receipt before any recovery action."}else{"The contributor can inspect the saved job and resolve the displayed prerequisite before an explicitly supported retry."}})}).collect();
    let result=json!({"scope":"native-operations-read-only","actorId":a.user_id,"health":{"database":"ready","protocolSchemaReady":s.protocol.is_ready(),"draining":s.runtime.is_draining(),"registrationEnabled":s.config.registration_enabled,"emailConfigured":s.config.auth_mail.is_some(),"publicationConfigured":s.config.protocol_publication.is_some(),"instancePoliciesConfigured":s.config.instance.configured()},"pendingPublications":pending,"publicationCount":publication_count,"blockedSchedules":blocked,"scheduleCount":schedule_count,"runbook":"/staff/operations","authorityNotice":"This view does not grant original-service operator, publisher, peer-management or storage-repair authority."});
    tx.commit().await?;Ok(Json(result))
}
