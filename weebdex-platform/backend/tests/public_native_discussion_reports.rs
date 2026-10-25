//! Written HTTP/concurrency controls for typed native report creation.
//! NOT EXECUTED in the bounded 2026-10-22 source/Node-only qualification.
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
// Public thread subjects legitimately contain "profile"; inspect field names,
// not arbitrary public text, when excluding native account/profile objects.
fn contains_private_profile_fields(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields.iter().any(|(key, value)|
            ["profile", "email", "password_hash", "private_key"].contains(&key.as_str())
                || contains_private_profile_fields(value)),
        Value::Array(values) => values.iter().any(contains_private_profile_fields),
        _ => false,
    }
}
#[test]
fn privacy_field_control_allows_stored_subject_but_refuses_nested_account_fields() {
    assert!(!contains_private_profile_fields(&json!({"thread":{"subject":"GenericForumUser4 profile discussion"}})));
    for key in ["profile", "email", "password_hash", "private_key"] {
        let mut field = serde_json::Map::new(); field.insert(key.into(), json!("synthetic private fixture"));
        assert!(contains_private_profile_fields(&json!({"data":[Value::Object(field)]})));
    }
}
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
        let count:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE $1",&[&pattern]).await.unwrap().get(0);
        if count>0{return;} tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("native operation did not reach its exact controlled lock");
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
async fn signed_native_reports_bind_real_targets_and_visibility_witnesses() {
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
    let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:10,maximum_json_bytes:1024,uploads_dir:runtime.join("media"),ui_dir:runtime.join("no-ui"),protocol_publication:Some(OperatorRuntimeConfig{operator_key_file:runtime.join("unused/operator.key"),expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),storage:json!({"path":runtime.join("unused/storage"),"volume":"genericdiscussionfixture","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144}),group_keys_dir:None,backend:None,native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":runtime.join("unused/service.key"),"admin_pubkey":public(1)}))}),..Config::default()},dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();state.protocol.verify_schema().await.unwrap();
    let normal=json!(["report.create","reply.create","self.reply.update","self.reply.remove"]);
    let moderator=json!(["reply.create","reply.update","reply.remove","reply.delete"]);
    let db=state.db.client().await.unwrap();
    for key in [2,4,5,6,7,8] {
        let user=format!("GenericForumUser{key}");let roles=if key>=6 {json!(["moderator"])} else {json!(["user"])};
        let perms=if key==6 {moderator.clone()} else if key==7 {json!([])} else {normal.clone()};
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,disabled,roles,permissions,profile) VALUES($1,$1,$2,'synthetic',TRUE,$3,$4,$5,$6)",&[&user,&format!("discussion{key}@fixture.invalid"),&false,&roles,&perms,&json!({"description":"Public-ish profile","email":"private-profile@fixture.invalid","private_key":"synthetic-private-value"})]).await.unwrap();
        db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await.unwrap();
        db.execute("UPDATE platform.users SET disabled=$2 WHERE id=$1",&[&user,&(key==5)]).await.unwrap();
    }

    state.protocol.install_visibility_function().await.unwrap();
    let resolver=PostgresPublicIdResolver::new(state.db.clone());
    let actor=resolver.preferred(ResourceKind::User,"GenericForumUser2").await.unwrap().unwrap();
    let other=resolver.preferred(ResourceKind::User,"GenericForumUser4").await.unwrap().unwrap();
    let moderator=resolver.preferred(ResourceKind::User,"GenericForumUser6").await.unwrap().unwrap();
    db.batch_execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES('GenManga01','GenericForumUser4','Owner','Generic fixture title','ja','ongoing','safe','published'); INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,language) VALUES('GenChap001','GenManga01','GenericForumUser4','Owner','en'); INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES('GenGroup01','GenericForumUser4','Owner','Generic fixture group'); INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES('GenChap001','GenGroup01');").await.unwrap();
    for (kind,native) in [("manga","GenManga01"),("chapter","GenChap001"),("group","GenGroup01")] {
        db.execute("INSERT INTO platform.catalog_threads(id,entity_type,entity_id,author_id,author_name,subject) VALUES($1,$2,$3,'GenericForumUser4','Owner','Actual native discussion')", &[&format!("GenericThread{kind}"),&kind,&native]).await.unwrap();
    }
    for reason in ["thread_gen_optional","thread_gen_escaped","thread_gen_concurrent","thread_gen_rollback","thread_gen_barrier","thread_gen_native","thread_gen_reason_lock","thread_gen_source"] {
        db.execute("INSERT INTO platform.report_reasons(id,category,reason,details_required) VALUES($1,'thread',$1,FALSE)",&[&reason]).await.unwrap();
    }
    let aliases:i64=db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);drop(db);
    let app=weebdex_platform::router::router(state.clone());let _server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap();}));
    let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build().unwrap();
    for (kind,native,resource) in [("manga","GenManga01",ResourceKind::Manga),("chapter","GenChap001",ResourceKind::Chapter),("group","GenGroup01",ResourceKind::Group)] {
        let target=resolver.preferred(resource,native).await.unwrap().unwrap();let thread=format!("GenericThread{kind}");
        let path=format!("/api/v1/nostr/{kind}/{target}/discussion/reports");let url=format!("{origin}{path}");
        let preview=format!("{path}/context?expectedAccountId={actor}&expectedThreadId={thread}&targetType=thread");
        let command=|reason:&str,details:&str|json!({"expectedAccountId":actor,"expectedTargetId":target,"expectedThreadId":thread,"targetType":"thread","reasonId":reason,"expectedReasonVersion":1,"details":details});
        let reply=state.db.client().await.unwrap().query_one("INSERT INTO platform.community_replies(thread_id,reply_number,author_id,body) VALUES($1,1,'GenericForumUser4','Synthetic native report reply') RETURNING id",&[&thread]).await.unwrap().get::<_,i64>(0);
        state.db.client().await.unwrap().execute("UPDATE platform.catalog_threads SET next_reply_number=2 WHERE id=$1",&[&thread]).await.unwrap();
        let reply_command=|version:i32|{let mut c=command("reply_spam","Synthetic reply report");c["targetType"]=json!("reply");c["replyId"]=json!(reply);c["expectedReplyVersion"]=json!(version);c};
        let before=counts(&state).await;
        call(&client,&origin,"GET",&preview,None,None,401).await;
        for key in [3,5,8] { call(&client,&origin,"GET",&preview,None,Some(key),403).await; }
        call(&client,&origin,"GET",&preview.replace(actor.as_str(),moderator.as_str()),None,Some(6),403).await;
        let mut denied=command("thread_spam","No report grant");denied["expectedAccountId"]=json!(moderator);call(&client,&origin,"POST",&path,Some(denied),Some(6),403).await;
        for suffix in ["&limit=20","&targetType=reply","&replyId=1","&expectedReplyVersion=1","&expectedAccountId=null"] {call(&client,&origin,"GET",&format!("{preview}{suffix}"),None,Some(2),400).await;}
        call(&client,&origin,"GET",&format!("{path}/context"),None,Some(2),400).await;
        call(&client,&origin,"GET",&preview,Some(json!({})),Some(2),400).await;
        let context=call(&client,&origin,"GET",&preview,None,Some(2),200).await;
        assert_eq!(context["nostr"],json!({"scope":"instance-account","source":"persisted-native-discussion-report","identity":"native_catalogue_resource","accountId":actor,"targetKind":kind,"targetId":target,"threadId":thread}));
        assert!(!contains_private_profile_fields(&context));
        for (field,value) in [("expectedAccountId",json!(other)),("expectedTargetId",json!(actor)),("expectedThreadId",json!("OtherStoredThread"))] {
            let mut bad=command("thread_spam","Wrong tuple");bad[field]=value;call(&client,&origin,"POST",&path,Some(bad),Some(2),409).await;
        }
        for (field,value) in [("expectedUserId",json!(other)),("targetType",json!(kind)),("replyId",json!(reply)),("expectedReasonVersion",json!(0)),("details",json!("🌌".repeat(5001))),("details",json!("\0"))] {
            let mut bad=command("thread_spam","Invalid shape");bad[field]=value;call(&client,&origin,"POST",&path,Some(bad),Some(2),400).await;
        }
        let bytes=serde_json::to_vec(&command("thread_spam","Duplicate key")).unwrap();
        let duplicate=String::from_utf8(bytes.clone()).unwrap().replacen('{',"{\"targetType\":\"reply\",",1).into_bytes();
        assert_eq!(client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&duplicate)).body(duplicate).send().await.unwrap().status(),400);
        assert_eq!(client.post(&url).header("content-type","text/plain").header("authorization",proof(2,&url,"POST",&bytes)).body(bytes.clone()).send().await.unwrap().status(),400);
        call(&client,&origin,"POST",&path,Some(reply_command(2)),Some(2),409).await;
        let mut off_page=reply_command(1);off_page["replyId"]=json!(2147483647);call(&client,&origin,"POST",&path,Some(off_page),Some(2),404).await;
        for state_name in ["pending","hidden","deleted_by_user"] {
            state.db.client().await.unwrap().execute("UPDATE platform.community_replies SET state=$2 WHERE id=$1",&[&reply,&state_name]).await.unwrap();
            call(&client,&origin,"POST",&path,Some(reply_command(1)),Some(2),404).await;
        }
        state.db.client().await.unwrap().execute("UPDATE platform.community_replies SET state='active' WHERE id=$1",&[&reply]).await.unwrap();
        // Real stored mappings, not a publisher-synthesized thread, are required.
        state.db.client().await.unwrap().execute("UPDATE platform.catalog_threads SET entity_id='NoSuchTarget' WHERE id=$1",&[&thread]).await.unwrap();
        call(&client,&origin,"POST",&path,Some(command("thread_spam","No mapping")),Some(2),404).await;
        state.db.client().await.unwrap().execute("UPDATE platform.catalog_threads SET entity_id=$2 WHERE id=$1",&[&thread,&native]).await.unwrap();
        let target_sql=match kind {"manga"=>"SELECT id FROM platform.catalog_manga WHERE id='GenManga01' FOR UPDATE","chapter"=>"SELECT id FROM platform.catalog_chapters WHERE id='GenChap001' FOR UPDATE",_=>"SELECT id FROM platform.catalog_groups WHERE id='GenGroup01' FOR UPDATE"};
        for sql in [target_sql,"SELECT id FROM platform.catalog_threads WHERE id=$1 FOR UPDATE","SELECT public_id FROM platform.public_resource_ids WHERE public_id=$1::text::uuid FOR UPDATE","SELECT id FROM platform.community_replies WHERE id=$1 FOR UPDATE"] {
            let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();
            if sql.contains("public_id") {block.query_one(sql,&[&target.as_str()]).await.unwrap();}else if sql.contains("community_replies"){block.query_one(sql,&[&reply]).await.unwrap();}else if sql.contains("$1"){block.query_one(sql,&[&thread]).await.unwrap();}else{block.query_one(sql,&[]).await.unwrap();}
            tokio::time::timeout(Duration::from_secs(2),call(&client,&origin,"POST",&path,Some(reply_command(1)),Some(2),409)).await.unwrap();assert_eq!(counts(&state).await,before);block.rollback().await.unwrap();
        }
        assert_eq!(counts(&state).await,before);
        let receipt=call(&client,&origin,"POST",&path,Some(command("thread_spam","\u{2003}Synthetic thread report\u{2003}")),Some(2),200).await;
        let report=receipt["data"]["report"]["id"].as_str().unwrap();
        let row=state.db.client().await.unwrap().query_one("SELECT r.submitter_id,r.entity_type,r.entity_id,r.body,a.actor_id,a.operation FROM platform.reports r JOIN platform.report_audit a ON a.report_id=r.id WHERE r.id=$1",&[&report]).await.unwrap();
        assert_eq!(row.get::<_,String>(0),"GenericForumUser2");assert_eq!(row.get::<_,String>(1),"thread");assert_eq!(row.get::<_,String>(2),thread);assert_eq!(row.get::<_,String>(3),"Synthetic thread report");assert_eq!(row.get::<_,String>(4),"GenericForumUser2");assert_eq!(row.get::<_,String>(5),"create");
        let before=counts(&state).await;call(&client,&origin,"POST",&path,Some(command("thread_spam","Duplicate")),Some(2),409).await;assert_eq!(counts(&state).await,before);
        call(&client,&origin,"POST",&path,Some(reply_command(1)),Some(2),200).await;
    }
    let target=resolver.preferred(ResourceKind::Manga,"GenManga01").await.unwrap().unwrap();let thread="GenericThreadmanga";
    let path=format!("/api/v1/nostr/manga/{target}/discussion/reports");let url=format!("{origin}{path}");let preview=format!("{path}/context?expectedAccountId={actor}&expectedThreadId={thread}&targetType=thread");
    let command=|reason:&str,details:&str|json!({"expectedAccountId":actor,"expectedTargetId":target,"expectedThreadId":thread,"targetType":"thread","reasonId":reason,"expectedReasonVersion":1,"details":details});
    // Strict body boundary does not alter default or protocol-source endpoints.
    call(&client,&origin,"POST",&path,Some(command("thread_gen_optional",&format!("\u{2003}{}\u{2003}","🌌".repeat(5000)))),Some(2),200).await;
    call(&client,&origin,"POST",&path,Some(command("thread_gen_escaped",&"\u{1}".repeat(20000))),Some(2),200).await;
    let huge=vec![b'x';131073];let response=client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&huge)).body(huge).send().await.unwrap();assert_eq!(response.status(),413);assert!(response.headers()["cache-control"].to_str().unwrap().contains("private"));
    let before=counts(&state).await;
    // Actor permission and binding are checked after the real blocking lock.
    let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one("SELECT id FROM platform.users WHERE id='GenericForumUser2' FOR NO KEY UPDATE",&[]).await.unwrap();
    let bytes=serde_json::to_vec(&command("thread_gen_barrier","permission withdrawal")).unwrap();let request=client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&bytes)).body(bytes);let waiting=tokio::spawn(async move{request.send().await.unwrap()});wait_for_lock(&state,"%FOR SHARE OF b FOR NO KEY UPDATE OF u%").await;
    block.execute("UPDATE platform.users SET permissions='[]' WHERE id='GenericForumUser2'",&[]).await.unwrap();block.commit().await.unwrap();drop(blocker);assert_eq!(waiting.await.unwrap().status(),403);
    state.db.client().await.unwrap().execute("UPDATE platform.users SET permissions=$1 WHERE id='GenericForumUser2'",&[&normal]).await.unwrap();
    let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one("SELECT publisher FROM platform.protocol_identity_bindings WHERE publisher=$1 FOR UPDATE",&[&public(2)]).await.unwrap();
    let bytes=serde_json::to_vec(&command("thread_gen_barrier","binding replacement")).unwrap();let request=client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&bytes)).body(bytes);let waiting=tokio::spawn(async move{request.send().await.unwrap()});wait_for_lock(&state,"%FOR SHARE OF b FOR NO KEY UPDATE OF u%").await;
    block.execute("UPDATE platform.protocol_identity_bindings SET user_id='GenericForumUser4' WHERE publisher=$1",&[&public(2)]).await.unwrap();block.commit().await.unwrap();drop(blocker);assert_eq!(waiting.await.unwrap().status(),403);
    call(&client,&origin,"GET",&preview,None,Some(2),409).await;call(&client,&origin,"POST",&path,Some(command("thread_gen_barrier","stale binding")),Some(2),409).await;
    state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='GenericForumUser2' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    original.set_account_admission(id(2),false).await.unwrap();call(&client,&origin,"POST",&path,Some(command("thread_gen_barrier","withdrawn admission")),Some(2),403).await;original.set_account_admission(id(2),true).await.unwrap();assert_eq!(counts(&state).await,before);
    // Cookie authority never substitutes for explicitly selected signed actor A.
    use sha2::{Digest,Sha256};let token="synthetic-generic-reports-cookie";let hash=Sha256::digest(token.as_bytes()).to_vec();
    state.db.client().await.unwrap().execute("INSERT INTO platform.sessions(id,token_hash,user_id,expires_at) VALUES('GenericReportSession2',$1,'GenericForumUser2',clock_timestamp()+interval '1 hour')",&[&hash]).await.unwrap();let cookie=format!("{}={token}",state.config.session_cookie_name);
    assert_eq!(client.post(&url).header("cookie",&cookie).json(&command("thread_gen_native","cookie only")).send().await.unwrap().status(),401);
    assert_eq!(client.get(format!("{origin}{preview}")).header("cookie",&cookie).send().await.unwrap().status(),401);
    assert_eq!(client.post(&url).header("cookie",&cookie).header("authorization","Nostr invalid").json(&command("thread_gen_native","invalid proof")).send().await.unwrap().status(),401);
    let token_b="synthetic-generic-reports-cookie-b";let hash_b=Sha256::digest(token_b.as_bytes()).to_vec();
    state.db.client().await.unwrap().execute("INSERT INTO platform.sessions(id,token_hash,user_id,expires_at) VALUES('GenericReportSession4',$1,'GenericForumUser4',clock_timestamp()+interval '1 hour')",&[&hash_b]).await.unwrap();
    let bytes=serde_json::to_vec(&command("thread_gen_native","Signed A with cookie B")).unwrap();let cookie_b=format!("{}={token_b}",state.config.session_cookie_name);
    let response=client.post(&url).header("cookie",&cookie_b).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&bytes)).body(bytes).send().await.unwrap();assert_eq!(response.status(),200);let receipt:Value=response.json().await.unwrap();let report=receipt["data"]["report"]["id"].as_str().unwrap();
    assert_eq!(state.db.client().await.unwrap().query_one("SELECT submitter_id FROM platform.reports WHERE id=$1",&[&report]).await.unwrap().get::<_,String>(0),"GenericForumUser2");
    assert_eq!(client.post(format!("{origin}/api/report")).header("cookie",&cookie).header("origin",&origin).json(&json!({"entity_type":"thread","entity_id":thread,"reason_id":"thread_gen_native","details":"Native duplicate"})).send().await.unwrap().status(),409);
    // A core insert failure rolls back both report and audit.
    let before=counts(&state).await;
    state.db.client().await.unwrap().batch_execute("CREATE FUNCTION platform.report_fixture_rollback() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.reason_id='thread_gen_rollback' THEN RAISE EXCEPTION 'synthetic report rollback'; END IF; RETURN NEW; END $$; CREATE TRIGGER report_fixture_rollback AFTER INSERT ON platform.reports FOR EACH ROW EXECUTE FUNCTION platform.report_fixture_rollback();").await.unwrap();
    call(&client,&origin,"POST",&path,Some(command("thread_gen_rollback","rollback")),Some(2),503).await;
    assert_eq!(counts(&state).await,before);
    state.db.client().await.unwrap().batch_execute("DROP TRIGGER report_fixture_rollback ON platform.reports; DROP FUNCTION platform.report_fixture_rollback();").await.unwrap();
    // Actual cookie-native mutation owns the thread before inserting its audit
    // actor FK. The signed report must refuse without waiting for that thread.
    state.db.client().await.unwrap().batch_execute("CREATE FUNCTION platform.report_fixture_native_barrier() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.body='Native report lock control' THEN PERFORM pg_advisory_xact_lock(620251); END IF; RETURN NEW; END $$; CREATE TRIGGER report_fixture_native_barrier BEFORE INSERT ON platform.community_replies FOR EACH ROW EXECUTE FUNCTION platform.report_fixture_native_barrier();").await.unwrap();
    let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one("SELECT pg_advisory_xact_lock(620251)",&[]).await.unwrap();
    let native=client.post(format!("{origin}/api/thread/{thread}/replies")).header("cookie",&cookie).header("origin",&origin).json(&json!({"body":"Native report lock control"}));let native_waiting=tokio::spawn(async move{native.send().await.unwrap()});
    wait_for_advisory(&state,"%INSERT INTO platform.community_replies%").await;
    let before=counts(&state).await;let native_audit:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get(0);
    tokio::time::timeout(Duration::from_secs(2),call(&client,&origin,"POST",&path,Some(command("thread_gen_barrier","same actor native-cookie collision")),Some(2),409)).await.expect("native cookie thread / actor FK must not form a deadlock");
    // Prove the refusal occurred while the real native INSERT is still blocked.
    wait_for_advisory(&state,"%INSERT INTO platform.community_replies%").await;
    assert!(!native_waiting.is_finished(),"native competitor ended before signed refusal");
    assert_eq!(counts(&state).await,before);assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get::<_,i64>(0),native_audit);
    block.commit().await.unwrap();drop(blocker);assert_eq!(native_waiting.await.unwrap().status(),201);
    assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get::<_,i64>(0),native_audit+1);
    let committed=state.db.client().await.unwrap().query("SELECT author_id,version,state FROM platform.community_replies WHERE thread_id=$1 AND body='Native report lock control'",&[&thread]).await.unwrap();
    assert_eq!(committed.len(),1);assert_eq!(committed[0].get::<_,String>(0),"GenericForumUser2");
    assert_eq!(committed[0].get::<_,i32>(1),1);assert_eq!(committed[0].get::<_,String>(2),"active");
    assert_eq!(counts(&state).await,before);
    state.db.client().await.unwrap().batch_execute("DROP TRIGGER report_fixture_native_barrier ON platform.community_replies; DROP FUNCTION platform.report_fixture_native_barrier();").await.unwrap();
    // The reviewed reason lock remains held THROUGH the shared core insert.
    state.db.client().await.unwrap().batch_execute("CREATE FUNCTION platform.report_fixture_reason_barrier() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.reason_id='thread_gen_reason_lock' THEN PERFORM pg_advisory_xact_lock(620252); END IF; RETURN NEW; END $$; CREATE TRIGGER report_fixture_reason_barrier BEFORE INSERT ON platform.reports FOR EACH ROW EXECUTE FUNCTION platform.report_fixture_reason_barrier();").await.unwrap();
    let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one("SELECT pg_advisory_xact_lock(620252)",&[]).await.unwrap();
    let bytes=serde_json::to_vec(&command("thread_gen_reason_lock","reviewed reason")).unwrap();let request=client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&bytes)).body(bytes);let waiting=tokio::spawn(async move{request.send().await.unwrap()});
    wait_for_advisory(&state,"%INSERT INTO platform.reports%").await;
    let changer=state.db.clone();let change=tokio::spawn(async move {changer.client().await.unwrap().execute("UPDATE platform.report_reasons SET reason='New reason after submit' WHERE id='thread_gen_reason_lock'",&[]).await.unwrap()});
    wait_for_lock(&state,"%UPDATE platform.report_reasons SET reason='New reason after submit'%").await;assert!(!change.is_finished());
    block.commit().await.unwrap();drop(blocker);assert_eq!(waiting.await.unwrap().status(),200);assert_eq!(change.await.unwrap(),1);
    state.db.client().await.unwrap().batch_execute("DROP TRIGGER report_fixture_reason_barrier ON platform.reports; DROP FUNCTION platform.report_fixture_reason_barrier();").await.unwrap();
    let before=counts(&state).await;call(&client,&origin,"POST",&path,Some(command("thread_gen_reason_lock","old reviewed version")),Some(2),409).await;assert_eq!(counts(&state).await,before);
    // Simultaneous duplicate submissions share actor serialization and core rule.
    let a=serde_json::to_vec(&command("thread_gen_concurrent","Parallel A")).unwrap();let b=serde_json::to_vec(&command("thread_gen_concurrent","Parallel B")).unwrap();
    let (a,b)=tokio::join!(client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&a)).body(a).send(),client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&b)).body(b).send());let mut statuses=[a.unwrap().status().as_u16(),b.unwrap().status().as_u16()];statuses.sort();assert_eq!(statuses,[200,409]);
    assert_eq!(counts(&state).await,(before.0+1,before.1+1));
    // Introduce a genuine stored imported group witness and source generation.
    // All data is synthetic and confined to this private disposable DB.
    let db=state.db.client().await.unwrap();
    db.batch_execute("INSERT INTO capacity_reservations(id,requested,actual,state) VALUES(decode(repeat('41',32),'hex'),1,1,'settled'); INSERT INTO blob_intents(hash,generation,volume,owner,size,state,reservation,verified_at) VALUES(decode(repeat('42',32),'hex'),decode(repeat('43',16),'hex'),'generic-fixture',decode(repeat('44',32),'hex'),1,'available',decode(repeat('41',32),'hex'),clock_timestamp()); INSERT INTO events(id,publisher,kind,created_at,raw_event,state,manifest) VALUES(decode(repeat('45',32),'hex'),decode(repeat('44',32),'hex'),1002,1,'{}','hosted',decode(repeat('42',32),'hex')); INSERT INTO publication_blobs(event_id,hash,generation,mime) VALUES(decode(repeat('45',32),'hex'),decode(repeat('42',32),'hex'),decode(repeat('43',16),'hex'),'application/cbor'); UPDATE platform.catalog_chapters SET protocol_event_id=repeat('45',32) WHERE id='GenChap001'; INSERT INTO platform.protocol_resource_ids(kind,source_target,compatibility_id,catalog_id) VALUES('group','synthetic-real-stored-group','eeeeeeee-eeee-8eee-8eee-eeeeeeeeeeee','GenGroup01');").await.unwrap();drop(db);
    let source_aliases:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);
    for (kind,native,resource) in [("chapter","GenChap001",ResourceKind::Chapter),("group","GenGroup01",ResourceKind::Group)] {
        let target=resolver.preferred(resource,native).await.unwrap().unwrap();let thread=format!("GenericThread{kind}");let path=format!("/api/v1/nostr/{kind}/{target}/discussion/reports");let preview=format!("{path}/context?expectedAccountId={actor}&expectedThreadId={thread}&targetType=thread");
        let command=||json!({"expectedAccountId":actor,"expectedTargetId":target,"expectedThreadId":thread,"targetType":"thread","reasonId":"thread_gen_source","expectedReasonVersion":1,"details":"Witness-bound report"});
        call(&client,&origin,"GET",&preview,None,Some(2),200).await;
        for sql in ["SELECT id FROM public.events WHERE id=decode(repeat('45',32),'hex') FOR UPDATE","SELECT hash FROM public.blob_intents WHERE hash=decode(repeat('42',32),'hex') FOR UPDATE","SELECT id FROM platform.catalog_manga WHERE id='GenManga01' FOR UPDATE"] {
            let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one(sql,&[]).await.unwrap();
            let before=counts(&state).await;
            call(&client,&origin,"POST",&path,Some(command()),Some(2),409).await;
            assert_eq!(counts(&state).await,before);block.rollback().await.unwrap();
        }
        if kind=="group" {
            let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one("SELECT chapter_id FROM platform.catalog_chapter_groups WHERE group_id='GenGroup01' FOR UPDATE",&[]).await.unwrap();call(&client,&origin,"POST",&path,Some(command()),Some(2),409).await;block.execute("DELETE FROM platform.catalog_chapter_groups WHERE group_id='GenGroup01'",&[]).await.unwrap();block.commit().await.unwrap();
            call(&client,&origin,"POST",&path,Some(command()),Some(2),404).await;state.db.client().await.unwrap().batch_execute("INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES('GenChap001','GenGroup01');").await.unwrap();
        }
        // Contended source/parent writers refuse first, then their committed
        // withdrawal becomes invisible, including a previously absent negative row.
        for (lock,withdraw,restore) in [
            ("SELECT id FROM platform.catalog_manga WHERE id='GenManga01' FOR UPDATE","UPDATE platform.catalog_manga SET deleted_at=clock_timestamp() WHERE id='GenManga01'","UPDATE platform.catalog_manga SET deleted_at=NULL WHERE id='GenManga01'"),
            ("SELECT id FROM events WHERE id=decode(repeat('45',32),'hex') FOR UPDATE","UPDATE events SET state='withdrawing' WHERE id=decode(repeat('45',32),'hex'); INSERT INTO backend_outbox(event_id,action,payload,event_version) VALUES(decode(repeat('45',32),'hex'),'delete','{}',1)","UPDATE events SET state='hosted' WHERE id=decode(repeat('45',32),'hex'); UPDATE backend_outbox SET completed_at=clock_timestamp() WHERE event_id=decode(repeat('45',32),'hex')"),
            ("SELECT hash FROM blob_intents WHERE hash=decode(repeat('42',32),'hex') FOR UPDATE","INSERT INTO quarantined_blobs(hash,generation,reason) VALUES(decode(repeat('42',32),'hex'),decode(repeat('43',16),'hex'),'audit_failed')","DELETE FROM quarantined_blobs WHERE hash=decode(repeat('42',32),'hex')")
        ] {
            let before=counts(&state).await;
            let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one(lock,&[]).await.unwrap();
            call(&client,&origin,"POST",&path,Some(command()),Some(2),409).await;block.batch_execute(withdraw).await.unwrap();block.commit().await.unwrap();drop(blocker);
            call(&client,&origin,"POST",&path,Some(command()),Some(2),404).await;
            assert_eq!(counts(&state).await,before);
            state.db.client().await.unwrap().batch_execute(restore).await.unwrap();
        }
        // A merely hosted event is insufficient if its stored manifest witness,
        // reference membership or available exact blob generation is missing.
        for (corrupt,restore) in [
            ("UPDATE events SET manifest=decode(repeat('46',32),'hex') WHERE id=decode(repeat('45',32),'hex')","UPDATE events SET manifest=decode(repeat('42',32),'hex') WHERE id=decode(repeat('45',32),'hex')"),
            ("DELETE FROM publication_blobs WHERE event_id=decode(repeat('45',32),'hex')","INSERT INTO publication_blobs(event_id,hash,generation,mime) VALUES(decode(repeat('45',32),'hex'),decode(repeat('42',32),'hex'),decode(repeat('43',16),'hex'),'application/cbor')"),
            ("UPDATE blob_intents SET state='writing',verified_at=NULL WHERE hash=decode(repeat('42',32),'hex')","UPDATE blob_intents SET state='available',verified_at=clock_timestamp() WHERE hash=decode(repeat('42',32),'hex')")
        ] {state.db.client().await.unwrap().batch_execute(corrupt).await.unwrap();call(&client,&origin,"POST",&path,Some(command()),Some(2),404).await;state.db.client().await.unwrap().batch_execute(restore).await.unwrap();}
        call(&client,&origin,"POST",&path,Some(command()),Some(2),200).await;
    }
    // Native 20-per-hour limit counts reviewed rows; no moderation action is added.
    state.db.client().await.unwrap().execute("INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,body,status) SELECT 'GenericReportRate'||n,'GenericForumUser4','thread',$1,'thread_spam','synthetic rate fixture','refused' FROM generate_series(1,20)n",&[&thread]).await.unwrap();
    let before=counts(&state).await;let mut limited=command("thread_gen_native","rate limited");limited["expectedAccountId"]=json!(other);call(&client,&origin,"POST",&path,Some(limited),Some(4),429).await;assert_eq!(counts(&state).await,before);
    assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get::<_,i64>(0),source_aliases);
    assert!(!runtime.join("unused/operator.key").exists());assert!(!runtime.join("unused/service.key").exists());
}
