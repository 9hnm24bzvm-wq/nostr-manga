//! Storage fairness through the application staging and reclamation lifecycle.
//! PostgreSQL locks arrange stalled cleanup; results come from public operations.
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    library_manifest::{Chunk, Family, Limits, Record},
    library_storage::{LibraryObjects, ObjectType},
    upload::{StorageConfig, UploadStore},
};
use std::time::Duration;

// Each case installs external PostgreSQL trigger faults. CREATE/DROP TRIGGER
// takes table-wide locks, even when the trigger predicate is case-specific, so
// those fault arrangements cannot safely overlap on the fixture database.
// Cleanup and foreground operations within each case still run concurrently.
static DATABASE_FAULT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn chunk(n: u8) -> Vec<u8> {
    Chunk::new(
        Family::Chapters,
        vec![Record::Chapter {
            id: [n; 32],
            completed: true,
            page: None,
            updated_at: 1,
        }],
        &Limits::default(),
    )
    .unwrap()
    .encode(&Limits::default())
    .unwrap()
}
async fn repository() -> Result<Repository> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    ensure!(
        dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test"),
        "fixture only"
    );
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 12,
        acquisition_timeout: Duration::from_secs(3),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    Ok(repo)
}
async fn wait_for_cleanup_lock(repo: &Repository, lock: i64) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if repo.pool.get().await?.query_one(
                "SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND objid=$1::bigint AND NOT granted)",
                &[&lock],
            ).await?.get::<_, bool>(0) { return Ok::<_, anyhow::Error>(()); }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await??;
    Ok(())
}

async fn foreground_staging_during_cleanup(pressure: bool) -> Result<()> {
    let _fault = DATABASE_FAULT.lock().await;
    let repo = repository().await?;
    let dir = tempfile::tempdir()?;
    let owner = if pressure { [238; 32] } else { [231; 32] };
    let volume = if pressure {
        "storagefairpressure"
    } else {
        "storagefairness"
    };
    let store = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.path().into(),
            volume: volume.into(),
            physical_bytes: 100_000,
            staging_bytes: 100_000,
            max_blob_bytes: 100_000,
            upload_slots: 4,
            reclamation_slots: 4,
            credit_lanes: 16,
            formats: vec!["image/png".into()],
        },
        owner,
        [232; 32],
    )
    .await?;
    let mut retired = Vec::new();
    for i in 0..4u8 {
        // Blob identity is global across admissions, not local to this volume.
        let bytes = vec![if pressure { i + 10 } else { i }; 128];
        let hash = hex::decode(auth::hash(&bytes))?.try_into().unwrap();
        retired.push(store.put(hash, &bytes).await?.key);
    }
    // Expire only fixture objects, then use the real sparse candidate selection.
    repo.pool
        .get()
        .await?
        .execute(
            "UPDATE blob_intents SET staging_until=clock_timestamp()-interval '1 second' WHERE owner=$1", &[&owner.as_slice()],
        )
        .await?;
    repo.enqueue_expired_staging(owner, volume).await?;
    if pressure {
        let filler = vec![99; 90_000];
        store
            .put(
                hex::decode(auth::hash(&filler))?.try_into().unwrap(),
                &filler,
            )
            .await?;
        store.reclaim_objects_page(false, false, false).await?;
    }
    let blocker = repo.pool.get().await?;
    let fault_name = if pressure { "fair_pressure" } else { "fair" };
    let lock = if pressure { 7183944 } else { 7183941 };
    let fault_hashes = retired
        .iter()
        .map(|key| format!("decode('{}','hex')", hex::encode(key.hash)))
        .collect::<Vec<_>>()
        .join(",");
    blocker.batch_execute(&format!("CREATE FUNCTION fixture_hold_{fault_name}_cleanup() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.hash IN ({fault_hashes}) THEN PERFORM pg_advisory_xact_lock({lock}); END IF; RETURN NEW; END $$; CREATE TRIGGER fixture_hold_{fault_name}_cleanup BEFORE INSERT ON blob_reclaims FOR EACH ROW EXECUTE FUNCTION fixture_hold_{fault_name}_cleanup(); SELECT pg_advisory_lock({lock})")).await?;
    let mut cleanup = Vec::new();
    for key in retired.clone() {
        let store = store.clone();
        cleanup.push(tokio::spawn(
            async move { store.reclaim_library(key).await },
        ));
    }
    wait_for_cleanup_lock(&repo, lock).await?;
    // Give all submitted contenders a chance to reach their admission boundary.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let account = if pressure { [239; 32] } else { [233; 32] };
    repo.set_account_admission(account, true).await?;
    let permit = repo.account_permit(account).await?;
    let library = LibraryObjects::new(repo.clone(), store.clone());
    let bytes = chunk(if pressure { 20 } else { 19 });
    let staged = tokio::time::timeout(
        Duration::from_millis(500),
        library.stage(permit.clone(), ObjectType::Chunk, bytes.clone()),
    )
    .await;
    // Always release the external fault before reporting a failed assertion.
    blocker
        .batch_execute(&format!("SELECT pg_advisory_unlock({lock})"))
        .await?;
    for task in cleanup {
        ensure!(task.await??, "eligible cleanup must complete");
    }
    blocker.batch_execute(&format!("DROP TRIGGER fixture_hold_{fault_name}_cleanup ON blob_reclaims; DROP FUNCTION fixture_hold_{fault_name}_cleanup()")).await?;
    let staged = staged.context("stalled cleanup exhausted foreground staging capacity")??;
    ensure!(
        library.read(&permit, staged.key.hash).await? == bytes,
        "foreground stage/read must preserve exact bytes"
    );
    for key in retired {
        ensure!(
            store.get(key.hash, owner).await.is_err(),
            "cleanup did not finish"
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL"]
async fn scarce_physical_quota_accelerates_cleanup_without_stopping_foreground() -> Result<()> {
    let _fault = DATABASE_FAULT.lock().await;
    let repo = repository().await?;
    let dir = tempfile::tempdir()?;
    let owner = [234; 32];
    let store = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.path().into(),
            volume: "storagepressure".into(),
            physical_bytes: 100_000,
            staging_bytes: 100_000,
            max_blob_bytes: 100_000,
            upload_slots: 4,
            reclamation_slots: 4,
            credit_lanes: 16,
            formats: vec!["image/png".into()],
        },
        owner,
        [235; 32],
    )
    .await?;
    let mut retired = Vec::new();
    for i in 41..44u8 {
        let bytes = vec![i; 512];
        let hash = hex::decode(auth::hash(&bytes))?.try_into().unwrap();
        retired.push(store.put(hash, &bytes).await?.key);
    }
    repo.pool.get().await?.execute("UPDATE blob_intents SET staging_until=clock_timestamp()-interval '1 second' WHERE owner=$1", &[&owner.as_slice()]).await?;
    repo.enqueue_expired_staging(owner, "storagepressure")
        .await?;
    let blocker = repo.pool.get().await?;
    blocker.batch_execute(&format!("CREATE FUNCTION fixture_hold_pressure_cleanup() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.hash=decode('{}','hex') THEN PERFORM pg_advisory_xact_lock(7183942); END IF; RETURN NEW; END $$; CREATE TRIGGER fixture_hold_pressure_cleanup BEFORE INSERT ON blob_reclaims FOR EACH ROW EXECUTE FUNCTION fixture_hold_pressure_cleanup(); SELECT pg_advisory_lock(7183942)", hex::encode(retired[0].hash))).await?;
    let first = {
        let store = store.clone();
        let key = retired[0].clone();
        tokio::spawn(async move { store.reclaim_library(key).await })
    };
    wait_for_cleanup_lock(&repo, 7183942).await?;
    let mut second = {
        let store = store.clone();
        let key = retired[1].clone();
        tokio::spawn(async move { store.reclaim_library(key).await })
    };
    ensure!(
        tokio::time::timeout(Duration::from_millis(50), &mut second)
            .await
            .is_err(),
        "normal cleanup should leave most capacity available for foreground"
    );
    // Actual staging consumes >90% of the configured physical-byte budget.
    let filler = vec![91; 90_000];
    let fill_hash = hex::decode(auth::hash(&filler))?.try_into().unwrap();
    store.put(fill_hash, &filler).await?;
    // Existing collector page refreshes its headroom policy; no polling service.
    let page = {
        let store = store.clone();
        tokio::spawn(async move { store.reclaim_objects_page(false, false, true).await })
    };
    let accelerated = tokio::time::timeout(Duration::from_millis(400), &mut second).await;
    let foreground =
        tokio::time::timeout(Duration::from_millis(200), store.get(fill_hash, owner)).await;
    blocker
        .batch_execute("SELECT pg_advisory_unlock(7183942)")
        .await?;
    ensure!(first.await??, "first cleanup must resume");
    page.await??;
    blocker.batch_execute("DROP TRIGGER fixture_hold_pressure_cleanup ON blob_reclaims; DROP FUNCTION fixture_hold_pressure_cleanup()").await?;
    ensure!(
        accelerated
            .context("low physical-quota headroom did not increase cleanup progress")???,
        "second cleanup must finish"
    );
    ensure!(
        foreground?? == filler,
        "foreground bytes remain readable under cleanup pressure"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL"]
async fn single_slot_cancellation_preserves_cleanup_ownership_and_queued_progress() -> Result<()> {
    let _fault = DATABASE_FAULT.lock().await;
    let repo = repository().await?;
    let dir = tempfile::tempdir()?;
    let owner = [236; 32];
    let store = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.path().into(),
            volume: "storagesingle".into(),
            physical_bytes: 100_000,
            staging_bytes: 100_000,
            max_blob_bytes: 100_000,
            upload_slots: 1,
            reclamation_slots: 16,
            credit_lanes: 16,
            formats: vec!["image/png".into()],
        },
        owner,
        [237; 32],
    )
    .await?;
    let one = b"owned single-slot cleanup";
    let two = b"queued cleanup cancellation";
    let first_key = store
        .put(hex::decode(auth::hash(one))?.try_into().unwrap(), one)
        .await?
        .key;
    let second_key = store
        .put(hex::decode(auth::hash(two))?.try_into().unwrap(), two)
        .await?
        .key;
    repo.pool.get().await?.execute("UPDATE blob_intents SET staging_until=clock_timestamp()-interval '1 second' WHERE owner=$1", &[&owner.as_slice()]).await?;
    repo.enqueue_expired_staging(owner, "storagesingle").await?;
    let blocker = repo.pool.get().await?;
    blocker.batch_execute(&format!("CREATE FUNCTION fixture_hold_single_cleanup() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.hash=decode('{}','hex') THEN PERFORM pg_advisory_xact_lock(7183943); END IF; RETURN NEW; END $$; CREATE TRIGGER fixture_hold_single_cleanup BEFORE INSERT ON blob_reclaims FOR EACH ROW EXECUTE FUNCTION fixture_hold_single_cleanup(); SELECT pg_advisory_lock(7183943)", hex::encode(first_key.hash))).await?;
    let first = {
        let store = store.clone();
        let key = first_key.clone();
        tokio::spawn(async move { store.reclaim_library(key).await })
    };
    wait_for_cleanup_lock(&repo, 7183943).await?;
    let mut queued = {
        let store = store.clone();
        let key = second_key.clone();
        tokio::spawn(async move { store.reclaim_library(key).await })
    };
    ensure!(
        tokio::time::timeout(Duration::from_millis(50), &mut queued)
            .await
            .is_err(),
        "one slot cannot run two cleanups"
    );
    queued.abort();
    ensure!(
        queued.await.unwrap_err().is_cancelled(),
        "queued cleanup cancellation failed"
    );
    first.abort();
    ensure!(
        first.await.unwrap_err().is_cancelled(),
        "cleanup caller cancellation failed"
    );
    let bytes = b"foreground resumes after owned cleanup settles";
    let hash = hex::decode(auth::hash(bytes))?.try_into().unwrap();
    let mut foreground = {
        let store = store.clone();
        tokio::spawn(async move { store.put(hash, bytes).await })
    };
    ensure!(
        tokio::time::timeout(Duration::from_millis(50), &mut foreground)
            .await
            .is_err(),
        "cancelled caller must not release the active single storage slot"
    );
    blocker
        .batch_execute("SELECT pg_advisory_unlock(7183943)")
        .await?;
    let staged = tokio::time::timeout(Duration::from_secs(2), foreground).await???;
    ensure!(
        store.get(staged.key.hash, owner).await? == bytes,
        "foreground must resume after cleanup"
    );
    ensure!(
        store.get(first_key.hash, owner).await.is_err(),
        "owned cleanup must finish after caller cancellation"
    );
    ensure!(
        store.get(second_key.hash, owner).await? == two,
        "queued cleanup must not survive cancellation"
    );
    ensure!(
        store.reclaim_library(second_key).await?,
        "canceled queued ticket must not block a later cleanup"
    );
    blocker.batch_execute("DROP TRIGGER fixture_hold_single_cleanup ON blob_reclaims; DROP FUNCTION fixture_hold_single_cleanup()").await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL"]
async fn stalled_cleanup_preserves_foreground_library_staging() -> Result<()> {
    foreground_staging_during_cleanup(false).await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL"]
async fn pressured_cleanup_still_reserves_foreground_library_staging() -> Result<()> {
    foreground_staging_during_cleanup(true).await
}
