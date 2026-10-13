mod support;
use anyhow::{Context, Result};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    publication::Publications,
    upload::{StorageConfig, UploadStore},
};
use serde_json::json;
use std::time::Duration;
use support::*;
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL"]
async fn author_deletion_fences_reordered_and_concurrent_events() -> Result<()> {
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
    repo.set_account_admission(id(&public(21)), true).await?;
    let permit = repo.account_permit(id(&public(21))).await?;
    for n in 0..32 {
        let event = sign_at(21, 1, vec![], format!("synthetic race{n}"), auth::now());
        let delete = sign_at(
            21,
            5,
            vec![vec!["e".into(), event.id.clone()]],
            String::new(),
            auth::now(),
        );
        let (stage, deletion) = tokio::join!(
            repo.stage_regular_event(&event),
            repo.delete_ordinary(&permit, &delete)
        );
        deletion?;
        assert!(repo.visible_event(id(&delete.id)).await?.is_none());
        assert!(
            repo.ordinary_deletion_request(id(&delete.id))
                .await?
                .is_some()
        );
        let _ = stage;
        assert!(repo.visible_event(id(&event.id)).await?.is_none());
        assert!(
            repo.stage_regular_event(&event).await.is_err(),
            "deleted event replay admitted"
        );
    }
    repo.bind_instance(id(&public(11)), id(&public(21))).await?;
    let dir = tempfile::tempdir()?;
    let store = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.path().into(),
            volume: "deletions".into(),
            physical_bytes: 1048576,
            staging_bytes: 1048576,
            max_blob_bytes: 1048576,
            upload_slots: 2,
            reclamation_slots: 4,
            credit_lanes: 16,
            formats: vec!["image/png".into()],
        },
        id(&public(21)),
        id(&public(11)),
    )
    .await?;
    let image = png();
    store.put(id(&auth::hash(&image)), &image).await?;
    let manifest = cbor(
        json!({"version":1,"series":"ab".repeat(32),"language":"en","chapter":"1","pages":[page(&image)]}),
    );
    store.put(id(&auth::hash(&manifest)), &manifest).await?;
    let chapter = publication(21, 1001, &manifest);
    Publications::new(repo.clone(), store)
        .publish(&chapter)
        .await?;
    let request = sign_at(
        21,
        5,
        vec![vec!["e".into(), chapter.id.clone()]],
        "Local moderation still required".into(),
        auth::now(),
    );
    assert_eq!(repo.delete_ordinary(&permit, &request).await?, 0);
    assert!(repo.visible_event(id(&chapter.id)).await?.is_some());
    let foreign = sign_at(22, 1, vec![], "foreign post".into(), auth::now());
    repo.stage_regular_event(&foreign).await?;
    let request = sign_at(
        21,
        5,
        vec![vec!["e".into(), foreign.id.clone()]],
        String::new(),
        auth::now(),
    );
    assert_eq!(repo.delete_ordinary(&permit, &request).await?, 0);
    assert!(repo.visible_event(id(&foreign.id)).await?.is_some());
    let request2 = sign_at(
        21,
        5,
        vec![vec!["e".into(), request.id.clone()]],
        String::new(),
        auth::now(),
    );
    repo.delete_ordinary(&permit, &request2).await?;
    assert!(
        repo.ordinary_deletion_request(id(&request.id))
            .await?
            .is_some()
    );
    repo.set_account_admission(id(&public(21)), false).await?;
    assert!(repo.delete_ordinary(&permit, &request).await.is_err());
    for deletion in [&request, &request2] {
        assert!(repo.visible_event(id(&deletion.id)).await?.is_none());
        assert!(
            repo.ordinary_deletion_request(id(&deletion.id))
                .await?
                .is_some()
        );
    }
    std::fs::write(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../results/ordinary-deletion-races.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"concurrent_reordered_event_cases":32,"checks":["author-qualified exact tombstones prevent late replay","protected own chapter remains hosted pending moderation","foreign author event unaffected","deletion request cannot delete a deletion request","stale permission denied","deletion requests remain available only through deletion lifecycle"],"scope":"isolated PostgreSQL with real protected publication bytes and public lifecycle outcomes"}),
        )?,
    )?;
    Ok(())
}
