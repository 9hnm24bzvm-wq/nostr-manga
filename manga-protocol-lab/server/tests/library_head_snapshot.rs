mod support;
use anyhow::{Context, Result, ensure};
use manga_node::{
    accounts::StageOutcome,
    auth,
    db::{DbConfig, Repository},
    library::ExpectedHead,
    library_manifest::{Descriptor, Family, FamilyRoot, Limits, Root},
    library_storage::{LibraryObjects, ObjectType},
    library_view::SnapshotChanged,
    upload::{StorageConfig, UploadStore},
};
use serde_json::json;
use std::{
    process::{Child, Command},
    time::Duration,
};
use support::*;
struct Node(Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn id(value: &str) -> [u8; 32] {
    hex::decode(value).unwrap().try_into().unwrap()
}
async fn connect(dsn: String) -> Result<Repository> {
    Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and actual HTTP node"]
async fn signed_head_reads_are_one_snapshot_during_replacement() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture database")?;
    ensure!(
        dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test"),
        "fixture only"
    );
    let repo = connect(dsn.clone()).await?;
    repo.migrate().await?;
    // External database/network-delay surrogate: only the designated reader's
    // search path sees this updatable view. A statement takes its snapshot,
    // then waits on a real PostgreSQL advisory lock before returning a head.
    // Writers retain the normal schema and use the real publication lifecycle.
    // No production hooks, private query-text matching or SQL outcome oracles.
    repo.pool.get().await?.batch_execute("CREATE SCHEMA head_read_fault; CREATE FUNCTION head_read_fault.pause() RETURNS boolean LANGUAGE plpgsql VOLATILE AS $$ BEGIN PERFORM pg_advisory_xact_lock(718234); RETURN true; END $$; CREATE VIEW head_read_fault.account_heads AS SELECT * FROM public.account_heads WHERE head_read_fault.pause()").await?;
    // Repository installs its own connection options. Set the disposable
    // database's default schema for subsequently opened reader connections;
    // pin every existing writer connection to the normal schema first.
    let mut writer_connections = Vec::new();
    for _ in 0..8 {
        let connection = repo.pool.get().await?;
        connection.batch_execute("SET search_path=public").await?;
        writer_connections.push(connection);
    }
    writer_connections[0]
        .batch_execute("ALTER DATABASE manga_test SET search_path=head_read_fault,public")
        .await?;
    drop(writer_connections);
    let reader_dsn = dsn.clone();
    let dir = tempfile::tempdir()?;
    let storage = json!({"path":dir.path().join("objects"),"volume":"head-snapshot","physical_bytes":1000000,"staging_bytes":1000000,"max_blob_bytes":1000000});
    let store = UploadStore::open(
        repo.clone(),
        serde_json::from_value::<StorageConfig>(storage.clone())?,
        id(&public(1)),
        id(&public(11)),
    )
    .await?;
    let library = LibraryObjects::new(repo.clone(), store.clone());
    let publisher = id(&public(21));
    repo.set_account_admission(publisher, true).await?;
    let permit = repo.account_permit(publisher).await?;
    let bytes = Root {
        families: Family::ALL
            .into_iter()
            .map(|family| FamilyRoot {
                family,
                chunks: vec![Descriptor::empty()],
            })
            .collect(),
    }
    .encode(&Limits::default())?;
    library
        .stage(permit.clone(), ObjectType::Root, bytes.clone())
        .await?;
    let make_event = |time| {
        sign_at(
            21,
            11999,
            vec![],
            json!({"manifest":auth::hash(&bytes),"version":1}).to_string(),
            time,
        )
    };
    let first = make_event(auth::now() - 10);
    assert_eq!(
        library
            .commit(permit.clone(), first.clone(), ExpectedHead::Absent)
            .await?,
        StageOutcome::Accepted
    );
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
            &json!({"bind":bind.to_string(),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"peer_discovery_enabled":false,"staging_reclamation":false}),
        )?,
    )?;
    let log = std::fs::File::create(dir.path().join("node.log"))?;
    let _node = Node(
        Command::new(env!("CARGO_BIN_EXE_manga-node"))
            .args(["serve", config.to_str().unwrap()])
            .env("MANGA_DATABASE_URL", &reader_dsn)
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    );
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if http
                .get(&origin)
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .with_context(|| {
        format!(
            "node readiness: {}",
            std::fs::read_to_string(dir.path().join("node.log")).unwrap_or_default()
        )
    })?;
    let url = format!("{origin}/libraries/{}", public(21));
    let initial: serde_json::Value = http
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(initial["event"]["id"], first.id);
    let fault = repo.pool.get().await?;
    let blocker: i32 = fault
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    fault
        .query_one("SELECT pg_advisory_lock(718234)", &[])
        .await?;
    let pending_http = tokio::spawn({
        let http = http.clone();
        let url = url.clone();
        async move { http.get(url).send().await }
    });
    wait_for_blocked_session(&repo, blocker).await?;
    let second = make_event(first.created_at + 1);
    assert_eq!(
        library
            .commit(
                permit.clone(),
                second.clone(),
                ExpectedHead::Exact(id(&first.id))
            )
            .await?,
        StageOutcome::Accepted
    );
    fault
        .query_one("SELECT pg_advisory_unlock(718234)", &[])
        .await?;
    let response = pending_http.await??;
    assert_eq!(
        response.status(),
        200,
        "a replacement must not manufacture a missing library head"
    );
    let snapshot: serde_json::Value = response.json().await?;
    let signed: auth::Event = serde_json::from_value(snapshot["event"].clone())?;
    signed.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(
        serde_json::to_value(&signed)?,
        serde_json::to_value(&first)?,
        "the paused statement must return its coherent signed snapshot"
    );
    assert_eq!(snapshot["projected"], false);
    let latest: serde_json::Value = http
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(latest["event"]["id"], second.id);
    // The convenience view is allowed to report a real snapshot conflict after
    // replacement; it must not report that the author never had a public head.
    let reader = LibraryObjects::new(connect(reader_dsn).await?, store);
    fault
        .query_one("SELECT pg_advisory_lock(718234)", &[])
        .await?;
    let pending_view = tokio::spawn({
        let reader = reader.clone();
        async move { reader.public_view(publisher, None).await }
    });
    wait_for_blocked_session(&repo, blocker).await?;
    let third = make_event(second.created_at + 1);
    assert_eq!(
        library
            .commit(permit, third.clone(), ExpectedHead::Exact(id(&second.id)))
            .await?,
        StageOutcome::Accepted
    );
    fault
        .query_one("SELECT pg_advisory_unlock(718234)", &[])
        .await?;
    assert!(
        matches!(pending_view.await?, Err(ref error) if error.is::<SnapshotChanged>()),
        "concurrent view replacement must be an explicit snapshot conflict, not absent data"
    );
    let current = reader
        .public_view(publisher, None)
        .await?
        .context("current view disappeared")?;
    assert_eq!(
        serde_json::to_value(&current.event)?,
        serde_json::to_value(&third)?
    );
    assert!(
        matches!(reader.public_view(publisher, Some(&second.id)).await, Err(ref error) if error.is::<SnapshotChanged>())
    );
    Ok(())
}
