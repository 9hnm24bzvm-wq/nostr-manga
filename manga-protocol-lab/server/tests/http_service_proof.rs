mod support;
use anyhow::{Context, Result};
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
struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
async fn repository(dsn: String) -> Result<Repository> {
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
#[ignore = "requires isolated PostgreSQL, strfry and synthetic loopback proxy"]
async fn forwarded_identity_does_not_authorize_modified_or_replayed_metadata() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(cfg.get_dbname() == Some("manga_test"), "fixture only");
    let a = repository(dsn.clone()).await?;
    a.migrate().await?;
    a.pool
        .get()
        .await?
        .batch_execute("CREATE DATABASE manga_proof_b")
        .await?;
    let bdsn = dsn.replace("dbname=manga_test", "dbname=manga_proof_b");
    let b = repository(bdsn.clone()).await?;
    b.migrate().await?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let dir = tempfile::tempdir()?;
    let mode = dir.path().join("mode");
    std::fs::write(&mode, "pass")?;
    let proxy_log = std::fs::File::create(dir.path().join("proxy.log"))?;
    let _proxy = Process(
        Command::new("python3")
            .current_dir(root)
            .env("PYTHONPATH", root.join("vendor/pydeps"))
            .args([
                "server/tests/support/tamper_proxy.py",
                "19560",
                "19561",
                mode.to_str().unwrap(),
            ])
            .stdout(proxy_log.try_clone()?)
            .stderr(proxy_log)
            .spawn()?,
    );
    let origins = ["http://127.0.0.1:19560", "http://127.0.0.1:19562"];
    let mut processes = vec![];
    for (i, dsn) in [dsn, bdsn].into_iter().enumerate() {
        let base = dir.path().join(format!("node{i}"));
        std::fs::create_dir(&base)?;
        let key = base.join("key");
        write_key(&key, 11 + i as u8)?;
        let config = base.join("node.json");
        std::fs::write(
            &config,
            serde_json::to_vec(
                &json!({"bind":format!("127.0.0.1:{}",19561+i),"base_url":origins[i],"service_key_file":key,"admin_pubkey":public(1+i as u8),"database_plaintext_local":true,"storage":{"path":base.join("objects"),"volume":format!("proof{i}"),"physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":base.join("backend"),"map_bytes":67108864}}),
            )?,
        )?;
        let log = std::fs::File::create(base.join("node.log"))?;
        processes.push(Process(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", config.to_str().unwrap()])
                .env("MANGA_DATABASE_URL", dsn)
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        ));
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()?;
    for origin in origins {
        let mut ready = false;
        for _ in 0..100 {
            if client
                .get(origin)
                .send()
                .await
                .is_ok_and(|r| r.status() == 200)
            {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        anyhow::ensure!(
            ready,
            "node/proxy startup failed: {}",
            std::fs::read_to_string(dir.path().join("proxy.log"))?
        );
    }
    for (origin, key, peer, endpoint) in [
        (origins[0], 1, 12, origins[1]),
        (origins[1], 2, 11, origins[0]),
    ] {
        assert_eq!(
            post_json(
                &client,
                key,
                &format!("{origin}/admin/peers"),
                &json!({"key":public(peer),"endpoint":endpoint,"allowed":true})
            )
            .await?
            .status(),
            200
        );
    }
    let image = png();
    upload_to(&client, 1, origins[0], &image).await?;
    let manifest = cbor(
        json!({"version":1,"series":"ab".repeat(32),"language":"en","chapter":"1","pages":[page(&image)]}),
    );
    upload_to(&client, 1, origins[0], &manifest).await?;
    let event = publication(1, 1001, &manifest);
    assert!(
        post_json(
            &client,
            1,
            &format!("{}/publications", origins[0]),
            &serde_json::to_value(&event)?
        )
        .await?
        .status()
        .is_success()
    );
    wait_projected(&client, origins[0], &event.id).await?;
    let nonce = "ca".repeat(32);
    let identity: auth::Event = client
        .get(format!("{}/identity?nonce={nonce}", origins[0]))
        .send()
        .await?
        .json()
        .await?;
    identity.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(identity.pubkey, public(11));
    assert_eq!(identity.one("nonce").unwrap(), nonce);
    let sync_url = format!("{}/admin/sync", origins[1]);
    let request = json!({"peer":public(11),"event":event.id});
    for attack in ["tamper", "strip"] {
        std::fs::write(&mode, attack)?;
        assert_eq!(
            post_json(&client, 2, &sync_url, &request).await?.status(),
            400,
            "{attack} metadata admitted"
        );
        assert!(
            b.visible_event(hex::decode(&event.id)?.try_into().unwrap())
                .await?
                .is_none()
        );
    }
    std::fs::write(&mode, "pass")?;
    assert!(
        post_json(&client, 2, &sync_url, &request)
            .await?
            .status()
            .is_success()
    );
    wait_projected(&client, origins[1], &event.id).await?;
    std::fs::write(&mode, "replay")?;
    assert_eq!(
        post_json(&client, 2, &sync_url, &request).await?.status(),
        400,
        "proof from another request replayed"
    );
    std::fs::write(&mode, "tamper")?;
    let job: Value = post_json(
        &client,
        2,
        &format!("{}/admin/sync-jobs", origins[1]),
        &json!({"peer":public(11)}),
    )
    .await?
    .json()
    .await?;
    let job = job["id"].as_str().context("job missing")?;
    let mut rejected = false;
    for _ in 0..100 {
        let state = b.sync_status(hex::decode(job)?.try_into().unwrap()).await?;
        if state["last_error"]
            .as_str()
            .is_some_and(|s| s.contains("service response payload binding"))
        {
            rejected = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    anyhow::ensure!(
        rejected,
        "modified inventory was not rejected by proof verification"
    );
    b.cancel_sync(hex::decode(job)?.try_into().unwrap()).await?;
    for attack in ["pass", "acktamper"] {
        std::fs::write(&mode, attack)?;
        let code = r#"import sys
from src.postgres_migration import Client,Signer,MigrationError
c=Client(sys.argv[1],Signer(f'{1:064x}'));c.verify_service(sys.argv[2])
try:
 result=c.request('POST','/admin/check',{'synthetic-proof-test':True})
except MigrationError as error:
 assert sys.argv[3]=='acktamper' and 'response proof rejected' in str(error),str(error)
else:
 assert sys.argv[3]=='pass' and result['ok']
"#;
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
            .args(["-c", code, origins[0], &public(11), attack])
            .output()
            .await?;
        anyhow::ensure!(
            output.status.success(),
            "import-client proof test: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    std::fs::write(
        root.join("results/federation-response-proof.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["identity challenge remains valid through synthetic adversarial proxy","JSON-equivalent metadata rewrite refused before hosting","missing response proof refused","honest signed transfer succeeds with verified local originals","proof replay across fresh request refused","modified inventory response refused before reconciliation","migration client accepts genuine acknowledgements and rejects rewritten ones"],"scope":"two synthetic PostgreSQL/strfry nodes and loopback HTTP proxy; no real domain takeover"}),
        )?,
    )?;
    Ok(())
}
