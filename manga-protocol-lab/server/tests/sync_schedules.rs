use anyhow::{Context, Result};
use manga_node::{
    db::{DbConfig, Repository},
    sync_jobs::SyncSpec,
    sync_schedules::ScheduleSpec,
};
use serde_json::json;
use std::time::Duration;
fn spec(n: u32) -> SyncSpec {
    SyncSpec {
        peer: "11".repeat(32),
        inventory: "profiles".into(),
        filter: format!("created_at>={n}"),
        fallbacks: vec![],
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL"]
async fn explicit_schedules_are_bounded_atomic_and_revocable() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(cfg.get_dbname() == Some("manga_test"), "fixture only");
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    repo.set_peer([17; 32], "http://127.0.0.1:19700", true)
        .await?;
    assert!(
        repo.dispatch_sync_schedule().await?.is_none(),
        "approval alone scheduled work"
    );
    let policy = repo
        .save_sync_schedule(ScheduleSpec {
            spec: spec(0),
            interval_seconds: 30,
            enabled: false,
        })
        .await?;
    assert!(repo.dispatch_sync_schedule().await?.is_none());
    repo.enable_sync_schedule(policy, true).await?;
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..24 {
        let r = repo.clone();
        tasks.spawn(async move { r.dispatch_sync_schedule().await });
    }
    let mut dispatched = 0;
    while let Some(result) = tasks.join_next().await {
        if result??.is_some() {
            dispatched += 1;
        }
    }
    assert_eq!(dispatched, 1, "concurrent tick duplicated dispatch");
    let row = repo.sync_schedules().await?.remove(0);
    assert_eq!(row["dispatches"], 1);
    let batch = hex::decode(row["last_batch"].as_str().unwrap())?;
    assert!(
        repo.dispatch_sync_schedule().await?.is_none(),
        "interval ignored"
    );
    repo.pool
        .get()
        .await?
        .execute(
            "UPDATE sync_schedules SET next_run=clock_timestamp()-interval '1 second' WHERE id=$1",
            &[&policy.as_slice()],
        )
        .await?;
    repo.dispatch_sync_schedule().await?;
    assert_eq!(
        repo.sync_schedules().await?[0]["last_batch"],
        row["last_batch"],
        "active work was not coalesced"
    );
    let manual = repo.enqueue_sync(spec(0)).await?;
    assert_eq!(manual.as_slice(), batch);
    repo.enable_sync_schedule(policy, false).await?;
    repo.pool
        .get()
        .await?
        .execute(
            "UPDATE sync_schedules SET next_run=clock_timestamp()-interval '1 second' WHERE id=$1",
            &[&policy.as_slice()],
        )
        .await?;
    assert!(repo.dispatch_sync_schedule().await?.is_none());
    assert_eq!(
        repo.sync_status(manual).await?["state"],
        "queued",
        "disable unexpectedly cancelled admitted work"
    );
    repo.cancel_sync(manual).await?;
    repo.enable_sync_schedule(policy, true).await?;
    repo.set_peer([17; 32], "http://127.0.0.1:19700", false)
        .await?;
    repo.dispatch_sync_schedule().await?;
    let row = repo.sync_schedules().await?.remove(0);
    assert!(
        row["dispatch_error"]
            .as_str()
            .unwrap()
            .contains("not approved")
    );
    assert_eq!(row["last_batch"], hex::encode(manual));
    repo.enable_sync_schedule(policy, false).await?;
    assert!(repo.enable_sync_schedule(policy, true).await.is_err());
    repo.set_peer([17; 32], "http://127.0.0.1:19700", true)
        .await?;
    for interval_seconds in [29, 86401] {
        assert!(
            repo.save_sync_schedule(ScheduleSpec {
                spec: spec(0),
                interval_seconds,
                enabled: true
            })
            .await
            .is_err()
        );
    }
    repo.enable_sync_schedule(policy, true).await?;
    for n in 1..32 {
        repo.save_sync_schedule(ScheduleSpec {
            spec: spec(n),
            interval_seconds: 60,
            enabled: true,
        })
        .await?;
    }
    assert!(
        repo.save_sync_schedule(ScheduleSpec {
            spec: spec(32),
            interval_seconds: 60,
            enabled: true
        })
        .await
        .is_err()
    );
    for item in repo.sync_schedules().await? {
        let id: [u8; 32] = hex::decode(item["id"].as_str().unwrap())?
            .try_into()
            .unwrap();
        if id != policy {
            repo.enable_sync_schedule(id, false).await?;
        }
    }
    // Generate real terminal schedule history. Public status must retain manual
    // interest while pruning only the oldest automatically scheduled batch.
    let mut history = Vec::new();
    for _ in 0..513 {
        repo.enable_sync_schedule(policy, true).await?;
        assert!(repo.dispatch_sync_schedule().await?.is_some());
        let work = repo.claim_sync().await?.context("scheduled work missing")?;
        history.push(work.batch);
        repo.save_missing(&work, vec![]).await?;
        repo.finish_sync_quantum(&work).await?;
        assert_eq!(repo.sync_status(work.batch).await?["state"], "completed");
    }
    assert!(
        repo.sync_status(history[0]).await.is_err(),
        "oldest automatic history must be pruned"
    );
    for batch in &history[1..] {
        assert_eq!(repo.sync_status(*batch).await?["state"], "completed");
    }
    assert_eq!(
        repo.sync_status(manual).await?["state"],
        "cancelled",
        "manual interest must preserve history"
    );
    assert!(
        repo.claim_sync().await?.is_none(),
        "completed history must not yield more work"
    );
    std::fs::write(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../results/sync-schedules.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["peer approval never creates schedules","explicit disabled policy stays idle","24 concurrent dispatches yield one job","interval respected and active job deduplicated","disable fences future dispatch without cancelling current work","revoked peer cannot dispatch or be enabled","32 enabled-policy and interval limits","513 real completed schedules retain the newest512 public statuses","manual job history survives automatic pruning and no completed work is claimable"],"scope":"real isolated PostgreSQL; due timestamps fault-arranged, all job history created through application lifecycle"}),
        )?,
    )?;
    Ok(())
}
