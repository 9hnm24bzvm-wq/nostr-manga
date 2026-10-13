mod support;
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    inventory::{FRAME_BYTES, storage},
};
use negentropy::Negentropy;
use serde_json::{Value, json};
use std::{
    process::{Child, Command},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
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
struct Target {
    origin: String,
    key: Arc<AtomicU8>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Target {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn target(key: u8) -> Result<Target> {
    use axum::{Json, Router, extract::Query, routing::get};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let origin = format!("http://{}", listener.local_addr()?);
    let key = Arc::new(AtomicU8::new(key));
    let k = key.clone();
    let endpoint = origin.clone();
    let app = Router::new().route(
        "/identity",
        get(
            move |Query(q): Query<std::collections::HashMap<String, String>>| {
                let k = k.clone();
                let endpoint = endpoint.clone();
                async move {
                    Json(
                        auth::sign(
                            &format!("{:064x}", k.load(Ordering::Acquire)),
                            20005,
                            vec![
                                vec!["nonce".into(), q["nonce"].clone()],
                                vec!["endpoint".into(), endpoint],
                            ],
                            "fixture service proof".into(),
                        )
                        .unwrap(),
                    )
                }
            },
        ),
    );
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Ok(Target { origin, key, task })
}
async fn verify(
    c: &reqwest::Client,
    url: &str,
    key: u8,
    endpoint: &str,
    expected: Option<String>,
) -> Result<reqwest::Response> {
    let body = serde_json::to_vec(&json!({"endpoint":endpoint,"expected_key":expected}))?;
    Ok(c.post(url)
        .header("authorization", http(key, url, &body))
        .body(body)
        .send()
        .await?)
}
async fn assert_unapproved(c: &reqwest::Client, origin: &str, key: u8) -> Result<()> {
    let mut empty = Negentropy::owned(storage(&[])?, FRAME_BYTES as u64)?;
    let response = post_json(
        c,
        key,
        &format!("{origin}/reconcile"),
        &json!({"message":hex::encode(empty.initiate()?)}),
    )
    .await?;
    assert_eq!(
        response.status(),
        403,
        "identity verification must not authorize signed peer reconciliation"
    );
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated PostgreSQL and real HTTP node"]
async fn remembers_possession_without_granting_trust_and_detects_domain_transfer() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    ensure!(cfg.get_dbname() == Some("manga_test"), "fixture only");
    let repo = Repository::connect(DbConfig {
        dsn: dsn.clone(),
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let first = target(11).await?;
    let moved = target(11).await?;
    let blocked = target(12).await?;
    let dir = tempfile::tempdir()?;
    let key = dir.path().join("key");
    write_key(&key, 9)?;
    let socket = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = socket.local_addr()?.to_string();
    drop(socket);
    let origin = format!("http://{bind}");
    let config = dir.path().join("config.json");
    std::fs::write(
        &config,
        serde_json::to_vec(
            &json!({"bind":bind,"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"discovery_private_overrides":[first.origin,moved.origin]}),
        )?,
    )?;
    let log = std::fs::File::create(dir.path().join("node.log"))?;
    let node = Node(
        Command::new(env!("CARGO_BIN_EXE_manga-node"))
            .args(["serve", config.to_str().unwrap()])
            .env("MANGA_DATABASE_URL", &dsn)
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    );
    let c = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()?;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if c.get(&origin).send().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .context("node readiness")?;
    let url = format!("{origin}/admin/identities/verify");
    assert_eq!(
        verify(&c, &url, 2, &first.origin, None).await?.status(),
        403
    );
    assert_eq!(
        verify(&c, &url, 1, &blocked.origin, None).await?.status(),
        403
    );
    assert_eq!(
        verify(&c, &url, 1, &first.origin, Some(public(12)))
            .await?
            .status(),
        409
    );
    assert!(repo.remembered_service_key(&first.origin).await?.is_none());
    let response = verify(&c, &url, 1, &first.origin, None).await?;
    assert_eq!(response.status(), 200);
    let saved: Value = response.json().await?;
    assert_eq!(saved["service_pubkey"], public(11));
    assert_eq!(saved["first_observation"], true);
    assert_eq!(saved["peer_permissions_granted"], false);
    assert_unapproved(&c, &origin, 11).await?;
    let again: Value = verify(&c, &url, 1, &first.origin, None)
        .await?
        .json()
        .await?;
    assert_eq!(again["first_observation"], false);
    let changed_domain: Value = verify(&c, &url, 1, &moved.origin, Some(public(11)))
        .await?
        .json()
        .await?;
    assert_eq!(changed_domain["service_pubkey"], public(11));
    assert_eq!(changed_domain["first_observation"], true);
    assert_unapproved(&c, &origin, 11).await?;
    first.key.store(12, Ordering::Release);
    assert_eq!(
        verify(&c, &url, 1, &first.origin, None).await?.status(),
        409
    );
    assert_eq!(
        verify(&c, &url, 1, &first.origin, Some(public(12)))
            .await?
            .status(),
        409
    );
    assert_unapproved(&c, &origin, 12).await?;
    drop(node);
    let restart_log = std::fs::File::create(dir.path().join("restarted-node.log"))?;
    let _restarted = Node(
        Command::new(env!("CARGO_BIN_EXE_manga-node"))
            .args(["serve", config.to_str().unwrap()])
            .env("MANGA_DATABASE_URL", &dsn)
            .stdout(restart_log.try_clone()?)
            .stderr(restart_log)
            .spawn()?,
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if c.get(&origin).send().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .context("restarted readiness")?;
    assert_eq!(
        verify(&c, &url, 1, &first.origin, None).await?.status(),
        409
    );
    let remembered_after_restart: Value = verify(&c, &url, 1, &moved.origin, None)
        .await?
        .json()
        .await?;
    assert_eq!(remembered_after_restart["first_observation"], false);
    assert_eq!(remembered_after_restart["service_pubkey"], public(11));
    first.key.store(11, Ordering::Release);
    let original_after_restart: Value = verify(&c, &url, 1, &first.origin, None)
        .await?
        .json()
        .await?;
    assert_eq!(original_after_restart["first_observation"], false);
    assert_eq!(original_after_restart["service_pubkey"], public(11));
    assert_unapproved(&c, &origin, 11).await?;
    assert_unapproved(&c, &origin, 12).await?;
    let stored = repo.remembered_service_key(&first.origin).await?.unwrap();
    assert_eq!(hex::encode(stored), public(11));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    std::fs::write(
        root.join("results/http-remembered-identity.json"),
        serde_json::to_vec_pretty(
            &json!({"first_possession_remembered":true,"same_key_new_origin":true,"transferred_origin_rejected":true,"explicit_wrong_key_rejected":true,"private_override_required":true,"operator_auth_required":true,"verified_identity_signed_reconcile_forbidden":true,"replacement_identity_signed_reconcile_forbidden":true,"both_origin_bindings_survive_actual_node_restart":true,"signed_reconcile_remains_forbidden_after_restart":true}),
        )?,
    )?;
    Ok(())
}
