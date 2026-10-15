//! Public instance-local relation/recommendation resources over immutable edge aliases.
//! Neither an edge UUID nor an editorial score is a signed protocol identity.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdResolver, PublicIdTarget,
    PublicUuid, ResourceKind,
    catalogue_dto::AliasIndex,
    dto::{CollectionResponse, Relationship, Resource, ResourceType},
    query::{PublicQuery, QuerySchema},
};
use crate::AppState;
use axum::{
    Json, Router,
    extract::{Path, RawQuery, State},
    routing::get,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
const MAX_RELATIONS: usize = 1000;
const RATINGS: &[&str] = &["safe", "suggestive", "erotica", "pornographic"];
const VISIBLE: &str = "m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)";
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/manga/{p0}/relation", get(relations))
        .route("/manga/{p0}/recommendation", get(recommendations))
}
fn query(raw: Option<&str>, recommendation: bool) -> PublicApiResult<PublicQuery> {
    for (key, _) in url::form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
        if key != "includes[]"
            && !(recommendation && (key == "contentRating[]" || key == "order[score]"))
        {
            return Err(PublicApiError::bad_request(
                "Unsupported manga edge query parameter",
            ));
        }
    }
    PublicQuery::parse(
        raw,
        &QuerySchema {
            default_limit: 0,
            max_limit: 0,
            max_offset: 0,
            scalar: &[],
            arrays: if recommendation {
                &["contentRating[]"]
            } else {
                &[]
            },
            includes: &["manga"],
            order: if recommendation { &["score"] } else { &[] },
        },
    )
}
async fn relations(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> PublicApiResult<Json<CollectionResponse<Resource>>> {
    read(state, id, raw, false).await
}
async fn recommendations(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> PublicApiResult<Json<CollectionResponse<Resource>>> {
    read(state, id, raw, true).await
}
async fn read(
    state: AppState,
    id: String,
    raw: Option<String>,
    recommendation: bool,
) -> PublicApiResult<Json<CollectionResponse<Resource>>> {
    let q = query(raw.as_deref(), recommendation)?;
    let resolver = PostgresPublicIdResolver::new(state.db.clone());
    let public = PublicUuid::parse(&id)?;
    let source = resolver
        .resolve(ResourceKind::Manga, &public)
        .await?
        .filter(|r| r.target == PublicIdTarget::NativeResource)
        .ok_or_else(PublicApiError::not_found)?;
    let mut ratings = q.list("contentRating[]");
    if ratings.iter().any(|v| !RATINGS.contains(&v.as_str())) {
        return Err(PublicApiError::bad_request("Invalid content rating"));
    }
    if ratings.is_empty() {
        ratings = RATINGS[..3].iter().map(|v| (*v).to_owned()).collect();
    }
    let (kind, table, target, field, order, bound) = if recommendation {
        (
            ResourceKind::MangaRecommendation,
            "platform.catalog_recommendations",
            "recommended_id",
            "score",
            format!(
                "e.score {} NULLS LAST,m.id ASC",
                q.order.first().map(|(_, d)| d.sql()).unwrap_or("DESC")
            ),
            20i64,
        )
    } else {
        (
            ResourceKind::MangaRelation,
            "platform.catalog_relations",
            "related_id",
            "relation_type",
            "e.relation_type ASC,lower(m.title) ASC,m.id ASC".to_owned(),
            (MAX_RELATIONS + 1) as i64,
        )
    };
    // Only fixed identifiers above enter SQL. Every user value remains a bind.
    // Count and selected native rows share the same visibility snapshot.
    let statement = format!(
        "WITH root AS MATERIALIZED(SELECT m.id FROM platform.catalog_manga m WHERE m.id=$1 AND {VISIBLE}),matched AS MATERIALIZED(SELECT e.{field} edge_value,m.id target_id,platform.public_manga_edge_key(e.manga_id,e.{target}) edge_key,row_number() OVER(ORDER BY {order}) position FROM {table} e JOIN root ON root.id=e.manga_id JOIN platform.catalog_manga m ON m.id=e.{target} WHERE {VISIBLE} AND ($2::boolean=FALSE OR m.content_rating=ANY($3::text[]))),selected AS(SELECT * FROM matched ORDER BY position LIMIT $4) SELECT EXISTS(SELECT 1 FROM root),(SELECT count(*) FROM matched),COALESCE((SELECT jsonb_agg(jsonb_build_object('id',(SELECT a.public_id::text FROM platform.public_resource_ids a WHERE a.kind=$5 AND a.native_id=selected.edge_key AND a.preferred),'value',selected.edge_value,'target',platform.catalog_manga_json(selected.target_id)) ORDER BY selected.position) FROM selected),'[]'::jsonb)"
    );
    let row = state
        .db
        .client()
        .await?
        .query_one(
            &statement,
            &[
                &source.native_id,
                &recommendation,
                &ratings,
                &bound,
                &kind.as_str(),
            ],
        )
        .await?;
    if !row.get::<_, bool>(0) {
        return Err(PublicApiError::not_found());
    }
    let total: i64 = row.get(1);
    if !recommendation && total > MAX_RELATIONS as i64 {
        return Err(PublicApiError::bad_request(
            "Relation view exceeds its documented 1000-edge budget",
        ));
    }
    let values: Value = row.get(2);
    let rows = values.as_array().ok_or_else(PublicApiError::unavailable)?;
    let targets = rows
        .iter()
        .map(|r| (ResourceKind::Manga, r["target"].clone()))
        .collect::<Vec<_>>();
    let aliases = AliasIndex::load(&state, &resolver, &targets).await?;
    let mut data = Vec::with_capacity(rows.len());
    for edge in rows {
        let edge_id = edge["id"]
            .as_str()
            .ok_or_else(PublicApiError::unavailable)?;
        let id = PublicUuid::parse(edge_id).map_err(|_| PublicApiError::unavailable())?;
        let target = aliases.resource(ResourceKind::Manga, &edge["target"], &BTreeSet::new())?;
        // The path identifies the source. The standard manga relationship is the target.
        let relationship = Relationship {
            id: target.id,
            resource_type: ResourceType::Manga,
            related: None,
            attributes: q.includes.contains("manga").then_some(target.attributes),
        };
        let attributes = if recommendation {
            json!({"score":edge["value"]})
        } else {
            json!({"relation":edge["value"]})
        };
        // Native relations have no independent revision column. Omit the optional
        // version rather than manufacture one from the target or parent version.
        data.push(Resource {
            id,
            resource_type: if recommendation {
                ResourceType::MangaRecommendation
            } else {
                ResourceType::MangaRelation
            },
            attributes,
            relationships: vec![relationship],
        });
    }
    let limit = if recommendation {
        20
    } else {
        u32::try_from(data.len()).map_err(|_| PublicApiError::unavailable())?
    };
    let mut response = CollectionResponse::new(
        data,
        limit,
        0,
        u64::try_from(total).map_err(|_| PublicApiError::unavailable())?,
    );
    response.extensions.insert("nostr".into(),json!({"scope":"instance_local_editorial","sourceManga":source.preferred_id,"edgeIdentityVersion":"edge-v2","scoreSource":if recommendation {Some("native_catalog_recommendations")} else {None}}));
    Ok(Json(response))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edge_queries_match_their_operation_instead_of_ignoring_input() {
        assert!(query(Some("includes[]=manga"), false).is_ok());
        assert!(query(Some("order[score]=asc&contentRating[]=pornographic"), true).is_ok());
        for q in [
            "limit=1",
            "offset=1",
            "order[title]=asc",
            "includes[]=author",
            "source=native",
        ] {
            assert!(query(Some(q), true).is_err(), "{q}");
        }
        assert!(query(Some("contentRating[]=safe"), false).is_err());
    }
}
