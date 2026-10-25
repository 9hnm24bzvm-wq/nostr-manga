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
        "author.lock.update",
        "author.delete",
        "scanlation_group.create",
        "scanlation_group.update",
        "scanlation_group.delete",
        "self.scanlation_group.update",
        "manga.create",
        "manga.update",
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
        sql.execute("INSERT INTO platform.users(id,username,email,password_hash,roles,permissions,verified,disabled) VALUES($1,$1,$2,'synthetic-no-password',$3,$4,$5,$6)",&[&user,&format!("editorial-{key}@fixture.invalid"),&roles,&perms,&verified,&false]).await.unwrap();
        sql.execute(
            "INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",
            &[&public(key), &user],
        )
        .await
        .unwrap();
        sql.execute("UPDATE platform.users SET disabled=$2 WHERE id=$1",&[&user,&disabled]).await.unwrap();
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
    // Capture exact bytes for account2, then rebind the signer to account4.
    // Both accounts have colliding create/update grants: only the signed actor
    // intent, checked inside the mutation transaction, prevents silent retarget.
    let intended=json!({"name":"Captured author intent","expectedAccountId":owner.as_str()});
    let bytes=serde_json::to_vec(&intended).unwrap();let target=format!("{origin}/api/v1/author");let token=proof(2,&target,"POST",&bytes);
    let db=state.db.client().await.unwrap();db.execute("UPDATE platform.protocol_identity_bindings SET user_id='EditorialUser4' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    let response=client.post(&target).header("content-type","application/json").header("authorization",token).body(bytes).send().await.unwrap();assert_eq!(response.status().as_u16(),409);
    let count:i64=db.query_one("SELECT count(*) FROM platform.catalog_authors WHERE name='Captured author intent'",&[]).await.unwrap().get(0);assert_eq!(count,0);
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='EditorialUser2' WHERE publisher=$1",&[&public(2)]).await.unwrap();drop(db);
    let mut payload = json!({"name":"Author","biography":{"en":"A biography","ja":"漫画家"},"twitter":"https://twitter.com/author","fanBox":null});
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
    payload["expectedAccountId"]=json!(owner.as_str());
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
        json!({"version":2,"biography":null}),
    ] {
        call(&client, &origin, "PUT", &apath, Some(invalid), Some(2), 400).await;
    }
    // Lock is now supported, but this actor is not staff. A forbidden lock
    // change must preserve both the visible author and the exact audit count.
    let before_lock = call(&client, &origin, "GET", &apath, None, None, 200).await;
    let audit_before: i64 = state.db.client().await.unwrap()
        .query_one("SELECT count(*) FROM platform.catalog_audit", &[]).await.unwrap().get(0);
    call(&client, &origin, "PUT", &apath,
        Some(json!({"version":2,"locked":true})), Some(2), 403).await;
    let after_lock = call(&client, &origin, "GET", &apath, None, None, 200).await;
    assert_eq!(after_lock["data"], before_lock["data"]);
    let audit_after: i64 = state.db.client().await.unwrap()
        .query_one("SELECT count(*) FROM platform.catalog_audit", &[]).await.unwrap().get(0);
    assert_eq!(audit_after, audit_before);
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
    // Deterministic delete/reference race: the deletion request is proven to
    // be blocked on the actual author row. A reference committed by its current
    // lock holder must be observed before any author soft-delete succeeds.
    let race = call(&client,&origin,"POST","/api/v1/author",Some(json!({"name":"Concurrent referenced author"})),Some(2),200).await;
    let race_id = PublicUuid::parse(race["data"]["id"].as_str().unwrap()).unwrap();
    let native_race = resolver.resolve(ResourceKind::Author,&race_id).await.unwrap().unwrap().native_id;
    let mut locked = state.db.client().await.unwrap();
    let guard = locked.transaction().await.unwrap();
    guard.query_one("SELECT id FROM platform.catalog_authors WHERE id=$1 FOR UPDATE",&[&native_race]).await.unwrap();
    let delete_url = format!("{origin}/api/v1/author/{race_id}");
    let deletion = client.delete(&delete_url).header("authorization",proof(5,&delete_url,"DELETE",&[]));
    let deletion = tokio::spawn(async move {deletion.send().await.unwrap()});
    let observer = state.db.client().await.unwrap();
    tokio::time::timeout(Duration::from_secs(3),async {
        loop {
            let waiting:bool = observer.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE '%platform.catalog_authors%' AND pid<>pg_backend_pid())",&[]).await.unwrap().get(0);
            if waiting {break;} tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.expect("Actual HTTP deletion never reached its author-row lock");
    guard.execute("INSERT INTO platform.catalog_manga_creators(manga_id,author_id,role) VALUES('EditorialDraft1',$1,'author')",&[&native_race]).await.unwrap();
    guard.commit().await.unwrap();
    let deleted = deletion.await.unwrap();let status=deleted.status().as_u16();let receipt:Value=deleted.json().await.unwrap();
    assert_eq!(status,409,"Concurrent reference must prevent author deletion: {receipt}");
    let visible:bool=observer.query_one("SELECT deleted_at IS NULL FROM platform.catalog_authors WHERE id=$1",&[&native_race]).await.unwrap().get(0);assert!(visible);
    observer.execute("DELETE FROM platform.catalog_manga_creators WHERE author_id=$1",&[&native_race]).await.unwrap();
    drop(observer);drop(locked);
    // Reverse ordering: an author DELETE already holds its row lock while a
    // real signed title edit attempts to add that author. After deletion commits
    // the edit must reject the now-hidden author instead of relying on a stale
    // validation snapshot followed by an FK check that only proves existence.
    let raced = call(&client,&origin,"POST","/api/v1/author",Some(json!({"name":"Author deleted during title assignment"})),Some(2),200).await;
    let raced_id=PublicUuid::parse(raced["data"]["id"].as_str().unwrap()).unwrap();
    let raced_native=resolver.resolve(ResourceKind::Author,&raced_id).await.unwrap().unwrap().native_id;
    let mut barrier=state.db.client().await.unwrap();let barrier_tx=barrier.transaction().await.unwrap();
    barrier_tx.query_one("SELECT pg_advisory_xact_lock(16261017)",&[]).await.unwrap();
    let observer=state.db.client().await.unwrap();
    observer.batch_execute("CREATE FUNCTION platform.pause_editorial_author_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.name='Author deleted during title assignment' AND NEW.deleted_at IS NOT NULL THEN PERFORM pg_advisory_xact_lock(16261017); END IF; RETURN NEW; END $$; CREATE TRIGGER pause_editorial_author_delete BEFORE UPDATE ON platform.catalog_authors FOR EACH ROW EXECUTE FUNCTION platform.pause_editorial_author_delete()").await.unwrap();
    let delete_url=format!("{origin}/api/v1/author/{raced_id}");
    let deletion=client.delete(&delete_url).header("authorization",proof(5,&delete_url,"DELETE",&[]));
    let deletion=tokio::spawn(async move{deletion.send().await.unwrap()});
    tokio::time::timeout(Duration::from_secs(3),async{loop{let waiting:bool=observer.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event='advisory' AND query LIKE '%platform.catalog_authors%')",&[]).await.unwrap().get(0);if waiting{break;}tokio::time::sleep(Duration::from_millis(5)).await;}}).await.expect("Actual author DELETE did not reach its controlled row-lock barrier");
    let title_url=format!("{origin}/api/v1/manga/{draft}");let edit=json!({"version":1,"authors":[raced_id.as_str()]});let raw=serde_json::to_vec(&edit).unwrap();
    let assignment=client.put(&title_url).header("content-type","application/json").header("authorization",proof(2,&title_url,"PUT",&raw)).body(raw);
    let assignment=tokio::spawn(async move{assignment.send().await.unwrap()});
    tokio::time::timeout(Duration::from_secs(3),async{loop{let waiting:bool=observer.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND wait_event<>'advisory' AND (query LIKE '%platform.catalog_authors%' OR query LIKE '%platform.catalog_manga_creators%'))",&[]).await.unwrap().get(0);if waiting{break;}tokio::time::sleep(Duration::from_millis(5)).await;}}).await.expect("Actual signed title assignment did not reach its author lock");
    barrier_tx.commit().await.unwrap();
    assert_eq!(deletion.await.unwrap().status().as_u16(),200);
    let assigned=assignment.await.unwrap();let status=assigned.status().as_u16();let receipt:Value=assigned.json().await.unwrap();assert_eq!(status,400,"A deleted author cannot be assigned by a waiting title edit: {receipt}");
    let count:i64=observer.query_one("SELECT count(*) FROM platform.catalog_manga_creators WHERE author_id=$1",&[&raced_native]).await.unwrap().get(0);assert_eq!(count,0);
    observer.batch_execute("DROP TRIGGER pause_editorial_author_delete ON platform.catalog_authors; DROP FUNCTION platform.pause_editorial_author_delete()").await.unwrap();drop(observer);drop(barrier);
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
    // Rich editorial controls use explicit optimistic local roles and locks.
    // Officer UUIDs undergo the same native-only lookup and shared-core authority
    // as members; source publisher aliases cannot become local officers.
    let rich_group=call(&client,&origin,"POST","/api/v1/group",Some(json!({"name":"Rich local roles"})),Some(2),200).await;
    let rich_path=format!("/api/v1/group/{}",rich_group["data"]["id"].as_str().unwrap());
    call(&client,&origin,"PUT",&rich_path,Some(json!({"version":1,"officers":[publisher]})),Some(2),400).await;
    call(&client,&origin,"PUT",&rich_path,Some(json!({"version":1,"officers":[member]})),Some(4),403).await;
    let promoted=call(&client,&origin,"PUT",&rich_path,Some(json!({"version":1,"officers":[member],"members":[]})),Some(2),200).await;
    assert_eq!(promoted["data"]["attributes"]["localMembershipRoles"],"leader-officer-member-v1");
    let officer=promoted["data"]["relationships"].as_array().unwrap().iter().find(|r|r["id"]==member.as_str()).unwrap();
    assert_eq!(officer["attributes"]["isOfficer"],true);
    assert_eq!(officer["attributes"]["username"],"EditorialUser4");
    call(&client,&origin,"PUT",&rich_path,Some(json!({"version":1,"officers":[]})),Some(2),409).await;
    // Being an officer does not grant the leader-only group edit/delete action.
    call(&client,&origin,"PUT",&rich_path,Some(json!({"version":2,"description":"Officer write"})),Some(4),403).await;
    call(&client,&origin,"DELETE",&rich_path,None,Some(2),403).await;
    let moderator=resolver.preferred(ResourceKind::User,"EditorialUser5").await.unwrap().unwrap();
    let stale_delete=format!("{rich_path}?expectedAccountId={moderator}&expectedVersion=1");
    call(&client,&origin,"DELETE",&stale_delete,None,Some(5),409).await;
    let exact_delete=format!("{rich_path}?expectedAccountId={moderator}&expectedVersion=2");
    call(&client,&origin,"DELETE",&exact_delete,None,Some(5),200).await;
    let locked_author=call(&client,&origin,"POST","/api/v1/author",Some(json!({"name":"Lock control author"})),Some(2),200).await;
    let lock_path=format!("/api/v1/author/{}",locked_author["data"]["id"].as_str().unwrap());
    call(&client,&origin,"PUT",&lock_path,Some(json!({"version":1,"locked":true})),Some(2),403).await;
    let locked=call(&client,&origin,"PUT",&lock_path,Some(json!({"version":1,"locked":true})),Some(5),200).await;
    assert_eq!(locked["data"]["attributes"]["locked"],true);
    call(&client,&origin,"PUT",&lock_path,Some(json!({"version":2,"name":"Cannot edit locked"})),Some(2),403).await;
    call(&client,&origin,"PUT",&lock_path,Some(json!({"version":1,"locked":false})),Some(5),409).await;
    call(&client,&origin,"PUT",&lock_path,Some(json!({"version":2,"locked":false})),Some(5),200).await;
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
    // The private list must remain attached to the explicitly captured
    // account even if the signer binding changes after UI account discovery.
    call(&client,&origin,"GET",&format!("/api/v1/manga/draft?expectedAccountId={member}"),None,Some(2),409).await;
    let pinned=call(&client,&origin,"GET",&format!("/api/v1/manga/draft?expectedAccountId={owner}"),None,Some(2),200).await;
    assert_eq!(pinned["total"],3);
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
    call(&client,&origin,"DELETE",&format!("{apath}?expectedAccountId={owner}"),None,Some(5),409).await;
    let staff=resolver.preferred(ResourceKind::User,"EditorialUser5").await.unwrap().unwrap();
    call(&client,&origin,"DELETE",&format!("{apath}?expectedAccountId={staff}"),None,Some(5),200).await;
    call(&client, &origin, "DELETE", &apath, None, Some(5), 404).await;
    call(&client, &origin, "DELETE", &gpath, None, Some(4), 403).await;
    call(&client, &origin, "DELETE", &gpath, None, Some(5), 200).await;
    let sql = state.db.client().await.unwrap();
    let audits=sql.query("SELECT actor_id,operation,old_version,new_version FROM platform.catalog_audit ORDER BY id",&[]).await.unwrap();
    assert_eq!(audits.len(), 17);
    // Exact per-entity histories also prove every refused lock, role and stale
    // deletion left no audit side effect.
    for (kind, created, expected) in [
        (ResourceKind::Group, &rich_group, vec![("create",None,Some(1)),("update",Some(1),Some(2)),("remove",None,None)]),
        (ResourceKind::Author, &locked_author, vec![("create",None,Some(1)),("update",Some(1),Some(2)),("update",Some(2),Some(3))]),
    ] {
        let public_id=PublicUuid::parse(created["data"]["id"].as_str().unwrap()).unwrap();
        let native=resolver.resolve(kind,&public_id).await.unwrap().unwrap().native_id;
        let rows=sql.query("SELECT operation,old_version,new_version FROM platform.catalog_audit WHERE entity_id=$1 ORDER BY id",&[&native]).await.unwrap();
        let actual:Vec<(String,Option<i32>,Option<i32>)>=rows.iter().map(|r|(r.get(0),r.get(1),r.get(2))).collect();
        assert_eq!(actual,expected.into_iter().map(|(op,old,new)|(op.to_owned(),old,new)).collect::<Vec<_>>());
    }
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
