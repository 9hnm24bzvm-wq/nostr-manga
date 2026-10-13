mod support;
use anyhow::{Context, Result};
use manga_node::{
    accounts::StageOutcome,
    auth,
    db::{DbConfig, Repository},
};
use std::time::Duration;
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run scripts/test-postgres.sh"]
async fn account_replacement_authority_and_ordering() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture database required")?;
    let parsed: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(
        parsed.get_dbname() == Some("manga_test"),
        "fixture database only"
    );
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1_048_576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let secret = format!("{:064x}", 91);
    let created_at = auth::now() - 1;
    let mut events: Vec<_> = (0..32)
        .map(|n| {
            support::sign_at(
                91,
                0,
                vec![],
                format!("{{\"name\":\"reader{n}\"}}"),
                created_at,
            )
        })
        .collect();
    events.sort_by(|a, b| a.id.cmp(&b.id));
    let author: [u8; 32] = hex::decode(&events[0].pubkey)?.try_into().unwrap();
    assert!(repo.account_permit(author).await.is_err());
    repo.set_account_admission(author, true).await?;
    let permit = repo.account_permit(author).await?;
    let oldest = events.last().unwrap();
    assert_eq!(
        repo.stage_account_metadata(&permit, oldest).await?,
        StageOutcome::Accepted
    );
    let old_id: [u8; 32] = hex::decode(&oldest.id)?.try_into().unwrap();
    assert!(repo.visible_event(old_id).await?.is_some()); // Warm Moka before replacing.
    let mut tasks = vec![];
    for event in events.clone() {
        let r = repo.clone();
        let p = permit.clone();
        tasks.push(tokio::spawn(async move {
            r.stage_account_metadata(&p, &event).await
        }));
    }
    for task in tasks {
        task.await??;
    }
    assert!(repo.visible_event(old_id).await?.is_none());
    let winner = &events[0];
    assert_eq!(
        repo.stage_account_metadata(&permit, winner).await?,
        StageOutcome::Current
    );
    assert_eq!(
        repo.stage_account_metadata(&permit, oldest).await?,
        StageOutcome::Superseded
    );
    for event in &events {
        let event_id: [u8; 32] = hex::decode(&event.id)?.try_into().unwrap();
        let visible = repo.visible_event(event_id).await?;
        assert_eq!(visible.is_some(), event.id == winner.id);
        if let Some(visible) = visible {
            assert_eq!(
                serde_json::to_value(visible.as_ref())?,
                serde_json::to_value(event)?
            );
        }
    }
    repo.set_account_admission(author, false).await?;
    repo.set_account_admission(author, true).await?;
    assert!(repo.stage_account_metadata(&permit, winner).await.is_err()); // ABA epoch fencing.
    let fresh = repo.account_permit(author).await?;
    assert_eq!(
        repo.stage_account_metadata(&fresh, winner).await?,
        StageOutcome::Current
    );
    let pointer = auth::sign(&secret, 11999, vec![], "{}".into()).map_err(anyhow::Error::msg)?;
    assert!(repo.stage_account_metadata(&fresh, &pointer).await.is_err());
    let stranger = auth::sign(&format!("{:064x}", 892), 3, vec![], String::new())
        .map_err(anyhow::Error::msg)?;
    assert!(
        repo.stage_account_metadata(&fresh, &stranger)
            .await
            .is_err()
    );
    let invalid = auth::sign(&secret, 0, vec![], "[]".into()).map_err(anyhow::Error::msg)?;
    assert!(repo.stage_account_metadata(&fresh, &invalid).await.is_err());
    // Drain the versioned replacement sequence through real strfry.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let dir = tempfile::tempdir()?;
    let backend = manga_node::backend::Backend::open(
        repo.clone(),
        manga_node::backend::BackendConfig {
            binary: root.join("vendor/strfry/strfry"),
            data_dir: dir.path().into(),
            timeout_seconds: 10,
            map_bytes: 64 * 1024 * 1024,
        },
    )
    .await?;
    let worker = tokio::spawn(backend.run());
    let drained = support::drain_backend(&repo).await;
    worker.abort();
    let _ = worker.await;
    drained?;
    let winner_id: [u8; 32] = hex::decode(&winner.id)?.try_into().unwrap();
    assert!(repo.projection_complete(winner_id).await?);
    let output = std::process::Command::new(root.join("vendor/strfry/strfry"))
        .arg("--config")
        .arg(dir.path().join("strfry.conf"))
        .arg("scan")
        .arg(serde_json::json!({"authors":[winner.pubkey],"kinds":[0]}).to_string())
        .output()?;
    anyhow::ensure!(output.status.success(), "strfry scan failed");
    let persisted: Vec<auth::Event> = output
        .stdout
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
        .map(serde_json::from_slice)
        .collect::<Result<_, _>>()?;
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].id, winner.id);
    println!(
        "32 concurrent same-clock updates select lower ID; cached predecessor hidden; replay idempotent; revocation ABA fenced; library bypass rejected"
    );
    Ok(())
}
