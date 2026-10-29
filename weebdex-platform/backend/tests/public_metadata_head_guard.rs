//! Actual owned protocol HTTP + PostgreSQL conflict/readback tests, not browser UI.
#[path = "support/nostr_library.rs"]
mod fixture;
use anyhow::{Result, ensure};
use fixture::*;
use manga_node::auth;
use serde_json::{Value, json};
async fn send(f: &Fixture, key: u8, body: &Value) -> Result<(u16, Value)> {
    let url = format!("{}/compat/mangadex/nostr/account-metadata", f.native_origin);
    let bytes = serde_json::to_vec(body)?;
    let response =
        f.c.post(&url)
            .header("authorization", http(key, &url, &bytes))
            .body(bytes)
            .send()
            .await?;
    Ok((response.status().as_u16(), response.json().await?))
}
fn guarded(event: &auth::Event, head: Option<&str>) -> Value {
    let mut body = serde_json::to_value(event).unwrap();
    body["expectedHead"] = json!(head);
    body
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires run-auth-tests.sh disposable PostgreSQL and actual strfry"]
async fn metadata_http_atomic_head_guard_preserves_rich_profile_and_rejects_concurrent_loss()
-> Result<()> {
    let f = Fixture::start().await?;
    let result = exercise(&f).await;
    f.shutdown().await?;
    result
}
async fn exercise(f: &Fixture) -> Result<()> {
    let at = auth::now() - 30;
    let rich = json!({"name":"First","display_name":"Reader","picture":"https://example.invalid/p","banner":"https://example.invalid/b","nip05":"reader@example.invalid","lud16":"light@example.invalid","custom":{"nested":[1,true]}});
    let first = sign_at(
        2,
        0,
        vec![vec!["custom".into(), "preserved".into()]],
        rich.to_string(),
        at,
    );
    let missing_url = format!("{}/accounts/{}/metadata/0", f.native_origin, public(2));
    let missing = f.c.get(&missing_url).send().await?;
    assert_eq!(missing.status(), 404);
    assert_eq!(
        missing.json::<Value>().await?["expectedHeadSupported"],
        true
    );
    let (status, _) = send(f, 2, &guarded(&first, None)).await?;
    assert!([200, 202].contains(&status));
    let mut edited = rich.clone();
    edited["name"] = json!("Edited");
    let second = sign_at(2, 0, first.tags.clone(), edited.to_string(), at + 1);
    let (status, _) = send(f, 2, &guarded(&second, Some(&first.id))).await?;
    assert!([200, 202].contains(&status));
    drain_backend(&f.repo).await?;
    let head = f.c.get(&missing_url).send().await?.json::<Value>().await?;
    assert_eq!(head["id"], second.id);
    assert_eq!(head["expectedHeadSupported"], true);
    let read =
        f.c.get(format!("{}/events/{}", f.native_origin, second.id))
            .send()
            .await?
            .json::<auth::Event>()
            .await?;
    read.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(serde_json::from_str::<Value>(&read.content)?, edited);
    assert_eq!(read.tags, first.tags);
    // Null is a real absent-head expectation, distinct from omitted legacy field.
    let stale = sign_at(
        2,
        0,
        vec![],
        "{\"name\":\"would lose rich fields\"}".into(),
        at + 2,
    );
    for previous in [None, Some(first.id.as_str())] {
        let (status, receipt) = send(f, 2, &guarded(&stale, previous)).await?;
        assert_eq!(status, 409);
        assert_eq!(receipt["headChanged"], true);
        assert_eq!(receipt["id"], stale.id);
    }
    let absent: i64 = f
        .repo
        .pool
        .get()
        .await?
        .query_one(
            "SELECT count(*) FROM events WHERE id=$1",
            &[&id(&stale.id).as_slice()],
        )
        .await?
        .get(0);
    assert_eq!(absent, 0);
    // Already accepted exact event is idempotent even with its original old expectation.
    let (status, receipt) = send(f, 2, &guarded(&second, Some(&first.id))).await?;
    assert_eq!(status, 200);
    assert_eq!(receipt["inserted"], false);
    // Two concurrent reviewed updates can no longer both commit, even with newer timestamps.
    let left = sign_at(2, 0, vec![], json!({"name":"Left"}).to_string(), at + 3);
    let right = sign_at(2, 0, vec![], json!({"name":"Right"}).to_string(), at + 4);
    let left_body = guarded(&left, Some(&second.id));
    let right_body = guarded(&right, Some(&second.id));
    let (a, b) = tokio::join!(send(f, 2, &left_body), send(f, 2, &right_body));
    let a = a?;
    let b = b?;
    assert_eq!([a.0, b.0].iter().filter(|&&s| s == 409).count(), 1);
    assert_eq!(
        [a.0, b.0].iter().filter(|&&s| s == 200 || s == 202).count(),
        1
    );
    let winner = if a.0 == 409 { &right } else { &left };
    let loser = if a.0 == 409 { &left } else { &right };
    drain_backend(&f.repo).await?;
    assert_eq!(
        f.c.get(&missing_url).send().await?.json::<Value>().await?["id"],
        winner.id
    );
    let counts=f.repo.pool.get().await?.query_one("SELECT (SELECT count(*) FROM events WHERE id=$1),(SELECT count(*) FROM events WHERE id=$2)",&[&id(&winner.id).as_slice(),&id(&loser.id).as_slice()]).await?;
    assert_eq!(counts.get::<_, i64>(0), 1);
    assert_eq!(counts.get::<_, i64>(1), 0);
    // Guard cannot bypass admission, author binding, body proof, or malformed presence.
    let forbidden = sign_at(5, 0, vec![], "{}".into(), at + 10);
    assert_eq!(send(f, 5, &guarded(&forbidden, None)).await?.0, 403);
    let mut malformed = guarded(&stale, None);
    malformed["expectedHead"] = json!({"id":winner.id});
    assert_eq!(send(f, 2, &malformed).await?.0, 400);
    let wrong_author = sign_at(3, 0, vec![], "{}".into(), at + 10);
    assert_eq!(send(f, 2, &guarded(&wrong_author, None)).await?.0, 403);
    let url = format!("{}/compat/mangadex/nostr/account-metadata", f.native_origin);
    let mut changed = guarded(&stale, Some(&winner.id));
    let original = serde_json::to_vec(&changed)?;
    changed["expectedHead"] = Value::Null;
    let response =
        f.c.post(&url)
            .header("authorization", http(2, &url, &original))
            .body(serde_json::to_vec(&changed)?)
            .send()
            .await?;
    ensure!(
        response.status() == 403,
        "body hash guard {}",
        response.status()
    );
    // Legacy omission remains protocol-compatible, never a client fallback.
    let legacy = sign_at(3, 0, vec![], "{}".into(), at + 20);
    assert!([200, 202].contains(&send(f, 3, &serde_json::to_value(&legacy)?).await?.0));
    Ok(())
}
