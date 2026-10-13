use crate::auth::valid_hash;
use ciborium::Value;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Page {
    #[serde(rename = "sha256")]
    pub hash: String,
    pub size: u64,
    pub mime: String,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug)]
pub struct Manifest {
    pub series: String,
    pub language: String,
    pub chapter: Option<String>,
    pub volume: Option<String>,
    pub title: Option<String>,
    pub pages: Vec<Page>,
}
pub(crate) fn field<'a>(v: &'a Value, k: &str) -> Result<&'a Value, String> {
    v.as_map()
        .and_then(|m| {
            m.iter()
                .find(|(key, _)| key.as_text() == Some(k))
                .map(|(_, x)| x)
        })
        .ok_or_else(|| format!("missing {k}"))
}
pub(crate) fn text(v: &Value) -> Result<String, String> {
    v.as_text().map(str::to_owned).ok_or("text required".into())
}
pub(crate) fn uint(v: &Value) -> Result<u64, String> {
    v.as_integer()
        .and_then(|i| u64::try_from(i).ok())
        .ok_or("unsigned integer required".into())
}
fn canonical(v: &Value, depth: usize, nodes: &mut usize) -> Result<Value, String> {
    *nodes += 1;
    if depth > 20 || *nodes > 50000 {
        return Err("CBOR complexity budget".into());
    }
    Ok(match v {
        Value::Map(entries) => {
            let mut out = Vec::new();
            let mut seen = BTreeSet::new();
            for (k, val) in entries {
                let key = k.as_text().ok_or("prototype CBOR maps require text keys")?;
                if !seen.insert(key) {
                    return Err("duplicate CBOR key".into());
                }
                let keyval = Value::Text(key.into());
                let mut bytes = Vec::new();
                ciborium::into_writer(&keyval, &mut bytes).map_err(|e| e.to_string())?;
                out.push((bytes, keyval, canonical(val, depth + 1, nodes)?));
            }
            out.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Map(out.into_iter().map(|(_, k, v)| (k, v)).collect())
        }
        Value::Array(a) => Value::Array(
            a.iter()
                .map(|v| canonical(v, depth + 1, nodes))
                .collect::<Result<_, _>>()?,
        ),
        Value::Integer(_) | Value::Text(_) | Value::Bytes(_) | Value::Bool(_) | Value::Null => {
            v.clone()
        }
        _ => return Err("CBOR type excluded by experimental profile".into()),
    })
}
pub fn decode(data: &[u8]) -> Result<Value, String> {
    if data.len() > 1_000_000 {
        return Err("manifest byte limit".into());
    }
    // Bound nesting and declared collection sizes BEFORE a general decoder can
    // allocate or recurse according to an untrusted CBOR header.
    fn walk(data: &[u8], pos: &mut usize, depth: usize, nodes: &mut usize) -> Result<(), String> {
        *nodes += 1;
        if depth > 20 || *nodes > 50000 {
            return Err("CBOR preflight complexity budget".into());
        }
        let first = *data.get(*pos).ok_or("truncated CBOR")?;
        *pos += 1;
        let major = first >> 5;
        let ai = first & 31;
        if major == 7 {
            return if [20, 21, 22].contains(&ai) {
                Ok(())
            } else {
                Err("unsupported CBOR simple/float".into())
            };
        }
        let n = if ai < 24 {
            ai as u64
        } else {
            let width = match ai {
                24 => 1,
                25 => 2,
                26 => 4,
                27 => 8,
                _ => return Err("indefinite/reserved CBOR".into()),
            };
            let end = pos.checked_add(width).ok_or("length overflow")?;
            let bytes = data.get(*pos..end).ok_or("truncated header")?;
            *pos = end;
            bytes.iter().fold(0u64, |a, b| (a << 8) | *b as u64)
        };
        match major {
            0 | 1 => Ok(()),
            2 | 3 => {
                let len = usize::try_from(n).map_err(|_| "length overflow")?;
                let end = pos.checked_add(len).ok_or("length overflow")?;
                if end > data.len() {
                    return Err("truncated string".into());
                }
                *pos = end;
                Ok(())
            }
            4 | 5 => {
                let count = n
                    .checked_mul(if major == 5 { 2 } else { 1 })
                    .ok_or("collection overflow")?;
                if count > 50000 || count > data.len().saturating_sub(*pos) as u64 {
                    return Err("collection budget".into());
                }
                for _ in 0..count {
                    walk(data, pos, depth + 1, nodes)?;
                }
                Ok(())
            }
            _ => Err("CBOR tags excluded".into()),
        }
    }
    let mut pos = 0;
    walk(data, &mut pos, 0, &mut 0)?;
    if pos != data.len() {
        return Err("trailing CBOR data".into());
    }
    let mut cursor = std::io::Cursor::new(data);
    let v: Value = ciborium::from_reader(&mut cursor).map_err(|e| e.to_string())?;
    if cursor.position() != data.len() as u64 {
        return Err("trailing CBOR bytes".into());
    }
    let c = canonical(&v, 0, &mut 0)?;
    let mut encoded = Vec::new();
    ciborium::into_writer(&c, &mut encoded).map_err(|e| e.to_string())?;
    if encoded != data {
        return Err("non-deterministic CBOR encoding".into());
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_map() {
        assert_eq!(
            decode(&hex::decode("a2616101616202").unwrap()).unwrap(),
            Value::Map(vec![
                (Value::Text("a".into()), Value::Integer(1.into())),
                (Value::Text("b".into()), Value::Integer(2.into()))
            ])
        );
    }
    #[test]
    fn duplicate_keys() {
        assert!(decode(&hex::decode("a2616101616102").unwrap()).is_err());
    }
    #[test]
    fn wrong_order() {
        assert!(decode(&hex::decode("a2616202616101").unwrap()).is_err());
    }
    #[test]
    fn trailing_data() {
        assert!(decode(&hex::decode("0102").unwrap()).is_err());
    }
    #[test]
    fn unbounded_collection() {
        assert!(decode(&hex::decode("9bffffffffffffffff").unwrap()).is_err());
    }
    #[test]
    fn deeply_nested_cbor_is_rejected() {
        let mut b = vec![0x81; 10000];
        b.push(0);
        assert!(decode(&b).is_err());
    }
    #[test]
    fn indefinite_rejected() {
        assert!(decode(&hex::decode("9f01ff").unwrap()).is_err());
    }
    #[test]
    fn nonminimal_integer() {
        assert!(decode(&hex::decode("1801").unwrap()).is_err());
    }
    #[test]
    fn invalid_utf8() {
        assert!(decode(&hex::decode("61ff").unwrap()).is_err());
    }
    #[test]
    fn seeded_malformed_inputs_do_not_panic() {
        use rand::{RngCore, SeedableRng};
        let mut r = rand::rngs::StdRng::seed_from_u64(20261006);
        for len in 0..1024 {
            let mut b = vec![0; len];
            r.fill_bytes(&mut b);
            let _ = decode(&b);
        }
    }
}
pub fn series(data: &[u8]) -> Result<(), String> {
    series_value(&decode(data)?)
}
fn series_value(value: &Value) -> Result<(), String> {
    crate::series_metadata::parse_value(value).map(|_| ())
}
pub fn chapter(data: &[u8]) -> Result<Manifest, String> {
    chapter_value(&decode(data)?)
}
fn chapter_value(v: &Value) -> Result<Manifest, String> {
    if uint(field(v, "version")?)? != 1 {
        return Err("unsupported schema".into());
    }
    if let Ok(required) = field(v, "required")
        && !required.as_array().ok_or("required array")?.is_empty()
    {
        return Err("unsupported required feature".into());
    }
    let series = text(field(v, "series")?)?;
    if !valid_hash(&series) {
        return Err("series ID".into());
    }
    let language = text(field(v, "language")?)?;
    if language.len() > 64 || language_tags::LanguageTag::parse(&language).is_err() {
        return Err("malformed BCP47 language tag".into());
    }
    let entries = field(v, "pages")?.as_array().ok_or("pages array")?;
    if entries.is_empty() || entries.len() > 10000 {
        return Err("page-count limit".into());
    }
    let mut pages = Vec::new();
    for p in entries {
        pages.push(original_descriptor(p)?);
    }
    let chapter = field(v, "chapter").ok().map(text).transpose()?;
    Ok(Manifest {
        series,
        language,
        chapter,
        volume: field(v, "volume").ok().map(text).transpose()?,
        title: field(v, "title").ok().map(text).transpose()?,
        pages,
    })
}
pub(crate) fn original_descriptor(p: &Value) -> Result<Page, String> {
    let hash = match field(p, "sha256")? {
        Value::Bytes(b) if b.len() == 32 => hex::encode(b),
        Value::Text(s) if valid_hash(s) => s.clone(),
        _ => return Err("page hash".into()),
    };
    let width = u32::try_from(uint(field(p, "width")?)?).map_err(|_| "width")?;
    let height = u32::try_from(uint(field(p, "height")?)?).map_err(|_| "height")?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 40_000_000 {
        return Err("pixel budget".into());
    }
    Ok(Page {
        hash,
        size: uint(field(p, "size")?)?,
        mime: text(field(p, "mime")?)?,
        width,
        height,
    })
}
pub fn chapter_for_sync(
    data: &[u8],
) -> Result<Manifest, crate::transfer_rejection::TransferRejection> {
    use crate::transfer_rejection::{
        RejectionReason as Why, TransferRejection as R, manifest_support,
    };
    let value = decode(data).map_err(|_| R::invalid(Why::Manifest))?;
    manifest_support(&value, Why::Manifest)?;
    chapter_value(&value).map_err(|_| R::invalid(Why::Manifest))
}
pub fn series_for_sync(data: &[u8]) -> Result<(), crate::transfer_rejection::TransferRejection> {
    use crate::transfer_rejection::{RejectionReason as Why, TransferRejection as R};
    let value = decode(data).map_err(|_| R::invalid(Why::Manifest))?;
    crate::transfer_rejection::manifest_support_features(
        &value,
        Why::Manifest,
        &[crate::series_metadata::COVER_FEATURE],
    )?;
    series_value(&value).map_err(|_| R::invalid(Why::Manifest))
}
/// Preserve page ordering in the manifest while validating repeated originals once.
/// Conflicting metadata for a repeated hash is invalid, never a cache hit.
pub fn unique_pages(pages: &[Page]) -> Result<Vec<&Page>, String> {
    let mut seen = std::collections::HashMap::new();
    let mut originals = Vec::new();
    for page in pages {
        if let Some(previous) = seen.get(page.hash.as_str()) {
            if *previous != page {
                return Err("conflicting metadata for repeated original".into());
            }
        } else {
            seen.insert(page.hash.as_str(), page);
            originals.push(page);
        }
    }
    Ok(originals)
}
pub fn detected_mime(data: &[u8]) -> Option<&'static str> {
    match image::guess_format(data).ok()? {
        image::ImageFormat::Png => Some("image/png"),
        image::ImageFormat::Jpeg => Some("image/jpeg"),
        image::ImageFormat::WebP => Some("image/webp"),
        _ => None,
    }
}
pub fn extension(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        _ => "cbor",
    }
}
pub fn validate_image(data: &[u8], p: &Page) -> Result<(), String> {
    if data.len() as u64 != p.size || crate::auth::hash(data) != p.hash {
        return Err("image hash/size mismatch".into());
    }
    let format = image::guess_format(data).map_err(|e| e.to_string())?;
    if detected_mime(data) != Some(p.mime.as_str()) {
        return Err("image MIME/encoding mismatch".into());
    }
    let reader = image::ImageReader::with_format(std::io::Cursor::new(data), format);
    let (w, h) = reader.into_dimensions().map_err(|e| e.to_string())?;
    if (w, h) != (p.width, p.height) {
        return Err("image dimensions mismatch".into());
    }
    // Budget dimensions before full decode; actual decoding rejects malformed data.
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(data), format);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits.max_image_width = Some(40_000_000);
    limits.max_image_height = Some(40_000_000);
    reader.limits(limits);
    reader.decode().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod schema_tests {
    use super::*;
    #[test]
    fn repeated_originals_preserve_first_occurrence() {
        let page = Page {
            hash: "a".repeat(64),
            size: 100,
            mime: "image/png".into(),
            width: 8,
            height: 8,
        };
        let mut second = page.clone();
        second.hash = "b".repeat(64);
        let pages = vec![page.clone(), second.clone(), page.clone()];
        let unique = unique_pages(&pages).unwrap();
        assert_eq!(unique, vec![&page, &second]);
    }
    #[test]
    fn repeated_hash_cannot_hide_conflicting_descriptors() {
        let page = Page {
            hash: "a".repeat(64),
            size: 100,
            mime: "image/png".into(),
            width: 8,
            height: 8,
        };
        for field in 0..4 {
            let mut bad = page.clone();
            match field {
                0 => bad.size += 1,
                1 => bad.mime = "image/jpeg".into(),
                2 => bad.width += 1,
                _ => bad.height += 1,
            };
            assert!(unique_pages(&[page.clone(), bad]).is_err());
        }
    }
}

#[cfg(test)]
mod independent_vectors {
    use crate::manifest::{chapter, series};

    #[test]
    fn literal_series_and_chapter_bytes_enforce_schema_and_language_contracts() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/protocol-vectors/series.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let bytes = hex::decode(case["hex"].as_str().unwrap()).unwrap();
            let accepted =
                case.get("expected").is_some() || case["accepted"].as_bool().unwrap_or(false);
            assert_eq!(series(&bytes).is_ok(), accepted, "{}", case["name"]);
        }
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/protocol-vectors/chapters.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let bytes = hex::decode(case["hex"].as_str().unwrap()).unwrap();
            let result = chapter(&bytes);
            assert_eq!(
                result.is_ok(),
                case["accepted"].as_bool().unwrap(),
                "{}",
                case["language"]
            );
            if let Ok(chapter) = result {
                assert_eq!(chapter.language, case["language"].as_str().unwrap());
                assert_eq!(chapter.series, "ab".repeat(32));
                assert_eq!(chapter.chapter.as_deref(), Some("7.5"));
                assert_eq!(chapter.volume.as_deref(), Some("02"));
                assert_eq!(chapter.title.as_deref(), Some("例"));
                assert_eq!(chapter.pages.len(), 1);
                assert_eq!(chapter.pages[0].hash, "cd".repeat(32));
                assert_eq!(
                    (
                        chapter.pages[0].size,
                        chapter.pages[0].width,
                        chapter.pages[0].height
                    ),
                    (10, 1, 1)
                );
                assert_eq!(chapter.pages[0].mime, "image/png");
            }
        }
    }
}
