use super::{
    entity_mutations::{self, AuthorCreate, AuthorUpdate, CatalogMutationActor, GroupCreate, GroupUpdate},
    repository::{audit, create_thread, require_manga_edit, require_mutable_manga},
    validation as v,
};
use crate::{
    domains::auth::{SessionPrincipal, require_principal},
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use tokio_postgres::Transaction;

const MANGA_FIELDS: &[&str] = &[
    "title",
    "description",
    "year",
    "language",
    "demographic",
    "status",
    "content_rating",
    "last_volume",
    "last_chapter",
    "alt_titles",
    "tags",
    "authors",
    "artists",
    "links",
    "version",
];
const MANGA_META: &[&str] = &[
    "description",
    "last_volume",
    "last_chapter",
    "alt_titles",
    "links",
];

pub async fn author_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let actor = mutation_principal(&state, &headers).await?;
    actor.require_permission("author.create")?;
    v::allowed(&payload, &["name"])?;
    let input = AuthorCreate::parse(&payload)?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let data = entity_mutations::author_create(&tx, &actor, input).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(data)))
}
pub async fn author_update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    let actor = mutation_principal(&state, &headers).await?;
    actor.require_permission("author.update")?;
    v::id(&id)?;
    let input = AuthorUpdate::parse(&actor, &payload)?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let data = entity_mutations::author_update(&tx, &actor, &id, input).await?;
    tx.commit().await?;
    Ok(Json(data))
}
pub async fn author_delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<StatusCode> {
    let actor = mutation_principal(&state, &headers).await?;
    staff_permission(&actor, "author.delete")?;
    v::id(&id)?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    entity_mutations::author_delete(&tx, &actor, &id).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn group_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let actor = mutation_principal(&state, &headers).await?;
    actor.require_permission("scanlation_group.create")?;
    v::allowed(&payload, &["name", "description", "contact_email", "discord", "mangaupdates", "twitter", "website"])?;
    let input = GroupCreate::parse(&payload)?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let data = entity_mutations::group_create(&tx, &actor, input).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(data)))
}
pub async fn group_update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    let actor = mutation_principal(&state, &headers).await?;
    v::id(&id)?;
    let input = GroupUpdate::parse(&payload)?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let data = entity_mutations::group_update(&tx, &actor, &id, input).await?;
    tx.commit().await?;
    Ok(Json(data))
}
pub async fn group_delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<StatusCode> {
    let actor = mutation_principal(&state, &headers).await?;
    staff_permission(&actor, "scanlation_group.delete")?;
    v::id(&id)?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    entity_mutations::group_delete(&tx, &actor, &id).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

struct MangaFields {
    title: Option<String>,
    year: Option<i32>,
    language: Option<String>,
    demographic: Option<String>,
    status: Option<String>,
    rating: Option<String>,
    metadata: Value,
    tags: Option<Vec<String>>,
    authors: Option<Vec<String>>,
    artists: Option<Vec<String>>,
}
fn manga_fields(payload: &Value, create: bool) -> ApiResult<MangaFields> {
    v::allowed(payload, MANGA_FIELDS)?;
    let title = v::string(payload, "title", 500, create)?;
    if title.as_deref() == Some("") {
        return Err(ApiError::bad_request("Title must not be empty"));
    }
    let year = match payload.get("year") {
        None => None,
        Some(val) => Some(
            val.as_i64()
                .filter(|n| *n >= 0 && *n <= 9999)
                .ok_or_else(|| ApiError::bad_request("Invalid year"))? as i32,
        ),
    };
    let language = v::string(payload, "language", 12, create)?;
    if let Some(lang) = &language {
        v::language(lang)?;
    }
    let demographic = v::enum_value(
        payload,
        "demographic",
        &["shounen", "shoujo", "josei", "seinen", "none"],
        false,
    )?;
    let status = v::enum_value(
        payload,
        "status",
        &["ongoing", "completed", "hiatus", "cancelled"],
        create,
    )?;
    let rating = v::enum_value(
        payload,
        "content_rating",
        &["safe", "suggestive", "erotica", "pornographic"],
        create,
    )?;
    let metadata = v::metadata(payload, MANGA_META)?;
    v::string(payload, "last_volume", 32, false)?;
    v::string(payload, "last_chapter", 32, false)?;
    Ok(MangaFields {
        title,
        year,
        language,
        demographic,
        status,
        rating,
        metadata,
        tags: v::ids(payload, "tags", false)?,
        authors: v::ids(payload, "authors", create)?,
        artists: v::ids(payload, "artists", create)?,
    })
}
fn merge_localized_metadata(fields: &mut MangaFields, extra: &Value) {
    fields.metadata.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
}
async fn set_manga_links(tx: &Transaction<'_>, id: &str, fields: &MangaFields) -> ApiResult<()> {
    if let Some(tags) = &fields.tags {
        let n: i64 = tx
            .query_one(
                "SELECT count(*) FROM platform.catalog_tags WHERE id=ANY($1)",
                &[tags],
            )
            .await?
            .get(0);
        if n as usize != tags.len() {
            return Err(ApiError::bad_request("Unknown tag"));
        }
        tx.execute(
            "DELETE FROM platform.catalog_manga_tags WHERE manga_id=$1",
            &[&id],
        )
        .await?;
        for tag in tags {
            tx.execute(
                "INSERT INTO platform.catalog_manga_tags(manga_id,tag_id) VALUES($1,$2)",
                &[&id, tag],
            )
            .await?;
        }
    }
    for (role, people) in [("author", &fields.authors), ("artist", &fields.artists)] {
        if let Some(people) = people {
            // Keep visible author rows stable until this title's reference
            // writes commit. Soft deletion holds the conflicting UPDATE lock
            // before checking current live-title references.
            let authors=tx.query("SELECT id FROM platform.catalog_authors WHERE id=ANY($1) AND deleted_at IS NULL AND platform.protocol_source_is_visible(protocol_event_id) ORDER BY id FOR SHARE",&[people]).await?;
            if authors.len() != people.len() {
                return Err(ApiError::bad_request("Unknown author or artist"));
            }
            tx.execute(
                "DELETE FROM platform.catalog_manga_creators WHERE manga_id=$1 AND role=$2",
                &[&id, &role],
            )
            .await?;
            for person in people {
                tx.execute("INSERT INTO platform.catalog_manga_creators(manga_id,author_id,role) VALUES($1,$2,$3)",&[&id,person,&role]).await?;
            }
        }
    }
    Ok(())
}
pub async fn manga_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let actor = mutation_principal(&state, &headers).await?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let data = manga_create_in(&tx, &actor, &payload, &json!({})).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(data)))
}
/// Shared native/public create core. Caller owns authentication and transaction.
pub(crate) async fn manga_create_in(
    tx: &Transaction<'_>, actor: &impl CatalogMutationActor, payload: &Value,
    extra_metadata: &Value,
) -> ApiResult<Value> {
    actor.require_permission("manga.create")?;
    let mut f = manga_fields(payload, true)?;
    let language = f.language.as_deref().unwrap();
    f.metadata["localized_titles"] = json!({language:f.title});
    if let Some(description) = payload.get("description") { f.metadata["localized_description"] = json!({language:description}); }
    merge_localized_metadata(&mut f, extra_metadata);
    if payload.get("version").is_some() {
        return Err(ApiError::bad_request("Version is assigned by the server"));
    }
    let id = v::new_id("manga");
    tx.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,year,language,demographic,publication_status,content_rating,metadata) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",&[&id,&actor.user_id(),&actor.username(),&f.title,&f.year.unwrap_or(0),&f.language,&f.demographic,&f.status,&f.rating,&f.metadata]).await?;
    set_manga_links(tx, &id, &f).await?;
    create_thread(
        tx,
        "manga",
        &id,
        &actor.user_id(),
        &actor.username(),
        f.title.as_deref().unwrap(),
    )
    .await?;
    tx.execute(
        "INSERT INTO platform.catalog_manga_counters(manga_id) VALUES($1)",
        &[&id],
    )
    .await?;
    audit(tx, &actor.user_id(), "manga", &id, "create", None, Some(1)).await?;
    let data = tx
        .query_one("SELECT platform.catalog_manga_json($1)", &[&id])
        .await?
        .get(0);
    Ok(data)
}

pub async fn manga_update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    let actor = mutation_principal(&state, &headers).await?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let data = manga_update_in(&tx, &actor, &id, &payload, &json!({})).await?;
    tx.commit().await?;
    Ok(Json(data))
}
pub(crate) async fn manga_update_in(
    tx: &Transaction<'_>, actor: &impl CatalogMutationActor, id: &str, payload: &Value,
    extra_metadata: &Value,
) -> ApiResult<Value> {
    v::id(&id)?;
    let version = v::version(&payload)?;
    let mut f = manga_fields(payload, false)?;
    require_manga_edit(tx, &id, actor, "manga.update").await?;
    for (fields, perm) in [
        (&["title", "alt_titles"][..], "manga.title.edit"),
        (&["tags"][..], "manga.tag.edit"),
        (&["authors"][..], "manga.author.edit"),
        (&["artists"][..], "manga.artist.edit"),
        (&["links"][..], "manga.link.edit"),
    ] {
        if fields.iter().any(|key| payload.get(*key).is_some()) {
            require_manga_edit(tx, &id, actor, perm).await?;
        }
    }
    let row = tx
        .query_one(
            "SELECT version,protocol_series_id IS NOT NULL,language FROM platform.catalog_manga WHERE id=$1",
            &[&id],
        )
        .await?;
    let protocol_overlay: bool = row.get(1);
    check_version(row.get(0), version)?;
    // A native edit must not leave an older localized public value shadowing it.
    let language = f.language.clone().unwrap_or_else(|| row.get(2));
    if let Some(title) = &f.title { f.metadata["localized_titles"] = json!({language.clone():title}); }
    if let Some(description) = payload.get("description") { f.metadata["localized_description"] = json!({language:description}); }
    if let Some(alts) = payload["alt_titles"].as_object() {
        let values:Vec<Value> = alts.iter().flat_map(|(lang,titles)|titles.as_array().into_iter().flatten().map(move |title|json!({lang:title}))).collect();
        f.metadata["localized_alt_titles"] = json!(values);
    }
    merge_localized_metadata(&mut f, extra_metadata);
    set_manga_links(tx, &id, &f).await?;
    tx.execute("UPDATE platform.catalog_manga SET title=COALESCE($2,title),year=COALESCE($3,year),language=COALESCE($4,language),demographic=COALESCE($5,demographic),publication_status=COALESCE($6,publication_status),content_rating=COALESCE($7,content_rating),metadata=metadata || $8,version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id,&f.title,&f.year,&f.language,&f.demographic,&f.status,&f.rating,&f.metadata]).await?;
    if protocol_overlay {
        let mut overlay = payload.clone();
        overlay.as_object_mut().unwrap().extend(extra_metadata.as_object().unwrap().clone());
        overlay.as_object_mut().unwrap().remove("version");
        overlay["scope"] = json!("instance-local");
        overlay["updated_by"] = json!(actor.user_id());
        tx.execute("UPDATE platform.catalog_manga SET metadata=metadata||jsonb_build_object('instance_overlay',COALESCE(metadata->'instance_overlay','{}'::jsonb)||$2::jsonb) WHERE id=$1", &[&id,&overlay]).await?;
    }
    audit(
        tx,
        &actor.user_id(),
        "manga",
        &id,
        if protocol_overlay {
            "overlay_update"
        } else {
            "update"
        },
        Some(version),
        Some(version + 1),
    )
    .await?;
    let data = tx
        .query_one("SELECT platform.catalog_manga_json($1)", &[&id])
        .await?
        .get(0);
    Ok(data)
}

pub async fn manga_commit(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    transition(state, id, headers, payload, "submitted").await
}
pub async fn manga_publish(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    transition(state, id, headers, payload, "published").await
}
pub async fn manga_reject(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    transition(state, id, headers, payload, "rejected").await
}
async fn transition(
    state: AppState,
    id: String,
    headers: HeaderMap,
    payload: Value,
    next: &str,
) -> ApiResult<Json<Value>> {
    let actor = mutation_principal(&state, &headers).await?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let data = manga_transition_in(&state, &tx, &actor, &id, &payload, next, false).await?;
    tx.commit().await?;
    Ok(Json(data))
}
pub(crate) async fn manga_transition_in(
    state: &AppState, tx: &Transaction<'_>, actor: &impl CatalogMutationActor,
    id: &str, payload: &Value, next: &str, require_original_cover: bool,
) -> ApiResult<Value> {
    if !["submitted", "published", "rejected"].contains(&next) {
        return Err(ApiError::bad_request("Invalid title transition"));
    }
    v::allowed(&payload, &["version"])?;
    v::id(&id)?;
    let version = v::version(&payload)?;
    let row=tx.query_opt("SELECT owner_id,locked,state,version FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL FOR UPDATE",&[&id]).await?.ok_or_else(||ApiError::not_found("Draft not found"))?;
    let current: String = row.get(2);
    let actual_version: i32 = row.get(3);
    let replay = if next == "published"
        && current == "published"
        && version.checked_add(1) == Some(actual_version)
    {
        tx.query_opt("SELECT 1 FROM platform.protocol_publication_commands WHERE command_id=$1 AND user_id=$2 AND catalog_id=$3 AND resource_kind='manga' AND state='materialized' AND native_commit_payload=$4", &[&format!("manga:{id}"),&actor.user_id(),&id,&json!({"action":"publish_manga","manga_id":id,"submitted_version":version})]).await?.is_some()
    } else {
        false
    };
    if !replay {
        check_version(actual_version, version)?;
    }
    if next == "submitted" {
        actor.require_permission("manga.create")?;
        if row.get::<_, String>(0) != actor.user_id() && !actor.is_moderator() {
            return Err(ApiError::forbidden("Only the draft owner can submit it"));
        }
        if row.get::<_, bool>(1) && !actor.is_moderator() {
            return Err(ApiError::forbidden("Manga is locked"));
        }
        if !matches!(current.as_str(), "draft" | "rejected") {
            return Err(ApiError::conflict(
                "Only draft or rejected titles can be submitted",
            ));
        }
    } else {
        staff_permission(actor, "manga.state.update")?;
        if current != "submitted" && !replay {
            return Err(ApiError::conflict("Only submitted titles can be reviewed"));
        }
    }
    if next != "published" {
        require_mutable_manga(tx, &id).await?;
    }
    if require_original_cover && next == "submitted" {
        let cover:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.catalog_manga m JOIN platform.catalog_covers c ON c.manga_id=m.id AND c.language=m.language JOIN platform.cover_media cm ON cm.cover_id=c.id AND cm.variant='original' JOIN platform.media_objects o ON o.id=cm.object_id WHERE m.id=$1 AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND o.bytes>0)",&[&id]).await?.get(0);
        if !cover {return Err(ApiError::bad_request("Draft submission requires an original-language cover"));}
    }
    let previous: Option<Value> = tx.query_one("SELECT platform.catalog_manga_json($1)", &[&id]).await?.get(0);
    let receipt = if next == "published" {
        crate::protocol::publication::publish_native_manga(state, tx, actor, &id, version)
            .await?
    } else {
        None
    };
    if !replay {
        tx.execute("UPDATE platform.catalog_manga SET state=$2,published_at=CASE WHEN $2='published' THEN clock_timestamp() ELSE published_at END,version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id,&next]).await?;
        audit(
            tx,
            &actor.user_id(),
            "manga",
            &id,
            next,
            Some(version),
            Some(version + 1),
        )
        .await?;
    }
    let mut data: Option<Value> = tx
        .query_one("SELECT platform.catalog_manga_json($1)", &[&id])
        .await?
        .get(0);
    if let Some(receipt) = receipt {
        if data.is_none() {
            let row = tx
                .query_one(
                    "SELECT title,version FROM platform.catalog_manga WHERE id=$1",
                    &[&id],
                )
                .await?;
            let mut pending = previous.unwrap_or_else(||json!({"id":id,"title":row.get::<_,String>(0)}));
            pending["version"] = json!(row.get::<_,i32>(1));
            pending["state"] = json!("published");
            pending["is_unavailable"] = json!(true);
            data = Some(pending);
        }
        let value = data.as_mut().ok_or_else(ApiError::unavailable)?;
        value["protocol_event_id"] = json!(receipt.event.id);
        value["protocol_publisher"] = json!(receipt.event.pubkey);
        value["publication_scope"] = json!("original-protocol");
        value["publication_status"] = json!(receipt.state);
    } else if next == "published" {
        let value = data.as_mut().ok_or_else(ApiError::unavailable)?;
        value["publication_scope"] = json!("instance-local");
        value["publication_status"] = json!("local");
        tx.execute(
            "UPDATE platform.catalog_manga SET metadata=metadata||$2 WHERE id=$1",
            &[
                &id,
                &json!({"publication_scope":"instance-local","publication_status":"local"}),
            ],
        )
        .await?;
    }
    let data = data.ok_or_else(ApiError::unavailable)?;
    Ok(data)
}

pub async fn manga_lock(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    lock(state, id, headers, true).await
}
pub async fn manga_unlock(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    lock(state, id, headers, false).await
}
async fn lock(
    state: AppState,
    id: String,
    headers: HeaderMap,
    locked: bool,
) -> ApiResult<Json<Value>> {
    let actor = mutation_principal(&state, &headers).await?;
    staff_permission(&actor, "manga.lock.update")?;
    v::id(&id)?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    if tx.query_opt("SELECT id FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id) FOR UPDATE", &[&id]).await?.is_none() {
        return Err(ApiError::not_found("Manga not found"));
    }
    require_mutable_manga(&tx, &id).await?;
    if tx.execute("UPDATE platform.catalog_manga SET locked=$2,version=version+1,updated_at=clock_timestamp() WHERE id=$1 AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id)",&[&id,&locked]).await?==0{return Err(ApiError::not_found("Manga not found"));}
    audit(
        &tx,
        &actor.user_id,
        "manga",
        &id,
        if locked { "lock" } else { "unlock" },
        None,
        None,
    )
    .await?;
    let data = tx
        .query_one("SELECT platform.catalog_manga_json($1)", &[&id])
        .await?
        .get(0);
    tx.commit().await?;
    Ok(Json(data))
}
pub async fn manga_delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<StatusCode> {
    let actor = mutation_principal(&state, &headers).await?;
    v::id(&id)?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let row=tx.query_opt("SELECT owner_id,state,locked FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL FOR UPDATE",&[&id]).await?.ok_or_else(||ApiError::not_found("Manga not found"))?;
    if !(actor.is_moderator() && actor.has_perm("manga.delete")) {
        actor.require_permission("manga.create")?;
        if row.get::<_, String>(0) != actor.user_id
            || row.get::<_, String>(1) == "published"
            || row.get::<_, bool>(2)
        {
            return Err(ApiError::forbidden(
                "This title cannot be deleted by this account",
            ));
        }
    }
    require_mutable_manga(&tx, &id).await?;
    tx.execute("UPDATE platform.catalog_manga SET deleted_at=clock_timestamp(),updated_at=clock_timestamp(),version=version+1 WHERE id=$1",&[&id]).await?;
    audit(&tx, &actor.user_id, "manga", &id, "remove", None, None).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn relation_create(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let actor = mutation_principal(&state, &headers).await?;
    v::id(&id)?;
    let (related, kind, expected) = relation_fields(&payload, false)?;
    if id == related {
        return Err(ApiError::bad_request("A manga cannot relate to itself"));
    }
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    require_manga_edit(&tx, &id, &actor, "manga.relation.edit").await?;
    let version = relation_owner_version(&tx, &id, expected).await?;
    require_related_title(&tx, &related).await?;
    if tx.execute("INSERT INTO platform.catalog_relations(manga_id,related_id,relation_type,created_by) VALUES($1,$2,$3,$4) ON CONFLICT(manga_id,related_id) DO NOTHING",&[&id,&related,&kind,&actor.user_id]).await?==0{return Err(ApiError::conflict("Relation already exists"));}
    tx.execute("UPDATE platform.catalog_manga SET version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id]).await?;
    audit(
        &tx,
        &actor.user_id,
        "manga",
        &id,
        "relation_create",
        Some(version),
        Some(version + 1),
    )
    .await?;
    let data = relation_response(&tx, &id, &related, version + 1).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(data)))
}

/// An explicit instance-local edit, distinct from duplicate-conflicting POST.
/// `version` belongs to the owner title, never the related title's DTO version.
pub async fn relation_update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    let actor = mutation_principal(&state, &headers).await?;
    v::id(&id)?;
    let (related, kind, expected) = relation_fields(&payload, true)?;
    if id == related {
        return Err(ApiError::bad_request("A manga cannot relate to itself"));
    }
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    require_manga_edit(&tx, &id, &actor, "manga.relation.edit").await?;
    let version = relation_owner_version(&tx, &id, expected).await?;
    require_related_title(&tx, &related).await?;
    let row=tx.query_opt("SELECT relation_type FROM platform.catalog_relations WHERE manga_id=$1 AND related_id=$2 FOR UPDATE",&[&id,&related]).await?.ok_or_else(||ApiError::not_found("Relation not found"))?;
    let changed = row.get::<_, String>(0) != kind;
    let new_version = if changed {
        tx.execute("UPDATE platform.catalog_relations SET relation_type=$3 WHERE manga_id=$1 AND related_id=$2",&[&id,&related,&kind]).await?;
        tx.execute("UPDATE platform.catalog_manga SET version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id]).await?;
        audit(
            &tx,
            &actor.user_id,
            "manga",
            &id,
            "relation_update",
            Some(version),
            Some(version + 1),
        )
        .await?;
        version + 1
    } else {
        version
    };
    let data = relation_response(&tx, &id, &related, new_version).await?;
    tx.commit().await?;
    Ok(Json(data))
}

fn relation_fields(
    payload: &Value,
    required_version: bool,
) -> ApiResult<(String, String, Option<i32>)> {
    v::allowed(payload, &["related_id", "type", "version"])?;
    let related = v::string(payload, "related_id", 128, true)?.unwrap();
    v::id(&related)?;
    let expected = if required_version || payload.get("version").is_some() {
        Some(v::version(payload)?)
    } else {
        None
    };
    let kind = v::enum_value(
        payload,
        "type",
        &[
            "main_story",
            "side_story",
            "prequel",
            "sequel",
            "adapted_from",
            "spin_off",
            "based_on",
            "doujinshi",
            "monochrome",
            "colored",
            "preserialization",
            "serialization",
            "alternate_story",
            "alternate_version",
            "same_franchise",
            "shared_universe",
        ],
        true,
    )?
    .unwrap();
    Ok((related, kind, expected))
}

async fn relation_owner_version(
    tx: &Transaction<'_>,
    id: &str,
    expected: Option<i32>,
) -> ApiResult<i32> {
    let actual = tx
        .query_one(
            "SELECT version FROM platform.catalog_manga WHERE id=$1",
            &[&id],
        )
        .await?
        .get(0);
    if let Some(expected) = expected {
        check_version(actual, expected)?;
    }
    Ok(actual)
}
async fn require_related_title(tx: &Transaction<'_>, related: &str) -> ApiResult<()> {
    // The response projection rechecks both roots. Do not introduce reversed
    // owner/related row locks for concurrent reciprocal relations.
    if tx.query_opt("SELECT id FROM platform.catalog_manga WHERE id=$1 AND state='published' AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id)",&[&related]).await?.is_none(){return Err(ApiError::not_found("Related manga not found"));}
    Ok(())
}
async fn relation_response(
    tx: &Transaction<'_>,
    id: &str,
    related: &str,
    owner_version: i32,
) -> ApiResult<Value> {
    let relations: Value = tx
        .query_one("SELECT platform.catalog_relations_json($1)", &[&id])
        .await?
        .get(0);
    let mut data = relations
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == related)
        .cloned()
        .ok_or_else(|| ApiError::not_found("Related manga is unavailable"))?;
    data["manga_version"] = json!(owner_version);
    data["relation_scope"] = json!("instance-local");
    Ok(data)
}
pub async fn relation_delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<Response> {
    let actor = mutation_principal(&state, &headers).await?;
    v::id(&id)?;
    v::allowed(&payload, &["related_id", "version"])?;
    let related = v::string(&payload, "related_id", 128, true)?.unwrap();
    v::id(&related)?;
    let expected = if payload.get("version").is_some() {
        Some(v::version(&payload)?)
    } else {
        None
    };
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    require_manga_edit(&tx, &id, &actor, "manga.relation.edit").await?;
    let version = relation_owner_version(&tx, &id, expected).await?;
    if tx
        .execute(
            "DELETE FROM platform.catalog_relations WHERE manga_id=$1 AND related_id=$2",
            &[&id, &related],
        )
        .await?
        == 0
    {
        return Err(ApiError::not_found("Relation not found"));
    }
    tx.execute("UPDATE platform.catalog_manga SET version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id]).await?;
    audit(
        &tx,
        &actor.user_id,
        "manga",
        &id,
        "relation_delete",
        Some(version),
        Some(version + 1),
    )
    .await?;
    tx.commit().await?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        "x-weebdex-manga-version",
        HeaderValue::from_str(&(version + 1).to_string()).map_err(|_| ApiError::unavailable())?,
    );
    Ok(response)
}
fn staff_permission(actor: &impl CatalogMutationActor, perm: &str) -> ApiResult<()> {
    if !actor.is_moderator() {
        return Err(ApiError::forbidden("Staff permission required"));
    }
    actor.require_permission(perm)
}
fn check_version(actual: i32, expected: i32) -> ApiResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(ApiError::conflict("Entity changed; reload before editing"))
    }
}

async fn mutation_principal(state: &AppState, headers: &HeaderMap) -> ApiResult<SessionPrincipal> {
    crate::domains::auth::validate_mutation(state, headers)?;
    require_principal(state, headers).await
}
