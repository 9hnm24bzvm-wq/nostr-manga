use crate::error::{ApiError, ApiResult};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub fn object(value: &Value) -> ApiResult<&Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| ApiError::bad_request("Payload must be an object"))
}
pub fn allowed(value: &Value, fields: &[&str]) -> ApiResult<()> {
    for field in object(value)?.keys() {
        if !fields.contains(&field.as_str()) {
            return Err(ApiError::bad_request(format!("Unknown field: {field}")));
        }
    }
    Ok(())
}
pub fn string(value: &Value, key: &str, max: usize, required: bool) -> ApiResult<Option<String>> {
    match value.get(key) {
        None if !required => Ok(None),
        Some(Value::String(text))
            if (!required || !text.trim().is_empty())
                && text.chars().count() <= max
                && !text.contains('\0') =>
        {
            Ok(Some(text.trim().to_owned()))
        }
        _ => Err(ApiError::bad_request(format!("Invalid {key}"))),
    }
}
pub fn id(value: &str) -> ApiResult<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b':'))
    {
        Err(ApiError::bad_request("Invalid identifier"))
    } else {
        Ok(())
    }
}
pub fn ids(value: &Value, key: &str, required: bool) -> ApiResult<Option<Vec<String>>> {
    match value.get(key) {
        None if !required => Ok(None),
        Some(Value::Array(values)) if values.len() <= 100 => {
            let mut result = BTreeSet::new();
            for value in values {
                let v = value
                    .as_str()
                    .ok_or_else(|| ApiError::bad_request(format!("Invalid {key}")))?;
                id(v)?;
                result.insert(v.to_owned());
            }
            Ok(Some(result.into_iter().collect()))
        }
        _ => Err(ApiError::bad_request(format!("Invalid {key}"))),
    }
}
pub fn language(value: &str) -> ApiResult<()> {
    if super::languages::LANGUAGES.contains(&value) {
        Ok(())
    } else {
        Err(ApiError::bad_request("Unsupported language code"))
    }
}
pub fn enum_value(
    value: &Value,
    key: &str,
    choices: &[&str],
    required: bool,
) -> ApiResult<Option<String>> {
    let result = string(value, key, 32, required)?;
    if result
        .as_ref()
        .is_some_and(|v| !choices.contains(&v.as_str()))
    {
        return Err(ApiError::bad_request(format!("Invalid {key}")));
    }
    Ok(result)
}
pub fn boolean(value: &Value, key: &str) -> ApiResult<Option<bool>> {
    match value.get(key) {
        None => Ok(None),
        Some(Value::Bool(v)) => Ok(Some(*v)),
        _ => Err(ApiError::bad_request(format!("Invalid {key}"))),
    }
}
pub fn version(value: &Value) -> ApiResult<i32> {
    value
        .get("version")
        .and_then(Value::as_i64)
        .filter(|v| *v > 0 && *v <= i32::MAX as i64)
        .map(|v| v as i32)
        .ok_or_else(|| ApiError::bad_request("A positive integer version is required"))
}
pub fn metadata(value: &Value, fields: &[&str]) -> ApiResult<Value> {
    let mut out = Map::new();
    for key in fields {
        if let Some(v) = value.get(*key) {
            if ["alt_titles", "links"].contains(key) {
                let entries = v
                    .as_object()
                    .ok_or_else(|| ApiError::bad_request(format!("Invalid {key}")))?;
                if entries.len() > 100 {
                    return Err(ApiError::bad_request("Too many metadata entries"));
                }
                for (k, v) in entries {
                    if k.len() > 100 {
                        return Err(ApiError::bad_request("Invalid metadata key"));
                    }
                    if *key == "alt_titles" {
                        language(k)?;
                        let titles = v
                            .as_array()
                            .ok_or_else(|| ApiError::bad_request("Invalid alternate titles"))?;
                        if titles.len() > 50
                            || titles.iter().any(|v| {
                                v.as_str()
                                    .is_none_or(|s| s.is_empty() || s.chars().count() > 500)
                            })
                        {
                            return Err(ApiError::bad_request("Invalid alternate titles"));
                        }
                    } else if v.as_str().is_none_or(|s| s.len() > 2048) {
                        return Err(ApiError::bad_request("Invalid external link"));
                    }
                }
            } else if *key == "focused_languages" {
                let langs = v
                    .as_array()
                    .ok_or_else(|| ApiError::bad_request("Invalid focused languages"))?;
                if langs.len() > 100 {
                    return Err(ApiError::bad_request("Too many languages"));
                }
                for lang in langs {
                    language(
                        lang.as_str()
                            .ok_or_else(|| ApiError::bad_request("Invalid language"))?,
                    )?;
                }
            } else {
                string(
                    value,
                    key,
                    if *key == "description" { 50_000 } else { 2048 },
                    false,
                )?;
            }
            out.insert((*key).to_owned(), v.clone());
        }
    }
    Ok(Value::Object(out))
}
pub fn new_id(_prefix: &str) -> String {
    use rand::Rng;
    rand::rngs::OsRng
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(10)
        .map(char::from)
        .collect()
}
