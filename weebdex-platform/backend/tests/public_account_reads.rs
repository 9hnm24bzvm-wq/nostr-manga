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
    // Keep the existing-membership baseline test anchored to migration0100,
    // even when a later, unrelated migration is added to the application.
    let before_follow_revisions=domains::MIGRATIONS.partition_point(|m|m.version<100);
    state.db.migrate(&domains::MIGRATIONS[..before_follow_revisions]).await.unwrap();
    state.protocol.verify_schema().await.unwrap();
    let db=state.db.client().await.unwrap();
    for (key,verified,disabled,roles) in [(2,true,false,json!(["user"])),(4,true,false,json!(["user"])),(5,true,true,json!(["user"])),(6,true,false,json!(["banned"])),(7,false,false,json!(["user"])),(8,true,false,json!(["user"]))] {
        let user=format!("ReadUser{key}");
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,disabled,roles,preferences) VALUES($1,$1,$2,'synthetic-no-password',$3,$4,$5,'{\"private_setting\":\"fixture-secret-value\"}')",&[&user,&format!("account-read-{key}@fixture.invalid"),&verified,&false,&roles]).await.unwrap();
        db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await.unwrap();
        db.execute("UPDATE platform.users SET disabled=$2 WHERE id=$1",&[&user,&disabled]).await.unwrap();
    }
    db.batch_execute(r#"
        INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES('ReadGroup1','ReadUser2','Fixture','Visible group'),('ReadGroup2','ReadUser2','Fixture','Hidden group');
        INSERT INTO platform.catalog_tags(id,name,tag_group) VALUES('ReadTag001','Fixture settings tag','theme');
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
        INSERT INTO platform.library_chapter_history(user_id,manga_id,chapter_id,read_at) VALUES
        ('ReadUser2','ReadManga1','ReadChap01','2024-01-01T12:00:00Z'),
        ('ReadUser2','ReadAdult1','ReadChap02','2024-02-01T12:00:00Z'),
        ('ReadUser2','ReadHidden','ReadChap03','2024-03-01T12:00:00Z'),
        ('ReadUser4','ReadOther1','ReadChap04','2024-04-01T12:00:00Z');
    "#).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();
    let publisher_uuid:String=db.query_one("SELECT platform.public_register_protocol_publisher($1)::text",&[&public(2)]).await.unwrap().get(0);
    let aliases_before:i64=db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);
    drop(db);
    let resolver=PostgresPublicIdResolver::new(state.db.clone());
    let mut ids=std::collections::BTreeMap::new();
    for (key,kind,native) in [("owner",ResourceKind::User,"ReadUser2"),("other",ResourceKind::User,"ReadUser4"),("disabled",ResourceKind::User,"ReadUser5"),("manga",ResourceKind::Manga,"ReadManga1"),("adult",ResourceKind::Manga,"ReadAdult1"),("hidden",ResourceKind::Manga,"ReadHidden"),("other_manga",ResourceKind::Manga,"ReadOther1"),("group",ResourceKind::Group,"ReadGroup1"),("hidden_group",ResourceKind::Group,"ReadGroup2"),("chapter",ResourceKind::Chapter,"ReadChap01")] {
        ids.insert(key,resolver.preferred(kind,native).await.unwrap().unwrap());
    }
    ids.insert("publisher",PublicUuid::parse(&publisher_uuid).unwrap());
    let alias=PublicUuid::parse("aaaaaaaa-aaaa-8aaa-8aaa-aaaaaaaaaaaa").unwrap();
    resolver.register_alias(ResourceKind::Manga,"ReadManga1",&alias).await.unwrap();
    let app=weebdex_platform::router::router(state.clone());
    let _server=Server(tokio::spawn(async move {axum::serve(listener,app).await.unwrap();}));
    let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build().unwrap();
    exercise_inventory_routing(&client,&origin,&runtime).await;
    for path in ["/api/v1/user","/api/v1/user/me","/api/v1/user/history","/api/v1/settings","/api/v1/settings/template","/api/v1/user/follows/manga","/api/v1/user/follows/group","/api/v1/user/follows/user","/api/v1/user/follows/manga/feed"] {
        call(&client,&origin,"GET",path,None,None,401).await;
    }
    for key in [3,5,6,7,8] {call(&client,&origin,"GET","/api/v1/user/me",None,Some(key),403).await;}
    let me=call(&client,&origin,"GET","/api/v1/user/me",None,Some(2),200).await;
    assert_eq!(me["data"]["id"],ids["owner"].as_str());
    assert_eq!(me["data"]["attributes"]["username"],"ReadUser2");
    assert_eq!(me["data"]["attributes"].as_object().unwrap().len(),3);
    assert!(!me.to_string().contains("fixture.invalid"));
    assert!(!me.to_string().contains("private_setting"));
    // Persisted native ratings remain private owner observations. Aggregate
    // cache values are never interpreted as another user's individual vote.
    let rating_path=format!("/api/v1/rating/{}",ids["manga"]);
    let rating_query=format!("/api/v1/rating?manga[]={}",ids["manga"]);
    call(&client,&origin,"GET",&rating_query,None,None,401).await;
    for key in [3,5,6,7,8] {call(&client,&origin,"POST",&rating_path,Some(json!({"rating":8})),Some(key),403).await;}
    call(&client,&origin,"POST",&rating_path,Some(json!({"rating":8})),Some(2),403).await;
    state.db.client().await.unwrap().execute("UPDATE platform.users SET permissions='[\"self.preferences.update\"]' WHERE id IN ('ReadUser2','ReadUser4')",&[]).await.unwrap();
    let empty=call(&client,&origin,"GET",&rating_query,None,Some(2),200).await;
    assert_eq!(empty["ratings"],json!({})); assert_eq!(empty["nostr"]["versions"][ids["manga"].as_str()],0);
    assert_eq!(empty["nostr"]["accountId"],ids["owner"].as_str());
    // Both accounts have version0 here. A signer rebind must still invalidate
    // the captured account preview, for both create and delete commands.
    state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='ReadUser4' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    call(&client,&origin,"POST",&rating_path,Some(json!({"rating":8,"version":0,"expectedAccountId":ids["owner"]})),Some(2),409).await;
    call(&client,&origin,"DELETE",&rating_path,Some(json!({"version":0,"expectedAccountId":ids["owner"]})),Some(2),409).await;
    let rebound=call(&client,&origin,"GET",&rating_query,None,Some(2),200).await;
    assert_eq!(rebound["nostr"]["accountId"],ids["other"].as_str()); assert_eq!(rebound["ratings"],json!({}));
    state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='ReadUser2' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    let created=call(&client,&origin,"POST",&rating_path,Some(json!({"rating":8,"version":0,"expectedAccountId":ids["owner"]})),Some(2),200).await;
    assert_eq!(created["nostr"]["version"],1); assert_eq!(created["nostr"]["changed"],true);
    assert_eq!(created["nostr"]["accountId"],ids["owner"].as_str());
    call(&client,&origin,"POST",&rating_path,Some(json!({"rating":8,"expectedAccountId":"not-a-user-uuid"})),Some(2),400).await;
    let read=call(&client,&origin,"GET",&rating_query,None,Some(2),200).await;
    let rated_at=read["ratings"][ids["manga"].as_str()]["createdAt"].clone();
    assert!(rated_at.is_string()); assert_eq!(read["ratings"][ids["manga"].as_str()]["rating"],8);
    let duplicate=call(&client,&origin,"POST",&rating_path,Some(json!({"rating":8})),Some(2),200).await;
    assert_eq!(duplicate["nostr"]["version"],1); assert_eq!(duplicate["nostr"]["changed"],false);
    assert_eq!(call(&client,&origin,"GET",&rating_query,None,Some(2),200).await,read,"same vote must not churn clocks or version");
    let other=call(&client,&origin,"GET",&rating_query,None,Some(4),200).await;
    assert_eq!(other["ratings"],json!({}));
    call(&client,&origin,"POST",&rating_path,Some(json!({"rating":9,"version":0})),Some(2),409).await;
    for payload in [json!({"rating":0}),json!({"rating":11}),json!({"rating":1.5}),json!({"rating":"8"}),json!({"rating":8,"user":"ReadUser4"}),json!({"rating":8,"version":-1})] {
        call(&client,&origin,"POST",&rating_path,Some(payload),Some(2),400).await;
    }
    call(&client,&origin,"POST",&format!("/api/v1/rating/{}",ids["hidden"]),Some(json!({"rating":8})),Some(2),404).await;
    call(&client,&origin,"GET","/api/v1/rating?manga[]=00000000-0000-8000-8000-000000000001",None,Some(2),404).await;
    call(&client,&origin,"GET",&format!("{rating_query}&limit=0"),None,Some(2),400).await;
    let canonical=call(&client,&origin,"GET",&format!("/api/v1/rating?manga={alias}"),None,Some(2),200).await;
    assert_eq!(canonical["ratings"],read["ratings"]);
    let url=format!("{origin}{rating_path}"); let raw=b"{\"rating\":7,\"version\":1}";
    let authorization=proof(2,&url,"POST",raw);
    let first=client.post(&url).header("content-type","application/json").header("authorization",&authorization).body(raw.as_slice()).send().await.unwrap(); assert_eq!(first.status(),200);
    let replay=client.post(&url).header("content-type","application/json").header("authorization",&authorization).body(raw.as_slice()).send().await.unwrap(); assert_eq!(replay.status(),401);
    let wrong=client.post(&url).header("content-type","application/json").header("authorization",proof(2,&url,"POST",b"{\"rating\":2}")).body(raw.as_slice()).send().await.unwrap(); assert_eq!(wrong.status(),401);
    // Two owners concurrently update the same title. The title lock makes both
    // committed observations visible to the second aggregate refresh.
    let (a,b)=tokio::join!(
        call(&client,&origin,"POST",&rating_path,Some(json!({"rating":8,"version":2})),Some(2),200),
        call(&client,&origin,"POST",&rating_path,Some(json!({"rating":7,"version":0})),Some(4),200)
    ); assert_eq!(a["nostr"]["version"],3); assert_eq!(b["nostr"]["version"],1);
    let stats=client.get(format!("{origin}/api/v1/statistics/manga/{}",ids["manga"])).send().await.unwrap().json::<Value>().await.unwrap();
    let aggregate=&stats["statistics"][ids["manga"].as_str()]["rating"];
    assert_eq!(aggregate["average"],7.5); assert_eq!(aggregate["distribution"]["7"],1); assert_eq!(aggregate["distribution"]["8"],1); assert_eq!(aggregate["nostr"]["observedCount"],2);
    assert!(aggregate.get("bayesian").is_none());
    let counter:String=state.db.client().await.unwrap().query_one("SELECT rating::text FROM platform.catalog_manga_counters WHERE manga_id='ReadManga1'",&[]).await.unwrap().get(0); assert_eq!(counter.parse::<f64>().unwrap(),7.5);
    assert_eq!(call(&client,&origin,"GET",&rating_query,None,Some(2),200).await["ratings"][ids["manga"].as_str()]["createdAt"],rated_at,"updates retain the original observed rating date");
    call(&client,&origin,"DELETE",&rating_path,Some(json!({"version":2})),Some(2),409).await;
    let deleted=call(&client,&origin,"DELETE",&rating_path,Some(json!({"version":3})),Some(2),200).await;
    assert_eq!(deleted["nostr"]["version"],4); assert_eq!(deleted["nostr"]["changed"],true);
    let repeated_delete=call(&client,&origin,"DELETE",&rating_path,None,Some(2),200).await;
    assert_eq!(repeated_delete["nostr"]["version"],4); assert_eq!(repeated_delete["nostr"]["changed"],false);
    let tombstone=call(&client,&origin,"GET",&rating_query,None,Some(2),200).await;
    assert_eq!(tombstone["ratings"],json!({})); assert_eq!(tombstone["nostr"]["versions"][ids["manga"].as_str()],4);
    call(&client,&origin,"POST",&rating_path,Some(json!({"rating":10,"version":0})),Some(2),409).await;
    call(&client,&origin,"POST",&rating_path,Some(json!({"rating":10,"version":4})),Some(2),200).await;
    // Genuine site sessions operate on the same observations; a cookie alone
    // never authorizes the public Nostr boundary, and cross-origin writes fail.
    use sha2::{Digest,Sha256};
    let token="synthetic-rating-browser-session"; let hash=Sha256::digest(token.as_bytes()).to_vec();
    state.db.client().await.unwrap().execute("INSERT INTO platform.sessions(id,token_hash,user_id,expires_at) VALUES('RatingSess2',$1,'ReadUser2',clock_timestamp()+interval '1 hour')",&[&hash]).await.unwrap();
    let cookie=format!("{}={token}",state.config.session_cookie_name);
    let client_token="synthetic-rating-read-client"; let client_hash=Sha256::digest(client_token.as_bytes()).to_vec();
    state.db.client().await.unwrap().execute("INSERT INTO platform.api_clients(id,user_id,name,secret_hash,scopes) VALUES('RatingClient2','ReadUser2','Rating fixture',$1,'[\"self.library.read\"]')",&[&client_hash]).await.unwrap();
    assert_eq!(client.get(format!("{origin}/api/rating?manga[]=ReadManga1")).bearer_auth(client_token).send().await.unwrap().status(),200);
    assert_eq!(client.post(format!("{origin}/api/rating/ReadManga1")).bearer_auth(client_token).json(&json!({"rating":1})).send().await.unwrap().status(),403);
    assert_eq!(client.get(format!("{origin}{rating_query}")).header("cookie",&cookie).send().await.unwrap().status(),401);
    let native=client.get(format!("{origin}/api/rating?manga[]=ReadManga1")).header("cookie",&cookie).send().await.unwrap(); assert_eq!(native.status(),200);
    let native=native.json::<Value>().await.unwrap();
    assert_eq!(native["ratings"]["ReadManga1"]["rating"],10); assert_eq!(native["nostr"]["versions"]["ReadManga1"],5);
    assert_eq!(client.get(format!("{origin}/api/rating?manga[]=ReadHidden")).header("cookie",&cookie).send().await.unwrap().status(),404);
    assert_eq!(client.post(format!("{origin}/api/rating/ReadManga1")).header("cookie",&cookie).header("origin","https://foreign.fixture.invalid").json(&json!({"rating":1})).send().await.unwrap().status(),403);
    let native=client.post(format!("{origin}/api/rating/ReadManga1")).header("cookie",&cookie).header("origin",&origin).json(&json!({"rating":9,"version":5})).send().await.unwrap(); assert_eq!(native.status(),200);
    assert_eq!(call(&client,&origin,"GET",&rating_query,None,Some(2),200).await["ratings"][ids["manga"].as_str()]["rating"],9);
    assert_eq!(client.delete(format!("{origin}/api/rating/ReadManga1")).header("cookie",&cookie).header("origin",&origin).json(&json!({"version":5})).send().await.unwrap().status(),409);
    call(&client,&origin,"DELETE",&rating_path,None,Some(2),200).await;
    call(&client,&origin,"DELETE",&rating_path,None,Some(4),200).await;
    let stats=client.get(format!("{origin}/api/v1/statistics/manga/{}",ids["manga"])).send().await.unwrap().json::<Value>().await.unwrap();
    assert!(stats["statistics"][ids["manga"].as_str()]["rating"]["average"].is_null());
    let remaining:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.library_manga_ratings WHERE rating IS NOT NULL",&[]).await.unwrap().get(0); assert_eq!(remaining,0);
    let template=call(&client,&origin,"GET","/api/v1/settings/template",None,Some(2),200).await;
    let template_id=weebdex_platform::public_api::account_settings::TEMPLATE_ID;
    assert_eq!(template["$id"],format!("urn:uuid:{template_id}"));
    assert_eq!(template["properties"]["groups"]["items"]["format"],"uuid");
    let versioned=call(&client,&origin,"GET",&format!("/api/v1/settings/template/{template_id}"),None,Some(2),200).await;
    assert_eq!(template,versioned);
    call(&client,&origin,"GET","/api/v1/settings/template/00000000-0000-4000-8000-000000000001",None,Some(2),404).await;
    call(&client,&origin,"GET","/api/v1/settings",None,Some(2),503).await;
    let stored_preferences=json!({"version":3,"items_per_page":"40","languages":["en","ja"],"translated_languages":["fr"],"show_safe":true,"show_suggestive":false,"show_erotic":true,"show_hentai":false,"data_saver":true,"tags":["ReadTag001"],"groups":["ReadGroup1","ReadGroup2"],"uploaders":["ReadUser4","ReadUser5"],"private_setting":"fixture-secret-value"});
    state.db.client().await.unwrap().execute("UPDATE platform.users SET preferences=$1,updated_at='2024-05-01T12:00:00Z' WHERE id='ReadUser2'",&[&stored_preferences]).await.unwrap();
    let settings=call(&client,&origin,"GET","/api/v1/settings",None,Some(2),200).await;
    assert_eq!(settings["template"],template_id);
    assert_eq!(settings["settings"]["version"],3);
    assert_eq!(settings["settings"]["items_per_page"],"40");
    assert_eq!(settings["updatedAt"],"2024-05-01T12:00:00.000Z");
    assert_eq!(settings["settings"]["groups"],json!([ids["group"]]));
    assert_eq!(settings["settings"]["uploaders"],json!([ids["other"]]));
    assert!(PublicUuid::parse(settings["settings"]["tags"][0].as_str().unwrap()).is_ok());
    assert_eq!(settings["nostr"]["unavailableFilterCounts"],json!({"groups":1,"uploaders":1}));
    assert!(settings["settings"].get("private_setting").is_none());
    let unchanged:Value=state.db.client().await.unwrap().query_one("SELECT preferences FROM platform.users WHERE id='ReadUser2'",&[]).await.unwrap().get(0);
    assert_eq!(unchanged,stored_preferences,"projection must preserve unavailable and unrecognized stored preferences");
    std::fs::write(runtime.join("settings-projection.json"),serde_json::to_vec(&settings["settings"]).unwrap()).unwrap();
    state.db.client().await.unwrap().execute("UPDATE platform.users SET preferences='{\"version\":9,\"items_per_page\":\"10\",\"languages\":[\"ja\"]}' WHERE id='ReadUser4'",&[]).await.unwrap();
    let other_settings=call(&client,&origin,"GET","/api/v1/settings",None,Some(4),200).await;
    assert_eq!(other_settings["settings"]["version"],9);
    assert!(other_settings["settings"].get("groups").is_none());
    // Public settings writes merge only known fields, with native CAS/account
    // proof and preservation of source values omitted by the read projection.
    assert_eq!(settings["nostr"]["accountId"],ids["owner"].as_str());
    call(&client,&origin,"POST","/api/v1/settings",Some(json!({"settings":{"version":3}})),None,401).await;
    for key in [3,5,6,7,8] {call(&client,&origin,"POST","/api/v1/settings",Some(json!({"settings":{"version":3}})),Some(key),403).await;}
    state.db.client().await.unwrap().execute("UPDATE platform.users SET permissions='[]' WHERE id='ReadUser2'",&[]).await.unwrap();
    call(&client,&origin,"POST","/api/v1/settings",Some(json!({"settings":{"version":3}})),Some(2),403).await;
    state.db.client().await.unwrap().execute("UPDATE platform.users SET permissions='[\"self.preferences.update\"]' WHERE id='ReadUser2'",&[]).await.unwrap();
    for value in [
        json!({"settings":{"version":3,"private_setting":"overwrite-secret"}}),
        json!({"settings":{"version":3,"show_safe":null}}),json!({"settings":{"version":3,"groups":null}}),
        json!({"settings":{"version":3,"items_per_page":"020"}}),json!({"settings":{"version":3,"items_per_page":20}}),
        json!({"settings":{"version":3,"languages":["en","en"]}}),json!({"settings":{"version":3,"groups":["ReadGroup1"]}}),
        json!({"settings":{"version":3,"groups":[ids["owner"]]}}),json!({"settings":{"version":3,"groups":["00000000-0000-8000-8000-000000000001"]}}),
        json!({"settings":{"version":3},"updatedAt":"2025-02-29T00:00:00Z"}),
        json!({"settings":{"version":3},"template":"00000000-0000-8000-8000-000000000001"}),
    ] {call(&client,&origin,"POST","/api/v1/settings",Some(value),Some(2),400).await;}
    call(&client,&origin,"POST","/api/v1/settings",Some(json!({"settings":{"version":3},"updatedAt":"2024-05-02T12:00:00Z"})),Some(2),409).await;
    call(&client,&origin,"POST","/api/v1/settings",Some(json!({"settings":{"version":3},"expectedAccountId":ids["other"]})),Some(2),409).await;
    let mut settings_patch=settings["settings"].clone();settings_patch["items_per_page"]=json!("60");settings_patch["groups"]=json!([]);settings_patch["uploaders"]=json!([]);
    let saved=call(&client,&origin,"POST","/api/v1/settings",Some(json!({"settings":settings_patch,"updatedAt":settings["updatedAt"],"expectedAccountId":ids["owner"],"template":template_id})),Some(2),200).await;
    assert_eq!(saved["settings"]["version"],4);assert_eq!(saved["settings"]["items_per_page"],"60");assert_eq!(saved["settings"]["groups"],json!([]));assert_eq!(saved["settings"]["uploaders"],json!([]));
    assert_eq!(saved["nostr"]["changed"],true);assert_eq!(saved["nostr"]["nativeStorageUnchanged"],false);assert_eq!(saved["nostr"]["preservedUnavailableFilterValues"],true);
    assert_eq!(saved["nostr"]["unavailableFilterCounts"],json!({"groups":1,"uploaders":1}));assert!(!saved.to_string().contains("fixture-secret-value"));
    let native_preferences:Value=state.db.client().await.unwrap().query_one("SELECT preferences FROM platform.users WHERE id='ReadUser2'",&[]).await.unwrap().get(0);
    assert_eq!(native_preferences["groups"],json!(["ReadGroup2"]));assert_eq!(native_preferences["uploaders"],json!(["ReadUser5"]));assert_eq!(native_preferences["private_setting"],"fixture-secret-value");assert_eq!(native_preferences["tags"],json!(["ReadTag001"]));
    call(&client,&origin,"POST","/api/v1/settings",Some(json!({"settings":settings_patch})),Some(2),409).await;
    let no_op=call(&client,&origin,"POST","/api/v1/settings",Some(json!({"settings":saved["settings"],"updatedAt":saved["updatedAt"],"expectedAccountId":ids["owner"]})),Some(2),200).await;
    assert_eq!(no_op["nostr"]["changed"],false);assert_eq!(no_op["nostr"]["nativeStorageUnchanged"],true);assert_eq!(no_op["settings"]["version"],4);assert_eq!(no_op["updatedAt"],saved["updatedAt"]);
    std::fs::write(runtime.join("settings-write-request-projection.json"),serde_json::to_vec(&settings_patch).unwrap()).unwrap();
    std::fs::write(runtime.join("settings-write-response-projection.json"),serde_json::to_vec(&saved["settings"]).unwrap()).unwrap();
    assert_eq!(call(&client,&origin,"GET","/api/v1/settings",None,Some(4),200).await["settings"],other_settings["settings"],"another account's settings stay unchanged");
    state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='ReadUser4' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    call(&client,&origin,"POST","/api/v1/settings",Some(json!({"settings":{"version":9,"show_safe":false},"expectedAccountId":ids["owner"]})),Some(2),409).await;
    state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='ReadUser2' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    let settings_url=format!("{origin}/api/v1/settings");
    let bodies=[serde_json::to_vec(&json!({"settings":{"version":4,"show_safe":false},"expectedAccountId":ids["owner"]})).unwrap(),serde_json::to_vec(&json!({"settings":{"version":4,"data_saver":false},"expectedAccountId":ids["owner"]})).unwrap()];
    let requests=bodies.iter().map(|body|client.post(&settings_url).header("content-type","application/json").header("authorization",proof(2,&settings_url,"POST",body)).body(body.clone()).send()).collect::<Vec<_>>();
    let mut requests=requests.into_iter();let (a,b)=tokio::join!(requests.next().unwrap(),requests.next().unwrap());let mut statuses=[a.unwrap().status().as_u16(),b.unwrap().status().as_u16()];statuses.sort();assert_eq!(statuses,[200,409],"one CAS writer wins; no lost preference update");
    let latest=call(&client,&origin,"GET","/api/v1/settings",None,Some(2),200).await;assert_eq!(latest["settings"]["version"],5);
    let raw=b"{\"settings\":{\"version\":5}}";let authorization=proof(2,&settings_url,"POST",raw);
    assert_eq!(client.post(&settings_url).header("content-type","application/json").header("authorization",&authorization).body(raw.as_slice()).send().await.unwrap().status(),200);
    assert_eq!(client.post(&settings_url).header("content-type","application/json").header("authorization",&authorization).body(raw.as_slice()).send().await.unwrap().status(),401);
    assert_eq!(client.post(&settings_url).header("cookie",&cookie).json(&json!({"settings":{"version":5}})).send().await.unwrap().status(),401);
    assert_eq!(client.put(format!("{origin}/api/settings")).bearer_auth(client_token).json(&json!({"version":5,"items_per_page":"55"})).send().await.unwrap().status(),403);
    let native=client.put(format!("{origin}/api/settings")).header("cookie",&cookie).header("origin",&origin).json(&json!({"version":5,"items_per_page":"55"})).send().await.unwrap();assert_eq!(native.status(),200);
    let native_saved=call(&client,&origin,"GET","/api/v1/settings",None,Some(2),200).await;assert_eq!(native_saved["settings"]["version"],6);assert_eq!(native_saved["settings"]["items_per_page"],"55");assert_eq!(native_saved["nostr"]["unavailableFilterCounts"],json!({"groups":1,"uploaders":1}));
    call(&client,&origin,"PUT","/api/v1/settings",Some(json!({"settings":{"version":6}})),Some(2),405).await;
    let oversized=call(&client,&origin,"POST","/api/v1/settings",Some(json!({"settings":{"version":6,"items_per_page":"9".repeat(33000)}})),Some(2),400).await;assert!(oversized["errors"][0]["detail"].as_str().unwrap().contains("32768"));
    // A failure projecting some other malformed known source field after the
    // candidate update still rolls the entire preference transaction back.
    state.db.client().await.unwrap().execute("UPDATE platform.users SET preferences=preferences||'{\"show_safe\":\"malformed-stored-value\"}' WHERE id='ReadUser2'",&[]).await.unwrap();
    let before_failure:Value=state.db.client().await.unwrap().query_one("SELECT preferences FROM platform.users WHERE id='ReadUser2'",&[]).await.unwrap().get(0);
    call(&client,&origin,"POST","/api/v1/settings",Some(json!({"settings":{"version":6,"items_per_page":"70"}})),Some(2),503).await;
    let after_failure:Value=state.db.client().await.unwrap().query_one("SELECT preferences FROM platform.users WHERE id='ReadUser2'",&[]).await.unwrap().get(0);assert_eq!(before_failure,after_failure,"projection failure must roll back candidate preference changes");
    state.db.client().await.unwrap().execute("UPDATE platform.users SET preferences=preferences||jsonb_build_object('show_safe',$1::jsonb) WHERE id='ReadUser2'",&[&native_saved["settings"]["show_safe"]]).await.unwrap();
    let history=call(&client,&origin,"GET","/api/v1/user/history",None,Some(2),200).await;
    assert_eq!(history["ratings"].as_array().unwrap().len(),2);
    let safe_read=history["ratings"].as_array().unwrap().iter().find(|row|row["chapterId"]==ids["chapter"].as_str()).unwrap();
    assert_eq!(safe_read["readDate"],"2024-01-01T12:00:00.000Z");
    assert!(history["ratings"].as_array().unwrap().iter().all(|row|PublicUuid::parse(row["chapterId"].as_str().unwrap()).is_ok()));
    let other_history=call(&client,&origin,"GET","/api/v1/user/history",None,Some(4),200).await;
    assert_eq!(other_history["ratings"].as_array().unwrap().len(),1);
    assert_eq!(other_history["ratings"][0]["readDate"],"2024-04-01T12:00:00.000Z");
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
    for path in ["/api/v1/user/me?limit=1","/api/v1/user/history?limit=1","/api/v1/settings?limit=1","/api/v1/settings/template?limit=1","/api/v1/settings/template/not-a-uuid","/api/v1/user/follows/manga?limit=0","/api/v1/user/follows/manga/feed?limit=0","/api/v1/user?ids[]=ReadUser2"] {call(&client,&origin,"GET",path,None,Some(2),400).await;}

    exercise_follow_writes(&client,&origin,&state,&ids,&alias,&cookie,client_token).await;

    // Request, method, service and nonce are actual original capability checks.
    let url=format!("{origin}/api/v1/user/me");let authorization=proof(2,&url,"GET",b"");
    assert_eq!(client.get(&url).header("authorization",&authorization).send().await.unwrap().status(),200);
    assert_eq!(client.get(&url).header("authorization",&authorization).send().await.unwrap().status(),401);
    assert_eq!(client.get(&url).header("authorization",proof(2,&format!("{origin}/api/v1/user"),"GET",b"")).send().await.unwrap().status(),401);
    assert_eq!(client.get(&url).header("cookie","session=untrusted-cookie").send().await.unwrap().status(),401);
    let aliases_after:i64=state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);
    assert_eq!(aliases_after,aliases_before+2,"only the explicit alias and fixture native client create aliases; rating observations do not");
    original.set_account_admission(id(2),false).await.unwrap();
    call(&client,&origin,"GET","/api/v1/user/follows/manga",None,Some(2),403).await;
    call(&client,&origin,"POST",&rating_path,Some(json!({"rating":8})),Some(2),403).await;
    call(&client,&origin,"POST",&format!("/api/v1/manga/{alias}/follow"),None,Some(2),403).await;
    original.set_account_admission(id(2),true).await.unwrap();
    state.db.client().await.unwrap().execute("DELETE FROM platform.protocol_identity_bindings WHERE publisher=$1",&[&public(2)]).await.unwrap();
    call(&client,&origin,"GET","/api/v1/user/me",None,Some(2),403).await;
    call(&client,&origin,"GET",&rating_query,None,Some(2),403).await;
}

// This is a route-boundary check, not successful operation acceptance. No
// request has a cookie, key or proof; mutation bodies are deliberately empty.
async fn exercise_inventory_routing(client:&reqwest::Client,origin:&str,runtime:&std::path::Path) {
    let inventory:Value=serde_json::from_str(include_str!("../../contracts/mangadex-5.13.1-inventory.json")).unwrap();
    let snapshot:Value=serde_json::from_str(include_str!("../../docs/mangadex-operation-compatibility-snapshot.json")).unwrap();
    assert_eq!(inventory["source_sha256"],snapshot["inventory_source_sha256"]);
    let declared=snapshot["operations"].as_array().unwrap();
    let unknown=client.get(format!("{origin}/api/v1/__inventory_probe_unknown")).send().await.unwrap();
    assert_eq!(unknown.status(),404);
    let unknown:Value=unknown.json().await.unwrap();
    assert_eq!(unknown["errors"][0]["detail"],"Unknown public API endpoint");
    let wrong_method=client.post(format!("{origin}/api/v1/ping")).send().await.unwrap();
    assert_eq!(wrong_method.status(),405);
    let mut seen=std::collections::BTreeSet::new();
    let mut observations=Vec::new();
    for operation in inventory["operations"].as_array().unwrap() {
        let method=operation["method"].as_str().unwrap();
        let pattern=operation["path"].as_str().unwrap();
        assert!(seen.insert((method,pattern)),"duplicate inventory operation");
        let row=declared.iter().find(|row|row["method"]==method&&row["path"]==pattern).expect("every operation classified");
        let path=pattern.split('/').map(|part|if part.starts_with('{') {
            if part=="{category}" {"manga"} else {"ffffffff-ffff-8fff-8fff-ffffffffffff"}
        } else {part}).collect::<Vec<_>>().join("/");
        let mut request=client.request(method.parse().unwrap(),format!("{origin}/api/v1{path}"));
        if method!="GET" {request=request.header("content-type","application/json").body("{}");}
        let response=request.send().await.unwrap();
        let status=response.status().as_u16();
        let bytes=response.bytes().await.unwrap();
        assert!(bytes.len()<=1024*1024,"bounded response: {method} {pattern}");
        let unsupported=row["status"].as_str().unwrap().contains("501");
        assert_eq!(status==501,unsupported,"routing classification {method} {pattern}: {status} {}",String::from_utf8_lossy(&bytes));
        if !unsupported {
            assert!(status<500,"unexpected backend failure {method} {pattern}: {status}");
            assert_ne!(status,405,"declared operation reached wrong-method fallback: {method} {pattern}");
        }
        if pattern=="/ping" {assert_eq!(bytes.as_ref(),b"pong");} else {
            let json:Value=serde_json::from_slice(&bytes).unwrap_or_else(|_|panic!("non-JSON {method} {pattern}: {status}"));
            if status>=400 {assert_eq!(json["result"],"error");assert!(json["errors"].is_array());}
            assert!(!json["errors"].as_array().is_some_and(|errors|errors.iter().any(|error|error["detail"]=="Unknown public API endpoint")),"declared operation reached unknown-route fallback: {method} {pattern}");
        }
        observations.push(json!({"method":method,"path":pattern,"operation_id":operation["operation_id"],"classification":row["status"],"http_status":status,"response_bytes":bytes.len(),"response_sha256":auth::hash(&bytes)}));
    }
    assert_eq!(seen.len(),113);assert_eq!(declared.len(),113);
    let result=json!({"source_baseline":snapshot["source_baseline"],"inventory_sha256":inventory["source_sha256"],"operation_count":113,"scope":"actual unauthenticated routing/auth/validation only; no successful mutation or full compatibility claim","fallback_controls":{"unknown_path":404,"wrong_method":405,"generic_unknown_route_detail_rejected":true},"observations":observations});
    let bytes=serde_json::to_vec_pretty(&result).unwrap();
    std::fs::write(runtime.join("mangadex-operation-routing-probe.json"),&bytes).unwrap();
    println!("113-operation routing probe passed; sha256={}",auth::hash(&bytes));
}

async fn exercise_follow_writes(client:&reqwest::Client,origin:&str,state:&AppState,ids:&std::collections::BTreeMap<&str,weebdex_platform::public_api::PublicUuid>,alias:&weebdex_platform::public_api::PublicUuid,cookie:&str,client_token:&str) {
    // Migration baseline preserves real source time for existing native follows.
    let baseline=state.db.client().await.unwrap().query_one("SELECT r.version,r.updated_at=f.followed_at FROM platform.library_follow_revisions r JOIN platform.library_manga_follows f ON f.user_id=r.user_id AND f.manga_id=r.target_id WHERE r.user_id='ReadUser2' AND r.kind='manga' AND r.target_id='ReadManga1'",&[]).await.unwrap();
    assert_eq!(baseline.get::<_,i32>(0),1);assert!(baseline.get::<_,bool>(1));
    state.db.client().await.unwrap().execute("UPDATE platform.library_manga_follows SET reading_status='completed',notification=false,is_private=false WHERE user_id='ReadUser2' AND manga_id='ReadManga1'",&[]).await.unwrap();
    for (kind,key,target) in [("manga","manga",alias),("group","group",&ids["group"]),("user","other",&ids["other"])] {
        let path=format!("/api/v1/{kind}/{target}/follow");let status_path=format!("/api/v1/user/follows/{kind}/{target}");
        call(client,origin,"POST",&path,None,None,401).await;
        for k in [3,5,6,7,8] {call(client,origin,"POST",&path,None,Some(k),403).await;}
        let before=call(client,origin,"GET",&status_path,None,Some(2),200).await;
        assert_eq!(before["nostr"]["version"],1);assert_eq!(before["nostr"]["accountId"],ids["owner"].as_str());assert_eq!(before["nostr"]["targetId"],ids[key].as_str());
        let noop=call(client,origin,"POST",&path,Some(json!({"version":1,"expectedAccountId":ids["owner"]})),Some(2),200).await;
        assert_eq!(noop["nostr"]["changed"],false);assert_eq!(noop["nostr"]["updatedAt"],before["nostr"]["updatedAt"]);
        call(client,origin,"DELETE",&path,Some(json!({"version":0})),Some(2),409).await;
        call(client,origin,"DELETE",&path,Some(json!({"version":1,"expectedAccountId":ids["other"]})),Some(2),409).await;
        if kind=="manga" {
            let row=state.db.client().await.unwrap().query_one("SELECT reading_status,notification,is_private FROM platform.library_manga_follows WHERE user_id='ReadUser2' AND manga_id='ReadManga1'",&[]).await.unwrap();
            assert_eq!(row.get::<_,&str>(0),"completed");assert!(!row.get::<_,bool>(1));assert!(!row.get::<_,bool>(2));
        }
        let removed=call(client,origin,"DELETE",&path,Some(json!({"version":1,"expectedAccountId":ids["owner"]})),Some(2),200).await;
        assert_eq!(removed["nostr"]["version"],2);assert_eq!(removed["nostr"]["followed"],false);assert_eq!(removed["nostr"]["changed"],true);
        call(client,origin,"GET",&status_path,None,Some(2),404).await;
        let same=call(client,origin,"DELETE",&path,Some(json!({"version":2})),Some(2),200).await;assert_eq!(same["nostr"]["version"],2);assert_eq!(same["nostr"]["changed"],false);assert_eq!(same["nostr"]["updatedAt"],removed["nostr"]["updatedAt"]);
        call(client,origin,"POST",&path,Some(json!({"version":1})),Some(2),409).await;
        let added=call(client,origin,"POST",&path,Some(json!({"version":2,"expectedAccountId":ids["owner"]})),Some(2),200).await;
        assert_eq!(added["nostr"]["version"],3);assert_eq!(added["nostr"]["followed"],true);
        assert_eq!(client.post(format!("{origin}{path}")).header("cookie",cookie).send().await.unwrap().status(),401);
        let _=call(client,origin,"GET",&status_path,None,Some(2),200).await;
    }
    let row=state.db.client().await.unwrap().query_one("SELECT reading_status,notification,is_private FROM platform.library_manga_follows WHERE user_id='ReadUser2' AND manga_id='ReadManga1'",&[]).await.unwrap();
    assert_eq!(row.get::<_,&str>(0),"plan_to_read");assert!(row.get::<_,bool>(1));assert!(row.get::<_,bool>(2));
    for (kind,native,key) in [("groups","ReadGroup1","group"),("users","ReadUser4","other")] {
        let native_path=format!("{origin}/api/user/follows/{kind}/{native}");
        assert_eq!(client.delete(&native_path).header("cookie",cookie).header("origin",origin).send().await.unwrap().status(),204);
        assert_eq!(client.post(&native_path).header("cookie",cookie).header("origin",origin).send().await.unwrap().status(),204);
        let kind=if kind=="groups"{"group"}else{"user"};
        let p=format!("/api/v1/{kind}/{}/follow",ids[key]);call(client,origin,"DELETE",&p,Some(json!({"version":3})),Some(2),409).await;
        let p=format!("/api/v1/user/follows/{kind}/{}",ids[key]);assert_eq!(call(client,origin,"GET",&p,None,Some(2),200).await["nostr"]["version"],5);
    }
    let new_path=format!("/api/v1/manga/{}/follow",ids["other_manga"]);
    assert_eq!(call(client,origin,"POST",&new_path,None,Some(2),200).await["nostr"]["version"],1);
    assert_eq!(call(client,origin,"DELETE",&new_path,None,Some(2),200).await["nostr"]["version"],2);
    let path=format!("/api/v1/manga/{alias}/follow");
    state.db.client().await.unwrap().execute("UPDATE platform.users SET permissions='[]' WHERE id='ReadUser2'",&[]).await.unwrap();
    call(client,origin,"DELETE",&path,None,Some(2),403).await;
    state.db.client().await.unwrap().execute("UPDATE platform.users SET permissions='[\"self.preferences.update\"]' WHERE id='ReadUser2'",&[]).await.unwrap();
    state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='ReadUser4' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    call(client,origin,"POST",&path,Some(json!({"version":0,"expectedAccountId":ids["owner"]})),Some(2),409).await;
    state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='ReadUser2' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    for (kind,key) in [("manga","hidden"),("group","hidden_group"),("user","disabled")] {
        let p=format!("/api/v1/{kind}/{}/follow",ids[key]);call(client,origin,"POST",&p,None,Some(2),404).await;
        let removed=call(client,origin,"DELETE",&p,Some(json!({"version":1,"expectedAccountId":ids["owner"]})),Some(2),200).await;
        assert_eq!(removed["nostr"]["version"],2,"own hidden memberships remain removable without source metadata");
    }
    call(client,origin,"POST",&format!("/api/v1/user/{}/follow",ids["owner"]),None,Some(2),400).await;
    call(client,origin,"POST",&format!("/api/v1/group/{}/follow",ids["manga"]),None,Some(2),404).await;
    call(client,origin,"POST",&format!("/api/v1/user/{}/follow",ids["publisher"]),None,Some(2),404).await;
    call(client,origin,"POST",&path,Some(json!({"version":-1})),Some(2),400).await;
    call(client,origin,"POST",&path,Some(json!({"version":null})),Some(2),400).await;
    call(client,origin,"POST",&path,Some(json!({"version":3,"private":false})),Some(2),400).await;
    call(client,origin,"POST",&format!("{path}?version=3"),None,Some(2),400).await;
    let url=format!("{origin}{path}");let raw=b"{\"version\":3}";let authorization=proof(2,&url,"POST",raw);
    assert_eq!(client.post(&url).header("content-type","application/json").header("authorization",&authorization).body(raw.as_slice()).send().await.unwrap().status(),200);
    assert_eq!(client.post(&url).header("content-type","application/json").header("authorization",&authorization).body(raw.as_slice()).send().await.unwrap().status(),401);
    let raw=b"{\"version\":3}";let authorization=proof(2,&url,"DELETE",raw);
    assert_eq!(client.post(&url).header("content-type","application/json").header("authorization",&authorization).body(raw.as_slice()).send().await.unwrap().status(),401);
    // Native session mutations use the same tables and automatically advance
    // membership CAS, while changing reading status alone keeps membership CAS.
    assert_eq!(client.post(format!("{origin}/api/user/follows/manga/ReadManga1")).bearer_auth(client_token).json(&json!({"status":"reading"})).send().await.unwrap().status(),403);
    assert_eq!(client.post(format!("{origin}/api/user/follows/manga/ReadManga1")).header("cookie",cookie).header("origin",origin).json(&json!({"status":"reading","notification":false})).send().await.unwrap().status(),204);
    let status=format!("/api/v1/user/follows/manga/{alias}");assert_eq!(call(client,origin,"GET",&status,None,Some(2),200).await["nostr"]["version"],3);
    assert_eq!(client.delete(format!("{origin}/api/user/follows/manga/ReadManga1")).header("cookie",cookie).header("origin",origin).send().await.unwrap().status(),204);
    call(client,origin,"POST",&path,Some(json!({"version":3})),Some(2),409).await;
    let added=call(client,origin,"POST",&path,Some(json!({"version":4})),Some(2),200).await;assert_eq!(added["nostr"]["version"],5);
    let before:i64=state.db.client().await.unwrap().query_one("SELECT follows FROM platform.catalog_manga_counters WHERE manga_id='ReadManga1'",&[]).await.unwrap().get(0);
    let raw=serde_json::to_vec(&json!({"version":5,"expectedAccountId":ids["owner"]})).unwrap();
    let a=client.delete(&url).header("content-type","application/json").header("authorization",proof(2,&url,"DELETE",&raw)).body(raw.clone()).send();
    let b=client.delete(&url).header("content-type","application/json").header("authorization",proof(2,&url,"DELETE",&raw)).body(raw).send();
    let (a,b)=tokio::join!(a,b);let mut statuses=[a.unwrap().status().as_u16(),b.unwrap().status().as_u16()];statuses.sort();assert_eq!(statuses,[200,409]);
    let after:i64=state.db.client().await.unwrap().query_one("SELECT follows FROM platform.catalog_manga_counters WHERE manga_id='ReadManga1'",&[]).await.unwrap().get(0);assert_eq!(after,(before-1).max(0));
    assert_eq!(call(client,origin,"GET","/api/v1/user/follows/manga",None,Some(4),200).await["total"],1,"other private account membership stays unchanged");
}
