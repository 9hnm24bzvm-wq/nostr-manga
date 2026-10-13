//! One-off admitted-account review. No path mutates publisher permissions.
use crate::{
    accounts::AccountPermit,
    auth::{self, Event},
    db::{Id, Repository},
    manifest,
    publication::Publications,
    upload::UploadStore,
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, serde::Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReviewLimits {
    pub account_pending: i64,
    pub instance_pending: i64,
    pub account_bytes: i64,
    pub instance_bytes: i64,
    pub submission_bytes: i64,
    pub objects: usize,
    pub lifetime_seconds: i64,
    pub records: i64,
}
impl Default for ReviewLimits {
    fn default() -> Self {
        Self {
            account_pending: 4,
            instance_pending: 128,
            account_bytes: 512 * 1024 * 1024,
            instance_bytes: 8 * 1024 * 1024 * 1024,
            submission_bytes: 256 * 1024 * 1024,
            objects: 1024,
            lifetime_seconds: 7 * 86400,
            records: 10000,
        }
    }
}
impl ReviewLimits {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=1024).contains(&self.account_pending)
                && (self.account_pending..=1_000_000).contains(&self.instance_pending)
                && (1..=4096).contains(&self.objects)
                && (1..=30 * 86400).contains(&self.lifetime_seconds)
                && self.submission_bytes > 0
                && self.submission_bytes <= self.account_bytes
                && self.account_bytes <= self.instance_bytes
                && self.instance_bytes <= 1024i64 * 1024 * 1024 * 1024
                && (1..=1_000_000_000).contains(&self.records),
            "review limits"
        );
        Ok(())
    }
}
#[derive(Debug, thiserror::Error)]
pub enum ReviewDenied {
    #[error("submission owner or account permission changed")]
    Authority,
    #[error("submission is closed, expired or changed")]
    Closed,
    #[error("review admission budget exhausted")]
    Budget,
    #[error("object was not declared or granted")]
    Object,
}
#[derive(Clone)]
pub(crate) struct ReviewPermit {
    pub(crate) submission: Id,
    pub(crate) author: Id,
    pub(crate) account_version: i64,
    pub(crate) version: i64,
    pub(crate) moderator: Option<Id>,
}
#[derive(Clone)]
pub struct Reviews {
    pub(crate) repo: Repository,
    pub(crate) storage: Arc<UploadStore>,
    pub(crate) publications: Arc<Publications>,
    pub(crate) limits: ReviewLimits,
}
impl Reviews {
    pub fn new(
        repo: Repository,
        storage: Arc<UploadStore>,
        publications: Arc<Publications>,
        limits: ReviewLimits,
    ) -> Result<Self> {
        limits.validate()?;
        Ok(Self {
            repo,
            storage,
            publications,
            limits,
        })
    }
}
fn event_objects(
    event: &Event,
    bytes: &[u8],
    storage: &UploadStore,
    limits: &ReviewLimits,
) -> Result<BTreeMap<Id, i64>> {
    ensure!(
        serde_json::to_vec(event)?.len() <= 16384 && bytes.len() <= 256 * 1024,
        "review manifest/event budget"
    );
    ensure!(
        event.created_at <= auth::now().saturating_add(60),
        "review event future timestamp"
    );
    let hash = crate::publication::pointer(event)?;
    ensure!(
        auth::hash(bytes) == hex::encode(hash) && bytes.len() as u64 <= storage.max_blob_bytes,
        "review manifest identity/size"
    );
    let mut objects = BTreeMap::from([(hash, i64::try_from(bytes.len())?)]);
    let originals = if event.kind == 1001 {
        manifest::chapter(bytes).map_err(anyhow::Error::msg)?.pages
    } else {
        crate::series_metadata::parse(bytes)
            .map_err(anyhow::Error::msg)?
            .cover
            .into_iter()
            .collect()
    };
    for page in manifest::unique_pages(&originals).map_err(anyhow::Error::msg)? {
        ensure!(
            page.size > 0
                && page.size <= storage.max_blob_bytes
                && storage.formats.contains(&page.mime),
            "review original size/format"
        );
        let hash: Id = hex::decode(&page.hash)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("page hash"))?;
        ensure!(
            objects.insert(hash, i64::try_from(page.size)?).is_none(),
            "manifest/original role collision"
        );
    }
    ensure!(objects.len() <= limits.objects, "review object budget");
    let total = objects
        .values()
        .try_fold(0i64, |sum, size| sum.checked_add(*size))
        .context("review size overflow")?;
    ensure!(
        total <= limits.submission_bytes,
        "review submission byte budget"
    );
    Ok(objects)
}
pub(crate) async fn check_review_tx(
    tx: &deadpool_postgres::Transaction<'_>,
    permit: &ReviewPermit,
    exclusive: bool,
) -> Result<()> {
    let row = tx
        .query_opt(
            "SELECT allowed,version FROM account_permissions WHERE publisher=$1 FOR SHARE",
            &[&permit.author.as_slice()],
        )
        .await?
        .ok_or(ReviewDenied::Authority)?;
    ensure!(
        row.get::<_, bool>(0) && row.get::<_, i64>(1) == permit.account_version,
        ReviewDenied::Authority
    );
    let sql = if exclusive {
        "SELECT author,account_version,version,state,expires_at>clock_timestamp() FROM review_submissions WHERE event_id=$1 FOR UPDATE"
    } else {
        "SELECT author,account_version,version,state,expires_at>clock_timestamp() FROM review_submissions WHERE event_id=$1 FOR SHARE"
    };
    let row = tx
        .query_opt(sql, &[&permit.submission.as_slice()])
        .await?
        .ok_or(ReviewDenied::Closed)?;
    ensure!(
        row.get::<_, Vec<u8>>(0) == permit.author
            && row.get::<_, i64>(1) == permit.account_version
            && row.get::<_, i64>(2) == permit.version
            && row.get::<_, String>(3) == "pending"
            && row.get::<_, bool>(4),
        ReviewDenied::Closed
    );
    Ok(())
}

fn key(value: &str) -> Result<Id> {
    ensure!(auth::valid_hash(value), "invalid identifier");
    Ok(hex::decode(value)?.try_into().unwrap())
}
impl Reviews {
    pub async fn declare(
        &self,
        account: &AccountPermit,
        event: Event,
        manifest_bytes: &[u8],
    ) -> Result<bool> {
        ensure!(
            account.account_peer.is_none()
                && account.sync_lease.is_none()
                && event.pubkey == hex::encode(account.publisher),
            ReviewDenied::Authority
        );
        let objects = event_objects(&event, manifest_bytes, &self.storage, &self.limits)?;
        let submission = key(&event.id)?;
        let manifest = crate::publication::pointer(&event)?;
        let total = objects.values().sum::<i64>();
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        // Intake is low-volume control work; the counter avoids catalogue scans.
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended('manga:review-intake:v1',0))",
            &[],
        )
        .await?;
        lock_event(&tx, submission).await?;
        check_account_epoch(&tx, account.publisher, account.version).await?;
        let old=tx.query_opt("SELECT author,account_version,version,state,expires_at>clock_timestamp() FROM review_submissions WHERE event_id=$1 FOR UPDATE",&[&submission.as_slice()]).await?;
        let inserted = old.is_none();
        if let Some(old) = old {
            ensure!(
                old.get::<_, Vec<u8>>(0) == account.publisher
                    && old.get::<_, i64>(1) == account.version,
                ReviewDenied::Authority
            );
            ensure!(
                old.get::<_, String>(3) == "pending" && old.get::<_, bool>(4),
                ReviewDenied::Closed
            );
        } else {
            let own=tx.query_one("SELECT COUNT(*),COALESCE(SUM(declared_bytes),0)::bigint FROM review_submissions WHERE author=$1 AND state='pending' AND expires_at>clock_timestamp()",&[&account.publisher.as_slice()]).await?;
            let all=tx.query_one("SELECT COUNT(*),COALESCE(SUM(declared_bytes),0)::bigint FROM review_submissions WHERE state='pending' AND expires_at>clock_timestamp()",&[]).await?;
            ensure!(
                own.get::<_, i64>(0) < self.limits.account_pending
                    && all.get::<_, i64>(0) < self.limits.instance_pending
                    && own
                        .get::<_, i64>(1)
                        .checked_add(total)
                        .is_some_and(|n| n <= self.limits.account_bytes)
                    && all
                        .get::<_, i64>(1)
                        .checked_add(total)
                        .is_some_and(|n| n <= self.limits.instance_bytes),
                ReviewDenied::Budget
            );
            let counter=tx.execute("UPDATE review_counters SET registrations=registrations+1 WHERE singleton AND registrations<$1",&[&self.limits.records]).await?;
            ensure!(counter == 1, ReviewDenied::Budget);
            tx.execute("INSERT INTO review_submissions(event_id,author,account_version,raw_event,manifest,declared_bytes,expires_at) VALUES($1,$2,$3,$4,$5,$6,clock_timestamp()+($7::bigint*interval '1 second'))",&[&submission.as_slice(),&account.publisher.as_slice(),&account.version,&serde_json::to_value(&event)?,&manifest.as_slice(),&total,&self.limits.lifetime_seconds]).await?;
            let hashes: Vec<Vec<u8>> = objects.keys().map(|h| h.to_vec()).collect();
            let sizes: Vec<i64> = objects.values().copied().collect();
            tx.execute("INSERT INTO review_objects(submission,hash,size) SELECT $1,h,s FROM unnest($2::bytea[],$3::bigint[]) AS items(h,s)",&[&submission.as_slice(),&hashes,&sizes]).await?;
        }
        tx.commit().await?;
        drop(client);
        // A failed physical stage leaves a bounded, visible pending declaration;
        // retrying the same signed event neither extends expiry nor reserves twice.
        self.upload(account, submission, manifest, manifest_bytes)
            .await?;
        Ok(inserted)
    }
    async fn permit(&self, account: &AccountPermit, submission: Id) -> Result<ReviewPermit> {
        ensure!(
            account.account_peer.is_none() && account.sync_lease.is_none(),
            ReviewDenied::Authority
        );
        let row = self
            .repo
            .pool
            .get()
            .await?
            .query_opt(
                "SELECT author,account_version,version FROM review_submissions WHERE event_id=$1 AND author=$2",
                &[&submission.as_slice(),&account.publisher.as_slice()],
            )
            .await?
            .ok_or(ReviewDenied::Object)?;
        ensure!(
            row.get::<_, Vec<u8>>(0) == account.publisher
                && row.get::<_, i64>(1) == account.version,
            ReviewDenied::Authority
        );
        Ok(ReviewPermit {
            submission,
            author: account.publisher,
            account_version: account.version,
            version: row.get(2),
            moderator: None,
        })
    }
    pub async fn expected_size(
        &self,
        account: &AccountPermit,
        submission: Id,
        hash: Id,
    ) -> Result<usize> {
        let permit = self.permit(account, submission).await?;
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        check_review_tx(&tx, &permit, false).await?;
        let row = tx
            .query_opt(
                "SELECT size FROM review_objects WHERE submission=$1 AND hash=$2",
                &[&submission.as_slice(), &hash.as_slice()],
            )
            .await?
            .ok_or(ReviewDenied::Object)?;
        let size = usize::try_from(row.get::<_, i64>(0))?;
        tx.commit().await?;
        Ok(size)
    }
    pub async fn upload(
        &self,
        account: &AccountPermit,
        submission: Id,
        hash: Id,
        bytes: &[u8],
    ) -> Result<()> {
        ensure!(
            bytes.len() == self.expected_size(account, submission, hash).await?,
            "declared object size mismatch"
        );
        let permit = self.permit(account, submission).await?;
        let result = self.storage.put_detailed(hash, bytes).await?;
        let blob = result.intent;
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        check_review_tx(&tx, &permit, false).await?;
        let row = tx
            .query_opt(
                "SELECT generation,owner,state,size FROM blob_intents WHERE hash=$1 FOR SHARE",
                &[&hash.as_slice()],
            )
            .await?
            .ok_or(ReviewDenied::Object)?;
        ensure!(
            row.get::<_, Vec<u8>>(0) == blob.key.generation
                && row.get::<_, Vec<u8>>(1) == self.storage.owner()
                && row.get::<_, String>(2) == "available"
                && row.get::<_, i64>(3) == bytes.len() as i64,
            ReviewDenied::Object
        );
        ensure!(
            !tx.query_one(
                "SELECT EXISTS(SELECT 1 FROM blob_reclaims WHERE hash=$1)",
                &[&hash.as_slice()]
            )
            .await?
            .get::<_, bool>(0),
            ReviewDenied::Object
        );
        tx.execute("INSERT INTO review_blob_grants(submission,hash,generation) VALUES($1,$2,$3) ON CONFLICT(submission,hash) DO UPDATE SET generation=EXCLUDED.generation",&[&submission.as_slice(),&hash.as_slice(),&blob.key.generation.as_slice()]).await?;
        tx.commit().await?;
        Ok(())
    }
    async fn authorize_read(
        &self,
        account: &AccountPermit,
        submission: Id,
        hash: Id,
    ) -> Result<()> {
        let permit = self.permit(account, submission).await?;
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        check_review_tx(&tx, &permit, false).await?;
        check_review_grants_tx(&tx, &permit, &[hash.to_vec()]).await?;
        tx.commit().await?;
        drop(client);
        Ok(())
    }
    pub async fn read(&self, account: &AccountPermit, submission: Id, hash: Id) -> Result<Vec<u8>> {
        self.authorize_read(account, submission, hash).await?;
        self.storage.get(hash, self.storage.owner()).await
    }
    pub async fn download(
        &self,
        account: &AccountPermit,
        submission: Id,
        hash: Id,
    ) -> Result<crate::download_budget::Snapshot> {
        self.authorize_read(account, submission, hash).await?;
        self.storage.download(hash, self.storage.owner()).await
    }

    pub async fn approve(&self, submission: Id, moderator: Id) -> Result<bool> {
        let row=self.repo.pool.get().await?.query_opt("SELECT author,account_version,version,state,raw_event FROM review_submissions WHERE event_id=$1",&[&submission.as_slice()]).await?.ok_or(ReviewDenied::Closed)?;
        if row.get::<_, String>(3) == "approved" {
            return Ok(false);
        }
        ensure!(row.get::<_, String>(3) == "pending", ReviewDenied::Closed);
        let permit = ReviewPermit {
            submission,
            author: row
                .get::<_, Vec<u8>>(0)
                .try_into()
                .map_err(|_| anyhow::anyhow!("stored author"))?,
            account_version: row.get(1),
            version: row.get(2),
            moderator: Some(moderator),
        };
        let event: Event = serde_json::from_value(row.get(4))?;
        let result = async {
            let prepared = self.publications.prepare_for_review(&event, permit).await?;
            self.publications.commit(prepared).await
        }
        .await;
        match result {
            Ok(value) => Ok(value),
            Err(error) if error.is::<ReviewDenied>() => {
                let approved = self
                    .repo
                    .pool
                    .get()
                    .await?
                    .query_one(
                        "SELECT state='approved' FROM review_submissions WHERE event_id=$1",
                        &[&submission.as_slice()],
                    )
                    .await?
                    .get::<_, bool>(0);
                if approved { Ok(false) } else { Err(error) }
            }
            Err(error) => Err(error),
        }
    }
    pub async fn reject(&self, submission: Id, moderator: Id, reason: String) -> Result<()> {
        self.close(submission, Some(moderator), None, "rejected", reason)
            .await
    }
    pub async fn withdraw(&self, submission: Id, account: &AccountPermit) -> Result<()> {
        self.close(submission, None, Some(account), "withdrawn", String::new())
            .await
    }
    async fn close(
        &self,
        submission: Id,
        moderator: Option<Id>,
        account: Option<&AccountPermit>,
        state: &str,
        reason: String,
    ) -> Result<()> {
        ensure!(reason.len() <= 4096, "review reason budget");
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        lock_event(&tx, submission).await?;
        if let Some(account) = account {
            ensure!(
                account.account_peer.is_none() && account.sync_lease.is_none(),
                ReviewDenied::Authority
            );
            check_account_epoch(&tx, account.publisher, account.version).await?;
        }
        let row=tx.query_opt("SELECT author,state,expires_at>clock_timestamp() FROM review_submissions WHERE event_id=$1 FOR UPDATE",&[&submission.as_slice()]).await?.ok_or(ReviewDenied::Closed)?;
        if let Some(account) = account {
            ensure!(
                row.get::<_, Vec<u8>>(0) == account.publisher,
                ReviewDenied::Authority
            );
        }
        if row.get::<_, String>(1) == state {
            return Ok(());
        }
        ensure!(
            row.get::<_, String>(1) == "pending" && row.get::<_, bool>(2),
            ReviewDenied::Closed
        );
        let actor = moderator
            .or_else(|| account.map(|a| a.publisher))
            .map(|k| k.to_vec());
        tx.execute("UPDATE review_submissions SET state=$2,version=version+1,decided_at=clock_timestamp(),decided_by=$3,reason=$4 WHERE event_id=$1",&[&submission.as_slice(),&state,&actor,&reason]).await?;
        tx.execute("UPDATE blob_gc_queue q SET due_at=LEAST(q.due_at,clock_timestamp()) FROM review_blob_grants g WHERE g.submission=$1 AND q.hash=g.hash AND q.generation=g.generation", &[&submission.as_slice()]).await?;
        tx.execute(
            "DELETE FROM review_blob_grants WHERE submission=$1",
            &[&submission.as_slice()],
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
pub(crate) async fn check_review_grants_tx(
    tx: &deadpool_postgres::Transaction<'_>,
    permit: &ReviewPermit,
    hashes: &[Vec<u8>],
) -> Result<()> {
    let rows=tx.query("SELECT g.hash FROM review_blob_grants g JOIN blob_intents b ON b.hash=g.hash AND b.generation=g.generation WHERE g.submission=$1 AND g.hash=ANY($2) ORDER BY g.hash FOR SHARE OF g",&[&permit.submission.as_slice(),&hashes]).await?;
    ensure!(rows.len() == hashes.len(), ReviewDenied::Object);
    Ok(())
}
pub(crate) async fn complete_review_tx(
    tx: &deadpool_postgres::Transaction<'_>,
    permit: &ReviewPermit,
) -> Result<()> {
    let moderator = permit.moderator.context("operator decision required")?;
    let changed=tx.execute("UPDATE review_submissions SET state='approved',version=version+1,decided_at=clock_timestamp(),decided_by=$3 WHERE event_id=$1 AND version=$2 AND state='pending' AND expires_at>clock_timestamp()",&[&permit.submission.as_slice(),&permit.version,&moderator.as_slice()]).await?;
    ensure!(changed == 1, ReviewDenied::Closed);
    tx.execute(
        "DELETE FROM review_blob_grants WHERE submission=$1",
        &[&permit.submission.as_slice()],
    )
    .await?;
    Ok(())
}
impl Repository {
    pub async fn expire_reviews(&self) -> Result<u64> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let rows=tx.query("UPDATE review_submissions SET state='expired',version=version+1,decided_at=clock_timestamp() WHERE event_id IN (SELECT event_id FROM review_submissions WHERE state='pending' AND expires_at<=clock_timestamp() ORDER BY expires_at,event_id LIMIT 64 FOR UPDATE SKIP LOCKED) RETURNING event_id",&[]).await?;
        let ids: Vec<Vec<u8>> = rows.iter().map(|r| r.get(0)).collect();
        if !ids.is_empty() {
            tx.execute(
                "DELETE FROM review_blob_grants WHERE submission=ANY($1)",
                &[&ids],
            )
            .await?;
        }
        tx.commit().await?;
        Ok(ids.len() as u64)
    }
}

fn status_row(row: &tokio_postgres::Row) -> serde_json::Value {
    serde_json::json!({"id":hex::encode(row.get::<_,Vec<u8>>(0)),"author":hex::encode(row.get::<_,Vec<u8>>(1)),"state":row.get::<_,String>(2),"declared_bytes":row.get::<_,i64>(3),"created_at":row.get::<_,i64>(4),"expires_at":row.get::<_,i64>(5),"decided_at":row.get::<_,Option<i64>>(6),"decided_by":row.get::<_,Option<Vec<u8>>>(7).map(hex::encode),"reason":row.get::<_,String>(8),"review_window_elapsed":row.get::<_,bool>(9),"manifest":hex::encode(row.get::<_,Vec<u8>>(10))})
}
impl Repository {
    pub async fn review_status(
        &self,
        submission: Id,
        author: Option<Id>,
    ) -> Result<Option<serde_json::Value>> {
        let author = author.map(|a| a.to_vec());
        let client = self.pool.get().await?;
        let row=client.query_opt("SELECT event_id,author,state,declared_bytes,EXTRACT(EPOCH FROM created_at)::bigint,EXTRACT(EPOCH FROM expires_at)::bigint,EXTRACT(EPOCH FROM decided_at)::bigint,decided_by,reason,expires_at<=clock_timestamp(),manifest,raw_event FROM review_submissions WHERE event_id=$1 AND ($2::bytea IS NULL OR author=$2)",&[&submission.as_slice(),&author]).await?;
        let Some(row) = row else { return Ok(None) };
        let mut value = status_row(&row);
        value["event"] = row.get(11);
        let objects=client.query("SELECT o.hash,o.size,g.generation IS NOT NULL FROM review_objects o LEFT JOIN review_blob_grants g ON g.submission=o.submission AND g.hash=o.hash WHERE o.submission=$1 ORDER BY o.hash",&[&submission.as_slice()]).await?;
        value["objects"]=serde_json::json!(objects.into_iter().map(|r|serde_json::json!({"hash":hex::encode(r.get::<_,Vec<u8>>(0)),"size":r.get::<_,i64>(1),"staged_for_review":r.get::<_,bool>(2)})).collect::<Vec<_>>());
        Ok(Some(value))
    }
    pub async fn list_reviews(
        &self,
        author: Option<Id>,
        after: Option<Id>,
        limit: i64,
        pending_only: bool,
    ) -> Result<serde_json::Value> {
        ensure!((1..=64).contains(&limit), "review page limit");
        let author = author.map(|a| a.to_vec());
        let after = after.map(|a| a.to_vec());
        let rows=self.pool.get().await?.query("SELECT event_id,author,state,declared_bytes,EXTRACT(EPOCH FROM created_at)::bigint,EXTRACT(EPOCH FROM expires_at)::bigint,EXTRACT(EPOCH FROM decided_at)::bigint,decided_by,reason,expires_at<=clock_timestamp(),manifest FROM review_submissions WHERE ($1::bytea IS NULL OR author=$1) AND ($2::bytea IS NULL OR event_id>$2) AND (NOT $3 OR state='pending') ORDER BY event_id LIMIT $4",&[&author,&after,&pending_only,&(limit+1)]).await?;
        let more = rows.len() > limit as usize;
        let items: Vec<_> = rows.iter().take(limit as usize).map(status_row).collect();
        let next = if more {
            items.last().map(|v| v["id"].clone())
        } else {
            None
        };
        Ok(serde_json::json!({"items":items,"next_after":next,"pagination":"live_event_id_keyset"}))
    }
}

pub(crate) async fn lock_event(tx: &deadpool_postgres::Transaction<'_>, event: Id) -> Result<()> {
    tx.query_one("SELECT pg_advisory_xact_lock(hashtextextended('manga:review-verdict:'||encode($1::bytea,'hex'),0))",&[&event.as_slice()]).await?;
    Ok(())
}
pub(crate) async fn check_publication_verdict(
    tx: &deadpool_postgres::Transaction<'_>,
    event: Id,
) -> Result<()> {
    let rejected=tx.query_one("SELECT EXISTS(SELECT 1 FROM review_submissions WHERE event_id=$1 AND state='rejected')",&[&event.as_slice()]).await?.get::<_,bool>(0);
    ensure!(
        !rejected,
        crate::transfer_rejection::TransferRejection::policy(
            crate::transfer_rejection::RejectionReason::ReviewRejected
        )
    );
    Ok(())
}
async fn check_account_epoch(
    tx: &deadpool_postgres::Transaction<'_>,
    author: Id,
    version: i64,
) -> Result<()> {
    let row = tx
        .query_opt(
            "SELECT allowed,version FROM account_permissions WHERE publisher=$1 FOR SHARE",
            &[&author.as_slice()],
        )
        .await?
        .ok_or(ReviewDenied::Authority)?;
    ensure!(
        row.get::<_, bool>(0) && row.get::<_, i64>(1) == version,
        ReviewDenied::Authority
    );
    Ok(())
}
