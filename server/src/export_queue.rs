//! Durable, opt-in original projection work. No node config enables this role yet.
use crate::{
    db::{Id, Repository},
    storage::BlobKey,
};
use anyhow::{Context, Result, ensure};
#[derive(Clone)]
pub(crate) struct ExportWork {
    pub key: BlobKey,
    pub version: i64,
}
#[derive(Default, Debug, serde::Serialize)]
pub struct ExportStep {
    pub scanned: usize,
    pub health_scanned: usize,
    pub exported: usize,
    pub withdrawn: usize,
    pub retried: usize,
}
impl Repository {
    pub(crate) async fn configure_public_export_role(
        &self,
        owner: Id,
        volume: &str,
        root: Id,
    ) -> Result<()> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        ensure!(!tx.query_one("SELECT EXISTS(SELECT 1 FROM public_export_holds WHERE owner=$1 AND volume=$2 AND root_identity IS DISTINCT FROM $3)",&[&owner.as_slice(),&volume,&root.as_slice()]).await?.get::<_,bool>(0),"foreign or unbound public holds require offline recovery");
        tx.execute("INSERT INTO public_export_roles(owner,volume,root_identity) VALUES($1,$2,$3) ON CONFLICT(owner,volume) DO NOTHING",&[&owner.as_slice(),&volume,&root.as_slice()]).await?;
        let row=tx.query_one("SELECT root_identity,enabled FROM public_export_roles WHERE owner=$1 AND volume=$2 FOR UPDATE",&[&owner.as_slice(),&volume]).await?;
        ensure!(
            row.get::<_, Vec<u8>>(0) == root && row.get::<_, bool>(1),
            "public export role root changed or paused"
        );
        tx.commit().await?;
        Ok(())
    }
    pub(crate) async fn backfill_public_exports(
        &self,
        owner: Id,
        volume: &str,
        root: Id,
    ) -> Result<usize> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let role=tx.query_opt("SELECT root_identity,enabled,backfill_done,backfill_after FROM public_export_roles WHERE owner=$1 AND volume=$2 FOR UPDATE",&[&owner.as_slice(),&volume]).await?.context("public export role not configured")?;
        ensure!(
            role.get::<_, Vec<u8>>(0) == root && role.get::<_, bool>(1),
            "public export role changed or paused"
        );
        if role.get::<_, bool>(2) {
            return Ok(0);
        }
        let after: Option<Vec<u8>> = role.get(3);
        let rows=tx.query("SELECT b.hash,EXISTS(SELECT 1 FROM publication_blobs p WHERE p.hash=b.hash AND p.mime IN ('image/png','image/jpeg','image/webp')) OR EXISTS(SELECT 1 FROM public_export_holds h WHERE h.hash=b.hash) FROM blob_intents b WHERE b.owner=$1 AND b.volume=$2 AND b.hash>COALESCE($3::bytea,''::bytea) ORDER BY b.hash LIMIT 64",&[&owner.as_slice(),&volume,&after]).await?;
        for row in &rows {
            if row.get::<_, bool>(1) {
                tx.query_one(
                    "SELECT enqueue_public_export($1)",
                    &[&row.get::<_, Vec<u8>>(0)],
                )
                .await?;
            }
        }
        let last = rows.last().map(|r| r.get::<_, Vec<u8>>(0)).or(after);
        tx.execute("UPDATE public_export_roles SET backfill_after=$3,backfill_done=$4 WHERE owner=$1 AND volume=$2",&[&owner.as_slice(),&volume,&last,&(rows.len()<64)]).await?;
        tx.commit().await?;
        Ok(rows.len())
    }
    pub(crate) async fn public_export_work(
        &self,
        owner: Id,
        volume: &str,
        root: Id,
    ) -> Result<Vec<ExportWork>> {
        self.pool.get().await?.query("SELECT q.hash,b.generation,q.version FROM public_export_queue q JOIN blob_intents b ON b.hash=q.hash AND b.generation=q.generation JOIN public_export_roles r ON r.owner=q.owner AND r.volume=q.volume AND r.root_identity=q.root_identity AND r.enabled WHERE q.owner=$1 AND q.volume=$2 AND q.root_identity=$3 AND q.due_at<=statement_timestamp() ORDER BY q.due_at,q.hash LIMIT 16",&[&owner.as_slice(),&volume,&root.as_slice()]).await?.into_iter().map(|row|Ok(ExportWork{key:BlobKey{hash:row.get::<_,Vec<u8>>(0).try_into().map_err(|_|anyhow::anyhow!("invalid export work hash"))?,generation:row.get::<_,Vec<u8>>(1).try_into().map_err(|_|anyhow::anyhow!("invalid export generation"))?},version:row.get(2)})).collect()
    }
    pub(crate) async fn public_export_mime(&self, key: &BlobKey) -> Result<Option<String>> {
        Ok(self.pool.get().await?.query_opt("SELECT p.mime FROM publication_blobs p JOIN events e ON e.id=p.event_id WHERE p.hash=$1 AND p.generation=$2 AND p.mime IN ('image/png','image/jpeg','image/webp') AND e.state='hosted' AND NOT EXISTS(SELECT 1 FROM publication_blobs bad JOIN quarantined_blobs q ON q.hash=bad.hash AND q.generation=bad.generation WHERE bad.event_id=e.id) AND NOT EXISTS(SELECT 1 FROM backend_outbox o WHERE o.event_id=e.id AND o.completed_at IS NULL) ORDER BY e.id LIMIT 1",&[&key.hash.as_slice(),&key.generation.as_slice()]).await?.map(|row|row.get(0)))
    }
    pub(crate) async fn finish_public_export_work(
        &self,
        work: &ExportWork,
        root: Id,
    ) -> Result<()> {
        self.pool
            .get()
            .await?
            .execute(
                "DELETE FROM public_export_queue WHERE hash=$1 AND version=$2 AND root_identity=$3 AND generation=$4",
                &[&work.key.hash.as_slice(), &work.version, &root.as_slice(), &work.key.generation.as_slice()],
            )
            .await?;
        Ok(())
    }
    pub(crate) async fn retry_public_export_work(
        &self,
        work: &ExportWork,
        root: Id,
        error: &str,
    ) -> Result<()> {
        let error: String = error.chars().take(240).collect();
        self.pool.get().await?.execute("UPDATE public_export_queue SET attempts=LEAST(attempts+1,32),due_at=clock_timestamp()+(LEAST(60,1<<LEAST(attempts,6))*interval '1 second'),last_error=$4 WHERE hash=$1 AND version=$2 AND root_identity=$3 AND generation=$5",&[&work.key.hash.as_slice(),&work.version,&root.as_slice(),&error,&work.key.generation.as_slice()]).await?;
        Ok(())
    }
    pub async fn public_export_idle(&self, owner: Id, volume: &str, root: Id) -> Result<bool> {
        Ok(self.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM public_export_roles WHERE owner=$1 AND volume=$2 AND root_identity=$3 AND enabled AND backfill_done) AND NOT EXISTS(SELECT 1 FROM public_export_queue WHERE owner=$1 AND volume=$2 AND root_identity=$3) AND NOT EXISTS(SELECT 1 FROM public_export_health_work WHERE owner=$1 AND volume=$2 AND root_identity=$3)",&[&owner.as_slice(),&volume,&root.as_slice()]).await?.get(0))
    }
}

impl Repository {
    pub(crate) async fn fanout_public_health(
        &self,
        owner: Id,
        volume: &str,
        root: Id,
    ) -> Result<usize> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let Some(job)=tx.query_opt("SELECT h.hash,h.generation,h.after_event,h.current_event,h.after_hash FROM public_export_health_work h JOIN public_export_roles r ON r.owner=h.owner AND r.volume=h.volume AND r.root_identity=h.root_identity AND r.enabled WHERE h.owner=$1 AND h.volume=$2 AND h.root_identity=$3 ORDER BY h.hash,h.generation LIMIT 1 FOR UPDATE OF h SKIP LOCKED",&[&owner.as_slice(),&volume,&root.as_slice()]).await? else {return Ok(0)};
        let hash: Vec<u8> = job.get(0);
        let generation: Vec<u8> = job.get(1);
        let after_event: Option<Vec<u8>> = job.get(2);
        let current: Option<Vec<u8>> = job.get(3);
        let after_hash: Option<Vec<u8>> = job.get(4);
        let current = if let Some(current) = current {
            Some(current)
        } else {
            tx.query_opt("SELECT event_id FROM publication_blobs WHERE hash=$1 AND generation=$2 AND event_id>COALESCE($3::bytea,''::bytea) ORDER BY event_id LIMIT 1",&[&hash,&generation,&after_event]).await?.map(|row|row.get::<_,Vec<u8>>(0))
        };
        let Some(current) = current else {
            tx.execute(
                "DELETE FROM public_export_health_work WHERE hash=$1 AND generation=$2",
                &[&hash, &generation],
            )
            .await?;
            tx.commit().await?;
            return Ok(0);
        };
        let pages=tx.query("SELECT hash FROM publication_blobs WHERE event_id=$1 AND hash>COALESCE($2::bytea,''::bytea) AND mime IN ('image/png','image/jpeg','image/webp') ORDER BY hash LIMIT 64",&[&current,&after_hash]).await?;
        let hashes: Vec<Vec<u8>> = pages.iter().map(|row| row.get(0)).collect();
        if !hashes.is_empty() {
            tx.query(
                "SELECT enqueue_public_export(h) FROM unnest($1::bytea[]) h ORDER BY h",
                &[&hashes],
            )
            .await?;
        }
        if hashes.len() < 64 {
            tx.execute("UPDATE public_export_health_work SET after_event=$3,current_event=NULL,after_hash=NULL WHERE hash=$1 AND generation=$2",&[&hash,&generation,&current]).await?;
        } else {
            tx.execute("UPDATE public_export_health_work SET current_event=$3,after_hash=$4 WHERE hash=$1 AND generation=$2",&[&hash,&generation,&current,&hashes.last().unwrap()]).await?;
        }
        tx.commit().await?;
        Ok(hashes.len())
    }
    pub async fn public_export_pending(&self, owner: Id, volume: &str, root: Id) -> Result<bool> {
        Ok(self.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM public_export_queue WHERE owner=$1 AND volume=$2 AND root_identity=$3) OR EXISTS(SELECT 1 FROM public_export_health_work WHERE owner=$1 AND volume=$2 AND root_identity=$3) OR EXISTS(SELECT 1 FROM public_export_roles WHERE owner=$1 AND volume=$2 AND root_identity=$3 AND NOT backfill_done)",&[&owner.as_slice(),&volume,&root.as_slice()]).await?.get(0))
    }
}
