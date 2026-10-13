mod support;
use anyhow::{Context, Result};
use manga_node::{
    auth,
    backend::{Backend, BackendConfig},
    db::{DbConfig, Repository},
    publication::Publications,
    upload::{StorageConfig, UploadStore},
};
use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Child, Command},
    time::Duration,
};
use support::*;
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
struct Node(Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn bind() -> Result<String> {
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(l.local_addr()?.to_string())
}
fn start(config: &Path, dsn: &str, log: &Path) -> Result<Node> {
    let log = std::fs::File::create(log)?;
    Ok(Node(
        Command::new(env!("CARGO_BIN_EXE_manga-node"))
            .args(["serve", config.to_str().unwrap()])
            .env("MANGA_DATABASE_URL", dsn)
            .env("RUST_LOG", "info")
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    ))
}
async fn wait_http(c: &reqwest::Client, url: &str) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if c.get(url).send().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    Ok(())
}
async fn closed(bind: &str) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if tokio::net::TcpStream::connect(bind).await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    Ok(())
}
async fn wait_fault(c: &reqwest::Client, status_url: &str) -> Result<Value> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let value: Value = c
                .get(status_url)
                .header("authorization", http_get(21, status_url))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if matches!(
                value["public_serving"]["state"].as_str(),
                Some("failed_closed" | "shutdown_unconfirmed")
            ) {
                return Ok::<_, anyhow::Error>(value["public_serving"].clone());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?
}
async fn wait_active(c: &reqwest::Client, status_url: &str) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let value: Value = c
                .get(status_url)
                .header("authorization", http_get(21, status_url))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if value["public_serving"]["state"] == "active" {
                return Ok::<_, anyhow::Error>(());
            }
            assert!(matches!(
                value["public_serving"]["state"].as_str(),
                Some("prepared" | "activating")
            ));
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?
}
async fn helper_stopped(runtime: &Path) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let s: Value = serde_json::from_slice(&std::fs::read(runtime.join("state.json"))?)?;
            if s["phase"] == "stopped" {
                break Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    Ok(())
}
fn kill_helper(runtime: &Path) -> Result<()> {
    let state: Value = serde_json::from_slice(&std::fs::read(runtime.join("state.json"))?)?;
    let pid = state["master"]["pid"]
        .as_u64()
        .context("nginx master missing")?;
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let fields: Vec<_> = stat[stat.rfind(')').unwrap() + 2..]
        .split_whitespace()
        .collect();
    let parent = fields[1];
    anyhow::ensure!(
        Command::new("kill")
            .args(["-KILL", parent])
            .status()?
            .success(),
        "fixture helper kill failed"
    );
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and real strfry/nginx"]
async fn node_serving_recovers_before_policy_and_database_startup() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    let pg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(
        pg.get_dbname() == Some("manga_test"),
        "fixture database only"
    );
    let repo = Repository::connect(DbConfig {
        dsn: dsn.clone(),
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let dir = tempfile::tempdir()?;
    let owner = id(&public(21));
    let service = id(&public(11));
    repo.bind_instance(service, owner).await?;
    let storage = dir.path().join("store");
    let backend_path = dir.path().join("backend");
    let store = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: storage.clone(),
            volume: "publicnode".into(),
            physical_bytes: 1048576,
            staging_bytes: 1048576,
            max_blob_bytes: 1048576,
            upload_slots: 4,
            reclamation_slots: 4,
            credit_lanes: 16,
            formats: vec!["image/png".into()],
        },
        owner,
        service,
    )
    .await?;
    let publications = Publications::new(repo.clone(), store.clone());
    let image = png();
    let hash = auth::hash(&image);
    store.put(id(&hash), &image).await?;
    let manifest = cbor(
        json!({"version":1,"series":"ab".repeat(32),"chapter":"1","language":"en","pages":[page(&image)]}),
    );
    store.put(id(&auth::hash(&manifest)), &manifest).await?;
    let event = publication(21, 1001, &manifest);
    publications.publish(&event).await?;
    let backend = Backend::open(
        repo.clone(),
        BackendConfig {
            binary: root.join("vendor/strfry/strfry"),
            data_dir: backend_path.clone(),
            timeout_seconds: 3,
            map_bytes: 67108864,
        },
    )
    .await?;
    let task = tokio::spawn(backend.run());
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if repo.projection_complete(id(&event.id)).await? {
                break Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    task.abort();
    let _ = task.await;
    drop(publications);
    drop(store);
    let api_bind = bind()?;
    let image_bind = bind()?;
    let origin = format!("http://{api_bind}");
    let image_url = format!("http://{image_bind}/{hash}.png");
    let key = dir.path().join("service.key");
    write_key(&key, 11)?;
    let helper = dir.path().join("serving_supervisor.py");
    std::fs::copy(root.join("src/serving_supervisor.py"), &helper)?;
    let mut cfg = json!({"bind":api_bind,"base_url":origin,"service_key_file":key,"admin_pubkey":public(21),"database_plaintext_local":true,"sync_worker_enabled":false,"storage":{"path":storage,"volume":"publicnode","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":1048576},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":backend_path,"timeout_seconds":3,"map_bytes":67108864},"public_serving":{"enabled":true,"helper":helper,"nginx":root.join("vendor/nginx-1.28.0/objs/nginx"),"bind":image_bind,"shutdown_seconds":1}});
    let config = dir.path().join("node.json");
    let log = dir.path().join("node.log");
    let runtime = storage.join("public/.serving");
    let write = |cfg: &Value| -> Result<()> {
        std::fs::write(&config, serde_json::to_vec(cfg)?)?;
        Ok(())
    };
    let c = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()?;
    write(&cfg)?;
    let node = start(&config, &dsn, &log)?;
    wait_http(&c, &origin)
        .await
        .context(std::fs::read_to_string(&log).unwrap_or_default())?;
    wait_http(&c, &image_url).await?;
    assert_eq!(
        c.get(&image_url).send().await?.bytes().await?.as_ref(),
        image.as_slice()
    );
    let status_url = format!("{origin}/admin/status");
    wait_active(&c, &status_url).await?;
    // The live node detects helper loss and independently recovers orphan readers.
    kill_helper(&runtime)?;
    closed(&image_bind).await?;
    let failure = wait_fault(&c, &status_url).await?;
    assert_eq!(failure["fault"], "reader_health");
    assert_eq!(failure["state"], "failed_closed");
    assert_eq!(failure["automatic_reactivation"], false);
    assert!(c.get(&origin).send().await?.status().is_success());
    drop(node);
    helper_stopped(&runtime).await?;
    let node = start(&config, &dsn, &log)?;
    wait_http(&c, &image_url).await?;
    wait_active(&c, &status_url).await?;
    // Lose the live helper and externally prevent the recovery executable from
    // starting. Operator status must not confuse an unverified stop with safety.
    // Restore the executable before asserting so even failed assertions leave
    // the documented operator recovery path available.
    let held_helper = helper.with_extension("held");
    std::fs::rename(&helper, &held_helper)?;
    let fault = async {
        kill_helper(&runtime)?;
        wait_fault(&c, &status_url).await
    }
    .await;
    std::fs::rename(&held_helper, &helper)?;
    let failure = fault?;
    assert_eq!(failure["state"], "shutdown_unconfirmed");
    assert_eq!(failure["fault"], "reader_health");
    assert_eq!(failure["automatic_reactivation"], false);
    assert_eq!(
        c.get(&image_url).send().await?.bytes().await?.as_ref(),
        image.as_slice()
    );
    let still_failed = wait_fault(&c, &status_url).await?;
    assert_eq!(
        still_failed, failure,
        "restoring the helper must not erase the fault or reactivate the role"
    );
    drop(node);
    let node = start(&config, &dsn, &log)?;
    wait_http(&c, &origin).await?;
    wait_http(&c, &image_url).await?;
    wait_active(&c, &status_url).await?;
    // Kill both owners before live recovery; restart must stop surviving nginx.
    kill_helper(&runtime)?;
    drop(node);
    assert!(c.get(&image_url).send().await?.status().is_success());
    cfg["read_policy"] = json!("authenticated");
    cfg["public_serving"]["enabled"] = json!(false);
    write(&cfg)?;
    let node = start(&config, &dsn, &log)?;
    wait_http(&c, &origin).await?;
    assert!(
        tokio::net::TcpStream::connect(&image_bind).await.is_err(),
        "guarded HTTP opened before old image readers stopped"
    );
    assert_eq!(
        c.get(format!("{origin}/originals/{hash}"))
            .send()
            .await?
            .status(),
        403
    );
    drop(node);
    helper_stopped(&runtime).await?;
    let serving_config = cfg
        .as_object_mut()
        .unwrap()
        .remove("public_serving")
        .unwrap();
    write(&cfg)?;
    let mut rejected = start(&config, &dsn, &log)?;
    assert!(!rejected.0.wait()?.success());
    assert!(std::fs::read_to_string(&log)?.contains("managed serving state exists"));
    drop(rejected);
    cfg["public_serving"] = serving_config;
    cfg["public_serving"]["enabled"] = json!(true);
    write(&cfg)?;
    let mut rejected = start(&config, &dsn, &log)?;
    assert!(!rejected.0.wait()?.success());
    assert!(std::fs::read_to_string(&log)?.contains("public read policy"));
    drop(rejected);
    cfg["read_policy"] = json!("public");
    write(&cfg)?;
    let node = start(&config, &dsn, &log)?;
    wait_http(&c, &image_url).await?;
    // A filesystem withdrawal error must close the entire direct reader and keep holds.
    use std::os::unix::fs::PermissionsExt;
    let shard = storage.join("public").join(&hash[..2]).join(&hash[2..4]);
    std::fs::set_permissions(&shard, std::fs::Permissions::from_mode(0o500))?;
    let deletion = sign_at(
        21,
        5,
        vec![vec!["e".into(), event.id.clone()]],
        "Synthetic serving removal".into(),
        auth::now(),
    );
    repo.record_deletion_request(&deletion).await?;
    repo.approve_deletion(id(&deletion.id), None).await?;
    closed(&image_bind).await?;
    let failure = wait_fault(&c, &status_url).await?;
    assert_eq!(failure["fault"], "namespace_reconciliation");

    let retained = repo
        .blob_intent(id(&hash))
        .await?
        .context("withdrawal must retain original")?;
    let original = storage
        .join("objects")
        .join(&hash[..2])
        .join(&hash[2..4])
        .join(format!("{hash}.{}", hex::encode(retained.key.generation)));
    assert_eq!(
        std::fs::read(original)?,
        image,
        "failed withdrawal must preserve original bytes"
    );
    assert!(shard.join(format!("{hash}.png")).exists());
    std::fs::set_permissions(&shard, std::fs::Permissions::from_mode(0o755))?;
    drop(node);
    helper_stopped(&runtime).await?;
    let node = start(&config, &dsn, &log)?;
    // Every observed image response during startup must be absent, never the stale alias.
    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            if let Ok(response) = c.get(&image_url).send().await {
                assert_eq!(response.status(), 404);
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert!(!shard.join(format!("{hash}.png")).exists());
    // Recovery runs before attempting the DB connection, including an unavailable DB.
    kill_helper(&runtime)?;
    drop(node);
    // A fresh explicit local DSN avoids retaining the fixture's old port entry.
    let unavailable = "host=127.0.0.1 port=9 user=fixture dbname=manga_test connect_timeout=1";
    let mut failed = start(&config, unavailable, &log)?;
    assert!(!failed.0.wait()?.success());
    drop(failed);
    closed(&image_bind).await?;
    helper_stopped(&runtime).await?;
    std::fs::write(
        root.join("results/http-public-serving.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["automatic exact original projection","live helper loss recovers orphan readers","killed helper leaves real orphan readers","guarded restart stops orphan readers before HTTP","missing recovery config refused","guarded direct-serving config refused","filesystem unlink failure closes direct reader and retains exact original bytes","operator status distinguishes reader health from namespace reconciliation faults","startup drains pending deletion before exposure","orphan recovery precedes failed DB connection"],"scope":"loopback lifecycle correctness; no target-capacity claim"}),
        )?,
    )?;
    Ok(())
}
