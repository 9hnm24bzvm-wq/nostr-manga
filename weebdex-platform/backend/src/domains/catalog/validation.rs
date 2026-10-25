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
/// Preserve all three edit states: omitted, explicit clear, and a checked value.
/// Callers must use the outer presence bit when writing nullable columns; SQL
/// COALESCE alone would silently turn an explicit clear into an omitted edit.
pub fn nullable_string(value: &Value, key: &str, max: usize) -> ApiResult<Option<Option<String>>> {
    match value.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(_) => string(value, key, max, false).map(Some),
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
/// Transport validation is deliberately separate from the retained native enum.
/// Public writes follow the pinned API syntax, not an invented locale registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LanguageContract { Native, Public }
impl LanguageContract {
    pub(crate) fn validate(self, value: &str) -> ApiResult<()> {
        match self { Self::Native => language(value), Self::Public => public_language(value) }
    }
    fn localized_key(self, value: &str) -> ApiResult<()> {
        match self { Self::Native => language(value), Self::Public => localized_key(value) }
    }
}
/// Exact ^[a-z]{2}(-[a-z]{2})?$; no trimming, case folding or native mapping.
pub(crate) fn public_language(value: &str) -> ApiResult<()> {
    let b = value.as_bytes();
    if (b.len() == 2 && b.iter().all(u8::is_ascii_lowercase))
        || (b.len() == 5 && b[2] == b'-'
            && b[..2].iter().chain(&b[3..]).all(u8::is_ascii_lowercase)) {
        Ok(())
    } else { Err(ApiError::bad_request("Expected a lowercase two-letter language, optionally with a two-letter region")) }
}
/// Search has the distinct, broader ^[a-zA-Z\-]{2,5}$ contract.
pub(crate) fn public_search_language(value: &str) -> ApiResult<()> {
    if (2..=5).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_alphabetic() || b == b'-') {
        Ok(())
    } else { Err(ApiError::bad_request("Invalid language search filter")) }
}
/// The pinned LocalizedString pattern is on VALUES, not dictionary keys.
/// Preserve bounded opaque keys and ordinary text as an intentional extension;
/// do not reinterpret that upstream inconsistency as a locale enum.
pub(crate) fn localized_key(value: &str) -> ApiResult<()> {
    if value.is_empty() || value.len() > 100 || value.contains('\0') {
        Err(ApiError::bad_request("Invalid localized text key"))
    } else { Ok(()) }
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
    metadata_for(value, fields, LanguageContract::Native)
}
pub(crate) fn metadata_for(value: &Value, fields: &[&str], contract: LanguageContract) -> ApiResult<Value> {
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
                        contract.localized_key(k)?;
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
                    contract.validate(
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

#[cfg(test)]
mod nullable_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn nullable_string_preserves_presence_and_existing_string_validation() {
        assert_eq!(nullable_string(&json!({}), "field", 3).unwrap(), None);
        assert_eq!(nullable_string(&json!({"field":null}), "field", 3).unwrap(), Some(None));
        for (input, expected) in [("", ""), (" a ", "a"), ("猫犬", "猫犬")] {
            assert_eq!(nullable_string(&json!({"field":input}), "field", 3).unwrap(), Some(Some(expected.into())));
        }
        for input in [json!(4), json!(true), json!([]), json!({}), json!("abcd"), json!("a\0")] {
            assert!(nullable_string(&json!({"field":input}), "field", 3).is_err());
        }
        // This opt-in helper does not weaken the legacy non-null validator.
        assert!(string(&json!({"field":null}), "field", 3, false).is_err());
    }
}

#[cfg(test)]
mod public_language_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn public_language_contracts_do_not_redefine_native_vocabulary() {
        for value in ["ta", "en", "zz", "pt-br", "zh-hk", "aa-zz"] {
            assert!(public_language(value).is_ok(), "{value}");
        }
        for value in ["tam", "und", "zh-hant", "en-US", "EN", "a", "abc", "en-", "-en", "en-us-x", " ta", "ta ", "éé", "en\n", "en\0"] {
            assert!(public_language(value).is_err(), "{value:?}");
        }
        for value in ["TA", "tam", "und", "EN-us", "--", "a-b", "a----"] {
            assert!(public_search_language(value).is_ok(), "{value}");
        }
        for value in ["a", "zh-hant", "1a", "e_n", " en", "éé", "en\0"] {
            assert!(public_search_language(value).is_err(), "{value:?}");
        }
        assert!(language("tam").is_ok());
        assert!(language("ta").is_err());
        assert!(LanguageContract::Public.validate("ta").is_ok());
        assert!(LanguageContract::Native.validate("ta").is_err());
    }
    #[test]
    fn localized_public_keys_are_not_an_invented_language_enum() {
        let payload = json!({"alt_titles":{"zh-Hant":["保留"],"opaque label":["Ordinary text"]}});
        assert!(metadata_for(&payload, &["alt_titles"], LanguageContract::Public).is_ok());
        assert!(metadata(&payload, &["alt_titles"]).is_err());
        assert!(localized_key("").is_err());
        assert!(localized_key("bad\0").is_err());
    }
}
