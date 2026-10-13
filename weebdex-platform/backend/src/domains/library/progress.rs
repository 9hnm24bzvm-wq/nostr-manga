//! Additive private page progress; the original frontend only marks chapter history on mount.
use super::{repository as r, validation as v};
use crate::{
    contracts::library::ProgressPatch,
    domains::auth,
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde_json::Value;
const PROJECTION: &str = "jsonb_build_object('chapter_id',p.chapter_id,'page',p.page,'completed',p.completed,'version',p.version,'updated_at',platform.catalog_timestamp(p.updated_at))";
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let actor = auth::require_library_principal(&state, &headers).await?;
    let row=state.db.client().await?.query_opt(&format!("SELECT {PROJECTION} FROM platform.library_chapter_progress p JOIN platform.catalog_chapters c ON c.id=p.chapter_id JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE p.user_id=$1 AND p.chapter_id=$2 AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)"),&[&actor.user_id,&id]).await?.ok_or_else(||ApiError::not_found("No saved page progress"))?;
    Ok(Json(row.get(0)))
}
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(p): Json<ProgressPatch>,
) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let actor = auth::require_principal(&state, &headers).await?;
    auth::validate_mutation(&state, &headers)?;
    actor.require_permission("self.preferences.update")?;
    if p.version < 0 || p.page < 0 {
        return Err(ApiError::bad_request(
            "Progress version and zero-based page must be nonnegative",
        ));
    }
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    r::lock_user(&tx, &actor.user_id).await?;
    let chapter=tx.query_opt("SELECT c.manga_id,jsonb_array_length(COALESCE(c.metadata->'data','[]'::jsonb)) FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=$1 AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id) FOR KEY SHARE OF c",&[&id]).await?.ok_or_else(||ApiError::not_found("Chapter not found"))?;
    let count: i32 = chapter.get(1);
    if count <= 0 || p.page >= count {
        return Err(ApiError::bad_request(
            "Progress page is outside the chapter",
        ));
    }
    let manga: String = chapter.get(0);
    let current=tx.query_opt("SELECT version,completed FROM platform.library_chapter_progress WHERE user_id=$1 AND chapter_id=$2 FOR UPDATE",&[&actor.user_id,&id]).await?;
    let version = current
        .as_ref()
        .map(|row| row.get::<_, i32>(0))
        .unwrap_or(0);
    if p.version != version {
        return Err(ApiError::conflict(
            "Page progress changed; reload before saving",
        ));
    }
    let completed = p
        .completed
        .unwrap_or_else(|| current.as_ref().is_some_and(|r| r.get(1)));
    let next = version
        .checked_add(1)
        .ok_or_else(|| ApiError::conflict("Progress version limit reached"))?;
    tx.execute("INSERT INTO platform.library_chapter_progress(user_id,chapter_id,page,completed,version) VALUES($1,$2,$3,$4,$5) ON CONFLICT(user_id,chapter_id) DO UPDATE SET page=EXCLUDED.page,completed=EXCLUDED.completed,version=EXCLUDED.version,updated_at=clock_timestamp()",&[&actor.user_id,&id,&p.page,&completed,&next]).await?;
    if completed {
        tx.execute("INSERT INTO platform.library_chapter_history(user_id,manga_id,chapter_id) VALUES($1,$2,$3) ON CONFLICT(user_id,chapter_id) DO NOTHING",&[&actor.user_id,&manga,&id]).await?;
    }
    let value:Value=tx.query_one(&format!("SELECT {PROJECTION} FROM platform.library_chapter_progress p WHERE p.user_id=$1 AND p.chapter_id=$2"),&[&actor.user_id,&id]).await?.get(0);
    tx.commit().await?;
    Ok(Json(value))
}
