//! Actual HTTP advertisements must agree and reflect each configured role.
mod support;
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
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
#[ignore = "isolated PostgreSQL; run scripts/test-postgres.sh"]
async fn unified_public_registry_matches_role_and_enforced_limits() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    let parsed: tokio_postgres::Config = dsn.parse()?;
    ensure!(
        parsed.get_dbname() == Some("manga_test"),
        "isolated fixture required"
    );
    let repo = manga_node::db::Repository::connect(manga_node::db::DbConfig {
        dsn: dsn.clone(),
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let dir = tempfile::tempdir()?;
    let key = dir.path().join("private-service-key");
    write_key(&key, 11)?;
    let c = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()?;
    let mut evidence = Vec::new();
    for with_storage in [false, true] {
        let socket = std::net::TcpListener::bind("127.0.0.1:0")?;
        let bind = socket.local_addr()?.to_string();
        drop(socket);
        let origin = format!("http://{bind}");
        let mut config = json!({"bind":bind,"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"library_concurrency":{"request_slots":32,"processing_slots":2,"commit_slots":1}});
        if with_storage {
            config["storage"] = json!({"path":dir.path().join("private-storage-path"),"volume":"caps","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":524288,"formats":["image/png"]});
        }
        let path = dir.path().join("config.json");
        std::fs::write(&path, serde_json::to_vec(&config)?)?;
        let log = std::fs::File::create(dir.path().join(format!("node-{with_storage}.log")))?;
        let node = Node(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", path.to_str().unwrap()])
                .env("MANGA_DATABASE_URL", &dsn)
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        );
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if c.get(&origin).send().await.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .with_context(|| {
            format!(
                "node readiness: {}",
                std::fs::read_to_string(dir.path().join(format!("node-{with_storage}.log")))
                    .unwrap_or_default()
            )
        })?;
        let plain: Value = c
            .get(&origin)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let response = c
            .get(&origin)
            .header("accept", "application/nostr+json")
            .send()
            .await?
            .error_for_status()?;
        assert_eq!(response.headers()["content-type"], "application/nostr+json");
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
        assert!(
            response
                .headers()
                .contains_key("access-control-allow-methods")
        );
        let nip: Value = response.json().await?;
        assert_eq!(plain["manga"], nip["manga"]);
        assert_eq!(nip["self"], public(11));
        assert_eq!(nip["pubkey"], public(1));
        assert_eq!(plain["features"], plain["manga"]["features"]);
        assert_eq!(plain["features"]["upload"], with_storage);
        assert_eq!(plain["features"]["publication"], false);
        assert_eq!(plain["features"]["public_libraries"], false);
        assert_eq!(plain["manga"]["limits"]["library"]["processing_slots"], 2);
        assert_eq!(plain["manga"]["blob_protocol"]["standard_blossom"], false);
        if with_storage {
            assert_eq!(plain["manga"]["image_formats"], json!(["image/png"]));
            assert_eq!(plain["manga"]["limits"]["blob_bytes"], 524288);
        } else {
            assert_eq!(plain["manga"]["image_formats"], json!([]));
            assert!(plain["manga"]["limits"]["blob_bytes"].is_null());
        }
        let text = plain.to_string();
        for secret in ["private-service-key", "private-storage-path", dsn.as_str()] {
            assert!(!text.contains(secret));
        }
        let options = c
            .request(reqwest::Method::OPTIONS, &origin)
            .header("origin", "https://reader.example")
            .header("access-control-request-method", "GET")
            .send()
            .await?;
        assert_eq!(options.status(), 204);
        assert_eq!(options.headers()["access-control-allow-origin"], "*");
        let max = plain["manga"]["limits"]["discovery"]["page_items"]
            .as_i64()
            .unwrap();
        assert_eq!(
            c.get(format!("{origin}/query?order=asc&limit={}", max + 1))
                .send()
                .await?
                .status(),
            400
        );
        evidence.push(plain);
        drop(node);
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    std::fs::write(
        root.join("results/http-capabilities.json"),
        serde_json::to_vec_pretty(&evidence)?,
    )?;
    Ok(())
}
