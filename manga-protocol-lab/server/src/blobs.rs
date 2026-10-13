//! Durable staged originals. Publication/retention is a separate lifecycle.
//! Callers hold the volume's hash mutation guard across intent, I/O and completion.
use crate::{
    capacity::{reserve_tx, settle_tx},
    db::{Id, Repository},
    storage::{BlobKey, CleanupReceipt, DurableReceipt},
};
use anyhow::{Context, Result, ensure};
#[derive(Clone, Debug)]
pub struct BlobIntent {
    pub key: BlobKey,
    pub volume: String,
    pub owner: Id,
    pub size: u64,
    pub available: bool,
    pub reservation: Id,
}
fn decode(row: tokio_postgres::Row) -> Result<BlobIntent> {
    Ok(BlobIntent {
        key: BlobKey {
            hash: row
                .get::<_, Vec<u8>>(0)
                .try_into()
                .ok()
                .context("invalid hash")?,
            generation: row
                .get::<_, Vec<u8>>(1)
                .try_into()
                .ok()
                .context("invalid generation")?,
        },
        volume: row.get(2),
        owner: row
            .get::<_, Vec<u8>>(3)
            .try_into()
            .ok()
            .context("invalid owner")?,
        size: u64::try_from(row.get::<_, i64>(4))?,
        available: row.get::<_, String>(5) == "available",
        reservation: row
            .get::<_, Vec<u8>>(6)
            .try_into()
            .ok()
            .context("invalid reservation")?,
    })
}
const COLUMNS: &str = "hash,generation,volume,owner,size,state,reservation";
impl Repository {
    pub async fn blob_intent(&self, hash: Id) -> Result<Option<BlobIntent>> {
        let client = self.pool.get().await?;
        client
            .query_opt(
                &client.prepare_cached(&format!("SELECT {COLUMNS} FROM blob_intents WHERE hash=$1 AND NOT EXISTS(SELECT 1 FROM blob_reclaims r WHERE r.hash=blob_intents.hash AND r.generation=blob_intents.generation)")).await?,
                &[&hash.as_slice()],
            )
            .await?
            .map(decode)
            .transpose()
    }
    /// Atomic claim + debit. Duplicate owned objects do not need fresh quota.
    /// A different owner requires the future explicit sharing/retention path.
    pub async fn begin_blob(
        &self,
        hash: Id,
        volume: &str,
        owner: Id,
        size: u64,
        grants: &[Id],
    ) -> Result<BlobIntent> {
        let size = i64::try_from(size)?;
        ensure!(
            size > 0 && !volume.is_empty() && volume.len() <= 160,
            "invalid blob bounds"
        );
        let generation: [u8; 16] = rand::random();
        let reservation: Id = rand::random();
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let inserted=tx.execute(&tx.prepare_cached("INSERT INTO blob_intents(hash,generation,volume,owner,size,reservation) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(hash) DO NOTHING").await?, &[&hash.as_slice(),&generation.as_slice(),&volume,&owner.as_slice(),&size,&reservation.as_slice()]).await?==1;
        let intent = decode(
            tx.query_one(
                &tx.prepare_cached(&format!(
                    "SELECT {COLUMNS} FROM blob_intents WHERE hash=$1 FOR UPDATE"
                ))
                .await?,
                &[&hash.as_slice()],
            )
            .await?,
        )?;
        ensure!(
            intent.volume == volume && intent.owner == owner && intent.size == size as u64,
            "blob belongs to another admission or descriptor"
        );
        ensure!(
            !tx.query_one(
                &tx.prepare_cached(
                    "SELECT EXISTS(SELECT 1 FROM blob_reclaims WHERE hash=$1 AND generation=$2)"
                )
                .await?,
                &[
                    &intent.key.hash.as_slice(),
                    &intent.key.generation.as_slice()
                ]
            )
            .await?
            .get::<_, bool>(0),
            "blob reclamation is pending"
        );
        if inserted {
            tx.execute(
                &tx.prepare_cached(
                    "INSERT INTO blob_retention_credits(hash,generation) VALUES($1,$2)",
                )
                .await?,
                &[&hash.as_slice(), &generation.as_slice()],
            )
            .await?;
            reserve_tx(&tx, reservation, grants, size).await?;
        }
        tx.commit().await?;
        Ok(intent)
    }
    /// Receipt is minted by verified file write + fsync, never by request fields.
    pub async fn finish_blob(&self, volume: &str, receipt: &DurableReceipt) -> Result<bool> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let intent = decode(
            tx.query_one(
                &tx.prepare_cached(&format!(
                    "SELECT {COLUMNS} FROM blob_intents WHERE hash=$1 FOR UPDATE"
                ))
                .await?,
                &[&receipt.key().hash.as_slice()],
            )
            .await?,
        )?;
        ensure!(
            intent.key == *receipt.key()
                && intent.volume == volume
                && intent.size == receipt.size(),
            "stale or mismatched durable receipt"
        );
        ensure!(
            !tx.query_one(
                &tx.prepare_cached(
                    "SELECT EXISTS(SELECT 1 FROM blob_reclaims WHERE hash=$1 AND generation=$2)"
                )
                .await?,
                &[
                    &intent.key.hash.as_slice(),
                    &intent.key.generation.as_slice()
                ]
            )
            .await?
            .get::<_, bool>(0),
            "blob reclamation is pending"
        );
        if intent.available {
            tx.execute(&tx.prepare_cached("UPDATE blob_intents SET staging_until=clock_timestamp()+interval '5 minutes' WHERE hash=$1").await?,&[&receipt.key().hash.as_slice()]).await?;
            tx.commit().await?;
            return Ok(false);
        }
        settle_tx(&tx, intent.reservation, i64::try_from(receipt.size())?).await?;
        tx.execute(&tx.prepare_cached("UPDATE blob_intents SET state='available',verified_at=clock_timestamp(),staging_until=clock_timestamp()+interval '5 minutes' WHERE hash=$1").await?,
            &[&receipt.key().hash.as_slice()],
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }
    /// Cleanup can release only an unfinished matching generation. Published or
    /// available bytes need a separate reference-aware deletion transaction.
    pub async fn abort_blob(&self, volume: &str, receipt: &CleanupReceipt) -> Result<bool> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let Some(row) = tx
            .query_opt(
                &tx.prepare_cached(&format!(
                    "SELECT {COLUMNS} FROM blob_intents WHERE hash=$1 FOR UPDATE"
                ))
                .await?,
                &[&receipt.key().hash.as_slice()],
            )
            .await?
        else {
            return Ok(false);
        };
        let intent = decode(row)?;
        ensure!(
            intent.key == *receipt.key() && intent.volume == volume && !intent.available,
            "cleanup cannot refund this generation/state"
        );
        settle_tx(&tx, intent.reservation, 0).await?;
        tx.execute(
            &tx.prepare_cached("DELETE FROM blob_intents WHERE hash=$1")
                .await?,
            &[&receipt.key().hash.as_slice()],
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }
}
