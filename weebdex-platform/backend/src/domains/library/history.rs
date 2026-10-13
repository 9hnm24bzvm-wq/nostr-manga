use super::{follows, query, repository as r, validation as v};
use crate::{
    contracts::library::HistoryBatch,
    domains::auth,
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    extract::{Path, RawQuery, State},
    http::HeaderMap,
    Json,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;
pub async fn search(
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(raw): RawQuery,
) -> ApiResult<Json<Value>> {
    let actor = auth::require_library_principal(&state, &headers).await?;
    let q = query::parse(raw)?;
    let ids = v::ids(&q.list("id"), 100)?;
    Ok(Json(
        r::nested_history(&state.db, &actor.user_id, &ids).await?,
    ))
}
pub async fn list(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let actor = auth::require_library_principal(&state, &headers).await?;
    if state.db.client().await?.query_opt("SELECT id FROM platform.catalog_manga WHERE id=$1 AND state='published' AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id)",&[&id]).await?.is_none(){return Err(ApiError::not_found("Manga not found"));}
    Ok(Json(
        r::nested_history(&state.db, &actor.user_id, &[id]).await?,
    ))
}
pub async fn feed(
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(raw): RawQuery,
) -> ApiResult<Json<Value>> {
    let actor = auth::require_library_principal(&state, &headers).await?;
    let q = query::parse(raw)?;
    Ok(Json(
        follows::chapters(&state, &actor, &q, true, false).await?,
    ))
}
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(p): Json<HistoryBatch>,
) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let actor = auth::require_principal(&state, &headers).await?;
    auth::validate_mutation(&state, &headers)?;
    actor.require_permission("self.preferences.update")?;
    let reads = v::ids(&p.read_ids.unwrap_or_default(), 400)?;
    let unreads = v::ids(&p.unread_ids.unwrap_or_default(), 400)?;
    if reads.len() + unreads.len() > 400 || reads.is_empty() && unreads.is_empty() {
        return Err(ApiError::bad_request(
            "History batch must contain between 1 and 400 chapters",
        ));
    }
    let read_set: BTreeSet<_> = reads.iter().collect();
    if unreads.iter().any(|id| read_set.contains(id)) {
        return Err(ApiError::bad_request(
            "A chapter cannot be both read and unread",
        ));
    }
    let ids: Vec<String> = reads.iter().chain(unreads.iter()).cloned().collect();
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    r::lock_user(&tx, &actor.user_id).await?;
    r::published_manga(&tx, &id).await?;
    let count:i64=tx.query_one("SELECT count(*) FROM platform.catalog_chapters WHERE id=ANY($1) AND manga_id=$2 AND deleted_at IS NULL AND platform.protocol_source_is_visible(protocol_event_id)",&[&ids,&id]).await?.get(0);
    if count as usize != ids.len() {
        return Err(ApiError::bad_request(
            "History batch includes an unknown chapter or a chapter from another manga",
        ));
    }
    let read_updated=tx.execute("INSERT INTO platform.library_chapter_history(user_id,manga_id,chapter_id) SELECT $1,$2,unnest($3::TEXT[]) ON CONFLICT(user_id,chapter_id) DO NOTHING",&[&actor.user_id,&id,&reads]).await?;
    let unread_updated=tx.execute("DELETE FROM platform.library_chapter_history WHERE user_id=$1 AND manga_id=$2 AND chapter_id=ANY($3)",&[&actor.user_id,&id,&unreads]).await?;
    tx.execute("UPDATE platform.library_chapter_progress SET completed=FALSE,version=version+1,updated_at=clock_timestamp() WHERE user_id=$1 AND chapter_id=ANY($2) AND completed",&[&actor.user_id,&unreads]).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"read_updated":read_updated,"unread_updated":unread_updated}),
    ))
}
