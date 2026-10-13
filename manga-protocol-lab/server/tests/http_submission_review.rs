mod support;
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
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
fn id(s: &str) -> Vec<u8> {
    hex::decode(s).unwrap()
}
fn chapter(label: &str, image: &[u8]) -> (auth::Event, Vec<u8>) {
    let bytes = cbor(
        json!({"version":1,"series":"ab".repeat(32),"language":"en","chapter":label,"pages":[page(image)]}),
    );
    (publication(2, 1001, &bytes), bytes)
}
async fn declare(
    c: &reqwest::Client,
    origin: &str,
    event: &auth::Event,
    bytes: &[u8],
) -> Result<reqwest::Response> {
    post_json(
        c,
        2,
        &format!("{origin}/submissions"),
        &json!({"event":event,"manifest":STANDARD.encode(bytes)}),
    )
    .await
}
async fn stage(
    c: &reqwest::Client,
    origin: &str,
    event: &str,
    bytes: &[u8],
    actor: u8,
) -> Result<reqwest::Response> {
    let url = format!("{origin}/submissions/{event}/blobs/{}", auth::hash(bytes));
    Ok(c.post(&url)
        .header("authorization", http(actor, &url, bytes))
        .body(bytes.to_vec())
        .send()
        .await?)
}
async fn decision(
    c: &reqwest::Client,
    origin: &str,
    event: &str,
    action: &str,
) -> Result<reqwest::Response> {
    post_json(
        c,
        1,
        &format!("{origin}/admin/submissions/{event}/decision"),
        &json!({"action":action}),
    )
    .await
}
async fn status(
    c: &reqwest::Client,
    origin: &str,
    event: &str,
    actor: u8,
) -> Result<reqwest::Response> {
    let url = format!("{origin}/submissions/{event}");
    Ok(c.get(&url)
        .header("authorization", http_get(actor, &url))
        .send()
        .await?)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated PostgreSQL and real HTTP node"]
async fn admitted_review_is_private_one_off_atomic_and_expiring() -> Result<()> {
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
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = listener.local_addr()?;
    drop(listener);
    let origin = format!("http://{bind}");
    let key = dir.path().join("service.key");
    write_key(&key, 11)?;
    let path = dir.path().join("node.json");
    std::fs::write(
        &path,
        serde_json::to_vec(
            &json!({"bind":bind.to_string(),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"submission_review":{"account_pending":2,"instance_pending":4,"account_bytes":32768,"instance_bytes":65536,"submission_bytes":16384,"objects":8,"lifetime_seconds":3600,"records":32},"storage":{"path":dir.path().join("originals"),"volume":"review","physical_bytes":2097152,"staging_bytes":65536,"max_blob_bytes":262144},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}}),
        )?,
    )?;
    let spawn = || -> Result<Node> {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.path().join("node.log"))?;
        Ok(Node(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", path.to_str().unwrap()])
                .env("MANGA_DATABASE_URL", &dsn)
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        ))
    };
    let node = spawn()?;
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
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
    .with_context(|| std::fs::read_to_string(dir.path().join("node.log")).unwrap_or_default())?;
    let image = png();
    let (event, manifest) = chapter("approved", &image);
    assert_eq!(
        declare(&c, &origin, &event, &manifest).await?.status(),
        403,
        "non-admitted intake denied"
    );
    for actor in [2, 3] {
        assert_eq!(
            post_json(
                &c,
                1,
                &format!("{origin}/admin/accounts/{}", public(actor)),
                &json!({"allowed":true})
            )
            .await?
            .status(),
            200
        );
    }
    assert_eq!(declare(&c, &origin, &event, &manifest).await?.status(), 201);
    assert_eq!(
        post_json(
            &c,
            2,
            &format!("{origin}/admin/submissions/{}/decision", event.id),
            &json!({"action":"approve"})
        )
        .await?
        .status(),
        403,
        "submitter cannot approve their draft"
    );
    assert_eq!(
        stage(&c, &origin, &event.id, b"undeclared", 2)
            .await?
            .status(),
        404,
        "undeclared hash denied before storage"
    );
    let before: Value = status(&c, &origin, &event.id, 2).await?.json().await?;
    assert_eq!(before["state"], "pending");
    assert_eq!(declare(&c, &origin, &event, &manifest).await?.status(), 200);
    let after: Value = status(&c, &origin, &event.id, 2).await?.json().await?;
    assert_eq!(after["expires_at"], before["expires_at"]);
    // An idempotent redeclaration must not consume the second account slot.
    let (slot, slot_manifest) = chapter("duplicate-quota-probe", &image);
    let (overflow, overflow_manifest) = chapter("duplicate-quota-overflow", &image);
    assert_eq!(
        declare(&c, &origin, &slot, &slot_manifest).await?.status(),
        201
    );
    assert_eq!(
        declare(&c, &origin, &overflow, &overflow_manifest)
            .await?
            .status(),
        429
    );
    assert_eq!(
        post_json(
            &c,
            2,
            &format!("{origin}/submissions/{}/withdraw", slot.id),
            &json!({})
        )
        .await?
        .status(),
        200
    );
    assert_eq!(
        declare(&c, &origin, &overflow, &overflow_manifest)
            .await?
            .status(),
        201
    );
    assert_eq!(
        post_json(
            &c,
            2,
            &format!("{origin}/submissions/{}/withdraw", overflow.id),
            &json!({})
        )
        .await?
        .status(),
        200
    );
    assert_eq!(status(&c, &origin, &event.id, 3).await?.status(), 404);
    assert_eq!(
        c.get(format!("{origin}/events/{}", event.id))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        decision(&c, &origin, &event.id, "approve").await?.status(),
        404,
        "missing original cannot be approved"
    );
    assert_eq!(
        stage(&c, &origin, &event.id, &image, 3).await?.status(),
        404
    );
    assert_eq!(
        stage(&c, &origin, &event.id, &image, 2).await?.status(),
        200
    );
    let read_url = format!(
        "{origin}/submissions/{}/blobs/{}",
        event.id,
        auth::hash(&image)
    );
    let response = c
        .get(&read_url)
        .header("authorization", http_get(2, &read_url))
        .send()
        .await?;
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    assert_eq!(response.bytes().await?.as_ref(), image);
    let delete_url = format!("{origin}/blobs/{}", auth::hash(&image));
    assert_eq!(
        c.delete(&delete_url)
            .header(
                "authorization",
                blossom(1, &origin, "delete", &auth::hash(&image))
            )
            .send()
            .await?
            .status(),
        409,
        "pending review protects private originals"
    );
    repo.pool.get().await?.batch_execute("ALTER TABLE review_submissions ADD CONSTRAINT fixture_no_approval CHECK(state<>'approved') NOT VALID").await?;
    let failed = decision(&c, &origin, &event.id, "approve").await?;
    repo.pool
        .get()
        .await?
        .batch_execute("ALTER TABLE review_submissions DROP CONSTRAINT fixture_no_approval")
        .await?;
    assert_eq!(failed.status(), 503, "operational failure is retryable");
    let pending: Value = status(&c, &origin, &event.id, 2).await?.json().await?;
    assert_eq!(pending["state"], "pending");
    assert_eq!(
        c.get(format!("{origin}/events/{}", event.id))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        c.get(format!("{origin}/originals/{}", auth::hash(&image)))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        c.get(&read_url)
            .header("authorization", http_get(2, &read_url))
            .send()
            .await?
            .bytes()
            .await?
            .as_ref(),
        image
    );
    assert_eq!(
        c.delete(&delete_url)
            .header(
                "authorization",
                blossom(1, &origin, "delete", &auth::hash(&image))
            )
            .send()
            .await?
            .status(),
        409,
        "failed approval retains private draft protection"
    );
    let response = decision(&c, &origin, &event.id, "approve").await?;
    let code = response.status();
    let body = response.text().await?;
    ensure!(code == 200, "approve: {code} {body}");
    wait_projected(&c, &origin, &event.id).await?;
    assert_eq!(
        decision(&c, &origin, &event.id, "approve").await?.status(),
        200,
        "lost receipt retry idempotent"
    );
    assert_eq!(
        post_json(
            &c,
            2,
            &format!("{origin}/publications"),
            &serde_json::to_value(&event)?
        )
        .await?
        .status(),
        403
    );
    assert_eq!(
        c.get(format!("{origin}/originals/{}", auth::hash(&image)))
            .send()
            .await?
            .bytes()
            .await?
            .as_ref(),
        image
    );
    let (rejected, rejected_manifest) = chapter("rejected", &image);
    assert_eq!(
        declare(&c, &origin, &rejected, &rejected_manifest)
            .await?
            .status(),
        201
    );
    assert_eq!(
        decision(&c, &origin, &rejected.id, "reject")
            .await?
            .status(),
        200
    );
    assert_eq!(
        declare(&c, &origin, &rejected, &rejected_manifest)
            .await?
            .status(),
        409,
        "rejected request never reopens"
    );
    assert_eq!(
        decision(&c, &origin, &rejected.id, "approve")
            .await?
            .status(),
        409
    );
    let (withdrawn, withdrawn_manifest) = chapter("withdrawn", &image);
    assert_eq!(
        declare(&c, &origin, &withdrawn, &withdrawn_manifest)
            .await?
            .status(),
        201
    );
    assert_eq!(
        post_json(
            &c,
            2,
            &format!("{origin}/submissions/{}/withdraw", withdrawn.id),
            &json!({})
        )
        .await?
        .status(),
        200
    );
    let (expired, expired_manifest) = chapter("expired", &image);
    assert_eq!(
        declare(&c, &origin, &expired, &expired_manifest)
            .await?
            .status(),
        201
    );
    repo.pool.get().await?.execute("UPDATE review_submissions SET expires_at=clock_timestamp()-interval '1 second' WHERE event_id=$1",&[&id(&expired.id)]).await?;
    assert_eq!(
        stage(&c, &origin, &expired.id, &image, 2).await?.status(),
        409
    );
    assert_eq!(repo.expire_reviews().await?, 1);
    let expired_state: Value = status(&c, &origin, &expired.id, 2).await?.json().await?;
    assert_eq!(expired_state["state"], "expired");
    let series_manifest = cbor(
        json!({"version":1,"titles":{"en":"Reviewed cover"},"required":["series-cover-v1"],"cover":page(&image)}),
    );
    let series = publication(2, 1002, &series_manifest);
    assert_eq!(
        declare(&c, &origin, &series, &series_manifest)
            .await?
            .status(),
        201
    );
    assert_eq!(
        decision(&c, &origin, &series.id, "approve").await?.status(),
        404,
        "review cover needs its declared grant even when bytes are shared"
    );
    assert_eq!(
        stage(&c, &origin, &series.id, &image, 2).await?.status(),
        200
    );
    assert_eq!(
        decision(&c, &origin, &series.id, "approve").await?.status(),
        200
    );
    wait_projected(&c, &origin, &series.id).await?;
    let (quota_a, qa) = chapter("quota-a", &image);
    let (quota_b, qb) = chapter("quota-b", &image);
    let (quota_c, qc) = chapter("quota-c", &image);
    assert_eq!(declare(&c, &origin, &quota_a, &qa).await?.status(), 201);
    assert_eq!(declare(&c, &origin, &quota_b, &qb).await?.status(), 201);
    assert_eq!(declare(&c, &origin, &quota_c, &qc).await?.status(), 429);
    assert_eq!(
        post_json(
            &c,
            1,
            &format!("{origin}/admin/accounts/{}", public(2)),
            &json!({"allowed":false})
        )
        .await?
        .status(),
        200
    );
    assert_eq!(
        decision(&c, &origin, &quota_a.id, "approve")
            .await?
            .status(),
        403,
        "revoked author cannot be approved"
    );
    assert_eq!(
        stage(&c, &origin, &quota_a.id, &image, 2).await?.status(),
        403,
        "revoked account cannot upload"
    );
    drop(node);
    let _node = spawn()?;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if c.get(&origin).send().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    let admin_url = format!("{origin}/admin/submissions/{}", event.id);
    let saved: Value = c
        .get(&admin_url)
        .header("authorization", http_get(1, &admin_url))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(saved["state"], "approved");
    assert_eq!(saved["decided_by"], public(1));
    std::fs::write(
        root.join("results/http-submission-review.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["admitted-account intake only","submitter cannot moderate","undeclared hash denied","revoked account cannot upload","private drafts and owner-scoped objects","idempotent declaration without TTL extension","missing originals prevent approval","pending reviews protect originals","approval and hosted references commit atomically","database error returns503 with no partial hosting","approval grants no ongoing publisher authority","rejection/withdrawal/expiry remain closed","pending-count budget enforced","revoked account blocks approval","operator decision survives restart","one-off series review validates and retains original cover"],"limits":["synthetic loopback single-node workflow","publication policy rejection does not itself delete an already-hosted copy"]}),
        )?,
    )?;
    Ok(())
}
