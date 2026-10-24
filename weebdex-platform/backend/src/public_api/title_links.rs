//! The pinned MangaDex schema defines officialLinks as a separate string map.
//! Keys/values are opaque text, not provider UUIDs, language/region objects or
//! automatically followed URLs. These are bounded instance-local annotations.
use super::{PublicApiError, PublicApiResult};
use serde_json::Value;

pub(super) fn dictionary(value: &Value) -> PublicApiResult<Value> {
    let entries = value.as_object().filter(|v| v.len() <= 100)
        .ok_or_else(|| PublicApiError::bad_request("officialLinks must be a string dictionary of at most 100 entries"))?;
    for (key, value) in entries {
        let value = value.as_str().ok_or_else(|| PublicApiError::bad_request("officialLinks values must be strings"))?;
        if key.len() > 100 || value.len() > 2048 || [key.as_str(), value].iter().any(|text| text.contains('\0')) {
            return Err(PublicApiError::bad_request("officialLinks exceeds the key/value text budget"));
        }
    }
    Ok(value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn opaque_official_links_are_distinct_bounded_strings_without_reinterpretation() {
        let input = json!({"":"", "opaque-provider":"123", "literal":"  unchanged  ", "__proto__":"<script>text only</script>", "url":"https://publisher.example/作品"});
        assert_eq!(dictionary(&input).unwrap(), input);
        assert_eq!(dictionary(&json!({})).unwrap(), json!({}));
        for invalid in [Value::Null, json!([]), json!({"provider":{"language":"en","region":"US"}}), json!({"provider":1}), json!({"provider":null}), json!({"provider":"x".repeat(2049)}), json!({"provider":"題".repeat(683)}), json!({"x".repeat(101):"value"}), json!({"x":"\0"})] { assert!(dictionary(&invalid).is_err()); }
        let too_many = (0..101).map(|i| (i.to_string(),json!("value"))).collect::<serde_json::Map<_,_>>();
        assert!(dictionary(&Value::Object(too_many)).is_err());
    }
}
