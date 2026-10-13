mod support;
use anyhow::{Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    library_client_bundle::SaveBundle,
    library_manifest::{Chunk, Descriptor, Family, FamilyRoot, Limits, Record, Root},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
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
fn bundle(
    key: u8,
    time: u64,
    series: [u8; 32],
    chapter: [u8; 32],
    name: Option<&str>,
    completed: bool,
) -> Result<SaveBundle> {
    let limits = Limits::default();
    let mut chunks = BTreeMap::new();
    let mut families = vec![];
    for family in Family::ALL {
        let records = match family {
            Family::Chapters => vec![Record::Chapter {
                id: chapter,
                completed,
                page: completed.then_some(12),
                updated_at: 1,
            }],
            Family::Series => vec![Record::Series {
                id: series,
                status: if completed { "reading" } else { "completed" }.into(),
                updated_at: 1,
            }],
            Family::Lists => name
                .map(|name| {
                    vec![Record::List {
                        id: [5; 16],
                        name: name.into(),
                        updated_at: 1,
                    }]
                })
                .unwrap_or_default(),
            Family::Members => name
                .map(|_| {
                    vec![Record::Member {
                        list: [5; 16],
                        series,
                        position: 7,
                        updated_at: 1,
                    }]
                })
                .unwrap_or_default(),
        };
        let descriptor = if records.is_empty() {
            Descriptor::empty()
        } else {
            let bytes = Chunk::new(family, records, &limits)?.encode(&limits)?;
            let summary = Chunk::decode(&bytes, &limits)?.1;
            chunks.insert(summary.hash(), bytes);
            Descriptor::from(&summary)
        };
        families.push(FamilyRoot {
            family,
            chunks: vec![descriptor],
        });
    }
    let root = Root { families }.encode(&limits)?;
    let event = sign_at(
        key,
        11999,
        vec![],
        json!({"manifest":auth::hash(&root),"version":1}).to_string(),
        time,
    );
    SaveBundle::validate(event, root, chunks)
}
async fn save(c: &reqwest::Client, origin: &str, key: u8, bundle: &SaveBundle) -> Result<()> {
    let mut cursor = 0;
    while let Some(batch) = bundle.batch(cursor)? {
        let url = format!("{origin}/compat/mangadex/nostr/library/batches");
        let r = c
            .post(&url)
            .header("authorization", http(key, &url, &batch.body))
            .body(batch.body)
            .send()
            .await?;
        ensure!(r.status().is_success(), "batch failed: {}", r.text().await?);
        cursor = batch.next_object;
    }
    let url = format!("{origin}/compat/mangadex/nostr/libraries");
    let body = bundle.commit_body()?;
    let r = c
        .post(&url)
        .header("authorization", http(key, &url, &body))
        .body(body)
        .send()
        .await?;
    ensure!(
        r.status().is_success(),
        "commit failed: {}",
        r.text().await?
    );
    Ok(())
}
async fn get(c: &reqwest::Client, url: &str, key: Option<u8>) -> Result<Value> {
    let mut q = c.get(url);
    if let Some(k) = key {
        q = q.header("authorization", http_get(k, url))
    }
    let r = q.send().await?;
    ensure!(
        r.status().is_success(),
        "GET {url}: {} {}",
        r.status(),
        r.text().await?
    );
    Ok(r.json().await?)
}
fn start(config: &std::path::Path, dsn: &str, log: &std::path::Path) -> Result<Node> {
    let log = std::fs::File::create(log)?;
    Ok(Node(
        Command::new(env!("CARGO_BIN_EXE_manga-node"))
            .args(["serve", config.to_str().unwrap()])
            .env("MANGA_DATABASE_URL", dsn)
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    ))
}
async fn ready(c: &reqwest::Client, origin: &str) -> Result<()> {
    for _ in 0..100 {
        if c.get(origin)
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    anyhow::bail!("node readiness timeout")
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated PostgreSQL and real strfry; reader enrollment lifecycle"]
async fn closed_default_scoped_enrollment_limits_and_revocation() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    ensure!(
        dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test"),
        "isolated fixture"
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
    let socket = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = socket.local_addr()?;
    drop(socket);
    let origin = format!("http://{bind}");
    let key = dir.path().join("service.key");
    write_key(&key, 11)?;
    let cfg = dir.path().join("node.json");
    let mut config = json!({"bind":bind.to_string(),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"read_policy":"admitted","database_plaintext_local":true,"sync_worker_enabled":false,"storage":{"path":dir.path().join("objects"),"volume":"enrollment","physical_bytes":10485760,"staging_bytes":10485760,"max_blob_bytes":1048576},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}});
    std::fs::write(&cfg, serde_json::to_vec(&config)?)?;
    let mut node = Some(start(&cfg, &dsn, &dir.path().join("node.log"))?);
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    ready(&c, &origin).await?;
    let url = format!("{origin}/enrollment");
    let metadata = sign_at(
        2,
        0,
        vec![],
        json!({"name":"New reader"}).to_string(),
        auth::now(),
    );

    assert_eq!(get(&c, &url, None).await?["enabled"], false);
    assert_eq!(post_json(&c, 2, &url, &json!({})).await?.status(), 403);
    assert_eq!(
        post_json(
            &c,
            2,
            &format!("{origin}/account-metadata"),
            &serde_json::to_value(&metadata)?
        )
        .await?
        .status(),
        403
    );
    let protected_query = format!("{origin}/query");
    assert_eq!(
        c.get(&protected_query)
            .header("authorization", http_get(2, &protected_query))
            .send()
            .await?
            .status(),
        403
    );
    drop(node.take());
    config["enrollment"] =
        json!({"enabled":true,"max_new_accounts_per_hour":2,"max_self_enrolled_accounts":3});
    std::fs::write(&cfg, serde_json::to_vec(&config)?)?;
    node = Some(start(&cfg, &dsn, &dir.path().join("node.log"))?);
    ready(&c, &origin).await?;
    assert_eq!(get(&c, &url, None).await?["enabled"], true);
    let capabilities = get(&c, &origin, None).await?;
    assert!(capabilities.to_string().contains("self_registration"));
    assert_eq!(c.post(&url).body("{}").send().await?.status(), 403);
    let body = serde_json::to_vec(&json!({}))?;
    let token = http(2, &url, &body);
    assert_eq!(
        c.post(&url)
            .header("authorization", &token)
            .body("{\"unexpected\":true}")
            .send()
            .await?
            .status(),
        403
    );
    let response = c
        .post(&url)
        .header("authorization", &token)
        .body(body.clone())
        .send()
        .await?;
    ensure!(
        response.status() == 201,
        "enrollment {}",
        response.text().await?
    );
    let first: Value = response.json().await?;
    assert_eq!(first["publisher_permission_granted"], false);
    assert_eq!(first["account"]["publisher"], public(2));
    assert_eq!(
        c.post(&url)
            .header("authorization", &token)
            .body(body)
            .send()
            .await?
            .status(),
        403
    );
    let version = first["account"]["permission_version"].clone();
    let mut tasks = vec![];
    for _ in 0..8 {
        let c = c.clone();
        let url = url.clone();
        tasks.push(tokio::spawn(async move {
            let response = post_json(&c, 2, &url, &json!({})).await?;
            ensure!(
                response.status() == 200,
                "duplicate enrollment {}",
                response.text().await?
            );
            response.json::<Value>().await.map_err(anyhow::Error::from)
        }));
    }
    for task in tasks {
        let value = task.await??;
        assert_eq!(value["account"]["created"], false);
        assert_eq!(value["account"]["permission_version"], version);
    }
    // Reader enrollment is not a publishing grant: valid publication/upload
    // requests must be denied at their real HTTP admission boundaries.
    let image = png();
    let manifest =
        cbor(json!({"version":1,"series":"ab".repeat(32),"language":"en","pages":[page(&image)]}));
    let publication = publication(2, 1001, &manifest);
    assert_eq!(
        post_json(
            &c,
            2,
            &format!("{origin}/publications"),
            &serde_json::to_value(&publication)?
        )
        .await?
        .status(),
        403
    );
    let upload_url = format!("{origin}/upload/{}", auth::hash(&image));
    assert_eq!(
        c.put(&upload_url)
            .header(
                "authorization",
                blossom(2, &origin, "upload", &auth::hash(&image))
            )
            .body(image)
            .send()
            .await?
            .status(),
        403
    );
    assert_eq!(
        post_json(
            &c,
            2,
            &format!("{origin}/admin/accounts/{}", public(3)),
            &json!({"allowed":true})
        )
        .await?
        .status(),
        403
    );
    assert_eq!(
        post_json(&c, 2, &url, &json!({"publisher":public(3)}))
            .await?
            .status(),
        400
    );
    let response = post_json(
        &c,
        2,
        &format!("{origin}/account-metadata"),
        &serde_json::to_value(&metadata)?,
    )
    .await?;
    ensure!(
        response.status().is_success(),
        "metadata {}",
        response.text().await?
    );
    let graph = bundle(2, auth::now(), [8; 32], [9; 32], Some("Public list"), true)?;
    save(&c, &origin, 2, &graph).await?;
    let entries = format!("{origin}/libraries/{}/entries?family=chapters", public(2));
    assert_eq!(get(&c, &entries, Some(2)).await?["total"], 1);
    assert_eq!(c.get(&entries).send().await?.status(), 403);
    assert_eq!(
        post_json(
            &c,
            1,
            &format!("{origin}/admin/accounts/{}", public(3)),
            &json!({"allowed":false})
        )
        .await?
        .status(),
        200
    );
    assert_eq!(post_json(&c, 3, &url, &json!({})).await?.status(), 403);
    // Arrange a pre-existing unadmitted federation placeholder, not an outcome oracle.
    repo.pool
        .get()
        .await?
        .execute(
            "INSERT INTO account_permissions(publisher,allowed,enrollment_blocked) VALUES($1,FALSE,FALSE)",
            &[&id(&public(4)).as_slice()],
        )
        .await?;
    let alias = format!("{origin}/compat/mangadex/nostr/enrollment");
    assert_eq!(post_json(&c, 4, &alias, &json!({})).await?.status(), 201);
    assert_eq!(post_json(&c, 5, &url, &json!({})).await?.status(), 429);
    assert_eq!(post_json(&c, 2, &url, &json!({})).await?.status(), 200);
    repo.pool.get().await?.batch_execute("UPDATE reader_enrollment_budget SET window_started=clock_timestamp()-interval '2 hours'").await?;
    // Competing new keys share the final lifetime slot after the hourly reset.
    let request = json!({});
    let (fifth, sixth) = tokio::join!(
        post_json(&c, 5, &url, &request),
        post_json(&c, 6, &url, &request)
    );
    let fifth = fifth?;
    let sixth = sixth?;
    let (winner, denied) = match (fifth.status().as_u16(), sixth.status().as_u16()) {
        (201, 429) => (5, 6),
        (429, 201) => (6, 5),
        statuses => anyhow::bail!("expected exactly one final-slot enrollment, got {statuses:?}"),
    };
    assert_eq!(post_json(&c, winner, &url, &json!({})).await?.status(), 200);
    assert_eq!(post_json(&c, denied, &url, &json!({})).await?.status(), 429);
    assert_eq!(
        post_json(
            &c,
            1,
            &format!("{origin}/admin/accounts/{}", public(denied)),
            &json!({"allowed":true})
        )
        .await?
        .status(),
        200
    );
    assert_eq!(post_json(&c, denied, &url, &json!({})).await?.status(), 200);
    assert_eq!(
        post_json(
            &c,
            1,
            &format!("{origin}/admin/accounts/{}", public(2)),
            &json!({"allowed":false})
        )
        .await?
        .status(),
        200
    );
    assert_eq!(post_json(&c, 2, &url, &json!({})).await?.status(), 403);
    assert_eq!(
        post_json(
            &c,
            2,
            &format!("{origin}/account-metadata"),
            &serde_json::to_value(&metadata)?
        )
        .await?
        .status(),
        403
    );
    drop(node.take());
    node = Some(start(&cfg, &dsn, &dir.path().join("node.log"))?);
    ready(&c, &origin).await?;
    assert_eq!(post_json(&c, 2, &url, &json!({})).await?.status(), 403);
    assert_eq!(post_json(&c, winner, &url, &json!({})).await?.status(), 200);
    // A new key is still over the lifetime limit after restart, even after the
    // hourly window is reset again. Existing and operator-admitted keys retry.
    repo.pool.get().await?.batch_execute("UPDATE reader_enrollment_budget SET window_started=clock_timestamp()-interval '2 hours'").await?;
    assert_eq!(post_json(&c, 7, &url, &json!({})).await?.status(), 429);
    assert_eq!(post_json(&c, denied, &url, &json!({})).await?.status(), 200);
    drop(node.take());
    config["enrollment"]["enabled"] = json!(false);
    std::fs::write(&cfg, serde_json::to_vec(&config)?)?;
    node = Some(start(&cfg, &dsn, &dir.path().join("node.log"))?);
    ready(&c, &origin).await?;
    assert_eq!(post_json(&c, 7, &url, &json!({})).await?.status(), 403);
    let existing = sign_at(
        winner,
        0,
        vec![],
        json!({"name":"Existing reader after signup closes"}).to_string(),
        auth::now(),
    );
    let response = post_json(
        &c,
        winner,
        &format!("{origin}/account-metadata"),
        &serde_json::to_value(&existing)?,
    )
    .await?;
    ensure!(
        response.status().is_success(),
        "existing account after signup closes {}",
        response.text().await?
    );
    assert_eq!(
        get(
            &c,
            &format!("{origin}/accounts/{}/metadata/0", public(winner)),
            Some(winner)
        )
        .await?["id"],
        existing.id
    );
    assert_eq!(get(&c, &entries, Some(winner)).await?["total"], 1);
    drop(node);
    std::fs::write(
        root.join("results/http-enrollment.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["disabled default denies enrollment, account writes and admitted-only reads","new key proves exact NIP98 request and replay protection","concurrent duplicate keeps the same permission version and leaves the next hourly slot available","no publisher or operator grant","new reader saves signed profile and public library","restricted read policy remains enforced","operator denial cannot be undone","new federation placeholder can enroll","hourly cap denies a new key while existing keys retry","two keys compete for one remaining lifetime slot and exactly one succeeds","lifetime cap persists across process restart and hourly reset","existing admitted accounts and operator admission bypass new-account limits","revocation survives restart","disabling signup preserves existing admission"]}),
        )?,
    )?;
    Ok(())
}
