use anyhow::{Context, Result};
use manga_node::{
    db::{DbConfig, Repository},
    sync_jobs::{QueueFull, SyncSpec},
};
use std::time::Duration;
fn spec(n: u32) -> SyncSpec {
    SyncSpec {
        inventory: "chapters".into(),
        peer: "11".repeat(32),
        filter: format!("pages>={n}"),
        fallbacks: vec![],
    }
}
fn ids(start: u64, n: usize) -> Vec<[u8; 32]> {
    (start..start + n as u64)
        .map(|v| {
            let mut id = [0u8; 32];
            id[24..].copy_from_slice(&v.to_be_bytes());
            id
        })
        .collect()
}
async fn prove_empty_pending_budget(repo: &Repository) -> Result<()> {
    let mut batches = Vec::new();
    for n in 0..2 {
        batches.push(repo.enqueue_sync(spec(900 + n)).await?);
        let work = repo
            .claim_sync()
            .await?
            .context("capacity probe work missing")?;
        repo.save_missing(&work, ids(u64::from(n) * 100000, 100000))
            .await?;
    }
    let overflow = repo.enqueue_sync(spec(902)).await?;
    let work = repo
        .claim_sync()
        .await?
        .context("overflow probe work missing")?;
    assert!(
        repo.save_missing(&work, ids(200000, 1))
            .await
            .unwrap_err()
            .is::<QueueFull>()
    );
    for batch in batches.into_iter().chain([overflow]) {
        assert!(repo.cancel_sync(batch).await?);
    }
    Ok(())
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL; run scripts/test-postgres.sh"]
async fn bounded_queue_fencing_fairness_and_backoff() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(cfg.get_dbname() == Some("manga_test"), "fixture required");
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    repo.set_peer([0x11; 32], "http://127.0.0.1:19999", true)
        .await?;
    let one = repo.enqueue_sync(spec(1)).await?;
    assert_eq!(one, repo.enqueue_sync(spec(1)).await?);
    repo.pool.get().await?.execute("UPDATE sync_batches SET spec=spec-'inventory',request_key=decode(repeat('af',32),'hex') WHERE id=$1",&[&one.as_slice()]).await?;
    assert_eq!(
        one,
        repo.enqueue_sync(spec(1)).await?,
        "legacy default inventory deduplication"
    );
    let a = repo.claim_sync().await?.unwrap();
    assert_eq!(a.batch, one);
    assert!(
        repo.finish_sync_quantum(&a).await.is_err(),
        "unreconciled partition cannot claim completion"
    );
    repo.save_missing(&a, ids(0, 100000)).await?;
    let two = repo.enqueue_sync(spec(2)).await?;
    let b = repo.claim_sync().await?.unwrap();
    assert_eq!(b.batch, two);
    repo.save_missing(&b, ids(100000, 100000)).await?;
    let three = repo.enqueue_sync(spec(3)).await?;
    let c = repo.claim_sync().await?.unwrap();
    assert_eq!(c.batch, three);
    let error = repo.save_missing(&c, ids(200000, 1)).await.unwrap_err();
    assert!(error.is::<QueueFull>());
    repo.retry_sync(&c, &error).await?;
    let connection = repo.pool.get().await?;
    // More than the ordinary retry budget of five backpressure cycles must
    // remain recoverable, rather than inspecting an internal failure count.
    for _ in 0..6 {
        connection
            .execute(
                "UPDATE jobs SET available_at=clock_timestamp() WHERE id=$1",
                &[&c.lease.id.as_slice()],
            )
            .await?;
        let retry = repo
            .claim_sync()
            .await?
            .context("backpressure consumed retry budget")?;
        assert_eq!(retry.batch, three);
        let error = repo.save_missing(&retry, ids(200000, 1)).await.unwrap_err();
        assert!(error.is::<QueueFull>());
        repo.retry_sync(&retry, &error).await?;
        assert_ne!(repo.sync_status(three).await?["state"], "failed");
    }
    repo.cancel_sync(one).await?;
    assert!(repo.acknowledge_sync_item(&a, [0; 32]).await.is_err());
    connection
        .execute(
            "UPDATE jobs SET available_at=clock_timestamp() WHERE id=$1",
            &[&c.lease.id.as_slice()],
        )
        .await?;
    let current = repo.claim_sync().await?.unwrap();
    assert_eq!(current.batch, three);
    assert!(current.lease.generation > c.lease.generation);
    repo.save_missing(&current, ids(200000, 100000)).await?;
    let overflow = repo.enqueue_sync(spec(30)).await?;
    let probe = repo.claim_sync().await?.unwrap();
    assert!(
        repo.save_missing(&probe, ids(300000, 1))
            .await
            .unwrap_err()
            .is::<QueueFull>()
    );
    repo.cancel_sync(overflow).await?;
    assert!(
        repo.claim_sync().await?.is_none(),
        "new connections do not steal live leases"
    );
    connection
        .execute(
            "UPDATE jobs SET lease_until=clock_timestamp()-interval '1 second' WHERE id=$1",
            &[&b.lease.id.as_slice()],
        )
        .await?;
    let replacement = repo.claim_sync().await?.unwrap();
    assert_eq!(replacement.batch, two);
    assert!(replacement.lease.generation > b.lease.generation);
    assert!(
        repo.acknowledge_sync_item(&b, ids(100000, 1)[0])
            .await
            .is_err()
    );
    repo.cancel_sync(two).await?;
    repo.cancel_sync(three).await?;
    // A partition yields after16 items so a different batch can run.
    let first = repo.enqueue_sync(spec(4)).await?;
    let first_work = repo.claim_sync().await?.unwrap();
    repo.save_missing(&first_work, ids(0, 17)).await?;
    let second = repo.enqueue_sync(spec(5)).await?;
    let quantum = repo.sync_items(&first_work).await?;
    assert_eq!(quantum.len(), 16);
    for id in quantum {
        repo.acknowledge_sync_item(&first_work, id).await?;
    }
    repo.finish_sync_quantum(&first_work).await?;
    let next = repo.claim_sync().await?.unwrap();
    assert_eq!(next.batch, second);
    repo.save_missing(&next, vec![]).await?;
    repo.finish_sync_quantum(&next).await?;
    tokio::time::sleep(Duration::from_millis(20)).await;
    let last = repo.claim_sync().await?.unwrap();
    assert_eq!(last.batch, first);
    for id in repo.sync_items(&last).await? {
        repo.acknowledge_sync_item(&last, id).await?;
    }
    repo.finish_sync_quantum(&last).await?;
    assert_eq!(repo.sync_status(first).await?["state"], "completed");
    let split = repo.enqueue_sync(spec(6)).await?;
    let work = repo.claim_sync().await?.unwrap();
    repo.split_sync_partition(&work).await?;
    let status = repo.sync_status(split).await?;
    assert_eq!(status["partitions_created"], 17);
    assert_eq!(status["partitions_done"], 1);
    let mut prefixes = Vec::new();
    for _ in 0..16 {
        let child = repo.claim_sync().await?.context("split child missing")?;
        assert_eq!(child.batch, split);
        prefixes.push(child.prefix.clone());
        repo.save_missing(&child, vec![]).await?;
        repo.finish_sync_quantum(&child).await?;
    }
    prefixes.sort();
    assert_eq!(
        prefixes,
        "0123456789abcdef"
            .chars()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(repo.sync_status(split).await?["state"], "completed");
    assert_eq!(repo.sync_status(split).await?["partitions_done"], 17);
    let failing = repo.enqueue_sync(spec(7)).await?;
    for attempt in 1..=5 {
        let work = repo.claim_sync().await?.unwrap();
        assert_eq!(work.batch, failing);
        if attempt == 1 {
            repo.save_missing(&work, ids(0, 17)).await?;
        }
        repo.retry_sync(&work, &anyhow::anyhow!("synthetic transient failure"))
            .await?;
        if attempt < 5 {
            assert!(
                repo.claim_sync().await?.is_none(),
                "backoff must delay immediate retry"
            );
            connection
                .execute(
                    "UPDATE jobs SET available_at=clock_timestamp() WHERE id=$1",
                    &[&work.lease.id.as_slice()],
                )
                .await?;
        }
    }
    assert_eq!(repo.sync_status(failing).await?["state"], "failed");
    prove_empty_pending_budget(&repo).await?;
    let mut batches = vec![];
    for n in 100..164 {
        batches.push(repo.enqueue_sync(spec(n)).await?);
    }
    assert!(repo.enqueue_sync(spec(164)).await.is_err());
    for id in batches {
        repo.cancel_sync(id).await?;
    }
    std::fs::write(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../results/postgres-sync-queue.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"scope":"synthetic pending IDs and queue state; not media admission","checks":["active deduplication and64-batch cap","global200000-item cap is reusable after cancellation and exhausted failure cleanup","backpressure does not consume failure retries","cancelled and replaced leases cannot acknowledge stale work","new claim does not reset live lease","16-item quantum yields to another batch","prefix split creates16 bounded children","five consecutive failures stop batch after backoff"]}),
        )?,
    )?;
    Ok(())
}
