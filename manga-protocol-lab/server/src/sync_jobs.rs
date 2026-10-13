//! Durable bounded synchronization partitions. Cancellation fences future commits;
//! content admitted before cancellation is retained rather than rolled back.
use crate::{
    auth,
    db::{Id, Lease, Repository, check_lease_tx},
    filter::Filter,
    inventory::{MAX_ITEMS, PartitionRequired, prefix_bounds},
    peers::{PeerSync, SyncRequest},
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc, time::Duration};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncSpec {
    #[serde(default = "crate::inventory::inventory_name")]
    pub inventory: String,
    pub peer: String,
    #[serde(default)]
    pub filter: String,
    #[serde(default)]
    pub fallbacks: Vec<String>,
}
#[derive(Debug, thiserror::Error)]
#[error("global pending-item budget full; retry later")]
pub struct QueueFull;
pub struct Work {
    pub lease: Lease,
    pub batch: Id,
    pub prefix: String,
    pub spec: SyncSpec,
    pub reconciled: bool,
}
fn id(s: &str) -> Result<Id> {
    ensure!(auth::valid_hash(s), "invalid identifier");
    Ok(hex::decode(s)?.try_into().unwrap())
}
async fn lock_work(tx: &deadpool_postgres::Transaction<'_>, lease: &Lease) -> Result<Id> {
    let batch: Vec<u8> = tx
        .query_one(
            "SELECT batch_id FROM sync_partitions WHERE job_id=$1",
            &[&lease.id.as_slice()],
        )
        .await?
        .get(0);
    let state: String = tx
        .query_one(
            "SELECT state FROM sync_batches WHERE id=$1 FOR UPDATE",
            &[&batch],
        )
        .await?
        .get(0);
    ensure!(
        matches!(state.as_str(), "queued" | "running"),
        "batch no longer active"
    );
    check_lease_tx(tx, lease).await?;
    batch.try_into().ok().context("invalid batch ID")
}
async fn insert_partition(
    tx: &tokio_postgres::Transaction<'_>,
    batch: Id,
    prefix: &str,
) -> Result<()> {
    let job: Id = rand::random();
    let key = format!("sync:{}:{prefix}", hex::encode(batch));
    tx.execute(
        "INSERT INTO jobs(id,task_key,payload,queue) VALUES($1,$2,$3,'sync')",
        &[
            &job.as_slice(),
            &key,
            &json!({"batch":hex::encode(batch),"prefix":prefix}),
        ],
    )
    .await?;
    tx.execute(
        "INSERT INTO sync_partitions(job_id,batch_id,prefix) VALUES($1,$2,$3)",
        &[&job.as_slice(), &batch.as_slice(), &prefix],
    )
    .await?;
    Ok(())
}
impl Repository {
    pub(crate) async fn validate_sync_spec(&self, spec: &SyncSpec) -> Result<()> {
        crate::inventory::inventory_kinds(&spec.inventory)?;
        Filter::new(&spec.filter)
            .and_then(Filter::compile)
            .map_err(anyhow::Error::msg)?;
        ensure!(spec.fallbacks.len() <= 7, "fallback budget");
        ensure!(
            matches!(spec.inventory.as_str(), "chapters" | "series") || spec.fallbacks.is_empty(),
            "account inventory fallback is not configured in this profile"
        );
        let mut keys = BTreeSet::from([id(&spec.peer)?]);
        for key in &spec.fallbacks {
            ensure!(keys.insert(id(key)?), "duplicate peer");
        }
        for key in keys {
            self.approved_peer(key).await?;
        }
        Ok(())
    }
    pub async fn enqueue_sync(&self, spec: SyncSpec) -> Result<Id> {
        self.validate_sync_spec(&spec).await?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        tx.execute("SELECT pg_advisory_xact_lock(697241115)", &[])
            .await?;
        let batch = enqueue_sync_tx(&tx, &spec, false).await?;
        tx.commit().await?;
        Ok(batch)
    }
    pub async fn claim_sync(&self) -> Result<Option<Work>> {
        let token: [u8; 16] = rand::random();
        let row=self.pool.get().await?.query_opt("WITH next AS (SELECT j.id FROM jobs j JOIN sync_partitions p ON p.job_id=j.id JOIN sync_batches b ON b.id=p.batch_id WHERE j.queue='sync' AND b.state IN ('queued','running') AND ((j.state='queued' AND j.available_at<=clock_timestamp()) OR (j.state='leased' AND j.lease_until<=clock_timestamp())) ORDER BY j.available_at,j.created_at,j.id FOR UPDATE OF j SKIP LOCKED LIMIT 1) UPDATE jobs j SET state='leased',generation=generation+1,claim_token=$1,lease_until=clock_timestamp()+interval '30 seconds',attempts=attempts+1,updated_at=clock_timestamp() FROM next WHERE j.id=next.id RETURNING j.id,j.generation,j.payload", &[&token.as_slice()]).await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let lease = Lease {
            id: row
                .get::<_, Vec<u8>>(0)
                .try_into()
                .ok()
                .context("invalid job ID")?,
            generation: row.get(1),
            token,
            payload: row.get(2),
        };
        let row=self.pool.get().await?.query_one("SELECT p.batch_id,p.prefix,p.reconciled,b.spec FROM sync_partitions p JOIN sync_batches b ON b.id=p.batch_id WHERE p.job_id=$1", &[&lease.id.as_slice()]).await?;
        Ok(Some(Work {
            lease,
            batch: row
                .get::<_, Vec<u8>>(0)
                .try_into()
                .ok()
                .context("invalid batch ID")?,
            prefix: row.get(1),
            reconciled: row.get(2),
            spec: serde_json::from_value(row.get(3))?,
        }))
    }
    pub async fn save_missing(&self, work: &Work, ids: Vec<Id>) -> Result<()> {
        ensure!(ids.len() <= MAX_ITEMS, "missing item budget");
        let unique: BTreeSet<_> = ids.into_iter().collect();
        ensure!(
            unique
                .iter()
                .all(|id| hex::encode(id).starts_with(&work.prefix)),
            "peer IDs outside partition"
        );
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        lock_work(&tx, &work.lease).await?;
        let reconciled: bool = tx
            .query_one(
                "SELECT reconciled FROM sync_partitions WHERE job_id=$1 FOR UPDATE",
                &[&work.lease.id.as_slice()],
            )
            .await?
            .get(0);
        if reconciled {
            return Ok(());
        }
        let count = unique.len() as i64;
        let reserved=tx.execute("UPDATE sync_queue_budget SET pending_items=pending_items+$1 WHERE singleton=TRUE AND pending_items+$1<=200000", &[&count]).await?;
        if reserved != 1 {
            return Err(QueueFull.into());
        }
        let ids: Vec<Vec<u8>> = unique.into_iter().map(|id| id.to_vec()).collect();
        tx.execute(
            "INSERT INTO sync_items(job_id,event_id) SELECT $1,unnest($2::bytea[])",
            &[&work.lease.id.as_slice(), &ids],
        )
        .await?;
        tx.execute(
            "UPDATE sync_partitions SET reconciled=TRUE,failures=0 WHERE job_id=$1",
            &[&work.lease.id.as_slice()],
        )
        .await?;
        tx.execute(
            "UPDATE sync_batches SET state='running',last_error=NULL,updated_at=clock_timestamp() WHERE id=$1",
            &[&work.batch.as_slice()],
        )
        .await?;
        check_lease_tx(&tx, &work.lease).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn split_sync_partition(&self, work: &Work) -> Result<()> {
        prefix_bounds(&work.prefix)?;
        ensure!(work.prefix.len() < 64, "cannot split full event ID");
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        // Budget lock precedes batch lock in every operation that creates partitions.
        tx.execute("SELECT pg_advisory_xact_lock(697241115)", &[])
            .await?;
        lock_work(&tx, &work.lease).await?;
        let reconciled: bool = tx
            .query_one(
                "SELECT reconciled FROM sync_partitions WHERE job_id=$1",
                &[&work.lease.id.as_slice()],
            )
            .await?
            .get(0);
        ensure!(!reconciled, "cannot split an already reconciled partition");
        let count: i32 = tx
            .query_one(
                "SELECT partitions_created FROM sync_batches WHERE id=$1",
                &[&work.batch.as_slice()],
            )
            .await?
            .get(0);
        ensure!(count + 16 <= 4096, "batch partition limit; narrow filter");
        let active: i64 = tx
            .query_one(
                "SELECT COUNT(*) FROM jobs WHERE queue='sync' AND state IN ('queued','leased')",
                &[],
            )
            .await?
            .get(0);
        if active + 15 > 4096 {
            return Err(QueueFull.into());
        }
        for digit in "0123456789abcdef".chars() {
            insert_partition(&tx, work.batch, &format!("{}{digit}", work.prefix)).await?;
        }
        tx.execute("UPDATE jobs SET state='completed',claim_token=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1", &[&work.lease.id.as_slice()]).await?;
        tx.execute("UPDATE sync_batches SET state='running',partitions_created=partitions_created+16,partitions_done=partitions_done+1,updated_at=clock_timestamp() WHERE id=$1", &[&work.batch.as_slice()]).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn sync_items(&self, work: &Work) -> Result<Vec<Id>> {
        ensure!(self.lease_current(&work.lease).await?, "stale work");
        self.pool
            .get()
            .await?
            .query(
                "SELECT event_id FROM sync_items WHERE job_id=$1 ORDER BY event_id LIMIT 16",
                &[&work.lease.id.as_slice()],
            )
            .await?
            .into_iter()
            .map(|r| {
                r.get::<_, Vec<u8>>(0)
                    .try_into()
                    .ok()
                    .context("invalid pending ID")
            })
            .collect()
    }
    pub async fn acknowledge_sync_item(&self, work: &Work, event: Id) -> Result<()> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        lock_work(&tx, &work.lease).await?;
        let n = tx
            .execute(
                "DELETE FROM sync_items WHERE job_id=$1 AND event_id=$2",
                &[&work.lease.id.as_slice(), &event.as_slice()],
            )
            .await? as i64;
        tx.execute(
            "UPDATE sync_queue_budget SET pending_items=pending_items-$1 WHERE singleton=TRUE",
            &[&n],
        )
        .await?;
        tx.execute("UPDATE sync_batches SET acknowledged_items=acknowledged_items+$2,last_error=NULL,updated_at=clock_timestamp() WHERE id=$1", &[&work.batch.as_slice(),&n]).await?;
        tx.execute(
            "UPDATE sync_partitions SET failures=0 WHERE job_id=$1",
            &[&work.lease.id.as_slice()],
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
    /// Atomically settle one rejected queued item. Counts cover every skip;
    /// detailed IDs are a clearly labelled bounded diagnostic sample.
    pub async fn skip_sync_item(
        &self,
        work: &Work,
        event: Id,
        rejection: &crate::transfer_rejection::TransferRejection,
    ) -> Result<()> {
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        lock_work(&tx, &work.lease).await?;
        let n = tx
            .execute(
                "DELETE FROM sync_items WHERE job_id=$1 AND event_id=$2",
                &[&work.lease.id.as_slice(), &event.as_slice()],
            )
            .await? as i64;
        if n == 0 {
            tx.commit().await?;
            return Ok(());
        }
        tx.execute(
            "UPDATE sync_queue_budget SET pending_items=pending_items-$1 WHERE singleton=TRUE",
            &[&n],
        )
        .await?;
        let count: i32 = tx
            .query_one(
                "SELECT skip_details_count FROM sync_batches WHERE id=$1",
                &[&work.batch.as_slice()],
            )
            .await?
            .get(0);
        let stored = if count < 256 {
            tx.execute("INSERT INTO sync_skipped_items(batch_id,event_id,category,reason) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING", &[&work.batch.as_slice(),&event.as_slice(),&rejection.class.code(),&rejection.reason.code()]).await? as i32
        } else {
            0
        };
        let code = format!("{}:{}", rejection.class.code(), rejection.reason.code());
        tx.execute("UPDATE sync_batches SET acknowledged_items=acknowledged_items+1,skipped_items=skipped_items+1,skip_details_count=skip_details_count+$2,skip_counts=jsonb_set(skip_counts,ARRAY[$3::text],to_jsonb(COALESCE((skip_counts->>$3)::bigint,0)+1),true),last_error=NULL,updated_at=clock_timestamp() WHERE id=$1", &[&work.batch.as_slice(),&stored,&code]).await?;
        tx.execute(
            "UPDATE sync_partitions SET failures=0 WHERE job_id=$1",
            &[&work.lease.id.as_slice()],
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn finish_sync_quantum(&self, work: &Work) -> Result<()> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        lock_work(&tx, &work.lease).await?;
        let reconciled: bool = tx
            .query_one(
                "SELECT reconciled FROM sync_partitions WHERE job_id=$1",
                &[&work.lease.id.as_slice()],
            )
            .await?
            .get(0);
        ensure!(reconciled, "inventory has not been reconciled");
        let pending: bool = tx
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM sync_items WHERE job_id=$1)",
                &[&work.lease.id.as_slice()],
            )
            .await?
            .get(0);
        if pending {
            tx.execute("UPDATE jobs SET state='queued',claim_token=NULL,lease_until=NULL,available_at=clock_timestamp()+interval '10 milliseconds',updated_at=clock_timestamp() WHERE id=$1", &[&work.lease.id.as_slice()]).await?;
        } else {
            tx.execute("UPDATE jobs SET state='completed',claim_token=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1", &[&work.lease.id.as_slice()]).await?;
            tx.execute("UPDATE sync_batches SET partitions_done=partitions_done+1,updated_at=clock_timestamp() WHERE id=$1", &[&work.batch.as_slice()]).await?;
            let pending:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM jobs j JOIN sync_partitions p ON p.job_id=j.id WHERE p.batch_id=$1 AND j.state IN ('queued','leased'))", &[&work.batch.as_slice()]).await?.get(0);
            if !pending {
                tx.execute("UPDATE sync_batches SET state=CASE WHEN skipped_items>0 THEN 'completed_with_skips' ELSE 'completed' END,last_error=NULL,updated_at=clock_timestamp() WHERE id=$1", &[&work.batch.as_slice()]).await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }
    pub async fn retry_sync(&self, work: &Work, error: &anyhow::Error) -> Result<()> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        lock_work(&tx, &work.lease).await?;
        let backpressure = error.is::<QueueFull>();
        let failures:i32=tx.query_one("UPDATE sync_partitions SET failures=failures+$2 WHERE job_id=$1 RETURNING failures", &[&work.lease.id.as_slice(),&if backpressure{0i32}else{1i32}]).await?.get(0);
        let message: String = error.to_string().chars().take(512).collect();
        if failures >= 5 {
            tx.execute("UPDATE jobs SET state=CASE WHEN id=$2 THEN 'failed' ELSE 'cancelled' END,generation=generation+1,claim_token=NULL,lease_until=NULL WHERE id IN (SELECT job_id FROM sync_partitions WHERE batch_id=$1) AND state IN ('queued','leased')", &[&work.batch.as_slice(),&work.lease.id.as_slice()]).await?;
            let n:i64=tx.query_one("WITH removed AS (DELETE FROM sync_items WHERE job_id IN (SELECT job_id FROM sync_partitions WHERE batch_id=$1) RETURNING 1) SELECT COUNT(*) FROM removed", &[&work.batch.as_slice()]).await?.get(0);
            tx.execute(
                "UPDATE sync_queue_budget SET pending_items=pending_items-$1 WHERE singleton=TRUE",
                &[&n],
            )
            .await?;
            tx.execute("UPDATE sync_batches SET state='failed',last_error=$2,updated_at=clock_timestamp() WHERE id=$1", &[&work.batch.as_slice(),&message]).await?;
        } else {
            let delay = if backpressure {
                1i64
            } else {
                1i64 << failures.min(5)
            };
            tx.execute("UPDATE jobs SET state='queued',claim_token=NULL,lease_until=NULL,available_at=clock_timestamp()+($2::BIGINT*interval '1 second'),updated_at=clock_timestamp() WHERE id=$1", &[&work.lease.id.as_slice(),&delay]).await?;
            tx.execute(
                "UPDATE sync_batches SET last_error=$2,updated_at=clock_timestamp() WHERE id=$1",
                &[&work.batch.as_slice(), &message],
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    pub async fn cancel_sync(&self, batch: Id) -> Result<bool> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let changed=tx.execute("UPDATE sync_batches SET state='cancelled',updated_at=clock_timestamp() WHERE id=$1 AND state IN ('queued','running')", &[&batch.as_slice()]).await?;
        if changed == 0 {
            return Ok(false);
        }
        tx.execute("UPDATE jobs SET state='cancelled',generation=generation+1,claim_token=NULL,lease_until=NULL WHERE id IN (SELECT job_id FROM sync_partitions WHERE batch_id=$1) AND state IN ('queued','leased')", &[&batch.as_slice()]).await?;
        let n:i64=tx.query_one("WITH removed AS (DELETE FROM sync_items WHERE job_id IN (SELECT job_id FROM sync_partitions WHERE batch_id=$1) RETURNING 1) SELECT COUNT(*) FROM removed", &[&batch.as_slice()]).await?.get(0);
        tx.execute(
            "UPDATE sync_queue_budget SET pending_items=pending_items-$1 WHERE singleton=TRUE",
            &[&n],
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }
    pub async fn sync_status(&self, batch: Id) -> Result<Value> {
        // One statement gives counts and bounded detail samples one snapshot.
        const STATUS_SQL: &str = r#"
            SELECT state,partitions_created,partitions_done,acknowledged_items,
                   last_error,skipped_items,skip_details_count,skip_counts,
                   COALESCE((SELECT jsonb_agg(jsonb_build_object(
                       'event',encode(d.event_id,'hex'),'category',d.category,'reason',d.reason)
                       ORDER BY d.event_id)
                       FROM (SELECT event_id,category,reason FROM sync_skipped_items
                             WHERE batch_id=$1 ORDER BY event_id LIMIT 256) d),'[]'::jsonb)
            FROM sync_batches WHERE id=$1
        "#;
        let row = self
            .pool
            .get()
            .await?
            .query_one(STATUS_SQL, &[&batch.as_slice()])
            .await?;
        let skipped: i64 = row.get(5);
        let stored: i32 = row.get(6);
        Ok(
            json!({"id":hex::encode(batch),"state":row.get::<_,String>(0),"partitions_created":row.get::<_,i32>(1),"partitions_done":row.get::<_,i32>(2),"acknowledged_items":row.get::<_,i64>(3),"last_error":row.get::<_,Option<String>>(4),"skipped_items":skipped,"skip_counts":row.get::<_,Value>(7),"skip_details":row.get::<_,Value>(8),"skip_details_limit":256,"skip_details_truncated":skipped>i64::from(stored),"note":"acknowledgements include filtered/skipped queued items, not only admitted content; admitted content is retained on cancellation"}),
        )
    }
}
pub struct SyncRunner {
    repo: Repository,
    peers: Arc<PeerSync>,
    libraries: Option<Arc<crate::library_storage::LibraryObjects>>,
}
impl SyncRunner {
    pub fn new(
        repo: Repository,
        peers: Arc<PeerSync>,
        libraries: Option<Arc<crate::library_storage::LibraryObjects>>,
    ) -> Self {
        Self {
            repo,
            peers,
            libraries,
        }
    }
    pub async fn run(self) {
        loop {
            let slot = match self.peers.slots.clone().acquire_owned().await {
                Ok(slot) => slot,
                Err(_) => return,
            };
            match self.repo.claim_sync().await {
                Ok(Some(work)) => {
                    let _slot = slot;
                    let repo = self.repo.clone();
                    let lease = work.lease.clone();
                    let heartbeat = crate::node::spawn_infrastructure_owned_scope(async move {
                        loop {
                            tokio::time::sleep(Duration::from_secs(10)).await;
                            if !repo
                                .renew(&lease, Duration::from_secs(30))
                                .await
                                .unwrap_or(false)
                            {
                                break;
                            }
                        }
                    });
                    // Cancellation of the sync parent must stop its private
                    // renewal loop; its actual termination stays lifecycle-owned.
                    let _heartbeat_owner=crate::node::AbortChildOnDrop(heartbeat.abort_handle());
                    let outcome =
                        tokio::time::timeout(Duration::from_secs(600), self.quantum(&work))
                            .await
                            .unwrap_or_else(|_| {
                                Err(anyhow::anyhow!("synchronization quantum deadline"))
                            });
                    heartbeat.abort();
                    if let Err(error) = outcome
                        && self.repo.lease_current(&work.lease).await.unwrap_or(false)
                        && let Err(save) = self.repo.retry_sync(&work, &error).await
                    {
                        tracing::warn!(error=%save,"sync retry state unavailable; lease retained");
                    }
                }
                Ok(None) => {
                    drop(slot);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(error) => {
                    drop(slot);
                    tracing::warn!(error=%error,"sync claim unavailable");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }
    async fn item_outcome<T>(
        &self,
        work: &Work,
        event: Id,
        outcome: Result<T>,
    ) -> Result<Option<T>> {
        match outcome {
            Ok(value) => Ok(Some(value)),
            Err(error) => {
                if let Some(rejection) =
                    error.downcast_ref::<crate::transfer_rejection::TransferRejection>()
                {
                    self.repo.skip_sync_item(work, event, rejection).await?;
                    Ok(None)
                } else {
                    Err(error)
                }
            }
        }
    }
    async fn quantum(&self, work: &Work) -> Result<()> {
        if !work.reconciled {
            match self
                .peers
                .reconcile_partition(
                    &work.spec.peer,
                    &work.spec.filter,
                    &work.prefix,
                    &work.spec.inventory,
                )
                .await
            {
                Ok(ids) => self.repo.save_missing(work, ids).await?,
                Err(error) if error.is::<PartitionRequired>() => {
                    return self.repo.split_sync_partition(work).await;
                }
                Err(error) => return Err(error),
            }
        }
        for event in self.repo.sync_items(work).await? {
            if work.spec.inventory == "libraries" {
                let outcome = self
                    .peers
                    .synchronize_library_inventory(
                        self.libraries
                            .clone()
                            .context("library storage unavailable")?,
                        &work.spec.peer,
                        event,
                        work.lease.clone(),
                        &work.spec.filter,
                    )
                    .await;
                let Some(result) = self.item_outcome(work, event, outcome).await? else {
                    continue;
                };
                if result.hosted {
                    let head = id(&result.event)?;
                    while !self.repo.projection_complete(head).await? {
                        ensure!(self.repo.lease_current(&work.lease).await?, "stale work");
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
                self.repo.acknowledge_sync_item(work, event).await?;
                continue;
            }

            if matches!(
                work.spec.inventory.as_str(),
                "profiles" | "deletions" | "social"
            ) {
                let outcome = self
                    .peers
                    .synchronize_account_fenced(
                        crate::peers::AccountSyncRequest {
                            peer: work.spec.peer.clone(),
                            event: hex::encode(event),
                        },
                        Some(work.lease.clone()),
                        Some(&work.spec.inventory),
                        &work.spec.filter,
                    )
                    .await;
                let Some(result) = self.item_outcome(work, event, outcome).await? else {
                    continue;
                };
                if result.get("projected").is_some() {
                    while !self.repo.projection_complete(event).await? {
                        ensure!(self.repo.lease_current(&work.lease).await?, "stale work");
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
                self.repo.acknowledge_sync_item(work, event).await?;
                continue;
            }

            let outcome = self
                .peers
                .synchronize_fenced(
                    SyncRequest {
                        peer: work.spec.peer.clone(),
                        event: hex::encode(event),
                        filter: work.spec.filter.clone(),
                        fallbacks: work.spec.fallbacks.clone(),
                    },
                    Some(work.lease.clone()),
                    Some(crate::inventory::inventory_kind(&work.spec.inventory)?),
                )
                .await;
            let Some(result) = self.item_outcome(work, event, outcome).await? else {
                continue;
            };
            if result.hosted {
                let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
                while !self.repo.projection_complete(event).await? {
                    ensure!(
                        tokio::time::Instant::now() < deadline,
                        "admitted event projection pending"
                    );
                    ensure!(self.repo.lease_current(&work.lease).await?, "stale work");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
            self.repo.acknowledge_sync_item(work, event).await?;
        }
        self.repo.finish_sync_quantum(work).await
    }
}

// Caller holds the global synchronization admission advisory lock.
pub(crate) async fn enqueue_sync_tx(
    tx: &tokio_postgres::Transaction<'_>,
    spec: &SyncSpec,
    scheduled: bool,
) -> Result<Id> {
    let payload = serde_json::to_value(spec)?;
    let key = id(&auth::hash(&serde_json::to_vec(&payload)?))?;
    if let Some(row)=tx.query_opt("SELECT id FROM sync_batches WHERE state IN ('queued','running') AND (request_key=$1 OR ('{\"inventory\":\"chapters\"}'::jsonb||spec)=$2) ORDER BY created_at,id LIMIT 1", &[&key.as_slice(),&payload]).await? {
            let existing:Vec<u8>=row.get(0);
            if !scheduled{tx.execute("UPDATE sync_batches SET scheduled=FALSE WHERE id=$1",&[&existing]).await?;}
            return existing.try_into().ok().context("invalid batch ID");
        }
    let active: i64 = tx
        .query_one(
            "SELECT COUNT(*) FROM sync_batches WHERE state IN ('queued','running')",
            &[],
        )
        .await?
        .get(0);
    ensure!(active < 64, "active synchronization batch limit");
    let jobs: i64 = tx
        .query_one(
            "SELECT COUNT(*) FROM jobs WHERE queue='sync' AND state IN ('queued','leased')",
            &[],
        )
        .await?
        .get(0);
    ensure!(jobs < 4096, "active partition limit");
    if scheduled {
        let retained: i64 = tx
            .query_one("SELECT count(*) FROM sync_batches WHERE scheduled", &[])
            .await?
            .get(0);
        if retained >= 512 {
            let victim=tx.query_opt("SELECT id FROM sync_batches b WHERE scheduled AND state IN ('completed','completed_with_skips','cancelled','failed') AND NOT EXISTS(SELECT 1 FROM sync_schedules s WHERE s.last_batch=b.id) ORDER BY created_at,id LIMIT 1 FOR UPDATE",&[]).await?.context("scheduled history budget full")?;
            let victim: Vec<u8> = victim.get(0);
            ensure!(!tx.query_one("SELECT EXISTS(SELECT 1 FROM sync_items i JOIN sync_partitions p ON p.job_id=i.job_id WHERE p.batch_id=$1)",&[&victim]).await?.get::<_,bool>(0),"terminal batch retains pending credit");
            let ids = tx
                .query(
                    "DELETE FROM sync_partitions WHERE batch_id=$1 RETURNING job_id",
                    &[&victim],
                )
                .await?
                .into_iter()
                .map(|r| r.get::<_, Vec<u8>>(0))
                .collect::<Vec<_>>();
            tx.execute("DELETE FROM jobs WHERE id=ANY($1)", &[&ids])
                .await?;
            tx.execute("DELETE FROM sync_batches WHERE id=$1", &[&victim])
                .await?;
        }
    }
    let batch: Id = rand::random();
    tx.execute(
        "INSERT INTO sync_batches(id,request_key,spec,scheduled) VALUES($1,$2,$3,$4)",
        &[&batch.as_slice(), &key.as_slice(), &payload, &scheduled],
    )
    .await?;
    insert_partition(tx, batch, "").await?;
    Ok(batch)
}
