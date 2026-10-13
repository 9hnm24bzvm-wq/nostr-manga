use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
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
fn token(body: &[u8]) -> String {
    let event = auth::sign(
        &format!("{:064x}", 1),
        27235,
        vec![
            vec!["u".into(), "http://127.0.0.1:19505/events".into()],
            vec!["method".into(), "POST".into()],
            vec!["payload".into(), auth::hash(body)],
            vec!["nonce".into(), hex::encode(rand::random::<[u8; 16]>())],
        ],
        String::new(),
    )
    .unwrap();
    format!(
        "Nostr {}",
        STANDARD.encode(serde_json::to_vec(&event).unwrap())
    )
}
async fn submit(c: &reqwest::Client, event: &auth::Event) -> Result<reqwest::Response> {
    let body = serde_json::to_vec(event)?;
    Ok(c.post("http://127.0.0.1:19505/events")
        .header("authorization", token(&body))
        .body(body)
        .send()
        .await?)
}
async fn wait_event(c: &reqwest::Client, id: &str) -> Result<auth::Event> {
    for _ in 0..150 {
        let r = c
            .get(format!("http://127.0.0.1:19505/events/{id}"))
            .send()
            .await?;
        if r.status() == 200 {
            let event: auth::Event = r.json().await?;
            event.verify().map_err(anyhow::Error::msg)?;
            return Ok(event);
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    anyhow::bail!("event remained pending")
}
async fn unique_history(expected: &auth::Event) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(8), async {
        let url = "ws://127.0.0.1:19505/";
        let (mut ws, _) = tokio_tungstenite::connect_async(url).await?;
        let challenge: Value =
            serde_json::from_str(&ws.next().await.context("AUTH challenge")??.into_text()?)?;
        let proof = auth::sign(
            &format!("{:064x}", 1),
            22242,
            vec![
                vec!["relay".into(), url.into()],
                vec![
                    "challenge".into(),
                    challenge[1].as_str().context("challenge")?.into(),
                ],
            ],
            String::new(),
        )
        .map_err(anyhow::Error::msg)?;
        ws.send(tokio_tungstenite::tungstenite::Message::Text(
            json!(["AUTH", proof]).to_string().into(),
        ))
        .await?;
        let ack: Value = serde_json::from_str(
            &ws.next()
                .await
                .context("AUTH acknowledgment")??
                .into_text()?,
        )?;
        assert_eq!(ack[2], true, "{ack}");
        ws.send(tokio_tungstenite::tungstenite::Message::Text(
            json!(["REQ","exact",{"ids":[expected.id]}])
                .to_string()
                .into(),
        ))
        .await?;
        let mut received = Vec::new();
        loop {
            let frame: Value = serde_json::from_str(
                &ws.next()
                    .await
                    .context("history before EOSE")??
                    .into_text()?,
            )?;
            match frame[0].as_str() {
                Some("EVENT") => {
                    let event: auth::Event = serde_json::from_value(frame[2].clone())?;
                    event.verify().map_err(anyhow::Error::msg)?;
                    received.push(event.id);
                }
                Some("EOSE") => break,
                _ => anyhow::bail!("unexpected history frame: {frame}"),
            }
        }
        assert_eq!(received, vec![expected.id.clone()]);
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires real strfry and isolated PostgreSQL; run scripts/test-postgres.sh"]
async fn durable_real_strfry_projection() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let parsed: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(
        parsed.get_dbname() == Some("manga_test"),
        "fixture database required"
    );
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
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let binary = std::env::var_os("STRFRY_BIN")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("vendor/strfry/strfry"));
    anyhow::ensure!(binary.is_file(), "real installed STRFRY_BIN required");
    let fail = dir.path().join("fail");
    std::fs::write(&fail, b"synthetic backend interruption")?;
    let wrapper = dir.path().join("strfry-fixture.sh");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nif [ -f '{}' ]; then exit 7; fi\nexec '{}' \"$@\"\n",
            fail.display(),
            binary.display()
        ),
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700))?;
    }
    let config = dir.path().join("node.json");
    let backend = std::env::var_os("MANGA_TEST_BACKEND_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| dir.path().join("backend"));
    std::fs::write(
        &config,
        serde_json::to_vec(
            &json!({"bind":"127.0.0.1:19505","base_url":"http://127.0.0.1:19505","service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"backend":{"binary":wrapper,"data_dir":backend,"timeout_seconds":2,"map_bytes":67108864}}),
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
                .env("RUST_LOG", "warn")
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        ))
    };
    let node = spawn()?;
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()?;
    let mut ready = false;
    for _ in 0..60 {
        if c.get("http://127.0.0.1:19505/").send().await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        ready,
        "{}",
        std::fs::read_to_string(dir.path().join("node.log")).unwrap_or_default()
    );
    let event = auth::sign(
        &format!("{:064x}", 1),
        1,
        vec![],
        "recoverable real strfry projection".into(),
    )
    .unwrap();
    let response = submit(&c, &event).await?;
    assert_eq!(response.status(), 202);
    let pending: Value = response.json().await?;
    assert_eq!(pending["projected"], false);
    let r = c
        .get(format!("http://127.0.0.1:19505/events/{}", event.id))
        .send()
        .await?;
    assert_eq!(r.status(), 202);
    // Accepted work survives a process restart while the real backend is unavailable.
    drop(node);
    let _node = spawn()?;
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if let Ok(r) = c
                .get(format!("http://127.0.0.1:19505/events/{}", event.id))
                .send()
                .await
            {
                assert_eq!(r.status(), 202);
                break;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await?;
    std::fs::remove_file(&fail)?;
    assert_eq!(wait_event(&c, &event.id).await?.id, event.id);
    let scan = Command::new(&binary)
        .arg("--config")
        .arg(backend.join("strfry.conf"))
        .arg("scan")
        .arg(json!({"ids":[event.id]}).to_string())
        .output()?;
    assert!(scan.status.success());
    let stored: auth::Event = serde_json::from_slice(&scan.stdout)?;
    assert_eq!(stored.id, event.id);
    stored.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(submit(&c, &event).await?.status(), 200);
    unique_history(&event).await?;
    let deletion = auth::sign(
        &format!("{:064x}", 1),
        5,
        vec![vec!["e".into(), event.id.clone()]],
        "request".into(),
    )
    .unwrap();
    assert_eq!(submit(&c, &deletion).await?.status(), 400);
    let replacement = auth::sign(&format!("{:064x}", 1), 0, vec![], "{}".into()).unwrap();
    assert_eq!(submit(&c, &replacement).await?.status(), 400);
    let expiring = auth::sign(
        &format!("{:064x}", 1),
        1,
        vec![vec!["expiration".into(), (auth::now() + 60).to_string()]],
        "expiry needs lifecycle".into(),
    )
    .unwrap();
    assert_eq!(submit(&c, &expiring).await?.status(), 400);
    let mut tampered = event.clone();
    tampered.content.push('!');
    assert_eq!(submit(&c, &tampered).await?.status(), 400);
    assert_eq!(wait_event(&c, &event.id).await?.id, event.id);
    // Distinct valid HTTP authorization events race the same immutable event.
    let duplicate = auth::sign(
        &format!("{:064x}", 1),
        7,
        vec![vec!["e".into(), event.id.clone()]],
        "+".into(),
    )
    .unwrap();
    let (a, b) = tokio::join!(submit(&c, &duplicate), submit(&c, &duplicate));
    assert!(a?.status().is_success());
    assert!(b?.status().is_success());
    wait_event(&c, &duplicate.id).await?;
    unique_history(&duplicate).await?;
    // Sequence allocation is not commit order. Keep an earlier outbox row
    // uncommitted while a later event projects, then ensure the earlier row isn't skipped.
    let late = auth::sign(
        &format!("{:064x}", 1),
        1,
        vec![],
        "earlier sequence, later commit".into(),
    )
    .unwrap();
    let mut held = repo.pool.get().await?;
    let tx = held.transaction().await?;
    let late_id = hex::decode(&late.id)?;
    let author = hex::decode(&late.pubkey)?;
    tx.execute("INSERT INTO events(id,publisher,kind,created_at,raw_event,state) VALUES($1,$2,1,$3,$4,'hosted')", &[&late_id,&author,&(late.created_at as i64),&serde_json::to_value(&late)?]).await?;
    tx.execute(
        "INSERT INTO backend_outbox(event_id,action,payload) VALUES($1,'import',$2)",
        &[&late_id, &serde_json::to_value(&late)?],
    )
    .await?;
    let early = auth::sign(
        &format!("{:064x}", 1),
        1,
        vec![],
        "later sequence, earlier commit".into(),
    )
    .unwrap();
    assert!(submit(&c, &early).await?.status().is_success());
    wait_event(&c, &early.id).await?;
    tx.commit().await?;
    wait_event(&c, &late.id).await?;
    // A new empty backend directory must not inherit completed projections.
    let mut wrong: Value = serde_json::from_slice(&std::fs::read(&config)?)?;
    wrong["bind"] = json!("127.0.0.1:19506");
    wrong["backend"]["data_dir"] = json!(dir.path().join("different-backend"));
    let wrong_path = dir.path().join("wrong.json");
    std::fs::write(&wrong_path, serde_json::to_vec(&wrong)?)?;
    let rejected = tokio::process::Command::new(env!("CARGO_BIN_EXE_manga-node"))
        .args(["serve", wrong_path.to_str().unwrap()])
        .env("MANGA_DATABASE_URL", &dsn)
        .output()
        .await?;
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("another backend target"));
    std::fs::write(
        root.join("results/postgres-http-backend.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["backend failure returns explicit 202 pending across process restart","real strfry recovers and scan verifies signed stored event before acknowledgement","duplicate and competing submissions expose exactly one signed Nostr history event","invalid signatures rejected before persistence", "different empty backend target cannot inherit completed projection state", "out-of-order transaction commits do not skip lower sequence outbox rows","kind5 deletion, replaceable and expiring events never enter immutable import path"],"limits":["one projection target per instance database","only regular immutable kinds1,7,1111","no publication/moderation projection yet","no throughput claim"]}),
        )?,
    )?;
    Ok(())
}
