//! One-client pool ownership through real HTTP and strfry projection.
//! SQL creates isolated fixture databases and migrates them; outcomes are HTTP.
mod support;

use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    mangadex_compat,
};
use serde_json::{Value, json};
use std::{
    path::Path,
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

struct Fixture {
    node: Option<Node>,
    directory: Option<tempfile::TempDir>,
    client: reqwest::Client,
    origin: String,
    succeeded: bool,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.node.take());
        if !self.succeeded
            && let Some(directory) = self.directory.take()
        {
            eprintln!(
                "pool-lifetime failure: retained private config and node.log at {}",
                directory.keep().display()
            );
        }
    }
}

async fn repository(dsn: String) -> Result<Repository> {
    Repository::connect(DbConfig {
        dsn,
        connections: 1,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await
}

impl Fixture {
    async fn start(label: &str) -> Result<Self> {
        let base = std::env::var("MANGA_TEST_DATABASE_URL")?;
        ensure!(
            base.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test"),
            "fixture requires the disposable manga_test database"
        );
        let directory = tempfile::Builder::new()
            .prefix(&format!("manga-pool-{label}-"))
            .tempdir()?;
        let bind = loop {
            let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
            let bind = listener.local_addr()?;
            if ![18767, 18768, 18769].contains(&bind.port()) {
                break bind;
            }
        };
        let mut fixture = Self {
            node: None,
            directory: Some(directory),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()?,
            origin: format!("http://{bind}"),
            succeeded: false,
        };
        let directory = fixture.directory.as_ref().unwrap().path();
        // Separate databases permit parallel tests without destructive resets.
        // They remain inside the parent runner's disposable PostgreSQL cluster.
        let database = format!("manga_pool_{}", hex::encode(rand::random::<[u8; 8]>()));
        let admin = repository(base.clone()).await?;
        admin
            .pool
            .get()
            .await?
            .batch_execute(&format!("CREATE DATABASE {database}"))
            .await?;
        drop(admin);
        let dsn = if base.starts_with("postgres://") || base.starts_with("postgresql://") {
            let mut url = url::Url::parse(&base)?;
            url.set_path(&format!("/{database}"));
            url.to_string()
        } else {
            format!("{base} dbname={database}")
        };
        ensure!(
            dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some(database.as_str()),
            "fixture database isolation failed"
        );
        std::fs::write(directory.join("fixture-database.txt"), &database)?;
        let repo = repository(dsn.clone()).await?;
        repo.migrate().await?;
        drop(repo);
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let key = directory.join("service.key");
        write_key(&key, 11)?;
        let config = directory.join("node.json");
        std::fs::write(
            &config,
            serde_json::to_vec(&json!({
                "bind": bind.to_string(), "base_url": fixture.origin,
                "service_key_file": key, "admin_pubkey": public(1),
                "database_plaintext_local": true, "database_connections": 1,
                "database_wait_ms": 500, "read_policy": "authenticated",
                "read_admission": {
                    "overall": 1, "wait_ms": 200,
                    "catalogue": {"active": 1, "waiting": 0},
                    "library_head": {"active": 1, "waiting": 0},
                    "live": {"active": 1, "waiting": 0}
                },
                "sync_worker_enabled": false,
                "backend": {
                    "binary": root.join("vendor/strfry/strfry"),
                    "data_dir": directory.join("backend"), "map_bytes": 67108864
                },
                "storage": {
                    "path": directory.join("objects"), "volume": "pool-lifetimes",
                    "physical_bytes": 1048576, "staging_bytes": 1048576,
                    "max_blob_bytes": 262144
                }
            }))?,
        )?;
        let log_path = directory.join("node.log");
        let log = std::fs::File::create(&log_path)?;
        fixture.node = Some(Node(
            Command::new(
                std::env::var_os("MANGA_TEST_NODE_BIN")
                    .unwrap_or_else(|| env!("CARGO_BIN_EXE_manga-node").into()),
            )
            .args(["serve", config.to_str().context("UTF-8 config path")?])
            .env("MANGA_DATABASE_URL", dsn)
            .env("RUST_LOG", "info")
            .env_remove("MANGA_BENCH_PROFILE")
            .env_remove("MANGA_READ_ADMISSION_DIAGNOSTICS")
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
        ));
        let ready = tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                ensure!(
                    fixture.node.as_mut().unwrap().0.try_wait()?.is_none(),
                    "node exited during normal startup"
                );
                if fixture
                    .client
                    .get(&fixture.origin)
                    .send()
                    .await
                    .is_ok_and(|response| response.status() == 200)
                {
                    return Ok::<_, anyhow::Error>(());
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await;
        match ready {
            Ok(Ok(())) => Ok(fixture),
            other => anyhow::bail!(
                "normal startup failed: {other:?}\n{}",
                std::fs::read_to_string(log_path)?
            ),
        }
    }

    async fn read(&self, path: &str) -> Result<reqwest::Response> {
        let url = format!("{}{path}", self.origin);
        Ok(self
            .client
            .get(&url)
            .header("authorization", http_get(2, &url))
            .send()
            .await?)
    }

    async fn json(&self, path: &str) -> Result<Value> {
        json_200(self.read(path).await?, path).await
    }

    async fn exact(&self, path: &str, expected: &[u8]) -> Result<()> {
        let response = self.read(path).await?;
        ensure!(
            response.status() == 200,
            "original GET {path}: {}",
            response.status()
        );
        assert_eq!(response.bytes().await?.as_ref(), expected, "GET {path}");
        Ok(())
    }
}

async fn json_200(response: reqwest::Response, context: &str) -> Result<Value> {
    let status = response.status();
    let body = response.text().await?;
    ensure!(status == 200, "{context}: HTTP {status}: {body}");
    Ok(serde_json::from_str(&body)?)
}

struct Series {
    event: auth::Event,
    manifest: Vec<u8>,
    cover: Vec<u8>,
}

async fn host_series(fixture: &Fixture) -> Result<Series> {
    let cover = png();
    let manifest = cbor(json!({
        "version": 1, "required": ["series-cover-v1"],
        "titles": {"en": "One connection series"},
        "credits": [{"role": "artist", "name": "Pool fixture artist"}],
        "originalLanguage": "ja", "status": "ongoing", "cover": page(&cover)
    }));
    let event = publication(1, 1002, &manifest);
    upload_to(&fixture.client, 1, &fixture.origin, &cover).await?;
    upload_to(&fixture.client, 1, &fixture.origin, &manifest).await?;
    let response = post_json(
        &fixture.client,
        1,
        &format!("{}/publications", fixture.origin),
        &serde_json::to_value(&event)?,
    )
    .await?;
    ensure!(
        response.status() == 200 || response.status() == 202,
        "signed publication failed: {} {}",
        response.status(),
        response.text().await?
    );
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let response = fixture.read(&format!("/catalogue/{}", event.id)).await?;
            if response.status() == 200 {
                let entry: Value = response.json().await?;
                assert_eq!(entry["event"], serde_json::to_value(&event)?);
                assert_eq!(entry["metadata"]["cover"], page(&cover));
                return Ok::<_, anyhow::Error>(());
            }
            ensure!(
                response.status() == 404,
                "catalogue readiness: {}",
                response.status()
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .context("real backend did not project the hosted series")??;
    Ok(Series {
        event,
        manifest,
        cover,
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "real PostgreSQL and strfry; one-client operator audit lifetime"]
async fn operator_audit_releases_its_single_pool_client() -> Result<()> {
    let mut fixture = Fixture::start("audit").await?;
    let audit = format!("{}/admin/storage/audit", fixture.origin);
    assert_eq!(
        post_json(&fixture.client, 2, &audit, &json!({"limit": 64}))
            .await?
            .status(),
        403,
        "a signed nonoperator cannot audit storage"
    );
    // The empty-store request is the first causal red on the preserved binary:
    // zero rows must still release the cursor client before export-status reads.
    let empty = json_200(
        post_json(&fixture.client, 1, &audit, &json!({"limit": 64})).await?,
        "empty operator audit with database_connections=1",
    )
    .await?;
    assert_eq!(empty["checked"], 0);
    assert_eq!(empty["checked_bytes"], 0);
    assert_eq!(empty["quarantined"], 0);
    assert_eq!(empty["skipped"], 0);
    assert_eq!(empty["wrapped"], true);
    assert_eq!(empty["public_export_pending"], false);

    let series = host_series(&fixture).await?;
    let populated = json_200(
        post_json(&fixture.client, 1, &audit, &json!({"limit": 64})).await?,
        "populated operator audit with database_connections=1",
    )
    .await?;
    assert_eq!(populated["checked"], 2);
    assert_eq!(
        populated["checked_bytes"],
        series.cover.len() + series.manifest.len()
    );
    assert_eq!(populated["quarantined"], 0);
    assert_eq!(populated["skipped"], 0);
    assert_eq!(populated["wrapped"], false);
    assert_eq!(populated["public_export_pending"], false);
    let wrapped = json_200(
        post_json(&fixture.client, 1, &audit, &json!({"limit": 64})).await?,
        "completed audit cursor wraps without retaining its client",
    )
    .await?;
    assert_eq!(wrapped["checked"], 0);
    assert_eq!(wrapped["wrapped"], true);
    for bytes in [&series.cover, &series.manifest] {
        fixture
            .exact(&format!("/originals/{}", auth::hash(bytes)), bytes)
            .await?;
    }
    assert_eq!(
        fixture
            .json(&format!("/events/{}", series.event.id))
            .await?,
        serde_json::to_value(&series.event)?
    );
    fixture.succeeded = true;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "real PostgreSQL and strfry; one-client compatibility list lifetime"]
async fn compatibility_lists_release_their_single_pool_client() -> Result<()> {
    let mut fixture = Fixture::start("compatibility").await?;
    for path in ["/compat/mangadex/cover", "/compat/mangadex/author"] {
        assert_eq!(
            fixture
                .client
                .get(format!("{}{path}", fixture.origin))
                .send()
                .await?
                .status(),
            403,
            "authenticated read policy still applies to {path}"
        );
        let empty = fixture.json(path).await?;
        assert_eq!(empty["total"], 0);
        assert_eq!(empty["data"], json!([]));
    }
    let series = host_series(&fixture).await?;
    let manga_id = mangadex_compat::uuid("manga", &series.event.id);
    let cover_id = mangadex_compat::uuid("cover", &series.event.id);
    let author_id = mangadex_compat::uuid("author", &format!("{}:0", series.event.id));
    let detail = fixture
        .json(&format!("/compat/mangadex/cover/{cover_id}"))
        .await?;
    assert_eq!(detail["data"]["id"], cover_id);
    assert_eq!(detail["data"]["type"], "cover_art");
    let filename = format!("{}.png", auth::hash(&series.cover));
    assert_eq!(detail["data"]["attributes"]["fileName"], filename);
    let image_path = format!("/compat/mangadex/covers/{manga_id}/{filename}");
    fixture.exact(&image_path, &series.cover).await?;

    // Obtain both responses before asserting so neither child checkout is hidden
    // by an early assertion on the other resource's committed list snapshot.
    let covers_response = fixture.read("/compat/mangadex/cover").await?;
    let authors_response = fixture.read("/compat/mangadex/author").await?;
    let covers_status = covers_response.status();
    let authors_status = authors_response.status();
    let covers_body = covers_response.text().await?;
    let authors_body = authors_response.text().await?;
    ensure!(
        covers_status == 200 && authors_status == 200,
        "nonempty one-client lists: cover HTTP {covers_status}: {covers_body}; author HTTP {authors_status}: {authors_body}"
    );
    let covers: Value = serde_json::from_str(&covers_body)?;
    let authors: Value = serde_json::from_str(&authors_body)?;
    assert_eq!(covers["total"], 1);
    assert_eq!(covers["data"], json!([detail["data"].clone()]));
    assert_eq!(
        covers["data"][0]["relationships"],
        json!([{"id": manga_id, "type": "manga"}])
    );
    assert_eq!(authors["total"], 1);
    assert_eq!(authors["data"].as_array().unwrap().len(), 1);
    assert_eq!(authors["data"][0]["id"], author_id);
    assert_eq!(authors["data"][0]["type"], "author");
    assert_eq!(
        authors["data"][0]["attributes"]["name"],
        "Pool fixture artist"
    );
    assert_eq!(
        authors["data"][0]["relationships"],
        json!([{"id": manga_id, "type": "manga"}])
    );
    assert_eq!(authors["data"][0]["nostr"]["descriptiveCreditOnly"], true);
    let manga = fixture
        .json(&format!("/compat/mangadex/manga/{manga_id}"))
        .await?;
    let relationships = manga["data"]["relationships"].as_array().unwrap();
    assert!(relationships.contains(&json!({"id": cover_id, "type": "cover_art"})));
    assert!(relationships.contains(&json!({"id": author_id, "type": "artist"})));

    let url = format!("{}/compat/mangadex/author", fixture.origin);
    let wrong_url = format!("{}/compat/mangadex/cover", fixture.origin);
    assert_eq!(
        fixture
            .client
            .get(&url)
            .header("authorization", http_get(2, &wrong_url))
            .send()
            .await?
            .status(),
        403,
        "a signature for another resource is not a read authorization"
    );
    fixture.exact(&image_path, &series.cover).await?;
    fixture
        .exact(
            &format!("/originals/{}", auth::hash(&series.manifest)),
            &series.manifest,
        )
        .await?;
    fixture.succeeded = true;
    Ok(())
}
