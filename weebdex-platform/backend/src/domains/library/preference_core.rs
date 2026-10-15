//! Shared native preference validation. Callers retain their existing authority
//! and lock the affected user in the very transaction used for these checks.
use super::validation as v;
use crate::{
    contracts::library::PreferencesPatch,
    error::{ApiError, ApiResult},
};
use serde_json::{Value, json};
use tokio_postgres::Transaction;

pub const BASELINE_TEMPLATE: &str = "3d4e082d-f013-46d9-9b24-643bf65e27ea";
pub const NATIVE_KEYS: [&str; 11] = [
    "items_per_page",
    "languages",
    "show_safe",
    "show_suggestive",
    "show_erotic",
    "show_hentai",
    "tags",
    "translated_languages",
    "uploaders",
    "groups",
    "data_saver",
];

pub fn validated_patch(p: PreferencesPatch) -> ApiResult<(i32, Value)> {
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
    Ok((p.version, patch))
}

pub async fn validate_references(tx: &Transaction<'_>, patch: &Value) -> ApiResult<()> {
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
    Ok(())
}

/// An entire public document shares the native fields and preserves every opaque
/// key. The caller cannot select the stored revision. A supplied revision is
/// only an additional optimistic-concurrency check against the locked row.
pub async fn complete(
    tx: &Transaction<'_>,
    mut settings: Value,
    current: &Value,
) -> ApiResult<Value> {
    let object = settings
        .as_object_mut()
        .ok_or_else(|| ApiError::bad_request("settings must be an object"))?;
    let version = current
        .get("version")
        .and_then(Value::as_i64)
        .and_then(|v| i32::try_from(v).ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| ApiError::conflict("Invalid current preference revision"))?;
    if object
        .get("version")
        .is_some_and(|v| v.as_i64() != Some(i64::from(version)))
    {
        return Err(ApiError::conflict(
            "Preferences changed; reload before saving",
        ));
    }
    let mut known = serde_json::Map::new();
    for key in NATIVE_KEYS {
        let value = object.get(key).filter(|v| !v.is_null()).ok_or_else(|| {
            ApiError::bad_request(format!("settings must retain native preference {key}"))
        })?;
        known.insert(key.into(), value.clone());
    }
    known.insert("version".into(), json!(version));
    let p: PreferencesPatch = serde_json::from_value(Value::Object(known))
        .map_err(|_| ApiError::bad_request("Invalid native preference value"))?;
    let (_, patch) = validated_patch(p)?;
    validate_references(tx, &patch).await?;
    for (key, value) in patch.as_object().expect("validated object") {
        object.insert(key.clone(), value.clone());
    }
    object.insert(
        "version".into(),
        json!(
            version
                .checked_add(1)
                .ok_or_else(|| ApiError::conflict("Preference version limit reached"))?
        ),
    );
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_validation_retains_limits_and_normalization() {
        for value in ["0", "101", "nope"] {
            let p: PreferencesPatch =
                serde_json::from_value(json!({"version":1,"items_per_page":value})).unwrap();
            assert!(validated_patch(p).is_err());
        }
        let p =
            serde_json::from_value(json!({"version":1,"items_per_page":"020","data_saver":true}))
                .unwrap();
        assert_eq!(
            validated_patch(p).unwrap(),
            (1, json!({"items_per_page":"20","data_saver":true}))
        );
        let p = serde_json::from_value(json!({"version":1})).unwrap();
        assert_eq!(validated_patch(p).unwrap().1, json!({}));
    }
}
