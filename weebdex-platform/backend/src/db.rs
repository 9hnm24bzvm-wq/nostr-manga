use crate::error::{ApiError, ApiResult};
use anyhow::{Result, ensure};
use deadpool_postgres::{Object, Pool};
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct PgStore {
    pool: Pool,
}

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

impl PgStore {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
    pub fn pool(&self) -> &Pool {
        &self.pool
    }

    pub async fn client(&self) -> ApiResult<Object> {
        self.pool.get().await.map_err(ApiError::from)
    }

    pub async fn health(&self) -> ApiResult<()> {
        self.client().await?.simple_query("SELECT 1").await?;
        Ok(())
    }

    /// Explicit CLI operation. Protocol's 52 checksummed migrations are untouched.
    pub async fn migrate(&self, migrations: &[Migration]) -> Result<()> {
        ensure!(
            migrations
                .windows(2)
                .all(|pair| pair[0].version < pair[1].version),
            "platform migration versions must be ordered and unique"
        );
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        // Migration-only budget, bounded and local to this transaction. Normal
        // repository statements retain the original 5s/1s production limits.
        tx.batch_execute("SET LOCAL statement_timeout = '60s'; SET LOCAL lock_timeout = '3s'; SET LOCAL idle_in_transaction_session_timeout = '60s'").await?;
        tx.query_one("SELECT pg_advisory_xact_lock(697241114)", &[])
            .await?;
        tx.batch_execute("CREATE SCHEMA IF NOT EXISTS platform; CREATE TABLE IF NOT EXISTS platform.schema_migrations(version BIGINT PRIMARY KEY,name TEXT NOT NULL,checksum BYTEA NOT NULL,applied_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp());").await?;
        let rows = tx
            .query(
                "SELECT version,name,checksum FROM platform.schema_migrations ORDER BY version",
                &[],
            )
            .await?;
        ensure!(
            rows.len() <= migrations.len(),
            "platform schema is newer than this binary"
        );
        for (row, expected) in rows.iter().zip(migrations) {
            ensure!(
                row.get::<_, i64>(0) == expected.version
                    && row.get::<_, String>(1) == expected.name
                    && row.get::<_, Vec<u8>>(2) == Sha256::digest(expected.sql.as_bytes()).to_vec(),
                "platform migration history mismatch"
            );
        }
        for migration in &migrations[rows.len()..] {
            tx.batch_execute(migration.sql).await?;
            let checksum = Sha256::digest(migration.sql.as_bytes()).to_vec();
            tx.execute(
                "INSERT INTO platform.schema_migrations(version,name,checksum) VALUES($1,$2,$3)",
                &[&migration.version, &migration.name, &checksum],
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn verify_schema(&self, migrations: &[Migration]) -> Result<()> {
        let rows = self
            .pool
            .get()
            .await?
            .query(
                "SELECT version,name,checksum FROM platform.schema_migrations ORDER BY version",
                &[],
            )
            .await?;
        ensure!(
            rows.len() == migrations.len(),
            "platform migrations are required"
        );
        for (row, expected) in rows.iter().zip(migrations) {
            ensure!(
                row.get::<_, i64>(0) == expected.version
                    && row.get::<_, String>(1) == expected.name
                    && row.get::<_, Vec<u8>>(2) == Sha256::digest(expected.sql.as_bytes()).to_vec(),
                "platform migration checksum/version mismatch"
            );
        }
        Ok(())
    }
}
