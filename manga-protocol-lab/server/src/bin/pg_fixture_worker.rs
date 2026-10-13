//! Isolated integration-test worker. Refuses non-fixture databases.
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    sync_jobs::SyncSpec,
};
use serde_json::json;
use std::time::Duration;
#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    let parsed: tokio_postgres::Config = dsn.parse()?;
    ensure!(
        parsed.get_dbname() == Some("manga_test"),
        "fixture database required"
    );
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 2,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1_048_576,
        plaintext_local: true,
    })
    .await?;
    let mode = std::env::args().nth(1).context("worker mode required")?;
    if mode == "event" {
        let e = auth::sign(
            &format!("{:064x}", 1),
            1,
            vec![],
            "multi-process fixture".into(),
        )
        .map_err(anyhow::Error::msg)?;
        println!("{}", serde_json::to_string(&e)?);
        return Ok(());
    }
    if mode == "seed-jobs" {
        repo.set_peer([0x11; 32], "http://127.0.0.1:19999", true)
            .await?;
        for i in 0..32 {
            repo.enqueue_sync(SyncSpec {
                inventory: "chapters".into(),
                peer: "11".repeat(32),
                filter: format!("pages>={i}"),
                fallbacks: vec![],
            })
            .await?;
        }
        println!("{}", json!({"seeded":32}));
        return Ok(());
    }
    if mode == "work" {
        let mut ids = vec![];
        while let Some(work) = repo.claim_sync().await? {
            tokio::time::sleep(Duration::from_millis(5)).await;
            repo.save_missing(&work, vec![]).await?;
            repo.finish_sync_quantum(&work).await?;
            ensure!(
                repo.sync_status(work.batch).await?["state"] == "completed",
                "sync batch incomplete"
            );
            ids.push(hex::encode(work.batch));
        }
        println!("{}", json!({"completed":ids}));
        return Ok(());
    }
    let event: auth::Event =
        serde_json::from_slice(&std::fs::read(std::env::var("MANGA_TEST_EVENT_PATH")?)?)?;
    let id: [u8; 32] = hex::decode(&event.id)?.try_into().unwrap();
    if mode == "write" {
        println!(
            "{}",
            json!({"created":repo.stage_regular_event(&event).await?})
        );
        return Ok(());
    }
    if mode == "read" {
        let visible = repo
            .visible_event(id)
            .await?
            .context("durable event missing")?;
        ensure!(
            serde_json::to_value(visible.as_ref())? == serde_json::to_value(&event)?,
            "fresh process returned changed event"
        );
        println!("{}", json!({"visible":true}));
        return Ok(());
    }
    if mode == "cache-outage" {
        let cached = repo
            .visible_event(id)
            .await?
            .context("cache fixture missing")?;
        let pg = std::env::var("MANGA_TEST_PG_CTL")?;
        let dir = std::env::var("MANGA_TEST_PG_DATA")?;
        let status = std::process::Command::new(&pg)
            .args(["-D", &dir, "-m", "immediate", "-w", "stop"])
            .stdout(std::process::Stdio::null())
            .status()?;
        ensure!(status.success(), "fixture crash-stop failed");
        ensure!(
            repo.visible_event(id).await.is_err(),
            "cached body bypassed failed authoritative database"
        );
        let log = std::env::var("MANGA_TEST_PG_LOG")?;
        let status = std::process::Command::new(&pg)
            .args(["-D", &dir, "-l", &log, "-w", "start"])
            .stdout(std::process::Stdio::null())
            .status()?;
        ensure!(status.success(), "fixture restart failed");
        let mut recovered = None;
        for _ in 0..20 {
            match repo.visible_event(id).await {
                Ok(Some(e)) => {
                    recovered = Some(e);
                    break;
                }
                _ => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
        let recovered = recovered.context("pool did not recover after PostgreSQL restart")?;
        ensure!(
            serde_json::to_value(cached.as_ref())? == serde_json::to_value(recovered.as_ref())?,
            "expected same immutable event after authoritative recovery"
        );
        println!(
            "{}",
            json!({"outage_failed_closed":true,"pool_recovered":true})
        );
        return Ok(());
    }
    anyhow::bail!("unknown fixture mode")
}
