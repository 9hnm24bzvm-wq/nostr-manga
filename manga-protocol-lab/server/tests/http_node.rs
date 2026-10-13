use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::Path,
    process::{Child, Command},
    time::Duration,
};
struct Children(Vec<Child>);
impl Drop for Children {
    fn drop(&mut self) {
        for child in &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
fn key_file(path: &Path, key: u8) -> Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    write!(file, "{key:064x}")?;
    Ok(())
}
fn public(key: u8) -> String {
    auth::sign(&format!("{key:064x}"), 1, vec![], String::new())
        .unwrap()
        .pubkey
}
fn token(key: u8, url: &str, method: &str) -> String {
    let e = auth::sign(
        &format!("{key:064x}"),
        27235,
        vec![
            vec!["u".into(), url.into()],
            vec!["method".into(), method.into()],
            vec!["nonce".into(), hex::encode(rand::random::<[u8; 16]>())],
        ],
        String::new(),
    )
    .unwrap();
    format!("Nostr {}", STANDARD.encode(serde_json::to_vec(&e).unwrap()))
}
fn body_token(key: u8, url: &str, body: &[u8]) -> String {
    let e = auth::sign(
        &format!("{key:064x}"),
        27235,
        vec![
            vec!["u".into(), url.into()],
            vec!["method".into(), "POST".into()],
            vec!["payload".into(), auth::hash(body)],
            vec!["nonce".into(), hex::encode(rand::random::<[u8; 16]>())],
        ],
        String::new(),
    )
    .unwrap();
    format!("Nostr {}", STANDARD.encode(serde_json::to_vec(&e).unwrap()))
}
fn expired_token(url: &str) -> String {
    use secp256k1::{Keypair, Message, Secp256k1, SecretKey};
    let secret = format!("{:064x}", 1);
    let mut e = auth::sign(
        &secret,
        27235,
        vec![
            vec!["u".into(), url.into()],
            vec!["method".into(), "GET".into()],
        ],
        String::new(),
    )
    .unwrap();
    e.created_at = auth::now() - 120;
    let digest = e.digest();
    e.id = hex::encode(&digest);
    let secp = Secp256k1::new();
    let keypair = Keypair::from_secret_key(
        &secp,
        &SecretKey::from_slice(&hex::decode(secret).unwrap()).unwrap(),
    );
    e.sig = secp
        .sign_schnorr_no_aux_rand(&Message::from_digest_slice(&digest).unwrap(), &keypair)
        .to_string();
    e.verify().unwrap();
    format!("Nostr {}", STANDARD.encode(serde_json::to_vec(&e).unwrap()))
}
async fn ready(client: &reqwest::Client, url: &str) -> Result<Value> {
    for _ in 0..60 {
        if let Ok(r) = client.get(url).send().await
            && r.status().is_success()
        {
            return Ok(r.json().await?);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    anyhow::bail!("HTTP node did not become ready")
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run scripts/test-postgres.sh"]
async fn authenticated_node_boot() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("ephemeral database required")?;
    let parsed: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(
        parsed.get_dbname() == Some("manga_test"),
        "fixture database required"
    );
    let repo = Repository::connect(DbConfig {
        dsn: dsn.clone(),
        connections: 2,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1_048_576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let dir = tempfile::tempdir()?;
    let key = dir.path().join("service.key");
    key_file(&key, 11)?;
    let binary = env!("CARGO_BIN_EXE_manga-node");
    let mut children = Children(vec![]);
    let mut configs = vec![];
    for port in [19501, 19502] {
        let config = json!({"bind":format!("127.0.0.1:{port}"),"base_url":"http://127.0.0.1:19501","service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"database_connections":3});
        let path = dir.path().join(format!("node-{port}.json"));
        std::fs::write(&path, serde_json::to_vec(&config)?)?;
        let log = std::fs::File::create(dir.path().join(format!("node-{port}.log")))?;
        children.0.push(
            Command::new(binary)
                .args(["serve", path.to_str().unwrap()])
                .env("MANGA_DATABASE_URL", &dsn)
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        );
        configs.push(config);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()?;
    let caps = ready(&client, "http://127.0.0.1:19501/").await?;
    ready(&client, "http://127.0.0.1:19502/").await?;
    assert_eq!(caps["service_pubkey"], public(11));
    assert_eq!(caps["features"]["publication"], false);
    let mut checks = vec![
        "two actual HTTP processes bind the same persistent service/admin identity and advertise only implemented features",
    ];
    let nonce = hex::encode([22u8; 32]);
    let proof: auth::Event = client
        .get(format!("http://127.0.0.1:19501/identity?nonce={nonce}"))
        .send()
        .await?
        .json()
        .await?;
    proof.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(proof.pubkey, public(11));
    assert_eq!(proof.kind, 20005);
    assert_eq!(proof.one("nonce").unwrap(), nonce);
    assert_eq!(proof.one("endpoint").unwrap(), "http://127.0.0.1:19501");
    assert_eq!(
        client
            .get("http://127.0.0.1:19501/identity?nonce=bad")
            .send()
            .await?
            .status(),
        400
    );
    checks.push("identity proof verifies the nonce, endpoint and expected service key; malformed nonces reject");
    let url = "http://127.0.0.1:19501/admin/status";
    assert_eq!(client.get(url).send().await?.status(), 403);
    assert_eq!(
        client
            .get(url)
            .header("authorization", token(2, url, "GET"))
            .send()
            .await?
            .status(),
        403
    );
    assert_eq!(
        client
            .get(url)
            .header(
                "authorization",
                token(1, "http://127.0.0.1:19501/admin/status?different=1", "GET")
            )
            .send()
            .await?
            .status(),
        403
    );
    assert_eq!(
        client
            .get(url)
            .header("authorization", token(1, url, "POST"))
            .send()
            .await?
            .status(),
        403
    );
    checks.push(
        "missing, wrong-actor, wrong-URL and wrong-method operator authorization rejects over HTTP",
    );
    assert_eq!(
        client
            .get(url)
            .header("authorization", expired_token(url))
            .send()
            .await?
            .status(),
        403
    );
    let body_url = "http://127.0.0.1:19501/admin/check";
    let body = b"signed body fixture";
    let body_auth = body_token(1, body_url, body);
    assert_eq!(
        client
            .post(body_url)
            .header("authorization", &body_auth)
            .body("different body")
            .send()
            .await?
            .status(),
        403
    );
    assert_eq!(
        client
            .post(body_url)
            .header("authorization", &body_auth)
            .body(body.to_vec())
            .send()
            .await?
            .status(),
        200
    );
    assert_eq!(
        client
            .post(body_url)
            .header("authorization", &body_auth)
            .body(body.to_vec())
            .send()
            .await?
            .status(),
        403
    );
    {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = tokio::net::TcpStream::connect("127.0.0.1:19501").await?;
        stream.write_all(b"POST /admin/check HTTP/1.1\r\nHost: 127.0.0.1:19501\r\nContent-Length: 1000000\r\nConnection: close\r\n\r\n").await?;
        let mut buffer = [0; 1024];
        let n = tokio::time::timeout(Duration::from_secs(1), stream.read(&mut buffer)).await??;
        assert!(String::from_utf8_lossy(&buffer[..n]).starts_with("HTTP/1.1 403"));
    }
    checks.push("validly signed stale tokens and body mismatches reject; unauthorized headers receive rejection without sending a body");
    let auth = token(1, url, "GET");
    let first = client
        .get(url)
        .header("authorization", &auth)
        .send()
        .await?;
    assert_eq!(first.status(), 200);
    let status = first.json::<Value>().await?;
    assert_eq!(status["database"], "ready");
    assert_eq!(status["background_workers"]["unexpected_exit"], false);
    let roles = status["background_workers"]["workers"].as_array().unwrap();
    assert!(
        roles
            .iter()
            .any(|r| r["name"] == "authorization_expiry" && r["state"] == "running")
    );
    assert!(roles.iter().any(|r| r["name"] == "relay_notifications"));
    assert!(!roles.iter().any(|r| r["name"] == "blob_reclamation"));
    // Both private replicas represent the same configured public origin, as behind a proxy.
    assert_eq!(
        client
            .get("http://127.0.0.1:19502/admin/status")
            .header("authorization", &auth)
            .send()
            .await?
            .status(),
        403
    );
    let race = token(1, url, "GET");
    let (a, b) = tokio::join!(
        client.get(url).header("authorization", &race).send(),
        client
            .get("http://127.0.0.1:19502/admin/status")
            .header("authorization", &race)
            .send()
    );
    let mut codes = vec![a?.status().as_u16(), b?.status().as_u16()];
    codes.sort();
    assert_eq!(codes, vec![200, 403]);
    checks.push(
        "authorization replay is rejected across processes, including concurrent use of one token",
    );
    let wrong_key = dir.path().join("wrong.key");
    key_file(&wrong_key, 12)?;
    let mut wrong = configs[0].clone();
    wrong["bind"] = json!("127.0.0.1:19503");
    wrong["service_key_file"] = json!(wrong_key);
    let wrong_path = dir.path().join("wrong.json");
    std::fs::write(&wrong_path, serde_json::to_vec(&wrong)?)?;
    let failed = Command::new(binary)
        .args(["serve", wrong_path.to_str().unwrap()])
        .env("MANGA_DATABASE_URL", &dsn)
        .output()?;
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("different service/admin identity"));
    checks.push("a changed service key cannot silently reuse an existing instance database");
    // An incompatible schema must stop a new process, not be rewritten implicitly.
    let c = repo.pool.get().await?;
    c.execute(
        "INSERT INTO schema_migrations(version,checksum) VALUES(999,$1)",
        &[&vec![0u8; 32]],
    )
    .await?;
    let failed = Command::new(binary)
        .args(["serve", wrong_path.to_str().unwrap()])
        .env("MANGA_DATABASE_URL", &dsn)
        .output()?;
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("schema version"));
    c.execute("DELETE FROM schema_migrations WHERE version=999", &[])
        .await?;
    drop(c);
    checks.push("newer schema prevents startup without implicit migration or data rewrite");
    let c = repo.pool.get().await?;
    let old_checksum: Vec<u8> = c
        .query_one(
            "SELECT checksum FROM schema_migrations WHERE version=1",
            &[],
        )
        .await?
        .get(0);
    c.execute(
        "UPDATE schema_migrations SET checksum=$1 WHERE version=1",
        &[&vec![0u8; 32]],
    )
    .await?;
    let failed = Command::new(binary)
        .args(["serve", wrong_path.to_str().unwrap()])
        .env("MANGA_DATABASE_URL", &dsn)
        .output()?;
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("checksum"));
    c.execute(
        "UPDATE schema_migrations SET checksum=$1 WHERE version=1",
        &[&old_checksum],
    )
    .await?;
    c.batch_execute("ALTER TABLE schema_migrations RENAME TO fixture_hidden_migrations")
        .await?;
    let failed = Command::new(binary)
        .args(["serve", wrong_path.to_str().unwrap()])
        .env("MANGA_DATABASE_URL", &dsn)
        .output()?;
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("migrations are required"));
    c.batch_execute("ALTER TABLE fixture_hidden_migrations RENAME TO schema_migrations")
        .await?;
    drop(c);
    checks.push(
        "missing migration metadata or mismatched checksums prevents startup without silent repair",
    );
    let report = json!({"scope":"two private HTTP processes with real PostgreSQL; public synthetic signing keys; identity proof is not TLS-channel-bound","checks":checks,"count":checks.len()});
    if let Ok(path) = std::env::var("MANGA_HTTP_NODE_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    }
    println!("{report}");
    Ok(())
}
