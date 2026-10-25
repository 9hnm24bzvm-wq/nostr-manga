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
pub(crate) mod mutations;
use mutations::{self as core, visible};

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
    core::thread_vote_in(&tx, &a, &id, vote, None).await?;
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
    let mut db = s.db.client().await?;
    let tx = db.transaction().await?;
    core::thread_vote_in(&tx, &a, &id, 0, None).await?;
    tx.commit().await?;
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
    let data = core::create_in(&tx, &a, &id, &body, parent).await?;
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
    let data = core::update_in(&tx, &a, &id, r, &body, version).await?;
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
    core::remove_in(&tx, &a, &id, r, None).await?;
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
    core::delete_in(&tx, &a, &id, r, None).await?;
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
    core::reply_vote_in(&tx, &a, &id, r, vote, None, None).await?;
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
    core::reply_vote_in(&tx, &a, &id, r, 0, None, None).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
