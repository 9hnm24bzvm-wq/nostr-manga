//! Public policy discovery and proof-of-key self-enrollment. No publisher grant.
use super::*;
use crate::enrollment::{Denied, Policy};
pub(super) fn description(policy: &Policy) -> Value {
    json!({"mode":if policy.enabled{"self_registration"}else{"operator_only"},"enabled":policy.enabled,"endpoint":"/enrollment","authorization":"NIP98 exact URL/method/body with fresh replay-protected proof","scope":"personal_account_only","max_new_accounts_per_hour":policy.max_new_accounts_per_hour,"max_self_enrolled_accounts":policy.max_self_enrolled_accounts,"budget":"persistent instance-wide fixed one-hour window and lifetime self-enrollment count","revocation_override":false})
}
pub(super) async fn policy(State(a): State<Shared>) -> Json<Value> {
    Json(description(&a.enrollment))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
pub(super) async fn enroll(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if !a.enrollment.enabled {
        return Err(forbidden("self-registration is disabled"));
    }
    if !a.projection_enabled {
        return Err(unavailable());
    }
    let _slot = a.event_slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"enrollment_slots_busy"})),
        )
    })?;
    let header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .filter(|h| h.len() <= 16384)
        .ok_or_else(|| forbidden("NIP98 authorization required"))?;
    let authorization =
        auth::token(header).map_err(|_| forbidden("invalid authorization event"))?;
    auth::http_scope(&authorization, &format!("{}{}", a.base_url, uri), "POST")
        .map_err(|_| forbidden("invalid URL, method, signature or freshness"))?;
    if !authorization.one("payload").is_ok_and(auth::valid_hash) {
        return Err(forbidden("signed body hash required"));
    }
    let body = group_body(&a, &uri, &authorization, request, 1024).await?;
    let _: Empty = serde_json::from_slice(&body).map_err(|_| bad("empty JSON object required"))?;
    let publisher = id(&authorization.pubkey).map_err(|_| forbidden("invalid account key"))?;
    let admission = a
        .repo
        .enroll_reader(publisher, &a.enrollment)
        .await
        .map_err(|e| match e.downcast_ref::<Denied>() {
            Some(Denied::Disabled | Denied::Blocked) => {
                forbidden("local admission policy denies enrollment")
            }
            Some(Denied::Limit) => (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({"error":"enrollment_creation_limit"})),
            ),
            None => {
                tracing::warn!(error=%e,"reader enrollment failed");
                unavailable()
            }
        })?;
    Ok((
        if admission.created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(
            json!({"account":admission,"grants":["personal_account"],"publisher_permission_granted":false}),
        ),
    ))
}
