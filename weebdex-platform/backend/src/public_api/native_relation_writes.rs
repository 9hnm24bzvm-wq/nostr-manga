//! Exact signed, versioned instance-local relation commands over the native core.
//! Persisted directional edge UUIDs never confer source or publisher authority.
use super::{
    PublicApiError, PublicApiResult, PublicUuid, ResourceKind,
    catalogue_dto::AliasIndex,
    dto::{CollectionResponse, EntityResponse, Relationship, Resource, ResourceType},
    native_editorial::expected_account,
    uploads::{preferred, resolve},
};
use crate::{
    AppState,
    domains::catalog::{
        entity_mutations::CatalogMutationActor, mutations as core, validation as v,
    },
};
use axum::{
    Json, Router,
    extract::{OriginalUri, Path, Request, State},
    http::HeaderMap,
    routing::{delete, get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use tokio_postgres::Transaction;
const MAX_RELATIONS: usize = 4096;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RelationCommand {
    target_manga: PublicUuid,
    relation: String,
    version: i32,
    expected_account_id: PublicUuid,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeleteCommand {
    version: i32,
    expected_account_id: PublicUuid,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/account/manga/{p0}/relation", post(create).put(update))
        .route("/account/manga/{p0}/relation/{p1}", delete(remove))
        .route(
            "/account/manga/draft/{p0}/relation",
            get(editing_relations).layer(axum::middleware::from_fn(private_page_headers)),
        )
        .route(
            "/account/manga/{p0}/relations",
            get(editing_relations).layer(axum::middleware::from_fn(private_page_headers)),
        )
}
async fn resource(
    tx: &Transaction<'_>,
    owner: &str,
    related: &str,
    version: i32,
) -> PublicApiResult<Resource> {
    let row=tx.query_opt("SELECT r.relation_type,platform.catalog_public_manga_json(m.id) FROM platform.catalog_relations r JOIN platform.catalog_manga m ON m.id=r.related_id WHERE r.manga_id=$1 AND r.related_id=$2 AND m.state='published' AND m.deleted_at IS NULL AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)",&[&owner,&related]).await?.ok_or_else(PublicApiError::not_found)?;
    let target: Value = row
        .get::<_, Option<Value>>(1)
        .ok_or_else(PublicApiError::unavailable)?;
    let aliases = AliasIndex::load_in(tx, &[(ResourceKind::Manga, target.clone())]).await?;
    let target = aliases.resource(ResourceKind::Manga, &target, &BTreeSet::new())?;
    let edge = super::edge_identity::manga_edge_key(owner, related);
    Ok(Resource {
        id: preferred(tx, ResourceKind::MangaRelation, &edge).await?,
        resource_type: ResourceType::MangaRelation,
        attributes: json!({"relation":row.get::<_,String>(0),"version":version,"ownerMangaId":preferred(tx,ResourceKind::Manga,owner).await?,"nostr":{"scope":"instance-local","versionScope":"owner_manga"}}),
        relationships: vec![Relationship {
            id: target.id,
            resource_type: ResourceType::Manga,
            related: None,
            attributes: Some(target.attributes),
        }],
    })
}
async fn create(
    state: State<AppState>,
    path: Path<String>,
    uri: OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<EntityResponse<Resource>>> {
    mutate(state, path, uri, headers, request, false).await
}
async fn update(
    state: State<AppState>,
    path: Path<String>,
    uri: OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<EntityResponse<Resource>>> {
    mutate(state, path, uri, headers, request, true).await
}
async fn mutate(
    State(state): State<AppState>,
    Path(owner): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
    edit: bool,
) -> PublicApiResult<Json<EntityResponse<Resource>>> {
    let owner = PublicUuid::parse(&owner)?;
    super::uploads::no_query(&uri)?;
    let body = super::native_account_profile_settings::body(request, &headers, true, 4096).await?;
    // Typed parsing rejects both unknown and duplicate intent fields, rather
    // than allowing different consumers to select different repeated keys.
    let input: RelationCommand = serde_json::from_slice(&body).map_err(|_| {
        PublicApiError::bad_request("Invalid, duplicate or unknown relation command fields")
    })?;
    let expected = json!(input.expected_account_id);
    let version = v::version(&json!({"version":input.version}))?;
    let target = input.target_manga;
    let kind = input.relation;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let actor = super::native_contributions::actor(&state, &headers, &tx, true).await?;
    expected_account(&tx, &actor, Some(&expected)).await?;
    let owner = resolve(&tx, ResourceKind::Manga, &owner).await?;
    let target = resolve(&tx, ResourceKind::Manga, &target).await?;
    let native = json!({"related_id":target,"type":kind,"version":version});
    let native = if edit {
        core::relation_update_in(&tx, &actor, &owner, &native).await?
    } else {
        core::relation_create_in(&tx, &actor, &owner, &native).await?
    };
    let version = native["manga_version"]
        .as_i64()
        .and_then(|v| i32::try_from(v).ok())
        .ok_or_else(PublicApiError::unavailable)?;
    let result = resource(&tx, &owner, &target, version).await?;
    let mut envelope = EntityResponse::new(result);
    envelope.extensions.insert("nostr".into(),json!({"scope":"instance-account","authentication":"native-session","accountId":input.expected_account_id}));
    tx.commit().await?;
    Ok(Json(envelope))
}
async fn remove(
    State(state): State<AppState>,
    Path((owner, edge)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    let owner = PublicUuid::parse(&owner)?;
    let edge = PublicUuid::parse(&edge)?;
    super::uploads::no_query(&uri)?;
    let body = super::native_account_profile_settings::body(request, &headers, true, 1024).await?;
    let input: DeleteCommand = serde_json::from_slice(&body).map_err(|_| {
        PublicApiError::bad_request("Invalid, duplicate or unknown relation deletion fields")
    })?;
    let expected = json!(input.expected_account_id);
    let version = v::version(&json!({"version":input.version}))?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let actor = super::native_contributions::actor(&state, &headers, &tx, true).await?;
    expected_account(&tx, &actor, Some(&expected)).await?;
    let owner = resolve(&tx, ResourceKind::Manga, &owner).await?;
    let key = resolve(&tx, ResourceKind::MangaRelation, &edge).await?;
    // Edge identity must belong to this exact directional owner, not merely a
    // related title UUID or a relation owned by some other catalogue root.
    let target:String=tx.query_opt("SELECT related_id FROM platform.catalog_relations WHERE manga_id=$1 AND platform.public_manga_edge_key(manga_id,related_id)=$2",&[&owner,&key]).await?.ok_or_else(PublicApiError::not_found)?.get(0);
    let edge_id = preferred(&tx, ResourceKind::MangaRelation, &key).await?;
    let owner_id = preferred(&tx, ResourceKind::Manga, &owner).await?;
    let version = core::relation_delete_in(
        &tx,
        &actor,
        &owner,
        &json!({"related_id":target,"version":version}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"result":"ok","mangaId":owner_id,"relationId":edge_id,"version":version,"nostr":{"scope":"instance-account","versionScope":"owner_manga","authentication":"native-session","accountId":input.expected_account_id}}),
    ))
}
async fn editing_relations(
    State(state): State<AppState>,
    Path(owner): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<CollectionResponse<Resource>>> {
    use super::relation_paging::{self, RelationPageMeta, RelationPageRequest, SnapshotEdge};
    use axum::body::to_bytes;
    let requested = PublicUuid::parse(&owner)?;
    let (query, page) = RelationPageRequest::parse(uri.query(), true, &["expectedAccountId"])?;
    let expected = PublicUuid::parse(
        query
            .first("expectedAccountId")
            .ok_or_else(|| PublicApiError::bad_request("expectedAccountId is required"))?,
    )?;
    let body = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        to_bytes(request.into_body(), 1),
    )
    .await
    .map_err(|_| PublicApiError::bad_request("Request body timeout"))?
    .map_err(|_| PublicApiError::bad_request("Relation reads require an empty body"))?;
    if !body.is_empty() {
        return Err(PublicApiError::bad_request(
            "Relation reads require an empty body",
        ));
    }

    let mut client = state.db.client().await?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = super::native_contributions::actor(&state, &headers, &tx, false).await?;
    if resolve(&tx, ResourceKind::User, &expected).await? != actor.user_id() {
        return Err(PublicApiError::conflict(
            "The captured editor account changed",
        ));
    }
    let owner = resolve(&tx, ResourceKind::Manga, &requested).await?;
    let row=tx.query_opt("SELECT owner_id,version,state,locked FROM platform.catalog_manga WHERE id=$1 AND state IN('draft','submitted','rejected','published') AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id)",&[&owner]).await?.ok_or_else(PublicApiError::not_found)?;
    let version: i32 = row.get(1);
    let state: String = row.get(2);
    if uri.path().starts_with("/api/v1/account/manga/draft/") && state == "published" {
        return Err(PublicApiError::not_found());
    }
    // Read eligibility mirrors the native relation editor, without acquiring
    // mutation/signing authority or blocking read-only pending-intent recovery.
    if !(actor.is_moderator() && actor.has_perm("manga.relation.edit")) {
        if row.get::<_, bool>(3) {
            return Err(PublicApiError::forbidden("Manga is locked"));
        }
        if state == "published" {
            actor.require_permission("manga.relation.edit")?;
        } else if row.get::<_, &str>(0) == actor.user_id() {
            actor.require_permission("manga.create")?;
        } else {
            return Err(PublicApiError::forbidden(
                "Only the draft owner can edit its relations",
            ));
        }
    }
    if page.owner_version != Some(version) {
        return Err(PublicApiError::conflict(
            "Relation owner version changed; refresh the title",
        ));
    }
    let persisted: i64 = tx
        .query_one(
            "SELECT count(*) FROM platform.catalog_relations WHERE manga_id=$1",
            &[&owner],
        )
        .await?
        .get(0);
    if !(0..=MAX_RELATIONS as i64).contains(&persisted) {
        return Err(PublicApiError::unavailable());
    }
    // Only lightweight identities are scanned for continuity. Full unchanged
    // native target metadata is projected for the selected small page alone.
    let rows=tx.query("SELECT a.public_id::text,r.relation_type,m.id,m.version FROM platform.catalog_relations r JOIN platform.catalog_manga m ON m.id=r.related_id LEFT JOIN platform.public_resource_ids a ON a.kind='manga_relation' AND a.native_id=platform.public_manga_edge_key(r.manga_id,r.related_id) AND a.preferred WHERE r.manga_id=$1 AND m.state='published' AND m.deleted_at IS NULL AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id) ORDER BY r.created_at,m.id LIMIT 4097",&[&owner]).await?;
    if rows.len() > MAX_RELATIONS {
        return Err(PublicApiError::unavailable());
    }
    let edges = rows
        .iter()
        .map(|r| {
            Ok((
                r.get::<_, Option<String>>(0)
                    .ok_or_else(PublicApiError::unavailable)?,
                r.get(1),
                r.get(2),
                r.get(3),
            ))
        })
        .collect::<PublicApiResult<Vec<SnapshotEdge>>>()?;
    // The key encoding is the same actual native edge key as the public route.
    let edges = edges
        .iter()
        .map(|edge| {
            (
                super::edge_identity::manga_edge_key(&owner, &edge.2),
                edge.1.clone(),
                edge.2.clone(),
                edge.3,
            )
        })
        .collect::<Vec<_>>();
    let snapshot = relation_paging::snapshot(&owner, version, persisted as u64, &edges)?;
    page.check(version, &snapshot)?;
    let selected = rows
        .iter()
        .skip(page.offset as usize)
        .take(page.limit as usize)
        .collect::<Vec<_>>();
    let targets = selected
        .iter()
        .map(|r| r.get::<_, String>(2))
        .collect::<Vec<_>>();
    let mut projected = std::collections::BTreeMap::<String, Value>::new();
    if !targets.is_empty() {
        for r in tx.query("SELECT id,platform.catalog_public_manga_json(id) FROM platform.catalog_manga WHERE id=ANY($1::text[]) AND state='published' AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id)",&[&targets]).await?{projected.insert(r.get(0),r.get::<_,Option<Value>>(1).ok_or_else(PublicApiError::unavailable)?);}
    }
    let values = targets
        .iter()
        .map(|id| projected.remove(id).ok_or_else(PublicApiError::unavailable))
        .collect::<PublicApiResult<Vec<_>>>()?;
    let resources = values
        .iter()
        .cloned()
        .map(|v| (ResourceKind::Manga, v))
        .collect::<Vec<_>>();
    let aliases = AliasIndex::load_in(&tx, &resources).await?;
    let owner_id = preferred(&tx, ResourceKind::Manga, &owner).await?;
    let data=selected.iter().zip(values.iter()).map(|(row,value)|{let target=aliases.resource(ResourceKind::Manga,value,&BTreeSet::new())?;Ok(Resource{id:PublicUuid::parse(row.get::<_,Option<&str>>(0).ok_or_else(PublicApiError::unavailable)?)?,resource_type:ResourceType::MangaRelation,
        attributes:json!({"relation":row.get::<_,String>(1),"version":version,"ownerMangaId":owner_id,"nostr":{"scope":"instance-local","versionScope":"owner_manga"}}),
        relationships:vec![Relationship{id:target.id,resource_type:ResourceType::Manga,related:None,attributes:Some(target.attributes)}]})}).collect::<PublicApiResult<Vec<_>>>()?;
    let mut extensions = serde_json::Map::new();
    extensions.insert("nostr".into(),json!({"scope":"instance-account","versionScope":"owner_manga","pagination":"bounded-relations-v1","authentication":"native-session","accountId":expected,"snapshot":snapshot,"identity":{"kind":"manga","requestedId":requested,"canonicalId":owner_id,"source":"persisted-public-resource-ids"}}));
    let response = relation_paging::response(
        data,
        &page,
        RelationPageMeta {
            owner_id,
            owner_version: version,
            snapshot,
            total: rows.len() as u64,
            persisted_total: Some(persisted as u64),
            version_alias: true,
            extensions,
        },
    )?;
    tx.commit().await?;
    Ok(Json(response))
}
async fn private_page_headers(
    request: Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("private, no-store"),
    );
    response.headers_mut().insert(
        axum::http::header::VARY,
        axum::http::HeaderValue::from_static("Origin, Authorization"),
    );
    response.headers_mut().insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        axum::http::HeaderValue::from_static("nosniff"),
    );
    response
}
