mod support;
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    library_client_bundle::SaveBundle,
    library_manifest::{Chunk, Descriptor, Family, FamilyRoot, Limits, Record, Root},
    mangadex_compat,
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
#[ignore = "real PostgreSQL and strfry; reader workflows, not load testing"]
async fn profiles_lists_progress_and_signed_mutations_follow_current_public_library() -> Result<()>
{
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture database")?;
    ensure!(
        dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test"),
        "fixture only"
    );
    let repo = Repository::connect(DbConfig {
        dsn: dsn.clone(),
        connections: 4,
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
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = listener.local_addr()?;
    drop(listener);
    let origin = format!("http://{bind}");
    let key = dir.path().join("service.key");
    write_key(&key, 11)?;
    let path = dir.path().join("node.json");
    let mut cfg = json!({"bind":bind.to_string(),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"storage":{"path":dir.path().join("objects"),"volume":"readerstate","physical_bytes":16777216,"staging_bytes":16777216,"max_blob_bytes":1000000},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}});
    std::fs::write(&path, serde_json::to_vec(&cfg)?)?;
    let mut node = Some(start(&path, &dsn, &dir.path().join("node.log"))?);
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()?;
    ready(&c, &origin).await?;
    for k in [2, 3] {
        assert!(
            post_json(
                &c,
                1,
                &format!("{origin}/admin/accounts/{}", public(k)),
                &json!({"allowed":true})
            )
            .await?
            .status()
            .is_success()
        );
    }
    let me = format!("{origin}/compat/mangadex/user/me");
    assert_eq!(c.get(&me).send().await?.status(), 403);
    assert_eq!(
        get(&c, &me, Some(2)).await?["data"]["nostr"]["pubkey"],
        public(2)
    );
    let token = http_get(2, &me);
    assert_eq!(
        c.get(&me)
            .header("authorization", &token)
            .send()
            .await?
            .status(),
        200
    );
    assert_eq!(
        c.get(&me)
            .header("authorization", &token)
            .send()
            .await?
            .status(),
        403
    );
    let metadata = sign_at(
        2,
        0,
        vec![],
        json!({"name":"Reader A","about":"Public profile"}).to_string(),
        auth::now() - 20,
    );
    let metadata_url = format!("{origin}/compat/mangadex/nostr/account-metadata");
    assert!(
        post_json(&c, 2, &metadata_url, &serde_json::to_value(&metadata)?)
            .await?
            .status()
            .is_success()
    );
    let user = get(
        &c,
        &format!("{origin}/compat/mangadex/user/{}", public(2)),
        None,
    )
    .await?;
    assert_eq!(user["data"]["attributes"]["username"], "Reader A");
    let user_id = user["data"]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        get(
            &c,
            &format!("{origin}/compat/mangadex/user/{user_id}"),
            None
        )
        .await?["data"]["nostr"]["pubkey"],
        public(2)
    );
    let my_lists = format!("{origin}/compat/mangadex/user/list");
    assert_eq!(get(&c, &my_lists, Some(2)).await?["total"], 0);
    let series_bytes = cbor(json!({"type":"series","version":1,"titles":{"en":"Reader fixture"}}));
    upload_to(&c, 1, &origin, &series_bytes).await?;
    let series = publication(1, 1002, &series_bytes);
    assert!(
        post_json(
            &c,
            1,
            &format!("{origin}/publications"),
            &serde_json::to_value(&series)?
        )
        .await?
        .status()
        .is_success()
    );
    let image = png();
    upload_to(&c, 1, &origin, &image).await?;
    let chapter_bytes = cbor(
        json!({"type":"chapter","version":1,"series":series.id,"language":"en","chapter":"1","pages":[page(&image)]}),
    );
    upload_to(&c, 1, &origin, &chapter_bytes).await?;
    let chapter = publication(1, 1001, &chapter_bytes);
    assert!(
        post_json(
            &c,
            1,
            &format!("{origin}/publications"),
            &serde_json::to_value(&chapter)?
        )
        .await?
        .status()
        .is_success()
    );
    let manga_id = mangadex_compat::uuid("manga", &series.id);
    let chapter_id = mangadex_compat::uuid("chapter", &chapter.id);
    let chapter_url = format!("{origin}/compat/mangadex/chapter/{chapter_id}");
    for _ in 0..100 {
        if c.get(&chapter_url).send().await?.status().is_success() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    assert!(c.get(&chapter_url).send().await?.status().is_success());
    let at = auth::now() - 10;
    let first = bundle(2, at, id(&series.id), id(&chapter.id), Some("Saved"), true)?;
    save(&c, &origin, 2, &first).await?;
    let lists = get(&c, &my_lists, Some(2)).await?;
    assert_eq!(lists["total"], 1);
    assert_eq!(lists["data"][0]["attributes"]["name"], "Saved");
    assert_eq!(
        lists["data"][0]["relationships"].as_array().unwrap().len(),
        2
    );
    let list_id = lists["data"][0]["id"].as_str().unwrap().to_owned();
    let list_url = format!("{origin}/compat/mangadex/list/{list_id}");
    assert_eq!(
        get(&c, &list_url, None).await?["data"]["attributes"]["visibility"],
        "public"
    );
    assert_eq!(
        get(&c, &format!("{list_url}/members"), None).await?["data"][0]["position"],
        7
    );
    let statuses = format!("{origin}/compat/mangadex/manga/status");
    assert_eq!(
        get(&c, &statuses, Some(2)).await?["statuses"][&manga_id],
        "reading"
    );
    let status_url = format!("{origin}/compat/mangadex/manga/{manga_id}/status");
    assert_eq!(get(&c, &status_url, Some(2)).await?["status"], "reading");
    let read_url = format!("{origin}/compat/mangadex/manga/{manga_id}/read");
    assert_eq!(
        get(&c, &read_url, Some(2)).await?["data"],
        json!([chapter_id])
    );
    let bulk_url = format!("{origin}/compat/mangadex/manga/read?ids[]={manga_id}&grouped=true");
    assert_eq!(
        get(&c, &bulk_url, Some(2)).await?["data"][&manga_id],
        json!([chapter_id])
    );
    let entries = format!("{origin}/libraries/{}/entries?family=chapters", public(2));
    let page = get(&c, &entries, None).await?;
    assert_eq!(page["entries"][0]["completed"], true);
    assert_eq!(page["entries"][0]["id"], chapter.id);
    assert_eq!(page["event"]["id"], first.event().id);
    assert_eq!(
        get(&c, &format!("{entries}&limit=0"), None).await?["entries"],
        json!([])
    );
    assert_eq!(
        get(&c, &format!("{entries}&offset=1"), None).await?["entries"],
        json!([])
    );
    assert_eq!(
        c.get(format!("{entries}&limit=101")).send().await?.status(),
        400
    );
    assert_eq!(
        c.get(format!("{entries}&limit=1&limit=2"))
            .send()
            .await?
            .status(),
        400
    );
    let other = bundle(3, at, id(&series.id), id(&chapter.id), Some("Saved"), true)?;
    save(&c, &origin, 3, &other).await?;
    let other_lists = get(&c, &my_lists, Some(3)).await?;
    let other_id = other_lists["data"][0]["id"].as_str().unwrap();
    assert_ne!(list_id, other_id);
    let commit = format!("{origin}/compat/mangadex/nostr/libraries");
    let body = other.commit_body()?;
    assert_eq!(
        c.post(&commit)
            .header("authorization", http(2, &commit, &body))
            .body(body)
            .send()
            .await?
            .status(),
        403
    );
    let second = bundle(
        2,
        at + 1,
        id(&series.id),
        id(&chapter.id),
        Some("Renamed"),
        false,
    )?;
    save(&c, &origin, 2, &second).await?;
    assert_eq!(
        get(&c, &list_url, None).await?["data"]["attributes"]["name"],
        "Renamed"
    );
    assert_eq!(get(&c, &read_url, Some(2)).await?["data"], json!([]));
    assert_eq!(get(&c, &status_url, Some(2)).await?["status"], "completed");
    assert_eq!(
        c.get(format!("{entries}&head={}", first.event().id))
            .send()
            .await?
            .status(),
        409
    );
    let gone = bundle(2, at + 2, id(&series.id), id(&chapter.id), None, false)?;
    save(&c, &origin, 2, &gone).await?;
    assert_eq!(c.get(&list_url).send().await?.status(), 404);
    assert_eq!(get(&c, &my_lists, Some(2)).await?["total"], 0);
    let other_url = format!("{origin}/compat/mangadex/list/{other_id}");
    assert_eq!(
        get(&c, &other_url, None).await?["data"]["attributes"]["name"],
        "Saved"
    );
    // Restricted reads cannot bypass policy through the new list-members route.
    node.take();
    cfg["read_policy"] = json!("admitted");
    std::fs::write(&path, serde_json::to_vec(&cfg)?)?;
    node = Some(start(&path, &dsn, &dir.path().join("restricted.log"))?);
    ready(&c, &origin).await?;
    let members_url = format!("{other_url}/members");
    assert_eq!(c.get(&members_url).send().await?.status(), 403);
    assert_eq!(
        c.get(&members_url)
            .header("authorization", http_get(4, &members_url))
            .send()
            .await?
            .status(),
        403
    );
    assert_eq!(get(&c, &members_url, Some(2)).await?["total"], 1);
    assert_eq!(
        get(&c, &me, Some(2)).await?["data"]["attributes"]["username"],
        "Reader A"
    );
    let deletion = sign_at(
        2,
        5,
        vec![
            vec!["e".into(), gone.event().id.clone()],
            vec!["k".into(), "11999".into()],
        ],
        "".into(),
        auth::now(),
    );
    assert!(
        post_json(
            &c,
            2,
            &format!("{origin}/compat/mangadex/nostr/account-deletions"),
            &serde_json::to_value(&deletion)?
        )
        .await?
        .status()
        .is_success()
    );
    assert_eq!(get(&c, &my_lists, Some(2)).await?["total"], 0);
    assert_eq!(get(&c, &statuses, Some(2)).await?["statuses"], json!({}));
    assert_eq!(
        get(&c, &other_url, Some(3)).await?["data"]["attributes"]["name"],
        "Saved"
    );
    let stale = format!("{statuses}?head={}", gone.event().id);
    assert_eq!(
        c.get(&stale)
            .header("authorization", http_get(2, &stale))
            .send()
            .await?
            .status(),
        409
    );
    drop(node);
    std::fs::write(
        root.join("results/http-reader-state.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["NIP98 current-user identity and replay rejection","public profile aliases","signed metadata/library write mappings","list create/rename/remove with stable owner-scoped IDs","exact read/unread and series statuses","public entry pagination and signed-head pinning","cross-account signing rejection and shared-object preservation","restricted instance policy on list-member reads","ordinary signed library deletion"]}),
        )?,
    )?;
    Ok(())
}
