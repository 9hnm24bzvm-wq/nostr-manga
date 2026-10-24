//! Real signed HTTP title drafts, native review and original source publication.
//! All accounts, keys, PNG/JPEG bytes, paths and databases are owned test data.
use base64::{Engine,engine::general_purpose::STANDARD};
use image::{DynamicImage,ImageFormat,Rgb,RgbImage};
use manga_node::{auth,db::{DbConfig,Repository}};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use std::{io::Cursor,time::Duration};
use weebdex_platform::{AppState,Config,domains,protocol::publication::OperatorRuntimeConfig,
    public_api::{PostgresPublicIdResolver,PublicIdResolver,PublicUuid,ResourceKind}};
// Publication commands and protocol aliases use the native ten-byte ID shape.
// The wider catalogue table alone does not establish publication compatibility.
const FIXTURE_MANGA_ID:&str="UploadMng1";
const FIXTURE_GROUP_ID:&str="UploadGrp1";
fn assert_catalog_fixture_ids() {
    for (kind,id) in [("manga",FIXTURE_MANGA_ID),("group",FIXTURE_GROUP_ID)] {
        assert!(domains::uploads::storage::valid_id(id),"invalid native {kind} fixture ID");
    }
}
#[test]
fn catalog_fixture_identifiers_match_publication_schema() {
    let schema=include_str!("../migrations/0070_protocol_bridge.sql");
    assert!(schema.contains("catalog_id TEXT NOT NULL CHECK(length(catalog_id)=10)"));
    assert_catalog_fixture_ids();
    for invalid in ["UploadManga","ShortID09","Upload_ID1","Upload-ID1","ééééé"] {
        assert!(!domains::uploads::storage::valid_id(invalid),"invalid fixture regression accepted");
    }
}
fn secret(key:u8)->String { format!("{key:064x}") }
fn public(key:u8)->String { auth::sign(&secret(key),1,vec![],String::new()).unwrap().pubkey }
fn id(key:u8)->[u8;32] { hex::decode(public(key)).unwrap().try_into().unwrap() }
fn proof(key:u8, service:u8, url:&str, method:&str, body:&[u8])->String {
    let event=auth::sign(&secret(key),27235,vec![vec!["u".into(),url.into()],vec!["method".into(),method.into()],
        vec!["service".into(),public(service)],vec!["payload".into(),auth::hash(body)],
        vec!["nonce".into(),hex::encode(rand::random::<[u8;16]>())]],String::new()).unwrap();
    format!("Nostr {}",STANDARD.encode(serde_json::to_vec(&event).unwrap()))
}
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server { fn drop(&mut self) { self.0.abort(); } }
struct Fixture {
    state:AppState, repository:Repository, http:reqwest::Client, origin:String,
    _server:Server, _storage:tempfile::TempDir,
    native: Option<weebdex_platform::protocol::publication::NativeListener>,
}
impl Fixture {
    async fn raw(&self,method:&str,path:&str,body:Vec<u8>,content_type:&str,authorization:Option<String>,expected:u16)->Value {
        let request_started=std::time::Instant::now();
        let mut request=self.http.request(method.parse().unwrap(),format!("{}{path}",self.origin))
            .header("content-type",content_type).header("origin",&self.origin);
        let private_relation=method=="GET" && path.split('?').next().unwrap().starts_with("/api/v1/nostr/manga/") && path.split('?').next().unwrap().ends_with("/relations");
        if private_relation {
            request=request.timeout(Duration::from_secs(60));
        }
        if let Some(value)=authorization { request=request.header("authorization",value); }
        let response=request.body(body).send().await.unwrap_or_else(|error| {
            eprintln!("RELATION_HTTP_PHASE method={method} request_error elapsed_ms={} path={path}",request_started.elapsed().as_millis());
            panic!("Owned HTTP request failed: {error}");
        });
        let status=response.status().as_u16();
        let private_upload=path.split('?').next().unwrap().starts_with("/api/v1/nostr/uploads/")
            && ["/preview","/replace"].iter().any(|suffix|path.split('?').next().unwrap().ends_with(suffix));
        let private_cover=private_relation || path.starts_with("/api/v1/nostr/") && (path.ends_with("/covers")||path.ends_with("/original")) || path.starts_with("/api/v1/cover/") && ["POST","PUT","DELETE"].contains(&method);
        assert_eq!(response.headers()["cache-control"],if private_upload || private_cover {"private, no-store"}else{"no-store"},"{method} {path}");
        if private_cover {
            let vary=response.headers().get_all("vary").iter().map(|v|v.to_str().unwrap().to_ascii_lowercase()).collect::<Vec<_>>().join(",");
            assert!(vary.split(',').any(|v|v.trim()=="authorization"),"missing Authorization variation: {vary}");
            assert!(vary.split(',').any(|v|v.trim()=="origin"),"missing Origin variation: {vary}");
        }
        let text=response.text().await.unwrap_or_else(|error| {
            eprintln!("RELATION_HTTP_PHASE method={method} body_error elapsed_ms={} path={path}",request_started.elapsed().as_millis());
            panic!("Owned HTTP response failed: {error}");
        });
        let value:Value=serde_json::from_str(&text).unwrap_or_else(|_|panic!("{method} {path}: non-JSON {text}"));
        if path.contains("/relation") {
            eprintln!("RELATION_HTTP_PHASE method={method} status={status} elapsed_ms={} response_bytes={} path={path}",request_started.elapsed().as_millis(),text.len());
            if private_relation {
                assert!(request_started.elapsed()<Duration::from_secs(60),"Private relation read exceeded the actual signed transport60000ms budget");
                assert!(text.len()<=1024*1024,"Private relation read exceeded the actual signed transport1MiB JSON budget");
            }
        }
        assert_eq!(status,expected,"{method} {path}: {value}");
        if status>=400 { assert_eq!(value["result"],"error","{value}"); assert_eq!(value["errors"][0]["status"],status,"{value}"); }
        value
    }
    async fn call(&self,key:Option<u8>,method:&str,path:&str,payload:Option<Value>,expected:u16)->Value {
        let mut payload=payload;
        if path.starts_with("/api/v1/cover/")&&["PUT","DELETE"].contains(&method){
            if let (Some(key),Some(input))=(key,payload.as_mut()){
                let user=PostgresPublicIdResolver::new(self.state.db.clone()).preferred(ResourceKind::User,&format!("PublicUploadUser{key}")).await.unwrap().unwrap();
                input["expectedAccountId"]=json!(user);
            }
        }
        let body=payload.map_or_else(Vec::new,|v|serde_json::to_vec(&v).unwrap());
        let authorization=key.map(|key|proof(key,11,&format!("{}{path}",self.origin),method,&body));
        self.raw(method,path,body,"application/json",authorization,expected).await
    }
    async fn relation_page(&self,owner:&str,account:&PublicUuid,version:i64,offset:u32,snapshot:Option<&str>,expected:u16)->Value {
        let mut path=format!("/api/v1/nostr/manga/{owner}/relations?limit=32&offset={offset}&ownerVersion={version}&expectedAccountId={account}");
        if let Some(snapshot)=snapshot{path.push_str("&snapshot=");path.push_str(snapshot);}
        self.call(Some(1),"GET",&path,None,expected).await
    }
    async fn native(&self,kind:ResourceKind,public:&str)->String {
        PostgresPublicIdResolver::new(self.state.db.clone()).resolve(kind,&PublicUuid::parse(public).unwrap())
            .await.unwrap().unwrap().native_id
    }
}
async fn fixture()->Fixture {
    assert_catalog_fixture_ids();
    // Native conversion logs only safe SQLSTATE, never SQL or bound parameters.
    let _=tracing_subscriber::fmt().with_max_level(tracing::Level::WARN).with_test_writer().try_init();
    let dsn=std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use scripts/run-auth-tests.sh");
    let pg:tokio_postgres::Config=dsn.parse().unwrap(); assert_eq!(pg.get_dbname(),Some("weebdex_platform_test"));
    assert!(!pg.get_hosts().is_empty()); assert!(pg.get_hosts().iter().all(|h|matches!(h,tokio_postgres::config::Host::Tcp(h)if h=="127.0.0.1")));
    let runtime=std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap(); assert!(runtime.contains("/weebdex-platform/test-runtime/"));
    let storage=tempfile::Builder::new().prefix("public-upload-").tempdir_in(runtime).unwrap();
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin=format!("http://{}",listener.local_addr().unwrap());
    let signer=storage.path().join("SYNTHETIC_EPHEMERAL_operator.key");
    let service=storage.path().join("SYNTHETIC_EPHEMERAL_service.key");
    for (file,key) in [(&signer,1),(&service,11)] { std::fs::write(file,secret(key)).unwrap();
        use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(file,std::fs::Permissions::from_mode(0o600)).unwrap(); }
    let native_reservation=std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let native_bind=native_reservation.local_addr().unwrap();drop(native_reservation);
    let binary=std::env::var_os("STRFRY_BIN").map(std::path::PathBuf::from).expect("Set STRFRY_BIN to the verified pinned strfry binary");
    assert!(binary.is_absolute() && binary.is_file());
    let publication=OperatorRuntimeConfig {
        operator_key_file:signer,expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),
        storage:json!({"path":storage.path().join("protocol-originals"),"volume":"PublicUploadFixture","physical_bytes":64*1024*1024,
            "staging_bytes":64*1024*1024,"max_blob_bytes":8*1024*1024,"upload_slots":2}),group_keys_dir:None,backend:None,
        native_node:Some(json!({"bind":native_bind.to_string(),"base_url":format!("http://{native_bind}"),"service_key_file":service,"admin_pubkey":public(1),
            "database_plaintext_local":true,"read_policy":"public","sync_worker_enabled":false,"auto_approve_publication_deletions":false,
            "backend":{"binary":binary,"data_dir":storage.path().join("real-projection"),"map_bytes":67108864,"timeout_seconds":5}})) };
    let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:6,
        uploads_dir:storage.path().join("native-images"),protocol_publication:Some(publication.clone()),..Config::default()},dsn.clone()).await.unwrap();
    let mut repository=Repository::connect(DbConfig{dsn,connections:1,acquisition_timeout:Duration::from_secs(2),cache_weight:1048576,plaintext_local:true}).await.unwrap();
    repository.pool=state.db.pool().clone();repository.migrate().await.unwrap();repository.bind_instance(id(11),id(1)).await.unwrap();
    for key in 1..=8 {repository.set_account_admission(id(key),true).await.unwrap();}
    repository.set_publisher_admission(id(1),true).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();state.protocol.verify_schema().await.unwrap();
    state.protocol.install_visibility_function().await.unwrap();state.protocol.configure_publication(publication).await.unwrap();
    let native=Some(state.protocol.publication_runtime().await.unwrap().start_native_listener().await.unwrap().unwrap());
    let db=state.db.client().await.unwrap();
    for key in [1,2,4,5,6,7,8] {
        let permissions=if key==4 {json!([])}else{json!(["chapter.create","self.chapter.update","self.chapter.remove"])};
        let roles=if key==7 {json!(["restricted"])}else{json!(["user"])};
        let user=format!("PublicUploadUser{key}");
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,disabled,roles,permissions) VALUES($1,$1,$2,'synthetic-no-password',$3,$4,$5,$6)",
            &[&user,&format!("upload-{key}@fixture.invalid"),&(key!=5),&false,&roles,&permissions]).await.unwrap();
        db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await.unwrap();
        db.execute("UPDATE platform.users SET disabled=$2 WHERE id=$1",&[&user,&(key==6)]).await.unwrap();
    }
    db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES('public-upload-native-cookie','PublicUploadUser1',$1,clock_timestamp()+interval '1 hour')",&[&Sha256::digest(b"synthetic-public-upload-cookie").to_vec()]).await.unwrap();
    db.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES($1,'PublicUploadUser1','Synthetic uploader','Public upload fixture','en','ongoing','safe','published')",&[&FIXTURE_MANGA_ID]).await.unwrap();
    let native_manga=FIXTURE_MANGA_ID.to_owned();
    PostgresPublicIdResolver::new(state.db.clone()).preferred(ResourceKind::Manga,&native_manga).await.unwrap().unwrap();
    drop(db);
    let app=weebdex_platform::router::router(state.clone());
    let server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap()}));
    Fixture{state,repository,http:reqwest::Client::new(),origin,_server:server,_storage:storage,native}
}
fn png()->Vec<u8> {let mut bytes=Cursor::new(Vec::new());DynamicImage::ImageRgb8(RgbImage::from_pixel(13,17,Rgb([71,28,149]))).write_to(&mut bytes,ImageFormat::Png).unwrap();bytes.into_inner()}
#[allow(dead_code)]
async fn cover(f:&Fixture,manga:&str) {
    // Real native cover route is an independent existing interface; the public
    // title adapter never invents cover bytes or claims unsupported upload parity.
    let boundary="OwnedTitleCoverBoundary";
    let mut body=format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"synthetic-title.png\"\r\nContent-Type: image/png\r\n\r\n").into_bytes();
    body.extend(png());body.extend(format!("\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"language\"\r\n\r\nja\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"main\"\r\n\r\ntrue\r\n--{boundary}--\r\n").as_bytes());
    let response=f.http.post(format!("{}/api/cover/{manga}",f.origin)).header("origin",&f.origin)
        .header("cookie","weebdex_session=synthetic-public-upload-cookie").header("content-type",format!("multipart/form-data; boundary={boundary}")).body(body).send().await.unwrap();
    let status=response.status().as_u16();let text=response.text().await.unwrap();assert_eq!(status,200,"native owned cover: {text}");
}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires owned PostgreSQL guard and pinned original strfry"]
async fn signed_relations_preserve_directional_identity_owner_versions_and_original_source(){
    use weebdex_platform::protocol::bridge;
    let mut f=fixture().await;let db=f.state.db.client().await.unwrap();
    db.batch_execute(r#"UPDATE platform.users SET permissions='["manga.create","manga.update","manga.title.edit","manga.cover.edit","manga.relation.edit"]' WHERE id IN('PublicUploadUser1','PublicUploadUser2'); UPDATE platform.users SET roles='["moderator"]',permissions=permissions||'["manga.state.update"]'::jsonb WHERE id='PublicUploadUser1'; INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES('RelationTarget02','PublicUploadUser2','Fixture','Related target2','ja','ongoing','safe','published'),('RelationTarget03','PublicUploadUser2','Fixture','Related target3','ja','ongoing','safe','published');"#).await.unwrap();
    let resolver=PostgresPublicIdResolver::new(f.state.db.clone());
    let account=resolver.preferred(ResourceKind::User,"PublicUploadUser1").await.unwrap().unwrap();
    let other=resolver.preferred(ResourceKind::User,"PublicUploadUser2").await.unwrap().unwrap();
    let target=resolver.preferred(ResourceKind::Manga,FIXTURE_MANGA_ID).await.unwrap().unwrap();
    let target2=resolver.preferred(ResourceKind::Manga,"RelationTarget02").await.unwrap().unwrap();
    let target3=resolver.preferred(ResourceKind::Manga,"RelationTarget03").await.unwrap().unwrap();
    let input=json!({"title":{"ja":"Original relation owner"},"originalLanguage":"ja","status":"ongoing","contentRating":"safe","expectedAccountId":account});
    let created=f.call(Some(1),"POST","/api/v1/manga",Some(input.clone()),200).await;
    let owner=created["data"]["id"].as_str().unwrap().to_owned();let native=f.native(ResourceKind::Manga,&owner).await;
    cover(&f,&native).await;let version:i32=db.query_one("SELECT version FROM platform.catalog_manga WHERE id=$1",&[&native]).await.unwrap().get(0);
    let submitted=f.call(Some(1),"POST",&format!("/api/v1/manga/draft/{owner}/commit"),Some(json!({"version":version,"expectedAccountId":account})),201).await;
    let v=submitted["data"]["attributes"]["version"].as_i64().unwrap();
    let published=f.call(Some(1),"POST",&format!("/api/v1/manga/draft/{owner}/publish"),Some(json!({"version":v,"expectedAccountId":account})),200).await;
    let eid:[u8;32]=hex::decode(published["data"]["attributes"]["nostr"]["eventId"].as_str().unwrap()).unwrap().try_into().unwrap();
    tokio::time::timeout(Duration::from_secs(20),async{loop{if f.state.protocol.entry(eid).await.unwrap().is_some(){break;}tokio::time::sleep(Duration::from_millis(20)).await;}}).await.unwrap();
    for _ in 0..8{let r=bridge::import_page(&f.state).await.unwrap();assert_eq!(r.rejected,0);if r.completed_sweep&&r.deferred==0&&r.scanned>0{break;}}
    let source=f.state.protocol.entry(eid).await.unwrap().unwrap();source.event.verify().unwrap();
    let path=format!("/api/v1/manga/{owner}/relation");let mut version=published["data"]["attributes"]["version"].as_i64().unwrap();
    let make=|target:&PublicUuid,kind:&str,version:i64|json!({"targetManga":target,"relation":kind,"version":version,"expectedAccountId":account});
    f.call(None,"POST",&path,Some(make(&target,"sequel",version)),401).await;
    f.call(Some(1),"POST",&path,Some(json!({"targetManga":target,"relation":"sequel","version":version})),400).await;
    f.call(Some(1),"POST",&path,Some(make(&PublicUuid::parse(&owner).unwrap(),"sequel",version)),400).await;
    let exact=serde_json::to_string(&make(&target,"sequel",version)).unwrap();
    for extra in [format!(",\"version\":{version}"),format!(",\"targetManga\":\"{target2}\""),format!(",\"expectedAccountId\":\"{other}\"")] {
        let mut duplicate=exact.clone();duplicate.pop();duplicate.push_str(&extra);duplicate.push('}');let bytes=duplicate.into_bytes();let token=proof(1,11,&format!("{}{path}",f.origin),"POST",&bytes);f.raw("POST",&path,bytes,"application/json",Some(token),400).await;
    }
    let edge=f.call(Some(1),"POST",&path,Some(make(&target,"sequel",version)),200).await;version+=1;
    let edge_id=edge["data"]["id"].as_str().unwrap().to_owned();assert_eq!(edge["data"]["type"],"manga_relation");assert_eq!(edge["data"]["attributes"]["version"],version);assert_eq!(edge["data"]["attributes"]["ownerMangaId"],owner);assert_eq!(edge["data"]["relationships"][0]["id"],target.as_str());
    let read:Value=f.http.get(format!("{}{path}?includes[]=manga",f.origin)).send().await.unwrap().json().await.unwrap();assert_eq!(read["total"],1);assert_eq!(read["data"][0]["id"],edge_id);
    f.call(Some(1),"POST",&path,Some(make(&target,"sequel",version)),409).await;
    f.call(Some(1),"PUT",&path,Some(make(&target,"prequel",version-1)),409).await;
    let edited=f.call(Some(1),"PUT",&path,Some(make(&target,"prequel",version)),200).await;version+=1;assert_eq!(edited["data"]["id"],edge_id);assert_eq!(edited["data"]["attributes"]["version"],version);
    let before:i64=db.query_one("SELECT count(*) FROM platform.catalog_audit WHERE entity_id=$1",&[&native]).await.unwrap().get(0);
    let same=f.call(Some(1),"PUT",&path,Some(make(&target,"prequel",version)),200).await;assert_eq!(same["data"]["attributes"]["version"],version);
    let after:i64=db.query_one("SELECT count(*) FROM platform.catalog_audit WHERE entity_id=$1",&[&native]).await.unwrap().get(0);assert_eq!(before,after);
    // The shared single-edge response remains byte-for-value equivalent to
    // the historical native collection item, with its actual owner token.
    let legacy:Value=db.query_one("SELECT platform.catalog_relations_json($1)",&[&native]).await.unwrap().get(0);
    let mut expected_native=legacy.as_array().unwrap().iter().find(|edge|edge["id"]==FIXTURE_MANGA_ID).unwrap().clone();
    expected_native["manga_version"]=json!(version);expected_native["relation_scope"]=json!("instance-local");
    let native_response=f.http.put(format!("{}/api/manga/{native}/relations",f.origin)).header("origin",&f.origin).header("cookie","weebdex_session=synthetic-public-upload-cookie").json(&json!({"related_id":FIXTURE_MANGA_ID,"type":"prequel","version":version})).send().await.unwrap();
    assert_eq!(native_response.status().as_u16(),200);assert_eq!(native_response.json::<Value>().await.unwrap(),expected_native);
    f.call(Some(1),"DELETE",&format!("{path}/{target}"),Some(json!({"version":version,"expectedAccountId":account})),404).await;
    f.call(Some(1),"DELETE",&format!("/api/v1/manga/{target}/relation/{edge_id}"),Some(json!({"version":1,"expectedAccountId":account})),404).await;
    // Exact eligible-account rebind cannot retarget already signed bytes.
    let body=serde_json::to_vec(&make(&target2,"side_story",version)).unwrap();let token=proof(1,11,&format!("{}{path}",f.origin),"POST",&body);
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser2' WHERE publisher=$1",&[&public(1)]).await.unwrap();f.raw("POST",&path,body,"application/json",Some(token),409).await;db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser1' WHERE publisher=$1",&[&public(1)]).await.unwrap();
    let a=serde_json::to_vec(&make(&target2,"side_story",version)).unwrap();let mut second_input=make(&target2,"prequel",version);second_input["expectedAccountId"]=json!(other);let b=serde_json::to_vec(&second_input).unwrap();let url=format!("{}{path}",f.origin);
    let first=f.http.post(&url).header("content-type","application/json").header("authorization",proof(1,11,&url,"POST",&a)).body(a).send();
    let second=f.http.post(&url).header("content-type","application/json").header("authorization",proof(2,11,&url,"POST",&b)).body(b).send();
    let(first,second)=tokio::join!(first,second);let mut statuses=[first.unwrap().status().as_u16(),second.unwrap().status().as_u16()];statuses.sort();assert_eq!(statuses,[200,409]);version+=1;
    let body=serde_json::to_vec(&make(&target3,"main_story",version)).unwrap();let url=format!("{}{path}",f.origin);let token=proof(1,11,&url,"POST",&body);
    db.execute("UPDATE platform.protocol_publication_commands SET state='prepared' WHERE resource_kind='manga' AND catalog_id=$1",&[&native]).await.unwrap();f.raw("POST",&path,body.clone(),"application/json",Some(token),409).await;db.execute("UPDATE platform.protocol_publication_commands SET state='materialized' WHERE resource_kind='manga' AND catalog_id=$1",&[&native]).await.unwrap();
    // Native audit failure must roll back edge, alias and owner version together.
    db.batch_execute("CREATE FUNCTION platform.reject_relation_fixture_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.operation='relation_create' THEN RAISE EXCEPTION 'synthetic relation audit failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_relation_fixture_audit BEFORE INSERT ON platform.catalog_audit FOR EACH ROW EXECUTE FUNCTION platform.reject_relation_fixture_audit()").await.unwrap();
    f.call(Some(1),"POST",&path,Some(make(&target3,"main_story",version)),503).await;
    db.batch_execute("DROP TRIGGER reject_relation_fixture_audit ON platform.catalog_audit; DROP FUNCTION platform.reject_relation_fixture_audit()").await.unwrap();
    let actual:i32=db.query_one("SELECT version FROM platform.catalog_manga WHERE id=$1",&[&native]).await.unwrap().get(0);assert_eq!(actual as i64,version);
    let removed=f.call(Some(1),"DELETE",&format!("{path}/{edge_id}"),Some(json!({"version":version,"expectedAccountId":account})),200).await;version+=1;assert_eq!(removed["version"],version);assert_eq!(removed["relationId"],edge_id);
    let readded=f.call(Some(1),"POST",&path,Some(make(&target,"sequel",version)),200).await;version+=1;assert_eq!(readded["data"]["id"],edge_id);
    let draft=f.call(Some(1),"POST","/api/v1/manga",Some(input),200).await;let draft=draft["data"]["id"].as_str().unwrap();
    f.call(Some(1),"POST",&format!("/api/v1/manga/{draft}/relation"),Some(make(&target,"sequel",1)),200).await;
    let private=f.relation_page(draft,&account,2,0,None,200).await;assert_eq!(private["total"],1);assert_eq!(private["version"],2);
    let foreign=format!("/api/v1/nostr/manga/{draft}/relations?limit=32&offset=0&ownerVersion=2&expectedAccountId={other}");
    f.call(Some(2),"GET",&foreign,None,403).await;
    // Separate fixture construction from actual request time. This is diagnostic
    // timing over owned data; it does not assert a production load benchmark.
    let boundary_started=std::time::Instant::now();
    eprintln!("RELATION_BOUNDARY_PHASE setup_started elapsed_ms=0");
    // Exercise the actual collection boundary under the same owner-row lock.
    // Hidden persisted targets count too: visibility cannot create extra slots.
    let draft_native=f.native(ResourceKind::Manga,draft).await;
    // Alias triggers take transaction-scoped advisory locks. Bounded autocommit
    // batches preserve the default PostgreSQL lock budget and real aliases.
    for first in (1_i32..=4095).step_by(128) {
        let last=(first+127).min(4095);
        db.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) SELECT 'Rcap'||lpad(n::text,6,'0'),'PublicUploadUser2','Fixture','Bounded relation fixture','ja','ongoing','safe','published' FROM generate_series($1::integer,$2::integer) n",&[&first,&last]).await.unwrap();
        db.execute("INSERT INTO platform.catalog_relations(manga_id,related_id,relation_type,created_by) SELECT $1,'Rcap'||lpad(n::text,6,'0'),'sequel','PublicUploadUser1' FROM generate_series($2::integer,$3::integer) n",&[&draft_native,&first,&last]).await.unwrap();
    }
    eprintln!("RELATION_BOUNDARY_PHASE setup_finished elapsed_ms={}",boundary_started.elapsed().as_millis());
    let audit_before:i64=db.query_one("SELECT count(*) FROM platform.catalog_audit WHERE entity_id=$1",&[&draft_native]).await.unwrap().get(0);
    let draft_path=format!("/api/v1/manga/{draft}/relation");
    // One hidden edge must still consume its persisted slot.
    db.execute("UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='Rcap000001'",&[]).await.unwrap();
    f.call(Some(1),"POST",&draft_path,Some(make(&target2,"side_story",2)),409).await;
    let limit_state=db.query_one("SELECT version,(SELECT count(*) FROM platform.catalog_relations WHERE manga_id=$1),(SELECT count(*) FROM platform.catalog_audit WHERE entity_id=$1) FROM platform.catalog_manga WHERE id=$1",&[&draft_native]).await.unwrap();
    assert_eq!(limit_state.get::<_,i32>(0),2);assert_eq!(limit_state.get::<_,i64>(1),4096);assert_eq!(limit_state.get::<_,i64>(2),audit_before);
    let hidden=f.relation_page(draft,&account,2,0,None,200).await;assert_eq!(hidden["total"],4095);assert_eq!(hidden["persistedTotal"],4096);
    let hidden_token=hidden["snapshot"].as_str().unwrap();let next=hidden["nextOffset"].as_u64().unwrap() as u32;
    // Equal visible counts do not permit a mixed set across pages.
    db.batch_execute("UPDATE platform.catalog_manga SET protocol_source_visible=true WHERE id='Rcap000001'; UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='Rcap000002'").await.unwrap();
    f.relation_page(draft,&account,2,next,Some(hidden_token),409).await;
    db.execute("UPDATE platform.catalog_manga SET protocol_source_visible=true WHERE id='Rcap000002'",&[]).await.unwrap();
    // Full legal localized metadata may shorten a page prefix, never a field.
    let large="x".repeat(32600);
    for n in 1..=64 {
        let native=format!("Rcap{n:06}");let id=resolver.preferred(ResourceKind::Manga,&native).await.unwrap().unwrap();
        f.call(Some(1),"PUT",&format!("/api/v1/manga/{id}"),Some(json!({"version":1,"expectedAccountId":account,"description":{"ja":large}})),200).await;
    }
    let expected=db.query("SELECT a.public_id::text FROM platform.catalog_relations r JOIN platform.catalog_manga m ON m.id=r.related_id JOIN platform.public_resource_ids a ON a.kind='manga_relation' AND a.native_id=platform.public_manga_edge_key(r.manga_id,r.related_id) AND a.preferred WHERE r.manga_id=$1 ORDER BY r.created_at,m.id",&[&draft_native]).await.unwrap().into_iter().map(|r|r.get::<_,String>(0)).collect::<Vec<_>>();assert_eq!(expected.len(),4096);
    eprintln!("RELATION_BOUNDARY_PHASE paged_crawl_started elapsed_ms={}",boundary_started.elapsed().as_millis());
    let(mut offset,mut token,mut observed,mut shortened,mut large_count)=(0_u32,None::<String>,Vec::new(),false,0_usize);
    loop {
        let page=f.relation_page(draft,&account,2,offset,token.as_deref(),200).await;
        assert_eq!(page["ownerMangaId"],draft);assert_eq!(page["ownerVersion"],2);assert_eq!(page["version"],2);assert_eq!(page["total"],4096);assert_eq!(page["persistedTotal"],4096);assert_eq!(page["offset"],offset);assert_eq!(page["limit"],32);
        let actual=page["snapshot"].as_str().unwrap();if let Some(token)=&token{assert_eq!(actual,token);}else{token=Some(actual.to_owned());}
        let data=page["data"].as_array().unwrap();assert!(!data.is_empty()&&data.len()<=32);
        for edge in data {
            assert_eq!(edge["attributes"]["version"],2);assert_eq!(edge["attributes"]["ownerMangaId"],draft);observed.push(edge["id"].as_str().unwrap().to_owned());
            if let Some(description)=edge["relationships"][0]["attributes"]["description"]["ja"].as_str(){assert_eq!(description,large);large_count+=1;}
        }
        assert!(serde_json::to_vec(&page).unwrap().len()<=1024*1024);
        if let Some(next)=page["nextOffset"].as_u64(){assert_eq!(next,offset as u64+data.len() as u64);if data.len()<32&&4096-offset>=32{shortened=true;}offset=next as u32;}else{break;}
    }
    assert_eq!(observed,expected);assert_eq!(large_count,64);assert!(shortened,"Large complete metadata must exercise byte-bounded prefix paging");
    eprintln!("RELATION_BOUNDARY_PHASE paged_crawl_finished elapsed_ms={}",boundary_started.elapsed().as_millis());
    // A captured account cannot drift even to another otherwise eligible user.
    let stale_account=format!("/api/v1/nostr/manga/{draft}/relations?limit=32&ownerVersion=2&expectedAccountId={account}");let bytes=Vec::new();let proof=proof(1,11,&format!("{}{stale_account}",f.origin),"GET",&bytes);
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser2' WHERE publisher=$1",&[&public(1)]).await.unwrap();f.raw("GET",&stale_account,bytes,"application/json",Some(proof),409).await;db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser1' WHERE publisher=$1",&[&public(1)]).await.unwrap();
    // Removal stays available at capacity; version drift invalidates continuations.
    let draft_edge=private["data"][0]["id"].as_str().unwrap();
    f.call(Some(1),"DELETE",&format!("{draft_path}/{draft_edge}"),Some(json!({"version":2,"expectedAccountId":account})),200).await;
    f.relation_page(draft,&account,2,32,token.as_deref(),409).await;
    let reused=f.call(Some(1),"POST",&draft_path,Some(make(&target2,"side_story",3)),200).await;assert_eq!(reused["data"]["attributes"]["version"],4);
    let first=f.relation_page(draft,&account,4,0,None,200).await;assert_eq!(first["total"],4096);assert_eq!(first["persistedTotal"],4096);assert_ne!(first["snapshot"],token.unwrap());
    let last=f.relation_page(draft,&account,4,4095,first["snapshot"].as_str(),200).await;assert_eq!(last["data"].as_array().unwrap().len(),1);assert!(last["nextOffset"].is_null());
    let unchanged=f.state.protocol.entry(eid).await.unwrap().unwrap();assert_eq!(serde_json::to_value(unchanged.event).unwrap(),serde_json::to_value(source.event).unwrap());assert_eq!(unchanged.metadata,source.metadata);
    let original:i64=db.query_one("SELECT count(*) FROM platform.protocol_publication_commands WHERE catalog_id=$1",&[&native]).await.unwrap().get(0);assert_eq!(original,1);
    drop(db);let mut native=f.native.take().unwrap();if let Some(shutdown)=native.shutdown.take(){let _=shutdown.send(());}tokio::time::timeout(Duration::from_secs(30),native.task).await.unwrap().unwrap().unwrap();
}
