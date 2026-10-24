//! Request-bound reports of an actual native public user's persisted discussion.
//! Reports and audit remain private local moderation state, never protocol events.
use super::{
    PublicApiError, PublicApiResult, PublicUuid,
    auth::{admitted_request, bound_account_read, bound_account_write},
    native_user_discussion::{private_headers, report_target},
    query::PublicQuery,
};
use crate::{AppState, domains::{catalog::validation as v, library::account_public_id_in, moderation}};
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{OriginalUri, Path, Request, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{io::{self, Write}, time::Duration};
use tokio_postgres::Transaction;

pub(crate) const BODY_LIMIT: usize = 131_072;
const MAX_CONTEXT_BYTES: usize = 1024 * 1024;
/// Count actual JSON escaping without allocating an oversized response buffer.
struct ContextByteCounter(usize);
impl Write for ContextByteCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self.0.checked_add(bytes.len()).filter(|n| *n <= MAX_CONTEXT_BYTES)
            .ok_or_else(|| io::Error::other("Native report context exceeds response budget"))?;
        self.0 = next;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
pub(crate) fn require_context_budget(value: &Value) -> PublicApiResult<()> {
    serde_json::to_writer(ContextByteCounter(0), value).map_err(|_| PublicApiError::unavailable())
}
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TargetType { Thread, Reply }
impl TargetType {
    pub(crate) fn category(self) -> &'static str { match self { Self::Thread => "thread", Self::Reply => "reply" } }
    pub(crate) fn parse(value: &str) -> PublicApiResult<Self> {
        match value { "thread" => Ok(Self::Thread), "reply" => Ok(Self::Reply), _ => Err(PublicApiError::bad_request("Invalid report target type")) }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Command {
    expected_account_id: PublicUuid,
    expected_user_id: PublicUuid,
    expected_thread_id: String,
    target_type: TargetType,
    reply_id: Option<i64>,
    expected_reply_version: Option<i32>,
    reason_id: String,
    expected_reason_version: i32,
    details: String,
}
impl Command {
    fn parse(bytes: &[u8]) -> PublicApiResult<Self> {
        // Typed deserialization detects duplicate keys before Value loses them.
        let mut command: Self = serde_json::from_slice(bytes).map_err(|_| PublicApiError::bad_request("Invalid native report command"))?;
        let shape: Value = serde_json::from_slice(bytes).map_err(|_| PublicApiError::bad_request("Invalid native report JSON"))?;
        if shape.as_object().ok_or_else(|| PublicApiError::bad_request("Report command must be an object"))?.values().any(Value::is_null) {
            return Err(PublicApiError::bad_request("Report fields cannot be null"));
        }
        v::id(&command.expected_thread_id)?;
        v::id(&command.reason_id)?;
        validate_target_fields(command.target_type, command.reply_id, command.expected_reply_version)?;
        if command.expected_reason_version <= 0 { return Err(PublicApiError::bad_request("Expected positive report reason version")); }
        // Match native Unicode trimming, then enforce the persisted octet limit.
        command.details = command.details.trim().to_owned();
        if command.details.len() > 20_000 || command.details.contains('\0') {
            return Err(PublicApiError::bad_request("Report details must not exceed 20000 UTF-8 bytes or contain NUL"));
        }
        Ok(command)
    }
}
pub(crate) fn validate_target_fields(kind: TargetType, reply: Option<i64>, version: Option<i32>) -> PublicApiResult<()> {
    let is_reply = kind == TargetType::Reply;
    if reply.is_some() != is_reply || version.is_some() != is_reply
        || reply.is_some_and(|value| value <= 0 || value > i32::MAX as i64)
        || version.is_some_and(|value| value <= 0) {
        return Err(PublicApiError::bad_request("Missing, invalid or irrelevant report target fields"));
    }
    Ok(())
}
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/nostr/user/{p0}/discussion/reports/context", get(context))
        .route("/nostr/user/{p0}/discussion/reports", post(create))
        .layer(axum::middleware::from_fn(private_headers))
}
pub(crate) async fn body(request: Request) -> PublicApiResult<axum::body::Bytes> {
    tokio::time::timeout(Duration::from_secs(5), to_bytes(request.into_body(), BODY_LIMIT)).await
        .map_err(|_| PublicApiError::bad_request("Report body timeout"))?
        .map_err(|_| PublicApiError::bad_request("Report body exceeds 131072 bytes"))
}
fn metadata(account: &str, user: &PublicUuid, thread: &str) -> Value {
    json!({"scope":"instance-account","source":"persisted-native-user-report","identity":"native_public_user","accountId":account,"userId":user,"threadId":thread})
}
pub(crate) async fn report_subject(tx: &Transaction<'_>, kind: TargetType, thread: &str, reply: Option<i64>, expected_version: Option<i32>) -> PublicApiResult<(String, Value)> {
    if kind == TargetType::Thread {
        return Ok((thread.to_owned(), json!({"type":"thread","threadId":thread})));
    }
    let reply = reply.ok_or_else(|| PublicApiError::bad_request("replyId is required"))?;
    // Lock after the actor and parent thread. Never wait while owning the actor
    // upgrade: native cookie edits may need that actor's FK KEY SHARE to audit.
    let row = tx.query_opt("SELECT version,state FROM platform.community_replies WHERE id=$1 AND thread_id=$2 FOR SHARE NOWAIT", &[&reply, &thread]).await
        .map_err(super::native_user_discussion::report_lock_error)?
        .ok_or_else(PublicApiError::not_found)?;
    if row.get::<_, String>(1) != "active" { return Err(PublicApiError::not_found()); }
    let version: i32 = row.get(0);
    if Some(version) != expected_version { return Err(PublicApiError::conflict("Reply changed; reload before reporting")); }
    Ok((reply.to_string(), json!({"type":"reply","threadId":thread,"replyId":reply,"version":version})))
}
/// Shared bounded native thread/reply reason catalogue. No source-report scope.
pub(crate) async fn reasons(tx: &Transaction<'_>, kind: TargetType) -> PublicApiResult<Vec<Value>> {
    // Fail closed rather than truncate rows or text. SQL guards both unbounded
    // TEXT columns before transfer/materialization (<=1001 * (8000+128) text
    // bytes plus fixed fields), including sorting by their bounded projections.
    let rows = tx.query("SELECT CASE WHEN octet_length(id) BETWEEN 1 AND 128 THEN id END AS bounded_id,version,CASE WHEN octet_length(reason) BETWEEN 1 AND 8000 THEN reason END AS bounded_reason,details_required FROM platform.report_reasons WHERE category=$1 AND enabled ORDER BY bounded_reason,bounded_id LIMIT 1001 FOR SHARE", &[&kind.category()]).await?;
    if rows.len() > 1000 { return Err(PublicApiError::unavailable()); }
    let mut reasons = Vec::with_capacity(rows.len());
    for row in rows {
        let id: String = row.get::<_,Option<String>>(0).ok_or_else(PublicApiError::unavailable)?;
        v::id(&id).map_err(|_| PublicApiError::unavailable())?;
        let reason: String = row.get::<_,Option<String>>(2).ok_or_else(PublicApiError::unavailable)?;
        reasons.push(json!({"id":id,"version":row.get::<_,i32>(1),"reason":reason,"detailsRequired":row.get::<_,bool>(3)}));
    }
    Ok(reasons)
}
async fn context(State(state): State<AppState>, OriginalUri(uri): OriginalUri, Path(id): Path<String>, headers: HeaderMap, request: Request) -> PublicApiResult<Response> {
    let q = PublicQuery::parse_unchecked(uri.query())?;
    q.require_allowed(&["expectedAccountId", "expectedThreadId", "targetType", "replyId", "expectedReplyVersion"])?;
    let required = |name| q.first(name).ok_or_else(|| PublicApiError::bad_request(format!("{name} is required")));
    let expected_account = PublicUuid::parse(required("expectedAccountId")?)?;
    let expected_thread = required("expectedThreadId")?;
    v::id(expected_thread)?;
    let kind = TargetType::parse(required("targetType")?)?;
    let reply = q.first("replyId").map(|_| q.bounded_usize("replyId", 1, 1, i32::MAX as usize).map(|v| v as i64)).transpose()?;
    let version = q.first("expectedReplyVersion").map(|_| q.bounded_usize("expectedReplyVersion", 1, 1, i32::MAX as usize).map(|v| v as i32)).transpose()?;
    validate_target_fields(kind, reply, version)?;
    let id = PublicUuid::parse(&id)?;
    let method = request.method().as_str().to_owned();
    let bytes = body(request).await?;
    if !bytes.is_empty() { return Err(PublicApiError::bad_request("Report context accepts no body")); }
    let cap = admitted_request(&state, &uri, &headers, &method, &bytes).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = bound_account_read(&tx, &cap).await?;
    let account = account_public_id_in(&tx, actor.user_id()).await?;
    if expected_account.as_str() != account { return Err(PublicApiError::conflict("Account binding changed; refresh before reading")); }
    actor.require_permission("report.create")?;
    let (resolved, thread) = report_target(&tx, &id, expected_thread).await?;
    let (_, target) = report_subject(&tx, kind, &thread, reply, version).await?;
    let reasons = reasons(&tx, kind).await?;
    let result = json!({"result":"ok","nostr":metadata(&account,&resolved.preferred_id,&thread),"data":{"target":target,"reasons":reasons}});
    // Check the complete escaped wire size before Json allocates its body.
    require_context_budget(&result)?;
    tx.commit().await?;
    Ok(Json(result).into_response())
}
async fn create(State(state): State<AppState>, OriginalUri(uri): OriginalUri, Path(id): Path<String>, headers: HeaderMap, request: Request) -> PublicApiResult<Response> {
    if uri.query().is_some() { return Err(PublicApiError::bad_request("Report writes accept no query")); }
    let id = PublicUuid::parse(&id)?;
    let method = request.method().as_str().to_owned();
    let bytes = body(request).await?;
    let mut content_types = headers.get_all(header::CONTENT_TYPE).iter();
    if !content_types.next().and_then(|v| v.to_str().ok()).is_some_and(|v| v.split(';').next().is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))) || content_types.next().is_some() {
        return Err(PublicApiError::bad_request("JSON content type required"));
    }
    let cap = admitted_request(&state, &uri, &headers, &method, &bytes).await?;
    let command = Command::parse(&bytes)?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = bound_account_write(&tx, &cap).await?;
    let account = account_public_id_in(&tx, actor.user_id()).await?;
    if command.expected_account_id.as_str() != account { return Err(PublicApiError::conflict("Account binding changed; refresh before reporting")); }
    actor.require_permission("report.create")?;
    // Preserve actor-first lock ordering BEFORE resource locks. A native
    // same-actor mutation may already hold KEY SHARE while owning a target;
    // refuse a contended upgrade instead of waiting for that mutation to abort.
    tx.query_one("SELECT id FROM platform.users WHERE id=$1 FOR UPDATE NOWAIT", &[&actor.user_id()]).await
        .map_err(super::native_user_discussion::report_lock_error)?;
    let (resolved, thread) = report_target(&tx, &id, &command.expected_thread_id).await?;
    if command.expected_user_id != resolved.preferred_id { return Err(PublicApiError::conflict("User identity changed; refresh before reporting")); }
    let (target, _) = report_subject(&tx, command.target_type, &thread, command.reply_id, command.expected_reply_version).await?;
    // Hold the exact reviewed native reason version until report and audit commit.
    let reason = tx.query_opt("SELECT version FROM platform.report_reasons WHERE id=$1 AND category=$2 AND enabled FOR SHARE", &[&command.reason_id, &command.target_type.category()]).await?
        .ok_or_else(|| PublicApiError::conflict("Report reason is no longer available; reload reasons"))?;
    if reason.get::<_, i32>(0) != command.expected_reason_version { return Err(PublicApiError::conflict("Report reason changed; reload reasons")); }
    let report = moderation::create_report(&tx, &actor, command.target_type.category(), &target, &command.reason_id, &command.details).await?;
    let mut meta = metadata(&account, &resolved.preferred_id, &thread);
    meta["requestHash"] = json!(hex::encode(Sha256::digest(&bytes)));
    meta["requestMethod"] = json!(method);
    meta["requestPath"] = json!(uri.to_string());
    let result = json!({"result":"ok","nostr":meta,"data":{"report":{"id":report,"status":"waiting"}}});
    tx.commit().await?;
    Ok(Json(result).into_response())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn command() -> Value { json!({"expectedAccountId":"11111111-1111-8111-8111-111111111111","expectedUserId":"22222222-2222-8222-8222-222222222222","expectedThreadId":"NativeThread","targetType":"thread","reasonId":"thread_spam","expectedReasonVersion":1,"details":"\u{2003}native report\u{2003}"}) }
    #[test]
    fn strict_report_shape_native_trim_and_octet_limit() {
        let good = command();
        assert_eq!(Command::parse(&serde_json::to_vec(&good).unwrap()).unwrap().details,"native report");
        for field in ["expectedAccountId","expectedUserId","expectedThreadId","targetType","reasonId","expectedReasonVersion","details"] {
            let mut bad = good.clone(); bad.as_object_mut().unwrap().remove(field);
            assert!(Command::parse(&serde_json::to_vec(&bad).unwrap()).is_err());
            let mut bad = good.clone(); bad[field] = Value::Null;
            assert!(Command::parse(&serde_json::to_vec(&bad).unwrap()).is_err());
        }
        for (field, value) in [("replyId",json!(1)),("expectedReplyVersion",json!(1)),("category",json!("thread")),("reasonId",json!("bad/reason")),("expectedReasonVersion",json!(0)),("details",json!("🌌".repeat(5001))),("details",json!("\0"))] {
            let mut bad = good.clone(); bad[field] = value;
            assert!(Command::parse(&serde_json::to_vec(&bad).unwrap()).is_err(),"{field}");
        }
        let duplicate = serde_json::to_string(&good).unwrap().replacen('{',"{\"targetType\":\"reply\",",1);
        assert!(Command::parse(duplicate.as_bytes()).is_err());
        let mut edge = good; edge["details"] = json!(format!("\u{2003}{}\u{2003}","🌌".repeat(5000)));
        assert!(Command::parse(&serde_json::to_vec(&edge).unwrap()).is_ok());
    }
    #[test]
    fn context_counter_bounds_exact_escaped_wire_bytes() {
        let exact = json!("x".repeat(MAX_CONTEXT_BYTES - 2));
        assert!(require_context_budget(&exact).is_ok());
        assert!(require_context_budget(&json!("x".repeat(MAX_CONTEXT_BYTES - 1))).is_err());
        let escaped_count = (MAX_CONTEXT_BYTES - 2) / 6;
        assert!(require_context_budget(&json!("\u{1}".repeat(escaped_count))).is_ok());
        assert!(require_context_budget(&json!("\u{1}".repeat(escaped_count + 1))).is_err());
        let multibyte_count = (MAX_CONTEXT_BYTES - 2) / 4;
        assert!(require_context_budget(&json!("🌌".repeat(multibyte_count))).is_ok());
        assert!(require_context_budget(&json!("🌌".repeat(multibyte_count + 1))).is_err());
        let mut counter = ContextByteCounter(MAX_CONTEXT_BYTES);
        assert_eq!(counter.write(b"").unwrap(), 0);
        assert!(counter.write(b"x").is_err());
        assert_eq!(counter.0, MAX_CONTEXT_BYTES);
    }
    #[test]
    fn reply_requires_only_exact_positive_captures() {
        let mut reply = command(); reply["targetType"] = json!("reply"); reply["replyId"] = json!(1); reply["expectedReplyVersion"] = json!(2); reply["reasonId"] = json!("reply_spam");
        assert!(Command::parse(&serde_json::to_vec(&reply).unwrap()).is_ok());
        for field in ["replyId","expectedReplyVersion"] {
            let mut bad = reply.clone(); bad.as_object_mut().unwrap().remove(field);
            assert!(Command::parse(&serde_json::to_vec(&bad).unwrap()).is_err());
            for value in [Value::Null,json!(0),json!(-1),json!(2147483648_i64),json!("1")] {
                let mut bad = reply.clone(); bad[field] = value;
                assert!(Command::parse(&serde_json::to_vec(&bad).unwrap()).is_err());
            }
        }
    }
}
