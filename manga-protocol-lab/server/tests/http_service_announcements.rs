mod support;
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    service_announcement::Announcement,
};
use serde_json::{Value, json};
use std::{
    process::{Child, Command},
    time::Duration,
};
use support::*;
struct Node(Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
async fn repo(dsn: String) -> Result<Repository> {
    let r = Repository::connect(DbConfig {
        dsn,
        connections: 4,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    r.migrate().await?;
    Ok(r)
}
async fn post(c: &reqwest::Client, who: u8, url: &str, value: &Value) -> Result<reqwest::Response> {
    let body = serde_json::to_vec(value)?;
    Ok(c.post(url)
        .header("authorization", http(who, url, &body))
        .body(body)
        .send()
        .await?)
}
async fn ready(c: &reqwest::Client, url: &str) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if c.get(url).send().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .context("node readiness")?;
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated PostgreSQL and multiple real HTTP nodes"]
async fn signed_announcements_survive_founder_loss_and_reject_rollback() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    let parsed: tokio_postgres::Config = dsn.parse()?;
    ensure!(parsed.get_dbname() == Some("manga_test"), "fixture only");
    let a = repo(dsn.clone()).await?;
    for name in ["manga_ann_b", "manga_ann_c"] {
        a.pool
            .get()
            .await?
            .batch_execute(&format!("CREATE DATABASE {name}"))
            .await?;
    }
    let dsns = [
        dsn.clone(),
        dsn.replace("dbname=manga_test", "dbname=manga_ann_b"),
        dsn.replace("dbname=manga_test", "dbname=manga_ann_c"),
        dsn,
    ];
    let b = repo(dsns[1].clone()).await?;
    let cdb = repo(dsns[2].clone()).await?;
    let dir = tempfile::tempdir()?;
    let mut binds = vec![];
    let mut origins = vec![];
    for _ in 0..4 {
        let sock = std::net::TcpListener::bind("127.0.0.1:0")?;
        let bind = sock.local_addr()?.to_string();
        drop(sock);
        origins.push(format!("http://{bind}"));
        binds.push(bind);
    }
    let mut configs = vec![];
    for n in 0..4 {
        let k = if n == 3 { 11 } else { 11 + n as u8 };
        let operator = if n == 3 { 1 } else { n as u8 + 1 };
        let key = dir.path().join(format!("key-{n}"));
        write_key(&key, k)?;
        let path = dir.path().join(format!("config-{n}.json"));
        std::fs::write(
            &path,
            serde_json::to_vec(
                &json!({"bind":binds[n],"base_url":origins[n],"service_key_file":key,"admin_pubkey":public(operator),"database_plaintext_local":true,"sync_worker_enabled":false,"discovery_private_overrides":origins}),
            )?,
        )?;
        configs.push(path);
    }
    let spawn = |n: usize| -> Result<Node> {
        let log = std::fs::File::create(dir.path().join(format!("node-{n}.log")))?;
        Ok(Node(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", configs[n].to_str().unwrap()])
                .env("MANGA_DATABASE_URL", &dsns[n])
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        ))
    };
    let mut nodes = [Some(spawn(0)?), Some(spawn(1)?), Some(spawn(2)?)];
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()?;
    for origin in &origins[..3] {
        ready(&client, origin).await?;
    }
    let old: auth::Event = client
        .get(format!("{}/service/announcement", origins[0]))
        .send()
        .await?
        .json()
        .await?;
    old.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(old.kind, 11998);
    assert_eq!(old.pubkey, public(11));
    let publish = format!("{}/admin/service-announcement", origins[0]);
    assert_eq!(
        post(&client, 2, &publish, &json!({"endpoints":[]}))
            .await?
            .status(),
        403
    );
    let moved: auth::Event = post(&client, 1, &publish, &json!({"endpoints":[origins[3]]}))
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert!(moved.created_at > old.created_at);
    let fetch_b = format!("{}/admin/service-announcements/fetch", origins[1]);
    let body =
        json!({"seed_endpoint":origins[0],"expected_seed_key":public(11),"service_key":public(11)});
    let learned: Value = post(&client, 2, &fetch_b, &body)
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(learned["event"]["id"], moved.id);
    drop(nodes[0].take()); // The founder's old address is now offline.
    let new_origin = spawn(3)?;
    ready(&client, &origins[3]).await?;
    let fetch_c = format!("{}/admin/service-announcements/fetch", origins[2]);
    cdb.set_peer(id(&public(12)), &origins[1], true).await?;
    let through_peer = json!({"seed_endpoint":origins[1],"expected_seed_key":public(12),"service_key":public(11),"peer_authenticated":true});
    assert!(
        !post(&client, 3, &fetch_c, &through_peer)
            .await?
            .status()
            .is_success()
    );
    assert!(cdb.service_announcement(id(&public(11))).await?.is_none());
    b.set_peer(id(&public(13)), &origins[2], true).await?;
    let cold: Value = post(&client, 3, &fetch_c, &through_peer)
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(cold["event"]["id"], moved.id);
    assert_eq!(cold["peer_permissions_granted"], false);
    assert_eq!(cold["peer_endpoints_changed"], false);
    assert!(cdb.approved_peer(id(&public(11))).await.is_err());
    let resolved:Value=post(&client,3,&format!("{}/admin/identities/discover",origins[2]),&json!({"service_key":public(11),"seeds":[{"endpoint":origins[1],"expected_key":public(12),"peer_authenticated":true}]})).await?.error_for_status()?.json().await?;
    assert_eq!(resolved["service_pubkey"], public(11));
    assert_eq!(resolved["endpoint"], origins[3]);
    assert_eq!(resolved["peer_endpoints_changed"], false);
    let policy = manga_node::endpoint_policy::EndpointPolicy::new(&origins)?;
    assert!(
        manga_node::peer_discovery::refresh(&cdb, &policy, id(&public(11)))
            .await
            .is_err(),
        "discovery never creates peer approval"
    );
    let before_epoch = cdb.set_peer(id(&public(11)), &origins[0], true).await?;
    assert!(matches!(
        manga_node::peer_discovery::refresh(&cdb, &policy, id(&public(11))).await?,
        manga_node::peer_discovery::Outcome::Relocated
    ));
    let relocated = cdb.approved_peer(id(&public(11))).await?;
    assert_eq!(relocated.endpoint, origins[3]);
    assert!(relocated.discovered_endpoint);
    assert!(
        relocated.version > before_epoch,
        "old in-flight peer operations fenced"
    );
    assert!(matches!(
        manga_node::peer_discovery::refresh(&cdb, &policy, id(&public(11))).await?,
        manga_node::peer_discovery::Outcome::Unchanged
    ));
    let cache_c = format!("{}/admin/service-announcements", origins[2]);
    let replay: Value = post(&client, 3, &cache_c, &serde_json::to_value(&old)?)
        .await?
        .json()
        .await?;
    assert_eq!(replay["outcome"], "superseded");
    let withdrawal: auth::Event = post(
        &client,
        1,
        &format!("{}/admin/service-announcement", origins[3]),
        &json!({"endpoints":[]}),
    )
    .await?
    .error_for_status()?
    .json()
    .await?;
    assert!(
        Announcement::parse(withdrawal.clone())?
            .endpoints()
            .is_empty()
    );
    post(&client,2,&fetch_b,&json!({"seed_endpoint":origins[3],"expected_seed_key":public(11),"service_key":public(11)})).await?.error_for_status()?;
    drop(new_origin);
    let withdrawn: Value = post(&client, 3, &fetch_c, &through_peer)
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(withdrawn["withdrawn"], true);
    assert!(matches!(
        manga_node::peer_discovery::refresh(&cdb, &policy, id(&public(11))).await?,
        manga_node::peer_discovery::Outcome::Withdrawn
    ));
    cdb.set_peer(id(&public(11)), &origins[3], false).await?;
    assert!(
        manga_node::peer_discovery::refresh(&cdb, &policy, id(&public(11)))
            .await
            .is_err(),
        "revocation is not reversed"
    );

    let replay: Value = post(&client, 3, &cache_c, &serde_json::to_value(&moved)?)
        .await?
        .json()
        .await?;
    assert_eq!(replay["outcome"], "superseded");
    assert!(
        cdb.service_announcement(id(&public(11)))
            .await?
            .unwrap()
            .endpoints()
            .is_empty()
    );
    assert_eq!(
        post(
            &client,
            3,
            &format!("{}/admin/identities/discover", origins[2]),
            &json!({"service_key":public(11)})
        )
        .await?
        .status(),
        410
    );
    // Equal timestamps select the lower event ID, independent of arrival order.
    let tied_at = auth::now() - 30;
    let mut tied = vec![
        Announcement::signed(
            &format!("{:064x}", 21),
            vec!["https://a.example".into()],
            tied_at,
        )?,
        Announcement::signed(
            &format!("{:064x}", 21),
            vec!["https://b.example".into()],
            tied_at,
        )?,
    ];
    tied.sort_by(|a, b| a.event().id.cmp(&b.event().id));
    for record in [&tied[1], &tied[0], &tied[1]] {
        post(&client, 3, &cache_c, &serde_json::to_value(record.event())?)
            .await?
            .error_for_status()?;
    }
    assert_eq!(
        cdb.service_announcement(id(&public(21)))
            .await?
            .unwrap()
            .event()
            .id,
        tied[0].event().id
    );
    let mut corrupted = moved.clone();
    corrupted.content = "{}".into();
    assert_eq!(
        post(&client, 3, &cache_c, &serde_json::to_value(corrupted)?)
            .await?
            .status(),
        400
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    std::fs::write(
        root.join("results/http-service-announcements.json"),
        serde_json::to_vec_pretty(
            &json!({"founder_old_origin_offline":true,"third_party_signed_cache_discovery":true,"new_origin_live_key_verified":true,"bidirectional_peer_approval_enforced":true,"replay_rejected":true,"withdrawal_survives_source_loss":true,"equal_timestamp_lowest_id":true,"new_peer_permission_granted":false,"automatic_peer_endpoint_update":true,"relocation_advances_permission_epoch":true,"withdrawal_blocks_relocation":true,"revocation_not_reversed":true}),
        )?,
    )?;
    Ok(())
}
