mod support;
use anyhow::{Context, Result};
use manga_node::auth;
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
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and real strfry"]
async fn personal_metadata_http_authority() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture database required")?;
    let pg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(
        pg.get_dbname() == Some("manga_test"),
        "fixture database only"
    );
    let repo = manga_node::db::Repository::connect(manga_node::db::DbConfig {
        dsn: dsn.clone(),
        connections: 4,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1_048_576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    drop(repo);
    let dir = tempfile::tempdir()?;
    let key = dir.path().join("service.key");
    write_key(&key, 11)?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let config = dir.path().join("node.json");
    let origin = "http://127.0.0.1:19516";
    std::fs::write(
        &config,
        serde_json::to_vec(
            &json!({"bind":"127.0.0.1:19516","base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"library_concurrency":{"request_slots":256,"processing_slots":16,"commit_slots":8},"storage":{"path":dir.path().join("objects"),"volume":"libraryhttp","physical_bytes":10000000,"staging_bytes":10000000,"max_blob_bytes":1000000,"upload_slots":4},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}}),
        )?,
    )?;
    let log = std::fs::File::create(dir.path().join("node.log"))?;
    let _node = Node(
        Command::new(env!("CARGO_BIN_EXE_manga-node"))
            .args(["serve", config.to_str().unwrap()])
            .env("MANGA_DATABASE_URL", dsn)
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    );
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let mut ready = false;
    for _ in 0..100 {
        if c.get(origin).send().await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    anyhow::ensure!(
        ready,
        "node startup: {}",
        std::fs::read_to_string(dir.path().join("node.log"))?
    );
    let url = format!("{origin}/account-metadata");
    let admin_url = format!("{origin}/admin/accounts/{}", public(2));
    let head_url = format!("{origin}/accounts/{}/metadata/0", public(2));
    let at = auth::now() - 10;
    let first = sign_at(2, 0, vec![], "{\"name\":\"first\"}".into(), at);
    let value = serde_json::to_value(&first)?;
    assert_eq!(post_json(&c, 2, &url, &value).await?.status(), 403);
    assert_eq!(
        post_json(&c, 2, &admin_url, &json!({"allowed":true}))
            .await?
            .status(),
        403
    );
    assert_eq!(
        post_json(&c, 1, &admin_url, &json!({"allowed":true}))
            .await?
            .status(),
        200
    );
    assert_eq!(
        post_json(&c, 1, &admin_url, &json!({})).await?.status(),
        400
    );
    let body = serde_json::to_vec(&first)?;
    let authorization = http(2, &url, &body);
    let result = c
        .post(&url)
        .header("authorization", &authorization)
        .body(body.clone())
        .send()
        .await?;
    assert!(result.status().is_success(), "{}", result.text().await?);
    assert_eq!(
        c.post(&url)
            .header("authorization", &authorization)
            .body(body)
            .send()
            .await?
            .status(),
        403
    );
    wait_projected(&c, origin, &first.id).await?;
    let duplicate: Value = post_json(&c, 2, &url, &value).await?.json().await?;
    assert_eq!(duplicate["inserted"], false);
    let second = sign_at(2, 0, vec![], "{\"name\":\"second\"}".into(), at + 1);
    assert!(
        post_json(&c, 2, &url, &serde_json::to_value(&second)?)
            .await?
            .status()
            .is_success()
    );
    wait_projected(&c, origin, &second.id).await?;
    let head: Value = c.get(&head_url).send().await?.json().await?;
    assert_eq!(head["id"], second.id);
    assert_eq!(head["projected"], true);
    assert_eq!(
        c.get(format!("{origin}/events/{}", first.id))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(post_json(&c, 2, &url, &value).await?.status(), 409);
    let other = sign_at(3, 0, vec![], "{}".into(), at + 2);
    assert_eq!(
        post_json(&c, 2, &url, &serde_json::to_value(other)?)
            .await?
            .status(),
        403
    );
    let pointer = sign_at(2, 11999, vec![], "{}".into(), at + 2);
    assert_eq!(
        post_json(&c, 2, &url, &serde_json::to_value(pointer)?)
            .await?
            .status(),
        400
    );

    let library_url = format!("{origin}/library/objects/chunk");
    let chunk = manga_node::library_manifest::Chunk::new(
        manga_node::library_manifest::Family::Chapters,
        vec![manga_node::library_manifest::Record::Chapter {
            id: [6; 32],
            completed: true,
            page: None,
            updated_at: 1,
        }],
        &manga_node::library_manifest::Limits::default(),
    )?
    .encode(&manga_node::library_manifest::Limits::default())?;
    assert_eq!(
        c.post(&library_url)
            .body(chunk.clone())
            .send()
            .await?
            .status(),
        403
    );
    let result = c
        .post(&library_url)
        .header("authorization", http(2, &library_url, &chunk))
        .body(chunk.clone())
        .send()
        .await?;
    assert_eq!(result.status(), 200, "{}", result.text().await?);
    let staged_hash = auth::hash(&chunk);
    let read_url = format!("{origin}/library/staging/{staged_hash}");
    assert_eq!(c.get(&read_url).send().await?.status(), 403);
    assert_eq!(
        post_json(
            &c,
            1,
            &format!("{origin}/admin/accounts/{}", public(3)),
            &json!({"allowed":true})
        )
        .await?
        .status(),
        200
    );
    assert_eq!(
        c.get(&read_url)
            .header("authorization", http_get(3, &read_url))
            .send()
            .await?
            .status(),
        403
    );
    let response = c
        .get(&read_url)
        .header("authorization", http_get(2, &read_url))
        .send()
        .await?;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    assert_eq!(response.bytes().await?.as_ref(), chunk.as_slice());
    assert_eq!(
        c.get(format!("{origin}/originals/{staged_hash}"))
            .send()
            .await?
            .status(),
        404
    );
    let bad = b"invalid cbor";
    assert_eq!(
        c.post(&library_url)
            .header("authorization", http(2, &library_url, bad))
            .body(bad.to_vec())
            .send()
            .await?
            .status(),
        400
    );

    let limits = manga_node::library_manifest::Limits::default();
    let summary = manga_node::library_manifest::Chunk::decode(&chunk, &limits)?.1;
    let root = manga_node::library_manifest::Root {
        families: manga_node::library_manifest::Family::ALL
            .into_iter()
            .map(|family| manga_node::library_manifest::FamilyRoot {
                family,
                chunks: vec![
                    if family == manga_node::library_manifest::Family::Chapters {
                        manga_node::library_manifest::Descriptor::from(&summary)
                    } else {
                        manga_node::library_manifest::Descriptor::empty()
                    },
                ],
            })
            .collect(),
    }
    .encode(&limits)?;
    let root_url = format!("{origin}/library/objects/root");
    assert_eq!(
        c.post(&root_url)
            .header("authorization", http(2, &root_url, &root))
            .body(root.clone())
            .send()
            .await?
            .status(),
        200
    );
    let public_chunk = format!("{origin}/library/blobs/{staged_hash}");
    assert_eq!(c.get(&public_chunk).send().await?.status(), 404);
    let library_event = sign_at(
        2,
        11999,
        vec![],
        json!({"manifest":auth::hash(&root),"version":1}).to_string(),
        at + 2,
    );
    let commit_url = format!("{origin}/libraries");
    let wrapper = json!({"event":library_event.clone(),"expected_head":null});
    let admitted = post_json(&c, 2, &commit_url, &wrapper).await?;
    assert!(admitted.status().is_success(), "{}", admitted.text().await?);
    wait_projected(&c, origin, &library_event.id).await?;
    let library_head_url = format!("{origin}/libraries/{}", public(2));
    let head: Value = c.get(&library_head_url).send().await?.json().await?;
    assert_eq!(head["event"]["id"], library_event.id);
    assert_eq!(head["projected"], true);
    assert_eq!(
        c.get(&public_chunk).send().await?.bytes().await?.as_ref(),
        chunk.as_slice()
    );
    let duplicate: Value = post_json(
        &c,
        2,
        &commit_url,
        &json!({"event":library_event.clone(),"expected_head":"77".repeat(32)}),
    )
    .await?
    .json()
    .await?;
    assert_eq!(duplicate["inserted"], false);
    let newer = sign_at(2, 11999, vec![], library_event.content.clone(), at + 3);
    assert_eq!(
        post_json(
            &c,
            2,
            &commit_url,
            &json!({"event":newer,"expected_head":"77".repeat(32)})
        )
        .await?
        .status(),
        409
    );
    assert_eq!(post_json(&c, 3, &commit_url, &wrapper).await?.status(), 403);

    use base64::{Engine, engine::general_purpose::STANDARD};
    let batch_read = format!("{origin}/library/read");
    let response = c
        .post(&batch_read)
        .header("accept-encoding", "gzip")
        .json(&json!({"hashes":[auth::hash(&root),staged_hash]}))
        .send()
        .await?;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-encoding"], "gzip");
    let decoded = manga_node::library_wire::decode_body(response.bytes().await?.to_vec(), true)?;
    let bundle: Value = serde_json::from_slice(&decoded)?;
    assert_eq!(bundle["objects"].as_array().unwrap().len(), 2);
    for object in bundle["objects"].as_array().unwrap() {
        let bytes = STANDARD.decode(object["data"].as_str().unwrap())?;
        assert_eq!(auth::hash(&bytes), object["hash"].as_str().unwrap());
    }
    let next_chunk = manga_node::library_manifest::Chunk::new(
        manga_node::library_manifest::Family::Chapters,
        vec![manga_node::library_manifest::Record::Chapter {
            id: [7; 32],
            completed: true,
            page: Some(4),
            updated_at: 2,
        }],
        &limits,
    )?
    .encode(&limits)?;
    let batch_url = format!("{origin}/library/batches");
    let staged = post_json(
        &c,
        2,
        &batch_url,
        &json!({"objects":[{"type":"chunk","data":STANDARD.encode(&next_chunk)}]}),
    )
    .await?;
    assert_eq!(staged.status(), 200);
    assert_eq!(staged.json::<Value>().await?["public"], false);
    assert_eq!(
        c.post(&batch_read)
            .json(&json!({"hashes":[staged_hash,auth::hash(&next_chunk)]}))
            .send()
            .await?
            .status(),
        404,
        "a mixed batch exposed a private object"
    );
    let summary = manga_node::library_manifest::Chunk::decode(&next_chunk, &limits)?.1;
    let next_root = manga_node::library_manifest::Root {
        families: manga_node::library_manifest::Family::ALL
            .into_iter()
            .map(|family| manga_node::library_manifest::FamilyRoot {
                family,
                chunks: vec![
                    if family == manga_node::library_manifest::Family::Chapters {
                        manga_node::library_manifest::Descriptor::from(&summary)
                    } else {
                        manga_node::library_manifest::Descriptor::empty()
                    },
                ],
            })
            .collect(),
    }
    .encode(&limits)?;
    let next_event = sign_at(
        2,
        11999,
        vec![],
        json!({"manifest":auth::hash(&next_root),"version":1}).to_string(),
        at + 4,
    );
    let combined = serde_json::to_vec(
        &json!({"event":next_event.clone(),"expected_head":library_event.id,"objects":[{"type":"chunk","data":STANDARD.encode(&next_chunk)},{"type":"root","data":STANDARD.encode(&next_root)}]}),
    )?;
    let compressed = manga_node::library_wire::compress(combined.clone())?;
    assert_eq!(
        c.post(&commit_url)
            .header("content-encoding", "gzip")
            .header("authorization", http(2, &commit_url, &combined))
            .body(compressed.clone())
            .send()
            .await?
            .status(),
        403,
        "body authentication must cover exact wire bytes"
    );
    let committed = c
        .post(&commit_url)
        .header("content-encoding", "gzip")
        .header("authorization", http(2, &commit_url, &compressed))
        .body(compressed)
        .send()
        .await?;
    assert!(
        committed.status().is_success(),
        "{}",
        committed.text().await?
    );
    wait_projected(&c, origin, &next_event.id).await?;
    let head: Value = c.get(&library_head_url).send().await?.json().await?;
    assert_eq!(head["event"]["id"], next_event.id);
    let bundle: Value = c
        .post(&batch_read)
        .header("accept-encoding", "gzip;q=0")
        .json(&json!({"hashes":[auth::hash(&next_root),auth::hash(&next_chunk)]}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(bundle["objects"].as_array().unwrap().len(), 2);
    assert_eq!(
        c.post(&batch_read)
            .json(&json!({"hashes":vec![staged_hash.clone();65]}))
            .send()
            .await?
            .status(),
        400
    );
    let bomb = manga_node::library_wire::compress(vec![0; manga_node::library_wire::MAX_WIRE + 1])?;
    assert_eq!(
        c.post(&commit_url)
            .header("content-encoding", "gzip")
            .header("authorization", http(2, &commit_url, &bomb))
            .body(bomb)
            .send()
            .await?
            .status(),
        400
    );
    assert_eq!(
        post_json(&c, 1, &admin_url, &json!({"allowed":false}))
            .await?
            .status(),
        200
    );
    assert_eq!(
        post_json(&c, 2, &url, &serde_json::to_value(second)?)
            .await?
            .status(),
        403
    );
    assert_eq!(
        c.get(&read_url)
            .header("authorization", http_get(2, &read_url))
            .send()
            .await?
            .status(),
        403
    );
    assert_eq!(c.get(library_head_url).send().await?.status(), 200);
    assert_eq!(c.get(public_chunk).send().await?.status(), 200);
    assert_eq!(c.get(head_url).send().await?.status(), 200); // Posting revocation is not deletion.
    // NIP09 replaceable coordinates establish a durable cutoff, while a newer
    // library may reuse the same retained immutable root without revision links.
    assert_eq!(
        post_json(&c, 1, &admin_url, &json!({"allowed":true}))
            .await?
            .status(),
        200
    );
    let delete = sign_at(
        2,
        5,
        vec![vec!["a".into(), format!("11999:{}:", public(2))]],
        "Delete library".into(),
        auth::now() - 1,
    );
    let response = post_json(
        &c,
        2,
        &format!("{origin}/account-deletions"),
        &serde_json::to_value(&delete)?,
    )
    .await?;
    assert_eq!(response.status(), 202, "{}", response.text().await?);
    assert_eq!(
        c.get(format!("{origin}/libraries/{}", public(2)))
            .send()
            .await?
            .status(),
        404
    );
    let blocked = sign_at(
        2,
        11999,
        vec![],
        next_event.content.clone(),
        delete.created_at,
    );
    assert_eq!(
        post_json(
            &c,
            2,
            &commit_url,
            &json!({"event":blocked,"expected_head":null})
        )
        .await?
        .status(),
        400
    );
    let fresh = sign_at(2, 11999, vec![], next_event.content.clone(), auth::now());
    let response = post_json(
        &c,
        2,
        &commit_url,
        &json!({"event":fresh,"expected_head":null}),
    )
    .await?;
    assert!(response.status().is_success(), "{}", response.text().await?);
    wait_projected(&c, origin, &fresh.id).await?;
    assert_eq!(
        c.get(format!("{origin}/libraries/{}", public(2)))
            .send()
            .await?
            .status(),
        200
    );
    println!(
        "NIP-98 author binding, explicit operator admission, replay rejection, idempotent retry, public head and replacement visibility passed"
    );
    Ok(())
}
