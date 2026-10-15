use super::{preference_core, repository};
use crate::{
    contracts::library::PreferencesPatch,
    domains::auth,
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{Json, extract::State, http::HeaderMap};
use serde_json::{Value, json};

pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(p): Json<PreferencesPatch>,
) -> ApiResult<Json<Value>> {
    let actor = auth::require_principal(&state, &headers).await?;
    auth::validate_mutation(&state, &headers)?;
    actor.require_permission("self.preferences.update")?;
    let (version, mut patch) = preference_core::validated_patch(p)?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    repository::lock_user(&tx, &actor.user_id).await?;
    let current: Value = tx
        .query_one(
            "SELECT preferences FROM platform.users WHERE id=$1",
            &[&actor.user_id],
        )
        .await?
        .get(0);
    if current.get("version").and_then(Value::as_i64) != Some(version as i64) {
        return Err(ApiError::conflict(
            "Preferences changed; reload before saving",
        ));
    }
    preference_core::validate_references(&tx, &patch).await?;
    if !patch.as_object().expect("preference object").is_empty() {
        patch["version"] = json!(
            version
                .checked_add(1)
                .ok_or_else(|| ApiError::conflict("Preference version limit reached"))?
        );
        tx.execute("UPDATE platform.users SET preferences=preferences || $2,preferences_template_id=$3::text::uuid,updated_at=clock_timestamp() WHERE id=$1",&[&actor.user_id,&patch,&preference_core::BASELINE_TEMPLATE]).await?;
    }
    tx.commit().await?;
    let projection = repository::session_projection(&state.db, &actor.user_id).await?;
    Ok(Json(projection["preferences"].clone()))
}
