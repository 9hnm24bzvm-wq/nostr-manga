//! Native-user reports: signed identity, native moderation rules, fences and locking.
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
async fn wait_for_lock(state: &AppState, pattern: &str) {
    for _ in 0..200 {
        let count: i64 = state.db.client().await.unwrap().query_one("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE $1", &[&pattern]).await.unwrap().get(0);
        if count > 0 { return; }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("request did not wait on expected final authority lock: {pattern}");
}
async fn counts(state: &AppState) -> (i64, i64) {
    let row = state.db.client().await.unwrap().query_one("SELECT (SELECT count(*) FROM platform.reports),(SELECT count(*) FROM platform.report_audit)",&[]).await.unwrap();
    (row.get(0), row.get(1))
}
async fn wait_for_advisory(state: &AppState, pattern: &str) {
    for _ in 0..200 {
        let count: i64 = state.db.client().await.unwrap().query_one("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event='advisory' AND query LIKE $1", &[&pattern]).await.unwrap().get(0);
        if count > 0 { return; }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("request did not reach advisory barrier: {pattern}");
}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires isolated scripts/run-auth-tests.sh PostgreSQL"]
async fn signed_native_user_reports_share_private_core_and_recheck_every_capture() {
    use weebdex_platform::public_api::{PostgresPublicIdResolver,PublicIdResolver,ResourceKind,PublicUuid};
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
    // Deliberately tiny generic budget: only the exact reports POST gets 131072.
    let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:10,maximum_json_bytes:1024,uploads_dir:runtime.join("media"),ui_dir:runtime.join("no-ui"),protocol_publication:Some(OperatorRuntimeConfig{operator_key_file:runtime.join("unused/operator.key"),expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),storage:json!({"path":runtime.join("unused/storage"),"volume":"reportsfixture","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144}),group_keys_dir:None,backend:None,native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":runtime.join("unused/service.key"),"admin_pubkey":public(1)}))}),..Config::default()},dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();state.protocol.verify_schema().await.unwrap();
    let normal=json!(["report.create","reply.create"]);
    let db=state.db.client().await.unwrap();
    for key in [2,4,5,6,7,8] {
        let user=format!("ReportUser{key}"); let perms=if key==6 {json!([])} else {normal.clone()};
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,disabled,roles,permissions,profile) VALUES($1,$1,$2,'synthetic',TRUE,$3,$4,$5,$6)",&[&user,&format!("report{key}@fixture.invalid"),&false,&json!(["user"]),&perms,&json!({"email":"private-profile@fixture.invalid","private_key":"synthetic-private-value"})]).await.unwrap();
        db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await.unwrap();
        db.execute("UPDATE platform.users SET disabled=$2 WHERE id=$1",&[&user,&(key==5)]).await.unwrap();
    }
    for reason in ["thread_optional","thread_escaped","thread_concurrent","thread_rollback","thread_barrier","thread_reciprocal","thread_native","thread_reason_lock"] {
        db.execute("INSERT INTO platform.report_reasons(id,category,reason,details_required) VALUES($1,'thread',$1,FALSE)",&[&reason]).await.unwrap();
    }
    let resolver=PostgresPublicIdResolver::new(state.db.clone());
    let actor=resolver.preferred(ResourceKind::User,"ReportUser2").await.unwrap().unwrap();
    let user=resolver.preferred(ResourceKind::User,"ReportUser4").await.unwrap().unwrap();
    let limited=resolver.preferred(ResourceKind::User,"ReportUser6").await.unwrap().unwrap();
    let rate_actor=resolver.preferred(ResourceKind::User,"ReportUser7").await.unwrap().unwrap();
    let alias=PublicUuid::parse("aaaaaaaa-aaaa-8aaa-8aaa-aaaaaaaaaaaa").unwrap();resolver.register_alias(ResourceKind::User,"ReportUser4",&alias).await.unwrap();
    let publisher:String=db.query_one("SELECT platform.public_register_protocol_publisher($1)::text",&[&public(4)]).await.unwrap().get(0);
    let thread:String=db.query_one("SELECT id FROM platform.catalog_threads WHERE entity_type='user' AND entity_id='ReportUser4'",&[]).await.unwrap().get(0);
    let other_thread:String=db.query_one("SELECT id FROM platform.catalog_threads WHERE entity_type='user' AND entity_id='ReportUser2'",&[]).await.unwrap().get(0);
    let reply:i64=db.query_one("INSERT INTO platform.community_replies(thread_id,reply_number,author_id,body) VALUES($1,1,'ReportUser4','Synthetic target reply') RETURNING id",&[&thread]).await.unwrap().get(0);
    let wrong_reply:i64=db.query_one("INSERT INTO platform.community_replies(thread_id,reply_number,author_id,body) VALUES($1,1,'ReportUser2','Other thread reply') RETURNING id",&[&other_thread]).await.unwrap().get(0);
    db.execute("UPDATE platform.catalog_threads SET next_reply_number=2 WHERE id=$1 OR id=$2",&[&thread,&other_thread]).await.unwrap();drop(db);
    let app=weebdex_platform::router::router(state.clone());let _server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap();}));
    let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build().unwrap();
    let path=format!("/api/v1/nostr/user/{alias}/discussion/reports");
    let preview=format!("{path}/context?expectedAccountId={actor}&expectedThreadId={thread}&targetType=thread");
    let reply_preview=format!("{path}/context?expectedAccountId={actor}&expectedThreadId={thread}&targetType=reply&replyId={reply}&expectedReplyVersion=1");
    let command=|reason:&str,details:&str|json!({"expectedAccountId":actor,"expectedUserId":user,"expectedThreadId":thread,"targetType":"thread","reasonId":reason,"expectedReasonVersion":1,"details":details});
    let reply_command=|version:i32|json!({"expectedAccountId":actor,"expectedUserId":user,"expectedThreadId":thread,"targetType":"reply","replyId":reply,"expectedReplyVersion":version,"reasonId":"reply_spam","expectedReasonVersion":1,"details":"Synthetic reply report"});
    let before=counts(&state).await;
    call(&client,&origin,"GET",&preview,None,None,401).await;
    for key in [3,5,8] {call(&client,&origin,"GET",&preview,None,Some(key),403).await;}
    call(&client,&origin,"GET",&preview.replace(actor.as_str(),limited.as_str()),None,Some(6),403).await;
    let mut denied=command("thread_spam","not permitted");denied["expectedAccountId"]=json!(limited);
    call(&client,&origin,"POST",&path,Some(denied),Some(6),403).await;
    for suffix in ["&limit=20","&targetType=reply","&replyId=1","&expectedReplyVersion=1","&email=private","&expectedAccountId=null"] {call(&client,&origin,"GET",&format!("{preview}{suffix}"),None,Some(2),400).await;}
    call(&client,&origin,"GET",&format!("{path}/context"),None,Some(2),400).await;
    call(&client,&origin,"GET",&preview,Some(json!({})),Some(2),400).await;
    for bad in [reply_preview.replace("expectedReplyVersion=1","expectedReplyVersion=0"),reply_preview.replace("expectedReplyVersion=1","expectedReplyVersion=2"),reply_preview.replace(&format!("replyId={reply}"),&format!("replyId={wrong_reply}"))] {
        let expected=if bad.contains("expectedReplyVersion=0") {400} else if bad.contains("expectedReplyVersion=2") {409} else {404};
        call(&client,&origin,"GET",&bad,None,Some(2),expected).await;
    }
    call(&client,&origin,"GET",&preview.replace(actor.as_str(),user.as_str()),None,Some(2),409).await;
    call(&client,&origin,"GET",&preview.replace(&thread,&other_thread),None,Some(2),409).await;
    let publisher_path=format!("/api/v1/nostr/user/{publisher}/discussion/reports");
    call(&client,&origin,"GET",&preview.replace(&path,&publisher_path),None,Some(2),404).await;
    call(&client,&origin,"POST",&publisher_path,Some(command("thread_spam","No native thread inference")),Some(2),404).await;
    let context=call(&client,&origin,"GET",&preview,None,Some(2),200).await;
    assert_eq!(context["nostr"],json!({"scope":"instance-account","source":"persisted-native-user-report","identity":"native_public_user","accountId":actor,"userId":user,"threadId":thread}));
    assert_eq!(context["data"]["target"],json!({"type":"thread","threadId":thread}));
    let reasons=context["data"]["reasons"].as_array().unwrap();
    assert!(reasons.iter().any(|r| *r==json!({"id":"thread_spam","version":1,"reason":"Spam or abusive content","detailsRequired":true})));
    assert!(reasons.iter().all(|r|r.as_object().unwrap().len()==4 && r["id"].as_str().unwrap().starts_with("thread_")));
    assert!(!context.to_string().contains("private-profile"));assert!(!context.to_string().contains("fixture.invalid"));assert!(!context.to_string().contains("submitter"));
    let context_reply=call(&client,&origin,"GET",&reply_preview,None,Some(2),200).await;
    assert_eq!(context_reply["data"]["target"],json!({"type":"reply","threadId":thread,"replyId":reply,"version":1}));
    assert!(context_reply["data"]["reasons"].as_array().unwrap().iter().all(|r|r["id"].as_str().unwrap().starts_with("reply_")));
    // Complete bounded catalogues: exactly 1000 enabled reasons are returned,
    // but 1001 must fail closed, never silently truncate the reviewed options.
    state.db.client().await.unwrap().execute("INSERT INTO platform.report_reasons(id,category,reason,details_required) SELECT 'thread_overflow_'||n,'thread','Bounded reason '||n,FALSE FROM generate_series(1,1000-(SELECT count(*)::INTEGER FROM platform.report_reasons WHERE category='thread' AND enabled))n",&[]).await.unwrap();
    let full=call(&client,&origin,"GET",&preview,None,Some(2),200).await;
    assert_eq!(full["data"]["reasons"].as_array().unwrap().len(),1000);
    // Each individual reason is valid, but the complete wire response can
    // still exceed 1 MiB. Reject before allocating/returning that JSON body.
    state.db.client().await.unwrap().execute("UPDATE platform.report_reasons SET reason=repeat('x',1500) WHERE id LIKE 'thread_overflow_%'",&[]).await.unwrap();
    let oversized_wire=call(&client,&origin,"GET",&preview,None,Some(2),503).await;
    assert!(oversized_wire.get("data").is_none());assert_eq!(counts(&state).await,before);
    state.db.client().await.unwrap().execute("UPDATE platform.report_reasons SET reason='Bounded reason '||id WHERE id LIKE 'thread_overflow_%'",&[]).await.unwrap();
    state.db.client().await.unwrap().execute("INSERT INTO platform.report_reasons(id,category,reason,details_required) VALUES('thread_overflow_extra','thread','Overflow reason',FALSE)",&[]).await.unwrap();
    let overflow=call(&client,&origin,"GET",&preview,None,Some(2),503).await;
    assert_eq!(overflow["result"],"error");assert!(overflow.get("data").is_none());
    assert_eq!(call(&client,&origin,"GET",&reply_preview,None,Some(2),200).await,context_reply);
    assert_eq!(counts(&state).await,before);
    state.db.client().await.unwrap().execute("UPDATE platform.report_reasons SET enabled=FALSE WHERE id LIKE 'thread_overflow_%'",&[]).await.unwrap();
    assert_eq!(call(&client,&origin,"GET",&preview,None,Some(2),200).await,context);
    // Native reason text uses an explicit UTF-8 byte cap, not a UTF-16 or
    // incidental character-count cap. Preserve complete multibyte/escaped text.
    state.db.client().await.unwrap().execute("INSERT INTO platform.report_reasons(id,category,reason,details_required) VALUES('thread_text_limit','thread','Initial bounded reason',FALSE)",&[]).await.unwrap();
    for reason in ["x".repeat(8000),"🌌".repeat(2000),"\u{1}".repeat(8000)] {
        state.db.client().await.unwrap().execute("UPDATE platform.report_reasons SET reason=$1 WHERE id='thread_text_limit'",&[&reason]).await.unwrap();
        let value=call(&client,&origin,"GET",&preview,None,Some(2),200).await;
        assert!(value["data"]["reasons"].as_array().unwrap().iter().any(|r|r["id"]=="thread_text_limit" && r["reason"]==reason));
    }
    for reason in ["x".repeat(8001),"🌌".repeat(2001),String::new()] {
        state.db.client().await.unwrap().execute("UPDATE platform.report_reasons SET reason=$1 WHERE id='thread_text_limit'",&[&reason]).await.unwrap();
        let value=call(&client,&origin,"GET",&preview,None,Some(2),503).await;
        assert!(value.get("data").is_none());assert_eq!(counts(&state).await,before);
    }
    // Giant persisted TEXT is never selected into the driver/JSON DTO.
    state.db.client().await.unwrap().execute("UPDATE platform.report_reasons SET reason=repeat('x',2097152) WHERE id='thread_text_limit'",&[]).await.unwrap();
    assert!(call(&client,&origin,"GET",&preview,None,Some(2),503).await.get("data").is_none());
    state.db.client().await.unwrap().execute("UPDATE platform.report_reasons SET enabled=FALSE WHERE id='thread_text_limit'",&[]).await.unwrap();
    // Id TEXT is guarded before materialization as well, and native syntax is
    // checked fail-closed instead of exposing an unreportable selection.
    for reason_id in ["x".repeat(129),"bad/reason".to_owned()] {
        state.db.client().await.unwrap().execute("INSERT INTO platform.report_reasons(id,category,reason,details_required) VALUES($1,'thread','Invalid fixture identifier',FALSE)",&[&reason_id]).await.unwrap();
        assert!(call(&client,&origin,"GET",&preview,None,Some(2),503).await.get("data").is_none());
        state.db.client().await.unwrap().execute("UPDATE platform.report_reasons SET enabled=FALSE WHERE id=$1",&[&reason_id]).await.unwrap();
    }
    assert_eq!(call(&client,&origin,"GET",&preview,None,Some(2),200).await,context);
    assert_eq!(counts(&state).await,before);
    // Captures, field presence, type and byte budget fail before private writes.
    for (field,value,expected) in [("expectedAccountId",json!(user),409),("expectedUserId",json!(actor),409),("expectedThreadId",json!(other_thread),409),("details",json!("🌌".repeat(5001)),400),("details",json!("\0"),400),("details",Value::Null,400),("details",json!(" \u{2003} "),400),("replyId",json!(reply),400),("expectedReplyVersion",json!(1),400),("targetType",json!("user"),400),("reasonId",json!("reply_spam"),409),("expectedReasonVersion",json!(0),400),("expectedReasonVersion",json!(2),409),("category",json!("source-event"),400)] {
        let mut c=command("thread_spam","valid details");c[field]=value;call(&client,&origin,"POST",&path,Some(c),Some(2),expected).await;
    }
    let mut mismatch=reply_command(1);mismatch["replyId"]=json!(wrong_reply);call(&client,&origin,"POST",&path,Some(mismatch),Some(2),404).await;
    call(&client,&origin,"POST",&format!("{path}?extra=1"),Some(command("thread_spam","query")),Some(2),400).await;
    let url=format!("{origin}{path}");let bytes=serde_json::to_vec(&command("thread_spam","signed" )).unwrap();
    for authorization in [proof_for(2,12,&url,"POST",&bytes),proof(2,&format!("{url}/other"),"POST",&bytes),proof(2,&url,"PUT",&bytes),proof(2,&url,"POST",b"{}"),"Nostr invalid".into()] {
        let response=client.post(&url).header("content-type","application/json").header("authorization",authorization).body(bytes.clone()).send().await.unwrap();assert_eq!(response.status(),401);
    }
    let duplicate=String::from_utf8(bytes.clone()).unwrap().replacen('{',"{\"targetType\":\"reply\",",1).into_bytes();
    assert_eq!(client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&duplicate)).body(duplicate).send().await.unwrap().status(),400);
    assert_eq!(client.post(&url).header("content-type","text/plain").header("authorization",proof(2,&url,"POST",&bytes)).body(bytes.clone()).send().await.unwrap().status(),400);
    assert_eq!(counts(&state).await,before);
    // Hidden targets and inactive replies cannot become report targets.
    state.db.client().await.unwrap().execute("UPDATE platform.users SET disabled=TRUE WHERE id='ReportUser4'",&[]).await.unwrap();
    call(&client,&origin,"GET",&preview,None,Some(2),404).await;call(&client,&origin,"POST",&path,Some(command("thread_spam","hidden")),Some(2),404).await;
    state.db.client().await.unwrap().execute("UPDATE platform.users SET disabled=FALSE WHERE id='ReportUser4'",&[]).await.unwrap();
    for state_name in ["pending","hidden","deleted_by_user"] {
        state.db.client().await.unwrap().execute("UPDATE platform.community_replies SET state=$2 WHERE id=$1",&[&reply,&state_name]).await.unwrap();
        call(&client,&origin,"GET",&reply_preview,None,Some(2),404).await;call(&client,&origin,"POST",&path,Some(reply_command(1)),Some(2),404).await;
    }
    state.db.client().await.unwrap().execute("UPDATE platform.community_replies SET state='active',version=2 WHERE id=$1",&[&reply]).await.unwrap();
    call(&client,&origin,"POST",&path,Some(reply_command(1)),Some(2),409).await;
    // Only the persisted profile relation counts, never a supplied thread ID.
    state.db.client().await.unwrap().execute("UPDATE platform.catalog_threads SET entity_id='NoReportProfile' WHERE id=$1",&[&thread]).await.unwrap();
    call(&client,&origin,"GET",&preview,None,Some(2),404).await;
    call(&client,&origin,"POST",&path,Some(command("thread_spam","withdrawn profile relation")),Some(2),404).await;
    state.db.client().await.unwrap().execute("UPDATE platform.catalog_threads SET entity_id='ReportUser4' WHERE id=$1",&[&thread]).await.unwrap();
    // Rebinding/admission changes cannot reuse previously reviewed captures.
    state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='ReportUser4' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    call(&client,&origin,"GET",&preview,None,Some(2),409).await;call(&client,&origin,"POST",&path,Some(command("thread_spam","rebound")),Some(2),409).await;
    state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='ReportUser2' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    original.set_account_admission(id(2),false).await.unwrap();call(&client,&origin,"POST",&path,Some(command("thread_spam","not admitted")),Some(2),403).await;original.set_account_admission(id(2),true).await.unwrap();
    // Reason text/requirements are actual persisted versions, not static categories.
    state.db.client().await.unwrap().execute("UPDATE platform.report_reasons SET reason='Changed fixture reason' WHERE id='thread_spam'",&[]).await.unwrap();
    call(&client,&origin,"POST",&path,Some(command("thread_spam","stale reason")),Some(2),409).await;
    let changed=call(&client,&origin,"GET",&preview,None,Some(2),200).await;
    assert!(changed["data"]["reasons"].as_array().unwrap().iter().any(|r| r["id"]=="thread_spam" && r["version"]==2 && r["reason"]=="Changed fixture reason"));
    state.db.client().await.unwrap().execute("UPDATE platform.report_reasons SET enabled=FALSE WHERE id='thread_incorrect'",&[]).await.unwrap();
    call(&client,&origin,"POST",&path,Some(command("thread_incorrect","disabled reason")),Some(2),409).await;
    assert_eq!(counts(&state).await,before);
    // Success is narrow, exact-body attested, same native target and moderation audit.
    let mut first=command("thread_spam","\u{2003}Synthetic thread report\u{2003}");first["expectedReasonVersion"]=json!(2);
    let receipt=call(&client,&origin,"POST",&path,Some(first.clone()),Some(2),200).await;
    let report=receipt["data"]["report"]["id"].as_str().unwrap();assert_eq!(receipt["data"],json!({"report":{"id":report,"status":"waiting"}}));
    assert_eq!(receipt["nostr"]["source"],"persisted-native-user-report");assert_eq!(receipt["nostr"]["userId"],user.as_str());
    let db=state.db.client().await.unwrap();let row=db.query_one("SELECT submitter_id,entity_type,entity_id,reason_id,body,status FROM platform.reports WHERE id=$1",&[&report]).await.unwrap();
    assert_eq!(row.get::<_,String>(0),"ReportUser2");assert_eq!(row.get::<_,String>(1),"thread");assert_eq!(row.get::<_,String>(2),thread);assert_eq!(row.get::<_,String>(3),"thread_spam");assert_eq!(row.get::<_,String>(4),"Synthetic thread report");assert_eq!(row.get::<_,String>(5),"waiting");
    let audit=db.query_one("SELECT actor_id,operation,new_status,new_version FROM platform.report_audit WHERE report_id=$1",&[&report]).await.unwrap();assert_eq!(audit.get::<_,String>(0),"ReportUser2");assert_eq!(audit.get::<_,String>(1),"create");assert_eq!(audit.get::<_,String>(2),"waiting");assert_eq!(audit.get::<_,i32>(3),1);drop(db);
    let count=counts(&state).await;call(&client,&origin,"POST",&path,Some(first),Some(2),409).await;assert_eq!(counts(&state).await,count);
    let receipt=call(&client,&origin,"POST",&path,Some(reply_command(2)),Some(2),200).await;
    let report=receipt["data"]["report"]["id"].as_str().unwrap();let row=state.db.client().await.unwrap().query_one("SELECT entity_type,entity_id FROM platform.reports WHERE id=$1",&[&report]).await.unwrap();assert_eq!(row.get::<_,String>(0),"reply");assert_eq!(row.get::<_,String>(1),reply.to_string());
    // Rust trim and maximum UTF-8 bytes, plus worst JSON escaping under exact budget.
    call(&client,&origin,"POST",&path,Some(command("thread_optional",&format!("\u{2003}{}\u{2003}","🌌".repeat(5000)))),Some(2),200).await;
    let escaped=command("thread_escaped",&"\u{1}".repeat(20000));assert!(serde_json::to_vec(&escaped).unwrap().len()>120000);
    call(&client,&origin,"POST",&path,Some(escaped),Some(2),200).await;
    let huge=vec![b' ';131073];let response=client.post(&url).header("content-type","application/json").body(huge).send().await.unwrap();assert_eq!(response.status(),413);assert_eq!(response.headers()["cache-control"],"private, no-store");
    assert!(response.headers().get_all("vary").iter().any(|v|v.to_str().unwrap().contains("Cookie")));
    for other in ["/api/report".to_owned(),path.replace(alias.as_str(),"native1234"),format!("{path}/context")] {
        assert_eq!(client.post(format!("{origin}{other}")).body(vec![b' ';1025]).send().await.unwrap().status(),413);
    }
    // Cookies alone never authorize reports, while native cookie reports share duplicates.
    use sha2::{Digest,Sha256};let token="synthetic-reports-cookie";let hash=Sha256::digest(token.as_bytes()).to_vec();
    state.db.client().await.unwrap().execute("INSERT INTO platform.sessions(id,token_hash,user_id,expires_at) VALUES('ReportSession2',$1,'ReportUser2',clock_timestamp()+interval '1 hour')",&[&hash]).await.unwrap();let cookie=format!("{}={token}",state.config.session_cookie_name);
    assert_eq!(client.post(&url).header("cookie",&cookie).json(&command("thread_native","cookie only")).send().await.unwrap().status(),401);
    assert_eq!(client.get(format!("{origin}{preview}")).header("cookie",&cookie).send().await.unwrap().status(),401);
    assert_eq!(client.post(&url).header("cookie",&cookie).header("authorization","Nostr invalid").json(&command("thread_native","invalid proof with cookie")).send().await.unwrap().status(),401);
    assert_eq!(client.get(format!("{origin}{preview}")).header("cookie",&cookie).header("authorization","Nostr invalid").send().await.unwrap().status(),401);
    let c=command("thread_native","native duplicate");call(&client,&origin,"POST",&path,Some(c),Some(2),200).await;
    assert_eq!(client.post(format!("{origin}/api/report")).header("cookie",&cookie).header("origin",&origin).json(&json!({"entity_type":"thread","entity_id":thread,"reason_id":"thread_native","details":"native duplicate"})).send().await.unwrap().status(),409);
    // NIP-98 replay is not a second mutation.
    let c=command("thread_other","Replay guarded");let bytes=serde_json::to_vec(&c).unwrap();let authorization=proof(2,&url,"POST",&bytes);
    assert_eq!(client.post(&url).header("content-type","application/json").header("authorization",&authorization).body(bytes.clone()).send().await.unwrap().status(),200);
    assert_eq!(client.post(&url).header("content-type","application/json").header("authorization",&authorization).body(bytes).send().await.unwrap().status(),401);
    // A core insert failure rolls back both report and audit.
    let before=counts(&state).await;
    state.db.client().await.unwrap().batch_execute("CREATE FUNCTION platform.report_fixture_rollback() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.reason_id='thread_rollback' THEN RAISE EXCEPTION 'synthetic report rollback'; END IF; RETURN NEW; END $$; CREATE TRIGGER report_fixture_rollback AFTER INSERT ON platform.reports FOR EACH ROW EXECUTE FUNCTION platform.report_fixture_rollback();").await.unwrap();
    call(&client,&origin,"POST",&path,Some(command("thread_rollback","rollback")),Some(2),503).await;
    assert_eq!(counts(&state).await,before);
    state.db.client().await.unwrap().batch_execute("DROP TRIGGER report_fixture_rollback ON platform.reports; DROP FUNCTION platform.report_fixture_rollback();").await.unwrap();
    // Every target lock is NOWAIT, and must fail before any report/audit writes.
    for sql in ["SELECT id FROM platform.users WHERE id='ReportUser4' FOR NO KEY UPDATE".to_owned(),format!("SELECT id FROM platform.catalog_threads WHERE id='{thread}' FOR UPDATE"),format!("SELECT id FROM platform.community_replies WHERE id={reply} FOR UPDATE")] {
        let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one(&sql,&[]).await.unwrap();
        let c=if sql.contains("community_replies") {reply_command(2)} else {command("thread_barrier","contended")};
        tokio::time::timeout(Duration::from_secs(2),call(&client,&origin,"POST",&path,Some(c),Some(2),409)).await.expect("contended report target must not wait");
        assert_eq!(counts(&state).await,before);block.rollback().await.unwrap();
    }
    // Native permission withdrawal while the real final actor lock waits.
    let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one("SELECT id FROM platform.users WHERE id='ReportUser2' FOR NO KEY UPDATE",&[]).await.unwrap();
    let bytes=serde_json::to_vec(&command("thread_barrier","revoked permission")).unwrap();let request=client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&bytes)).body(bytes);let waiting=tokio::spawn(async move{request.send().await.unwrap()});
    wait_for_lock(&state,"%FOR SHARE OF b FOR NO KEY UPDATE OF u%").await;
    block.execute("UPDATE platform.users SET permissions='[]' WHERE id='ReportUser2'",&[]).await.unwrap();block.commit().await.unwrap();assert_eq!(waiting.await.unwrap().status(),403);drop(blocker);
    state.db.client().await.unwrap().execute("UPDATE platform.users SET permissions=$1 WHERE id='ReportUser2'",&[&normal]).await.unwrap();assert_eq!(counts(&state).await,before);
    // Same-signer rebinding while its actual binding lock waits also refuses.
    let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one("SELECT publisher FROM platform.protocol_identity_bindings WHERE publisher=$1 FOR UPDATE",&[&public(2)]).await.unwrap();
    let bytes=serde_json::to_vec(&command("thread_barrier","binding moved")).unwrap();let request=client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&bytes)).body(bytes);let waiting=tokio::spawn(async move{request.send().await.unwrap()});
    wait_for_lock(&state,"%FOR SHARE OF b FOR NO KEY UPDATE OF u%").await;
    assert_eq!(block.execute("UPDATE platform.protocol_identity_bindings SET user_id='ReportUser4' WHERE publisher=$1",&[&public(2)]).await.unwrap(),1);block.commit().await.unwrap();
    // The blocked joined-row recheck loses its original binding/user match and
    // has no bound actor, rather than a valid actor with a stale captured ID.
    let refused=waiting.await.unwrap();let status=refused.status().as_u16();let refused:Value=refused.json().await.unwrap();
    assert_eq!(status,403,"{refused}");assert_eq!(refused["result"],"error");assert_eq!(refused["errors"][0]["detail"],"An explicit native account binding is required");assert!(refused.get("data").is_none());drop(blocker);
    assert_eq!(state.db.client().await.unwrap().query_one("SELECT user_id FROM platform.protocol_identity_bindings WHERE publisher=$1",&[&public(2)]).await.unwrap().get::<_,String>(0),"ReportUser4");assert_eq!(counts(&state).await,before);
    // A subsequent query sees the now-stable new actor and the same captured
    // old account is rejected by the distinct exact account fence with 409.
    let mismatch=call(&client,&origin,"POST",&path,Some(command("thread_barrier","stable binding moved")),Some(2),409).await;
    assert_eq!(mismatch["errors"][0]["detail"],"Account binding changed; refresh before reporting");assert!(mismatch.get("data").is_none());assert_eq!(counts(&state).await,before);
    state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='ReportUser2' WHERE publisher=$1",&[&public(2)]).await.unwrap();assert_eq!(counts(&state).await,before);
    // Real reciprocal A→B / B→A requests cannot deadlock on actor upgrades.
    let reverse_path=format!("/api/v1/nostr/user/{actor}/discussion/reports");
    let reverse=json!({"expectedAccountId":user,"expectedUserId":actor,"expectedThreadId":other_thread,"targetType":"thread","reasonId":"thread_reciprocal","expectedReasonVersion":1,"details":"Reciprocal B to A"});
    let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one("SELECT id FROM platform.users WHERE id='ReportUser2' FOR NO KEY UPDATE",&[]).await.unwrap();
    let bytes=serde_json::to_vec(&command("thread_reciprocal","Reciprocal A to B")).unwrap();let request=client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&bytes)).body(bytes);let waiting=tokio::spawn(async move{request.send().await.unwrap()});
    wait_for_lock(&state,"%FOR SHARE OF b FOR NO KEY UPDATE OF u%").await;
    tokio::time::timeout(Duration::from_secs(2),call(&client,&origin,"POST",&reverse_path,Some(reverse),Some(4),409)).await.expect("reciprocal report must not deadlock");
    assert_eq!(counts(&state).await,before);block.commit().await.unwrap();drop(blocker);assert_eq!(waiting.await.unwrap().status(),200);assert_eq!(counts(&state).await,(before.0+1,before.1+1));
    // Actual cookie-native mutation owns the thread before inserting its audit
    // actor FK. The signed report must refuse without waiting for that thread.
    state.db.client().await.unwrap().batch_execute("CREATE FUNCTION platform.report_fixture_native_barrier() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.body='Native report lock control' THEN PERFORM pg_advisory_xact_lock(620251); END IF; RETURN NEW; END $$; CREATE TRIGGER report_fixture_native_barrier BEFORE INSERT ON platform.community_replies FOR EACH ROW EXECUTE FUNCTION platform.report_fixture_native_barrier();").await.unwrap();
    let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one("SELECT pg_advisory_xact_lock(620251)",&[]).await.unwrap();
    let native=client.post(format!("{origin}/api/thread/{thread}/replies")).header("cookie",&cookie).header("origin",&origin).json(&json!({"body":"Native report lock control"}));let native_waiting=tokio::spawn(async move{native.send().await.unwrap()});
    wait_for_advisory(&state,"%INSERT INTO platform.community_replies%").await;
    let before=counts(&state).await;let native_audit:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get(0);
    tokio::time::timeout(Duration::from_secs(2),call(&client,&origin,"POST",&path,Some(command("thread_barrier","same actor native-cookie collision")),Some(2),409)).await.expect("native cookie thread / actor FK must not form a deadlock");
    // Prove the refusal occurred while the real native INSERT is still blocked.
    wait_for_advisory(&state,"%INSERT INTO platform.community_replies%").await;
    assert!(!native_waiting.is_finished(),"native competitor ended before signed refusal");
    assert_eq!(counts(&state).await,before);assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get::<_,i64>(0),native_audit);
    block.commit().await.unwrap();drop(blocker);assert_eq!(native_waiting.await.unwrap().status(),201);
    assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get::<_,i64>(0),native_audit+1);
    let committed=state.db.client().await.unwrap().query("SELECT author_id,version,state FROM platform.community_replies WHERE thread_id=$1 AND body='Native report lock control'",&[&thread]).await.unwrap();
    assert_eq!(committed.len(),1);assert_eq!(committed[0].get::<_,String>(0),"ReportUser2");
    assert_eq!(committed[0].get::<_,i32>(1),1);assert_eq!(committed[0].get::<_,String>(2),"active");
    assert_eq!(counts(&state).await,before);
    state.db.client().await.unwrap().batch_execute("DROP TRIGGER report_fixture_native_barrier ON platform.community_replies; DROP FUNCTION platform.report_fixture_native_barrier();").await.unwrap();
    // The reviewed reason lock remains held THROUGH the shared core insert.
    state.db.client().await.unwrap().batch_execute("CREATE FUNCTION platform.report_fixture_reason_barrier() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.reason_id='thread_reason_lock' THEN PERFORM pg_advisory_xact_lock(620252); END IF; RETURN NEW; END $$; CREATE TRIGGER report_fixture_reason_barrier BEFORE INSERT ON platform.reports FOR EACH ROW EXECUTE FUNCTION platform.report_fixture_reason_barrier();").await.unwrap();
    let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one("SELECT pg_advisory_xact_lock(620252)",&[]).await.unwrap();
    let bytes=serde_json::to_vec(&command("thread_reason_lock","reviewed reason")).unwrap();let request=client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&bytes)).body(bytes);let waiting=tokio::spawn(async move{request.send().await.unwrap()});
    wait_for_advisory(&state,"%INSERT INTO platform.reports%").await;
    let changer=state.db.clone();let change=tokio::spawn(async move {changer.client().await.unwrap().execute("UPDATE platform.report_reasons SET reason='New reason after submit' WHERE id='thread_reason_lock'",&[]).await.unwrap()});
    wait_for_lock(&state,"%UPDATE platform.report_reasons SET reason='New reason after submit'%").await;assert!(!change.is_finished());
    block.commit().await.unwrap();drop(blocker);assert_eq!(waiting.await.unwrap().status(),200);assert_eq!(change.await.unwrap(),1);
    state.db.client().await.unwrap().batch_execute("DROP TRIGGER report_fixture_reason_barrier ON platform.reports; DROP FUNCTION platform.report_fixture_reason_barrier();").await.unwrap();
    let before=counts(&state).await;call(&client,&origin,"POST",&path,Some(command("thread_reason_lock","old reviewed version")),Some(2),409).await;assert_eq!(counts(&state).await,before);
    // Simultaneous duplicate submissions share actor serialization and core rule.
    let a=serde_json::to_vec(&command("thread_concurrent","Parallel A")).unwrap();let b=serde_json::to_vec(&command("thread_concurrent","Parallel B")).unwrap();
    let (a,b)=tokio::join!(client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&a)).body(a).send(),client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&b)).body(b).send());let mut statuses=[a.unwrap().status().as_u16(),b.unwrap().status().as_u16()];statuses.sort();assert_eq!(statuses,[200,409]);
    assert_eq!(counts(&state).await,(before.0+1,before.1+1));
    // Native twenty-per-hour rule is preserved, including already reviewed rows.
    state.db.client().await.unwrap().execute("INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,body,status) SELECT 'ReportRate'||n,'ReportUser7','thread',$1,'thread_spam','synthetic rate fixture','refused' FROM generate_series(1,20)n",&[&thread]).await.unwrap();
    let before=counts(&state).await;let mut limited=command("thread_optional","rate limited");limited["expectedAccountId"]=json!(rate_actor);
    call(&client,&origin,"POST",&path,Some(limited),Some(7),429).await;assert_eq!(counts(&state).await,before);
    assert!(!runtime.join("unused/operator.key").exists());assert!(!runtime.join("unused/service.key").exists());
}
