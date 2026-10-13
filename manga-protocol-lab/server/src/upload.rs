//! Bounded storage-owner role. API replicas may route to this role later; they
//! must not open a second mutation owner on the same physical volume.
use crate::{
    blobs::BlobIntent,
    db::{Id, Repository},
    storage::FilesystemVolume,
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{Mutex, RwLock, Semaphore};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    pub path: PathBuf,
    pub volume: String,
    pub physical_bytes: i64,
    pub staging_bytes: i64,
    #[serde(default = "max_bytes")]
    pub max_blob_bytes: u64,
    #[serde(default = "slots")]
    pub upload_slots: usize,
    /// Maximum cleanup workers; volume admission reserves foreground capacity.
    #[serde(default = "slots")]
    pub reclamation_slots: usize,
    #[serde(default = "credit_lanes")]
    pub credit_lanes: usize,
    #[serde(default = "formats")]
    pub formats: Vec<String>,
}
impl StorageConfig {
    fn validate_concurrency(&self) -> Result<()> {
        ensure!(
            (1..=16).contains(&self.upload_slots),
            "upload slots outside 1..16"
        );
        ensure!(
            (1..=16).contains(&self.reclamation_slots),
            "reclamation slots outside 1..16"
        );
        Ok(())
    }
}
fn formats() -> Vec<String> {
    vec!["image/png".into(), "image/jpeg".into(), "image/webp".into()]
}
fn max_bytes() -> u64 {
    16 * 1024 * 1024
}
fn credit_lanes() -> usize {
    16
}
fn slots() -> usize {
    4
}
#[derive(Debug, thiserror::Error)]
#[error("blob unavailable to actor")]
pub struct BlobUnavailable;
pub struct PutOutcome {
    pub intent: BlobIntent,
    pub created: bool,
}
#[derive(Debug, thiserror::Error)]
#[error("original verification failed and serving was withdrawn")]
pub struct ReadVerificationFailed;

#[derive(Debug, thiserror::Error)]
#[error("original upload admission busy")]
pub(crate) struct UploadBusy;

pub struct UploadStore {
    pub(crate) repo: Repository,
    volume: Arc<FilesystemVolume>,
    name: String,
    owner: Id,
    holder: Id,
    budgets: [String; 2],
    grants: RwLock<()>,
    grant_lanes: Vec<Mutex<()>>,
    grant_refill: Mutex<()>,
    audit_lock: Mutex<()>,
    export_drain_lock: Arc<Mutex<()>>,
    export_work_lock: Arc<Mutex<()>>,
    grant_batches: [i64; 2],
    pub max_blob_bytes: u64,
    pub(crate) formats: Vec<String>,
    pub slots: Arc<Semaphore>,
    upload_waiters: Arc<Semaphore>,
    read_slots: Arc<Semaphore>,
    downloads: crate::download_budget::Budget,
    reclamation_slots: usize,
    public_fault: tokio::sync::watch::Sender<bool>,
}
impl UploadStore {
    /// Original receipt joins the same FIFO as library staging before reading
    /// its body. At most upload_slots originals wait, for at most two seconds;
    /// active body count/bytes remain bounded by the existing shared slots.
    /// Dropping this future removes both its waiting ticket and FIFO position.
    pub(crate) async fn admit_original_upload(
        &self,
    ) -> std::result::Result<tokio::sync::OwnedSemaphorePermit, UploadBusy> {
        let _waiting = self
            .upload_waiters
            .clone()
            .try_acquire_owned()
            .map_err(|_| UploadBusy)?;
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            self.slots.clone().acquire_owned(),
        )
        .await
        .map_err(|_| UploadBusy)?
        .map_err(|_| UploadBusy)
    }
    pub(crate) fn public_faults(&self) -> tokio::sync::watch::Receiver<bool> {
        self.public_fault.subscribe()
    }
    pub(crate) fn volume_name(&self) -> &str {
        &self.name
    }
    pub fn public_volume_identity(&self) -> Id {
        self.volume.identity()
    }
    pub(crate) fn owner(&self) -> Id {
        self.owner
    }

    pub async fn open(
        repo: Repository,
        cfg: StorageConfig,
        owner: Id,
        holder: Id,
    ) -> Result<Arc<Self>> {
        ensure!(
            (1..=256).contains(&cfg.credit_lanes) && cfg.credit_lanes.is_power_of_two(),
            "credit lanes must be a power of two within1..256"
        );
        cfg.validate_concurrency()?;
        ensure!(
            (1..=64 * 1024 * 1024).contains(&cfg.max_blob_bytes),
            "blob limit outside 1..64MiB"
        );
        ensure!(
            !cfg.volume.is_empty()
                && cfg.volume.len() <= 64
                && cfg
                    .volume
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "invalid volume name"
        );
        ensure!(
            cfg.formats.iter().all(|m| formats().contains(m)),
            "unsupported local image format"
        );
        let volume =
            FilesystemVolume::open_with_policy(cfg.path, cfg.upload_slots, cfg.reclamation_slots)?;
        let budgets = [
            format!("physical:{}", cfg.volume),
            format!("staging:{}", hex::encode(owner)),
        ];
        repo.configure_budget(&budgets[0], cfg.physical_bytes)
            .await?;
        repo.configure_budget(&budgets[1], cfg.staging_bytes)
            .await?;
        Ok(Arc::new(Self {
            repo,
            volume: Arc::new(volume),
            name: cfg.volume,
            owner,
            holder,
            budgets,
            grants: RwLock::new(()),
            grant_lanes: (0..cfg.credit_lanes).map(|_| Mutex::new(())).collect(),
            grant_refill: Mutex::new(()),
            audit_lock: Mutex::new(()),
            export_drain_lock: Arc::new(Mutex::new(())),
            export_work_lock: Arc::new(Mutex::new(())),
            grant_batches: [
                (cfg.physical_bytes / (4 * cfg.credit_lanes as i64)).clamp(1, 4 * 1024 * 1024),
                (cfg.staging_bytes / (4 * cfg.credit_lanes as i64)).clamp(1, 4 * 1024 * 1024),
            ],
            max_blob_bytes: cfg.max_blob_bytes,
            formats: cfg.formats,
            slots: Arc::new(Semaphore::new(cfg.upload_slots)),
            upload_waiters: Arc::new(Semaphore::new(cfg.upload_slots)),
            read_slots: Arc::new(Semaphore::new(cfg.upload_slots)),
            downloads: crate::download_budget::Budget::new(4, 256 * 1024 * 1024)?,
            reclamation_slots: cfg.reclamation_slots,
            public_fault: tokio::sync::watch::channel(false).0,
        }))
    }
    fn credit_holder(&self, lane: usize) -> Id {
        let mut hash = Sha256::new();
        hash.update(b"manga-storage-credit-lane-v1");
        hash.update(self.holder);
        hash.update([lane as u8]);
        hash.finalize().into()
    }
    // Independent credit lanes remove the instance-wide per-object hot grant.
    // A shared gate excludes rare rebalance while a lane selects/reserves credits.
    // File I/O, settlement and publication never hold this process-local gate.
    async fn begin(&self, hash: Id, size: u64) -> Result<BlobIntent> {
        let lane = usize::from(hash[0]) % self.grant_lanes.len();
        {
            let _gate = self.grants.read().await;
            let _lane = self.grant_lanes[lane].lock().await;
            if let Some(intent) = self.begin_lane(hash, size, lane).await? {
                return Ok(intent);
            }
        }
        // Idle credits in another lane must not cause false quota exhaustion.
        // Seal/release only unused bytes; uncertain or used charges stay allocated.
        let _gate = self.grants.write().await;
        let _lane = self.grant_lanes[lane].lock().await;
        let mut holders: Vec<Vec<u8>> = (0..256).map(|n| self.credit_holder(n).to_vec()).collect();
        holders.push(self.holder.to_vec()); // also reclaim pre-lane legacy grants
        for budget in &self.budgets {
            let rows=self.repo.pool.get().await?.query("SELECT id FROM capacity_grants WHERE budget_key=$1 AND holder=ANY($2) AND (state='active' OR available>0) ORDER BY id", &[budget,&holders]).await?;
            for row in rows {
                self.repo
                    .seal_and_release_unused(
                        row.get::<_, Vec<u8>>(0)
                            .try_into()
                            .ok()
                            .context("grant encoding")?,
                    )
                    .await?;
            }
        }
        self.begin_lane(hash, size, lane)
            .await?
            .context("storage quota exhausted")
    }
    async fn begin_lane(&self, hash: Id, size: u64, lane: usize) -> Result<Option<BlobIntent>> {
        if let Some(old) = self.repo.blob_intent(hash).await? {
            ensure!(
                old.owner == self.owner && old.volume == self.name && old.size == size,
                "existing blob descriptor/owner mismatch"
            );
            return Ok(Some(old));
        }
        let holder = self.credit_holder(lane);
        let mut grants = Vec::new();
        for (index, budget) in self.budgets.iter().enumerate() {
            let existing=self.repo.pool.get().await?.query_opt("SELECT id FROM capacity_grants WHERE budget_key=$1 AND holder=$2 AND state='active' AND available >= $3 ORDER BY available,id LIMIT 1", &[budget,&holder.as_slice(),&(size as i64)]).await?;
            if let Some(row) = existing {
                grants.push(
                    row.get::<_, Vec<u8>>(0)
                        .try_into()
                        .ok()
                        .context("grant encoding")?,
                );
                continue;
            }
            // Only rare grant refills serialize against the shared budget counter.
            let _refill = self.grant_refill.lock().await;
            let old=self.repo.pool.get().await?.query("SELECT id FROM capacity_grants WHERE budget_key=$1 AND holder=$2 AND (state='active' OR available>0) ORDER BY id", &[budget,&holder.as_slice()]).await?;
            for row in old {
                self.repo
                    .seal_and_release_unused(
                        row.get::<_, Vec<u8>>(0)
                            .try_into()
                            .ok()
                            .context("grant encoding")?,
                    )
                    .await?;
            }
            let free: i64 = self
                .repo
                .pool
                .get()
                .await?
                .query_one(
                    "SELECT limit_bytes-allocated_bytes FROM capacity_budgets WHERE key=$1",
                    &[budget],
                )
                .await?
                .get(0);
            if free < size as i64 {
                return Ok(None);
            }
            grants.push(
                self.repo
                    .allocate_grant(
                        budget,
                        holder,
                        free.min(self.grant_batches[index].max(size as i64)),
                    )
                    .await?,
            );
        }
        Ok(Some(
            self.repo
                .begin_blob(hash, &self.name, self.owner, size, &grants)
                .await?,
        ))
    }
    /// Run in an owned task holding its semaphore permit. Disconnects must not
    /// cancel file I/O and release the hash guard while a write is still running.
    pub async fn put(&self, hash: Id, bytes: &[u8]) -> Result<BlobIntent> {
        Ok(self.put_detailed(hash, bytes).await?.intent)
    }
    pub async fn put_detailed(&self, hash: Id, bytes: &[u8]) -> Result<PutOutcome> {
        let mut trace = crate::perf::Trace::new("put");
        ensure!(
            !bytes.is_empty() && bytes.len() as u64 <= self.max_blob_bytes,
            "blob size budget"
        );
        ensure!(
            crate::auth::hash(bytes) == hex::encode(hash),
            "blob hash mismatch"
        );
        let mutation = self.volume.mutation(hash).await;
        trace.mark("mutation");
        let intent = self.begin(hash, bytes.len() as u64).await?;
        trace.mark("credit_admission");
        let receipt = match mutation
            .put_verified(intent.key.generation, bytes, self.max_blob_bytes)
            .await
        {
            Ok(receipt) => receipt,
            Err(error) => {
                // Available corruption is not an unfinished upload: never refund
                // or silently overwrite it here. Maintenance handles withdrawal.
                if intent.available {
                    self.public_fault.send_replace(true);
                    self.repo
                        .begin_public_withdrawal(&intent.key, self.volume.identity(), true)
                        .await?;
                    let withdrawal = mutation.withdraw_public(intent.key.generation).await?;
                    self.repo
                        .quarantine_withdrawn(&intent.key, "write_verification_failed", &withdrawal)
                        .await?;
                }
                if !intent.available {
                    let cleanup = mutation.cleanup(intent.key.generation).await?;
                    self.repo.abort_blob(&self.name, &cleanup).await?;
                }
                return Err(error);
            }
        };
        trace.mark("file_write_sync");
        self.repo.finish_blob(&self.name, &receipt).await?;
        trace.mark("settle");
        let committed = self
            .repo
            .blob_intent(hash)
            .await?
            .context("committed blob disappeared")?;
        Ok(PutOutcome {
            intent: committed,
            created: !intent.available,
        })
    }
    /// One bounded recovery page. A process restart is not proof of cleanup:
    /// inspect and sync existing bytes, otherwise unlink this generation before refund.
    pub async fn recover_page(&self, after: Option<Id>) -> Result<Option<Id>> {
        let after = after.map(|v| v.to_vec());
        let rows=self.repo.pool.get().await?.query("SELECT hash FROM blob_intents WHERE volume=$1 AND state='writing' AND ($2::bytea IS NULL OR hash>$2) ORDER BY hash LIMIT 64", &[&self.name,&after]).await?;
        let mut last = None;
        for row in rows {
            let hash: Id = row
                .get::<_, Vec<u8>>(0)
                .try_into()
                .ok()
                .context("invalid recovery hash")?;
            last = Some(hash);
            let mutation = self.volume.background_mutation(hash).await;
            let Some(intent) = self.repo.blob_intent(hash).await? else {
                continue;
            };
            if intent.available || intent.volume != self.name {
                continue;
            }
            ensure!(
                intent.size <= self.max_blob_bytes,
                "pending blob exceeds configured recovery limit; credits retained"
            );
            match mutation
                .recover(intent.key.generation, intent.size, self.max_blob_bytes)
                .await
            {
                Ok(receipt) => {
                    self.repo.finish_blob(&self.name, &receipt).await?;
                }
                Err(_) => {
                    // A cleanup error preserves the reserved charge. Never refund
                    // solely because read/verification failed or a deadline passed.
                    let cleanup = mutation.cleanup(intent.key.generation).await?;
                    self.repo.abort_blob(&self.name, &cleanup).await?;
                }
            }
        }
        Ok(last)
    }
    pub(crate) async fn snapshot(&self, hash: Id, actor: Id) -> Result<(BlobIntent, Vec<u8>)> {
        let intent = self
            .repo
            .blob_intent(hash)
            .await?
            .context("blob unavailable")?;
        ensure!(
            intent.available && intent.owner == actor && intent.volume == self.name,
            "original unavailable to publisher"
        );
        let bytes = self.read_observed(&intent).await?;
        Ok((intent, bytes))
    }
    pub async fn public_original(&self, hash: Id) -> Result<(Vec<u8>, String)> {
        self.public_original_optional(hash)
            .await?
            .context("original is not publicly retained")
    }
    pub async fn public_original_optional(&self, hash: Id) -> Result<Option<(Vec<u8>, String)>> {
        let Some((intent, mime)) = self.public_metadata(hash).await? else {
            return Ok(None);
        };
        Ok(Some((self.read_observed(&intent).await?, mime)))
    }
    async fn public_metadata(&self, hash: Id) -> Result<Option<(BlobIntent, String)>> {
        use crate::read_diagnostics::{Phase, during, phase};
        let row = {
            let client = during(Phase::PoolCheckout, self.repo.pool.get()).await?;
            during(Phase::DbRoundtrip, client.query_opt("SELECT b.owner,p.mime,b.generation FROM blob_intents b JOIN publication_blobs p ON p.hash=b.hash AND p.generation=b.generation JOIN events e ON e.id=p.event_id WHERE b.hash=$1 AND b.volume=$2 AND b.state='available' AND e.state='hosted' AND NOT EXISTS(SELECT 1 FROM publication_blobs badp JOIN quarantined_blobs q ON q.hash=badp.hash AND q.generation=badp.generation WHERE badp.event_id=e.id) AND NOT EXISTS(SELECT 1 FROM backend_outbox o WHERE o.event_id=e.id AND o.completed_at IS NULL) LIMIT 1", &[&hash.as_slice(),&self.name])).await?
        };
        let Some(row) = row else { return Ok(None) };
        let owner: Vec<u8> = row.get(0);
        let generation: Vec<u8> = row.get(2);
        let intent = during(Phase::Uninstrumented, self.repo.blob_intent(hash))
            .await?
            .ok_or(BlobUnavailable)?;
        let _decode = phase(Phase::DecodeResponse);
        ensure!(
            intent.available
                && intent.owner.as_slice() == owner
                && intent.volume == self.name
                && intent.key.generation.as_slice() == generation,
            BlobUnavailable
        );
        Ok(Some((intent, row.get(1))))
    }
    pub async fn download_public_optional(
        &self,
        hash: Id,
    ) -> Result<Option<(crate::download_budget::Snapshot, String)>> {
        let Some((intent, mime)) = self.public_metadata(hash).await? else {
            return Ok(None);
        };
        Ok(Some((self.download_observed(&intent).await?, mime)))
    }
    pub async fn download(&self, hash: Id, actor: Id) -> Result<crate::download_budget::Snapshot> {
        let intent = self.repo.blob_intent(hash).await?.ok_or(BlobUnavailable)?;
        ensure!(
            intent.available && intent.owner == actor && intent.volume == self.name,
            BlobUnavailable
        );
        self.download_observed(&intent).await
    }
    async fn download_observed(
        &self,
        intent: &BlobIntent,
    ) -> Result<crate::download_budget::Snapshot> {
        let lease = self.downloads.acquire(intent.size)?;
        let (bytes, lease) = self.read_observed_with(intent, lease).await?;
        Ok(crate::download_budget::Snapshot::verified(bytes, lease))
    }
    pub async fn get(&self, hash: Id, actor: Id) -> Result<Vec<u8>> {
        let intent = self.repo.blob_intent(hash).await?.ok_or(BlobUnavailable)?;
        if !intent.available || intent.owner != actor || intent.volume != self.name {
            return Err(BlobUnavailable.into());
        }
        self.read_observed(&intent).await
    }
    async fn read_observed(&self, intent: &BlobIntent) -> Result<Vec<u8>> {
        Ok(self.read_observed_with(intent, ()).await?.0)
    }
    async fn read_observed_with<T: Send + 'static>(
        &self,
        intent: &BlobIntent,
        lease: T,
    ) -> Result<(Vec<u8>, T)> {
        self.read_observed_class(intent, lease, false).await
    }
    async fn read_observed_class<T: Send + 'static>(
        &self,
        intent: &BlobIntent,
        lease: T,
        background: bool,
    ) -> Result<(Vec<u8>, T)> {
        // Inclusive wait for the owned operation: storage admission, metadata
        // rechecks and filesystem verification. Its child has no phase context.
        let _storage = crate::read_diagnostics::phase(crate::read_diagnostics::Phase::StorageWait);
        let slot = self.read_slots.clone().acquire_owned().await?;
        let volume = self.volume.clone();
        let repo = self.repo.clone();
        let intent = intent.clone();
        let limit = self.max_blob_bytes;
        let fault = self.public_fault.clone();
        crate::node::spawn_owned_scope(async move {
            let _slot = slot;
            let observation = if background {
                volume.background_observation(intent.key.hash).await
            } else {
                volume.observation(intent.key.hash).await
            };
            let current = repo
                .blob_intent(intent.key.hash)
                .await?
                .ok_or(BlobUnavailable)?;
            ensure!(
                current.key == intent.key
                    && current.size == intent.size
                    && current.owner == intent.owner
                    && current.volume == intent.volume
                    && current.available,
                BlobUnavailable
            );
            ensure!(
                !repo.blob_reclaiming(&intent.key).await?,
                "blob reclamation pending"
            );
            ensure!(
                !repo.blob_quarantined(&intent.key).await?,
                "blob quarantined; verified repair required"
            );
            match observation
                .read_verified(&intent.key, intent.size, limit)
                .await
            {
                Ok(bytes) => Ok((bytes, lease)),
                Err(error) => {
                    fault.send_replace(true);
                    if !repo.blob_reclaiming(&intent.key).await? {
                        repo.begin_public_withdrawal(&intent.key, volume.identity(), true)
                            .await?;
                        let withdrawal = observation.withdraw_public(&intent.key).await?;
                        repo.quarantine_withdrawn(
                            &intent.key,
                            "read_verification_failed",
                            &withdrawal,
                        )
                        .await?;
                    }
                    Err(error.context(ReadVerificationFailed))
                }
            }
        })
        .await?
    }

    /// Disconnects do not interrupt an authorized generation's cleanup/refund.
    pub async fn delete_staged(
        self: &Arc<Self>,
        authority: crate::staged_deletion::DeleteAuthority,
        hash: Id,
    ) -> Result<()> {
        let slot = self.slots.clone().try_acquire_owned()?;
        let this = self.clone();
        crate::node::spawn_owned_scope(async move {
            let _slot = slot;
            let mutation = this.volume.mutation(hash).await;
            let key = this
                .repo
                .claim_staged_deletion(&authority, hash, this.owner, &this.name)
                .await?;
            ensure!(
                this.repo
                    .claim_library_reclaim(&key, this.owner, &this.name)
                    .await?,
                "staged deletion claim lost"
            );
            let receipt = mutation.cleanup(key.generation).await?;
            ensure!(
                this.repo
                    .finish_library_reclaim(this.owner, &this.name, &receipt)
                    .await?,
                "staged deletion receipt not settled"
            );
            Ok::<_, anyhow::Error>(())
        })
        .await?
    }
    /// The owned task retains admission/hash ownership through cleanup and refund.
    pub async fn reclaim_library(self: &Arc<Self>, key: crate::storage::BlobKey) -> Result<bool> {
        let this = self.clone();
        let mut waiting = crate::perf::Trace::new("reclaim_queue");
        let mutation = self.volume.background_mutation(key.hash).await;
        waiting.mark("storage_slot");
        drop(waiting);
        crate::node::spawn_owned_scope(async move {
            let mut trace = crate::perf::Trace::new("reclaim");
            trace.mark("mutation");
            if !this
                .repo
                .claim_library_reclaim(&key, this.owner, &this.name)
                .await?
            {
                return Ok(false);
            }
            trace.mark("claim");
            let receipt = mutation.cleanup(key.generation).await?;
            trace.mark("file_cleanup");
            let result = this
                .repo
                .finish_library_reclaim(this.owner, &this.name, &receipt)
                .await;
            trace.mark("refund");
            result
        })
        .await?
    }
    /// Age of the oldest currently due cleanup/retry candidate for this store.
    /// Null means no candidate is currently due; a candidate is not permission to
    /// reclaim, and all grace, authority and receipt checks still run at claim.
    pub async fn oldest_cleanup_due_seconds(&self) -> Result<Option<f64>> {
        Ok(self.repo.pool.get().await?.query_one(
            "SELECT extract(epoch FROM statement_timestamp()-min(due_at))::double precision FROM ((SELECT q.due_at FROM blob_gc_queue q JOIN blob_intents b ON b.hash=q.hash AND b.generation=q.generation WHERE b.owner=$1 AND b.volume=$2 AND q.due_at<=statement_timestamp() ORDER BY q.due_at,q.hash LIMIT 1) UNION ALL (SELECT r.retry_after AS due_at FROM blob_reclaims r JOIN blob_intents b ON b.hash=r.hash AND b.generation=r.generation WHERE b.owner=$1 AND b.volume=$2 AND r.retry_after<=statement_timestamp() ORDER BY r.retry_after,r.claimed_at,r.hash LIMIT 1)) due",
            &[&self.owner.as_slice(), &self.name],
        ).await?.get(0))
    }

    /// Process bounded pages; claimed work is resumed even when new GC is disabled.
    pub async fn reclaim_objects_page(
        self: &Arc<Self>,
        library_candidates: bool,
        publication_candidates: bool,
        staging_candidates: bool,
    ) -> Result<usize> {
        // Sample once per existing bounded collector page, not per object or
        // on a separate polling loop. Allocated-but-unused lane credits remain
        // genuine quota headroom; only used and in-flight bytes consume it.
        let capacity = self.repo.pool.get().await?.query_one(
            "SELECT b.limit_bytes,GREATEST(b.limit_bytes-COALESCE(sum(g.used+g.reserved),0),0)::bigint FROM capacity_budgets b LEFT JOIN capacity_grants g ON g.budget_key=b.key WHERE b.key=$1 GROUP BY b.limit_bytes",
            &[&self.budgets[0]],
        ).await?;
        self.volume
            .update_cleanup_headroom((
                capacity.get::<_, i64>(1) as u64,
                capacity.get::<_, i64>(0) as u64,
            ))
            .await?;
        if staging_candidates {
            self.repo
                .enqueue_expired_staging(self.owner, &self.name)
                .await?;
        }
        let mut rows=self.repo.pool.get().await?.query("SELECT r.hash,r.generation FROM blob_reclaims r JOIN blob_intents b ON b.hash=r.hash AND b.generation=r.generation WHERE b.owner=$1 AND b.volume=$2 AND r.retry_after<=statement_timestamp() ORDER BY r.retry_after,r.claimed_at,r.hash LIMIT 32", &[&self.owner.as_slice(),&self.name]).await?;
        if rows.len() < 64 && (library_candidates || publication_candidates || staging_candidates) {
            let more=self.repo.pool.get().await?.query("SELECT q.hash,q.generation FROM blob_gc_queue q JOIN blob_intents b ON b.hash=q.hash AND b.generation=q.generation WHERE q.due_at<=statement_timestamp() AND b.owner=$1 AND b.volume=$2 AND NOT EXISTS(SELECT 1 FROM blob_reclaims r WHERE r.hash=q.hash AND r.generation=q.generation) AND (($3 AND (q.source_mask&1)<>0) OR ($4 AND (q.source_mask&2)<>0) OR ($5 AND (q.source_mask&4)<>0)) ORDER BY q.due_at,q.hash LIMIT $6", &[&self.owner.as_slice(),&self.name,&library_candidates,&publication_candidates,&staging_candidates,&((64-rows.len()) as i64)]).await?;
            rows.extend(more);
        }
        let n = rows.len();
        let mut jobs = tokio::task::JoinSet::new();
        for row in rows {
            let key = crate::storage::BlobKey {
                hash: row
                    .get::<_, Vec<u8>>(0)
                    .try_into()
                    .ok()
                    .context("reclaim hash")?,
                generation: row
                    .get::<_, Vec<u8>>(1)
                    .try_into()
                    .ok()
                    .context("reclaim generation")?,
            };
            let this = self.clone();
            jobs.spawn(crate::node::owned_future_scope(async move {
                if let Err(error)=this.reclaim_library(key.clone()).await {
                    tracing::warn!(error=%error,"retired object cleanup deferred; charges retained");
                    this.repo.pool.get().await?.execute("UPDATE blob_reclaims SET retry_after=clock_timestamp()+interval '5 seconds',failures=LEAST(failures+1,1000000) WHERE hash=$1 AND generation=$2",&[&key.hash.as_slice(),&key.generation.as_slice()]).await?;
                }
                Ok::<_,anyhow::Error>(())
            }));
            if jobs.len() >= self.reclamation_slots {
                jobs.join_next().await.unwrap()??;
            }
        }
        while let Some(result) = jobs.join_next().await {
            result??;
        }
        Ok(n)
    }
}

#[derive(serde::Serialize)]
pub struct AuditResult {
    pub checked: usize,
    pub quarantined: usize,
    pub skipped: usize,
    pub checked_bytes: u64,
    pub wrapped: bool,
    pub public_export_pending: bool,
}
impl UploadStore {
    pub async fn audit_page(self: &Arc<Self>, limit: usize) -> Result<AuditResult> {
        ensure!((1..=64).contains(&limit), "audit page bounds");
        let this = self.clone();
        crate::node::spawn_owned_scope(async move {
            let _audit=this.audit_lock.lock().await;
            let c=this.repo.pool.get().await?;
            c.execute("INSERT INTO storage_audit_cursors(owner,volume) VALUES($1,$2) ON CONFLICT DO NOTHING",&[&this.owner.as_slice(),&this.name]).await?;
            let cursor:Option<Vec<u8>>=c.query_one("SELECT last_hash FROM storage_audit_cursors WHERE owner=$1 AND volume=$2",&[&this.owner.as_slice(),&this.name]).await?.get(0);
            let rows=c.query("SELECT hash FROM blob_intents WHERE owner=$1 AND volume=$2 AND state='available' AND ($3::bytea IS NULL OR hash>$3) ORDER BY hash LIMIT $4",&[&this.owner.as_slice(),&this.name,&cursor,&(limit as i64)]).await?;
            drop(c);
            let mut result=AuditResult{checked:0,quarantined:0,skipped:0,checked_bytes:0,wrapped:rows.is_empty(),public_export_pending:this.repo.public_export_pending(this.owner,&this.name,this.volume.identity()).await?};
            if rows.is_empty(){this.repo.pool.get().await?.execute("UPDATE storage_audit_cursors SET last_hash=NULL,laps=laps+1,updated_at=clock_timestamp() WHERE owner=$1 AND volume=$2",&[&this.owner.as_slice(),&this.name]).await?;return Ok::<_,anyhow::Error>(result);}
            let mut last=None;
            for row in rows {
                let hash:Vec<u8>=row.get(0);let key:Id=hash.clone().try_into().unwrap();
                if let Some(intent)=this.repo.blob_intent(key).await? {
                    if result.checked>0 && result.checked_bytes+intent.size>64*1024*1024 {break;}
                    if this.repo.blob_quarantined(&intent.key).await? {result.skipped+=1;}else{
                        result.checked+=1;result.checked_bytes+=intent.size;
                        if this.read_observed_class(&intent, (), true).await.is_err(){
                            // DB failure must not be treated as a durable health decision.
                            if this.repo.blob_quarantined(&intent.key).await? {result.quarantined+=1;}else{anyhow::bail!("audit read failed without a durable quarantine");}
                        }
                    }
                }else{result.skipped+=1;}
                last=Some(hash);
            }
            let c=this.repo.pool.get().await?;
            c.execute("UPDATE storage_audit_cursors SET last_hash=$3,updated_at=clock_timestamp() WHERE owner=$1 AND volume=$2",&[&this.owner.as_slice(),&this.name,&last]).await?;
            drop(c);
            result.public_export_pending=this.repo.public_export_pending(this.owner,&this.name,this.volume.identity()).await?;
            Ok(result)
        }).await?
    }
    pub async fn repair_verified(
        self: &Arc<Self>,
        intent: BlobIntent,
        bytes: Vec<u8>,
        peer: crate::peers::Peer,
    ) -> Result<bool> {
        ensure!(
            bytes.len() as u64 == intent.size
                && <[u8; 32]>::from(Sha256::digest(&bytes)) == intent.key.hash,
            "repair bytes do not match trusted receipt"
        );
        let this = self.clone();
        let mutation = self.volume.background_mutation(intent.key.hash).await;
        crate::node::spawn_owned_scope(async move {
            let current = this
                .repo
                .blob_intent(intent.key.hash)
                .await?
                .context("repair target unavailable")?;
            ensure!(
                current.key == intent.key
                    && current.owner == this.owner
                    && current.volume == this.name
                    && current.available
                    && current.size == intent.size,
                "repair target changed"
            );
            let approved = this.repo.approved_peer(peer.key).await?;
            ensure!(
                approved.version == peer.version && approved.endpoint == peer.endpoint,
                "repair peer changed"
            );
            ensure!(
                this.repo.blob_quarantined(&intent.key).await?,
                "repair requires quarantined generation"
            );
            ensure!(
                !this.repo.public_export_held(&intent.key).await?,
                "external serving readers must drain before repair"
            );
            let receipt = match mutation
                .recover(intent.key.generation, intent.size, this.max_blob_bytes)
                .await
            {
                Ok(receipt) => receipt,
                Err(_) => {
                    mutation.cleanup(intent.key.generation).await?;
                    mutation
                        .put_verified(intent.key.generation, &bytes, this.max_blob_bytes)
                        .await?
                }
            };
            this.repo
                .finish_verified_repair(this.owner, &this.name, &peer, &receipt)
                .await
        })
        .await?
    }
}

impl UploadStore {
    /// Withdrawal is owned through directory fsync and its durable DB marker.
    /// The hold remains until a later exact-set reader drain succeeds.
    pub async fn withdraw_public_original(
        self: &Arc<Self>,
        key: crate::storage::BlobKey,
    ) -> Result<bool> {
        let this = self.clone();
        let mutation = self.volume.background_mutation(key.hash).await;
        crate::node::spawn_owned_scope(async move {
            let intent = this
                .repo
                .blob_intent(key.hash)
                .await?
                .context("withdrawal original absent")?;
            ensure!(
                intent.key == key && intent.owner == this.owner && intent.volume == this.name,
                "withdrawal generation/owner mismatch"
            );
            if !this.repo.public_export_held(&key).await? {
                return Ok(false);
            }
            let receipt = mutation.withdraw_public(key.generation).await?;
            this.repo
                .mark_public_withdrawal(this.owner, &this.name, &receipt)
                .await
        })
        .await?
    }
    /// Snapshot committed tokens before the drain, then release only that exact
    /// still-draining set. Late withdrawals/re-exports cannot inherit its proof.
    pub async fn drain_public_originals(
        self: &Arc<Self>,
        serving: Arc<crate::serving::ManagedServing>,
    ) -> Result<usize> {
        ensure!(
            serving.owner() == self.owner && serving.public_path() == self.volume.public_path(),
            "serving family is bound to another owner/public tree"
        );
        let guard = self.export_drain_lock.clone().lock_owned().await;
        let this = self.clone();
        crate::node::spawn_owned_scope(async move {
            let _guard = guard;
            let pending = this
                .repo
                .pending_public_drains(this.owner, &this.name, this.volume.identity())
                .await?;
            if pending.is_empty() {
                return Ok(0);
            }
            let receipt = serving.drain().await?;
            this.repo
                .release_public_drains(
                    this.owner,
                    &this.name,
                    this.volume.identity(),
                    pending,
                    receipt,
                )
                .await
        })
        .await?
    }
}

impl UploadStore {
    // Positive projection is owned by bounded reconciliation.
    async fn export_public_original(
        self: &Arc<Self>,
        key: crate::storage::BlobKey,
        mime: String,
    ) -> Result<()> {
        let this = self.clone();
        let mutation = self.volume.background_mutation(key.hash).await;
        crate::node::spawn_owned_scope(async move {
            let intent = this
                .repo
                .blob_intent(key.hash)
                .await?
                .context("export original absent")?;
            ensure!(
                intent.key == key
                    && intent.available
                    && intent.owner == this.owner
                    && intent.volume == this.name,
                "export original generation/owner changed"
            );
            let receipt = match mutation
                .recover(key.generation, intent.size, this.max_blob_bytes)
                .await
            {
                Ok(receipt) => receipt,
                Err(error) => {
                    this.public_fault.send_replace(true);
                    this.repo
                        .begin_public_withdrawal(&key, this.volume.identity(), false)
                        .await?;
                    let withdrawal = mutation.withdraw_public(key.generation).await?;
                    this.repo
                        .quarantine_withdrawn(&key, "read_verification_failed", &withdrawal)
                        .await?;
                    return Err(error);
                }
            };

            this.repo
                .reserve_public_export(&key, this.owner, &this.name, &mime, this.volume.identity())
                .await?;
            this.repo
                .activate_public_export(this.volume.identity(), &receipt, &mime, &mutation)
                .await
        })
        .await?
    }
}

impl UploadStore {
    pub async fn configure_public_export_role(&self) -> Result<()> {
        self.repo
            .configure_public_export_role(self.owner, &self.name, self.volume.identity())
            .await
    }
    /// One bounded maintenance quantum; no busy full-catalog scans. Node-level
    /// activation still awaits quarantine and startup/read-policy integration.
    pub async fn reconcile_public_exports(
        self: &Arc<Self>,
    ) -> Result<crate::export_queue::ExportStep> {
        let guard = self.export_work_lock.clone().lock_owned().await;
        let this = self.clone();
        crate::node::spawn_owned_scope(async move {
            let _guard = guard;
            this.reconcile_public_exports_owned().await
        })
        .await?
    }
    async fn reconcile_public_exports_owned(
        self: &Arc<Self>,
    ) -> Result<crate::export_queue::ExportStep> {
        let mut report = crate::export_queue::ExportStep {
            scanned: self
                .repo
                .backfill_public_exports(self.owner, &self.name, self.volume.identity())
                .await?,
            health_scanned: self
                .repo
                .fanout_public_health(self.owner, &self.name, self.volume.identity())
                .await?,
            ..Default::default()
        };
        for work in self
            .repo
            .public_export_work(self.owner, &self.name, self.volume.identity())
            .await?
        {
            let result = async {
                if let Some(mime) = self.repo.public_export_mime(&work.key).await? {
                    self.export_public_original(work.key.clone(), mime).await?;
                    report.exported += 1;
                } else if self.withdraw_public_original(work.key.clone()).await? {
                    report.withdrawn += 1;
                }
                Ok::<_, anyhow::Error>(())
            }
            .await;
            match result {
                Ok(()) => {
                    self.repo
                        .finish_public_export_work(&work, self.volume.identity())
                        .await?
                }
                Err(error) => {
                    report.retried += 1;
                    self.repo
                        .retry_public_export_work(&work, self.volume.identity(), &error.to_string())
                        .await?;
                }
            }
        }
        Ok(report)
    }
}
