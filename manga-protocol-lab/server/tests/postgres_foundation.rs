use anyhow::{Context, Result};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    sync_jobs::SyncSpec,
};
use serde_json::json;
use std::time::Duration;

fn config(dsn: &str, connections: usize, timeout: Duration) -> DbConfig {
    DbConfig {
        dsn: dsn.into(),
        connections,
        acquisition_timeout: timeout,
        cache_weight: 1_048_576,
        plaintext_local: true,
    }
}
fn spec(n: u32) -> SyncSpec {
    SyncSpec {
        inventory: "chapters".into(),
        peer: "11".repeat(32),
        filter: format!("pages>={n}"),
        fallbacks: vec![],
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run scripts/test-postgres.sh"]
async fn postgres_foundation_concurrency_and_cache() -> Result<()> {
    let dsn =
        std::env::var("MANGA_TEST_DATABASE_URL").context("ephemeral test database required")?;
    let parsed: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(
        parsed.get_dbname() == Some("manga_test"),
        "refuse a non-fixture database"
    );
    let a = Repository::connect(config(&dsn, 8, Duration::from_secs(2))).await?;
    let b = Repository::connect(config(&dsn, 8, Duration::from_secs(2))).await?;
    // Two concurrent deployment attempts must not race schema construction.
    let (x, y) = tokio::join!(a.migrate(), b.migrate());
    x?;
    y?;
    let mut checks = vec!["concurrent explicit migrations serialize and agree on checksum"];
    let event = auth::sign(
        &format!("{:064x}", 1),
        1,
        vec![],
        "duplicate fixture".into(),
    )
    .map_err(anyhow::Error::msg)?;
    let id: [u8; 32] = hex::decode(&event.id)?.try_into().unwrap();
    let mut tasks = vec![];
    for i in 0..32 {
        let repo = if i % 2 == 0 { a.clone() } else { b.clone() };
        let event = event.clone();
        tasks.push(tokio::spawn(async move {
            repo.stage_regular_event(&event).await
        }));
    }
    let mut inserted = 0;
    for t in tasks {
        inserted += u64::from(t.await??);
    }
    assert_eq!(inserted, 1);
    assert_eq!(a.visible_event(id).await?.unwrap().id, event.id);
    checks.push("32 competing writes across independent pools admit one visible event");
    // A simulated second statement failure must roll the event insertion back.
    let rollback = auth::sign(&format!("{:064x}", 1), 1, vec![], "rollback-fixture".into())
        .map_err(anyhow::Error::msg)?;
    let client = a.pool.get().await?;
    client.batch_execute("CREATE FUNCTION fail_outbox_fixture() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.payload->>'content'='rollback-fixture' THEN RAISE EXCEPTION 'injected outbox failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER fail_outbox_fixture BEFORE INSERT ON backend_outbox FOR EACH ROW EXECUTE FUNCTION fail_outbox_fixture();").await?;
    assert!(a.stage_regular_event(&rollback).await.is_err());
    let rollback_id = hex::decode(&rollback.id)?;
    assert!(
        a.visible_event(rollback_id.try_into().unwrap())
            .await?
            .is_none()
    );
    client.batch_execute("DROP TRIGGER fail_outbox_fixture ON backend_outbox; DROP FUNCTION fail_outbox_fixture();").await?;
    drop(client);
    checks.push("projection-intent failure rolls back the event transaction");
    let mut tasks = vec![];
    for _ in 0..32 {
        let repo = a.clone();
        tasks.push(tokio::spawn(async move { repo.visible_event(id).await }));
    }
    let mut values = vec![];
    for t in tasks {
        values.push(t.await??.unwrap());
    }
    for value in values {
        assert_eq!(
            serde_json::to_value(value.as_ref())?,
            serde_json::to_value(&event)?
        );
    }
    assert!(b.visible_event(id).await?.is_some());
    checks.push(
        "concurrent readers return the same verified immutable event across independent pools",
    );
    // No invalidation is sent to either cache: primary state must still win.
    a.pool
        .get()
        .await?
        .execute(
            "UPDATE events SET state='removed',version=version+1 WHERE id=$1",
            &[&id.as_slice()],
        )
        .await?;
    assert!(a.visible_event(id).await?.is_none());
    assert!(b.visible_event(id).await?.is_none());
    assert!(!b.stage_regular_event(&event).await?);
    assert!(a.visible_event(id).await?.is_none());
    checks.push(
        "warm immutable caches cannot bypass current removal state or resurrect a duplicate event",
    );
    // Application sync batches coalesce and fence real worker transitions.
    a.set_peer([0x11; 32], "http://127.0.0.1:19999", true)
        .await?;
    let mut tasks = vec![];
    for i in 0..24 {
        let repo = if i % 2 == 0 { a.clone() } else { b.clone() };
        tasks.push(tokio::spawn(
            async move { repo.enqueue_sync(spec(0)).await },
        ));
    }
    let mut ids = vec![];
    for task in tasks {
        ids.push(task.await??);
    }
    assert!(ids.iter().all(|id| *id == ids[0]));
    let first = a.claim_sync().await?.unwrap();
    assert_eq!(first.batch, ids[0]);
    b.save_missing(&first, vec![]).await?;
    b.finish_sync_quantum(&first).await?;
    assert_eq!(a.sync_status(first.batch).await?["state"], "completed");
    for i in 1..=32 {
        a.enqueue_sync(spec(i)).await?;
    }
    let mut tasks = vec![];
    for i in 0..48 {
        let repo = if i % 2 == 0 { a.clone() } else { b.clone() };
        tasks.push(tokio::spawn(async move { repo.claim_sync().await }));
    }
    let mut claims = vec![];
    for task in tasks {
        if let Some(work) = task.await?? {
            claims.push(work);
        }
    }
    let distinct: std::collections::HashSet<_> = claims.iter().map(|w| w.batch).collect();
    assert_eq!(distinct.len(), 32);
    assert_eq!(claims.len(), 32);
    for work in &claims {
        a.save_missing(work, vec![]).await?;
        a.finish_sync_quantum(work).await?;
        assert_eq!(b.sync_status(work.batch).await?["state"], "completed");
    }
    checks.push("real sync batches coalesce and concurrent workers claim each batch exactly once");
    let recovery = a.enqueue_sync(spec(100)).await?;
    let old = a.claim_sync().await?.unwrap();
    assert_eq!(old.batch, recovery);
    let fresh = Repository::connect(config(&dsn, 2, Duration::from_secs(2))).await?;
    assert!(fresh.claim_sync().await?.is_none());
    a.pool
        .get()
        .await?
        .execute(
            "UPDATE jobs SET lease_until=clock_timestamp()-interval '1 second' WHERE id=$1",
            &[&old.lease.id.as_slice()],
        )
        .await?;
    let newer = b.claim_sync().await?.unwrap();
    assert_eq!(newer.batch, recovery);
    assert!(newer.lease.generation > old.lease.generation);
    assert_ne!(newer.lease.token, old.lease.token);
    assert!(a.save_missing(&old, vec![]).await.is_err());
    assert!(a.finish_sync_quantum(&old).await.is_err());
    assert!(!a.renew(&old.lease, Duration::from_secs(30)).await?);
    assert!(b.renew(&newer.lease, Duration::from_secs(30)).await?);
    b.save_missing(&newer, vec![]).await?;
    b.finish_sync_quantum(&newer).await?;
    assert_eq!(fresh.sync_status(recovery).await?["state"], "completed");
    let cancelled = a.enqueue_sync(spec(101)).await?;
    let work = a.claim_sync().await?.unwrap();
    assert!(b.cancel_sync(cancelled).await?);
    assert!(a.save_missing(&work, vec![]).await.is_err());
    assert!(a.finish_sync_quantum(&work).await.is_err());
    assert_eq!(fresh.sync_status(cancelled).await?["state"], "cancelled");
    assert!(fresh.claim_sync().await?.is_none());
    checks.push("reconnected workers preserve live leases; expired or cancelled sync ownership cannot save, complete or renew stale work");
    let limited = Repository::connect(config(&dsn, 1, Duration::from_millis(25))).await?;
    let held = limited.pool.get().await?;
    assert!(limited.visible_event(id).await.is_err());
    drop(held);
    assert!(limited.visible_event(id).await?.is_none());
    checks.push("pool exhaustion times out and subsequent requests recover after capacity returns");
    let mut bad = config(
        "host=192.0.2.1 dbname=manga_test",
        1,
        Duration::from_secs(1),
    );
    bad.plaintext_local = true;
    assert!(
        Repository::connect(bad)
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("loopback")
    );
    let bad = config(
        "host=127.0.0.1 hostaddr=192.0.2.1 dbname=manga_test",
        1,
        Duration::from_secs(1),
    );
    assert!(
        Repository::connect(bad)
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("loopback")
    );
    checks.push("plaintext database configuration rejects remote hosts and hostaddr overrides before connecting");
    let report = json!({"scope":"real PostgreSQL with independent pools; synthetic events; not full manga-server feature parity or target-scale certification","checks":checks,"count":checks.len()});
    if let Ok(path) = std::env::var("MANGA_TEST_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    }
    println!("{report}");
    Ok(())
}
