//! Genuine original signed publication/staging/commit and real wrapper HTTP.
//! No synthetic database library records, server user keys, or fake receipts.
#[path = "support/nostr_library.rs"]
mod fixture;
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use fixture::*;
use manga_node::{auth, library_client_bundle::SaveBundle, library_manifest::Record};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use weebdex_platform::public_api::{PostgresPublicIdResolver, PublicIdResolver, ResourceKind};
fn post_event(key: u8, url: &str, body: &[u8], service: u8, time: u64) -> auth::Event {
    sign_at(
        key,
        27235,
        vec![
            vec!["u".into(), url.into()],
            vec!["method".into(), "POST".into()],
            vec!["payload".into(), auth::hash(body)],
            vec!["service".into(), public(service)],
            vec!["nonce".into(), hex::encode(rand::random::<[u8; 16]>())],
        ],
        String::new(),
        time,
    )
}
fn post_proof(key: u8, url: &str, body: &[u8]) -> String {
    token(&post_event(key, url, body, 11, auth::now()))
}
fn extension(old: Option<&SaveBundle>, new: &SaveBundle, expected: Option<&str>) -> Value {
    let unchanged = old
        .into_iter()
        .flat_map(|bundle| bundle.objects())
        .map(|(_, hash, _)| hash)
        .collect::<BTreeSet<_>>();
    json!({"version":1,"commit":{"event":new.event(),"expected_head":expected,"objects":new.objects().filter(|(kind,hash,_)|*kind=="root"||!unchanged.contains(hash)).map(|(kind,_,bytes)|json!({"type":kind,"data":STANDARD.encode(bytes)})).collect::<Vec<_>>()}})
}
fn read_body(chapter: &str, read: bool, extension: Value) -> Vec<u8> {
    serde_json::to_vec(&if read {
        json!({"chapterIdsRead":[chapter],"nostr":extension})
    } else {
        json!({"chapterIdsUnread":[chapter],"nostr":extension})
    })
    .unwrap()
}
async fn post(f: &Fixture, url: &str, key: u8, body: &[u8], status: u16) -> Result<Value> {
    response(
        f.c.post(url)
            .header("content-type", "application/json")
            .header("authorization", post_proof(key, url, body))
            .body(body.to_vec()),
        status,
    )
    .await
}
async fn success(f: &Fixture, url: &str, key: u8, body: &[u8]) -> Result<Value> {
    let response =
        f.c.post(url)
            .header("content-type", "application/json")
            .header("authorization", post_proof(key, url, body))
            .body(body.to_vec())
            .send()
            .await?;
    let status = response.status();
    let value: Value = response.json().await?;
    ensure!(
        matches!(status.as_u16(), 200 | 202),
        "expected accepted original commit, got{status}:{value}"
    );
    ensure!(value["result"] == "ok");
    Ok(value)
}
async fn head(f: &Fixture, key: u8) -> Result<Option<String>> {
    Ok(f.repo
        .current_library_head(id(&public(key)))
        .await?
        .map(|head| head.event.id.clone()))
}
async fn hold_count(f: &Fixture, key: u8) -> Result<i64> {
    Ok(f.repo
        .pool
        .get()
        .await?
        .query_one(
            "SELECT count(*) FROM library_object_holds WHERE publisher=$1 AND state='current'",
            &[&id(&public(key)).as_slice()],
        )
        .await?
        .get(0))
}
async fn import(f: &Fixture) -> Result<()> {
    drain_backend(&f.repo).await?;
    for _ in 0..2 {
        for _ in 0..20 {
            if weebdex_platform::protocol::bridge::import_page(&f.state)
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?
                .completed_sweep
            {
                break;
            }
        }
    }
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires owned PostgreSQL18.6 fixture and actual pinned strfry"]
async fn client_signed_read_status_intent_is_exact_self_scoped_cas_bound_and_original_committed()
-> Result<()> {
    let f = Fixture::start().await?;
    let result = exercise(&f).await;
    f.shutdown().await?;
    result
}
async fn exercise(f: &Fixture) -> Result<()> {
    let s=publish(&f.runtime,1002,cbor(json!({"type":"series","version":1,"titles":{"en":"Mutation series"},"contentRating":"safe"})),BTreeMap::new()).await?;
    let other=publish(&f.runtime,1002,cbor(json!({"type":"series","version":1,"titles":{"en":"Other mutation series"},"contentRating":"safe"})),BTreeMap::new()).await?;
    let image = png();
    let mut chapters = vec![];
    for n in 1..=2 {
        chapters.push(publish(&f.runtime,1001,cbor(json!({"type":"chapter","version":1,"series":s.id,"language":"en","chapter":n.to_string(),"pages":[page(&image)]})),BTreeMap::from([(auth::hash(&image),image.clone())])).await?);
    }
    import(f).await?;
    let resolver = PostgresPublicIdResolver::new(f.state.db.clone());
    let client = f
        .state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let native: String = client
        .query_one(
            "SELECT id FROM platform.catalog_manga WHERE protocol_series_id=$1",
            &[&s.id],
        )
        .await?
        .get(0);
    let other_native: String = client
        .query_one(
            "SELECT id FROM platform.catalog_manga WHERE protocol_series_id=$1",
            &[&other.id],
        )
        .await?
        .get(0);
    let chapter_native: String = client
        .query_one(
            "SELECT id FROM platform.catalog_chapters WHERE protocol_event_id=$1",
            &[&chapters[0].id],
        )
        .await?
        .get(0);
    drop(client);
    let manga = resolver
        .preferred(ResourceKind::Manga, &native)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .context("manga alias")?;
    let other_alias = resolver
        .preferred(ResourceKind::Manga, &other_native)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    let ca = resolver
        .preferred(ResourceKind::Chapter, &chapter_native)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    let url = format!("{}/api/v1/manga/{manga}/read", f.origin);
    let status_url = format!("{}/api/v1/manga/{manga}/status", f.origin);
    let t = auth::now() - 30;
    let records = vec![
        chapter(&chapters[0], false, Some(2)),
        chapter(&chapters[1], true, None),
        Record::Series {
            id: id(&s.id),
            status: "reading".into(),
            updated_at: 1,
        },
    ];
    let a = bundle(2, t, records.clone())?;
    save(&f.c, &f.native_origin, 2, &a).await?;
    let b = bundle(3, t, vec![chapter(&chapters[0], true, None)])?;
    save(&f.c, &f.native_origin, 3, &b).await?;
    let holds = hold_count(f, 2).await?;
    let mut read_records = records.clone();
    read_records[0] = chapter(&chapters[0], true, Some(2));
    let read = bundle(2, t + 1, read_records.clone())?;
    let body = read_body(
        ca.as_str(),
        true,
        extension(Some(&a), &read, Some(&a.event().id)),
    );
    response(
        f.c.post(&url)
            .header("content-type", "application/json")
            .body(body.clone()),
        401,
    )
    .await?;
    post(f, &url, 5, &body, 403).await?;
    for event in [
        post_event(2, &url, &body, 12, auth::now()),
        post_event(2, &url, &body, 11, auth::now() - 120),
        post_event(2, &url, &body, 11, auth::now() + 120),
        post_event(
            2,
            "https://wrong-instance.invalid/api/v1/manga/x/read",
            &body,
            11,
            auth::now(),
        ),
        post_event(2, &url, b"wrong raw bytes", 11, auth::now()),
    ] {
        response(
            f.c.post(&url)
                .header("content-type", "application/json")
                .header("authorization", token(&event))
                .body(body.clone()),
            401,
        )
        .await?;
    }
    let plain = serde_json::to_vec(&json!({"chapterIdsRead":[ca]}))?;
    let receipt = post(f, &url, 2, &plain, 400).await?;
    ensure!(
        receipt["errors"][0]["detail"]
            .as_str()
            .unwrap()
            .contains("client-signed")
    );
    post(f, &format!("{url}?updateHistory=true"), 2, &body, 400).await?;
    post(
        f,
        &format!("{url}?updateHistory=false&updateHistory=false"),
        2,
        &body,
        400,
    )
    .await?;
    let mut invalid: Value = serde_json::from_slice(&body)?;
    invalid["nostr"]["version"] = json!(2);
    post(f, &url, 2, &serde_json::to_vec(&invalid)?, 400).await?;
    let mut invalid: Value = serde_json::from_slice(&body)?;
    invalid["nostr"]["commit"]
        .as_object_mut()
        .unwrap()
        .remove("expected_head");
    post(f, &url, 2, &serde_json::to_vec(&invalid)?, 400).await?;
    let mut invalid: Value = serde_json::from_slice(&body)?;
    invalid["nostr"]["commit"]["expected_head"] = json!("0".repeat(64));
    post(f, &url, 2, &serde_json::to_vec(&invalid)?, 409).await?;
    let mut invalid: Value = serde_json::from_slice(&body)?;
    invalid["nostr"]["commit"]["expected_head"] = Value::Null;
    post(f, &url, 2, &serde_json::to_vec(&invalid)?, 409).await?;
    let other_signed = bundle(3, t + 1, read_records.clone())?;
    let other_body = read_body(
        ca.as_str(),
        true,
        extension(Some(&a), &other_signed, Some(&a.event().id)),
    );
    post(f, &url, 2, &other_body, 403).await?;
    // Requested false no-op and every unrelated record change are rejected.
    let false_noop = bundle(2, t + 1, records.clone())?;
    post(
        f,
        &url,
        2,
        &read_body(
            ca.as_str(),
            true,
            extension(Some(&a), &false_noop, Some(&a.event().id)),
        ),
        400,
    )
    .await?;
    let mut changed = read_records.clone();
    changed[1] = Record::Chapter {
        id: id(&chapters[1].id),
        completed: true,
        page: None,
        updated_at: 2,
    };
    let bad = bundle(2, t + 1, changed)?;
    post(
        f,
        &url,
        2,
        &read_body(
            ca.as_str(),
            true,
            extension(Some(&a), &bad, Some(&a.event().id)),
        ),
        400,
    )
    .await?;
    let mut changed = read_records.clone();
    changed[2] = Record::Series {
        id: id(&s.id),
        status: "completed".into(),
        updated_at: 1,
    };
    let bad = bundle(2, t + 1, changed)?;
    post(
        f,
        &url,
        2,
        &read_body(
            ca.as_str(),
            true,
            extension(Some(&a), &bad, Some(&a.event().id)),
        ),
        400,
    )
    .await?;
    let wrong_series = format!("{}/api/v1/manga/{other_alias}/read", f.origin);
    post(f, &wrong_series, 2, &body, 404).await?;
    let wrong_kind = format!("{}/api/v1/manga/{ca}/read", f.origin);
    post(f, &wrong_kind, 2, &body, 404).await?;
    ensure!(head(f, 2).await? == Some(a.event().id.clone()));
    ensure!(head(f, 3).await? == Some(b.event().id.clone()));
    ensure!(hold_count(f, 2).await? == holds);
    // Replay consumed exactly once; a real accepted commit uses original holds/outbox.
    let authorization = post_proof(2, &url, &body);
    let accepted =
        f.c.post(&url)
            .header("content-type", "application/json")
            .header("authorization", authorization.clone())
            .body(body.clone())
            .send()
            .await?;
    let status = accepted.status();
    let accepted: Value = accepted.json().await?;
    ensure!(matches!(status.as_u16(), 200 | 202), "{status}:{accepted}");
    ensure!(accepted["nostr"]["head"] == read.event().id);
    response(
        f.c.post(&url)
            .header("content-type", "application/json")
            .header("authorization", authorization)
            .body(body.clone()),
        401,
    )
    .await?;
    post(f, &url, 2, &body, 409).await?;
    ensure!(head(f, 2).await? == Some(read.event().id.clone()));
    ensure!(head(f, 3).await? == Some(b.event().id.clone()));
    drain_backend(&f.repo).await?;
    ensure!(
        get(&f.c, &url, Some(2), 200).await?["data"]
            .as_array()
            .unwrap()
            .len()
            == 2
    );
    let unread = bundle(2, t + 2, records.clone())?;
    success(
        f,
        &url,
        2,
        &read_body(
            ca.as_str(),
            false,
            extension(Some(&read), &unread, Some(&read.event().id)),
        ),
    )
    .await?;
    let mut completed = records.clone();
    completed[2] = Record::Series {
        id: id(&s.id),
        status: "completed".into(),
        updated_at: 7,
    };
    let done = bundle(2, t + 3, completed.clone())?;
    let status_body = serde_json::to_vec(
        &json!({"status":"completed","nostr":extension(Some(&unread),&done,Some(&unread.event().id))}),
    )?;
    success(f, &status_url, 2, &status_body).await?;
    let mut removed = completed;
    removed.pop();
    let no_status = bundle(2, t + 4, removed)?;
    success(f,&status_url,2,&serde_json::to_vec(&json!({"status":null,"nostr":extension(Some(&done),&no_status,Some(&done.event().id))}))?).await?;
    ensure!(head(f, 2).await? == Some(no_status.event().id.clone()));
    let missing = serde_json::to_vec(
        &json!({"nostr":extension(Some(&done),&no_status,Some(&done.event().id))}),
    )?;
    post(f, &status_url, 2, &missing, 400).await?;
    let fresh = bundle(
        4,
        t + 5,
        vec![Record::Series {
            id: id(&s.id),
            status: "plan_to_read".into(),
            updated_at: 1,
        }],
    )?;
    success(
        f,
        &status_url,
        4,
        &serde_json::to_vec(&json!({"status":"plan_to_read","nostr":extension(None,&fresh,None)}))?,
    )
    .await?;
    ensure!(head(f, 4).await? == Some(fresh.event().id.clone()));
    // Removing an unread record cannot erase a previously signed partial page.
    let removed_page = bundle(2, t + 5, vec![chapter(&chapters[1], true, None)])?;
    post(
        f,
        &url,
        2,
        &read_body(
            ca.as_str(),
            false,
            extension(Some(&no_status), &removed_page, Some(&no_status.event().id)),
        ),
        400,
    )
    .await?;
    ensure!(head(f, 2).await? == Some(no_status.event().id.clone()));
    Ok(())
}

async fn wait_blocked(f: &Fixture, pattern: &str, minimum: i64) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(2),async {
        loop {
            let count:i64=f.repo.pool.get().await?.query_one("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE $1",&[&pattern]).await?.get(0);
            if count>=minimum { break Ok::<_,anyhow::Error>(()); }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.context("expected original transaction fence was not reached")?
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires owned PostgreSQL18.6 fixture and actual pinned strfry"]
async fn incomplete_body_does_not_consume_proof_and_disconnected_accepted_mutation_is_original_owned_through_shutdown()
-> Result<()> {
    let mut f = Fixture::start().await?;
    let result = lifecycle(&mut f).await;
    f.shutdown().await?;
    result
}
async fn lifecycle(f: &mut Fixture) -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let s=publish(&f.runtime,1002,cbor(json!({"type":"series","version":1,"titles":{"en":"Lifecycle series"},"contentRating":"safe"})),BTreeMap::new()).await?;
    let image = png();
    let ch=publish(&f.runtime,1001,cbor(json!({"type":"chapter","version":1,"series":s.id,"language":"en","chapter":"1","pages":[page(&image)]})),BTreeMap::from([(auth::hash(&image),image)])).await?;
    import(f).await?;
    let resolver = PostgresPublicIdResolver::new(f.state.db.clone());
    let client = f
        .state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let native: String = client
        .query_one(
            "SELECT id FROM platform.catalog_manga WHERE protocol_series_id=$1",
            &[&s.id],
        )
        .await?
        .get(0);
    let chapter_native: String = client
        .query_one(
            "SELECT id FROM platform.catalog_chapters WHERE protocol_event_id=$1",
            &[&ch.id],
        )
        .await?
        .get(0);
    drop(client);
    let manga = resolver
        .preferred(ResourceKind::Manga, &native)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    let ca = resolver
        .preferred(ResourceKind::Chapter, &chapter_native)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    let url = format!("{}/api/v1/manga/{manga}/read", f.origin);
    let parsed = url::Url::parse(&url)?;
    let address = parsed.socket_addrs(|| None)?[0];
    let path = parsed.path();
    // Anonymous slow bodies are refused from signed headers, without waiting for
    // or allocating their claimed body. The ordinary wrapper request slot drops.
    let mut anonymous = tokio::net::TcpStream::connect(address).await?;
    anonymous.write_all(format!("POST {path} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: 1000000\r\nConnection: close\r\n\r\n",parsed.authority()).as_bytes()).await?;
    let mut prefix = [0; 1024];
    let read = tokio::time::timeout(Duration::from_secs(1), anonymous.read(&mut prefix)).await??;
    ensure!(String::from_utf8_lossy(&prefix[..read]).starts_with("HTTP/1.1 401"));
    drop(anonymous);
    let t = auth::now() - 30;
    let fresh = bundle(4, t, vec![chapter(&ch, true, None)])?;
    let body = read_body(ca.as_str(), true, extension(None, &fresh, None));
    let authorization = post_proof(4, &url, &body);
    let mut incomplete = tokio::net::TcpStream::connect(address).await?;
    incomplete.write_all(format!("POST {path} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAuthorization: {authorization}\r\nConnection: close\r\n\r\n",parsed.authority(),body.len()).as_bytes()).await?;
    incomplete.write_all(&body[..body.len() / 2]).await?;
    tokio::time::sleep(Duration::from_millis(25)).await;
    drop(incomplete);
    tokio::time::sleep(Duration::from_millis(25)).await;
    ensure!(head(f, 4).await?.is_none());
    let response =
        f.c.post(&url)
            .header("content-type", "application/json")
            .header("authorization", authorization)
            .body(body)
            .send()
            .await?;
    let status = response.status();
    let result: Value = response.json().await?;
    ensure!(
        matches!(status.as_u16(), 200 | 202),
        "incomplete body consumed proof or leaked receipt:{status}:{result}"
    );
    ensure!(head(f, 4).await? == Some(fresh.event().id.clone()));
    // Hold the original author fence after full proof verification/owned spawn.
    let old = bundle(2, t, vec![chapter(&ch, false, None)])?;
    save(&f.c, &f.native_origin, 2, &old).await?;
    let next = bundle(2, t + 1, vec![chapter(&ch, true, None)])?;
    let body = read_body(
        ca.as_str(),
        true,
        extension(Some(&old), &next, Some(&old.event().id)),
    );
    let mut blocker = f.repo.pool.get().await?;
    let tx = blocker.transaction().await?;
    tx.query_one(
        "SELECT publisher FROM account_permissions WHERE publisher=$1 FOR UPDATE",
        &[&id(&public(2)).as_slice()],
    )
    .await?;
    let client = f.c.clone();
    let endpoint = url.clone();
    let bytes = body.clone();
    let authorization = post_proof(2, &url, &body);
    let caller = tokio::spawn(async move {
        client
            .post(endpoint)
            .header("content-type", "application/json")
            .header("authorization", authorization)
            .body(bytes)
            .send()
            .await
    });
    wait_blocked(f, "%account_permissions%FOR SHARE%", 1).await?;
    caller.abort();
    let _ = caller.await;
    f.signal_native_shutdown();
    tokio::time::sleep(Duration::from_millis(30)).await;
    ensure!(
        !f.native_finished(),
        "original shutdown reported completion while accepted mutation held its fence"
    );
    post(f, &url, 2, &body, 503).await?;
    tx.rollback().await?;
    drop(blocker);
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if head(f, 2).await? == Some(next.event().id.clone()) {
                break Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("disconnected original-owned commit was lost")??;
    ensure!(hold_count(f, 2).await? > 0);
    Ok(())
}
async fn stage_only(f: &Fixture, bundle: &SaveBundle) -> Result<()> {
    let mut cursor = 0;
    while let Some(batch) = bundle.batch(cursor)? {
        let url = format!("{}/compat/mangadex/nostr/library/batches", f.native_origin);
        let response =
            f.c.post(&url)
                .header("authorization", http(2, &url, &batch.body))
                .body(batch.body)
                .send()
                .await?;
        ensure!(
            response.status().is_success(),
            "original private staging failed:{}",
            response.text().await?
        );
        cursor = batch.next_object;
    }
    Ok(())
}
fn changed_chunk(bundle: &SaveBundle) -> manga_node::db::Id {
    bundle
        .objects()
        .find(|(kind, _, _)| *kind == "chunk")
        .unwrap()
        .1
}
async fn assert_no_event(f: &Fixture, event: &auth::Event) -> Result<()> {
    let count: i64 = f
        .repo
        .pool
        .get()
        .await?
        .query_one(
            "SELECT count(*) FROM events WHERE id=$1",
            &[&id(&event.id).as_slice()],
        )
        .await?
        .get(0);
    ensure!(
        count == 0,
        "rejected signed root was inserted into original events"
    );
    Ok(())
}
async fn call_owned(client: reqwest::Client, url: String, body: Vec<u8>) -> Result<(u16, Value)> {
    let authorization = post_proof(2, &url, &body);
    let response = client
        .post(url)
        .header("content-type", "application/json")
        .header("authorization", authorization)
        .body(body)
        .send()
        .await?;
    Ok((response.status().as_u16(), response.json().await?))
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires its own isolated PostgreSQL fixture and actual pinned strfry"]
async fn concurrent_staged_cas_and_late_service_or_account_changes_never_publish_rejected_roots()
-> Result<()> {
    let f = Fixture::start().await?;
    let result = late_fences(&f).await;
    f.shutdown().await?;
    result
}
async fn late_fences(f: &Fixture) -> Result<()> {
    let s=publish(&f.runtime,1002,cbor(json!({"type":"series","version":1,"titles":{"en":"Late-fence series"},"contentRating":"safe"})),BTreeMap::new()).await?;
    let image = png();
    let ch=publish(&f.runtime,1001,cbor(json!({"type":"chapter","version":1,"series":s.id,"language":"en","chapter":"1","pages":[page(&image)]})),BTreeMap::from([(auth::hash(&image),image)])).await?;
    import(f).await?;
    let resolver = PostgresPublicIdResolver::new(f.state.db.clone());
    let db = f
        .state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let native: String = db
        .query_one(
            "SELECT id FROM platform.catalog_manga WHERE protocol_series_id=$1",
            &[&s.id],
        )
        .await?
        .get(0);
    let chapter_native: String = db
        .query_one(
            "SELECT id FROM platform.catalog_chapters WHERE protocol_event_id=$1",
            &[&ch.id],
        )
        .await?
        .get(0);
    drop(db);
    let manga = resolver
        .preferred(ResourceKind::Manga, &native)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    let ca = resolver
        .preferred(ResourceKind::Chapter, &chapter_native)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    let url = format!("{}/api/v1/manga/{manga}/read", f.origin);
    let time = auth::now() - 30;
    let old = bundle(2, time, vec![chapter(&ch, false, Some(1))])?;
    save(&f.c, &f.native_origin, 2, &old).await?;
    let a = bundle(2, time + 1, vec![chapter(&ch, true, Some(1))])?;
    let b = bundle(
        2,
        time + 2,
        vec![Record::Chapter {
            id: id(&ch.id),
            completed: true,
            page: Some(1),
            updated_at: 2,
        }],
    )?;
    stage_only(f, &a).await?;
    stage_only(f, &b).await?;
    let mut blocker = f.repo.pool.get().await?;
    let tx = blocker.transaction().await?;
    let hashes = vec![changed_chunk(&a).to_vec(), changed_chunk(&b).to_vec()];
    tx.query(
        "SELECT hash FROM blob_intents WHERE hash=ANY($1) ORDER BY hash FOR UPDATE",
        &[&hashes],
    )
    .await?;
    let aa = tokio::spawn(call_owned(
        f.c.clone(),
        url.clone(),
        read_body(
            ca.as_str(),
            true,
            extension(Some(&old), &a, Some(&old.event().id)),
        ),
    ));
    let bb = tokio::spawn(call_owned(
        f.c.clone(),
        url.clone(),
        read_body(
            ca.as_str(),
            true,
            extension(Some(&old), &b, Some(&old.event().id)),
        ),
    ));
    // Both requests passed snapshot/intent validation and reached original private
    // durable receipt fences before either can enter its final author CAS transaction.
    wait_blocked(f, "%FROM blob_intents WHERE hash=$1 FOR UPDATE%", 2).await?;
    tx.rollback().await?;
    drop(blocker);
    let aa = aa.await??;
    let bb = bb.await??;
    let (winner, rejected) = if matches!(aa.0, 200 | 202) && bb.0 == 409 {
        (&a, &b)
    } else if matches!(bb.0, 200 | 202) && aa.0 == 409 {
        (&b, &a)
    } else {
        anyhow::bail!("strict concurrent CAS receipts:{aa:?},{bb:?}")
    };
    ensure!(head(f, 2).await? == Some(winner.event().id.clone()));
    assert_no_event(f, rejected.event()).await?;
    let holds = hold_count(f, 2).await?;
    ensure!(holds == 2);
    // Rotate only the selected service after proof and intent verification, while
    // original staging is blocked. Final author TX must reject before retention.
    let next = bundle(
        2,
        time + 3,
        vec![Record::Chapter {
            id: id(&ch.id),
            completed: false,
            page: Some(1),
            updated_at: 3,
        }],
    )?;
    stage_only(f, &next).await?;
    let body = read_body(
        ca.as_str(),
        false,
        extension(Some(winner), &next, Some(&winner.event().id)),
    );
    let mut blocker = f.repo.pool.get().await?;
    let tx = blocker.transaction().await?;
    tx.query_one(
        "SELECT hash FROM blob_intents WHERE hash=$1 FOR UPDATE",
        &[&changed_chunk(&next).as_slice()],
    )
    .await?;
    let caller = tokio::spawn(call_owned(f.c.clone(), url.clone(), body.clone()));
    wait_blocked(f, "%FROM blob_intents WHERE hash=$1 FOR UPDATE%", 1).await?;
    f.repo
        .pool
        .get()
        .await?
        .execute(
            "UPDATE node_identity SET service_key=$1 WHERE singleton",
            &[&id(&public(12)).as_slice()],
        )
        .await?;
    tx.rollback().await?;
    drop(blocker);
    let result = caller.await??;
    ensure!(result.0 == 503, "late service rotation:{result:?}");
    ensure!(head(f, 2).await? == Some(winner.event().id.clone()));
    ensure!(hold_count(f, 2).await? == holds);
    assert_no_event(f, next.event()).await?;
    f.repo
        .pool
        .get()
        .await?
        .execute(
            "UPDATE node_identity SET service_key=$1 WHERE singleton",
            &[&id(&public(11)).as_slice()],
        )
        .await?;
    // Move from a private staging barrier to the final service fence. Revoke
    // admission only once the guarded commit TX has reached that service fence,
    // before it takes the account UPDATE lock. No earlier check can stand in.
    let mut blocker = f.repo.pool.get().await?;
    let tx = blocker.transaction().await?;
    tx.query_one(
        "SELECT hash FROM blob_intents WHERE hash=$1 FOR UPDATE",
        &[&changed_chunk(&next).as_slice()],
    )
    .await?;
    let caller = tokio::spawn(call_owned(f.c.clone(), url.clone(), body));
    wait_blocked(f, "%FROM blob_intents WHERE hash=$1 FOR UPDATE%", 1).await?;
    let mut service_blocker = f.repo.pool.get().await?;
    let service_tx = service_blocker.transaction().await?;
    service_tx
        .query_one(
            "SELECT service_key FROM node_identity WHERE singleton FOR UPDATE",
            &[],
        )
        .await?;
    tx.rollback().await?;
    drop(blocker);
    wait_blocked(
        f,
        "%SELECT service_key FROM node_identity WHERE singleton FOR SHARE%",
        1,
    )
    .await?;
    f.repo.set_account_admission(id(&public(2)), false).await?;
    service_tx.rollback().await?;
    drop(service_blocker);
    let result = caller.await??;
    ensure!(
        matches!(result.0, 403 | 503),
        "late account change:{result:?}"
    );
    ensure!(head(f, 2).await? == Some(winner.event().id.clone()));
    ensure!(hold_count(f, 2).await? == holds);
    assert_no_event(f, next.event()).await?;
    Ok(())
}
