//! Privileged preparation is explicit; runtime checks catalog state without DDL.
use super::ProtocolFacade;
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};

const POLICY_VERSION: i64 = 1;
const NATIVE_BODY: &str = " SELECT source_event IS NULL ";

struct Policy {
    mode: &'static str,
    language: &'static str,
    body: String,
    sql: String,
}

fn expected_policy(original_ready: bool) -> Policy {
    if original_ready {
        Policy {
            mode: "canonical",
            language: "plpgsql",
            body: manga_node::platform_bridge::visibility_function_body(),
            sql: manga_node::platform_bridge::visibility_function_sql(),
        }
    } else {
        Policy {
            mode: "native-only",
            language: "sql",
            body: NATIVE_BODY.to_owned(),
            sql: format!(
                "CREATE OR REPLACE FUNCTION platform.protocol_source_is_visible(source_event TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE SECURITY INVOKER SET search_path = pg_catalog, public, pg_temp AS $bridge${NATIVE_BODY}$bridge$;"
            ),
        }
    }
}

impl ProtocolFacade {
    /// Called only by the explicit owner-role `migrate` action or fixture setup.
    /// A failed original-schema verification selects the native-only policy;
    /// later availability requires another deliberate privileged preparation.
    pub async fn prepare_visibility_function(&self) -> Result<()> {
        let original_ready = self.verify_schema().await.is_ok();
        let policy = expected_policy(original_ready);
        let checksum = Sha256::digest(policy.sql.as_bytes()).to_vec();
        let mut client = self.repository.pool.get().await?;
        let tx = client.transaction().await?;
        tx.batch_execute("SET LOCAL statement_timeout = '60s'; SET LOCAL lock_timeout = '3s'; SET LOCAL idle_in_transaction_session_timeout = '60s'").await?;
        tx.query_one("SELECT pg_advisory_xact_lock(697241114)", &[])
            .await?;
        tx.batch_execute(&policy.sql).await?;
        tx.execute(
            "INSERT INTO platform.protocol_visibility_policy(singleton,version,mode,checksum) VALUES(TRUE,$1,$2,$3) ON CONFLICT(singleton) DO UPDATE SET version=EXCLUDED.version,mode=EXCLUDED.mode,checksum=EXCLUDED.checksum,prepared_at=clock_timestamp()",
            &[&POLICY_VERSION, &policy.mode, &checksum],
        ).await?;
        tx.commit().await?;
        Ok(())
    }

    /// `verify` and `serve` need only SELECT on the attestation/catalogs and
    /// EXECUTE on the prepared function. Metadata alone never blesses its body.
    pub async fn verify_visibility_function(&self) -> Result<()> {
        let policy = expected_policy(self.is_ready());
        let mut client = self.repository.pool.get().await?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await?;
        let row = tx.query_opt(
            "SELECT version,mode,checksum FROM platform.protocol_visibility_policy WHERE singleton",
            &[],
        ).await?.context("visibility policy preparation is required; run platform migrate with the owner role")?;
        ensure!(
            row.get::<_, i64>(0) == POLICY_VERSION
                && row.get::<_, String>(1) == policy.mode
                && row.get::<_, Vec<u8>>(2) == Sha256::digest(policy.sql.as_bytes()).to_vec(),
            "visibility policy version/mode/checksum mismatch; run platform migrate with the owner role"
        );
        let function = tx.query_opt(
            "SELECT p.prosrc,l.lanname,p.provolatile::text,p.prosecdef,p.proisstrict,p.proparallel::text,p.proretset,p.prorettype='boolean'::regtype,p.proconfig,p.proargnames,p.prokind::text,p.pronargdefaults,p.provariadic=0,p.proargmodes IS NULL,p.proleakproof,p.prosupport=0,p.probin IS NULL FROM pg_catalog.pg_proc p JOIN pg_catalog.pg_language l ON l.oid=p.prolang WHERE p.oid=pg_catalog.to_regprocedure('platform.protocol_source_is_visible(text)')",
            &[],
        ).await?.context("prepared visibility function is missing")?;
        ensure!(
            function.get::<_, String>(0) == policy.body
                && function.get::<_, String>(1) == policy.language
                && function.get::<_, String>(2) == "s"
                && !function.get::<_, bool>(3)
                && !function.get::<_, bool>(4)
                && function.get::<_, String>(5) == "u"
                && !function.get::<_, bool>(6)
                && function.get::<_, bool>(7)
                && function.get::<_, Option<Vec<String>>>(8)
                    == Some(vec!["search_path=pg_catalog, public, pg_temp".to_owned()])
                && function.get::<_, Option<Vec<String>>>(9)
                    == Some(vec!["source_event".to_owned()])
                && function.get::<_, String>(10) == "f"
                && function.get::<_, i16>(11) == 0
                && function.get::<_, bool>(12)
                && function.get::<_, bool>(13)
                && !function.get::<_, bool>(14)
                && function.get::<_, bool>(15)
                && function.get::<_, bool>(16),
            "installed visibility function policy mismatch; run platform migrate with the owner role"
        );
        ensure!(
            tx.query_one("SELECT platform.protocol_source_is_visible(NULL)", &[])
                .await?
                .get::<_, bool>(0),
            "prepared visibility function rejected native resources"
        );
        tx.commit().await?;
        Ok(())
    }
}
