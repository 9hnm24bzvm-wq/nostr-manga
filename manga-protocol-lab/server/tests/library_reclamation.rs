mod support;
use anyhow::{Context, Result};
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

async fn expire(repo: &Repository) -> Result<()> {
    repo.pool.get().await?.batch_execute("UPDATE library_object_holds SET grace_until=clock_timestamp()-interval '1 second' WHERE state='grace';UPDATE blob_gc_queue SET due_at=clock_timestamp()-interval '1 second';UPDATE account_blob_grants SET lease_until=clock_timestamp()-interval '1 second';UPDATE blob_intents SET staging_until=clock_timestamp()-interval '1 second'").await?;
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and actual strfry"]
async fn reclamation_is_reference_lease_receipt_and_generation_fenced() -> Result<()> {
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
            path: dir.path().into(),
            volume: "libraryroots".into(),
            physical_bytes: 10_000_000,
            staging_bytes: 10_000_000,
            max_blob_bytes: 1_000_000,
            upload_slots: 4,
            reclamation_slots: 8,
            credit_lanes: 16,
            formats: vec!["image/png".into()],
        },
        [91; 32],
        [92; 32],
    )
    .await?;
    let library = LibraryObjects::new(repo.clone(), store.clone());

    for key in [21, 22] {
        repo.set_account_admission(id(&public(key)), true).await?;
    }
    let p1 = repo.account_permit(id(&public(21))).await?;
    let p2 = repo.account_permit(id(&public(22))).await?;
    let c1 = chunk(1);
    let c2 = chunk(2);
    let c3 = chunk(3);
    let r1 = root(Some(&c1));
    let r2 = root(Some(&c2));
    let r3 = root(Some(&c3));
    let mut accounted: u64 = [&c1, &c2, &c3, &r1, &r2, &r3]
        .iter()
        .map(|b| b.len() as u64)
        .sum();
    for p in [&p1, &p2] {
        for b in [&c1, &c2, &c3] {
            library
                .stage(p.clone(), ObjectType::Chunk, b.clone())
                .await?;
        }
        for b in [&r1, &r2, &r3] {
            library
                .stage(p.clone(), ObjectType::Root, b.clone())
                .await?;
        }
    }
    let old_chunk = repo.blob_intent(id(&auth::hash(&c1))).await?.unwrap();
    let old_root = repo.blob_intent(id(&auth::hash(&r1))).await?.unwrap();
    let now = auth::now() - 30;
    let a1 = event(21, &r1, now);
    let a2 = event(21, &r2, now + 1);
    assert_eq!(
        library
            .commit(p1.clone(), a1.clone(), ExpectedHead::Absent)
            .await?,
        StageOutcome::Accepted
    );
    library
        .commit(p1.clone(), a2.clone(), ExpectedHead::Exact(id(&a1.id)))
        .await?;
    // Pending backend work cannot authorize collection.
    assert!(!store.reclaim_library(old_chunk.key.clone()).await?);
    let root_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let backend_dir = tempfile::tempdir()?;
    let backend = Backend::open(
        repo.clone(),
        BackendConfig {
            binary: root_path.join("vendor/strfry/strfry"),
            data_dir: backend_dir.path().into(),
            timeout_seconds: 10,
            map_bytes: 67108864,
        },
    )
    .await?;
    let worker = tokio::spawn(backend.run());
    drain_backend(&repo).await?;
    assert!(
        !store.reclaim_library(old_chunk.key.clone()).await?,
        "unexpired grace must protect bytes"
    );
    repo.pool.get().await?.batch_execute("UPDATE library_object_holds SET grace_until=clock_timestamp()-interval '1 second' WHERE state='grace';UPDATE blob_gc_queue SET due_at=clock_timestamp()-interval '1 second'").await?;
    assert!(
        !store.reclaim_library(old_chunk.key.clone()).await?,
        "fresh staging lease must protect bytes"
    );
    let b1 = event(22, &r1, now);
    library
        .commit(p2.clone(), b1.clone(), ExpectedHead::Absent)
        .await?;
    expire(&repo).await?;
    assert!(
        !store.reclaim_library(old_chunk.key.clone()).await?,
        "another author's current hold must protect shared content"
    );
    assert_eq!(library.public_object(old_chunk.key.hash).await?, c1);
    let b2 = event(22, &r2, now + 1);
    library
        .commit(p2.clone(), b2, ExpectedHead::Exact(id(&b1.id)))
        .await?;
    drain_backend(&repo).await?;
    expire(&repo).await?;
    assert_storage_used(&repo, &store, 10_000_000, accounted).await?;
    assert!(store.reclaim_library(old_chunk.key.clone()).await?);
    assert!(repo.blob_intent(old_chunk.key.hash).await?.is_none());
    accounted -= c1.len() as u64;
    assert_storage_used(&repo, &store, 10_000_000, accounted).await?;
    assert!(!store.reclaim_library(old_chunk.key.clone()).await?);
    assert_eq!(library.public_object(id(&auth::hash(&c2))).await?, c2);
    // Simulate a crash after the durable claim, then after unlink before refund.
    let health = repo.storage_health_epoch().await?;
    repo.pool
        .get()
        .await?
        .execute(
            "INSERT INTO blob_reclaims(hash,generation) VALUES($1,$2)",
            &[
                &old_root.key.hash.as_slice(),
                &old_root.key.generation.as_slice(),
            ],
        )
        .await?;
    assert!(repo.blob_intent(old_root.key.hash).await?.is_none());
    assert!(
        library
            .stage(p1.clone(), ObjectType::Root, r1.clone())
            .await
            .is_err()
    );
    assert!(library.read(&p1, old_root.key.hash).await.is_err());
    assert!(
        library
            .commit(p1.clone(), event(21, &r1, now + 2), ExpectedHead::Any)
            .await
            .is_err()
    );
    let h = hex::encode(old_root.key.hash);
    let path = dir
        .path()
        .join("objects")
        .join(&h[..2])
        .join(&h[2..4])
        .join(format!("{}.{}", h, hex::encode(old_root.key.generation)));
    assert_storage_used(&repo, &store, 10_000_000, accounted).await?;
    std::fs::remove_file(path)?;
    assert_storage_used(&repo, &store, 10_000_000, accounted).await?;
    assert_eq!(store.reclaim_objects_page(false, false, false).await?, 1);
    accounted -= r1.len() as u64;
    assert_storage_used(&repo, &store, 10_000_000, accounted).await?;
    assert_eq!(store.reclaim_objects_page(false, false, false).await?, 0);
    assert_eq!(repo.storage_health_epoch().await?, health);
    // Reusing a hash creates a new generation. An old cleanup cannot touch it.
    let new_chunk = library
        .stage(p1.clone(), ObjectType::Chunk, c1.clone())
        .await?;
    assert_ne!(old_chunk.key, new_chunk.key);
    library
        .stage(p1.clone(), ObjectType::Root, r1.clone())
        .await?;
    accounted += (c1.len() + r1.len()) as u64;
    let a3 = event(21, &r1, now + 3);
    library
        .commit(p1.clone(), a3.clone(), ExpectedHead::Exact(id(&a2.id)))
        .await?;
    assert_storage_used(&repo, &store, 10_000_000, accounted).await?;
    assert!(!store.reclaim_library(old_chunk.key.clone()).await?);
    assert_storage_used(&repo, &store, 10_000_000, accounted).await?;
    assert_eq!(library.public_object(new_chunk.key.hash).await?, c1);
    // A retired library object retained as group staging is deliberately excluded.
    let a4 = event(21, &r3, now + 4);
    library
        .commit(p1.clone(), a4.clone(), ExpectedHead::Exact(id(&a3.id)))
        .await?;
    library
        .commit(
            p1.clone(),
            event(21, &r1, now + 5),
            ExpectedHead::Exact(id(&a4.id)),
        )
        .await?;
    drain_backend(&repo).await?;
    expire(&repo).await?;
    let group = [7u8; 32];
    let c3hash = id(&auth::hash(&c3));
    repo.pool
        .get()
        .await?
        .execute(
            "INSERT INTO managed_groups(group_key,key_ref) VALUES($1,'synthetic-no-signing-key')",
            &[&group.as_slice()],
        )
        .await?;
    repo.pool
        .get()
        .await?
        .execute(
            "INSERT INTO group_blob_grants(group_key,hash,generation,lease_until) SELECT $1,hash,generation,clock_timestamp()+interval '5 minutes' FROM blob_intents WHERE hash=$2",
            &[&group.as_slice(), &c3hash.as_slice()],
        )
        .await?;
    let protected = repo.blob_intent(c3hash).await?.unwrap();
    assert!(!store.reclaim_library(protected.key.clone()).await?);
    assert_eq!(store.get(protected.key.hash, [91; 32]).await?, c3);
    repo.pool.get().await?.execute("UPDATE group_blob_grants SET lease_until=clock_timestamp()-interval '1 second' WHERE group_key=$1", &[&group.as_slice()]).await?;
    repo.pool
        .get()
        .await?
        .execute(
            "UPDATE blob_gc_queue SET due_at=clock_timestamp()-interval '1 second' WHERE hash=$1",
            &[&c3hash.as_slice()],
        )
        .await?;
    assert!(
        store.reclaim_library(protected.key).await?,
        "retired object should be reclaimed after group lease expires"
    );
    accounted -= c3.len() as u64;
    repo.set_account_admission(id(&public(22)), false).await?;
    let current = repo.blob_intent(id(&auth::hash(&c2))).await?.unwrap();
    assert!(
        !store.reclaim_library(current.key).await?,
        "account revocation is not content deletion"
    );
    assert_storage_used(&repo, &store, 10_000_000, accounted).await?;
    // Event history is separately aged by trusted retirement time. Completed
    // bodies/outbox can go away without erasing the author's winner watermark.
    assert_eq!(repo.prune_library_history().await?.removed, 0);
    let old_id = id(&a1.id);
    let conn = repo.pool.get().await?;
    conn.execute(
        "UPDATE events SET retired_at=clock_timestamp()-interval '6 minutes' WHERE id=$1",
        &[&old_id.as_slice()],
    )
    .await?;
    worker.abort();
    let _ = worker.await;
    let pending:i64=conn.query_one("INSERT INTO backend_outbox(event_id,action,payload) VALUES($1,'delete','{}') RETURNING id", &[&old_id.as_slice()]).await?.get(0);
    assert_eq!(
        repo.prune_library_history().await?.removed,
        0,
        "pending projection blocks pruning"
    );
    conn.execute("DELETE FROM backend_outbox WHERE id=$1", &[&pending])
        .await?;
    let before = library
        .public_view(id(&public(21)), None)
        .await?
        .context("current view")?;
    assert_eq!(repo.prune_library_history().await?.removed, 1);
    assert_eq!(repo.prune_library_history().await?.removed, 0);
    let after = library
        .public_view(id(&public(21)), None)
        .await?
        .context("current view after pruning")?;
    assert_eq!(
        serde_json::to_value(&before.event)?,
        serde_json::to_value(&after.event)?
    );
    assert_eq!(
        library.commit(p1.clone(), a1, ExpectedHead::Any).await?,
        StageOutcome::Superseded
    );
    assert!(repo.visible_event(old_id).await?.is_none());
    Ok(())
}
