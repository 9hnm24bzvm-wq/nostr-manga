//! Private signed reports for actual stored manga/chapter/group discussions.
//! Shares native moderation policy; cannot report protocol source events.
use super::{
    PublicApiError, PublicApiResult, PublicUuid,
    auth::{admitted_request, bound_account_read, bound_account_write},
    native_discussion::{Kind, target},
    native_user_discussion::private_headers,
    native_user_reports::{body, reasons, report_subject, require_context_budget, validate_target_fields, TargetType},
    query::PublicQuery,
};
use crate::{AppState, domains::{catalog::validation as v, library::account_public_id_in, moderation}};
use axum::{Json, Router, extract::{OriginalUri, Path, Request, State}, http::{HeaderMap, header}, response::{IntoResponse, Response}, routing::{get, post}};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Command {
    expected_account_id: PublicUuid,
    expected_target_id: PublicUuid,
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
        // Typed parsing must see duplicate keys before Value can discard them.
        let mut c: Self = serde_json::from_slice(bytes).map_err(|_| PublicApiError::bad_request("Invalid native discussion report command"))?;
        let shape: Value = serde_json::from_slice(bytes).map_err(|_| PublicApiError::bad_request("Invalid native report JSON"))?;
        if shape.as_object().ok_or_else(|| PublicApiError::bad_request("Report command must be an object"))?.values().any(Value::is_null) {
            return Err(PublicApiError::bad_request("Report fields cannot be null"));
        }
        v::id(&c.expected_thread_id)?; v::id(&c.reason_id)?;
        validate_target_fields(c.target_type, c.reply_id, c.expected_reply_version)?;
        if c.expected_reason_version <= 0 { return Err(PublicApiError::bad_request("Expected positive report reason version")); }
        c.details = c.details.trim().to_owned();
        if c.details.len() > 20_000 || c.details.contains('\0') { return Err(PublicApiError::bad_request("Report details must not exceed 20000 UTF-8 bytes or contain NUL")); }
        Ok(c)
    }
}
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/nostr/{kind}/{id}/discussion/reports/context", get(context))
        .route("/nostr/{kind}/{id}/discussion/reports", post(create))
        .layer(axum::middleware::from_fn(private_headers))
}
fn metadata(account: &str, kind: Kind, target: &PublicUuid, thread: &str) -> Value {
    json!({"scope":"instance-account","source":"persisted-native-discussion-report","identity":"native_catalogue_resource","accountId":account,"targetKind":kind.name(),"targetId":target,"threadId":thread})
}
async fn context(State(state): State<AppState>, OriginalUri(uri): OriginalUri, Path((kind,id)): Path<(String,String)>, headers: HeaderMap, request: Request) -> PublicApiResult<Response> {
    let kind = Kind::parse(&kind)?; let id = PublicUuid::parse(&id)?;
    let q = PublicQuery::parse_unchecked(uri.query())?;
    q.require_allowed(&["expectedAccountId", "expectedThreadId", "targetType", "replyId", "expectedReplyVersion"])?;
    let required = |name| q.first(name).ok_or_else(|| PublicApiError::bad_request(format!("{name} is required")));
    let expected_account = PublicUuid::parse(required("expectedAccountId")?)?;
    let expected_thread = required("expectedThreadId")?; v::id(expected_thread)?;
    let report_kind = TargetType::parse(required("targetType")?)?;
    let reply = q.first("replyId").map(|_| q.bounded_usize("replyId", 1, 1, i32::MAX as usize).map(|v| v as i64)).transpose()?;
    let version = q.first("expectedReplyVersion").map(|_| q.bounded_usize("expectedReplyVersion", 1, 1, i32::MAX as usize).map(|v| v as i32)).transpose()?;
    validate_target_fields(report_kind, reply, version)?;
    let method = request.method().as_str().to_owned(); let bytes = body(request).await?;
    if !bytes.is_empty() { return Err(PublicApiError::bad_request("Report context accepts no body")); }
    let cap = admitted_request(&state, &uri, &headers, &method, &bytes).await?;
    let mut db = state.db.client().await?; let tx = db.transaction().await?;
    let actor = bound_account_read(&tx, &cap).await?;
    let account = account_public_id_in(&tx, actor.user_id()).await?;
    if expected_account.as_str() != account { return Err(PublicApiError::conflict("Account binding changed; refresh before reading")); }
    actor.require_permission("report.create")?;
    let resolved = target(&tx, kind, &id, expected_thread, false).await?;
    let (_, subject) = report_subject(&tx, report_kind, expected_thread, reply, version).await?;
    let reasons = reasons(&tx, report_kind).await?;
    let result = json!({"result":"ok","nostr":metadata(&account,kind,&resolved.preferred_id,expected_thread),"data":{"target":subject,"reasons":reasons}});
    require_context_budget(&result)?;
    tx.commit().await?; Ok(Json(result).into_response())
}
async fn create(State(state): State<AppState>, OriginalUri(uri): OriginalUri, Path((kind,id)): Path<(String,String)>, headers: HeaderMap, request: Request) -> PublicApiResult<Response> {
    let kind = Kind::parse(&kind)?; let id = PublicUuid::parse(&id)?;
    if uri.query().is_some() { return Err(PublicApiError::bad_request("Report writes accept no query")); }
    let method = request.method().as_str().to_owned(); let bytes = body(request).await?;
    let mut content_types = headers.get_all(header::CONTENT_TYPE).iter();
    if !content_types.next().and_then(|v| v.to_str().ok()).is_some_and(|v| v.split(';').next().is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))) || content_types.next().is_some() {
        return Err(PublicApiError::bad_request("JSON content type required"));
    }
    let cap = admitted_request(&state, &uri, &headers, &method, &bytes).await?;
    let c = Command::parse(&bytes)?;
    let mut db = state.db.client().await?; let tx = db.transaction().await?;
    let actor = bound_account_write(&tx, &cap).await?;
    let account = account_public_id_in(&tx, actor.user_id()).await?;
    if c.expected_account_id.as_str() != account { return Err(PublicApiError::conflict("Account binding changed; refresh before reporting")); }
    actor.require_permission("report.create")?;
    // Preserve actor-first lock ordering BEFORE resource locks. A native
    // same-actor mutation may already hold KEY SHARE while owning a target;
    // refuse a contended upgrade instead of waiting for that mutation to abort.
    tx.query_one("SELECT id FROM platform.users WHERE id=$1 FOR UPDATE NOWAIT", &[&actor.user_id()]).await
        .map_err(super::native_user_discussion::report_lock_error)?;
    let resolved = target(&tx, kind, &id, &c.expected_thread_id, false).await?;
    if c.expected_target_id != resolved.preferred_id { return Err(PublicApiError::conflict("Discussion target identity changed; refresh before reporting")); }
    let (subject, _) = report_subject(&tx, c.target_type, &c.expected_thread_id, c.reply_id, c.expected_reply_version).await?;
    let reason = tx.query_opt("SELECT version FROM platform.report_reasons WHERE id=$1 AND category=$2 AND enabled FOR SHARE", &[&c.reason_id, &c.target_type.category()]).await?
        .ok_or_else(|| PublicApiError::conflict("Report reason is no longer available; reload reasons"))?;
    if reason.get::<_, i32>(0) != c.expected_reason_version { return Err(PublicApiError::conflict("Report reason changed; reload reasons")); }
    let report = moderation::create_report(&tx, &actor, c.target_type.category(), &subject, &c.reason_id, &c.details).await?;
    let mut meta = metadata(&account,kind,&resolved.preferred_id,&c.expected_thread_id);
    meta["requestHash"] = json!(hex::encode(Sha256::digest(&bytes))); meta["requestMethod"] = json!(method); meta["requestPath"] = json!(uri.to_string());
    let result = json!({"result":"ok","nostr":meta,"data":{"report":{"id":report,"status":"waiting"}}});
    tx.commit().await?; Ok(Json(result).into_response())
}
#[cfg(test)] mod tests {
    use super::*;
    fn command() -> Value { json!({"expectedAccountId":"11111111-1111-8111-8111-111111111111","expectedTargetId":"22222222-2222-8222-8222-222222222222","expectedThreadId":"ActualThread","targetType":"thread","reasonId":"thread_spam","expectedReasonVersion":1,"details":"\u{2003}native report\u{2003}"}) }
    #[test] fn sealed_shape_rejects_user_source_aliases_and_duplicate_keys() {
        let good=command(); assert_eq!(Command::parse(&serde_json::to_vec(&good).unwrap()).unwrap().details,"native report");
        for field in ["expectedAccountId","expectedTargetId","expectedThreadId","targetType","reasonId","expectedReasonVersion","details"] {
            let mut bad=good.clone(); bad.as_object_mut().unwrap().remove(field); assert!(Command::parse(&serde_json::to_vec(&bad).unwrap()).is_err());
            let mut bad=good.clone(); bad[field]=Value::Null; assert!(Command::parse(&serde_json::to_vec(&bad).unwrap()).is_err());
        }
        for (field,value) in [("expectedUserId",good["expectedTargetId"].clone()),("targetKind",json!("manga")),("targetType",json!("manga")),("replyId",json!(1)),("expectedReplyVersion",json!(1)),("reasonId",json!("invalid/reason")),("expectedReasonVersion",json!(0)),("details",json!("🌌".repeat(5001))),("details",json!("\0"))] {
            let mut bad=good.clone(); bad[field]=value; assert!(Command::parse(&serde_json::to_vec(&bad).unwrap()).is_err(),"{field}");
        }
        let duplicate=serde_json::to_string(&good).unwrap().replacen('{',"{\"targetType\":\"reply\",",1); assert!(Command::parse(duplicate.as_bytes()).is_err());
        let mut reply=good; reply["targetType"]=json!("reply"); reply["replyId"]=json!(1); reply["expectedReplyVersion"]=json!(2); reply["details"]=json!("🌌".repeat(5000));
        assert!(Command::parse(&serde_json::to_vec(&reply).unwrap()).is_ok());
        for field in ["replyId","expectedReplyVersion"] { for value in [Value::Null,json!(0),json!(-1),json!(2147483648_i64),json!("1")] { let mut bad=reply.clone();bad[field]=value;assert!(Command::parse(&serde_json::to_vec(&bad).unwrap()).is_err()); } }
    }
}
