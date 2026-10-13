//! Bounded HTTP envelopes. Canonical manifest bytes and their hashes are unchanged.
use crate::{auth::Event, db::Id, library::ExpectedHead, library_storage::ObjectType};
use anyhow::{Result, anyhow, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{
    Deserialize, Deserializer,
    de::{Error, SeqAccess, Visitor},
};
use serde_json::value::RawValue;
use std::{
    borrow::Cow,
    fmt,
    io::{Read, Write},
    marker::PhantomData,
};
pub const MAX_WIRE: usize = 8 * 1024 * 1024;
pub const MAX_OBJECTS: usize = 64;
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Chunk,
    Root,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Upload<'a> {
    #[serde(rename = "type")]
    kind: Kind,
    #[serde(borrow)]
    data: Cow<'a, str>,
}
fn uploads<'de: 'a, 'a, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<Upload<'a>>, D::Error> {
    struct List<'a>(PhantomData<&'a ()>);
    impl<'de: 'a, 'a> Visitor<'de> for List<'a> {
        type Value = Vec<Upload<'a>>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("at most64 library objects")
        }
        fn visit_seq<A: SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            if seq.size_hint().is_some_and(|n| n > MAX_OBJECTS) {
                return Err(A::Error::custom("object count budget"));
            }
            let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(MAX_OBJECTS));
            while let Some(item) = seq.next_element()? {
                if out.len() == MAX_OBJECTS {
                    return Err(A::Error::custom("object count budget"));
                }
                out.push(item);
            }
            Ok(out)
        }
    }
    deserializer.deserialize_seq(List(PhantomData))
}
fn expected<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<ExpectedHead, D::Error> {
    struct Head;
    impl Visitor<'_> for Head {
        type Value = ExpectedHead;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("event ID or null")
        }
        fn visit_unit<E: Error>(self) -> std::result::Result<Self::Value, E> {
            Ok(ExpectedHead::Absent)
        }
        fn visit_str<E: Error>(self, s: &str) -> std::result::Result<Self::Value, E> {
            if !crate::auth::valid_hash(s) {
                return Err(E::custom("invalid expected head"));
            }
            Ok(ExpectedHead::Exact(
                hex::decode(s).unwrap().try_into().unwrap(),
            ))
        }
    }
    d.deserialize_any(Head)
}
fn any() -> ExpectedHead {
    ExpectedHead::Any
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Commit<'a> {
    #[serde(borrow)]
    event: &'a RawValue,
    #[serde(default = "any", deserialize_with = "expected")]
    expected_head: ExpectedHead,
    #[serde(default, borrow, deserialize_with = "uploads")]
    objects: Vec<Upload<'a>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Batch<'a> {
    #[serde(borrow, deserialize_with = "uploads")]
    objects: Vec<Upload<'a>>,
}
pub struct DecodedCommit {
    pub event: Event,
    pub expected: ExpectedHead,
    pub objects: Vec<(ObjectType, Vec<u8>)>,
}
fn decode_objects(objects: Vec<Upload<'_>>) -> Result<Vec<(ObjectType, Vec<u8>)>> {
    let mut out = Vec::with_capacity(objects.len());
    let mut total = 0usize;
    for object in objects {
        let (kind, limit): (ObjectType, usize) = match object.kind {
            Kind::Chunk => (ObjectType::Chunk, 1_000_000),
            Kind::Root => (ObjectType::Root, 256 * 1024),
        };
        ensure!(
            object.data.len() <= limit.div_ceil(3) * 4,
            "encoded object byte budget"
        );
        let bytes = STANDARD
            .decode(object.data.as_bytes())
            .map_err(|_| anyhow!("invalid object base64"))?;
        ensure!(
            !bytes.is_empty() && bytes.len() <= limit,
            "decoded object byte budget"
        );
        total = total
            .checked_add(bytes.len())
            .ok_or_else(|| anyhow!("object size overflow"))?;
        ensure!(total <= 6 * 1024 * 1024, "decoded batch byte budget");
        out.push((kind, bytes));
    }
    Ok(out)
}
pub fn commit(bytes: &[u8]) -> Result<DecodedCommit> {
    ensure!(bytes.len() <= MAX_WIRE, "wire byte budget");
    let wrapper: Commit<'_> =
        serde_json::from_slice(bytes).map_err(|_| anyhow!("invalid bounded commit envelope"))?;
    ensure!(wrapper.event.get().len() <= 16_384, "event byte budget");
    let event =
        serde_json::from_str(wrapper.event.get()).map_err(|_| anyhow!("invalid signed event"))?;
    Ok(DecodedCommit {
        event,
        expected: wrapper.expected_head,
        objects: decode_objects(wrapper.objects)?,
    })
}
pub fn batch(bytes: &[u8]) -> Result<Vec<(ObjectType, Vec<u8>)>> {
    ensure!(bytes.len() <= MAX_WIRE, "wire byte budget");
    let wrapper: Batch<'_> =
        serde_json::from_slice(bytes).map_err(|_| anyhow!("invalid bounded upload envelope"))?;
    ensure!(!wrapper.objects.is_empty(), "empty upload batch");
    decode_objects(wrapper.objects)
}
pub fn decode_body(bytes: Vec<u8>, gzip: bool) -> Result<Vec<u8>> {
    ensure!(bytes.len() <= MAX_WIRE, "wire byte budget");
    if !gzip {
        return Ok(bytes);
    }
    let mut decoder = flate2::bufread::GzDecoder::new(bytes.as_slice());
    let mut output = vec![];
    (&mut decoder)
        .take((MAX_WIRE + 1) as u64)
        .read_to_end(&mut output)
        .map_err(|_| anyhow!("invalid gzip body"))?;
    ensure!(
        output.len() <= MAX_WIRE && decoder.into_inner().is_empty(),
        "gzip expansion or trailing-member budget"
    );
    Ok(output)
}
pub fn compress(bytes: Vec<u8>) -> Result<Vec<u8>> {
    let mut encoder = flate2::write::GzEncoder::new(
        Vec::with_capacity(bytes.len() + 1_000_000),
        flate2::Compression::new(3),
    );
    encoder.write_all(&bytes)?;
    let mut out = encoder.finish()?;
    out.shrink_to_fit();
    Ok(out)
}
pub fn requested_gzip(header: Option<&str>) -> bool {
    header.unwrap_or("").split(',').any(|item| {
        let mut p = item.trim().split(';');
        if !p.next().unwrap_or("").trim().eq_ignore_ascii_case("gzip") {
            return false;
        }
        p.all(|v| {
            v.trim()
                .strip_prefix("q=")
                .is_none_or(|q| q.parse::<f32>().is_ok_and(|q| q > 0.0 && q <= 1.0))
        })
    })
}
pub fn hashes(bytes: &[u8]) -> Result<Vec<Id>> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Read {
        hashes: Vec<String>,
    }
    ensure!(bytes.len() <= 8192, "read request byte budget");
    let request: Read =
        serde_json::from_slice(bytes).map_err(|_| anyhow!("invalid read request"))?;
    ensure!(
        !request.hashes.is_empty() && request.hashes.len() <= MAX_OBJECTS,
        "read object count budget"
    );
    request
        .hashes
        .iter()
        .map(|s| {
            ensure!(crate::auth::valid_hash(s), "invalid object hash");
            Ok(hex::decode(s)?.try_into().unwrap())
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn oversized_lists_and_objects_are_rejected() {
        let object = json!({"type":"chunk","data":"AA=="});
        assert!(batch(&serde_json::to_vec(&json!({"objects":vec![object;65]})).unwrap()).is_err());
        assert!(batch(br#"{"objects":[{"type":"chunk","data":[1,2,3]}]}"#).is_err());
        assert!(
            batch(
                &serde_json::to_vec(
                    &json!({"objects":[{"type":"root","data":"A".repeat(400000)}]})
                )
                .unwrap()
            )
            .is_err()
        );
    }
    #[test]
    fn gzip_is_bounded_and_single_member() {
        let bytes = b"bounded fixture".to_vec();
        let gzip = compress(bytes.clone()).unwrap();
        assert_eq!(decode_body(gzip.clone(), true).unwrap(), bytes);
        let mut trailing = gzip.clone();
        trailing.extend(gzip);
        assert!(decode_body(trailing, true).is_err());
        assert!(decode_body(compress(vec![0; MAX_WIRE + 1]).unwrap(), true).is_err());
    }
    #[test]
    fn gzip_matches_an_independent_wire_vector() {
        // Python gzip.compress(b"chapter one\n", mtime=0), independent of this codec.
        let external =
            hex::decode("1f8b08000000000002034bce482c28492d52c8cf4be5020014ab64af0c000000")
                .unwrap();
        assert_eq!(decode_body(external, true).unwrap(), b"chapter one\n");
        let encoded = compress(b"chapter one\n".to_vec()).unwrap();
        let mut decoded = Vec::new();
        flate2::read::GzDecoder::new(encoded.as_slice())
            .read_to_end(&mut decoded)
            .unwrap();
        assert_eq!(decoded, b"chapter one\n");
    }
    #[test]
    fn expected_null_and_absence_are_distinct() {
        let event = crate::auth::sign(&format!("{:064x}", 1), 11999, vec![], "{}".into()).unwrap();
        assert!(matches!(
            commit(&serde_json::to_vec(&json!({"event":event})).unwrap())
                .unwrap()
                .expected,
            ExpectedHead::Any
        ));
        assert!(matches!(
            commit(&serde_json::to_vec(&json!({"event":event,"expected_head":null})).unwrap())
                .unwrap()
                .expected,
            ExpectedHead::Absent
        ));
        assert!(!requested_gzip(Some("gzip;q=0")));
        assert!(requested_gzip(Some("br, gzip;q=0.5")));
    }
}
