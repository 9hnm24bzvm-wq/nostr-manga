//! Read-only application-owned preference schema and private UUID projection.
//! The caller retains the original capability/account locks on this transaction.
use super::{PublicApiError, PublicApiResult, PublicUuid, ResourceKind};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tokio_postgres::Transaction;

pub const TEMPLATE_ID: &str = "513e0d48-ecc4-5301-b5eb-5bb369efaf22";
const TEMPLATE: &str = include_str!("../../../contracts/native-settings-template-v1.json");
pub(super) fn template() -> PublicApiResult<Value> {
    serde_json::from_str(TEMPLATE).map_err(|_| PublicApiError::unavailable())
}

fn string_values(value: &Value) -> PublicApiResult<Vec<String>> {
    let values = value.as_array().filter(|values| values.len() <= 100)
        .ok_or_else(PublicApiError::unavailable)?;
    let mut seen = BTreeSet::new();
    values.iter().map(|value| {
        let text = value.as_str().filter(|text| !text.is_empty() && text.len() <= 128 && !text.contains('\0'))
            .ok_or_else(PublicApiError::unavailable)?;
        Ok(text.to_owned())
    }).collect::<PublicApiResult<Vec<_>>>()
        .map(|values| values.into_iter().filter(|value| seen.insert(value.clone())).collect())
}

fn scalars(value: &Value) -> PublicApiResult<Map<String, Value>> {
    let source = value.as_object().ok_or_else(PublicApiError::unavailable)?;
    let version = source.get("version").and_then(Value::as_i64)
        .filter(|version| (1..=i64::from(i32::MAX)).contains(version))
        .ok_or_else(PublicApiError::unavailable)?;
    let mut settings = Map::new();
    settings.insert("version".into(), json!(version));
    for key in ["show_safe", "show_suggestive", "show_erotic", "show_hentai", "data_saver"] {
        if let Some(value) = source.get(key) {
            if !value.is_boolean() { return Err(PublicApiError::unavailable()); }
            settings.insert(key.into(), value.clone());
        }
    }
    if let Some(value) = source.get("items_per_page") {
        let text = value.as_str().ok_or_else(PublicApiError::unavailable)?;
        if !text.bytes().all(|byte| byte.is_ascii_digit()) || text.starts_with('0') || !text.parse::<u32>().is_ok_and(|n| (1..=100).contains(&n)) {
            return Err(PublicApiError::unavailable());
        }
        settings.insert("items_per_page".into(), value.clone());
    }
    for key in ["languages", "translated_languages"] {
        if let Some(value) = source.get(key) {
            let values = string_values(value)?;
            for language in &values {
                crate::domains::catalog::validation::language(language)
                    .map_err(|_| PublicApiError::unavailable())?;
            }
            settings.insert(key.into(), json!(values));
        }
    }
    Ok(settings)
}

pub(super) async fn read_in(tx: &Transaction<'_>, owner: &str) -> PublicApiResult<Value> {
    let row = tx.query_opt("SELECT preferences,platform.catalog_timestamp(updated_at) FROM platform.users WHERE id=$1 AND NOT disabled", &[&owner]).await?
        .ok_or_else(PublicApiError::not_found)?;
    let source: Value = row.get(0);
    let account_id = crate::domains::library::account_public_id_in(tx, owner).await?;
    let mut settings = scalars(&source)?;
    let mut unavailable = Map::new();
    for (key, kind, table, visibility) in [
        ("tags", ResourceKind::Tag, "platform.catalog_tags r", "TRUE"),
        ("groups", ResourceKind::Group, "platform.catalog_groups r", "r.deleted_at IS NULL AND platform.protocol_group_is_visible(r.id)"),
        ("uploaders", ResourceKind::User, "platform.users r", "NOT r.disabled"),
    ] {
        let Some(value) = source.get(key) else { continue; };
        let native = string_values(value)?;
        let rows = tx.query(&format!("SELECT r.id,p.public_id::text FROM {table} LEFT JOIN platform.public_resource_ids p ON p.kind=$1 AND p.native_id=r.id AND p.preferred WHERE r.id=ANY($2::text[]) AND {visibility}"), &[&kind.as_str(), &native]).await?;
        let mut mapped = BTreeMap::new();
        for row in rows {
            let id: Option<&str> = row.get(1);
            let id = PublicUuid::parse(id.ok_or_else(PublicApiError::unavailable)?)
                .map_err(|_| PublicApiError::unavailable())?;
            mapped.insert(row.get::<_, String>(0), id);
        }
        let missing = native.iter().filter(|id| !mapped.contains_key(*id)).count();
        if missing > 0 { unavailable.insert(key.into(), json!(missing)); }
        settings.insert(key.into(), json!(native.iter().filter_map(|id| mapped.get(id)).collect::<Vec<_>>()));
    }
    Ok(json!({"result":"ok","settings":settings,"template":TEMPLATE_ID,"updatedAt":row.get::<_,String>(1),
        "nostr":{"scope":"instance-account","accountId":account_id,"projection":"known-native-preferences-only","readOnlyProjection":true,
        "unavailableFilterCounts":unavailable,"nativeStorageUnchanged":true,"updatedAtSource":"native-account-row"}}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versioned_schema_and_scalar_projection_are_real_and_bounded() {
        let schema = template().unwrap();
        assert_eq!(schema["$id"], format!("urn:uuid:{TEMPLATE_ID}"));
        assert!(PublicUuid::parse(TEMPLATE_ID).is_ok());
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["properties"]["languages"]["items"]["enum"], json!(crate::domains::catalog::languages::LANGUAGES));
        assert_eq!(schema["properties"]["groups"]["items"]["format"], "uuid");
        let settings = scalars(&json!({"version":3,"items_per_page":"20","show_safe":false,"languages":["en","ja"],"private_secret":"must-not-escape"})).unwrap();
        assert_eq!(settings["version"], 3);
        assert!(!settings.contains_key("private_secret"));
        for bad in [json!({}),json!({"version":0}),json!({"version":1,"items_per_page":"020"}),json!({"version":1,"items_per_page":"101"}),json!({"version":1,"show_safe":"true"}),json!({"version":1,"languages":["unknown"]})] {
            assert!(scalars(&bad).is_err(), "{bad}");
        }
    }
}
