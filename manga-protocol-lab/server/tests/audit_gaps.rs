//! Readout of known contract gaps, not an acceptance test claiming those features work.
mod support;
use anyhow::{Context, Result};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    upload::{StorageConfig, UploadStore},
};
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
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated diagnostic audit with PostgreSQL and real backend; observations are not conformance passes"]
async fn characterize_missing_contracts() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    let pg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(pg.get_dbname() == Some("manga_test"), "fixture only");
    let repo = Repository::connect(DbConfig {
        dsn: dsn.clone(),
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let owner = id(&public(1));
    repo.bind_instance(id(&public(11)), owner).await?;
    let dir = tempfile::tempdir()?;
    let store = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.path().join("ttl"),
            volume: "ttlaudit".into(),
            physical_bytes: 1048576,
            staging_bytes: 1048576,
            max_blob_bytes: 1048576,
            upload_slots: 4,
            reclamation_slots: 4,
            credit_lanes: 16,
            formats: vec!["image/png".into()],
        },
        owner,
        id(&public(11)),
    )
    .await?;
    let bytes = b"synthetic never-published abandoned upload";
    let hash = id(&auth::hash(bytes));
    store.put(hash, bytes).await?;
    repo.pool.get().await?.execute("UPDATE blob_intents SET staging_until=clock_timestamp()-interval '1 second' WHERE hash=$1",&[&hash.as_slice()]).await?;
    let recovery = store.recover_page(None).await?;
    let cleanup_candidates = store.reclaim_objects_page(true, true, true).await?;
    let staged_present = store.get(hash, owner).await.is_ok();
    assert_storage_used(
        &repo,
        &store,
        1048576,
        if staged_present {
            bytes.len() as u64
        } else {
            0
        },
    )
    .await?;
    drop(store);
    repo.set_account_admission(id(&public(2)), true).await?;
    repo.set_publisher_admission(id(&public(2)), true).await?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = listener.local_addr()?.to_string();
    drop(listener);
    let origin = format!("http://{bind}");
    let key = dir.path().join("key");
    write_key(&key, 11)?;
    let config = dir.path().join("node.json");
    std::fs::write(
        &config,
        serde_json::to_vec(
            &json!({"bind":bind,"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"storage":{"path":dir.path().join("node-store"),"volume":"nodeaudit","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":1048576},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}}),
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
        .no_proxy()
        .timeout(Duration::from_secs(3))
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
    .context(std::fs::read_to_string(dir.path().join("node.log")).unwrap_or_default())?;
    let info: Value = c
        .get(&origin)
        .header("Accept", "application/nostr+json")
        .send()
        .await?
        .json()
        .await?;
    let preflight = c
        .request(reqwest::Method::OPTIONS, format!("{origin}/libraries"))
        .header("Origin", "https://reader.example")
        .header("Access-Control-Request-Method", "POST")
        .header(
            "Access-Control-Request-Headers",
            "authorization,content-type",
        )
        .send()
        .await?;
    let preflight_status = preflight.status().as_u16();
    let cors = preflight
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let image = png();
    let h = auth::hash(&image);
    let nonadmin_upload = c
        .put(format!("{origin}/upload/{h}"))
        .header("authorization", blossom(2, &origin, "upload", &h))
        .body(image.clone())
        .send()
        .await?;
    let nonadmin_status = nonadmin_upload.status().as_u16();
    let nonadmin_body = nonadmin_upload.text().await?;
    let body = b"{}";
    let publish_url = format!("{origin}/publications");
    let nonadmin_publish = c
        .post(&publish_url)
        .header("authorization", http(2, &publish_url, body))
        .body(body.to_vec())
        .send()
        .await?;
    let publish_status = nonadmin_publish.status().as_u16();
    let publish_body = nonadmin_publish.text().await?;
    let standard_upload = c
        .put(format!("{origin}/upload"))
        .header("authorization", blossom(1, &origin, "upload", &h))
        .header("X-SHA-256", &h)
        .body(image)
        .send()
        .await?
        .status()
        .as_u16();
    let deletion = c
        .delete(format!("{origin}/blobs/{h}"))
        .header("authorization", blossom(1, &origin, "delete", &h))
        .send()
        .await?
        .status()
        .as_u16();
    let category_query = c
        .get(format!("{origin}/query"))
        .query(&[("order", "asc"), ("filter", "categories=\"Action\"")])
        .send()
        .await?
        .status()
        .as_u16();
    let report = json!({"scope":"diagnostic observations, not feature conformance passes","expired_unreferenced_upload":{"clock_advanced_by_fixture_sql":true,"recover_cursor":recovery.map(hex::encode),"cleanup_candidates":cleanup_candidates,"capacity_checked_through_actual_stage_delete":true,"still_present":staged_present},"admitted_nonadmin":{"account_and_publisher_permissions_true":true,"upload_status":nonadmin_status,"upload_body":nonadmin_body,"publication_status":publish_status,"publication_body":publish_body},"nip11":info,"cross_origin_preflight":{"status":preflight_status,"allow_origin":cors},"blossom_standard_upload_status":standard_upload,"direct_blob_delete_status":deletion,"category_query_status":category_query});
    std::fs::write(
        root.join("results/feature-audit-current.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{report}");
    Ok(())
}
