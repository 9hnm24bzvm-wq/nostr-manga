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
async fn health_withdrawal_is_bounded_cancellation_safe_and_repair_fenced() -> Result<()> {
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
    let mut images = Vec::new();
    let mut originals = Vec::new();
    for n in 0..71u8 {
        let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([n, 47, 79, 255]));
        let mut data = std::io::Cursor::new(vec![]);
        image::DynamicImage::ImageRgba8(image).write_to(&mut data, image::ImageFormat::Png)?;
        let bytes = data.into_inner();
        originals.push(store.put(id(&auth::hash(&bytes)), &bytes).await?);
        images.push(bytes);
    }
    let chapters = [(0..70).collect::<Vec<_>>(), vec![0, 2], vec![1], vec![70]];
    let mut events = Vec::new();
    for (n, pages) in chapters.iter().enumerate() {
        let pages: Vec<_> = pages.iter().map(|i| page(&images[*i])).collect();
        let manifest = cbor(
            json!({"version":1,"series":"ab".repeat(32),"chapter":n.to_string(),"language":"en","pages":pages}),
        );
        store.put(id(&auth::hash(&manifest)), &manifest).await?;
        let event = publication(21, 1001, &manifest);
        publications.publish(&event).await?;
        events.push(event);
    }
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
    async fn settle(store: &Arc<UploadStore>, repo: &Repository, owner: [u8; 32]) -> Result<usize> {
        tokio::time::timeout(Duration::from_secs(20), async {
            let mut maximum = 0;
            loop {
                let step = store.reconcile_public_exports().await?;
                assert_eq!(step.retried, 0);
                maximum = maximum.max(step.health_scanned);
                assert!(step.health_scanned <= 64);
                if repo
                    .public_export_idle(owner, "publicationgc", store.public_volume_identity())
                    .await?
                {
                    return Ok::<_, anyhow::Error>(maximum);
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await?
    }
    settle(&store, &repo, owner).await?;
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = listener.local_addr()?.to_string();
    drop(listener);
    let public_dir = dir.path().join("store/public");
    let serving = Arc::new(
        ManagedServing::prepare(ServingConfig {
            python: PathBuf::from("python3"),
            helper: project.join("src/serving_supervisor.py"),
            nginx: project.join("vendor/nginx-1.28.0/objs/nginx"),
            runtime: public_dir.join(".serving"),
            public: public_dir,
            bind: bind.clone(),
            owner,
            workers: 1,
            shutdown_seconds: 1,
        })
        .await?,
    );
    serving.activate().await?;
    let http = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()?;
    let url = |n: usize| format!("http://{bind}/{}.png", hex::encode(originals[n].key.hash));
    assert_eq!(
        http.get(url(0)).send().await?.bytes().await?.as_ref(),
        images[0].as_slice()
    );
    repo.pool
        .get()
        .await?
        .execute("UPDATE public_export_roles SET enabled=FALSE", &[])
        .await?;
    for n in [0, 2] {
        let hash = hex::encode(originals[n].key.hash);
        let path = dir
            .path()
            .join("store/objects")
            .join(&hash[..2])
            .join(&hash[2..4])
            .join(format!(
                "{hash}.{}",
                hex::encode(originals[n].key.generation)
            ));
        std::fs::write(path, b"damaged synthetic original")?;
    }
    repo.pool.get().await?.batch_execute("CREATE FUNCTION test_health_barrier() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN PERFORM pg_advisory_xact_lock(7773501);RETURN NEW;END$$;CREATE TRIGGER test_health_barrier BEFORE INSERT ON quarantined_blobs FOR EACH ROW EXECUTE FUNCTION test_health_barrier()").await?;
    let mut blocker = repo.pool.get().await?;
    let blocker_pid: i32 = blocker
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    let tx = blocker.transaction().await?;
    tx.query_one("SELECT pg_advisory_xact_lock(7773501)", &[])
        .await?;
    let request = {
        let store = store.clone();
        let hash = originals[0].key.hash;
        tokio::spawn(async move { store.get(hash, owner).await })
    };
    wait_for_blocked_session(&repo, blocker_pid).await?;
    assert_eq!(http.get(url(0)).send().await?.status(), 404);
    assert!(repo.public_export_held(&originals[0].key).await?);
    assert!(
        repo.public_export_pending(owner, "publicationgc", store.public_volume_identity())
            .await?
    );
    assert_eq!(
        store.drain_public_originals(serving.clone()).await?,
        0,
        "unfinished withdrawal cannot inherit an older reader-drain token"
    );
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    tx.commit().await?;
    drop(blocker);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if repo.blob_quarantined(&originals[0].key).await? {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    assert!(store.get(originals[2].key.hash, owner).await.is_err());
    assert_eq!(http.get(url(2)).send().await?.status(), 404);
    assert_eq!(
        http.get(url(3)).send().await?.status(),
        200,
        "other valid aliases may remain while cleanup is pending"
    );
    assert!(
        repo.public_export_pending(owner, "publicationgc", store.public_volume_identity())
            .await?
    );
    for n in [0, 2, 3] {
        assert!(store.public_original(originals[n].key.hash).await.is_err());
    }
    for n in [1, 70] {
        assert_eq!(
            store.public_original(originals[n].key.hash).await?.0,
            images[n]
        );
    }
    let peer_key = id(&public(23));
    repo.set_peer(peer_key, "http://127.0.0.1:19999", true)
        .await?;
    let peer = repo.approved_peer(peer_key).await?;
    assert!(
        store
            .repair_verified(originals[0].clone(), images[0].clone(), peer.clone())
            .await
            .is_err()
    );
    repo.pool
        .get()
        .await?
        .execute("UPDATE public_export_roles SET enabled=TRUE", &[])
        .await?;
    assert_eq!(settle(&store, &repo, owner).await?, 64);
    for n in 0..71 {
        assert_eq!(
            http.get(url(n)).send().await?.status().as_u16(),
            if n == 1 || n == 70 { 200 } else { 404 }
        );
    }
    assert_eq!(store.drain_public_originals(serving.clone()).await?, 64);
    assert_eq!(store.drain_public_originals(serving.clone()).await?, 5);
    assert_eq!(store.drain_public_originals(serving.clone()).await?, 0);
    assert!(
        store
            .repair_verified(originals[0].clone(), images[0].clone(), peer.clone())
            .await?
    );
    settle(&store, &repo, owner).await?;
    assert_eq!(
        http.get(url(0)).send().await?.status(),
        404,
        "one remaining corrupt page keeps both chapters hidden"
    );
    assert!(
        store
            .repair_verified(originals[2].clone(), images[2].clone(), peer)
            .await?
    );
    settle(&store, &repo, owner).await?;
    for (n, image) in images.iter().enumerate() {
        assert_eq!(
            http.get(url(n)).send().await?.bytes().await?.as_ref(),
            image.as_slice()
        );
    }
    serving.shutdown_verified().await?;
    worker.abort();
    let _ = worker.await;
    std::fs::write(
        project.join("results/public-export-health.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"images":71,"checks":["known bad alias unlinked before quarantine commit","cancelled caller cannot cancel health commit","paused role retains work","64-page bounded fanout","shared and unrelated originals retained","64 plus 5 managed reader drains","partial repair remains hidden","complete repair reexports exact originals"],"scope":"component integration; automatic node serving not enabled"}),
        )?,
    )?;
    Ok(())
}
