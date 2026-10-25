//! Actual loopback HTTP + disposable PostgreSQL: native links, durable aliases,
//! public profile fields and persisted discussion context. No schema expansion,
//! publishing, invented users/threads, signing identity or account authority.
use serde_json::{Value,json};
use std::time::Duration;
use weebdex_platform::{AppState,Config,domains,public_api::{PostgresPublicIdResolver,PublicIdResolver,PublicUuid,ResourceKind}};
const USER:&str="LegacyVisibleUser";
const OTHER:&str="LegacyOtherUser";
const HIDDEN:&str="LegacyHiddenUser";
const UUID_NATIVE:&str="bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const NON_ACCOUNT_UUID_NATIVE:&str="bbbbbbbb-bbbb-8bbb-8bbb-bbbbbbbbbbbb";
const ALIAS:&str="aaaaaaaa-aaaa-8aaa-8aaa-aaaaaaaaaaaa";
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server{fn drop(&mut self){self.0.abort();}}
async fn fixture()->AppState{
    let dsn=std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use scripts/run-auth-tests.sh");
    let pg:tokio_postgres::Config=dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(),Some("weebdex_platform_test"));assert!(!pg.get_hosts().is_empty());
    assert!(pg.get_hosts().iter().all(|h|matches!(h,tokio_postgres::config::Host::Tcp(h) if h=="127.0.0.1")));
    assert!(pg.get_ports().iter().all(|p|*p>1024&&*p!=5432));
    let runtime=std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap();assert!(runtime.contains("/weebdex-platform/test-runtime/"));assert!(std::path::Path::new(&runtime).join("fixture.json").is_file());
    let state=AppState::connect(Config{database_plaintext_local:true,database_connections:4,..Config::default()},dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();state.db.verify_schema(domains::MIGRATIONS).await.unwrap();state
}
async fn call(client:&reqwest::Client,base:&str,path:&str,body:Option<Value>,expected:u16)->Value{
    let request=if let Some(body)=body{client.post(format!("{base}{path}")).json(&body)}else{client.get(format!("{base}{path}"))};
    let response=request.send().await.unwrap();let status=response.status().as_u16();
    assert!(response.headers()["content-type"].to_str().unwrap().starts_with("application/json"));
    let value:Value=response.json().await.unwrap();assert_eq!(status,expected,"{path}: {value}");value
}
#[tokio::test]
#[ignore="Requires isolated scripts/run-auth-tests.sh PostgreSQL"]
async fn actual_native_user_links_aliases_profile_and_discussion_remain_same_visible_resource(){
    let state=fixture().await;let db=state.db.client().await.unwrap();
    for user in [USER,OTHER,HIDDEN,UUID_NATIVE,NON_ACCOUNT_UUID_NATIVE]{let username=if user==UUID_NATIVE{"UuidNativeFixture"}else if user==NON_ACCOUNT_UUID_NATIVE{"NonAccountUuidFixture"}else{user};db.execute("INSERT INTO platform.users(id,username,email,password_hash,disabled,profile) VALUES($1,$2,$3,'synthetic-unused',$4,$5)",&[&user,&username,&format!("{user}@fixture.invalid"),&(user==HIDDEN),&json!({"description":"Actual public biography","privateSentinel":"never expose","email":"private-profile@fixture.invalid"})]).await.unwrap();}
    // Deliberately use a stored thread id that cannot be guessed from the user id.
    db.execute("UPDATE platform.catalog_threads SET id='actual-persisted-profile-discussion',subject='Actual stored profile discussion' WHERE entity_type='user' AND entity_id=$1",&[&USER]).await.unwrap();
    db.execute("INSERT INTO platform.community_replies(thread_id,reply_number,author_id,body) VALUES('actual-persisted-profile-discussion',1,$1,'Actual persisted profile reply')",&[&OTHER]).await.unwrap();
    let resolver=PostgresPublicIdResolver::new(state.db.clone());
    let public=resolver.preferred(ResourceKind::User,USER).await.unwrap().unwrap();let hidden=resolver.preferred(ResourceKind::User,HIDDEN).await.unwrap().unwrap();let other=resolver.preferred(ResourceKind::User,OTHER).await.unwrap().unwrap();
    let non_account_public=resolver.preferred(ResourceKind::User,NON_ACCOUNT_UUID_NATIVE).await.unwrap().unwrap();
    assert_ne!(non_account_public.as_str(),NON_ACCOUNT_UUID_NATIVE,"Only existing v4 account identifiers are self aliases; other native keys retain their stored public aliases");
    resolver.register_alias(ResourceKind::User,USER,&PublicUuid::parse(ALIAS).unwrap()).await.unwrap();
    let publisher_key="a".repeat(64);let publisher:String=db.query_one("SELECT platform.public_register_protocol_publisher($1)::text",&[&publisher_key]).await.unwrap().get(0);
    let namespace:String=db.query_one("SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton",&[]).await.unwrap().get(0);
    let snapshot=db.query_one("SELECT (SELECT count(*) FROM platform.public_resource_ids),(SELECT count(*) FROM platform.users),(SELECT count(*) FROM platform.catalog_threads)",&[]).await.unwrap();let before:(i64,i64,i64)=(snapshot.get(0),snapshot.get(1),snapshot.get(2));drop(db);
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let base=format!("http://{}",listener.local_addr().unwrap());let app=weebdex_platform::router::router(state.clone());let _server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap();}));let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build().unwrap();
    let references=json!([{"kind":"user","id":USER},{"kind":"user","id":ALIAS},{"kind":"user","id":UUID_NATIVE},{"kind":"user","id":NON_ACCOUNT_UUID_NATIVE},{"kind":"user","id":HIDDEN},{"kind":"user","id":"MissingNativeUser"},{"kind":"user","id":"npub1arbitrarynotanativeaccount"},{"kind":"user","id":format!("protocol:{publisher_key}")},{"kind":"user","id":publisher}]);
    let mapping=call(&client,&base,"/api/v1/legacy/resolve",Some(json!({"catalog_namespace":namespace,"resources":references})),200).await;
    assert_eq!(mapping["data"],json!([{"kind":"user","legacy_id":USER,"id":public,"identity":"native_public_user"},{"kind":"user","legacy_id":ALIAS,"id":public,"identity":"native_public_user"},{"kind":"user","legacy_id":UUID_NATIVE,"id":UUID_NATIVE,"identity":"native_public_user"}]));assert_eq!(mapping["unresolved"].as_array().unwrap().len(),6);
    let profile=call(&client,&base,&format!("/api/v1/user/{public}"),None,200).await;
    assert_eq!(profile["data"]["id"],public.as_str());assert_eq!(profile["data"]["nostr"]["identity"],"native_public_user");assert_eq!(profile["data"]["attributes"],json!({"username":USER,"roles":["user"],"version":1,"description":"Actual public biography"}));
    assert_eq!(call(&client,&base,&format!("/api/v1/user/{ALIAS}"),None,200).await,profile);
    assert_eq!(call(&client,&base,&format!("/api/v1/user/{UUID_NATIVE}"),None,200).await["data"]["id"],UUID_NATIVE);
    // A UUID input always means a public alias, never fallback native-key lookup.
    call(&client,&base,&format!("/api/v1/user/{NON_ACCOUNT_UUID_NATIVE}"),None,404).await;
    assert_eq!(call(&client,&base,&format!("/api/v1/user/{non_account_public}"),None,200).await["data"]["id"],non_account_public.as_str());
    call(&client,&base,&format!("/api/v1/user/{hidden}"),None,404).await;
    call(&client,&base,&format!("/api/v1/user/{USER}"),None,400).await;
    let contexts=call(&client,&base,"/api/v1/nostr/native-forum/contexts",Some(json!({"resources":[{"kind":"user","id":public},{"kind":"user","id":hidden},{"kind":"user","id":publisher}]})),200).await;
    assert_eq!(contexts["data"].as_array().unwrap().len(),1);assert_eq!(contexts["unavailable"].as_array().unwrap().len(),2);
    let thread=&contexts["data"][0]["thread"];assert_eq!(thread["id"],"actual-persisted-profile-discussion");assert_eq!(thread["subject"],"Actual stored profile discussion");assert_eq!(thread["relationships"]["stats"]["replies"],1);
    let native=call(&client,&base,&format!("/api/user/{USER}"),None,200).await;assert_eq!(&native["relationships"]["thread"],thread,"public UUID context uses exact same persisted native discussion");
    let replies=call(&client,&base,"/api/thread/actual-persisted-profile-discussion/replies",None,200).await;assert_eq!(replies["data"][0]["body"],"Actual persisted profile reply");
    for value in [&mapping,&profile,&contexts]{let text=value.to_string();for private in ["privateSentinel","private-profile","fixture.invalid","password_hash","permissions"]{assert!(!text.contains(private),"private data leaked: {text}");}}
    call(&client,&base,"/api/v1/legacy/resolve",Some(json!({"catalog_namespace":ALIAS,"resources":[{"kind":"user","id":USER}]})),422).await;
    for bad in [json!({"kind":"user","id":"../escape"}),json!({"kind":"user","id":""}),json!({"kind":"npub","id":USER})]{call(&client,&base,"/api/v1/legacy/resolve",Some(json!({"catalog_namespace":namespace,"resources":[bad]})),400).await;}
    call(&client,&base,"/api/v1/legacy/resolve?foo=bar",Some(json!({"catalog_namespace":namespace,"resources":[{"kind":"user","id":USER}]})),400).await;
    // Missing stored thread remains absent; a GET/resolve never recreates it.
    state.db.client().await.unwrap().execute("DELETE FROM platform.catalog_threads WHERE entity_type='user' AND entity_id=$1",&[&OTHER]).await.unwrap();
    let absent=call(&client,&base,"/api/v1/nostr/native-forum/contexts",Some(json!({"resources":[{"kind":"user","id":other}]})),200).await;assert_eq!(absent["data"].as_array().unwrap().len(),1);assert!(absent["data"][0]["thread"].is_null());
    // Visibility is checked again after an earlier successful alias resolution.
    state.db.client().await.unwrap().execute("UPDATE platform.users SET disabled=TRUE WHERE id=$1",&[&USER]).await.unwrap();
    let hidden_later=call(&client,&base,"/api/v1/legacy/resolve",Some(json!({"catalog_namespace":namespace,"resources":[{"kind":"user","id":USER}]})),200).await;assert_eq!(hidden_later["data"],json!([]));
    let hidden_context=call(&client,&base,"/api/v1/nostr/native-forum/contexts",Some(json!({"resources":[{"kind":"user","id":public}]})),200).await;assert_eq!(hidden_context["data"],json!([]));call(&client,&base,&format!("/api/v1/user/{public}"),None,404).await;
    let db=state.db.client().await.unwrap();let after=db.query_one("SELECT (SELECT count(*) FROM platform.public_resource_ids),(SELECT count(*) FROM platform.users),(SELECT count(*) FROM platform.catalog_threads)",&[]).await.unwrap();assert_eq!((after.get::<_,i64>(0),after.get::<_,i64>(1),after.get::<_,i64>(2)),(before.0,before.1,before.2-1));
}
