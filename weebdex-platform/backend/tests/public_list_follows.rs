//! Real original signed lists, actual HTTP and isolated PostgreSQL membership.
#[path = "support/nostr_library.rs"]
mod fixture;
use anyhow::{Context, Result, ensure};
use fixture::*;
use manga_node::{auth, library_manifest::Record};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use weebdex_platform::{
    protocol::list_aliases,
    public_api::{PostgresPublicIdResolver, PublicIdResolver, ResourceKind},
};

fn list(n: u8, name: &str) -> Record {
    Record::List {
        id: [n; 16],
        name: name.into(),
        updated_at: 1,
    }
}
fn request_proof(key: u8, url: &str, method: &str, body: &[u8]) -> String {
    token(&sign_at(
        key,
        27235,
        vec![
            vec!["u".into(), url.into()],
            vec!["method".into(), method.into()],
            vec!["service".into(), public(11)],
            vec!["payload".into(), auth::hash(body)],
            vec!["nonce".into(), hex::encode(rand::random::<[u8; 16]>())],
        ],
        String::new(),
        auth::now(),
    ))
}
async fn call(
    f: &Fixture,
    key: Option<u8>,
    method: &str,
    path: &str,
    payload: Option<Value>,
    status: u16,
) -> Result<Value> {
    let url = format!("{}/api/v1{path}", f.origin);
    let body = payload.map_or_else(Vec::new, |p| serde_json::to_vec(&p).unwrap());
    let mut request =
        f.c.request(method.parse()?, &url)
            .header("content-type", "application/json");
    if let Some(key) = key {
        request = request.header("authorization", request_proof(key, &url, method, &body));
    }
    response(request.body(body), status).await
}
fn native_request(f: &Fixture, user: &str, method: &str, path: &str) -> reqwest::RequestBuilder {
    f.c.request(
        method.parse().unwrap(),
        format!("{}/api/v1/account{path}", f.origin),
    )
    .header(
        "cookie",
        format!("{}=list-cookie-{user}", f.state.config.session_cookie_name),
    )
    .header("x-weebdex-expected-account", user)
    .header("content-type", "application/json")
}
async fn native(
    f: &Fixture,
    user: &str,
    method: &str,
    path: &str,
    payload: Option<Value>,
    status: u16,
) -> Result<Value> {
    let mut request = native_request(f, user, method, path).header("origin", &f.origin);
    if let Some(payload) = payload {
        request = request.json(&payload);
    }
    let response = request.send().await?;
    assert_eq!(response.status().as_u16(), status);
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    let vary = response
        .headers()
        .get_all("vary")
        .iter()
        .map(|v| v.to_str().unwrap().to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(",");
    assert!(
        vary.contains("cookie")
            && vary.contains("authorization")
            && vary.contains("x-weebdex-expected-account")
    );
    let value: Value = response.json().await?;
    if status == 200 {
        assert_eq!(value["nostr"]["authentication"], "native-session");
    }
    Ok(value)
}
async fn project(f: &Fixture) -> Result<()> {
    for _ in 0..32 {
        if list_aliases::import_page(&f.state)
            .await
            .map_err(|e| anyhow::anyhow!("{e:?}"))?
            .completed_sweep
        {
            return Ok(());
        }
    }
    anyhow::bail!("bounded list projection did not complete")
}
async fn graph(
    f: &Fixture,
    key: u8,
    time: u64,
    records: Vec<Record>,
) -> Result<manga_node::library_client_bundle::SaveBundle> {
    let b = bundle(key, time, records)?;
    save(&f.c, &f.native_origin, key, &b).await?;
    drain_backend(&f.repo).await?;
    Ok(b)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires owned PG runner and verified actual strfry"]
async fn signed_public_list_follows_scope_accounts_and_track_real_visibility() -> Result<()> {
    let f = Fixture::start().await?;
    let result = exercise(&f).await;
    f.shutdown().await?;
    result
}
async fn exercise(f: &Fixture) -> Result<()> {
    let now = auth::now() - 30;
    let a = graph(
        f,
        2,
        now,
        vec![list(1, "First public"), list(2, "Second public")],
    )
    .await?;
    let b = graph(f, 3, now, vec![list(3, "Other publisher")]).await?;
    project(f).await?;
    let db = f
        .state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    for key in [2, 3] {
        let user = format!("FollowUser{key}");
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,permissions) VALUES($1,$1,$2,'synthetic-no-password',true,'[\"self.preferences.update\"]')",&[&user,&format!("follow-{key}@fixture.invalid")]).await?;
        db.execute(
            "INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",
            &[&public(key), &user],
        )
        .await?;
    }
    db.execute("INSERT INTO platform.library_lists(id,user_id,name,visibility) VALUES('PrivateList1','FollowUser2','Private native list','private')",&[]).await?;
    // This reader has no protocol admission, key or binding. Ordinary login is
    // sufficient to persist a private follow of somebody else's public list.
    db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,permissions) VALUES('NativeList4','NativeList4','native-list@fixture.invalid','synthetic-no-password',true,'[\"self.preferences.update\"]')",&[]).await?;
    for user in ["NativeList4", "FollowUser2", "FollowUser3"] {
        let hash = Sha256::digest(format!("list-cookie-{user}").as_bytes()).to_vec();
        db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at,purpose) VALUES($1,$1,$2,clock_timestamp()+interval '1 hour','normal')",&[&user,&hash]).await?;
    }

    let resolver = PostgresPublicIdResolver::new(f.state.db.clone());
    let key1 = format!("protocol-list:{}:{}", public(2), hex::encode([1; 16]));
    let key2 = format!("protocol-list:{}:{}", public(2), hex::encode([2; 16]));
    let key3 = format!("protocol-list:{}:{}", public(3), hex::encode([3; 16]));
    let mut ids = Vec::new();
    for key in [&key1, &key2, &key3] {
        ids.push(
            resolver
                .preferred(ResourceKind::List, key)
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?
                .context("real list alias")?,
        );
    }
    let owner = resolver
        .preferred(ResourceKind::User, "FollowUser2")
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    let other = resolver
        .preferred(ResourceKind::User, "FollowUser3")
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    let private = resolver
        .preferred(ResourceKind::List, "PrivateList1")
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    let native_owner = resolver
        .preferred(ResourceKind::User, "NativeList4")
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    let alias = "aaaaaaaa-bbbb-8ccc-8ddd-eeeeeeeeeeee";
    db.query_one(
        "SELECT platform.public_register_id('list',$1,$2::text::uuid,'legacy','protocol_list')",
        &[&key1, &alias],
    )
    .await?;
    let aliases_before: i64 = db
        .query_one("SELECT count(*) FROM platform.public_resource_ids", &[])
        .await?
        .get(0);
    drop(db);
    let follows = "/user/follows/list";
    let first = format!("/list/{}/follow", ids[0]);
    let first_status = format!("{follows}/{}", ids[0]);
    let initial = native(f, "NativeList4", "GET", &first_status, None, 200).await?;
    assert_eq!(initial["nostr"]["followed"], false);
    assert_eq!(initial["nostr"]["version"], 0);
    assert_eq!(initial["nostr"]["head"], a.event().id);
    let command = json!({"version":0,"expectedAccountId":native_owner,"expectedHead":a.event().id});
    assert_eq!(
        native_request(f, "NativeList4", "POST", &first)
            .json(&command)
            .send()
            .await?
            .status(),
        403,
        "CSRF origin required"
    );
    assert_eq!(
        native_request(f, "NativeList4", "POST", &first)
            .header("origin", "https://evil.invalid")
            .json(&command)
            .send()
            .await?
            .status(),
        403
    );
    assert_eq!(
        native_request(f, "NativeList4", "GET", follows)
            .header("authorization", "Nostr invalid")
            .send()
            .await?
            .status(),
        403,
        "No mixed-auth fallback"
    );
    native(
        f,
        "NativeList4",
        "POST",
        &first,
        Some(json!({"version":0,"expectedAccountId":native_owner})),
        400,
    )
    .await?;
    native(
        f,
        "NativeList4",
        "POST",
        &first,
        Some(json!({"version":0,"expectedAccountId":other,"expectedHead":a.event().id})),
        409,
    )
    .await?;
    native(
        f,
        "NativeList4",
        "POST",
        &format!("/list/{private}/follow"),
        Some(command.clone()),
        404,
    )
    .await?;
    let added = native(
        f,
        "NativeList4",
        "POST",
        &format!("/list/{alias}/follow"),
        Some(command.clone()),
        200,
    )
    .await?;
    assert_eq!(added["nostr"]["listId"], ids[0].as_str());
    assert_eq!(added["nostr"]["version"], 1);
    assert_eq!(added["nostr"]["accountId"], native_owner.as_str());
    native(f, "NativeList4", "POST", &first, Some(command), 409).await?;
    assert_eq!(
        native(f, "NativeList4", "GET", follows, None, 200).await?["total"],
        1
    );
    assert_eq!(
        native(f, "FollowUser3", "GET", follows, None, 200).await?["total"],
        0
    );
    let db = f
        .state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    assert_eq!(
        db.query_one(
            "SELECT count(*) FROM platform.protocol_identity_bindings WHERE user_id='NativeList4'",
            &[]
        )
        .await?
        .get::<_, i64>(0),
        0
    );
    db.execute(
        "UPDATE platform.users SET verified=false WHERE id='NativeList4'",
        &[],
    )
    .await?;
    native(f, "NativeList4", "GET", follows, None, 200).await?;
    // Unverified native preference changes remain allowed, with exact CAS.
    let unchanged = native(
        f,
        "NativeList4",
        "POST",
        &first,
        Some(json!({"version":1,"expectedAccountId":native_owner,"expectedHead":a.event().id})),
        200,
    )
    .await?;
    assert_eq!(unchanged["nostr"]["changed"], false);
    db.execute(
        "UPDATE platform.users SET verified=true,permissions='[]' WHERE id='NativeList4'",
        &[],
    )
    .await?;
    native(
        f,
        "NativeList4",
        "DELETE",
        &first,
        Some(json!({"version":1,"expectedAccountId":native_owner})),
        403,
    )
    .await?;
    db.execute("UPDATE platform.users SET permissions='[\"self.preferences.update\"]' WHERE id='NativeList4'",&[]).await?;
    drop(db);
    call(f, None, "GET", follows, None, 401).await?;
    call(f, Some(4), "GET", follows, None, 403).await?;
    call(f, Some(2), "GET", &first_status, None, 404).await?;
    assert_eq!(
        call(f, Some(2), "GET", follows, None, 200).await?["total"],
        0
    );
    call(
        f,
        Some(2),
        "POST",
        &format!("/list/{private}/follow"),
        None,
        404,
    )
    .await?;
    call(
        f,
        Some(2),
        "GET",
        &format!("{follows}/{private}"),
        None,
        404,
    )
    .await?;
    call(
        f,
        Some(2),
        "POST",
        &format!("/list/{owner}/follow"),
        None,
        404,
    )
    .await?;
    call(
        f,
        Some(2),
        "POST",
        &first,
        Some(json!({"version":0,"expectedAccountId":other})),
        409,
    )
    .await?;
    let created = call(
        f,
        Some(2),
        "POST",
        &format!("/list/{alias}/follow"),
        Some(json!({"version":0,"expectedAccountId":owner,"expectedHead":a.event().id})),
        200,
    )
    .await?;
    assert_eq!(created["nostr"]["version"], 1);
    assert_eq!(created["nostr"]["listId"], ids[0].as_str());
    assert_eq!(created["nostr"]["accountId"], owner.as_str());
    let status = call(f, Some(2), "GET", &first_status, None, 200).await?;
    assert_eq!(status["nostr"]["version"], 1);
    let duplicate = call(f, Some(2), "POST", &first, None, 200).await?;
    assert_eq!(duplicate["nostr"]["changed"], false);
    assert_eq!(duplicate["nostr"]["version"], 1);
    assert_eq!(
        call(f, Some(2), "GET", &first_status, None, 200).await?,
        status,
        "idempotent follow retains real clocks"
    );
    call(f, Some(2), "POST", &first, Some(json!({"version":0})), 409).await?;
    call(f, Some(3), "GET", &first_status, None, 404).await?;
    assert_eq!(
        call(f, Some(3), "GET", follows, None, 200).await?["total"],
        0
    );
    call(
        f,
        Some(2),
        "POST",
        &format!("/list/{}/follow", ids[1]),
        None,
        200,
    )
    .await?;
    call(
        f,
        Some(2),
        "POST",
        &format!("/list/{}/follow", ids[2]),
        None,
        200,
    )
    .await?;
    let same = native(f, "FollowUser2", "GET", follows, None, 200).await?;
    assert_eq!(same["total"], 3);
    assert_eq!(same["nostr"]["accountId"], owner.as_str());
    let all = call(f, Some(2), "GET", follows, None, 200).await?;
    assert_eq!(all["total"], 3);
    assert_eq!(all["nostr"]["accountId"], owner.as_str());
    assert_eq!(all["data"][0]["id"], ids[2].as_str());
    assert_ne!(
        all["data"][2]["relationships"][0]["id"],
        owner.as_str(),
        "native follower UUID is not a protocol publisher UUID"
    );
    let page = call(
        f,
        Some(2),
        "GET",
        &format!("{follows}?limit=1&offset=1"),
        None,
        200,
    )
    .await?;
    assert_eq!(page["total"], 3);
    assert_eq!(page["data"][0]["id"], ids[1].as_str());
    for raw in ["limit=0", "limit=101", "includes[]=user", "head=bad"] {
        call(f, Some(2), "GET", &format!("{follows}?{raw}"), None, 400).await?;
    }
    call(
        f,
        Some(2),
        "POST",
        &first,
        Some(json!({"private":true})),
        400,
    )
    .await?;
    call(
        f,
        Some(2),
        "DELETE",
        &first,
        Some(json!({"expectedHead":a.event().id})),
        400,
    )
    .await?;
    let url = format!("{}/api/v1{first}", f.origin);
    let authorization = request_proof(2, &url, "POST", b"");
    response(f.c.post(&url).header("authorization", &authorization), 200).await?;
    response(f.c.post(&url).header("authorization", &authorization), 401).await?;
    // Names change through a new genuine signed graph, never local follow state.
    let renamed = graph(
        f,
        2,
        now + 1,
        vec![list(1, "Renamed public"), list(2, "Second public")],
    )
    .await?;
    call(f, Some(2), "GET", follows, None, 503).await?;
    project(f).await?;
    let current = call(f, Some(2), "GET", follows, None, 200).await?;
    assert_eq!(
        current["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == ids[0].as_str())
            .unwrap()["attributes"]["name"],
        "Renamed public"
    );
    call(
        f,
        Some(2),
        "POST",
        &first,
        Some(json!({"expectedHead":a.event().id})),
        409,
    )
    .await?;
    call(
        f,
        Some(2),
        "POST",
        &first,
        Some(json!({"expectedHead":renamed.event().id})),
        200,
    )
    .await?;
    graph(f, 2, now + 2, vec![list(2, "Second public")]).await?;
    project(f).await?;
    call(f, Some(2), "GET", &first_status, None, 404).await?;
    let withdrawn_native = native(f, "NativeList4", "GET", &first_status, None, 200).await?;
    assert_eq!(withdrawn_native["nostr"]["followed"], true);
    assert_eq!(withdrawn_native["nostr"]["version"], 1);
    assert!(withdrawn_native["nostr"]["head"].is_null());
    assert_eq!(
        native(f, "NativeList4", "GET", follows, None, 200).await?["nostr"]["unavailableFollowCount"],
        1
    );
    native(
        f,
        "NativeList4",
        "DELETE",
        &first,
        Some(json!({"version":1,"expectedAccountId":native_owner})),
        200,
    )
    .await?;
    let tombstone = native(f, "NativeList4", "GET", &first_status, None, 200).await?;
    assert_eq!(tombstone["nostr"]["followed"], false);
    assert_eq!(tombstone["nostr"]["version"], 2);
    assert!(tombstone["nostr"].get("followedAt").is_none());
    let withdrawn = call(f, Some(2), "GET", follows, None, 200).await?;
    assert_eq!(withdrawn["total"], 2);
    assert_eq!(withdrawn["nostr"]["unavailableFollowCount"], 1);
    // Source withdrawal hides metadata but retains private membership, and an
    // explicit unfollow can still remove it without requiring a visible source.
    let removed = call(
        f,
        Some(2),
        "DELETE",
        &first,
        Some(json!({"version":1,"expectedAccountId":owner})),
        200,
    )
    .await?;
    assert_eq!(removed["nostr"]["version"], 2);
    assert_eq!(
        call(f, Some(2), "DELETE", &first, None, 200).await?["nostr"]["changed"],
        false
    );
    graph(
        f,
        2,
        now + 3,
        vec![list(1, "Returned public"), list(2, "Second public")],
    )
    .await?;
    project(f).await?;
    call(f, Some(2), "POST", &first, Some(json!({"version":0})), 409).await?;
    call(f, Some(2), "POST", &first, Some(json!({"version":2})), 200).await?;
    // Original signed deletion is authoritative even while alias rows remain.
    let deletion = auth::sign(
        &format!("{:064x}", 3),
        5,
        vec![vec!["e".into(), b.event().id.clone()]],
        "fixture list deletion".into(),
    )
    .unwrap();
    let permit = f.repo.account_permit(id(&public(3))).await?;
    assert_eq!(f.repo.delete_ordinary(&permit, &deletion).await?, 1);
    call(
        f,
        Some(2),
        "GET",
        &format!("{follows}/{}", ids[2]),
        None,
        404,
    )
    .await?;
    let deleted = call(f, Some(2), "GET", follows, None, 200).await?;
    assert_eq!(deleted["total"], 2);
    assert_eq!(deleted["nostr"]["unavailableFollowCount"], 1);
    ensure!(
        !deleted.to_string().contains("Other publisher"),
        "deleted source metadata leaked"
    );
    call(
        f,
        Some(2),
        "DELETE",
        &format!("/list/{}/follow", ids[2]),
        None,
        200,
    )
    .await?;
    let count: i64 = f
        .state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .query_one("SELECT count(*) FROM platform.public_resource_ids", &[])
        .await?
        .get(0);
    assert_eq!(
        count, aliases_before,
        "follow/read operations never manufacture source aliases"
    );
    f.repo.set_account_admission(id(&public(2)), false).await?;
    call(f, Some(2), "GET", follows, None, 403).await?;
    call(f, Some(2), "DELETE", &first, None, 403).await?;
    f.repo.set_account_admission(id(&public(2)), true).await?;
    f.state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .execute(
            "UPDATE platform.users SET permissions='[]' WHERE id='FollowUser2'",
            &[],
        )
        .await?;
    call(f, Some(2), "DELETE", &first, None, 403).await?;
    // CAS serializes two ordinary requests without any source key or binding.
    let observation = native(f, "NativeList4", "GET", &first_status, None, 200).await?;
    let command = json!({"expectedAccountId":native_owner,"version":observation["nostr"]["version"],"expectedHead":observation["nostr"]["head"]});
    let one = native_request(f, "NativeList4", "POST", &first)
        .header("origin", &f.origin)
        .json(&command)
        .send();
    let two = native_request(f, "NativeList4", "POST", &first)
        .header("origin", &f.origin)
        .json(&command)
        .send();
    let (one, two) = tokio::join!(one, two);
    let mut statuses = [one?.status().as_u16(), two?.status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, [200, 409]);
    f.state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .execute(
            "UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE id='NativeList4'",
            &[],
        )
        .await?;
    native(f, "NativeList4", "GET", follows, None, 401).await?;
    native(
        f,
        "NativeList4",
        "DELETE",
        &first,
        Some(json!({"expectedAccountId":native_owner,"version":3})),
        401,
    )
    .await?;
    Ok(())
}
