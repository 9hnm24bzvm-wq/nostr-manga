//! Persisted local discussion API. Signed protocol events remain immutable.
use crate::{
    domains::{
        auth::{
            SessionPrincipal, optional_personal_principal, require_principal, validate_mutation,
        },
        catalog::{query::QueryParams, validation as v},
    },
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, RawQuery, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use serde_json::{Value, json};
use tokio_postgres::Transaction;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/thread/{id}", get(thread_get))
        .route(
            "/thread/{id}/vote",
            get(thread_vote_get).post(thread_vote).delete(thread_unvote),
        )
        .route("/thread/{id}/replies", get(replies).post(create_reply))
        .route(
            "/thread/{id}/replies/{reply}",
            get(reply_get).put(update_reply).delete(remove_reply),
        )
        .route(
            "/thread/{id}/replies/{reply}/hard",
            axum::routing::delete(delete_reply),
        )
        .route(
            "/thread/{id}/replies/{reply}/vote",
            axum::routing::post(reply_vote).delete(reply_unvote),
        )
}
async fn actor(s: &AppState, h: &HeaderMap) -> ApiResult<SessionPrincipal> {
    let a = require_principal(s, h).await?;
    a.require_browser_session()?;
    validate_mutation(s, h)?;
    a.require_permission("reply.create")?;
    Ok(a)
}
async fn mutable_actor(s: &AppState, h: &HeaderMap) -> ApiResult<SessionPrincipal> {
    let a = require_principal(s, h).await?;
    a.require_browser_session()?;
    validate_mutation(s, h)?;
    a.require_verified()?;
    if a.has_role("banned") || a.has_role("restricted") {
        return Err(ApiError::forbidden(
            "This account cannot perform that action",
        ));
    }
    Ok(a)
}
fn reply_id(value: &str) -> ApiResult<i64> {
    value
        .parse::<i64>()
        .ok()
        .filter(|v| *v > 0 && *v <= i32::MAX as i64)
        .ok_or_else(|| ApiError::bad_request("Invalid reply identifier"))
}
async fn visible(db: &impl tokio_postgres::GenericClient, id: &str) -> ApiResult<()> {
    v::id(id)?;
    if !db
        .query_one("SELECT platform.community_thread_visible($1)", &[&id])
        .await?
        .get::<_, bool>(0)
    {
        return Err(ApiError::not_found("Thread not found"));
    }
    Ok(())
}
async fn locked(tx: &Transaction<'_>, id: &str) -> ApiResult<bool> {
    visible(tx, id).await?;
    Ok(tx
        .query_one(
            "SELECT locked FROM platform.catalog_threads WHERE id=$1 FOR UPDATE",
            &[&id],
        )
        .await?
        .get(0))
}
fn authority(a: &SessionPrincipal, owner: &str, action: &str) -> ApiResult<()> {
    if a.is_moderator() && a.has_perm(&format!("reply.{action}")) {
        return Ok(());
    }
    if a.user_id == owner {
        a.require_permission(&format!("self.reply.{action}"))
    } else {
        Err(ApiError::forbidden(
            "Only the reply owner or an authorized moderator may change it",
        ))
    }
}
async fn audit(
    tx: &Transaction<'_>,
    a: &SessionPrincipal,
    id: &str,
    reply: i64,
    operation: &str,
    old: Option<i32>,
    new: Option<i32>,
) -> ApiResult<()> {
    tx.execute("INSERT INTO platform.community_audit(actor_id,thread_id,reply_id,operation,old_version,new_version) VALUES($1,$2,$3,$4,$5,$6)",&[&a.user_id,&id,&reply,&operation,&old,&new]).await?;
    Ok(())
}
async fn thread_get(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let db = s.db.client().await?;
    visible(&**db, &id).await?;
    Ok(Json(
        db.query_one("SELECT platform.community_thread_json($1)", &[&id])
            .await?
            .get(0),
    ))
}
async fn thread_vote_get(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let a = require_principal(&s, &h).await?;
    a.require_browser_session()?;
    let db = s.db.client().await?;
    visible(&**db, &id).await?;
    let vote:i16=db.query_one("SELECT COALESCE((SELECT vote FROM platform.community_thread_votes WHERE thread_id=$1 AND user_id=$2),0)::SMALLINT",&[&id,&a.user_id]).await?.get(0);
    Ok(Json(json!({"vote":vote})))
}
fn vote(payload: &Value) -> ApiResult<i16> {
    v::allowed(payload, &["vote"])?;
    match payload.get("vote").and_then(Value::as_i64) {
        Some(n @ (-1 | 1)) => Ok(n as i16),
        _ => Err(ApiError::bad_request("Vote must be -1 or 1")),
    }
}
async fn thread_vote(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(p): Json<Value>,
) -> ApiResult<StatusCode> {
    let a = mutable_actor(&s, &h).await?;
    a.require_permission("reply.create")?;
    let vote = vote(&p)?;
    let mut db = s.db.client().await?;
    let tx = db.transaction().await?;
    if locked(&tx, &id).await? {
        return Err(ApiError::forbidden("Thread is locked"));
    }
    let owner: String = tx
        .query_one(
            "SELECT author_id FROM platform.catalog_threads WHERE id=$1",
            &[&id],
        )
        .await?
        .get(0);
    if owner == a.user_id {
        return Err(ApiError::forbidden("You cannot vote on your own thread"));
    }
    tx.execute("INSERT INTO platform.community_thread_votes(thread_id,user_id,vote) VALUES($1,$2,$3) ON CONFLICT(thread_id,user_id) DO UPDATE SET vote=EXCLUDED.vote,updated_at=clock_timestamp()",&[&id,&a.user_id,&vote]).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn thread_unvote(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let a = mutable_actor(&s, &h).await?;
    a.require_permission("reply.create")?;
    let db = s.db.client().await?;
    visible(&**db, &id).await?;
    db.execute(
        "DELETE FROM platform.community_thread_votes WHERE thread_id=$1 AND user_id=$2",
        &[&id, &a.user_id],
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn replies(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> ApiResult<Json<Value>> {
    let q = QueryParams::parse(raw)?;
    let order = q.order("desc")?;
    let a = optional_personal_principal(&s, &h).await?;
    let viewer = a
        .filter(|a| a.require_browser_session().is_ok())
        .map(|a| a.user_id);
    let db = s.db.client().await?;
    visible(&**db, &id).await?;
    let offset = (q.page - 1) * q.limit;
    let row=db.query_one(&format!("SELECT (SELECT count(*) FROM platform.community_replies WHERE thread_id=$1),COALESCE((SELECT jsonb_agg(x.dto ORDER BY x.reply_number {order}) FROM (SELECT r.reply_number,platform.community_reply_json(r.id,$2) dto FROM platform.community_replies r WHERE r.thread_id=$1 ORDER BY r.reply_number {order} LIMIT $3 OFFSET $4)x),'[]'::jsonb)"),&[&id,&viewer,&q.limit,&offset]).await?;
    Ok(Json(
        json!({"total":row.get::<_,i64>(0),"limit":q.limit,"page":q.page,"data":row.get::<_,Value>(1)}),
    ))
}
async fn reply_get(
    State(s): State<AppState>,
    h: HeaderMap,
    Path((id, r)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    let r = reply_id(&r)?;
    let a = optional_personal_principal(&s, &h).await?;
    let viewer = a
        .filter(|a| a.require_browser_session().is_ok())
        .map(|a| a.user_id);
    let db = s.db.client().await?;
    visible(&**db, &id).await?;
    let row=db.query_opt("SELECT platform.community_reply_json(id,$3) FROM platform.community_replies WHERE thread_id=$1 AND id=$2",&[&id,&r,&viewer]).await?.ok_or_else(||ApiError::not_found("Reply not found"))?;
    Ok(Json(row.get(0)))
}
async fn create_reply(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(p): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let a = actor(&s, &h).await?;
    v::allowed(&p, &["body", "parent_id"])?;
    let body = v::string(&p, "body", 50000, true)?.unwrap();
    let parent = match p.get("parent_id") {
        None => None,
        Some(v) => Some(
            v.as_i64()
                .filter(|r| *r > 0 && *r <= i32::MAX as i64)
                .ok_or_else(|| ApiError::bad_request("Invalid parent reply"))?,
        ),
    };
    let mut db = s.db.client().await?;
    let tx = db.transaction().await?;
    if locked(&tx, &id).await? {
        return Err(ApiError::forbidden("Thread is locked"));
    }
    if let Some(parent) = parent {
        if tx.query_opt("SELECT id FROM platform.community_replies WHERE id=$1 AND thread_id=$2 AND state='active' FOR SHARE",&[&parent,&id]).await?.is_none(){return Err(ApiError::bad_request("Parent reply must be active and belong to this thread"));}
    }
    let n:i32=tx.query_one("UPDATE platform.catalog_threads SET next_reply_number=next_reply_number+1,updated_at=clock_timestamp() WHERE id=$1 RETURNING next_reply_number-1",&[&id]).await?.get(0);
    let r:i64=tx.query_one("INSERT INTO platform.community_replies(thread_id,reply_number,parent_id,author_id,body) VALUES($1,$2,$3,$4,$5) RETURNING id",&[&id,&n,&parent,&a.user_id,&body]).await?.get(0);
    audit(&tx, &a, &id, r, "create", None, Some(1)).await?;
    let data = tx
        .query_one(
            "SELECT platform.community_reply_json($1,$2)",
            &[&r, &a.user_id],
        )
        .await?
        .get(0);
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(data)))
}
async fn update_reply(
    State(s): State<AppState>,
    h: HeaderMap,
    Path((id, r)): Path<(String, String)>,
    Json(p): Json<Value>,
) -> ApiResult<Json<Value>> {
    let a = mutable_actor(&s, &h).await?;
    let r = reply_id(&r)?;
    v::allowed(&p, &["body", "version"])?;
    let version = v::version(&p)?;
    let body = v::string(&p, "body", 50000, true)?.unwrap();
    let mut db = s.db.client().await?;
    let tx = db.transaction().await?;
    let thread_locked = locked(&tx, &id).await?;
    let row=tx.query_opt("SELECT author_id,version,state FROM platform.community_replies WHERE thread_id=$1 AND id=$2 FOR UPDATE",&[&id,&r]).await?.ok_or_else(||ApiError::not_found("Reply not found"))?;
    authority(&a, &row.get::<_, String>(0), "update")?;
    if thread_locked && !a.is_moderator() {
        return Err(ApiError::forbidden("Thread is locked"));
    }
    if row.get::<_, String>(2) != "active" {
        return Err(ApiError::conflict("Only active replies may be edited"));
    }
    if row.get::<_, i32>(1) != version {
        return Err(ApiError::conflict("Reply changed; reload before editing"));
    }
    tx.execute("UPDATE platform.community_replies SET body=$3,version=version+1,updated_at=clock_timestamp() WHERE thread_id=$1 AND id=$2",&[&id,&r,&body]).await?;
    audit(&tx, &a, &id, r, "update", Some(version), Some(version + 1)).await?;
    let data = tx
        .query_one(
            "SELECT platform.community_reply_json($1,$2)",
            &[&r, &a.user_id],
        )
        .await?
        .get(0);
    tx.commit().await?;
    Ok(Json(data))
}
async fn remove_reply(
    State(s): State<AppState>,
    h: HeaderMap,
    Path((id, r)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    let a = mutable_actor(&s, &h).await?;
    let r = reply_id(&r)?;
    let mut db = s.db.client().await?;
    let tx = db.transaction().await?;
    locked(&tx, &id).await?;
    let row=tx.query_opt("SELECT author_id,version,state FROM platform.community_replies WHERE thread_id=$1 AND id=$2 FOR UPDATE",&[&id,&r]).await?.ok_or_else(||ApiError::not_found("Reply not found"))?;
    let owner: String = row.get(0);
    authority(&a, &owner, "remove")?;
    if !row.get::<_, String>(2).starts_with("deleted_") {
        let state = if owner == a.user_id {
            "deleted_by_user"
        } else {
            "deleted_by_moderator"
        };
        let version: i32 = row.get(1);
        tx.execute("UPDATE platform.community_replies SET state=$3,version=version+1,updated_at=clock_timestamp() WHERE thread_id=$1 AND id=$2",&[&id,&r,&state]).await?;
        tx.execute(
            "DELETE FROM platform.community_reply_votes WHERE reply_id=$1",
            &[&r],
        )
        .await?;
        audit(&tx, &a, &id, r, "remove", Some(version), Some(version + 1)).await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn delete_reply(
    State(s): State<AppState>,
    h: HeaderMap,
    Path((id, r)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    let a = mutable_actor(&s, &h).await?;
    if !a.is_moderator() {
        return Err(ApiError::forbidden(
            "Moderator deletion permission required",
        ));
    }
    a.require_permission("reply.delete")?;
    let r = reply_id(&r)?;
    let mut db = s.db.client().await?;
    let tx = db.transaction().await?;
    locked(&tx, &id).await?;
    let row = tx
        .query_opt(
            "DELETE FROM platform.community_replies WHERE thread_id=$1 AND id=$2 RETURNING version,reply_number",
            &[&id, &r],
        )
        .await?
        .ok_or_else(|| ApiError::not_found("Reply not found"))?;
    // Retained deep-link code computes pages from dense reply ordinals.
    let removed_number: i32 = row.get(1);
    tx.execute("UPDATE platform.community_replies SET reply_number=reply_number-1 WHERE thread_id=$1 AND reply_number>$2",&[&id,&removed_number]).await?;
    tx.execute("UPDATE platform.catalog_threads SET next_reply_number=next_reply_number-1,updated_at=clock_timestamp() WHERE id=$1",&[&id]).await?;
    audit(&tx, &a, &id, r, "delete", Some(row.get(0)), None).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn reply_vote(
    State(s): State<AppState>,
    h: HeaderMap,
    Path((id, r)): Path<(String, String)>,
    Json(p): Json<Value>,
) -> ApiResult<StatusCode> {
    let a = mutable_actor(&s, &h).await?;
    a.require_permission("reply.create")?;
    let r = reply_id(&r)?;
    let vote = vote(&p)?;
    let mut db = s.db.client().await?;
    let tx = db.transaction().await?;
    if locked(&tx, &id).await? {
        return Err(ApiError::forbidden("Thread is locked"));
    }
    let row=tx.query_opt("SELECT author_id,state FROM platform.community_replies WHERE thread_id=$1 AND id=$2 FOR SHARE",&[&id,&r]).await?.ok_or_else(||ApiError::not_found("Reply not found"))?;
    if row.get::<_, String>(0) == a.user_id {
        return Err(ApiError::forbidden("You cannot vote on your own reply"));
    }
    if row.get::<_, String>(1) != "active" {
        return Err(ApiError::conflict("Reply cannot receive votes"));
    }
    tx.execute("INSERT INTO platform.community_reply_votes(reply_id,user_id,vote) VALUES($1,$2,$3) ON CONFLICT(reply_id,user_id) DO UPDATE SET vote=EXCLUDED.vote,updated_at=clock_timestamp()",&[&r,&a.user_id,&vote]).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn reply_unvote(
    State(s): State<AppState>,
    h: HeaderMap,
    Path((id, r)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    let a = mutable_actor(&s, &h).await?;
    a.require_permission("reply.create")?;
    let r = reply_id(&r)?;
    let mut db = s.db.client().await?;
    let tx = db.transaction().await?;
    locked(&tx, &id).await?;
    if tx
        .query_opt(
            "SELECT id FROM platform.community_replies WHERE thread_id=$1 AND id=$2 FOR SHARE",
            &[&id, &r],
        )
        .await?
        .is_none()
    {
        return Err(ApiError::not_found("Reply not found"));
    }
    tx.execute(
        "DELETE FROM platform.community_reply_votes WHERE reply_id=$1 AND user_id=$2",
        &[&r, &a.user_id],
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
