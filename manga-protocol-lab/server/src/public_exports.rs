//! Durable external-reader inhibitors. No expiry or process death releases them.
//! The managed export/drain adapter is a separate gate, still under construction.
use crate::{
    db::{Id, Repository},
    storage::BlobKey,
};
use anyhow::{Context, Result, ensure};
impl Repository {
    pub async fn public_export_held(&self, key: &BlobKey) -> Result<bool> {
        Ok(self
            .pool
            .get()
            .await?
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM public_export_holds WHERE hash=$1 AND generation=$2)",
                &[&key.hash.as_slice(), &key.generation.as_slice()],
            )
            .await?
            .get(0))
    }
    pub async fn reserve_public_export(
        &self,
        key: &BlobKey,
        owner: Id,
        volume: &str,
        mime: &str,
        root_identity: Id,
    ) -> Result<bool> {
        ensure!(
            matches!(mime, "image/png" | "image/jpeg" | "image/webp"),
            "only original image MIME types may export"
        );
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        ensure!(!tx.query_one("SELECT EXISTS(SELECT 1 FROM public_export_roles WHERE owner=$1 AND volume=$2 AND root_identity<>$3)",&[&owner.as_slice(),&volume,&root_identity.as_slice()]).await?.get::<_,bool>(0),"another physical root owns the public export role");
        let row = tx
            .query_opt(
                "SELECT generation,owner,volume,state FROM blob_intents WHERE hash=$1 FOR UPDATE",
                &[&key.hash.as_slice()],
            )
            .await?
            .context("export original unavailable")?;
        ensure!(
            row.get::<_, Vec<u8>>(0) == key.generation
                && row.get::<_, Vec<u8>>(1) == owner
                && row.get::<_, String>(2) == volume
                && row.get::<_, String>(3) == "available",
            "export generation/owner/state changed"
        );
        ensure!(!tx.query_one("SELECT EXISTS(SELECT 1 FROM blob_reclaims WHERE hash=$1 AND generation=$2) OR EXISTS(SELECT 1 FROM quarantined_blobs WHERE hash=$1 AND generation=$2)",&[&key.hash.as_slice(),&key.generation.as_slice()]).await?.get::<_,bool>(0),"quarantined or reclaiming original cannot export");
        let eligible:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM publication_blobs p JOIN events e ON e.id=p.event_id WHERE p.hash=$1 AND p.generation=$2 AND p.mime=$3 AND e.state='hosted' AND NOT EXISTS(SELECT 1 FROM publication_blobs bad JOIN quarantined_blobs q ON q.hash=bad.hash AND q.generation=bad.generation WHERE bad.event_id=e.id) AND NOT EXISTS(SELECT 1 FROM backend_outbox o WHERE o.event_id=e.id AND o.completed_at IS NULL))",&[&key.hash.as_slice(),&key.generation.as_slice(),&mime]).await?.get(0);
        ensure!(
            eligible,
            "no complete projected publication permits this original"
        );
        let inserted=tx.execute("INSERT INTO public_export_holds(hash,generation,owner,volume,mime,root_identity) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(hash) DO NOTHING",&[&key.hash.as_slice(),&key.generation.as_slice(),&owner.as_slice(),&volume,&mime,&root_identity.as_slice()]).await?==1;
        let saved = tx
            .query_one(
                "SELECT generation,owner,volume,mime,root_identity FROM public_export_holds WHERE hash=$1 FOR UPDATE",
                &[&key.hash.as_slice()],
            )
            .await?;
        ensure!(
            saved.get::<_, Vec<u8>>(0) == key.generation
                && saved.get::<_, Vec<u8>>(1) == owner
                && saved.get::<_, String>(2) == volume
                && saved.get::<_, String>(3) == mime
                && saved.get::<_, Option<Vec<u8>>>(4).as_deref() == Some(root_identity.as_slice()),
            "another export generation remains protected"
        );
        tx.execute("UPDATE public_export_holds SET state='creating',withdrawal=NULL,updated_at=clock_timestamp() WHERE hash=$1",&[&key.hash.as_slice()]).await?;
        tx.commit().await?;
        Ok(inserted)
    }
}

#[derive(Clone)]
pub(crate) struct PendingDrain {
    hash: Id,
    generation: [u8; 16],
    token: [u8; 16],
}
impl Repository {
    pub(crate) async fn mark_public_withdrawal(
        &self,
        owner: Id,
        volume: &str,
        receipt: &crate::storage::PublicWithdrawal,
    ) -> Result<bool> {
        let key = receipt.key();
        let token = rand::random::<[u8; 16]>();
        let changed=self.pool.get().await?.execute("UPDATE public_export_holds SET state='draining',withdrawal=$5,updated_at=clock_timestamp() WHERE hash=$1 AND generation=$2 AND owner=$3 AND volume=$4 AND root_identity=$6",&[&key.hash.as_slice(),&key.generation.as_slice(),&owner.as_slice(),&volume,&token.as_slice(),&receipt.root_identity().as_slice()]).await?;
        Ok(changed == 1)
    }
    pub(crate) async fn pending_public_drains(
        &self,
        owner: Id,
        volume: &str,
        root_identity: Id,
    ) -> Result<Vec<PendingDrain>> {
        self.pool.get().await?.query("SELECT hash,generation,withdrawal FROM public_export_holds WHERE owner=$1 AND volume=$2 AND root_identity=$3 AND state='draining' AND withdrawal IS NOT NULL ORDER BY updated_at,hash LIMIT 64",&[&owner.as_slice(),&volume,&root_identity.as_slice()]).await?.into_iter().map(|row|Ok(PendingDrain{
            hash:row.get::<_,Vec<u8>>(0).try_into().map_err(|_|anyhow::anyhow!("invalid export hash"))?,
            generation:row.get::<_,Vec<u8>>(1).try_into().map_err(|_|anyhow::anyhow!("invalid export generation"))?,
            token:row.get::<_,Vec<u8>>(2).try_into().map_err(|_|anyhow::anyhow!("invalid withdrawal token"))?,
        })).collect()
    }
    pub(crate) async fn release_public_drains(
        &self,
        owner: Id,
        volume: &str,
        root_identity: Id,
        pending: Vec<PendingDrain>,
        _receipt: crate::serving::ReaderDrain,
    ) -> Result<usize> {
        ensure!(pending.len() <= 64, "drain batch budget");
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let mut released = 0;
        for item in pending {
            released+=tx.execute("DELETE FROM public_export_holds WHERE hash=$1 AND generation=$2 AND owner=$3 AND volume=$4 AND state='draining' AND withdrawal=$5 AND root_identity=$6",&[&item.hash.as_slice(),&item.generation.as_slice(),&owner.as_slice(),&volume,&item.token.as_slice(),&root_identity.as_slice()]).await? as usize;
        }
        tx.commit().await?;
        Ok(released)
    }
}

impl Repository {
    pub(crate) async fn activate_public_export(
        &self,
        root_identity: Id,
        receipt: &crate::storage::DurableReceipt,
        mime: &str,
        mutation: &crate::storage::Mutation,
    ) -> Result<()> {
        let key = receipt.key();
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        // Prevent health changes during the final visibility check/link. The
        // caller already owns the hash mutation guard and a durable creating hold.
        tx.query_one(
            "SELECT epoch FROM storage_health_epoch WHERE singleton FOR SHARE",
            &[],
        )
        .await?;
        let blob = tx
            .query_one(
                "SELECT generation,state FROM blob_intents WHERE hash=$1 FOR UPDATE",
                &[&key.hash.as_slice()],
            )
            .await?;
        ensure!(
            blob.get::<_, Vec<u8>>(0) == key.generation && blob.get::<_, String>(1) == "available",
            "export generation/state changed"
        );
        let held=tx.query_one("SELECT generation,root_identity,state,mime FROM public_export_holds WHERE hash=$1 FOR UPDATE",&[&key.hash.as_slice()]).await?;
        ensure!(
            held.get::<_, Vec<u8>>(0) == key.generation
                && held.get::<_, Option<Vec<u8>>>(1).as_deref() == Some(root_identity.as_slice())
                && held.get::<_, String>(2) == "creating"
                && held.get::<_, String>(3) == mime,
            "export creating hold changed"
        );
        let eligible=tx.query_opt("SELECT e.id FROM events e JOIN publication_blobs p ON p.event_id=e.id WHERE p.hash=$1 AND p.generation=$2 AND p.mime=$3 AND e.state='hosted' AND NOT EXISTS(SELECT 1 FROM publication_blobs bad JOIN quarantined_blobs q ON q.hash=bad.hash AND q.generation=bad.generation WHERE bad.event_id=e.id) AND NOT EXISTS(SELECT 1 FROM backend_outbox o WHERE o.event_id=e.id AND o.completed_at IS NULL) ORDER BY e.id LIMIT 1 FOR SHARE OF e",&[&key.hash.as_slice(),&key.generation.as_slice(),&mime]).await?;
        ensure!(
            eligible.is_some(),
            "publication no longer eligible for public export"
        );
        mutation.link_public(receipt, mime).await?;
        tx.execute("UPDATE public_export_holds SET state='active',updated_at=clock_timestamp() WHERE hash=$1",&[&key.hash.as_slice()]).await?;
        tx.commit().await?;
        Ok(())
    }
}

impl Repository {
    pub(crate) async fn begin_public_withdrawal(
        &self,
        key: &BlobKey,
        root: Id,
        bump: bool,
    ) -> Result<()> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        if let Some(row)=tx.query_opt("SELECT root_identity FROM public_export_holds WHERE hash=$1 AND generation=$2 FOR UPDATE",&[&key.hash.as_slice(),&key.generation.as_slice()]).await? {
            ensure!(row.get::<_,Option<Vec<u8>>>(0).as_deref()==Some(root.as_slice()),"withdrawal journal belongs to another physical root");
            tx.execute("UPDATE public_export_holds SET state='creating',withdrawal=NULL,updated_at=clock_timestamp() WHERE hash=$1 AND generation=$2",&[&key.hash.as_slice(),&key.generation.as_slice()]).await?;
        }
        tx.query_one(
            "SELECT enqueue_public_export($1,$2)",
            &[&key.hash.as_slice(), &bump],
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
