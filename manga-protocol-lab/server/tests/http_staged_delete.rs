mod support;
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
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
async fn post(c: &reqwest::Client, key: u8, url: &str, body: &[u8]) -> Result<reqwest::Response> {
    Ok(c.post(url)
        .header("authorization", http(key, url, body))
        .body(body.to_vec())
        .send()
        .await?)
}
async fn upload(c: &reqwest::Client, key: u8, origin: &str, bytes: &[u8]) -> Result<String> {
    let hash = auth::hash(bytes);
    let r = c
        .put(format!("{origin}/upload/{hash}"))
        .header("authorization", blossom(key, origin, "upload", &hash))
        .body(bytes.to_vec())
        .send()
        .await?;
    ensure!(r.status().is_success(), "upload: {}", r.text().await?);
    Ok(hash)
}
async fn delete(
    c: &reqwest::Client,
    key: u8,
    origin: &str,
    hash: &str,
) -> Result<reqwest::Response> {
    Ok(c.delete(format!("{origin}/blobs/{hash}"))
        .header("authorization", blossom(key, origin, "delete", hash))
        .send()
        .await?)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated PostgreSQL and real authenticated HTTP node"]
async fn direct_upload_deletion_is_scoped_and_never_moderates_hosted_content() -> Result<()> {
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
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let dir = tempfile::tempdir()?;
    let key = dir.path().join("key");
    write_key(&key, 11)?;
    let sock = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = sock.local_addr()?.to_string();
    drop(sock);
    let origin = format!("http://{bind}");
    let config = dir.path().join("config.json");
    std::fs::write(
        &config,
        serde_json::to_vec(
            &json!({"bind":bind,"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"group_keys_dir":dir.path().join("groups"),"storage":{"path":dir.path().join("objects"),"volume":"deletehttp","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":1048576},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}}),
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
    for who in [2, 3] {
        repo.set_publisher_admission(id(&public(who)), true).await?;
    }
    let h = upload(&c, 2, &origin, b"owned disposable bytes").await?;
    assert_eq!(
        c.delete(format!("{origin}/blobs/{h}"))
            .send()
            .await?
            .status(),
        403
    );
    assert_eq!(delete(&c, 3, &origin, &h).await?.status(), 403);
    assert_eq!(
        c.delete(format!("{origin}/blobs/{h}"))
            .header("authorization", blossom(2, &origin, "get", &h))
            .send()
            .await?
            .status(),
        403
    );
    assert_eq!(
        c.delete(format!("{origin}/blobs/{h}"))
            .header(
                "authorization",
                blossom(2, "http://127.0.0.1:1", "delete", &h)
            )
            .send()
            .await?
            .status(),
        403
    );
    assert_eq!(
        c.delete(format!("{origin}/blobs/{h}"))
            .header("authorization", blossom(2, &origin, "delete", &h))
            .body("unexpected")
            .send()
            .await?
            .status(),
        400
    );
    let r = delete(&c, 2, &origin, &h).await?;
    assert_eq!(r.status(), 200, "{}", r.text().await?);
    let v: Value = r.json().await?;
    assert_eq!(v["deleted"], true);
    assert!(repo.blob_intent(id(&h)).await?.is_none());
    assert_eq!(delete(&c, 2, &origin, &h).await?.status(), 404);
    let bytes = b"shared administrator upload";
    let h = upload(&c, 2, &origin, bytes).await?;
    upload(&c, 1, &origin, bytes).await?;
    assert_eq!(delete(&c, 2, &origin, &h).await?.status(), 409);
    assert_eq!(delete(&c, 1, &origin, &h).await?.status(), 409);
    repo.pool.get().await?.execute("UPDATE publisher_blob_grants SET lease_until=clock_timestamp()-interval '1 second' WHERE publisher=$1 AND hash=$2", &[&id(&public(1)).as_slice(),&id(&h).as_slice()]).await?;
    assert_eq!(delete(&c, 2, &origin, &h).await?.status(), 200);
    let series = cbor(json!({"version":1,"titles":{"en":"Protected published fixture"}}));
    let h = upload(&c, 2, &origin, &series).await?;
    let e = publication(2, 1002, &series);
    assert!(
        post(
            &c,
            2,
            &format!("{origin}/publications"),
            &serde_json::to_vec(&e)?
        )
        .await?
        .status()
        .is_success()
    );
    assert_eq!(delete(&c, 2, &origin, &h).await?.status(), 409);
    assert_eq!(delete(&c, 1, &origin, &h).await?.status(), 409);
    assert!(repo.blob_intent(id(&h)).await?.is_some());
    // Group membership governs staging deletion without returning the group key.
    let r = post(
        &c,
        1,
        &format!("{origin}/admin/groups"),
        &serde_json::to_vec(&json!({"administrator":public(2)}))?,
    )
    .await?;
    assert!(r.status().is_success(), "{}", r.text().await?);
    let group: Value = r.json().await?;
    assert_eq!(group["private_key_returned"], false);
    let group = group["group"].as_str().unwrap();
    let bytes = b"group disposable staging";
    let h = auth::hash(bytes);
    assert!(
        post(&c, 2, &format!("{origin}/groups/{group}/upload/{h}"), bytes)
            .await?
            .status()
            .is_success()
    );
    let url = format!("{origin}/groups/{group}/delete-upload/{h}");
    assert_eq!(post(&c, 3, &url, b"{}").await?.status(), 403);
    let r = post(&c, 2, &url, b"{}").await?;
    assert_eq!(r.status(), 200, "{}", r.text().await?);
    assert_eq!(r.json::<Value>().await?["deleted"], true);
    assert!(repo.blob_intent(id(&h)).await?.is_none());
    std::fs::write(
        root.join("results/http-staged-deletion.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["publisher-owned staged upload physically deleted with receipt settlement","unauthenticated, wrong publisher/action/domain and nonempty body rejected","missing object returns404","both publisher and administrator interests protect shared bytes","hosted publication cannot be removed through blob deletion","group member removal role gates staged deletion without exposing private key"],"policy":"shared or retained objects return409 without releasing another interest","remaining":"standard Blossom route/profile and browser CORS interoperability remain separate ticket22"}),
        )?,
    )?;
    Ok(())
}
