//! Authenticated private draft reads. Current account/admission locks and every
//! UUID lookup, projection and expansion are held on one repeatable-read snapshot.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdTarget, PublicUuid,
    ResourceKind,
    catalogue_dto::AliasIndex,
    dto::{CollectionResponse, EntityResponse, Resource},
    native_contributions::actor,
    query::{Direction, PublicQuery, QuerySchema},
};
use crate::{AppState, domains::catalog::query::Sql};
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{OriginalUri, Path, Request, State},
    http::HeaderMap,
    routing::get,
};
use serde_json::Value;
use std::time::Duration;
const INCLUDES: &[&str] = &["manga", "cover_art", "author", "artist", "tag", "creator"];
const STATES: &[&str] = &["draft", "submitted", "rejected"];
const QUERY: QuerySchema<'static> = QuerySchema {
    default_limit: 10,
    max_limit: 100,
    max_offset: 10000,
    scalar: &["state", "expectedAccountId"],
    arrays: &[],
    includes: INCLUDES,
    order: &["title", "year", "createdAt", "updatedAt"],
};
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/account/manga/draft", get(list))
        .route("/account/manga/draft/{p0}", get(detail))
}
fn query(raw: Option<&str>, detail: bool) -> PublicApiResult<PublicQuery> {
    let schema = if detail {
        QuerySchema {
            default_limit: 0,
            max_limit: 0,
            max_offset: 0,
            scalar: &[],
            arrays: &[],
            includes: INCLUDES,
            order: &[],
        }
    } else {
        QUERY
    };
    if detail {
        for (key, _) in url::form_urlencoded::parse(raw.unwrap_or("").as_bytes()) {
            if key != "includes[]" {
                return Err(PublicApiError::bad_request(
                    "Draft detail only accepts includes[]",
                ));
            }
        }
    }
    let mut query = PublicQuery::parse(raw, &schema)?;
    if let Some(expected) = query.first("expectedAccountId") {
        PublicUuid::parse(expected)?;
    }
    if !detail && query.limit == 0 {
        return Err(PublicApiError::bad_request(
            "Draft limit must be between 1 and 100",
        ));
    }
    if query
        .first("state")
        .is_some_and(|state| !STATES.contains(&state))
    {
        return Err(PublicApiError::bad_request("Invalid draft state"));
    }
    if !detail && query.order.is_empty() {
        query.order.push(("createdAt".into(), Direction::Desc));
    }
    Ok(query)
}
async fn body(request: Request) -> PublicApiResult<(String, Vec<u8>)> {
    let method = request.method().as_str().to_owned();
    let body = tokio::time::timeout(Duration::from_secs(5), to_bytes(request.into_body(), 1024))
        .await
        .map_err(|_| PublicApiError::bad_request("Request body timeout"))?
        .map_err(|_| PublicApiError::bad_request("GET accepts no request body"))?;
    if !body.is_empty() {
        return Err(PublicApiError::bad_request("GET accepts no request body"));
    }
    Ok((method, body.to_vec()))
}
async fn list(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<CollectionResponse<Resource>>> {
    let query = query(uri.query(), false)?;
    body(request).await?;
    let mut client = state.db.client().await?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = actor(&state, &headers, &tx, false).await?;
    if let Some(expected) = query.first("expectedAccountId") {
        let expected = PublicUuid::parse(expected)?;
        let current = super::uploads::preferred(&tx, ResourceKind::User, actor.user_id()).await?;
        if expected != current {
            return Err(PublicApiError::conflict(
                "The account changed; reload draft access",
            ));
        }
    }
    let mut sql = Sql::default();
    sql.conditions.push("m.deleted_at IS NULL AND m.state IN ('draft','submitted','rejected') AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)".into());
    if let Some(owner) = actor.draft_owner_filter()? {
        let owner = sql.bind(owner.to_owned());
        sql.conditions.push(format!("m.owner_id={owner}"));
    }
    if let Some(state) = query.first("state") {
        let state = sql.bind(state.to_owned());
        sql.conditions.push(format!("m.state={state}"));
    }
    let predicate = sql.predicate();
    let total: i64 = tx
        .query_one(
            &format!("SELECT count(*) FROM platform.catalog_manga m WHERE {predicate}"),
            &sql.params(),
        )
        .await?
        .get(0);
    let order = query
        .order
        .iter()
        .map(|(field, direction)| {
            let column = match field.as_str() {
                "title" => "m.title",
                "year" => "m.year",
                "createdAt" => "m.created_at",
                "updatedAt" => "m.updated_at",
                _ => unreachable!("validated draft sort"),
            };
            format!("{column} {}", direction.sql())
        })
        .chain(std::iter::once("m.id ASC".into()))
        .collect::<Vec<_>>()
        .join(",");
    let limit = sql.bind(i64::from(query.limit));
    let offset = sql.bind(i64::from(query.offset));
    let values = tx.query(&format!("SELECT platform.catalog_public_manga_json(m.id) FROM platform.catalog_manga m WHERE {predicate} ORDER BY {order} LIMIT {limit} OFFSET {offset}"), &sql.params()).await?
        .into_iter().map(|row| row.get::<_, Option<Value>>(0).ok_or_else(PublicApiError::unavailable)).collect::<PublicApiResult<Vec<_>>>()?;
    let resources = values
        .iter()
        .cloned()
        .map(|v| (ResourceKind::Manga, v))
        .collect::<Vec<_>>();
    let index = AliasIndex::load_in(&tx, &resources).await?;
    let mut data = values
        .iter()
        .map(|v| index.resource(ResourceKind::Manga, v, &query.includes))
        .collect::<PublicApiResult<Vec<_>>>()?;
    for (resource, native) in data.iter_mut().zip(values.iter()) {
        resource.attributes["reviewTimeline"] = super::native_draft_reviews::summary(&tx, native["id"].as_str().ok_or_else(PublicApiError::unavailable)?).await?;
    }
    let mut result = CollectionResponse::new(data, query.limit, query.offset, total as u64);
    result.extensions.insert("nostr".into(),serde_json::json!({"scope":"instance-account","authentication":"native-session","accountId":super::uploads::preferred(&tx,ResourceKind::User,actor.user_id()).await?}));
    tx.commit().await?;
    Ok(Json(result))
}
async fn detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<EntityResponse<Resource>>> {
    let query = query(uri.query(), true)?;
    let id = PublicUuid::parse(&id)?;
    body(request).await?;
    let mut client = state.db.client().await?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = actor(&state, &headers, &tx, false).await?;
    let target = PostgresPublicIdResolver::resolve_many_in(
        &*tx,
        ResourceKind::Manga,
        std::slice::from_ref(&id),
    )
    .await?
    .remove(&id)
    .filter(|v| v.target == PublicIdTarget::NativeResource)
    .ok_or_else(PublicApiError::not_found)?;
    let row = tx.query_opt("SELECT m.owner_id,platform.catalog_public_manga_json(m.id) FROM platform.catalog_manga m WHERE m.id=$1 AND m.deleted_at IS NULL AND m.state IN ('draft','submitted','rejected') AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)", &[&target.native_id]).await?.ok_or_else(PublicApiError::not_found)?;
    actor.require_draft_owner(row.get::<_, &str>(0))?;
    let value: Value = row
        .get::<_, Option<Value>>(1)
        .ok_or_else(PublicApiError::unavailable)?;
    let index = AliasIndex::load_in(&tx, &[(ResourceKind::Manga, value.clone())]).await?;
    let mut resource = index.resource(ResourceKind::Manga, &value, &query.includes)?;
    resource.attributes["reviewTimeline"] = super::native_draft_reviews::timeline(&tx, &target.native_id).await?;
    let mut result = EntityResponse::new(resource);
    result.extensions.insert("nostr".into(),serde_json::json!({"scope":"instance-account","authentication":"native-session","accountId":super::uploads::preferred(&tx,ResourceKind::User,actor.user_id()).await?}));
    tx.commit().await?;
    Ok(Json(result))
}
