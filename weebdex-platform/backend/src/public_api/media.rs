//! Public at-home descriptor over actual native/original media metadata.
//! Existing upload/protocol byte handlers remain the only download authority.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdResolver, PublicIdTarget,
    PublicUuid, ResourceKind,
    at_home_core::{self, ChapterMediaAuthority, StoredChapterMedia, StoredPage},
    query::PublicQuery,
};
use crate::{
    AppState,
    error::{ApiError, ApiResult},
};
use axum::{
    Json, Router,
    extract::{Path, RawQuery, State},
    routing::get,
};
use serde_json::Value;
use std::collections::BTreeSet;
pub fn router() -> Router<AppState> {
    Router::new().route("/at-home/server/{p0}", get(server))
}
struct Authority(AppState);
impl ChapterMediaAuthority for Authority {
    fn public_origin(&self) -> &str {
        &self.0.config.public_origin
    }
    async fn visible_media(
        &self,
        id: PublicUuid,
    ) -> PublicApiResult<(PublicUuid, StoredChapterMedia)> {
        let resolver = PostgresPublicIdResolver::new(self.0.db.clone());
        let target = resolver
            .resolve(ResourceKind::Chapter, &id)
            .await?
            .filter(|r| r.target == PublicIdTarget::NativeResource)
            .ok_or_else(PublicApiError::not_found)?;
        let client = self.0.db.client().await?;
        let row=client.query_opt("SELECT c.metadata,c.protocol_event_id FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=$1 AND c.deleted_at IS NULL AND NOT c.is_unavailable AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(c.protocol_event_id) AND platform.protocol_source_is_visible(m.protocol_series_id)",&[&target.native_id]).await?.ok_or_else(PublicApiError::not_found)?;
        let metadata: Value = row.get(0);
        let source: Option<String> = row.get(1);
        let original = pages(&metadata["data"])?;
        if let Some(event) = &source {
            let entry = crate::protocol::media::visible_chapter_source(&self.0, event).await?;
            let originals = entry.metadata["pages"]
                .as_array()
                .ok_or_else(PublicApiError::unavailable)?;
            if originals.len() != original.len()
                || originals.iter().zip(&original).any(|(descriptor, page)| {
                    descriptor["sha256"].as_str() != Some(page.file_name.as_str())
                })
            {
                return Err(PublicApiError::unavailable());
            }
        }

        let optimized = if metadata["data_optimized"].is_null() {
            None
        } else {
            Some(pages(&metadata["data_optimized"])?)
        };
        // Source originals use the original module's immutable verified descriptors.
        // Local originals and all derivatives must have real persisted media rows.
        let rows=client.query("SELECT p.name,p.derived FROM(SELECT name,FALSE derived FROM platform.chapter_pages WHERE chapter_id=$1 UNION ALL SELECT name,TRUE derived FROM platform.chapter_page_derivatives WHERE chapter_id=$1)p",&[&target.native_id]).await?;
        let existing = rows
            .into_iter()
            .map(|r| (r.get::<_, String>(0), r.get::<_, bool>(1)))
            .collect::<BTreeSet<_>>();
        if source.is_none()
            && original
                .iter()
                .any(|p| !existing.contains(&(p.file_name.clone(), false)))
        {
            return Err(PublicApiError::unavailable());
        }
        if optimized
            .iter()
            .flatten()
            .any(|p| !existing.contains(&(p.file_name.clone(), true)))
        {
            return Err(PublicApiError::unavailable());
        }
        Ok((
            target.preferred_id,
            StoredChapterMedia {
                original,
                optimized,
            },
        ))
    }
}
fn pages(value: &Value) -> PublicApiResult<Vec<StoredPage>> {
    let Some(pages) = value.as_array() else {
        if value.is_null() {
            return Ok(vec![]);
        }
        return Err(PublicApiError::unavailable());
    };
    if pages.len() > 1000 {
        return Err(PublicApiError::unavailable());
    }
    pages
        .iter()
        .map(|page| {
            let file_name = page["name"]
                .as_str()
                .ok_or_else(PublicApiError::unavailable)?
                .to_owned();
            let dimensions = match page.get("dimensions") {
                None | Some(Value::Null) => None,
                Some(d) => {
                    let d = d
                        .as_array()
                        .filter(|d| d.len() == 2)
                        .ok_or_else(PublicApiError::unavailable)?;
                    let w = d[0]
                        .as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .filter(|n| *n > 0)
                        .ok_or_else(PublicApiError::unavailable)?;
                    let h = d[1]
                        .as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .filter(|n| *n > 0)
                        .ok_or_else(PublicApiError::unavailable)?;
                    Some([w, h])
                }
            };
            Ok(StoredPage {
                file_name,
                dimensions,
            })
        })
        .collect()
}
async fn server(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> PublicApiResult<Json<at_home_core::AtHomeResponse>> {
    let q = PublicQuery::parse_unchecked(raw.as_deref())?;
    q.require_allowed(&["forcePort443"])?;
    if q.boolean("forcePort443")?.unwrap_or(false) {
        let origin = url::Url::parse(&state.config.public_origin)
            .map_err(|_| PublicApiError::unavailable())?;
        if origin.scheme() != "https" || origin.port_or_known_default() != Some(443) {
            return Err(PublicApiError::unsupported());
        }
    }
    Ok(Json(
        at_home_core::at_home(&Authority(state), PublicUuid::parse(&id)?).await?,
    ))
}
/// Additive media transport alias boundary. Legacy native IDs and original hashes
/// keep their exact values; public UUIDs are resolved without changing filenames.
pub(crate) async fn native_media_id(
    state: &AppState,
    kind: ResourceKind,
    value: &str,
) -> ApiResult<String> {
    if value.len() != 36 {
        return Ok(value.to_owned());
    }
    let id =
        PublicUuid::parse(value).map_err(|_| ApiError::bad_request("Invalid public media UUID"))?;
    PostgresPublicIdResolver::new(state.db.clone())
        .resolve(kind, &id)
        .await?
        .filter(|r| r.target == PublicIdTarget::NativeResource)
        .map(|r| r.native_id)
        .ok_or_else(|| ApiError::not_found("Media resource not found"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persisted_page_metadata_is_never_synthesized() {
        assert!(pages(&serde_json::json!([{"name":"actual.webp","dimensions":[32,48]}])).is_ok());
        assert!(pages(&serde_json::json!([{"name":"actual.webp","dimensions":[0,48]}])).is_err());
        assert!(pages(&serde_json::json!([{}])).is_err());
    }
}
