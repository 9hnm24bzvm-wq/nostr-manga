use super::{repository, validation as v};
use crate::{
    contracts::library::PreferencesPatch,
    domains::auth,
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{extract::State, http::HeaderMap, Json};
use serde_json::{json, Value};

pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(p): Json<PreferencesPatch>,
) -> ApiResult<Json<Value>> {
    let actor = auth::require_principal(&state, &headers).await?;
    auth::validate_mutation(&state, &headers)?;
    actor.require_permission("self.preferences.update")?;
    if p.version < 1 {
        return Err(ApiError::bad_request(
            "A positive preferences version is required",
        ));
    }
    let mut patch = json!({});
    if let Some(n) = p.items_per_page {
        let value = n
            .parse::<u32>()
            .map_err(|_| ApiError::bad_request("items_per_page must be a decimal count"))?;
        if !(1..=100).contains(&value) {
            return Err(ApiError::bad_request(
                "items_per_page must be between 1 and 100",
            ));
        }
        patch["items_per_page"] = json!(value.to_string());
    }
    for (key, values) in [
        ("languages", p.languages),
        ("translated_languages", p.translated_languages),
    ] {
        if let Some(values) = values {
            let values = v::ids(&values, 100)?;
            for lang in &values {
                crate::domains::catalog::validation::language(lang)?;
            }
            patch[key] = json!(values);
        }
    }
    for (key, value) in [
        ("show_safe", p.show_safe),
        ("show_suggestive", p.show_suggestive),
        ("show_erotic", p.show_erotic),
        ("show_hentai", p.show_hentai),
        ("data_saver", p.data_saver),
    ] {
        if let Some(value) = value {
            patch[key] = json!(value);
        }
    }
    for (key, values) in [
        ("tags", p.tags),
        ("uploaders", p.uploaders),
        ("groups", p.groups),
    ] {
        if let Some(values) = values {
            patch[key] = json!(v::ids(&values, 100)?);
        }
    }
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
    if current.get("version").and_then(Value::as_i64) != Some(p.version as i64) {
        return Err(ApiError::conflict(
            "Preferences changed; reload before saving",
        ));
    }
    for (key, table, column, condition) in [
        ("tags", "platform.catalog_tags", "id", "TRUE"),
        ("uploaders", "platform.users", "id", "NOT disabled"),
        (
            "groups",
            "platform.catalog_groups",
            "id",
            "deleted_at IS NULL AND platform.protocol_group_is_visible(id)",
        ),
    ] {
        if let Some(ids) = patch.get(key) {
            let ids: Vec<String> = serde_json::from_value(ids.clone())
                .map_err(|_| ApiError::bad_request("Invalid preference identifiers"))?;
            let count: i64 = tx
                .query_one(
                    &format!("SELECT count(*) FROM {table} WHERE {column}=ANY($1) AND {condition}"),
                    &[&ids],
                )
                .await?
                .get(0);
            if count as usize != ids.len() {
                return Err(ApiError::bad_request(format!("Unknown {key} preference")));
            }
        }
    }
    if !patch.as_object().expect("preference object").is_empty() {
        patch["version"] = json!(p
            .version
            .checked_add(1)
            .ok_or_else(|| ApiError::conflict("Preference version limit reached"))?);
        tx.execute("UPDATE platform.users SET preferences=preferences || $2,updated_at=clock_timestamp() WHERE id=$1",&[&actor.user_id,&patch]).await?;
    }
    tx.commit().await?;
    let projection = repository::session_projection(&state.db, &actor.user_id).await?;
    Ok(Json(projection["preferences"].clone()))
}
