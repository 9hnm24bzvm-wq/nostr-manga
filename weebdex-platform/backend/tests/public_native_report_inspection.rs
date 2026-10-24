//! Personal native report inspection: real signed HTTP and private owner scope.
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
    let response = request.body(body.clone()).send().await.unwrap();
    let status = response.status().as_u16();
    assert!(
        response.headers()["cache-control"]
            .to_str()
            .unwrap()
            .contains("no-store")
    );
    assert!(response.headers()["cache-control"].to_str().unwrap().contains("private"));
    assert!(response.headers().get_all("vary").iter().any(|v|v.to_str().unwrap().contains("Authorization")));
    assert!(response.headers().get_all("vary").iter().any(|v|v.to_str().unwrap().contains("Cookie")));
    let value: Value = response.json().await.unwrap();
    assert_eq!(status, expected, "{method} {path}: {value}");
    if status == 200 && method == "POST" {
        assert_eq!(value["nostr"]["requestHash"],auth::hash(&body));
        assert_eq!(value["nostr"]["requestMethod"],method);
        assert_eq!(value["nostr"]["requestPath"],path);
    }
    value
}
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires isolated scripts/run-auth-tests.sh PostgreSQL"]
async fn signed_native_report_inspection_is_personal_bounded_and_transaction_fenced() {
    use weebdex_platform::public_api::{PostgresPublicIdResolver, PublicIdResolver, ResourceKind};
    let dsn=std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use isolated runner");
    let pg:tokio_postgres::Config=dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(),Some("weebdex_platform_test"));
    assert!(!pg.get_hosts().is_empty());
    assert!(pg.get_hosts().iter().all(|h|matches!(h,tokio_postgres::config::Host::Tcp(h) if h=="127.0.0.1")));
    assert!(pg.get_ports().iter().all(|p| *p>1024 && *p!=5432));
    let runtime=std::path::PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap());
    assert!(runtime.to_str().unwrap().contains("/weebdex-platform/test-runtime/"));
    assert!(runtime.join("fixture.json").is_file());
    let original=Repository::connect(DbConfig{dsn:dsn.clone(),connections:2,acquisition_timeout:Duration::from_secs(2),cache_weight:1048576,plaintext_local:true}).await.unwrap();
    original.migrate().await.unwrap();original.bind_instance(id(11),id(1)).await.unwrap();
    for key in [2,3,4,5,6,7] {original.set_account_admission(id(key),true).await.unwrap();}
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let origin=format!("http://{}",listener.local_addr().unwrap());
    let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:10,uploads_dir:runtime.join("media"),ui_dir:runtime.join("no-ui"),protocol_publication:Some(OperatorRuntimeConfig{operator_key_file:runtime.join("unused/operator.key"),expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),storage:json!({"path":runtime.join("unused/storage"),"volume":"reportsfixture","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144}),group_keys_dir:None,backend:None,native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":runtime.join("unused/service.key"),"admin_pubkey":public(1)}))}),..Config::default()},dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();state.protocol.verify_schema().await.unwrap();
    let db=state.db.client().await.unwrap();
    for key in [2,4,5,6,7,8] {
        let user=format!("InspectUser{key}");
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,disabled,roles,permissions,profile) VALUES($1,$1,$2,'synthetic',TRUE,$3,$4,$5,$6)", &[&user,&format!("inspection{key}@fixture.invalid"),&false,&if key==7 {json!(["moderator"])} else {json!(["user"])},&if key==7 {json!(["report.update"])} else {json!([])},&json!({"email":"private-profile@fixture.invalid"})]).await.unwrap();
        db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)", &[&public(key),&user]).await.unwrap();
        db.execute("UPDATE platform.users SET disabled=$2 WHERE id=$1",&[&user,&(key==5)]).await.unwrap();
    }
    // Synthetic persisted history deliberately includes an unavailable subject.
    // Inspection must retain its identifier without reading or expanding content.
    for n in 0..7 {
        let category=if n%2==0 {"thread"} else {"reply"};
        let object=if category=="thread" {format!("MissingThread{n}")} else {format!("{}",900+n)};
        db.execute("INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,body,notes,reviewed_by,created_at) VALUES($1,'InspectUser2',$2,$3,$4,$5,'PRIVATE_MODERATOR_NOTE','InspectUser4','2026-10-22T00:00:00.123456Z')", &[&format!("native_report_{n}"),&category,&object,&format!("{category}_spam"),&format!("Owner detail {n}")]).await.unwrap();
    }
    db.execute("INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,body) VALUES('foreign_report','InspectUser4','thread','ForeignTarget','thread_spam','FOREIGN_PRIVATE_BODY'),('moderator_report','InspectUser7','reply','998','reply_spam','Moderator own submission'),('deleted_report','InspectUser2','thread','DeletedTarget','thread_other','DELETED_PRIVATE_BODY'),('catalogue_report','InspectUser2','user','InspectUser4','user_spam','CATALOGUE_PRIVATE_BODY')",&[]).await.unwrap();
    db.execute("UPDATE platform.reports SET deleted_at=clock_timestamp() WHERE id='deleted_report'",&[]).await.unwrap();
    db.execute("INSERT INTO platform.report_audit(report_id,actor_id,operation,new_status,new_version,notes,effect) VALUES('native_report_6','InspectUser4','accept','accepted',1,'PRIVATE_AUDIT_NOTE','{\"secret\":\"PRIVATE_EFFECT\"}')",&[]).await.unwrap();
    let resolver=PostgresPublicIdResolver::new(state.db.clone());
    let actor=resolver.preferred(ResourceKind::User,"InspectUser2").await.unwrap().unwrap();
    let other=resolver.preferred(ResourceKind::User,"InspectUser4").await.unwrap().unwrap();
    let empty=resolver.preferred(ResourceKind::User,"InspectUser6").await.unwrap().unwrap();
    let moderator=resolver.preferred(ResourceKind::User,"InspectUser7").await.unwrap().unwrap();
    drop(db);
    let app=weebdex_platform::router::router(state.clone());let _server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap();}));
    let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build().unwrap();
    let base="/api/v1/nostr/account/reports/native";
    let list=format!("{base}?expectedAccountId={actor}");
    let detail=format!("{base}/native_report_6?expectedAccountId={actor}");
    call(&client,&origin,"GET",&list,None,None,401).await;
    for key in [3,5,8] {call(&client,&origin,"GET",&list,None,Some(key),403).await;}
    for path in [base.to_owned(),format!("{base}/native_report_6")] {call(&client,&origin,"GET",&path,None,Some(2),400).await;}
    for suffix in ["&scope=all","&owner=InspectUser4","&includes[]=user","&page=0","&page=01","&page=2002","&page=1&page=2","&category=manga","&status=resolved","&reasonId=bad%2Fid","&objectId=","&expectedAccountId=null"] {call(&client,&origin,"GET",&format!("{list}{suffix}"),None,Some(2),400).await;}
    call(&client,&origin,"GET",&list,Some(json!({})),Some(2),400).await;
    call(&client,&origin,"GET",&format!("{detail}&page=1"),None,Some(2),400).await;
    let first=call(&client,&origin,"GET",&list,None,Some(2),200).await;
    assert_eq!(first["total"],7);assert_eq!(first["limit"],5);assert_eq!(first["offset"],0);assert_eq!(first["page"],1);
    assert_eq!(first["data"].as_array().unwrap().iter().map(|r|r["id"].as_str().unwrap()).collect::<Vec<_>>(),vec!["native_report_6","native_report_5","native_report_4","native_report_3","native_report_2"]);
    assert_eq!(first["nostr"],json!({"scope":"instance-account","source":"persisted-native-personal-reports","accountId":actor,"nativeSessionAuthority":false}));
    for secret in ["FOREIGN_PRIVATE_BODY","DELETED_PRIVATE_BODY","CATALOGUE_PRIVATE_BODY","PRIVATE_MODERATOR_NOTE","PRIVATE_AUDIT_NOTE","PRIVATE_EFFECT","private-profile","fixture.invalid","reviewed_by","submitter_id","relationships"] {assert!(!first.to_string().contains(secret),"{secret}");}
    let one=call(&client,&origin,"GET",&detail,None,Some(2),200).await;
    assert_eq!(one["data"],first["data"][0]);assert_eq!(one["data"]["objectId"],"MissingThread6");assert_eq!(one["data"].as_object().unwrap().len(),7);
    assert_eq!(one["data"]["createdAt"],"2026-10-22T00:00:00.123456Z");
    for report in ["foreign_report","moderator_report","deleted_report","catalogue_report","missing_report"] {assert!(call(&client,&origin,"GET",&format!("{base}/{report}?expectedAccountId={actor}"),None,Some(2),404).await.get("data").is_none());}
    let second=call(&client,&origin,"GET",&format!("{list}&page=2"),None,Some(2),200).await;assert_eq!(second["total"],7);assert_eq!(second["offset"],5);assert_eq!(second["data"].as_array().unwrap().len(),2);
    for suffix in ["&page=2001","&status=autoresolved","&reasonId=missing_reason","&objectId=missing_target"] {let value=call(&client,&origin,"GET",&format!("{list}{suffix}"),None,Some(2),200).await;assert_eq!(value["data"],json!([]));}
    let filtered=call(&client,&origin,"GET",&format!("{list}&category=reply&reasonId=reply_spam&objectId=905&status=waiting"),None,Some(2),200).await;assert_eq!(filtered["total"],1);assert_eq!(filtered["data"][0]["id"],"native_report_5");
    assert_eq!(call(&client,&origin,"GET",&format!("{base}?expectedAccountId={empty}"),None,Some(6),200).await["total"],0,"No create grant required");
    let own_mod=call(&client,&origin,"GET",&format!("{base}?expectedAccountId={moderator}"),None,Some(7),200).await;assert_eq!(own_mod["total"],1);assert_eq!(own_mod["data"][0]["id"],"moderator_report");
    call(&client,&origin,"GET",&format!("{base}/native_report_6?expectedAccountId={moderator}"),None,Some(7),404).await;
    // Cookie headers provide no authority, with or without a signed wrong owner.
    let url=format!("{origin}{list}");
    assert_eq!(client.get(&url).header("cookie","platform_session=synthetic-invalid-cookie").send().await.unwrap().status().as_u16(),401);
    let mixed=client.get(&url).header("cookie","platform_session=synthetic-invalid-cookie").header("authorization",proof(4,&url,"GET",b"")).send().await.unwrap();assert_eq!(mixed.status().as_u16(),409);
    // Exact request proof: URL/service/body/method mismatch and replay cannot read.
    assert_eq!(client.get(&url).header("authorization",proof_for(2,12,&url,"GET",b"")).send().await.unwrap().status().as_u16(),401);
    assert_eq!(client.get(&url).header("authorization",proof(2,&format!("{url}&page=2"),"GET",b"")).send().await.unwrap().status().as_u16(),401);
    let auth=proof(2,&url,"GET",b"");assert_eq!(client.get(&url).header("authorization",&auth).send().await.unwrap().status().as_u16(),200);assert_eq!(client.get(&url).header("authorization",auth).send().await.unwrap().status().as_u16(),401);
    // A proof captured for A, then a completed rebind to B, must fail BEFORE
    // count/rows even for empty pages and unknown/foreign detail identifiers.
    let captured=proof(2,&url,"GET",b"");
    state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='InspectUser4' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    let rebound=client.get(&url).header("authorization",captured).send().await.unwrap();assert_eq!(rebound.status().as_u16(),409);assert!(rebound.json::<Value>().await.unwrap().get("data").is_none());
    for path in [format!("{list}&page=2001&status=autoresolved"),detail.clone(),format!("{base}/missing_report?expectedAccountId={actor}")] {let error=call(&client,&origin,"GET",&path,None,Some(2),409).await;assert!(error.get("data").is_none());assert!(error.get("total").is_none());}
    assert_eq!(call(&client,&origin,"GET",&format!("{base}?expectedAccountId={other}"),None,Some(2),200).await["data"][0]["id"],"foreign_report");
    state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='InspectUser2' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    // Existing, disabled reasons remain visible with current label/revision.
    state.db.client().await.unwrap().execute("UPDATE platform.report_reasons SET reason='Renamed reason',enabled=FALSE WHERE id='thread_spam'",&[]).await.unwrap();
    let renamed=call(&client,&origin,"GET",&detail,None,Some(2),200).await;assert_eq!(renamed["data"]["reason"]["reason"],"Renamed reason");assert!(renamed["data"]["reason"]["version"].as_i64().unwrap()>1);
    // Five maximally escaped persisted reports plus maximally escaped reasons
    // fit the full signed JSON budget without truncation.
    let details="\u{1}".repeat(20000);let label="\u{1}".repeat(8000);
    state.db.client().await.unwrap().execute("UPDATE platform.reports SET body=$1 WHERE submitter_id='InspectUser2' AND entity_type IN ('thread','reply')",&[&details]).await.unwrap();
    state.db.client().await.unwrap().execute("UPDATE platform.report_reasons SET reason=$1 WHERE id IN ('thread_spam','reply_spam')",&[&label]).await.unwrap();
    let bounded=call(&client,&origin,"GET",&list,None,Some(2),200).await;assert!(serde_json::to_vec(&bounded).unwrap().len()<1024*1024);assert_eq!(bounded["data"][0]["details"],details);assert_eq!(bounded["data"][0]["reason"]["reason"],label);
    // Malformed selected reason fails the WHOLE response, without truncating,
    // omitting a row, exposing private partial data, or reading huge TEXT.
    for value in [String::new(),"x".repeat(8001),"x".repeat(2*1024*1024)] {
        state.db.client().await.unwrap().execute("UPDATE platform.report_reasons SET reason=$1 WHERE id='thread_spam'",&[&value]).await.unwrap();
        for path in [&list,&detail] {let error=call(&client,&origin,"GET",path,None,Some(2),503).await;assert!(error.get("data").is_none());assert!(error.get("total").is_none());}
    }
    state.db.client().await.unwrap().execute("UPDATE platform.report_reasons SET reason='Restored reason' WHERE id='thread_spam'",&[]).await.unwrap();
    for (column,value) in [("entity_id","bad/target".to_owned()),("entity_id","x".repeat(129))] {
        state.db.client().await.unwrap().execute(&format!("UPDATE platform.reports SET {column}=$1 WHERE id='native_report_6'"),&[&value]).await.unwrap();
        assert!(call(&client,&origin,"GET",&list,None,Some(2),503).await.get("data").is_none());
    }
}
