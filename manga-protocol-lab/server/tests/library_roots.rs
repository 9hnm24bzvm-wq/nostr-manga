mod support;
use anyhow::{Context, Result};
use manga_node::{
    accounts::StageOutcome,
    auth,
    backend::{Backend, BackendConfig},
    db::{DbConfig, Repository},
    library::{ExpectedHead, HeadConflict},
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
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and actual strfry"]
async fn root_commit_retention_projection_and_health() -> Result<()> {
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
            reclamation_slots: 4,
            credit_lanes: 16,
            formats: vec!["image/png".into()],
        },
        [91; 32],
        [92; 32],
    )
    .await?;
    let library = LibraryObjects::new(repo.clone(), store.clone());
    for key in [21, 22, 23] {
        repo.set_account_admission(id(&public(key)), true).await?;
    }
    let permit = repo.account_permit(id(&public(21))).await?;
    let c1 = chunk(1);
    let c2 = chunk(2);
    let r1 = root(Some(&c1));
    let r2 = root(Some(&c2));
    let empty = root(None);
    for b in [&c1, &c2] {
        library
            .stage(permit.clone(), ObjectType::Chunk, b.clone())
            .await?;
    }
    for b in [&r1, &r2, &empty] {
        library
            .stage(permit.clone(), ObjectType::Root, b.clone())
            .await?;
    }
    assert!(library.public_object(id(&auth::hash(&r1))).await.is_err());
    let now = auth::now() - 30;
    let e1 = event(21, &r1, now);
    let e2 = event(21, &r2, now + 1);
    let e3 = event(21, &r1, now + 2);
    assert_eq!(
        library
            .commit(permit.clone(), e1.clone(), ExpectedHead::Absent)
            .await?,
        StageOutcome::Accepted
    );
    assert_eq!(library.public_object(id(&auth::hash(&r1))).await?, r1);
    assert_eq!(
        library
            .commit(permit.clone(), e1.clone(), ExpectedHead::Exact([9; 32]))
            .await?,
        StageOutcome::Current
    );
    assert_eq!(
        library
            .commit(permit.clone(), e2.clone(), ExpectedHead::Exact(id(&e1.id)))
            .await?,
        StageOutcome::Accepted
    );
    assert_eq!(
        library.public_object(id(&auth::hash(&r1))).await?,
        r1,
        "predecessor bytes vanished before backend retirement"
    );
    assert_eq!(
        library
            .commit(permit.clone(), e3.clone(), ExpectedHead::Exact(id(&e2.id)))
            .await?,
        StageOutcome::Accepted
    );
    assert_eq!(
        library
            .commit(permit.clone(), e1.clone(), ExpectedHead::Any)
            .await?,
        StageOutcome::Superseded
    );
    let missing = root(Some(&chunk(99)));
    library
        .stage(permit.clone(), ObjectType::Root, missing.clone())
        .await?;
    assert!(
        library
            .commit(
                permit.clone(),
                event(21, &missing, now + 3),
                ExpectedHead::Any
            )
            .await
            .is_err()
    );
    let stranger = repo.account_permit(id(&public(23))).await?;
    assert!(
        library
            .commit(stranger, event(23, &r1, now), ExpectedHead::Any)
            .await
            .is_err()
    );
    // Independent account has its own grants despite sharing physical content.
    let p2 = repo.account_permit(id(&public(22))).await?;
    for b in [&c1, &c2] {
        library
            .stage(p2.clone(), ObjectType::Chunk, b.clone())
            .await?;
    }
    for b in [&r1, &r2, &empty] {
        library
            .stage(p2.clone(), ObjectType::Root, b.clone())
            .await?;
    }
    let a = event(22, &r1, now);
    library
        .commit(p2.clone(), a.clone(), ExpectedHead::Absent)
        .await?;
    let (a_result, b_result) = tokio::join!(
        library.commit(
            p2.clone(),
            event(22, &r2, now + 1),
            ExpectedHead::Exact(id(&a.id))
        ),
        library.commit(
            p2.clone(),
            event(22, &empty, now + 1),
            ExpectedHead::Exact(id(&a.id))
        )
    );
    assert_eq!(
        usize::from(a_result.is_ok()) + usize::from(b_result.is_ok()),
        1
    );
    let other_kept_r2 = a_result.is_ok();
    let error = if let Err(e) = a_result {
        e
    } else {
        b_result.unwrap_err()
    };
    assert!(error.is::<HeadConflict>());
    // No backend was running while heads changed; old holds must survive the queue.
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
            map_bytes: 64 * 1024 * 1024,
        },
    )
    .await?;
    let worker = tokio::spawn(backend.run());
    let result = drain_backend(&repo).await;
    worker.abort();
    let _ = worker.await;
    result?;
    assert!(repo.projection_complete(id(&e3.id)).await?);
    assert!(repo.visible_event(id(&e1.id)).await?.is_none());
    assert_eq!(library.public_object(id(&auth::hash(&c1))).await?, c1);
    assert_eq!(library.public_object(id(&auth::hash(&r1))).await?, r1);
    repo.pool.get().await?.execute("UPDATE library_object_holds SET grace_until=statement_timestamp()-interval '1 second' WHERE state='grace'", &[]).await?;
    assert_eq!(
        library.public_object(id(&auth::hash(&r2))).await.is_ok(),
        other_kept_r2,
        "expired predecessor remains public only if another current head retains it"
    );
    let used = [&c1, &c2, &r1, &r2, &empty, &missing]
        .iter()
        .map(|bytes| bytes.len() as u64)
        .sum();
    assert_storage_used(&repo, &store, 10_000_000, used).await?;
    // Negative health propagates to the signed head and its root response.
    let intent = repo.blob_intent(id(&auth::hash(&c1))).await?.unwrap();
    let corrupt_hash = hex::encode(intent.key.hash);
    std::fs::write(
        dir.path()
            .join("objects")
            .join(&corrupt_hash[..2])
            .join(&corrupt_hash[2..4])
            .join(format!(
                "{}.{}",
                corrupt_hash,
                hex::encode(intent.key.generation)
            )),
        b"corrupt retained child",
    )?;
    assert!(library.public_object(intent.key.hash).await.is_err());
    assert!(repo.visible_event(id(&e3.id)).await?.is_none());
    assert!(library.public_object(id(&auth::hash(&r1))).await.is_err());
    println!(
        "Root completeness, exact retry, CAS races, cross-account grants, actual strfry retirement, reused holds, grace and quarantine passed"
    );
    Ok(())
}
