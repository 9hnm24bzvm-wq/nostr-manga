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
pub(crate) mod personal_reports;

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
    // This route is My reports for EVERY account. Site-wide work has an
    // explicitly named and separately authorized /staff/reports workspace.
    let actor = sql.bind(a.user_id);
    sql.conditions.push(format!("r.submitter_id={actor}"));
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
    crate::domains::auth::closure::active_in(&tx,&a.user_id).await?;
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
    crate::domains::auth::closure::active_in(&tx,&a.user_id).await?;
    let decision = review_report(&tx, &a, id, notes.as_deref(), expected, status, operation).await?;
    let value = tx.query_one("SELECT platform.report_json($1)", &[&id]).await?.get(0);
    let _ = decision;
    tx.commit().await?;
    Ok(Json(value))
}

/// Sealed genuine mutation actor. Personal inspection capabilities do not
/// implement this trait; neither UUIDs nor report ownership grant review rights.
pub(crate) trait ReportReviewActor: crate::domains::catalog::entity_mutations::CatalogMutationActor + sealed::Sealed {}
impl ReportReviewActor for SessionPrincipal {}
#[derive(Debug)]
pub(crate) struct ReportDecision {
    pub old_version: i32,
    pub new_version: i32,
    pub effect: Value,
}
/// Native and signed adapters share exactly one terminal transition/effect/audit
/// transaction. Caller authenticates, owns locks and commits. Never emits events.
pub(crate) async fn review_report(
    tx: &tokio_postgres::Transaction<'_>, a: &impl ReportReviewActor,
    id: &str, notes: Option<&str>, expected: Option<i32>, status: &str, operation: &str,
) -> ApiResult<ReportDecision> {
    if !a.is_moderator() { return Err(ApiError::forbidden("Moderator report permission required")); }
    a.require_permission("report.update")?;
    if !matches!((status, operation), ("accepted", "accept") | ("refused", "reject")) {
        return Err(ApiError::bad_request("Invalid report decision"));
    }
    crate::domains::auth::closure::active_in(tx,a.user_id()).await?;
    // Title removal uses target-first locking, matching the dedicated DELETE
    // path. A report's immutable target/reason is re-read under the report lock.
    let title_target=if status=="accepted" {
        if let Some(row)=tx.query_opt("SELECT entity_id,status,version FROM platform.reports WHERE id=$1 AND entity_type='manga' AND reason_id='manga_removal_request' AND deleted_at IS NULL",&[&id]).await? {
            if row.get::<_,String>(1)!="waiting"||expected.is_some_and(|v|v!=row.get::<_,i32>(2)){return Err(ApiError::conflict("Report has already changed or been reviewed"));}
            Some(row.get::<_,String>(0))
        }else{None}
    }else{None};
    if let Some(target)=&title_target {
        tx.query_opt("SELECT id FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id) FOR UPDATE",&[&target]).await?.ok_or_else(||ApiError::not_found("Title removal target is unavailable"))?;
    }
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
        let previous_member=tx.query_opt("SELECT name,is_leader,is_officer FROM platform.catalog_group_members WHERE group_id=$1 AND user_id=$2 FOR UPDATE",&[&group_id,&claimant]).await?;
        let previous_membership=previous_member.map(|r|json!({"name":r.get::<_,String>(0),"is_leader":r.get::<_,bool>(1),"is_officer":r.get::<_,bool>(2)}));
        tx.execute("INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader,is_officer) VALUES($1,$2,$3,TRUE,FALSE) ON CONFLICT(group_id,user_id) DO UPDATE SET name=EXCLUDED.name,is_leader=TRUE,is_officer=FALSE",&[&group_id,&claimant,&claimant_name]).await?;
        tx.execute("UPDATE platform.catalog_groups SET version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&group_id]).await?;
        crate::domains::catalog::repository::audit(
            tx,
            &a.user_id(),
            "group",
            &group_id,
            &format!("claim_accept_report:{id}"),
            Some(group_version),
            Some(group_version + 1),
        )
        .await?;
        effect = json!({"action":"grant_group_leadership","group_id":group_id,"claimant_id":claimant,"old_group_version":group_version,"new_group_version":group_version+1,"previous_membership":previous_membership,"new_member_name":claimant_name});
    }
    if status == "accepted" && row.get::<_, String>(5) == "chapter_removal_request" {
        a.require_permission("chapter.remove")?;
        let chapter: String = row.get(4);
        let submitted_version: Option<i32> = row.get(6);
        let current = tx.query_opt("SELECT version FROM platform.catalog_chapters WHERE id=$1 AND deleted_at IS NULL FOR UPDATE", &[&chapter]).await?
            .ok_or_else(|| ApiError::not_found("Chapter removal target no longer exists"))?;
        if submitted_version != Some(current.get::<_, i32>(0)) {
            return Err(ApiError::conflict("Chapter changed since this removal request; review its current state"));
        }
        crate::domains::catalog::CatalogRepository::delete_chapter(tx, &chapter, a).await?;
        let old_version=current.get::<_,i32>(0);
        effect = json!({"action":"instance_soft_hide", "chapter_id":chapter, "source_deletion":"independent_original_moderation","old_version":old_version,"new_version":old_version+1,"previous_deleted":false});
    }
    if status == "accepted" && row.get::<_,String>(5) == "manga_removal_request" {
        a.require_permission("manga.delete")?;
        let manga:String=row.get(4);
        if title_target.as_deref()!=Some(manga.as_str()){return Err(ApiError::conflict("Removal request target changed"));}
        let submitted:Option<i32>=row.get(6);
        let current:i32=tx.query_one("SELECT version FROM platform.catalog_manga WHERE id=$1",&[&manga]).await?.get(0);
        if submitted!=Some(current){return Err(ApiError::conflict("Title changed since this removal request; review its current state"));}
        let next=crate::domains::catalog::mutations::manga_delete_in(tx,a,&manga,Some(current)).await?;
        effect=json!({"action":"instance_soft_hide","manga_id":manga,"old_version":current,"new_version":next,"source_deletion":"not_requested","original_event_rewritten":false,"previous_deleted":false});
    }
    tx.execute("UPDATE platform.reports SET status=$2,notes=$3,reviewed_by=$4,version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id,&status,&notes,&a.user_id()]).await?;
    tx.execute("INSERT INTO platform.report_audit(report_id,actor_id,operation,old_status,new_status,old_version,new_version,notes,effect) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",&[&id,&a.user_id(),&operation,&old,&status,&version,&(version+1),&notes,&effect]).await?;
    Ok(ReportDecision {old_version:version, new_version:version+1, effect})
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
    delete_report(&tx,&a,&id,None).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Report-only native soft deletion. Never reads or mutates the reported target.
/// Preserve the native status, notes, reviewer and empty audit effect for every
/// undeleted status. The caller owns the transaction and commit.
pub(crate) struct ReportDeletion {pub status:String,pub old_version:i32,pub new_version:i32}
pub(crate) async fn delete_report(tx:&tokio_postgres::Transaction<'_>,a:&impl ReportReviewActor,id:&str,expected:Option<i32>)->ApiResult<ReportDeletion>{
    delete_report_reasoned(tx,a,id,expected,None).await
}
pub(crate) async fn delete_report_reasoned(tx:&tokio_postgres::Transaction<'_>,a:&impl ReportReviewActor,id:&str,expected:Option<i32>,notes:Option<&str>)->ApiResult<ReportDeletion>{
    if !a.is_moderator(){return Err(ApiError::forbidden("Moderator report permission required"));}
    a.require_permission("report.delete")?;v::id(id)?;
    crate::domains::auth::closure::active_in(tx,a.user_id()).await?;
    let row=tx.query_opt("SELECT status,version FROM platform.reports WHERE id=$1 AND deleted_at IS NULL FOR UPDATE",&[&id]).await?.ok_or_else(||ApiError::not_found("Report not found"))?;
    let status:String=row.get(0);let old_version:i32=row.get(1);
    if expected.is_some_and(|v|v!=old_version){return Err(ApiError::conflict("Report changed; reopen deletion review"));}
    let new_version=old_version.checked_add(1).ok_or_else(||ApiError::conflict("Report revision exhausted"))?;
    tx.execute("UPDATE platform.reports SET deleted_at=clock_timestamp(),updated_at=clock_timestamp(),version=version+1 WHERE id=$1",&[&id]).await?;
    tx.execute("INSERT INTO platform.report_audit(report_id,actor_id,operation,old_status,new_status,old_version,new_version,notes) VALUES($1,$2,'delete',$3,$3,$4,$5,$6)",&[&id,&a.user_id(),&status,&old_version,&new_version,&notes]).await?;
    Ok(ReportDeletion{status,old_version,new_version})
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
    crate::domains::auth::closure::active_in(tx,a.user_id()).await?;
    a.require_permission("report.create")?;
    category(entity)?;
    v::id(target)?;
    v::id(reason)?;
    // Serialize per-user limits and duplicate creation, without trusting a client-supplied identity.
    tx.query_one(
        "SELECT id FROM platform.users WHERE id=$1 AND NOT disabled FOR NO KEY UPDATE",
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
