//! Logical upload/publication authority, independent of physical file ownership.
use crate::{
    blobs::BlobIntent,
    db::{Id, Repository},
    upload::UploadStore,
};
use anyhow::{Context, Result, ensure};

#[derive(Debug, thiserror::Error)]
pub enum PublisherDenied {
    #[error("publisher not admitted")]
    NotAdmitted,
    #[error("publisher permission changed during operation")]
    Changed,
    #[error("publisher lacks current staging grants")]
    MissingGrant,
}

#[derive(Clone, Debug)]
pub struct PublisherPermit {
    pub(crate) publisher: Id,
    pub(crate) version: i64,
}
impl Repository {
    /// Operator-authenticated adapters only; federation never grants login rights.
    pub async fn set_publisher_admission(&self, publisher: Id, allowed: bool) -> Result<()> {
        self.pool.get().await?.execute("INSERT INTO publisher_permissions(publisher,allowed) VALUES($1,$2) ON CONFLICT(publisher) DO UPDATE SET allowed=EXCLUDED.allowed", &[&publisher.as_slice(),&allowed]).await?;
        Ok(())
    }
    pub async fn publisher_permit(&self, publisher: Id) -> Result<PublisherPermit> {
        let row = self
            .pool
            .get()
            .await?
            .query_opt(
                "SELECT version FROM publisher_permissions WHERE publisher=$1 AND allowed",
                &[&publisher.as_slice()],
            )
            .await?
            .ok_or(PublisherDenied::NotAdmitted)?;
        Ok(PublisherPermit {
            publisher,
            version: row.get(0),
        })
    }
}
pub(crate) async fn check_publisher_tx(
    tx: &deadpool_postgres::Transaction<'_>,
    permit: &PublisherPermit,
) -> Result<()> {
    let row = tx
        .query_opt(
            "SELECT allowed,version FROM publisher_permissions WHERE publisher=$1 FOR SHARE",
            &[&permit.publisher.as_slice()],
        )
        .await?
        .ok_or(PublisherDenied::NotAdmitted)?;
    if !row.get::<_, bool>(0) || row.get::<_, i64>(1) != permit.version {
        return Err(PublisherDenied::Changed.into());
    }
    Ok(())
}
pub(crate) async fn check_grants_tx(
    tx: &deadpool_postgres::Transaction<'_>,
    permit: &PublisherPermit,
    hashes: &[Vec<u8>],
) -> Result<()> {
    let rows=tx.query("SELECT g.hash FROM publisher_blob_grants g JOIN blob_intents b ON b.hash=g.hash AND b.generation=g.generation WHERE g.publisher=$1 AND g.permission_version=$2 AND g.hash=ANY($3) AND g.lease_until>clock_timestamp() ORDER BY g.hash FOR SHARE OF g", &[&permit.publisher.as_slice(),&permit.version,&hashes]).await?;
    if rows.len() != hashes.len() {
        return Err(PublisherDenied::MissingGrant.into());
    }
    Ok(())
}
impl UploadStore {
    /// Caller runs this in an owned task. Revocation during I/O leaves only an
    /// ungranted staging object, never a publication or logical access grant.
    pub async fn put_for_publisher(
        &self,
        permit: &PublisherPermit,
        hash: Id,
        bytes: &[u8],
    ) -> Result<BlobIntent> {
        Ok(self
            .put_for_publisher_detailed(permit, hash, bytes)
            .await?
            .intent)
    }
    pub async fn put_for_publisher_detailed(
        &self,
        permit: &PublisherPermit,
        hash: Id,
        bytes: &[u8],
    ) -> Result<crate::upload::PutOutcome> {
        {
            let mut c = self.repo.pool.get().await?;
            let tx = c.transaction().await?;
            check_publisher_tx(&tx, permit).await?;
            tx.commit().await?;
        }
        let outcome = self.put_detailed(hash, bytes).await?;
        let intent = &outcome.intent;
        let mut c = self.repo.pool.get().await?;
        let tx = c.transaction().await?;
        check_publisher_tx(&tx, permit).await?;
        let row=tx.query_opt("SELECT generation,owner,state,staging_until FROM blob_intents WHERE hash=$1 FOR SHARE", &[&hash.as_slice()]).await?.context("publisher blob disappeared")?;
        ensure!(
            row.get::<_, Vec<u8>>(0) == intent.key.generation
                && row.get::<_, Vec<u8>>(1) == self.owner()
                && row.get::<_, String>(2) == "available",
            "publisher blob generation unavailable"
        );
        ensure!(
            !tx.query_one(
                "SELECT EXISTS(SELECT 1 FROM blob_reclaims WHERE hash=$1)",
                &[&hash.as_slice()]
            )
            .await?
            .get::<_, bool>(0),
            "publisher blob reclamation pending"
        );
        tx.execute("INSERT INTO publisher_blob_grants(publisher,hash,generation,permission_version,lease_until) SELECT $1,hash,generation,$2,staging_until FROM blob_intents WHERE hash=$3 ON CONFLICT(publisher,hash) DO UPDATE SET generation=EXCLUDED.generation,permission_version=EXCLUDED.permission_version,lease_until=EXCLUDED.lease_until", &[&permit.publisher.as_slice(),&permit.version,&hash.as_slice()]).await?;
        tx.commit().await?;
        Ok(outcome)
    }
    async fn authorize_blob_read(&self, permit: &PublisherPermit, hash: Id) -> Result<()> {
        let mut c = self.repo.pool.get().await?;
        let tx = c.transaction().await?;
        check_publisher_tx(&tx, permit).await?;
        if permit.publisher != self.owner() {
            check_grants_tx(&tx, permit, &[hash.to_vec()]).await?;
        }
        tx.commit().await?;
        drop(c);
        Ok(())
    }
    pub async fn get_for_publisher(&self, permit: &PublisherPermit, hash: Id) -> Result<Vec<u8>> {
        self.authorize_blob_read(permit, hash).await?;
        self.get(hash, self.owner()).await
    }
    pub async fn download_for_publisher(&self, permit: &PublisherPermit, hash: Id) -> Result<crate::download_budget::Snapshot> {
        self.authorize_blob_read(permit, hash).await?;
        self.download(hash, self.owner()).await
    }
}
