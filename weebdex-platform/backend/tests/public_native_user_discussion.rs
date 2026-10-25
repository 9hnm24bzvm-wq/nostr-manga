//! Signed native-user discussion identity, permissions, CAS, privacy and rollback seams.
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
    assert!(!contains_private_profile_fields(&json!({"thread":{"subject":"DiscussionUser4 profile discussion"}})));
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
        let count: i64 = state.db.client().await.unwrap().query_one("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE $1", &[&pattern]).await.unwrap().get(0);
        if count > 0 { return; }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("request did not wait on expected final authority lock: {pattern}");
}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires isolated scripts/run-auth-tests.sh PostgreSQL"]
async fn signed_native_user_discussion_uses_real_core_and_fails_closed() {
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
    let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:8,uploads_dir:runtime.join("media"),ui_dir:runtime.join("no-ui"),protocol_publication:Some(OperatorRuntimeConfig{operator_key_file:runtime.join("unused/operator.key"),expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),storage:json!({"path":runtime.join("unused/storage"),"volume":"discussionfixture","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144}),group_keys_dir:None,backend:None,native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":runtime.join("unused/service.key"),"admin_pubkey":public(1)}))}),..Config::default()},dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();state.protocol.verify_schema().await.unwrap();
    let normal=json!(["reply.create","self.reply.update","self.reply.remove"]);
    let moderator=json!(["reply.create","reply.update","reply.remove","reply.delete"]);
    let db=state.db.client().await.unwrap();
    for key in [2,4,5,6,7,8] {
        let user=format!("DiscussionUser{key}");let roles=if key>=6 {json!(["moderator"])} else {json!(["user"])};
        let perms=if key==6 {moderator.clone()} else if key==7 {json!([])} else {normal.clone()};
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,disabled,roles,permissions,profile) VALUES($1,$1,$2,'synthetic',TRUE,$3,$4,$5,$6)",&[&user,&format!("discussion{key}@fixture.invalid"),&false,&roles,&perms,&json!({"description":"Public-ish profile","email":"private-profile@fixture.invalid","private_key":"synthetic-private-value"})]).await.unwrap();
        db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await.unwrap();
        db.execute("UPDATE platform.users SET disabled=$2 WHERE id=$1",&[&user,&(key==5)]).await.unwrap();
    }
    let resolver=PostgresPublicIdResolver::new(state.db.clone());
    let actor=resolver.preferred(ResourceKind::User,"DiscussionUser2").await.unwrap().unwrap();
    let user=resolver.preferred(ResourceKind::User,"DiscussionUser4").await.unwrap().unwrap();
    let mod_id=resolver.preferred(ResourceKind::User,"DiscussionUser6").await.unwrap().unwrap();
    let limited=resolver.preferred(ResourceKind::User,"DiscussionUser7").await.unwrap().unwrap();
    let alias=PublicUuid::parse("aaaaaaaa-aaaa-8aaa-8aaa-aaaaaaaaaaaa").unwrap();resolver.register_alias(ResourceKind::User,"DiscussionUser4",&alias).await.unwrap();
    let publisher:String=db.query_one("SELECT platform.public_register_protocol_publisher($1)::text",&[&public(4)]).await.unwrap().get(0);
    let thread:String=db.query_one("SELECT id FROM platform.catalog_threads WHERE entity_type='user' AND entity_id='DiscussionUser4'",&[]).await.unwrap().get(0);
    let other_thread:String=db.query_one("SELECT id FROM platform.catalog_threads WHERE entity_type='user' AND entity_id='DiscussionUser2'",&[]).await.unwrap().get(0);
    let aliases:i64=db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);drop(db);
    let app=weebdex_platform::router::router(state.clone());let _server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap();}));
    let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build().unwrap();
    let path=format!("/api/v1/nostr/user/{alias}/discussion");
    let preview=format!("{path}?expectedAccountId={actor}&expectedThreadId={thread}");
    let command=|operation:&str,fields:Value,account:&PublicUuid| {let mut c=json!({"expectedAccountId":account,"expectedUserId":user,"expectedThreadId":thread,"operation":operation});c.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());c};
    let create=|body:&str|command("create",json!({"body":body}),&actor);
    call(&client,&origin,"GET",&preview,None,None,401).await;
    for key in [3,5,8] {call(&client,&origin,"GET",&preview,None,Some(key),403).await;}
    for suffix in ["&limit=20","&page=0","&page=1000001","&order=sideways","&page=1&page=2","&expectedThreadId=wrong"] {call(&client,&origin,"GET",&format!("{preview}{suffix}"),None,Some(2),400).await;}
    call(&client,&origin,"GET",&path,None,Some(2),400).await;
    call(&client,&origin,"GET",&preview,Some(json!({})),Some(2),400).await;
    call(&client,&origin,"GET",&format!("{path}?expectedAccountId={user}&expectedThreadId={thread}"),None,Some(2),409).await;
    call(&client,&origin,"GET",&format!("{path}?expectedAccountId={actor}&expectedThreadId={other_thread}"),None,Some(2),409).await;
    let publisher_path=format!("/api/v1/nostr/user/{publisher}/discussion");
    call(&client,&origin,"GET",&format!("{publisher_path}?expectedAccountId={actor}&expectedThreadId={thread}"),None,Some(2),404).await;
    call(&client,&origin,"POST",&publisher_path,Some(create("publisher cannot own a native thread")),Some(2),404).await;
    let first=call(&client,&origin,"GET",&preview,None,Some(2),200).await;
    assert_eq!(first["nostr"],json!({"scope":"instance-account","source":"persisted-native-user-discussion","identity":"native_public_user","accountId":actor,"userId":user,"threadId":thread}));
    assert_eq!(first["data"]["thread"]["id"],thread);assert_eq!(first["data"]["feed"],json!({"total":0,"limit":20,"page":1,"data":[]}));
    assert_eq!(first["data"]["actions"]["create"],true);assert_eq!(first["data"]["actions"]["threadVote"],true);
    assert!(!first.to_string().contains("private"));assert!(!first.to_string().contains("fixture.invalid"));assert!(!contains_private_profile_fields(&first));assert!(!first.to_string().contains("Public-ish profile"));
    let reply=call(&client,&origin,"POST",&path,Some(create("\u{2003}First reply 🌌\u{2003}")),Some(2),200).await;
    let r=reply["data"]["reply"]["id"].as_i64().unwrap();assert_eq!(reply["data"]["reply"]["body"],"First reply 🌌");assert_eq!(reply["nostr"]["changed"],true);
    let updated=call(&client,&origin,"POST",&path,Some(command("update",json!({"replyId":r,"version":1,"body":"Edited reply"}),&actor)),Some(2),200).await;
    assert_eq!(updated["data"]["reply"]["version"],2);
    call(&client,&origin,"POST",&path,Some(command("update",json!({"replyId":r,"version":1,"body":"stale"}),&actor)),Some(2),409).await;
    call(&client,&origin,"POST",&path,Some(command("update",json!({"replyId":r,"version":2,"body":"not your reply"}),&user)),Some(4),403).await;
    call(&client,&origin,"POST",&path,Some(command("delete",json!({"replyId":r,"version":2}),&actor)),Some(2),403).await;
    call(&client,&origin,"POST",&path,Some(command("delete",json!({"replyId":r,"version":2}),&limited)),Some(7),403).await;
    // Every input is signed, exact, operation-specific, and tied to all captures.
    for (field,value) in [("expectedAccountId",json!(user)),("expectedUserId",json!(actor)),("expectedThreadId",json!(other_thread))] {
        let mut c=create("mismatch");c[field]=value;call(&client,&origin,"POST",&path,Some(c),Some(2),409).await;
    }
    for (field,value) in [("body",json!("🌌".repeat(12501))),("body",json!("\0")),("body",json!(" \n ")),("parentId",json!(-1)),("parentId",Value::Null),("replyId",json!(r)),("version",json!(1)),("email",json!("forged"))] {
        let mut c=create("valid");c[field]=value;call(&client,&origin,"POST",&path,Some(c),Some(2),400).await;
    }
    call(&client,&origin,"POST",&preview,Some(create("query not accepted")),Some(2),400).await;
    let mut wrong_parent=create("wrong parent");wrong_parent["parentId"]=json!(2147483647_i64);call(&client,&origin,"POST",&path,Some(wrong_parent),Some(2),400).await;
    let mut child=create("child reply");child["parentId"]=json!(r);let child=call(&client,&origin,"POST",&path,Some(child),Some(2),200).await;
    let child_id=child["data"]["reply"]["id"].as_i64().unwrap();
    let own_vote=command("reply_vote",json!({"replyId":r,"version":2,"vote":1,"expectedVote":0}),&actor);
    call(&client,&origin,"POST",&path,Some(own_vote),Some(2),403).await;
    let voter=|vote:i16,expected:i16,version:i32|command("reply_vote",json!({"replyId":r,"version":version,"vote":vote,"expectedVote":expected}),&user);
    let voted=call(&client,&origin,"POST",&path,Some(voter(1,0,2)),Some(4),200).await;assert_eq!(voted["data"]["reply"]["relationships"]["vote_state"],1);
    call(&client,&origin,"POST",&path,Some(voter(-1,0,2)),Some(4),409).await;
    call(&client,&origin,"POST",&path,Some(voter(-1,1,1)),Some(4),409).await;
    let same=call(&client,&origin,"POST",&path,Some(voter(1,1,2)),Some(4),200).await;assert_eq!(same["nostr"]["changed"],false);
    call(&client,&origin,"POST",&path,Some(voter(-1,1,2)),Some(4),200).await;
    let thread_vote=|vote:i16,expected:i16|command("thread_vote",json!({"vote":vote,"expectedVote":expected}),&actor);
    call(&client,&origin,"POST",&path,Some(command("thread_vote",json!({"vote":1,"expectedVote":0}),&user)),Some(4),403).await;
    call(&client,&origin,"POST",&path,Some(thread_vote(1,0)),Some(2),200).await;
    call(&client,&origin,"POST",&path,Some(thread_vote(-1,0)),Some(2),409).await;
    let personal=call(&client,&origin,"GET",&preview,None,Some(2),200).await;
    assert_eq!(personal["data"]["threadVote"],1);assert_eq!(personal["data"]["actions"]["replies"][r.to_string()]["update"],true);assert_eq!(personal["data"]["actions"]["replies"][r.to_string()]["vote"],false);
    // Thread lock blocks create/edit/votes, but moderator edit and native vote clearing remain available.
    state.db.client().await.unwrap().execute("UPDATE platform.catalog_threads SET locked=TRUE WHERE id=$1",&[&thread]).await.unwrap();
    call(&client,&origin,"POST",&path,Some(create("locked")),Some(2),403).await;
    call(&client,&origin,"POST",&path,Some(command("update",json!({"replyId":r,"version":2,"body":"locked"}),&actor)),Some(2),403).await;
    call(&client,&origin,"POST",&path,Some(thread_vote(-1,1)),Some(2),403).await;
    call(&client,&origin,"POST",&path,Some(thread_vote(0,1)),Some(2),200).await;
    call(&client,&origin,"POST",&path,Some(voter(0,-1,2)),Some(4),200).await;
    let moderated=call(&client,&origin,"POST",&path,Some(command("update",json!({"replyId":r,"version":2,"body":"Moderator edit"}),&mod_id)),Some(6),200).await;
    assert_eq!(moderated["data"]["reply"]["version"],3);
    call(&client,&origin,"POST",&path,Some(command("remove",json!({"replyId":r,"version":2}),&actor)),Some(2),409).await;
    let removed=call(&client,&origin,"POST",&path,Some(command("remove",json!({"replyId":r,"version":3}),&actor)),Some(2),200).await;
    assert_eq!(removed["data"]["reply"]["state"],"deleted_by_user");assert_eq!(removed["data"]["reply"]["version"],4);assert!(removed["data"]["reply"].get("body").is_none());assert!(removed["data"]["reply"]["relationships"].get("author").is_none());
    assert_eq!(call(&client,&origin,"POST",&path,Some(command("remove",json!({"replyId":r,"version":4}),&actor)),Some(2),200).await["nostr"]["changed"],false);
    call(&client,&origin,"POST",&path,Some(command("delete",json!({"replyId":r,"version":3}),&mod_id)),Some(6),409).await;
    call(&client,&origin,"POST",&path,Some(command("delete",json!({"replyId":r,"version":4}),&mod_id)),Some(6),200).await;
    let db=state.db.client().await.unwrap();let row=db.query_one("SELECT parent_id,reply_number FROM platform.community_replies WHERE id=$1",&[&child_id]).await.unwrap();assert_eq!(row.get::<_,Option<i64>>(0),None);assert_eq!(row.get::<_,i32>(1),1);drop(db);
    let mod_removed=call(&client,&origin,"POST",&path,Some(command("remove",json!({"replyId":child_id,"version":1}),&mod_id)),Some(6),200).await;assert_eq!(mod_removed["data"]["reply"]["state"],"deleted_by_moderator");
    state.db.client().await.unwrap().execute("UPDATE platform.catalog_threads SET locked=FALSE WHERE id=$1",&[&thread]).await.unwrap();
    // Cookies do not confer signed authority, and genuine native edits share the same CAS/audit.
    use sha2::{Digest,Sha256};let token="synthetic-discussion-cookie";let hash=Sha256::digest(token.as_bytes()).to_vec();
    state.db.client().await.unwrap().execute("INSERT INTO platform.sessions(id,token_hash,user_id,expires_at) VALUES('DiscussionSession2',$1,'DiscussionUser2',clock_timestamp()+interval '1hour')",&[&hash]).await.unwrap();let cookie=format!("{}={token}",state.config.session_cookie_name);
    assert_eq!(client.post(format!("{origin}{path}")).header("cookie",&cookie).json(&create("cookie only")).send().await.unwrap().status(),401);
    assert_eq!(client.get(format!("{origin}{preview}")).header("cookie",&cookie).send().await.unwrap().status(),401);
    let native=client.post(format!("{origin}/api/thread/{thread}/replies")).header("cookie",&cookie).header("origin",&origin).json(&json!({"body":"Native reply"})).send().await.unwrap();assert_eq!(native.status(),201);let native:Value=native.json().await.unwrap();let native_id=native["id"].as_i64().unwrap();
    call(&client,&origin,"POST",&path,Some(command("update",json!({"replyId":native_id,"version":1,"body":"Signed edit of native reply"}),&actor)),Some(2),200).await;
    assert_eq!(client.put(format!("{origin}/api/thread/{thread}/replies/{native_id}")).header("cookie",&cookie).header("origin",&origin).json(&json!({"version":1,"body":"Stale native edit"})).send().await.unwrap().status(),409);
    // The exact URL, method, service, payload and signer are original admission inputs.
    let url=format!("{origin}{path}");let bytes=serde_json::to_vec(&create("signed once")).unwrap();
    for authorization in [proof_for(2,12,&url,"POST",&bytes),proof(2,&format!("{url}/other"),"POST",&bytes),proof(2,&url,"PUT",&bytes),proof(2,&url,"POST",b"{}"),"Nostr invalid".into()] {
        let response=client.post(&url).header("content-type","application/json").header("authorization",authorization).body(bytes.clone()).send().await.unwrap();assert_eq!(response.status(),401);
    }
    let authorization=proof(2,&url,"POST",&bytes);
    assert_eq!(client.post(&url).header("content-type","application/json").header("authorization",&authorization).body(bytes.clone()).send().await.unwrap().status(),200);
    assert_eq!(client.post(&url).header("content-type","application/json").header("authorization",&authorization).body(bytes).send().await.unwrap().status(),401);
    // Same signer rebound to another eligible native account cannot reuse captures.
    state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='DiscussionUser4' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    call(&client,&origin,"GET",&preview,None,Some(2),409).await;call(&client,&origin,"POST",&path,Some(create("rebound")),Some(2),409).await;
    state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='DiscussionUser2' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    original.set_account_admission(id(2),false).await.unwrap();call(&client,&origin,"POST",&path,Some(create("denied admission")),Some(2),403).await;original.set_account_admission(id(2),true).await.unwrap();
    // Rolled-back insert must not consume an ordinal or emit a successful audit.
    let snapshot=call(&client,&origin,"GET",&preview,None,Some(2),200).await;
    let audit_before:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get(0);
    state.db.client().await.unwrap().batch_execute("CREATE FUNCTION platform.discussion_fixture_rollback() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.body='rollback' THEN RAISE EXCEPTION 'synthetic rollback'; END IF; RETURN NEW; END $$; CREATE TRIGGER discussion_fixture_rollback AFTER INSERT ON platform.community_replies FOR EACH ROW EXECUTE FUNCTION platform.discussion_fixture_rollback();").await.unwrap();
    call(&client,&origin,"POST",&path,Some(create("rollback")),Some(2),503).await;
    assert_eq!(call(&client,&origin,"GET",&preview,None,Some(2),200).await,snapshot);
    state.db.client().await.unwrap().batch_execute("DROP TRIGGER discussion_fixture_rollback ON platform.community_replies; DROP FUNCTION platform.discussion_fixture_rollback();").await.unwrap();
    assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get::<_,i64>(0),audit_before);
    // Withdraw native permissions while the final bound-account lock is pending.
    let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one("SELECT id FROM platform.users WHERE id='DiscussionUser2' FOR NO KEY UPDATE",&[]).await.unwrap();
    let bytes=serde_json::to_vec(&create("withdrawn grant")).unwrap();let request=client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&bytes)).body(bytes);let waiting=tokio::spawn(async move{request.send().await.unwrap()});
    wait_for_lock(&state,"%FOR SHARE OF b FOR NO KEY UPDATE OF u%").await;
    block.execute("UPDATE platform.users SET permissions='[]' WHERE id='DiscussionUser2'",&[]).await.unwrap();block.commit().await.unwrap();assert_eq!(waiting.await.unwrap().status(),403);drop(blocker);
    state.db.client().await.unwrap().execute("UPDATE platform.users SET permissions=$1 WHERE id='DiscussionUser2'",&[&normal]).await.unwrap();
    // A contested target row is a known pre-mutation 409, never a deadlock.
    let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one("SELECT id FROM platform.users WHERE id='DiscussionUser4' FOR NO KEY UPDATE",&[]).await.unwrap();
    let before:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get(0);
    call(&client,&origin,"POST",&path,Some(create("contested target")),Some(2),409).await;
    assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get::<_,i64>(0),before);
    block.execute("UPDATE platform.users SET disabled=TRUE WHERE id='DiscussionUser4'",&[]).await.unwrap();block.commit().await.unwrap();drop(blocker);
    call(&client,&origin,"GET",&preview,None,Some(2),404).await;
    state.db.client().await.unwrap().batch_execute("UPDATE platform.users SET disabled=FALSE WHERE id='DiscussionUser4';").await.unwrap();
    // Actual reciprocal A→B/B→A requests cannot form an actor/target cycle.
    // Hold A, queue A→B at its genuine actor lock, then B→A must refuse before
    // any native row or audit changes. Old waiting target acquisition blocks.
    let actor_thread:String=state.db.client().await.unwrap().query_one("SELECT id FROM platform.catalog_threads WHERE entity_type='user' AND entity_id='DiscussionUser2'",&[]).await.unwrap().get(0);
    let reverse_path=format!("/api/v1/nostr/user/{actor}/discussion");
    let reverse=json!({"expectedAccountId":user,"expectedUserId":actor,"expectedThreadId":actor_thread,"operation":"create","body":"Reciprocal B to A"});
    let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one("SELECT id FROM platform.users WHERE id='DiscussionUser2' FOR NO KEY UPDATE",&[]).await.unwrap();
    let bytes=serde_json::to_vec(&create("Reciprocal A to B")).unwrap();let request=client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&bytes)).body(bytes);let waiting=tokio::spawn(async move{request.send().await.unwrap()});
    wait_for_lock(&state,"%FOR SHARE OF b FOR NO KEY UPDATE OF u%").await;
    let before:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get(0);
    tokio::time::timeout(Duration::from_secs(2),call(&client,&origin,"POST",&reverse_path,Some(reverse),Some(4),409)).await.expect("reciprocal request must not wait or deadlock");
    let reverse_read=format!("{reverse_path}?expectedAccountId={user}&expectedThreadId={actor_thread}");
    tokio::time::timeout(Duration::from_secs(2),call(&client,&origin,"GET",&reverse_read,None,Some(4),409)).await.expect("reciprocal signed read must not wait or deadlock");
    assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get::<_,i64>(0),before);
    block.commit().await.unwrap();drop(blocker);assert_eq!(waiting.await.unwrap().status(),200);
    assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get::<_,i64>(0),before+1);
    // Withdrawing the entity relation under the actual thread lock cannot retarget a command.
    let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one("SELECT id FROM platform.catalog_threads WHERE id=$1 FOR UPDATE",&[&thread]).await.unwrap();
    let bytes=serde_json::to_vec(&create("relation changed while waiting")).unwrap();let request=client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&bytes)).body(bytes);let waiting=tokio::spawn(async move{request.send().await.unwrap()});
    wait_for_lock(&state,"%entity_type='user' AND entity_id=$1 FOR UPDATE%").await;
    block.execute("UPDATE platform.catalog_threads SET entity_id='NoDiscussionUser' WHERE id=$1",&[&thread]).await.unwrap();block.commit().await.unwrap();assert_eq!(waiting.await.unwrap().status(),404);drop(blocker);
    state.db.client().await.unwrap().execute("UPDATE platform.catalog_threads SET entity_id='DiscussionUser4' WHERE id=$1",&[&thread]).await.unwrap();
    // Parallel compare-and-set commands cannot both edit the same captured version.
    let a=serde_json::to_vec(&command("update",json!({"replyId":native_id,"version":2,"body":"Parallel A"}),&actor)).unwrap();
    let b=serde_json::to_vec(&command("update",json!({"replyId":native_id,"version":2,"body":"Parallel B"}),&actor)).unwrap();
    let (a,b)=tokio::join!(client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&a)).body(a).send(),client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&b)).body(b).send());let mut statuses=[a.unwrap().status().as_u16(),b.unwrap().status().as_u16()];statuses.sort();assert_eq!(statuses,[200,409]);
    // Full persisted 50000-byte body must survive worst-case JSON escaping
    // through the actual default 1 MiB outer boundary and 393216-byte adapter.
    let escaped_body = "\u{1}".repeat(50000);
    let escaped = create(&escaped_body);
    assert!(serde_json::to_vec(&escaped).unwrap().len() > 300000);
    let escaped_reply = call(&client,&origin,"POST",&path,Some(escaped),Some(2),200).await;
    assert_eq!(escaped_reply["data"]["reply"]["body"],escaped_body);
    let before_limit = call(&client,&origin,"GET",&preview,None,Some(2),200).await;
    let audit_limit:i64 = state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get(0);
    // Valid signed JSON shape padded beyond the adapter's exact local budget,
    // still below the generic outer budget: refusal must not mutate/audit.
    let mut too_large = serde_json::to_vec(&create("below persisted limit")).unwrap();
    too_large.resize(393217,b' ');
    let refused = client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&too_large)).body(too_large).send().await.unwrap();
    assert_eq!(refused.status(),400);
    assert_eq!(refused.headers()["cache-control"],"private, no-store");
    assert_eq!(call(&client,&origin,"GET",&preview,None,Some(2),200).await,before_limit);
    assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.community_audit",&[]).await.unwrap().get::<_,i64>(0),audit_limit);
    // Multiple pages use exact real counts and ordinals in either direction.
    for n in 0..22 {call(&client,&origin,"POST",&path,Some(create(&format!("Page fixture {n}"))),Some(2),200).await;}
    let asc=call(&client,&origin,"GET",&format!("{preview}&order=asc&page=1"),None,Some(2),200).await;
    let desc=call(&client,&origin,"GET",&format!("{preview}&order=desc&page=1"),None,Some(2),200).await;
    let next=call(&client,&origin,"GET",&format!("{preview}&order=asc&page=2"),None,Some(2),200).await;
    assert_eq!(asc["data"]["feed"]["data"].as_array().unwrap().len(),20);assert_eq!(asc["data"]["feed"]["data"][0]["reply_number"],1);
    assert_eq!(next["data"]["feed"]["data"][0]["reply_number"],21);assert_eq!(desc["data"]["feed"]["data"][0]["reply_number"],asc["data"]["feed"]["total"]);
    let db=state.db.client().await.unwrap();assert_eq!(db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get::<_,i64>(0),aliases);
    let audit=db.query("SELECT operation,old_version,new_version,actor_id FROM platform.community_audit WHERE reply_id=$1 ORDER BY id",&[&r]).await.unwrap();
    assert_eq!(audit.iter().map(|r|r.get::<_,String>(0)).collect::<Vec<_>>(),["create","update","update","remove","delete"]);
    assert_eq!(audit[2].get::<_,String>(3),"DiscussionUser6");assert_eq!(audit[4].get::<_,Option<i32>>(1),Some(4));assert_eq!(audit[4].get::<_,Option<i32>>(2),None);
    assert!(!runtime.join("unused/operator.key").exists());assert!(!runtime.join("unused/service.key").exists());
}
