//! Public read-only relations and recommendations over persisted catalogue edges.
//! Source/target visibility is rechecked in SQL; aliases never confer authority.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdResolver, PublicIdTarget, PublicUuid, ResourceKind,
    catalogue::{MANGA_VISIBLE, resolve},
    catalogue_dto::AliasIndex,
    dto::{CollectionResponse, Relationship, Resource, ResourceType},
    query::{PublicQuery, QuerySchema, SortDirection},
    relation_paging::{self, RelationPageMeta, RelationPageRequest},
};
use crate::{AppState, domains::catalog::query::Sql};
use axum::{
    Json, Router,
    extract::{Path, RawQuery, State},
    routing::get,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

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
    let q = PublicQuery::parse_unchecked(raw.as_deref())?;
    if ["limit", "offset", "ownerVersion", "snapshot"].iter().any(|key| q.first(key).is_some()) {
        return relation_page(state, &id, raw.as_deref()).await;
    }
    let result = edges(state, &id, raw.as_deref(), false).await?;
    if serde_json::to_vec(&result.0).map_err(|_| PublicApiError::unavailable())?.len() > relation_paging::MAX_RESPONSE_BYTES {
        return Err(PublicApiError::new(axum::http::StatusCode::SERVICE_UNAVAILABLE, "Relation response exceeds transport budget", "Use the explicit bounded relation pagination extension; no result has been silently truncated"));
    }
    Ok(result)
}

/// Optional application extension. Upstream's no-pagination response remains
/// unchanged; callers opt in to bounded complete-resource pages explicitly.
async fn relation_page(state: AppState, public: &str, raw: Option<&str>) -> PublicApiResult<Json<CollectionResponse<Resource>>> {
    let (q, page) = RelationPageRequest::parse(raw, false, &["includes[]"])?;
    let includes = q.includes(&["manga"])?;
    let requested = PublicUuid::parse(public)?;
    let mut client = state.db.client().await?;
    let tx = client.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).read_only(true).start().await?;
    let source = PostgresPublicIdResolver::resolve_many_in(&*tx, ResourceKind::Manga, std::slice::from_ref(&requested)).await?
        .remove(&requested).filter(|row| row.target == PublicIdTarget::NativeResource).ok_or_else(PublicApiError::not_found)?;
    let owner_version: i32 = tx.query_opt(&format!("SELECT m.version FROM platform.catalog_manga m WHERE m.id=$1 AND {MANGA_VISIBLE}"), &[&source.native_id]).await?
        .ok_or_else(PublicApiError::not_found)?.get(0);
    let persisted_total: i64 = tx.query_one("SELECT count(*) FROM platform.catalog_relations WHERE manga_id=$1", &[&source.native_id]).await?.get(0);
    if !(0..=MAX_RELATIONS as i64).contains(&persisted_total) { return Err(PublicApiError::unavailable()); }
    // The complete lightweight ordered eligibility set binds every page. Only
    // selected targets execute the full native metadata projection.
    let rows = tx.query(&format!("SELECT platform.public_manga_edge_key(r.manga_id,r.related_id),r.relation_type,m.id,m.version,a.public_id::text,t.public_id::text FROM platform.catalog_relations r JOIN platform.catalog_manga m ON m.id=r.related_id LEFT JOIN platform.public_resource_ids a ON a.kind='manga_relation' AND a.native_id=platform.public_manga_edge_key(r.manga_id,r.related_id) AND a.preferred LEFT JOIN platform.public_resource_ids t ON t.kind='manga' AND t.native_id=m.id AND t.preferred WHERE r.manga_id=$1 AND {MANGA_VISIBLE} ORDER BY r.created_at ASC,m.id ASC LIMIT 4097"), &[&source.native_id]).await?;
    if rows.len() > MAX_RELATIONS { return Err(PublicApiError::unavailable()); }
    let snapshot_rows = rows.iter().map(|row| (row.get::<_,String>(0),row.get::<_,String>(1),row.get::<_,String>(2),row.get::<_,i32>(3))).collect::<Vec<_>>();
    // A small hidden-count range can be brute-forced from a hash. Public page
    // continuity therefore binds the visible count only; physical edge counts
    // remain an internal resource budget, never public token input.
    let snapshot = relation_paging::snapshot(&source.native_id, owner_version, rows.len() as u64, &snapshot_rows)?;
    page.check(owner_version, &snapshot)?;
    let selected = rows.iter().skip(page.offset as usize).take(page.limit as usize).collect::<Vec<_>>();
    let mut projected = BTreeMap::<String,Value>::new();
    if includes.contains("manga") && !selected.is_empty() {
        let targets = selected.iter().map(|row| row.get::<_,String>(2)).collect::<Vec<_>>();
        for row in tx.query("SELECT id,platform.catalog_public_manga_json(id) FROM platform.catalog_manga WHERE id=ANY($1::text[])", &[&targets]).await? {
            projected.insert(row.get(0), row.get::<_,Option<Value>>(1).ok_or_else(PublicApiError::unavailable)?);
        }
    }
    let resources = projected.values().cloned().map(|value| (ResourceKind::Manga,value)).collect::<Vec<_>>();
    let aliases = AliasIndex::load_in(&tx, &resources).await?;
    let data = selected.iter().map(|row| {
        let native: String = row.get(2);
        let target_id = PublicUuid::parse(row.get::<_,Option<&str>>(5).ok_or_else(PublicApiError::unavailable)?).map_err(|_|PublicApiError::unavailable())?;
        let attributes = if includes.contains("manga") {
            let target = aliases.resource(ResourceKind::Manga, projected.get(&native).ok_or_else(PublicApiError::unavailable)?, &BTreeSet::new())?;
            if target.id != target_id { return Err(PublicApiError::unavailable()); }
            Some(target.attributes)
        } else { None };
        Ok(Resource {
            id:PublicUuid::parse(row.get::<_,Option<&str>>(4).ok_or_else(PublicApiError::unavailable)?).map_err(|_|PublicApiError::unavailable())?,
            resource_type:ResourceType::MangaRelation,
            attributes:json!({"relation":row.get::<_,String>(1),"version":owner_version,"nostr":{"scope":"instance-local","versionScope":"owner_manga"}}),
            relationships:vec![Relationship{id:target_id,resource_type:ResourceType::Manga,related:None,attributes}],
        })
    }).collect::<PublicApiResult<Vec<_>>>()?;
    let mut extensions = serde_json::Map::new();
    extensions.insert("nostr".into(),json!({"scope":"instance-local","versionScope":"owner_manga","pagination":"bounded-relations-v1","identity":{"kind":"manga","requestedId":requested,"canonicalId":source.preferred_id,"source":"persisted-public-resource-ids"}}));
    let response = relation_paging::response(data, &page, RelationPageMeta { owner_id:source.preferred_id,owner_version,snapshot,total:rows.len() as u64,persisted_total:None,version_alias:false,extensions })?;
    tx.commit().await?;
    Ok(Json(response))
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
        &format!("SELECT platform.public_manga_edge_key(edge.manga_id,edge.{target}),{attributes},platform.catalog_public_manga_json(m.id),count(*) OVER() FROM {table} edge JOIN platform.catalog_manga root ON root.id=edge.manga_id JOIN platform.catalog_manga m ON m.id=edge.{target} WHERE {} ORDER BY {order} LIMIT {limit}", sql.predicate()),
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
