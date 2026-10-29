//! Strict JSON for canonical account commands. Duplicate keys at any depth are
//! rejected before untyped normalization can collapse authority/version intent.
use super::{PublicApiError, PublicApiResult};
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Number, Value};
struct Strict(Value);
impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Strict;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("unique-key JSON")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Strict, E> {
                Ok(Strict(Value::Bool(v)))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Strict, E> {
                Ok(Strict(Value::Number(v.into())))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Strict, E> {
                Ok(Strict(Value::Number(v.into())))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Strict, E> {
                Ok(Strict(Value::Number(
                    Number::from_f64(v).ok_or_else(|| E::custom("Invalid number"))?,
                )))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Strict, E> {
                Ok(Strict(Value::String(v.into())))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Strict, E> {
                Ok(Strict(Value::String(v)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_none<E: de::Error>(self) -> Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Strict, A::Error> {
                let mut values = Vec::new();
                while let Some(Strict(v)) = a.next_element()? {
                    values.push(v);
                }
                Ok(Strict(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Strict, A::Error> {
                let mut fields = Map::new();
                while let Some((key, Strict(v))) = a.next_entry::<String, Strict>()? {
                    if fields.insert(key, v).is_some() {
                        return Err(de::Error::custom("Duplicate JSON key"));
                    }
                }
                Ok(Strict(Value::Object(fields)))
            }
        }
        d.deserialize_any(V)
    }
}
pub(super) fn parse(bytes: &[u8]) -> PublicApiResult<Value> {
    serde_json::from_slice::<Strict>(bytes)
        .map(|v| v.0)
        .map_err(|_| PublicApiError::bad_request("Invalid or duplicate JSON command fields"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_duplicate_intent_at_every_depth() {
        for value in [
            r#"{"expectedAccountId":"a","expectedAccountId":"b"}"#,
            r#"{"draft":{"version":1,"version":2}}"#,
            r#"{"title":{"en":"first","en":"second"}}"#,
        ] {
            assert!(parse(value.as_bytes()).is_err());
        }
        assert_eq!(
            parse(br#"{"clear":null,"n":1,"values":[true,"a"]}"#).unwrap()["clear"],
            Value::Null
        );
    }
}
