//! Draft production codec for the selected chunked public-library format.
//! Immutable object validation is separate from storage admission/authority.
use crate::{
    auth::{self, Event},
    db::Id,
    manifest,
};
use anyhow::{Context, Result, ensure};
use ciborium::Value;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Family {
    Chapters,
    Series,
    Lists,
    Members,
}
impl Family {
    pub const ALL: [Self; 4] = [Self::Chapters, Self::Series, Self::Lists, Self::Members];
    pub fn name(self) -> &'static str {
        match self {
            Self::Chapters => "chapters",
            Self::Series => "series",
            Self::Lists => "lists",
            Self::Members => "members",
        }
    }
    fn parse(s: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|f| f.name() == s)
            .context("unknown library family")
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Record {
    Chapter {
        id: Id,
        completed: bool,
        page: Option<u32>,
        updated_at: u64,
    },
    Series {
        id: Id,
        status: String,
        updated_at: u64,
    },
    List {
        id: [u8; 16],
        name: String,
        updated_at: u64,
    },
    Member {
        list: [u8; 16],
        series: Id,
        position: u64,
        updated_at: u64,
    },
}
impl Record {
    pub fn family(&self) -> Family {
        match self {
            Self::Chapter { .. } => Family::Chapters,
            Self::Series { .. } => Family::Series,
            Self::List { .. } => Family::Lists,
            Self::Member { .. } => Family::Members,
        }
    }
    pub fn key(&self) -> Vec<u8> {
        match self {
            Self::Chapter { id, .. } | Self::Series { id, .. } => id.to_vec(),
            Self::List { id, .. } => id.to_vec(),
            Self::Member { list, series, .. } => [list.as_slice(), series.as_slice()].concat(),
        }
    }
    fn validate(&self) -> Result<()> {
        let timestamp = match self {
            Self::Chapter { updated_at, .. } => *updated_at,
            Self::Series {
                status, updated_at, ..
            } => {
                ensure!(
                    [
                        "reading",
                        "on_hold",
                        "plan_to_read",
                        "dropped",
                        "re_reading",
                        "completed"
                    ]
                    .contains(&status.as_str()),
                    "invalid series reading status"
                );
                *updated_at
            }
            Self::List {
                name, updated_at, ..
            } => {
                ensure!(
                    !name.is_empty()
                        && name.len() <= 512
                        && name.chars().count() <= 128
                        && !name.chars().any(char::is_control),
                    "invalid public list name"
                );
                *updated_at
            }
            Self::Member {
                position,
                updated_at,
                ..
            } => {
                ensure!(*position <= i64::MAX as u64, "list position bound");
                *updated_at
            }
        };
        ensure!(timestamp <= i64::MAX as u64, "record timestamp bound");
        Ok(())
    }
    fn value(&self) -> Value {
        Value::Array(match self {
            Self::Chapter {
                id,
                completed,
                page,
                updated_at,
            } => vec![
                Value::Bytes(id.to_vec()),
                Value::Bool(*completed),
                page.map_or(Value::Null, |n| uint(n as u64)),
                uint(*updated_at),
            ],
            Self::Series {
                id,
                status,
                updated_at,
            } => vec![
                Value::Bytes(id.to_vec()),
                Value::Text(status.clone()),
                uint(*updated_at),
            ],
            Self::List {
                id,
                name,
                updated_at,
            } => vec![
                Value::Bytes(id.to_vec()),
                Value::Text(name.clone()),
                uint(*updated_at),
            ],
            Self::Member {
                list,
                series,
                position,
                updated_at,
            } => vec![
                Value::Bytes(list.to_vec()),
                Value::Bytes(series.to_vec()),
                uint(*position),
                uint(*updated_at),
            ],
        })
    }
}
#[derive(Clone, Debug)]
pub struct Limits {
    pub max_root_bytes: usize,
    pub max_chunk_bytes: usize,
    pub max_chunk_records: usize,
    pub max_family_buckets: usize,
    pub max_total_chunks: usize,
    pub max_records: u64,
    pub max_total_bytes: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_root_bytes: 256 * 1024,
            max_chunk_bytes: 1_000_000,
            max_chunk_records: 4096,
            max_family_buckets: 2048,
            max_total_chunks: 4096,
            max_records: 2_000_000,
            max_total_bytes: 256 * 1024 * 1024,
        }
    }
}
impl Limits {
    fn validate(&self) -> Result<()> {
        ensure!(
            (1..=1_000_000).contains(&self.max_root_bytes)
                && (1..=1_000_000).contains(&self.max_chunk_bytes),
            "library object byte limits"
        );
        ensure!(
            (1..=4096).contains(&self.max_chunk_records),
            "chunk record bound"
        );
        ensure!(
            self.max_family_buckets.is_power_of_two() && self.max_family_buckets <= 8192,
            "family bucket bound"
        );
        ensure!(
            (4..=8192).contains(&self.max_total_chunks)
                && self.max_records > 0
                && self.max_total_bytes > 0,
            "library aggregate limits"
        );
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct VerifiedChunk {
    family: Family,
    hash: Id,
    size: u64,
    records: u64,
    bucket_base: u32,
    common_low_bits: u32,
}
impl VerifiedChunk {
    pub fn family(&self) -> Family {
        self.family
    }
    /// Only reconstruct from a trusted summary row joined to its exact generation.
    pub(crate) fn from_storage(
        hash: Id,
        family: &str,
        size: u64,
        records: u64,
        base: u32,
        bits: u32,
    ) -> Result<Self> {
        ensure!(
            (1..=1_000_000).contains(&size) && (1..=4096).contains(&records) && bits <= 32,
            "invalid stored chunk summary"
        );
        Ok(Self {
            hash,
            family: Family::parse(family)?,
            size,
            records,
            bucket_base: base,
            common_low_bits: bits,
        })
    }
    pub(crate) fn storage_bucket_summary(&self) -> (u32, u32) {
        (self.bucket_base, self.common_low_bits)
    }
    pub fn hash(&self) -> Id {
        self.hash
    }
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn records(&self) -> u64 {
        self.records
    }
    pub fn fits(&self, family: Family, buckets: usize, index: usize) -> bool {
        if family != self.family || !buckets.is_power_of_two() || buckets > 8192 || index >= buckets
        {
            return false;
        }
        self.records == 0
            || (buckets.trailing_zeros() <= self.common_low_bits
                && self.bucket_base as usize & (buckets - 1) == index)
    }
}
#[derive(Clone, Debug)]
pub struct Chunk {
    family: Family,
    records: Vec<Record>,
}
impl Chunk {
    pub fn new(family: Family, mut records: Vec<Record>, limits: &Limits) -> Result<Self> {
        records.sort_by_key(Record::key);
        let chunk = Self { family, records };
        chunk.validate(limits)?;
        Ok(chunk)
    }
    pub fn records(&self) -> &[Record] {
        &self.records
    }
    fn validate(&self, limits: &Limits) -> Result<()> {
        limits.validate()?;
        ensure!(
            self.records.len() <= limits.max_chunk_records,
            "chunk record limit"
        );
        let mut previous = None;
        for record in &self.records {
            ensure!(record.family() == self.family, "mixed chunk family");
            record.validate()?;
            let key = record.key();
            if let Some(old) = &previous {
                ensure!(old < &key, "duplicate or unordered library record");
            }
            previous = Some(key);
        }
        Ok(())
    }
    pub fn encode(&self, limits: &Limits) -> Result<Vec<u8>> {
        self.validate(limits)?;
        let value = map(vec![
            ("type", text("library-chunk")),
            ("version", uint(1)),
            ("family", text(self.family.name())),
            (
                "entries",
                Value::Array(self.records.iter().map(Record::value).collect()),
            ),
        ]);
        let bytes = encode(&value)?;
        ensure!(bytes.len() <= limits.max_chunk_bytes, "chunk byte limit");
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8], limits: &Limits) -> Result<(Self, VerifiedChunk)> {
        limits.validate()?;
        ensure!(bytes.len() <= limits.max_chunk_bytes, "chunk byte limit");
        let value = manifest::decode(bytes).map_err(anyhow::Error::msg)?;
        let fields = fields(&value, &["type", "version", "family", "entries"])?;
        ensure!(
            str_value(fields["type"])? == "library-chunk" && number(fields["version"])? == 1,
            "library chunk type/version"
        );
        let family = Family::parse(str_value(fields["family"])?)?;
        let entries = array(fields["entries"])?;
        ensure!(
            entries.len() <= limits.max_chunk_records,
            "chunk record limit"
        );
        let mut records = Vec::with_capacity(entries.len());
        for entry in entries {
            let parts = array(entry)?;
            let record = match family {
                Family::Chapters => {
                    ensure!(parts.len() == 4, "chapter state tuple");
                    Record::Chapter {
                        id: bytes_exact(&parts[0])?,
                        completed: parts[1].as_bool().context("completed must be boolean")?,
                        page: if matches!(&parts[2], Value::Null) {
                            None
                        } else {
                            Some(u32::try_from(number(&parts[2])?)?)
                        },
                        updated_at: number(&parts[3])?,
                    }
                }
                Family::Series => {
                    ensure!(parts.len() == 3, "series state tuple");
                    Record::Series {
                        id: bytes_exact(&parts[0])?,
                        status: str_value(&parts[1])?.into(),
                        updated_at: number(&parts[2])?,
                    }
                }
                Family::Lists => {
                    ensure!(parts.len() == 3, "list descriptor tuple");
                    Record::List {
                        id: bytes_exact(&parts[0])?,
                        name: str_value(&parts[1])?.into(),
                        updated_at: number(&parts[2])?,
                    }
                }
                Family::Members => {
                    ensure!(parts.len() == 4, "list member tuple");
                    Record::Member {
                        list: bytes_exact(&parts[0])?,
                        series: bytes_exact(&parts[1])?,
                        position: number(&parts[2])?,
                        updated_at: number(&parts[3])?,
                    }
                }
            };
            records.push(record);
        }
        let chunk = Self { family, records };
        chunk.validate(limits)?;
        let mut base = 0;
        let mut diff = 0;
        for (i, record) in chunk.records.iter().enumerate() {
            let hash = key_bucket_hash(&record.key());
            if i == 0 {
                base = hash;
            } else {
                diff |= base ^ hash;
            }
        }
        let summary = VerifiedChunk {
            family,
            hash: Sha256::digest(bytes).into(),
            size: bytes.len() as u64,
            records: chunk.records.len() as u64,
            bucket_base: base,
            common_low_bits: diff.trailing_zeros(),
        };
        Ok((chunk, summary))
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Descriptor {
    pub hash: Id,
    pub size: u64,
    pub records: u64,
}
impl Descriptor {
    /// Empty buckets are CBOR null, not shared hot reference-counter rows.
    pub fn empty() -> Self {
        Self {
            hash: [0; 32],
            size: 0,
            records: 0,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}
impl From<&VerifiedChunk> for Descriptor {
    fn from(v: &VerifiedChunk) -> Self {
        if v.records == 0 {
            return Self::empty();
        }
        Self {
            hash: v.hash,
            size: v.size,
            records: v.records,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyRoot {
    pub family: Family,
    pub chunks: Vec<Descriptor>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Root {
    pub families: Vec<FamilyRoot>,
}
impl Root {
    pub fn totals(&self, limits: &Limits) -> Result<(u64, u64)> {
        limits.validate()?;
        ensure!(
            self.families.len() == 4,
            "all four library families required"
        );
        let mut records = 0u64;
        let mut bytes = 0u64;
        let mut count = 0usize;
        for (actual, expected) in self.families.iter().zip(Family::ALL) {
            ensure!(actual.family == expected, "library family order/uniqueness");
            let n = actual.chunks.len();
            ensure!(
                n.is_power_of_two() && n <= limits.max_family_buckets,
                "invalid family bucket count"
            );
            count = count.checked_add(n).context("chunk count overflow")?;
            for d in &actual.chunks {
                if d.is_empty() {
                    ensure!(d.hash == [0; 32] && d.records == 0, "invalid empty bucket");
                    continue;
                }
                ensure!(
                    (1..=limits.max_chunk_bytes as u64).contains(&d.size)
                        && (1..=limits.max_chunk_records as u64).contains(&d.records),
                    "chunk descriptor bounds"
                );
                records = records
                    .checked_add(d.records)
                    .context("record total overflow")?;
                bytes = bytes.checked_add(d.size).context("byte total overflow")?;
            }
        }
        ensure!(
            count <= limits.max_total_chunks
                && records <= limits.max_records
                && bytes <= limits.max_total_bytes,
            "library aggregate budget"
        );
        Ok((records, bytes))
    }
    pub fn encode(&self, limits: &Limits) -> Result<Vec<u8>> {
        self.totals(limits)?;
        let families = self
            .families
            .iter()
            .map(|f| {
                map(vec![
                    ("family", text(f.family.name())),
                    (
                        "chunks",
                        Value::Array(
                            f.chunks
                                .iter()
                                .map(|d| {
                                    if d.is_empty() {
                                        return Value::Null;
                                    }
                                    Value::Array(vec![
                                        Value::Bytes(d.hash.to_vec()),
                                        uint(d.size),
                                        uint(d.records),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ])
            })
            .collect();
        let bytes = encode(&map(vec![
            ("type", text("library-root")),
            ("version", uint(1)),
            ("families", Value::Array(families)),
        ]))?;
        ensure!(
            bytes.len() <= limits.max_root_bytes,
            "library root byte limit"
        );
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8], limits: &Limits) -> Result<Self> {
        limits.validate()?;
        ensure!(
            bytes.len() <= limits.max_root_bytes,
            "library root byte limit"
        );
        let value = manifest::decode(bytes).map_err(anyhow::Error::msg)?;
        let f = fields(&value, &["type", "version", "families"])?;
        ensure!(
            str_value(f["type"])? == "library-root" && number(f["version"])? == 1,
            "library root type/version"
        );
        let raw = array(f["families"])?;
        ensure!(raw.len() == 4, "all four library families required");
        let mut families = Vec::with_capacity(4);
        for item in raw {
            let f = fields(item, &["family", "chunks"])?;
            let family = Family::parse(str_value(f["family"])?)?;
            let raw = array(f["chunks"])?;
            ensure!(raw.len() <= limits.max_family_buckets, "family chunk count");
            let mut chunks = Vec::with_capacity(raw.len());
            for d in raw {
                if matches!(d, Value::Null) {
                    chunks.push(Descriptor::empty());
                    continue;
                }
                let p = array(d)?;
                ensure!(p.len() == 3, "chunk descriptor tuple");
                chunks.push(Descriptor {
                    hash: bytes_exact(&p[0])?,
                    size: number(&p[1])?,
                    records: number(&p[2])?,
                });
            }
            families.push(FamilyRoot { family, chunks });
        }
        let root = Self { families };
        root.totals(limits)?;
        Ok(root)
    }
    /// Summaries must come from validated immutable bytes/persisted trusted metadata.
    /// Storage generation, current availability and authority need an additional commit fence.
    pub fn validate_chunks(
        &self,
        summaries: &HashMap<Id, VerifiedChunk>,
        limits: &Limits,
    ) -> Result<()> {
        self.totals(limits)?;
        for f in &self.families {
            for (index, d) in f.chunks.iter().enumerate() {
                if d.is_empty() {
                    continue;
                }
                let s = summaries
                    .get(&d.hash)
                    .context("missing verified library chunk")?;
                ensure!(
                    s.hash == d.hash && s.size == d.size && s.records == d.records,
                    "chunk descriptor mismatch"
                );
                ensure!(
                    s.fits(f.family, f.chunks.len(), index),
                    "chunk is in the wrong family/bucket"
                );
            }
        }
        Ok(())
    }
}
pub fn bucket_for(key: &[u8], buckets: usize) -> Result<usize> {
    ensure!(buckets.is_power_of_two() && buckets <= 8192, "bucket count");
    Ok(key_bucket_hash(key) as usize & (buckets - 1))
}
fn key_bucket_hash(key: &[u8]) -> u32 {
    u32::from_be_bytes(Sha256::digest(key)[..4].try_into().unwrap())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pointer {
    manifest: String,
    version: u64,
}
pub fn pointer(event: &Event) -> Result<Id> {
    event.verify().map_err(anyhow::Error::msg)?;
    ensure!(
        event.kind == 11999 && event.values("expiration").is_empty(),
        "library event kind/lifecycle"
    );
    let p: Pointer = serde_json::from_str(&event.content)?;
    ensure!(
        p.version == 1 && auth::valid_hash(&p.manifest),
        "library pointer version/hash"
    );
    Ok(hex::decode(p.manifest)?.try_into().unwrap())
}
pub fn pointer_for_sync(event: &Event) -> Result<Id> {
    use crate::transfer_rejection::{
        RejectionReason as Why, TransferRejection as R, pointer_version,
    };
    event
        .verify()
        .map_err(|_| R::invalid(Why::LibraryPointer))?;
    pointer_version(&event.content, Why::LibraryPointer)?;
    if !event.values("expiration").is_empty() {
        return Err(R::unsupported(Why::Lifecycle).into());
    }
    pointer(event).map_err(|_| R::invalid(Why::LibraryPointer).into())
}
pub fn check_object_support(bytes: &[u8]) -> Result<()> {
    use crate::transfer_rejection::{
        RejectionReason as Why, TransferRejection as R, manifest_support,
    };
    let value = manifest::decode(bytes).map_err(|_| R::invalid(Why::LibraryObject))?;
    manifest_support(&value, Why::LibraryObject)?;
    Ok(())
}
fn text(s: &str) -> Value {
    Value::Text(s.into())
}
fn uint(n: u64) -> Value {
    Value::Integer(n.into())
}
fn encode(v: &Value) -> Result<Vec<u8>> {
    let mut b = Vec::new();
    ciborium::into_writer(v, &mut b)?;
    Ok(b)
}
fn map(entries: Vec<(&str, Value)>) -> Value {
    let mut entries: Vec<_> = entries
        .into_iter()
        .map(|(k, v)| {
            let key = text(k);
            (encode(&key).unwrap(), key, v)
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Value::Map(entries.into_iter().map(|(_, k, v)| (k, v)).collect())
}
fn fields<'a>(v: &'a Value, expected: &[&str]) -> Result<BTreeMap<String, &'a Value>> {
    let map = v.as_map().context("library object must be map")?;
    ensure!(map.len() == expected.len(), "library object fields");
    let mut out = BTreeMap::new();
    for (k, v) in map {
        let key = str_value(k)?;
        ensure!(expected.contains(&key), "unknown library field");
        ensure!(
            out.insert(key.into(), v).is_none(),
            "duplicate library field"
        );
    }
    Ok(out)
}
fn number(v: &Value) -> Result<u64> {
    u64::try_from(v.as_integer().context("unsigned integer required")?)
        .context("unsigned integer required")
}
fn str_value(v: &Value) -> Result<&str> {
    v.as_text().context("text required")
}
fn array(v: &Value) -> Result<&[Value]> {
    v.as_array().map(Vec::as_slice).context("array required")
}
fn bytes_exact<const N: usize>(v: &Value) -> Result<[u8; N]> {
    v.as_bytes()
        .context("binary identifier required")?
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("identifier length"))
}
