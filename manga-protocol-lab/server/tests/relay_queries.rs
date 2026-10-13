mod support;
use anyhow::{Context, Result};
use manga_node::{
    auth,
    backend::{Backend, BackendConfig},
    db::{DbConfig, Repository},
    relay_query::RelayFilter,
};
use serde_json::json;
use std::time::Duration;
use support::*;
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL and real strfry"]
async fn guarded_history_filters_and_notifications() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(cfg.get_dbname() == Some("manga_test"), "fixture only");
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let feed = manga_node::relay_feed::Feed::new();
    let feed_worker = tokio::spawn(feed.clone().run(repo.clone()));
    tokio::time::timeout(Duration::from_secs(5), async {
        while !feed.ready.load(std::sync::atomic::Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    let mut receiver = feed.sender.subscribe();
    repo.set_account_admission(id(&public(21)), true).await?;
    let permit = repo.account_permit(id(&public(21))).await?;
    let old = sign_at(
        21,
        1,
        vec![vec!["t".into(), "manga".into()]],
        "old".into(),
        auth::now() - 1,
    );
    let new = sign_at(
        21,
        1,
        vec![vec!["t".into(), "anime".into()]],
        "new".into(),
        auth::now(),
    );
    for event in [&old, &new] {
        repo.stage_personal_note(&permit, event).await?;
        assert!(
            repo.relay_event(id(&event.id)).await?.is_none(),
            "pending projection leaked"
        );
    }
    let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let directory = tempfile::tempdir()?;
    let backend = Backend::open(
        repo.clone(),
        BackendConfig {
            binary: project.join("vendor/strfry/strfry"),
            data_dir: directory.path().into(),
            timeout_seconds: 10,
            map_bytes: 67_108_864,
        },
    )
    .await?;
    let worker = tokio::spawn(backend.run());
    let mut announced = std::collections::BTreeSet::new();
    for _ in 0..2 {
        announced.insert(
            tokio::time::timeout(Duration::from_secs(10), receiver.recv())
                .await??
                .context("unexpected feed discontinuity")?,
        );
    }
    assert_eq!(
        announced,
        std::collections::BTreeSet::from([id(&old.id), id(&new.id)])
    );
    let f = |v| RelayFilter::parse(&v).unwrap();
    assert_eq!(
        repo.relay_history(&[f(json!({"limit":1}))]).await?[0].id,
        new.id
    );
    assert_eq!(
        repo.relay_history(&[f(json!({"#t":["manga"]}))]).await?[0].id,
        old.id
    );
    assert_eq!(
        repo.relay_history(&[f(json!({"#t":["manga"]})), f(json!({"ids":[old.id]}))])
            .await?
            .len(),
        1
    );
    assert!(
        repo.relay_history(&[f(json!({"limit":0}))])
            .await?
            .is_empty()
    );
    assert!(
        f(json!({"limit":0})).matches(&new),
        "limit must not apply to live events"
    );
    // A forged index candidate cannot bypass exact tag checks.
    repo.pool
        .get()
        .await?
        .execute(
            "INSERT INTO relay_tags(event_id,name,value_hash) VALUES($1,'t',md5('fake'))",
            &[&id(&old.id).as_slice()],
        )
        .await?;
    assert!(
        repo.relay_history(&[f(json!({"#t":["fake"]}))])
            .await?
            .is_empty()
    );
    let deletion = sign_at(
        21,
        5,
        vec![vec!["e".into(), old.id.clone()]],
        String::new(),
        auth::now(),
    );
    repo.delete_ordinary(&permit, &deletion).await?;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), receiver.recv()).await??,
        Some(id(&deletion.id))
    );
    assert!(repo.relay_event(id(&old.id)).await?.is_none());
    assert_eq!(
        repo.relay_history(&[f(json!({"kinds":[5],"#e":[old.id]}))])
            .await?[0]
            .id,
        deletion.id
    );
    repo.set_account_admission(id(&public(21)), false).await?;
    assert!(
        repo.stage_personal_note(
            &permit,
            &sign_at(21, 1, vec![], "revoked".into(), auth::now())
        )
        .await
        .is_err()
    );
    for invalid in [
        json!({"ids":["ab"]}),
        json!({"kinds":[-1]}),
        json!({"limit":-1}),
        json!({"since":2,"until":1}),
        json!({"#word":["x"]}),
    ] {
        assert!(RelayFilter::parse(&invalid).is_err());
    }
    repo.pool.get().await?.simple_query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE application_name='manga-relay-feed'").await?;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), receiver.recv()).await??,
        None,
        "feed must signal discontinuity"
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        while !feed.ready.load(std::sync::atomic::Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    let note = sign_at(22, 1, vec![], "feed reconnected".into(), auth::now());
    repo.stage_regular_event(&note).await?;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), receiver.recv()).await??,
        Some(id(&note.id))
    );
    feed_worker.abort();
    assert!(feed_worker.await.unwrap_err().is_cancelled());
    assert!(!feed.ready.load(std::sync::atomic::Ordering::Acquire));
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), receiver.recv()).await??,
        None,
        "cancelling actual feed role signals discontinuity"
    );
    worker.abort();
    let _ = worker.await;
    Ok(())
}
