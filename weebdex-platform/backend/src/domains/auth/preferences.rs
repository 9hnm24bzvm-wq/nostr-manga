use super::principal;
use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
};
use serde_json::json;

pub(super) async fn check(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let current = principal::optional_principal(&state, &headers)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    if current.recovery_session {
        // This is a reset capability, never a normal Session/User identity.
        // No personal projection is queried for a limited recovery cookie.
        let mut response = Json(json!({"session_purpose":"recovery","recovery":{"can_reset_password":true},"permissions":[],"roles":[]})).into_response();
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        return Ok(response);
    }
    current.require_browser_session()?;
    // Relationships are projected from library/catalog persisted rows; auth does
    // not guess follow state or treat cached membership as authorization.
    let projection =
        crate::domains::library::session_projection(&state.db, &current.user_id).await?;
    let preferences = projection
        .get("preferences")
        .ok_or_else(ApiError::unavailable)?;
    let follows = projection
        .get("follows")
        .ok_or_else(ApiError::unavailable)?;
    let groups = projection.get("groups").ok_or_else(ApiError::unavailable)?;
    let permissions = current
        .permissions
        .iter()
        .filter(|permission| current.has_perm(permission))
        .cloned()
        .collect::<Vec<_>>();
    let mut response=Json(json!({"identity":{"id":current.user_id,"internal_id":current.user_id,"email":current.email,"username":current.username,"is_verified":current.verified},"preferences":preferences,"follows":follows,"groups":groups,"roles":current.roles,"permissions":permissions})).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
