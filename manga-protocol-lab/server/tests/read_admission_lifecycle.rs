//! The same owned admission API is used by HTTP, Nostr and live dispatch.
use anyhow::Result;
use manga_node::read_admission::{Admission, Class, ClassConfig, Config};
use std::time::Duration;

fn config() -> Config {
    Config {
        overall: 2,
        wait_ms: 150,
        catalogue: ClassConfig {
            active: 1,
            waiting: 1,
        },
        library_head: ClassConfig {
            active: 2,
            waiting: 1,
        },
        live: ClassConfig {
            active: 2,
            waiting: 1,
        },
    }
}
#[tokio::test]
async fn bounded_burst_waits_without_owning_other_capacity() -> Result<()> {
    let admission = Admission::new(config(), 4)?;
    let catalogue = admission.acquire(Class::Catalogue).await?;
    let waiting = admission.acquire(Class::Catalogue);
    tokio::pin!(waiting);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut waiting)
            .await
            .is_err()
    );
    // The class waiter cannot reserve the remaining overall slot.
    let library = tokio::time::timeout(
        Duration::from_millis(30),
        admission.acquire(Class::LibraryHead),
    )
    .await??;
    // The class queue is bounded, even though the active request is still busy.
    assert!(
        tokio::time::timeout(
            Duration::from_millis(30),
            admission.acquire(Class::Catalogue)
        )
        .await?
        .is_err()
    );
    // A free class cannot bypass the overall active cap.
    let live = admission.acquire(Class::Live);
    tokio::pin!(live);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut live)
            .await
            .is_err()
    );
    drop(catalogue);
    let replacement = tokio::time::timeout(Duration::from_millis(30), &mut waiting).await??;
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut live)
            .await
            .is_err()
    );
    drop(library);
    let live = tokio::time::timeout(Duration::from_millis(30), &mut live).await??;
    drop((replacement, live));
    Ok(())
}
#[tokio::test]
async fn cancellation_and_timeout_release_bounded_waiting_places() -> Result<()> {
    let admission = Admission::new(config(), 4)?;
    let first = admission.acquire(Class::Catalogue).await?;
    {
        let cancelled = admission.acquire(Class::Catalogue);
        tokio::pin!(cancelled);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut cancelled)
                .await
                .is_err()
        );
    }
    let started = tokio::time::Instant::now();
    assert!(admission.acquire(Class::Catalogue).await.is_err());
    assert!(
        started.elapsed() >= Duration::from_millis(100),
        "cancelled queue position must be reusable, not immediately full"
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    let after_timeout = admission.acquire(Class::Catalogue);
    tokio::pin!(after_timeout);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut after_timeout)
            .await
            .is_err()
    );
    drop(first);
    let active = tokio::time::timeout(Duration::from_millis(30), &mut after_timeout).await??;
    drop(active);
    admission.acquire(Class::Catalogue).await?;
    Ok(())
}
#[test]
fn validates_actual_runtime_configuration() {
    assert!(Admission::new(config(), 2).is_ok());
    for pool in 1..=4 {
        assert!(Admission::new(Config::for_database(pool), pool).is_ok());
    }
    assert!(Admission::new(config(), 1).is_err());
    for value in [0, 5001] {
        let mut c = config();
        c.wait_ms = value;
        assert!(Admission::new(c, 4).is_err());
    }
    for value in [0, 3] {
        let mut c = config();
        c.catalogue.active = value;
        assert!(Admission::new(c, 4).is_err());
    }
    let mut c = config();
    c.live.waiting = 1025;
    assert!(Admission::new(c, 4).is_err());
    let mut c = config();
    c.library_head.waiting = 0;
    assert!(Admission::new(c, 4).is_ok());
    assert!(serde_json::from_str::<Config>(r#"{"catalogue":{"typo":1}}"#).is_err());
}
