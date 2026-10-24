//! Read-only personal native thread/reply reports, separate from both the public
//! UUID report categories and native-cookie moderation routes.
use super::{PublicApiError, PublicApiResult, PublicUuid, auth::{admitted_request, bound_account_read, BoundAccountRead}, query::PublicQuery};
use crate::{AppState, domains::{catalog::validation as v, library::account_public_id_in, moderation::{self, personal_reports as core}}};
use axum::{Json, Router, body::to_bytes, extract::{OriginalUri, Path, Request, State}, http::{HeaderMap, Uri}, response::{IntoResponse, Response}, routing::get};
use serde_json::json;
use std::time::Duration;

impl moderation::sealed::Sealed for BoundAccountRead {}
impl core::PersonalReportReader for BoundAccountRead {
    fn user_id(&self) -> &str { self.user_id() }
}
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/nostr/account/reports/native", get(list))
        .route("/nostr/account/reports/native/{id}", get(detail))
        .layer(axum::middleware::from_fn(super::native_user_discussion::private_headers))
}
fn query(raw: Option<&str>, id: Option<String>) -> PublicApiResult<(PublicUuid, core::Filters)> {
    let q = PublicQuery::parse_unchecked(raw)?;
    if id.is_some() { q.require_allowed(&["expectedAccountId"])?; }
    else { q.require_allowed(&["expectedAccountId", "page", "category", "status", "objectId", "reasonId"])?; }
    let account = PublicUuid::parse(q.first("expectedAccountId").ok_or_else(|| PublicApiError::bad_request("expectedAccountId is required"))?)?;
    let page = q.bounded_usize("page", 1, 1, core::MAX_PAGE)?;
    if q.first("page").is_some_and(|p| p != page.to_string()) { return Err(PublicApiError::bad_request("Noncanonical report page")); }
    if q.first("category").is_some_and(|v| !["thread", "reply"].contains(&v)) || q.first("status").is_some_and(|v| !["waiting", "accepted", "refused", "autoresolved"].contains(&v)) {
        return Err(PublicApiError::bad_request("Invalid native report filter"));
    }
    for value in [q.first("objectId"), q.first("reasonId"), id.as_deref()].into_iter().flatten() { v::id(value)?; }
    Ok((account, core::Filters {category:q.first("category").map(str::to_owned), status:q.first("status").map(str::to_owned), object_id:q.first("objectId").map(str::to_owned), reason_id:q.first("reasonId").map(str::to_owned), report_id:id, page}))
}
async fn list(State(state): State<AppState>, OriginalUri(uri): OriginalUri, headers: HeaderMap, request: Request) -> PublicApiResult<Response> {
    read(state, uri, headers, request, None).await
}
async fn detail(State(state): State<AppState>, OriginalUri(uri): OriginalUri, Path(id): Path<String>, headers: HeaderMap, request: Request) -> PublicApiResult<Response> {
    read(state, uri, headers, request, Some(id)).await
}
async fn read(state: AppState, uri: Uri, headers: HeaderMap, request: Request, id: Option<String>) -> PublicApiResult<Response> {
    let (expected, filters) = query(uri.query(), id)?;
    let method = request.method().as_str().to_owned();
    let bytes = tokio::time::timeout(Duration::from_secs(5), to_bytes(request.into_body(), 1024)).await
        .map_err(|_| PublicApiError::bad_request("Report read timeout"))?
        .map_err(|_| PublicApiError::bad_request("Report reads accept no body"))?;
    if !bytes.is_empty() { return Err(PublicApiError::bad_request("Report reads accept no body")); }
    let cap = admitted_request(&state, &uri, &headers, &method, &bytes).await?;
    let mut db = state.db.client().await?;
    let tx = db.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).start().await?;
    let actor = bound_account_read(&tx, &cap).await?;
    let account = account_public_id_in(&tx, actor.user_id()).await?;
    // Required stale-account fence, NOT an owner selector. Must precede all
    // private counts, detail lookups and rows, including empty filters/pages.
    if expected.as_str() != account { return Err(PublicApiError::conflict("Account binding changed; reopen report inspection")); }
    let (total, mut data) = core::read(&tx, &actor, &filters).await?;
    let meta = json!({"scope":"instance-account","source":"persisted-native-personal-reports","accountId":account,"nativeSessionAuthority":false});
    let result = if filters.report_id.is_some() {
        let report = data.pop().ok_or_else(PublicApiError::not_found)?;
        json!({"result":"ok","response":"entity","nostr":meta,"data":report})
    } else {
        json!({"result":"ok","response":"collection","nostr":meta,"data":data,"total":total,"limit":core::PAGE_SIZE,"page":filters.page,"offset":(filters.page-1)*core::PAGE_SIZE as usize})
    };
    super::native_user_reports::require_context_budget(&result)?;
    tx.commit().await?;
    Ok(Json(result).into_response())
}
#[cfg(test)]
mod tests {
    use super::*;
    const ACCOUNT: &str = "expectedAccountId=11111111-1111-8111-8111-111111111111";
    #[test]
    fn required_account_and_bounded_native_filters() {
        assert!(query(None, None).is_err());
        for suffix in ["page=0", "page=01", "page=2002", "page=-1", "page=1&page=2", "status=resolved", "category=manga", "objectId=", "reasonId=bad/reason", "scope=all", "owner=Other", "includes[]=user", "expectedAccountId=22222222-2222-8222-8222-222222222222"] {
            assert!(query(Some(&format!("{ACCOUNT}&{suffix}")), None).is_err(), "{suffix}");
        }
        assert!(query(Some(&format!("{ACCOUNT}&page=2001&category=reply&status=accepted&objectId=123&reasonId=reply_spam")), None).is_ok());
        assert!(query(Some(ACCOUNT), Some("report_1".into())).is_ok());
        assert!(query(Some(&format!("{ACCOUNT}&page=1")), Some("report_1".into())).is_err());
        assert!(query(Some(ACCOUNT), Some("bad/id".into())).is_err());
    }
}
