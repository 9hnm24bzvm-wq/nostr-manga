//! Bulk-arranged signed metadata fixture, not media/publication admission.
mod support;
use anyhow::{Context, Result};
use manga_node::{
    db::{DbConfig, Repository},
    inventory::{FRAME_BYTES, ReconcileReply, storage},
};
use negentropy::Negentropy;
use serde_json::json;
use std::{
    collections::BTreeSet,
    process::{Child, Command},
    time::{Duration, Instant},
};
use support::*;
const ORIGIN: &str = "http://127.0.0.1:19520";
struct Nodes(Vec<Child>);
impl Drop for Nodes {
    fn drop(&mut self) {
        for child in &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
async fn frame(
    c: &reqwest::Client,
    port: u16,
    key: u8,
    body: serde_json::Value,
) -> Result<reqwest::Response> {
    let bytes = serde_json::to_vec(&body)?;
    Ok(c.post(format!("http://127.0.0.1:{port}/reconcile"))
        .header(
            "authorization",
            http(key, &format!("{ORIGIN}/reconcile"), &bytes),
        )
        .body(bytes)
        .send()
        .await?)
}
async fn close(c: &reqwest::Client, session: &str, key: u8) -> Result<()> {
    let r = frame(c, 19520, key, json!({"session":session,"close":true})).await?;
    anyhow::ensure!(r.status() == 200, "close rejected");
    Ok(())
}
async fn reconcile(c: &reqwest::Client, prefix: &str) -> Result<Vec<negentropy::Id>> {
    let mut neg = Negentropy::owned(storage(&[])?, FRAME_BYTES as u64)?;
    let mut message = neg.initiate()?;
    let mut session = None::<String>;
    let mut need = vec![];
    let mut have = vec![];
    for round in 0..256 {
        let r = frame(
            c,
            19520 + (round % 2) as u16,
            21,
            json!({"session":session,"prefix":prefix,"message":hex::encode(&message)}),
        )
        .await?;
        anyhow::ensure!(
            r.status() == 200,
            "reconciliation rejected: {}",
            r.text().await?
        );
        let reply: ReconcileReply = r.json().await?;
        session = Some(reply.session.clone());
        match neg.reconcile_with_ids(&hex::decode(reply.message)?, &mut have, &mut need)? {
            Some(next) => message = next,
            None => {
                close(c, &reply.session, 21).await?;
                return Ok(need);
            }
        }
    }
    anyhow::bail!("round budget")
}
// SQL arranges a large inventory; the expected IDs come only from signed inputs
// retained before insertion, never from querying the application's stored state.
async fn seed_events(
    db: &tokio_postgres::Client,
    sequence: std::ops::RangeInclusive<u32>,
) -> Result<BTreeSet<Vec<u8>>> {
    let mut expected = BTreeSet::new();
    let mut batch = Vec::with_capacity(512);
    let mut sequence = sequence.peekable();
    while let Some(number) = sequence.next() {
        let event = sign_at(1, 1001, vec![], format!("inventory fixture {number}"), 1000);
        assert!(expected.insert(hex::decode(&event.id)?));
        batch.push(serde_json::to_value(event)?);
        if batch.len() == 512 || sequence.peek().is_none() {
            db.execute(
                "INSERT INTO events(id,publisher,kind,created_at,raw_event,state) \
                 SELECT decode(value->>'id','hex'),decode(value->>'pubkey','hex'), \
                 (value->>'kind')::integer,(value->>'created_at')::bigint,value,'hosted' \
                 FROM jsonb_array_elements($1::jsonb)",
                &[&serde_json::Value::Array(std::mem::take(&mut batch))],
            )
            .await?;
        }
    }
    Ok(expected)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run scripts/test-postgres.sh"]
async fn durable_snapshots_and_equal_timestamp_partitions() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(cfg.get_dbname() == Some("manga_test"), "fixture required");
    let repo = Repository::connect(DbConfig {
        dsn: dsn.clone(),
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    for key in 21..=25 {
        repo.set_peer(
            hex::decode(public(key))?.try_into().unwrap(),
            "http://127.0.0.1:19999",
            true,
        )
        .await?;
    }
    let source = repo.pool.get().await?;
    let old_ids = seed_events(&source, 1..=100000).await?;
    let dir = tempfile::tempdir()?;
    let key = dir.path().join("key");
    write_key(&key, 11)?;
    let mut nodes = Nodes(vec![]);
    for port in [19520, 19521] {
        let path = dir.path().join(format!("{port}.json"));
        std::fs::write(
            &path,
            serde_json::to_vec(
                &json!({"bind":format!("127.0.0.1:{port}"),"base_url":ORIGIN,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true}),
            )?,
        )?;
        let log = std::fs::File::create(dir.path().join(format!("{port}.log")))?;
        nodes.0.push(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", path.to_str().unwrap()])
                .env("MANGA_DATABASE_URL", &dsn)
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        );
    }
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    for port in [19520, 19521] {
        let mut ready = false;
        for _ in 0..100 {
            if c.get(format!("http://127.0.0.1:{port}/"))
                .send()
                .await
                .is_ok()
            {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        assert!(ready);
    }
    let started = Instant::now();
    let mut neg = Negentropy::owned(storage(&[])?, FRAME_BYTES as u64)?;
    let first = hex::encode(neg.initiate()?);
    let r = frame(&c, 19520, 21, json!({"message":first})).await?;
    assert_eq!(r.status(), 200);
    let first: ReconcileReply = r.json().await?;
    // Change live state after snapshot construction, then continue on a different process.
    let new_ids = seed_events(&source, 100001..=120001).await?;
    let removed_id = old_ids.first().unwrap().clone();
    source
        .execute(
            "UPDATE events SET state='removed' WHERE id=$1",
            &[&removed_id],
        )
        .await?;
    assert_eq!(
        frame(
            &c,
            19521,
            21,
            json!({"session":first.session,"prefix":"a","message":""})
        )
        .await?
        .status(),
        400
    );
    assert_eq!(
        frame(
            &c,
            19521,
            21,
            json!({"session":first.session,"inventory":"series","message":""})
        )
        .await?
        .status(),
        400,
        "cross-inventory session reuse allowed"
    );
    // Another approved peer cannot release this peer's snapshot.
    close(&c, &first.session, 22).await?;
    let mut need = vec![];
    let mut have = vec![];
    let mut reply = first;
    let mut rounds = 1;
    loop {
        let next = neg.reconcile_with_ids(&hex::decode(&reply.message)?, &mut have, &mut need)?;
        let Some(message) = next else {
            break;
        };
        let r = frame(
            &c,
            19520 + (rounds % 2) as u16,
            21,
            json!({"session":reply.session,"message":hex::encode(message)}),
        )
        .await?;
        assert_eq!(r.status(), 200);
        reply = r.json().await?;
        rounds += 1;
        assert!(rounds <= 256);
    }
    close(&c, &reply.session, 21).await?;
    assert_eq!(
        need.iter()
            .map(|v| v.as_bytes().to_vec())
            .collect::<BTreeSet<_>>(),
        old_ids
    );
    let stable_ms = started.elapsed().as_millis();
    let mut empty = Negentropy::owned(storage(&[])?, FRAME_BYTES as u64)?;
    let initial = hex::encode(empty.initiate()?);
    assert_eq!(
        frame(&c, 19520, 21, json!({"message":initial}))
            .await?
            .status(),
        413,
        "oversized root requires explicit partitioning"
    );
    let mut combined = BTreeSet::new();
    let mut partition_sizes = vec![];
    for prefix in "0123456789abcdef".chars() {
        let prefix = prefix.to_string();
        let rows = reconcile(&c, &prefix).await?;
        partition_sizes.push(rows.len());
        for id in rows {
            assert!(hex::encode(id.as_bytes()).starts_with(&prefix));
            assert!(combined.insert(id.as_bytes().to_vec()));
        }
    }
    let mut current = old_ids;
    current.extend(new_ids);
    assert!(current.remove(&removed_id));
    assert_eq!(combined, current);
    assert_eq!(combined.len(), 120000);
    let mut sessions = vec![];
    for key in 21..=24 {
        for _ in 0..2 {
            let r = frame(&c, 19520, key, json!({"prefix":"a","message":initial})).await?;
            assert_eq!(r.status(), 200);
            sessions.push((key, r.json::<ReconcileReply>().await?.session));
        }
    }
    assert_eq!(
        frame(&c, 19521, 21, json!({"prefix":"a","message":initial}))
            .await?
            .status(),
        400,
        "per-peer cap shared across processes"
    );
    assert_eq!(
        frame(&c, 19521, 25, json!({"prefix":"a","message":initial}))
            .await?
            .status(),
        400,
        "global cap shared across processes"
    );
    for (key, session) in sessions {
        close(&c, &session, key).await?;
    }
    let r = frame(&c, 19520, 21, json!({"prefix":"a","message":initial}))
        .await?
        .json::<ReconcileReply>()
        .await?;
    source
        .execute(
            "UPDATE inventory_snapshot_slots SET expires_at='-infinity' WHERE session_id=$1",
            &[&hex::decode(&r.session)?],
        )
        .await?;
    assert_eq!(
        frame(
            &c,
            19521,
            21,
            json!({"session":r.session,"prefix":"a","message":initial})
        )
        .await?
        .status(),
        400,
        "expired cache cannot authorize a frame"
    );
    let r = frame(&c, 19520, 21, json!({"prefix":"a","message":initial}))
        .await?
        .json::<ReconcileReply>()
        .await?;
    repo.set_peer(
        hex::decode(public(21))?.try_into().unwrap(),
        "http://127.0.0.1:19999",
        false,
    )
    .await?;
    assert_eq!(
        frame(
            &c,
            19521,
            21,
            json!({"session":r.session,"prefix":"a","message":initial})
        )
        .await?
        .status(),
        403
    );
    repo.set_peer(
        hex::decode(public(21))?.try_into().unwrap(),
        "http://127.0.0.1:19999",
        true,
    )
    .await?;
    assert_eq!(
        frame(
            &c,
            19521,
            21,
            json!({"session":r.session,"prefix":"a","message":initial})
        )
        .await?
        .status(),
        400,
        "restored approval does not revive old epoch"
    );

    repo.pool
        .get()
        .await?
        .execute(
            "UPDATE inventory_snapshot_slots SET expires_at='-infinity'",
            &[],
        )
        .await?;
    let r = frame(&c, 19520, 21, json!({"prefix":"a","message":initial}))
        .await?
        .json::<ReconcileReply>()
        .await?;
    // Simulate the epoch transition committed by a generation-fenced quarantine.
    repo.pool
        .get()
        .await?
        .execute(
            "UPDATE storage_health_epoch SET epoch=epoch+1 WHERE singleton",
            &[],
        )
        .await?;
    assert_eq!(
        frame(
            &c,
            19521,
            21,
            json!({"session":r.session,"prefix":"a","message":initial})
        )
        .await?
        .status(),
        400,
        "a cached snapshot cannot survive storage-health change"
    );
    assert_eq!(
        frame(&c, 19520, 21, json!({"prefix":"a","message":initial}))
            .await?
            .status(),
        200
    );
    std::fs::write(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../results/postgres-http-inventory.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"scope":"Bulk-arranged signed metadata with input-owned expected IDs, not signature/media admission or capacity certification","stable_snapshot_items":100000,"stable_snapshot_rounds":rounds,"stable_snapshot_milliseconds":stable_ms,"equal_timestamp_live_items":120000,"partition_sizes":partition_sizes,"checks":["actual Negentropy across alternating API processes","snapshot stable across live insert/removal","16 event-ID-prefix partitions cover oversized equal-timestamp inventory exactly","foreign peer cannot close another session","shared per-peer/global caps","expiry and revoked/reapproved epoch reject cached state","storage-health epoch invalidates cached snapshots across API processes"]}),
        )?,
    )?;
    Ok(())
}
