//! Explicitly additive instance-only custom lists. Private default; unlisted links
//! are direct-access only; public discovery never returns another account's private lists.
use super::{query, repository as r, validation as v};
use crate::{
    contracts::library::{ListCreate, ListPatch, ListVisibility, VersionRequest},
    domains::auth,
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    Json,
    extract::{Path, RawQuery, State},
    http::{HeaderMap, StatusCode},
};
use serde_json::{Value, json};
async fn authorize(state: &AppState, headers: &HeaderMap) -> ApiResult<auth::SessionPrincipal> {
    let actor = auth::require_principal(state, headers).await?;
    auth::validate_mutation(state, headers)?;
    actor.require_permission("self.preferences.update")?;
    Ok(actor)
}
fn validate_visibility(
    actor: &auth::SessionPrincipal,
    visibility: ListVisibility,
) -> ApiResult<()> {
    if visibility != ListVisibility::Private {
        actor.require_verified()?;
    }
    Ok(())
}
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(p): Json<ListCreate>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let actor = authorize(&state, &headers).await?;
    let name = v::text(&p.name, 200, true)?;
    let description = v::text(p.description.as_deref().unwrap_or(""), 20_000, false)?;
    let visibility = p.visibility.unwrap_or(ListVisibility::Private);
    validate_visibility(&actor, visibility)?;
    let ids = v::ids(&p.manga_ids.unwrap_or_default(), 100)?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    r::lock_user(&tx, &actor.user_id).await?;
    let count: i64 = tx
        .query_one(
            "SELECT count(*) FROM platform.library_lists WHERE user_id=$1",
            &[&actor.user_id],
        )
        .await?
        .get(0);
    if count >= 100 {
        return Err(ApiError::bad_request(
            "An account may have at most 100 lists",
        ));
    }
    r::validate_manga_ids(&tx, &ids).await?;
    let id = v::new_id();
    tx.execute("INSERT INTO platform.library_lists(id,user_id,name,description,visibility) VALUES($1,$2,$3,$4,$5)",&[&id,&actor.user_id,&name,&description,&visibility.as_str()]).await?;
    r::replace_members(&tx, &id, &ids).await?;
    let value = r::list_json(&tx, &id).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(value)))
}
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let actor = auth::optional_personal_principal(&state, &headers).await?;
    let db = state.db.client().await?;
    let reader = actor.as_ref().map(|a| a.user_id.as_str());
    Ok(Json(r::authorized_list_json(&**db, &id, reader).await?))
}
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(p): Json<ListPatch>,
) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let actor = authorize(&state, &headers).await?;
    if p.version < 1 {
        return Err(ApiError::bad_request("A positive list version is required"));
    }
    let name = p
        .name
        .as_deref()
        .map(|n| v::text(n, 200, true))
        .transpose()?;
    let description = p
        .description
        .as_deref()
        .map(|n| v::text(n, 20_000, false))
        .transpose()?;
    if let Some(visibility) = p.visibility {
        validate_visibility(&actor, visibility)?;
    }
    let ids = p
        .manga_ids
        .as_ref()
        .map(|ids| v::ids(ids, 100))
        .transpose()?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    r::lock_user(&tx, &actor.user_id).await?;
    let row = tx
        .query_opt(
            "SELECT version FROM platform.library_lists WHERE id=$1 AND user_id=$2 FOR UPDATE",
            &[&id, &actor.user_id],
        )
        .await?
        .ok_or_else(|| ApiError::not_found("List not found"))?;
    if row.get::<_, i32>(0) != p.version {
        return Err(ApiError::conflict("List changed; reload before editing"));
    }
    if name.is_none() && description.is_none() && p.visibility.is_none() && ids.is_none() {
        return Err(ApiError::bad_request("No list changes supplied"));
    }
    if let Some(ids) = ids {
        r::replace_members(&tx, &id, &ids).await?;
    }
    let visibility = p.visibility.map(|v| v.as_str());
    let version = p
        .version
        .checked_add(1)
        .ok_or_else(|| ApiError::conflict("List version limit reached"))?;
    tx.execute("UPDATE platform.library_lists SET name=COALESCE($2,name),description=COALESCE($3,description),visibility=COALESCE($4,visibility),version=$5,updated_at=clock_timestamp() WHERE id=$1",&[&id,&name,&description,&visibility,&version]).await?;
    let value = r::list_json(&tx, &id).await?;
    tx.commit().await?;
    Ok(Json(value))
}
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(p): Json<VersionRequest>,
) -> ApiResult<StatusCode> {
    v::id(&id)?;
    let actor = authorize(&state, &headers).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    r::lock_user(&tx, &actor.user_id).await?;
    let row = tx
        .query_opt(
            "SELECT version FROM platform.library_lists WHERE id=$1 AND user_id=$2 FOR UPDATE",
            &[&id, &actor.user_id],
        )
        .await?
        .ok_or_else(|| ApiError::not_found("List not found"))?;
    if row.get::<_, i32>(0) != p.version {
        return Err(ApiError::conflict("List changed; reload before deleting"));
    }
    tx.execute(
        "DELETE FROM platform.library_lists WHERE id=$1 AND user_id=$2",
        &[&id, &actor.user_id],
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn feed(
    state: &AppState,
    headers: &HeaderMap,
    owner: &str,
    raw: Option<String>,
    mine: bool,
) -> ApiResult<Value> {
    v::id(owner)?;
    let actor = auth::optional_personal_principal(state, headers).await?;
    if mine
        && actor
            .as_ref()
            .is_none_or(|a| a.user_id != owner || a.require_library_read().is_err())
    {
        return Err(ApiError::unauthorized());
    }
    let q = query::parse(raw)?;
    let is_owner = actor
        .as_ref()
        .is_some_and(|a| a.user_id == owner && a.require_library_read().is_ok());
    let db = state.db.client().await?;
    let total:i64=db.query_one("SELECT count(*) FROM platform.library_lists WHERE user_id=$1 AND ($2 OR visibility='public')",&[&owner,&is_owner]).await?.get(0);
    let offset = (q.page - 1) * q.limit;
    let rows=db.query("SELECT jsonb_build_object('id',id,'name',name,'description',description,'visibility',visibility,'version',version,'created_at',platform.catalog_timestamp(created_at),'updated_at',platform.catalog_timestamp(updated_at),'member_count',(SELECT count(*) FROM platform.library_list_members lm WHERE lm.list_id=l.id)) FROM platform.library_lists l WHERE user_id=$1 AND ($2 OR visibility='public') ORDER BY created_at DESC,id ASC LIMIT $3 OFFSET $4",&[&owner,&is_owner,&q.limit,&offset]).await?;
    Ok(
        json!({"total":total,"limit":q.limit,"page":q.page,"data":rows.iter().map(|r|r.get::<_,Value>(0)).collect::<Vec<_>>() }),
    )
}
pub async fn mine(
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(raw): RawQuery,
) -> ApiResult<Json<Value>> {
    let actor = auth::require_library_principal(&state, &headers).await?;
    Ok(Json(
        feed(&state, &headers, &actor.user_id, raw, true).await?,
    ))
}
pub async fn by_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    RawQuery(raw): RawQuery,
) -> ApiResult<Json<Value>> {
    Ok(Json(feed(&state, &headers, &id, raw, false).await?))
}
