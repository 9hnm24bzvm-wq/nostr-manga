//! Bounded physical retention for an in-flight transfer, not publishing authority.
use crate::{
    blobs::BlobIntent,
    db::{Id, Repository},
};
use anyhow::{Context, Result, ensure};
#[derive(Clone)]
pub struct StagingOperation {
    pub(crate) id: Id,
    owner: Id,
    volume: String,
}
impl Repository {
    pub async fn begin_staging_operation(
        &self,
        owner: Id,
        volume: &str,
        seconds: i64,
    ) -> Result<StagingOperation> {
        ensure!(
            (1..=660).contains(&seconds),
            "staging operation deadline budget"
        );
        let id: Id = rand::random();
        self.pool.get().await?.execute("INSERT INTO staging_operations(id,owner,volume,expires_at) VALUES($1,$2,$3,clock_timestamp()+($4::bigint*interval '1 second'))", &[&id.as_slice(),&owner.as_slice(),&volume,&seconds]).await?;
        Ok(StagingOperation {
            id,
            owner,
            volume: volume.into(),
        })
    }
    pub async fn hold_staging_original(
        &self,
        operation: &StagingOperation,
        intent: &BlobIntent,
    ) -> Result<()> {
        ensure!(
            intent.owner == operation.owner && intent.volume == operation.volume,
            "staging operation storage mismatch"
        );
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        ensure!(tx.query_opt("SELECT id FROM staging_operations WHERE id=$1 AND owner=$2 AND volume=$3 AND expires_at>clock_timestamp() FOR SHARE", &[&operation.id.as_slice(),&operation.owner.as_slice(),&operation.volume]).await?.is_some(),"staging operation expired or closed");
        let row = tx
            .query_opt(
                "SELECT generation,owner,volume,state FROM blob_intents WHERE hash=$1 FOR SHARE",
                &[&intent.key.hash.as_slice()],
            )
            .await?
            .context("staging original disappeared")?;
        ensure!(
            row.get::<_, Vec<u8>>(0) == intent.key.generation
                && row.get::<_, Vec<u8>>(1) == operation.owner
                && row.get::<_, String>(2) == operation.volume
                && row.get::<_, String>(3) == "available",
            "staging original generation changed"
        );
        ensure!(
            !tx.query_one(
                "SELECT EXISTS(SELECT 1 FROM blob_reclaims WHERE hash=$1)",
                &[&intent.key.hash.as_slice()]
            )
            .await?
            .get::<_, bool>(0),
            "staging original reclaim pending"
        );
        tx.execute("INSERT INTO staging_operation_blobs(operation,hash,generation) VALUES($1,$2,$3) ON CONFLICT(operation,hash) DO UPDATE SET generation=EXCLUDED.generation", &[&operation.id.as_slice(),&intent.key.hash.as_slice(),&intent.key.generation.as_slice()]).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn end_staging_operation(&self, operation: &StagingOperation) -> Result<()> {
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        // Synchronize with a late hold attachment before waking queued cleanup.
        tx.query_opt(
            "SELECT id FROM staging_operations WHERE id=$1 FOR UPDATE",
            &[&operation.id.as_slice()],
        )
        .await?;
        tx.execute("UPDATE blob_gc_queue q SET due_at=LEAST(q.due_at,clock_timestamp()) FROM staging_operation_blobs b WHERE b.operation=$1 AND q.hash=b.hash AND q.generation=b.generation", &[&operation.id.as_slice()]).await?;
        tx.execute(
            "DELETE FROM staging_operations WHERE id=$1",
            &[&operation.id.as_slice()],
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
