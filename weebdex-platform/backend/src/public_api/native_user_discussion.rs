//! Request-bound signed access to a native public user's actual persisted thread.
//! UUID aliases never create identity, thread, session, or mutation authority.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdTarget, PublicUuid,
    ResolvedPublicId, ResourceKind,
    auth::{admitted_request, bound_account_read, bound_account_write},
    native_discussion_common::{Command, body},
    query::PublicQuery,
};
use crate::{
    AppState,
    domains::{catalog::validation as v, community::mutations as core, library::account_public_id_in},
};
use axum::{
    Json, Router,
    extract::{OriginalUri, Path, Request, State},
    http::{HeaderMap, HeaderValue, header},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
};
use sha2::{Digest, Sha256};
use serde_json::{Map, Value, json};
use tokio_postgres::Transaction;

pub fn router() -> Router<AppState> {
    Router::new().route("/nostr/user/{p0}/discussion", get(read).post(write))
        .layer(axum::middleware::from_fn(private_headers))
}
pub(super) async fn private_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    response.headers_mut().append(header::VARY, HeaderValue::from_static("Authorization, Cookie"));
    response
}
fn metadata(account: &str, user: &PublicUuid, thread: &str) -> Value {
    json!({"scope":"instance-account","source":"persisted-native-user-discussion","identity":"native_public_user","accountId":account,"userId":user,"threadId":thread})
}
async fn target(tx: &Transaction<'_>, id: &PublicUuid, expected_thread: &str, write: bool) -> PublicApiResult<(ResolvedPublicId, String)> {
    target_with_policy(tx, id, expected_thread, write, false).await
}
/// Reports upgrade their actor before acquiring resources, so all resource locks
/// must be NOWAIT to avoid cycles with native mutations that audit that actor.
pub(super) async fn report_target(tx: &Transaction<'_>, id: &PublicUuid, expected_thread: &str) -> PublicApiResult<(ResolvedPublicId, String)> {
    target_with_policy(tx, id, expected_thread, false, true).await
}
pub(super) fn report_lock_error(error: tokio_postgres::Error) -> PublicApiError {
    if error.code() == Some(&tokio_postgres::error::SqlState::LOCK_NOT_AVAILABLE) {
        PublicApiError::conflict("Discussion target is being changed; reload before retrying")
    } else { error.into() }
}
async fn target_with_policy(tx: &Transaction<'_>, id: &PublicUuid, expected_thread: &str, write: bool, nowait: bool) -> PublicApiResult<(ResolvedPublicId, String)> {
    v::id(expected_thread)?;
    let resolved = PostgresPublicIdResolver::resolve_many_in(tx, ResourceKind::User, &[id.clone()]).await?.into_values().next()
        .filter(|r| r.target == PublicIdTarget::NativeResource).ok_or_else(PublicApiError::not_found)?;
    // Lock the actual active native target, then its relation. A publisher ID or
    // client-supplied unrelated thread can never be substituted for this join.
    // Never wait for another account's row while holding this actor's lock.
    // Reciprocal operations (including native profile edits) must receive a
    // known pre-mutation conflict, not a deadlock/uncertain server failure.
    tx.query_opt("SELECT id FROM platform.users WHERE id=$1 AND NOT disabled FOR SHARE NOWAIT", &[&resolved.native_id]).await
        .map_err(|error| if error.code() == Some(&tokio_postgres::error::SqlState::LOCK_NOT_AVAILABLE) {
            PublicApiError::conflict("Profile account is being changed; reload before retrying")
        } else { error.into() })?
        .ok_or_else(PublicApiError::not_found)?;
    let sql = if write {
        "SELECT id FROM platform.catalog_threads WHERE entity_type='user' AND entity_id=$1 FOR UPDATE"
    } else if nowait {
        "SELECT id FROM platform.catalog_threads WHERE entity_type='user' AND entity_id=$1 FOR SHARE NOWAIT"
    } else {
        "SELECT id FROM platform.catalog_threads WHERE entity_type='user' AND entity_id=$1 FOR SHARE"
    };
    let thread: String = tx.query_opt(sql, &[&resolved.native_id]).await.map_err(report_lock_error)?.ok_or_else(PublicApiError::not_found)?.get(0);
    if thread != expected_thread { return Err(PublicApiError::conflict("User discussion changed; refresh before continuing")); }
    core::visible(tx, &thread).await?;
    Ok((resolved, thread))
}
async fn read(State(state): State<AppState>, OriginalUri(uri): OriginalUri, Path(id): Path<String>, headers: HeaderMap, request: Request) -> PublicApiResult<Response> {
    let q = PublicQuery::parse_unchecked(uri.query())?;
    q.require_allowed(&["expectedAccountId", "expectedThreadId", "page", "order"])?;
    let expected_account = PublicUuid::parse(q.first("expectedAccountId").ok_or_else(|| PublicApiError::bad_request("expectedAccountId is required"))?)?;
    let expected_thread = q.first("expectedThreadId").ok_or_else(|| PublicApiError::bad_request("expectedThreadId is required"))?;
    let page = q.bounded_usize("page", 1, 1, 1_000_000)? as i64;
    let order = match q.first("order").unwrap_or("desc") { "asc" => "ASC", "desc" => "DESC", _ => return Err(PublicApiError::bad_request("Invalid reply order")) };
    let id = PublicUuid::parse(&id)?;
    let method = request.method().as_str().to_owned();
    let bytes = body(request).await?;
    if !bytes.is_empty() { return Err(PublicApiError::bad_request("Discussion GET accepts no body")); }
    let cap = admitted_request(&state, &uri, &headers, &method, &bytes).await?;
    let mut db = state.db.client().await?;
    let tx = db.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).start().await?;
    let actor = bound_account_read(&tx, &cap).await?;
    let account = account_public_id_in(&tx, actor.user_id()).await?;
    if expected_account.as_str() != account { return Err(PublicApiError::conflict("Account binding changed; refresh before reading")); }
    let (resolved, thread) = target(&tx, &id, expected_thread, false).await?;
    let data = super::native_discussion_common::view(&tx, &actor, &thread, page, order).await?;
    let result = json!({"result":"ok","data":data,"nostr":metadata(&account,&resolved.preferred_id,&thread)});
    tx.commit().await?;
    Ok(Json(result).into_response())
}
async fn write(State(state): State<AppState>, OriginalUri(uri): OriginalUri, Path(id): Path<String>, headers: HeaderMap, request: Request) -> PublicApiResult<Response> {
    if uri.query().is_some() { return Err(PublicApiError::bad_request("Discussion writes accept no query")); }
    let id = PublicUuid::parse(&id)?;
    let method = request.method().as_str().to_owned();
    let bytes = body(request).await?;
    let mut content_types = headers.get_all(header::CONTENT_TYPE).iter();
    if !content_types.next().and_then(|v|v.to_str().ok()).is_some_and(|v|v.split(';').next().is_some_and(|v|v.trim().eq_ignore_ascii_case("application/json"))) || content_types.next().is_some() {
        return Err(PublicApiError::bad_request("JSON content type required"));
    }
    let cap = admitted_request(&state, &uri, &headers, &method, &bytes).await?;
    let c = Command::parse(&bytes)?;
    let mut db = state.db.client().await?; let tx = db.transaction().await?;
    let actor = bound_account_write(&tx, &cap).await?;
    let account = account_public_id_in(&tx, actor.user_id()).await?;
    if c.expected_account_id.as_str() != account { return Err(PublicApiError::conflict("Account binding changed; refresh before submitting")); }
    let (resolved, thread) = target(&tx, &id, &c.expected_thread_id, true).await?;
    if c.expected_user_id.as_ref() != Some(&resolved.preferred_id) { return Err(PublicApiError::conflict("User identity changed; refresh before submitting")); }
    let (reply, changed) = super::native_discussion_common::execute(&tx, &actor, &thread, &c).await?;
    let mut meta = metadata(&account,&resolved.preferred_id,&thread);
    meta["operation"] = json!(c.operation.name()); meta["changed"] = json!(changed);
    meta["requestHash"] = json!(hex::encode(Sha256::digest(&bytes)));
    meta["requestMethod"] = json!(method); meta["requestPath"] = json!(uri.to_string());
    let mut data = Map::new(); if let Some(reply) = reply { data.insert("reply".into(),reply); }
    // Project before commit: a projection failure must still roll back mutation/audit.
    let result = json!({"result":"ok","nostr":meta,"data":data});
    tx.commit().await?;
    Ok(Json(result).into_response())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn command() -> Value { json!({"expectedAccountId":"11111111-1111-8111-8111-111111111111","expectedUserId":"22222222-2222-8222-8222-222222222222","expectedThreadId":"NativeThread","operation":"create","body":" reply "}) }
    #[test] fn exact_command_shape_and_persisted_body_limit() {
        let good = command();
        assert_eq!(Command::parse(&serde_json::to_vec(&good).unwrap()).unwrap().body.unwrap(),"reply");
        for field in ["expectedAccountId","expectedUserId","expectedThreadId","operation","body"] {
            let mut bad=good.clone();bad.as_object_mut().unwrap().remove(field);assert!(Command::parse(&serde_json::to_vec(&bad).unwrap()).is_err());
        }
        for (field,value) in [("replyId",json!(1)),("parentId",Value::Null),("version",json!(1)),("vote",json!(1)),("email",json!("private")),("body",json!("🌌".repeat(12501))),("body",json!("\0")),("body",json!("\u{2003}"))] {
            let mut bad=good.clone();bad[field]=value;assert!(Command::parse(&serde_json::to_vec(&bad).unwrap()).is_err());
        }
        let duplicate=serde_json::to_string(&good).unwrap().replacen('{',"{\"operation\":\"delete\",",1);
        assert!(Command::parse(duplicate.as_bytes()).is_err());
        let mut c=good;c["body"]=json!("🌌".repeat(12500));assert!(Command::parse(&serde_json::to_vec(&c).unwrap()).is_ok());
    }
    #[test] fn each_operation_requires_only_its_capture_fields() {
        for operation in ["update","remove","delete","thread_vote","reply_vote"] {
            let mut c=command();c.as_object_mut().unwrap().remove("body");c["operation"]=json!(operation);
            if operation != "thread_vote" { c["replyId"]=json!(1);c["version"]=json!(2); }
            if operation == "update" { c["body"]=json!("edited"); }
            if operation.ends_with("vote") { c["vote"]=json!(1);c["expectedVote"]=json!(0); }
            assert!(Command::parse(&serde_json::to_vec(&c).unwrap()).is_ok(),"{c}");
            for field in ["replyId","version","body","vote","expectedVote"] {
                if c.get(field).is_some() {let mut missing=c.clone();missing.as_object_mut().unwrap().remove(field);assert!(Command::parse(&serde_json::to_vec(&missing).unwrap()).is_err(),"{missing}");}
            }
        }
    }
}
