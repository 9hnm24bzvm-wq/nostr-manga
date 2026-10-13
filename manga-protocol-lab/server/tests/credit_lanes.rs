mod support;
use anyhow::{Context, Result};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    publication::Publications,
    staged_deletion::DeleteAuthority,
    upload::{StorageConfig, UploadStore},
};
use serde_json::json;
use std::time::Duration;
use support::*;
fn id(text: &str) -> [u8; 32] {
    hex::decode(text).unwrap().try_into().unwrap()
}
fn bytes(lane: u8, round: u8) -> Vec<u8> {
    let mut bytes = png();
    bytes.resize(128, 0);
    for nonce in 0u32..100000 {
        bytes[120..124].copy_from_slice(&nonce.to_be_bytes());
        bytes[124] = round;
        if u8::from_str_radix(&auth::hash(&bytes)[..2], 16).unwrap() % 64 == lane {
            return bytes;
        }
    }
    panic!("synthetic lane search failed")
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL"]
async fn idle_lane_credits_rebalance_without_refunding_used_bytes() -> Result<()> {
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
    let holder = id(&public(11));
    repo.bind_instance(holder, owner).await?;
    let dir = tempfile::tempdir()?;
    let mut chapters = Vec::new();
    for n in 0..64u8 {
        let image = bytes(if n == 0 { 63 } else { n % 64 }, n);
        let manifest = cbor(
            json!({"version":1,"series":"ab".repeat(32),"language":"en","chapter":n.to_string(),"pages":[page(&image)]}),
        );
        chapters.push((image, manifest));
    }
    let physical: usize = chapters
        .iter()
        .map(|(image, manifest)| image.len() + manifest.len())
        .sum();
    let staging = 4 * chapters
        .iter()
        .map(|(image, manifest)| image.len() + manifest.len())
        .max()
        .unwrap();
    let config = |lanes| StorageConfig {
        path: dir.path().into(),
        volume: "lanes".into(),
        physical_bytes: physical as i64,
        staging_bytes: staging as i64,
        max_blob_bytes: 1024,
        upload_slots: 4,
        reclamation_slots: 4,
        credit_lanes: lanes,
        formats: vec!["image/png".into()],
    };
    let store = UploadStore::open(repo.clone(), config(64), owner, holder).await?;
    let publications = Publications::new(repo.clone(), store.clone());
    let first_hash = id(&auth::hash(&chapters[0].0));
    let mut first_key = None;
    for (image, manifest) in &chapters[..16] {
        let intent = store.put(id(&auth::hash(image)), image).await?;
        if first_key.is_none() {
            first_key = Some(intent.key);
        }
        store.put(id(&auth::hash(manifest)), manifest).await?;
        assert!(
            publications
                .publish(&publication(21, 1001, manifest))
                .await?
        );
        assert_eq!(store.get(id(&auth::hash(image)), owner).await?, *image);
    }
    drop(publications);
    drop(store);
    // Reopen with fewer lanes. Previously retained originals remain charged;
    // idle staging credits from the old high lanes must be reusable.
    let store = UploadStore::open(repo.clone(), config(16), owner, holder).await?;
    let publications = Publications::new(repo.clone(), store.clone());
    for group in chapters[16..].chunks(4) {
        let mut tasks = tokio::task::JoinSet::new();
        for (image, manifest) in group {
            let store = store.clone();
            let publications = publications.clone();
            let image = image.clone();
            let manifest = manifest.clone();
            tasks.spawn(async move {
                store.put(id(&auth::hash(&image)), &image).await?;
                store.put(id(&auth::hash(&manifest)), &manifest).await?;
                anyhow::ensure!(
                    publications
                        .publish(&publication(21, 1001, &manifest))
                        .await?,
                    "new chapter was not admitted"
                );
                Ok::<_, anyhow::Error>(())
            });
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            while let Some(result) = tasks.join_next().await {
                result??;
            }
            Ok::<_, anyhow::Error>(())
        })
        .await??;
    }
    for (image, manifest) in &chapters {
        assert_eq!(store.get(id(&auth::hash(image)), owner).await?, *image);
        assert_eq!(
            store.get(id(&auth::hash(manifest)), owner).await?,
            *manifest
        );
    }
    assert!(
        store.put(id(&auth::hash(&[201])), &[201]).await.is_err(),
        "all exact physical bytes must remain charged after staging release"
    );
    assert_eq!(
        store.put(first_hash, &chapters[0].0).await?.key,
        first_key.unwrap()
    );
    let permit = repo.publisher_permit(owner).await?;
    assert!(
        store
            .delete_staged(DeleteAuthority::Publisher(permit), first_hash)
            .await
            .is_err(),
        "retention must not be bypassed by staging deletion"
    );
    assert_eq!(store.get(first_hash, owner).await?, chapters[0].0);
    std::fs::write(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../results/rebuild-capacity-seams/credit-lanes.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"chapters":64,"physical_bytes":physical,"staging_bytes":staging,"checks":["real publication releases staging but retains physical bytes","64-to16 lane restart preserves old high-lane original","four concurrent chapter lifecycles recycle bounded staging across lanes","exact physical fill rejects another byte","retained original rejects staged deletion and remains exact"],"scope":"real PostgreSQL and original/manifest/publication lifecycle; no private ledger oracle or throughput claim"}),
        )?,
    )?;
    Ok(())
}
