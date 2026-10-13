mod support;
use anyhow::{Context, Result, ensure};
use manga_node::{
    db::{DbConfig, Repository},
    upload::{StorageConfig, UploadStore},
};
use std::time::Duration;
use support::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "real PostgreSQL and externally blocked health persistence"]
async fn cancelled_health_read_fences_repair_until_its_decision_is_durable() -> Result<()> {
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
    let owner = [71; 32];
    repo.bind_instance([72; 32], owner).await?;
    let dir = tempfile::tempdir()?;
    // Configuration is accepted/rejected through the real application constructor.
    // No private validator or test-only public facade is involved.
    for limit in [1usize, 4, 8, 16] {
        let config: StorageConfig = serde_json::from_value(serde_json::json!({
            "path":dir.path().join(format!("config-{limit}")), "volume":format!("config{limit}"),
            "physical_bytes":1048576,"staging_bytes":1048576,
            "upload_slots":1,"reclamation_slots":limit
        }))?;
        drop(UploadStore::open(repo.clone(), config, owner, [72; 32]).await?);
    }
    for field in ["upload_slots", "reclamation_slots"] {
        for limit in [0usize, 17, 1024] {
            let mut value = serde_json::json!({
                "path":dir.path().join("invalid-config"), "volume":"invalidconfig",
                "physical_bytes":1048576,"staging_bytes":1048576
            });
            value[field] = limit.into();
            let config: StorageConfig = serde_json::from_value(value)?;
            ensure!(
                UploadStore::open(repo.clone(), config, owner, [72; 32])
                    .await
                    .is_err(),
                "out-of-range {field} must be rejected by public storage open"
            );
        }
    }
    let store = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.path().into(),
            volume: "healthlifecycle".into(),
            physical_bytes: 1048576,
            staging_bytes: 1048576,
            max_blob_bytes: 1048576,
            upload_slots: 4,
            reclamation_slots: 4,
            credit_lanes: 16,
            formats: vec!["image/png".into()],
        },
        owner,
        [72; 32],
    )
    .await?;
    let bytes = b"health observation must finish before repair overwrites these bytes";
    let hash: [u8; 32] = hex::decode(manga_node::auth::hash(bytes))?
        .try_into()
        .unwrap();
    let intent = store.put(hash, bytes).await?;
    let other = b"unrelated verified object remains readable";
    let other_hash = hex::decode(manga_node::auth::hash(other))?
        .try_into()
        .unwrap();
    store.put(other_hash, other).await?;
    let encoded = hex::encode(hash);
    let path = dir
        .path()
        .join("objects")
        .join(&encoded[..2])
        .join(&encoded[2..4])
        .join(format!("{encoded}.{}", hex::encode(intent.key.generation)));
    std::fs::write(path, b"external damage")?;
    let peer_key = hex::decode(public(23))?.try_into().unwrap();
    repo.set_peer(peer_key, "http://127.0.0.1:19999", true)
        .await?;
    let peer = repo.approved_peer(peer_key).await?;
    // SQL is solely external fault arrangement. Outcomes below are application
    // read/repair results, never table contents or private lock counters.
    repo.pool.get().await?.batch_execute("CREATE FUNCTION lifecycle_health_barrier() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN PERFORM pg_advisory_xact_lock(7773502);RETURN NEW;END$$;CREATE TRIGGER lifecycle_health_barrier BEFORE INSERT ON quarantined_blobs FOR EACH ROW EXECUTE FUNCTION lifecycle_health_barrier()").await?;
    let mut blocker = repo.pool.get().await?;
    let pid: i32 = blocker
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    let transaction = blocker.transaction().await?;
    transaction
        .query_one("SELECT pg_advisory_xact_lock(7773502)", &[])
        .await?;
    let read = {
        let store = store.clone();
        tokio::spawn(async move { store.get(hash, owner).await })
    };
    wait_for_blocked_session(&repo, pid)
        .await
        .context("health fault did not engage")?;
    read.abort();
    ensure!(
        read.await.unwrap_err().is_cancelled(),
        "caller cancellation"
    );
    let mut repair = {
        let store = store.clone();
        tokio::spawn(async move { store.repair_verified(intent, bytes.to_vec(), peer).await })
    };
    ensure!(
        tokio::time::timeout(Duration::from_millis(100), &mut repair)
            .await
            .is_err(),
        "repair overtook an unfinished health decision"
    );
    assert_eq!(store.get(other_hash, owner).await?, other);
    transaction.commit().await?;
    drop(blocker);
    ensure!(
        tokio::time::timeout(Duration::from_secs(5), repair).await???,
        "verified repair must complete after the health fence"
    );
    assert_eq!(
        store.get(hash, owner).await?,
        bytes,
        "completed old read must not quarantine repaired bytes afterwards"
    );
    repo.pool.get().await?.batch_execute("DROP TRIGGER lifecycle_health_barrier ON quarantined_blobs; DROP FUNCTION lifecycle_health_barrier()").await?;
    Ok(())
}
