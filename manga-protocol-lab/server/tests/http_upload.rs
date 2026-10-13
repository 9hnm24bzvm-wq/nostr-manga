use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
};
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Child, Command},
    time::Duration,
};
struct Node(Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn public(key: u8) -> String {
    auth::sign(&format!("{key:064x}"), 1, vec![], String::new())
        .unwrap()
        .pubkey
}
fn token(key: u8, action: &str, hash: &str) -> String {
    let mut event = auth::sign(
        &format!("{key:064x}"),
        24242,
        vec![
            vec!["t".into(), action.into()],
            vec!["server".into(), "http://127.0.0.1:19504".into()],
            vec!["x".into(), hash.into()],
            vec!["expiration".into(), (auth::now() + 120).to_string()],
        ],
        "Authorize fixture blob transfer".into(),
    )
    .unwrap();
    // Blossom requires created_at strictly before current time.
    event.created_at = auth::now() - 1;
    let digest = event.digest();
    event.id = hex::encode(&digest);
    let secp = secp256k1::Secp256k1::new();
    let keypair = secp256k1::Keypair::from_secret_key(
        &secp,
        &secp256k1::SecretKey::from_slice(&hex::decode(format!("{key:064x}")).unwrap()).unwrap(),
    );
    event.sig = secp
        .sign_schnorr_no_aux_rand(
            &secp256k1::Message::from_digest_slice(&digest).unwrap(),
            &keypair,
        )
        .to_string();
    format!(
        "Nostr {}",
        STANDARD.encode(serde_json::to_vec(&event).unwrap())
    )
}
async fn ready(c: &reqwest::Client) -> Result<()> {
    for _ in 0..60 {
        if c.get("http://127.0.0.1:19504/").send().await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    anyhow::bail!("HTTP fixture failed to start")
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run scripts/test-postgres.sh"]
async fn verified_http_upload() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(cfg.get_dbname() == Some("manga_test"), "fixture required");
    let repo = Repository::connect(DbConfig {
        dsn: dsn.clone(),
        connections: 4,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let dir = tempfile::tempdir()?;
    let key = dir.path().join("service.key");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    write!(options.open(&key)?, "{:064x}", 11)?;
    let config = dir.path().join("node.json");
    std::fs::write(
        &config,
        serde_json::to_vec(
            &json!({"bind":"127.0.0.1:19504","base_url":"http://127.0.0.1:19504","service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"storage":{"path":dir.path().join("volume"),"volume":"httpfixture","physical_bytes":32,"staging_bytes":32,"max_blob_bytes":32,"upload_slots":2}}),
        )?,
    )?;
    let spawn = || -> Result<Node> {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.path().join("node.log"))?;
        Ok(Node(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", config.to_str().unwrap()])
                .env("MANGA_DATABASE_URL", &dsn)
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        ))
    };
    let node = spawn()?;
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;
    ready(&c).await?;
    let bytes = vec![7u8; 32];
    let hash = auth::hash(&bytes);
    let url = format!("http://127.0.0.1:19504/upload/{hash}");
    let get = format!("http://127.0.0.1:19504/blobs/{hash}");
    assert_eq!(c.put(&url).body(bytes.clone()).send().await?.status(), 403);
    assert_eq!(
        c.put(&url)
            .header("authorization", token(2, "upload", &hash))
            .body(bytes.clone())
            .send()
            .await?
            .status(),
        403
    );
    assert_eq!(
        c.put(&url)
            .header("authorization", token(1, "get", &hash))
            .body(bytes.clone())
            .send()
            .await?
            .status(),
        403
    );
    assert_eq!(
        c.put(&url)
            .header("authorization", token(1, "upload", &hash))
            .body(vec![8u8; 32])
            .send()
            .await?
            .status(),
        400
    );
    assert_eq!(
        c.put(&url)
            .header("authorization", token(1, "upload", &hash))
            .body(vec![7u8; 33])
            .send()
            .await?
            .status(),
        413
    );
    // Body-independent authorization returns without receiving a declared body.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect("127.0.0.1:19504").await?;
    stream.write_all(format!("PUT /upload/{hash} HTTP/1.1\r\nHost: 127.0.0.1:19504\r\nContent-Length: 10000000\r\nConnection: close\r\n\r\n").as_bytes()).await?;
    let mut buf = [0u8; 1024];
    let n = tokio::time::timeout(Duration::from_secs(1), stream.read(&mut buf)).await??;
    assert!(String::from_utf8_lossy(&buf[..n]).starts_with("HTTP/1.1 403"));
    let response = c
        .put(&url)
        .header("authorization", token(1, "upload", &hash))
        .body(bytes.clone())
        .send()
        .await?;
    assert_eq!(response.status(), 200, "{}", response.text().await?);
    assert_eq!(c.get(&get).send().await?.status(), 403);
    assert_eq!(
        c.get(&get)
            .header("authorization", token(2, "get", &hash))
            .send()
            .await?
            .status(),
        403
    );
    let response = c
        .get(&get)
        .header("authorization", token(1, "get", &hash))
        .send()
        .await?;
    assert_eq!(response.status(), 200);
    assert_eq!(response.bytes().await?.as_ref(), bytes);
    let duplicate = c
        .put(&url)
        .header("authorization", token(1, "upload", &hash))
        .body(bytes.clone())
        .send()
        .await?;
    assert_eq!(duplicate.status(), 200);
    let other = vec![9u8; 1];
    let other_hash = auth::hash(&other);
    let response = c
        .put(format!("http://127.0.0.1:19504/upload/{other_hash}"))
        .header("authorization", token(1, "upload", &other_hash))
        .body(other)
        .send()
        .await?;
    assert_eq!(response.status(), 503);
    assert!(
        repo.blob_intent(hex::decode(other_hash)?.try_into().unwrap())
            .await?
            .is_none()
    );
    drop(node);
    let _node = spawn()?;
    ready(&c).await?;
    let response = c
        .get(&get)
        .header("authorization", token(1, "get", &hash))
        .send()
        .await?;
    assert_eq!(response.status(), 200);
    assert_eq!(response.bytes().await?.as_ref(), bytes);
    let duplicate = c
        .put(&url)
        .header("authorization", token(1, "upload", &hash))
        .body(bytes.clone())
        .send()
        .await?;
    assert_eq!(duplicate.status(), 200);
    // Real process interruption after fsync but before the metadata commit.
    drop(_node);
    let mut expanded: Value = serde_json::from_slice(&std::fs::read(&config)?)?;
    expanded["storage"]["physical_bytes"] = json!(64);
    expanded["storage"]["staging_bytes"] = json!(64);
    std::fs::write(&config, serde_json::to_vec(&expanded)?)?;
    let interrupted_node = spawn()?;
    ready(&c).await?;
    let fault = repo.pool.get().await?;
    fault
        .execute("SELECT pg_advisory_lock(927441)", &[])
        .await?;
    fault.batch_execute("CREATE FUNCTION fixture_pause_blob() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(927441); RETURN NEW; END $$; CREATE TRIGGER fixture_pause_blob BEFORE UPDATE OF state ON blob_intents FOR EACH ROW EXECUTE FUNCTION fixture_pause_blob();").await?;
    let crash_bytes = vec![19u8; 16];
    let crash_hash = auth::hash(&crash_bytes);
    let pending = c
        .put(format!("http://127.0.0.1:19504/upload/{crash_hash}"))
        .header("authorization", token(1, "upload", &crash_hash))
        .body(crash_bytes.clone());
    let pending = tokio::spawn(async move { pending.send().await });
    let mut blocked_pid = None;
    for _ in 0..100 {
        blocked_pid=fault.query_opt("SELECT pid FROM pg_stat_activity WHERE datname=current_database() AND pg_backend_pid()=ANY(pg_blocking_pids(pid)) ORDER BY pid LIMIT 1",&[]).await?.map(|row|row.get::<_,i32>(0));
        if blocked_pid.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let blocked_pid = blocked_pid.context("upload never reached injected commit boundary")?;
    drop(interrupted_node);
    // Explicitly terminate the blocked fixture backend so rollback is observed
    // before restarting; no production connection is involved.
    fault
        .execute("SELECT pg_terminate_backend($1)", &[&blocked_pid])
        .await?;
    fault
        .execute("SELECT pg_advisory_unlock(927441)", &[])
        .await?;
    fault
        .batch_execute(
            "DROP TRIGGER fixture_pause_blob ON blob_intents; DROP FUNCTION fixture_pause_blob();",
        )
        .await?;
    let _ = pending.await;
    assert!(
        !repo
            .blob_intent(hex::decode(&crash_hash)?.try_into().unwrap())
            .await?
            .unwrap()
            .available
    );
    let _node = spawn()?;
    ready(&c).await?;
    let mut recovered = false;
    for _ in 0..100 {
        if repo
            .blob_intent(hex::decode(&crash_hash)?.try_into().unwrap())
            .await?
            .is_some_and(|b| b.available)
        {
            recovered = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(recovered, "restart must register durable uncommitted bytes");
    let response = c
        .get(format!("http://127.0.0.1:19504/blobs/{crash_hash}"))
        .header("authorization", token(1, "get", &crash_hash))
        .send()
        .await?;
    assert_eq!(response.status(), 200);
    assert_eq!(response.bytes().await?.as_ref(), crash_bytes);
    let caps: Value = c
        .get("http://127.0.0.1:19504/")
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(caps["features"]["upload"], true);
    assert_eq!(caps["features"]["publication"], false);
    std::fs::write(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../results/postgres-http-upload.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["missing actor/action scope denied before body","wrong hash and oversized body rejected","exact original bytes round trip","staged originals are not anonymous or available to another actor","duplicate succeeds at full quota","over-quota object leaves no intent","restart preserves original and accounting", "real process killed after fsync and before commit recovers verified bytes","capabilities distinguish uploads from publication"],"limitations":["loopback synthetic bytes only","one operator uploader","no publication/retention yet","not target-scale validation"]}),
        )?,
    )?;
    Ok(())
}
