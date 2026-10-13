mod support;
use anyhow::{Context, Result};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
};
use serde_json::{Value, json};
use std::{
    process::{Child, Command},
    time::Duration,
};
use support::*;
const A: &str = "http://127.0.0.1:19540";
const B: &str = "http://127.0.0.1:19541";
struct Node(Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
async fn repo(dsn: String) -> Result<Repository> {
    Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await
}
async fn ready(c: &reqwest::Client, origin: &str) -> Result<()> {
    for _ in 0..100 {
        if c.get(origin).send().await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    anyhow::bail!("node failed to start")
}
async fn read_audit(c: &reqwest::Client, group: &str) -> Result<Value> {
    let url = format!("{A}/admin/groups/{group}/audit");
    let response = c
        .get(&url)
        .header("authorization", http_get(1, &url))
        .send()
        .await?;
    anyhow::ensure!(
        response.status() == 200,
        "operator audit response {}",
        response.status()
    );
    Ok(response.json().await?)
}
async fn chapter_query(c: &reqwest::Client, group: &str) -> Result<Value> {
    let mut url = url::Url::parse(&format!("{A}/query"))?;
    url.query_pairs_mut()
        .append_pair("order", "asc")
        .append_pair("filter", &format!("publisher={group:?}"));
    let response = c
        .get(url.as_str())
        .header("authorization", http_get(1, url.as_str()))
        .send()
        .await?;
    anyhow::ensure!(
        response.status() == 200,
        "chapter query response {}",
        response.status()
    );
    Ok(response.json().await?)
}
async fn member(c: &reqwest::Client, group: &str, member: u8, role: Value) -> Result<()> {
    assert_eq!(
        post_json(
            c,
            2,
            &format!(
                "{A}/compat/mangadex/group/{}/members",
                manga_node::mangadex_compat::uuid("group", group)
            ),
            &json!({"member":public(member),"role":role})
        )
        .await?
        .status(),
        200
    );
    Ok(())
}
async fn upload(c: &reqwest::Client, group: &str, key: u8, bytes: &[u8]) -> Result<String> {
    let hash = auth::hash(bytes);
    let url = format!(
        "{A}/compat/mangadex/group/{}/upload/{hash}",
        manga_node::mangadex_compat::uuid("group", group)
    );
    let r = c
        .post(&url)
        .header("authorization", http(key, &url, bytes))
        .body(bytes.to_vec())
        .send()
        .await?;
    anyhow::ensure!(
        r.status() == 200,
        "group upload failed: {}",
        r.text().await?
    );
    Ok(hash)
}
async fn publish(
    c: &reqwest::Client,
    origin: &str,
    group: &str,
    key: u8,
    manifest: &str,
) -> Result<reqwest::Response> {
    post_json(
        c,
        key,
        &if origin == A {
            format!(
                "{origin}/compat/mangadex/group/{}/publish",
                manga_node::mangadex_compat::uuid("group", group)
            )
        } else {
            format!("{origin}/groups/{group}/publish")
        },
        &json!({"kind":1001,"manifest":manifest,"version":1}),
    )
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and real strfry; run scripts/test-postgres.sh"]
async fn member_roles_signing_restart_and_key_only_recovery() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(
        cfg.get_dbname() == Some("manga_test") && dsn.contains("dbname=manga_test"),
        "fixture required"
    );
    let source = repo(dsn.clone()).await?;
    source.migrate().await?;
    source
        .pool
        .get()
        .await?
        .batch_execute("CREATE DATABASE manga_group_recovery")
        .await?;
    let bdsn = dsn.replace("dbname=manga_test", "dbname=manga_group_recovery");
    let recovered = repo(bdsn.clone()).await?;
    recovered.migrate().await?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let dir = tempfile::tempdir()?;
    let binary = std::env::var_os("STRFRY_BIN")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("vendor/strfry/strfry"));
    let mut paths = vec![];
    let mut key_dirs = vec![];
    for (n, origin) in [A, B].iter().enumerate() {
        let base = dir.path().join(format!("node-{n}"));
        std::fs::create_dir_all(&base)?;
        let key = base.join("key");
        write_key(&key, 11 + n as u8)?;
        let path = base.join("config.json");
        let groups = base.join("group-keys");
        std::fs::write(
            &path,
            serde_json::to_vec(
                &json!({"bind":format!("127.0.0.1:{}",19540+n),"base_url":origin,"service_key_file":key,"admin_pubkey":public(if n==0{1}else{5}),"database_plaintext_local":true,"group_keys_dir":groups,"backend":{"binary":binary,"data_dir":base.join("backend"),"timeout_seconds":3,"map_bytes":67108864},"storage":{"path":base.join("originals"),"volume":format!("groups{n}"),"physical_bytes":2097152,"staging_bytes":65536,"max_blob_bytes":262144}}),
            )?,
        )?;
        paths.push(path);
        key_dirs.push(groups);
    }
    let spawn = |n: usize| -> Result<Node> {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.path().join(format!("node-{n}/node.log")))?;
        Ok(Node(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", paths[n].to_str().unwrap()])
                .env("MANGA_DATABASE_URL", if n == 0 { &dsn } else { &bdsn })
                .env("RUST_LOG", "warn")
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        ))
    };
    let a = spawn(0)?;
    let _b = spawn(1)?;
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    ready(&c, A).await?;
    ready(&c, B).await?;
    let created: Value = post_json(
        &c,
        1,
        &format!("{A}/compat/mangadex/group"),
        &json!({"administrator":public(2)}),
    )
    .await?
    .json()
    .await?;
    let created = created["data"]["nostr"].clone();
    let group = created["group"].as_str().context("group key")?.to_owned();
    assert_eq!(created["private_key_returned"], false);
    let gid = manga_node::mangadex_compat::uuid("group", &group);
    let groups: Value = c
        .get(format!("{A}/compat/mangadex/group?ids[]={gid}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(groups["total"], 1);
    assert!(
        !groups.to_string().contains(&public(2)),
        "public group records do not leak member identities"
    );
    let one: Value = c
        .get(format!("{A}/compat/mangadex/group/{gid}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(one["data"]["nostr"]["pubkey"], group);

    member(&c, &group, 3, json!("publisher")).await?;
    member(&c, &group, 4, json!("remove")).await?;
    let members_url = format!(
        "{A}/compat/mangadex/group/{}/members",
        manga_node::mangadex_compat::uuid("group", &group)
    );
    assert_eq!(
        c.get(&members_url)
            .header("authorization", http_get(2, &members_url))
            .send()
            .await?
            .status(),
        200
    );
    assert_eq!(
        c.get(&members_url)
            .header("authorization", http_get(3, &members_url))
            .send()
            .await?
            .status(),
        403
    );

    assert_eq!(
        post_json(
            &c,
            3,
            &format!("{A}/groups/{group}/members"),
            &json!({"member":public(3),"role":"admin"})
        )
        .await?
        .status(),
        403
    );
    assert_eq!(
        post_json(
            &c,
            2,
            &format!("{A}/groups/{group}/members"),
            &json!({"member":public(4)})
        )
        .await?
        .status(),
        400,
        "missing role is not implicit revocation"
    );
    let image = png();
    let hash = auth::hash(&image);
    let url = format!("{A}/groups/{group}/upload/{hash}");
    assert_eq!(
        c.post(&url)
            .header("authorization", http(4, &url, &image))
            .body(image.clone())
            .send()
            .await?
            .status(),
        403
    );
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut socket = tokio::net::TcpStream::connect("127.0.0.1:19540").await?;
    socket.write_all(format!("POST /groups/{group}/upload/{hash} HTTP/1.1\r\nHost:127.0.0.1:19540\r\nAuthorization: {}\r\nContent-Length:10000000\r\nConnection:close\r\n\r\n",http(6,&url,&[])).as_bytes()).await?;
    let mut buffer = [0u8; 1024];
    let n = tokio::time::timeout(Duration::from_secs(1), socket.read(&mut buffer)).await??;
    assert!(String::from_utf8_lossy(&buffer[..n]).starts_with("HTTP/1.1 403"));
    upload(&c, &group, 3, &image).await?;
    let manifest =
        cbor(json!({"version":1,"series":"ab".repeat(32),"language":"en","pages":[page(&image)]}));
    let mh = upload(&c, &group, 3, &manifest).await?;
    // A failed attribution write must roll back the hosted transition too.
    let before_publish_audit = read_audit(&c, &group).await?;
    source.pool.get().await?.batch_execute("ALTER TABLE group_action_audit ADD CONSTRAINT fixture_audit_failure CHECK(false) NOT VALID").await?;
    let failed = publish(&c, A, &group, 3, &mh).await?;
    source
        .pool
        .get()
        .await?
        .batch_execute("ALTER TABLE group_action_audit DROP CONSTRAINT fixture_audit_failure")
        .await?;
    assert_eq!(failed.status(), 503);
    assert!(
        chapter_query(&c, &group).await?["events"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(read_audit(&c, &group).await?, before_publish_audit);

    let response = publish(&c, A, &group, 3, &mh).await?;
    assert!(response.status().is_success(), "{}", response.text().await?);
    let response: Value = response.json().await?;
    let event: auth::Event = serde_json::from_value(response["event"].clone())?;
    event.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(event.pubkey, group);
    assert!(event.tags.is_empty());
    wait_projected(&c, A, &event.id).await?;
    let actual = c
        .get(format!("{A}/originals/{hash}"))
        .send()
        .await?
        .bytes()
        .await?;
    assert_eq!(actual.as_ref(), image);
    for request in [
        json!({"kind":27235,"manifest":mh,"version":1}),
        json!({"kind":1001,"manifest":mh,"version":1,"content":"sign arbitrary data"}),
    ] {
        assert_eq!(
            post_json(&c, 3, &format!("{A}/groups/{group}/publish"), &request)
                .await?
                .status(),
            400
        );
    }
    assert_eq!(
        post_json(
            &c,
            3,
            &format!("{A}/groups/{group}/deletion-requests"),
            &json!({"targets":[event.id]})
        )
        .await?
        .status(),
        403
    );
    let before_deletion_audit = read_audit(&c, &group).await?;
    source.pool.get().await?.batch_execute("ALTER TABLE group_action_audit ADD CONSTRAINT fixture_audit_failure CHECK(action<>'deletion_request') NOT VALID").await?;
    let failed = post_json(
        &c,
        4,
        &format!("{A}/groups/{group}/deletion-requests"),
        &json!({"targets":[event.id],"reason":"fixture removal"}),
    )
    .await?;
    source
        .pool
        .get()
        .await?
        .batch_execute("ALTER TABLE group_action_audit DROP CONSTRAINT fixture_audit_failure")
        .await?;
    assert_eq!(failed.status(), 503);
    assert_eq!(read_audit(&c, &group).await?, before_deletion_audit);
    assert_eq!(
        c.get(format!("{A}/events/{}", event.id))
            .send()
            .await?
            .status(),
        200
    );

    let deletion: Value = post_json(
        &c,
        4,
        &format!("{A}/groups/{group}/deletion-requests"),
        &json!({"targets":[event.id],"reason":"fixture removal"}),
    )
    .await?
    .json()
    .await?;
    let deletion: auth::Event = serde_json::from_value(deletion["event"].clone())?;
    deletion.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(deletion.pubkey, group);
    assert_eq!(deletion.kind, 5);
    assert_eq!(
        c.get(format!("{A}/events/{}", event.id))
            .send()
            .await?
            .status(),
        200,
        "group request still needs local moderation"
    );
    assert!(
        deletion
            .tags
            .iter()
            .all(|tag| tag.first().is_some_and(|name| name == "e"))
    );
    let audit_url = format!("{A}/admin/groups/{group}/audit?limit=1");
    assert_eq!(c.get(&audit_url).send().await?.status(), 403);
    for key in [2, 3, 4] {
        assert_eq!(
            c.get(&audit_url)
                .header("authorization", http_get(key, &audit_url))
                .send()
                .await?
                .status(),
            403
        );
    }
    let audit: Value = c
        .get(&audit_url)
        .header("authorization", http_get(1, &audit_url))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(audit["items"].as_array().unwrap().len(), 1);
    assert_eq!(audit["items"][0]["action"], "deletion_request");
    assert_eq!(audit["items"][0]["member"], public(4));
    assert_eq!(audit["items"][0]["event_id"], deletion.id);
    let cursor = audit["next_before_id"].as_i64().unwrap();
    let next_url = format!("{A}/admin/groups/{group}/audit?limit=1&before_id={cursor}");
    let page: Value = c
        .get(&next_url)
        .header("authorization", http_get(1, &next_url))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(page["items"][0]["action"], "publish");
    assert_eq!(page["items"][0]["member"], public(3));
    assert_eq!(page["items"][0]["event_id"], event.id);
    assert!(page["next_before_id"].is_null());
    for query in ["limit=0", "limit=101", "before_id=-1"] {
        let url = format!("{A}/admin/groups/{group}/audit?{query}");
        assert_eq!(
            c.get(&url)
                .header("authorization", http_get(1, &url))
                .send()
                .await?
                .status(),
            400
        );
    }
    // Another group cannot publish this group's private staging by knowing a hash.
    let other: Value = post_json(
        &c,
        1,
        &format!("{A}/admin/groups"),
        &json!({"administrator":public(2)}),
    )
    .await?
    .json()
    .await?;
    let other = other["group"].as_str().unwrap();
    assert_eq!(publish(&c, A, other, 2, &mh).await?.status(), 400);
    // A publisher removed while its request body is still arriving cannot obtain a grant.
    let bytes = cbor(json!({"version":1,"titles":{"en":"Rejected grant cannot publish"}}));
    manga_node::manifest::series(&bytes).map_err(anyhow::Error::msg)?;
    let h = auth::hash(&bytes);
    let url = format!("{A}/groups/{group}/upload/{h}");
    let mut socket = tokio::net::TcpStream::connect("127.0.0.1:19540").await?;
    socket.write_all(format!("POST /groups/{group}/upload/{h} HTTP/1.1\r\nHost:127.0.0.1:19540\r\nAuthorization: {}\r\nContent-Length:{}\r\nExpect: 100-continue\r\nConnection:close\r\n\r\n",http(3,&url,&bytes),bytes.len()).as_bytes()).await?;
    // HTTP 100 Continue is emitted when the accepted request starts reading its
    // body. Revoke after this real protocol barrier, not a scheduler sleep.
    let interim = tokio::time::timeout(Duration::from_secs(2), async {
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            anyhow::ensure!(header.len() < 1024, "interim header budget");
            header.push(socket.read_u8().await?);
        }
        Ok::<_, anyhow::Error>(header)
    })
    .await??;
    assert!(String::from_utf8_lossy(&interim).starts_with("HTTP/1.1 100 Continue"));
    member(&c, &group, 3, Value::Null).await?;
    socket.write_all(&bytes).await?;
    let n = tokio::time::timeout(Duration::from_secs(2), socket.read(&mut buffer)).await??;
    let response = String::from_utf8_lossy(&buffer[..n]);
    assert!(response.starts_with("HTTP/1.1 400"), "{response}");

    // The payload is a valid series manifest; an authorized administrator still
    // cannot publish it because the revoked transfer must confer no staging grant.
    assert_eq!(
        post_json(
            &c,
            2,
            &format!("{A}/groups/{group}/publish"),
            &json!({"kind":1002,"manifest":h,"version":1})
        )
        .await?
        .status(),
        400
    );
    drop(a);
    let _a = spawn(0)?;
    ready(&c, A).await?;
    assert_eq!(publish(&c, A, &group, 3, &mh).await?.status(), 403);
    let url = format!("{A}/admin/groups/{group}/audit");
    let restarted: Value = c
        .get(&url)
        .header("authorization", http_get(1, &url))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(restarted["items"].as_array().unwrap().len(), 2);

    let url = format!("{A}/groups/{group}/members");
    let members: Value = c
        .get(&url)
        .header("authorization", http_get(2, &url))
        .send()
        .await?
        .json()
        .await?;
    assert!(
        !members["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["member"] == public(3))
    );
    let key_id: [u8; 32] = hex::decode(&group)?.try_into().unwrap();
    let key_path = recovered_key_path(&key_dirs[0], key_id);
    let secret = std::fs::read_to_string(&key_path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&key_path)?.permissions().mode() & 0o077,
            0
        );
    }
    let recovered_path = recovered_key_path(&key_dirs[1], key_id);
    let backup = Command::new(env!("CARGO_BIN_EXE_manga-key-tool"))
        .arg("backup")
        .arg(&key_path)
        .arg(&recovered_path)
        .arg(&group)
        .output()?;
    assert!(
        backup.status.success(),
        "offline backup failed: {}",
        String::from_utf8_lossy(&backup.stderr)
    );
    let text = String::from_utf8(backup.stdout)?;
    assert!(
        !text.contains(secret.trim()),
        "backup must not print private key"
    );
    let saved: Value = serde_json::from_str(&text)?;
    assert_eq!(saved["verified"], true);
    assert_eq!(saved["public_key"], group);
    assert_eq!(saved["private_key_returned"], false);
    assert_eq!(saved["network_used"], false);
    let verified = Command::new(env!("CARGO_BIN_EXE_manga-key-tool"))
        .arg("verify")
        .arg(&recovered_path)
        .arg(&group)
        .output()?;
    assert!(verified.status.success(), "offline backup readback failed");
    let imported: Value = post_json(
        &c,
        5,
        &format!("{B}/admin/groups/import"),
        &json!({"key":group}),
    )
    .await?
    .json()
    .await?;
    assert_eq!(imported["membership_recreated"], false);

    let members_url = format!("{B}/groups/{group}/members");
    for former_member in [2, 3, 4] {
        assert_eq!(
            c.get(&members_url)
                .header("authorization", http_get(former_member, &members_url))
                .send()
                .await?
                .status(),
            403
        );
    }
    assert_eq!(publish(&c, B, &group, 2, &mh).await?.status(), 403);
    assert_eq!(
        post_json(
            &c,
            5,
            &format!("{B}/admin/groups/{group}/bootstrap"),
            &json!({"administrator":public(6)})
        )
        .await?
        .status(),
        200
    );
    let url = format!("{B}/groups/{group}/members");
    let members: Value = c
        .get(&url)
        .header("authorization", http_get(6, &url))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(members["members"].as_array().unwrap().len(), 1);
    assert_eq!(members["members"][0]["member"], public(6));
    assert_eq!(publish(&c, B, &group, 2, &mh).await?.status(), 403);
    drop(_a);
    assert!(
        c.get(A).send().await.is_err(),
        "founder instance is offline"
    );
    // A recovered signer can resume public metadata under the same group
    // identity, but only through the explicitly bootstrapped local admin.
    let profile_url = format!("{B}/groups/{group}/profile");
    assert_eq!(
        post_json(
            &c,
            2,
            &profile_url,
            &json!({"metadata":{"name":"Old admin"}})
        )
        .await?
        .status(),
        403
    );
    let response = post_json(
        &c,
        6,
        &profile_url,
        &json!({"metadata":{"name":"Recovered group"}}),
    )
    .await?;
    anyhow::ensure!(
        response.status().is_success(),
        "recovered profile: {}",
        response.text().await?
    );
    let profile: Value = response.json().await?;
    let profile_event: auth::Event = serde_json::from_value(profile["event"].clone())?;
    profile_event.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(profile_event.pubkey, group);
    assert_eq!(profile_event.kind, 0);
    assert_eq!(profile["private_key_returned"], false);
    // With the founder offline, the recovered administrator can stage complete
    // local originals and continue publishing under the same group identity.
    let replacement_manifest = cbor(
        json!({"version":1,"series":"ab".repeat(32),"language":"en","chapter":"after-founder-loss","pages":[support::page(&image)]}),
    );
    for bytes in [&image, &replacement_manifest] {
        let url = format!("{B}/groups/{group}/upload/{}", auth::hash(bytes));
        let response = c
            .post(&url)
            .header("authorization", http(6, &url, bytes))
            .body(bytes.clone())
            .send()
            .await?;
        anyhow::ensure!(
            response.status().is_success(),
            "recovered original upload: {}",
            response.text().await?
        );
    }
    let response = publish(&c, B, &group, 6, &auth::hash(&replacement_manifest)).await?;
    anyhow::ensure!(
        response.status().is_success(),
        "recovered publication: {}",
        response.text().await?
    );
    let value: Value = response.json().await?;
    let continued: auth::Event = serde_json::from_value(value["event"].clone())?;
    continued.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(continued.pubkey, group);
    wait_projected(&c, B, &continued.id).await?;
    let bytes = c
        .get(format!("{B}/originals/{}", auth::hash(&image)))
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    assert_eq!(bytes.as_ref(), image);
    std::fs::write(
        root.join("results/postgres-http-groups.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["local operator-only member attribution commits atomically, paginates and survives restart without public tags","instance key signs group event for authorized personal publisher","role checks occur before untrusted upload body","publisher cannot administer members or request removal","remove-only member cannot publish or upload","purpose-specific API rejects arbitrary kinds and content","signed group deletion remains moderator-pending","group staging grants prevent cross-group private-hash use","revocation during upload prevents group grant","revocation persists after real process restart","private key file restricted and PostgreSQL contains only signer reference","offline backup CLI verifies identity without exposing secret or using network; import restores no membership","explicit recovery bootstrap establishes only requested new administrator","recovered group profile retains signing identity and rejects old administrator","after founder shutdown the new administrator stages originals and publishes a new chapter with the recovered group identity"],"limits":["synthetic keys in ephemeral loopback fixtures","custodial files are not an HSM or encrypted vault","copying a key does not revoke another holder"]}),
        )?,
    )?;
    Ok(())
}
