//! Permission-checked private reports, terminal review transitions and append-only audit.
use crate::{
    domains::{
        auth::{SessionPrincipal, require_principal, validate_mutation},
        catalog::{
            query::{QueryParams, Sql},
            validation as v,
        },
    },
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, RawQuery, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post},
};
use serde_json::{Value, json};
const CATEGORIES: &[&str] = &[
    "user",
    "manga",
    "author",
    "chapter",
    "scanlation_group",
    "thread",
    "reply",
];
const REPORT_STATUSES: &[&str] = &["waiting", "accepted", "refused", "autoresolved"];
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/report", get(search).post(create))
        .route("/report/reasons/{entity}", get(reasons))
        .route("/report/{id}", delete(remove))
        .route("/report/{id}/accept", post(accept))
        .route("/report/{id}/reject", post(reject))
}
fn category(value: &str) -> ApiResult<()> {
    if CATEGORIES.contains(&value) {
        Ok(())
    } else {
        Err(ApiError::bad_request("Unsupported report category"))
    }
}
fn reviewer(a: &SessionPrincipal, permission: &str) -> ApiResult<()> {
    if !a.is_moderator() {
        return Err(ApiError::forbidden("Moderator report permission required"));
    }
    a.require_permission(permission)
}
async fn reasons(State(s): State<AppState>, Path(entity): Path<String>) -> ApiResult<Json<Value>> {
    category(&entity)?;
    let db = s.db.client().await?;
    let result:Value=db.query_one("SELECT COALESCE(jsonb_agg(jsonb_build_object('id',id,'category',category,'reason',reason,'details_required',details_required) ORDER BY reason,id),'[]'::jsonb) FROM platform.report_reasons WHERE category=$1 AND enabled",&[&entity]).await?.get(0);
    Ok(Json(result))
}
async fn search(
    State(s): State<AppState>,
    h: HeaderMap,
    RawQuery(raw): RawQuery,
) -> ApiResult<Json<Value>> {
    let a = require_principal(&s, &h).await?;
    a.require_browser_session()?;
    let q = QueryParams::parse_with_statuses(raw, REPORT_STATUSES)?;
    let order = q.order("desc")?;
    let mut sql = Sql::default();
    sql.conditions.push("r.deleted_at IS NULL".into());
    if !(a.is_moderator() && a.has_perm("report.update")) {
        let actor = sql.bind(a.user_id);
        sql.conditions.push(format!("r.submitter_id={actor}"));
    }
    for (key, column) in [
        ("category", "entity_type"),
        ("object", "entity_id"),
        ("reason", "reason_id"),
        ("status", "status"),
    ] {
        if let Some(value) = q.first(key).filter(|v| !v.is_empty()) {
            match key {
                "category" => category(value)?,
                "status" => {
                    if !REPORT_STATUSES.contains(&value) {
                        return Err(ApiError::bad_request("Invalid report status"));
                    }
                }
                _ => v::id(value)?,
            }
            let p = sql.bind(value.to_owned());
            sql.conditions.push(format!("r.{column}={p}"));
        }
    }
    let predicate = sql.predicate();
    let limit = sql.bind(q.limit);
    let offset = sql.bind((q.page - 1) * q.limit);
    let db = s.db.client().await?;
    let row=db.query_one(&format!("SELECT (SELECT count(*) FROM platform.reports r WHERE {predicate}),COALESCE((SELECT jsonb_agg(x.dto ORDER BY x.created_at {order},x.id {order}) FROM (SELECT r.id,r.created_at,platform.report_json(r.id) dto FROM platform.reports r WHERE {predicate} ORDER BY r.created_at {order},r.id {order} LIMIT {limit} OFFSET {offset})x),'[]'::jsonb)"),&sql.params()).await?;
    Ok(Json(
        json!({"total":row.get::<_,i64>(0),"limit":q.limit,"page":q.page,"data":row.get::<_,Value>(1)}),
    ))
}
async fn create(
    State(s): State<AppState>,
    h: HeaderMap,
    Json(p): Json<Value>,
) -> ApiResult<StatusCode> {
    let a = require_principal(&s, &h).await?;
    validate_mutation(&s, &h)?;
    a.require_permission("report.create")?;
    v::allowed(&p, &["entity_type", "entity_id", "reason_id", "details"])?;
    let entity = v::string(&p, "entity_type", 32, true)?.unwrap();
    category(&entity)?;
    let target = v::string(&p, "entity_id", 128, true)?.unwrap();
    v::id(&target)?;
    let reason = v::string(&p, "reason_id", 128, true)?.unwrap();
    v::id(&reason)?;
    let details = v::string(&p, "details", 20000, false)?.unwrap_or_default();
    let mut db = s.db.client().await?;
    let tx = db.transaction().await?;
    create_report(&tx, &a, &entity, &target, &reason, &details).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn accept(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(p): Json<Value>,
) -> ApiResult<Json<Value>> {
    review(&s, &h, &id, &p, "accepted", "accept").await
}
async fn reject(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(p): Json<Value>,
) -> ApiResult<Json<Value>> {
    review(&s, &h, &id, &p, "refused", "reject").await
}
async fn review(
    s: &AppState,
    h: &HeaderMap,
    id: &str,
    p: &Value,
    status: &str,
    operation: &str,
) -> ApiResult<Json<Value>> {
    let a = require_principal(s, h).await?;
    validate_mutation(s, h)?;
    reviewer(&a, "report.update")?;
    v::id(id)?;
    v::allowed(p, &["notes", "version"])?;
    let notes = v::string(p, "notes", 20000, false)?;
    let expected = if p.get("version").is_some() {
        Some(v::version(p)?)
    } else {
        None
    };
    let mut db = s.db.client().await?;
    let tx = db.transaction().await?;
    let row=tx.query_opt("SELECT status,version,submitter_id,entity_type,entity_id,reason_id,target_version FROM platform.reports WHERE id=$1 AND deleted_at IS NULL FOR UPDATE",&[&id]).await?.ok_or_else(||ApiError::not_found("Report not found"))?;
    let old: String = row.get(0);
    let version: i32 = row.get(1);
    if old != "waiting" || expected.is_some_and(|v| v != version) {
        return Err(ApiError::conflict(
            "Report has already changed or been reviewed",
        ));
    }
    let mut effect = json!({});
    if status == "accepted" && row.get::<_, String>(5) == "scanlation_group_claim" {
        // This deliberate moderator acceptance is the only claim-to-authority transition.
        // A generic report decision never adds global roles or group membership.
        a.require_permission("scanlation_group.update")?;
        let claimant: String = row.get(2);
        let group_id: String = row.get(4);
        let submitted_version: Option<i32> = row.get(6);
        let user=tx.query_opt("SELECT username FROM platform.users WHERE id=$1 AND verified AND NOT disabled AND NOT (roles ?| ARRAY['banned','restricted']) FOR UPDATE",&[&claimant]).await?.ok_or_else(||ApiError::conflict("Claimant is no longer eligible to lead this group"))?;
        let group=tx.query_opt("SELECT version,locked FROM platform.catalog_groups WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_group_is_visible(id) FOR UPDATE",&[&group_id]).await?.ok_or_else(||ApiError::not_found("Claim target group no longer exists"))?;
        let group_version: i32 = group.get(0);
        if group.get::<_, bool>(1) || submitted_version != Some(group_version) {
            return Err(ApiError::conflict(
                "Group changed since this ownership claim was submitted",
            ));
        }
        if tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.catalog_group_members WHERE group_id=$1 AND is_leader)",&[&group_id]).await?.get::<_,bool>(0){return Err(ApiError::conflict("This group has already been claimed"));}
        let claimant_name: String = user.get(0);
        tx.execute("INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader,is_officer) VALUES($1,$2,$3,TRUE,FALSE) ON CONFLICT(group_id,user_id) DO UPDATE SET name=EXCLUDED.name,is_leader=TRUE,is_officer=FALSE",&[&group_id,&claimant,&claimant_name]).await?;
        tx.execute("UPDATE platform.catalog_groups SET version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&group_id]).await?;
        crate::domains::catalog::repository::audit(
            &tx,
            &a.user_id,
            "group",
            &group_id,
            &format!("claim_accept_report:{id}"),
            Some(group_version),
            Some(group_version + 1),
        )
        .await?;
        effect = json!({"action":"grant_group_leadership","group_id":group_id,"claimant_id":claimant,"old_group_version":group_version,"new_group_version":group_version+1});
    }
    tx.execute("UPDATE platform.reports SET status=$2,notes=$3,reviewed_by=$4,version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id,&status,&notes,&a.user_id]).await?;
    tx.execute("INSERT INTO platform.report_audit(report_id,actor_id,operation,old_status,new_status,old_version,new_version,notes,effect) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",&[&id,&a.user_id,&operation,&old,&status,&version,&(version+1),&notes,&effect]).await?;
    let value = tx
        .query_one("SELECT platform.report_json($1)", &[&id])
        .await?
        .get(0);
    tx.commit().await?;
    Ok(Json(value))
}
async fn remove(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let a = require_principal(&s, &h).await?;
    validate_mutation(&s, &h)?;
    reviewer(&a, "report.delete")?;
    v::id(&id)?;
    let mut db = s.db.client().await?;
    let tx = db.transaction().await?;
    let row=tx.query_opt("UPDATE platform.reports SET deleted_at=clock_timestamp(),updated_at=clock_timestamp(),version=version+1 WHERE id=$1 AND deleted_at IS NULL RETURNING status,version",&[&id]).await?.ok_or_else(||ApiError::not_found("Report not found"))?;
    let status: String = row.get(0);
    let version: i32 = row.get(1);
    tx.execute("INSERT INTO platform.report_audit(report_id,actor_id,operation,old_status,new_status,old_version,new_version) VALUES($1,$2,'delete',$3,$3,$4,$5)",&[&id,&a.user_id,&status,&(version-1),&version]).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// Only genuine native sessions and private, transaction-rechecked public write
// capabilities can invoke this shared local-instance mutation core.
pub(crate) mod sealed {
    pub trait Sealed {}
}
pub(crate) trait ReportMutationActor: sealed::Sealed {
    fn user_id(&self) -> &str;
    fn require_permission(&self, permission: &str) -> ApiResult<()>;
}
impl sealed::Sealed for SessionPrincipal {}
impl ReportMutationActor for SessionPrincipal {
    fn user_id(&self) -> &str { &self.user_id }
    fn require_permission(&self, permission: &str) -> ApiResult<()> {
        SessionPrincipal::require_permission(self, permission)
    }
}
/// Transaction-owned creation. Authentication, account binding and commit belong
/// to the caller. Reports/audit are private instance moderation, never events.
pub(crate) async fn create_report(
    tx: &tokio_postgres::Transaction<'_>,
    a: &impl ReportMutationActor,
    entity: &str,
    target: &str,
    reason: &str,
    details: &str,
) -> ApiResult<String> {
    a.require_permission("report.create")?;
    category(entity)?;
    v::id(target)?;
    v::id(reason)?;
    // Serialize per-user limits and duplicate creation, without trusting a client-supplied identity.
    tx.query_one(
        "SELECT id FROM platform.users WHERE id=$1 FOR UPDATE",
        &[&a.user_id()],
    )
    .await?;
    let count:i64=tx.query_one("SELECT count(*) FROM platform.reports WHERE submitter_id=$1 AND created_at>clock_timestamp()-interval '1 hour'",&[&a.user_id()]).await?.get(0);
    if count >= 20 {
        return Err(ApiError::busy());
    }
    let reason_row=tx.query_opt("SELECT details_required FROM platform.report_reasons WHERE id=$1 AND category=$2 AND enabled FOR SHARE",&[&reason,&entity]).await?.ok_or_else(||ApiError::bad_request("Reason does not belong to this report category"))?;
    if reason_row.get::<_, bool>(0) && details.is_empty() {
        return Err(ApiError::bad_request(
            "Details are required for this report reason",
        ));
    }
    if !tx
        .query_one(
            "SELECT platform.report_entity_exists($1,$2)",
            &[&entity, &target],
        )
        .await?
        .get::<_, bool>(0)
    {
        return Err(ApiError::not_found("Report target not found"));
    }
    if tx.query_opt("SELECT id FROM platform.reports WHERE submitter_id=$1 AND entity_type=$2 AND entity_id=$3 AND reason_id=$4 AND status='waiting' AND deleted_at IS NULL",&[&a.user_id(),&entity,&target,&reason]).await?.is_some(){return Err(ApiError::conflict("An identical report is already awaiting review"));}
    let target_version: Option<i32> = if reason == "scanlation_group_claim" {
        let group=tx.query_opt("SELECT version,locked FROM platform.catalog_groups WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_group_is_visible(id) FOR SHARE",&[&target]).await?.ok_or_else(||ApiError::not_found("Group not found"))?;
        if group.get::<_, bool>(1) {
            return Err(ApiError::forbidden("This group is locked"));
        }
        if tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.catalog_group_members WHERE group_id=$1 AND is_leader)",&[&target]).await?.get::<_,bool>(0){return Err(ApiError::conflict("This group has already been claimed"));}
        Some(group.get(0))
    } else {
        None
    };
    let id = v::new_id("report");
    tx.execute("INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,body,target_version) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&id,&a.user_id(),&entity,&target,&reason,&details,&target_version]).await?;
    tx.execute("INSERT INTO platform.report_audit(report_id,actor_id,operation,new_status,new_version) VALUES($1,$2,'create','waiting',1)",&[&id,&a.user_id()]).await?;
    Ok(id)
}
