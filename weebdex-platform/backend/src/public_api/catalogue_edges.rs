//! Public read-only relations and recommendations over persisted catalogue edges.
//! Source/target visibility is rechecked in SQL; aliases never confer authority.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdResolver, ResourceKind,
    catalogue::{MANGA_VISIBLE, resolve},
    catalogue_dto::AliasIndex,
    dto::{CollectionResponse, Relationship, Resource, ResourceType},
    query::{PublicQuery, QuerySchema, SortDirection},
};
use crate::{AppState, domains::catalog::query::Sql};
use axum::{
    Json, Router,
    extract::{Path, RawQuery, State},
    routing::get,
};
use serde_json::Value;
use std::collections::BTreeSet;

const MAX_RELATIONS: usize = 4096;
const MAX_RECOMMENDATIONS: usize = 20;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/manga/{p0}/relation", get(relations))
        .route("/manga/{p0}/recommendation", get(recommendations))
}

fn edge_query(raw: Option<&str>, recommendations: bool) -> PublicApiResult<PublicQuery> {
    // These operations have no public pagination parameters in the upstream API.
    // Reject even zero-valued limit/offset rather than silently ignoring them.
    let unchecked = PublicQuery::parse_unchecked(raw)?;
    unchecked.require_allowed(if recommendations {
        &["includes[]", "contentRating[]", "order[score]"]
    } else {
        &["includes[]"]
    })?;
    let query = PublicQuery::parse(
        raw,
        &QuerySchema {
            default_limit: 0,
            max_limit: 0,
            max_offset: 0,
            scalar: &[],
            arrays: if recommendations { &["contentRating[]"] } else { &[] },
            includes: &["manga"],
            order: if recommendations { &["score"] } else { &[] },
        },
    )?;
    if query.list("contentRating[]").iter().any(|rating| {
        !["safe", "suggestive", "erotica", "pornographic"].contains(&rating.as_str())
    }) {
        return Err(PublicApiError::bad_request("Invalid content rating"));
    }
    Ok(query)
}

async fn relations(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> PublicApiResult<Json<CollectionResponse<Resource>>> {
    edges(state, &id, raw.as_deref(), false).await
}

async fn recommendations(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> PublicApiResult<Json<CollectionResponse<Resource>>> {
    edges(state, &id, raw.as_deref(), true).await
}

async fn edges(
    state: AppState,
    public: &str,
    raw: Option<&str>,
    recommendations: bool,
) -> PublicApiResult<Json<CollectionResponse<Resource>>> {
    let query = edge_query(raw, recommendations)?;
    let source = resolve(&state, ResourceKind::Manga, public).await?;
    let client = state.db.client().await?;
    if client
        .query_opt(
            &format!("SELECT m.id FROM platform.catalog_manga m WHERE m.id=$1 AND {MANGA_VISIBLE}"),
            &[&source],
        )
        .await?
        .is_none()
    {
        return Err(PublicApiError::not_found());
    }
    let mut sql = Sql::default();
    let source = sql.bind(source);
    sql.conditions.push(format!("root.id={source}"));
    sql.conditions.push(MANGA_VISIBLE.into());
    sql.conditions.push("root.deleted_at IS NULL AND root.state='published' AND root.protocol_source_visible AND platform.protocol_source_is_visible(root.protocol_series_id)".into());
    let (kind, table, target, attributes, order, limit) = if recommendations {
        let mut ratings = query.list("contentRating[]");
        if ratings.is_empty() {
            ratings = vec!["safe".into(), "suggestive".into(), "erotica".into()];
        }
        sql.array("m.content_rating", ratings, false);
        let direction = query.order.first().map(|(_, direction)| *direction)
            .unwrap_or(SortDirection::Desc);
        (
            ResourceKind::MangaRecommendation,
            "platform.catalog_recommendations",
            "recommended_id",
            "jsonb_build_object('score',edge.score)",
            format!("edge.score {} NULLS LAST,m.id ASC", direction.sql()),
            MAX_RECOMMENDATIONS,
        )
    } else {
        (
            ResourceKind::MangaRelation,
            "platform.catalog_relations",
            "related_id",
            // Relation writes version the owner title, not a fictional edge row.
            "jsonb_build_object('relation',edge.relation_type,'version',root.version,'nostr',jsonb_build_object('scope','instance-local','versionScope','owner_manga'))",
            "edge.created_at ASC,m.id ASC".into(),
            MAX_RELATIONS + 1,
        )
    };
    let rows = client.query(
        &format!("SELECT platform.public_manga_edge_key(edge.manga_id,edge.{target}),{attributes},platform.catalog_manga_json(m.id),count(*) OVER() FROM {table} edge JOIN platform.catalog_manga root ON root.id=edge.manga_id JOIN platform.catalog_manga m ON m.id=edge.{target} WHERE {} ORDER BY {order} LIMIT {limit}", sql.predicate()),
        &sql.params(),
    ).await?;
    drop(client);
    if !recommendations && rows.len() > MAX_RELATIONS {
        return Err(PublicApiError::unavailable());
    }
    let keys = rows.iter().map(|row| row.get::<_, String>(0)).collect::<Vec<_>>();
    let resolver = PostgresPublicIdResolver::new(state.db.clone());
    let mut aliases = std::collections::BTreeMap::new();
    for batch in keys.chunks(super::postgres_identity::MAX_PUBLIC_ID_BATCH) {
        aliases.extend(resolver.preferred_many(kind, batch).await?);
    }
    let resources = rows.iter().map(|row| (ResourceKind::Manga, row.get::<_, Value>(2)))
        .collect::<Vec<_>>();
    let index = AliasIndex::load(&state, &resolver, &resources).await?;
    let total = rows.first().map(|row| row.get::<_, i64>(3)).unwrap_or(0);
    let data = rows.iter().zip(resources.iter()).map(|(row, (_, manga))| {
        let key: String = row.get(0);
        let target = index.resource(ResourceKind::Manga, manga, &BTreeSet::new())?;
        Ok(Resource {
            id: aliases.get(&key).cloned().ok_or_else(PublicApiError::unavailable)?,
            resource_type: if recommendations { ResourceType::MangaRecommendation } else { ResourceType::MangaRelation },
            attributes: row.get(1),
            relationships: vec![Relationship {
                id: target.id,
                resource_type: ResourceType::Manga,
                related: None,
                attributes: query.includes.contains("manga").then_some(target.attributes),
            }],
        })
    }).collect::<PublicApiResult<Vec<_>>>()?;
    Ok(Json(CollectionResponse::new(data, if recommendations { MAX_RECOMMENDATIONS as u32 } else { total as u32 }, 0, total as u64)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_queries_use_only_documented_operation_parameters() {
        assert!(edge_query(Some("includes[]=manga"), false).is_ok());
        assert!(edge_query(Some("includes[]=manga&contentRating[]=safe&order[score]=asc"), true).is_ok());
        for query in ["limit=0", "offset=0", "contentRating[]=safe", "order[score]=asc", "includes[]=author"] {
            assert!(edge_query(Some(query), false).is_err(), "{query}");
        }
        for query in ["limit=1", "order[title]=asc", "order[score]=invalid", "contentRating[]=invalid", "includes[]=creator"] {
            assert!(edge_query(Some(query), true).is_err(), "{query}");
        }
    }
}
