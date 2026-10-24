//! Signed native typed catalogue discussion identity, permissions, CAS, privacy and rollback seams.
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
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires isolated scripts/run-auth-tests.sh PostgreSQL"]
async fn signed_native_discussions_bind_real_targets_and_visibility_witnesses() {
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
    let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:8,uploads_dir:runtime.join("media"),ui_dir:runtime.join("no-ui"),protocol_publication:Some(OperatorRuntimeConfig{operator_key_file:runtime.join("unused/operator.key"),expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),storage:json!({"path":runtime.join("unused/storage"),"volume":"genericdiscussionfixture","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144}),group_keys_dir:None,backend:None,native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":runtime.join("unused/service.key"),"admin_pubkey":public(1)}))}),..Config::default()},dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();state.protocol.verify_schema().await.unwrap();
    let normal=json!(["reply.create","self.reply.update","self.reply.remove"]);
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
    let aliases:i64=db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);drop(db);
    let app=weebdex_platform::router::router(state.clone());let _server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap();}));
    let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build().unwrap();
    for (kind,native,resource) in [("manga","GenManga01",ResourceKind::Manga),("chapter","GenChap001",ResourceKind::Chapter),("group","GenGroup01",ResourceKind::Group)] {
        let target=resolver.preferred(resource,native).await.unwrap().unwrap();let thread=format!("GenericThread{kind}");
        let path=format!("/api/v1/nostr/{kind}/{target}/discussion");
        let preview=format!("{path}?expectedAccountId={actor}&expectedThreadId={thread}");
        let command=|operation:&str,fields:Value,account:&PublicUuid| {let mut c=json!({"expectedAccountId":account,"expectedTargetId":target,"expectedThreadId":thread,"operation":operation});c.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());c};
        let create=|body:&str|command("create",json!({"body":body}),&actor);
        let mapped=call(&client,&origin,"GET",&format!("/api/v1/nostr/native-forum/threads/{thread}/context"),None,None,200).await;
        assert_eq!(mapped["data"],json!({"kind":kind,"id":target,"threadId":thread}));
        call(&client,&origin,"GET",&preview,None,None,401).await;
        let first=call(&client,&origin,"GET",&preview,None,Some(2),200).await;
        assert_eq!(first["nostr"],json!({"scope":"instance-account","source":"persisted-native-discussion","identity":"native_catalogue_resource","accountId":actor,"targetKind":kind,"targetId":target,"threadId":thread}));
        assert!(!contains_private_profile_fields(&first));
        for (field,value) in [("expectedAccountId",json!(other)),("expectedTargetId",json!(actor)),("expectedThreadId",json!("UnrelatedThread"))] {
            let mut bad=create("Wrong binding");bad[field]=value;call(&client,&origin,"POST",&path,Some(bad),Some(2),409).await;
        }
        let mut wrong_type=create("Wrong target type");wrong_type["expectedUserId"]=json!(other);call(&client,&origin,"POST",&path,Some(wrong_type),Some(2),400).await;
        let r=call(&client,&origin,"POST",&path,Some(create("  Actual **reply**  ")),Some(2),200).await;
        let reply=r["data"]["reply"]["id"].as_i64().unwrap();assert_eq!(r["data"]["reply"]["body"],"Actual **reply**");
        call(&client,&origin,"POST",&path,Some(command("update",json!({"replyId":reply,"version":1,"body":"Edited"}),&actor)),Some(2),200).await;
        call(&client,&origin,"POST",&path,Some(command("update",json!({"replyId":reply,"version":1,"body":"Stale"}),&actor)),Some(2),409).await;
        call(&client,&origin,"POST",&path,Some(command("update",json!({"replyId":reply,"version":2,"body":"Not owner"}),&other)),Some(4),403).await;
        call(&client,&origin,"POST",&path,Some(command("reply_vote",json!({"replyId":reply,"version":2,"vote":1,"expectedVote":0}),&other)),Some(4),200).await;
        call(&client,&origin,"POST",&path,Some(command("reply_vote",json!({"replyId":reply,"version":1,"vote":-1,"expectedVote":1}),&other)),Some(4),409).await;
        call(&client,&origin,"POST",&path,Some(command("thread_vote",json!({"vote":1,"expectedVote":0}),&actor)),Some(2),200).await;
        state.db.client().await.unwrap().execute("UPDATE platform.catalog_threads SET locked=TRUE WHERE id=$1",&[&thread]).await.unwrap();
        call(&client,&origin,"POST",&path,Some(create("Locked")),Some(2),403).await;
        call(&client,&origin,"POST",&path,Some(command("thread_vote",json!({"vote":0,"expectedVote":1}),&actor)),Some(2),200).await;
        call(&client,&origin,"POST",&path,Some(command("reply_vote",json!({"replyId":reply,"version":2,"vote":0,"expectedVote":1}),&other)),Some(4),200).await;
        state.db.client().await.unwrap().execute("UPDATE platform.catalog_threads SET locked=FALSE WHERE id=$1",&[&thread]).await.unwrap();
        call(&client,&origin,"POST",&path,Some(command("remove",json!({"replyId":reply,"version":2}),&actor)),Some(2),200).await;
        call(&client,&origin,"POST",&path,Some(command("delete",json!({"replyId":reply,"version":3}),&actor)),Some(2),403).await;
        call(&client,&origin,"POST",&path,Some(command("delete",json!({"replyId":reply,"version":3}),&moderator)),Some(6),200).await;
        // Different cookie B and signer A remain distinct; cookie alone is not authority.
        use sha2::{Digest,Sha256};let token=format!("generic-cookie-{kind}");let hash=Sha256::digest(token.as_bytes()).to_vec();
        state.db.client().await.unwrap().execute("INSERT INTO platform.sessions(id,token_hash,user_id,expires_at) VALUES($1,$2,'GenericForumUser4',clock_timestamp()+interval '1 hour')",&[&format!("GenericSession{kind}"),&hash]).await.unwrap();
        let cookie=format!("{}={token}",state.config.session_cookie_name);let bytes=serde_json::to_vec(&create("Signer A cookie B")).unwrap();let url=format!("{origin}{path}");
        assert_eq!(client.post(&url).header("cookie",&cookie).json(&create("Cookie only")).send().await.unwrap().status(),401);
        let response=client.post(&url).header("cookie",&cookie).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&bytes)).body(bytes).send().await.unwrap();assert_eq!(response.status(),200);
        let signed:Value=response.json().await.unwrap();assert_eq!(signed["data"]["reply"]["relationships"]["author"]["id"],"GenericForumUser2");
        let native_response=client.post(format!("{origin}/api/thread/{thread}/replies")).header("cookie",&cookie).header("origin",&origin).json(&json!({"body":"Actual browser B"})).send().await.unwrap();assert_eq!(native_response.status(),201);let native_reply:Value=native_response.json().await.unwrap();assert_eq!(native_reply["relationships"]["author"]["id"],"GenericForumUser4");
        // Every target/witness contention is known before mutation/audit.
        let target_sql=match kind {"manga"=>"SELECT id FROM platform.catalog_manga WHERE id='GenManga01' FOR UPDATE","chapter"=>"SELECT id FROM platform.catalog_chapters WHERE id='GenChap001' FOR UPDATE",_=>"SELECT id FROM platform.catalog_groups WHERE id='GenGroup01' FOR UPDATE"};
        for sql in [target_sql,"SELECT id FROM platform.catalog_threads WHERE id=$1 FOR UPDATE"] {
            let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();
            if sql.contains("$1") {block.query_one(sql,&[&thread]).await.unwrap();}else{block.query_one(sql,&[]).await.unwrap();}
            let before:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get(0);
            tokio::time::timeout(Duration::from_secs(2),call(&client,&origin,"POST",&path,Some(create("Contested")),Some(2),409)).await.unwrap();
            assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get::<_,i64>(0),before);block.rollback().await.unwrap();
        }
    }
    assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get::<_,i64>(0),aliases);
    // Actual native-cookie mutation holds the real thread lock. The signed
    // route must refuse without waiting/cycling, then the native write completes.
    state.db.client().await.unwrap().batch_execute("CREATE FUNCTION platform.generic_discussion_gate() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.body='Wait actual native' THEN PERFORM pg_advisory_xact_lock(842971233); END IF; RETURN NEW; END $$; CREATE TRIGGER generic_discussion_gate BEFORE INSERT ON platform.community_replies FOR EACH ROW EXECUTE FUNCTION platform.generic_discussion_gate();").await.unwrap();
    let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one("SELECT pg_advisory_xact_lock(842971233)",&[]).await.unwrap();
    let native_request=client.post(format!("{origin}/api/thread/GenericThreadmanga/replies")).header("cookie",format!("{}=generic-cookie-manga",state.config.session_cookie_name)).header("origin",&origin).json(&json!({"body":"Wait actual native"}));
    let native_waiting=tokio::spawn(async move{native_request.send().await.unwrap()});wait_for_lock(&state,"INSERT INTO platform.community_replies%").await;
    let target=resolver.preferred(ResourceKind::Manga,"GenManga01").await.unwrap().unwrap();
    call(&client,&origin,"POST",&format!("/api/v1/nostr/manga/{target}/discussion"),Some(json!({"expectedAccountId":actor,"expectedTargetId":target,"expectedThreadId":"GenericThreadmanga","operation":"create","body":"No reciprocal lock cycle"})),Some(2),409).await;
    block.commit().await.unwrap();drop(blocker);assert_eq!(native_waiting.await.unwrap().status(),201);
    state.db.client().await.unwrap().batch_execute("DROP TRIGGER generic_discussion_gate ON platform.community_replies; DROP FUNCTION platform.generic_discussion_gate();").await.unwrap();
    // Introduce a genuine stored imported group witness and source generation.
    // All data is synthetic and confined to this private disposable DB.
    let db=state.db.client().await.unwrap();
    db.batch_execute("INSERT INTO capacity_reservations(id,requested,actual,state) VALUES(decode(repeat('41',32),'hex'),1,1,'settled'); INSERT INTO blob_intents(hash,generation,volume,owner,size,state,reservation,verified_at) VALUES(decode(repeat('42',32),'hex'),decode(repeat('43',16),'hex'),'generic-fixture',decode(repeat('44',32),'hex'),1,'available',decode(repeat('41',32),'hex'),clock_timestamp()); INSERT INTO events(id,publisher,kind,created_at,raw_event,state,manifest) VALUES(decode(repeat('45',32),'hex'),decode(repeat('44',32),'hex'),1002,1,'{}','hosted',decode(repeat('42',32),'hex')); INSERT INTO publication_blobs(event_id,hash,generation,mime) VALUES(decode(repeat('45',32),'hex'),decode(repeat('42',32),'hex'),decode(repeat('43',16),'hex'),'application/cbor'); UPDATE platform.catalog_chapters SET protocol_event_id=repeat('45',32) WHERE id='GenChap001'; INSERT INTO platform.protocol_resource_ids(kind,source_target,compatibility_id,catalog_id) VALUES('group','synthetic-real-stored-group','eeeeeeee-eeee-8eee-8eee-eeeeeeeeeeee','GenGroup01');").await.unwrap();drop(db);
    let source_aliases:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);
    for (kind,native,resource) in [("chapter","GenChap001",ResourceKind::Chapter),("group","GenGroup01",ResourceKind::Group)] {
        let target=resolver.preferred(resource,native).await.unwrap().unwrap();let thread=format!("GenericThread{kind}");let path=format!("/api/v1/nostr/{kind}/{target}/discussion");let preview=format!("{path}?expectedAccountId={actor}&expectedThreadId={thread}");
        let command=||json!({"expectedAccountId":actor,"expectedTargetId":target,"expectedThreadId":thread,"operation":"create","body":"Witness-bound reply"});
        call(&client,&origin,"GET",&preview,None,Some(2),200).await;
        for sql in ["SELECT id FROM public.events WHERE id=decode(repeat('45',32),'hex') FOR UPDATE","SELECT hash FROM public.blob_intents WHERE hash=decode(repeat('42',32),'hex') FOR UPDATE","SELECT id FROM platform.catalog_manga WHERE id='GenManga01' FOR UPDATE"] {
            let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one(sql,&[]).await.unwrap();
            let before:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get(0);
            call(&client,&origin,"POST",&path,Some(command()),Some(2),409).await;
            assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get::<_,i64>(0),before);block.rollback().await.unwrap();
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
            let before:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get(0);
            let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one(lock,&[]).await.unwrap();
            call(&client,&origin,"POST",&path,Some(command()),Some(2),409).await;block.batch_execute(withdraw).await.unwrap();block.commit().await.unwrap();drop(blocker);
            call(&client,&origin,"POST",&path,Some(command()),Some(2),404).await;
            assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get::<_,i64>(0),before);
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
    // Adapter never creates aliases, reports, protocol events or private files.
    assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get::<_,i64>(0),source_aliases);
    assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.reports",&[]).await.unwrap().get::<_,i64>(0),0);
    assert!(!runtime.join("unused/operator.key").exists());assert!(!runtime.join("unused/service.key").exists());
}
