//! Actual native reading-status CAS, private binding and native/session/import seams.
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
async fn native_status_has_independent_cas_binding_and_no_membership_side_effects() {
    use weebdex_platform::public_api::{PostgresPublicIdResolver,PublicIdResolver,ResourceKind,PublicUuid};
    let dsn=std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use isolated runner");
    let pg:tokio_postgres::Config=dsn.parse().unwrap();assert_eq!(pg.get_dbname(),Some("weebdex_platform_test"));assert!(pg.get_hosts().iter().all(|h|matches!(h,tokio_postgres::config::Host::Tcp(h) if h=="127.0.0.1")));
    let runtime=std::path::PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap());assert!(runtime.to_str().unwrap().contains("/weebdex-platform/test-runtime/"));
    let original=Repository::connect(DbConfig{dsn:dsn.clone(),connections:2,acquisition_timeout:Duration::from_secs(2),cache_weight:1048576,plaintext_local:true}).await.unwrap();original.migrate().await.unwrap();original.bind_instance(id(11),id(1)).await.unwrap();for key in [2,3,4,5] {original.set_account_admission(id(key),true).await.unwrap();}
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let origin=format!("http://{}",listener.local_addr().unwrap());
    let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:4,uploads_dir:runtime.join("media"),ui_dir:runtime.join("no-ui"),protocol_publication:Some(OperatorRuntimeConfig{operator_key_file:runtime.join("unused/operator.key"),expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),storage:json!({"path":runtime.join("unused/storage"),"volume":"statusfixture","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144}),group_keys_dir:None,backend:None,native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":runtime.join("unused/service.key"),"admin_pubkey":public(1)}))}),..Config::default()},dsn).await.unwrap();
    let before=domains::MIGRATIONS.partition_point(|m|m.version<102);state.db.migrate(&domains::MIGRATIONS[..before]).await.unwrap();state.protocol.verify_schema().await.unwrap();
    let db=state.db.client().await.unwrap();
    for key in [2,4,5] {let user=format!("StatusUser{key}");db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,disabled,permissions) VALUES($1,$1,$2,'synthetic',TRUE,$3,'[\"self.preferences.update\"]')",&[&user,&format!("status{key}@fixture.invalid"),&(key==5)]).await.unwrap();db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await.unwrap();}
    db.batch_execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES('StatusManga1','StatusUser2','Fixture','Status fixture','en','ongoing','safe','published'),('StatusEmpty1','StatusUser2','Fixture','Empty fixture','en','ongoing','safe','published'),('StatusGone01','StatusUser2','Fixture','Old removed fixture','en','ongoing','safe','published'); INSERT INTO platform.library_manga_follows(user_id,manga_id,reading_status,notification,is_private,followed_at,updated_at) VALUES('StatusUser2','StatusManga1','reading',FALSE,TRUE,'2024-01-01T00:00:00Z','2024-02-01T00:00:00Z'),('StatusUser4','StatusManga1','completed',TRUE,TRUE,'2024-01-01T00:00:00Z','2024-02-01T00:00:00Z'); INSERT INTO platform.catalog_manga_counters(manga_id,follows) VALUES('StatusManga1',2);").await.unwrap();
    db.batch_execute("INSERT INTO platform.library_manga_follows(user_id,manga_id,reading_status) VALUES('StatusUser2','StatusGone01','completed'); DELETE FROM platform.library_manga_follows WHERE user_id='StatusUser2' AND manga_id='StatusGone01';").await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();
    let baseline=db.query_one("SELECT version,updated_at='2024-02-01T00:00:00Z'::timestamptz FROM platform.library_reading_status_revisions WHERE user_id='StatusUser2' AND manga_id='StatusManga1'",&[]).await.unwrap();assert_eq!(baseline.get::<_,i32>(0),1);assert!(baseline.get::<_,bool>(1));drop(db);
    let resolver=PostgresPublicIdResolver::new(state.db.clone());let owner=resolver.preferred(ResourceKind::User,"StatusUser2").await.unwrap().unwrap();let other=resolver.preferred(ResourceKind::User,"StatusUser4").await.unwrap().unwrap();let manga=resolver.preferred(ResourceKind::Manga,"StatusManga1").await.unwrap().unwrap();let empty=resolver.preferred(ResourceKind::Manga,"StatusEmpty1").await.unwrap().unwrap();let alias=PublicUuid::parse("aaaaaaaa-aaaa-8aaa-8aaa-aaaaaaaaaaaa").unwrap();resolver.register_alias(ResourceKind::Manga,"StatusManga1",&alias).await.unwrap();
    let gone=resolver.preferred(ResourceKind::Manga,"StatusGone01").await.unwrap().unwrap();
    let aliases:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);
    let app=weebdex_platform::router::router(state.clone());let _server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap();}));let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build().unwrap();
    let preview=format!("/api/v1/nostr/manga/{manga}/reading-status?expectedAccountId={owner}");let write=format!("/api/v1/manga/{alias}/status");
    let command=|status:&str,version:i32|json!({"status":status,"expectedAccountId":owner,"expectedMangaId":manga,"version":version});
    call(&client,&origin,"GET",&preview,None,None,401).await;for key in [3,5] {call(&client,&origin,"GET",&preview,None,Some(key),403).await;}
    for path in [format!("/api/v1/nostr/manga/{manga}/reading-status"),format!("{preview}&expectedAccountId={owner}"),format!("{preview}&limit=1")] {call(&client,&origin,"GET",&path,None,Some(2),400).await;}
    call(&client,&origin,"GET",&format!("/api/v1/nostr/manga/{owner}/reading-status?expectedAccountId={owner}"),None,Some(2),404).await;
    let first=call(&client,&origin,"GET",&preview,None,Some(2),200).await;assert_eq!(first["status"],"reading");assert_eq!(first["nostr"]["accountId"],owner.as_str());assert_eq!(first["nostr"]["mangaId"],manga.as_str());assert_eq!(first["nostr"]["version"],1);assert_eq!(first["nostr"]["followed"],true);assert_eq!(first["nostr"]["versionSemantics"],"reading-status-change-revision");
    use sha2::{Digest,Sha256};let token="synthetic-native-status-cookie";let hash=Sha256::digest(token.as_bytes()).to_vec();state.db.client().await.unwrap().execute("INSERT INTO platform.sessions(id,token_hash,user_id,expires_at) VALUES('StatusSession2',$1,'StatusUser2',clock_timestamp()+interval '1hour')",&[&hash]).await.unwrap();let cookie=format!("{}={token}",state.config.session_cookie_name);
    assert_eq!(client.post(format!("{origin}/api/user/follows/manga/StatusManga1")).header("cookie",&cookie).header("origin",&origin).json(&json!({"status":"reading","notification":true,"private":false})).send().await.unwrap().status(),204);
    assert_eq!(call(&client,&origin,"GET",&preview,None,Some(2),200).await,first,"native same-status metadata edit preserves the status revision and ledger clock");
    let changed=call(&client,&origin,"POST",&write,Some(command("on_hold",1)),Some(2),200).await;assert_eq!(changed["nostr"]["version"],2);assert_eq!(changed["nostr"]["changed"],true);let no_op=call(&client,&origin,"POST",&write,Some(command("on_hold",2)),Some(2),200).await;assert_eq!(no_op["nostr"]["changed"],false);assert_eq!(no_op["nostr"]["updatedAt"],changed["nostr"]["updatedAt"]);assert_eq!(no_op["nostr"]["version"],2);
    let preserved=state.db.client().await.unwrap().query_one("SELECT notification AND NOT is_private AND followed_at='2024-01-01T00:00:00Z'::timestamptz,(SELECT follows FROM platform.catalog_manga_counters WHERE manga_id='StatusManga1'),(SELECT version FROM platform.library_follow_revisions WHERE user_id='StatusUser2' AND kind='manga' AND target_id='StatusManga1') FROM platform.library_manga_follows WHERE user_id='StatusUser2' AND manga_id='StatusManga1'",&[]).await.unwrap();assert!(preserved.get::<_,bool>(0));assert_eq!(preserved.get::<_,i64>(1),2);assert_eq!(preserved.get::<_,i32>(2),1);
    call(&client,&origin,"POST",&write,Some(command("completed",1)),Some(2),409).await;let mut bad=command("completed",2);bad["expectedMangaId"]=json!(empty);call(&client,&origin,"POST",&write,Some(bad),Some(2),409).await;
    let mut null_clear=command("reading",2);null_clear["status"]=Value::Null;call(&client,&origin,"POST",&write,Some(null_clear),Some(2),400).await;
    assert_eq!(client.post(format!("{origin}{write}")).header("cookie",&cookie).json(&command("reading",2)).send().await.unwrap().status(),401);
    assert_eq!(client.post(format!("{origin}/api/user/follows/manga/StatusManga1")).header("cookie",&cookie).header("origin",&origin).json(&json!({"status":"completed"})).send().await.unwrap().status(),204);
    assert_eq!(call(&client,&origin,"GET",&preview,None,Some(2),200).await["nostr"]["version"],3);call(&client,&origin,"POST",&write,Some(command("reading",2)),Some(2),409).await;
    assert_eq!(client.post(format!("{origin}/api/user/follows/manga/import")).header("cookie",&cookie).header("origin",&origin).json(&json!({"weebdex":{"StatusManga1":"re_reading"}})).send().await.unwrap().status(),204);
    assert_eq!(call(&client,&origin,"GET",&preview,None,Some(2),200).await["nostr"]["version"],4);
    // Same source timestamp (including microseconds) cannot conceal status ABA.
    state.db.client().await.unwrap().batch_execute("UPDATE platform.library_manga_follows SET reading_status='reading',updated_at='2024-04-01T00:00:00.123456Z' WHERE user_id='StatusUser2'; UPDATE platform.library_manga_follows SET reading_status='completed',updated_at='2024-04-01T00:00:00.123456Z' WHERE user_id='StatusUser2';").await.unwrap();
    assert_eq!(call(&client,&origin,"GET",&preview,None,Some(2),200).await["nostr"]["version"],6);call(&client,&origin,"POST",&write,Some(command("reading",4)),Some(2),409).await;
    assert_eq!(client.delete(format!("{origin}/api/user/follows/manga/StatusManga1")).header("cookie",&cookie).header("origin",&origin).send().await.unwrap().status(),204);
    let tombstone=call(&client,&origin,"GET",&preview,None,Some(2),200).await;assert_eq!(tombstone["status"],Value::Null);assert_eq!(tombstone["nostr"]["version"],7);assert_eq!(tombstone["nostr"]["followed"],false);assert!(tombstone["nostr"]["updatedAt"].is_string());call(&client,&origin,"POST",&write,Some(command("reading",7)),Some(2),409).await;
    assert_eq!(client.post(format!("{origin}/api/user/follows/manga/StatusManga1")).header("cookie",&cookie).header("origin",&origin).json(&json!({"status":"completed"})).send().await.unwrap().status(),204);assert_eq!(call(&client,&origin,"GET",&preview,None,Some(2),200).await["nostr"]["version"],8);call(&client,&origin,"POST",&write,Some(command("reading",6)),Some(2),409).await;
    let url=format!("{origin}{write}");let a=serde_json::to_vec(&command("reading",8)).unwrap();let b=serde_json::to_vec(&command("on_hold",8)).unwrap();let (a,b)=tokio::join!(client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&a)).body(a).send(),client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",&b)).body(b).send());let mut statuses=[a.unwrap().status().as_u16(),b.unwrap().status().as_u16()];statuses.sort();assert_eq!(statuses,[200,409]);
    state.db.client().await.unwrap().batch_execute("UPDATE platform.library_reading_status_revisions SET version=9 WHERE user_id='StatusUser4';").await.unwrap();state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='StatusUser4' WHERE publisher=$1",&[&public(2)]).await.unwrap();call(&client,&origin,"GET",&preview,None,Some(2),409).await;call(&client,&origin,"POST",&write,Some(command("reading",9)),Some(2),409).await;state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='StatusUser2' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    let old_removed=call(&client,&origin,"GET",&format!("/api/v1/nostr/manga/{gone}/reading-status?expectedAccountId={owner}"),None,Some(2),200).await;assert_eq!(old_removed["status"],Value::Null);assert_eq!(old_removed["nostr"]["version"],1);assert_eq!(old_removed["nostr"]["followed"],false);assert!(old_removed["nostr"]["updatedAt"].is_string());
    let never=format!("/api/v1/nostr/manga/{empty}/reading-status?expectedAccountId={owner}");let empty_state=call(&client,&origin,"GET",&never,None,Some(2),200).await;assert_eq!(empty_state["status"],Value::Null);assert_eq!(empty_state["nostr"]["version"],0);assert_eq!(empty_state["nostr"]["updatedAt"],Value::Null);
    let mut absent=command("reading",0);absent["expectedMangaId"]=json!(empty);call(&client,&origin,"POST",&format!("/api/v1/manga/{empty}/status"),Some(absent),Some(2),409).await;
    let before_failure=call(&client,&origin,"GET",&preview,None,Some(2),200).await;
    state.db.client().await.unwrap().batch_execute("CREATE FUNCTION platform.status_fixture_rollback() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.status='dropped' THEN RAISE EXCEPTION 'synthetic status rollback'; END IF; RETURN NEW; END $$; CREATE TRIGGER status_fixture_rollback AFTER UPDATE ON platform.library_reading_status_revisions FOR EACH ROW EXECUTE FUNCTION platform.status_fixture_rollback();").await.unwrap();call(&client,&origin,"POST",&write,Some(command("dropped",9)),Some(2),503).await;assert_eq!(call(&client,&origin,"GET",&preview,None,Some(2),200).await,before_failure);state.db.client().await.unwrap().batch_execute("DROP TRIGGER status_fixture_rollback ON platform.library_reading_status_revisions; DROP FUNCTION platform.status_fixture_rollback(); UPDATE platform.catalog_manga SET protocol_source_visible=FALSE WHERE id='StatusManga1';").await.unwrap();call(&client,&origin,"GET",&preview,None,Some(2),404).await;call(&client,&origin,"POST",&write,Some(command("reading",9)),Some(2),404).await;
    state.db.client().await.unwrap().batch_execute("UPDATE platform.catalog_manga SET protocol_source_visible=TRUE WHERE id='StatusManga1'; UPDATE platform.users SET permissions='[]' WHERE id='StatusUser2';").await.unwrap();call(&client,&origin,"POST",&write,Some(command("reading",9)),Some(2),403).await;state.db.client().await.unwrap().batch_execute("UPDATE platform.users SET permissions='[\"self.preferences.update\"]' WHERE id='StatusUser2';").await.unwrap();
    let body=serde_json::to_vec(&command("reading",9)).unwrap();let authorization=proof(2,&url,"POST",&body);let response=client.post(&url).header("content-type","application/json").header("authorization",&authorization).body(body.clone()).send().await.unwrap();assert_eq!(response.status(),200);assert_eq!(client.post(&url).header("content-type","application/json").header("authorization",&authorization).body(body).send().await.unwrap().status(),401);
    let aliases_after:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);assert_eq!(aliases_after,aliases,"status writes create no resource aliases");
    // Original signed-library reads are unaffected by these native observations.
    let old_url=format!("{origin}/api/v1/manga/status");let old:Value=client.get(&old_url).header("authorization",proof(2,&old_url,"GET",b"")).send().await.unwrap().json().await.unwrap();assert_eq!(old["nostr"]["scope"],"signed_public_library");assert_eq!(old["statuses"],json!({}));
    assert_ne!(owner,other);
}
