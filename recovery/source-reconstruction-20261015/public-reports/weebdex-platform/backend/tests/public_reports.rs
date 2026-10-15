//! RECONSTRUCTED-UNVERIFIED after executor replacement. Fresh execution required.
//! Actual TCP, original request authorization and private instance moderation.
//! Every key/account below is synthetic and belongs only to this disposable DB.
use base64::{Engine, engine::general_purpose::STANDARD};
use manga_node::{auth::{self, Event}, db::{DbConfig, Id, Repository}};
use secp256k1::{Keypair, Message, Secp256k1, SecretKey};
use serde_json::{Value, json};
use std::{sync::atomic::{AtomicUsize, Ordering}, time::Duration};
use weebdex_platform::{AppState, Config, domains, protocol::publication::OperatorRuntimeConfig};
fn public(key: u8) -> String {
    auth::sign(&format!("{key:064x}"),1,vec![],String::new()).unwrap().pubkey
}
fn id(key: u8) -> Id { hex::decode(public(key)).unwrap().try_into().unwrap() }
fn proof_for(key: u8, service: u8, url: &str, method: &str, body: &[u8]) -> String {
    static NONCE: AtomicUsize = AtomicUsize::new(0);
    let secret = SecretKey::from_slice(&hex::decode(format!("{key:064x}")).unwrap()).unwrap();
    let secp = Secp256k1::new(); let pair = Keypair::from_secret_key(&secp,&secret);
    let mut event = Event { id: String::new(), pubkey: public(key), created_at: auth::now(), kind:27235,
        tags:vec![vec!["u".into(),url.into()],vec!["method".into(),method.into()],vec!["service".into(),public(service)],vec!["payload".into(),auth::hash(body)],vec!["nonce".into(),NONCE.fetch_add(1,Ordering::Relaxed).to_string()]],content:String::new(),sig:String::new() };
    let digest=event.digest(); event.id=hex::encode(&digest);
    event.sig=secp.sign_schnorr_no_aux_rand(&Message::from_digest_slice(&digest).unwrap(),&pair).to_string();
    format!("Nostr {}",STANDARD.encode(serde_json::to_vec(&event).unwrap()))
}
fn proof(key:u8,url:&str,method:&str,body:&[u8])->String { proof_for(key,11,url,method,body) }
async fn call(client:&reqwest::Client,origin:&str,method:&str,path:&str,payload:Option<Value>,key:Option<u8>,expected:u16)->Value {
    let url=format!("{origin}{path}");
    let body=payload.map_or_else(Vec::new,|value|serde_json::to_vec(&value).unwrap());
    let mut request=client.request(method.parse().unwrap(),&url).header("content-type","application/json");
    if let Some(key)=key { request=request.header("authorization",proof(key,&url,method,&body)); }
    let response=request.body(body).send().await.unwrap(); let status=response.status().as_u16();
    assert!(response.headers()["cache-control"].to_str().unwrap().contains("no-store"));
    let value:Value=response.json().await.unwrap(); assert_eq!(status,expected,"{method} {path}: {value}"); value
}
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server { fn drop(&mut self) { self.0.abort(); } }
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires scripts/run-auth-tests.sh and its owned isolated PostgreSQL"]
async fn reports_use_genuine_bound_authority_private_scope_and_native_audit() {
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use isolated runner");
    let pg: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
    assert!(!pg.get_hosts().is_empty());
    assert!(pg.get_hosts().iter().all(|host| matches!(host,tokio_postgres::config::Host::Tcp(host)if host=="127.0.0.1")));
    assert!(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap().contains("/weebdex-platform/test-runtime/"));
    let repository = Repository::connect(DbConfig { dsn: dsn.clone(), connections: 2,
        acquisition_timeout: Duration::from_secs(2), cache_weight: 1048576, plaintext_local: true }).await.unwrap();
    repository.migrate().await.unwrap();
    repository.bind_instance(id(11), id(1)).await.unwrap();
    for key in 2..=10 { repository.set_account_admission(id(key), true).await.unwrap(); }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:4,
        protocol_publication:Some(OperatorRuntimeConfig{operator_key_file:"fixture-unused/operator.key".into(),expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),storage:json!({"path":"fixture-unused/storage","volume":"reportfixture","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144}),group_keys_dir:None,backend:None,native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":"fixture-unused/service.key","admin_pubkey":public(1)}))}),..Config::default()},dsn).await.unwrap();
    // Exercise real additive backfill on existing reason rows before migration92.
    let position = domains::MIGRATIONS.iter().position(|m|m.version==92).unwrap();
    state.db.migrate(&domains::MIGRATIONS[..position]).await.unwrap();
    state.db.client().await.unwrap().execute("INSERT INTO platform.report_reasons(id,category,reason,details_required) VALUES('manga_fixture','manga','Existing fixture reason',false)",&[]).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();
    state.db.verify_schema(domains::MIGRATIONS).await.unwrap();
    state.protocol.verify_schema().await.unwrap();
    let sql = state.db.client().await.unwrap();
    for (key,roles,permissions,verified,disabled) in [
        (2,json!([]),json!(["report.create"]),true,false),
        (4,json!([]),json!(["report.create"]),true,false),
        (5,json!(["moderator"]),json!([]),true,false),
        (6,json!(["moderator"]),json!(["report.update"]),true,false),
        (7,json!([]),json!(["report.update"]),true,false),
        (8,json!(["restricted"]),json!(["report.create"]),true,false),
        (9,json!([]),json!(["report.create"]),true,true),
        (10,json!([]),json!(["report.create"]),false,false),
    ] {
        let user = format!("ReportUser{key}");
        sql.execute("INSERT INTO platform.users(id,username,email,password_hash,roles,permissions,verified,disabled) VALUES($1,$1,$2,'synthetic-no-password',$3,$4,$5,$6)",&[&user,&format!("report-{key}@fixture.invalid"),&roles,&permissions,&verified,&disabled]).await.unwrap();
        sql.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await.unwrap();
    }
    sql.batch_execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES('ReportManga1','ReportUser2','Owner','Fixture one','en','ongoing','safe','published'),('ReportManga2','ReportUser2','Owner','Fixture two','en','ongoing','safe','published'); INSERT INTO platform.catalog_authors(id,owner_id,owner_name,name) VALUES('ReportAuthor1','ReportUser2','Owner','Fixture author'); INSERT INTO platform.report_reasons(id,category,reason,details_required) VALUES('manga_new_fixture','manga','Inserted fixture reason',false)").await.unwrap();
    drop(sql);
    use weebdex_platform::public_api::{PostgresPublicIdResolver,PublicIdResolver,PublicUuid,ResourceKind};
    let resolver=PostgresPublicIdResolver::new(state.db.clone());
    let manga=resolver.preferred(ResourceKind::Manga,"ReportManga1").await.unwrap().unwrap();
    let manga2=resolver.preferred(ResourceKind::Manga,"ReportManga2").await.unwrap().unwrap();
    let author=resolver.preferred(ResourceKind::Author,"ReportAuthor1").await.unwrap().unwrap();
    let reason=resolver.preferred(ResourceKind::ReportReason,"manga_spam").await.unwrap().unwrap();
    let other_reason=resolver.preferred(ResourceKind::ReportReason,"author_spam").await.unwrap().unwrap();
    let backfilled=resolver.preferred(ResourceKind::ReportReason,"manga_fixture").await.unwrap().unwrap();
    let inserted=resolver.preferred(ResourceKind::ReportReason,"manga_new_fixture").await.unwrap().unwrap();
    assert_ne!(backfilled,inserted);
    assert!(resolver.resolve(ResourceKind::Manga,&reason).await.unwrap().is_none());
    let old_alias=PublicUuid::parse("aaaaaaaa-aaaa-8aaa-8aaa-aaaaaaaaaaaa").unwrap();
    resolver.register_alias(ResourceKind::ReportReason,"manga_spam",&old_alias).await.unwrap();
    let target_alias=PublicUuid::parse("bbbbbbbb-bbbb-8bbb-8bbb-bbbbbbbbbbbb").unwrap();
    resolver.register_alias(ResourceKind::Manga,"ReportManga1",&target_alias).await.unwrap();
    let app = weebdex_platform::router::router(state.clone());
    let _server = Server(tokio::spawn(async move { axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await.unwrap(); }));
    let client=reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(10)).build().unwrap();
    call(&client,&origin,"GET","/api/v1/report/reasons/manga",None,None,401).await;
    for key in [3,8,9,10,12] {
        call(&client,&origin,"GET","/api/v1/report",None,Some(key),403).await;
    }
    let no_cookie_authority=client.get(format!("{origin}/api/v1/report")).header("cookie","weebdex_session=synthetic-fixture-cookie").send().await.unwrap();
    assert_eq!(no_cookie_authority.status().as_u16(),401);
    let reasons=call(&client,&origin,"GET","/api/v1/report/reasons/manga",None,Some(2),200).await;
    assert_eq!(reasons["response"],"collection");
    let item=reasons["data"].as_array().unwrap().iter().find(|r|r["id"]==reason.as_str()).unwrap();
    assert_eq!(item["type"],"report_reason"); assert_eq!(item["attributes"]["version"],1);
    assert_eq!(item["attributes"]["detailsRequired"],true); assert_eq!(item["attributes"]["category"],"manga");
    state.db.client().await.unwrap().execute("UPDATE platform.report_reasons SET reason='Updated fixture label' WHERE id='manga_spam'",&[]).await.unwrap();
    let reasons=call(&client,&origin,"GET","/api/v1/report/reasons/manga",None,Some(2),200).await;
    let item=reasons["data"].as_array().unwrap().iter().find(|r|r["id"]==reason.as_str()).unwrap();
    assert_eq!(item["attributes"]["version"],2); assert_eq!(item["attributes"]["reason"]["en"],"Updated fixture label");
    for path in ["/api/v1/report/reasons/reply","/api/v1/report/reasons/manga?limit=1","/api/v1/report?limit=0","/api/v1/report?limit=101","/api/v1/report?offset=10001","/api/v1/report?status=other","/api/v1/report?category=reply","/api/v1/report?reasonId=manga_spam","/api/v1/report?order[status]=asc","/api/v1/report?includes[]=manga"] {
        call(&client,&origin,"GET",path,None,Some(2),400).await;
    }
    let url=format!("{origin}/api/v1/report");
    let first=proof(2,&url,"GET",&[]);
    for expected in [200,401] { assert_eq!(client.get(&url).header("authorization",&first).send().await.unwrap().status().as_u16(),expected); }
    assert_eq!(client.get(&url).header("authorization",proof_for(2,10,&url,"GET",&[])).send().await.unwrap().status().as_u16(),401);
    assert_eq!(client.get(&url).header("authorization",proof(2,&format!("{url}?limit=1"),"GET",&[])).send().await.unwrap().status().as_u16(),401);
    let payload=json!({"category":"manga","reason":old_alias,"objectId":target_alias,"details":"Local fixture report"});
    call(&client,&origin,"POST","/api/v1/report",Some(payload.clone()),None,401).await;
    call(&client,&origin,"POST","/api/v1/report",Some(payload.clone()),Some(3),403).await;
    call(&client,&origin,"POST","/api/v1/report",Some(payload.clone()),Some(5),403).await;
    for (field,value) in [("reason",json!(other_reason)),("reason",json!(manga)),("details",json!("   ")),("version",json!(1)),("status",json!("accepted")),("category",json!("thread"))] {
        let mut invalid=payload.clone(); invalid[field]=value;
        call(&client,&origin,"POST","/api/v1/report",Some(invalid),Some(2),400).await;
    }
    let mut invalid=payload.clone(); invalid["objectId"]=json!(author);
    call(&client,&origin,"POST","/api/v1/report",Some(invalid),Some(2),404).await;
    // Current original-source visibility wins over an otherwise active native row.
    state.db.client().await.unwrap().execute("UPDATE platform.catalog_manga SET protocol_series_id=$1 WHERE id='ReportManga1'", &[&"f".repeat(64)]).await.unwrap();
    call(&client,&origin,"POST","/api/v1/report",Some(payload.clone()),Some(2),404).await;
    state.db.client().await.unwrap().execute("UPDATE platform.catalog_manga SET protocol_series_id=NULL WHERE id='ReportManga1'", &[]).await.unwrap();
    state.db.client().await.unwrap().execute("UPDATE platform.report_reasons SET enabled=false WHERE id='manga_fixture'", &[]).await.unwrap();
    let mut disabled_reason=payload.clone(); disabled_reason["reason"]=json!(backfilled);
    call(&client,&origin,"POST","/api/v1/report",Some(disabled_reason),Some(2),400).await;
    let mut oversized=payload.clone(); oversized["details"]=json!("a".repeat(8192));
    call(&client,&origin,"POST","/api/v1/report",Some(oversized),Some(2),400).await;
    let tampered=client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",b"{}")).json(&payload).send().await.unwrap();
    assert_eq!(tampered.status().as_u16(),401);
    let created=call(&client,&origin,"POST","/api/v1/report",Some(payload.clone()),Some(2),201).await;
    assert_eq!(created,json!({"result":"ok"}));
    call(&client,&origin,"POST","/api/v1/report",Some(payload.clone()),Some(2),409).await;
    call(&client,&origin,"POST","/api/v1/report",Some(payload.clone()),Some(4),201).await;
    // Same-account concurrent duplicates must serialize before the native core.
    let mut concurrent=payload.clone(); concurrent["objectId"]=json!(manga2);
    let raw=serde_json::to_vec(&concurrent).unwrap();
    let a=client.post(&url).header("content-type","application/json").header("authorization",proof(4,&url,"POST",&raw)).body(raw.clone()).send();
    let b=client.post(&url).header("content-type","application/json").header("authorization",proof(4,&url,"POST",&raw)).body(raw).send();
    let (a,b)=tokio::join!(a,b); let mut statuses=[a.unwrap().status().as_u16(),b.unwrap().status().as_u16()]; statuses.sort(); assert_eq!(statuses,[201,409]);
    let mut second=payload.clone(); second["objectId"]=json!(manga2);
    call(&client,&origin,"POST","/api/v1/report",Some(second),Some(2),201).await;
    let own=call(&client,&origin,"GET","/api/v1/report?includes[]=user&includes[]=reason&order[createdAt]=asc",None,Some(2),200).await;
    assert_eq!(own["total"],2); assert_eq!(own["limit"],10); assert_eq!(own["offset"],0);
    let first=&own["data"][0]; assert!(PublicUuid::parse(first["id"].as_str().unwrap()).is_ok());
    assert_eq!(first["type"],"report"); assert_eq!(first["attributes"]["details"],"Local fixture report");
    assert_eq!(first["attributes"]["objectId"],manga.as_str()); assert_eq!(first["attributes"]["status"],"waiting");
    assert!(first["attributes"]["createdAt"].is_string()); assert_eq!(first["relationships"][0]["attributes"]["username"],"ReportUser2");
    assert_eq!(first["relationships"][1]["id"],reason.as_str()); assert_eq!(first["relationships"][1]["attributes"]["version"],2);
    for hidden in ["body","notes","submitter_id","reviewed_by","entity_id","reason_id"] { assert!(!first.to_string().contains(&format!("\"{hidden}\""))); }
    let page=call(&client,&origin,"GET","/api/v1/report?limit=1&offset=1&order[createdAt]=asc",None,Some(2),200).await;
    assert_eq!(page["total"],2); assert_eq!(page["data"].as_array().unwrap().len(),1); assert_eq!(page["data"][0]["id"],own["data"][1]["id"]);
    assert!(page["data"][0]["relationships"][0].get("attributes").is_none());
    for (key,total) in [(4,2),(5,0),(6,4),(7,0)] { assert_eq!(call(&client,&origin,"GET","/api/v1/report",None,Some(key),200).await["total"],total); }
    let filter=format!("/api/v1/report?category=manga&reasonId={old_alias}&objectId={target_alias}&status=waiting");
    assert_eq!(call(&client,&origin,"GET",&filter,None,Some(2),200).await["total"],1);
    for filter in [format!("/api/v1/report?reasonId={manga}"),format!("/api/v1/report?category=author&objectId={manga}"),"/api/v1/report?reasonId=00000000-0000-4000-8000-000000000000".into()] {
        assert_eq!(call(&client,&origin,"GET",&filter,None,Some(2),200).await["total"],0);
    }
    let sql=state.db.client().await.unwrap();
    let audits:i64=sql.query_one("SELECT count(*) FROM platform.report_audit WHERE operation='create' AND new_status='waiting' AND new_version=1",&[]).await.unwrap().get(0);
    assert_eq!(audits,4);
    let report_native:String=sql.query_one("SELECT id FROM platform.reports WHERE submitter_id='ReportUser2' AND entity_id='ReportManga1'",&[]).await.unwrap().get(0);
    sql.execute("UPDATE platform.reports SET status='accepted',version=version+1 WHERE id=$1",&[&report_native]).await.unwrap();
    assert_eq!(call(&client,&origin,"GET","/api/v1/report?status=accepted",None,Some(2),200).await["total"],1);
    sql.batch_execute("UPDATE platform.catalog_manga SET deleted_at=clock_timestamp() WHERE id='ReportManga1'").await.unwrap();
    call(&client,&origin,"POST","/api/v1/report",Some(payload.clone()),Some(2),404).await;
    // Report history remains private metadata; withdrawn targets have no expansion.
    let history=call(&client,&origin,"GET","/api/v1/report?status=accepted",None,Some(2),200).await;
    assert_eq!(history["data"][0]["attributes"]["objectId"],manga.as_str());
    sql.execute("UPDATE platform.reports SET deleted_at=clock_timestamp() WHERE id=$1",&[&report_native]).await.unwrap();
    assert_eq!(call(&client,&origin,"GET","/api/v1/report",None,Some(2),200).await["total"],1);
    assert!(sql.execute("UPDATE platform.report_reasons SET id='manga_renamed' WHERE id='manga_fixture'",&[]).await.is_err());
    assert!(sql.execute("UPDATE platform.public_resource_ids SET native_id='manga_spam' WHERE kind='report_reason' AND native_id='manga_fixture'",&[]).await.is_err());
    sql.batch_execute("SELECT platform.public_backfill_resource_ids()").await.unwrap();
    assert_eq!(resolver.preferred(ResourceKind::ReportReason,"manga_spam").await.unwrap(),Some(reason));
    // Shared rate/duplicate guards are serialized and leave no extra audit rows.
    sql.batch_execute("UPDATE platform.catalog_manga SET deleted_at=NULL WHERE id='ReportManga1'; INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,body,status) SELECT 'ReportRate' || n,'ReportUser2','manga','ReportManga2','manga_fixture','Fixture rate','accepted' FROM generate_series(1,18)n").await.unwrap();
    call(&client,&origin,"POST","/api/v1/report",Some(payload),Some(2),429).await;
    let audits:i64=sql.query_one("SELECT count(*) FROM platform.report_audit WHERE operation='create'",&[]).await.unwrap().get(0); assert_eq!(audits,4);
}
