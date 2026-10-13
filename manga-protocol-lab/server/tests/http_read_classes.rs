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
async fn read_classes_isolate_policy_contention() -> Result<()> {
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
            &json!({"bind":bind.to_string(),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"read_policy":"admitted","database_plaintext_local":true,"sync_worker_enabled":false,"group_keys_dir":dir.path().join("groups"),"storage":{"path":dir.path().join("objects"),"volume":"groupprofile","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}}),
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
    assert_eq!(
        post_json(
            &c,
            1,
            &format!("{origin}/admin/accounts/{}", public(2)),
            &json!({"allowed":true})
        )
        .await?
        .status(),
        200
    );
    let ws_url = format!("ws://{bind}/");
    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await?;
    let challenge: Value = serde_json::from_str(&ws.next().await.unwrap()?.into_text()?)?;
    let proof = auth::sign(
        &format!("{:064x}", 2),
        22242,
        vec![
            vec!["relay".into(), ws_url],
            vec!["challenge".into(), challenge[1].as_str().unwrap().into()],
        ],
        String::new(),
    )
    .unwrap();
    let mut locker = repo.pool.get().await?;
    let tx = locker.transaction().await?;
    tx.batch_execute("LOCK TABLE account_permissions IN ACCESS EXCLUSIVE MODE")
        .await?;
    let mut pending = tokio::task::JoinSet::new();
    for _ in 0..4 {
        let client = c.clone();
        let url = format!("{origin}/query?order=asc");
        pending.spawn(async move {
            client
                .get(&url)
                .header("authorization", http_get(2, &url))
                .send()
                .await
        });
    }
    // Observe the externally held policy barrier, never admission internals.
    tokio::time::timeout(Duration::from_secs(2),async {
        loop {
            let blocked:i64=repo.pool.get().await?.query_one("SELECT count(*) FROM pg_locks WHERE relation='account_permissions'::regclass AND NOT granted",&[]).await?.get(0);
            if blocked>=2 {break Ok::<_,anyhow::Error>(());}
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await??;
    tokio::time::sleep(Duration::from_millis(20)).await;
    let head = c
        .get(format!("{origin}/libraries/{}", public(2)))
        .send()
        .await?;
    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        json!(["AUTH", proof]).to_string().into(),
    ))
    .await?;
    let auth_reply: Value = serde_json::from_str(
        &tokio::time::timeout(Duration::from_secs(1), ws.next())
            .await?
            .unwrap()?
            .into_text()?,
    )?;
    tx.rollback().await?;
    assert_eq!(
        head.status(),
        403,
        "catalogue policy contention must not consume library-head admission before authentication"
    );
    assert_eq!(
        auth_reply[2], true,
        "catalogue contention must not consume live AUTH admission: {auth_reply}"
    );
    while let Some(result) = pending.join_next().await {
        assert_eq!(
            result??.status(),
            200,
            "short catalogue burst must wait and succeed after policy unlock"
        );
    }
    // Two active reads plus eight queued reads are bounded; the eleventh is
    // rejected immediately, while releasing the short external stall drains ten.
    let tx = locker.transaction().await?;
    tx.batch_execute("LOCK TABLE account_permissions IN ACCESS EXCLUSIVE MODE")
        .await?;
    let mut burst = tokio::task::JoinSet::new();
    for _ in 0..11 {
        let client = c.clone();
        let url = format!("{origin}/query?order=desc");
        burst.spawn(async move {
            client
                .get(&url)
                .header("authorization", http_get(2, &url))
                .send()
                .await
        });
    }
    let overflow = tokio::time::timeout(Duration::from_millis(150), burst.join_next())
        .await?
        .unwrap()??;
    assert_eq!(overflow.status(), 429);
    assert_eq!(overflow.headers()["retry-after"], "1");
    tx.rollback().await?;
    let mut accepted = 0;
    while let Some(result) = burst.join_next().await {
        assert_eq!(result??.status(), 200);
        accepted += 1;
    }
    assert_eq!(accepted, 10);
    Ok(())
}
