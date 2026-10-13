use anyhow::{Context, Result};
use manga_node::{
    db::{DbConfig, Repository},
    storage::FilesystemVolume,
};
use sha2::{Digest, Sha256};
use std::time::Duration;
#[tokio::test]
#[ignore = "requires isolated PostgreSQL; run scripts/test-postgres.sh"]
async fn receipt_and_quota_transactions() -> Result<()> {
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
    let dir = tempfile::tempdir()?;
    let volume = FilesystemVolume::open(dir.path().into())?;
    let bytes = b"verified original";
    let size = bytes.len() as u64;
    let hash: [u8; 32] = Sha256::digest(bytes).into();
    repo.configure_budget("receipt-volume", size as i64).await?;
    repo.configure_budget("receipt-staging", size as i64)
        .await?;
    let grants = [
        repo.allocate_grant("receipt-volume", [81; 32], size as i64)
            .await?,
        repo.allocate_grant("receipt-staging", [81; 32], size as i64)
            .await?,
    ];
    let mutation = volume.mutation(hash).await;
    let intent = repo
        .begin_blob(hash, "fixture", [81; 32], size, &grants)
        .await?;
    assert!(!intent.available);
    assert!(
        repo.begin_blob([88; 32], "fixture", [81; 32], 1, &grants)
            .await
            .is_err()
    );
    assert!(
        repo.blob_intent([88; 32]).await?.is_none(),
        "failed reservation must roll back intent"
    );
    let receipt = mutation
        .put_verified(intent.key.generation, bytes, 100)
        .await?;
    // Simulated crash boundary: bytes exist, but metadata still says writing.
    assert!(!repo.blob_intent(hash).await?.unwrap().available);
    let recovered = mutation.recover(intent.key.generation, size, 100).await?;
    assert!(repo.finish_blob("fixture", &recovered).await?);
    assert!(!repo.finish_blob("fixture", &receipt).await?);
    assert!(repo.blob_intent(hash).await?.unwrap().available);
    let duplicate = repo
        .begin_blob(hash, "fixture", [81; 32], size, &grants)
        .await?;
    assert_eq!(
        duplicate.key, intent.key,
        "duplicate works even when budget is full"
    );
    assert!(
        repo.begin_blob(hash, "fixture", [82; 32], size, &grants)
            .await
            .is_err()
    );
    assert!(
        repo.begin_blob([89; 32], "fixture", [81; 32], 1, &grants)
            .await
            .is_err(),
        "settled or duplicate receipts must not release charged space"
    );
    drop(mutation);
    repo.configure_budget("receipt-abort", 10).await?;
    let grant = repo.allocate_grant("receipt-abort", [81; 32], 10).await?;
    let data = b"abort";
    let hash: [u8; 32] = Sha256::digest(data).into();
    let mutation = volume.mutation(hash).await;
    let old = repo
        .begin_blob(hash, "fixture", [81; 32], 5, &[grant])
        .await?;
    let old_receipt = mutation.put_verified(old.key.generation, data, 100).await?;
    let cleanup = mutation.cleanup(old.key.generation).await?;
    assert!(repo.abort_blob("fixture", &cleanup).await?);
    assert!(!repo.abort_blob("fixture", &cleanup).await?);
    let new = repo
        .begin_blob(hash, "fixture", [81; 32], 5, &[grant])
        .await?;
    assert_ne!(old.key, new.key);
    assert!(repo.finish_blob("fixture", &old_receipt).await.is_err());
    assert!(repo.abort_blob("fixture", &cleanup).await.is_err());
    let fresh = mutation.put_verified(new.key.generation, data, 100).await?;
    repo.finish_blob("fixture", &fresh).await?;
    drop(mutation); // Release the exclusive writer before acquiring a shared read pin.
    assert_eq!(volume.read_verified(&new.key, 5, 100).await?, data);
    // Exactly the other five bytes remain available after abort/replacement.
    let probe_bytes = b"probe";
    let probe_hash: [u8; 32] = Sha256::digest(probe_bytes).into();
    let probe = repo
        .begin_blob(probe_hash, "fixture", [81; 32], 5, &[grant])
        .await?;
    assert!(
        repo.begin_blob([90; 32], "fixture", [81; 32], 1, &[grant])
            .await
            .is_err(),
        "stale cleanup must not refund replacement bytes"
    );
    let probe_mutation = volume.mutation(probe_hash).await;
    let cleanup = probe_mutation.cleanup(probe.key.generation).await?;
    assert!(repo.abort_blob("fixture", &cleanup).await?);
    std::fs::write(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../results/postgres-blob-receipts.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"checks":["atomic intent plus paired reservations","quota exhaustion leaves no phantom intent","durable uncommitted bytes recover and settle once","duplicate upload succeeds at full quota","different owner cannot acquire staged original","cleanup releases only matching unfinished generation","stale receipt and stale cleanup reject replacement"]}),
        )?,
    )?;
    Ok(())
}
