mod support;
use anyhow::{Context, Result};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    upload::{StorageConfig, UploadStore},
};
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL"]
async fn parallel_cleanup_preserves_shared_admission_and_exact_refunds() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let config: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(config.get_dbname() == Some("manga_test"), "fixture only");
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 4,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let dir = tempfile::tempdir()?;
    let store = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.path().into(),
            volume: "parallelcleanup".into(),
            physical_bytes: 96 * 512,
            staging_bytes: 96 * 512,
            max_blob_bytes: 4096,
            upload_slots: 2,
            reclamation_slots: 16,
            credit_lanes: 16,
            formats: vec!["image/png".into()],
        },
        [201; 32],
        [202; 32],
    )
    .await?;
    let mut hashes = Vec::new();
    for i in 0u32..96 {
        let mut bytes = vec![7; 512];
        bytes[..4].copy_from_slice(&i.to_be_bytes());
        let hash: [u8; 32] = hex::decode(auth::hash(&bytes))?.try_into().unwrap();
        store.put(hash, &bytes).await?;
        hashes.push(hash.to_vec());
    }
    repo.pool.get().await?.execute(
        "UPDATE blob_intents SET staging_until=clock_timestamp()-interval '1 second' WHERE hash=ANY($1)",
        &[&hashes],
    ).await?;
    tokio::time::timeout(Duration::from_secs(30), async {
        for _ in 0..8 {
            store.reclaim_objects_page(false, false, true).await?;
            let mut all_absent = true;
            for hash in &hashes {
                if store
                    .get(hash.clone().try_into().unwrap(), [201; 32])
                    .await
                    .is_ok()
                {
                    all_absent = false;
                    break;
                }
            }
            if all_absent {
                return Ok::<_, anyhow::Error>(());
            }
        }
        anyhow::bail!("bounded cleanup failed to clear expired synthetic blobs")
    })
    .await??;
    for hash in &hashes {
        assert!(
            store
                .get(hash.clone().try_into().unwrap(), [201; 32])
                .await
                .is_err()
        );
    }
    support::assert_storage_used(&repo, &store, 96 * 512, 0).await?;
    // Repeated cleanup cannot refund a second time; refill the exact capacity
    // using real staged puts/deletes and reject an extra byte again.
    assert_eq!(store.reclaim_objects_page(false, false, true).await?, 0);
    support::assert_storage_used(&repo, &store, 96 * 512, 0).await?;
    Ok(())
}
