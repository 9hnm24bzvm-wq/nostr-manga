//! Offline, bounded JSON Schema compilation. This is a resource policy around a
//! vetted validator, not an alternative schema evaluator.
use super::{PublicApiError, PublicApiResult};
use jsonschema::{Draft, PatternOptions, Retrieve, Uri, Validator};
use serde_json::Value;
use std::sync::{Arc, LazyLock};
use tokio::sync::Semaphore;

pub const TEMPLATE_BYTES: usize = 65_536;
pub const SETTINGS_BYTES: usize = 131_072;
static VALIDATORS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(2)));
struct Offline;
impl Retrieve for Offline {
    fn retrieve(&self, _: &Uri<String>) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err("External schema retrieval is forbidden".into())
    }
}
fn draft(schema: &Value) -> PublicApiResult<Draft> {
    match schema.get("$schema").and_then(Value::as_str) {
        None
        | Some("http://json-schema.org/draft-07/schema#")
        | Some("https://json-schema.org/draft-07/schema") => Ok(Draft::Draft7),
        Some("http://json-schema.org/draft-04/schema#") => Ok(Draft::Draft4),
        Some("http://json-schema.org/draft-06/schema#") => Ok(Draft::Draft6),
        Some("https://json-schema.org/draft/2019-09/schema") => Ok(Draft::Draft201909),
        Some("https://json-schema.org/draft/2020-12/schema") => Ok(Draft::Draft202012),
        _ => Err(PublicApiError::bad_request(
            "Unsupported settings schema dialect",
        )),
    }
}
/// Value depth/node caps apply to opaque keys too, before any schema evaluation.
pub fn value_budget(value: &Value) -> PublicApiResult<()> {
    fn visit(value: &Value, depth: usize, nodes: &mut usize) -> PublicApiResult<()> {
        *nodes += 1;
        if depth > 16 || *nodes > 4096 {
            return Err(PublicApiError::bad_request(
                "Settings JSON exceeds the depth or node budget",
            ));
        }
        match value {
            Value::Object(values) => {
                for (key, value) in values {
                    if key.len() > 256 {
                        return Err(PublicApiError::bad_request(
                            "Settings key exceeds the byte budget",
                        ));
                    }
                    visit(value, depth + 1, nodes)?;
                }
            }
            Value::Array(values) => {
                for value in values {
                    visit(value, depth + 1, nodes)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    visit(value, 0, &mut 0)
}
fn policy(schema: &Value) -> PublicApiResult<()> {
    if !schema.is_object() {
        return Err(PublicApiError::bad_request(
            "A JSON Schema object is required",
        ));
    }
    draft(schema)?;
    fn visit(
        value: &Value,
        root: &Value,
        path: String,
        stack: &mut Vec<String>,
        cost: &mut usize,
        patterns: &mut usize,
        depth: usize,
    ) -> PublicApiResult<()> {
        *cost += 1;
        if depth > 20 || *cost > 2048 {
            return Err(PublicApiError::bad_request(
                "Settings schema exceeds the expansion budget",
            ));
        }
        match value {
            Value::Object(values) => {
                if values.contains_key("$dynamicRef") || values.contains_key("$recursiveRef") {
                    return Err(PublicApiError::bad_request(
                        "Recursive settings schemas are outside the operation budget",
                    ));
                }
                if let Some(id) = values
                    .get("$id")
                    .or_else(|| values.get("id").filter(|v| v.is_string()))
                {
                    if !id
                        .as_str()
                        .is_some_and(|v| v.starts_with("urn:") && v.len() <= 256)
                    {
                        return Err(PublicApiError::bad_request(
                            "Schema identifiers must be offline URNs",
                        ));
                    }
                }
                if values.contains_key("$schema") {
                    draft(value)?;
                }
                if let Some(reference) = values.get("$ref") {
                    let reference = reference
                        .as_str()
                        .filter(|v| v.starts_with("#/"))
                        .ok_or_else(|| {
                            PublicApiError::bad_request(
                                "Only acyclic in-document JSON Pointer references are allowed",
                            )
                        })?;
                    let pointer = &reference[1..];
                    if stack.iter().any(|v| v == pointer) {
                        return Err(PublicApiError::bad_request(
                            "Cyclic settings schema references are forbidden",
                        ));
                    }
                    let target = root.pointer(pointer).ok_or_else(|| {
                        PublicApiError::bad_request("Unresolved settings schema reference")
                    })?;
                    stack.push(pointer.into());
                    visit(
                        target,
                        root,
                        pointer.into(),
                        stack,
                        cost,
                        patterns,
                        depth + 1,
                    )?;
                    stack.pop();
                }
                for (key, child) in values {
                    if key == "pattern" {
                        *patterns += 1;
                        if !child.as_str().is_some_and(|v| v.len() <= 256) {
                            return Err(PublicApiError::bad_request(
                                "Schema regex exceeds its byte budget",
                            ));
                        }
                    }
                    if key == "patternProperties" {
                        if let Some(properties) = child.as_object() {
                            *patterns += properties.len();
                            if properties.keys().any(|v| v.len() > 256) {
                                return Err(PublicApiError::bad_request(
                                    "Schema regex exceeds its byte budget",
                                ));
                            }
                        }
                    }
                    if *patterns > 16 {
                        return Err(PublicApiError::bad_request(
                            "Schema regex count exceeds the budget",
                        ));
                    }
                    let next = format!("{}/{}", path, key.replace('~', "~0").replace('/', "~1"));
                    stack.push(next.clone());
                    visit(child, root, next, stack, cost, patterns, depth + 1)?;
                    stack.pop();
                }
            }
            Value::Array(values) => {
                if values.len() > 128 {
                    return Err(PublicApiError::bad_request(
                        "Schema array exceeds the budget",
                    ));
                }
                for (index, child) in values.iter().enumerate() {
                    visit(
                        child,
                        root,
                        format!("{path}/{index}"),
                        stack,
                        cost,
                        patterns,
                        depth + 1,
                    )?;
                }
            }
            Value::String(value) if value.len() > 4096 => {
                return Err(PublicApiError::bad_request(
                    "Schema string exceeds the byte budget",
                ));
            }
            _ => (),
        }
        Ok(())
    }
    visit(
        schema,
        schema,
        String::new(),
        &mut vec![String::new()],
        &mut 0,
        &mut 0,
        0,
    )
}
fn compile(schema: &Value) -> PublicApiResult<Validator> {
    policy(schema)?;
    jsonschema::options()
        .with_draft(draft(schema)?)
        .with_retriever(Offline)
        .with_pattern_options(
            PatternOptions::regex()
                .size_limit(65_536)
                .dfa_size_limit(65_536),
        )
        .should_validate_formats(true)
        .should_ignore_unknown_formats(false)
        .build(schema)
        .map_err(|_| PublicApiError::bad_request("Invalid or unsupported settings JSON Schema"))
}
/// CPU work is admitted by a bounded semaphore before spawn_blocking. Error
/// messages deliberately omit schema and preference values, which can be private.
pub async fn validate(schema: Value, settings: Option<Value>) -> PublicApiResult<()> {
    let permit = Arc::clone(&VALIDATORS)
        .acquire_owned()
        .await
        .map_err(|_| PublicApiError::unavailable())?;
    let result = tokio::task::spawn_blocking(move || {
        // Cancellation of the HTTP task cannot release CPU capacity while its
        // blocking validator continues running.
        let _permit = permit;
        let validator = compile(&schema)?;
        if let Some(settings) = settings {
            value_budget(&settings)?;
            if !validator.is_valid(&settings) {
                return Err(PublicApiError::bad_request(
                    "settings does not match the latest template",
                ));
            }
        }
        Ok(())
    })
    .await
    .map_err(|_| PublicApiError::unavailable())?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn real_validator_enforces_offline_schema_and_native_baseline() {
        let baseline: Value = serde_json::from_str(include_str!("settings_baseline.json")).unwrap();
        let validator = compile(&baseline).unwrap();
        assert!(!validator.is_valid(&json!({})));
        for schema in [
            json!({"type":"nonsense"}),
            json!({"$ref":"file:///etc/passwd"}),
            json!({"$ref":"https://example.invalid/schema"}),
            json!({"$ref":"#"}),
            json!({"definitions":{"a":{"$ref":"#/definitions/a"}},"$ref":"#/definitions/a"}),
            json!({"type":"string","pattern":"(?=lookahead)"}),
        ] {
            assert!(compile(&schema).is_err(), "{schema}");
        }
        let schema = json!({"type":"object","properties":{"color":{"$ref":"#/definitions/color"}},"definitions":{"color":{"enum":["light","dark"]}},"additionalProperties":false});
        let validator = compile(&schema).unwrap();
        assert!(validator.is_valid(&json!({"color":"dark"})));
        assert!(!validator.is_valid(&json!({"color":"red"})));
        assert!(compile(&json!({"type":"object","properties":{"secret":{"type":"string","format":"unsupported-private-format"}}})).is_err());
    }
    #[test]
    fn resource_policy_rejects_large_recursive_or_expensive_inputs() {
        let many: serde_json::Map<String, Value> =
            (0..4097).map(|n| (format!("k{n}"), json!(false))).collect();
        assert!(value_budget(&Value::Object(many)).is_err());
        let mut deep = json!(false);
        for _ in 0..18 {
            deep = json!({"nested":deep});
        }
        assert!(value_budget(&deep).is_err());
        assert!(compile(&json!({"type":"string","pattern":"a{1000000000}"})).is_err());
        assert!(compile(&json!({"type":"string","pattern":"a".repeat(257)})).is_err());
        let patterns: serde_json::Map<String, Value> = (0..17)
            .map(|n| (format!("^a{n}$"), json!({"type":"boolean"})))
            .collect();
        assert!(compile(&json!({"type":"object","patternProperties":patterns})).is_err());
        assert!(
            compile(&json!({"$schema":"https://example.invalid/schema","type":"object"})).is_err()
        );
    }
}
