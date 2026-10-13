mod support;
use anyhow::{Context, Result};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    publication::Publications,
    sync_jobs::SyncSpec,
    upload::{StorageConfig, UploadStore},
};
use serde_json::json;
use std::time::Duration;
use support::*;
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL; run scripts/test-postgres.sh"]
async fn preparation_is_not_authority_to_commit() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(cfg.get_dbname() == Some("manga_test"), "fixture required");
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
            path: dir.path().into(),
            volume: "fences".into(),
            physical_bytes: 1048576,
            staging_bytes: 1048576,
            max_blob_bytes: 1048576,
            upload_slots: 2,
            reclamation_slots: 4,
            credit_lanes: 16,
            formats: vec!["image/png".into()],
        },
        owner,
        [22; 32],
    )
    .await?;
    let publisher = Publications::new(repo.clone(), store.clone());
    let image = png();
    let image_id = id(&auth::hash(&image));
    store.put(image_id, &image).await?;
    let manifest = |chapter: &str| {
        cbor(
            json!({"version":1,"series":"a4".repeat(32),"language":"en","chapter":chapter,"pages":[page(&image)]}),
        )
    };
    let original = repo.blob_intent(image_id).await?.unwrap();
    let mut client = repo.pool.get().await?;
    let bytes = manifest("generation");
    store.put(id(&auth::hash(&bytes)), &bytes).await?;
    let event = publication(21, 1001, &bytes);
    let prepared = publisher.prepare(&event).await?;
    replace_fixture_generation(&mut client, image_id, [77; 16]).await?;
    assert!(publisher.commit(prepared).await.is_err());
    replace_fixture_generation(&mut client, image_id, original.key.generation).await?;
    let prepared = publisher.prepare(&event).await?;
    client
        .execute(
            "UPDATE publisher_permissions SET allowed=FALSE WHERE publisher=$1",
            &[&owner.as_slice()],
        )
        .await?;
    assert!(publisher.commit(prepared).await.is_err());
    client
        .execute(
            "UPDATE publisher_permissions SET allowed=TRUE WHERE publisher=$1",
            &[&owner.as_slice()],
        )
        .await?;
    let prepared = publisher.prepare(&event).await?;
    client
        .execute(
            "UPDATE publisher_permissions SET allowed=FALSE WHERE publisher=$1",
            &[&owner.as_slice()],
        )
        .await?;
    client
        .execute(
            "UPDATE publisher_permissions SET allowed=TRUE WHERE publisher=$1",
            &[&owner.as_slice()],
        )
        .await?;
    assert!(
        publisher.commit(prepared).await.is_err(),
        "revoke then restore must invalidate old permission epoch"
    );
    let prepared = publisher.prepare(&event).await?;
    client
        .execute(
            "UPDATE events SET state='removed',version=version+1 WHERE id=$1",
            &[&id(&event.id).as_slice()],
        )
        .await?;
    assert!(publisher.commit(prepared).await.is_err());
    assert!(
        publisher.publish(&event).await.is_err(),
        "same event cannot resurrect removed publication"
    );
    let bytes = manifest("atomic");
    store.put(id(&auth::hash(&bytes)), &bytes).await?;
    let event = publication(21, 1001, &bytes);
    client.batch_execute("CREATE FUNCTION fixture_refuse_projection() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected outbox failure'; END $$; CREATE TRIGGER fixture_refuse_projection BEFORE INSERT ON backend_outbox FOR EACH ROW EXECUTE FUNCTION fixture_refuse_projection();").await?;
    assert!(publisher.publish(&event).await.is_err());
    assert!(repo.visible_event(id(&event.id)).await?.is_none());
    assert!(!repo.projection_complete(id(&event.id)).await?);
    client.batch_execute("DROP TRIGGER fixture_refuse_projection ON backend_outbox; DROP FUNCTION fixture_refuse_projection();").await?;
    let a = publisher.prepare(&event).await?;
    let b = publisher.prepare(&event).await?;
    let (a, b) = tokio::join!(publisher.commit(a), publisher.commit(b));
    assert_ne!(a?, b?);
    assert_eq!(
        repo.visible_event(id(&event.id)).await?.unwrap().id,
        event.id
    );
    assert!(!publisher.publish(&event).await?);
    // A retained shared original needs compatible health access, not another
    // exclusive receipt/accounting update on every publication.
    let next = manifest("shared-with-held-accounting-lock");
    store.put(id(&auth::hash(&next)), &next).await?;
    let next = publication(21, 1001, &next);
    let prepared = publisher.prepare(&next).await?;
    let held = client.transaction().await?;
    held.query_one(
        "SELECT hash FROM blob_intents WHERE hash=$1 FOR SHARE",
        &[&image_id.as_slice()],
    )
    .await?;
    held.query_one(
        "SELECT hash FROM blob_retention_credits WHERE hash=$1 FOR UPDATE",
        &[&image_id.as_slice()],
    )
    .await?;
    assert!(
        tokio::time::timeout(Duration::from_millis(750), publisher.commit(prepared)).await??,
        "unrelated retained-image lock blocked admission"
    );
    held.commit().await?;
    // Distinct event transactions race to retain one previously staged original.
    let extra_png = png(); // Use distinct valid bytes by appending harmless PNG trailing bytes.
    let mut extra = extra_png.clone();
    extra.extend_from_slice(b"shared fixture trailer");
    let extra_id = id(&auth::hash(&extra));
    store.put(extra_id, &extra).await?;
    let mut pending = vec![];
    for chapter in ["shared-race-a", "shared-race-b"] {
        let bytes = cbor(
            json!({"version":1,"series":"a4".repeat(32),"language":"en","chapter":chapter,"pages":[page(&extra)]}),
        );
        store.put(id(&auth::hash(&bytes)), &bytes).await?;
        pending.push(publisher.prepare(&publication(21, 1001, &bytes)).await?);
    }
    let right = pending.pop().unwrap();
    let left = pending.pop().unwrap();
    let (left, right) = tokio::join!(publisher.commit(left), publisher.commit(right));
    assert!(left?);
    assert!(right?);
    let bytes = manifest("lease-fence");
    store.put(id(&auth::hash(&bytes)), &bytes).await?;
    let event = publication(21, 1001, &bytes);
    repo.set_peer([0x11; 32], "http://127.0.0.1:19999", true)
        .await?;
    let batch = repo
        .enqueue_sync(SyncSpec {
            inventory: "chapters".into(),
            peer: "11".repeat(32),
            filter: String::new(),
            fallbacks: vec![],
        })
        .await?;
    let work = repo.claim_sync().await?.unwrap();
    assert_eq!(work.batch, batch);
    let lease = work.lease.clone();
    let prepared = publisher
        .prepare(&event)
        .await?
        .with_lease(Some(lease.clone()));
    repo.cancel_sync(batch).await?;
    assert!(publisher.commit(prepared).await.is_err());
    repo.set_peer([0x11; 32], "http://127.0.0.1:19999", true)
        .await?;
    let batch = repo
        .enqueue_sync(SyncSpec {
            inventory: "chapters".into(),
            peer: "11".repeat(32),
            filter: String::new(),
            fallbacks: vec![],
        })
        .await?;
    let work = repo.claim_sync().await?.unwrap();
    assert_eq!(work.batch, batch);
    let lease = work.lease.clone();
    let prepared = publisher
        .prepare(&event)
        .await?
        .with_lease(Some(lease.clone()));
    client
        .execute(
            "UPDATE jobs SET lease_until=clock_timestamp()-interval '1 second' WHERE id=$1",
            &[&lease.id.as_slice()],
        )
        .await?;
    assert!(publisher.commit(prepared).await.is_err());
    assert!(repo.visible_event(id(&event.id)).await?.is_none());
    let current = repo
        .claim_sync()
        .await?
        .context("expired sync work must recover")?;
    assert_eq!(current.batch, batch);
    let prepared = publisher
        .prepare(&event)
        .await?
        .with_lease(Some(current.lease.clone()));
    assert!(publisher.commit(prepared).await?);
    repo.save_missing(&current, vec![]).await?;
    repo.finish_sync_quantum(&current).await?;
    assert_eq!(repo.sync_status(batch).await?["state"], "completed");
    assert_eq!(
        repo.visible_event(id(&event.id)).await?.unwrap().id,
        event.id
    );
    std::fs::write(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../results/postgres-publication-fences.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["blob generation change rejects prepared publication","revocation and revoke-then-restore invalidate preparation","removal during validation rejects completion and replay","outbox failure leaves publication invisible and retryable","competing commits admit one visible immutable publication", "cancelled or expired job lease cannot commit a prepared chapter"]}),
        )?,
    )?;
    Ok(())
}

// Fault injection replaces the matching accounting record as well as its parent.
// No production path mutates a generation in place or bypasses foreign keys.
async fn replace_fixture_generation(
    client: &mut deadpool_postgres::Object,
    hash: [u8; 32],
    generation: [u8; 16],
) -> Result<()> {
    let tx = client.transaction().await?;
    tx.execute(
        "DELETE FROM blob_retention_credits WHERE hash=$1",
        &[&hash.as_slice()],
    )
    .await?;
    tx.execute(
        "UPDATE blob_intents SET generation=$2 WHERE hash=$1",
        &[&hash.as_slice(), &generation.as_slice()],
    )
    .await?;
    tx.execute(
        "INSERT INTO blob_retention_credits(hash,generation) VALUES($1,$2)",
        &[&hash.as_slice(), &generation.as_slice()],
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
