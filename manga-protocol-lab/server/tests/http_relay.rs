mod support;
use anyhow::{Context, Result};
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
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and real strfry"]
async fn guarded_websocket_real_backend() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture database required")?;
    let pg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(
        pg.get_dbname() == Some("manga_test"),
        "fixture database only"
    );
    let repo = manga_node::db::Repository::connect(manga_node::db::DbConfig {
        dsn: dsn.clone(),
        connections: 4,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1_048_576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    repo.set_account_admission(hex::decode(public(2))?.try_into().unwrap(), true)
        .await?;
    repo.set_account_admission(hex::decode(public(3))?.try_into().unwrap(), true)
        .await?;
    drop(repo);
    let dir = tempfile::tempdir()?;
    let key = dir.path().join("service.key");
    write_key(&key, 11)?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let config = dir.path().join("node.json");
    let origin = "http://127.0.0.1:19516";
    std::fs::write(
        &config,
        serde_json::to_vec(
            &json!({"bind":"127.0.0.1:19516","base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"library_concurrency":{"request_slots":256,"processing_slots":16,"commit_slots":8},"storage":{"path":dir.path().join("objects"),"volume":"libraryhttp","physical_bytes":10000000,"staging_bytes":10000000,"max_blob_bytes":1000000,"upload_slots":4},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}}),
        )?,
    )?;
    let log = std::fs::File::create(dir.path().join("node.log"))?;
    let _node = Node(
        Command::new(env!("CARGO_BIN_EXE_manga-node"))
            .args(["serve", config.to_str().unwrap()])
            .env("MANGA_DATABASE_URL", &dsn)
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    );
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let mut ready = false;
    for _ in 0..100 {
        if c.get(origin).send().await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    anyhow::ensure!(
        ready,
        "node startup: {}",
        std::fs::read_to_string(dir.path().join("node.log"))?
    );
    let output = tokio::process::Command::new("python3")
        .current_dir(root)
        .env(
            "PYTHONPATH",
            format!(
                "{}:{}",
                root.join("vendor/pydeps").display(),
                root.display()
            ),
        )
        .args(["server/tests/support/relay_client.py", origin])
        .output()
        .await?;
    anyhow::ensure!(
        output.status.success(),
        "WS fixture: {}\nnode: {}",
        String::from_utf8_lossy(&output.stderr),
        std::fs::read_to_string(dir.path().join("node.log"))?
    );
    std::fs::write(root.join("results/relay-websocket.json"), &output.stdout)?;
    drop(_node);
    for policy in ["authenticated", "admitted"] {
        let mut value: serde_json::Value = serde_json::from_slice(&std::fs::read(&config)?)?;
        value["read_policy"] = json!(policy);
        std::fs::write(&config, serde_json::to_vec(&value)?)?;
        let log = std::fs::File::create(dir.path().join(format!("{policy}.log")))?;
        let _node = Node(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", config.to_str().unwrap()])
                .env("MANGA_DATABASE_URL", &dsn)
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        );
        let mut ready = false;
        for _ in 0..100 {
            if c.get(origin).send().await.is_ok() {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        anyhow::ensure!(ready, "policy node startup failed");
        let output = tokio::process::Command::new("python3")
            .current_dir(root)
            .env(
                "PYTHONPATH",
                format!(
                    "{}:{}",
                    root.join("vendor/pydeps").display(),
                    root.display()
                ),
            )
            .args([
                "server/tests/support/relay_policy_client.py",
                origin,
                policy,
            ])
            .output()
            .await?;
        anyhow::ensure!(
            output.status.success(),
            "policy fixture: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::write(
            root.join(format!("results/relay-policy-{policy}.json")),
            &output.stdout,
        )?;
    }
    Ok(())
}
