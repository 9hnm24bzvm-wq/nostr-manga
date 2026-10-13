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
async fn post_as_group(
    c: &reqwest::Client,
    secret: &str,
    url: &str,
    event: &auth::Event,
) -> Result<reqwest::Response> {
    let body = serde_json::to_vec(event)?;
    let authorization = auth::sign(
        secret,
        27235,
        vec![
            vec!["u".into(), url.into()],
            vec!["method".into(), "POST".into()],
            vec!["payload".into(), auth::hash(&body)],
            vec!["nonce".into(), hex::encode(rand::random::<[u8; 16]>())],
        ],
        String::new(),
    )
    .map_err(anyhow::Error::msg)?;
    Ok(c.post(url)
        .header("authorization", token(&authorization))
        .body(body)
        .send()
        .await?)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "real PostgreSQL and strfry; managed public group profiles"]
async fn group_profiles_are_admin_fenced_signed_and_privately_audited() -> Result<()> {
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
            &json!({"bind":bind.to_string(),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"group_keys_dir":dir.path().join("groups"),"storage":{"path":dir.path().join("objects"),"volume":"groupprofile","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}}),
        )?,
    )?;
    let log = std::fs::File::create(dir.path().join("node.log"))?;
    let _node = Node(
        Command::new(env!("CARGO_BIN_EXE_manga-node"))
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
    let gid = mangadex_compat::uuid("group", &group);
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
    let url = format!("{origin}/compat/mangadex/group/{gid}/profile");
    let view = format!("{origin}/compat/mangadex/group/{gid}");
    assert_eq!(get(&c, &view).await?["data"]["attributes"]["name"], group);
    let first = json!({"metadata":{"name":"First Group","about":"Public description","website":"https://example.org"}});
    assert_eq!(post_json(&c, 3, &url, &first).await?.status(), 403);
    let body = serde_json::to_vec(&first)?;
    let token = http(2, &url, &body);
    let response = c
        .post(&url)
        .header("authorization", &token)
        .body(body.clone())
        .send()
        .await?;
    ensure!(
        response.status().is_success(),
        "profile {}",
        response.text().await?
    );
    let value: Value = response.json().await?;
    let event: auth::Event = serde_json::from_value(value["event"].clone())?;
    event.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(event.kind, 0);
    assert_eq!(event.pubkey, group);
    assert!(event.tags.is_empty());
    assert_eq!(value["private_key_returned"], false);
    assert_eq!(
        c.post(&url)
            .header("authorization", &token)
            .body(body)
            .send()
            .await?
            .status(),
        403
    );
    let public_profile = get(&c, &view).await?;
    assert_eq!(public_profile["data"]["attributes"]["name"], "First Group");
    assert_eq!(
        public_profile["data"]["attributes"]["description"],
        "Public description"
    );
    assert!(
        !public_profile
            .to_string()
            .contains(&public_key_for_member())
    );
    let searched = get(
        &c,
        &format!("{origin}/compat/mangadex/group?name=First%20Group"),
    )
    .await?;
    assert_eq!(searched["total"], 1);
    let second=post_json(&c,2,&url,&json!({"metadata":{"display_name":"Second Group","name":"Fallback"},"expected_head":event.id})).await?;
    ensure!(
        second.status().is_success(),
        "second {}",
        second.text().await?
    );
    let second: Value = second.json().await?;
    let next: auth::Event = serde_json::from_value(second["event"].clone())?;
    next.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(next.pubkey, group);
    assert!(next.created_at > event.created_at);
    assert_eq!(
        post_json(
            &c,
            2,
            &url,
            &json!({"metadata":{"name":"Stale"},"expected_head":event.id})
        )
        .await?
        .status(),
        409
    );
    assert_eq!(
        get(&c, &view).await?["data"]["attributes"]["name"],
        "Second Group"
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
    let profile_audit = read_audit(&c, &origin, &group).await?;
    assert_eq!(profile_audit["group"], group);
    assert!(profile_audit["next_before_id"].is_null());
    let actions: Vec<_> = profile_audit["items"].as_array().unwrap().iter().map(|item|
        json!({"member":item["member"], "action":item["action"], "event_id":item["event_id"]})
    ).collect();
    assert_eq!(
        actions,
        vec![
            json!({"member":public(2), "action":"profile", "event_id":next.id}),
            json!({"member":public(2), "action":"profile", "event_id":event.id}),
        ]
    );
    // An external audit-write fault must leave the public profile and audit unchanged.
    repo.pool.get().await?.batch_execute("ALTER TABLE group_action_audit ADD CONSTRAINT profile_audit_failure CHECK(action<>'profile') NOT VALID").await?;
    assert_eq!(
        post_json(
            &c,
            2,
            &url,
            &json!({"metadata":{"name":"Must Roll Back"},"expected_head":next.id})
        )
        .await?
        .status(),
        503
    );
    assert_eq!(
        get(&c, &view).await?["data"]["nostr"]["profile"]["id"],
        next.id
    );
    repo.pool
        .get()
        .await?
        .batch_execute("ALTER TABLE group_action_audit DROP CONSTRAINT profile_audit_failure")
        .await?;
    assert_eq!(read_audit(&c, &origin, &group).await?, profile_audit);
    wait_projected(&c, &origin, &next.id).await?;
    assert_eq!(
        get(&c, &format!("{origin}/events/{}", next.id)).await?,
        second["event"]
    );
    assert_eq!(
        get(&c, &format!("{origin}/accounts/{group}/metadata/0")).await?["id"],
        next.id
    );

    // Use only this fixture's generated key to make correctly signed wire input.
    // Profile management must not silently admit independent account posting.
    let secret = std::fs::read_to_string(dir.path().join("groups").join(format!("{group}.key")))?;
    let account_event = auth::sign(
        secret.trim(),
        10002,
        vec![vec!["r".into(), "wss://relay.example.org".into()]],
        String::new(),
    )
    .map_err(anyhow::Error::msg)?;
    let account_url = format!("{origin}/account-metadata");
    assert_eq!(
        post_as_group(&c, secret.trim(), &account_url, &account_event)
            .await?
            .status(),
        403
    );
    assert_eq!(
        c.get(format!("{origin}/events/{}", account_event.id))
            .send()
            .await?
            .status(),
        404
    );
    let admission_url = format!("{origin}/admin/accounts/{group}");
    assert_eq!(
        post_json(&c, 1, &admission_url, &json!({"allowed":true}))
            .await?
            .status(),
        200
    );
    let admitted = post_as_group(&c, secret.trim(), &account_url, &account_event).await?;
    ensure!(
        admitted.status().is_success(),
        "explicit account admission {}",
        admitted.text().await?
    );
    wait_projected(&c, &origin, &account_event.id).await?;
    assert_eq!(
        get(&c, &format!("{origin}/events/{}", account_event.id)).await?,
        serde_json::to_value(&account_event)?
    );
    assert_eq!(
        post_json(&c, 1, &admission_url, &json!({"allowed":false}))
            .await?
            .status(),
        200
    );
    assert_eq!(
        post_as_group(&c, secret.trim(), &account_url, &account_event)
            .await?
            .status(),
        403
    );

    // The independently authorized member can still publish original-backed
    // chapters under the managed identity without general account admission.
    let image = png();
    let manifest =
        cbor(json!({"version":1,"series":"ab".repeat(32),"language":"en","pages":[page(&image)]}));
    for bytes in [&image, &manifest] {
        let upload_url = format!("{origin}/groups/{group}/upload/{}", auth::hash(bytes));
        let response = c
            .post(&upload_url)
            .header("authorization", http(3, &upload_url, bytes))
            .body(bytes.clone())
            .send()
            .await?;
        ensure!(
            response.status().is_success(),
            "member upload {}",
            response.text().await?
        );
    }
    let publication = post_json(
        &c,
        3,
        &format!("{origin}/groups/{group}/publish"),
        &json!({"kind":1001,"manifest":auth::hash(&manifest),"version":1}),
    )
    .await?;
    ensure!(
        publication.status().is_success(),
        "member publication {}",
        publication.text().await?
    );
    let publication: Value = publication.json().await?;
    let chapter: auth::Event = serde_json::from_value(publication["event"].clone())?;
    chapter.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(chapter.pubkey, group);
    wait_projected(&c, &origin, &chapter.id).await?;
    assert_eq!(
        get(&c, &format!("{origin}/events/{}", chapter.id)).await?,
        publication["event"]
    );
    assert_eq!(
        post_json(&c, 2, &members, &json!({"member":public(4),"role":"admin"}))
            .await?
            .status(),
        200
    );
    assert_eq!(
        post_json(&c, 4, &members, &json!({"member":public(2),"role":null}))
            .await?
            .status(),
        200
    );
    assert_eq!(post_json(&c, 2, &url, &first).await?.status(), 403);
    assert_eq!(
        post_json(
            &c,
            1,
            &format!("{origin}/admin/groups/{group}/enabled"),
            &json!({"enabled":false})
        )
        .await?
        .status(),
        200
    );
    assert_eq!(post_json(&c, 4, &url, &first).await?.status(), 403);
    assert_eq!(
        post_json(
            &c,
            1,
            &format!("{origin}/admin/groups/{group}/enabled"),
            &json!({"enabled":true})
        )
        .await?
        .status(),
        200
    );
    assert_eq!(
        post_json(&c, 4, &url, &json!({"metadata":{}}))
            .await?
            .status(),
        200
    );
    assert_eq!(get(&c, &view).await?["data"]["attributes"]["name"], group);
    assert_eq!(
        get(&c, &format!("{origin}/events/{}", chapter.id)).await?,
        publication["event"]
    );
    let original = c
        .get(format!("{origin}/originals/{}", auth::hash(&image)))
        .send()
        .await?;
    assert_eq!(original.status(), 200);
    assert_eq!(original.bytes().await?.as_ref(), image);
    std::fs::write(
        root.join("results/http-group-profile.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["admin-only kind0 signing","no private-key or member attribution in public profile","monotonic timestamp and expected-head conflict","profile name discovery","operator-only HTTP audit preserves exact profile identities and actors","audit failure leaves signed profile, public account head and HTTP audit unchanged","revocation and disabled-group fences","general account posting denied until separate explicit admission and denied after revocation","authorized member publishes despite revoked general account admission","empty metadata clears descriptive fields without deleting key or chapters"]}),
        )?,
    )?;
    Ok(())
}
fn public_key_for_member() -> String {
    public(2)
}
