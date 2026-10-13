mod support;
use anyhow::{Result, ensure};
use manga_node::{
    db::{DbConfig, Repository},
    sync_jobs::{QueueFull, SyncSpec},
    transfer_rejection::{RejectionReason, TransferRejection},
};
use std::time::Duration;
#[tokio::test]
#[ignore = "isolated PostgreSQL fixture"]
async fn skip_reporting_is_durable_bounded_and_lease_fenced() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    ensure!(cfg.get_dbname() == Some("manga_test"), "fixture only");
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let peer = hex::decode(support::public(11))?.try_into().unwrap();
    repo.set_peer(peer, "http://127.0.0.1:19999", true).await?;
    let spec = SyncSpec {
        peer: hex::encode(peer),
        inventory: "chapters".into(),
        filter: String::new(),
        fallbacks: vec![],
    };
    let batch = repo.enqueue_sync(spec.clone()).await?;
    let work = repo.claim_sync().await?.unwrap();
    let ids = (0u32..300)
        .map(|n| {
            let mut id = [0u8; 32];
            id[28..].copy_from_slice(&n.to_be_bytes());
            id
        })
        .collect::<Vec<_>>();
    repo.save_missing(&work, ids.clone()).await?;
    let rejection = TransferRejection::unsupported(RejectionReason::ManifestSchema);
    for id in &ids {
        repo.skip_sync_item(&work, *id, &rejection).await?;
    }
    repo.skip_sync_item(&work, ids[0], &rejection).await?;
    repo.finish_sync_quantum(&work).await?;
    let status = repo.sync_status(batch).await?;
    assert_eq!(status["state"], "completed_with_skips");
    assert_eq!(status["acknowledged_items"], 300);
    assert_eq!(status["skipped_items"], 300);
    assert_eq!(status["skip_counts"]["unsupported:manifest_schema"], 300);
    assert_eq!(status["skip_details"].as_array().unwrap().len(), 256);
    assert_eq!(status["skip_details_truncated"], true);
    assert!(
        repo.skip_sync_item(&work, ids[0], &rejection)
            .await
            .is_err(),
        "completed lease cannot change counters"
    );
    let batch = repo.enqueue_sync(spec.clone()).await?;
    let work = repo.claim_sync().await?.unwrap();
    repo.save_missing(&work, vec![[99; 32]]).await?;
    assert!(repo.cancel_sync(batch).await?);
    assert!(
        repo.skip_sync_item(&work, [99; 32], &rejection)
            .await
            .is_err()
    );
    assert_eq!(repo.sync_status(batch).await?["skipped_items"], 0);
    // Ordinary retryable failures remain retries, never disguised as skips.
    let batch = repo.enqueue_sync(spec.clone()).await?;
    let work = repo.claim_sync().await?.unwrap();
    repo.save_missing(&work, vec![[88; 32]]).await?;
    repo.retry_sync(
        &work,
        &anyhow::anyhow!("synthetic transient transport outage"),
    )
    .await?;
    assert_eq!(repo.sync_status(batch).await?["skipped_items"], 0);
    assert!(
        repo.claim_sync().await?.is_none(),
        "retry backoff must keep work unavailable"
    );
    repo.pool
        .get()
        .await?
        .execute(
            "UPDATE jobs SET available_at=clock_timestamp() WHERE id=$1",
            &[&work.lease.id.as_slice()],
        )
        .await?;
    let retried = repo.claim_sync().await?.unwrap();
    assert_eq!(retried.batch, batch);
    assert_eq!(repo.sync_items(&retried).await?, vec![[88; 32]]);
    repo.acknowledge_sync_item(&retried, [88; 32]).await?;
    repo.finish_sync_quantum(&retried).await?;
    assert_eq!(repo.sync_status(batch).await?["state"], "completed");
    assert_eq!(repo.sync_status(batch).await?["skipped_items"], 0);
    let mut probes = Vec::new();
    for n in 0..2u32 {
        let mut probe_spec = spec.clone();
        probe_spec.filter = format!("pages>={}", 900 + n);
        probes.push(repo.enqueue_sync(probe_spec).await?);
        let work = repo.claim_sync().await?.unwrap();
        let ids = (0..100000u32)
            .map(|value| {
                let mut id = [0; 32];
                id[28..].copy_from_slice(&value.to_be_bytes());
                id
            })
            .collect();
        repo.save_missing(&work, ids).await?;
    }
    let overflow = repo.enqueue_sync(spec).await?;
    let work = repo.claim_sync().await?.unwrap();
    assert!(
        repo.save_missing(&work, vec![[99; 32]])
            .await
            .unwrap_err()
            .is::<QueueFull>()
    );
    for batch in probes.into_iter().chain([overflow]) {
        repo.cancel_sync(batch).await?;
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    std::fs::write(
        root.join("results/sync-skip-accounting.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"checks":["all300 skips counted with256 bounded sample IDs","explicit completed_with_skips terminal state","duplicate acknowledgement idempotency","completed and cancelled lease fencing","all pending capacity reusable after skips, cancellation and retry completion","retryable error is not a skip"],"skip_details_limit":256}),
        )?,
    )?;
    Ok(())
}
