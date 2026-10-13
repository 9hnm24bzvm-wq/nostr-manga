mod support;
use anyhow::{Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    endpoint_policy::EndpointPolicy,
    peer_discovery::{self, Outcome},
    service_announcement::Announcement,
};
use serde_json::json;
use std::{collections::HashMap, sync::Arc, time::Duration};
use support::*;
use tokio::sync::Notify;
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated PostgreSQL and controlled HTTP identity proof"]
async fn changed_approval_or_announcement_fences_late_resolution() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    ensure!(
        dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test"),
        "fixture only"
    );
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let key = id(&public(11));
    for mode in ["revocation", "withdrawal", "operator_endpoint"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let origin = format!("http://{}", listener.local_addr()?);
        let seen = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let handler_seen = seen.clone();
        let handler_release = release.clone();
        let handler_origin = origin.clone();
        let app = axum::Router::new().route(
            "/identity",
            axum::routing::get(
                move |axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>| {
                    let seen = handler_seen.clone();
                    let release = handler_release.clone();
                    let origin = handler_origin.clone();
                    async move {
                        seen.notify_one();
                        release.notified().await;
                        axum::Json(sign_at(
                            11,
                            20005,
                            vec![
                                vec!["nonce".into(), q["nonce"].clone()],
                                vec!["endpoint".into(), origin],
                            ],
                            String::new(),
                            auth::now(),
                        ))
                    }
                },
            ),
        );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let old = "http://127.0.0.1:9";
        repo.set_peer(key, old, true).await?;
        let record = repo
            .publish_service_endpoints(&format!("{:064x}", 11), vec![origin.clone()])
            .await?;
        let policy = EndpointPolicy::new(&[origin.clone(), old.into()])?;
        let task_repo = repo.clone();
        let task =
            tokio::spawn(async move { peer_discovery::refresh(&task_repo, &policy, key).await });
        tokio::time::timeout(Duration::from_secs(5), seen.notified()).await?;
        match mode {
            "revocation" => {
                repo.set_peer(key, old, false).await?;
            }
            "withdrawal" => {
                let empty = Announcement::signed(
                    &format!("{:064x}", 11),
                    vec![],
                    record.event().created_at + 1,
                )?;
                repo.cache_service_announcement(&empty).await?;
            }
            _ => {
                repo.set_peer(key, "https://operator.example", true).await?;
            }
        }
        release.notify_one();
        let outcome = tokio::time::timeout(Duration::from_secs(5), task).await???;
        if mode == "withdrawal" {
            assert!(matches!(outcome, Outcome::Unavailable));
        } else {
            assert!(matches!(outcome, Outcome::ApprovalChanged));
        }
        // This is the same approval/endpoint lookup used before runtime peer
        // connections. A stale proof must not revive access or replace the
        // operator's selected destination; storage flags are not the oracle.
        match mode {
            "revocation" => {
                assert!(repo.approved_peer(key).await.is_err());
                let policy = EndpointPolicy::new(&[origin.clone(), old.into()])?;
                assert!(peer_discovery::refresh(&repo, &policy, key).await.is_err());
            }
            "operator_endpoint" => {
                assert_eq!(
                    repo.approved_peer(key).await?.endpoint,
                    "https://operator.example"
                );
            }
            _ => {
                assert_eq!(repo.approved_peer(key).await?.endpoint, old);
                let policy = EndpointPolicy::new(&[origin.clone(), old.into()])?;
                assert!(matches!(
                    peer_discovery::refresh(&repo, &policy, key).await?,
                    Outcome::Withdrawn
                ));
            }
        }
        server.abort();
        let _ = server.await;
    }
    std::fs::write(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("results/peer-relocation-fences.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["revocation during proof remains revoked","newer withdrawal fences late proof","operator endpoint change wins over late discovery"]}),
        )?,
    )?;
    Ok(())
}
