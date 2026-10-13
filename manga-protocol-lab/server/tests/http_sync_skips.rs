//! A real wrapper/strfry peer with fixture-seeded future/invalid signed records.
//! Normal supported content is admitted through HTTP; seeding models other code.
mod support;
use anyhow::{Context, Result, ensure};
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
const A: &str = "http://127.0.0.1:19585";
const B: &str = "http://127.0.0.1:19586";
struct Node(Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
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
async fn ready(c: &reqwest::Client, url: &str) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if c.get(url).send().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    Ok(())
}
async fn seed(source: &Repository, event: &auth::Event, manifest: Option<&[u8]>) -> Result<()> {
    event.verify().map_err(anyhow::Error::msg)?;
    let mh = manifest.map(|b| id(&auth::hash(b)).to_vec());
    let mut c = source.pool.get().await?;
    let tx = c.transaction().await?;
    tx.execute("INSERT INTO events(id,publisher,kind,created_at,raw_event,state,manifest,language) VALUES($1,$2,$3,$4,$5,'hosted',$6,'en')", &[&id(&event.id).as_slice(),&id(&event.pubkey).as_slice(),&(event.kind as i32),&(event.created_at as i64),&serde_json::to_value(event)?,&mh]).await?;
    if let Some(hash) = mh {
        tx.execute("INSERT INTO publication_blobs(event_id,hash,generation,mime) SELECT $1,hash,generation,'application/cbor' FROM blob_intents WHERE hash=$2", &[&id(&event.id).as_slice(),&hash]).await?;
    }
    tx.execute(
        "INSERT INTO backend_outbox(event_id,action,payload) VALUES($1,'import',$2)",
        &[&id(&event.id).as_slice(), &serde_json::to_value(event)?],
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
async fn finish(c: &reqwest::Client, batch: &str) -> Result<Value> {
    let url = format!("{B}/admin/sync-jobs/{batch}");
    let mut last = Value::Null;
    let outcome = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let s: Value = c
                .get(&url)
                .header("authorization", http_get(2, &url))
                .send()
                .await?
                .json()
                .await?;
            last = s.clone();
            if matches!(
                s["state"].as_str(),
                Some("completed" | "completed_with_skips")
            ) {
                break Ok::<_, anyhow::Error>(s);
            }
            ensure!(s["state"] != "failed", "batch failed: {s}");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await;
    match outcome {
        Ok(result) => result,
        Err(_) => anyhow::bail!("batch {batch} timed out: {last}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated PostgreSQL and two real strfry-backed nodes"]
async fn mixed_versions_and_policy_rejections_do_not_cancel_other_items() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    ensure!(
        cfg.get_dbname() == Some("manga_test") && dsn.contains("dbname=manga_test"),
        "fixture only"
    );
    let source = repo(dsn.clone()).await?;
    source.migrate().await?;
    source
        .pool
        .get()
        .await?
        .batch_execute("CREATE DATABASE manga_skip_receiver")
        .await?;
    let bdsn = dsn.replace("dbname=manga_test", "dbname=manga_skip_receiver");
    let receiver = repo(bdsn.clone()).await?;
    receiver.migrate().await?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let directory = tempfile::tempdir()?;
    let retained = directory.keep();
    let dir = retained.as_path();
    eprintln!("retained mixed-version fixture: {}", dir.display());
    let mut nodes = vec![];
    for (n, origin) in [A, B].iter().enumerate() {
        let base = dir.join(format!("node{n}"));
        std::fs::create_dir_all(&base)?;
        let key = base.join("key");
        write_key(&key, 11 + n as u8)?;
        let path = base.join("config.json");
        std::fs::write(
            &path,
            serde_json::to_vec(
                &json!({"bind":format!("127.0.0.1:{}",19585+n),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1+n as u8),"database_plaintext_local":true,"sync_worker_enabled":n==1,"storage":{"path":base.join("objects"),"volume":format!("skip{n}"),"physical_bytes":4194304,"staging_bytes":4194304,"max_blob_bytes":1048576,"formats":if n==0{vec!["image/png","image/jpeg"]}else{vec!["image/png"]}},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":base.join("backend"),"map_bytes":67108864}}),
            )?,
        )?;
        let log = std::fs::File::create(base.join("node.log"))?;
        nodes.push(Node(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", path.to_str().unwrap()])
                .env("MANGA_DATABASE_URL", if n == 0 { &dsn } else { &bdsn })
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        ));
    }
    let c = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()?;
    ready(&c, A).await?;
    ready(&c, B).await?;
    for (origin, admin, peer, endpoint) in [(A, 1, 12, B), (B, 2, 11, A)] {
        let r = post_json(
            &c,
            admin,
            &format!("{origin}/admin/peers"),
            &json!({"key":public(peer),"endpoint":endpoint,"allowed":true}),
        )
        .await?;
        ensure!(r.status().is_success(), "peer admission");
    }
    let png = png();
    upload_to(&c, 1, A, &png).await?;
    let mut good = vec![];
    for chapter in ["good-1", "good-2"] {
        let bytes = cbor(
            json!({"version":1,"series":"ab".repeat(32),"language":"en","chapter":chapter,"pages":[page(&png)]}),
        );
        upload_to(&c, 1, A, &bytes).await?;
        let event = publication(1, 1001, &bytes);
        let r = post_json(
            &c,
            1,
            &format!("{A}/publications"),
            &serde_json::to_value(&event)?,
        )
        .await?;
        ensure!(r.status().is_success(), "normal publication");
        wait_projected(&c, A, &event.id).await?;
        good.push(event);
    }
    // The source keeps a chapter which this receiver explicitly removed.
    let bytes = cbor(
        json!({"version":1,"series":"ab".repeat(32),"language":"en","chapter":"locally-removed","pages":[page(&png)]}),
    );
    upload_to(&c, 1, A, &bytes).await?;
    let removed = publication(1, 1001, &bytes);
    ensure!(
        post_json(
            &c,
            1,
            &format!("{A}/publications"),
            &serde_json::to_value(&removed)?
        )
        .await?
        .status()
        .is_success(),
        "source removed-fixture publication"
    );
    wait_projected(&c, A, &removed.id).await?;
    ensure!(
        post_json(
            &c,
            2,
            &format!("{B}/admin/sync"),
            &json!({"peer":public(11),"event":removed.id,"filter":"","fallbacks":[]})
        )
        .await?
        .status()
        .is_success(),
        "initial receiver copy"
    );
    wait_projected(&c, B, &removed.id).await?;
    let deletion = auth::sign(
        &format!("{:064x}", 1),
        5,
        vec![vec!["e".into(), removed.id.clone()]],
        "local moderator fixture".into(),
    )
    .map_err(anyhow::Error::msg)?;
    assert_eq!(
        post_json(
            &c,
            2,
            &format!("{B}/deletion-requests"),
            &serde_json::to_value(&deletion)?
        )
        .await?
        .status(),
        202
    );
    let approval = post_json(
        &c,
        2,
        &format!("{B}/admin/deletions/{}/approve", deletion.id),
        &json!({}),
    )
    .await?;
    assert_eq!(approval.status(), 202);
    assert_eq!(approval.json::<Value>().await?["new_withdrawals"], 1);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if c.get(format!("{B}/events/{}", removed.id))
                .send()
                .await?
                .status()
                == 404
            {
                break Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    // A genuinely valid JPEG chapter, rejected only by the receiver's PNG policy.
    let mut jpeg = std::io::Cursor::new(vec![]);
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(2, 2, image::Rgb([31, 63, 95])))
        .write_to(&mut jpeg, image::ImageFormat::Jpeg)?;
    let jpeg = jpeg.into_inner();
    upload_to(&c, 1, A, &jpeg).await?;
    let bytes = cbor(
        json!({"version":1,"series":"ab".repeat(32),"language":"en","chapter":"jpeg","pages":[{"sha256":auth::hash(&jpeg),"size":jpeg.len(),"mime":"image/jpeg","width":2,"height":2}]}),
    );
    upload_to(&c, 1, A, &bytes).await?;
    let jpeg_event = publication(1, 1001, &bytes);
    ensure!(
        post_json(
            &c,
            1,
            &format!("{A}/publications"),
            &serde_json::to_value(&jpeg_event)?
        )
        .await?
        .status()
        .is_success(),
        "JPEG source publication"
    );
    wait_projected(&c, A, &jpeg_event.id).await?;
    let mut rejected = vec![jpeg_event, removed];
    // Force an unsupported event before both valid IDs in the ordered inventory.
    let minimum = good.iter().map(|e| e.id.as_str()).min().unwrap();
    let mut future = None;
    for nonce in 0..4096 {
        let event = auth::sign(
            &format!("{:064x}", 1),
            1001,
            vec![vec!["fixture".into(), nonce.to_string()]],
            json!({"manifest":"aa".repeat(32),"version":2}).to_string(),
        )
        .map_err(anyhow::Error::msg)?;
        if event.id.as_str() < minimum {
            future = Some(event);
            break;
        }
    }
    let future = future.context("fixture could not order future record")?;
    seed(&source, &future, None).await?;
    wait_projected(&c, A, &future.id).await?;
    rejected.push(future.clone());
    for (version, required, label) in [
        (2, vec![], "future-manifest"),
        (1, vec!["new-feature"], "required-feature"),
    ] {
        let bytes = cbor(
            json!({"version":version,"required":required,"series":"ab".repeat(32),"language":"en","chapter":label,"pages":[page(&png)]}),
        );
        upload_to(&c, 1, A, &bytes).await?;
        let e = publication(1, 1001, &bytes);
        seed(&source, &e, Some(&bytes)).await?;
        wait_projected(&c, A, &e.id).await?;
        rejected.push(e);
    }
    let mut bad_page = page(&png);
    bad_page["width"] = json!(3);
    let bytes = cbor(
        json!({"version":1,"series":"ab".repeat(32),"language":"en","chapter":"invalid-dimensions","pages":[bad_page]}),
    );
    upload_to(&c, 1, A, &bytes).await?;
    let invalid = publication(1, 1001, &bytes);
    seed(&source, &invalid, Some(&bytes)).await?;
    wait_projected(&c, A, &invalid.id).await?;
    rejected.push(invalid);
    let invalid_manifest_hash = auth::hash(&bytes);
    let direct = post_json(
        &c,
        2,
        &format!("{B}/admin/sync"),
        &json!({"peer":public(11),"event":future.id,"filter":"","fallbacks":[]}),
    )
    .await?;
    assert_eq!(direct.status(), 422);
    let direct: Value = direct.json().await?;
    assert_eq!(direct["category"], "unsupported");
    assert_eq!(direct["hosted"], false);
    // External queue pressure leaves exactly eight reservations available.
    // This is causal fault setup, never an outcome oracle.
    receiver
        .pool
        .get()
        .await?
        .execute(
            "UPDATE sync_queue_budget SET pending_items=pending_items+199992 WHERE singleton",
            &[],
        )
        .await?;
    let started: Value = post_json(
        &c,
        2,
        &format!("{B}/admin/sync-jobs"),
        &json!({"peer":public(11),"inventory":"chapters","filter":"","fallbacks":[]}),
    )
    .await?
    .json()
    .await?;
    let status = finish(&c, started["id"].as_str().context("batch")?).await?;
    assert_eq!(status["state"], "completed_with_skips");
    assert_eq!(status["acknowledged_items"], 8);
    assert_eq!(status["skipped_items"], 6);
    assert_eq!(status["skip_details"].as_array().unwrap().len(), 6);
    assert_eq!(status["skip_details_truncated"], false);
    for reason in [
        "unsupported:publication_schema",
        "unsupported:manifest_schema",
        "unsupported:required_features",
        "policy_rejected:local_format",
        "invalid:original",
        "policy_rejected:local_removal",
    ] {
        assert_eq!(status["skip_counts"][reason], 1, "{status}");
    }
    for e in &good {
        wait_projected(&c, B, &e.id).await?;
    }
    for e in &rejected {
        assert_eq!(
            c.get(format!("{B}/events/{}", e.id)).send().await?.status(),
            404
        );
    }
    // The invalid image was rejected after its manifest was staged. Its private
    // temporary transfer interest must be released, so owner deletion succeeds.
    let staged_url = format!("{B}/blobs/{invalid_manifest_hash}");
    assert_eq!(
        c.delete(&staged_url)
            .header(
                "authorization",
                blossom(2, B, "delete", &invalid_manifest_hash)
            )
            .send()
            .await?
            .status(),
        200
    );
    assert_eq!(
        c.get(format!("{B}/originals/{invalid_manifest_hash}"))
            .send()
            .await?
            .status(),
        404
    );
    // Six rejected chapters remain absent. Add two future records: the next
    // eight-item batch fits only if the first batch refunded every reservation.
    for n in 0..3 {
        let e = auth::sign(
            &format!("{:064x}", 1),
            1001,
            vec![vec!["fixture".into(), format!("refund-{n}")]],
            json!({"manifest":"cc".repeat(32),"version":2}).to_string(),
        )
        .map_err(anyhow::Error::msg)?;
        seed(&source, &e, None).await?;
        wait_projected(&c, A, &e.id).await?;
        if n == 1 {
            let batch: Value = post_json(
                &c,
                2,
                &format!("{B}/admin/sync-jobs"),
                &json!({"peer":public(11),"inventory":"chapters","filter":"","fallbacks":[]}),
            )
            .await?
            .json()
            .await?;
            let refunded = finish(&c, batch["id"].as_str().context("refund batch")?).await?;
            assert_eq!(refunded["acknowledged_items"], 8, "{refunded}");
            assert_eq!(refunded["skipped_items"], 8, "{refunded}");
        }
    }
    // Nine missing items exceed the same eight-credit ceiling. Releasing only
    // the external pressure lets this same queued job recover without reset.
    let batch: Value = post_json(
        &c,
        2,
        &format!("{B}/admin/sync-jobs"),
        &json!({"peer":public(11),"inventory":"chapters","filter":"","fallbacks":[]}),
    )
    .await?
    .json()
    .await?;
    let batch_id = batch["id"].as_str().context("pressure batch")?;
    let url = format!("{B}/admin/sync-jobs/{batch_id}");
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let state: Value = c
                .get(&url)
                .header("authorization", http_get(2, &url))
                .send()
                .await?
                .json()
                .await?;
            ensure!(
                !matches!(
                    state["state"].as_str(),
                    Some("failed" | "completed" | "completed_with_skips")
                ),
                "pressure must remain retryable: {state}"
            );
            if state["last_error"]
                .as_str()
                .is_some_and(|e| e.contains("global pending-item budget full"))
            {
                break Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await??;
    receiver
        .pool
        .get()
        .await?
        .execute(
            "UPDATE sync_queue_budget SET pending_items=pending_items-199992 WHERE singleton",
            &[],
        )
        .await?;
    let recovered = finish(&c, batch_id).await?;
    assert_eq!(recovered["acknowledged_items"], 9, "{recovered}");
    assert_eq!(recovered["skipped_items"], 9, "{recovered}");
    // Unknown library pointer versions also settle individually instead of retrying.
    for key in [3, 4] {
        let e = auth::sign(
            &format!("{key:064x}"),
            11999,
            vec![],
            json!({"manifest":"bb".repeat(32),"version":2}).to_string(),
        )
        .map_err(anyhow::Error::msg)?;
        seed(&source, &e, None).await?;
        wait_projected(&c, A, &e.id).await?;
    }
    let started: Value = post_json(
        &c,
        2,
        &format!("{B}/admin/sync-jobs"),
        &json!({"peer":public(11),"inventory":"libraries","filter":"","fallbacks":[]}),
    )
    .await?
    .json()
    .await?;
    let libraries = finish(&c, started["id"].as_str().unwrap()).await?;
    assert_eq!(libraries["state"], "completed_with_skips");
    assert_eq!(libraries["skipped_items"], 2);
    std::fs::write(
        root.join("results/http-sync-skips.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"chapters":status,"libraries":libraries,"checks":["unsupported event ordered before valid chapters","future pointer and manifest versions skipped","unknown required feature skipped","valid JPEG rejected only by local PNG policy","authenticated invalid dimensions skipped","local removal remains effective without poisoning batch","both supported chapters hosted and projected","direct sync returns explicit422 reason","unsupported library pointers settle independently","exact queue capacity refunds across skipped batches; pressure remains retryable and recovers; rejected staged manifest can be deleted"],"fixture_scope":"two real wrapper/strfry nodes; future/invalid signed records seeded to model another implementation"}),
        )?,
    )?;
    drop(nodes);
    Ok(())
}
