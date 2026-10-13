mod support;
use anyhow::{Context, Result, ensure};
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
async fn expire(repo: &Repository) -> Result<()> {
    repo.pool.get().await?.batch_execute("UPDATE blob_intents SET staging_until=clock_timestamp()-interval '1 second';UPDATE blob_gc_queue SET due_at=clock_timestamp()-interval '1 second';UPDATE blob_reclaims SET retry_after=clock_timestamp()-interval '1 second'").await?;
    Ok(())
}
// Exercise exact capacity through accepted/rejected writes, not accounting tables.
async fn capacity(
    store: &std::sync::Arc<UploadStore>,
    repo: &Repository,
    owner: [u8; 32],
    retained: usize,
) -> Result<()> {
    const LIMIT: usize = 1048576;
    let mut bytes = vec![93; LIMIT - retained];
    bytes[..16].copy_from_slice(&rand::random::<[u8; 16]>());
    let mut excess = bytes.clone();
    excess.push(1);
    if excess.len() <= LIMIT {
        ensure!(
            store.put(id(&auth::hash(&excess)), &excess).await.is_err(),
            "one byte beyond exact free capacity must be rejected"
        );
    }
    let hash = id(&auth::hash(&bytes));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match store.put(hash, &bytes).await {
                Ok(_) => break Ok::<_, anyhow::Error>(()),
                Err(error) if error.to_string().contains("quota exhausted") => {
                    tokio::time::sleep(Duration::from_millis(10)).await
                }
                Err(error) => return Err(error),
            }
        }
    })
    .await??;
    assert_eq!(store.get(hash, owner).await?, bytes);
    let extra = rand::random::<[u8; 16]>();
    ensure!(
        store.put(id(&auth::hash(&extra)), &extra).await.is_err(),
        "full capacity cannot accept another object"
    );
    store
        .delete_staged(
            manga_node::staged_deletion::DeleteAuthority::Publisher(
                repo.publisher_permit(owner).await?,
            ),
            hash,
        )
        .await?;
    ensure!(
        store.get(hash, owner).await.is_err(),
        "capacity probe must be deleted"
    );
    Ok(())
}
async fn collect(
    store: &std::sync::Arc<UploadStore>,
    owner: [u8; 32],
    hashes: &[[u8; 32]],
    select_new: bool,
) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            store.reclaim_objects_page(false, false, select_new).await?;
            let mut remaining = false;
            for hash in hashes {
                match store.get(*hash, owner).await {
                    Ok(_) => remaining = true,
                    Err(error) if error.is::<manga_node::upload::BlobUnavailable>() => {}
                    Err(error) if manga_node::db::is_database_error(&error) => return Err(error),
                    Err(_) => remaining = true,
                }
            }
            if !remaining {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated PostgreSQL and real filesystem receipts"]
async fn abandoned_staging_expires_without_bypassing_grants_or_receipts() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    ensure!(cfg.get_dbname() == Some("manga_test"), "fixture only");
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
            volume: "staginggc".into(),
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
    let mut originals = vec![];
    for i in 0..71 {
        let b = format!("synthetic abandoned upload {i:03}").into_bytes();
        originals.push(store.put(id(&auth::hash(&b)), &b).await?);
    }
    let charged = originals
        .iter()
        .map(|item| item.size as usize)
        .sum::<usize>();
    store.reclaim_objects_page(false, false, true).await?;
    for (i, item) in originals.iter().enumerate() {
        assert_eq!(
            store.get(item.key.hash, owner).await?,
            format!("synthetic abandoned upload {i:03}").as_bytes(),
            "unexpired staging must survive"
        );
    }
    expire(&repo).await?;
    store.reclaim_objects_page(false, false, false).await?;
    for item in &originals {
        ensure!(
            store.get(item.key.hash, owner).await.is_ok(),
            "disabled new selection must preserve expired staging"
        );
    }
    capacity(&store, &repo, owner, charged).await?;
    collect(
        &store,
        owner,
        &originals
            .iter()
            .map(|item| item.key.hash)
            .collect::<Vec<_>>(),
        true,
    )
    .await?;
    capacity(&store, &repo, owner, 0).await?;
    store.reclaim_objects_page(false, false, true).await?;
    capacity(&store, &repo, owner, 0).await?;
    // A stale physical generation cannot touch a newly uploaded identical hash.
    let b = b"synthetic abandoned upload 000";
    let replacement = store.put(id(&auth::hash(b)), b).await?;
    assert_ne!(replacement.key.generation, originals[0].key.generation);
    store.reclaim_library(originals[0].key.clone()).await?;
    assert_eq!(store.get(replacement.key.hash, owner).await?, b);
    expire(&repo).await?;
    for _ in 0..3 {
        store.reclaim_objects_page(false, false, true).await?;
    }
    capacity(&store, &repo, owner, 0).await?;
    // A current publisher staging grant survives an artificially older blob TTL.
    repo.set_publisher_admission(id(&public(23)), true).await?;
    let permit = repo.publisher_permit(id(&public(23))).await?;
    let bytes = b"publisher protected staging";
    let guarded = store
        .put_for_publisher(&permit, id(&auth::hash(bytes)), bytes)
        .await?;
    expire(&repo).await?;
    for _ in 0..3 {
        store.reclaim_objects_page(false, false, true).await?;
    }
    assert_eq!(store.get(guarded.key.hash, owner).await?, bytes);
    repo.pool.get().await?.execute("UPDATE publisher_blob_grants SET lease_until=clock_timestamp()-interval '1 second' WHERE hash=$1", &[&guarded.key.hash.as_slice()]).await?;
    expire(&repo).await?;
    store.reclaim_objects_page(false, false, true).await?;
    collect(&store, owner, &[guarded.key.hash], true).await?;
    capacity(&store, &repo, owner, 0).await?;
    let publications = Publications::new(repo.clone(), store.clone());
    let keys = dir.path().join("group-keys");
    let groups = manga_node::groups::Groups::open(
        repo.clone(),
        keys.clone(),
        store.clone(),
        publications.clone(),
    )?;
    let group = groups.create(owner).await?;
    let epoch = groups.authorize(group, owner, "publish").await?;
    let gb = cbor(json!({"version":1,"titles":{"en":"Expired group staging"}}));
    let gh = id(&auth::hash(&gb));
    groups.upload(group, owner, epoch, gh, &gb).await?;
    let secret = std::fs::read_to_string(recovered_key_path(&keys, group))?;
    let ge = auth::sign(
        &secret,
        1002,
        vec![],
        json!({"manifest":hex::encode(gh),"version":1}).to_string(),
    )
    .map_err(anyhow::Error::msg)?;
    let prepared = publications
        .prepare_for_group(&ge, group, owner, epoch)
        .await?;
    expire(&repo).await?;
    for _ in 0..3 {
        store.reclaim_objects_page(false, false, true).await?;
    }
    assert_eq!(
        store.get(gh, owner).await?,
        gb,
        "active group lease must protect staging"
    );
    repo.pool.get().await?.execute("UPDATE group_blob_grants SET lease_until=clock_timestamp()-interval '1 second' WHERE group_key=$1", &[&group.as_slice()]).await?;
    assert!(
        publications.commit(prepared).await.is_err(),
        "group lease expiry fences prepared publication"
    );
    expire(&repo).await?;
    for _ in 0..3 {
        store.reclaim_objects_page(false, false, true).await?;
    }
    collect(&store, owner, &[gh], true).await?;
    capacity(&store, &repo, owner, 0).await?;
    // Slow in-flight transfers outlive an individual object's staging clock.
    let b = b"bounded in-flight original";
    let pin = store.put(id(&auth::hash(b)), b).await?;
    let operation = repo
        .begin_staging_operation(owner, "staginggc", 660)
        .await?;
    repo.hold_staging_original(&operation, &pin).await?;
    expire(&repo).await?;
    for _ in 0..3 {
        store.reclaim_objects_page(false, false, true).await?;
    }
    assert_eq!(
        store.get(pin.key.hash, owner).await?,
        b,
        "in-flight operation must protect expired staging"
    );
    repo.end_staging_operation(&operation).await?;
    store.reclaim_objects_page(false, false, true).await?;
    collect(&store, owner, &[pin.key.hash], true).await?;
    assert!(
        repo.hold_staging_original(&operation, &pin).await.is_err(),
        "closed operation cannot regain a hold"
    );
    assert!(
        repo.begin_staging_operation(owner, "staginggc", 661)
            .await
            .is_err(),
        "operation hold has a strict deadline budget"
    );
    let b = b"abandoned operation original";
    let pin = store.put(id(&auth::hash(b)), b).await?;
    let operation = repo
        .begin_staging_operation(owner, "staginggc", 660)
        .await?;
    repo.hold_staging_original(&operation, &pin).await?;
    repo.pool.get().await?.execute("UPDATE staging_operations SET expires_at=clock_timestamp()-interval '1 second' WHERE id IN (SELECT operation FROM staging_operation_blobs WHERE hash=$1)", &[&pin.key.hash.as_slice()]).await?;
    expire(&repo).await?;
    for _ in 0..3 {
        store.reclaim_objects_page(false, false, true).await?;
    }
    collect(&store, owner, &[pin.key.hash], true).await?;
    capacity(&store, &repo, owner, 0).await?;
    // A retained publication is outside abandoned staging regardless of its TTL.
    let manifest = cbor(json!({"version":1,"titles":{"en":"Kept publication"}}));
    let hosted = store.put(id(&auth::hash(&manifest)), &manifest).await?;
    Publications::new(repo.clone(), store.clone())
        .publish(&publication(21, 1002, &manifest))
        .await?;
    expire(&repo).await?;
    for _ in 0..3 {
        store.reclaim_objects_page(false, false, true).await?;
    }
    assert_eq!(store.get(hosted.key.hash, owner).await?, manifest);
    // Existing external-reader inhibitors still fence unreferenced staging.
    let bytes = b"external reader protected staging";
    let reader = store.put(id(&auth::hash(bytes)), bytes).await?;
    repo.pool.get().await?.execute("INSERT INTO public_export_holds(hash,generation,owner,volume,mime,state,root_identity) VALUES($1,$2,$3,'staginggc','image/png','creating',$4)", &[&reader.key.hash.as_slice(),&reader.key.generation.as_slice(),&owner.as_slice(),&store.public_volume_identity().as_slice()]).await?;
    expire(&repo).await?;
    for _ in 0..3 {
        store.reclaim_objects_page(false, false, true).await?;
    }
    assert_eq!(store.get(reader.key.hash, owner).await?, bytes);
    // Fixture only: no external process was created; release the synthetic hold.
    repo.pool
        .get()
        .await?
        .execute(
            "DELETE FROM public_export_holds WHERE hash=$1",
            &[&reader.key.hash.as_slice()],
        )
        .await?;
    expire(&repo).await?;
    store.reclaim_objects_page(false, false, true).await?;
    collect(&store, owner, &[reader.key.hash], true).await?;
    // Failed unlink never refunds; a durable claim resumes with new GC disabled.
    let bytes = b"synthetic interrupted cleanup";
    let failed = store.put(id(&auth::hash(bytes)), bytes).await?;
    let h = hex::encode(failed.key.hash);
    let path = dir
        .path()
        .join("objects")
        .join(&h[..2])
        .join(&h[2..4])
        .join(format!("{}.{}", h, hex::encode(failed.key.generation)));
    let saved = dir.path().join("saved-synthetic-original");
    std::fs::rename(&path, &saved)?;
    std::fs::create_dir(&path)?;
    let before = manifest.len() + bytes.len();
    expire(&repo).await?;
    for _ in 0..3 {
        store.reclaim_objects_page(false, false, true).await?;
    }
    capacity(&store, &repo, owner, before).await?;
    std::fs::remove_dir(&path)?;
    std::fs::rename(saved, &path)?;
    expire(&repo).await?;
    collect(&store, owner, &[failed.key.hash], false).await?;
    capacity(&store, &repo, owner, manifest.len()).await?;
    store.reclaim_objects_page(false, false, false).await?;
    use manga_node::staged_deletion::{DeleteAuthority, DeleteDenied};
    let admin = repo.publisher_permit(owner).await?;
    assert!(
        store
            .delete_staged(DeleteAuthority::Publisher(admin.clone()), hosted.key.hash)
            .await
            .unwrap_err()
            .is::<DeleteDenied>(),
        "direct delete cannot bypass publication retention"
    );
    let own = repo.publisher_permit(id(&public(23))).await?;
    let bytes = b"shared admin and publisher staging";
    let shared = store
        .put_for_publisher(&own, id(&auth::hash(bytes)), bytes)
        .await?;
    store
        .put_for_publisher(&admin, shared.key.hash, bytes)
        .await?;
    assert!(
        matches!(
            store
                .delete_staged(DeleteAuthority::Publisher(own.clone()), shared.key.hash)
                .await
                .unwrap_err()
                .downcast_ref::<DeleteDenied>(),
            Some(DeleteDenied::Protected)
        ),
        "another uploader's administrator interest must be protected"
    );
    assert!(
        matches!(
            store
                .delete_staged(DeleteAuthority::Publisher(admin.clone()), shared.key.hash)
                .await
                .unwrap_err()
                .downcast_ref::<DeleteDenied>(),
            Some(DeleteDenied::Protected)
        ),
        "administrator cannot erase another active staging interest"
    );
    repo.pool.get().await?.execute("UPDATE publisher_blob_grants SET lease_until=clock_timestamp()-interval '1 second' WHERE publisher=$1 AND hash=$2", &[&owner.as_slice(),&shared.key.hash.as_slice()]).await?;
    store
        .delete_staged(DeleteAuthority::Publisher(own.clone()), shared.key.hash)
        .await?;
    ensure!(
        store.get(shared.key.hash, owner).await.is_err(),
        "deleted shared object unavailable"
    );
    capacity(&store, &repo, owner, manifest.len()).await?;
    let b = b"delete with active transfer";
    let item = store.put_for_publisher(&own, id(&auth::hash(b)), b).await?;
    let op = repo
        .begin_staging_operation(owner, "staginggc", 660)
        .await?;
    repo.hold_staging_original(&op, &item).await?;
    assert!(matches!(
        store
            .delete_staged(DeleteAuthority::Publisher(own.clone()), item.key.hash)
            .await
            .unwrap_err()
            .downcast_ref::<DeleteDenied>(),
        Some(DeleteDenied::Protected)
    ));
    repo.end_staging_operation(&op).await?;
    store
        .delete_staged(DeleteAuthority::Publisher(own.clone()), item.key.hash)
        .await?;
    // An authorized failed deletion retains its grant record for a safe retry.
    let b = b"failed direct deletion receipt";
    let item = store.put_for_publisher(&own, id(&auth::hash(b)), b).await?;
    let h = hex::encode(item.key.hash);
    let path = dir
        .path()
        .join("objects")
        .join(&h[..2])
        .join(&h[2..4])
        .join(format!("{}.{}", h, hex::encode(item.key.generation)));
    let saved = dir.path().join("direct-delete-original");
    std::fs::rename(&path, &saved)?;
    std::fs::create_dir(&path)?;
    let before = manifest.len() + b.len();
    assert!(
        store
            .delete_staged(DeleteAuthority::Publisher(own.clone()), item.key.hash)
            .await
            .is_err()
    );
    capacity(&store, &repo, owner, before).await?;
    std::fs::remove_dir(&path)?;
    std::fs::rename(saved, &path)?;
    store
        .delete_staged(DeleteAuthority::Publisher(own.clone()), item.key.hash)
        .await?;
    capacity(&store, &repo, owner, manifest.len()).await?;
    let b = b"group direct deletion";
    let hash = id(&auth::hash(b));
    groups.upload(group, owner, epoch, hash, b).await?;
    groups.delete_upload(group, owner, epoch, hash).await?;
    ensure!(
        store.get(hash, owner).await.is_err(),
        "deleted group upload unavailable"
    );
    // Aborting the requesting future does not abort the owned cleanup task.
    let b = b"cancelled direct deletion request";
    let item = store.put_for_publisher(&own, id(&auth::hash(b)), b).await?;
    let barrier = repo.pool.get().await?;
    barrier.batch_execute("CREATE FUNCTION hold_staged_delete_fixture() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(7183931); RETURN NEW; END $$; CREATE TRIGGER hold_staged_delete_fixture BEFORE INSERT ON blob_reclaims FOR EACH ROW EXECUTE FUNCTION hold_staged_delete_fixture();SELECT pg_advisory_lock(7183931)").await?;
    let deleting = {
        let store = store.clone();
        let permit = own.clone();
        tokio::spawn(async move {
            store
                .delete_staged(DeleteAuthority::Publisher(permit), item.key.hash)
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(5),async{loop{if repo.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND objid=7183931 AND NOT granted)",&[]).await?.get::<_,bool>(0){break Ok::<_,anyhow::Error>(());}tokio::time::sleep(Duration::from_millis(10)).await;}}).await??;
    deleting.abort();
    let _ = deleting.await;
    barrier
        .batch_execute("SELECT pg_advisory_unlock(7183931)")
        .await?;
    capacity(&store, &repo, owner, manifest.len()).await?;
    ensure!(
        store.get(item.key.hash, owner).await.is_err(),
        "cancelled deletion must finish owned cleanup"
    );
    barrier.batch_execute("DROP TRIGGER hold_staged_delete_fixture ON blob_reclaims;DROP FUNCTION hold_staged_delete_fixture()").await?;
    // An old admission epoch cannot authorize a deletion after re-admission.
    let b = b"revoked deletion authority";
    let item = store.put_for_publisher(&own, id(&auth::hash(b)), b).await?;
    repo.set_publisher_admission(id(&public(23)), false).await?;
    repo.set_publisher_admission(id(&public(23)), true).await?;
    assert!(matches!(
        store
            .delete_staged(DeleteAuthority::Publisher(own), item.key.hash)
            .await
            .unwrap_err()
            .downcast_ref::<DeleteDenied>(),
        Some(DeleteDenied::Unauthorized)
    ));
    assert_eq!(store.get(item.key.hash, owner).await?, b);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("project root")?;
    std::fs::write(
        root.join("results/rebuild-admission/staging-reclamation.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["71 expired unretained uploads reclaimed across bounded pages","new selection disabled and unexpired staging preserved","physical and staging refunds occur exactly once","stale generation cannot remove re-uploaded same hash","active publisher grant protects bytes","bounded in-flight transfer holds protect until closed or expired","group staging lease protects then expires and fences prepared publication","retained publication survives staging expiry","external-reader hold protects unreferenced generation","failed unlink retains charge and durable claim","claimed cleanup resumes with new GC disabled"],"not_claimed":["standard Blossom interoperability or target-scale capacity"]}),
        )?,
    )?;
    Ok(())
}
