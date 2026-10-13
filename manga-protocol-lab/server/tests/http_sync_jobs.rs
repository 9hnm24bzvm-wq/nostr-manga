mod support;
use anyhow::{Context, Result};
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
const A: &str = "http://127.0.0.1:19530";
const B: &str = "http://127.0.0.1:19531";
struct Node(Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
async fn repo(dsn: String) -> Result<Repository> {
    Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await
}
async fn ready(c: &reqwest::Client, origin: &str) -> Result<()> {
    for _ in 0..100 {
        if c.get(origin).send().await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    anyhow::bail!("node failed to start")
}
async fn status(c: &reqwest::Client, job: &str) -> Result<Value> {
    let url = format!("{B}/admin/sync-jobs/{job}");
    Ok(c.get(&url)
        .header("authorization", http_get(2, &url))
        .send()
        .await?
        .json()
        .await?)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and real strfry; run scripts/test-postgres.sh"]
async fn durable_job_resumes_after_real_worker_crash() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(
        cfg.get_dbname() == Some("manga_test") && dsn.contains("dbname=manga_test"),
        "fixture required"
    );
    let source = repo(dsn.clone()).await?;
    source.migrate().await?;
    source
        .pool
        .get()
        .await?
        .batch_execute("CREATE DATABASE manga_sync_b")
        .await?;
    let bdsn = dsn.replace("dbname=manga_test", "dbname=manga_sync_b");
    let receiver = repo(bdsn.clone()).await?;
    receiver.migrate().await?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let dir = tempfile::tempdir()?;
    let binary = std::env::var_os("STRFRY_BIN")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("vendor/strfry/strfry"));
    let mut paths = vec![];
    for (n, origin) in [A, B].iter().enumerate() {
        let base = dir.path().join(format!("node-{n}"));
        std::fs::create_dir_all(&base)?;
        let key = base.join("key");
        write_key(&key, 11 + n as u8)?;
        let path = base.join("config.json");
        std::fs::write(
            &path,
            serde_json::to_vec(
                &json!({"bind":format!("127.0.0.1:{}",19530+n),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1+n as u8),"database_plaintext_local":true,"sync_worker_enabled":false,"backend":{"binary":binary,"data_dir":base.join("backend"),"timeout_seconds":3,"map_bytes":67108864},"storage":{"path":base.join("originals"),"volume":format!("sync{n}"),"physical_bytes":2097152,"staging_bytes":65536,"max_blob_bytes":262144}}),
            )?,
        )?;
        paths.push(path);
    }
    let spawn = |n: usize| -> Result<Node> {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.path().join(format!("node-{n}/node.log")))?;
        Ok(Node(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", paths[n].to_str().unwrap()])
                .env("MANGA_DATABASE_URL", if n == 0 { &dsn } else { &bdsn })
                .env("RUST_LOG", "warn")
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        ))
    };
    let _a = spawn(0)?;
    let b = spawn(1)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    ready(&client, A).await?;
    ready(&client, B).await?;
    for (origin, admin, peer, endpoint) in [(A, 1, 12, B), (B, 2, 11, A)] {
        assert_eq!(
            post_json(
                &client,
                admin,
                &format!("{origin}/admin/peers"),
                &json!({"key":public(peer),"endpoint":endpoint,"allowed":true})
            )
            .await?
            .status(),
            200
        );
    }
    let image = png();
    upload_to(&client, 1, A, &image).await?;
    let mut events = vec![];
    let mut expected_physical = image.len();
    for chapter in 1..=4 {
        let bytes = cbor(
            json!({"version":1,"series":"ab".repeat(32),"language":"en","chapter":chapter.to_string(),"pages":[page(&image)]}),
        );
        expected_physical += bytes.len();
        upload_to(&client, 1, A, &bytes).await?;
        let event = publication(1, 1001, &bytes);
        assert!(
            post_json(
                &client,
                1,
                &format!("{A}/publications"),
                &serde_json::to_value(&event)?
            )
            .await?
            .status()
            .is_success()
        );
        wait_projected(&client, A, &event.id).await?;
        events.push(event);
    }
    let spec = json!({"peer":public(11),"filter":"language=\"en\"","fallbacks":[]});
    let first: Value = post_json(&client, 2, &format!("{B}/admin/sync-jobs"), &spec)
        .await?
        .json()
        .await?;
    let job = first["id"].as_str().context("job ID")?.to_owned();
    let duplicate: Value = post_json(&client, 2, &format!("{B}/admin/sync-jobs"), &spec)
        .await?
        .json()
        .await?;
    assert_eq!(duplicate["id"], job);
    assert_eq!(status(&client, &job).await?["state"], "queued");
    // Block the first publication commit after durable transfer, then kill the
    // actual worker process. Queue, partial progress and live lease must survive.
    let fault = receiver.pool.get().await?;
    let fault_pid: i32 = fault
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    fault
        .execute("SELECT pg_advisory_lock(927442)", &[])
        .await?;
    fault.batch_execute("CREATE FUNCTION fixture_block_sync_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.state='hosted' THEN PERFORM pg_advisory_xact_lock(927442); END IF; RETURN NEW; END $$; CREATE TRIGGER fixture_block_sync_commit BEFORE UPDATE OF state ON events FOR EACH ROW EXECUTE FUNCTION fixture_block_sync_commit();").await?;
    drop(b);
    let mut config: Value = serde_json::from_slice(&std::fs::read(&paths[1])?)?;
    config["sync_worker_enabled"] = json!(true);
    config["storage"]["physical_bytes"] = json!(expected_physical);
    std::fs::write(&paths[1], serde_json::to_vec(&config)?)?;
    let b = spawn(1)?;
    ready(&client, B).await?;
    let blocked = wait_for_blocked_session(&receiver, fault_pid).await?;
    // Capture the externally interrupted worker identity solely to arrange
    // lease-expiry fault and exercise the public stale-authority fence later.
    let lease=fault.query_one("SELECT j.id,j.generation,j.claim_token,j.payload FROM jobs j JOIN sync_partitions p ON p.job_id=j.id WHERE p.batch_id=$1 AND j.state='leased'", &[&hex::decode(&job)?]).await?;
    let job_id: Vec<u8> = lease.get(0);
    let old_lease = manga_node::db::Lease {
        id: job_id.clone().try_into().unwrap(),
        generation: lease.get(1),
        token: lease.get::<_, Vec<u8>>(2).try_into().unwrap(),
        payload: lease.get(3),
    };
    drop(b);
    fault
        .execute("SELECT pg_terminate_backend($1)", &[&blocked])
        .await?;
    fault
        .execute("SELECT pg_advisory_unlock(927442)", &[])
        .await?;
    fault.batch_execute("DROP TRIGGER fixture_block_sync_commit ON events; DROP FUNCTION fixture_block_sync_commit();").await?;
    let _b = spawn(1)?;
    ready(&client, B).await?;
    assert!(
        receiver.lease_current(&old_lease).await?,
        "startup must preserve live worker authority"
    );
    assert!(
        receiver.claim_sync().await?.is_none(),
        "restart must not make live work claimable"
    );
    fault
        .execute(
            "UPDATE jobs SET lease_until=clock_timestamp()-interval '1 second' WHERE id=$1",
            &[&job_id],
        )
        .await?;
    let mut complete = None;
    for _ in 0..300 {
        let value = status(&client, &job).await?;
        if value["state"] == "completed" {
            complete = Some(value);
            break;
        }
        assert_ne!(value["state"], "failed", "{value}");
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    let complete = complete.context("resumed job did not complete")?;
    assert_eq!(complete["acknowledged_items"], 4);
    for event in &events {
        wait_projected(&client, B, &event.id).await?;
    }
    let original = client
        .get(format!("{B}/originals/{}", auth::hash(&image)))
        .send()
        .await?;
    assert_eq!(original.status(), 200);
    assert_eq!(original.bytes().await?.as_ref(), image);
    assert!(
        !receiver.lease_current(&old_lease).await?,
        "completed recovery must fence the old worker"
    );
    // Exact quota leaves no room for duplicate charging on restart. Identical
    // uploads still work, while a distinct valid object must be rejected.
    upload_to(&client, 2, B, &image).await?;
    let mut distinct_image = image.clone();
    distinct_image.extend_from_slice(b"new physical object");
    let distinct_hash = auth::hash(&distinct_image);
    let rejected = client
        .put(format!("{B}/upload/{distinct_hash}"))
        .header("authorization", blossom(2, B, "upload", &distinct_hash))
        .body(distinct_image.clone())
        .send()
        .await?;
    assert!(
        !rejected.status().is_success(),
        "exact physical budget admitted another original"
    );
    assert_eq!(
        client
            .get(format!("{B}/originals/{distinct_hash}"))
            .send()
            .await?
            .status(),
        404
    );
    drop(_b);
    config["storage"]["physical_bytes"] = json!(2097152);
    std::fs::write(&paths[1], serde_json::to_vec(&config)?)?;
    let _b = spawn(1)?;
    ready(&client, B).await?;
    upload_to(&client, 2, B, &distinct_image).await?;
    // Common synthetic inventory IDs force automatic prefix splitting, while
    // only one genuinely new chapter needs original/manifest admission.
    let seed = "INSERT INTO events(id,publisher,kind,created_at,raw_event,state,language) SELECT decode(md5('bulk'||g::text)||md5('fixture'||g::text),'hex'),decode(repeat('11',32),'hex'),1001,1000,'{}'::jsonb,'hosted','en' FROM generate_series($1::BIGINT,$2::BIGINT) g";
    for start in (1i64..=120000).step_by(5000) {
        let end = start + 4999;
        source
            .pool
            .get()
            .await?
            .execute(seed, &[&start, &end])
            .await?;
        fault.execute(seed, &[&start, &end]).await?;
    }
    let bytes = cbor(
        json!({"version":1,"series":"ab".repeat(32),"language":"en","chapter":"5","pages":[page(&image)]}),
    );
    upload_to(&client, 1, A, &bytes).await?;
    let fifth = publication(1, 1001, &bytes);
    assert!(
        post_json(
            &client,
            1,
            &format!("{A}/publications"),
            &serde_json::to_value(&fifth)?
        )
        .await?
        .status()
        .is_success()
    );
    wait_projected(&client, A, &fifth.id).await?;
    let large: Value = post_json(&client, 2, &format!("{B}/admin/sync-jobs"), &spec)
        .await?
        .json()
        .await?;
    let large_job = large["id"].as_str().unwrap();
    let mut large_complete = None;
    for _ in 0..1000 {
        let value = status(&client, large_job).await?;
        if value["state"] == "completed" {
            large_complete = Some(value);
            break;
        }
        assert_ne!(value["state"], "failed", "{value}");
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    let large_complete = large_complete.context("partitioned job did not finish")?;
    assert_eq!(large_complete["partitions_created"], 17);
    assert_eq!(large_complete["partitions_done"], 17);
    assert_eq!(large_complete["acknowledged_items"], 1);
    wait_projected(&client, B, &fifth.id).await?;
    // A new queue request can be cancelled without rolling back earlier content.
    let queued: Value = post_json(
        &client,
        2,
        &format!("{B}/admin/sync-jobs"),
        &json!({"peer":public(11),"filter":"language=\"ja\""}),
    )
    .await?
    .json()
    .await?;
    let cancelled = post_json(
        &client,
        2,
        &format!(
            "{B}/admin/sync-jobs/{}/cancel",
            queued["id"].as_str().unwrap()
        ),
        &json!({}),
    )
    .await?;
    assert_eq!(cancelled.status(), 200);
    assert_eq!(
        client
            .get(format!("{B}/events/{}", events[0].id))
            .send()
            .await?
            .status(),
        200
    );
    // Series use their own inventory domain and the same protected publication fences.
    let series_bytes = cbor(json!({"version":1,"titles":{"en":"Synthetic federation series"}}));
    upload_to(&client, 1, A, &series_bytes).await?;
    let series = publication(1, 1002, &series_bytes);
    assert!(
        post_json(
            &client,
            1,
            &format!("{A}/publications"),
            &serde_json::to_value(&series)?
        )
        .await?
        .status()
        .is_success()
    );
    wait_projected(&client, A, &series.id).await?;
    let filtered: Value = post_json(
        &client,
        2,
        &format!("{B}/admin/sync"),
        &json!({"peer":public(11),"event":series.id,"filter":"language=\"en\""}),
    )
    .await?
    .json()
    .await?;
    assert_eq!(
        filtered["matched"], false,
        "series has no chapter language field"
    );
    let series_filter = format!(
        "publisher=\"{}\" AND created_at>={}",
        public(1),
        series.created_at
    );
    let series_rows = source
        .inventory_rows_scoped("series", &series_filter, "")
        .await?;
    assert_eq!(series_rows.len(), 1);
    assert_eq!(hex::encode(series_rows[0].1), series.id);
    assert!(
        source
            .inventory_rows_scoped("series", &format!("created_at>{}", series.created_at), "")
            .await?
            .is_empty()
    );
    let submitted: Value = post_json(
        &client,
        2,
        &format!("{B}/admin/sync-jobs"),
        &json!({"peer":public(11),"inventory":"series","filter":series_filter}),
    )
    .await?
    .json()
    .await?;
    let series_job = submitted["id"].as_str().context("series job rejected")?;
    let mut completed = false;
    for _ in 0..300 {
        let state = status(&client, series_job).await?;
        if state["state"] == "completed" {
            assert_eq!(state["acknowledged_items"], 1);
            completed = true;
            break;
        }
        assert_ne!(state["state"], "failed", "{state}");
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    anyhow::ensure!(completed, "series reconciliation did not finish");
    wait_projected(&client, B, &series.id).await?;
    let retained_series: auth::Event = client
        .get(format!("{B}/events/{}", series.id))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(
        serde_json::to_value(&retained_series)?,
        serde_json::to_value(&series)?
    );
    let retained_manifest = client
        .get(format!("{B}/blobs/{}", auth::hash(&series_bytes)))
        .header(
            "authorization",
            blossom(2, B, "get", &auth::hash(&series_bytes)),
        )
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    assert_eq!(retained_manifest.as_ref(), series_bytes);
    std::fs::write(
        root.join("results/series-federation.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["series manifest validated and independently hosted","chapter-only field is NULL for series filtering","publisher and timestamp inventory filters","separate durable series reconciliation","real strfry projection","one retained local series manifest"],"scope":"synthetic two-node PostgreSQL/strfry fixture"}),
        )?,
    )?;
    // Standalone account metadata and signed deletion evidence reconcile separately.
    for key in [1, 21] {
        source
            .set_account_admission(hex::decode(public(key))?.try_into().unwrap(), true)
            .await?;
    }
    let mut profile_events = vec![];
    for kind in [0, 3, 10002] {
        let event = sign_at(
            21,
            kind,
            vec![],
            if kind == 0 {
                "{\"name\":\"Federated profile\"}".into()
            } else {
                String::new()
            },
            manga_node::auth::now(),
        );
        assert!(
            post_json(
                &client,
                21,
                &format!("{A}/account-metadata"),
                &serde_json::to_value(&event)?
            )
            .await?
            .status()
            .is_success()
        );
        wait_projected(&client, A, &event.id).await?;
        profile_events.push(event);
    }
    let profile_filter = format!("publisher=\"{}\" AND created_at>=0", public(21));
    let submitted: Value = post_json(
        &client,
        2,
        &format!("{B}/admin/sync-jobs"),
        &json!({"peer":public(11),"inventory":"profiles","filter":profile_filter}),
    )
    .await?
    .json()
    .await?;
    let job = submitted["id"].as_str().context("profile job rejected")?;
    let mut done = false;
    for _ in 0..300 {
        let value = status(&client, job).await?;
        if value["state"] == "completed" {
            assert_eq!(value["acknowledged_items"], 3);
            done = true;
            break;
        }
        assert_ne!(value["state"], "failed", "{value}");
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    anyhow::ensure!(done, "profile inventory did not finish");
    for e in &profile_events {
        wait_projected(&client, B, &e.id).await?;
    }
    assert!(
        receiver
            .account_permit(hex::decode(public(21))?.try_into().unwrap())
            .await
            .is_err(),
        "replication granted personal posting"
    );
    let deletion = sign_at(
        21,
        5,
        vec![vec!["e".into(), profile_events[0].id.clone()]],
        String::new(),
        manga_node::auth::now(),
    );
    assert!(
        post_json(
            &client,
            21,
            &format!("{A}/account-deletions"),
            &serde_json::to_value(&deletion)?
        )
        .await?
        .status()
        .is_success()
    );
    let protected = sign_at(
        1,
        5,
        vec![vec!["e".into(), series.id.clone()]],
        String::new(),
        manga_node::auth::now(),
    );
    assert!(
        post_json(
            &client,
            1,
            &format!("{A}/account-deletions"),
            &serde_json::to_value(&protected)?
        )
        .await?
        .status()
        .is_success()
    );
    let submitted: Value = post_json(
        &client,
        2,
        &format!("{B}/admin/sync-jobs"),
        &json!({"peer":public(11),"inventory":"deletions"}),
    )
    .await?
    .json()
    .await?;
    let job = submitted["id"].as_str().context("deletion job rejected")?;
    let mut done = false;
    for _ in 0..300 {
        let value = status(&client, job).await?;
        if value["state"] == "completed" {
            assert_eq!(value["acknowledged_items"], 2);
            done = true;
            break;
        }
        assert_ne!(value["state"], "failed", "{value}");
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    anyhow::ensure!(done, "deletion inventory did not finish");
    assert_eq!(
        client
            .get(format!("{B}/events/{}", profile_events[0].id))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        client
            .get(format!("{B}/events/{}", series.id))
            .send()
            .await?
            .status(),
        200,
        "protected series automatically removed"
    );
    assert!(
        receiver
            .ordinary_deletion_request(hex::decode(&deletion.id)?.try_into().unwrap())
            .await?
            .is_some()
    );
    std::fs::write(
        root.join("results/account-inventory.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["durable profiles inventory imports kinds0/3/10002","publisher/timestamp filtering","remote authors cannot log in implicitly","durable signed deletion evidence imports and applies ordinary deletion","protected series remains hosted awaiting local moderation"],"scope":"synthetic two-node PostgreSQL and strfry"}),
        )?,
    )?;
    // Force lexical queue order reaction -> reply -> parent; replication must
    // obtain selected dependencies first, not rely on ID or timestamp order.
    let make_social = |key: u8,
                       kind: u32,
                       mut tags: Vec<Vec<String>>,
                       content: &str,
                       prefix: &str|
     -> manga_node::auth::Event {
        tags.push(vec!["nonce".into(), String::new()]);
        for nonce in 0..4096 {
            tags.last_mut().unwrap()[1] = nonce.to_string();
            let e = sign_at(
                key,
                kind,
                tags.clone(),
                content.into(),
                manga_node::auth::now(),
            );
            if e.id.starts_with(prefix) {
                return e;
            }
        }
        panic!("synthetic prefix search exhausted");
    };
    let comment_tags = |parent: &str, parent_kind: u32, parent_key: u8| {
        vec![
            vec!["E".into(), series.id.clone()],
            vec!["K".into(), "1002".into()],
            vec!["P".into(), public(1)],
            vec!["e".into(), parent.into()],
            vec!["k".into(), parent_kind.to_string()],
            vec!["p".into(), public(parent_key)],
        ]
    };
    let parent = make_social(
        21,
        1111,
        comment_tags(&series.id, 1002, 1),
        "Synthetic parent",
        "f",
    );
    let reply = make_social(
        21,
        1111,
        comment_tags(&parent.id, 1111, 21),
        "Synthetic reply",
        "8",
    );
    let reaction = make_social(
        21,
        7,
        vec![
            vec!["e".into(), reply.id.clone()],
            vec!["p".into(), public(21)],
            vec!["k".into(), "1111".into()],
        ],
        "+",
        "1",
    );
    for event in [&parent, &reply, &reaction] {
        let response = post_json(
            &client,
            21,
            &format!("{A}/social"),
            &serde_json::to_value(event)?,
        )
        .await?;
        assert!(response.status().is_success(), "{}", response.text().await?);
        wait_projected(&client, A, &event.id).await?;
    }
    let social_spec = manga_node::sync_jobs::SyncSpec {
        peer: public(11),
        inventory: "social".into(),
        filter: format!("publisher=\"{}\"", public(21)),
        fallbacks: vec![],
    };
    // Observe actual live Nostr delivery, not private projection queue ordering.
    let (mut social_feed, _) =
        tokio_tungstenite::connect_async(B.replace("http://", "ws://")).await?;
    social_feed
        .send(tokio_tungstenite::tungstenite::Message::Text(
            json!(["REQ","dependency-order",{"ids":[parent.id,reply.id,reaction.id]}])
                .to_string()
                .into(),
        ))
        .await?;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let message = social_feed
                .next()
                .await
                .context("social feed closed before EOSE")??;
            if let Ok(text) = message.to_text() {
                let value: Value = serde_json::from_str(text)?;
                if value[0] == "EOSE" && value[1] == "dependency-order" {
                    break Ok::<_, anyhow::Error>(());
                }
                assert_ne!(
                    value[0], "EVENT",
                    "receiver unexpectedly already had social dependency"
                );
            }
        }
    })
    .await??;
    let social_job = receiver.enqueue_sync(social_spec.clone()).await?;
    let mut done = false;
    for _ in 0..300 {
        let value = status(&client, &hex::encode(social_job)).await?;
        if value["state"] == "completed" {
            assert_eq!(value["acknowledged_items"], 3);
            done = true;
            break;
        }
        assert_ne!(value["state"], "failed", "{value}");
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    anyhow::ensure!(done, "social dependency job did not finish");
    let observed = tokio::time::timeout(Duration::from_secs(5), async {
        let mut observed = Vec::new();
        while observed.len() < 3 {
            let message = social_feed.next().await.context("social feed closed")??;
            if let Ok(text) = message.to_text() {
                let value: Value = serde_json::from_str(text)?;
                if value[0] == "EVENT" && value[1] == "dependency-order" {
                    observed.push(
                        value[2]["id"]
                            .as_str()
                            .context("event ID missing")?
                            .to_owned(),
                    );
                }
            }
        }
        Ok::<_, anyhow::Error>(observed)
    })
    .await??;
    assert_eq!(
        observed,
        vec![parent.id.clone(), reply.id.clone(), reaction.id.clone()],
        "live Nostr delivery must preserve dependency order without duplicate identities"
    );
    social_feed.close(None).await?;
    for event in [&parent, &reply, &reaction] {
        let fetched: auth::Event = client
            .get(format!("{B}/events/{}", event.id))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        assert_eq!(serde_json::to_value(fetched)?, serde_json::to_value(event)?);
    }
    source
        .set_account_admission(hex::decode(public(22))?.try_into().unwrap(), true)
        .await?;
    let excluded = make_social(
        22,
        1111,
        comment_tags(&series.id, 1002, 1),
        "Excluded author parent",
        "f",
    );
    let blocked = make_social(
        21,
        1111,
        comment_tags(&excluded.id, 1111, 22),
        "Reply needing excluded parent",
        "8",
    );
    for (key, event) in [(22, &excluded), (21, &blocked)] {
        assert!(
            post_json(
                &client,
                key,
                &format!("{A}/social"),
                &serde_json::to_value(event)?
            )
            .await?
            .status()
            .is_success()
        );
        wait_projected(&client, A, &event.id).await?;
    }
    let blocked_job = receiver.enqueue_sync(social_spec).await?;
    let mut refused = false;
    for _ in 0..300 {
        let value = status(&client, &hex::encode(blocked_job)).await?;
        if value["last_error"]
            .as_str()
            .is_some_and(|s| s.contains("excluded by selected filter"))
        {
            refused = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    anyhow::ensure!(refused, "excluded dependency was not explicitly rejected");
    assert!(receiver.cancel_sync(blocked_job).await?);
    for e in [&excluded, &blocked] {
        assert!(
            receiver
                .visible_event(hex::decode(&e.id)?.try_into().unwrap())
                .await?
                .is_none()
        );
    }
    std::fs::write(
        root.join("results/social-inventory.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["reaction ID sorts before reply before parent","dependency traversal imports parent then reply then reaction","live Nostr delivery emits parent, reply and reaction identities in order","out-of-filter parent and dependent reply are not imported","explicit error and cancellation leave earlier admitted comments intact"],"scope":"synthetic two-node PostgreSQL/strfry; manga root already locally hosted"}),
        )?,
    )?;
    use base64::{Engine, engine::general_purpose::STANDARD};
    use manga_node::library_manifest::{
        Chunk, Descriptor, Family, FamilyRoot, Limits, Record, Root,
    };
    let limits = Limits::default();
    let snapshot = |progress: u32| -> Result<(Vec<u8>, Vec<u8>)> {
        let chunk = Chunk::new(
            Family::Chapters,
            vec![Record::Chapter {
                id: [63; 32],
                completed: false,
                page: Some(progress),
                updated_at: progress as u64,
            }],
            &limits,
        )?
        .encode(&limits)?;
        let descriptor = Descriptor::from(&Chunk::decode(&chunk, &limits)?.1);
        let root = Root {
            families: Family::ALL
                .into_iter()
                .map(|family| FamilyRoot {
                    family,
                    chunks: vec![if family == Family::Chapters {
                        descriptor.clone()
                    } else {
                        Descriptor::empty()
                    }],
                })
                .collect(),
        }
        .encode(&limits)?;
        Ok((chunk, root))
    };
    let mut library_heads = vec![];
    let mut library_chunks = vec![];
    for progress in [1, 2] {
        let (chunk, root_bytes) = snapshot(progress)?;
        let pointer = sign_at(
            21,
            11999,
            vec![],
            json!({"manifest":manga_node::auth::hash(&root_bytes),"version":1}).to_string(),
            manga_node::auth::now() + progress as u64,
        );
        let expected = library_heads
            .last()
            .map(|e: &manga_node::auth::Event| e.id.clone());
        let body = json!({"event":pointer,"expected_head":expected,"objects":[{"type":"chunk","data":STANDARD.encode(&chunk)},{"type":"root","data":STANDARD.encode(&root_bytes)}]});
        let response = post_json(&client, 21, &format!("{A}/libraries"), &body).await?;
        assert!(response.status().is_success(), "{}", response.text().await?);
        wait_projected(&client, A, &pointer.id).await?;
        library_heads.push(pointer);
        library_chunks.push(chunk);
    }
    // Persist an old snapshot ID with the worker stopped, then resume. The old
    // signed pointer routes to the author, but only the current complete head imports.
    drop(_b);
    let library_spec = manga_node::sync_jobs::SyncSpec {
        peer: public(11),
        inventory: "libraries".into(),
        filter: format!("publisher=\"{}\"", public(21)),
        fallbacks: vec![],
    };
    let library_job = receiver.enqueue_sync(library_spec.clone()).await?;
    let work = receiver
        .claim_sync()
        .await?
        .context("library work missing")?;
    assert_eq!(work.batch, library_job);
    receiver
        .save_missing(
            &work,
            vec![hex::decode(&library_heads[0].id)?.try_into().unwrap()],
        )
        .await?;
    receiver.finish_sync_quantum(&work).await?;
    let _b = spawn(1)?;
    ready(&client, B).await?;
    let mut done = false;
    for _ in 0..300 {
        let value = status(&client, &hex::encode(library_job)).await?;
        if value["state"] == "completed" {
            assert_eq!(value["acknowledged_items"], 1);
            done = true;
            break;
        }
        assert_ne!(value["state"], "failed", "{value}");
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    anyhow::ensure!(done, "library inventory did not finish");
    let head: Value = client
        .get(format!("{B}/libraries/{}", public(21)))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(head["event"]["id"], library_heads[1].id);
    let fetched = client
        .get(format!(
            "{B}/library/blobs/{}",
            manga_node::auth::hash(&library_chunks[1])
        ))
        .send()
        .await?
        .bytes()
        .await?;
    assert_eq!(&fetched[..], library_chunks[1]);
    let converged = receiver.enqueue_sync(library_spec.clone()).await?;
    let mut done = false;
    for _ in 0..300 {
        let value = status(&client, &hex::encode(converged)).await?;
        if value["state"] == "completed" {
            assert_eq!(value["acknowledged_items"], 0);
            done = true;
            break;
        }
        assert_ne!(value["state"], "failed", "{value}");
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    anyhow::ensure!(done, "current library inventory did not converge");
    // Cancel while an owned staging transaction is paused. Private bytes may
    // remain charged; no new public head may commit after cancellation completes.
    let (chunk, root_bytes) = snapshot(3)?;
    let pointer = sign_at(
        21,
        11999,
        vec![],
        json!({"manifest":manga_node::auth::hash(&root_bytes),"version":1}).to_string(),
        manga_node::auth::now() + 3,
    );
    assert!(post_json(&client,21,&format!("{A}/libraries"),&json!({"event":pointer,"expected_head":library_heads[1].id,"objects":[{"type":"chunk","data":STANDARD.encode(&chunk)},{"type":"root","data":STANDARD.encode(&root_bytes)}]})).await?.status().is_success());
    wait_projected(&client, A, &pointer.id).await?;
    fault
        .execute("SELECT pg_advisory_lock(927445)", &[])
        .await?;
    fault.batch_execute("CREATE FUNCTION fixture_pause_library_job() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(927445); RETURN NEW; END $$; CREATE TRIGGER fixture_pause_library_job BEFORE INSERT ON account_blob_grants FOR EACH ROW EXECUTE FUNCTION fixture_pause_library_job();").await?;
    let cancelled_library = receiver.enqueue_sync(library_spec).await?;
    let blocked_worker = wait_for_blocked_session(&receiver, fault_pid).await?;
    let cancel_repo = receiver.clone();
    let cancel = tokio::spawn(async move { cancel_repo.cancel_sync(cancelled_library).await });
    wait_for_blocked_session(&receiver, blocked_worker).await?;
    fault
        .execute("SELECT pg_advisory_unlock(927445)", &[])
        .await?;
    assert!(cancel.await??);
    fault.batch_execute("DROP TRIGGER fixture_pause_library_job ON account_blob_grants;DROP FUNCTION fixture_pause_library_job();").await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let head: Value = client
        .get(format!("{B}/libraries/{}", public(21)))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(
        head["event"]["id"], library_heads[1].id,
        "cancelled root became public"
    );
    // Real HTTP policy and background ticker, not a manual dispatch call.
    let scheduled_profile = sign_at(
        22,
        0,
        vec![],
        "{\"name\":\"Scheduled author\"}".into(),
        manga_node::auth::now(),
    );
    assert!(
        post_json(
            &client,
            22,
            &format!("{A}/account-metadata"),
            &serde_json::to_value(&scheduled_profile)?
        )
        .await?
        .status()
        .is_success()
    );
    wait_projected(&client, A, &scheduled_profile.id).await?;
    let schedule_body = json!({"spec":{"peer":public(11),"inventory":"profiles","filter":format!("publisher=\"{}\"",public(22))},"interval_seconds":30,"enabled":true});
    assert_eq!(
        post_json(
            &client,
            21,
            &format!("{B}/admin/sync-schedules"),
            &schedule_body
        )
        .await?
        .status(),
        403
    );
    let response = post_json(
        &client,
        2,
        &format!("{B}/admin/sync-schedules"),
        &schedule_body,
    )
    .await?;
    assert_eq!(response.status(), 200);
    let policy: Value = response.json().await?;
    let policy_id = policy["id"].as_str().context("schedule ID missing")?;
    wait_projected(&client, B, &scheduled_profile.id).await?;
    let list_url = format!("{B}/admin/sync-schedules");
    assert_eq!(client.get(&list_url).send().await?.status(), 403);
    let listed: Value = client
        .get(&list_url)
        .header("authorization", http_get(2, &list_url))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(listed["schedules"][0]["id"], policy_id);
    assert_eq!(listed["schedules"][0]["dispatches"], 1);
    assert_eq!(
        post_json(
            &client,
            2,
            &format!("{B}/admin/sync-schedules/{policy_id}/enabled"),
            &json!({"enabled":false})
        )
        .await?
        .status(),
        200
    );
    receiver
        .pool
        .get()
        .await?
        .execute(
            "UPDATE sync_schedules SET next_run=clock_timestamp()-interval '1 second' WHERE id=$1",
            &[&hex::decode(policy_id)?],
        )
        .await?;
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(
        receiver.sync_schedules().await?[0]["dispatches"],
        1,
        "disabled schedule dispatched again"
    );
    std::fs::write(
        root.join("results/sync-schedules-http.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["operator-only policy creation and inspection","explicit background policy imports newly published profile","actual ticker dispatches without manual enqueue","disabled policy remains idle even when due","existing data remains hosted"],"scope":"synthetic loopback two-node PostgreSQL/strfry; no recurring external service configured"}),
        )?,
    )?;
    drop(_a);
    // The receiver is now the only online copy: original delivery must stay
    // local rather than silently depending on the former source endpoint.
    let surviving_original = client
        .get(format!("{B}/originals/{}", auth::hash(&image)))
        .send()
        .await?;
    assert_eq!(surviving_original.status(), 200);
    assert_eq!(surviving_original.bytes().await?.as_ref(), image);
    let fetched = client
        .get(format!(
            "{B}/library/blobs/{}",
            manga_node::auth::hash(&library_chunks[1])
        ))
        .send()
        .await?
        .bytes()
        .await?;
    assert_eq!(&fetched[..], library_chunks[1]);
    std::fs::write(
        root.join("results/library-inventory.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["persisted old inventory ID resumes after worker restart","retained signed pointer routes to current author head","complete current root and chunks retained locally","repeat inventory converges with zero missing IDs","cancelled staging cannot publish a new head","current progress readable after source shutdown","exact original still served locally after source shutdown"],"scope":"synthetic two-node PostgreSQL/strfry; root churn can require a new bounded reconciliation"}),
        )?,
    )?;
    std::fs::write(
        root.join("results/postgres-http-sync-jobs.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"chapters":5,"common_synthetic_inventory_ids":120000,"automatic_partition_jobs":17,"checks":["HTTP job enqueue and active deduplication","queued job persists across process restart","real worker killed after original transfer before publication commit","restart does not steal live lease","expired lease reclaimed and old authority fenced","persisted missing-ID work resumes and completes","actual chapters projected with exact local original", "120000 common equal-time metadata IDs trigger automatic16-way split and only the missing real chapter is imported","exact-byte physical quota survives recovery and rejects a new object until expanded","cancellation retains already admitted content"],"limits":["synthetic loopback originals","forced fixture lease expiry speeds recovery test","large inventory uses injected common metadata, with one additional genuine chapter"]}),
        )?,
    )?;
    Ok(())
}
