//! Privileged policy preparation, separate from read-only runtime validation.
use super::ProtocolFacade;
use anyhow::{Result, ensure};
use sha2::{Digest, Sha256};
use std::sync::atomic::Ordering;

const STANDALONE_BODY: &str = " SELECT source_event IS NULL ";
const POLICY_VERSION: i64 = 1;

struct Policy {
    body: String,
    language: &'static str,
    mode: &'static str,
    sql: String,
}

fn expected_policy(original_ready: bool) -> Policy {
    if original_ready {
        Policy {
            body: manga_node::platform_bridge::visibility_function_body(),
            language: "plpgsql",
            mode: "canonical",
            sql: manga_node::platform_bridge::visibility_function_sql(),
        }
    } else {
        Policy {
            body: STANDALONE_BODY.to_owned(),
            language: "sql",
            mode: "native-only",
            sql: format!(
                "CREATE OR REPLACE FUNCTION platform.protocol_source_is_visible(source_event TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $${STANDALONE_BODY}$$;"
            ),
        }
    }
}

// Check the implementation and all execution-relevant function attributes, not
// just its existence or a writable version marker. The text signature lookup is
// schema-qualified; built-in catalog objects cannot be shadowed by search_path.
const INSTALLED_POLICY: &str = "
SELECT p.prosrc, l.lanname,
       COALESCE(p.prokind = 'f'
       AND p.prorettype = 'pg_catalog.bool'::pg_catalog.regtype
       AND p.proargnames = ARRAY['source_event']::pg_catalog.text[]
       AND p.proargmodes IS NULL AND p.proallargtypes IS NULL
       AND p.pronargdefaults = 0 AND p.provariadic = 0
       AND NOT p.proretset AND NOT p.proisstrict
       AND NOT p.prosecdef AND NOT p.proleakproof
       AND p.provolatile = 's' AND p.proparallel = 'u'
       AND p.proconfig IS NULL AND p.prosupport = 0 AND p.probin IS NULL
       AND pg_catalog.has_function_privilege(p.oid, 'EXECUTE'), FALSE)
FROM pg_catalog.pg_proc p
JOIN pg_catalog.pg_language l ON l.oid = p.prolang
WHERE p.oid = pg_catalog.to_regprocedure(
    'platform.protocol_source_is_visible(pg_catalog.text)')";

fn policy_matches(expected: &Policy, actual: Option<(&str, &str, bool)>) -> bool {
    actual.is_some_and(|(body, language, attributes)| {
        body == expected.body && language == expected.language && attributes
    })
}

fn attestation_matches(expected: &Policy, actual: Option<(i64, &str, &[u8])>) -> bool {
    actual.is_some_and(|(version, mode, checksum)| {
        version == POLICY_VERSION
            && mode == expected.mode
            && checksum == Sha256::digest(expected.sql.as_bytes()).as_slice()
    })
}

async fn verify_function<C: tokio_postgres::GenericClient + Sync>(
    client: &C,
    expected: &Policy,
) -> Result<()> {
    let row = client.query_opt(INSTALLED_POLICY, &[]).await?;
    let actual = row.as_ref().map(|row| {
        (
            row.get::<_, &str>(0),
            row.get::<_, &str>(1),
            row.get::<_, bool>(2),
        )
    });
    ensure!(
        policy_matches(expected, actual),
        "visibility policy is missing or mismatched; run explicit platform migrate with the preparation role"
    );
    Ok(())
}

async fn verify_policy<C: tokio_postgres::GenericClient + Sync>(
    client: &C,
    expected: &Policy,
) -> Result<()> {
    let row = client
        .query_opt(
            "SELECT version,mode,checksum FROM platform.protocol_visibility_policy WHERE singleton",
            &[],
        )
        .await?;
    let actual = row.as_ref().map(|row| {
        (
            row.get::<_, i64>(0),
            row.get::<_, &str>(1),
            row.get::<_, &[u8]>(2),
        )
    });
    ensure!(
        attestation_matches(expected, actual),
        "visibility policy preparation is missing or mismatched; run explicit platform migrate with the preparation role"
    );
    // An attestation alone cannot bless a subsequently replaced function.
    verify_function(client, expected).await
}

impl ProtocolFacade {
    /// Privileged preparation only: explicit `migrate` or disposable fixtures.
    /// Never call this from verify/serve or from a runtime recovery worker.
    pub async fn install_visibility_function(&self) -> Result<()> {
        let expected = expected_policy(self.is_ready());
        let mut client = self.repository.pool.get().await?;
        let tx = client.transaction().await?;
        tx.batch_execute("SET LOCAL statement_timeout = '60s'; SET LOCAL lock_timeout = '3s'; SET LOCAL idle_in_transaction_session_timeout = '60s'").await?;
        // Same lock as PgStore::migrate: preparations cannot overlap migrations.
        tx.query_one("SELECT pg_advisory_xact_lock(697241114)", &[])
            .await?;
        tx.batch_execute(&expected.sql).await?;
        // Refuse any retained unsafe execution attributes before attesting.
        verify_function(&*tx, &expected).await?;
        let checksum = Sha256::digest(expected.sql.as_bytes()).to_vec();
        tx.execute(
            "INSERT INTO platform.protocol_visibility_policy(singleton,version,mode,checksum) VALUES(TRUE,$1,$2,$3) ON CONFLICT(singleton) DO UPDATE SET version=EXCLUDED.version,mode=EXCLUDED.mode,checksum=EXCLUDED.checksum,prepared_at=clock_timestamp()",
            &[&POLICY_VERSION, &expected.mode, &checksum],
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Verify exact canonical/fallback policy without DDL, DML, or ownership.
    pub async fn verify_visibility_function(&self) -> Result<()> {
        let expected = expected_policy(self.is_ready());
        let result = async {
            let mut client = self.repository.pool.get().await?;
            let tx = client
                .build_transaction()
                .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await?;
            verify_policy(&*tx, &expected).await?;
            tx.commit().await?;
            Ok(())
        }
        .await;
        if result.is_err() {
            self.schema_ready.store(false, Ordering::Release);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_preparation_and_verification_share_original_policy() {
        let expected = expected_policy(true);
        assert_eq!(expected.language, "plpgsql");
        assert_eq!(
            expected.sql,
            manga_node::platform_bridge::visibility_function_sql()
        );
        assert!(expected.sql.contains(&format!("$bridge${}$bridge$", expected.body)));
        assert!(policy_matches(
            &expected,
            Some((&expected.body, "plpgsql", true))
        ));
    }

    #[test]
    fn missing_body_language_or_execution_attribute_drift_fails_closed() {
        for original_ready in [false, true] {
            let expected = expected_policy(original_ready);
            assert!(!policy_matches(&expected, None));
            assert!(!policy_matches(
                &expected,
                Some((" SELECT TRUE ", expected.language, true))
            ));
            assert!(!policy_matches(
                &expected,
                Some((&expected.body, "untrusted_language", true))
            ));
            assert!(!policy_matches(
                &expected,
                Some((&expected.body, expected.language, false))
            ));
        }
    }

    #[test]
    fn standalone_cannot_reuse_a_previously_installed_canonical_policy() {
        let canonical = expected_policy(true);
        let standalone = expected_policy(false);
        assert!(!policy_matches(
            &standalone,
            Some((&canonical.body, canonical.language, true))
        ));
        assert!(!policy_matches(
            &canonical,
            Some((&standalone.body, standalone.language, true))
        ));
        assert_eq!(standalone.body, " SELECT source_event IS NULL ");
        assert!(policy_matches(
            &standalone,
            Some((&standalone.body, "sql", true))
        ));
    }

    #[test]
    fn attestation_must_match_version_mode_and_exact_prepared_sql() {
        for ready in [false, true] {
            let expected = expected_policy(ready);
            let checksum = Sha256::digest(expected.sql.as_bytes());
            assert!(attestation_matches(
                &expected,
                Some((POLICY_VERSION, expected.mode, &checksum))
            ));
            assert!(!attestation_matches(&expected, None));
            for version in [0, POLICY_VERSION + 1] {
                assert!(!attestation_matches(
                    &expected,
                    Some((version, expected.mode, &checksum))
                ));
            }
            assert!(!attestation_matches(
                &expected,
                Some((POLICY_VERSION, "other-mode", &checksum))
            ));
            assert!(!attestation_matches(
                &expected,
                Some((POLICY_VERSION, expected.mode, &[0; 32]))
            ));
        }
    }
}
