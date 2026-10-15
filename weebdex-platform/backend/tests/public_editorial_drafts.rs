//! Fresh signed-HTTP regression for restored local editorial writes and private drafts.
//! Actual TCP, original request authorization and private instance moderation.
//! Every key/account below is synthetic and belongs only to this disposable DB.
use base64::{Engine, engine::general_purpose::STANDARD};
use manga_node::{
    auth::{self, Event},
    db::{DbConfig, Id, Repository},
};
use secp256k1::{Keypair, Message, Secp256k1, SecretKey};
use serde_json::{Value, json};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use weebdex_platform::{AppState, Config, domains, protocol::publication::OperatorRuntimeConfig};
fn public(key: u8) -> String {
    auth::sign(&format!("{key:064x}"), 1, vec![], String::new())
        .unwrap()
        .pubkey
}
fn id(key: u8) -> Id {
    hex::decode(public(key)).unwrap().try_into().unwrap()
}
fn proof_for(key: u8, service: u8, url: &str, method: &str, body: &[u8]) -> String {
    static NONCE: AtomicUsize = AtomicUsize::new(0);
    let secret = SecretKey::from_slice(&hex::decode(format!("{key:064x}")).unwrap()).unwrap();
    let secp = Secp256k1::new();
    let pair = Keypair::from_secret_key(&secp, &secret);
    let mut event = Event {
        id: String::new(),
        pubkey: public(key),
        created_at: auth::now(),
        kind: 27235,
        tags: vec![
            vec!["u".into(), url.into()],
            vec!["method".into(), method.into()],
            vec!["service".into(), public(service)],
            vec!["payload".into(), auth::hash(body)],
            vec![
                "nonce".into(),
                NONCE.fetch_add(1, Ordering::Relaxed).to_string(),
            ],
        ],
        content: String::new(),
        sig: String::new(),
    };
    let digest = event.digest();
    event.id = hex::encode(&digest);
    event.sig = secp
        .sign_schnorr_no_aux_rand(&Message::from_digest_slice(&digest).unwrap(), &pair)
        .to_string();
    format!(
        "Nostr {}",
        STANDARD.encode(serde_json::to_vec(&event).unwrap())
    )
}
fn proof(key: u8, url: &str, method: &str, body: &[u8]) -> String {
    proof_for(key, 11, url, method, body)
}
async fn call(
    client: &reqwest::Client,
    origin: &str,
    method: &str,
    path: &str,
    payload: Option<Value>,
    key: Option<u8>,
    expected: u16,
) -> Value {
    let url = format!("{origin}{path}");
    let body = payload.map_or_else(Vec::new, |value| serde_json::to_vec(&value).unwrap());
    let mut request = client
        .request(method.parse().unwrap(), &url)
        .header("content-type", "application/json");
    if let Some(key) = key {
        request = request.header("authorization", proof(key, &url, method, &body));
    }
    let response = request.body(body).send().await.unwrap();
    let status = response.status().as_u16();
    assert!(
        response.headers()["cache-control"]
            .to_str()
            .unwrap()
            .contains("no-store")
    );
    let value: Value = response.json().await.unwrap();
    assert_eq!(status, expected, "{method} {path}: {value}");
    value
}
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires scripts/run-auth-tests.sh and its owned isolated PostgreSQL"]
async fn editorial_and_drafts_require_real_current_bound_authority_and_atomic_audit() {
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use isolated runner");
    let pg: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
    assert!(!pg.get_hosts().is_empty());
    assert!(
        pg.get_hosts().iter().all(
            |host| matches!(host,tokio_postgres::config::Host::Tcp(host) if host=="127.0.0.1")
        )
    );
    assert!(
        std::env::var("PLATFORM_TEST_RUNTIME_DIR")
            .unwrap()
            .contains("/weebdex-platform/test-runtime/")
    );
    let repository = Repository::connect(DbConfig {
        dsn: dsn.clone(),
        connections: 2,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await
    .unwrap();
    repository.migrate().await.unwrap();
    repository.bind_instance(id(11), id(1)).await.unwrap();
    for key in 2..=10 {
        repository
            .set_account_admission(id(key), true)
            .await
            .unwrap();
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:6,
        protocol_publication:Some(OperatorRuntimeConfig{operator_key_file:"fixture-unused/operator.key".into(),expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),storage:json!({"path":"fixture-unused/storage","volume":"editorialfixture","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144}),group_keys_dir:None,backend:None,native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":"fixture-unused/service.key","admin_pubkey":public(1)}))}),..Config::default()},dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();
    state.db.verify_schema(domains::MIGRATIONS).await.unwrap();
    state.protocol.verify_schema().await.unwrap();
    let permissions = json!([
        "author.create",
        "author.update",
        "author.delete",
        "scanlation_group.create",
        "scanlation_group.update",
        "scanlation_group.delete",
        "self.scanlation_group.update",
        "manga.create",
        "manga.state.update"
    ]);
    let sql = state.db.client().await.unwrap();
    for (key, roles, verified, disabled) in [
        (2, json!(["user"]), true, false),
        (4, json!(["user"]), true, false),
        (5, json!(["moderator"]), true, false),
        (6, json!(["banned"]), true, false),
        (7, json!(["restricted"]), true, false),
        (8, json!(["user"]), true, true),
        (9, json!(["user"]), false, false),
        (10, json!(["user"]), true, false),
    ] {
        let user = format!("EditorialUser{key}");
        let perms = if key == 10 {
            json!([])
        } else {
            permissions.clone()
        };
        sql.execute("INSERT INTO platform.users(id,username,email,password_hash,roles,permissions,verified,disabled) VALUES($1,$1,$2,'synthetic-no-password',$3,$4,$5,$6)",&[&user,&format!("editorial-{key}@fixture.invalid"),&roles,&perms,&verified,&disabled]).await.unwrap();
        sql.execute(
            "INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",
            &[&public(key), &user],
        )
        .await
        .unwrap();
    }
    for (native, owner, title, draft_state) in [
        ("EditorialDraft1", "EditorialUser2", "Alpha", "draft"),
        ("EditorialDraft2", "EditorialUser2", "Beta", "submitted"),
        ("EditorialDraft3", "EditorialUser2", "Gamma", "rejected"),
        ("EditorialDraft4", "EditorialUser4", "Other", "draft"),
        (
            "EditorialPublished",
            "EditorialUser2",
            "Public",
            "published",
        ),
    ] {
        sql.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES($1,$2,$2,$3,'en','ongoing','safe',$4)",&[&native,&owner,&title,&draft_state]).await.unwrap();
    }
    drop(sql);
    use weebdex_platform::public_api::{
        PostgresPublicIdResolver, PublicIdResolver, PublicUuid, ResourceKind,
    };
    let resolver = PostgresPublicIdResolver::new(state.db.clone());
    let owner = resolver
        .preferred(ResourceKind::User, "EditorialUser2")
        .await
        .unwrap()
        .unwrap();
    let member = resolver
        .preferred(ResourceKind::User, "EditorialUser4")
        .await
        .unwrap()
        .unwrap();
    let draft = resolver
        .preferred(ResourceKind::Manga, "EditorialDraft1")
        .await
        .unwrap()
        .unwrap();
    let other = resolver
        .preferred(ResourceKind::Manga, "EditorialDraft4")
        .await
        .unwrap()
        .unwrap();
    let published = resolver
        .preferred(ResourceKind::Manga, "EditorialPublished")
        .await
        .unwrap()
        .unwrap();
    let publisher: String = state
        .db
        .client()
        .await
        .unwrap()
        .query_one(
            "SELECT platform.public_register_protocol_publisher($1)::text",
            &[&public(3)],
        )
        .await
        .unwrap()
        .get(0);
    let app = weebdex_platform::router::router(state.clone());
    let _server = Server(tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    }));
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let payload = json!({"name":"Author","biography":{"en":"A biography","ja":"漫画家"},"twitter":"https://twitter.com/author","fanBox":null});
    call(
        &client,
        &origin,
        "POST",
        "/api/v1/author",
        Some(payload.clone()),
        None,
        401,
    )
    .await;
    for key in [3, 6, 7, 8, 9, 10] {
        call(
            &client,
            &origin,
            "POST",
            "/api/v1/author",
            Some(payload.clone()),
            Some(key),
            403,
        )
        .await;
    }
    let url = format!("{origin}/api/v1/author");
    let raw = serde_json::to_vec(&payload).unwrap();
    for authorization in [
        proof(2, &url, "POST", b"{}"),
        proof_for(2, 12, &url, "POST", &raw),
        proof(2, &url, "PUT", &raw),
    ] {
        assert_eq!(
            client
                .post(&url)
                .header("content-type", "application/json")
                .header("authorization", authorization)
                .body(raw.clone())
                .send()
                .await
                .unwrap()
                .status()
                .as_u16(),
            401
        );
    }
    let replay = proof(2, &url, "POST", &raw);
    let response = client
        .post(&url)
        .header("content-type", "application/json")
        .header("authorization", replay.clone())
        .body(raw.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let author: Value = response.json().await.unwrap();
    assert_eq!(
        client
            .post(&url)
            .header("content-type", "application/json")
            .header("authorization", replay)
            .body(raw)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        401
    );
    let aid = author["data"]["id"].as_str().unwrap();
    PublicUuid::parse(aid).unwrap();
    assert_eq!(author["data"]["attributes"]["fanBox"], Value::Null);
    assert_eq!(author["data"]["attributes"]["biography"]["ja"], "漫画家");
    let apath = format!("/api/v1/author/{aid}");
    let updated = call(
        &client,
        &origin,
        "PUT",
        &apath,
        Some(json!({"version":1,"biography":{},"twitter":null})),
        Some(2),
        200,
    )
    .await;
    assert_eq!(updated["data"]["attributes"]["version"], 2);
    assert_eq!(updated["data"]["attributes"]["twitter"], Value::Null);
    call(
        &client,
        &origin,
        "PUT",
        &apath,
        Some(json!({"version":1,"name":"Stale"})),
        Some(2),
        409,
    )
    .await;
    for invalid in [
        json!({"version":2,"fanbox":null}),
        json!({"version":2,"twitter":"https://twitter.com.evil/a"}),
        json!({"version":2,"locked":true}),
        json!({"version":2,"biography":null}),
    ] {
        call(&client, &origin, "PUT", &apath, Some(invalid), Some(2), 400).await;
    }
    let concurrent_url = format!("{origin}{apath}");
    let edit = json!({"version":2,"name":"Concurrent"});
    let raw = serde_json::to_vec(&edit).unwrap();
    let a = client
        .put(&concurrent_url)
        .header("content-type", "application/json")
        .header("authorization", proof(2, &concurrent_url, "PUT", &raw))
        .body(raw.clone())
        .send();
    let b = client
        .put(&concurrent_url)
        .header("content-type", "application/json")
        .header("authorization", proof(4, &concurrent_url, "PUT", &raw))
        .body(raw)
        .send();
    let (a, b) = tokio::join!(a, b);
    let mut statuses = [a.unwrap().status().as_u16(), b.unwrap().status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, [200, 409]);
    // A failed audit insert must roll back the actual catalogue mutation and
    // generated UUID, not merely return an error after committing the entity.
    let sql = state.db.client().await.unwrap();
    sql.batch_execute("CREATE FUNCTION platform.reject_editorial_fixture_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.entity_type='author' AND NEW.operation='create' THEN RAISE EXCEPTION 'synthetic audit failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_editorial_fixture_audit BEFORE INSERT ON platform.catalog_audit FOR EACH ROW EXECUTE FUNCTION platform.reject_editorial_fixture_audit()").await.unwrap();
    drop(sql);
    call(
        &client,
        &origin,
        "POST",
        "/api/v1/author",
        Some(json!({"name":"Rollback author"})),
        Some(2),
        503,
    )
    .await;
    let sql = state.db.client().await.unwrap();
    assert_eq!(
        sql.query_one(
            "SELECT count(*) FROM platform.catalog_authors WHERE name='Rollback author'",
            &[]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        0
    );
    sql.batch_execute("DROP TRIGGER reject_editorial_fixture_audit ON platform.catalog_audit; DROP FUNCTION platform.reject_editorial_fixture_audit()").await.unwrap();
    drop(sql);
    let group=call(&client,&origin,"POST","/api/v1/group",Some(json!({"name":"Group","description":null,"website":null,"ircServer":"irc.fixture.invalid","publishDelay":"P1DT2H","inactive":true})),Some(2),200).await;
    let gid = group["data"]["id"].as_str().unwrap();
    let gpath = format!("/api/v1/group/{gid}");
    assert_eq!(group["data"]["relationships"][0]["type"], "leader");
    assert_eq!(group["data"]["relationships"][0]["id"], owner.as_str());
    assert_eq!(group["data"]["attributes"]["inactive"], true);
    call(
        &client,
        &origin,
        "PUT",
        &gpath,
        Some(json!({"version":1,"description":"Other member write"})),
        Some(4),
        403,
    )
    .await;
    call(
        &client,
        &origin,
        "PUT",
        &gpath,
        Some(json!({"version":1,"name":"Leader rename"})),
        Some(2),
        403,
    )
    .await;
    for value in [
        json!({"version":1,"members":[publisher]}),
        json!({"version":1,"members":[aid]}),
        json!({"version":1,"leader":"not-uuid"}),
        json!({"version":1,"publishDelay":null}),
    ] {
        call(&client, &origin, "PUT", &gpath, Some(value), Some(2), 400).await;
    }
    let group=call(&client,&origin,"PUT",&gpath,Some(json!({"version":1,"members":[member],"focusedLanguages":["en","ja"],"description":"Leader update"})),Some(2),200).await;
    assert_eq!(group["data"]["attributes"]["version"], 2);
    assert_eq!(group["data"]["relationships"].as_array().unwrap().len(), 2);
    let group = call(
        &client,
        &origin,
        "PUT",
        &gpath,
        Some(json!({"version":2,"leader":member,"members":[],"locked":true})),
        Some(5),
        200,
    )
    .await;
    assert_eq!(group["data"]["attributes"]["locked"], true);
    call(
        &client,
        &origin,
        "PUT",
        &gpath,
        Some(json!({"version":3,"description":"Locked leader"})),
        Some(4),
        403,
    )
    .await;
    call(
        &client,
        &origin,
        "PUT",
        &gpath,
        Some(json!({"version":2,"inactive":false})),
        Some(5),
        409,
    )
    .await;
    // Draft reads never turn a session cookie or admitted publisher into native
    // owner authority; moderator role alone also does not grant permission.
    call(
        &client,
        &origin,
        "GET",
        "/api/v1/manga/draft",
        None,
        None,
        401,
    )
    .await;
    assert_eq!(
        client
            .get(format!("{origin}/api/v1/manga/draft"))
            .header("cookie", "weebdex_session=anything")
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        401
    );
    for key in [3, 6, 7, 8, 9, 10] {
        call(
            &client,
            &origin,
            "GET",
            "/api/v1/manga/draft",
            None,
            Some(key),
            403,
        )
        .await;
    }
    let drafts = call(
        &client,
        &origin,
        "GET",
        "/api/v1/manga/draft?order[title]=asc&includes[]=creator&includes[]=author&includes[]=tag",
        None,
        Some(2),
        200,
    )
    .await;
    assert_eq!(drafts["total"], 3);
    assert_eq!(drafts["data"][0]["id"], draft.as_str());
    assert_eq!(drafts["data"][0]["relationships"][0]["type"], "creator");
    assert_eq!(
        drafts["data"][0]["relationships"][0]["attributes"]["username"],
        "EditorialUser2"
    );
    assert_eq!(
        call(
            &client,
            &origin,
            "GET",
            "/api/v1/manga/draft?state=submitted&limit=1&offset=0",
            None,
            Some(2),
            200
        )
        .await["total"],
        1
    );
    assert_eq!(
        call(
            &client,
            &origin,
            "GET",
            "/api/v1/manga/draft?limit=1&offset=10001",
            None,
            Some(2),
            200
        )
        .await["total"],
        3
    );
    assert_eq!(
        call(
            &client,
            &origin,
            "GET",
            "/api/v1/manga/draft",
            None,
            Some(5),
            200
        )
        .await["total"],
        4
    );
    call(
        &client,
        &origin,
        "GET",
        &format!("/api/v1/manga/draft/{other}"),
        None,
        Some(2),
        403,
    )
    .await;
    call(
        &client,
        &origin,
        "GET",
        &format!("/api/v1/manga/draft/{other}?includes[]=creator"),
        None,
        Some(5),
        200,
    )
    .await;
    call(
        &client,
        &origin,
        "GET",
        &format!("/api/v1/manga/draft/{published}"),
        None,
        Some(2),
        404,
    )
    .await;
    call(
        &client,
        &origin,
        "GET",
        &format!("/api/v1/manga/{draft}"),
        None,
        None,
        404,
    )
    .await;
    for path in [
        "/api/v1/manga/draft?limit=0",
        "/api/v1/manga/draft?state=published",
        "/api/v1/manga/draft?order[rating]=desc",
        "/api/v1/manga/draft?includes[]=user",
    ] {
        call(&client, &origin, "GET", path, None, Some(2), 400).await;
    }
    call(
        &client,
        &origin,
        "GET",
        &format!("/api/v1/manga/draft/{draft}?limit=0"),
        None,
        Some(2),
        400,
    )
    .await;
    // Current service and admission are rechecked for each fresh signed request.
    repository
        .set_account_admission(id(2), false)
        .await
        .unwrap();
    call(
        &client,
        &origin,
        "PUT",
        &apath,
        Some(json!({"version":3,"name":"Revoked"})),
        Some(2),
        403,
    )
    .await;
    repository.set_account_admission(id(2), true).await.unwrap();
    let sql = state.db.client().await.unwrap();
    sql.execute(
        "DELETE FROM platform.protocol_identity_bindings WHERE publisher=$1",
        &[&public(2)],
    )
    .await
    .unwrap();
    drop(sql);
    call(
        &client,
        &origin,
        "GET",
        "/api/v1/manga/draft",
        None,
        Some(2),
        403,
    )
    .await;
    call(&client, &origin, "DELETE", &apath, None, Some(4), 403).await;
    call(&client, &origin, "DELETE", &apath, None, Some(5), 200).await;
    call(&client, &origin, "DELETE", &apath, None, Some(5), 404).await;
    call(&client, &origin, "DELETE", &gpath, None, Some(4), 403).await;
    call(&client, &origin, "DELETE", &gpath, None, Some(5), 200).await;
    let sql = state.db.client().await.unwrap();
    let audits=sql.query("SELECT actor_id,operation,old_version,new_version FROM platform.catalog_audit ORDER BY id",&[]).await.unwrap();
    assert_eq!(audits.len(), 8);
    for row in audits {
        assert!(row.get::<_, String>(0).starts_with("EditorialUser"));
        if row.get::<_, String>(1) == "update" {
            assert_eq!(
                row.get::<_, Option<i32>>(3),
                row.get::<_, Option<i32>>(2).map(|v| v + 1)
            );
        }
    }
    println!(
        "Passed genuine signed HTTP author/group CRUD, typed UUID native-member mapping, current binding/account/admission policy, wrong body/method/service/replay rejection, concurrent version serialization, audit rollback, and private draft owner/moderator query scope"
    );
}
