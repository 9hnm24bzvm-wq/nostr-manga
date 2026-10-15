//! RECONSTRUCTED-UNVERIFIED after executor replacement. Fresh execution required.
//! Native author/group regression over a real loopback HTTP listener and owned
//! disposable PostgreSQL fixture. No successful response or authority stubs.
use axum::http::StatusCode;
use reqwest::{Client, Method};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use weebdex_platform::{AppState, Config, domains};

async fn call(client: &Client, base: &str, method: Method, path: &str,
              payload: Option<Value>, actor: Option<&str>, expected: StatusCode) -> Value {
    let mut request = client.request(method, format!("{base}{path}"))
        .header("origin", "https://catalog-entities.fixture.invalid");
    if let Some(actor) = actor {
        request = request.header("cookie", format!("weebdex_session=entity-fixture-{actor}"));
    }
    if let Some(payload) = payload { request = request.json(&payload); }
    let response = request.send().await.unwrap();
    let actual = response.status();
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    let bytes = response.bytes().await.unwrap();
    let result = if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes).unwrap() };
    assert_eq!(actual, expected, "{path}: {result}");
    result
}

#[tokio::test]
#[ignore = "Requires the explicitly isolated PostgreSQL runner"]
async fn native_entity_crud_preserves_locks_memberships_versions_and_audit() {
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL")
        .expect("Use scripts/run-auth-tests.sh with the owned disposable fixture");
    let pg: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
    assert!(!pg.get_hosts().is_empty());
    for host in pg.get_hosts() {
        assert!(matches!(host, tokio_postgres::config::Host::Tcp(host) if host == "127.0.0.1"));
    }
    assert!(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap().contains("weebdex-platform/test-runtime/"));
    let state = AppState::connect(Config {
        public_origin: "https://catalog-entities.fixture.invalid".into(),
        database_plaintext_local: true, registration_enabled: false, ..Config::default()
    }, dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();
    let db = state.db.client().await.unwrap();
    let permissions = json!(["author.create","author.update","author.delete","author.lock.update","scanlation_group.create","scanlation_group.update","scanlation_group.delete","self.scanlation_group.update"]);
    for (id, role) in [("EntityStaff", "staff"), ("EntityLeader", "user"), ("EntityMember", "user")] {
        let roles = json!([role]);
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,roles,permissions) VALUES($1,$1,$1||'@fixture.invalid','synthetic-no-password',TRUE,$2,$3)", &[&id,&roles,&permissions]).await.unwrap();
        let digest = Sha256::digest(format!("entity-fixture-{id}").as_bytes()).to_vec();
        db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES($1,$1,$2,clock_timestamp()+interval '1 hour')", &[&id,&digest]).await.unwrap();
    }
    drop(db);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}",listener.local_addr().unwrap());
    let app = weebdex_platform::router::router(state.clone());
    let (stop,stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener,app).with_graceful_shutdown(async {let _ = stopped.await;}).await.unwrap();
    });
    let client = Client::builder().timeout(std::time::Duration::from_secs(10)).build().unwrap();
    let staff = Some("EntityStaff"); let leader = Some("EntityLeader"); let member = Some("EntityMember");
    call(&client,&base,Method::POST,"/api/author",Some(json!({"name":"Anonymous"})),None,StatusCode::UNAUTHORIZED).await;
    // Native creation's original name-only contract stays closed.
    call(&client,&base,Method::POST,"/api/author",Some(json!({"name":"Author","description":"No native create expansion"})),staff,StatusCode::BAD_REQUEST).await;
    let author = call(&client,&base,Method::POST,"/api/author",Some(json!({"name":"  Core author  "})),staff,StatusCode::CREATED).await;
    let aid = author["id"].as_str().unwrap();
    assert_eq!(author["name"],"Core author"); assert_eq!(author["version"],1);
    let updated = call(&client,&base,Method::PUT,&format!("/api/author/{aid}"),Some(json!({"version":1,"description":"Author metadata","twitter":"https://fixture.invalid/author"})),staff,StatusCode::OK).await;
    assert_eq!(updated["version"],2); assert_eq!(updated["created_at"],author["created_at"]);
    assert_eq!(updated["description"],"Author metadata");
    call(&client,&base,Method::PUT,&format!("/api/author/{aid}"),Some(json!({"version":1,"name":"Stale write"})),staff,StatusCode::CONFLICT).await;
    call(&client,&base,Method::PUT,&format!("/api/author/{aid}"),Some(json!({"version":2,"locked":true})),staff,StatusCode::OK).await;
    call(&client,&base,Method::PUT,&format!("/api/author/{aid}"),Some(json!({"version":3,"name":"Locked write"})),leader,StatusCode::FORBIDDEN).await;
    call(&client,&base,Method::PUT,&format!("/api/author/{aid}"),Some(json!({"version":3,"locked":false})),leader,StatusCode::FORBIDDEN).await;
    call(&client,&base,Method::DELETE,&format!("/api/author/{aid}"),None,leader,StatusCode::FORBIDDEN).await;
    call(&client,&base,Method::PUT,&format!("/api/author/{aid}"),Some(json!({"version":3,"name":" "})),staff,StatusCode::BAD_REQUEST).await;
    // Two requests with the same version must serialize on the native row lock.
    let edit = |name: &'static str| client.put(format!("{base}/api/author/{aid}"))
        .header("origin","https://catalog-entities.fixture.invalid")
        .header("cookie","weebdex_session=entity-fixture-EntityStaff")
        .json(&json!({"version":3,"name":name})).send();
    let (first,second) = tokio::join!(edit("Concurrent one"),edit("Concurrent two"));
    let statuses = [first.unwrap().status(),second.unwrap().status()];
    assert_eq!(statuses.iter().filter(|s| **s == StatusCode::OK).count(),1);
    assert_eq!(statuses.iter().filter(|s| **s == StatusCode::CONFLICT).count(),1);
    // Canonical source visibility is checked in the locked entity lookup.
    let db = state.db.client().await.unwrap(); let hidden = "0".repeat(64);
    db.execute("INSERT INTO platform.catalog_authors(id,owner_id,owner_name,name,protocol_event_id) VALUES('HiddenAuthor','EntityStaff','EntityStaff','Hidden source',$1)",&[&hidden]).await.unwrap();
    db.execute("INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES('HiddenG001','EntityStaff','EntityStaff','Hidden group')",&[]).await.unwrap();
    db.execute("INSERT INTO platform.protocol_resource_ids(kind,source_target,compatibility_id,catalog_id) VALUES('group','synthetic-hidden-source','11111111-1111-4111-8111-111111111111','HiddenG001')",&[]).await.unwrap();
    drop(db);
    call(&client,&base,Method::PUT,"/api/group/HiddenG001",Some(json!({"version":1,"description":"Must stay hidden"})),staff,StatusCode::NOT_FOUND).await;
    call(&client,&base,Method::PUT,"/api/author/HiddenAuthor",Some(json!({"version":1,"name":"Must stay hidden"})),staff,StatusCode::NOT_FOUND).await;
    call(&client,&base,Method::POST,"/api/group",Some(json!({"name":"Group","focused_languages":["en"]})),staff,StatusCode::BAD_REQUEST).await;
    let group = call(&client,&base,Method::POST,"/api/group",Some(json!({"name":"Core group","description":"Initial group metadata"})),staff,StatusCode::CREATED).await;
    let gid = group["id"].as_str().unwrap();
    assert_eq!(group["relationships"]["members"],json!([]));
    assert!(group["relationships"]["thread"]["id"].is_string());
    call(&client,&base,Method::PUT,&format!("/api/group/{gid}"),Some(json!({"version":1,"description":"Not a leader"})),member,StatusCode::FORBIDDEN).await;
    let group = call(&client,&base,Method::PUT,&format!("/api/group/{gid}"),Some(json!({"version":1,"leader":"EntityLeader","officers":["EntityMember"],"members":[]})),staff,StatusCode::OK).await;
    assert_eq!(group["version"],2); assert_eq!(group["relationships"]["members"].as_array().unwrap().len(),2);
    let group = call(&client,&base,Method::PUT,&format!("/api/group/{gid}"),Some(json!({"version":2,"description":"Leader updated","inactive":true,"focused_languages":["en","zh-hk"]})),leader,StatusCode::OK).await;
    assert_eq!(group["version"],3); assert_eq!(group["inactive"],true);
    call(&client,&base,Method::PUT,&format!("/api/group/{gid}"),Some(json!({"version":3,"name":"Leader rename"})),leader,StatusCode::FORBIDDEN).await;
    call(&client,&base,Method::PUT,&format!("/api/group/{gid}"),Some(json!({"version":3,"locked":false})),leader,StatusCode::FORBIDDEN).await;
    call(&client,&base,Method::PUT,&format!("/api/group/{gid}"),Some(json!({"version":2,"description":"Stale"})),staff,StatusCode::CONFLICT).await;
    call(&client,&base,Method::PUT,&format!("/api/group/{gid}"),Some(json!({"version":3,"description":"Must roll back","members":["UnknownMember"]})),leader,StatusCode::BAD_REQUEST).await;
    let db = state.db.client().await.unwrap();
    let row = db.query_one("SELECT version,metadata->>'description',(SELECT count(*) FROM platform.catalog_group_members WHERE group_id=$1) FROM platform.catalog_groups WHERE id=$1",&[&gid]).await.unwrap();
    assert_eq!(row.get::<_,i32>(0),3); assert_eq!(row.get::<_,String>(1),"Leader updated");
    assert_eq!(row.get::<_,i64>(2),2); drop(db);
    call(&client,&base,Method::PUT,&format!("/api/group/{gid}"),Some(json!({"version":3,"locked":true})),staff,StatusCode::OK).await;
    call(&client,&base,Method::PUT,&format!("/api/group/{gid}"),Some(json!({"version":4,"description":"Locked group write"})),leader,StatusCode::FORBIDDEN).await;
    call(&client,&base,Method::DELETE,&format!("/api/group/{gid}"),None,leader,StatusCode::FORBIDDEN).await;
    call(&client,&base,Method::DELETE,&format!("/api/group/{gid}"),None,staff,StatusCode::NO_CONTENT).await;
    call(&client,&base,Method::DELETE,&format!("/api/group/{gid}"),None,staff,StatusCode::NOT_FOUND).await;
    // Referenced authors cannot be removed while a nondeleted manga uses them.
    let db = state.db.client().await.unwrap();
    db.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating) VALUES('EntityRefManga','EntityStaff','EntityStaff','Reference guard','en','ongoing','safe')",&[]).await.unwrap();
    db.execute("INSERT INTO platform.catalog_manga_creators(manga_id,author_id,role) VALUES('EntityRefManga',$1,'author')",&[&aid]).await.unwrap(); drop(db);
    call(&client,&base,Method::DELETE,&format!("/api/author/{aid}"),None,staff,StatusCode::CONFLICT).await;
    let db = state.db.client().await.unwrap();
    db.execute("UPDATE platform.catalog_manga SET deleted_at=clock_timestamp() WHERE id='EntityRefManga'",&[]).await.unwrap(); drop(db);
    call(&client,&base,Method::DELETE,&format!("/api/author/{aid}"),None,staff,StatusCode::NO_CONTENT).await;
    call(&client,&base,Method::DELETE,&format!("/api/author/{aid}"),None,staff,StatusCode::NOT_FOUND).await;
    let db = state.db.client().await.unwrap();
    let rows = db.query("SELECT entity_type,operation,old_version,new_version,actor_id FROM platform.catalog_audit WHERE entity_id=ANY($1) ORDER BY id",&[&vec![aid,gid]]).await.unwrap();
    assert_eq!(rows.len(),10); // 5 author + 5 group successful operations only.
    for row in &rows { assert!(matches!(row.get::<_,String>(4).as_str(),"EntityStaff"|"EntityLeader")); }
    for row in rows.iter().filter(|r| r.get::<_,String>(1)=="update") {
        assert_eq!(row.get::<_,Option<i32>>(3),row.get::<_,Option<i32>>(2).map(|n|n+1));
    }
    assert_eq!(db.query_one("SELECT version FROM platform.catalog_authors WHERE id='HiddenAuthor'",&[]).await.unwrap().get::<_,i32>(0),1);
    drop(db); stop.send(()).unwrap(); server.await.unwrap();
    println!("Native author/group real-HTTP CRUD, immutable create whitelist, metadata, locks, staff/leader authority, stale versions, source visibility, member rollback, soft deletion and exact audit counts passed");
}
