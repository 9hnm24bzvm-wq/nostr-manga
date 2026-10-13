mod support;
use anyhow::{Context, Result, ensure};
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
async fn live_fanout_survives_transient_read_pressure() -> Result<()> {
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
            &json!({"bind":bind.to_string(),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"read_policy":"admitted","read_admission":{"overall":2,"wait_ms":100,"catalogue":{"active":2,"waiting":8},"library_head":{"active":2,"waiting":8},"live":{"active":2,"waiting":8}},"database_plaintext_local":true,"sync_worker_enabled":false,"group_keys_dir":dir.path().join("groups"),"storage":{"path":dir.path().join("objects"),"volume":"groupprofile","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}}),
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
    for who in [2, 3] {
        assert_eq!(
            post_json(
                &c,
                1,
                &format!("{origin}/admin/accounts/{}", public(who)),
                &json!({"allowed":true})
            )
            .await?
            .status(),
            200
        );
    }
    let status_url = format!("{origin}/admin/status");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let status: Value = c
                .get(&status_url)
                .header("authorization", http_get(1, &status_url))
                .send()
                .await?
                .json()
                .await?;
            if status["relay_feed_ready"] == true {
                break Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    let mut clients = Vec::new();
    for _ in 0..3 {
        let url = format!("ws://{bind}/");
        let (mut ws, _) = tokio_tungstenite::connect_async(&url).await?;
        let challenge: Value = serde_json::from_str(&ws.next().await.unwrap()?.into_text()?)?;
        let proof = auth::sign(
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
            json!(["AUTH", proof]).to_string().into(),
        ))
        .await?;
        let ack: Value = serde_json::from_str(&ws.next().await.unwrap()?.into_text()?)?;
        assert_eq!(ack[2], true);
        ws.send(tokio_tungstenite::tungstenite::Message::Text(
            json!(["REQ","live",{"kinds":[1],"limit":0}])
                .to_string()
                .into(),
        ))
        .await?;
        let eose: Value = serde_json::from_str(&ws.next().await.unwrap()?.into_text()?)?;
        assert_eq!(eose[0], "EOSE", "{eose}");
        clients.push(ws);
    }
    if std::env::var("MANGA_LIVE_CASE").as_deref() == Ok("policy_deadline") {
        let mut locker = repo.pool.get().await?;
        let tx = locker.transaction().await?;
        tx.batch_execute("LOCK TABLE account_permissions IN ACCESS EXCLUSIVE MODE")
            .await?;
        let event = auth::sign(
            &format!("{:064x}", 1),
            1,
            vec![],
            "policy deadline must not leak unchecked delivery".into(),
        )
        .unwrap();
        assert!(
            post_json(
                &c,
                1,
                &format!("{origin}/events"),
                &serde_json::to_value(event)?
            )
            .await?
            .status()
            .is_success()
        );
        for ws in &mut clients {
            let frame = tokio::time::timeout(Duration::from_secs(3), ws.next())
                .await
                .context("blocked live policy read must have a bounded deadline")?
                .context("policy deadline frame")??;
            if frame.is_text() {
                let value: Value = serde_json::from_str(&frame.into_text()?)?;
                assert_eq!(
                    value[0], "CLOSED",
                    "unverified event must not be delivered: {value}"
                );
            } else {
                assert!(
                    frame.is_close(),
                    "deadline must close or invalidate the stream"
                );
            }
        }
        tx.rollback().await?;
        return Ok(());
    }
    let mut locker = repo.pool.get().await?;
    let tx = locker.transaction().await?;
    tx.batch_execute("LOCK TABLE account_heads IN ACCESS EXCLUSIVE MODE")
        .await?;
    let mut held = tokio::task::JoinSet::new();
    for _ in 0..2 {
        let client = c.clone();
        let url = format!("{origin}/libraries/{}", public(2));
        held.spawn(async move {
            client
                .get(&url)
                .header("authorization", http_get(2, &url))
                .send()
                .await
        });
    }
    tokio::time::timeout(Duration::from_secs(2),async {
        loop {let n:i64=repo.pool.get().await?.query_one("SELECT count(*) FROM pg_locks WHERE relation='account_heads'::regclass AND NOT granted",&[]).await?.get(0);if n>=2 {break Ok::<_,anyhow::Error>(());}tokio::time::sleep(Duration::from_millis(5)).await;}
    }).await??;
    let event = auth::sign(
        &format!("{:064x}", 1),
        1,
        vec![],
        "fanout across a short read-admission stall".into(),
    )
    .unwrap();
    assert!(
        post_json(
            &c,
            1,
            &format!("{origin}/events"),
            &serde_json::to_value(&event)?
        )
        .await?
        .status()
        .is_success()
    );
    tokio::time::sleep(Duration::from_millis(350)).await;
    tx.rollback().await?;
    for ws in &mut clients {
        let frame = tokio::time::timeout(Duration::from_secs(3), ws.next())
            .await?
            .context("healthy live session closed during admission pressure")??;
        assert!(
            frame.is_text(),
            "healthy live session closed during transient admission pressure: {frame:?}"
        );
        let value: Value = serde_json::from_str(&frame.into_text()?)?;
        assert_eq!(
            value[0], "EVENT",
            "admission pressure must not lose the subscription: {value}"
        );
        let received: auth::Event = serde_json::from_value(value[2].clone())?;
        received.verify().map_err(anyhow::Error::msg)?;
        assert_eq!(received.id, event.id);
    }
    while let Some(result) = held.join_next().await {
        assert_eq!(result??.status(), 404);
    }
    // History/live handoff on a fresh subscription: an event published after
    // REQ must be present either in history or in the queued live continuation.
    clients[0]
        .send(tokio_tungstenite::tungstenite::Message::Text(
            json!(["CLOSE", "live"]).to_string().into(),
        ))
        .await?;
    clients[0]
        .send(tokio_tungstenite::tungstenite::Message::Text(
            json!(["REQ","handoff",{"kinds":[1],"limit":10}])
                .to_string()
                .into(),
        ))
        .await?;
    let next = auth::sign(
        &format!("{:064x}", 1),
        1,
        vec![],
        "published during history handoff".into(),
    )
    .unwrap();
    assert!(
        post_json(
            &c,
            1,
            &format!("{origin}/events"),
            &serde_json::to_value(&next)?
        )
        .await?
        .status()
        .is_success()
    );
    let mut seen = std::collections::BTreeSet::new();
    let mut eose = false;
    tokio::time::timeout(Duration::from_secs(5), async {
        while !eose || !seen.contains(&next.id) || !seen.contains(&event.id) {
            let value: Value = serde_json::from_str(
                &clients[0]
                    .next()
                    .await
                    .context("history/live stream")??
                    .into_text()?,
            )?;
            match value[0].as_str() {
                Some("EVENT") => {
                    seen.insert(value[2]["id"].as_str().context("event ID")?.to_owned());
                }
                Some("EOSE") => eose = true,
                _ => anyhow::bail!("history/live loss: {value}"),
            }
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    for ws in clients.iter_mut().skip(1) {
        let value: Value = serde_json::from_str(
            &tokio::time::timeout(Duration::from_secs(3), ws.next())
                .await?
                .unwrap()?
                .into_text()?,
        )?;
        assert_eq!(value[2]["id"], next.id);
    }
    // Deterministically leave an already-authorized event in the first TCP
    // socket. Revocation cannot retract bytes that were sent before it committed.
    let buffered = auth::sign(
        &format!("{:064x}", 1),
        1,
        vec![],
        "known event buffered before permission revocation".into(),
    )
    .unwrap();
    assert!(
        post_json(
            &c,
            1,
            &format!("{origin}/events"),
            &serde_json::to_value(&buffered)?
        )
        .await?
        .status()
        .is_success()
    );
    for ws in clients.iter_mut().skip(1) {
        let value: Value = serde_json::from_str(
            &tokio::time::timeout(Duration::from_secs(3), ws.next())
                .await?
                .context("buffered marker witness")??
                .into_text()?,
        )?;
        assert_eq!(value[0], "EVENT");
        let received: auth::Event = serde_json::from_value(value[2].clone())?;
        received.verify().map_err(anyhow::Error::msg)?;
        assert_eq!(value[2], serde_json::to_value(&buffered)?);
    }
    let tokio_tungstenite::MaybeTlsStream::Plain(socket) = clients[0].get_ref() else {
        anyhow::bail!("fixture requires a local plain WebSocket");
    };
    let mut byte = [0];
    ensure!(
        tokio::time::timeout(Duration::from_secs(3), socket.peek(&mut byte)).await?? > 0,
        "pre-revocation frame was not buffered"
    );

    // Revocation remains effective even though the event payload is shared.
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
    // One end-to-end deadline starts at the revocation acknowledgement, not
    // anew for each buffered frame or subscription.
    tokio::time::timeout(Duration::from_secs(4), async {
        // An independent, still-admitted subscriber proves the distinct new event
        // actually reaches live delivery after the revocation acknowledgement.
        let forbidden = auth::sign(
            &format!("{:064x}", 1),
            1,
            vec![],
            "distinct event published only after permission revocation".into(),
        )
        .unwrap();
        let forbidden_value = serde_json::to_value(&forbidden)?;
        let witness_url = format!("ws://{bind}/");
        let (mut witness, _) = tokio_tungstenite::connect_async(&witness_url).await?;
        let challenge: Value = serde_json::from_str(
            &witness
                .next()
                .await
                .context("witness challenge")??
                .into_text()?,
        )?;
        let proof = auth::sign(
            &format!("{:064x}", 3),
            22242,
            vec![
                vec!["relay".into(), witness_url],
                vec![
                    "challenge".into(),
                    challenge[1]
                        .as_str()
                        .context("witness challenge value")?
                        .into(),
                ],
            ],
            String::new(),
        )
        .unwrap();
        witness
            .send(tokio_tungstenite::tungstenite::Message::Text(
                json!(["AUTH", proof]).to_string().into(),
            ))
            .await?;
        let ack: Value =
            serde_json::from_str(&witness.next().await.context("witness auth")??.into_text()?)?;
        assert_eq!(ack[2], true);
        witness
            .send(tokio_tungstenite::tungstenite::Message::Text(
                json!(["REQ","witness",{"ids":[forbidden.id],"limit":0}])
                    .to_string()
                    .into(),
            ))
            .await?;
        let eose: Value =
            serde_json::from_str(&witness.next().await.context("witness EOSE")??.into_text()?)?;
        assert_eq!(eose[0], "EOSE", "witness subscription: {eose}");
        assert!(
            post_json(&c, 1, &format!("{origin}/events"), &forbidden_value)
                .await?
                .status()
                .is_success()
        );
        let delivered: Value = serde_json::from_str(
            &tokio::time::timeout(Duration::from_secs(3), witness.next())
                .await?
                .context("post-revocation witness event")??
                .into_text()?,
        )?;
        assert_eq!(delivered[0], "EVENT");
        let received: auth::Event = serde_json::from_value(delivered[2].clone())?;
        received.verify().map_err(anyhow::Error::msg)?;
        assert_eq!(delivered[2], forbidden_value);
        witness.close(None).await?;

        let permitted = [&event, &next, &buffered]
            .into_iter()
            .map(serde_json::to_value)
            .collect::<serde_json::Result<Vec<_>>>()?;
        for (index, ws) in clients.iter_mut().enumerate() {
            let subscription = if index == 0 { "handoff" } else { "live" };
            tokio::time::timeout(Duration::from_secs(4), async {
                let mut buffered_frames = 0;
                loop {
                    let frame = ws.next().await.context("revocation response")??;
                    let value: Value = serde_json::from_str(&frame.into_text()?)?;
                    assert_eq!(
                        value[1], subscription,
                        "unexpected subscription frame: {value}"
                    );
                    match value[0].as_str() {
                        Some("CLOSED") => {
                            ensure!(
                                value[2]
                                    .as_str()
                                    .is_some_and(|reason| reason.contains("revoked")),
                                "unexpected close: {value}"
                            );
                            break;
                        }
                        Some("EVENT") => {
                            let received: auth::Event = serde_json::from_value(value[2].clone())?;
                            received.verify().map_err(anyhow::Error::msg)?;
                            ensure!(
                                received.id != forbidden.id,
                                "post-revocation event delivered: {value}"
                            );
                            ensure!(
                                permitted.contains(&value[2]),
                                "unrecognized or changed pre-revocation event: {value}"
                            );
                            buffered_frames += 1;
                            ensure!(
                                buffered_frames <= 6,
                                "unbounded duplicate delivery before revocation close"
                            );
                        }
                        _ => anyhow::bail!("unexpected revocation frame: {value}"),
                    }
                }
                Ok::<_, anyhow::Error>(())
            })
            .await
            .context("revoked subscription did not close within its deadline")??;
            // After CLOSED no event is allowed; a fresh history request for the
            // forbidden event must also be rejected while this identity is revoked.
            let probe = format!("revoked-probe-{index}");
            ws.send(tokio_tungstenite::tungstenite::Message::Text(
                json!(["REQ",probe,{"ids":[forbidden.id],"limit":1}])
                    .to_string()
                    .into(),
            ))
            .await?;
            let denied: Value = serde_json::from_str(
                &tokio::time::timeout(Duration::from_secs(3), ws.next())
                    .await?
                    .context("revoked history response")??
                    .into_text()?,
            )?;
            assert_eq!(
                denied[0], "CLOSED",
                "event delivered after subscription revocation: {denied}"
            );
            assert_eq!(denied[1], probe);
            ensure!(
                denied[2]
                    .as_str()
                    .is_some_and(|reason| reason.contains("not admitted")),
                "revoked history was not denied: {denied}"
            );
        }
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("revocation observation exceeded four seconds")??;
    // A real TCP slow reader uses a small receive window set before connect.
    // Its transport/queue must not hold up a fast subscriber or unbound memory.
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
    let mut slow_clients = Vec::new();
    for _ in 0..2 {
        let socket = tokio::net::TcpSocket::new_v4()?;
        socket.set_recv_buffer_size(1024)?;
        let tcp = socket.connect(bind).await?.into_std()?;
        let control = tokio::net::TcpSocket::from_std_stream(tcp.try_clone()?);
        let tcp = tokio::net::TcpStream::from_std(tcp)?;
        let url = format!("ws://{bind}/");
        let (mut slow, _) = tokio_tungstenite::client_async(&url, tcp).await?;
        let challenge: Value = serde_json::from_str(&slow.next().await.unwrap()?.into_text()?)?;
        let proof = auth::sign(
            &format!("{:064x}", 2),
            22242,
            vec![
                vec!["relay".into(), url],
                vec!["challenge".into(), challenge[1].as_str().unwrap().into()],
            ],
            String::new(),
        )
        .unwrap();
        slow.send(tokio_tungstenite::tungstenite::Message::Text(
            json!(["AUTH", proof]).to_string().into(),
        ))
        .await?;
        let ack: Value = serde_json::from_str(&slow.next().await.unwrap()?.into_text()?)?;
        assert_eq!(ack[2], true);
        slow.send(tokio_tungstenite::tungstenite::Message::Text(
            json!(["REQ","slow",{"kinds":[1],"#t":["slow"],"limit":0}])
                .to_string()
                .into(),
        ))
        .await?;
        let eose: Value = serde_json::from_str(&slow.next().await.unwrap()?.into_text()?)?;
        assert_eq!(eose[0], "EOSE");
        slow_clients.push((slow, control));
    }
    let mut fast = clients.remove(0);
    fast.send(tokio_tungstenite::tungstenite::Message::Text(
        json!(["REQ","fast",{"kinds":[1],"#t":["slow"],"limit":0}])
            .to_string()
            .into(),
    ))
    .await?;
    let eose: Value = serde_json::from_str(&fast.next().await.unwrap()?.into_text()?)?;
    assert_eq!(eose[0], "EOSE");
    let notes: Vec<_> = (0..120)
        .map(|n| {
            auth::sign(
                &format!("{:064x}", 1),
                1,
                vec![vec!["t".into(), "slow".into()]],
                format!("{n}:{}", "x".repeat(60000)),
            )
            .unwrap()
        })
        .collect();
    let expected: std::collections::BTreeSet<_> = notes.iter().map(|e| e.id.clone()).collect();
    let fast_reader = tokio::spawn(async move {
        let mut seen = std::collections::BTreeSet::new();
        while seen.len() < expected.len() {
            let frame = fast.next().await.context("fast stream")??;
            ensure!(
                frame.is_text(),
                "fast reader closed while two slow sockets stalled: {frame:?}"
            );
            let value: Value = serde_json::from_str(&frame.into_text()?)?;
            ensure!(value[0] == "EVENT", "fast reader lost delivery: {value}");
            let event: auth::Event = serde_json::from_value(value[2].clone())?;
            event.verify().map_err(anyhow::Error::msg)?;
            ensure!(expected.contains(&event.id), "unexpected fast-reader event");
            seen.insert(event.id);
        }
        Ok::<_, anyhow::Error>(fast)
    });
    let results: Vec<_> = futures_util::stream::iter(notes)
        .map(|event| {
            let client = c.clone();
            let target = format!("{origin}/events");
            async move { post_json(&client, 1, &target, &serde_json::to_value(event)?).await }
        })
        .buffer_unordered(8)
        .collect()
        .await;
    for response in results {
        let response = response?;
        ensure!(
            response.status().is_success(),
            "slow-reader fixture publication: {}",
            response.text().await?
        );
    }
    let mut fast = tokio::time::timeout(Duration::from_secs(15), fast_reader).await???;
    // Allow the production five-second send deadline to expire while unread.
    tokio::time::sleep(Duration::from_secs(6)).await;
    for (mut slow, control) in slow_clients {
        control.set_recv_buffer_size(4 * 1024 * 1024)?;
        let delivered = tokio::time::timeout(Duration::from_secs(15), async {
            let mut count = 0;
            loop {
                match slow.next().await {
                    None | Some(Err(_)) => break,
                    Some(Ok(message)) if message.is_close() => break,
                    Some(Ok(message)) if message.is_text() => {
                        let value: Value = serde_json::from_str(&message.into_text()?)?;
                        if value[0] == "CLOSED" {
                            break;
                        }
                        if value[0] == "EVENT" {
                            count += 1;
                        }
                    }
                    _ => {}
                }
            }
            Ok::<_, anyhow::Error>(count)
        })
        .await??;
        assert!(
            delivered < 120,
            "slow reader must observe explicit termination rather than silent loss"
        );
    }
    // Actual PostgreSQL notification transport loss invalidates a healthy stream.
    repo.pool.get().await?.simple_query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE application_name='manga-relay-feed'").await?;
    let lost: Value = serde_json::from_str(
        &tokio::time::timeout(Duration::from_secs(5), fast.next())
            .await?
            .context("loss marker")??
            .into_text()?,
    )?;
    assert_eq!(lost[0], "CLOSED");
    assert!(lost[2].as_str().unwrap().contains("interrupted"));
    Ok(())
}
