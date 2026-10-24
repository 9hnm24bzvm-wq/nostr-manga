//! Native profile captured account/target/version, privacy, session and rollback seams.
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
    assert!(response.headers()["cache-control"].to_str().unwrap().contains("private"));
    assert!(response.headers().get_all("vary").iter().any(|v|v.to_str().unwrap().contains("Authorization")));
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
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires isolated scripts/run-auth-tests.sh PostgreSQL"]
async fn native_profile_preserves_identity_privacy_binding_cas_and_native_core() {
    use weebdex_platform::public_api::{PostgresPublicIdResolver,PublicIdResolver,ResourceKind,PublicUuid};
    let dsn=std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use isolated runner");
    let pg:tokio_postgres::Config=dsn.parse().unwrap();assert_eq!(pg.get_dbname(),Some("weebdex_platform_test"));assert!(pg.get_hosts().iter().all(|h|matches!(h,tokio_postgres::config::Host::Tcp(h) if h=="127.0.0.1")));
    let runtime=std::path::PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap());assert!(runtime.to_str().unwrap().contains("/weebdex-platform/test-runtime/"));
    let original=Repository::connect(DbConfig{dsn:dsn.clone(),connections:2,acquisition_timeout:Duration::from_secs(2),cache_weight:1048576,plaintext_local:true}).await.unwrap();original.migrate().await.unwrap();original.bind_instance(id(11),id(1)).await.unwrap();for key in [2,3,4,5] {original.set_account_admission(id(key),true).await.unwrap();}
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let origin=format!("http://{}",listener.local_addr().unwrap());
    let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:6,uploads_dir:runtime.join("media"),ui_dir:runtime.join("no-ui"),protocol_publication:Some(OperatorRuntimeConfig{operator_key_file:runtime.join("unused/operator.key"),expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),storage:json!({"path":runtime.join("unused/storage"),"volume":"profilefixture","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144}),group_keys_dir:None,backend:None,native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":runtime.join("unused/service.key"),"admin_pubkey":public(1)}))}),..Config::default()},dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();state.protocol.verify_schema().await.unwrap();
    let db=state.db.client().await.unwrap();
    for key in [2,4,5] {let user=format!("ProfileUser{key}");db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,disabled,permissions) VALUES($1,$1,$2,'synthetic',TRUE,FALSE,'[\"self.user.update\"]')",&[&user,&format!("profile{key}@fixture.invalid")]).await.unwrap();db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await.unwrap();}
    db.execute("UPDATE platform.users SET disabled=TRUE WHERE id='ProfileUser5'",&[]).await.unwrap();
    db.execute("UPDATE platform.users SET profile=$1,preferences=$2 WHERE id='ProfileUser2'",&[&json!({"description":"Old biography","discord":"invite_42","website":"https://example.invalid/old","email":"private-profile@fixture.invalid","private_key":"synthetic-private-value","permissions":["fake-grant"],"nostr":{"pubkey":"spoof"}}),&json!({"version":1,"private_setting":"retained"})]).await.unwrap();
    let resolver=PostgresPublicIdResolver::new(state.db.clone());let owner=resolver.preferred(ResourceKind::User,"ProfileUser2").await.unwrap().unwrap();let other=resolver.preferred(ResourceKind::User,"ProfileUser4").await.unwrap().unwrap();let alias=PublicUuid::parse("aaaaaaaa-aaaa-8aaa-8aaa-aaaaaaaaaaaa").unwrap();resolver.register_alias(ResourceKind::User,"ProfileUser2",&alias).await.unwrap();
    let publisher=PublicUuid::parse(db.query_one("SELECT platform.public_register_protocol_publisher($1)::text",&[&public(2)]).await.unwrap().get::<_,&str>(0)).unwrap();
    let aliases:i64=db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);drop(db);
    let app=weebdex_platform::router::router(state.clone());let _server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap();}));let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build().unwrap();
    let preview=format!("/api/v1/nostr/user/{owner}/profile?expectedAccountId={owner}");let write=format!("/api/v1/user/{alias}");
    let command=|version:i32,description:&str|json!({"expectedAccountId":owner,"expectedUserId":owner,"version":version,"description":description});
    call(&client,&origin,"GET",&preview,None,None,401).await;for key in [3,5] {call(&client,&origin,"GET",&preview,None,Some(key),403).await;}
    for path in [format!("/api/v1/nostr/user/{owner}/profile"),format!("{preview}&expectedAccountId={owner}"),format!("{preview}&limit=1")] {call(&client,&origin,"GET",&path,None,Some(2),400).await;}
    call(&client,&origin,"GET",&format!("/api/v1/nostr/user/{other}/profile?expectedAccountId={owner}"),None,Some(2),403).await;
    call(&client,&origin,"GET",&format!("/api/v1/nostr/user/{publisher}/profile?expectedAccountId={owner}"),None,Some(2),404).await;
    let first=call(&client,&origin,"GET",&preview,None,Some(2),200).await;
    assert_eq!(first["data"]["id"],owner.as_str());assert_eq!(first["data"]["attributes"],json!({"username":"ProfileUser2","roles":["user"],"version":1,"description":"Old biography","discord":"invite_42","website":"https://example.invalid/old"}));
    assert_eq!(first["nostr"]["accountId"],owner.as_str());assert_eq!(first["nostr"]["userId"],owner.as_str());assert_eq!(first["nostr"]["versionSemantics"],"shared-native-user-revision");assert_eq!(first["data"]["nostr"]["identity"],"native_public_user");assert!(first.to_string().find("private").is_none(),"no private attributes in preview: {first}");
    let public_value:Value=client.get(format!("{origin}/api/v1/user/{owner}")).send().await.unwrap().json().await.unwrap();assert_eq!(public_value["data"]["attributes"],first["data"]["attributes"]);
    let changed=call(&client,&origin,"PUT",&write,Some(command(1,"説明 biography")),Some(2),200).await;assert_eq!(changed["data"]["attributes"]["version"],2);assert_eq!(changed["nostr"]["changed"],true);assert_eq!(changed["data"]["attributes"]["website"],"https://example.invalid/old");
    call(&client,&origin,"PUT",&write,Some(command(1,"stale")),Some(2),409).await;
    let mut mismatch=command(2,"wrong target");mismatch["expectedUserId"]=json!(other);call(&client,&origin,"PUT",&write,Some(mismatch),Some(2),409).await;
    let mut wrong_account=command(2,"wrong account");wrong_account["expectedAccountId"]=json!(other);call(&client,&origin,"PUT",&write,Some(wrong_account),Some(2),409).await;
    for (field,value) in [("website",json!("javascript:alert(1)")),("twitter",json!("https://example.invalid/not-twitter")),("discord",json!("https://example.invalid/not-discord")),("description",json!("x".repeat(20001))),("website",Value::Null),("email",json!("unsafe@fixture.invalid")),("username",json!("substitute")),("version",json!(0)),("version",json!(2147483648_i64))] {let mut bad=command(2,"valid");bad[field]=value;call(&client,&origin,"PUT",&write,Some(bad),Some(2),400).await;}
    let empty=json!({"expectedAccountId":owner,"expectedUserId":owner,"version":2});call(&client,&origin,"PUT",&write,Some(empty),Some(2),400).await;
    let clear=json!({"expectedAccountId":owner,"expectedUserId":owner,"version":2,"website":"","twitter":"https://x.com/fixture"});let cleared=call(&client,&origin,"PUT",&write,Some(clear),Some(2),200).await;assert!(cleared["data"]["attributes"].get("website").is_none());assert_eq!(cleared["data"]["attributes"]["description"],"説明 biography");
    let same=call(&client,&origin,"PUT",&write,Some(command(3,"説明 biography")),Some(2),200).await;assert_eq!(same["data"]["attributes"]["version"],4,"native same-value accepted patch still advances shared revision");
    use sha2::{Digest,Sha256};let token="synthetic-native-profile-cookie";let hash=Sha256::digest(token.as_bytes()).to_vec();state.db.client().await.unwrap().execute("INSERT INTO platform.sessions(id,token_hash,user_id,expires_at) VALUES('ProfileSession2',$1,'ProfileUser2',clock_timestamp()+interval '1hour')",&[&hash]).await.unwrap();let cookie=format!("{}={token}",state.config.session_cookie_name);
    assert_eq!(client.put(format!("{origin}{write}")).header("cookie",&cookie).json(&command(4,"cookie has no proof")).send().await.unwrap().status(),401);
    assert_eq!(client.put(format!("{origin}/api/user/ProfileUser2")).header("cookie",&cookie).header("origin",&origin).json(&json!({"version":4,"description":"Native session update"})).send().await.unwrap().status(),200);call(&client,&origin,"PUT",&write,Some(command(4,"stale after native")),Some(2),409).await;
    let db=state.db.client().await.unwrap();let row=db.query_one("SELECT profile,preferences,username,email,permissions FROM platform.users WHERE id='ProfileUser2'",&[]).await.unwrap();let profile:Value=row.get(0);assert_eq!(profile["private_key"],"synthetic-private-value");assert_eq!(profile["permissions"],json!(["fake-grant"]));assert_eq!(profile["nostr"],json!({"pubkey":"spoof"}));assert_eq!(row.get::<_,Value>(1)["private_setting"],"retained");assert_eq!(row.get::<_,String>(2),"ProfileUser2");assert_eq!(row.get::<_,String>(3),"profile2@fixture.invalid");assert_eq!(row.get::<_,Value>(4),json!(["self.user.update"]));drop(db);
    // A colliding target revision cannot permit an eligible A -> B rebind.
    state.db.client().await.unwrap().batch_execute("UPDATE platform.users SET version=5 WHERE id='ProfileUser4';").await.unwrap();state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='ProfileUser4' WHERE publisher=$1",&[&public(2)]).await.unwrap();call(&client,&origin,"GET",&preview,None,Some(2),409).await;call(&client,&origin,"PUT",&write,Some(command(5,"binding drift")),Some(2),409).await;state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='ProfileUser2' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    let before_failure=call(&client,&origin,"GET",&preview,None,Some(2),200).await;
    let audit_before:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.catalog_audit WHERE entity_type='user'",&[]).await.unwrap().get(0);
    state.db.client().await.unwrap().batch_execute("CREATE FUNCTION platform.profile_fixture_rollback() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.profile->>'description'='rollback' THEN RAISE EXCEPTION 'synthetic profile rollback'; END IF; RETURN NEW; END $$; CREATE TRIGGER profile_fixture_rollback AFTER UPDATE ON platform.users FOR EACH ROW EXECUTE FUNCTION platform.profile_fixture_rollback();").await.unwrap();call(&client,&origin,"PUT",&write,Some(command(5,"rollback")),Some(2),503).await;assert_eq!(call(&client,&origin,"GET",&preview,None,Some(2),200).await,before_failure);state.db.client().await.unwrap().batch_execute("DROP TRIGGER profile_fixture_rollback ON platform.users; DROP FUNCTION platform.profile_fixture_rollback();").await.unwrap();assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.catalog_audit WHERE entity_type='user'",&[]).await.unwrap().get::<_,i64>(0),audit_before);
    let url=format!("{origin}{write}");let a=serde_json::to_vec(&command(5,"Parallel A")).unwrap();let b=serde_json::to_vec(&command(5,"Parallel B")).unwrap();let (a,b)=tokio::join!(client.put(&url).header("content-type","application/json").header("authorization",proof(2,&url,"PUT",&a)).body(a).send(),client.put(&url).header("content-type","application/json").header("authorization",proof(2,&url,"PUT",&b)).body(b).send());let mut statuses=[a.unwrap().status().as_u16(),b.unwrap().status().as_u16()];statuses.sort();assert_eq!(statuses,[200,409]);
    // Block the real final user lock; withdraw the grant before releasing it.
    let mut blocker=state.db.client().await.unwrap();let block=blocker.transaction().await.unwrap();block.query_one("SELECT id FROM platform.users WHERE id='ProfileUser2' FOR NO KEY UPDATE",&[]).await.unwrap();
    let body=serde_json::to_vec(&command(6,"withdrawn while waiting")).unwrap();let request=client.put(&url).header("content-type","application/json").header("authorization",proof(2,&url,"PUT",&body)).body(body);let waiting=tokio::spawn(async move{request.send().await.unwrap()});
    let mut seen=false;for _ in 0..100 {let count:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE '%FOR SHARE OF b FOR NO KEY UPDATE OF u%'",&[]).await.unwrap().get(0);if count>0 {seen=true;break;}tokio::time::sleep(Duration::from_millis(10)).await;}assert!(seen,"request must be waiting on final account authority lock");block.execute("UPDATE platform.users SET permissions='[]' WHERE id='ProfileUser2'",&[]).await.unwrap();block.commit().await.unwrap();assert_eq!(waiting.await.unwrap().status(),403);state.db.client().await.unwrap().batch_execute("UPDATE platform.users SET permissions='[\"self.user.update\"]' WHERE id='ProfileUser2';").await.unwrap();
    let body=serde_json::to_vec(&command(6,"one proof once")).unwrap();let authorization=proof(2,&url,"PUT",&body);assert_eq!(client.put(&url).header("content-type","application/json").header("authorization",&authorization).body(body.clone()).send().await.unwrap().status(),200);assert_eq!(client.put(&url).header("content-type","application/json").header("authorization",&authorization).body(body).send().await.unwrap().status(),401);
    let db=state.db.client().await.unwrap();assert_eq!(db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get::<_,i64>(0),aliases);assert_eq!(db.query_one("SELECT version FROM platform.users WHERE id='ProfileUser4'",&[]).await.unwrap().get::<_,i32>(0),5);assert!(!runtime.join("unused/operator.key").exists());assert!(!runtime.join("unused/service.key").exists());
}
