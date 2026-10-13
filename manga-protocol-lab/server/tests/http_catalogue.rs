mod support;
use anyhow::{Result, ensure};
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
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    r.migrate().await?;
    Ok(r)
}
async fn stage(c: &reqwest::Client, origin: &str, bytes: &[u8]) -> Result<()> {
    let hash = auth::hash(bytes);
    let r = c
        .put(format!("{origin}/upload/{hash}"))
        .header("authorization", blossom(1, origin, "upload", &hash))
        .body(bytes.to_vec())
        .send()
        .await?;
    ensure!(r.status().is_success(), "upload: {}", r.text().await?);
    Ok(())
}
async fn query(c: &reqwest::Client, origin: &str, args: &[(&str, &str)]) -> Result<Value> {
    Ok(c.get(format!("{origin}/series"))
        .query(args)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "real PostgreSQL/strfry nodes with local cover originals"]
async fn catalogue_and_original_cover_survive_replication_and_deletion() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    ensure!(
        dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test"),
        "fixture only"
    );
    let a = repo(dsn.clone()).await?;
    a.pool
        .get()
        .await?
        .batch_execute("CREATE DATABASE manga_catalogue_b")
        .await?;
    let dsns = [
        dsn.clone(),
        dsn.replace("dbname=manga_test", "dbname=manga_catalogue_b"),
    ];
    let b = repo(dsns[1].clone()).await?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let dir = tempfile::tempdir()?;
    let mut origins = vec![];
    let mut nodes = vec![];
    for (n, dsn) in dsns.iter().enumerate() {
        let l = std::net::TcpListener::bind("127.0.0.1:0")?;
        let bind = l.local_addr()?;
        drop(l);
        let origin = format!("http://{bind}");
        let key = dir.path().join(format!("key{n}"));
        write_key(&key, 11 + n as u8)?;
        let cfg = dir.path().join(format!("node{n}.json"));
        std::fs::write(
            &cfg,
            serde_json::to_vec(
                &json!({"bind":bind.to_string(),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join(format!("backend{n}")),"map_bytes":67108864},"storage":{"path":dir.path().join(format!("originals{n}")),"volume":format!("catalogue{n}"),"physical_bytes":2097152,"staging_bytes":262144,"max_blob_bytes":262144}}),
            )?,
        )?;
        let log = std::fs::File::create(dir.path().join(format!("node{n}.log")))?;
        nodes.push(Some(Node(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", cfg.to_str().unwrap()])
                .env("MANGA_DATABASE_URL", dsn)
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        )));
        origins.push(origin);
    }
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    for origin in &origins {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if c.get(origin)
                    .send()
                    .await
                    .is_ok_and(|r| r.status().is_success())
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await?;
    }
    let cover = png();
    let metadata = json!({"version":1,"required":["series-cover-v1"],"titles":{"en":"Example","ja":"例"},"altTitles":[{"en":"Alternative"}],"description":{"en":"Description"},"categories":["Drama","drama"],"credits":[{"role":"artist","name":"Name"}],"originalLanguage":"ja","status":"ongoing","year":2026,"cover":page(&cover),"unknownOptional":{"keep":"exact"}});
    let manifest = cbor(metadata);
    let event = publication(1, 1002, &manifest);
    stage(&c, &origins[0], &manifest).await?;
    assert!(
        post_json(
            &c,
            1,
            &format!("{}/publications", origins[0]),
            &serde_json::to_value(&event)?
        )
        .await?
        .status()
        .is_client_error(),
        "cover must be present before series hosting"
    );
    stage(&c, &origins[0], &cover).await?;
    post_json(
        &c,
        1,
        &format!("{}/publications", origins[0]),
        &serde_json::to_value(&event)?,
    )
    .await?
    .error_for_status()?;
    wait_projected(&c, &origins[0], &event.id).await?;
    let found = query(
        &c,
        &origins[0],
        &[
            ("order", "asc"),
            ("title", "Alternative"),
            ("category", "Drama"),
            ("year", "2026"),
            ("include_total", "true"),
        ],
    )
    .await?;
    assert_eq!(found["total"], 1);
    assert_eq!(found["entries"][0]["event"]["id"], event.id);
    assert_eq!(
        found["entries"][0]["metadata"]["cover"]["sha256"],
        auth::hash(&cover)
    );
    for (name, value) in [
        ("title", "alternative"),
        ("category", "DRAMA"),
        ("search", "%' OR true--"),
        ("filter", "NOT(language=\"en\")"),
    ] {
        assert!(
            query(&c, &origins[0], &[("order", "asc"), (name, value)]).await?["entries"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    assert_eq!(
        query(&c, &origins[0], &[("order", "desc"), ("search", "xam")]).await?["entries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(
        c.get(format!("{}/series?order=asc&order=desc", origins[0]))
            .send()
            .await?
            .status()
            .is_client_error()
    );
    let raw = c
        .get(format!(
            "{}/originals/{}",
            origins[0],
            auth::hash(&manifest)
        ))
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    assert_eq!(raw.as_ref(), manifest);
    let cm = cbor(
        json!({"version":1,"series":event.id,"language":"en","chapter":"001.5","volume":"03","title":"Alt Chapter","pages":[page(&cover)]}),
    );
    let chapter = publication(1, 1001, &cm);
    stage(&c, &origins[0], &cm).await?;
    post_json(
        &c,
        1,
        &format!("{}/compat/mangadex/nostr/publications", origins[0]),
        &serde_json::to_value(&chapter)?,
    )
    .await?
    .error_for_status()?;
    wait_projected(&c, &origins[0], &chapter.id).await?;
    let compat = format!("{}/compat/mangadex", origins[0]);
    let manga_id = manga_node::mangadex_compat::uuid("manga", &event.id);
    let chapter_id = manga_node::mangadex_compat::uuid("chapter", &chapter.id);
    let list: Value = c
        .get(format!("{compat}/manga"))
        .query(&[("title", "Example"), ("includes[]", "cover_art")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(list["total"], 1);
    assert_eq!(list["data"][0]["id"], manga_id);
    let cover_id = list["data"][0]["relationships"][0]["id"].as_str().unwrap();
    let cover_entity: Value = c
        .get(format!("{compat}/cover/{cover_id}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let filename = cover_entity["data"]["attributes"]["fileName"]
        .as_str()
        .unwrap();
    assert_eq!(
        c.get(format!("{compat}/covers/{manga_id}/{filename}"))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?
            .as_ref(),
        cover
    );
    let covers: Value = c
        .get(format!("{compat}/cover"))
        .query(&[("manga[]", manga_id.as_str())])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(covers["total"], 1);
    let authors: Value = c
        .get(format!("{compat}/author?name=Name"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(authors["total"], 1);
    let author = authors["data"][0]["id"].as_str().unwrap();
    assert!(
        c.get(format!("{compat}/author/{author}"))
            .send()
            .await?
            .status()
            .is_success()
    );
    let feed: Value = c
        .get(format!("{compat}/manga/{manga_id}/feed"))
        .query(&[("translatedLanguage[]", "en"), ("order[chapter]", "asc")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(feed["total"], 1);
    let expanded: Value = c
        .get(format!(
            "{compat}/chapter/{chapter_id}?includes[]=manga&includes[]=user"
        ))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(
        expanded["data"]["relationships"][0]["attributes"]["title"]["en"],
        "Example"
    );

    assert_eq!(feed["data"][0]["attributes"]["chapter"], "001.5");
    let aggregated: Value = c
        .get(format!("{compat}/manga/{manga_id}/aggregate"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(
        aggregated["volumes"]["03"]["chapters"]["001.5"]["id"],
        chapter_id
    );
    let home: Value = c
        .get(format!("{compat}/at-home/server/{chapter_id}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let page = home["chapter"]["data"][0].as_str().unwrap();
    assert_eq!(
        c.get(format!("{compat}/data/{}/{page}", chapter.id))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?
            .as_ref(),
        cover
    );
    let tags: Value = c
        .get(format!("{compat}/manga/tag"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(tags["data"].as_array().unwrap().len(), 2);
    let alias = "01234567-89ab-4cde-8123-456789abcdef";
    post_json(
        &c,
        1,
        &format!("{}/admin/compatibility/aliases", origins[0]),
        &json!({"kind":"manga","id":alias,"target":event.id,"preferred":true}),
    )
    .await?
    .error_for_status()?;
    let mapped: Value = c
        .get(format!("{compat}/manga/{alias}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(mapped["data"]["nostr"]["eventId"], event.id);
    assert_eq!(
        c.put(format!("{compat}/manga/{alias}"))
            .send()
            .await?
            .status(),
        501
    );
    assert_eq!(
        c.get(format!("{compat}/manga?invented=true"))
            .send()
            .await?
            .status(),
        400
    );
    let labels: Value = c
        .get(format!("{}/query", origins[0]))
        .query(&[
            ("order", "asc"),
            ("filter", "volume=\"03\" AND title=\"Alt Chapter\""),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(labels["events"][0]["id"], chapter.id);
    a.set_peer(id(&public(12)), &origins[1], true).await?;
    b.set_peer(id(&public(11)), &origins[0], true).await?;
    let result: Value = post_json(
        &c,
        1,
        &format!("{}/admin/sync", origins[1]),
        &json!({"peer":public(11),"event":event.id,"filter":""}),
    )
    .await?
    .error_for_status()?
    .json()
    .await?;
    assert_eq!(result["originals_fetched"], 1);
    wait_projected(&c, &origins[1], &event.id).await?;
    post_json(
        &c,
        1,
        &format!("{}/admin/sync", origins[1]),
        &json!({"peer":public(11),"event":chapter.id,"filter":"volume=\"03\""}),
    )
    .await?
    .error_for_status()?;
    wait_projected(&c, &origins[1], &chapter.id).await?;
    drop(nodes[0].take());
    // Arrange an interrupted legacy index with a review marker. SQL is fault
    // setup only; recovery is observed through the operator and reader HTTP APIs.
    let fault = a.pool.get().await?;
    fault
        .execute(
            "DELETE FROM catalogue_metadata WHERE event_id=$1",
            &[&id(&event.id).as_slice()],
        )
        .await?;
    fault
        .execute(
            "INSERT INTO catalogue_index_errors(event_id,reason) VALUES($1,'manifest_unavailable')",
            &[&id(&event.id).as_slice()],
        )
        .await?;
    drop(fault);
    let cfg = dir.path().join("node0.json");
    let mut config: Value = serde_json::from_slice(&std::fs::read(&cfg)?)?;
    config["read_policy"] = json!("authenticated");
    std::fs::write(&cfg, serde_json::to_vec(&config)?)?;
    let log = std::fs::File::create(dir.path().join("guarded-restart.log"))?;
    nodes[0] = Some(Node(
        Command::new(env!("CARGO_BIN_EXE_manga-node"))
            .args(["serve", cfg.to_str().unwrap()])
            .env("MANGA_DATABASE_URL", &dsns[0])
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    ));
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if c.get(&origins[0])
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                break Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    let errors_url = format!("{}/admin/catalogue/errors", origins[0]);
    let read_errors = || async {
        c.get(&errors_url)
            .header("authorization", http_get(1, &errors_url))
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await
    };
    let indexed_url = format!("{}/catalogue/{}", origins[0], event.id);
    assert_eq!(
        c.get(&indexed_url)
            .header("authorization", http_get(1, &indexed_url))
            .send()
            .await?
            .status(),
        404
    );
    let review = read_errors().await?;
    assert_eq!(review["items"].as_array().unwrap().len(), 1);
    assert_eq!(review["items"][0]["event"], event.id);
    assert_eq!(review["items"][0]["reason"], "manifest_unavailable");
    let reindex_url = format!("{}/admin/catalogue/reindex", origins[0]);
    let request = json!({"event":event.id});
    assert_eq!(
        post_json(&c, 2, &reindex_url, &request).await?.status(),
        403
    );
    assert_eq!(
        read_errors().await?["items"],
        review["items"],
        "unauthorized retry must not remove the review marker"
    );
    let retry: Value = post_json(&c, 1, &reindex_url, &request)
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(retry["event"], event.id);
    assert_eq!(retry["eligible_for_reindex"], true);
    assert_eq!(retry["publication_mutated"], false);
    let restored: Value = tokio::time::timeout(Duration::from_secs(45), async {
        loop {
            let response = c
                .get(&indexed_url)
                .header("authorization", http_get(1, &indexed_url))
                .send()
                .await?;
            if response.status().is_success() {
                return response.json::<Value>().await;
            }
            assert_eq!(response.status(), 404);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await??;
    assert_eq!(restored["event"], serde_json::to_value(&event)?);
    assert_eq!(restored["metadata"]["titles"]["en"], "Example");
    assert_eq!(restored["metadata"]["altTitles"][0]["en"], "Alternative");
    assert_eq!(restored["metadata"]["cover"]["sha256"], auth::hash(&cover));
    assert!(read_errors().await?["items"].as_array().unwrap().is_empty());
    for (hash, bytes) in [
        (auth::hash(&manifest), &manifest),
        (auth::hash(&cover), &cover),
    ] {
        let url = format!("{}/originals/{hash}", origins[0]);
        assert_eq!(
            c.get(&url)
                .header("authorization", http_get(1, &url))
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await?
                .as_ref(),
            bytes
        );
    }
    for path in [
        "/series?order=asc".to_owned(),
        format!("/catalogue/{}", event.id),
        format!("/compat/mangadex/manga/{manga_id}"),
        format!("/compat/mangadex/at-home/server/{chapter_id}"),
        format!(
            "/compat/mangadex/data/{}/{}.png",
            chapter.id,
            auth::hash(&cover)
        ),
        "/compat/mangadex/manga/tag".to_owned(),
        "/compat/mangadex/author".to_owned(),
        "/compat/mangadex/cover".to_owned(),
    ] {
        let url = format!("{}{path}", origins[0]);
        assert_eq!(c.get(&url).send().await?.status(), 403);
        assert!(
            c.get(&url)
                .header("authorization", http_get(1, &url))
                .send()
                .await?
                .status()
                .is_success()
        );
    }
    let errors_url = format!("{}/admin/catalogue/errors", origins[0]);
    assert_eq!(
        c.get(&errors_url)
            .header("authorization", http_get(2, &errors_url))
            .send()
            .await?
            .status(),
        403
    );
    let errors: Value = c
        .get(&errors_url)
        .header("authorization", http_get(1, &errors_url))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert!(errors["items"].as_array().unwrap().is_empty());
    drop(nodes[0].take());

    assert_eq!(
        c.get(format!("{}/originals/{}", origins[1], auth::hash(&cover)))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?
            .as_ref(),
        cover
    );
    let detail: Value = c
        .get(format!("{}/catalogue/{}", origins[1], event.id))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(detail["metadata"]["titles"]["en"], "Example");
    let deletion = sign_at(
        1,
        5,
        vec![vec!["e".into(), event.id.clone()]],
        String::new(),
        auth::now(),
    );
    b.record_deletion_request(&deletion).await?;
    b.approve_deletion(id(&deletion.id), None).await?;
    let compat_b = format!("{}/compat/mangadex", origins[1]);
    assert_eq!(
        c.get(format!("{compat_b}/manga/{manga_id}"))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        c.get(format!(
            "{compat_b}/covers/{manga_id}/{}.png",
            auth::hash(&cover)
        ))
        .send()
        .await?
        .status(),
        404
    );
    assert!(
        c.get(format!("{compat_b}/at-home/server/{chapter_id}"))
            .send()
            .await?
            .status()
            .is_success()
    );
    assert!(
        query(&c, &origins[1], &[("order", "asc")]).await?["entries"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        c.get(format!("{}/catalogue/{}", origins[1], event.id))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        c.get(format!("{}/originals/{}", origins[1], auth::hash(&cover)))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?
            .as_ref(),
        cover,
        "shared chapter retains image after series deletion"
    );
    assert!(
        c.get(format!("{}/catalogue/{}", origins[1], chapter.id))
            .send()
            .await?
            .status()
            .is_success()
    );
    std::fs::write(
        root.join("results/http-mangadex-compat.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["manga/chapter list and individual entity mapping","feed preserves textual chapter and volume labels","at-home descriptor and exact local original bytes","cover/author/tag collections","chapter aggregation keeps exact publication IDs","explicit legacy UUID alias preserves signed identity","immutable edit refused","unsupported filters rejected","authenticated reads protect metadata and image aliases","series moderation hides cover aliases without deleting shared chapter original"],"scope":"reference content-read facade; no claim of unchanged third-party client compatibility"}),
        )?,
    )?;
    std::fs::write(
        root.join("results/http-catalogue.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["rich metadata and exact-case localized/alternative title/category queries","literal substring search and parameter binding","missing-field three-valued filter behavior","cover required before hosting","raw unknown optional fields preserved","cover replicated and self-hosted after origin loss","local series deletion hides catalogue entry","textual chapter volume/title filtering","shared cover/chapter original survives series-only deletion","operator-authorized reindex restores exact catalogue metadata after interrupted legacy indexing without mutating event or original bytes","new routes enforce authenticated read policy","index error diagnostics are operator-only"]}),
        )?,
    )?;
    Ok(())
}
