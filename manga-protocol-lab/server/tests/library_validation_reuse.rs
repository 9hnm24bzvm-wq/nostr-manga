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
// Immutable validation can be reused, but it never supplies authority, health
// or a current storage generation. This exercises the actual commit interface.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and real filesystem"]
async fn accepted_unchanged_root_reuses_validation_without_reusing_authority() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture database")?;
    anyhow::ensure!(
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
    let dir = tempfile::tempdir()?;
    let store = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.path().into(),
            volume: "validation-reuse".into(),
            physical_bytes: 10000000,
            staging_bytes: 10000000,
            max_blob_bytes: 1000000,
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
    let c1 = chunk(1);
    let c2 = chunk(2);
    let r1 = root(Some(&c1));
    let r2 = root(Some(&c2));
    for bytes in [&c1, &c2] {
        library
            .stage(permit.clone(), ObjectType::Chunk, bytes.clone())
            .await?;
    }
    for bytes in [&r1, &r2] {
        library
            .stage(permit.clone(), ObjectType::Root, bytes.clone())
            .await?;
    }
    let first = event(21, &r1, auth::now() - 20);
    assert_eq!(
        library
            .commit(permit.clone(), first.clone(), ExpectedHead::Absent)
            .await?,
        StageOutcome::Accepted
    );
    let second = event(21, &r1, first.created_at + 1);
    let mut connection = repo.pool.get().await?;
    let blocker: i32 = connection
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    let fault = connection.transaction().await?;
    fault.batch_execute("LOCK TABLE library_chunk_summaries IN ACCESS EXCLUSIVE MODE; LOCK TABLE blob_retention_credits IN ACCESS EXCLUSIVE MODE").await?;
    let mut save = tokio::spawn({
        let library = library.clone();
        let permit = permit.clone();
        let second = second.clone();
        async move {
            library
                .commit(permit, second, ExpectedHead::Exact(id(&first.id)))
                .await
        }
    });
    let completed = tokio::select! {
        result = &mut save => Some(result??),
        blocked = wait_for_blocked_session(&repo, blocker) => { blocked?; None }
    };
    fault.rollback().await?;
    drop(connection);
    // Clean up the owned commit before reporting the red behavior, so failure
    // cannot strand locks or leave a misleading partial fixture state.
    if completed.is_none() {
        let _ = save.await?;
    }
    assert_eq!(
        completed,
        Some(StageOutcome::Accepted),
        "unchanged accepted objects unnecessarily waited for immutable-summary or already-retained credit storage"
    );
    assert_eq!(library.public_object(id(&auth::hash(&r1))).await?, r1);
    // A different root does need its previously unaccepted chunk summary.
    let third = event(21, &r2, second.created_at + 1);
    let mut connection = repo.pool.get().await?;
    let blocker: i32 = connection
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    let fault = connection.transaction().await?;
    fault
        .batch_execute("LOCK TABLE library_chunk_summaries IN ACCESS EXCLUSIVE MODE")
        .await?;
    let save = tokio::spawn({
        let library = library.clone();
        let permit = permit.clone();
        let third = third.clone();
        async move {
            library
                .commit(permit, third, ExpectedHead::Exact(id(&second.id)))
                .await
        }
    });
    wait_for_blocked_session(&repo, blocker).await?;
    fault.rollback().await?;
    drop(connection);
    assert_eq!(save.await??, StageOutcome::Accepted);
    assert_eq!(library.public_object(id(&auth::hash(&r2))).await?, r2);
    // Reusing the exact accepted bytes does not revive a revoked account permit.
    repo.set_account_admission(publisher, false).await?;
    let fourth = event(21, &r2, third.created_at + 1);
    assert!(
        library
            .commit(permit, fourth.clone(), ExpectedHead::Any)
            .await
            .is_err()
    );
    repo.set_account_admission(publisher, true).await?;
    let permit = repo.account_permit(publisher).await?;
    assert_eq!(
        library
            .commit(permit.clone(), fourth.clone(), ExpectedHead::Any)
            .await?,
        StageOutcome::Accepted
    );
    // A fresh application instance has no trusted previous validation proof.
    // It must conservatively reload summaries even for identical root bytes.
    let cold = LibraryObjects::new(repo.clone(), store.clone());
    let fifth = event(21, &r2, fourth.created_at + 1);
    let mut connection = repo.pool.get().await?;
    let blocker: i32 = connection
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    let fault = connection.transaction().await?;
    fault
        .batch_execute("LOCK TABLE library_chunk_summaries IN ACCESS EXCLUSIVE MODE")
        .await?;
    let save = tokio::spawn({
        let permit = permit.clone();
        let fifth = fifth.clone();
        async move { cold.commit(permit, fifth, ExpectedHead::Any).await }
    });
    wait_for_blocked_session(&repo, blocker).await?;
    fault.rollback().await?;
    drop(connection);
    assert_eq!(save.await??, StageOutcome::Accepted);
    // The older process-local accepted event is now stale. Refresh through the
    // public operation before testing corruption against an actual warm entry.
    let sixth = event(21, &r2, fifth.created_at + 1);
    assert_eq!(
        library
            .commit(permit.clone(), sixth.clone(), ExpectedHead::Any)
            .await?,
        StageOutcome::Accepted
    );
    // The immutable proof cannot hide newly observed corruption in a child.
    let intent = repo
        .blob_intent(id(&auth::hash(&c2)))
        .await?
        .context("staged child")?;
    let hash = hex::encode(intent.key.hash);
    std::fs::write(
        dir.path()
            .join("objects")
            .join(&hash[..2])
            .join(&hash[2..4])
            .join(format!("{}.{}", hash, hex::encode(intent.key.generation))),
        b"corrupt retained child",
    )?;
    assert!(library.public_object(intent.key.hash).await.is_err());
    let seventh = event(21, &r2, sixth.created_at + 1);
    assert!(
        library
            .commit(permit, seventh.clone(), ExpectedHead::Any)
            .await
            .is_err()
    );
    assert!(repo.current_library_head(publisher).await?.is_none());
    assert!(repo.visible_event(id(&seventh.id)).await?.is_none());
    Ok(())
}
