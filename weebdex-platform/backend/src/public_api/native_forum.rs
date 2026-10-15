//! Explicit native forum companion. Public UUIDs select actual authorized
//! catalogue records; returned persisted native thread IDs remain a module type.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdResolver, PublicUuid,
    ResolvedPublicId, ResourceKind,
    native_forum_core::{self, ContextRequest, ContextResponse, ForumKind, NativeForumRepository},
};
use crate::AppState;
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{OriginalUri, Request, State},
    routing::post,
};
use serde_json::Value;
use std::collections::BTreeMap;
struct Repository(AppState);
impl NativeForumRepository for Repository {
    async fn resolve_many(
        &self,
        kind: ResourceKind,
        ids: &[PublicUuid],
    ) -> PublicApiResult<BTreeMap<PublicUuid, ResolvedPublicId>> {
        Ok(PostgresPublicIdResolver::new(self.0.db.clone())
            .resolve_many(kind, ids)
            .await?)
    }
    async fn visible_contexts(
        &self,
        kind: ForumKind,
        ids: &[String],
    ) -> PublicApiResult<BTreeMap<String, Option<Value>>> {
        if ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let sql = match kind {
            ForumKind::Manga => {
                "SELECT m.id,platform.catalog_thread_json('manga',m.id) FROM platform.catalog_manga m WHERE m.id=ANY($1::text[]) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)"
            }
            ForumKind::Chapter => {
                "SELECT c.id,platform.catalog_thread_json('chapter',c.id) FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=ANY($1::text[]) AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)"
            }
            ForumKind::Group => {
                "SELECT g.id,platform.catalog_thread_json('group',g.id) FROM platform.catalog_groups g WHERE g.id=ANY($1::text[]) AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id)"
            }
        };
        Ok(self
            .0
            .db
            .client()
            .await?
            .query(sql, &[&ids])
            .await?
            .into_iter()
            .map(|r| (r.get(0), r.get(1)))
            .collect())
    }
}
pub fn router() -> Router<AppState> {
    Router::new().route("/nostr/native-forum/contexts", post(contexts))
}
async fn contexts(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    request: Request,
) -> PublicApiResult<Json<ContextResponse>> {
    if uri.query().is_some() {
        return Err(PublicApiError::bad_request(
            "Forum contexts accept no query parameters",
        ));
    }
    if request
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_none_or(|v| !v.trim().eq_ignore_ascii_case("application/json"))
    {
        return Err(PublicApiError::bad_request(
            "Content-Type must be application/json",
        ));
    }
    let body = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        to_bytes(request.into_body(), 16 * 1024),
    )
    .await
    .map_err(|_| PublicApiError::bad_request("Body timeout"))?
    .map_err(|_| PublicApiError::bad_request("Body exceeds 16KiB"))?;
    let payload: ContextRequest = serde_json::from_slice(&body)
        .map_err(|_| PublicApiError::bad_request("Invalid forum context request"))?;
    Ok(Json(
        native_forum_core::contexts(&Repository(state), payload).await?,
    ))
}
