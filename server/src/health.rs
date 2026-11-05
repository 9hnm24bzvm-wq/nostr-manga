//! Generation-fenced negative health observations. A quarantined available blob
//! is not an abandoned writing intent: no bytes or capacity are removed/refunded.
use crate::{db::Repository, storage::BlobKey};
use anyhow::{Result, ensure};
impl Repository {
    pub(crate) async fn quarantine_withdrawn(
        &self,
        key: &BlobKey,
        reason: &str,
        receipt: &crate::storage::PublicWithdrawal,
    ) -> Result<bool> {
        ensure!(
            receipt.key() == key,
            "withdrawal observation generation mismatch"
        );
        ensure!(
            matches!(
                reason,
                "read_verification_failed" | "write_verification_failed" | "audit_failed"
            ),
            "unknown quarantine reason"
        );
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row = tx
            .query_opt(
                "SELECT generation,state FROM blob_intents WHERE hash=$1 FOR UPDATE",
                &[&key.hash.as_slice()],
            )
            .await?;
        let Some(row) = row else { return Ok(false) };
        if row.get::<_, Vec<u8>>(0) != key.generation || row.get::<_, String>(1) != "available" {
            return Ok(false);
        }
        if let Some(held)=tx.query_opt("SELECT root_identity FROM public_export_holds WHERE hash=$1 AND generation=$2 FOR UPDATE",&[&key.hash.as_slice(),&key.generation.as_slice()]).await? {
            ensure!(held.get::<_,Option<Vec<u8>>>(0).as_deref()==Some(receipt.root_identity().as_slice()),"quarantine withdrawal belongs to another physical root");
            let token=rand::random::<[u8;16]>();
            tx.execute("UPDATE public_export_holds SET state='draining',withdrawal=$3,updated_at=clock_timestamp() WHERE hash=$1 AND generation=$2",&[&key.hash.as_slice(),&key.generation.as_slice(),&token.as_slice()]).await?;
        }
        let changed=tx.execute("INSERT INTO quarantined_blobs(hash,generation,reason) VALUES($1,$2,$3) ON CONFLICT(hash,generation) DO NOTHING",&[&key.hash.as_slice(),&key.generation.as_slice(),&reason]).await?==1;
        if changed {
            tx.execute(
                "UPDATE storage_health_epoch SET epoch=epoch+1 WHERE singleton",
                &[],
            )
            .await?;
        }
        tx.commit().await?;
        Ok(changed)
    }
    pub async fn blob_quarantined(&self, key: &BlobKey) -> Result<bool> {
        Ok(self
            .pool
            .get()
            .await?
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM quarantined_blobs WHERE hash=$1 AND generation=$2)",
                &[&key.hash.as_slice(), &key.generation.as_slice()],
            )
            .await?
            .get(0))
    }
    pub async fn storage_health_epoch(&self) -> Result<i64> {
        Ok(self
            .pool
            .get()
            .await?
            .query_one(
                "SELECT epoch FROM storage_health_epoch WHERE singleton",
                &[],
            )
            .await?
            .get(0))
    }
}

impl Repository {
    pub(crate) async fn finish_verified_repair(
        &self,
        owner: crate::db::Id,
        volume: &str,
        peer: &crate::peers::Peer,
        receipt: &crate::storage::DurableReceipt,
    ) -> Result<bool> {
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        let key = receipt.key();
        let policy=tx.query_one("SELECT allowed,version,endpoint FROM approved_peers WHERE service_key=$1 FOR SHARE",&[&peer.key.as_slice()]).await?;
        ensure!(
            policy.get::<_, bool>(0)
                && policy.get::<_, i64>(1) == peer.version
                && policy.get::<_, String>(2) == peer.endpoint,
            "repair peer authority changed"
        );
        let row=tx.query_one("SELECT generation,owner,volume,state,size FROM blob_intents WHERE hash=$1 FOR UPDATE",&[&key.hash.as_slice()]).await?;
        ensure!(
            row.get::<_, Vec<u8>>(0) == key.generation
                && row.get::<_, Vec<u8>>(1) == owner
                && row.get::<_, String>(2) == volume
                && row.get::<_, String>(3) == "available"
                && row.get::<_, i64>(4) == receipt.size() as i64,
            "repair generation/owner/size changed"
        );
        ensure!(
            !tx.query_one(
                "SELECT EXISTS(SELECT 1 FROM blob_reclaims WHERE hash=$1 AND generation=$2)",
                &[&key.hash.as_slice(), &key.generation.as_slice()]
            )
            .await?
            .get::<_, bool>(0),
            "object is being reclaimed"
        );
        let changed = tx
            .execute(
                "DELETE FROM quarantined_blobs WHERE hash=$1 AND generation=$2",
                &[&key.hash.as_slice(), &key.generation.as_slice()],
            )
            .await?
            == 1;
        if changed {
            tx.execute("UPDATE blob_intents SET verified_at=clock_timestamp() WHERE hash=$1 AND generation=$2",&[&key.hash.as_slice(),&key.generation.as_slice()]).await?;
            tx.execute(
                "UPDATE storage_health_epoch SET epoch=epoch+1 WHERE singleton",
                &[],
            )
            .await?;
        }
        tx.commit().await?;
        Ok(changed)
    }
}
