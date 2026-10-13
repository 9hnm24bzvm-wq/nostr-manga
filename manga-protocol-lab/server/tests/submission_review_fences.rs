mod support;
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    publication::Publications,
    storage::BlobKey,
    submission_review::{ReviewLimits, Reviews},
    upload::{StorageConfig, UploadStore},
};
use serde_json::json;
use std::time::Duration;
use support::*;
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
fn candidate(actor: u8, label: &str, image: &[u8]) -> (auth::Event, Vec<u8>) {
    let m = cbor(
        json!({"version":1,"series":"ab".repeat(32),"language":"en","chapter":label,"pages":[page(image)]}),
    );
    (publication(actor, 1001, &m), m)
}
async fn blob(repo: &Repository, hash: &str) -> Result<BlobKey> {
    Ok(BlobKey {
        hash: id(hash),
        generation: repo
            .pool
            .get()
            .await?
            .query_one(
                "SELECT generation FROM blob_intents WHERE hash=$1",
                &[&id(hash).as_slice()],
            )
            .await?
            .get::<_, Vec<u8>>(0)
            .try_into()
            .unwrap(),
    })
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated PostgreSQL and local storage"]
async fn review_holds_cleanup_and_decisions_are_fenced() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let config: tokio_postgres::Config = dsn.parse()?;
    ensure!(config.get_dbname() == Some("manga_test"), "fixture only");
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 12,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let owner = id(&public(1));
    let service = id(&public(11));
    repo.bind_instance(service, owner).await?;
    for actor in [2, 3] {
        repo.set_account_admission(id(&public(actor)), true).await?;
    }
    let a = repo.account_permit(id(&public(2))).await?;
    let b = repo.account_permit(id(&public(3))).await?;
    let dir = tempfile::tempdir()?;
    let store = UploadStore::open(
        repo.clone(),
        StorageConfig {
            path: dir.path().join("objects"),
            volume: "reviewfences".into(),
            physical_bytes: 2097152,
            staging_bytes: 1048576,
            max_blob_bytes: 262144,
            upload_slots: 4,
            reclamation_slots: 4,
            credit_lanes: 16,
            formats: vec!["image/png".into()],
        },
        owner,
        service,
    )
    .await?;
    let publications = Publications::new(repo.clone(), store.clone());
    let review = Reviews::new(
        repo.clone(),
        store.clone(),
        publications.clone(),
        ReviewLimits::default(),
    )?;
    let image = png();
    let hash = auth::hash(&image);
    let (event, manifest) = candidate(2, "cleanup", &image);
    review.declare(&a, event.clone(), &manifest).await?;
    review.upload(&a, id(&event.id), id(&hash), &image).await?;
    let key = blob(&repo, &hash).await?;
    repo.pool.get().await?.execute("UPDATE blob_intents SET staging_until=clock_timestamp()-interval '1 second' WHERE hash=$1",&[&key.hash.as_slice()]).await?;
    repo.enqueue_expired_staging(owner, "reviewfences").await?;
    assert!(
        !store.reclaim_library(key.clone()).await?,
        "pending review protects expired staging"
    );
    review
        .reject(id(&event.id), owner, "fixture".into())
        .await?;
    assert!(
        store.reclaim_library(key).await?,
        "rejection releases only its hold and advances the queued check"
    );
    assert!(
        store.get(id(&hash), owner).await.is_err(),
        "rejected original becomes unavailable after collection"
    );
    assert!(review.read(&a, id(&event.id), id(&hash)).await.is_err());
    assert_eq!(
        repo.review_status(id(&event.id), Some(id(&public(2))))
            .await?
            .unwrap()["state"],
        "rejected"
    );
    // A newly granted publisher still cannot resurrect an explicitly rejected ID.
    repo.set_publisher_admission(id(&public(2)), true).await?;
    let denied = publications.prepare(&event).await;
    assert!(denied.is_err());
    let error = denied.err().unwrap();
    assert_eq!(
        error
            .downcast_ref::<manga_node::transfer_rejection::TransferRejection>()
            .unwrap()
            .reason,
        manga_node::transfer_rejection::RejectionReason::ReviewRejected
    );
    let (one, om) = candidate(2, "shared", &image);
    let (two, tm) = candidate(3, "shared", &image);
    review.declare(&a, one.clone(), &om).await?;
    review.declare(&b, two.clone(), &tm).await?;
    review.upload(&a, id(&one.id), id(&hash), &image).await?;
    review.upload(&b, id(&two.id), id(&hash), &image).await?;
    let shared = blob(&repo, &hash).await?;
    repo.pool.get().await?.execute("UPDATE blob_intents SET staging_until=clock_timestamp()-interval '1 second' WHERE hash=$1",&[&shared.hash.as_slice()]).await?;
    repo.enqueue_expired_staging(owner, "reviewfences").await?;
    review.withdraw(id(&one.id), &a).await?;
    assert!(
        !store.reclaim_library(shared.clone()).await?,
        "another pending submission retains shared bytes"
    );
    review.approve(id(&two.id), owner).await?;
    assert!(
        !store.reclaim_library(shared).await?,
        "approved publication owns retained originals"
    );
    for n in 0..8 {
        let (event, manifest) = candidate(2, &format!("race-{n}"), &image);
        review.declare(&a, event.clone(), &manifest).await?;
        review.upload(&a, id(&event.id), id(&hash), &image).await?;
        let (approve, reject) = tokio::join!(
            review.approve(id(&event.id), owner),
            review.reject(id(&event.id), owner, "race".into())
        );
        assert_eq!(
            usize::from(approve.is_ok()) + usize::from(reject.is_ok()),
            1,
            "one final decision wins"
        );
        let detail = repo
            .review_status(id(&event.id), Some(id(&public(2))))
            .await?
            .unwrap();
        if approve.is_ok() {
            assert_eq!(detail["state"], "approved");
            let hosted = repo
                .visible_event(id(&event.id))
                .await?
                .expect("approved event visible");
            hosted.verify().map_err(anyhow::Error::msg)?;
            assert_eq!(hosted.id, event.id);
            assert_eq!(
                store.public_original(id(&auth::hash(&manifest))).await?.0,
                manifest
            );
            assert_eq!(store.public_original(id(&hash)).await?.0, image);
        } else {
            assert_eq!(detail["state"], "rejected");
            assert!(repo.visible_event(id(&event.id)).await?.is_none());
            assert!(
                store
                    .public_original(id(&auth::hash(&manifest)))
                    .await
                    .is_err()
            );
            assert!(review.read(&a, id(&event.id), id(&hash)).await.is_err());
        }
    }
    std::fs::write(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../results/submission-review-fences.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["pending review pins expired private staging","rejection releases its interest and accelerates cleanup","shared pending review interest survives another withdrawal","approved publication retains originals","publisher permission cannot resurrect rejected event","eight approve/reject races each commit exactly one decision"]}),
        )?,
    )?;
    Ok(())
}
