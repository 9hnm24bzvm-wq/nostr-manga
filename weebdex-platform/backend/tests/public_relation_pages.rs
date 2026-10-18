//! Actual loopback HTTP and disposable PostgreSQL; synthetic4096-edge ceiling.
use serde_json::{Value,json};
use std::collections::BTreeSet;
use std::time::{Duration,Instant};
use weebdex_platform::{AppState,Config,domains,public_api::{PostgresPublicIdResolver,PublicIdResolver,PublicUuid,ResourceKind}};
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server { fn drop(&mut self) { self.0.abort(); } }
async fn get(client:&reqwest::Client,base:&str,path:&str,status:u16)->(Value,usize) {
    let start=Instant::now();let response=client.get(format!("{base}/api/v1{path}")).send().await.unwrap();
    let actual=response.status().as_u16();let bytes=response.bytes().await.unwrap();
    assert!(start.elapsed()<Duration::from_secs(60),"one request exceeded existing60s budget");
    assert!(bytes.len()<=1024*1024,"response exceeds actual1MiB signed transport: {}",bytes.len());
    let value:Value=serde_json::from_slice(&bytes).unwrap();assert_eq!(actual,status,"{path}: {value}");(value,bytes.len())
}
fn continuation(owner:&str,first:&Value,offset:u64)->String {format!("/manga/{owner}/relation?includes[]=manga&limit=32&offset={offset}&ownerVersion={}&snapshot={}",first["ownerVersion"],first["snapshot"].as_str().unwrap())}
#[tokio::test(flavor="multi_thread",worker_threads=2)]
#[ignore="Requires explicitly isolated PostgreSQL runner"]
async fn public_relation_pages_recover_all_4096_with_full_metadata_and_snapshot_guards() {
    let dsn=std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use isolated runner");
    let pg:tokio_postgres::Config=dsn.parse().unwrap();assert_eq!(pg.get_dbname(),Some("weebdex_platform_test"));
    assert!(pg.get_hosts().iter().all(|h|matches!(h,tokio_postgres::config::Host::Tcp(h) if h=="127.0.0.1")));
    let runtime=std::path::PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap());assert!(runtime.to_str().unwrap().contains("/weebdex-platform/test-runtime/"));
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let base=format!("http://{}",listener.local_addr().unwrap());
    let state=AppState::connect(Config{database_plaintext_local:true,database_connections:4,public_origin:base.clone(),uploads_dir:runtime.join("media"),ui_dir:runtime.join("no-ui"),..Config::default()},dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();state.db.verify_schema(domains::MIGRATIONS).await.unwrap();
    let db=state.db.client().await.unwrap();
    db.batch_execute(r#"
        INSERT INTO platform.users(id,username,email,password_hash,verified) VALUES('RelPageUser','relation-pages','pages@fixture.invalid','not-a-password',true);
        INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state,version) VALUES
        ('RelPageRoot','RelPageUser','Fixture','Root','en','ongoing','safe','published',7),('RelLegacy','RelPageUser','Fixture','Legacy','en','ongoing','safe','published',3);
    "#).await.unwrap();
    // Alias creation takes genuine advisory locks. Bounded transactions keep
    // fixture generation inside the unchanged normal PostgreSQL lock budget.
    for start in (1..=4096i32).step_by(128) {
        let end=(start+127).min(4096);
        db.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state,version,metadata) SELECT 'RelTarget'||lpad(n::text,4,'0'),'RelPageUser','Fixture','Target '||n,'en','ongoing','safe','published',2,jsonb_build_object('description',jsonb_build_object('en','Complete description '||n),'alt_titles',jsonb_build_object('ja','Title '||n)) FROM generate_series($1::integer,$2::integer)n",&[&start,&end]).await.unwrap();
        db.execute("INSERT INTO platform.catalog_relations(manga_id,related_id,relation_type,created_by,created_at) SELECT 'RelPageRoot','RelTarget'||lpad(n::text,4,'0'),'sequel','RelPageUser','2026-10-17T00:00:00Z'::timestamptz+n*interval '1 microsecond' FROM generate_series($1::integer,$2::integer)n",&[&start,&end]).await.unwrap();
    }
    db.batch_execute("INSERT INTO platform.catalog_relations(manga_id,related_id,relation_type,created_by) VALUES('RelLegacy','RelTarget0001','prequel','RelPageUser')").await.unwrap();
    // Pre-register the one-over identity before the read-only alias baseline.
    db.batch_execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES('RelOverCap','RelPageUser','Fixture','Over cap','en','ongoing','safe','published');INSERT INTO platform.catalog_relations(manga_id,related_id,relation_type,created_by) VALUES('RelPageRoot','RelOverCap','sequel','RelPageUser');DELETE FROM platform.catalog_relations WHERE manga_id='RelPageRoot' AND related_id='RelOverCap'").await.unwrap();
    let resolver=PostgresPublicIdResolver::new(state.db.clone());let root=resolver.preferred(ResourceKind::Manga,"RelPageRoot").await.unwrap().unwrap();
    let legacy=resolver.preferred(ResourceKind::Manga,"RelLegacy").await.unwrap().unwrap();
    let alias=PublicUuid::parse("AAAAAAAA-AAAA-8AAA-8AAA-AAAAAAAAAAAA").unwrap();resolver.register_alias(ResourceKind::Manga,"RelPageRoot",&alias).await.unwrap();
    let expected=db.query("SELECT a.public_id::text,t.public_id::text FROM platform.catalog_relations r JOIN platform.public_resource_ids a ON a.kind='manga_relation' AND a.native_id=platform.public_manga_edge_key(r.manga_id,r.related_id) AND a.preferred JOIN platform.public_resource_ids t ON t.kind='manga' AND t.native_id=r.related_id AND t.preferred WHERE r.manga_id='RelPageRoot' ORDER BY r.created_at,r.related_id",&[]).await.unwrap().into_iter().map(|r|(r.get::<_,String>(0),r.get::<_,String>(1))).collect::<Vec<_>>();
    let alias_count:i64=db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);drop(db);
    let app=weebdex_platform::router::router(state.clone());let _server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap()}));
    let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(60)).build().unwrap();
    if std::env::var_os("PLATFORM_RELATION_OWNER_ALIAS_DIAGNOSTIC").is_some() {
        let start=Instant::now();let value:Value=state.db.client().await.unwrap().query_one("SELECT platform.catalog_manga_json('RelPageRoot')",&[]).await.unwrap().get(0);
        let native_projection_ms=start.elapsed().as_millis();let mut keys=BTreeSet::new();keys.insert(value["id"].as_str().unwrap().to_owned());
        for related in value["relationships"]["relations"].as_array().unwrap() {keys.insert(related["id"].as_str().unwrap().to_owned());}
        assert_eq!(keys.len(),4097);let start=Instant::now();
        let aliases=weebdex_platform::public_api::catalogue_dto::AliasIndex::load(&state,&resolver,&[(ResourceKind::Manga,value)]).await;
        eprintln!("owner_alias_diagnostic={}",json!({"actualNativeMangaIds":keys.len(),"nativeProjectionMs":native_projection_ms,"aliasLoadMs":start.elapsed().as_millis(),"aliasLoadSucceeded":aliases.is_ok(),"phaseOnlyNotFullQualification":true}));
        assert!(aliases.is_ok(),"Owner plus4096 relation target aliases must fit");
        return;
    }
    let (old,_)=get(&client,&base,&format!("/manga/{legacy}/relation?includes[]=manga"),200).await;
    let (paged,_)=get(&client,&base,&format!("/manga/{legacy}/relation?includes[]=manga&limit=1"),200).await;
    assert_eq!(old["data"],paged["data"],"legacy metadata unchanged");assert!(old.get("snapshot").is_none());assert!(paged["nextOffset"].is_null());
    for bad in ["limit=0","limit=33","offset=1","ownerVersion=0","limit=1&limit=2","limit=1&includes[]=author","snapshot=INVALID"] {get(&client,&base,&format!("/manga/{root}/relation?{bad}"),400).await;}
    let start=Instant::now();let requested=alias.as_str().to_uppercase();
    let (first,mut max_bytes)=get(&client,&base,&format!("/manga/{requested}/relation?includes[]=manga&limit=32"),200).await;
    assert_eq!(first["ownerMangaId"],root.as_str());assert_eq!(first["ownerVersion"],7);assert_eq!(first["total"],4096);
    assert_eq!(first["nostr"]["identity"],json!({"kind":"manga","requestedId":alias,"canonicalId":root,"source":"persisted-public-resource-ids"}));assert!(first.get("persistedTotal").is_none());
    let mut page=first.clone();let mut seen=BTreeSet::new();let mut targets=BTreeSet::new();let mut actual=Vec::new();let mut page_count=0;
    loop {
        page_count+=1;assert_eq!(page["total"],4096);assert_eq!(page["ownerVersion"],first["ownerVersion"]);assert_eq!(page["snapshot"],first["snapshot"]);
        assert!(page["data"].as_array().unwrap().len()<=32);
        for edge in page["data"].as_array().unwrap() {
            let id=edge["id"].as_str().unwrap();let target=edge["relationships"][0]["id"].as_str().unwrap();
            assert!(seen.insert(id.to_owned()));assert!(targets.insert(target.to_owned()));actual.push((id.to_owned(),target.to_owned()));
            assert_eq!(edge["attributes"]["version"],7);assert_eq!(edge["relationships"][0]["attributes"]["version"],2);
            assert!(edge["relationships"][0]["attributes"]["description"]["en"].as_str().unwrap().starts_with("Complete description "));
        }
        let Some(next)=page["nextOffset"].as_u64() else{break};assert_eq!(next,actual.len() as u64);
        let (next_page,bytes)=get(&client,&base,&continuation(&requested,&first,next),200).await;max_bytes=max_bytes.max(bytes);page=next_page;
    }
    assert_eq!(actual,expected);assert_eq!(page_count,128);assert_eq!(seen.len(),4096);
    let all_pages_ms=start.elapsed().as_millis();eprintln!("relation_pages_measurement={}",json!({"edges":4096,"pages":page_count,"elapsedMs":all_pages_ms,"maxResponseBytes":max_bytes,"fullTargetMetadata":true,"perRequestBudgetSeconds":60,"responseBudgetBytes":1024*1024}));
    let mut adapter_probe_passed = true;
    if let Ok(script) = std::env::var("PLATFORM_PUBLIC_RELATION_ADAPTER_SCRIPT") {
        use sha2::{Digest,Sha256};
        let script=std::path::PathBuf::from(script);
        assert!(script.ends_with("public-relation-ui-worktree/weebdex-platform/tests/ci/probe_public_relation_pages.mjs"));
        assert_eq!(hex::encode(Sha256::digest(std::fs::read(&script).unwrap())),"a3c857eb5be4d1140b8e8ec8d71b7c2af874f39910edfeb681d2e1678b09dc66");
        let output=tokio::time::timeout(Duration::from_secs(70),tokio::process::Command::new("node")
            .arg("--experimental-vm-modules").arg(script).env("PUBLIC_API_ORIGIN",&base).env("MANGA_ID",root.as_str())
            .env("EXPECTED_RELATIONS","4096").env("RELATION_PROBE_RECEIPT",runtime.join("actual-public-relation-adapter.json"))
            .kill_on_drop(true).output()).await.expect("Actual adapter exceeded its own60s budget and shutdown margin").unwrap();
        adapter_probe_passed=output.status.success();
        eprintln!("actual_relation_adapter_passed={adapter_probe_passed}\n{}\n{}",String::from_utf8_lossy(&output.stdout),String::from_utf8_lossy(&output.stderr));
    }
    // The relation editor refreshes its owner after mutation. Capacity is not a
    // usable journey if the owner+4096 related UUIDs cannot be presented too.
    let (owner_at_cap,_)=get(&client,&base,&format!("/manga/{root}"),200).await;
    assert_eq!(owner_at_cap["data"]["id"],root.as_str());
    assert_eq!(owner_at_cap["data"]["relationships"].as_array().unwrap().iter().filter(|r|r["type"]=="manga").count(),4096);
    assert!(adapter_probe_passed,"Actual complete public relation adapter failed; inspect retained fixture probe receipt");
    state.db.client().await.unwrap().batch_execute("INSERT INTO platform.catalog_relations(manga_id,related_id,relation_type,created_by) VALUES('RelPageRoot','RelOverCap','sequel','RelPageUser')").await.unwrap();
    get(&client,&base,&format!("/manga/{root}"),503).await;
    get(&client,&base,&format!("/manga/{root}/relation?limit=32"),503).await;
    state.db.client().await.unwrap().batch_execute("DELETE FROM platform.catalog_relations WHERE manga_id='RelPageRoot' AND related_id='RelOverCap'").await.unwrap();
    let (end,_)=get(&client,&base,&continuation(root.as_str(),&first,4096),200).await;assert_eq!(end["data"],json!([]));assert!(end["nextOffset"].is_null());
    let db=state.db.client().await.unwrap();
    db.batch_execute("UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='RelTarget4096'").await.unwrap();
    let (before,_)=get(&client,&base,&format!("/manga/{root}/relation?includes[]=manga&limit=32"),200).await;assert_eq!(before["total"],4095);
    // Equal visible count but a different eligible edge set must not mix pages.
    db.batch_execute("UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='RelTarget0001';UPDATE platform.catalog_manga SET protocol_source_visible=true WHERE id='RelTarget4096'").await.unwrap();
    get(&client,&base,&continuation(root.as_str(),&before,32),409).await;
    db.batch_execute("UPDATE platform.catalog_manga SET protocol_source_visible=true WHERE id='RelTarget0001'").await.unwrap();
    let (before,_)=get(&client,&base,&format!("/manga/{root}/relation?limit=32"),200).await;
    db.batch_execute("UPDATE platform.catalog_manga SET version=version+1 WHERE id='RelTarget0001'").await.unwrap();get(&client,&base,&continuation(root.as_str(),&before,32),409).await;
    let (before,_)=get(&client,&base,&format!("/manga/{root}/relation?limit=32"),200).await;
    db.batch_execute("UPDATE platform.catalog_manga SET version=version+1 WHERE id='RelPageRoot'").await.unwrap();get(&client,&base,&continuation(root.as_str(),&before,32),409).await;
    // Complete large metadata fits a shorter page, with no skipped edge.
    db.batch_execute("UPDATE platform.catalog_manga SET metadata=jsonb_build_object('description',jsonb_build_object('en',repeat('X',32768))),version=version+1 WHERE id>='RelTarget0001' AND id<='RelTarget0064'").await.unwrap();
    let (large,_)=get(&client,&base,&format!("/manga/{root}/relation?includes[]=manga&limit=32"),200).await;let prefix=large["data"].as_array().unwrap().len();assert!(prefix>0&&prefix<32);assert_eq!(large["nextOffset"],prefix);
    assert_eq!(large["data"][0]["relationships"][0]["attributes"]["description"]["en"].as_str().unwrap().len(),32768);
    let (next,_)=get(&client,&base,&continuation(root.as_str(),&large,prefix as u64),200).await;assert_eq!(next["data"][0]["relationships"][0]["id"],expected[prefix].1);
    db.batch_execute("UPDATE platform.catalog_manga SET metadata=jsonb_build_object('description',jsonb_build_object('en',repeat('X',1048576))),version=version+1 WHERE id='RelTarget0001'").await.unwrap();
    get(&client,&base,&format!("/manga/{root}/relation?includes[]=manga&limit=1"),503).await;
    let after:i64=db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);assert_eq!(after,alias_count,"read pages never create aliases");
    db.batch_execute("UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='RelPageRoot'").await.unwrap();get(&client,&base,&format!("/manga/{root}/relation?limit=32"),404).await;
}
