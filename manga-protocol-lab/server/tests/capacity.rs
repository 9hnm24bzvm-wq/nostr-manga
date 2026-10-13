mod support;
use anyhow::{Context, Result};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    staged_deletion::DeleteAuthority,
    upload::{StorageConfig, UploadStore},
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use support::assert_storage_used;
fn hash(bytes: &[u8]) -> [u8; 32] {
    hex::decode(auth::hash(bytes)).unwrap().try_into().unwrap()
}
async fn repo() -> Result<Repository> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("ephemeral database required")?;
    let c: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(
        c.get_dbname() == Some("manga_test"),
        "fixture database required"
    );
    Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1_048_576,
        plaintext_local: true,
    })
    .await
}
fn config(path: &std::path::Path, volume: &str, physical: i64, staging: i64) -> StorageConfig {
    StorageConfig {
        path: path.into(),
        volume: volume.into(),
        physical_bytes: physical,
        staging_bytes: staging,
        max_blob_bytes: 1024,
        upload_slots: 4,
        reclamation_slots: 4,
        credit_lanes: 16,
        formats: vec!["image/png".into()],
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run scripts/test-postgres.sh"]
async fn durable_capacity_credits() -> Result<()> {
    let a = repo().await?;
    a.migrate().await?;
    let b = repo().await?;
    let dir = tempfile::tempdir()?;
    let owner = [101; 32];
    a.set_publisher_admission(owner, true).await?;
    let permit = a.publisher_permit(owner).await?;
    let store = UploadStore::open(
        a.clone(),
        config(dir.path(), "paired", 256, 128),
        owner,
        [102; 32],
    )
    .await?;
    let first = vec![1; 128];
    let second = vec![2; 128];
    let key = store
        .put_for_publisher(&permit, hash(&first), &first)
        .await?
        .key;
    assert_eq!(
        store
            .put_for_publisher(&permit, hash(&first), &first)
            .await?
            .key,
        key
    );
    let mut duplicates = tokio::task::JoinSet::new();
    for _ in 0..16 {
        let store = store.clone();
        let permit = permit.clone();
        let bytes = first.clone();
        duplicates
            .spawn(async move { store.put_for_publisher(&permit, hash(&bytes), &bytes).await });
    }
    while let Some(result) = duplicates.join_next().await {
        assert_eq!(result??.key, key);
    }
    assert!(
        store
            .put_for_publisher(&permit, hash(&first), &second)
            .await
            .is_err(),
        "conflicting bytes must not alter an existing identity"
    );
    assert!(
        store
            .put_for_publisher(&permit, hash(&second), &second)
            .await
            .is_err(),
        "staging exhaustion must reject otherwise affordable physical bytes"
    );
    assert_eq!(store.get(hash(&first), owner).await?, first);
    store
        .delete_staged(DeleteAuthority::Publisher(permit.clone()), hash(&first))
        .await?;
    assert!(store.get(hash(&first), owner).await.is_err());
    drop(store);
    let store = UploadStore::open(
        b.clone(),
        config(dir.path(), "paired", 256, 256),
        owner,
        [102; 32],
    )
    .await?;
    for bytes in [&first, &second] {
        store.put_for_publisher(&permit, hash(bytes), bytes).await?;
    }
    assert_storage_used(&a, &store, 256, 256).await?;
    store
        .delete_staged(DeleteAuthority::Publisher(permit.clone()), hash(&first))
        .await?;
    assert!(
        store
            .delete_staged(DeleteAuthority::Publisher(permit.clone()), hash(&first))
            .await
            .is_err(),
        "repeat deletion must not refund twice"
    );
    assert_storage_used(&a, &store, 256, 128).await?;
    store
        .delete_staged(DeleteAuthority::Publisher(permit.clone()), hash(&second))
        .await?;
    assert_storage_used(&a, &store, 256, 0).await?;
    // A failure after both budget charges are inserted cannot leak either debit.
    // The trigger arranges an external database fault; only upload/read and
    // exact-fill outcomes below establish rollback.
    let fault = a.pool.get().await?;
    fault.batch_execute("CREATE FUNCTION fixture_fail_capacity_charge() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF EXISTS(SELECT 1 FROM capacity_charges c WHERE c.reservation=NEW.reservation AND c.grant_id<>NEW.grant_id) THEN RAISE EXCEPTION 'injected paired-charge failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER fixture_fail_capacity_charge AFTER INSERT ON capacity_charges FOR EACH ROW EXECUTE FUNCTION fixture_fail_capacity_charge();").await?;
    assert!(
        store
            .put_for_publisher(&permit, hash(&first), &first)
            .await
            .is_err()
    );
    fault.batch_execute("DROP TRIGGER fixture_fail_capacity_charge ON capacity_charges; DROP FUNCTION fixture_fail_capacity_charge();").await?;
    assert!(store.get(hash(&first), owner).await.is_err());
    assert_storage_used(&a, &store, 256, 0).await?;
    // Durable bytes without a settled receipt stay charged until actual recovery.
    let pending_dir = tempfile::tempdir()?;
    let pending_owner = [103; 32];
    a.set_publisher_admission(pending_owner, true).await?;
    let pending_permit = a.publisher_permit(pending_owner).await?;
    let pending = UploadStore::open(
        a.clone(),
        config(pending_dir.path(), "pending", 128, 128),
        pending_owner,
        [104; 32],
    )
    .await?;
    fault.batch_execute("CREATE FUNCTION fixture_fail_capacity_settle() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.state='available' AND NEW.volume='pending' THEN RAISE EXCEPTION 'injected receipt settlement failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER fixture_fail_capacity_settle BEFORE UPDATE OF state ON blob_intents FOR EACH ROW EXECUTE FUNCTION fixture_fail_capacity_settle();").await?;
    let uncertain = vec![3; 128];
    let replacement = vec![4; 128];
    assert!(
        pending
            .put_for_publisher(&pending_permit, hash(&uncertain), &uncertain)
            .await
            .is_err()
    );
    assert!(
        pending
            .put_for_publisher(&pending_permit, hash(&replacement), &replacement)
            .await
            .is_err()
    );
    assert!(pending.get(hash(&uncertain), pending_owner).await.is_err());
    fault.batch_execute("DROP TRIGGER fixture_fail_capacity_settle ON blob_intents; DROP FUNCTION fixture_fail_capacity_settle();").await?;
    drop(pending);
    let pending = UploadStore::open(
        b.clone(),
        config(pending_dir.path(), "pending", 128, 128),
        pending_owner,
        [104; 32],
    )
    .await?;
    pending.recover_page(None).await?;
    assert_eq!(
        pending.get(hash(&uncertain), pending_owner).await?,
        uncertain
    );
    pending
        .put_for_publisher(&pending_permit, hash(&uncertain), &uncertain)
        .await?;
    assert!(
        pending
            .put_for_publisher(&pending_permit, hash(&replacement), &replacement)
            .await
            .is_err()
    );
    pending
        .delete_staged(
            DeleteAuthority::Publisher(pending_permit.clone()),
            hash(&uncertain),
        )
        .await?;
    pending
        .put_for_publisher(&pending_permit, hash(&replacement), &replacement)
        .await?;
    assert_eq!(
        pending.get(hash(&replacement), pending_owner).await?,
        replacement
    );
    assert_storage_used(&a, &pending, 128, 128).await?;
    // Independent volumes/pools compete for one publisher staging budget.
    let shared_owner = [105; 32];
    a.set_publisher_admission(shared_owner, true).await?;
    let shared_permit = a.publisher_permit(shared_owner).await?;
    let left_dir = tempfile::tempdir()?;
    let right_dir = tempfile::tempdir()?;
    let left = UploadStore::open(
        a.clone(),
        config(left_dir.path(), "left", 512, 512),
        shared_owner,
        [106; 32],
    )
    .await?;
    let right = UploadStore::open(
        b.clone(),
        config(right_dir.path(), "right", 512, 512),
        shared_owner,
        [106; 32],
    )
    .await?;
    let mut tasks = tokio::task::JoinSet::new();
    for n in 0..32u8 {
        let store = if n % 2 == 0 {
            left.clone()
        } else {
            right.clone()
        };
        let permit = shared_permit.clone();
        tasks.spawn(async move {
            let bytes = vec![n + 10; 128];
            let result = store.put_for_publisher(&permit, hash(&bytes), &bytes).await;
            (store, bytes, result)
        });
    }
    let mut winners: Vec<(Arc<UploadStore>, Vec<u8>)> = Vec::new();
    while let Some(result) = tasks.join_next().await {
        let (store, bytes, outcome) = result?;
        if outcome.is_ok() {
            winners.push((store, bytes));
        }
    }
    assert_eq!(
        winners.len(),
        4,
        "shared staging budget must admit exactly four objects"
    );
    for (store, bytes) in winners {
        assert_eq!(store.get(hash(&bytes), shared_owner).await?, bytes);
        store
            .delete_staged(
                DeleteAuthority::Publisher(shared_permit.clone()),
                hash(&bytes),
            )
            .await?;
    }
    let fill = vec![71; 512];
    left.put_for_publisher(&shared_permit, hash(&fill), &fill)
        .await?;
    let overflow = vec![72; 128];
    assert!(
        right
            .put_for_publisher(&shared_permit, hash(&overflow), &overflow)
            .await
            .is_err()
    );
    left.delete_staged(
        DeleteAuthority::Publisher(shared_permit.clone()),
        hash(&fill),
    )
    .await?;
    right
        .put_for_publisher(&shared_permit, hash(&fill), &fill)
        .await?;
    assert_eq!(right.get(hash(&fill), shared_owner).await?, fill);
    let report = json!({"passed":true,"checks":["paired staging exhaustion does not leak physical space","16 duplicate uploads preserve immutable generation without new debit and mismatched bytes fail","delete receipts refund exact capacity and repeated deletion cannot over-refund","failed admission transaction rolls back through exact-fill probe","unsettled durable write remains charged across reopen until verified recovery","32 concurrent uploads through independent pools respect shared staging quota","full released staging capacity transfers between volumes"],"scope":"real PostgreSQL and filesystem UploadStore lifecycle; no private ledger/counter outcomes or capacity benchmark claim"});
    if let Ok(path) = std::env::var("MANGA_CAPACITY_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    }
    println!("{report}");
    Ok(())
}
