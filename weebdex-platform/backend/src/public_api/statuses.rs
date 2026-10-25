//! Original signed-library status transport, with UUID presentation only.
//! Reading status is never interpreted as a distinct follow flag.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdResolver, PublicIdTarget,
    PublicUuid, ResourceKind, auth, query::PublicQuery, statuses_core::ReadingStatus,
};
use crate::AppState;
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{OriginalUri, Path, Request, State},
    http::HeaderMap,
    routing::get,
};
use manga_node::{
    db::Id,
    http_auth::HttpAuthError,
    library_view::{SnapshotChanged, ViewBudget},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/manga/status", get(all))
        .route("/manga/{p0}/status", get(single))
}
fn error(error: anyhow::Error) -> PublicApiError {
    if error.is::<SnapshotChanged>() {
        return PublicApiError::conflict("The signed public library changed; refresh its head");
    }
    if error.is::<ViewBudget>() {
        return PublicApiError::bad_request("Original reading-status budget exceeded");
    }
    if error.is::<HttpAuthError>() {
        return auth::map_error(error.downcast::<HttpAuthError>().unwrap());
    }
    PublicApiError::unavailable()
}
fn id(value: &str) -> PublicApiResult<Id> {
    if !manga_node::auth::valid_hash(value) {
        return Err(PublicApiError::unavailable());
    }
    hex::decode(value)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or_else(PublicApiError::unavailable)
}
async fn all(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    read(state, uri, headers, request, None).await
}
async fn single(
    State(state): State<AppState>,
    Path(value): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    read(
        state,
        uri,
        headers,
        request,
        Some(PublicUuid::parse(&value)?),
    )
    .await
}
async fn read(
    state: AppState,
    uri: axum::http::Uri,
    headers: HeaderMap,
    request: Request,
    public: Option<PublicUuid>,
) -> PublicApiResult<Json<Value>> {
    let q = PublicQuery::parse_unchecked(uri.query())?;
    q.require_allowed(if public.is_some() {
        &["head"]
    } else {
        &["head", "status"]
    })?;
    let status = q.first("status").map(ReadingStatus::parse).transpose()?;
    if q.first("head")
        .is_some_and(|v| !manga_node::auth::valid_hash(v))
    {
        return Err(PublicApiError::bad_request("Invalid signed library head"));
    }
    let method = request.method().to_string();
    let body = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        to_bytes(request.into_body(), 1),
    )
    .await
    .map_err(|_| PublicApiError::bad_request("Body timeout"))?
    .map_err(|_| PublicApiError::bad_request("Status reads accept no body"))?;
    if !body.is_empty() {
        return Err(PublicApiError::bad_request("Status reads accept no body"));
    }
    let cap = auth::admitted_request(&state, &uri, &headers, &method, &body).await?;
    let resolver = PostgresPublicIdResolver::new(state.db.clone());
    let series = if let Some(public) = &public {
        let target = resolver
            .resolve(ResourceKind::Manga, public)
            .await?
            .filter(|r| r.target == PublicIdTarget::NativeResource)
            .ok_or_else(PublicApiError::not_found)?;
        let row=state.db.client().await?.query_opt("SELECT m.protocol_series_id FROM platform.catalog_manga m WHERE m.id=$1 AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)",&[&target.native_id]).await?.ok_or_else(PublicApiError::not_found)?;
        let source: Option<String> = row.get(0);
        Some(id(&source.ok_or_else(PublicApiError::not_found)?)?)
    } else {
        None
    };
    let status_name = q.first("status");
    let snapshot = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        state
            .protocol
            .public_reading_statuses(&cap, series, status_name, q.first("head")),
    )
    .await
    .map_err(|_| PublicApiError::unavailable())?
    .map_err(error)?;
    let keys = snapshot
        .entries
        .iter()
        .map(|e| hex::encode(e.series))
        .collect::<Vec<_>>();
    let mut projected = BTreeMap::new();
    for batch in keys.chunks(1024) {
        for row in state.db.client().await?.query("SELECT m.protocol_series_id,a.public_id::text FROM platform.catalog_manga m JOIN platform.public_resource_ids a ON a.kind='manga' AND a.native_id=m.id AND a.preferred WHERE m.protocol_series_id=ANY($1::text[]) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)",&[&batch]).await? {let source:String=row.get(0);let uuid:String=row.get(1);projected.insert(source,PublicUuid::parse(&uuid).map_err(|_|PublicApiError::unavailable())?);}
    }
    let mut statuses = BTreeMap::new();
    let mut selected = None;
    for entry in &snapshot.entries {
        let parsed =
            ReadingStatus::parse(&entry.status).map_err(|_| PublicApiError::unavailable())?;
        if status.is_some_and(|status| status != parsed) {
            return Err(PublicApiError::unavailable());
        }
        if let Some(uuid) = projected.get(&hex::encode(entry.series)) {
            statuses.insert(uuid.clone(), parsed);
            if series == Some(entry.series) {
                selected = Some(parsed);
            }
        }
    }
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        state
            .protocol
            .verify_reading_status_snapshot(&cap, &snapshot),
    )
    .await
    .map_err(|_| PublicApiError::unavailable())?
    .map_err(error)?;
    let nostr = json!({"scope":"signed_public_library","head":snapshot.head,"root":snapshot.root.map(hex::encode),"followFlagInferred":false});
    Ok(Json(if public.is_some() {
        json!({"result":"ok","status":selected,"nostr":nostr})
    } else {
        json!({"result":"ok","statuses":statuses,"nostr":nostr})
    }))
}
