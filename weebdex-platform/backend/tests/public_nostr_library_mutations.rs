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
