//! RECONSTRUCTED-UNVERIFIED after environment replacement on 2026-10-15.
//! Read-only MangaDex statistics over authoritative native public visibility.
//!
//! MangaDex Forums IDs and per-rater records do not exist in the native model.
//! Comments are therefore null and rating is absent, rather than inventing a
//! forum ID, zero observations, a distribution, or a Bayesian rating. The pinned
//! coverage matrix explicitly records these remaining integration gaps.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdResolver, PublicUuid,
    ResourceKind,
};
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, RawQuery, State},
    routing::get,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_STATISTICS_BATCH: usize = 100;
const MAX_STATISTICS_QUERY_BYTES: usize = 32_768;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/statistics/manga/{uuid}", get(manga_one))
        .route("/statistics/manga", get(manga_many))
        .route("/statistics/chapter/{uuid}", get(chapter_one))
        .route("/statistics/chapter", get(chapter_many))
        .route("/statistics/group/{uuid}", get(group_one))
        .route("/statistics/group", get(group_many))
        .method_not_allowed_fallback(method_not_allowed)
}

async fn method_not_allowed() -> PublicApiError {
    PublicApiError {
        status: axum::http::StatusCode::METHOD_NOT_ALLOWED,
        title: "Method not allowed",
        detail: "This statistics endpoint only supports GET".into(),
    }
}

#[derive(Serialize)]
struct StatisticsResponse {
    result: &'static str,
    statistics: BTreeMap<PublicUuid, Statistics>,
}
#[derive(Serialize)]
#[serde(untagged)]
enum Statistics {
    Manga(MangaStatistics),
    Comments(CommentStatistics),
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MangaStatistics {
    // None means no backing MangaDex Forums integration. Native discussions
    // remain available at their own existing API and retain their text IDs.
    comments: Option<()>,
    follows: i64,
    unavailable_chapter_count: i64,
}
#[derive(Serialize)]
struct CommentStatistics {
    comments: Option<()>,
}

macro_rules! handlers {
    ($one:ident, $many:ident, $kind:ident, $field:literal) => {
        async fn $one(
            State(state): State<AppState>,
            path: Result<Path<String>, axum::extract::rejection::PathRejection>,
            RawQuery(raw): RawQuery,
        ) -> PublicApiResult<Json<StatisticsResponse>> {
            if raw.as_deref().is_some_and(|query| !query.is_empty()) {
                return Err(PublicApiError::bad_request(
                    "This statistics endpoint has no query parameters",
                ));
            }
            let Path(id) =
                path.map_err(|_| PublicApiError::bad_request("A valid UUID path is required"))?;
            let id = PublicUuid::parse(&id)?;
            let response = statistics(&state, ResourceKind::$kind, &[id]).await?;
            if response.statistics.is_empty() {
                return Err(PublicApiError::not_found());
            }
            Ok(Json(response))
        }
        async fn $many(
            State(state): State<AppState>,
            RawQuery(raw): RawQuery,
        ) -> PublicApiResult<Json<StatisticsResponse>> {
            let ids = batch_ids(raw.as_deref(), $field)?;
            Ok(Json(statistics(&state, ResourceKind::$kind, &ids).await?))
        }
    };
}
handlers!(manga_one, manga_many, Manga, "manga[]");
handlers!(chapter_one, chapter_many, Chapter, "chapter[]");
handlers!(group_one, group_many, Group, "group[]");

fn batch_ids(raw: Option<&str>, field: &str) -> PublicApiResult<Vec<PublicUuid>> {
    let raw = raw.unwrap_or_default();
    if raw.len() > MAX_STATISTICS_QUERY_BYTES || !valid_percent_escapes(raw.as_bytes()) {
        return Err(PublicApiError::bad_request(
            "Invalid or over-budget query encoding",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut count = 0;
    for (key, value) in url::form_urlencoded::parse(raw.as_bytes()) {
        count += 1;
        if key != field || count > MAX_STATISTICS_BATCH {
            return Err(PublicApiError::bad_request(format!(
                "Statistics requires only 1 to {MAX_STATISTICS_BATCH} {field} UUID values"
            )));
        }
        ids.insert(PublicUuid::parse(&value)?);
    }
    if count == 0 {
        return Err(PublicApiError::bad_request(format!(
            "At least one {field} UUID is required"
        )));
    }
    Ok(ids.into_iter().collect())
}

fn valid_percent_escapes(raw: &[u8]) -> bool {
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'%' {
            if index + 2 >= raw.len()
                || !raw[index + 1].is_ascii_hexdigit()
                || !raw[index + 2].is_ascii_hexdigit()
            {
                return false;
            }
            index += 3;
        } else {
            index += 1;
        }
    }
    true
}

async fn statistics(
    state: &AppState,
    kind: ResourceKind,
    ids: &[PublicUuid],
) -> PublicApiResult<StatisticsResponse> {
    // Exactly one bounded resolver query and one set-based visibility/count
    // query, independent of the batch size. Reads never manufacture aliases.
    let aliases = PostgresPublicIdResolver::new(state.db.clone())
        .resolve_many(kind, ids)
        .await?;
    let targets: BTreeMap<String, PublicUuid> = aliases
        .into_values()
        .map(|resolved| (resolved.native_id, resolved.preferred_id))
        .collect();
    let native_ids: Vec<String> = targets.keys().cloned().collect();
    let mut output = BTreeMap::new();
    if !native_ids.is_empty() {
        let sql = match kind {
            ResourceKind::Manga => MANGA_STATISTICS_SQL,
            ResourceKind::Chapter => CHAPTER_STATISTICS_SQL,
            ResourceKind::Group => GROUP_STATISTICS_SQL,
            _ => unreachable!("statistics routes only select supported resource kinds"),
        };
        let rows = state
            .db
            .client()
            .await?
            .query(sql, &[&native_ids])
            .await
            .map_err(ApiError::from)?;
        for row in rows {
            let native_id: String = row.get(0);
            let Some(id) = targets.get(&native_id) else {
                continue;
            };
            let value = if kind == ResourceKind::Manga {
                Statistics::Manga(MangaStatistics {
                    comments: None,
                    follows: row.get(1),
                    unavailable_chapter_count: row.get(2),
                })
            } else {
                Statistics::Comments(CommentStatistics { comments: None })
            };
            output.insert(id.clone(), value);
        }
    }
    Ok(StatisticsResponse {
        result: "ok",
        statistics: output,
    })
}

// These visibility predicates mirror CatalogRepository public reads, including
// cached source withdrawal AND the live original-module visibility authority.
// Aggregate only requested rows. Private follow ownership/status are never read
// or returned; the count is the same public aggregate as native analytics.
const MANGA_STATISTICS_SQL: &str = r#"
WITH visible AS MATERIALIZED (
    SELECT m.id FROM platform.catalog_manga m
    WHERE m.id=ANY($1::text[]) AND m.deleted_at IS NULL AND m.state='published'
      AND m.protocol_source_visible
      AND platform.protocol_source_is_visible(m.protocol_series_id)
), follows AS (
    SELECT f.manga_id,count(*) AS total FROM platform.library_manga_follows f
    JOIN visible v ON v.id=f.manga_id GROUP BY f.manga_id
), unavailable AS (
    SELECT c.manga_id,count(*) AS total FROM platform.catalog_chapters c
    JOIN visible v ON v.id=c.manga_id
    WHERE c.deleted_at IS NULL AND c.is_unavailable
      AND platform.protocol_source_is_visible(c.protocol_event_id)
    GROUP BY c.manga_id
)
SELECT v.id,COALESCE(f.total,0)::bigint,COALESCE(u.total,0)::bigint
FROM visible v LEFT JOIN follows f ON f.manga_id=v.id
LEFT JOIN unavailable u ON u.manga_id=v.id
"#;
const CHAPTER_STATISTICS_SQL: &str = r#"
SELECT c.id FROM platform.catalog_chapters c
JOIN platform.catalog_manga m ON m.id=c.manga_id
WHERE c.id=ANY($1::text[]) AND c.deleted_at IS NULL
  AND platform.protocol_source_is_visible(c.protocol_event_id)
  AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible
  AND platform.protocol_source_is_visible(m.protocol_series_id)
"#;
const GROUP_STATISTICS_SQL: &str = r#"
SELECT g.id FROM platform.catalog_groups g
WHERE g.id=ANY($1::text[]) AND g.deleted_at IS NULL
  AND platform.protocol_group_is_visible(g.id)
"#;

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "01234567-89ab-4cde-8f01-23456789abcd";
    #[test]
    fn query_is_bounded_strict_and_canonical() {
        let raw = format!("manga%5B%5D={ID}&manga[]={}", ID.to_ascii_uppercase());
        assert_eq!(batch_ids(Some(&raw), "manga[]").unwrap().len(), 1);
        for raw in [
            "".into(),
            format!("manga={ID}"),
            format!("chapter[]={ID}"),
            format!("manga[]={ID}&limit=0"),
            format!("manga[]={ID}&includes[]=user"),
            "manga[]=not-a-uuid".into(),
            "manga[]=%".into(),
            "manga[]=%FF".into(),
            "manga[]=%00".into(),
            format!("manga[]={ID}&manga[]="),
            std::iter::repeat_n(format!("manga[]={ID}"), 101)
                .collect::<Vec<_>>()
                .join("&"),
            "x".repeat(MAX_STATISTICS_QUERY_BYTES + 1),
        ] {
            assert!(batch_ids(Some(&raw), "manga[]").is_err(), "{raw}");
        }
        let raw = std::iter::repeat_n(format!("manga[]={ID}"), 100)
            .collect::<Vec<_>>()
            .join("&");
        assert!(batch_ids(Some(&raw), "manga[]").is_ok());
    }
}
