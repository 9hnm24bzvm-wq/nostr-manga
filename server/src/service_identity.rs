//! Durable first-key remembrance, independent of admission/replication permissions.
use crate::{
    db::{Id, Repository},
    endpoint_policy::VerifiedEndpoint,
};
use anyhow::{Result, ensure};
#[derive(Debug, thiserror::Error)]
#[error("endpoint now proves a different service key")]
pub struct IdentityChanged;
#[derive(Debug, thiserror::Error)]
#[error("remembered endpoint budget exhausted")]
pub struct IdentityBudget;
impl Repository {
    pub async fn remembered_service_key(&self, endpoint: &str) -> Result<Option<Id>> {
        self.pool
            .get()
            .await?
            .query_opt(
                "SELECT service_key FROM remembered_service_identities WHERE endpoint=$1",
                &[&endpoint],
            )
            .await?
            .map(|r| {
                r.get::<_, Vec<u8>>(0)
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("invalid remembered service key"))
            })
            .transpose()
    }
    /// Only a live possession-verified endpoint can enter this method.
    pub async fn remember_service_identity(&self, endpoint: &VerifiedEndpoint) -> Result<bool> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        tx.query_one("SELECT pg_advisory_xact_lock(hashtextextended('manga:remembered-service-identities:v1',0))",&[]).await?;
        let known=tx.query_opt("SELECT service_key FROM remembered_service_identities WHERE endpoint=$1 FOR UPDATE",&[&endpoint.origin()]).await?;
        let first = known.is_none();
        if let Some(row) = known {
            ensure!(
                row.get::<_, Vec<u8>>(0) == endpoint.service_key(),
                IdentityChanged
            );
            tx.execute("UPDATE remembered_service_identities SET last_verified_at=clock_timestamp() WHERE endpoint=$1",&[&endpoint.origin()]).await?;
        } else {
            let count: i64 = tx
                .query_one("SELECT COUNT(*) FROM remembered_service_identities", &[])
                .await?
                .get(0);
            ensure!(count < 10000, IdentityBudget);
            tx.execute(
                "INSERT INTO remembered_service_identities(endpoint,service_key) VALUES($1,$2)",
                &[&endpoint.origin(), &endpoint.service_key().as_slice()],
            )
            .await?;
        }
        tx.commit().await?;
        Ok(first)
    }
}
