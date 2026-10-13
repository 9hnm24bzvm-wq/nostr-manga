mod support;
use anyhow::{Context, Result};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    groups::{GroupPublication, Groups},
    publication::Publications,
    upload::{StorageConfig, UploadStore},
};
use serde_json::json;
use std::{
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
async fn audit(client: &reqwest::Client, url: &str) -> Result<serde_json::Value> {
    let response = client
        .get(url)
        .header("authorization", http_get(1, url))
        .send()
        .await?;
    anyhow::ensure!(
        response.status() == 200,
        "audit response {}",
        response.status()
    );
    Ok(response.json().await?)
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL; run scripts/test-postgres.sh"]
async fn group_preparation_is_fenced_by_current_membership() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(cfg.get_dbname() == Some("manga_test"), "fixture required");
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
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = listener.local_addr()?.to_string();
    drop(listener);
    let origin = format!("http://{bind}");
    let key = dir.path().join("service-key");
    write_key(&key, 11)?;
    let config = dir.path().join("node.json");
    std::fs::write(
        &config,
        serde_json::to_vec(
            &json!({"bind":bind,"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true}),
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
    let http = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()?;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if http.get(&origin).send().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await?;
    let keys = dir.path().join("keys");
    let storage = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.path().join("originals"),
            volume: "groupfence".into(),
            physical_bytes: 1048576,
            staging_bytes: 1048576,
            max_blob_bytes: 1048576,
            upload_slots: 2,
            reclamation_slots: 4,
            credit_lanes: 16,
            formats: vec!["image/png".into()],
        },
        owner,
        id(&public(11)),
    )
    .await?;
    let publications = Publications::new(repo.clone(), storage.clone());
    let groups = Groups::open(repo.clone(), keys.clone(), storage, publications.clone())?;
    let admin = id(&public(2));
    let member = id(&public(3));
    let group = groups.create(admin).await?;
    let audit_url = format!("{origin}/admin/groups/{}/audit", hex::encode(group));
    let version = groups.authorize(group, admin, "admin").await?;
    assert!(
        groups
            .set_member(group, admin, version, admin, None)
            .await
            .is_err(),
        "last administrator cannot be removed accidentally"
    );
    groups
        .set_member(group, admin, version, member, Some("publisher".into()))
        .await?;
    let version = groups.authorize(group, member, "publish").await?;
    let image = png();
    groups
        .upload(group, member, version, id(&auth::hash(&image)), &image)
        .await?;
    let manifest =
        cbor(json!({"version":1,"series":"ab".repeat(32),"language":"en","pages":[page(&image)]}));
    let mh = auth::hash(&manifest);
    groups
        .upload(group, member, version, id(&mh), &manifest)
        .await?;
    let secret = std::fs::read_to_string(recovered_key_path(&keys, group))?;
    let event = auth::sign(
        &secret,
        1001,
        vec![],
        json!({"manifest":mh,"version":1}).to_string(),
    )
    .map_err(anyhow::Error::msg)?;
    let prepared = publications
        .prepare_for_group(&event, group, member, version)
        .await?;
    groups
        .set_member(group, admin, version, member, None)
        .await?;
    assert!(publications.commit(prepared).await.is_err());
    assert!(
        groups
            .deletion_request(
                group,
                member,
                version,
                vec![event.id.clone()],
                "stale role".into()
            )
            .await
            .is_err()
    );
    assert!(repo.visible_event(id(&event.id)).await?.is_none());
    assert!(!repo.projection_complete(id(&event.id)).await?);
    assert!(
        audit(&http, &audit_url).await?["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let version = groups.authorize(group, admin, "admin").await?;
    groups
        .set_member(group, admin, version, member, Some("publisher".into()))
        .await?;
    let version = groups.authorize(group, admin, "publish").await?;
    let first = publications
        .prepare_for_group(&event, group, admin, version)
        .await?;
    let duplicate = publications
        .prepare_for_group(&event, group, member, version)
        .await?;
    assert!(publications.commit(first).await?);
    assert!(!publications.commit(duplicate).await?);
    assert!(repo.visible_event(id(&event.id)).await?.is_some());
    let observed = audit(&http, &audit_url).await?;
    let entries = observed["items"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["action"], "publish");
    assert_eq!(entries[0]["member"], public(2));
    assert_eq!(entries[0]["event_id"], event.id);
    let version = groups.authorize(group, admin, "publish").await?;
    std::fs::write(recovered_key_path(&keys, group), format!("{:064x}", 19))?;
    assert!(
        groups
            .publish(
                group,
                admin,
                version,
                GroupPublication {
                    kind: 1001,
                    manifest: mh,
                    version: 1,
                    created_at: None
                }
            )
            .await
            .is_err(),
        "wrong key material cannot sign for a registered reference"
    );
    std::fs::write(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../results/postgres-group-fences.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["last administrator removal guarded","member revocation after full preparation blocks final publication commit","stale removal role cannot persist a signed group request","rejected role leaves publication invisible and no successful audit entry","key-reference mismatch rejects signing"]}),
        )?,
    )?;
    Ok(())
}
