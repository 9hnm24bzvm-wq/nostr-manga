//! Bounded descriptive series contract; raw canonical bytes remain authoritative.
use crate::manifest::{self, Page, field, text, uint};
use ciborium::Value;
use serde::Serialize;
use std::collections::BTreeMap;
pub const COVER_FEATURE: &str = "series-cover-v1";
#[derive(Clone, Debug, Serialize)]
pub struct Credit {
    pub role: String,
    pub name: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesMetadata {
    pub titles: BTreeMap<String, String>,
    pub alt_titles: Option<Vec<BTreeMap<String, String>>>,
    pub description: Option<BTreeMap<String, String>>,
    pub categories: Option<Vec<String>>,
    pub credits: Option<Vec<Credit>>,
    pub original_language: Option<String>,
    pub status: Option<String>,
    pub content_rating: Option<String>,
    pub publication_demographic: Option<String>,
    pub year: Option<u16>,
    pub cover: Option<Page>,
}
fn optional<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    field(v, key).ok().filter(|x| !matches!(x, Value::Null))
}
fn bounded(v: &Value, max: usize, empty: bool) -> Result<String, String> {
    let s = text(v)?;
    if s.len() > max || s.contains('\0') || (!empty && s.trim().is_empty()) {
        return Err("metadata text bounds".into());
    }
    Ok(s)
}
fn localized(v: &Value, max: usize, nonempty: bool) -> Result<BTreeMap<String, String>, String> {
    let map = v.as_map().ok_or("localized map required")?;
    if map.len() > 64 || (nonempty && map.is_empty()) {
        return Err("localized field count".into());
    }
    map.iter()
        .map(|(k, v)| {
            let key = bounded(k, 64, false)?;
            if language_tags::LanguageTag::parse(&key).is_err() {
                return Err("localized BCP47 key".into());
            }
            Ok((key, bounded(v, max, !nonempty)?))
        })
        .collect()
}
fn array(v: &Value, max: usize) -> Result<&Vec<Value>, String> {
    let a = v.as_array().ok_or("metadata array required")?;
    if a.len() > max {
        return Err("metadata array budget".into());
    }
    Ok(a)
}
pub fn parse(bytes: &[u8]) -> Result<SeriesMetadata, String> {
    parse_value(&manifest::decode(bytes)?)
}
pub(crate) fn parse_value(v: &Value) -> Result<SeriesMetadata, String> {
    if uint(field(v, "version")?)? != 1 {
        return Err("unsupported series version".into());
    }
    let mut cover_required = false;
    if let Ok(required) = field(v, "required") {
        for f in array(required, 32)? {
            if f.as_text() != Some(COVER_FEATURE) {
                return Err("unsupported required series feature".into());
            }
            cover_required = true;
        }
    }
    let titles = localized(field(v, "titles")?, 1024, true)?;
    let alt_titles = optional(v, "altTitles")
        .map(|a| {
            array(a, 128)?
                .iter()
                .map(|v| localized(v, 1024, true))
                .collect()
        })
        .transpose()?;
    let description = optional(v, "description")
        .map(|v| localized(v, 65536, false))
        .transpose()?;
    let categories = optional(v, "categories")
        .map(|a| {
            array(a, 256)?
                .iter()
                .map(|v| bounded(v, 512, false))
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?;
    let credits = optional(v, "credits")
        .map(|a| {
            array(a, 128)?
                .iter()
                .map(|v| {
                    Ok(Credit {
                        role: bounded(field(v, "role")?, 64, false)?,
                        name: bounded(field(v, "name")?, 1024, false)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?;
    let original_language = optional(v, "originalLanguage")
        .map(|v| bounded(v, 64, false))
        .transpose()?;
    if original_language
        .as_ref()
        .is_some_and(|l| language_tags::LanguageTag::parse(l).is_err())
    {
        return Err("original BCP47 language".into());
    }
    let status = optional(v, "status")
        .map(|v| bounded(v, 64, false))
        .transpose()?;
    let year = optional(v, "year")
        .map(|v| {
            uint(v).and_then(|n| {
                u16::try_from(n)
                    .ok()
                    .filter(|n| *n <= 9999)
                    .ok_or("year bounds".into())
            })
        })
        .transpose()?;
    let cover = optional(v, "cover")
        .map(manifest::original_descriptor)
        .transpose()?;
    if cover.is_some() != cover_required {
        return Err("cover requires series-cover-v1 marker, and marker requires a cover".into());
    }
    if cover.as_ref().is_some_and(|p| p.size == 0) {
        return Err("empty cover".into());
    }
    Ok(SeriesMetadata {
        titles,
        alt_titles,
        description,
        categories,
        credits,
        original_language,
        status,
        content_rating: optional(v, "contentRating")
            .map(|v| bounded(v, 64, false))
            .transpose()?,
        publication_demographic: optional(v, "publicationDemographic")
            .map(|v| bounded(v, 64, false))
            .transpose()?,
        year,
        cover,
    })
}
#[cfg(test)]
mod public_contract_tests {
    use crate::series_metadata::parse;

    #[test]
    fn canonical_bytes_preserve_descriptive_metadata_and_cover_contract() {
        // Literal wire bytes produced by Python cbor2, not the Rust decoder's helpers.
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/protocol-vectors/series.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let bytes = hex::decode(case["hex"].as_str().unwrap()).unwrap();
            let parsed = parse(&bytes);
            if let Some(expected) = case.get("expected") {
                assert_eq!(serde_json::to_value(parsed.unwrap()).unwrap(), *expected);
            } else {
                assert_eq!(
                    parsed.is_ok(),
                    case["accepted"].as_bool().unwrap(),
                    "{}",
                    case["name"]
                );
                if let (Ok(metadata), Some(expected)) = (parsed, case.get("cover")) {
                    assert_eq!(serde_json::to_value(metadata.cover).unwrap(), *expected);
                }
            }
        }
    }
}
