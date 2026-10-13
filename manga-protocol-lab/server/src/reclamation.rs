//! Generation-specific library cleanup. Filesystem cleanup precedes any refund.
use crate::{
    capacity::refund_reservation_tx,
    db::{Id, Repository},
    storage::{BlobKey, CleanupReceipt},
};
use anyhow::{Result, ensure};
impl Repository {
    /// Bounded sparse-index selection, independent of the hosted catalogue.
    /// A queued row is only a candidate; receipt-backed reclaim checks all holds.
    pub async fn enqueue_expired_staging(&self, owner: Id, volume: &str) -> Result<usize> {
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        tx.execute(&tx.prepare_cached("DELETE FROM staging_operations WHERE id IN (SELECT id FROM staging_operations WHERE expires_at<=statement_timestamp() ORDER BY expires_at,id LIMIT 64 FOR UPDATE SKIP LOCKED)").await?, &[]).await?;
        tx.execute(
            &tx.prepare_cached(
                "INSERT INTO staging_gc_cursors(owner,volume) VALUES($1,$2) ON CONFLICT DO NOTHING",
            )
            .await?,
            &[&owner.as_slice(), &volume],
        )
        .await?;
        let after: Option<Vec<u8>> = tx.query_one(&tx.prepare_cached("SELECT last_hash FROM staging_gc_cursors WHERE owner=$1 AND volume=$2 FOR UPDATE").await?,
                &[&owner.as_slice(), &volume],
            )
            .await?
            .get(0);
        // Limit the sparse unreleased-credit scan before owner/lease/queue
        // filtering. Protected or foreign rows still advance the cursor, so a
        // tick cannot traverse an unbounded catalogue looking for 64 matches.
        let candidates = tx.query(&tx.prepare_cached(
            "SELECT hash,generation FROM blob_retention_credits WHERE NOT staging_released AND hash>COALESCE($1::bytea,''::bytea) ORDER BY hash LIMIT 64"
        ).await?, &[&after]).await?;
        let hashes: Vec<Vec<u8>> = candidates.iter().map(|row| row.get(0)).collect();
        let rows = tx.query(&tx.prepare_cached(
            "SELECT b.hash,b.generation FROM blob_intents b WHERE b.hash=ANY($3) AND b.owner=$1 AND b.volume=$2 AND b.state='available' AND b.staging_until<=statement_timestamp() AND EXISTS(SELECT 1 FROM blob_retention_credits r WHERE r.hash=b.hash AND r.generation=b.generation AND NOT r.staging_released) AND NOT EXISTS(SELECT 1 FROM group_blob_grants g WHERE g.hash=b.hash AND g.generation=b.generation AND g.lease_until>statement_timestamp()) AND NOT EXISTS(SELECT 1 FROM blob_gc_queue q WHERE q.hash=b.hash AND q.generation=b.generation) ORDER BY b.hash FOR SHARE OF b"
        ).await?, &[&owner.as_slice(), &volume, &hashes]).await?;
        for row in &rows {
            let hash: Vec<u8> = row.get(0);
            let generation: Vec<u8> = row.get(1);
            tx.execute(&tx.prepare_cached("INSERT INTO blob_gc_queue(hash,generation,due_at,source_mask) SELECT hash,generation,staging_until,4 FROM blob_intents WHERE hash=$1 AND generation=$2 ON CONFLICT(hash,generation) DO UPDATE SET source_mask=blob_gc_queue.source_mask|4").await?, &[&hash,&generation]).await?;
        }
        let last: Option<Vec<u8>> = candidates.last().map(|r| r.get(0));
        if last != after {
            tx.execute(
                &tx.prepare_cached(
                    "UPDATE staging_gc_cursors SET last_hash=$3 WHERE owner=$1 AND volume=$2",
                )
                .await?,
                &[&owner.as_slice(), &volume, &last],
            )
            .await?;
        }
        tx.commit().await?;
        Ok(rows.len())
    }
    pub(crate) async fn blob_reclaiming(&self, key: &BlobKey) -> Result<bool> {
        Ok(self
            .pool
            .get()
            .await?
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM blob_reclaims WHERE hash=$1 AND generation=$2)",
                &[&key.hash.as_slice(), &key.generation.as_slice()],
            )
            .await?
            .get(0))
    }
    /// Caller owns the filesystem hash mutation guard before taking these row locks.
    pub(crate) async fn claim_library_reclaim(
        &self,
        key: &BlobKey,
        owner: Id,
        volume: &str,
    ) -> Result<bool> {
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        let Some(b) = tx.query_opt(&tx.prepare_cached("SELECT generation,owner,volume,state FROM blob_intents WHERE hash=$1 FOR UPDATE").await?,
                &[&key.hash.as_slice()],
            )
            .await?
        else {
            return Ok(false);
        };
        if b.get::<_, Vec<u8>>(0) != key.generation
            || b.get::<_, Vec<u8>>(1) != owner
            || b.get::<_, String>(2) != volume
        {
            return Ok(false);
        }
        ensure!(
            b.get::<_, String>(3) == "available",
            "only available generations can be reclaimed"
        );
        // Reference/grant writers also fence on this blob generation. Acquire
        // that lock in its own statement, then read one fresh protection snapshot.
        // Combining these checks with the locking statement could reuse a stale
        // pre-wait snapshot and miss a just-committed quarantine or reference.
        let checks = tx.query_one(&tx.prepare_cached(r#"
SELECT
    (EXISTS(SELECT 1 FROM public_export_holds WHERE hash=$1 AND generation=$2)) AS public_readers,
    (EXISTS(SELECT 1 FROM blob_reclaims WHERE hash=$1 AND generation=$2)) AS marked,
    (EXISTS(SELECT 1 FROM publication_blobs WHERE hash=$1 AND generation=$2) OR EXISTS(SELECT 1 FROM library_object_holds WHERE hash=$1 AND generation=$2 AND state IN ('current','pending'))) AS strong,
    (EXISTS(SELECT 1 FROM blob_gc_queue WHERE hash=$1 AND generation=$2 AND due_at<=clock_timestamp())) AS due,
    (((EXISTS(SELECT 1 FROM library_object_holds WHERE hash=$1 AND generation=$2 AND state='grace') OR EXISTS(SELECT 1 FROM publication_blob_grace WHERE hash=$1 AND generation=$2)) AND EXISTS(SELECT 1 FROM blob_retention_credits WHERE hash=$1 AND generation=$2 AND staging_released)) OR (EXISTS(SELECT 1 FROM blob_gc_queue WHERE hash=$1 AND generation=$2 AND (source_mask&4)<>0) AND EXISTS(SELECT 1 FROM blob_retention_credits WHERE hash=$1 AND generation=$2 AND NOT staging_released))) AS eligible,
    (EXISTS(SELECT 1 FROM blob_intents WHERE hash=$1 AND generation=$2 AND staging_until>clock_timestamp()) OR EXISTS(SELECT 1 FROM publication_blob_grace WHERE hash=$1 AND generation=$2 AND grace_until>clock_timestamp()) OR EXISTS(SELECT 1 FROM library_object_holds WHERE hash=$1 AND generation=$2 AND state='grace' AND grace_until>clock_timestamp()) OR EXISTS(SELECT 1 FROM account_blob_grants WHERE hash=$1 AND generation=$2 AND lease_until>clock_timestamp()) OR EXISTS(SELECT 1 FROM publisher_blob_grants WHERE hash=$1 AND generation=$2 AND lease_until>clock_timestamp()) OR EXISTS(SELECT 1 FROM group_blob_grants WHERE hash=$1 AND generation=$2 AND lease_until>clock_timestamp()) OR EXISTS(SELECT 1 FROM staging_operation_blobs h JOIN staging_operations o ON o.id=h.operation WHERE h.hash=$1 AND h.generation=$2 AND o.expires_at>clock_timestamp()) OR EXISTS(SELECT 1 FROM review_blob_grants g JOIN review_submissions s ON s.event_id=g.submission WHERE g.hash=$1 AND g.generation=$2 AND s.state='pending' AND s.expires_at>clock_timestamp())) AS protected
"#).await?, &[&key.hash.as_slice(), &key.generation.as_slice()]).await?;
        if checks.get::<_, bool>("public_readers") {
            tx.execute(&tx.prepare_cached("UPDATE blob_gc_queue SET due_at=GREATEST(due_at,clock_timestamp()+interval '5 seconds') WHERE hash=$1 AND generation=$2").await?,&[&key.hash.as_slice(),&key.generation.as_slice()]).await?;
            tx.commit().await?;
            return Ok(false);
        }
        let marked: bool = checks.get("marked");
        let strong: bool = checks.get("strong");
        if strong {
            ensure!(
                !marked,
                "reclamation marker unexpectedly gained a strong reference"
            );
            tx.execute(&tx.prepare_cached("DELETE FROM library_object_holds WHERE hash=$1 AND generation=$2 AND state='grace' AND grace_until<=clock_timestamp()").await?, &[&key.hash.as_slice(),&key.generation.as_slice()]).await?;
            tx.execute(&tx.prepare_cached("DELETE FROM publication_blob_grace WHERE hash=$1 AND generation=$2 AND grace_until<=clock_timestamp()").await?,&[&key.hash.as_slice(),&key.generation.as_slice()]).await?;
            tx.execute(
                &tx.prepare_cached("DELETE FROM blob_gc_queue WHERE hash=$1 AND generation=$2")
                    .await?,
                &[&key.hash.as_slice(), &key.generation.as_slice()],
            )
            .await?;
            tx.commit().await?;
            return Ok(false);
        }
        if marked {
            tx.commit().await?;
            return Ok(true);
        }
        let due: bool = checks.get("due");
        if !due {
            return Ok(false);
        }
        let eligible: bool = checks.get("eligible");
        if !eligible {
            tx.execute(
                &tx.prepare_cached("DELETE FROM blob_gc_queue WHERE hash=$1 AND generation=$2")
                    .await?,
                &[&key.hash.as_slice(), &key.generation.as_slice()],
            )
            .await?;
            tx.commit().await?;
            return Ok(false);
        }
        let protected: bool = checks.get("protected");
        if protected {
            tx.execute(&tx.prepare_cached("UPDATE blob_gc_queue SET due_at=GREATEST((SELECT staging_until FROM blob_intents WHERE hash=$1 AND generation=$2),(SELECT grace_until FROM publication_blob_grace WHERE hash=$1 AND generation=$2),(SELECT max(grace_until) FROM library_object_holds WHERE hash=$1 AND generation=$2 AND state='grace'),(SELECT max(lease_until) FROM account_blob_grants WHERE hash=$1 AND generation=$2),(SELECT max(lease_until) FROM publisher_blob_grants WHERE hash=$1 AND generation=$2),(SELECT max(lease_until) FROM group_blob_grants WHERE hash=$1 AND generation=$2),(SELECT max(o.expires_at) FROM staging_operation_blobs h JOIN staging_operations o ON o.id=h.operation WHERE h.hash=$1 AND h.generation=$2),(SELECT max(s.expires_at) FROM review_blob_grants g JOIN review_submissions s ON s.event_id=g.submission WHERE g.hash=$1 AND g.generation=$2 AND s.state='pending')) WHERE hash=$1 AND generation=$2").await?,&[&key.hash.as_slice(),&key.generation.as_slice()]).await?;
            tx.commit().await?;
            return Ok(false);
        }
        tx.execute(
            &tx.prepare_cached("INSERT INTO blob_reclaims(hash,generation) VALUES($1,$2)")
                .await?,
            &[&key.hash.as_slice(), &key.generation.as_slice()],
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }
    pub(crate) async fn finish_library_reclaim(
        &self,
        owner: Id,
        volume: &str,
        receipt: &CleanupReceipt,
    ) -> Result<bool> {
        let key = receipt.key();
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        let Some(b)=tx.query_opt(&tx.prepare_cached("SELECT generation,owner,volume,reservation FROM blob_intents WHERE hash=$1 FOR UPDATE").await?,&[&key.hash.as_slice()]).await? else{return Ok(false)};
        ensure!(
            b.get::<_, Vec<u8>>(0) == key.generation
                && b.get::<_, Vec<u8>>(1) == owner
                && b.get::<_, String>(2) == volume,
            "cleanup receipt belongs to another generation/owner"
        );
        let checks = tx.query_one(&tx.prepare_cached(r#"
SELECT
    (EXISTS(SELECT 1 FROM blob_reclaims WHERE hash=$1 AND generation=$2)) AS marked,
    (EXISTS(SELECT 1 FROM publication_blobs WHERE hash=$1 AND generation=$2) OR EXISTS(SELECT 1 FROM library_object_holds WHERE hash=$1 AND generation=$2 AND state IN ('current','pending')) OR EXISTS(SELECT 1 FROM group_blob_grants WHERE hash=$1 AND generation=$2 AND lease_until>clock_timestamp())) AS strong,
    (EXISTS(SELECT 1 FROM public_export_holds WHERE hash=$1 AND generation=$2)) AS public_readers,
    (EXISTS(SELECT 1 FROM staging_operation_blobs h JOIN staging_operations o ON o.id=h.operation WHERE h.hash=$1 AND h.generation=$2 AND o.expires_at>clock_timestamp())) AS transfers
"#).await?, &[&key.hash.as_slice(), &key.generation.as_slice()]).await?;
        ensure!(
            checks.get::<_, bool>("marked"),
            "cleanup lacks durable reclamation claim"
        );
        ensure!(
            !checks.get::<_, bool>("strong"),
            "cleanup gained an unexpected reference"
        );
        ensure!(
            !checks.get::<_, bool>("public_readers"),
            "external reader lifetime is not drained"
        );
        ensure!(
            !checks.get::<_, bool>("transfers"),
            "in-flight transfer still holds original"
        );
        let reservation: Vec<u8> = b.get(3);
        let rid: Id = reservation
            .clone()
            .try_into()
            .map_err(|_| anyhow::anyhow!("reservation encoding"))?;
        refund_reservation_tx(&tx, rid).await?;
        tx.execute(&tx.prepare_cached("DELETE FROM library_object_holds WHERE hash=$1 AND generation=$2 AND state='grace'").await?,
            &[&key.hash.as_slice(), &key.generation.as_slice()],
        )
        .await?;
        tx.execute(
            &tx.prepare_cached("DELETE FROM blob_intents WHERE hash=$1 AND generation=$2")
                .await?,
            &[&key.hash.as_slice(), &key.generation.as_slice()],
        )
        .await?;
        tx.execute(
            &tx.prepare_cached("DELETE FROM capacity_charges WHERE reservation=$1")
                .await?,
            &[&reservation],
        )
        .await?;
        tx.execute(
            &tx.prepare_cached("DELETE FROM capacity_reservations WHERE id=$1")
                .await?,
            &[&reservation],
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }
}

/// Called only after backend-confirmed locally authorized removal, before the
/// exact publication references are dropped in that same transaction.
pub(crate) async fn retire_publication(
    tx: &tokio_postgres::Transaction<'_>,
    event: &[u8],
) -> Result<()> {
    let rows = tx
        .query(
            "SELECT hash,generation FROM publication_blobs WHERE event_id=$1 ORDER BY hash",
            &[&event],
        )
        .await?;
    if rows.is_empty() {
        return Ok(());
    }
    let hashes: Vec<Vec<u8>> = rows.iter().map(|r| r.get(0)).collect();
    tx.query(
        "SELECT hash FROM blob_intents WHERE hash=ANY($1) ORDER BY hash FOR SHARE",
        &[&hashes],
    )
    .await?;
    for row in rows {
        let hash: Vec<u8> = row.get(0);
        let generation: Vec<u8> = row.get(1);
        tx.execute("INSERT INTO publication_blob_grace(hash,generation,grace_until) VALUES($1,$2,clock_timestamp()+interval '5 minutes') ON CONFLICT(hash,generation) DO UPDATE SET grace_until=GREATEST(publication_blob_grace.grace_until,EXCLUDED.grace_until)",&[&hash,&generation]).await?;
        tx.execute("INSERT INTO blob_gc_queue(hash,generation,due_at,source_mask) SELECT hash,generation,grace_until,2 FROM publication_blob_grace WHERE hash=$1 AND generation=$2 ON CONFLICT(hash,generation) DO UPDATE SET due_at=GREATEST(blob_gc_queue.due_at,EXCLUDED.due_at),source_mask=blob_gc_queue.source_mask|2",&[&hash,&generation]).await?;
    }
    Ok(())
}
