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
// Actual staged writes held at the account boundary must not consume the
// independent capacity needed by private and publicly retained library reads.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and real filesystem"]
async fn reads_progress_while_staging_waits_for_another_author() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(cfg.get_dbname() == Some("manga_test"), "fixture only");
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 10,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    // Keep the deliberate external row-lock fault alive past the read deadline.
    // These session settings arrange the fault; returned read bytes are the oracle.
    let mut sessions = Vec::new();
    for _ in 0..10 {
        let session = repo.pool.get().await?;
        session
            .batch_execute("SET lock_timeout='10s'; SET statement_timeout='15s'")
            .await?;
        sessions.push(session);
    }
    drop(sessions);
    let dir = tempfile::tempdir()?;
    let store = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.path().join("objects"),
            volume: "library-read-isolation".into(),
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
    let writer = id(&public(21));
    let reader = id(&public(22));
    repo.set_account_admission(writer, true).await?;
    repo.set_account_admission(reader, true).await?;
    let writing = repo.account_permit(writer).await?;
    let reading = repo.account_permit(reader).await?;
    let bytes = chunk(1);
    let staged = library
        .stage(writing.clone(), ObjectType::Chunk, bytes.clone())
        .await?;
    library
        .stage(reading.clone(), ObjectType::Chunk, bytes.clone())
        .await?;
    let manifest = root(Some(&bytes));
    let retained = library
        .stage(reading.clone(), ObjectType::Root, manifest.clone())
        .await?;
    assert_eq!(
        library
            .commit(
                reading.clone(),
                event(22, &manifest, auth::now() - 1),
                ExpectedHead::Absent
            )
            .await?,
        StageOutcome::Accepted
    );

    let mut client = repo.pool.get().await?;
    let pid: i32 = client
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    let lock = client.transaction().await?;
    lock.query_one(
        "SELECT publisher FROM account_permissions WHERE publisher=$1 FOR UPDATE",
        &[&writer.as_slice()],
    )
    .await?;
    let mut pending = tokio::task::JoinSet::new();
    for _ in 0..4 {
        let library = library.clone();
        let writing = writing.clone();
        let bytes = bytes.clone();
        pending.spawn(async move { library.stage(writing, ObjectType::Chunk, bytes).await });
    }
    // The external wait graph proves all four actual writes reached the fault.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let waiting:i64=repo.pool.get().await?.query_one("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND $1=ANY(pg_blocking_pids(pid))", &[&pid]).await?.get(0);
            if waiting==4 {return Ok::<_,anyhow::Error>(());}
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.context("staged writes did not reach external author fence")??;
    let (private, public) = tokio::join!(
        tokio::time::timeout(
            Duration::from_secs(3),
            library.read(&reading, staged.key.hash)
        ),
        tokio::time::timeout(
            Duration::from_secs(3),
            library.public_object(retained.key.hash)
        ),
    );
    lock.rollback().await?;
    drop(client);
    while let Some(result) = pending.join_next().await {
        assert_eq!(result??.key, staged.key);
    }
    assert_eq!(
        private
            .context("private read waited behind unrelated writes")?
            .context("private read rejected while mutation capacity occupied")?,
        bytes
    );
    assert_eq!(
        public
            .context("public read waited behind unrelated writes")?
            .context("public read rejected while mutation capacity occupied")?,
        manifest
    );
    Ok(())
}
