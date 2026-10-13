mod support;

use anyhow::{Context, Result, ensure};
use manga_node::{
    accounts::StageOutcome,
    auth,
    backend::{Backend, BackendConfig},
    db::{DbConfig, Repository},
    library::ExpectedHead,
    library_manifest::{Chunk, Descriptor, Family, FamilyRoot, Limits, Record, Root},
    library_storage::{LibraryObjects, ObjectType},
    upload::{StorageConfig, UploadStore},
};
use std::time::Duration;
use support::*;

fn hash(bytes: &[u8]) -> [u8; 32] {
    hex::decode(auth::hash(bytes)).unwrap().try_into().unwrap()
}

fn chunk(chapter: u8) -> Vec<u8> {
    Chunk::new(
        Family::Chapters,
        vec![Record::Chapter {
            id: [chapter; 32],
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

fn root(bytes: &[u8]) -> Vec<u8> {
    let (_, summary) = Chunk::decode(bytes, &Limits::default()).unwrap();
    Root {
        families: Family::ALL
            .into_iter()
            .map(|family| FamilyRoot {
                family,
                chunks: vec![if family == Family::Chapters {
                    Descriptor::from(&summary)
                } else {
                    Descriptor::empty()
                }],
            })
            .collect(),
    }
    .encode(&Limits::default())
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and real strfry"]
async fn physical_reclaim_does_not_wait_for_an_already_refunded_staging_grant() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let parsed: tokio_postgres::Config = dsn.parse()?;
    ensure!(parsed.get_dbname() == Some("manga_test"), "fixture only");
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
    let owner = [153; 32];
    let limit = 65_536;
    let store = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.path().join("objects"),
            volume: "refundedgrant".into(),
            physical_bytes: limit,
            staging_bytes: 1_000_000,
            max_blob_bytes: limit as u64,
            upload_slots: 4,
            reclamation_slots: 4,
            credit_lanes: 1,
            formats: vec!["image/png".into()],
        },
        owner,
        [154; 32],
    )
    .await?;
    let library = LibraryObjects::new(repo.clone(), store.clone());
    let author: [u8; 32] = hex::decode(public(155))?.try_into().unwrap();
    repo.set_account_admission(author, true).await?;
    let permit = repo.account_permit(author).await?;
    let old_chunk = chunk(156);
    let current_chunk = chunk(157);
    let old_root = root(&old_chunk);
    let current_root = root(&current_chunk);
    let retired = library
        .stage(permit.clone(), ObjectType::Chunk, old_chunk.clone())
        .await?;
    library
        .stage(permit.clone(), ObjectType::Chunk, current_chunk.clone())
        .await?;
    for bytes in [&old_root, &current_root] {
        library
            .stage(permit.clone(), ObjectType::Root, bytes.clone())
            .await?;
    }
    let now = auth::now() - 10;
    let pointer = |bytes: &[u8], time| {
        sign_at(
            155,
            11999,
            vec![],
            serde_json::json!({"manifest":auth::hash(bytes),"version":1}).to_string(),
            time,
        )
    };
    let previous = pointer(&old_root, now);
    assert_eq!(
        library
            .commit(permit.clone(), previous.clone(), ExpectedHead::Absent)
            .await?,
        StageOutcome::Accepted
    );
    let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let backend = Backend::open(
        repo.clone(),
        BackendConfig {
            binary: project.join("vendor/strfry/strfry"),
            data_dir: dir.path().join("backend"),
            timeout_seconds: 10,
            map_bytes: 67_108_864,
        },
    )
    .await?;
    let worker = tokio::spawn(backend.run());
    drain_backend(&repo).await?;
    assert_eq!(library.public_object(retired.key.hash).await?, old_chunk);
    assert_eq!(
        library
            .commit(
                permit.clone(),
                pointer(&current_root, now + 1),
                ExpectedHead::Exact(hex::decode(previous.id)?.try_into().unwrap()),
            )
            .await?,
        StageOutcome::Accepted
    );
    drain_backend(&repo).await?;
    let used: u64 = [&old_chunk, &current_chunk, &old_root, &current_root]
        .iter()
        .map(|bytes| bytes.len() as u64)
        .sum();
    assert!(!store.reclaim_library(retired.key.clone()).await?);
    assert_eq!(library.public_object(retired.key.hash).await?, old_chunk);
    assert_storage_used(&repo, &store, limit as u64, used).await?;

    // External clock arrangement, scoped to the two superseded objects. Actual
    // root replacement and backend acknowledgement established their retirement.
    let retired_hashes = vec![retired.key.hash.to_vec(), hash(&old_root).to_vec()];
    let fault = repo.pool.get().await?;
    for statement in [
        "UPDATE library_object_holds SET grace_until=clock_timestamp()-interval '1 second' WHERE hash=ANY($1) AND state='grace'",
        "UPDATE blob_gc_queue SET due_at=clock_timestamp()-interval '1 second' WHERE hash=ANY($1)",
        "UPDATE account_blob_grants SET lease_until=clock_timestamp()-interval '1 second' WHERE hash=ANY($1)",
        "UPDATE blob_intents SET staging_until=clock_timestamp()-interval '1 second' WHERE hash=ANY($1)",
    ] {
        fault.execute(statement, &[&retired_hashes]).await?;
    }
    drop(fault);

    // This SQL only identifies and holds the external fault target. Refund
    // correctness is observed through real cleanup/admission operations below.
    let mut blocker = repo.pool.get().await?;
    let blocked = blocker.transaction().await?;
    let budget = format!("staging:{}", hex::encode(owner));
    let grant: Vec<u8> = blocked
        .query_one(
            "SELECT grant_id FROM capacity_charges WHERE reservation=$1 AND budget_key=$2 AND refunded",
            &[&retired.reservation.as_slice(), &budget],
        )
        .await?
        .get(0);
    blocked
        .query_one(
            "SELECT id FROM capacity_grants WHERE id=$1 FOR UPDATE",
            &[&grant],
        )
        .await?;
    let reclaimed = tokio::time::timeout(
        Duration::from_secs(2),
        store.reclaim_library(retired.key.clone()),
    )
    .await;
    // The reclaim result was obtained while the unrelated grant stayed locked.
    // Always release the injected fault before propagating a failed assertion.
    blocked.rollback().await?;
    drop(blocker);
    assert!(
        reclaimed.context("physical reclaim waited for a refunded staging grant")??,
        "eligible physical generation was not reclaimed"
    );
    assert!(library.read(&permit, retired.key.hash).await.is_err());
    assert_eq!(
        library.public_object(hash(&current_chunk)).await?,
        current_chunk
    );
    assert_eq!(
        library.public_object(hash(&current_root)).await?,
        current_root
    );
    assert_storage_used(&repo, &store, limit as u64, used - old_chunk.len() as u64).await?;
    assert!(!store.reclaim_library(retired.key.clone()).await?);
    assert_storage_used(&repo, &store, limit as u64, used - old_chunk.len() as u64).await?;

    // A stale receipt cannot refund or delete a subsequently staged generation.
    library
        .stage(permit.clone(), ObjectType::Chunk, old_chunk.clone())
        .await?;
    assert!(!store.reclaim_library(retired.key).await?);
    assert_eq!(library.read(&permit, hash(&old_chunk)).await?, old_chunk);
    assert_storage_used(&repo, &store, limit as u64, used).await?;
    worker.abort();
    let _ = worker.await;
    Ok(())
}
