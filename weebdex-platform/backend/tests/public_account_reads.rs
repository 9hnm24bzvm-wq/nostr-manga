//! Real signed HTTP + PostgreSQL account/follow read boundaries.
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
#[ignore = "Requires explicitly isolated scripts/run-auth-tests.sh PostgreSQL"]
async fn signed_account_reads_scope_native_follows_and_never_disclose_other_private_state() {
    use weebdex_platform::public_api::{PostgresPublicIdResolver, PublicIdResolver, PublicUuid, ResourceKind};
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use isolated runner");
    let pg: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
    assert!(pg.get_hosts().iter().all(|h| matches!(h,tokio_postgres::config::Host::Tcp(h) if h=="127.0.0.1")));
    let runtime = std::path::PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap());
    assert!(runtime.to_str().unwrap().contains("/weebdex-platform/test-runtime/"));
    let original = Repository::connect(DbConfig { dsn:dsn.clone(), connections:2, acquisition_timeout:Duration::from_secs(2), cache_weight:1048576, plaintext_local:true }).await.unwrap();
    original.migrate().await.unwrap();
    original.bind_instance(id(11),id(1)).await.unwrap();
    for key in 2..=7 { original.set_account_admission(id(key),true).await.unwrap(); }
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin=format!("http://{}",listener.local_addr().unwrap());
    let state=AppState::connect(Config {
        public_origin:origin.clone(), database_plaintext_local:true, database_connections:4,
        uploads_dir:runtime.join("media"), ui_dir:runtime.join("no-ui"),
        protocol_publication:Some(OperatorRuntimeConfig {
            operator_key_file:runtime.join("fixture-unused/operator.key"), expected_publisher:public(1),
            storage_owner:public(1), storage_holder:public(11),
            storage:json!({"path":runtime.join("fixture-unused/storage"),"volume":"accountreadfixture","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144}),
            group_keys_dir:None,backend:None,
            native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":runtime.join("fixture-unused/service.key"),"admin_pubkey":public(1)})),
        }),..Config::default()
    },dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();
    state.protocol.verify_schema().await.unwrap();
    let db=state.db.client().await.unwrap();
    for (key,verified,disabled,roles) in [(2,true,false,json!(["user"])),(4,true,false,json!(["user"])),(5,true,true,json!(["user"])),(6,true,false,json!(["banned"])),(7,false,false,json!(["user"])),(8,true,false,json!(["user"]))] {
        let user=format!("ReadUser{key}");
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,disabled,roles,preferences) VALUES($1,$1,$2,'synthetic-no-password',$3,$4,$5,'{\"private_setting\":\"fixture-secret-value\"}')",&[&user,&format!("account-read-{key}@fixture.invalid"),&verified,&disabled,&roles]).await.unwrap();
        db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await.unwrap();
    }
    db.batch_execute(r#"
        INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES('ReadGroup1','ReadUser2','Fixture','Visible group'),('ReadGroup2','ReadUser2','Fixture','Hidden group');
        UPDATE platform.catalog_groups SET deleted_at=clock_timestamp() WHERE id='ReadGroup2';
        INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader) VALUES('ReadGroup1','ReadUser2','ReadUser2',true);
        INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES
        ('ReadManga1','ReadUser2','Fixture','Safe followed','en','ongoing','safe','published'),
        ('ReadAdult1','ReadUser2','Fixture','Adult followed','en','ongoing','pornographic','published'),
        ('ReadHidden','ReadUser2','Fixture','Hidden followed','en','ongoing','safe','published'),
        ('ReadOther1','ReadUser4','Fixture','Other private follow','en','ongoing','safe','published');
        UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='ReadHidden';
        INSERT INTO platform.library_manga_follows(user_id,manga_id,reading_status,is_private,notification) VALUES
        ('ReadUser2','ReadManga1','reading',true,true),('ReadUser2','ReadAdult1','completed',true,false),
        ('ReadUser2','ReadHidden','reading',true,true),('ReadUser4','ReadOther1','reading',true,true);
        INSERT INTO platform.library_group_follows(user_id,group_id) VALUES('ReadUser2','ReadGroup1'),('ReadUser2','ReadGroup2');
        INSERT INTO platform.library_user_follows(user_id,target_id) VALUES('ReadUser2','ReadUser4'),('ReadUser2','ReadUser5');
        INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,title,volume,chapter,language,metadata) VALUES
        ('ReadChap01','ReadManga1','ReadUser2','Fixture','Safe chapter','1','1','en','{"data":[{"name":"fixture.webp","dimensions":[1,1]}]}'),
        ('ReadChap02','ReadAdult1','ReadUser2','Fixture','Adult chapter','1','2','ja','{"data":[{"name":"fixture.webp","dimensions":[1,1]}]}'),
        ('ReadChap03','ReadHidden','ReadUser2','Fixture','Hidden chapter','1','3','en','{"data":[{"name":"fixture.webp","dimensions":[1,1]}]}'),
        ('ReadChap04','ReadOther1','ReadUser4','Fixture','Other chapter','1','4','en','{"data":[{"name":"fixture.webp","dimensions":[1,1]}]}');
        INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES('ReadChap01','ReadGroup1');
    "#).await.unwrap();
    let aliases_before:i64=db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);
    drop(db);
    let resolver=PostgresPublicIdResolver::new(state.db.clone());
    let mut ids=std::collections::BTreeMap::new();
    for (key,kind,native) in [("owner",ResourceKind::User,"ReadUser2"),("other",ResourceKind::User,"ReadUser4"),("disabled",ResourceKind::User,"ReadUser5"),("manga",ResourceKind::Manga,"ReadManga1"),("adult",ResourceKind::Manga,"ReadAdult1"),("hidden",ResourceKind::Manga,"ReadHidden"),("other_manga",ResourceKind::Manga,"ReadOther1"),("group",ResourceKind::Group,"ReadGroup1"),("hidden_group",ResourceKind::Group,"ReadGroup2"),("chapter",ResourceKind::Chapter,"ReadChap01")] {
        ids.insert(key,resolver.preferred(kind,native).await.unwrap().unwrap());
    }
    let alias=PublicUuid::parse("aaaaaaaa-aaaa-8aaa-8aaa-aaaaaaaaaaaa").unwrap();
    resolver.register_alias(ResourceKind::Manga,"ReadManga1",&alias).await.unwrap();
    let app=weebdex_platform::router::router(state.clone());
    let _server=Server(tokio::spawn(async move {axum::serve(listener,app).await.unwrap();}));
    let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build().unwrap();
    for path in ["/api/v1/user","/api/v1/user/me","/api/v1/user/follows/manga","/api/v1/user/follows/group","/api/v1/user/follows/user","/api/v1/user/follows/manga/feed"] {
        call(&client,&origin,"GET",path,None,None,401).await;
    }
    for key in [3,5,6,7,8] {call(&client,&origin,"GET","/api/v1/user/me",None,Some(key),403).await;}
    let me=call(&client,&origin,"GET","/api/v1/user/me",None,Some(2),200).await;
    assert_eq!(me["data"]["id"],ids["owner"].as_str());
    assert_eq!(me["data"]["attributes"]["username"],"ReadUser2");
    assert_eq!(me["data"]["attributes"].as_object().unwrap().len(),3);
    assert!(!me.to_string().contains("fixture.invalid"));
    assert!(!me.to_string().contains("private_setting"));
    let directory=call(&client,&origin,"GET",&format!("/api/v1/user?username=readuser&ids[]={}&ids[]={}&limit=1&order[username]=desc",ids["owner"],ids["other"]),None,Some(2),200).await;
    assert_eq!(directory["total"],2);assert_eq!(directory["data"][0]["id"],ids["other"].as_str());
    let disabled=call(&client,&origin,"GET",&format!("/api/v1/user?ids[]={}",ids["disabled"]),None,Some(2),200).await;
    assert_eq!(disabled["total"],0);
    let count=call(&client,&origin,"GET","/api/v1/user?limit=0",None,Some(2),200).await;
    assert_eq!(count["data"],json!([]));
    let follows=call(&client,&origin,"GET","/api/v1/user/follows/manga?includes[]=creator&limit=1&offset=1",None,Some(2),200).await;
    assert_eq!(follows["total"],2,"known adult follows must not inherit discovery rating defaults");
    assert_eq!(follows["data"].as_array().unwrap().len(),1);
    let other=call(&client,&origin,"GET","/api/v1/user/follows/manga",None,Some(4),200).await;
    assert_eq!(other["total"],1);assert_eq!(other["data"][0]["id"],ids["other_manga"].as_str());
    let groups=call(&client,&origin,"GET","/api/v1/user/follows/group?includes[]=leader",None,Some(2),200).await;
    assert_eq!(groups["total"],1);assert_eq!(groups["data"][0]["id"],ids["group"].as_str());
    assert_eq!(groups["data"][0]["relationships"][0]["attributes"]["username"],"ReadUser2");
    let users=call(&client,&origin,"GET","/api/v1/user/follows/user",None,Some(2),200).await;
    assert_eq!(users["total"],1);assert_eq!(users["data"][0]["id"],ids["other"].as_str());
    for path in [format!("/api/v1/user/follows/manga/{alias}"),format!("/api/v1/user/follows/group/{}",ids["group"]),format!("/api/v1/user/follows/user/{}",ids["other"])] {assert_eq!(call(&client,&origin,"GET",&path,None,Some(2),200).await["result"],"ok");}
    for path in [format!("/api/v1/user/follows/manga/{}",ids["hidden"]),format!("/api/v1/user/follows/manga/{}",ids["other_manga"]),format!("/api/v1/user/follows/manga/{}",ids["owner"]),format!("/api/v1/user/follows/group/{}",ids["hidden_group"]),format!("/api/v1/user/follows/user/{}",ids["disabled"])] {call(&client,&origin,"GET",&path,None,Some(2),404).await;}
    let feed=call(&client,&origin,"GET","/api/v1/user/follows/manga/feed?includes[]=manga&translatedLanguage[]=en",None,Some(2),200).await;
    assert_eq!(feed["total"],1);assert_eq!(feed["data"][0]["id"],ids["chapter"].as_str());
    let adult=call(&client,&origin,"GET","/api/v1/user/follows/manga/feed?contentRating[]=pornographic",None,Some(2),200).await;
    assert_eq!(adult["total"],1,"all followed titles contribute even when native notifications are off");
    let excluded=call(&client,&origin,"GET",&format!("/api/v1/user/follows/manga/feed?excludedGroups[]={}",ids["group"]),None,Some(2),200).await;
    assert_eq!(excluded["total"],0);
    for path in ["/api/v1/user/me?limit=1","/api/v1/user/follows/manga?limit=0","/api/v1/user/follows/manga/feed?limit=0","/api/v1/user?ids[]=ReadUser2"] {call(&client,&origin,"GET",path,None,Some(2),400).await;}

    // Request, method, service and nonce are actual original capability checks.
    let url=format!("{origin}/api/v1/user/me");let authorization=proof(2,&url,"GET",b"");
    assert_eq!(client.get(&url).header("authorization",&authorization).send().await.unwrap().status(),200);
    assert_eq!(client.get(&url).header("authorization",&authorization).send().await.unwrap().status(),401);
    assert_eq!(client.get(&url).header("authorization",proof(2,&format!("{origin}/api/v1/user"),"GET",b"")).send().await.unwrap().status(),401);
    assert_eq!(client.get(&url).header("cookie","session=untrusted-cookie").send().await.unwrap().status(),401);
    let aliases_after:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);
    assert_eq!(aliases_after,aliases_before+1,"only the explicit test alias registration writes an alias");
    original.set_account_admission(id(2),false).await.unwrap();
    call(&client,&origin,"GET","/api/v1/user/follows/manga",None,Some(2),403).await;
    original.set_account_admission(id(2),true).await.unwrap();
    state.db.client().await.unwrap().execute("DELETE FROM platform.protocol_identity_bindings WHERE publisher=$1",&[&public(2)]).await.unwrap();
    call(&client,&origin,"GET","/api/v1/user/me",None,Some(2),403).await;
}
