//! Pure, bounded intent checks over original signed-library bucket descriptors.
//! This is NOT admission, storage publication, a write capability or a commit.
//! Callers must obtain the previous root/bytes through the original current-head
//! and object policies, and still execute original CAS/retention authority fences.
use crate::{
    db::Id,
    library_manifest::{Chunk, Descriptor, Family, Limits, Record, Root, bucket_for},
};
use anyhow::{Context, Result, ensure};
use std::collections::{BTreeMap, BTreeSet};
pub const MAX_INTENT_IDS: usize = 1000;
pub const MAX_AFFECTED_BUCKETS: usize = 64;
pub const MAX_CHANGED_SCAN_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_CHANGED_RECORD_VISITS: u64 = 65536;
#[derive(Clone, Debug)]
pub enum Intent {
    ReadMarkers {
        read: BTreeSet<Id>,
        unread: BTreeSet<Id>,
    },
    Status {
        series: Id,
        status: Option<String>,
    },
}
#[derive(Clone, Debug)]
enum Action {
    Read,
    Unread,
    Status(Option<String>),
}
impl Intent {
    pub fn read_markers(read: &[Id], unread: &[Id]) -> Result<Self> {
        let r: BTreeSet<_> = read.iter().copied().collect();
        let u: BTreeSet<_> = unread.iter().copied().collect();
        ensure!(
            r.len() == read.len() && u.len() == unread.len(),
            "duplicate chapter intent"
        );
        let intent = Self::ReadMarkers { read: r, unread: u };
        intent.actions()?;
        Ok(intent)
    }
    pub fn status(series: Id, status: Option<String>) -> Result<Self> {
        let intent = Self::Status { series, status };
        intent.actions()?;
        Ok(intent)
    }
    fn actions(&self) -> Result<(Family, BTreeMap<Vec<u8>, Action>)> {
        match self {
            Self::ReadMarkers { read, unread } => {
                ensure!(
                    !read.is_empty() || !unread.is_empty(),
                    "empty read-marker intent"
                );
                ensure!(
                    read.len() + unread.len() <= MAX_INTENT_IDS && read.is_disjoint(unread),
                    "read-marker intent bound or overlap"
                );
                let mut actions = BTreeMap::new();
                for id in read {
                    actions.insert(id.to_vec(), Action::Read);
                }
                for id in unread {
                    actions.insert(id.to_vec(), Action::Unread);
                }
                Ok((Family::Chapters, actions))
            }
            Self::Status { series, status } => {
                if let Some(status) = status {
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
                        "unsupported reading status"
                    );
                }
                Ok((
                    Family::Series,
                    BTreeMap::from([(series.to_vec(), Action::Status(status.clone()))]),
                ))
            }
        }
    }
}
#[derive(Clone, Debug)]
struct Bucket {
    family: Family,
    index: usize,
    buckets: usize,
    old: Descriptor,
    new: Descriptor,
    actions: BTreeMap<Vec<u8>, Action>,
}
/// Descriptor-only preparation is O(root descriptors), never O(all records).
/// A no-op requested ID is still checked in its affected bucket: unchanged hash
/// alone cannot establish that the requested completed/status value is true.
pub struct DeltaPlan {
    affected: Vec<Bucket>,
    new_hashes: BTreeSet<Id>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeltaStats {
    pub old_chunks_decoded: usize,
    pub new_chunks_decoded: usize,
    pub records_visited: u64,
    pub bytes_decoded: u64,
    pub affected_buckets: usize,
}
impl DeltaPlan {
    pub fn prepare(old: Option<&Root>, new: &Root, intent: &Intent) -> Result<Self> {
        let limits = Limits::default();
        new.totals(&limits)?;
        if let Some(old) = old {
            old.totals(&limits)?;
        }
        let (family, actions) = intent.actions()?;
        let mut affected = vec![];
        let mut new_hashes = BTreeSet::new();
        let mut bytes = 0u64;
        let mut visits = 0u64;
        for (findex, n) in new.families.iter().enumerate() {
            let o = old.map(|old| &old.families[findex]);
            if let Some(o) = o {
                ensure!(
                    o.chunks.len() == n.chunks.len(),
                    "rebucketing requires the original full publication flow"
                );
            }
            let mut wanted: BTreeMap<usize, BTreeMap<Vec<u8>, Action>> = BTreeMap::new();
            if n.family == family {
                for (key, action) in &actions {
                    wanted
                        .entry(bucket_for(key, n.chunks.len())?)
                        .or_default()
                        .insert(key.clone(), action.clone());
                }
            }
            for (index, next) in n.chunks.iter().enumerate() {
                let previous = o
                    .map(|o| o.chunks[index].clone())
                    .unwrap_or_else(Descriptor::empty);
                if previous != *next {
                    ensure!(
                        n.family == family && wanted.contains_key(&index),
                        "unrelated library bucket changed"
                    );
                    if !next.is_empty() {
                        new_hashes.insert(next.hash);
                    }
                }
                if let Some(actions) = wanted.remove(&index) {
                    bytes = bytes
                        .checked_add(previous.size)
                        .context("scan byte overflow")?;
                    visits = visits
                        .checked_add(previous.records)
                        .context("scan record overflow")?;
                    if previous != *next {
                        bytes = bytes.checked_add(next.size).context("scan byte overflow")?;
                        visits = visits
                            .checked_add(next.records)
                            .context("scan record overflow")?;
                    }
                    ensure!(
                        bytes <= MAX_CHANGED_SCAN_BYTES && visits <= MAX_CHANGED_RECORD_VISITS,
                        "affected library scan budget"
                    );
                    affected.push(Bucket {
                        family: n.family,
                        index,
                        buckets: n.chunks.len(),
                        old: previous,
                        new: next.clone(),
                        actions,
                    });
                    ensure!(
                        affected.len() <= MAX_AFFECTED_BUCKETS,
                        "affected library bucket budget"
                    );
                }
            }
        }
        ensure!(
            new_hashes.len() + 1 <= crate::library_wire::MAX_OBJECTS,
            "changed library object count"
        );
        Ok(Self {
            affected,
            new_hashes,
        })
    }
    /// Only these bounded old descriptors require original object access. Unrelated
    /// chunk bytes are neither requested nor decoded, irrespective of library size.
    pub fn old_descriptors(&self) -> impl Iterator<Item = &Descriptor> {
        self.affected
            .iter()
            .map(|b| &b.old)
            .filter(|d| !d.is_empty())
    }
    pub fn validate(
        &self,
        new_chunks: &BTreeMap<Id, Vec<u8>>,
        mut load_old: impl FnMut(&Descriptor) -> Result<Vec<u8>>,
    ) -> Result<DeltaStats> {
        ensure!(
            new_chunks.keys().copied().collect::<BTreeSet<_>>() == self.new_hashes,
            "supply exactly the changed nonempty chunks"
        );
        let total = new_chunks
            .values()
            .try_fold(0usize, |sum, b| sum.checked_add(b.len()))
            .context("changed object byte overflow")?;
        ensure!(
            total <= 6 * 1024 * 1024,
            "changed object decoded-byte budget"
        );
        let mut stats = DeltaStats {
            affected_buckets: self.affected.len(),
            ..Default::default()
        };
        for bucket in &self.affected {
            let old = if bucket.old.is_empty() {
                vec![]
            } else {
                stats.old_chunks_decoded += 1;
                let bytes = load_old(&bucket.old)?;
                decode(&bytes, &bucket.old, bucket, &mut stats)?
            };
            let new = if bucket.old == bucket.new {
                old.clone()
            } else if bucket.new.is_empty() {
                vec![]
            } else {
                stats.new_chunks_decoded += 1;
                decode(
                    new_chunks
                        .get(&bucket.new.hash)
                        .context("missing changed chunk")?,
                    &bucket.new,
                    bucket,
                    &mut stats,
                )?
            };
            compare(bucket, &old, &new)?;
        }
        Ok(stats)
    }
}
fn decode(
    bytes: &[u8],
    descriptor: &Descriptor,
    bucket: &Bucket,
    stats: &mut DeltaStats,
) -> Result<Vec<Record>> {
    ensure!(
        bytes.len() as u64 == descriptor.size,
        "changed chunk size mismatch"
    );
    let (chunk, summary) = Chunk::decode(bytes, &Limits::default())?;
    ensure!(
        Descriptor::from(&summary) == *descriptor
            && summary.fits(bucket.family, bucket.buckets, bucket.index),
        "changed chunk descriptor/family/bucket mismatch"
    );
    stats.bytes_decoded = stats
        .bytes_decoded
        .checked_add(bytes.len() as u64)
        .context("scan byte overflow")?;
    stats.records_visited = stats
        .records_visited
        .checked_add(chunk.records().len() as u64)
        .context("scan record overflow")?;
    ensure!(
        stats.bytes_decoded <= MAX_CHANGED_SCAN_BYTES
            && stats.records_visited <= MAX_CHANGED_RECORD_VISITS,
        "affected library scan budget"
    );
    Ok(chunk.records().to_vec())
}
fn compare(bucket: &Bucket, old: &[Record], new: &[Record]) -> Result<()> {
    let old = old.iter().map(|r| (r.key(), r)).collect::<BTreeMap<_, _>>();
    let new = new.iter().map(|r| (r.key(), r)).collect::<BTreeMap<_, _>>();
    let keys = old.keys().chain(new.keys()).collect::<BTreeSet<_>>();
    for key in keys {
        if !bucket.actions.contains_key(key) {
            ensure!(
                old.get(key) == new.get(key),
                "unrelated library record changed"
            );
        }
    }
    for (key, action) in &bucket.actions {
        let previous = old.get(key).copied();
        let next = new.get(key).copied();
        match action {
            Action::Read => {
                let page = match previous {
                    Some(Record::Chapter { page, .. }) => *page,
                    None => None,
                    _ => anyhow::bail!("chapter family mismatch"),
                };
                ensure!(
                    matches!(next,Some(Record::Chapter{completed:true,page:p,..})if *p==page),
                    "read intent must complete the chapter and preserve page"
                );
            }
            Action::Unread => {
                let page = match previous {
                    Some(Record::Chapter { page, .. }) => *page,
                    None => None,
                    _ => anyhow::bail!("chapter family mismatch"),
                };
                match next {
                    None => ensure!(
                        page.is_none(),
                        "unread removal would discard partial-page progress"
                    ),
                    Some(Record::Chapter {
                        completed: false,
                        page: p,
                        ..
                    }) => ensure!(*p == page, "unread intent must preserve page"),
                    _ => anyhow::bail!("unread intent must clear completion"),
                }
            }
            Action::Status(None) => {
                ensure!(next.is_none(), "null status must remove the series record")
            }
            Action::Status(Some(want)) => ensure!(
                matches!(next,Some(Record::Series{status,..})if status==want),
                "series status intent mismatch"
            ),
        }
    }
    Ok(())
}
