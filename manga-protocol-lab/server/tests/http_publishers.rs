//! Non-admin publication and epoch/grant isolation against a real node/PostgreSQL.
mod support;
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    publication::Publications,
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
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
async fn send(
    c: &reqwest::Client,
    who: u8,
    url: &str,
    value: &serde_json::Value,
) -> Result<reqwest::Response> {
    let body = serde_json::to_vec(value)?;
    Ok(c.post(url)
        .header("authorization", http(who, url, &body))
        .body(body)
        .send()
        .await?)
}
async fn upload(
    c: &reqwest::Client,
    who: u8,
    origin: &str,
    bytes: &[u8],
) -> Result<reqwest::Response> {
    let hash = auth::hash(bytes);
    Ok(c.put(format!("{origin}/upload/{hash}"))
        .header("authorization", blossom(who, origin, "upload", &hash))
        .body(bytes.to_vec())
        .send()
        .await?)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated PostgreSQL and real strfry; run scripts/test-postgres.sh"]
async fn independent_publishers_have_scoped_staging_and_fenced_authority() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    let parsed: tokio_postgres::Config = dsn.parse()?;
    ensure!(
        parsed.get_dbname() == Some("manga_test"),
        "isolated fixture required"
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
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let dir = tempfile::tempdir()?;
    let key = dir.path().join("key");
    write_key(&key, 11)?;
    let socket = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = socket.local_addr()?.to_string();
    drop(socket);
    let origin = format!("http://{bind}");
    let storage_path = dir.path().join("objects");
    let storage = json!({"path":storage_path,"volume":"publishers","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":1048576,"formats":["image/png"]});
    let config = dir.path().join("config.json");
    std::fs::write(
        &config,
        serde_json::to_vec(
            &json!({"bind":bind,"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"storage":storage,"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"timeout_seconds":2,"map_bytes":67108864}}),
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
    let image = png();
    assert_eq!(upload(&c, 2, &origin, &image).await?.status(), 403);
    // Account admission alone is not permission to publish original content.
    repo.set_account_admission(id(&public(2)), true).await?;
    assert_eq!(upload(&c, 2, &origin, &image).await?.status(), 403);
    let admit = format!("{origin}/admin/publishers/{}", public(2));
    assert_eq!(
        send(&c, 2, &admit, &json!({"allowed":true}))
            .await?
            .status(),
        403
    );
    assert!(
        send(&c, 1, &admit, &json!({"allowed":true}))
            .await?
            .status()
            .is_success()
    );
    assert!(upload(&c, 2, &origin, &image).await?.status().is_success());
    let series = cbor(json!({"version":1,"titles":{"en":"Independent publisher fixture"}}));
    assert!(upload(&c, 2, &origin, &series).await?.status().is_success());
    let se = publication(2, 1002, &series);
    let publish_url = format!("{origin}/publications");
    let r = send(&c, 2, &publish_url, &serde_json::to_value(&se)?).await?;
    assert!(r.status().is_success(), "{}", r.text().await?);
    let chapter = cbor(
        json!({"version":1,"series":se.id,"language":"en","chapter":"1","pages":[page(&image)]}),
    );
    assert!(
        upload(&c, 2, &origin, &chapter)
            .await?
            .status()
            .is_success()
    );
    let event = publication(2, 1001, &chapter);
    // Even the operator request signer cannot impersonate another author.
    assert_eq!(
        send(&c, 1, &publish_url, &serde_json::to_value(&event)?)
            .await?
            .status(),
        403
    );
    let r = send(&c, 2, &publish_url, &serde_json::to_value(&event)?).await?;
    assert!(r.status().is_success(), "{}", r.text().await?);
    let eid = id(&event.id);
    tokio::time::timeout(Duration::from_secs(5), async {
        while !repo.projection_complete(eid).await? {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    assert_eq!(
        c.get(format!("{origin}/originals/{}", auth::hash(&image)))
            .send()
            .await?
            .bytes()
            .await?
            .as_ref(),
        image
    );
    let h = auth::hash(&image);
    assert_eq!(
        c.get(format!("{origin}/blobs/{h}"))
            .header("authorization", blossom(2, &origin, "get", &h))
            .send()
            .await?
            .bytes()
            .await?
            .as_ref(),
        image
    );
    repo.set_publisher_admission(id(&public(3)), true).await?;
    assert!(
        !c.get(format!("{origin}/blobs/{h}"))
            .header("authorization", blossom(3, &origin, "get", &h))
            .send()
            .await?
            .status()
            .is_success()
    );
    // Knowing another user's staged manifest hash is insufficient to publish it.
    let stolen = publication(3, 1001, &chapter);
    assert!(
        !send(&c, 3, &publish_url, &serde_json::to_value(&stolen)?)
            .await?
            .status()
            .is_success()
    );
    let before = repo
        .blob_intent(id(&h))
        .await?
        .context("shared original missing")?
        .key;
    assert!(upload(&c, 3, &origin, &image).await?.status().is_success());
    assert!(
        upload(&c, 3, &origin, &chapter)
            .await?
            .status()
            .is_success()
    );
    assert!(
        send(&c, 3, &publish_url, &serde_json::to_value(&stolen)?)
            .await?
            .status()
            .is_success()
    );
    let after = repo
        .blob_intent(id(&h))
        .await?
        .context("shared original missing")?
        .key;
    assert_eq!(
        before, after,
        "shared upload must retain the same immutable original"
    );
    let shared = c
        .get(format!("{origin}/originals/{h}"))
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    assert_eq!(shared.as_ref(), image);
    let ws_manifest = cbor(
        json!({"version":1,"series":se.id,"language":"en","chapter":"websocket","pages":[page(&image)]}),
    );
    assert!(
        upload(&c, 2, &origin, &ws_manifest)
            .await?
            .status()
            .is_success()
    );
    let ws_event = publication(2, 1001, &ws_manifest);
    let ws_path = dir.path().join("publisher-ws.json");
    std::fs::write(&ws_path, serde_json::to_vec(&ws_event)?)?;
    let ws_output = tokio::process::Command::new("python3")
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
            "server/tests/support/publisher_relay_client.py",
            &origin,
            ws_path.to_str().unwrap(),
            "true",
        ])
        .output()
        .await?;
    ensure!(
        ws_output.status.success(),
        "publisher WS: {}",
        String::from_utf8_lossy(&ws_output.stderr)
    );
    assert!(
        send(&c, 1, &admit, &json!({"allowed":false}))
            .await?
            .status()
            .is_success()
    );
    assert_eq!(upload(&c, 2, &origin, &image).await?.status(), 403);
    assert_eq!(
        send(&c, 2, &publish_url, &serde_json::to_value(&event)?)
            .await?
            .status(),
        403
    );
    repo.set_publisher_admission(id(&public(2)), true).await?;
    let duplicate = send(&c, 2, &publish_url, &serde_json::to_value(&event)?).await?;
    assert!(
        duplicate.status().is_success(),
        "already committed native event must remain retryable after old grants were revoked: {}",
        duplicate.text().await?
    );
    drop(node);
    // Domain-level fences exercise changes after preparation, without timing luck.
    let store = UploadStore::open(
        repo.clone(),
        serde_json::from_value::<StorageConfig>(storage)?,
        id(&public(1)),
        id(&public(11)),
    )
    .await?;
    let publisher = Publications::new(repo.clone(), store.clone());
    repo.set_publisher_admission(id(&public(2)), true).await?;
    let permit = repo.publisher_permit(id(&public(2))).await?;
    let prepared_duplicate = publisher.prepare(&event).await?;
    assert!(
        !publisher.commit(prepared_duplicate).await?,
        "duplicate must not create a new hosted transition"
    );
    let fresh = sign_at(
        2,
        1001,
        vec![],
        json!({"manifest":auth::hash(&chapter),"version":1}).to_string(),
        auth::now().max(event.created_at + 1),
    );
    assert!(
        publisher.prepare(&fresh).await.is_err(),
        "revocation/re-admission cannot revive old staging grants for a NEW event"
    );
    store.put_for_publisher(&permit, id(&h), &image).await?;
    let m = cbor(
        json!({"version":1,"series":se.id,"language":"en","chapter":"fence","pages":[page(&image)]}),
    );
    let mh = id(&auth::hash(&m));
    store.put_for_publisher(&permit, mh, &m).await?;
    let e = publication(2, 1001, &m);
    let prepared = publisher.prepare_with_permit(&e, permit.clone()).await?;
    repo.set_publisher_admission(id(&public(2)), false).await?;
    repo.set_publisher_admission(id(&public(2)), true).await?;
    assert!(publisher.commit(prepared).await.is_err());
    assert!(
        store.put_for_publisher(&permit, mh, &m).await.is_err(),
        "old permit must not upload after epoch changes"
    );
    let permit = repo.publisher_permit(id(&public(2))).await?;
    store.put_for_publisher(&permit, id(&h), &image).await?;
    store.put_for_publisher(&permit, mh, &m).await?;
    let prepared = publisher.prepare_with_permit(&e, permit.clone()).await?;
    repo.pool.get().await?.execute("UPDATE publisher_blob_grants SET lease_until=clock_timestamp()-interval '1 second' WHERE publisher=$1 AND hash=$2", &[&id(&public(2)).as_slice(),&mh.as_slice()]).await?;
    assert!(
        publisher.commit(prepared).await.is_err(),
        "expired grant must fail final commit"
    );
    store.put_for_publisher(&permit, mh, &m).await?;
    assert!(publisher.publish_with_permit(&e, permit).await?);
    let physical_owner = repo
        .blob_intent(mh)
        .await?
        .context("published manifest missing")?
        .owner;
    assert_eq!(
        physical_owner,
        id(&public(1)),
        "logical publisher must not replace storage owner"
    );
    std::fs::write(
        root.join("results/publisher-authority.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["non-admin HTTP original and manifest upload","NIP42 authenticated WebSocket publication shares publisher grant enforcement","author-bound HTTP publication and real strfry projection","operator-only publisher admission distinct from account admission","staged read and publication grant isolation","shared uploads preserve the exact immutable original","physical owner remains unchanged","revocation/re-admission fences upload and prepared publication","expired grant fails final commit","renewed exact-byte grant permits publication","already committed native event remains retryable without reviving expired grants for new publications"],"remaining":["standard Blossom routes and abandoned staging collection are separate audit tickets","no target-scale capacity claim"]}),
        )?,
    )?;
    Ok(())
}
