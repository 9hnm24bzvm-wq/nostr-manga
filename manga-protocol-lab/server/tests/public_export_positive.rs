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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and actual strfry"]
async fn positive_export_is_verified_idempotent_and_visibility_fenced() -> Result<()> {
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
    store.configure_public_export_role().await?;

    assert_eq!(store.reconcile_public_exports().await?.exported, 1);
    let hash = hex::encode(image_hash);
    let public = dir.path().join("store/public");
    assert_eq!(store.reconcile_public_exports().await?.exported, 0);
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
        .is_err()
    );
    assert!(
        repo.reserve_public_export(
            &b2.key,
            owner,
            "publicationgc",
            "image/png",
            store.public_volume_identity()
        )
        .await
        .is_err()
    );
    let mut stale = original.key.clone();
    stale.generation = [255; 16];
    assert!(
        repo.reserve_public_export(
            &stale,
            owner,
            "publicationgc",
            "image/png",
            store.public_volume_identity()
        )
        .await
        .is_err()
    );
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
    store.withdraw_public_original(original.key.clone()).await?;
    // A real new publication schedules a fresh verified export, which must
    // replace the previous draining token before relinking.
    let m3 = manifest("3");
    store.put(id(&auth::hash(&m3)), &m3).await?;
    let e3 = publication(21, 1001, &m3);
    publications.publish(&e3).await?;
    drain_backend(&repo).await?;
    assert_eq!(store.reconcile_public_exports().await?.exported, 1);
    assert_eq!(
        http.get(&url).send().await?.bytes().await?.as_ref(),
        image.as_slice()
    );
    assert!(repo.public_export_held(&original.key).await?);
    assert_eq!(store.drain_public_originals(serving.clone()).await?, 0);
    store.withdraw_public_original(original.key.clone()).await?;
    assert_eq!(store.drain_public_originals(serving.clone()).await?, 1);
    assert_eq!(http.get(&url).send().await?.status(), 404);
    // Queue work through another admitted publication rather than injecting a
    // private export call. The subsequent moderator transaction races that work.
    let m4 = manifest("4");
    store.put(id(&auth::hash(&m4)), &m4).await?;
    let e4 = publication(21, 1001, &m4);
    publications.publish(&e4).await?;
    drain_backend(&repo).await?;
    // The real moderator transaction acquires the first sorted event row and
    // pauses inside a fixture-only trigger. Positive export then waits on that
    // same event's shared visibility lock. Approval wins before link creation.
    repo.pool.get().await?.batch_execute("CREATE FUNCTION test_export_delete_barrier() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN PERFORM pg_advisory_xact_lock(7773401);RETURN NEW;END$$;CREATE TRIGGER test_export_delete_barrier BEFORE UPDATE OF state ON events FOR EACH ROW WHEN (NEW.state='withdrawing') EXECUTE FUNCTION test_export_delete_barrier()").await?;
    let deletion = sign_at(
        21,
        5,
        vec![
            vec!["e".into(), e1.id.clone()],
            vec!["e".into(), e2.id.clone()],
            vec!["e".into(), e3.id.clone()],
            vec!["e".into(), e4.id.clone()],
        ],
        "Export visibility race".into(),
        auth::now(),
    );
    repo.record_deletion_request(&deletion).await?;
    let mut blocker = repo.pool.get().await?;
    let blocker_pid: i32 = blocker
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    let tx = blocker.transaction().await?;
    tx.query_one("SELECT pg_advisory_xact_lock(7773401)", &[])
        .await?;
    let deleting = {
        let repo = repo.clone();
        let request = id(&deletion.id);
        tokio::spawn(async move { repo.approve_deletion(request, None).await })
    };
    let deleting_pid = wait_for_blocked_session(&repo, blocker_pid).await?;
    let exporting = {
        let store = store.clone();
        tokio::spawn(async move { store.reconcile_public_exports().await })
    };
    wait_for_blocked_session(&repo, deleting_pid).await?;
    tx.commit().await?;
    assert_eq!(deleting.await??, 4);
    let failed = exporting.await??;
    assert_eq!(failed.exported, 0);
    assert_eq!(failed.retried, 1);
    assert!(repo.public_export_held(&original.key).await?);
    assert_eq!(http.get(&url).send().await?.status(), 404);
    store.withdraw_public_original(original.key.clone()).await?;
    assert_eq!(store.drain_public_originals(serving.clone()).await?, 1);
    serving.shutdown_verified().await?;
    drain_backend(&repo).await?;
    worker.abort();
    let _ = worker.await;
    std::fs::write(
        project.join("results/public-export-positive.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["verified export serves exact original bytes","idempotent repeat","wrong MIME, manifest and stale generation refused","real nginx reads exact original","re-export clears draining token before relink","actual moderator approval defeats a blocked positive export","failed visibility retains charged creating hold"],"scope":"actual bounded export reconciliation and generation/visibility reservation APIs; no startup or backup/restore qualification"}),
        )?,
    )?;
    Ok(())
}
