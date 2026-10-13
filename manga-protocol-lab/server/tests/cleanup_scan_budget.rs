use anyhow::{Context, Result};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    upload::{StorageConfig, UploadStore},
};
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and real filesystem"]
async fn cleanup_advances_a_bounded_candidate_page_past_other_owners() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let config: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(config.get_dbname() == Some("manga_test"), "fixture only");
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let root = tempfile::tempdir()?;
    let configuration = |volume: &str| StorageConfig {
        path: root.path().join(volume),
        volume: volume.into(),
        physical_bytes: 1_000_000,
        staging_bytes: 1_000_000,
        max_blob_bytes: 10_000,
        upload_slots: 4,
        reclamation_slots: 4,
        credit_lanes: 16,
        formats: vec!["image/png".into()],
    };
    let foreign =
        UploadStore::open(repo.clone(), configuration("foreign"), [21; 32], [31; 32]).await?;
    let local = UploadStore::open(repo.clone(), configuration("local"), [22; 32], [32; 32]).await?;
    let mut objects: Vec<([u8; 32], Vec<u8>)> = (0..65)
        .map(|n| {
            let bytes = format!("bounded cleanup candidate {n:03}").into_bytes();
            (
                hex::decode(auth::hash(&bytes)).unwrap().try_into().unwrap(),
                bytes,
            )
        })
        .collect();
    objects.sort_by_key(|(hash, _)| *hash);
    for (hash, bytes) in &objects[..64] {
        foreign.put(*hash, bytes).await?;
    }
    let (hash, bytes) = &objects[64];
    local.put(*hash, bytes).await?;
    repo.pool
        .get()
        .await?
        .execute(
            "UPDATE blob_intents SET staging_until=clock_timestamp()-interval '1 second'",
            &[],
        )
        .await?;
    assert_eq!(
        local.reclaim_objects_page(false, false, true).await?,
        0,
        "cleanup traversed more than its 64-candidate work budget to find a local object"
    );
    assert_eq!(local.get(*hash, [22; 32]).await?, *bytes);
    assert!(local.oldest_cleanup_due_seconds().await?.is_none());
    assert_eq!(repo.enqueue_expired_staging([22; 32], "local").await?, 1);
    repo.pool
        .get()
        .await?
        .execute(
            "UPDATE blob_gc_queue SET due_at=clock_timestamp()-interval '7 seconds' WHERE hash=$1",
            &[&hash.as_slice()],
        )
        .await?;
    assert!(
        local
            .oldest_cleanup_due_seconds()
            .await?
            .is_some_and(|age| age >= 7.0)
    );
    assert_eq!(
        local.reclaim_objects_page(false, false, true).await?,
        1,
        "candidate cursor did not advance past another owner's rows"
    );
    assert!(local.get(*hash, [22; 32]).await.is_err());
    assert!(local.oldest_cleanup_due_seconds().await?.is_none());
    for (hash, bytes) in &objects[..64] {
        assert_eq!(foreign.get(*hash, [21; 32]).await?, *bytes);
    }
    Ok(())
}
