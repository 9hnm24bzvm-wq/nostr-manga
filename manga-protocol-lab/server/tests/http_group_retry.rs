mod support;
use anyhow::{Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    mangadex_compat,
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
async fn get(c: &reqwest::Client, url: &str) -> Result<Value> {
    let r = c.get(url).send().await?;
    ensure!(r.status().is_success(), "read failed {}", r.text().await?);
    Ok(r.json().await?)
}
async fn read_audit(c: &reqwest::Client, origin: &str, group: &str) -> Result<Value> {
    let url = format!("{origin}/admin/groups/{group}/audit");
    let response = c
        .get(&url)
        .header("authorization", http_get(1, &url))
        .send()
        .await?;
    ensure!(
        response.status() == 200,
        "operator audit {}",
        response.text().await?
    );
    Ok(response.json().await?)
}
async fn chapter_query(c: &reqwest::Client, origin: &str, group: &str) -> Result<Value> {
    let mut url = url::Url::parse(&format!("{origin}/query"))?;
    url.query_pairs_mut()
        .append_pair("order", "asc")
        .append_pair("filter", &format!("publisher={group:?}"));
    get(c, url.as_str()).await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "real PostgreSQL and strfry; retry-safe group publications"]
async fn group_retry_retains_event_after_response_loss_and_grant_expiry() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    ensure!(
        dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test"),
        "fixture only"
    );
    let repo = Repository::connect(DbConfig {
        dsn: dsn.clone(),
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let dir = tempfile::tempdir()?;
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = l.local_addr()?;
    drop(l);
    let origin = format!("http://{bind}");
    let key = dir.path().join("service.key");
    write_key(&key, 11)?;
    let cfg = dir.path().join("node.json");
    std::fs::write(
        &cfg,
        serde_json::to_vec(
            &json!({"bind":bind.to_string(),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"group_keys_dir":dir.path().join("groups"),"storage":{"path":dir.path().join("objects"),"volume":"groupretry","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}}),
        )?,
    )?;
    let log = std::fs::File::create(dir.path().join("node.log"))?;
    let _node = Node(
        Command::new(
            std::env::var_os("MANGA_TEST_NODE_BIN")
                .unwrap_or_else(|| env!("CARGO_BIN_EXE_manga-node").into()),
        )
        .args(["serve", cfg.to_str().unwrap()])
        .env("MANGA_DATABASE_URL", &dsn)
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?,
    );
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let mut ready = false;
    for _ in 0..100 {
        if c.get(&origin)
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
        {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    ensure!(
        ready,
        "node failed: {}",
        std::fs::read_to_string(dir.path().join("node.log"))?
    );
    let r = post_json(
        &c,
        1,
        &format!("{origin}/compat/mangadex/group"),
        &json!({"administrator":public(2)}),
    )
    .await?;
    ensure!(r.status().is_success(), "create {}", r.text().await?);
    let group = r.json::<Value>().await?["data"]["nostr"]["group"]
        .as_str()
        .unwrap()
        .to_owned();
    let members = format!("{origin}/groups/{group}/members");
    assert_eq!(
        post_json(
            &c,
            2,
            &members,
            &json!({"member":public(3),"role":"publisher"})
        )
        .await?
        .status(),
        200
    );
    let image = png();
    let manifest =
        cbor(json!({"version":1,"series":"ab".repeat(32),"language":"en","pages":[page(&image)]}));
    for bytes in [&image, &manifest] {
        let url = format!("{origin}/groups/{group}/upload/{}", auth::hash(bytes));
        let response = c
            .post(&url)
            .header("authorization", http(3, &url, bytes))
            .body(bytes.clone())
            .send()
            .await?;
        ensure!(
            response.status().is_success(),
            "upload {}",
            response.text().await?
        );
    }
    let alias = mangadex_compat::uuid("group", &group);
    let url = format!("{origin}/compat/mangadex/group/{alias}/publish");
    let created_at = auth::now();
    let request =
        json!({"kind":1001,"manifest":auth::hash(&manifest),"version":1,"created_at":created_at});
    let body = serde_json::to_vec(&request)?;
    let token = http(3, &url, &body);
    let response = c
        .post(&url)
        .header("authorization", &token)
        .body(body.clone())
        .send()
        .await?;
    ensure!(
        response.status().is_success(),
        "publish {}",
        response.text().await?
    );
    let first: Value = response.json().await?;
    let event: auth::Event = serde_json::from_value(first["event"].clone())?;
    event.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(event.created_at, created_at);
    assert_eq!(event.pubkey, group);
    wait_projected(&c, &origin, &event.id).await?;
    assert_eq!(
        get(&c, &format!("{origin}/events/{}", event.id)).await?,
        first["event"]
    );
    assert_eq!(
        chapter_query(&c, &origin, &group).await?["events"],
        json!([first["event"]])
    );
    let first_audit = read_audit(&c, &origin, &group).await?;
    assert_eq!(first_audit["group"], group);
    assert!(first_audit["next_before_id"].is_null());
    let actions: Vec<_> = first_audit["items"].as_array().unwrap().iter().map(|item|
        json!({"member":item["member"], "action":item["action"], "event_id":item["event_id"]})
    ).collect();
    assert_eq!(
        actions,
        vec![json!({"member":public(3), "action":"publish", "event_id":event.id})]
    );
    let audit_url = format!("{origin}/admin/groups/{group}/audit");
    assert_eq!(c.get(&audit_url).send().await?.status(), 403);
    for key in [2, 3] {
        assert_eq!(
            c.get(&audit_url)
                .header("authorization", http_get(key, &audit_url))
                .send()
                .await?
                .status(),
            403
        );
    }
    // Losing the application response does not authorize replaying HTTP auth.
    assert_eq!(
        c.post(&url)
            .header("authorization", &token)
            .body(body)
            .send()
            .await?
            .status(),
        403
    );
    repo.pool.get().await?.execute("UPDATE group_blob_grants SET lease_until=clock_timestamp()-interval '1 second' WHERE group_key=$1",&[&hex::decode(&group)?]).await?;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let retried = post_json(&c, 3, &url, &request).await?;
    ensure!(
        retried.status().is_success(),
        "retry {}",
        retried.text().await?
    );
    let retried: Value = retried.json().await?;
    assert_eq!(retried["event"], first["event"]);
    assert_eq!(
        get(&c, &format!("{origin}/events/{}", event.id)).await?,
        first["event"]
    );
    assert_eq!(
        chapter_query(&c, &origin, &group).await?["events"],
        json!([first["event"]])
    );
    assert_eq!(read_audit(&c, &origin, &group).await?, first_audit);
    let original = c
        .get(format!("{origin}/originals/{}", auth::hash(&image)))
        .send()
        .await?;
    assert_eq!(original.status(), 200);
    assert_eq!(original.bytes().await?.as_ref(), image);
    // Stable identity cannot turn a different payload into a successful retry.
    let altered =
        json!({"kind":1001,"manifest":"ff".repeat(32),"version":1,"created_at":created_at});
    assert_eq!(post_json(&c, 3, &url, &altered).await?.status(), 400);
    let future = json!({"kind":1001,"manifest":auth::hash(&manifest),"version":1,"created_at":auth::now()+120});
    assert_eq!(post_json(&c, 3, &url, &future).await?.status(), 400);
    assert_eq!(
        post_json(&c, 2, &members, &json!({"member":public(3),"role":null}))
            .await?
            .status(),
        200
    );
    assert_eq!(post_json(&c, 3, &url, &request).await?.status(), 403);
    // The still-current administrator can acknowledge the same event; attribution
    // remains its first actor, never rewritten to the administrator doing retry.
    let admin = post_json(&c, 2, &url, &request).await?;
    ensure!(
        admin.status().is_success(),
        "admin retry {}",
        admin.text().await?
    );
    let admin: Value = admin.json().await?;
    assert_eq!(admin["event"], first["event"]);
    assert_eq!(
        get(&c, &format!("{origin}/events/{}", event.id)).await?,
        first["event"]
    );
    assert_eq!(
        chapter_query(&c, &origin, &group).await?["events"],
        json!([first["event"]])
    );
    assert_eq!(read_audit(&c, &origin, &group).await?, first_audit);
    let deletion = post_json(
        &c,
        2,
        &format!("{origin}/groups/{group}/deletion-requests"),
        &json!({"targets":[event.id],"reason":"fixture"}),
    )
    .await?;
    ensure!(
        deletion.status().is_success(),
        "deletion {}",
        deletion.text().await?
    );
    let deletion: Value = deletion.json().await?;
    let request_id = deletion["event"]["id"].as_str().unwrap();
    let approved = post_json(
        &c,
        1,
        &format!("{origin}/admin/deletions/{request_id}/approve"),
        &json!({}),
    )
    .await?;
    ensure!(
        approved.status().is_success(),
        "approve {}",
        approved.text().await?
    );
    assert_eq!(
        post_json(&c, 2, &url, &request).await?.status(),
        400,
        "same ID cannot resurrect a locally removed publication"
    );
    assert_eq!(
        c.get(format!("{origin}/events/{}", event.id))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        chapter_query(&c, &origin, &group).await?["events"],
        json!([])
    );
    let state = get(&c, &format!("{origin}/admin/status")).await;
    // Operator status is intentionally private even while other public reads exist.
    assert!(state.is_err());
    std::fs::write(
        root.join("results/http-group-retry.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["same persisted timestamp and publication payload return exact first event after grant expiry","fresh HTTP authorization required on every retry","exact signed event and chapter query remain unchanged after retries","operator-only HTTP audit retains the original publishing actor","original bytes remain available after grant expiry","different payload is not same identity","future timestamp denied","revoked publisher denied","administrator retry does not rewrite first actor","moderated deletion is not resurrected"],"scope":"optional created_at command identity; omitted timestamp retains legacy behavior; no global exactly-once guarantee"}),
        )?,
    )?;
    Ok(())
}
