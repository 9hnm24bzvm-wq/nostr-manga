mod support;
use anyhow::{Result, ensure};
use manga_node::{
    db::{DbConfig, Repository},
    enrollment::{Denied, Policy},
};
use std::time::Duration;
fn id(n: u8) -> [u8; 32] {
    hex::decode(support::public(n)).unwrap().try_into().unwrap()
}
async fn wait_for_blocked_transaction(
    repo: &Repository,
    tx: &tokio_postgres::Transaction<'_>,
) -> Result<()> {
    let blocker: i32 = tx.query_one("SELECT pg_backend_pid()", &[]).await?.get(0);
    tokio::time::timeout(Duration::from_secs(5),async {loop {
        let waiting:bool=repo.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",&[&blocker]).await?.get(0);
        if waiting { return Ok::<_,anyhow::Error>(()); }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }}).await??;
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated PostgreSQL; migration and enrollment transaction races"]
async fn migration_denials_operator_races_and_budget_rollback() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    ensure!(
        dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test"),
        "fixture only"
    );
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    // Recreate a pre-0052 database shape externally, then use the actual migration
    // and enrollment interfaces as the observable upgrade contract.
    repo.pool.get().await?.batch_execute("DROP TRIGGER account_enrollment_denial ON account_permissions; DROP FUNCTION fence_account_enrollment_denial(); DROP TABLE reader_enrollment_budget; ALTER TABLE account_permissions DROP COLUMN enrollment_blocked; DELETE FROM schema_migrations WHERE version=52; INSERT INTO account_permissions(publisher,allowed,version) VALUES(decode(repeat('aa',32),'hex'),FALSE,4),(decode(repeat('bb',32),'hex'),TRUE,2)").await?;
    repo.migrate().await?;
    repo.verify_schema().await?;
    let upgrade_policy = Policy {
        enabled: true,
        ..Policy::default()
    };
    assert!(matches!(
        repo.enroll_reader([0xaa; 32], &upgrade_policy)
            .await
            .unwrap_err()
            .downcast_ref::<Denied>(),
        Some(Denied::Blocked)
    ));
    let existing = repo.enroll_reader([0xbb; 32], &upgrade_policy).await?;
    assert!(!existing.created);
    assert!(existing.admitted);
    assert_eq!(existing.permission_version, 2);
    repo.pool.get().await?.batch_execute("INSERT INTO account_permissions(publisher,allowed) VALUES(decode(repeat('cc',32),'hex'),FALSE); UPDATE account_permissions SET allowed=FALSE WHERE publisher=decode(repeat('bb',32),'hex')").await?;
    for denied in [[0xaa; 32], [0xbb; 32], [0xcc; 32]] {
        assert!(matches!(
            repo.enroll_reader(denied, &upgrade_policy)
                .await
                .unwrap_err()
                .downcast_ref::<Denied>(),
            Some(Denied::Blocked)
        ));
    }
    let policy = Policy {
        enabled: true,
        max_new_accounts_per_hour: 10,
        max_self_enrolled_accounts: 100,
    };
    assert!(matches!(
        repo.enroll_reader(id(2), &Policy::default())
            .await
            .unwrap_err()
            .downcast_ref::<Denied>(),
        Some(Denied::Disabled)
    ));
    // Admission-row race: an uncommitted operator denial must win against a
    // self-enrollment that saw no row before attempting its conflict-safe insert.
    let mut c = repo.pool.get().await?;
    let tx = c.transaction().await?;
    tx.execute("INSERT INTO account_permissions(publisher,allowed,enrollment_blocked) VALUES($1,FALSE,TRUE)",&[&id(2).as_slice()]).await?;
    let r = repo.clone();
    let p = policy.clone();
    let pending = tokio::spawn(async move { r.enroll_reader(id(2), &p).await });
    wait_for_blocked_transaction(&repo, &tx).await?;
    tx.commit().await?;
    drop(c);
    assert!(matches!(
        pending.await?.unwrap_err().downcast_ref::<Denied>(),
        Some(Denied::Blocked)
    ));
    assert!(repo.account_permit(id(2)).await.is_err());
    // A failed budget update rolls the newly inserted account back too.
    repo.pool.get().await?.batch_execute("ALTER TABLE reader_enrollment_budget ADD CONSTRAINT fixture_budget_failure CHECK(total_created=0) NOT VALID").await?;
    assert!(repo.enroll_reader(id(3), &policy).await.is_err());
    assert!(repo.account_permit(id(3)).await.is_err());
    repo.pool
        .get()
        .await?
        .batch_execute(
            "ALTER TABLE reader_enrollment_budget DROP CONSTRAINT fixture_budget_failure",
        )
        .await?;
    assert!(repo.enroll_reader(id(3), &policy).await?.created);
    // Existing row: FOR UPDATE observes an operator's completed revocation.
    let mut c = repo.pool.get().await?;
    let tx = c.transaction().await?;
    tx.execute("UPDATE account_permissions SET allowed=FALSE,enrollment_blocked=TRUE,version=version+1 WHERE publisher=$1",&[&id(3).as_slice()]).await?;
    let r = repo.clone();
    let p = policy.clone();
    let pending = tokio::spawn(async move { r.enroll_reader(id(3), &p).await });
    wait_for_blocked_transaction(&repo, &tx).await?;
    tx.commit().await?;
    drop(c);
    assert!(matches!(
        pending.await?.unwrap_err().downcast_ref::<Denied>(),
        Some(Denied::Blocked)
    ));
    assert!(repo.account_permit(id(3)).await.is_err());
    // A failed insertion and later revocation consumed only one lifetime slot.
    let limited = Policy {
        max_self_enrolled_accounts: 2,
        ..policy.clone()
    };
    assert!(repo.enroll_reader(id(4), &limited).await?.created);
    assert!(matches!(
        repo.enroll_reader(id(5), &limited)
            .await
            .unwrap_err()
            .downcast_ref::<Denied>(),
        Some(Denied::Limit)
    ));
    assert!(repo.account_permit(id(4)).await.is_ok());
    assert!(repo.publisher_permit(id(4)).await.is_err());
    // Moving the clock behind a persisted window cannot replenish its budget.
    repo.pool.get().await?.batch_execute("UPDATE reader_enrollment_budget SET window_started=clock_timestamp()+interval '2 hours',window_created=10").await?;
    assert!(matches!(
        repo.enroll_reader(id(6), &policy)
            .await
            .unwrap_err()
            .downcast_ref::<Denied>(),
        Some(Denied::Limit)
    ));
    assert!(repo.publisher_permit(id(3)).await.is_err());
    std::fs::write(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("results/enrollment-fences.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"checks":["migration preserves legacy denials and older binary insert/update revocations","operator insert denial wins signup conflict","budget failure rolls account insertion back","operator revocation serializes with duplicate enrollment","clock rollback does not replenish quota","enrollment grants no publisher permission"]}),
        )?,
    )?;
    Ok(())
}
