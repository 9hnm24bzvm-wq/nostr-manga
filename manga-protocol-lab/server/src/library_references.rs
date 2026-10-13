//! Immutable root-reference preparation outside the author transaction. The
//! prepared value is not authority: commit must lock and recheck its exact local
//! generations, grants and health before retaining anything.
use crate::{
    db::{Id, Repository},
    library_manifest::{Limits, Root, VerifiedChunk},
};
use anyhow::{Context, Result, ensure};
use moka::future::Cache;
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

/// Derived immutable facts from one successfully committed root. Neither this
/// value nor its cache can establish current visibility or account authority.
pub(crate) struct AcceptedValidation {
    event: Id,
    root_hash: Id,
    root_size: u64,
    root: Arc<Root>,
    generations: BTreeMap<Id, [u8; 16]>,
    summaries: HashMap<Id, VerifiedChunk>,
}
impl AcceptedValidation {
    pub(crate) fn decoded_root(&self, hash: Id, size: u64) -> Option<Arc<Root>> {
        (self.root_hash == hash && self.root_size == size).then(|| self.root.clone())
    }
    fn weight(&self) -> u32 {
        // Conservative accounting for map capacity, decoded descriptors and
        // allocation headers. Raw object bytes are never retained in this cache.
        let descriptors: usize = self.root.families.iter().map(|f| f.chunks.len()).sum();
        u32::try_from(4096 + 512 * self.generations.len() + 64 * descriptors).unwrap_or(u32::MAX)
    }
}
pub(crate) struct ValidationCache {
    accepted: Cache<Id, Arc<AcceptedValidation>>,
    capacity: u64,
}
impl ValidationCache {
    pub(crate) fn new(bytes: u64, entries: usize) -> Self {
        // Minimum charged weight also bounds entry count without a second,
        // separately synchronized eviction registry.
        let minimum = bytes.div_ceil(entries.max(1) as u64) as u32;
        Self {
            accepted: Cache::builder()
                .max_capacity(bytes)
                .weigher(move |_: &Id, value: &Arc<AcceptedValidation>| value.weight().max(minimum))
                .build(),
            capacity: bytes,
        }
    }
    pub(crate) async fn previous(
        &self,
        publisher: Id,
        event: Option<&[u8]>,
    ) -> Option<Arc<AcceptedValidation>> {
        self.accepted
            .get(&publisher)
            .await
            .filter(|value| Some(value.event.as_slice()) == event)
    }
    pub(crate) async fn remember(&self, publisher: Id, event: Id, prepared: PreparedReferences) {
        let value = Arc::new(AcceptedValidation {
            event,
            root_hash: prepared.root_hash,
            root_size: prepared.sizes[&prepared.root_hash],
            root: prepared.root,
            generations: prepared.generations,
            summaries: prepared.summaries,
        });
        if self.capacity > 0 && u64::from(value.weight()) <= self.capacity {
            self.accepted.insert(publisher, value).await;
            self.accepted.run_pending_tasks().await;
        }
    }
}

pub(crate) struct PreparedReferences {
    root_hash: Id,
    root: Arc<Root>,
    owner: Id,
    sizes: BTreeMap<Id, u64>,
    generations: BTreeMap<Id, [u8; 16]>,
    summaries: HashMap<Id, VerifiedChunk>,
    hashes: Vec<Vec<u8>>,
}
impl PreparedReferences {
    pub(crate) async fn prepare(
        repo: &Repository,
        publisher: Id,
        owner: Id,
        root_hash: Id,
        root_size: u64,
        root: Arc<Root>,
        previous: Option<&AcceptedValidation>,
    ) -> Result<Self> {
        let mut trace = crate::perf::Trace::new("reference_validation");
        let mut sizes = BTreeMap::from([(root_hash, root_size)]);
        for family in &root.families {
            for descriptor in &family.chunks {
                if !descriptor.is_empty() {
                    ensure!(descriptor.hash != root_hash, "root cannot be its own chunk");
                    if let Some(size) = sizes.insert(descriptor.hash, descriptor.size) {
                        ensure!(size == descriptor.size, "conflicting descriptor size");
                    }
                }
            }
        }
        let hashes: Vec<Vec<u8>> = sizes.keys().map(|hash| hash.to_vec()).collect();
        // Always observe every current generation and grant. Immutable summary
        // reuse never removes this authority/receipt lookup or the commit fence.
        let rows = repo.pool.get().await?.query(
            "SELECT b.hash,b.generation,b.size,b.owner,b.state,g.object_type FROM blob_intents b JOIN account_blob_grants g ON g.hash=b.hash AND g.generation=b.generation AND g.publisher=$1 WHERE b.hash=ANY($2)",
            &[&publisher.as_slice(), &hashes],
        ).await?;
        ensure!(
            rows.len() == hashes.len(),
            "library object or account grant missing"
        );
        let mut generations = BTreeMap::new();
        for row in rows {
            let hash: Id = row
                .get::<_, Vec<u8>>(0)
                .try_into()
                .ok()
                .context("object hash")?;
            let generation: [u8; 16] = row
                .get::<_, Vec<u8>>(1)
                .try_into()
                .ok()
                .context("object generation")?;
            ensure!(
                row.get::<_, i64>(2) == i64::try_from(sizes[&hash])?
                    && row.get::<_, Vec<u8>>(3) == owner
                    && row.get::<_, String>(4) == "available",
                "library generation/size/owner unavailable"
            );
            ensure!(
                row.get::<_, String>(5) == if hash == root_hash { "root" } else { "chunk" },
                "library object grant mismatch"
            );
            generations.insert(hash, generation);
        }
        trace.mark("current_receipts");
        trace.count("current_objects", generations.len() as u64);
        let mut summaries = HashMap::new();
        if let Some(previous) = previous {
            for (family, old_family) in root.families.iter().zip(&previous.root.families) {
                if family.family != old_family.family
                    || family.chunks.len() != old_family.chunks.len()
                {
                    continue;
                }
                for (descriptor, old_descriptor) in family.chunks.iter().zip(&old_family.chunks) {
                    if !descriptor.is_empty()
                        && descriptor == old_descriptor
                        && previous.generations.get(&descriptor.hash)
                            == generations.get(&descriptor.hash)
                        && let Some(summary) = previous.summaries.get(&descriptor.hash)
                    {
                        summaries.insert(descriptor.hash, summary.clone());
                    }
                }
            }
        }
        let missing: Vec<Vec<u8>> = generations
            .keys()
            .filter(|hash| **hash != root_hash && !summaries.contains_key(*hash))
            .map(|hash| hash.to_vec())
            .collect();
        trace.count("summaries_reused", summaries.len() as u64);
        trace.count("summaries_requested", missing.len() as u64);
        if !missing.is_empty() {
            let rows = repo.pool.get().await?.query(
                "SELECT hash,generation,family,size,records,bucket_base,common_low_bits FROM library_chunk_summaries WHERE hash=ANY($1)", &[&missing]
            ).await?;
            ensure!(rows.len() == missing.len(), "chunk summary missing");
            for row in rows {
                let hash: Id = row
                    .get::<_, Vec<u8>>(0)
                    .try_into()
                    .ok()
                    .context("summary hash")?;
                ensure!(
                    row.get::<_, Vec<u8>>(1) == generations[&hash],
                    "chunk summary generation changed"
                );
                summaries.insert(
                    hash,
                    VerifiedChunk::from_storage(
                        hash,
                        &row.get::<_, String>(2),
                        u64::try_from(row.get::<_, i64>(3))?,
                        u64::try_from(row.get::<_, i64>(4))?,
                        u32::try_from(row.get::<_, i64>(5))?,
                        u32::try_from(row.get::<_, i32>(6))?,
                    )?,
                );
            }
        }
        trace.mark("immutable_summaries");
        // Cheap whole-root limits and bucket placement remain mandatory even
        // when immutable summaries came from the prior accepted root.
        root.validate_chunks(&summaries, &Limits::default())?;
        trace.mark("root_structure");
        Ok(Self {
            root_hash,
            root,
            owner,
            sizes,
            generations,
            summaries,
            hashes,
        })
    }

    /// Caller already owns the author/authority fence. Locks are sorted before
    /// grants/credits/holds; a prepared value may never adopt a newer generation.
    pub(crate) async fn lock_and_recheck(
        &self,
        tx: &deadpool_postgres::Transaction<'_>,
        publisher: Id,
    ) -> Result<&BTreeMap<Id, [u8; 16]>> {
        let rows = tx.query(&tx.prepare_cached(
            "SELECT hash,generation,size,owner,state FROM blob_intents WHERE hash=ANY($1) ORDER BY hash FOR SHARE"
        ).await?, &[&self.hashes]).await?;
        ensure!(
            rows.len() == self.hashes.len(),
            "prepared library object missing"
        );
        for row in rows {
            let hash: Id = row
                .get::<_, Vec<u8>>(0)
                .try_into()
                .ok()
                .context("object hash")?;
            ensure!(
                row.get::<_, Vec<u8>>(1) == self.generations[&hash]
                    && row.get::<_, i64>(2) == i64::try_from(self.sizes[&hash])?
                    && row.get::<_, Vec<u8>>(3) == self.owner
                    && row.get::<_, String>(4) == "available",
                "prepared library generation/size/owner changed"
            );
        }
        let grants = tx.query(&tx.prepare_cached(
            "SELECT g.hash,g.generation,g.object_type,EXISTS(SELECT 1 FROM quarantined_blobs q WHERE q.hash=g.hash AND q.generation=g.generation) OR EXISTS(SELECT 1 FROM blob_reclaims r WHERE r.hash=g.hash AND r.generation=g.generation) FROM account_blob_grants g WHERE g.publisher=$1 AND g.hash=ANY($2) ORDER BY g.hash FOR SHARE OF g"
        ).await?, &[&publisher.as_slice(), &self.hashes]).await?;
        ensure!(
            grants.len() == self.hashes.len(),
            "account lacks a library object grant"
        );
        for row in grants {
            ensure!(
                !row.get::<_, bool>(3),
                "library object quarantined or reclamation pending"
            );
            let hash: Id = row
                .get::<_, Vec<u8>>(0)
                .try_into()
                .ok()
                .context("grant hash")?;
            ensure!(
                row.get::<_, Vec<u8>>(1) == self.generations[&hash]
                    && row.get::<_, String>(2)
                        == if hash == self.root_hash {
                            "root"
                        } else {
                            "chunk"
                        },
                "prepared library object grant changed"
            );
        }
        Ok(&self.generations)
    }
}
