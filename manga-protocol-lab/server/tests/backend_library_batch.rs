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
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and actual strfry"]
async fn mixed_replacements_batch_without_early_retirement() -> Result<()> {
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

    let c1 = chunk(1);
    let c2 = chunk(2);
    let r1 = root(Some(&c1));
    let r2 = root(Some(&c2));
    let now = auth::now() - 30;
    let mut heads = vec![];
    let mut old = vec![];
    for key in 30..62 {
        repo.set_account_admission(id(&public(key)), true).await?;
        let permit = repo.account_permit(id(&public(key))).await?;
        for b in [&c1, &c2] {
            library
                .stage(permit.clone(), ObjectType::Chunk, b.clone())
                .await?;
        }
        for b in [&r1, &r2] {
            library
                .stage(permit.clone(), ObjectType::Root, b.clone())
                .await?;
        }
        let e1 = event(key, &r1, now);
        let e2 = event(key, &r2, now + 1);
        assert_eq!(
            library
                .commit(permit.clone(), e1.clone(), ExpectedHead::Absent)
                .await?,
            StageOutcome::Accepted
        );
        assert_eq!(
            library
                .commit(permit, e2.clone(), ExpectedHead::Exact(id(&e1.id)))
                .await?,
            StageOutcome::Accepted
        );
        old.push(e1);
        heads.push(e2);
    }
    let root_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let backend = Backend::open(
        repo.clone(),
        BackendConfig {
            binary: root_path.join("vendor/strfry/strfry"),
            data_dir: dir.path().join("backend"),
            timeout_seconds: 10,
            map_bytes: 67108864,
        },
    )
    .await?;
    let worker = tokio::spawn(backend.run());
    let result = drain_backend(&repo).await;
    worker.abort();
    let _ = worker.await;
    result?;
    for e in &heads {
        assert!(repo.projection_complete(id(&e.id)).await?);
    }
    for e in &old {
        assert!(repo.visible_event(id(&e.id)).await?.is_none());
    }
    // External time arrangement; retained bytes are observed through the same
    // public object operation used by library readers.
    assert_eq!(library.public_object(id(&auth::hash(&r1))).await?, r1);
    repo.pool.get().await?.execute("UPDATE library_object_holds SET grace_until=clock_timestamp()-interval '1 second' WHERE state='grace'", &[]).await?;
    assert!(library.public_object(id(&auth::hash(&r1))).await.is_err());
    assert!(library.public_object(id(&auth::hash(&c1))).await.is_err());
    assert_eq!(library.public_object(id(&auth::hash(&r2))).await?, r2);
    assert_eq!(library.public_object(id(&auth::hash(&c2))).await?, c2);
    Ok(())
}
