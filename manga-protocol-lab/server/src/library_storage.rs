//! Private, generation-fenced staging for public-library objects. Staging is not
//! publication. Root commit, public retention and reclamation remain separate.
use crate::{
    accounts::{AccountPermit, check_account_authority_tx as check_account_tx},
    blobs::BlobIntent,
    db::{Id, Repository},
    library_manifest::{Chunk, Limits, Root, VerifiedChunk},
    upload::UploadStore,
};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectType {
    Chunk,
    Root,
}
impl ObjectType {
    fn name(self) -> &'static str {
        match self {
            Self::Chunk => "chunk",
            Self::Root => "root",
        }
    }
}
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LibraryConcurrency {
    pub request_slots: usize,
    pub processing_slots: usize,
    pub commit_slots: usize,
    pub validation_cache_bytes: u64,
    pub validation_cache_entries: usize,
}
impl Default for LibraryConcurrency {
    fn default() -> Self {
        Self {
            request_slots: 128,
            processing_slots: 8,
            commit_slots: 4,
            validation_cache_bytes: 8 * 1024 * 1024,
            validation_cache_entries: 256,
        }
    }
}
impl LibraryConcurrency {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=32).contains(&self.processing_slots),
            "library processing slots outside1..32"
        );
        ensure!(
            (1..=16).contains(&self.commit_slots) && self.commit_slots <= self.processing_slots,
            "invalid library commit slots"
        );
        ensure!(
            (self.processing_slots..=512).contains(&self.request_slots),
            "invalid library request slots"
        );
        ensure!(
            (self.validation_cache_bytes == 0 && self.validation_cache_entries == 0)
                || ((1..=64 * 1024 * 1024).contains(&self.validation_cache_bytes)
                    && (1..=4096).contains(&self.validation_cache_entries)),
            "invalid library validation cache limits"
        );
        Ok(())
    }
}
pub struct LibraryObjects {
    pub(crate) repo: Repository,
    pub(crate) validation: crate::library_references::ValidationCache,
    pub(crate) storage: Arc<UploadStore>,
    pub(crate) commits: Arc<tokio::sync::Semaphore>,
    pub(crate) admission: crate::library_admission::Admission,
    pub(crate) reads: Arc<tokio::sync::Semaphore>,
    pub(crate) response_bytes: Arc<tokio::sync::Semaphore>,
}
impl LibraryObjects {
    pub fn new(repo: Repository, storage: Arc<UploadStore>) -> Arc<Self> {
        Self::build(repo, storage, LibraryConcurrency::default())
    }
    pub fn configured(
        repo: Repository,
        storage: Arc<UploadStore>,
        limits: LibraryConcurrency,
    ) -> Result<Arc<Self>> {
        limits.validate()?;
        Ok(Self::build(repo, storage, limits))
    }
    fn build(repo: Repository, storage: Arc<UploadStore>, limits: LibraryConcurrency) -> Arc<Self> {
        Arc::new(Self {
            validation: crate::library_references::ValidationCache::new(
                limits.validation_cache_bytes,
                limits.validation_cache_entries,
            ),
            repo,
            storage,
            commits: Arc::new(tokio::sync::Semaphore::new(limits.commit_slots)),
            admission: crate::library_admission::Admission::with_limits(
                limits.request_slots,
                limits.processing_slots,
            ),
            reads: Arc::new(tokio::sync::Semaphore::new(4)),
            response_bytes: Arc::new(tokio::sync::Semaphore::new(64 * 1024 * 1024)),
        })
    }
    /// Callers must bound body receipt before constructing bytes. Owned execution
    /// retains storage admission permits through I/O even after caller disconnect.
    pub async fn stage(
        self: &Arc<Self>,
        permit: AccountPermit,
        kind: ObjectType,
        bytes: Vec<u8>,
    ) -> Result<BlobIntent> {
        let limits = Limits::default();
        let max = match kind {
            ObjectType::Chunk => limits.max_chunk_bytes,
            ObjectType::Root => limits.max_root_bytes,
        };
        ensure!(
            !bytes.is_empty() && bytes.len() <= max,
            "library object byte limit"
        );
        let mut waiting = crate::perf::Trace::new("stage_queue");
        let slot = crate::library_admission::wait(self.storage.slots.clone()).await?;
        waiting.mark("storage_slot");
        drop(waiting);
        let this = self.clone();
        crate::node::spawn_owned_scope(async move {
            let _slot = slot;
            let mut trace = crate::perf::Trace::new("stage");
            let (bytes, summary) = crate::node::spawn_blocking_owned_scope(move || -> Result<_> {
                let summary = match kind {
                    ObjectType::Chunk => {
                        let (_, summary) = Chunk::decode(&bytes, &limits)?;
                        ensure!(summary.records() > 0, "empty buckets require no object");
                        Some(summary)
                    }
                    ObjectType::Root => {
                        Root::decode(&bytes, &limits)?;
                        None
                    }
                };
                Ok((bytes, summary))
            })
            .await??;
            trace.mark("decode");
            // Re-check before consuming storage work; final admission is fenced again.
            this.repo.verify_account_storage_permit(&permit).await?;
            let hash: Id = hex::decode(crate::auth::hash(&bytes))?.try_into().unwrap();
            trace.mark("authority");
            let intent = this.storage.put(hash, &bytes).await?;
            trace.mark("put");
            this.grant(&permit, kind, &intent, summary.as_ref()).await?;
            trace.mark("grant");
            Ok(intent)
        })
        .await?
    }
    async fn grant(
        &self,
        permit: &AccountPermit,
        kind: ObjectType,
        intent: &BlobIntent,
        summary: Option<&VerifiedChunk>,
    ) -> Result<()> {
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        check_account_tx(&tx, permit).await?;
        let row=tx.query_one(&tx.prepare_cached("SELECT generation,state,size,owner,volume FROM blob_intents WHERE hash=$1 FOR SHARE").await?, &[&intent.key.hash.as_slice()]).await?;
        ensure!(
            row.get::<_, Vec<u8>>(0) == intent.key.generation
                && row.get::<_, String>(1) == "available"
                && row.get::<_, i64>(2) == i64::try_from(intent.size)?
                && row.get::<_, Vec<u8>>(3) == intent.owner
                && row.get::<_, String>(4) == intent.volume,
            "stale library object receipt"
        );
        let health = tx.query_one(&tx.prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM quarantined_blobs WHERE hash=$1 AND generation=$2),EXISTS(SELECT 1 FROM blob_reclaims WHERE hash=$1 AND generation=$2)"
        ).await?, &[&intent.key.hash.as_slice(), &intent.key.generation.as_slice()]).await?;
        ensure!(!health.get::<_, bool>(0), "library object quarantined");
        ensure!(
            !health.get::<_, bool>(1),
            "library object reclamation pending"
        );
        if let Some(s) = summary {
            ensure!(
                kind == ObjectType::Chunk && s.hash() == intent.key.hash && s.size() == intent.size,
                "chunk receipt mismatch"
            );
            let (base, bits) = s.storage_bucket_summary();
            tx.execute(&tx.prepare_cached("INSERT INTO library_chunk_summaries(hash,generation,family,size,records,bucket_base,common_low_bits) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(hash,generation) DO NOTHING").await?, &[&intent.key.hash.as_slice(),&intent.key.generation.as_slice(),&s.family().name(),&i64::try_from(s.size())?,&i64::try_from(s.records())?,&i64::from(base),&(bits as i32)]).await?;
        } else {
            ensure!(kind == ObjectType::Root, "chunk summary required");
        }
        tx.execute(&tx.prepare_cached("INSERT INTO account_blob_grants(publisher,hash,generation,object_type,lease_until) VALUES($1,$2,$3,$4,clock_timestamp()+interval '5 minutes') ON CONFLICT(publisher,hash,generation) DO UPDATE SET lease_until=EXCLUDED.lease_until").await?, &[&permit.publisher.as_slice(),&intent.key.hash.as_slice(),&intent.key.generation.as_slice(),&kind.name()]).await?;
        tx.commit().await?;
        Ok(())
    }
    /// Private staging access requires an exact generation grant. No hash-only
    /// access and no anonymous visibility are conferred by uploading an object.
    pub async fn read(self: &Arc<Self>, permit: &AccountPermit, hash: Id) -> Result<Vec<u8>> {
        // Snapshot owns its independent read permit through filesystem I/O.
        // An unrelated staged write must not consume library read admission.
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        check_account_tx(&tx, permit).await?;
        let row=tx.query_opt(&tx.prepare_cached("SELECT b.owner,b.generation FROM account_blob_grants g JOIN blob_intents b ON b.hash=g.hash AND b.generation=g.generation WHERE g.publisher=$1 AND g.hash=$2 AND b.state='available' AND NOT EXISTS(SELECT 1 FROM quarantined_blobs q WHERE q.hash=b.hash AND q.generation=b.generation)").await?, &[&permit.publisher.as_slice(),&hash.as_slice()]).await?.context("library staging grant required")?;
        let owner: Id = row
            .get::<_, Vec<u8>>(0)
            .try_into()
            .map_err(|_| anyhow::anyhow!("owner encoding"))?;
        let generation: Vec<u8> = row.get(1);
        tx.commit().await?;
        drop(client);
        let (intent, bytes) = self.storage.snapshot(hash, owner).await?;
        ensure!(
            intent.key.generation.as_slice() == generation,
            "library object generation changed"
        );
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        check_account_tx(&tx, permit).await?;
        ensure!(tx.query_opt(&tx.prepare_cached("SELECT 1 FROM account_blob_grants g JOIN blob_intents b ON b.hash=g.hash AND b.generation=g.generation WHERE g.publisher=$1 AND g.hash=$2 AND g.generation=$3 AND b.state='available' AND NOT EXISTS(SELECT 1 FROM quarantined_blobs q WHERE q.hash=b.hash AND q.generation=b.generation)").await?, &[&permit.publisher.as_slice(),&hash.as_slice(),&generation]).await?.is_some(),"library staging grant changed");
        tx.commit().await?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod concurrency_tests {
    use super::*;
    #[test]
    fn bounded_defaults_and_operator_limits() {
        let defaults: LibraryConcurrency = serde_json::from_str("{}").unwrap();
        defaults.validate().unwrap();
        assert_eq!(defaults.processing_slots, 8);
        assert_eq!(defaults.commit_slots, 4);
        assert_eq!(defaults.request_slots, 128);
        for (request_slots, processing_slots, commit_slots) in [
            (0, 8, 4),
            (128, 0, 1),
            (128, 33, 4),
            (128, 8, 9),
            (4, 8, 4),
            (513, 8, 4),
            (128, 32, 17),
        ] {
            assert!(
                LibraryConcurrency {
                    request_slots,
                    processing_slots,
                    commit_slots,
                    ..LibraryConcurrency::default()
                }
                .validate()
                .is_err()
            );
        }
        LibraryConcurrency {
            request_slots: 256,
            processing_slots: 16,
            commit_slots: 8,
            ..LibraryConcurrency::default()
        }
        .validate()
        .unwrap();
        for (bytes, entries) in [(0, 1), (1, 0), (64 * 1024 * 1024 + 1, 1), (1, 4097)] {
            assert!(
                LibraryConcurrency {
                    validation_cache_bytes: bytes,
                    validation_cache_entries: entries,
                    ..LibraryConcurrency::default()
                }
                .validate()
                .is_err()
            );
        }
        LibraryConcurrency {
            validation_cache_bytes: 0,
            validation_cache_entries: 0,
            ..LibraryConcurrency::default()
        }
        .validate()
        .unwrap();
    }
}
