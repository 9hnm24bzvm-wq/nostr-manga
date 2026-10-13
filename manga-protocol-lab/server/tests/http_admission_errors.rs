mod support;
use anyhow::{Result, ensure};
use futures_util::{SinkExt, StreamExt};
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
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "real PostgreSQL and strfry; database failure classification"]
async fn database_faults_are_retryable_not_role_denials() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    ensure!(
        dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test"),
        "fixture only"
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
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = l.local_addr()?;
    drop(l);
    let origin = format!("http://{bind}");
    let key = dir.path().join("service.key");
    write_key(&key, 11)?;
    let cfg = dir.path().join("node.json");
    std::fs::write(
        &cfg,
        serde_json::to_vec(
            &json!({"bind":bind.to_string(),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"read_policy":"admitted","read_admission":{"overall":4,"wait_ms":200,"catalogue":{"active":4,"waiting":0},"library_head":{"active":4,"waiting":0},"live":{"active":4,"waiting":0}},"database_plaintext_local":true,"sync_worker_enabled":false,"group_keys_dir":dir.path().join("groups"),"storage":{"path":dir.path().join("objects"),"volume":"groupprofile","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}}),
        )?,
    )?;
    let log = std::fs::File::create(dir.path().join("node.log"))?;
    let _node = Node(
        Command::new(
            std::env::var_os("MANGA_TEST_NODE_BIN")
                .unwrap_or_else(|| env!("CARGO_BIN_EXE_manga-node").into()),
        )
        .args(["serve", cfg.to_str().unwrap()])
        .env("MANGA_DATABASE_URL", &dsn)
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?,
    );
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let mut ready = false;
    for _ in 0..100 {
        if c.get(&origin)
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
        {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    ensure!(
        ready,
        "node failed: {}",
        std::fs::read_to_string(dir.path().join("node.log"))?
    );
    if std::env::var("MANGA_ADMISSION_CASE").as_deref() == Ok("cleanup") {
        let url = format!("{origin}/admin/status");
        let response = c
            .get(&url)
            .header("authorization", http_get(1, &url))
            .send()
            .await?;
        assert_eq!(response.status(), 200);
        let status: Value = response.json().await?;
        let cleanup = status
            .get("cleanup")
            .expect("operator cleanup observability is required");
        assert!(
            cleanup.get("oldest_due_seconds").is_some(),
            "explicit oldest-due metric required"
        );
        assert!(
            cleanup["oldest_due_seconds"].is_null(),
            "empty queue has no oldest due item"
        );
        return Ok(());
    }
    let r = post_json(
        &c,
        1,
        &format!("{origin}/compat/mangadex/group"),
        &json!({"administrator":public(2)}),
    )
    .await?;
    ensure!(r.status().is_success(), "create {}", r.text().await?);
    let group = r.json::<Value>().await?["data"]["nostr"]["group"]
        .as_str()
        .unwrap()
        .to_owned();
    let author: [u8; 32] = hex::decode(public(2))?.try_into().unwrap();
    repo.set_account_admission(author, true).await?;
    let account_url = format!("{origin}/account-metadata");
    let first = sign_at(
        2,
        0,
        vec![],
        json!({"name":"baseline"}).to_string(),
        auth::now(),
    );
    let response = post_json(&c, 2, &account_url, &serde_json::to_value(&first)?).await?;
    ensure!(
        response.status().is_success(),
        "initial metadata {}",
        response.text().await?
    );
    let unknown = sign_at(3, 0, vec![], "{}".into(), auth::now());
    assert_eq!(
        post_json(&c, 3, &account_url, &serde_json::to_value(unknown)?)
            .await?
            .status(),
        403
    );
    if std::env::var("MANGA_ADMISSION_CASE").as_deref() != Ok("outage") {
        let nostr = std::env::var("MANGA_ADMISSION_CASE")
            .unwrap_or_default()
            .starts_with("nostr");
        let nostr_auth = std::env::var("MANGA_ADMISSION_CASE").as_deref() == Ok("nostr_auth");
        let mut ws = if nostr {
            let url = format!("ws://{bind}/");
            let (mut ws, _) = tokio_tungstenite::connect_async(&url).await?;
            let challenge: Value = serde_json::from_str(&ws.next().await.unwrap()?.into_text()?)?;
            let event = auth::sign(
                &format!("{:064x}", 2),
                22242,
                vec![
                    vec!["relay".into(), url],
                    vec!["challenge".into(), challenge[1].as_str().unwrap().into()],
                ],
                String::new(),
            )
            .unwrap();
            ws.send(tokio_tungstenite::tungstenite::Message::Text(
                json!(["AUTH", event]).to_string().into(),
            ))
            .await?;
            let reply: Value = serde_json::from_str(&ws.next().await.unwrap()?.into_text()?)?;
            assert_eq!(reply[2], true, "baseline authentication");
            Some((ws, event))
        } else {
            None
        };
        // Hold policy storage externally: overflow must fail before touching that DB.
        let mut locker = repo.pool.get().await?;
        let transaction = locker.transaction().await?;
        transaction
            .batch_execute("LOCK TABLE account_permissions IN ACCESS EXCLUSIVE MODE")
            .await?;
        let mut pending = tokio::task::JoinSet::new();
        for _ in 0..8 {
            let client = c.clone();
            let url = format!("{origin}/query?order=asc");
            pending.spawn(async move {
                client
                    .get(&url)
                    .header("authorization", http_get(2, &url))
                    .send()
                    .await
                    .map(|r| r.status())
            });
        }
        let mut early = Vec::new();
        for _ in 0..4 {
            match tokio::time::timeout(Duration::from_millis(400), pending.join_next()).await {
                Ok(Some(result)) => early.push(result??),
                _ => break,
            }
        }
        let nostr_reply = if let Some((ws, event)) = ws.as_mut() {
            let command = if nostr_auth {
                json!(["AUTH", event])
            } else {
                json!(["REQ", "blocked", {"limit": 0}])
            };
            ws.send(tokio_tungstenite::tungstenite::Message::Text(
                command.to_string().into(),
            ))
            .await?;
            tokio::time::timeout(Duration::from_millis(400), ws.next())
                .await
                .ok()
                .flatten()
        } else {
            None
        };
        transaction.rollback().await?;
        if nostr {
            let reply = nostr_reply
                .expect("established Nostr read operation must reject before blocked policy DB")?;
            let reply: Value = serde_json::from_str(&reply.into_text()?)?;
            if nostr_auth {
                assert_eq!(reply[0], "OK");
                assert_eq!(reply[2], false);
                assert!(
                    reply[3].as_str().unwrap().contains("rate-limited"),
                    "Nostr AUTH gate: {reply}"
                );
            } else {
                assert_eq!(reply[0], "CLOSED");
                assert!(
                    reply[2].as_str().unwrap().contains("rate-limited"),
                    "Nostr read gate: {reply}"
                );
            }
        }
        drop(locker);
        assert_eq!(
            early.len(),
            4,
            "four excess reads must reject before blocked policy lookup"
        );
        assert!(
            early.iter().all(|status| *status == 429),
            "overflow must be retryable429: {early:?}"
        );
        while let Some(result) = pending.join_next().await {
            assert_eq!(
                result??, 200,
                "admitted read must acquire only one query permit"
            );
        }
    }
    let second = sign_at(
        2,
        0,
        vec![],
        json!({"name":"retry"}).to_string(),
        first.created_at + 1,
    );
    let query = format!("{origin}/query?order=asc");
    repo.pool
        .get()
        .await?
        .batch_execute("ALTER TABLE account_permissions RENAME TO fixture_unavailable_accounts")
        .await?;
    let read = c
        .get(&query)
        .header("authorization", http_get(2, &query))
        .send()
        .await?;
    let blossom_url = format!("{origin}/{}", "ab".repeat(32));
    let blossom_read = c
        .get(&blossom_url)
        .header(
            "authorization",
            bud(2, Some("127.0.0.1"), "get", Some(&"ab".repeat(32))),
        )
        .send()
        .await?;
    let write = post_json(&c, 2, &account_url, &serde_json::to_value(&second)?).await?;
    // Restore before assertions so a failing diagnostic retains a usable fixture.
    repo.pool
        .get()
        .await?
        .batch_execute("ALTER TABLE fixture_unavailable_accounts RENAME TO account_permissions")
        .await?;
    assert_eq!(
        blossom_read.status(),
        503,
        "admitted Blossom database outage is retryable"
    );
    assert_eq!(
        read.status(),
        503,
        "read admission store outage is not a forbidden reader"
    );
    assert_eq!(
        write.status(),
        503,
        "write admission store outage is not a forbidden author"
    );
    let response = post_json(&c, 2, &account_url, &serde_json::to_value(&second)?).await?;
    ensure!(
        response.status().is_success(),
        "retry after permission-store recovery {}",
        response.text().await?
    );
    let third = sign_at(
        2,
        0,
        vec![],
        json!({"name":"after transaction fault"}).to_string(),
        second.created_at + 1,
    );
    repo.pool
        .get()
        .await?
        .batch_execute("ALTER TABLE account_heads RENAME TO fixture_unavailable_heads")
        .await?;
    let write = post_json(&c, 2, &account_url, &serde_json::to_value(&third)?).await?;
    repo.pool
        .get()
        .await?
        .batch_execute("ALTER TABLE fixture_unavailable_heads RENAME TO account_heads")
        .await?;
    assert_eq!(
        write.status(),
        503,
        "transaction database error is not invalid user metadata"
    );
    assert!(
        post_json(&c, 2, &account_url, &serde_json::to_value(&third)?)
            .await?
            .status()
            .is_success()
    );
    let members = format!("{origin}/groups/{group}/members");
    repo.pool
        .get()
        .await?
        .batch_execute("ALTER TABLE managed_groups RENAME TO fixture_unavailable_groups")
        .await?;
    let response = c
        .get(&members)
        .header("authorization", http_get(2, &members))
        .send()
        .await?;
    repo.pool
        .get()
        .await?
        .batch_execute("ALTER TABLE fixture_unavailable_groups RENAME TO managed_groups")
        .await?;
    assert_eq!(
        response.status(),
        503,
        "group role store outage is retryable"
    );
    assert_eq!(
        c.get(&members)
            .header("authorization", http_get(2, &members))
            .send()
            .await?
            .status(),
        200
    );
    let peer: [u8; 32] = hex::decode(public(12))?.try_into().unwrap();
    repo.set_peer(peer, &origin, true).await?;
    let fetch = format!("{origin}/federation/account-events/{}", "ff".repeat(32));
    repo.pool
        .get()
        .await?
        .batch_execute("ALTER TABLE approved_peers RENAME TO fixture_unavailable_peers")
        .await?;
    let response = c
        .get(&fetch)
        .header("authorization", http_get(12, &fetch))
        .send()
        .await?;
    repo.pool
        .get()
        .await?
        .batch_execute("ALTER TABLE fixture_unavailable_peers RENAME TO approved_peers")
        .await?;
    assert_eq!(
        response.status(),
        503,
        "peer role store outage is retryable"
    );
    let response = c
        .get(&fetch)
        .header("authorization", http_get(12, &fetch))
        .send()
        .await?;
    assert_eq!(
        response.status(),
        404,
        "healthy approved peer reaches missing-resource check"
    );
    repo.set_peer(peer, &origin, false).await?;
    assert_eq!(
        c.get(&fetch)
            .header("authorization", http_get(12, &fetch))
            .send()
            .await?
            .status(),
        403
    );
    repo.set_account_admission(author, false).await?;
    assert_eq!(
        post_json(&c, 2, &account_url, &serde_json::to_value(third)?)
            .await?
            .status(),
        403
    );
    std::fs::write(
        root.join("results/rebuild-admission/admission-errors.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["unknown and revoked accounts remain403","account read/write permission database failures are503","metadata transaction database failure is503","fresh authorization retries succeed after database recovery","group role database failure is503","peer role database failure is503 but actual revocation is403"],"fault_injection":"temporary table renames in isolated fixture, restored before assertions"}),
        )?,
    )?;
    Ok(())
}

fn bud(key: u8, server: Option<&str>, action: &str, hash: Option<&str>) -> String {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let mut tags = vec![
        vec!["t".into(), action.into()],
        vec!["expiration".into(), (auth::now() + 120).to_string()],
    ];
    if let Some(server) = server {
        tags.push(vec!["server".into(), server.into()]);
    }
    if let Some(hash) = hash {
        tags.push(vec!["x".into(), hash.into()]);
    }
    format!(
        "Nostr {}",
        URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&sign_at(
                key,
                24242,
                tags,
                "Authorize fixture blob action".into(),
                auth::now() - 1
            ))
            .unwrap()
        )
    )
}
