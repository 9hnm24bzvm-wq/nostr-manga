//! Functional original-byte survival with the committed Met public-domain corpus.
//! Oracles are HTTP responses, original bytes and process lifecycle; no DB rows.
mod support;
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    library_client_bundle::SaveBundle,
    library_manifest::{Chunk, Descriptor, Family, FamilyRoot, Limits, Record, Root},
    mangadex_compat,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
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
struct Original {
    descriptor: Value,
    bytes: Vec<u8>,
    source: String,
}
fn bind() -> Result<String> {
    let socket = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(socket.local_addr()?.to_string())
}
fn binary() -> PathBuf {
    std::env::var_os("MANGA_TEST_NODE_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_manga-node").into())
}
fn start(config: &Path, dsn: &str, log: &Path) -> Result<Node> {
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)?;
    Ok(Node(
        Command::new(binary())
            .args(["serve", config.to_str().unwrap()])
            .env("MANGA_DATABASE_URL", dsn)
            .env("RUST_LOG", "info")
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    ))
}
async fn ready(client: &reqwest::Client, url: &str) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if client
                .get(url)
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .context("node readiness deadline")?;
    Ok(())
}
async fn closed(address: &str) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        while tokio::net::TcpStream::connect(address).await.is_ok() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .context("owned listener did not stop")?;
    Ok(())
}
async fn get_json(client: &reqwest::Client, url: &str) -> Result<Value> {
    let response = client.get(url).send().await?;
    ensure!(
        response.status().is_success(),
        "GET {url}: {} {}",
        response.status(),
        response.text().await?
    );
    Ok(response.json().await?)
}
async fn exact(
    client: &reqwest::Client,
    url: &str,
    expected: &[u8],
    mime: Option<&str>,
) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if let Ok(response) = client.get(url).send().await
                && response.status() == 200
            {
                if let Some(mime) = mime {
                    assert_eq!(response.headers()["content-type"], mime);
                }
                assert_eq!(
                    response.bytes().await?.as_ref(),
                    expected,
                    "original bytes changed at {url}"
                );
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .with_context(|| format!("original not served: {url}"))??;
    Ok(())
}
async fn absent(client: &reqwest::Client, url: &str) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if client.get(url).send().await?.status() == 404 {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .with_context(|| format!("retired content remained visible: {url}"))??;
    Ok(())
}
async fn publish(
    client: &reqwest::Client,
    origin: &str,
    kind: u32,
    manifest: &[u8],
) -> Result<auth::Event> {
    upload_to(client, 1, origin, manifest).await?;
    let event = publication(1, kind, manifest);
    let response = post_json(
        client,
        1,
        &format!("{origin}/publications"),
        &serde_json::to_value(&event)?,
    )
    .await?;
    ensure!(
        response.status().is_success(),
        "publication: {} {}",
        response.status(),
        response.text().await?
    );
    wait_projected(client, origin, &event.id).await?;
    assert_eq!(
        get_json(client, &format!("{origin}/events/{}", event.id)).await?,
        serde_json::to_value(&event)?
    );
    Ok(event)
}
async fn approve_peer(
    client: &reqwest::Client,
    origin: &str,
    key: u8,
    endpoint: &str,
) -> Result<()> {
    let response = post_json(
        client,
        1,
        &format!("{origin}/admin/peers"),
        &json!({"key":public(key),"endpoint":endpoint,"allowed":true}),
    )
    .await?;
    ensure!(
        response.status() == 200,
        "peer approval: {}",
        response.text().await?
    );
    Ok(())
}
async fn synchronize(client: &reqwest::Client, origin: &str, event: &auth::Event) -> Result<()> {
    let response = post_json(
        client,
        1,
        &format!("{origin}/admin/sync"),
        &json!({"peer":public(11),"event":event.id,"filter":"","fallbacks":[]}),
    )
    .await?;
    ensure!(
        response.status().is_success(),
        "peer sync: {} {}",
        response.status(),
        response.text().await?
    );
    wait_projected(client, origin, &event.id).await?;
    assert_eq!(
        get_json(client, &format!("{origin}/events/{}", event.id)).await?,
        serde_json::to_value(event)?
    );
    Ok(())
}
fn progress(chapter: &str) -> Result<SaveBundle> {
    let limits = Limits::default();
    let chunk = Chunk::new(
        Family::Chapters,
        vec![Record::Chapter {
            id: hex::decode(chapter)?.try_into().unwrap(),
            completed: false,
            page: Some(3),
            updated_at: 1,
        }],
        &limits,
    )?
    .encode(&limits)?;
    let summary = Chunk::decode(&chunk, &limits)?.1;
    let root = Root {
        families: Family::ALL
            .into_iter()
            .map(|family| FamilyRoot {
                family,
                chunks: vec![if family == Family::Chapters {
                    Descriptor::from(&summary)
                } else {
                    Descriptor::empty()
                }],
            })
            .collect(),
    }
    .encode(&limits)?;
    let event = sign_at(
        2,
        11999,
        vec![],
        json!({"manifest":auth::hash(&root),"version":1}).to_string(),
        auth::now(),
    );
    SaveBundle::validate(event, root, BTreeMap::from([(summary.hash(), chunk)]))
}
async fn save_progress(client: &reqwest::Client, origin: &str, bundle: &SaveBundle) -> Result<()> {
    let mut cursor = 0;
    while let Some(batch) = bundle.batch(cursor)? {
        let url = format!("{origin}/library/batches");
        let response = client
            .post(&url)
            .header("authorization", http(2, &url, &batch.body))
            .body(batch.body)
            .send()
            .await?;
        ensure!(
            response.status().is_success(),
            "library staging: {}",
            response.text().await?
        );
        cursor = batch.next_object;
    }
    let url = format!("{origin}/libraries");
    let body = bundle.commit_body()?;
    let response = client
        .post(&url)
        .header("authorization", http(2, &url, &body))
        .body(body)
        .send()
        .await?;
    ensure!(
        response.status().is_success(),
        "library commit: {}",
        response.text().await?
    );
    Ok(())
}
async fn assert_progress(
    client: &reqwest::Client,
    origin: &str,
    chapter: &str,
    bundle: &SaveBundle,
) -> Result<()> {
    let entries = get_json(
        client,
        &format!("{origin}/libraries/{}/entries?family=chapters", public(2)),
    )
    .await?;
    assert_eq!(entries["event"]["id"], bundle.event().id);
    assert_eq!(
        entries["entries"],
        json!([{"id":chapter,"completed":false,"page":3,"updated_at":1}])
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "real isolated PostgreSQL, strfry and nginx; functional Met-image acceptance, not load"]
async fn met_originals_survive_source_loss_with_progress_and_immutable_reupload() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("isolated fixture required")?;
    ensure!(
        dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test")
            && dsn.contains("dbname=manga_test"),
        "isolated fixture database only"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/real-images");
    let metadata: Value = serde_json::from_slice(&std::fs::read(corpus.join("MANIFEST.json"))?)?;
    let mut originals = vec![];
    for item in metadata["images"]
        .as_array()
        .context("image corpus metadata")?
    {
        assert_eq!(item["public_domain"], true);
        let bytes = std::fs::read(corpus.join(item["file"].as_str().unwrap()))?;
        assert_eq!(auth::hash(&bytes), item["sha256"]);
        assert_eq!(bytes.len() as u64, item["bytes"].as_u64().unwrap());
        let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg)?;
        assert_eq!(
            (decoded.width() as u64, decoded.height() as u64),
            (
                item["width"].as_u64().unwrap(),
                item["height"].as_u64().unwrap()
            )
        );
        originals.push(Original { descriptor: json!({"sha256":item["sha256"],"size":item["bytes"],"mime":"image/jpeg","width":item["width"],"height":item["height"]}), bytes, source: item["source"].as_str().unwrap().into() });
    }
    assert_eq!(originals.len(), 4);
    let created = Command::new(root.join("vendor/pgsql/bin/createdb"))
        .args(["--maintenance-db", &dsn, "manga_real_images_b"])
        .output()?;
    ensure!(
        created.status.success(),
        "receiver database creation: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    let dsns = [
        dsn.clone(),
        dsn.replace("dbname=manga_test", "dbname=manga_real_images_b"),
    ];
    let runtime = tempfile::Builder::new()
        .prefix("manga-real-images-")
        .tempdir()?
        .keep();
    eprintln!(
        "real-image runtime retained for diagnosis: {}",
        runtime.display()
    );
    let addresses = [bind()?, bind()?];
    let origins = addresses
        .each_ref()
        .map(|address| format!("http://{address}"));
    let image_bind = bind()?;
    let static_origin = format!("http://{image_bind}");
    let mut configs = vec![];
    for index in 0..2 {
        let key = runtime.join(format!("key-{index}"));
        write_key(&key, 11 + index as u8)?;
        let config = runtime.join(format!("node-{index}.json"));
        let mut value = json!({"bind":addresses[index],"base_url":origins[index],"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"discovery_private_overrides":origins,"storage":{"path":runtime.join(format!("originals-{index}")),"volume":format!("realimages{index}"),"formats":["image/jpeg"],"physical_bytes":67108864,"staging_bytes":16777216,"max_blob_bytes":2097152},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":runtime.join(format!("backend-{index}")),"timeout_seconds":5,"map_bytes":67108864}});
        if index == 1 {
            value["public_serving"] = json!({"enabled":true,"helper":root.join("src/serving_supervisor.py"),"nginx":root.join("vendor/nginx-1.28.0/objs/nginx"),"bind":image_bind,"shutdown_seconds":1});
        }
        std::fs::write(&config, serde_json::to_vec(&value)?)?;
        let migrated = Command::new(binary())
            .args(["migrate", config.to_str().unwrap()])
            .env("MANGA_DATABASE_URL", &dsns[index])
            .output()?;
        ensure!(
            migrated.status.success(),
            "CLI migration: {}",
            String::from_utf8_lossy(&migrated.stderr)
        );
        configs.push(config);
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(25))
        .build()?;
    let mut nodes = [
        Some(start(&configs[0], &dsns[0], &runtime.join("source.log"))?),
        Some(start(&configs[1], &dsns[1], &runtime.join("receiver.log"))?),
    ];
    for origin in &origins {
        ready(&client, origin).await?;
    }
    for original in &originals {
        upload_to(&client, 1, &origins[0], &original.bytes).await?;
        let hash = original.descriptor["sha256"].as_str().unwrap();
        assert_eq!(
            client
                .get(format!("{}/originals/{hash}", origins[0]))
                .send()
                .await?
                .status(),
            404,
            "staged originals remain private"
        );
    }
    let series_manifest = cbor(
        json!({"version":1,"titles":{"en":"Hokusai Sketchbooks — Metropolitan Museum public-domain corpus"}}),
    );
    let series = publish(&client, &origins[0], 1002, &series_manifest).await?;
    let chapter_manifest = cbor(
        json!({"version":1,"series":series.id,"chapter":"1","language":"ja","pages":originals.iter().map(|original| original.descriptor.clone()).collect::<Vec<_>>()}),
    );
    let chapter = publish(&client, &origins[0], 1001, &chapter_manifest).await?;
    let denied = post_json(
        &client,
        1,
        &format!("{}/admin/sync", origins[1]),
        &json!({"peer":public(11),"event":chapter.id,"filter":"","fallbacks":[]}),
    )
    .await?;
    assert_eq!(
        denied.status(),
        400,
        "an unapproved source cannot supply hosted content"
    );
    approve_peer(&client, &origins[1], 11, &origins[0]).await?;
    approve_peer(&client, &origins[0], 12, &origins[1]).await?;
    synchronize(&client, &origins[1], &series).await?;
    synchronize(&client, &origins[1], &chapter).await?;
    nodes[0].take();
    closed(&addresses[0]).await?;
    ensure!(
        client.get(&origins[0]).send().await.is_err(),
        "source unexpectedly reachable"
    );
    for original in &originals {
        let hash = original.descriptor["sha256"].as_str().unwrap();
        exact(
            &client,
            &format!("{}/originals/{hash}", origins[1]),
            &original.bytes,
            Some("image/jpeg"),
        )
        .await?;
        exact(
            &client,
            &format!("{static_origin}/{hash}.jpg"),
            &original.bytes,
            Some("image/jpeg"),
        )
        .await?;
    }
    exact(
        &client,
        &format!("{}/originals/{}", origins[1], auth::hash(&chapter_manifest)),
        &chapter_manifest,
        None,
    )
    .await?;
    eprintln!("source stopped; receiver API and nginx returned all four byte-exact originals");
    let admission = post_json(
        &client,
        1,
        &format!("{}/admin/accounts/{}", origins[1], public(2)),
        &json!({"allowed":true}),
    )
    .await?;
    ensure!(
        admission.status().is_success(),
        "reader admission: {}",
        admission.text().await?
    );
    let library = progress(&chapter.id)?;
    save_progress(&client, &origins[1], &library).await?;
    assert_progress(&client, &origins[1], &chapter.id, &library).await?;
    nodes[1].take();
    closed(&addresses[1]).await?;
    closed(&image_bind).await?;
    nodes[1] = Some(start(
        &configs[1],
        &dsns[1],
        &runtime.join("receiver-restart.log"),
    )?);
    ready(&client, &origins[1]).await?;
    assert_progress(&client, &origins[1], &chapter.id, &library).await?;
    for original in &originals {
        let hash = original.descriptor["sha256"].as_str().unwrap();
        exact(
            &client,
            &format!("{static_origin}/{hash}.jpg"),
            &original.bytes,
            Some("image/jpeg"),
        )
        .await?;
    }
    let chapter_url = format!(
        "{}/compat/mangadex/chapter/{}",
        origins[1],
        mangadex_compat::uuid("chapter", &chapter.id)
    );
    assert_eq!(
        client
            .put(&chapter_url)
            .json(&json!({"title":"Attempted in-place edit"}))
            .send()
            .await?
            .status(),
        501
    );
    assert_eq!(
        get_json(&client, &format!("{}/events/{}", origins[1], chapter.id)).await?,
        serde_json::to_value(&chapter)?
    );
    let deletion = sign_at(
        1,
        5,
        vec![vec!["e".into(), chapter.id.clone()]],
        "Replace this chapter by a new immutable publication".into(),
        auth::now(),
    );
    let requested = post_json(
        &client,
        1,
        &format!("{}/deletion-requests", origins[1]),
        &serde_json::to_value(&deletion)?,
    )
    .await?;
    assert_eq!(requested.status(), 202, "{}", requested.text().await?);
    assert_eq!(
        get_json(&client, &format!("{}/events/{}", origins[1], chapter.id)).await?,
        serde_json::to_value(&chapter)?,
        "request alone is not local moderation approval"
    );
    let approved = post_json(
        &client,
        1,
        &format!("{}/admin/deletions/{}/approve", origins[1], deletion.id),
        &json!({}),
    )
    .await?;
    assert_eq!(approved.status(), 202, "{}", approved.text().await?);
    absent(&client, &format!("{}/events/{}", origins[1], chapter.id)).await?;
    let replay = post_json(
        &client,
        1,
        &format!("{}/publications", origins[1]),
        &serde_json::to_value(&chapter)?,
    )
    .await?;
    assert_eq!(
        replay.status(),
        400,
        "deleted event replay cannot restore it"
    );
    for original in &originals {
        upload_to(&client, 1, &origins[1], &original.bytes).await?;
    }
    let replacement_manifest = cbor(
        json!({"version":1,"series":series.id,"chapter":"1","language":"ja","pages":originals.iter().rev().map(|original| original.descriptor.clone()).collect::<Vec<_>>()}),
    );
    let replacement = publish(&client, &origins[1], 1001, &replacement_manifest).await?;
    assert_ne!(replacement.id, chapter.id);
    absent(&client, &format!("{}/events/{}", origins[1], chapter.id)).await?;
    exact(
        &client,
        &format!(
            "{}/originals/{}",
            origins[1],
            auth::hash(&replacement_manifest)
        ),
        &replacement_manifest,
        None,
    )
    .await?;
    for original in &originals {
        let hash = original.descriptor["sha256"].as_str().unwrap();
        exact(
            &client,
            &format!("{}/originals/{hash}", origins[1]),
            &original.bytes,
            Some("image/jpeg"),
        )
        .await?;
        exact(
            &client,
            &format!("{static_origin}/{hash}.jpg"),
            &original.bytes,
            Some("image/jpeg"),
        )
        .await?;
    }
    assert_progress(&client, &origins[1], &chapter.id, &library).await?;
    ensure!(
        client.get(&origins[0]).send().await.is_err(),
        "source must remain stopped through reupload"
    );
    nodes[1].take();
    closed(&addresses[1]).await?;
    closed(&image_bind).await?;
    let report = std::env::var_os("MANGA_REAL_IMAGES_REPORT")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("results/rebuild-real-images/acceptance.json"));
    std::fs::create_dir_all(report.parent().unwrap())?;
    std::fs::write(
        &report,
        serde_json::to_vec_pretty(
            &json!({"passed":true,"scope":"functional four-original corpus, not capacity or production traffic acceptance","sources":originals.iter().map(|original| json!({"source":original.source,"descriptor":original.descriptor})).collect::<Vec<_>>(),"source_stopped":true,"receiver_process_restarted":true,"api_and_nginx_byte_exact":true,"progress":{"chapter":chapter.id,"page":3,"survived_restart":true,"not_silently_retargeted":true},"immutable":{"edit_status":501,"deletion_requires_local_approval":true,"old_event_replay_status":400,"replacement_event":replacement.id},"node_sha256":auth::hash(&std::fs::read(binary())?)}),
        )?,
    )?;
    eprintln!("real-image acceptance saved to {}", report.display());
    std::fs::remove_dir_all(&runtime)?;
    Ok(())
}
