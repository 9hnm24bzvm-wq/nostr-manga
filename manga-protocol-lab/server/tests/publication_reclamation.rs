mod support;
use anyhow::{Context, Result};
use manga_node::{
    auth,
    backend::{Backend, BackendConfig},
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

async fn expire(repo: &Repository) -> Result<()> {
    repo.pool.get().await?.batch_execute("UPDATE publication_blob_grace SET grace_until=clock_timestamp()-interval '1 second';UPDATE blob_gc_queue SET due_at=clock_timestamp()-interval '1 second';UPDATE blob_intents SET staging_until=clock_timestamp()-interval '1 second'").await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and actual strfry"]
async fn only_acknowledged_approved_last_references_release_originals() -> Result<()> {
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
    let owner = id(&public(21));
    repo.bind_instance([22; 32], owner).await?;
    let dir = tempfile::tempdir()?;
    let store = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.path().join("store"),
            volume: "publicationgc".into(),
            physical_bytes: 1048576,
            staging_bytes: 1048576,
            max_blob_bytes: 1048576,
            upload_slots: 4,
            reclamation_slots: 4,
            credit_lanes: 16,
            formats: vec!["image/png".into()],
        },
        owner,
        [22; 32],
    )
    .await?;
    let publications = Publications::new(repo.clone(), store.clone());
    let image = png();
    let image_hash = id(&auth::hash(&image));
    let original = store.put(image_hash, &image).await?;
    let manifest = |chapter: &str| {
        cbor(
            json!({"version":1,"series":"ab".repeat(32),"chapter":chapter,"language":"en","pages":[page(&image)]}),
        )
    };
    let m1 = manifest("1");
    let m2 = manifest("2");
    let b1 = store.put(id(&auth::hash(&m1)), &m1).await?;
    let b2 = store.put(id(&auth::hash(&m2)), &m2).await?;
    let e1 = publication(21, 1001, &m1);
    let e2 = publication(21, 1001, &m2);
    publications.publish(&e1).await?;
    publications.publish(&e2).await?;
    let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let backend = Backend::open(
        repo.clone(),
        BackendConfig {
            binary: project.join("vendor/strfry/strfry"),
            data_dir: dir.path().join("backend"),
            timeout_seconds: 3,
            map_bytes: 67108864,
        },
    )
    .await?;
    let worker = tokio::spawn(backend.run());
    drain_backend(&repo).await?;
    let request = |target: &str| {
        sign_at(
            21,
            5,
            vec![vec!["e".into(), target.into()]],
            "Synthetic removal".into(),
            auth::now(),
        )
    };
    let d1 = request(&e1.id);
    repo.record_deletion_request(&d1).await?;
    assert!(
        !store.reclaim_library(original.key.clone()).await?,
        "author request alone cannot collect"
    );
    repo.approve_deletion(id(&d1.id), None).await?;
    drain_backend(&repo).await?;
    assert!(
        !store.reclaim_library(b1.key.clone()).await?,
        "grace protects newly retired manifest"
    );
    expire(&repo).await?;
    assert_eq!(
        store.reclaim_objects_page(true, false, false).await?,
        0,
        "library-only policy must not select publication candidates"
    );
    assert!(
        !store.reclaim_library(original.key.clone()).await?,
        "another retained chapter protects shared original"
    );
    assert_storage_used(
        &repo,
        &store,
        1_048_576,
        (m1.len() + m2.len() + image.len()) as u64,
    )
    .await?;
    assert!(store.reclaim_library(b1.key.clone()).await?);
    assert_storage_used(&repo, &store, 1_048_576, (m2.len() + image.len()) as u64).await?;
    let d2 = request(&e2.id);
    repo.record_deletion_request(&d2).await?;
    repo.approve_deletion(id(&d2.id), None).await?;
    drain_backend(&repo).await?;
    expire(&repo).await?;
    store.put(image_hash, &image).await?;
    assert!(
        !store.reclaim_library(original.key.clone()).await?,
        "fresh restaging must protect a publication being prepared"
    );
    // A failed physical cleanup must stay charged, back off, and not starve
    // a different eligible original. This fixture substitutes an empty directory.
    let h = hex::encode(b2.key.hash);
    let broken = dir
        .path()
        .join("store/objects")
        .join(&h[..2])
        .join(&h[2..4])
        .join(format!("{}.{}", h, hex::encode(b2.key.generation)));
    std::fs::remove_file(&broken)?;
    std::fs::create_dir(&broken)?;
    assert!(store.reclaim_objects_page(false, true, false).await? > 0);
    assert!(
        repo.blob_intent(b2.key.hash).await?.is_none(),
        "claimed failed object must remain unavailable"
    );
    assert_storage_used(&repo, &store, 1_048_576, (m2.len() + image.len()) as u64).await?;
    expire(&repo).await?;
    assert!(store.reclaim_objects_page(false, true, false).await? > 0);
    assert_storage_used(&repo, &store, 1_048_576, m2.len() as u64).await?;
    std::fs::remove_dir(&broken)?;
    repo.pool
        .get()
        .await?
        .execute(
            "UPDATE blob_reclaims SET retry_after=clock_timestamp()-interval '1 second'",
            &[],
        )
        .await?;
    assert!(store.reclaim_objects_page(false, false, false).await? > 0);
    assert!(repo.blob_intent(image_hash).await?.is_none());
    assert_storage_used(&repo, &store, 1_048_576, 0).await?;
    assert!(!store.reclaim_library(original.key.clone()).await?);
    let replacement = store.put(image_hash, &image).await?;
    assert_ne!(replacement.key, original.key);
    assert!(!store.reclaim_library(original.key.clone()).await?);
    assert_eq!(store.get(image_hash, owner).await?, image);
    store.put(id(&auth::hash(&m1)), &m1).await?;
    assert!(
        publications.publish(&e1).await.is_err(),
        "re-upload must not erase moderation tombstone"
    );
    assert_storage_used(&repo, &store, 1_048_576, (m1.len() + image.len()) as u64).await?;
    worker.abort();
    let _ = worker.await;
    std::fs::write(
        project.join("results/publication-reclamation.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["author request alone retains protected bytes","backend acknowledged approved removal starts grace","library-only policy excludes publication candidates","shared current chapter prevents cleanup","fresh original restaging prevents cleanup","last eligible reference frees exact physical charges","old cleanup cannot delete replacement generation","moderation tombstone survives physical cleanup and re-upload","failed cleanup retains charge and does not starve unrelated candidates","committed claims recover with new collection disabled","exact physical capacity recovered through upload admission"],"scope":"synthetic real PostgreSQL/strfry originals; grace advanced by fixture SQL; external nginx readers excluded"}),
        )?,
    )?;
    Ok(())
}
