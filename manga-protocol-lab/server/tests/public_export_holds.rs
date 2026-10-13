mod support;
use anyhow::{Context, Result};
use manga_node::{
    auth,
    backend::{Backend, BackendConfig},
    db::{DbConfig, Repository},
    publication::Publications,
    serving::{ManagedServing, ServingConfig},
    upload::{StorageConfig, UploadStore},
};
use serde_json::json;
use std::{path::PathBuf, sync::Arc, time::Duration};
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
async fn external_reader_holds_block_refund_and_repair() -> Result<()> {
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
    assert!(
        repo.reserve_public_export(
            &original.key,
            owner,
            "publicationgc",
            "image/jpeg",
            store.public_volume_identity()
        )
        .await
        .is_err()
    );
    assert!(
        repo.reserve_public_export(
            &b1.key,
            owner,
            "publicationgc",
            "image/png",
            store.public_volume_identity()
        )
        .await
        .is_err(),
        "manifest cannot be exported as an original"
    );
    assert!(
        repo.reserve_public_export(
            &original.key,
            [99; 32],
            "publicationgc",
            "image/png",
            store.public_volume_identity()
        )
        .await
        .is_err()
    );
    assert!(
        repo.reserve_public_export(
            &original.key,
            owner,
            "publicationgc",
            "image/png",
            store.public_volume_identity()
        )
        .await?
    );
    assert!(
        !repo
            .reserve_public_export(
                &original.key,
                owner,
                "publicationgc",
                "image/png",
                store.public_volume_identity()
            )
            .await?
    );
    repo.pool
        .get()
        .await?
        .execute(
            "UPDATE public_export_holds SET created_at='2000-01-01' WHERE hash=$1",
            &[&image_hash.as_slice()],
        )
        .await?;
    assert!(repo.public_export_held(&original.key).await?);
    // Synthetic negative-health row isolates the repair inhibitor. No external
    // serving process is attached in this database guard test.
    repo.pool
        .get()
        .await?
        .execute(
            "INSERT INTO quarantined_blobs(hash,generation,reason) VALUES($1,$2,'audit_failed')",
            &[&image_hash.as_slice(), &original.key.generation.as_slice()],
        )
        .await?;
    repo.set_peer([23; 32], "http://127.0.0.1:19790", true)
        .await?;
    let before = (m1.len() + m2.len() + image.len()) as u64;
    assert_storage_used(&repo, &store, 1_048_576, before).await?;
    let error = store
        .repair_verified(
            original.clone(),
            image.clone(),
            repo.approved_peer([23; 32]).await?,
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("serving readers"));
    assert_storage_used(&repo, &store, 1_048_576, before).await?;
    repo.pool
        .get()
        .await?
        .execute(
            "DELETE FROM quarantined_blobs WHERE hash=$1",
            &[&image_hash.as_slice()],
        )
        .await?;
    // Positive and negative paths both use the real projection primitives.
    store.configure_public_export_role().await?;
    assert_eq!(store.reconcile_public_exports().await?.exported, 1);
    let public = dir.path().join("store/public");
    let hash = hex::encode(image_hash);
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = listener.local_addr()?.to_string();
    drop(listener);
    let serving = Arc::new(
        ManagedServing::prepare(ServingConfig {
            python: PathBuf::from("python3"),
            helper: project.join("src/serving_supervisor.py"),
            nginx: project.join("vendor/nginx-1.28.0/objs/nginx"),
            runtime: public.join(".serving"),
            public: public.clone(),
            bind: bind.clone(),
            owner,
            workers: 1,
            shutdown_seconds: 1,
        })
        .await?,
    );
    serving.activate().await?;
    let http = reqwest::Client::builder().no_proxy().build()?;
    let url = format!("http://{bind}/{hash}.png");
    assert_eq!(
        http.get(&url).send().await?.bytes().await?.as_ref(),
        image.as_slice()
    );
    let other = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.path().join("other-store"),
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
    assert_ne!(
        other.public_volume_identity(),
        store.public_volume_identity()
    );
    assert!(!other.withdraw_public_original(original.key.clone()).await?);
    assert!(other.drain_public_originals(serving.clone()).await.is_err());
    assert!(repo.public_export_held(&original.key).await?);
    assert_eq!(http.get(&url).send().await?.status(), 200);
    let deletion = sign_at(
        21,
        5,
        vec![
            vec!["e".into(), e1.id.clone()],
            vec!["e".into(), e2.id.clone()],
        ],
        "Synthetic withdrawal".into(),
        auth::now(),
    );
    repo.record_deletion_request(&deletion).await?;
    repo.approve_deletion(id(&deletion.id), None).await?;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let output = tokio::process::Command::new(project.join("vendor/strfry/strfry"))
                .arg("--config")
                .arg(dir.path().join("backend/strfry.conf"))
                .arg("scan")
                .arg(json!({"ids":[e1.id,e2.id]}).to_string())
                .output()
                .await?;
            anyhow::ensure!(output.status.success(), "strfry visibility check failed");
            if output.stdout.is_empty() {
                break Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    drain_backend(&repo).await?;
    for event in [&e1, &e2] {
        assert!(
            repo.restore_publication(id(&event.id)).await.is_err(),
            "live public alias must prevent completed removal/restoration"
        );
    }
    assert_eq!(http.get(&url).send().await?.status(), 200);
    assert!(store.withdraw_public_original(original.key.clone()).await?);
    assert_eq!(http.get(&url).send().await?.status(), 404);
    drain_backend(&repo).await?;
    expire(&repo).await?;
    assert!(
        !store.reclaim_library(original.key.clone()).await?,
        "elapsed grace cannot release an external reader hold"
    );
    assert!(repo.blob_intent(image_hash).await?.is_some());
    assert_storage_used(&repo, &store, 1_048_576, before).await?;
    assert!(store.reclaim_library(b1.key.clone()).await?);
    assert!(store.reclaim_library(b2.key.clone()).await?);
    assert_storage_used(&repo, &store, 1_048_576, image.len() as u64).await?;
    assert!(
        !store.reclaim_library(original.key.clone()).await?,
        "an unproven external reader drain cannot release the original"
    );
    assert!(
        store.public_original(image_hash).await.is_err(),
        "removed publication is not available through guarded serving"
    );
    assert_eq!(store.get(image_hash, owner).await?, image);
    assert!(store.withdraw_public_original(original.key.clone()).await?);
    assert_eq!(http.get(&url).send().await?.status(), 404);
    assert!(repo.public_export_held(&original.key).await?);
    assert!(!store.reclaim_library(original.key.clone()).await?);
    // Force a later token to commit after snapshot/drain but before DELETE.
    let mut blocker = repo.pool.get().await?;
    let blocker_pid: i32 = blocker
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    let tx = blocker.transaction().await?;
    tx.query_one(
        "SELECT hash FROM public_export_holds WHERE hash=$1 FOR UPDATE",
        &[&image_hash.as_slice()],
    )
    .await?;
    let draining = {
        let store = store.clone();
        let serving = serving.clone();
        tokio::spawn(async move { store.drain_public_originals(serving).await })
    };
    wait_for_blocked_session(&repo, blocker_pid).await?;
    tx.execute(
        "UPDATE public_export_holds SET withdrawal=$2 WHERE hash=$1",
        &[&image_hash.as_slice(), &[44u8; 16].as_slice()],
    )
    .await?;
    tx.commit().await?;
    assert_eq!(draining.await??, 0);
    assert!(repo.public_export_held(&original.key).await?);
    assert_eq!(store.drain_public_originals(serving.clone()).await?, 1);
    assert!(!repo.public_export_held(&original.key).await?);
    serving.shutdown_verified().await?;
    expire(&repo).await?;
    assert!(store.reclaim_library(original.key.clone()).await?);
    assert_storage_used(&repo, &store, 1_048_576, 0).await?;
    worker.abort();
    let _ = worker.await;
    std::fs::write(
        project.join("results/public-export-holds.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["only complete projected original image generations acquire holds","owner/MIME/manifest mismatches rejected","hold is idempotent and has no timed expiry","unproven quarantine acknowledgement refused","repair cannot overwrite an externally held generation","elapsed publication grace does not release original bytes or charge","other unexported objects remain reclaimable","receipt-backed cleanup refuses unproven reader release","blocked collection preserves queue/grace evidence","real verified positive projection replaces test-seeded hardlink","backend removal waits for last public namespace withdrawal","real nginx withdrawal returns404 before reader-drain releases hold","late committed withdrawal token cannot inherit an earlier drain proof","same logical owner/volume on another physical root cannot withdraw or drain the original hold"],"scope":"real PostgreSQL, strfry and nginx withdrawal/drain integration; positive/negative projection primitives and backend deletion barrier; automatic queue, quarantine and restart/backup integration still incomplete"}),
        )?,
    )?;
    Ok(())
}
