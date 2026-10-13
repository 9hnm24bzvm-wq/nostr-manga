mod support;
use anyhow::{Context, Result};
use manga_node::{
    db::{DbConfig, Repository},
    storage::FilesystemVolume,
    upload::{StorageConfig, UploadStore},
};
use std::time::Duration;
#[tokio::test]
#[ignore = "requires isolated PostgreSQL; run scripts/test-postgres.sh"]
async fn restart_recovers_pending_bytes_and_refunds_only_cleaned_files() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(cfg.get_dbname() == Some("manga_test"), "fixture required");
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
    let owner = [85; 32];
    let holder = [86; 32];
    let budget = "physical:recoverfixture";
    let staging = format!("staging:{}", hex::encode(owner));
    repo.configure_budget(budget, 100).await?;
    repo.configure_budget(&staging, 100).await?;
    let grants = [
        repo.allocate_grant(budget, holder, 100).await?,
        repo.allocate_grant(&staging, holder, 100).await?,
    ];
    let bytes = b"durable but not registered";
    let hash: [u8; 32] = hex::decode(manga_node::auth::hash(bytes))?
        .try_into()
        .unwrap();
    let intent = repo
        .begin_blob(hash, "recoverfixture", owner, bytes.len() as u64, &grants)
        .await?;
    let absent = [87; 32];
    repo.begin_blob(absent, "recoverfixture", owner, 10, &grants)
        .await?;
    {
        let volume = FilesystemVolume::open(dir.path().into())?;
        let mutation = volume.mutation(hash).await;
        mutation
            .put_verified(intent.key.generation, bytes, 100)
            .await?;
    }
    let store = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.path().into(),
            volume: "recoverfixture".into(),
            physical_bytes: 100,
            staging_bytes: 100,
            max_blob_bytes: 100,
            upload_slots: 2,
            reclamation_slots: 4,
            credit_lanes: 16,
            formats: vec!["image/png".into()],
        },
        owner,
        holder,
    )
    .await?;
    let mut cursor = None;
    while let Some(next) = store.recover_page(cursor).await? {
        cursor = Some(next);
    }
    assert!(repo.blob_intent(hash).await?.unwrap().available);
    assert!(repo.blob_intent(absent).await?.is_none());
    assert_eq!(store.get(hash, owner).await?, bytes);
    assert!(store.get(absent, owner).await.is_err());
    support::assert_storage_used(&repo, &store, 100, bytes.len() as u64).await?;
    assert!(store.recover_page(None).await?.is_none());
    support::assert_storage_used(&repo, &store, 100, bytes.len() as u64).await?;
    Ok(())
}
