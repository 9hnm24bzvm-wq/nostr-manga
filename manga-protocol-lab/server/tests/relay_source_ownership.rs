//! Application-used source cancellation arranged with a real external SQL lock.
use anyhow::{Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    read_admission::{Admission, Class, ClassConfig, Config},
    relay_dispatch::{EventSource, RepositorySource},
};
use std::{sync::Arc, time::Duration};
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated real PostgreSQL"]
async fn cancelled_source_retains_admission_until_database_work_terminates() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    ensure!(
        dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test"),
        "fixture database"
    );
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 4,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let class = ClassConfig {
        active: 1,
        waiting: 0,
    };
    let reads = Arc::new(Admission::new(
        Config {
            overall: 1,
            wait_ms: 100,
            catalogue: class.clone(),
            library_head: class.clone(),
            live: class,
        },
        4,
    )?);
    let source = Arc::new(RepositorySource::new(repo.clone(), reads.clone()));
    let mut locker = repo.pool.get().await?;
    let tx = locker.transaction().await?;
    tx.batch_execute("LOCK TABLE events IN ACCESS EXCLUSIVE MODE")
        .await?;
    let reader = source.clone();
    let work = tokio::spawn(async move { reader.fetch([7; 32]).await });
    tokio::time::timeout(Duration::from_secs(2),async {
        loop {let blocked:i64=repo.pool.get().await?.query_one("SELECT count(*) FROM pg_locks WHERE relation='events'::regclass AND NOT granted",&[]).await?.get(0);if blocked>0 {break Ok::<_,anyhow::Error>(());}tokio::time::sleep(Duration::from_millis(5)).await;}
    }).await??;
    work.abort();
    assert!(work.await.unwrap_err().is_cancelled());
    assert!(
        reads.acquire(Class::Catalogue).await.is_err(),
        "cancelled source must retain admission while PostgreSQL is still executing its blocked query"
    );
    // Existing server-side lock_timeout ends the owned SQL operation. Even while
    // our lock remains held, an unrelated admitted authorization query progresses.
    let permit = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(permit) = reads.acquire(Class::Catalogue).await {
                break permit;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert!(repo.authorization_clock_allowed(auth::now()).await?);
    drop(permit);
    tx.rollback().await?;
    assert!(
        source.fetch([7; 32]).await?.is_none(),
        "source recovers after external fault removal"
    );
    Ok(())
}
