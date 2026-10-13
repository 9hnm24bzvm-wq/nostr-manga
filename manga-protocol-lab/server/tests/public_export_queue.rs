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
async fn drain(repo: &Repository) -> Result<()> {
    support::drain_backend(repo).await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and actual strfry"]
async fn durable_queue_backfills_and_follows_publication_visibility() -> Result<()> {
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
    let dir = tempfile::tempdir()?.keep();
    eprintln!("export fixture retained on failure: {}", dir.display());
    let store = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.as_path().join("store"),
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
            data_dir: dir.as_path().join("backend"),
            timeout_seconds: 3,
            map_bytes: 67108864,
        },
    )
    .await?;
    let worker = tokio::spawn(backend.run());
    drain(&repo).await?;

    assert!(
        !repo
            .public_export_pending(owner, "publicationgc", store.public_volume_identity())
            .await?
    );
    let mut staged = Vec::new();
    for n in 0..70 {
        let bytes = format!("synthetic unpublished staged object {n}").into_bytes();
        staged.push(store.put(id(&auth::hash(&bytes)), &bytes).await?);
    }
    store.configure_public_export_role().await?;
    let first = store.reconcile_public_exports().await?;
    assert_eq!(first.retried, 0);
    assert!(
        !repo
            .public_export_idle(owner, "publicationgc", store.public_volume_identity())
            .await?
    );
    async fn settle(store: &Arc<UploadStore>, repo: &Repository, owner: [u8; 32]) -> Result<()> {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                store.configure_public_export_role().await?;
                let report = store.reconcile_public_exports().await?;
                assert_eq!(report.retried, 0);
                if repo
                    .public_export_idle(owner, "publicationgc", store.public_volume_identity())
                    .await?
                {
                    break Ok::<_, anyhow::Error>(());
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await??;
        Ok(())
    }
    settle(&store, &repo, owner).await?;
    assert!(repo.public_export_held(&original.key).await?);
    for blob in staged.iter().chain([&b1, &b2]) {
        assert!(!repo.public_export_held(&blob.key).await?);
    }
    let hash = hex::encode(image_hash);
    let public = dir.as_path().join("store/public");
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
    let response = http.get(&url).send().await?;
    assert_eq!(response.status(), 200);
    assert_eq!(response.bytes().await?.as_ref(), image.as_slice());
    async fn remove(repo: &Repository, event: &str) -> Result<()> {
        let deletion = sign_at(
            21,
            5,
            vec![vec!["e".into(), event.into()]],
            "Queued removal".into(),
            auth::now(),
        );
        repo.record_deletion_request(&deletion).await?;
        repo.approve_deletion(id(&deletion.id), None).await?;
        Ok(())
    }
    remove(&repo, &e1.id).await?;
    drain(&repo).await?;
    settle(&store, &repo, owner).await?;
    let response = http.get(&url).send().await?;
    assert_eq!(
        response.status(),
        200,
        "shared healthy publication lost its alias"
    );
    assert_eq!(response.bytes().await?.as_ref(), image.as_slice());
    remove(&repo, &e2.id).await?;
    let report = store.reconcile_public_exports().await?;
    assert_eq!(report.withdrawn, 1);
    assert_eq!(report.retried, 0);
    assert_eq!(http.get(&url).send().await?.status(), 404);
    drain(&repo).await?;
    settle(&store, &repo, owner).await?;
    assert_eq!(store.drain_public_originals(serving.clone()).await?, 1);
    // A late queue version cannot be consumed by an earlier completed quantum.
    repo.pool
        .get()
        .await?
        .query_one(
            "SELECT enqueue_public_export($1)",
            &[&image_hash.as_slice()],
        )
        .await?;
    let mut blocker = repo.pool.get().await?;
    let tx = blocker.transaction().await?;
    tx.query_one(
        "SELECT hash FROM public_export_queue WHERE hash=$1 FOR UPDATE",
        &[&image_hash.as_slice()],
    )
    .await?;
    let running = {
        let store = store.clone();
        tokio::spawn(async move { store.reconcile_public_exports().await })
    };
    async fn blocked_ack(repo: &Repository, blocker: i32) -> Result<()> {
        tokio::time::timeout(Duration::from_secs(10),async {loop {
            let waiting:bool=repo.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",&[&blocker]).await?.get(0);if waiting{break Ok::<_,anyhow::Error>(());}tokio::time::sleep(Duration::from_millis(10)).await;
        }}).await??;
        Ok(())
    }
    let blocker_pid: i32 = tx.query_one("SELECT pg_backend_pid()", &[]).await?.get(0);
    blocked_ack(&repo, blocker_pid).await?;
    tx.execute(
        "UPDATE public_export_queue SET version=version+1 WHERE hash=$1",
        &[&image_hash.as_slice()],
    )
    .await?;
    tx.commit().await?;
    running.await??;
    assert!(
        !repo
            .public_export_idle(owner, "publicationgc", store.public_volume_identity())
            .await?
    );
    settle(&store, &repo, owner).await?;
    // Isolate the ABA-generation predicate with a test-only generation mutation.
    repo.pool
        .get()
        .await?
        .query_one(
            "SELECT enqueue_public_export($1)",
            &[&image_hash.as_slice()],
        )
        .await?;
    let tx = blocker.transaction().await?;
    tx.query_one(
        "SELECT hash FROM public_export_queue WHERE hash=$1 FOR UPDATE",
        &[&image_hash.as_slice()],
    )
    .await?;
    let running = {
        let store = store.clone();
        tokio::spawn(async move { store.reconcile_public_exports().await })
    };
    let blocker_pid: i32 = tx.query_one("SELECT pg_backend_pid()", &[]).await?.get(0);
    blocked_ack(&repo, blocker_pid).await?;
    tx.execute(
        "UPDATE public_export_queue SET generation=$2 WHERE hash=$1",
        &[&image_hash.as_slice(), &[99u8; 16].as_slice()],
    )
    .await?;
    tx.commit().await?;
    running.await??;
    assert!(
        !repo
            .public_export_idle(owner, "publicationgc", store.public_volume_identity())
            .await?
    );
    repo.pool
        .get()
        .await?
        .execute(
            "UPDATE public_export_queue SET generation=$2 WHERE hash=$1",
            &[&image_hash.as_slice(), &original.key.generation.as_slice()],
        )
        .await?;
    settle(&store, &repo, owner).await?;
    // New publication changes enqueue after backfill without scanning old blobs.
    let m3 = manifest("3");
    store.put(id(&auth::hash(&m3)), &m3).await?;
    let e3 = publication(21, 1001, &m3);
    publications.publish(&e3).await?;
    drain(&repo).await?;
    let object = dir
        .as_path()
        .join("store/objects")
        .join(&hash[..2])
        .join(&hash[2..4])
        .join(format!("{hash}.{}", hex::encode(original.key.generation)));
    std::fs::write(&object, b"synthetic corruption")?;
    let failed = store.reconcile_public_exports().await?;
    assert_eq!(failed.retried, 1);
    assert!(
        repo.public_export_pending(owner, "publicationgc", store.public_volume_identity())
            .await?
    );
    assert_eq!(http.get(&url).send().await?.status(), 404);
    assert_eq!(
        store.reconcile_public_exports().await?.retried,
        0,
        "following maintenance must not immediately repeat failed verification"
    );
    assert!(repo.blob_quarantined(&original.key).await?);
    // Repair must use the verified lifecycle; copying bytes alone does not
    // authorize clearing a durable negative observation.
    let peer_key = id(&support::public(23));
    repo.set_peer(peer_key, "http://127.0.0.1:19999", true)
        .await?;
    let peer = repo.approved_peer(peer_key).await?;
    assert!(
        store
            .repair_verified(original.clone(), image.clone(), peer)
            .await?
    );
    tokio::time::sleep(Duration::from_millis(1100)).await;
    settle(&store, &repo, owner).await?;
    assert_eq!(
        http.get(&url).send().await?.bytes().await?.as_ref(),
        image.as_slice()
    );
    serving.shutdown_verified().await?;
    worker.abort();
    let _ = worker.await;
    std::fs::remove_dir_all(&dir)?;
    std::fs::write(
        project.join("results/public-export-queue.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["ordinary node has no automatic work","repeated reconfiguration preserves resumable backfill progress","unpublished staging excluded","live publication and backend changes enqueue durably","shared alias retained until last removal","last removal drives namespace withdrawal","late version and generation cannot inherit old completion","failed verification stays unavailable and is not immediately repeated before verified repair","new publication after backfill requires no full rescan"],"scope":"actual PostgreSQL/strfry/nginx with explicit bounded maintenance calls; automatic node startup remains disabled; version/generation race uses test-only SQL mutation"}),
        )?,
    )?;
    Ok(())
}
