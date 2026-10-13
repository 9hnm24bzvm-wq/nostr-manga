mod support;
use anyhow::{Context, Result};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    library_manifest::{Chunk, Descriptor, Family, FamilyRoot, Limits, Record, Root},
    library_storage::{LibraryObjects, ObjectType},
    upload::{StorageConfig, UploadStore},
};
use std::time::Duration;
use support::assert_storage_used;
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
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL"]
async fn library_objects_are_private_durable_and_epoch_fenced() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture database required")?;
    let pg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(pg.get_dbname() == Some("manga_test"), "fixture only");
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
            path: dir.path().into(),
            volume: "libraryfixture".into(),
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
    let author = [11; 32];
    let other = [12; 32];
    repo.set_account_admission(author, true).await?;
    repo.set_account_admission(other, true).await?;
    let permit = repo.account_permit(author).await?;
    let other_permit = repo.account_permit(other).await?;
    let bytes = chunk(1);
    let first = library
        .stage(permit.clone(), ObjectType::Chunk, bytes.clone())
        .await?;
    assert!(first.available);
    assert_eq!(library.read(&permit, first.key.hash).await?, bytes);
    // Concurrent real staging/read calls preserve the same immutable receipt.
    let mut staging = tokio::task::JoinSet::new();
    for _ in 0..4 {
        let library = library.clone();
        let permit = permit.clone();
        let bytes = bytes.clone();
        staging.spawn(async move { library.stage(permit, ObjectType::Chunk, bytes).await });
    }
    assert_eq!(library.read(&permit, first.key.hash).await?, bytes);
    while let Some(result) = staging.join_next().await {
        assert_eq!(result??.key, first.key);
    }

    assert!(library.read(&other_permit, first.key.hash).await.is_err());
    assert!(store.public_original(first.key.hash).await.is_err());
    let second = library
        .stage(other_permit.clone(), ObjectType::Chunk, bytes.clone())
        .await?;
    assert_eq!(first.key, second.key);
    assert_eq!(library.read(&other_permit, first.key.hash).await?, bytes);
    assert_storage_used(&repo, &store, 10_000_000, bytes.len() as u64).await?;
    assert!(
        library
            .stage(permit.clone(), ObjectType::Chunk, vec![0; 1_000_001])
            .await
            .is_err()
    );
    assert!(
        library
            .stage(
                permit.clone(),
                ObjectType::Chunk,
                b"not canonical CBOR".to_vec()
            )
            .await
            .is_err()
    );
    // Quarantine must serialize against a root/grant's shared generation fence.
    let fenced = library
        .stage(permit.clone(), ObjectType::Chunk, chunk(33))
        .await?;
    let fenced_hash = hex::encode(fenced.key.hash);
    std::fs::write(
        dir.path()
            .join("objects")
            .join(&fenced_hash[..2])
            .join(&fenced_hash[2..4])
            .join(format!(
                "{}.{}",
                fenced_hash,
                hex::encode(fenced.key.generation)
            )),
        b"corrupt fenced object",
    )?;
    let mut lock_client = repo.pool.get().await?;
    let blocker_pid: i32 = lock_client
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    let lock_tx = lock_client.transaction().await?;
    lock_tx
        .query_one(
            "SELECT hash FROM blob_intents WHERE hash=$1 FOR SHARE",
            &[&fenced.key.hash.as_slice()],
        )
        .await?;
    let reading = library.clone();
    let admitted = permit.clone();
    let fenced_key = fenced.key.clone();
    let quarantine = tokio::spawn(async move { reading.read(&admitted, fenced_key.hash).await });
    tokio::time::timeout(Duration::from_millis(750),async {
        loop {
            anyhow::ensure!(!quarantine.is_finished(),"quarantine crossed the shared admission fence");
            let waiting:bool=repo.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))", &[&blocker_pid]).await?.get(0);
            if waiting {break;}
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        Ok::<_,anyhow::Error>(())
    }).await??;
    lock_tx.commit().await?;
    drop(lock_client);
    assert!(quarantine.await?.is_err());
    assert!(repo.blob_quarantined(&fenced.key).await?);
    assert!(library.read(&permit, fenced.key.hash).await.is_err());
    // A structurally valid root is still only private staging, not complete hosting.
    let root = Root {
        families: Family::ALL
            .into_iter()
            .map(|family| FamilyRoot {
                family,
                chunks: vec![Descriptor::empty()],
            })
            .collect(),
    }
    .encode(&Limits::default())?;
    let root = library
        .stage(permit.clone(), ObjectType::Root, root)
        .await?;
    assert!(store.public_original(root.key.hash).await.is_err());
    assert!(library.public_object(root.key.hash).await.is_err());
    assert!(library.public_view(author, None).await?.is_none());
    // Freeze final authority admission after bytes can become durable, then revoke.
    let mut blocker = repo.pool.get().await?;
    let tx = blocker.transaction().await?;
    tx.query_one(
        "SELECT version FROM account_permissions WHERE publisher=$1 FOR UPDATE",
        &[&author.as_slice()],
    )
    .await?;
    let more = chunk(2);
    let hash: [u8; 32] = hex::decode(auth::hash(&more))?.try_into().unwrap();
    let l = library.clone();
    let p = permit.clone();
    let task = tokio::spawn(async move { l.stage(p, ObjectType::Chunk, more).await });
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if repo.blob_intent(hash).await?.is_some_and(|x| x.available) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    tx.execute(
        "UPDATE account_permissions SET allowed=false,version=version+1 WHERE publisher=$1",
        &[&author.as_slice()],
    )
    .await?;
    tx.commit().await?;
    drop(blocker);
    assert!(task.await?.is_err());
    assert!(repo.blob_intent(hash).await?.unwrap().available); // Accounted staging remains recoverable.
    repo.set_account_admission(author, true).await?;
    let readmitted = repo.account_permit(author).await?;
    assert!(
        library.read(&readmitted, hash).await.is_err(),
        "revoked in-flight staging must not create a grant recoverable by readmission"
    );
    library
        .stage(readmitted.clone(), ObjectType::Chunk, chunk(2))
        .await?;
    assert_eq!(library.read(&readmitted, hash).await?, chunk(2));
    assert!(library.read(&permit, first.key.hash).await.is_err());
    assert_eq!(library.read(&other_permit, first.key.hash).await?, bytes);
    // Hash failure cannot expose corrupt stored bytes or silently refund a charge.
    let h = hex::encode(first.key.hash);
    let path = dir
        .path()
        .join("objects")
        .join(&h[..2])
        .join(&h[2..4])
        .join(format!("{}.{}", h, hex::encode(first.key.generation)));
    std::fs::write(path, b"corrupt fixture bytes")?;
    assert!(library.read(&other_permit, first.key.hash).await.is_err());
    assert!(
        library
            .stage(other_permit, ObjectType::Chunk, bytes)
            .await
            .is_err()
    );
    assert!(repo.blob_quarantined(&first.key).await?);
    assert!(repo.blob_intent(first.key.hash).await?.unwrap().available); // Quarantine does not fabricate physical cleanup or change receipt availability.
    println!(
        "Durable private grants, shared physical deduplication, root staging isolation, revoked post-I/O admission and corruption rejection passed"
    );
    Ok(())
}
