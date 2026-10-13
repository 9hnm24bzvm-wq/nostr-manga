//! Persisted views and source DTO statistics. View counters never rely on fabricated seeds.
use crate::{
    client_peer::ClientPeer,
    domains::catalog::validation as v,
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rand::RngCore;
use serde_json::Value;
use sha2::{Digest, Sha256};
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/analytics/manga/{id}/views", post(manga_view))
        .route("/analytics/chapter/{id}/views", post(chapter_view))
        .route("/manga/{id}/statistics", get(manga_stats))
        .route("/chapter/{id}/statistics", get(chapter_stats))
        .route("/user/{id}/statistics", get(user_stats))
        .route("/group/{id}/statistics", get(group_stats))
        .route("/thread/{id}/statistics", get(thread_stats))
}
async fn manga_view(
    State(s): State<AppState>,
    h: HeaderMap,
    peer: Option<Extension<ClientPeer>>,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    view(&s, &h, peer.map(|p| p.0), "manga", &id).await
}
async fn chapter_view(
    State(s): State<AppState>,
    h: HeaderMap,
    peer: Option<Extension<ClientPeer>>,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    view(&s, &h, peer.map(|p| p.0), "chapter", &id).await
}
fn visitor_cookie(h: &HeaderMap) -> Option<String> {
    let mut cookies = h
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|h| h.to_str().ok())
        .flat_map(|h| h.split(';'))
        .filter_map(|v| v.trim().split_once('='))
        .filter(|(k, _)| *k == "weebdex_visitor")
        .map(|(_, v)| v);
    let first = cookies.next()?;
    if cookies.next().is_none() && first.len() == 64 && first.bytes().all(|b| b.is_ascii_hexdigit())
    {
        Some(first.to_owned())
    } else {
        None
    }
}
async fn view(
    s: &AppState,
    h: &HeaderMap,
    peer: Option<ClientPeer>,
    kind: &str,
    id: &str,
) -> ApiResult<Response> {
    v::id(id)?;
    // Cross-site analytics submissions are refused; genuine navigations on the configured app remain public.
    if h.get("sec-fetch-site")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == "cross-site")
    {
        return Err(ApiError::forbidden("Cross-site analytics are not accepted"));
    }
    if let Some(origin) = h.get(header::ORIGIN) {
        if origin.to_str().ok() != Some(s.config.public_origin.as_str()) {
            return Err(ApiError::forbidden(
                "Cross-origin analytics are not accepted",
            ));
        }
    }
    let existing = visitor_cookie(h);
    let visitor = existing.clone().unwrap_or_else(|| {
        let mut bytes = [0_u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        hex::encode(bytes)
    });
    let visitor_hash = Sha256::digest(format!("weebdex-local-view:{visitor}").as_bytes()).to_vec();
    // Central middleware trusts only configured transport proxies and exact validated
    // edge-overwritten chains. Raw headers/cookies never choose this rate identity.
    let peer_hash = Sha256::digest(
        format!(
            "weebdex-local-peer:{}",
            peer.map(|p| p.rate_identity())
                .unwrap_or_else(|| "unavailable-transport".into())
        )
        .as_bytes(),
    )
    .to_vec();
    let mut db = s.db.client().await?;
    let tx = db.transaction().await?;
    let target = if kind == "manga" {
        "SELECT id FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND state='published' AND platform.protocol_source_is_visible(protocol_series_id) FOR SHARE"
    } else {
        "SELECT c.id FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=$1 AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND NOT c.is_unavailable AND m.deleted_at IS NULL AND m.state='published' AND platform.protocol_source_is_visible(m.protocol_series_id) FOR SHARE OF c,m"
    };
    if tx.query_opt(target, &[&id]).await?.is_none() {
        return Err(ApiError::not_found("View target not found"));
    }
    let rate:i32=tx.query_one("INSERT INTO platform.analytics_rate_limits(peer_hash,bucket,count) VALUES($1,floor(extract(epoch FROM clock_timestamp())/60)::BIGINT,1) ON CONFLICT(peer_hash,bucket) DO UPDATE SET count=platform.analytics_rate_limits.count+1 RETURNING count",&[&peer_hash]).await?.get(0);
    if rate > 120 {
        return Err(ApiError::busy());
    }
    let inserted=tx.execute("INSERT INTO platform.catalog_view_events(entity_type,entity_id,visitor_hash,bucket) VALUES($1,$2,$3,floor(extract(epoch FROM clock_timestamp())/3600)::BIGINT) ON CONFLICT(entity_type,entity_id,visitor_hash,bucket) DO NOTHING",&[&kind,&id,&visitor_hash]).await?;
    if inserted > 0 {
        let sql = if kind == "manga" {
            "INSERT INTO platform.catalog_manga_counters(manga_id,views) VALUES($1,1) ON CONFLICT(manga_id) DO UPDATE SET views=platform.catalog_manga_counters.views+1"
        } else {
            "INSERT INTO platform.catalog_chapter_counters(chapter_id,views) VALUES($1,1) ON CONFLICT(chapter_id) DO UPDATE SET views=platform.catalog_chapter_counters.views+1"
        };
        tx.execute(sql, &[&id]).await?;
    }
    // Keep the short-lived limiter bounded. Historical view rows are aggregate evidence for rankings.
    tx.execute("DELETE FROM platform.analytics_rate_limits WHERE bucket<floor(extract(epoch FROM clock_timestamp())/60)::BIGINT-2",&[]).await?;
    tx.commit().await?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    if existing.is_none() {
        let secure = if s.config.public_origin.starts_with("https://") {
            "; Secure"
        } else {
            ""
        };
        let value=format!("weebdex_visitor={visitor}; Path=/api/analytics; HttpOnly; SameSite=Lax; Max-Age=31536000{secure}").parse().map_err(|_|ApiError::unavailable())?;
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    Ok(response)
}
async fn manga_stats(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let db = s.db.client().await?;
    let row=db.query_opt("SELECT jsonb_build_object('chapters',(SELECT count(*) FROM platform.catalog_chapters c WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id)),'follows',COALESCE(s.follows,0),'views',COALESCE(s.views,0)) FROM platform.catalog_manga m LEFT JOIN platform.catalog_manga_counters s ON s.manga_id=m.id WHERE m.id=$1 AND m.deleted_at IS NULL AND m.state='published' AND platform.protocol_source_is_visible(m.protocol_series_id)",&[&id]).await?.ok_or_else(||ApiError::not_found("Manga not found"))?;
    Ok(Json(row.get(0)))
}
async fn chapter_stats(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let db = s.db.client().await?;
    let row=db.query_opt("SELECT jsonb_build_object('views',COALESCE(s.views,0)) FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id LEFT JOIN platform.catalog_chapter_counters s ON s.chapter_id=c.id WHERE c.id=$1 AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND platform.protocol_source_is_visible(m.protocol_series_id)",&[&id]).await?.ok_or_else(||ApiError::not_found("Chapter not found"))?;
    Ok(Json(row.get(0)))
}
async fn user_stats(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let db = s.db.client().await?;
    let value: Option<Value> = db
        .query_one("SELECT platform.analytics_user_stats($1)", &[&id])
        .await?
        .get(0);
    Ok(Json(
        value.ok_or_else(|| ApiError::not_found("User not found"))?,
    ))
}
async fn group_stats(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let db = s.db.client().await?;
    let row=db.query_opt("SELECT jsonb_build_object('chapters',(SELECT count(*) FROM platform.catalog_chapter_groups cg JOIN platform.catalog_chapters c ON c.id=cg.chapter_id JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE cg.group_id=g.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND platform.protocol_source_is_visible(m.protocol_series_id))) FROM platform.catalog_groups g WHERE g.id=$1 AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id)",&[&id]).await?.ok_or_else(||ApiError::not_found("Group not found"))?;
    Ok(Json(row.get(0)))
}
async fn thread_stats(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let db = s.db.client().await?;
    if !db
        .query_one("SELECT platform.community_thread_visible($1)", &[&id])
        .await?
        .get::<_, bool>(0)
    {
        return Err(ApiError::not_found("Thread not found"));
    }
    let value = db
        .query_one("SELECT platform.community_thread_stats($1)", &[&id])
        .await?
        .get(0);
    Ok(Json(value))
}
