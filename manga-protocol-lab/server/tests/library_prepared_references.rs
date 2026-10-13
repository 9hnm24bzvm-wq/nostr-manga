mod support;
use anyhow::{Context, Result};
use manga_node::{
    accounts::StageOutcome,
    auth,
    db::{DbConfig, Repository},
    library::ExpectedHead,
    library_manifest::{Chunk, Descriptor, Family, FamilyRoot, Limits, Record, Root},
    library_storage::{LibraryObjects, ObjectType},
    upload::{StorageConfig, UploadStore},
};
use std::time::Duration;
use support::*;
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
fn root(chunk: Option<&[u8]>) -> Vec<u8> {
    let descriptor = chunk
        .map(|b| Chunk::decode(b, &Limits::default()).unwrap().1)
        .as_ref()
        .map(Descriptor::from)
        .unwrap_or_else(Descriptor::empty);
    Root {
        families: Family::ALL
            .into_iter()
            .map(|family| FamilyRoot {
                family,
                chunks: vec![if family == Family::Chapters {
                    descriptor.clone()
                } else {
                    Descriptor::empty()
                }],
            })
            .collect(),
    }
    .encode(&Limits::default())
    .unwrap()
}
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
fn event(key: u8, root: &[u8], time: u64) -> auth::Event {
    sign_at(
        key,
        11999,
        vec![],
        serde_json::json!({"manifest":auth::hash(root),"version":1}).to_string(),
        time,
    )
}
// A prepared signed root is bound to the verified object generations observed
// before it waits for the author. Identical bytes restaged later require retry.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and real filesystem"]
async fn prepared_root_rejects_reclaimed_and_restaged_generations() -> Result<()> {
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
    let dir = tempfile::tempdir()?;
    let store = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.path().join("objects"),
            volume: "prepared-references".into(),
            physical_bytes: 10_000_000,
            staging_bytes: 10_000_000,
            max_blob_bytes: 1_000_000,
            upload_slots: 4,
            reclamation_slots: 4,
            credit_lanes: 16,
            formats: vec!["image/png".into()],
        },
        [91; 32],
        [92; 32],
    )
    .await?;
    let library = LibraryObjects::new(repo.clone(), store.clone());
    let publisher = id(&public(21));
    repo.set_account_admission(publisher, true).await?;
    let permit = repo.account_permit(publisher).await?;
    // Prime the previous-accepted validation with a retained chapter chunk.
    // The next root reuses it while adding a different, still-unretained family.
    let retained = chunk(1);
    let previous_root = root(Some(&retained));
    library
        .stage(permit.clone(), ObjectType::Chunk, retained.clone())
        .await?;
    library
        .stage(permit.clone(), ObjectType::Root, previous_root.clone())
        .await?;
    let previous = event(21, &previous_root, auth::now() - 20);
    assert_eq!(
        library
            .commit(permit.clone(), previous.clone(), ExpectedHead::Absent)
            .await?,
        StageOutcome::Accepted
    );
    let bytes = Chunk::new(
        Family::Series,
        vec![Record::Series {
            id: [2; 32],
            status: "reading".into(),
            updated_at: 1,
        }],
        &Limits::default(),
    )?
    .encode(&Limits::default())?;
    let mut next_root = Root::decode(&previous_root, &Limits::default())?;
    next_root.families[1].chunks[0] =
        Descriptor::from(&Chunk::decode(&bytes, &Limits::default())?.1);
    let manifest = next_root.encode(&Limits::default())?;
    let first_chunk = library
        .stage(permit.clone(), ObjectType::Chunk, bytes.clone())
        .await?;
    let first_root = library
        .stage(permit.clone(), ObjectType::Root, manifest.clone())
        .await?;
    let pointer = event(21, &manifest, auth::now() - 10);
    let mut blocker = repo.pool.get().await?;
    let pid: i32 = blocker
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    let lock = blocker.transaction().await?;
    // SHARE permits preparation's authority reads but holds the exclusive commit.
    lock.query_one(
        "SELECT publisher FROM account_permissions WHERE publisher=$1 FOR SHARE",
        &[&publisher.as_slice()],
    )
    .await?;
    let committing = library.clone();
    let admitted = permit.clone();
    let pending_pointer = pointer.clone();
    let pending = tokio::spawn(async move {
        committing
            .commit(admitted, pending_pointer, ExpectedHead::Any)
            .await
    });
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let waiting: bool = repo.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))", &[&pid]).await?.get(0);
            if waiting { return Ok::<_, anyhow::Error>(()); }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.context("commit did not reach external author fence")??;
    // Advance only fixture clocks. Reclaim uses the actual receipt-backed public
    // lifecycle; no direct blob deletion or private table assertions.
    repo.pool.get().await?.batch_execute("UPDATE blob_intents SET staging_until=clock_timestamp()-interval '1 second'; UPDATE account_blob_grants SET lease_until=clock_timestamp()-interval '1 second'").await?;
    store.reclaim_objects_page(false, false, true).await?;
    assert!(store.get(first_chunk.key.hash, [91; 32]).await.is_err());
    assert!(store.get(first_root.key.hash, [91; 32]).await.is_err());
    let replacement_chunk = library
        .stage(permit.clone(), ObjectType::Chunk, bytes.clone())
        .await?;
    let replacement_root = library
        .stage(permit.clone(), ObjectType::Root, manifest.clone())
        .await?;
    assert_ne!(first_chunk.key.generation, replacement_chunk.key.generation);
    assert_ne!(first_root.key.generation, replacement_root.key.generation);
    lock.rollback().await?;
    drop(blocker);
    assert!(
        pending.await?.is_err(),
        "prepared root silently adopted replacement generations after waiting for its author"
    );
    assert!(repo.visible_event(id(&pointer.id)).await?.is_none());
    assert!(repo.visible_event(id(&previous.id)).await?.is_some());
    assert_eq!(
        library.public_object(id(&auth::hash(&retained))).await?,
        retained
    );
    assert_eq!(
        library
            .commit(permit, pointer.clone(), ExpectedHead::Any)
            .await?,
        StageOutcome::Accepted
    );
    assert_eq!(library.public_object(first_root.key.hash).await?, manifest);
    assert_eq!(library.public_object(first_chunk.key.hash).await?, bytes);
    Ok(())
}
