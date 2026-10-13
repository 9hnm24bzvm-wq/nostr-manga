mod support;
use anyhow::{Context, Result};
use manga_node::{
    accounts::StageOutcome,
    auth,
    backend::{Backend, BackendConfig},
    db::{DbConfig, Repository},
    library::ExpectedHead,
    library_manifest::{Chunk, Descriptor, Family, FamilyRoot, Limits, Root},
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
async fn background_history_worker_continues_full_bounded_pages_without_idle_delay() -> Result<()> {
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
            volume: "history-budget".into(),
            physical_bytes: 1_000_000,
            staging_bytes: 1_000_000,
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
    repo.set_account_admission(id(&public(21)), true).await?;
    let permit = repo.account_permit(id(&public(21))).await?;
    let bytes = root(None);
    library
        .stage(permit.clone(), ObjectType::Root, bytes.clone())
        .await?;
    let mut heads = Vec::new();
    for i in 0..258 {
        let pointer = event(21, &bytes, auth::now() - 300 + i);
        assert_eq!(
            library
                .commit(permit.clone(), pointer.clone(), ExpectedHead::Any)
                .await?,
            StageOutcome::Accepted
        );
        heads.push(pointer);
    }
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
    let drained = drain_backend(&repo).await;
    worker.abort();
    let _ = worker.await;
    drained?;
    assert_eq!(
        repo.prune_library_history().await?.removed,
        0,
        "retired heads lost their history grace"
    );
    repo.pool.get().await?.execute("UPDATE events SET retired_at=clock_timestamp()-interval '6 minutes' WHERE retired_at IS NOT NULL", &[]).await?;
    // Hold the actual author's fence until the real node's maintenance worker
    // has selected its first page. PostgreSQL blocking relationships establish
    // the start; no query-text matching or database outcome assertions.
    let mut connection = repo.pool.get().await?;
    let blocker: i32 = connection
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    let lock = connection.transaction().await?;
    lock.query_one(
        "SELECT publisher FROM account_permissions WHERE publisher=$1 FOR SHARE",
        &[&id(&public(21)).as_slice()],
    )
    .await?;
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = listener.local_addr()?;
    drop(listener);
    let origin = format!("http://{bind}");
    let key = dir.path().join("service.key");
    write_key(&key, 11)?;
    let config = dir.path().join("node.json");
    std::fs::write(
        &config,
        serde_json::to_vec(
            &serde_json::json!({"bind":bind.to_string(),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"peer_discovery_enabled":false,"staging_reclamation":false,"library_reclamation":true}),
        )?,
    )?;
    let log = std::fs::File::create(dir.path().join("node.log"))?;
    let _node = Node(
        std::process::Command::new(env!("CARGO_BIN_EXE_manga-node"))
            .args(["serve", config.to_str().unwrap()])
            .env(
                "MANGA_DATABASE_URL",
                std::env::var("MANGA_TEST_DATABASE_URL")?,
            )
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    );
    wait_for_blocked_session(&repo, blocker).await?;
    lock.rollback().await?;
    drop(connection);
    // Seventeen small transactions fit comfortably here. The old scheduler
    // imposed at least four seconds of idle sleeps alone. This is a maintenance
    // progress bound, not a production throughput benchmark.
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(
        repo.prune_library_history().await?.removed,
        0,
        "background worker inserted idle pacing between full bounded pages"
    );
    let latest: serde_json::Value = reqwest::Client::new()
        .get(format!("{origin}/libraries/{}", public(21)))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(latest["event"]["id"], heads.last().unwrap().id);
    assert_eq!(library.public_object(id(&auth::hash(&bytes))).await?, bytes);
    assert_eq!(
        library
            .commit(permit, heads[0].clone(), ExpectedHead::Any)
            .await?,
        StageOutcome::Superseded,
        "maintenance erased the winner watermark"
    );
    Ok(())
}
struct Node(std::process::Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
