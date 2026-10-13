mod support;
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
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
async fn policy(
    c: &reqwest::Client,
    origin: &str,
    admin: u8,
    peer: u8,
    endpoint: &str,
    allowed: bool,
) -> Result<()> {
    let r = post_json(
        c,
        admin,
        &format!("{origin}/admin/peers"),
        &json!({"key":public(peer),"endpoint":endpoint,"allowed":allowed}),
    )
    .await?;
    anyhow::ensure!(r.status() == 200, "peer policy failed: {}", r.text().await?);
    Ok(())
}
async fn sync(
    c: &reqwest::Client,
    origin: &str,
    admin: u8,
    peer: u8,
    event: &str,
    filter: &str,
    fallbacks: Vec<String>,
) -> Result<reqwest::Response> {
    post_json(
        c,
        admin,
        &format!("{origin}/admin/sync"),
        &json!({"peer":public(peer),"event":event,"filter":filter,"fallbacks":fallbacks}),
    )
    .await
}
async fn reads(c: &reqwest::Client, origin: &str, admin: u8) -> Result<u64> {
    let url = format!("{origin}/admin/status");
    let value: Value = c
        .get(&url)
        .header("authorization", http_get(admin, &url))
        .send()
        .await?
        .json()
        .await?;
    value["peer_blob_reads"]
        .as_u64()
        .context("peer read counter missing")
}
async fn publish(
    c: &reqwest::Client,
    origin: &str,
    bytes: &[u8],
    chapter: &str,
) -> Result<auth::Event> {
    upload_to(c, 1, origin, bytes).await?;
    let manifest = cbor(
        json!({"version":1,"series":"ab".repeat(32),"language":"en-US","chapter":chapter,"pages":[page(bytes),page(bytes)]}),
    );
    upload_to(c, 1, origin, &manifest).await?;
    let event = publication(1, 1001, &manifest);
    let r = post_json(
        c,
        1,
        &format!("{origin}/publications"),
        &serde_json::to_value(&event)?,
    )
    .await?;
    anyhow::ensure!(
        r.status().is_success(),
        "publication failed: {}",
        r.text().await?
    );
    wait_projected(c, origin, &event.id).await?;
    Ok(event)
}
/// Measure physical capacity through the same HTTP upload/delete operations used
/// by publishers. Successful fill plus a one-byte rejection checks both under-
/// and over-charging; deleting only our temporary blobs restores the fixture.
async fn assert_http_storage_used(c: &reqwest::Client, origin: &str, used: usize) -> Result<()> {
    let mut remaining = 2_097_152usize
        .checked_sub(used)
        .context("fixture capacity")?;
    let probe = [251u8];
    let probe_hash = upload_to(c, 249, origin, &probe).await?;
    let control = c
        .delete(format!("{origin}/blobs/{probe_hash}"))
        .header("authorization", blossom(249, origin, "delete", &probe_hash))
        .send()
        .await?;
    assert_eq!(control.status(), 200, "one-byte control cleanup");
    let mut fillers = Vec::new();
    while remaining > 0 {
        let size = remaining.min(262_144);
        let nonce: [u8; 32] = rand::random();
        let mut bytes: Vec<u8> = (0..size).map(|i| nonce[i % nonce.len()]).collect();
        if bytes == probe {
            bytes[0] = 250;
        }
        fillers.push(upload_to(c, 249, origin, &bytes).await?);
        remaining -= size;
    }
    let hash = probe_hash;
    let response = c
        .put(format!("{origin}/upload/{hash}"))
        .header("authorization", blossom(249, origin, "upload", &hash))
        .body(probe.to_vec())
        .send()
        .await?;
    assert_eq!(
        response.status(),
        503,
        "physical capacity must be exhausted"
    );
    for hash in fillers {
        let response = c
            .delete(format!("{origin}/blobs/{hash}"))
            .header("authorization", blossom(249, origin, "delete", &hash))
            .send()
            .await?;
        anyhow::ensure!(
            response.status() == 200,
            "probe cleanup failed: {}",
            response.text().await?
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and real strfry; run scripts/test-postgres.sh"]
async fn approved_three_node_transfer_and_fallback() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let parsed: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(
        parsed.get_dbname() == Some("manga_test") && dsn.contains("dbname=manga_test"),
        "fixture required"
    );
    let a = repo(dsn.clone()).await?;
    a.migrate().await?;
    a.pool
        .get()
        .await?
        .batch_execute("CREATE DATABASE manga_peer_b;")
        .await?;
    a.pool
        .get()
        .await?
        .batch_execute("CREATE DATABASE manga_peer_c;")
        .await?;
    let bdsn = dsn.replace("dbname=manga_test", "dbname=manga_peer_b");
    let cdsn = dsn.replace("dbname=manga_test", "dbname=manga_peer_c");
    let b = repo(bdsn.clone()).await?;
    b.migrate().await?;
    let c_repo = repo(cdsn.clone()).await?;
    c_repo.migrate().await?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let retained = tempfile::tempdir()?.keep();
    let dir = retained.as_path();
    eprintln!("retained peer fixture: {}", dir.display());
    let binary = std::env::var_os("STRFRY_BIN")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("vendor/strfry/strfry"));
    let origins = [
        "http://127.0.0.1:19510",
        "http://127.0.0.1:19511",
        "http://127.0.0.1:19512",
    ];
    let mut nodes = vec![];
    for (index, dsn) in [dsn, bdsn, cdsn].iter().enumerate() {
        let base = dir.join(format!("node-{index}"));
        std::fs::create_dir_all(&base)?;
        let key = base.join("key");
        write_key(&key, 11 + index as u8)?;
        let config = base.join("config.json");
        std::fs::write(
            &config,
            serde_json::to_vec(
                &json!({"bind":format!("127.0.0.1:{}",19510+index),"base_url":origins[index],"service_key_file":key,"admin_pubkey":public(1+index as u8),"database_plaintext_local":true,"discovery_private_overrides":origins,"backend":{"binary":binary,"data_dir":base.join("backend"),"timeout_seconds":3,"map_bytes":67108864},"storage":{"path":base.join("originals"),"volume":format!("peer{index}"),"physical_bytes":2097152,"staging_bytes":2097152,"max_blob_bytes":262144}}),
            )?,
        )?;
        let log = std::fs::File::create(base.join("node.log"))?;
        nodes.push(Some(Node(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", config.to_str().unwrap()])
                .env("MANGA_DATABASE_URL", dsn)
                .env("RUST_LOG", "warn")
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        )));
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()?;
    for (index, origin) in origins.iter().enumerate() {
        let mut ready = false;
        for _ in 0..100 {
            if client.get(*origin).send().await.is_ok() {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        assert!(
            ready,
            "{}",
            std::fs::read_to_string(dir.join(format!("node-{index}/node.log")))?
        );
    }
    use base64::{Engine, engine::general_purpose::STANDARD};
    use manga_node::library_manifest::{
        Chunk, Descriptor, Family, FamilyRoot, Limits, Record, Root,
    };
    let limits = Limits::default();
    let library_chunk = Chunk::new(
        Family::Chapters,
        vec![Record::Chapter {
            id: [51; 32],
            completed: true,
            page: Some(12),
            updated_at: 1,
        }],
        &limits,
    )?
    .encode(&limits)?;
    let descriptor = Descriptor::from(&Chunk::decode(&library_chunk, &limits)?.1);
    let library_root = Root {
        families: Family::ALL
            .into_iter()
            .map(|family| FamilyRoot {
                family,
                chunks: vec![if family == Family::Chapters {
                    descriptor.clone()
                } else {
                    Descriptor::empty()
                }],
            })
            .collect(),
    }
    .encode(&limits)?;
    assert_eq!(
        post_json(
            &client,
            1,
            &format!("{}/admin/accounts/{}", origins[0], public(21)),
            &json!({"allowed":true})
        )
        .await?
        .status(),
        200
    );
    let library_event = sign_at(
        21,
        11999,
        vec![],
        json!({"manifest":auth::hash(&library_root),"version":1}).to_string(),
        auth::now(),
    );
    let library_body = json!({"event":library_event,"expected_head":null,"objects":[{"type":"chunk","data":STANDARD.encode(&library_chunk)},{"type":"root","data":STANDARD.encode(&library_root)}]});
    let response = post_json(
        &client,
        21,
        &format!("{}/libraries", origins[0]),
        &library_body,
    )
    .await?;
    assert!(response.status().is_success(), "{}", response.text().await?);
    wait_projected(&client, origins[0], &library_event.id).await?;
    let library_sync_url = format!("{}/admin/sync-library", origins[1]);
    let library_sync_body = json!({"peer":public(11),"publisher":public(21)});
    assert_eq!(
        post_json(&client, 2, &library_sync_url, &library_sync_body)
            .await?
            .status(),
        400,
        "unapproved library source"
    );
    let image = png();
    let event = publish(&client, origins[0], &image, "1").await?;
    assert_eq!(
        sync(&client, origins[1], 2, 11, &event.id, "", vec![])
            .await?
            .status(),
        400,
        "unapproved outgoing peer blocked"
    );
    policy(&client, origins[1], 2, 11, origins[0], true).await?;
    b.pool
        .get()
        .await?
        .execute(
            "UPDATE approved_peers SET discovered_endpoint=true WHERE service_key=$1",
            &[&id(&public(11)).as_slice()],
        )
        .await?;
    assert!(
        b.approved_peer(id(&public(11))).await?.discovered_endpoint,
        "learned destinations use pinned verified transport"
    );

    assert_eq!(
        sync(&client, origins[1], 2, 11, &event.id, "", vec![])
            .await?
            .status(),
        400,
        "source must separately approve receiver service key"
    );
    assert_eq!(
        post_json(&client, 2, &library_sync_url, &library_sync_body)
            .await?
            .status(),
        400,
        "source must approve library receiver"
    );
    policy(&client, origins[0], 1, 12, origins[1], true).await?;
    let response = post_json(&client, 2, &library_sync_url, &library_sync_body).await?;
    assert!(response.status().is_success(), "{}", response.text().await?);
    let result: Value = response.json().await?;
    assert_eq!(result["objects_fetched"], 2);
    assert_eq!(result["hosted"], true);
    wait_projected(&client, origins[1], &library_event.id).await?;
    assert_eq!(
        post_json(
            &client,
            21,
            &format!("{}/libraries", origins[1]),
            &library_body
        )
        .await?
        .status(),
        403,
        "replication must not grant personal posting authority"
    );
    let duplicate = post_json(&client, 2, &library_sync_url, &library_sync_body).await?;
    assert!(duplicate.status().is_success());
    assert_eq!(
        post_json(
            &client,
            21,
            &format!("{}/libraries", origins[1]),
            &library_body
        )
        .await?
        .status(),
        403
    );

    // Revoke peer approval during a blocked object-grant transaction. Its shared
    // approval lock may finish that stage, but no library head may commit after
    // the waiting revocation advances the peer epoch.
    assert_eq!(
        post_json(
            &client,
            1,
            &format!("{}/admin/accounts/{}", origins[0], public(22)),
            &json!({"allowed":true})
        )
        .await?
        .status(),
        200
    );
    let interrupted_event = sign_at(
        22,
        11999,
        vec![],
        library_event.content.clone(),
        auth::now(),
    );
    let mut interrupted_body = library_body.clone();
    interrupted_body["event"] = serde_json::to_value(&interrupted_event)?;
    assert!(
        post_json(
            &client,
            22,
            &format!("{}/libraries", origins[0]),
            &interrupted_body
        )
        .await?
        .status()
        .is_success()
    );
    wait_projected(&client, origins[0], &interrupted_event.id).await?;
    let fault = deadpool_postgres::Object::take(b.pool.get().await?);
    fault
        .execute("SELECT pg_advisory_lock(927443)", &[])
        .await?;
    fault.batch_execute("CREATE FUNCTION fixture_pause_peer_library() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(927443); RETURN NEW; END $$; CREATE TRIGGER fixture_pause_peer_library BEFORE INSERT ON account_blob_grants FOR EACH ROW EXECUTE FUNCTION fixture_pause_peer_library();").await?;
    let transfer_client = client.clone();
    let transfer_url = library_sync_url.clone();
    let transfer = tokio::spawn(async move {
        post_json(
            &transfer_client,
            2,
            &transfer_url,
            &json!({"peer":public(11),"publisher":public(22)}),
        )
        .await
    });
    let blocker_pid: i32 = fault
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    let transfer_pid = wait_for_blocked_session(&b, blocker_pid).await?;
    let revoke_client = client.clone();
    let destination = origins[1];
    let source = origins[0];
    let revoke =
        tokio::spawn(
            async move { policy(&revoke_client, destination, 2, 11, source, false).await },
        );
    wait_for_blocked_session(&b, transfer_pid).await?;
    fault
        .execute("SELECT pg_advisory_unlock(927443)", &[])
        .await?;
    revoke.await??;
    assert_eq!(transfer.await??.status(), 400);
    fault.batch_execute("DROP TRIGGER fixture_pause_peer_library ON account_blob_grants;DROP FUNCTION fixture_pause_peer_library();").await?;
    assert_eq!(
        client
            .get(format!("{}/libraries/{}", origins[1], public(22)))
            .send()
            .await?
            .status(),
        404,
        "revoked transfer must not publish an incomplete head"
    );
    policy(&client, origins[1], 2, 11, origins[0], true).await?;
    let wrong_url = format!("{}/federation/events/{}", origins[0], event.id);
    assert_eq!(
        client
            .get(&wrong_url)
            .header("authorization", http_get(13, &wrong_url))
            .send()
            .await?
            .status(),
        403,
        "another key cannot inherit access at same URL"
    );
    policy(&client, origins[1], 2, 13, origins[0], true).await?;
    assert_eq!(
        sync(&client, origins[1], 2, 13, &event.id, "", vec![])
            .await?
            .status(),
        400,
        "expected key must match nonce proof, not just URL"
    );
    let before = reads(&client, origins[0], 1).await?;
    let filtered = sync(
        &client,
        origins[1],
        2,
        11,
        &event.id,
        "language=\"ja\"",
        vec![],
    )
    .await?;
    assert_eq!(filtered.status(), 200);
    let filtered: Value = filtered.json().await?;
    assert_eq!(filtered["matched"], false);
    assert_eq!(filtered["originals_fetched"], 0);
    assert_eq!(
        reads(&client, origins[0], 1).await? - before,
        1,
        "only manifest should be downloaded for out-of-filter content"
    );

    assert_eq!(
        post_json(
            &client,
            2,
            &format!("{}/admin/publishers/{}", origins[1], public(249)),
            &json!({"allowed":true})
        )
        .await?
        .status(),
        200
    );
    assert_eq!(
        client
            .get(format!("{}/events/{}", origins[1], event.id))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        client
            .get(format!("{}/originals/{}", origins[1], auth::hash(&image)))
            .send()
            .await?
            .status(),
        404
    );
    let library_bytes = library_root.len() + library_chunk.len();
    assert_http_storage_used(&client, origins[1], library_bytes).await?;
    let received = sync(
        &client,
        origins[1],
        2,
        11,
        &event.id,
        "language=\"EN\" AND pages=2",
        vec![],
    )
    .await?;
    assert!(received.status().is_success(), "{}", received.text().await?);
    let received: Value = received.json().await?;
    assert_eq!(received["originals_fetched"], 1);
    wait_projected(&client, origins[1], &event.id).await?;
    // Signed social/profile events and deletion evidence follow the approved
    // peer path without enabling the author's personal posting login.
    let comment = sign_at(
        21,
        1111,
        vec![
            vec!["E".into(), event.id.clone()],
            vec!["K".into(), "1001".into()],
            vec!["P".into(), public(1)],
            vec!["e".into(), event.id.clone()],
            vec!["k".into(), "1001".into()],
            vec!["p".into(), public(1)],
        ],
        "Federated comment".into(),
        auth::now(),
    );
    assert!(
        post_json(
            &client,
            21,
            &format!("{}/social", origins[0]),
            &serde_json::to_value(&comment)?
        )
        .await?
        .status()
        .is_success()
    );
    wait_projected(&client, origins[0], &comment.id).await?;
    let account_sync_url = format!("{}/admin/sync-account", origins[1]);
    let imported = post_json(
        &client,
        2,
        &account_sync_url,
        &json!({"peer":public(11),"event":comment.id}),
    )
    .await?;
    assert!(imported.status().is_success(), "{}", imported.text().await?);
    wait_projected(&client, origins[1], &comment.id).await?;
    let profile = sign_at(
        21,
        0,
        vec![],
        json!({"name":"Synthetic peer author"}).to_string(),
        auth::now(),
    );
    assert!(
        post_json(
            &client,
            21,
            &format!("{}/account-metadata", origins[0]),
            &serde_json::to_value(&profile)?
        )
        .await?
        .status()
        .is_success()
    );
    wait_projected(&client, origins[0], &profile.id).await?;
    assert!(
        post_json(
            &client,
            2,
            &account_sync_url,
            &json!({"peer":public(11),"event":profile.id})
        )
        .await?
        .status()
        .is_success()
    );
    wait_projected(&client, origins[1], &profile.id).await?;
    let deletion = sign_at(
        21,
        5,
        vec![vec!["e".into(), comment.id.clone()]],
        "Federated deletion".into(),
        auth::now(),
    );
    assert_eq!(
        post_json(
            &client,
            21,
            &format!("{}/account-deletions", origins[0]),
            &serde_json::to_value(&deletion)?
        )
        .await?
        .status(),
        202
    );
    let applied = post_json(
        &client,
        2,
        &account_sync_url,
        &json!({"peer":public(11),"event":deletion.id}),
    )
    .await?;
    assert!(applied.status().is_success(), "{}", applied.text().await?);
    assert_eq!(
        client
            .get(format!("{}/events/{}", origins[1], comment.id))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        client
            .get(format!("{}/events/{}", origins[1], event.id))
            .send()
            .await?
            .status(),
        200
    );
    assert_eq!(
        post_json(
            &client,
            21,
            &format!("{}/social", origins[1]),
            &serde_json::to_value(&comment)?
        )
        .await?
        .status(),
        403
    );
    std::fs::write(
        root.join("results/account-peer-http.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["signed comment imported through approved peer","profile imported with stable author signature","signed deletion evidence hides replicated comment","protected chapter unaffected","remote author still lacks personal posting authority"],"scope":"selected synthetic account events; automatic account inventory scheduling is not included"}),
        )?,
    )?;
    let library_hash = auth::hash(&library_chunk);
    // Read generation only to locate the file for external corruption setup.
    let library_generation: Vec<u8> = b
        .pool
        .get()
        .await?
        .query_one(
            "SELECT generation FROM blob_intents WHERE hash=$1",
            &[&hex::decode(&library_hash)?],
        )
        .await?
        .get(0);
    let library_path = dir
        .join("node-1/originals/objects")
        .join(&library_hash[..2])
        .join(&library_hash[2..4])
        .join(format!(
            "{}.{}",
            library_hash,
            hex::encode(library_generation)
        ));
    std::fs::write(&library_path, b"damaged library chunk")?;
    assert_eq!(
        client
            .get(format!("{}/library/blobs/{library_hash}", origins[1]))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        client
            .get(format!("{}/libraries/{}", origins[1], public(21)))
            .send()
            .await?
            .status(),
        404
    );
    let response = post_json(
        &client,
        2,
        &format!("{}/admin/storage/repair", origins[1]),
        &json!({"peer":public(11),"hash":library_hash,"library":true}),
    )
    .await?;
    assert_eq!(response.status(), 200, "{}", response.text().await?);
    let repaired_chunk = client
        .get(format!("{}/library/blobs/{library_hash}", origins[1]))
        .send()
        .await?;
    assert_eq!(repaired_chunk.status(), 200);
    assert_eq!(repaired_chunk.bytes().await?.as_ref(), library_chunk);
    assert_eq!(
        client
            .get(format!("{}/libraries/{}", origins[1], public(21)))
            .send()
            .await?
            .status(),
        200
    );
    // Detect receiver corruption without first triggering a reader failure, then
    // restore identical original bytes through an explicitly approved source.
    let original_hash = auth::hash(&image);
    // Read generation only to locate the next external corruption target.
    let generation: Vec<u8> = b
        .pool
        .get()
        .await?
        .query_one(
            "SELECT generation FROM blob_intents WHERE hash=$1",
            &[&hex::decode(&original_hash)?],
        )
        .await?
        .get(0);
    let object_path = dir
        .join("node-1/originals/objects")
        .join(&original_hash[..2])
        .join(&original_hash[2..4])
        .join(format!("{}.{}", original_hash, hex::encode(&generation)));
    let manifest_bytes = cbor(
        json!({"version":1,"series":"ab".repeat(32),"language":"en-US","chapter":"1","pages":[page(&image),page(&image)]}),
    );
    let retained_bytes = library_bytes + manifest_bytes.len() + image.len();
    assert_http_storage_used(&client, origins[1], retained_bytes).await?;

    std::fs::write(&object_path, b"corrupt fixture bytes")?;
    let mut detected = false;
    for _ in 0..12 {
        let response = post_json(
            &client,
            2,
            &format!("{}/admin/storage/audit", origins[1]),
            &json!({"limit":2}),
        )
        .await?;
        assert_eq!(response.status(), 200, "{}", response.text().await?);
        let audit: Value = response.json().await?;
        if audit["quarantined"] == 1 {
            detected = true;
            break;
        }
    }
    assert!(detected, "rotating audit did not find corrupt original");
    assert_eq!(
        client
            .get(format!("{}/events/{}", origins[1], event.id))
            .send()
            .await?
            .status(),
        404
    );
    let repair_url = format!("{}/admin/storage/repair", origins[1]);
    let repair_body = json!({"peer":public(11),"hash":original_hash});
    assert_eq!(
        post_json(
            &client,
            2,
            &repair_url,
            &json!({"peer":public(13),"hash":original_hash})
        )
        .await?
        .status(),
        400,
        "wrong identity at approved endpoint must fail repair"
    );
    let repaired = post_json(&client, 2, &repair_url, &repair_body).await?;
    assert_eq!(repaired.status(), 200, "{}", repaired.text().await?);
    let repaired_original = client
        .get(format!("{}/originals/{original_hash}", origins[1]))
        .send()
        .await?;
    assert_eq!(repaired_original.status(), 200);
    assert_eq!(repaired_original.bytes().await?.as_ref(), image);
    assert_eq!(
        client
            .get(format!("{}/events/{}", origins[1], event.id))
            .send()
            .await?
            .status(),
        200
    );
    let repeated: Value = post_json(&client, 2, &repair_url, &repair_body)
        .await?
        .json()
        .await?;
    assert_eq!(repeated["repaired"], false);
    assert_http_storage_used(&client, origins[1], retained_bytes).await?;
    // Kill the actual receiver after repaired bytes are durable but before the
    // quarantine-clear transaction commits. Restart must stay fail-closed.
    std::fs::write(&object_path, b"second corruption")?;
    assert_eq!(
        client
            .get(format!("{}/originals/{original_hash}", origins[1]))
            .send()
            .await?
            .status(),
        404
    );
    fault
        .execute("SELECT pg_advisory_lock(927444)", &[])
        .await?;
    fault.batch_execute("CREATE FUNCTION fixture_pause_repair() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(927444); RETURN OLD; END $$; CREATE TRIGGER fixture_pause_repair BEFORE DELETE ON quarantined_blobs FOR EACH ROW EXECUTE FUNCTION fixture_pause_repair();").await?;
    let repair_client = client.clone();
    let pending_url = repair_url.clone();
    let pending_body = repair_body.clone();
    let pending_repair =
        tokio::spawn(
            async move { post_json(&repair_client, 2, &pending_url, &pending_body).await },
        );
    let blocked = wait_for_blocked_session(&b, blocker_pid).await?;
    // Fault-window precondition only: the external trigger has stopped commit,
    // and repaired bytes are on disk before we kill the node. Public restart
    // visibility, verified retry and exact HTTP capacity are the outcome oracles.
    assert_eq!(std::fs::read(&object_path)?, image);
    drop(nodes[1].take());
    fault
        .execute("SELECT pg_terminate_backend($1)", &[&blocked])
        .await?;
    fault
        .execute("SELECT pg_advisory_unlock(927444)", &[])
        .await?;
    fault.batch_execute("DROP TRIGGER fixture_pause_repair ON quarantined_blobs;DROP FUNCTION fixture_pause_repair();").await?;
    let _ = pending_repair.await;

    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("node-1/node.log"))?;
    nodes[1] = Some(Node(
        Command::new(env!("CARGO_BIN_EXE_manga-node"))
            .args(["serve", dir.join("node-1/config.json").to_str().unwrap()])
            .env(
                "MANGA_DATABASE_URL",
                std::env::var("MANGA_TEST_DATABASE_URL")?
                    .replace("dbname=manga_test", "dbname=manga_peer_b"),
            )
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    ));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if client.get(origins[1]).send().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    assert_eq!(
        client
            .get(format!("{}/events/{}", origins[1], event.id))
            .send()
            .await?
            .status(),
        404,
        "restart cannot infer healthy state merely from present bytes"
    );
    let retry: Value = post_json(&client, 2, &repair_url, &repair_body)
        .await?
        .json()
        .await?;
    assert_eq!(retry["repaired"], true);

    assert_eq!(
        client
            .get(format!("{}/events/{}", origins[1], event.id))
            .send()
            .await?
            .status(),
        200
    );
    let repaired_after_restart = client
        .get(format!("{}/originals/{original_hash}", origins[1]))
        .send()
        .await?;
    assert_eq!(repaired_after_restart.status(), 200);
    assert_eq!(repaired_after_restart.bytes().await?.as_ref(), image);
    assert_http_storage_used(&client, origins[1], retained_bytes).await?;
    std::fs::write(
        root.join("results/storage-audit-repair-http.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["bounded rotating audit detects corrupt original","library chunk corruption hides root until approved repair","quarantine hides affected chapter","wrong service identity rejected","approved peer restores exact bytes","healthy retry is no-op","physical charges unchanged","actual node killed after durable repair before quarantine-clear commit","restart stays hidden until verified retry, without double charging"],"scope":"synthetic loopback PostgreSQL/HTTP fixture; process kill is not power-loss certification"}),
        )?,
    )?;

    let local = client
        .get(format!("{}/originals/{}", origins[1], auth::hash(&image)))
        .send()
        .await?;
    assert_eq!(local.status(), 200);
    assert_eq!(local.bytes().await?.as_ref(), image);

    let denied = client
        .put(format!("{}/upload/{}", origins[1], auth::hash(&image)))
        .header(
            "authorization",
            blossom(1, origins[1], "upload", &auth::hash(&image)),
        )
        .body(image.clone())
        .send()
        .await?;
    assert_eq!(
        denied.status(),
        403,
        "replication must not grant publisher upload authority"
    );
    let mut other = std::io::Cursor::new(vec![]);
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(2, 2, image::Rgb([91u8, 7, 34])))
        .write_to(&mut other, image::ImageFormat::Png)?;
    let other = other.into_inner();
    let second = publish(&client, origins[0], &other, "2").await?;
    policy(&client, origins[2], 3, 11, origins[0], true).await?;
    policy(&client, origins[0], 1, 13, origins[2], true).await?;
    assert!(
        sync(&client, origins[2], 3, 11, &second.id, "", vec![])
            .await?
            .status()
            .is_success()
    );
    wait_projected(&client, origins[2], &second.id).await?;
    // Explicitly revoke the mistaken key/endpoint policy, then require independent fallback approval.
    policy(&client, origins[1], 2, 13, origins[2], false).await?;
    assert_eq!(
        sync(&client, origins[1], 2, 11, &second.id, "", vec![public(13)])
            .await?
            .status(),
        400
    );
    policy(&client, origins[1], 2, 13, origins[2], true).await?;
    policy(&client, origins[2], 3, 12, origins[1], true).await?;
    drop(nodes[0].take());
    let fallback = sync(&client, origins[1], 2, 11, &second.id, "", vec![public(13)]).await?;
    assert!(fallback.status().is_success(), "{}", fallback.text().await?);
    let fallback: Value = fallback.json().await?;
    assert_eq!(fallback["originals_fetched"], 1);
    assert_eq!(fallback["used_peers"], json!([public(13)]));
    wait_projected(&client, origins[1], &second.id).await?;
    drop(nodes[2].take());
    let local = client
        .get(format!("{}/originals/{}", origins[1], auth::hash(&other)))
        .send()
        .await?;
    assert_eq!(local.status(), 200);
    assert_eq!(
        local.bytes().await?.as_ref(),
        other,
        "reader must not depend on either source remaining up"
    );
    policy(&client, origins[1], 2, 11, origins[0], false).await?;
    assert_eq!(
        sync(&client, origins[1], 2, 11, &event.id, "", vec![])
            .await?
            .status(),
        400
    );
    let local_library = client
        .get(format!(
            "{}/library/blobs/{}",
            origins[1],
            auth::hash(&library_chunk)
        ))
        .send()
        .await?;
    assert_eq!(local_library.status(), 200);
    assert_eq!(local_library.bytes().await?.as_ref(), library_chunk);
    assert_eq!(
        post_json(&client, 2, &library_sync_url, &library_sync_body)
            .await?
            .status(),
        400,
        "revoked library peer blocked"
    );
    std::fs::write(
        root.join("results/library-peer-http.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["bilateral service-key approval required","signed public library root and chunk locally retained","exact retry","remote author receives no personal posting login","approval revoked during staging prevents final root commit","library reads survive source shutdown","revoked peer blocks further transfer"],"scope":"synthetic loopback library with one chapter-progress chunk; selected snapshot import, not automatic library inventory reconciliation"}),
        )?,
    )?;
    std::fs::write(
        root.join("results/postgres-http-peers.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"nodes":3,"checks":["bilateral explicit service-key approvals","learned endpoint uses restricted pinned transport","changed/unapproved key cannot inherit access from endpoint","nonce/endpoint/expected-key proof checked","manifest filter rejects before original download, observed source read count","repeated original fetched once and fully locally retained","foreign event signature preserved without granting local publisher login","fallback separately approved on both sides","failed origin repaired through approved independent mirror","receiver serves exact originals with both sources offline","revoked outgoing peer blocked"],"limits":["synthetic loopback PNG originals","single-chapter sync; bulk reconciliation is next ticket","identity proof is not TLS-channel-bound"]}),
        )?,
    )?;
    drop(nodes);
    std::fs::remove_dir_all(dir)?;
    Ok(())
}
