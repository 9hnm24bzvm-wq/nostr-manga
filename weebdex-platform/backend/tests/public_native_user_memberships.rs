//! Synthetic actual TCP/PostgreSQL membership privacy, keyset and identity seams.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use manga_node::{auth, db::{DbConfig, Id, Repository}};
use serde_json::{Value,json};
use std::time::Duration;
use weebdex_platform::{AppState, Config, domains, protocol::publication::OperatorRuntimeConfig};
fn public(key:u8)->String { auth::sign(&format!("{key:064x}"),1,vec![],String::new()).unwrap().pubkey }
fn id(key:u8)->Id {hex::decode(public(key)).unwrap().try_into().unwrap()}
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {fn drop(&mut self){self.0.abort();}}
async fn get(client:&reqwest::Client,origin:&str,path:&str,status:u16)->Value {
    let response=client.get(format!("{origin}{path}")).send().await.unwrap();
    let actual=response.status().as_u16();
    let bytes=response.bytes().await.unwrap();
    assert_eq!(actual,status,"{path}: {}",String::from_utf8_lossy(&bytes));assert!(bytes.len()<=65536);
    serde_json::from_slice(&bytes).unwrap()
}
fn cursor(user:&str,group:&str,name:&str)->String {
    URL_SAFE_NO_PAD.encode(format!("{{\"version\":1,\"userId\":{},\"groupId\":{},\"name\":{}}}",json!(user),json!(group),json!(name)))
}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires isolated scripts/run-auth-tests.sh PostgreSQL"]
async fn public_memberships_are_visible_target_only_bounded_live_native_order() {
    use weebdex_platform::public_api::{PostgresPublicIdResolver,PublicIdResolver,ResourceKind,PublicUuid};
    let dsn=std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use isolated runner");
    let pg:tokio_postgres::Config=dsn.parse().unwrap();assert_eq!(pg.get_dbname(),Some("weebdex_platform_test"));assert!(pg.get_hosts().iter().all(|h|matches!(h,tokio_postgres::config::Host::Tcp(h) if h=="127.0.0.1")));
    let runtime=std::path::PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap());assert!(runtime.to_str().unwrap().contains("/weebdex-platform/test-runtime/"));
    let original=Repository::connect(DbConfig{dsn:dsn.clone(),connections:2,acquisition_timeout:Duration::from_secs(2),cache_weight:1048576,plaintext_local:true}).await.unwrap();original.migrate().await.unwrap();original.bind_instance(id(11),id(1)).await.unwrap();for key in [2,3,4,5] {original.set_account_admission(id(key),true).await.unwrap();}
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let origin=format!("http://{}",listener.local_addr().unwrap());
    let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:6,uploads_dir:runtime.join("media"),ui_dir:runtime.join("no-ui"),protocol_publication:Some(OperatorRuntimeConfig{operator_key_file:runtime.join("unused/operator.key"),expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),storage:json!({"path":runtime.join("unused/storage"),"volume":"membershipfixture","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144}),group_keys_dir:None,backend:None,native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":runtime.join("unused/service.key"),"admin_pubkey":public(1)}))}),..Config::default()},dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();state.protocol.verify_schema().await.unwrap();
    assert!(!pg.get_hosts().is_empty()); assert!(pg.get_ports().iter().all(|p|*p>1024&&*p!=5432)); assert!(runtime.join("fixture.json").is_file());
    let db=state.db.client().await.unwrap();
    for (user,disabled) in [("MembershipUser",false),("MembershipOther",false),("MembershipClosed",true),("154965c1-6a5d-4e15-9a87-b783db928ba5",false)] {
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,disabled,profile) VALUES($1,$5,$2,'synthetic',TRUE,$3,$4)",&[&user,&format!("{user}@fixture.invalid"),&disabled,&json!({"email":"private@fixture.invalid","private_key":"not-public"}),&if user.len()>32 {"FreshReader"}else{user}]).await.unwrap();
    }
    let resolver=PostgresPublicIdResolver::new(state.db.clone());
    let owner=resolver.preferred(ResourceKind::User,"MembershipUser").await.unwrap().unwrap();
    let other=resolver.preferred(ResourceKind::User,"MembershipOther").await.unwrap().unwrap();
    let closed=resolver.preferred(ResourceKind::User,"MembershipClosed").await.unwrap().unwrap();
    let alias=PublicUuid::parse("aaaaaaaa-aaaa-8aaa-8aaa-aaaaaaaaaaaa").unwrap();resolver.register_alias(ResourceKind::User,"MembershipUser",&alias).await.unwrap();
    let publisher:String=db.query_one("SELECT platform.public_register_protocol_publisher($1)::text",&[&public(2)]).await.unwrap().get(0);
    for n in 0..38 {
        let native=format!("MGroup{n:04}");let name=match n {0..=2=>"Same".to_owned(),3=>"Équipe".to_owned(),4=>"界".repeat(200),5|7=>"\u{1}".repeat(200),6|8=>"🦀".repeat(200),_=>format!("Group {n:02}")};
        db.execute("INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name,metadata,inactive) VALUES($1,'MembershipUser','Owner',$2,$3,$4)",&[&native,&name,&json!({"private_key":"not-public","contact_email":"private@fixture.invalid","members":["private"]}),&(n==8)]).await.unwrap();
        for user in ["MembershipUser","MembershipOther"] {db.execute("INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader,is_officer) VALUES($1,$2,$2,$3,$4)",&[&native,&user,&(user=="MembershipUser"&&n==0),&(user=="MembershipUser"&&n==1)]).await.unwrap();}
    }
    db.execute("UPDATE platform.catalog_groups SET deleted_at=clock_timestamp() WHERE id='MGroup0037'",&[]).await.unwrap();
    db.execute("INSERT INTO platform.protocol_resource_ids(kind,source_target,compatibility_id,catalog_id) VALUES('group',$1,'33333333-3333-8333-8333-333333333333','MGroup0036')",&[&format!("{}:fixture",public(1))]).await.unwrap();
    let expected=db.query("SELECT a.public_id::text,g.name,gm.is_leader,gm.is_officer FROM platform.catalog_group_members gm JOIN platform.catalog_groups g ON g.id=gm.group_id JOIN platform.public_resource_ids a ON a.kind='group' AND a.native_id=g.id AND a.preferred WHERE gm.user_id='MembershipUser' AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id) ORDER BY g.name,g.id",&[]).await.unwrap();
    assert_eq!(expected.len(),36);let expected_ids=expected.iter().map(|row|row.get::<_,String>(0)).collect::<Vec<_>>();
    let aliases:i64=db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);drop(db);
    let app=weebdex_platform::router::router(state.clone());let _server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap();}));
    let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build().unwrap();let base=format!("/api/v1/nostr/user/{owner}/memberships");
    // Fresh native readers use UUID IDs, unlike the historical fixture names.
    let fresh="154965c1-6a5d-4e15-9a87-b783db928ba5";
    let fresh_public=resolver.preferred(ResourceKind::User,fresh).await.unwrap().unwrap();
    let fresh_profile=get(&client,&origin,&format!("/api/v1/user/{fresh_public}"),200).await;
    assert_eq!(fresh_profile["data"]["id"],fresh_public.as_str());
    let empty=get(&client,&origin,&format!("/api/v1/nostr/user/{fresh_public}/memberships?limit=32"),200).await;
    assert_eq!(empty["data"],json!([]));assert!(empty["nextCursor"].is_null());
    assert_eq!(empty["nostr"]["userId"],fresh_public.as_str());
    let first=get(&client,&origin,&base,200).await;assert_eq!(first["data"].as_array().unwrap().len(),32);assert_eq!(first["nostr"]["consistency"],"live-keyset-v1");assert!(first.get("total").is_none());
    // Real server-emitted one-row continuations exercise same-name tie order,
    // multibyte DB collation and the maximum escaped cursor roundtrip.
    let mut one_cursor:Option<String>=None;let mut small_ids=Vec::new();let mut escaped_roundtrip=false;let mut utf8_roundtrip=false;
    for _ in 0..40 {
        let path=match &one_cursor {Some(token)=>format!("{base}?limit=1&after={token}"),None=>format!("{base}?limit=1")};
        let value=get(&client,&origin,&path,200).await;let rows=value["data"].as_array().unwrap();assert_eq!(rows.len(),1);
        small_ids.push(rows[0]["id"].as_str().unwrap().to_owned());
        if let Some(token)=value["nextCursor"].as_str() {assert!(token.len()<=2048);let name=rows[0]["attributes"]["name"].as_str().unwrap();escaped_roundtrip|=name=="\u{1}".repeat(200);utf8_roundtrip|=name=="🦀".repeat(200);one_cursor=Some(token.to_owned());} else {break;}
    }
    assert_eq!(small_ids,expected_ids);assert!(escaped_roundtrip&&utf8_roundtrip);
    let token=first["nextCursor"].as_str().unwrap();let second=get(&client,&origin,&format!("{base}?after={token}"),200).await;assert_eq!(second["data"].as_array().unwrap().len(),4);assert!(second["nextCursor"].is_null());
    let all=first["data"].as_array().unwrap().iter().chain(second["data"].as_array().unwrap()).collect::<Vec<_>>();
    assert_eq!(all.iter().map(|row|row["id"].as_str().unwrap().to_owned()).collect::<Vec<_>>(),expected_ids);
    for (resource,row) in all.iter().zip(&expected) {assert_eq!(resource["attributes"]["name"],row.get::<_,String>(1));assert_eq!(resource["membership"],json!({"isLeader":row.get::<_,bool>(2),"isOfficer":row.get::<_,bool>(3)}));assert_eq!(resource["relationships"],json!([]));assert_eq!(resource["attributes"].as_object().unwrap().len(),2);}
    for value in [&first,&second] {let text=value.to_string();for private in ["private_key","contact_email","MembershipOther","MembershipUser","permissions","account_lifecycle_id"] {assert!(!text.contains(private));}}
    let alias_page=get(&client,&origin,&format!("/api/v1/nostr/user/{alias}/memberships"),200).await;assert_eq!(alias_page["nostr"]["requestedId"],alias.as_str());assert_eq!(alias_page["nostr"]["userId"],owner.as_str());
    for path in [format!("/api/v1/nostr/user/{publisher}/memberships"),format!("/api/v1/nostr/user/{closed}/memberships"),"/api/v1/nostr/user/44444444-4444-8444-8444-444444444444/memberships".into()] {get(&client,&origin,&path,404).await;}
    for query in ["limit=0","limit=33","limit=1&limit=2","includes[]=member","offset=1","after=bad","after="] {get(&client,&origin,&format!("{base}?{query}"),400).await;}
    get(&client,&origin,&format!("/api/v1/nostr/user/{other}/memberships?after={token}"),409).await;
    let wrong_alias_cursor=cursor(owner.as_str(),other.as_str(),"Not a group");get(&client,&origin,&format!("{base}?after={wrong_alias_cursor}"),409).await;
    let last=first["data"].as_array().unwrap().last().unwrap();let anchor=last["id"].as_str().unwrap();let anchor_name=last["attributes"]["name"].as_str().unwrap();
    let db=state.db.client().await.unwrap();
    db.execute("UPDATE platform.catalog_groups SET name='Renamed anchor' WHERE id=(SELECT native_id FROM platform.public_resource_ids WHERE kind='group' AND public_id=$1::text::uuid)",&[&anchor]).await.unwrap();get(&client,&origin,&format!("{base}?after={token}"),409).await;
    db.execute("UPDATE platform.catalog_groups SET name=$2 WHERE id=(SELECT native_id FROM platform.public_resource_ids WHERE kind='group' AND public_id=$1::text::uuid)",&[&anchor,&anchor_name]).await.unwrap();
    db.execute("DELETE FROM platform.catalog_group_members WHERE user_id='MembershipUser' AND group_id=(SELECT native_id FROM platform.public_resource_ids WHERE kind='group' AND public_id=$1::text::uuid)",&[&anchor]).await.unwrap();get(&client,&origin,&format!("{base}?after={token}"),409).await;
    // Every valid worst-case escaped/multibyte name can be an actual page boundary.
    for native in ["MGroup0005","MGroup0006"] {let row=db.query_one("SELECT a.public_id::text,g.name FROM platform.catalog_groups g JOIN platform.public_resource_ids a ON a.kind='group' AND a.native_id=g.id AND a.preferred WHERE g.id=$1",&[&native]).await.unwrap();let token=cursor(owner.as_str(),row.get(0),row.get(1));assert!(token.len()<=2048);get(&client,&origin,&format!("{base}?limit=1&after={token}"),200).await;}
    assert_eq!(db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get::<_,i64>(0),aliases);
    // Missing alias fixture disables only the automatic alias trigger; read must
    // fail closed even if the unnamed row would precede the continuation anchor.
    db.batch_execute("ALTER TABLE platform.catalog_groups DISABLE TRIGGER public_group_alias; INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES('NoAliasGrp','MembershipUser','Owner','Hidden identity'); ALTER TABLE platform.catalog_groups ENABLE TRIGGER public_group_alias; INSERT INTO platform.catalog_group_members(group_id,user_id,name) VALUES('NoAliasGrp','MembershipUser','Owner');").await.unwrap();get(&client,&origin,&base,503).await;
    let row=db.query_one("SELECT a.public_id::text,g.name FROM platform.catalog_groups g JOIN platform.public_resource_ids a ON a.kind='group' AND a.native_id=g.id AND a.preferred JOIN platform.catalog_group_members gm ON gm.group_id=g.id WHERE gm.user_id='MembershipUser' AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id) ORDER BY g.name,g.id LIMIT 1",&[]).await.unwrap();let token=cursor(owner.as_str(),row.get(0),row.get(1));get(&client,&origin,&format!("{base}?after={token}"),503).await;
    db.execute("UPDATE platform.users SET disabled=TRUE WHERE id='MembershipUser'",&[]).await.unwrap();get(&client,&origin,&base,404).await;
}
