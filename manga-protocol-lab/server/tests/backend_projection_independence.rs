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
// Public lifecycle regression: a stalled author's retirement must not hold back
// unrelated verified imports or the first bounded retirement transaction.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and actual strfry"]
async fn imports_and_four_retirements_progress_before_a_fifth_author_unlocks() -> Result<()> {
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
            volume: "projection-independence".into(),
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
    let library = LibraryObjects::new(repo.clone(), store);
    let mut keys: Vec<_> = (30..35).collect();
    keys.sort_by_key(|key| public(*key));
    let mut previous = Vec::new();
    let mut current = Vec::new();
    for (index, key) in keys.iter().copied().enumerate() {
        repo.set_account_admission(id(&public(key)), true).await?;
        let permit = repo.account_permit(id(&public(key))).await?;
        let old_chunk = chunk(index as u8 + 1);
        let new_chunk = chunk(index as u8 + 10);
        let old_root = root(Some(&old_chunk));
        let new_root = root(Some(&new_chunk));
        for bytes in [&old_chunk, &new_chunk] {
            library
                .stage(permit.clone(), ObjectType::Chunk, bytes.clone())
                .await?;
        }
        for bytes in [&old_root, &new_root] {
            library
                .stage(permit.clone(), ObjectType::Root, bytes.clone())
                .await?;
        }
        let old_event = event(key, &old_root, auth::now() - 30);
        let new_event = event(key, &new_root, old_event.created_at + 1);
        assert_eq!(
            library
                .commit(permit.clone(), old_event.clone(), ExpectedHead::Absent)
                .await?,
            StageOutcome::Accepted
        );
        assert_eq!(
            library
                .commit(
                    permit,
                    new_event.clone(),
                    ExpectedHead::Exact(id(&old_event.id))
                )
                .await?,
            StageOutcome::Accepted
        );
        previous.push((old_root, old_chunk));
        current.push((new_event, new_root, new_chunk));
    }
    let unrelated = sign_at(
        90,
        1,
        vec![],
        "independent verified import".into(),
        auth::now(),
    );
    repo.stage_regular_event(&unrelated).await?;
    // This external database lock is fault setup, not an outcome oracle. Wait on
    // PostgreSQL's actual blocker relation rather than statement text or a sleep.
    let mut blocker = repo.pool.get().await?;
    let blocker_pid: i32 = blocker
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    let transaction = blocker.transaction().await?;
    transaction
        .query_one(
            "SELECT publisher FROM account_permissions WHERE publisher=$1 FOR UPDATE",
            &[&id(&public(keys[4])).as_slice()],
        )
        .await?;
    let root_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let backend = Backend::open(
        repo.clone(),
        BackendConfig {
            binary: root_path.join("vendor/strfry/strfry"),
            data_dir: dir.path().join("backend"),
            timeout_seconds: 10,
            map_bytes: 67_108_864,
        },
    )
    .await?;
    let worker = tokio::spawn(backend.run());
    let result: Result<()> = async {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let waiting: bool = repo.pool.get().await?.query_one(
                    "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))", &[&blocker_pid]
                ).await?.get(0);
                if waiting { return Ok::<_, anyhow::Error>(()); }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.context("projector did not reach external author lock")??;
        assert!(repo.projection_complete(id(&unrelated.id)).await?,
            "verified unrelated import waited behind another author's retirement");
        assert_eq!(serde_json::to_value(repo.visible_event(id(&unrelated.id)).await?.context("visible import")?.as_ref())?, serde_json::to_value(&unrelated)?);
        // Advancing fixture grace is external clock arrangement. The outcome is
        // public object availability, never a hold count or private queue state.
        repo.pool.get().await?.execute("UPDATE library_object_holds SET grace_until=clock_timestamp()-interval '1 second' WHERE state='grace'", &[]).await?;
        for (old_root, old_chunk) in &previous[..4] {
            assert!(library.public_object(id(&auth::hash(old_root))).await.is_err(), "completed retirement still publicly retained expired root");
            assert!(library.public_object(id(&auth::hash(old_chunk))).await.is_err(), "completed retirement still publicly retained expired chunk");
        }
        assert_eq!(library.public_object(id(&auth::hash(&previous[4].0))).await?, previous[4].0,
            "blocked retirement lost its predecessor before acknowledgement");
        Ok(())
    }.await;
    // Always release the fault and stop the background task, including a red run.
    transaction.rollback().await?;
    drop(blocker);
    if result.is_ok() {
        drain_backend(&repo).await?;
        assert_eq!(
            library
                .public_object(id(&auth::hash(&previous[4].0)))
                .await?,
            previous[4].0,
            "last retirement did not receive fresh post-acknowledgement grace"
        );
        repo.pool.get().await?.execute("UPDATE library_object_holds SET grace_until=clock_timestamp()-interval '1 second' WHERE state='grace'", &[]).await?;
        assert!(
            library
                .public_object(id(&auth::hash(&previous[4].0)))
                .await
                .is_err()
        );
        for (head, root, chunk) in current {
            assert!(repo.projection_complete(id(&head.id)).await?);
            assert_eq!(library.public_object(id(&auth::hash(&root))).await?, root);
            assert_eq!(library.public_object(id(&auth::hash(&chunk))).await?, chunk);
        }
    }
    worker.abort();
    let _ = worker.await;
    result
}
