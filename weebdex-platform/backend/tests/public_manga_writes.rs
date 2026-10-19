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
        let mut request=self.http.request(method.parse().unwrap(),format!("{}{path}",self.origin))
            .header("content-type",content_type).header("origin",&self.origin);
        if let Some(value)=authorization { request=request.header("authorization",value); }
        let response=request.body(body).send().await.unwrap();
        let status=response.status().as_u16();
        let private_upload=path.split('?').next().unwrap().starts_with("/api/v1/nostr/uploads/")
            && ["/preview","/replace"].iter().any(|suffix|path.split('?').next().unwrap().ends_with(suffix));
        let private_cover=path.starts_with("/api/v1/nostr/") && (path.ends_with("/covers")||path.ends_with("/original")) || path.starts_with("/api/v1/cover/") && ["POST","PUT","DELETE"].contains(&method);
        let private_title=path.starts_with("/api/v1/manga/")&&(method=="DELETE"||path.ends_with("/lock")||path.ends_with("/unlock"));
        assert_eq!(response.headers()["cache-control"],if private_upload || private_cover || private_title {"private, no-store"}else{"no-store"},"{method} {path}");
        if private_cover || private_title {
            let vary=response.headers().get_all("vary").iter().map(|v|v.to_str().unwrap().to_ascii_lowercase()).collect::<Vec<_>>().join(",");
            assert!(vary.split(',').any(|v|v.trim()=="authorization"),"missing Authorization variation: {vary}");
            assert!(vary.split(',').any(|v|v.trim()=="origin"),"missing Origin variation: {vary}");
        }
        let text=response.text().await.unwrap();
        let value:Value=serde_json::from_str(&text).unwrap_or_else(|_|panic!("{method} {path}: non-JSON {text}"));
        assert_eq!(status,expected,"{method} {path}: {value}");
        if status>=400 { assert_eq!(value["result"],"error","{value}"); assert_eq!(value["errors"][0]["status"],status,"{value}"); }
        value
    }
    async fn call(&self,key:Option<u8>,method:&str,path:&str,payload:Option<Value>,expected:u16)->Value {
        let mut payload=payload;
        if path.starts_with("/api/v1/cover/")&&["PUT","DELETE"].contains(&method)||path.starts_with("/api/v1/manga/")&&(method=="DELETE"||path.ends_with("/lock")||path.ends_with("/unlock")){
            if let (Some(key),Some(input))=(key,payload.as_mut()){
                let user=PostgresPublicIdResolver::new(self.state.db.clone()).preferred(ResourceKind::User,&format!("PublicUploadUser{key}")).await.unwrap().unwrap();
                input["expectedAccountId"]=json!(user);
                if method=="DELETE"&&path.starts_with("/api/v1/manga/")&&input.get("removalMode").is_none(){input["removalMode"]=json!(if key==8 {"approve-hide"}else{"request-review"});}
            }
        }
        let body=payload.map_or_else(Vec::new,|v|serde_json::to_vec(&v).unwrap());
        let authorization=key.map(|key|proof(key,11,&format!("{}{path}",self.origin),method,&body));
        self.raw(method,path,body,"application/json",authorization,expected).await
    }
    async fn native(&self,kind:ResourceKind,public:&str)->String {
        PostgresPublicIdResolver::new(self.state.db.clone()).resolve(kind,&PublicUuid::parse(public).unwrap())
            .await.unwrap().unwrap().native_id
    }
}
async fn fixture()->Fixture {
    assert_catalog_fixture_ids();
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
            &[&user,&format!("upload-{key}@fixture.invalid"),&(key!=5),&(key==6),&roles,&permissions]).await.unwrap();
        db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await.unwrap();
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
#[ignore="Requires scripts/run-auth-tests.sh, isolated PostgreSQL and pinned strfry"]
async fn signed_title_create_edit_submit_publish_and_overlay_preserve_original_authority() {
    use weebdex_platform::protocol::bridge;
    let mut f=fixture().await;
    let db=f.state.db.client().await.unwrap();
    db.batch_execute(r#"UPDATE platform.users SET permissions='["manga.create","manga.update","manga.title.edit","manga.author.edit","manga.artist.edit","manga.tag.edit","manga.link.edit","manga.cover.edit"]' WHERE id IN('PublicUploadUser1','PublicUploadUser2'); UPDATE platform.users SET roles='["moderator"]',permissions='["manga.create","manga.state.update","manga.update","manga.title.edit"]' WHERE id='PublicUploadUser8'; INSERT INTO platform.catalog_authors(id,owner_id,owner_name,name) VALUES('TitleAuth1','PublicUploadUser1','Fixture','Owned author'); INSERT INTO platform.catalog_tags(id,name,tag_group) VALUES('TitleTag01','Owned tag','genre');"#).await.unwrap();
    let resolver=PostgresPublicIdResolver::new(f.state.db.clone());
    let author=resolver.preferred(ResourceKind::Author,"TitleAuth1").await.unwrap().unwrap().to_string();
    let tag=resolver.preferred(ResourceKind::Tag,"TitleTag01").await.unwrap().unwrap().to_string();
    let input=json!({"title":{"ja":"合成の題","en":"Synthetic title"},"description":{"ja":"合成の説明","en":"Synthetic description"},"altTitles":[{"ja":"別の題","en":"Another title"}],"originalLanguage":"ja","status":"ongoing","contentRating":"safe","year":2025,"authors":[author],"artists":[author],"tags":[tag],"links":{"al":"123"},"chapterNumbersResetOnNewVolume":true});
    f.call(None,"POST","/api/v1/manga",Some(input.clone()),401).await;
    f.call(Some(3),"POST","/api/v1/manga",Some(input.clone()),403).await;
    f.call(Some(4),"POST","/api/v1/manga",Some(input.clone()),403).await;
    let cookie=f.http.post(format!("{}/api/v1/manga",f.origin)).header("cookie","weebdex_session=synthetic-public-upload-cookie").json(&input).send().await.unwrap();assert_eq!(cookie.status().as_u16(),401);
    let expected=resolver.preferred(ResourceKind::User,"PublicUploadUser1").await.unwrap().unwrap();
    let mut captured=input.clone();captured["expectedAccountId"]=json!(expected.as_str());let bytes=serde_json::to_vec(&captured).unwrap();let url=format!("{}/api/v1/manga",f.origin);let token=proof(1,11,&url,"POST",&bytes);
    let before:i64=db.query_one("SELECT count(*) FROM platform.catalog_manga",&[]).await.unwrap().get(0);
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser2' WHERE publisher=$1",&[&public(1)]).await.unwrap();
    f.raw("POST","/api/v1/manga",bytes,"application/json",Some(token),409).await;
    let after:i64=db.query_one("SELECT count(*) FROM platform.catalog_manga",&[]).await.unwrap().get(0);assert_eq!(after,before);
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser1' WHERE publisher=$1",&[&public(1)]).await.unwrap();
    let create=f.call(Some(1),"POST","/api/v1/manga",Some(captured),200).await;
    let manga=create["data"]["id"].as_str().unwrap();let native=f.native(ResourceKind::Manga,manga).await;let path=format!("/api/v1/manga/{manga}");
    assert_eq!(create["data"]["attributes"]["state"],"draft");assert_eq!(create["data"]["attributes"]["title"],input["title"]);assert_eq!(create["data"]["attributes"]["description"],input["description"]);
    assert!(create["data"]["relationships"].as_array().unwrap().iter().any(|r|r["type"]=="author" && r["id"]==author));
    assert_eq!(f.http.get(format!("{}{path}",f.origin)).send().await.unwrap().status().as_u16(),404);
    f.call(Some(2),"GET",&format!("/api/v1/manga/draft/{manga}"),None,403).await;
    f.call(Some(2),"PUT",&path,Some(json!({"version":1,"title":{"en":"Foreign"}})),403).await;
    for payload in [json!({"version":1,"officialLinks":{}}),json!({"version":1,"primaryCover":null}),json!({"version":1,"authors":["TitleAuth1"]}),json!({"version":1,"title":{}}),json!({"version":1,"title":{"xx":"Title"}})] {
        f.call(Some(1),"PUT",&path,Some(payload),400).await;
    }
    f.call(Some(1),"PUT",&path,Some(json!({"version":1,"authors":[tag]})),404).await;
    f.call(Some(1),"PUT",&format!("{path}?unexpected=1"),Some(json!({"version":1})),400).await;
    let edited=f.call(Some(1),"PUT",&path,Some(json!({"version":1,"title":{"ja":"編集した題","en":"Edited title"},"description":{"ja":"編集した説明","en":"Edited description"},"lastVolume":null,"year":null,"publicationDemographic":null})),200).await;
    assert_eq!(edited["data"]["attributes"]["version"],2);assert_eq!(edited["data"]["attributes"]["year"],Value::Null);
    f.call(Some(1),"PUT",&path,Some(json!({"version":1})),409).await;
    let body=serde_json::to_vec(&json!({"version":2,"lastChapter":"12"})).unwrap();
    f.raw("PUT",&path,body.clone(),"application/json",Some(proof(1,11,&format!("{}{path}",f.origin),"POST",&body)),401).await;
    f.raw("PUT",&path,b"{\"version\":2}".to_vec(),"application/json",Some(proof(1,11,&format!("{}{path}",f.origin),"PUT",&body)),401).await;
    f.raw("PUT",&path,body.clone(),"application/json",Some(proof(1,12,&format!("{}{path}",f.origin),"PUT",&body)),401).await;
    f.raw("PUT",&path,body.clone(),"application/json",Some(proof(1,11,&format!("{}{path}/other",f.origin),"PUT",&body)),401).await;
    let token=proof(1,11,&format!("{}{path}",f.origin),"PUT",&body);
    f.raw("PUT",&path,body.clone(),"application/json",Some(token.clone()),200).await;
    f.raw("PUT",&path,body,"application/json",Some(token),401).await;
    let commit=format!("/api/v1/manga/draft/{manga}/commit");let publish=format!("/api/v1/manga/draft/{manga}/publish");let reject=format!("/api/v1/manga/draft/{manga}/reject");
    f.call(Some(1),"POST",&commit,Some(json!({"version":3})),400).await;
    signed_cover_lifecycle(&f,&native).await;
    // Read the current title token after the signed cover lifecycle.
    let version:i32=db.query_one("SELECT version FROM platform.catalog_manga WHERE id=$1",&[&native]).await.unwrap().get(0);
    f.call(Some(2),"POST",&commit,Some(json!({"version":version})),403).await;
    let submitted=f.call(Some(1),"POST",&commit,Some(json!({"version":version})),201).await;
    let v=submitted["data"]["attributes"]["version"].as_i64().unwrap();assert_eq!(submitted["data"]["attributes"]["state"],"submitted");
    f.call(Some(1),"POST",&publish,Some(json!({"version":v})),403).await;
    // Staff role can review, but cannot borrow the configured operator's key.
    f.call(Some(8),"POST",&publish,Some(json!({"version":v})),403).await;
    let count:i64=db.query_one("SELECT count(*) FROM platform.protocol_publication_commands WHERE catalog_id=$1",&[&native]).await.unwrap().get(0);assert_eq!(count,0);
    let rejected=f.call(Some(8),"POST",&reject,Some(json!({"version":v})),200).await;
    let v=rejected["data"]["attributes"]["version"].as_i64().unwrap();assert_eq!(rejected["data"]["attributes"]["state"],"rejected");
    let submitted=f.call(Some(1),"POST",&commit,Some(json!({"version":v})),201).await;let v=submitted["data"]["attributes"]["version"].as_i64().unwrap();
    db.execute("UPDATE platform.users SET roles='[\"moderator\"]',permissions=permissions||'[\"manga.state.update\"]'::jsonb WHERE id='PublicUploadUser1'",&[]).await.unwrap();
    let expected_cover=cover_review_prerequisite(&f,&native,manga,v).await;
    // Fail only local association after the independently durable original
    // acceptance, then explicitly retry the same captured command. Public
    // cover removal is fenced; the SQL tombstone below is a labeled fault seam
    // proving that immutable retries do not rebuild input from mutable rows.
    db.batch_execute("CREATE FUNCTION platform.cover_review_association_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'owned original association fault'; END $$; CREATE TRIGGER cover_review_association_fault BEFORE UPDATE OF protocol_series_id ON platform.catalog_manga FOR EACH ROW WHEN (NEW.protocol_series_id IS NOT NULL AND OLD.protocol_series_id IS DISTINCT FROM NEW.protocol_series_id) EXECUTE FUNCTION platform.cover_review_association_fault();").await.unwrap();
    f.call(Some(1),"POST",&publish,Some(json!({"version":v})),503).await;
    db.batch_execute("DROP TRIGGER cover_review_association_fault ON platform.catalog_manga;DROP FUNCTION platform.cover_review_association_fault();").await.unwrap();
    let captured=db.query_one("SELECT state,source_manifest,event_id FROM platform.protocol_publication_commands WHERE catalog_id=$1",&[&native]).await.unwrap();assert_eq!(captured.get::<_,String>(0),"source_accepted");let manifest:Vec<u8>=captured.get(1);let captured_id:String=captured.get(2);assert!(!manifest.is_empty());
    let live=db.query_one("SELECT id,version FROM platform.catalog_covers WHERE manga_id=$1 AND deleted_at IS NULL AND platform.protocol_source_is_visible(protocol_event_id) LIMIT 1",&[&native]).await.unwrap();let live_id:String=live.get(0);let live_id=resolver.preferred(ResourceKind::Cover,&live_id).await.unwrap().unwrap();
    f.call(Some(8),"DELETE",&format!("/api/v1/cover/{live_id}"),Some(json!({"version":live.get::<_,i32>(1)})),409).await;
    db.execute("UPDATE platform.catalog_covers SET deleted_at=clock_timestamp() WHERE manga_id=$1 AND deleted_at IS NULL",&[&native]).await.unwrap();
    let published=f.call(Some(1),"POST",&publish,Some(json!({"version":v})),200).await;
    assert_eq!(published["data"]["attributes"]["state"],"published");assert_eq!(published["data"]["attributes"]["title"]["en"],"Edited title");
    assert_eq!(published["data"]["attributes"]["nostr"]["publicationScope"],"original-protocol");
    let event=published["data"]["attributes"]["nostr"]["eventId"].as_str().unwrap();assert_eq!(event,captured_id);assert_eq!(db.query_one("SELECT source_manifest FROM platform.protocol_publication_commands WHERE catalog_id=$1",&[&native]).await.unwrap().get::<_,Vec<u8>>(0),manifest);let eid:[u8;32]=hex::decode(event).unwrap().try_into().unwrap();
    tokio::time::timeout(Duration::from_secs(20),async{loop{if f.state.protocol.entry(eid).await.unwrap().is_some(){break;}tokio::time::sleep(Duration::from_millis(30)).await;}}).await.unwrap();
    for _ in 0..8 {let r=bridge::import_page(&f.state).await.unwrap();assert_eq!(r.rejected,0);if r.completed_sweep && r.deferred==0 && r.scanned>0 {break;}}
    let source=f.state.protocol.entry(eid).await.unwrap().unwrap();source.event.verify().unwrap();assert_eq!(source.event.pubkey,public(1));assert_eq!(source.event.kind,1002);
    let source_metadata=&source.metadata;assert_eq!(source_metadata["titles"]["ja"],"編集した題");assert_eq!(source_metadata["titles"]["en"],"Edited title");assert_eq!(source_metadata["description"]["ja"],"編集した説明");assert_eq!(source_metadata["description"]["en"],"Edited description");assert_eq!(source_metadata["altTitles"][0],input["altTitles"][0]);
    assert_eq!(source_metadata["cover"]["sha256"],expected_cover,"Original publication must select the eligible original-language cover, not another-language main");
    let public_read:Value=f.http.get(format!("{}{path}",f.origin)).send().await.unwrap().json().await.unwrap();assert_eq!(public_read["data"]["id"],manga);assert_eq!(public_read["data"]["attributes"]["nostr"]["publicationStatus"],"locally_hosted");
    // Import preserves the explicit local tombstones from the fault seam; it
    // does not resurrect removed covers. Add a real signed local cover for the
    // inherited annotation and materialized-replay-after-removal phase.
    assert_eq!(db.query_one("SELECT count(*) FROM platform.catalog_covers WHERE manga_id=$1 AND deleted_at IS NULL",&[&native]).await.unwrap().get::<_,i64>(0),0);
    let account=resolver.preferred(ResourceKind::User,"PublicUploadUser1").await.unwrap().unwrap();let cover_path=format!("/api/v1/cover/{manga}");
    let (body,mime)=cover_multipart(published["data"]["attributes"]["version"].as_i64().unwrap() as i32,account.as_str(),&[("locale","ja")],false);
    f.raw("POST",&cover_path,body.clone(),&mime,Some(proof(1,11,&format!("{}{cover_path}",f.origin),"POST",&body)),201).await;
    let covers=f.call(Some(1),"GET",&format!("/api/v1/nostr/manga/{manga}/covers"),None,200).await;
    let original=covers["data"].as_array().unwrap().iter().find(|row|row["attributes"]["nostr"]["original"].is_object()).unwrap();
    let cover_id=original["id"].as_str().unwrap();let cover_version=original["attributes"]["version"].as_i64().unwrap();
    let annotation=f.call(Some(1),"PUT",&format!("/api/v1/cover/{cover_id}"),Some(json!({"version":cover_version,"description":"Instance-only cover annotation"})),200).await;
    assert_eq!(annotation["data"]["attributes"]["nostr"]["original"],original["attributes"]["nostr"]["original"]);
    assert_eq!(annotation["data"]["attributes"]["nostr"]["sourceEventId"],original["attributes"]["nostr"]["sourceEventId"]);
    assert_eq!(annotation["data"]["attributes"]["nostr"]["originalEventRewritten"],false);
    remove_live_covers(&f,&native).await;
    assert_eq!(db.query_one("SELECT count(*) FROM platform.catalog_covers WHERE manga_id=$1 AND deleted_at IS NULL",&[&native]).await.unwrap().get::<_,i64>(0),0);
    // A fresh valid proof may replay the exact original publish command, once.
    let replay=f.call(Some(1),"POST",&publish,Some(json!({"version":v})),200).await;assert_eq!(replay["data"]["attributes"]["nostr"]["eventId"],event);
    f.repository.set_publisher_admission(id(1),false).await.unwrap();
    f.call(Some(1),"POST",&publish,Some(json!({"version":v})),403).await;
    f.repository.set_publisher_admission(id(1),true).await.unwrap();
    let actual=replay["data"]["attributes"]["version"].as_i64().unwrap();
    let overlay=f.call(Some(1),"PUT",&path,Some(json!({"version":actual,"title":{"ja":"このインスタンスの題","en":"Local annotation"}})),200).await;
    assert_eq!(overlay["data"]["attributes"]["nostr"]["metadataScope"],"instance-local");assert_eq!(overlay["data"]["attributes"]["title"]["en"],"Local annotation");
    let unchanged=f.state.protocol.entry(eid).await.unwrap().unwrap();assert_eq!(serde_json::to_value(&unchanged.event).unwrap(),serde_json::to_value(&source.event).unwrap());assert_eq!(unchanged.metadata,source.metadata);
    let provenance:Value=db.query_one("SELECT metadata->'protocol_source_metadata' FROM platform.catalog_manga WHERE id=$1",&[&native]).await.unwrap().get(0);assert_eq!(&provenance,source_metadata);
    let retained=f.state.protocol.entry(eid).await.unwrap().unwrap();assert_eq!(serde_json::to_value(&retained.event).unwrap(),serde_json::to_value(&source.event).unwrap());assert_eq!(retained.metadata,source.metadata);

    // Also annotate a genuinely imported title with no native publication
    // command. It must disclose the same local scope and exact source identity.
    let runtime=f.state.protocol.publication_runtime().await.unwrap();
    let metadata=manga_node::series_metadata::SeriesMetadata {
        titles:std::collections::BTreeMap::from([("en".into(),"Independent original title".into())]),
        alt_titles:None,description:None,categories:None,credits:None,original_language:Some("en".into()),
        status:Some("ongoing".into()),content_rating:Some("safe".into()),publication_demographic:None,year:None,cover:None,
    };
    let independent=runtime.publish_operator(weebdex_platform::protocol::publication::PublicationInput {
        kind:1002,canonical_manifest:manga_node::platform_bridge::encode_series(&metadata).unwrap(),
        originals:std::collections::BTreeMap::new(),created_at:None,
    }).await.unwrap().event;
    let source:[u8;32]=hex::decode(&independent.id).unwrap().try_into().unwrap();
    tokio::time::timeout(Duration::from_secs(20),async{loop{if f.state.protocol.entry(source).await.unwrap().is_some(){break;}tokio::time::sleep(Duration::from_millis(30)).await;}}).await.unwrap();
    for _ in 0..8 {let r=bridge::import_page(&f.state).await.unwrap();assert_eq!(r.rejected,0);if r.completed_sweep && r.deferred==0 && r.scanned>0 {break;}}
    let imported=db.query_one("SELECT id,version FROM platform.catalog_manga WHERE protocol_series_id=$1",&[&independent.id]).await.unwrap();let imported_native:String=imported.get(0);let imported_version:i32=imported.get(1);
    let imported_public=resolver.preferred(ResourceKind::Manga,&imported_native).await.unwrap().unwrap();
    let annotated=f.call(Some(1),"PUT",&format!("/api/v1/manga/{imported_public}"),Some(json!({"version":imported_version,"title":{"en":"Imported title local annotation"}})),200).await;
    assert_eq!(annotated["data"]["attributes"]["nostr"]["eventId"],independent.id);assert_eq!(annotated["data"]["attributes"]["nostr"]["metadataScope"],"instance-local");
    let retained=f.state.protocol.entry(source).await.unwrap().unwrap();assert_eq!(serde_json::to_value(retained.event).unwrap(),serde_json::to_value(independent).unwrap());assert_eq!(retained.metadata["titles"]["en"],"Independent original title");
    f.repository.set_account_admission(id(1),false).await.unwrap();
    let current=overlay["data"]["attributes"]["version"].as_i64().unwrap();f.call(Some(1),"PUT",&path,Some(json!({"version":current,"title":{"en":"Revoked"}})),403).await;
    f.repository.set_account_admission(id(1),true).await.unwrap();
    // Hold the actual authority row until the HTTP mutation is waiting on it,
    // revoke its grant, then release. The signed request must decode current
    // grants after the wait, never a pre-transaction authority snapshot.
    let mut blocker=f.state.db.client().await.unwrap();let tx=blocker.transaction().await.unwrap();
    tx.query_one("SELECT id FROM platform.users WHERE id='PublicUploadUser1' FOR NO KEY UPDATE",&[]).await.unwrap();
    let bytes=serde_json::to_vec(&json!({"version":current,"title":{"en":"Concurrent revoked grant"}})).unwrap();
    let http=f.http.clone();let target=format!("{}{path}",f.origin);let token=proof(1,11,&target,"PUT",&bytes);
    let mutation=tokio::spawn(async move{http.put(target).header("content-type","application/json").header("authorization",token).body(bytes).send().await.unwrap()});
    tokio::time::timeout(Duration::from_secs(5),async{loop{
        let waiting:bool=db.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE 'SELECT u.id,u.username,u.verified,u.roles,u.permissions,u.disabled%')",&[]).await.unwrap().get(0);
        if waiting {break;}tokio::time::sleep(Duration::from_millis(10)).await;
    }}).await.unwrap();
    tx.execute("UPDATE platform.users SET permissions='[]' WHERE id='PublicUploadUser1'",&[]).await.unwrap();tx.commit().await.unwrap();
    let refused=mutation.await.unwrap();assert_eq!(refused.status().as_u16(),403);let refused:Value=refused.json().await.unwrap();assert_eq!(refused["result"],"error");
    let unchanged:i32=db.query_one("SELECT version FROM platform.catalog_manga WHERE id=$1",&[&native]).await.unwrap().get(0);assert_eq!(i64::from(unchanged),current);
    let count:i64=db.query_one("SELECT count(*) FROM platform.protocol_publication_commands WHERE catalog_id=$1",&[&native]).await.unwrap().get(0);assert_eq!(count,1);
    title_disposition_lifecycle(&f,&native,event).await;
    drop(db);let mut native=f.native.take().unwrap();if let Some(shutdown)=native.shutdown.take(){let _=shutdown.send(());}
    tokio::time::timeout(Duration::from_secs(30),native.task).await.unwrap().unwrap().unwrap();
}

// Actual signed removal after submit cannot erase a fresh publication's
// original prerequisite. Replacing it with an eligible secondary and a newer
// other-language main proves that the serialized original uses the same rule.
async fn remove_live_covers(f:&Fixture,native:&str) {
    let db=f.state.db.client().await.unwrap();let resolver=PostgresPublicIdResolver::new(f.state.db.clone());
    let moderator=resolver.preferred(ResourceKind::User,"PublicUploadUser8").await.unwrap().unwrap();
    let covers=db.query("SELECT id,version FROM platform.catalog_covers WHERE manga_id=$1 AND deleted_at IS NULL",&[&native]).await.unwrap();
    assert!(!covers.is_empty());
    for row in covers {
        let id:String=row.get(0);let token:i32=row.get(1);let id=resolver.preferred(ResourceKind::Cover,&id).await.unwrap().unwrap();
        let path=format!("/api/v1/cover/{id}");let body=serde_json::to_vec(&json!({"version":token,"expectedAccountId":moderator})).unwrap();
        let response=f.http.delete(format!("{}{path}",f.origin)).header("origin",&f.origin).header("content-type","application/json").header("authorization",proof(8,11,&format!("{}{path}",f.origin),"DELETE",&body)).body(body).send().await.unwrap();assert_eq!(response.status().as_u16(),204);
    }
}
async fn cover_review_prerequisite(f:&Fixture,native:&str,public_id:&str,version:i64)->String {
    let db=f.state.db.client().await.unwrap();
    let resolver=PostgresPublicIdResolver::new(f.state.db.clone());
    let account=resolver.preferred(ResourceKind::User,"PublicUploadUser1").await.unwrap().unwrap();
    remove_live_covers(f,native).await;
    assert_eq!(db.query_one("SELECT version FROM platform.catalog_manga WHERE id=$1",&[&native]).await.unwrap().get::<_,i32>(0) as i64,version,"Separately versioned cover removal leaves the submitted title token current");
    let audit:i64=db.query_one("SELECT count(*) FROM platform.catalog_audit WHERE entity_type='manga' AND entity_id=$1",&[&native]).await.unwrap().get(0);
    f.call(Some(1),"POST",&format!("/api/v1/manga/draft/{public_id}/publish"),Some(json!({"version":version,"expectedAccountId":account})),400).await;
    let row=db.query_one("SELECT state,version,(SELECT count(*) FROM platform.protocol_publication_commands WHERE catalog_id=$1),(SELECT count(*) FROM platform.catalog_audit WHERE entity_type='manga' AND entity_id=$1) FROM platform.catalog_manga WHERE id=$1",&[&native]).await.unwrap();
    assert_eq!(row.get::<_,String>(0),"submitted");assert_eq!(row.get::<_,i32>(1) as i64,version);assert_eq!(row.get::<_,i64>(2),0);assert_eq!(row.get::<_,i64>(3),audit);
    let path=format!("/api/v1/cover/{public_id}");let mut expected=String::new();
    for (language,main,color) in [("ja","false",[9,71,113]),("en","true",[197,39,24])] {
        let (mut body,mime)=cover_multipart(version as i32,account.as_str(),&[("locale",language),("main",main)],false);
        let mut bytes=Cursor::new(Vec::new());DynamicImage::ImageRgb8(RgbImage::from_pixel(13,17,Rgb(color))).write_to(&mut bytes,ImageFormat::Png).unwrap();let bytes=bytes.into_inner();
        let original=png();let start=body.windows(original.len()).position(|part|part==original).unwrap();body.splice(start..start+original.len(),bytes.clone());
        let response=f.raw("POST",&path,body.clone(),&mime,Some(proof(1,11,&format!("{}{path}",f.origin),"POST",&body)),201).await;
        assert_eq!(response["data"]["attributes"]["nostr"]["original"]["sha256"],auth::hash(&bytes));
        if language=="ja"{expected=auth::hash(&bytes);}
    }
    // A real signed original supplies the cover identity. Only its owned
    // source-state transition is a SQL visibility seam, not a claim that a
    // public kind5 deletion was sent or approved by this title workflow.
    let mut bytes=Cursor::new(Vec::new());DynamicImage::ImageRgb8(RgbImage::from_pixel(13,17,Rgb([9,71,113]))).write_to(&mut bytes,ImageFormat::Png).unwrap();let bytes=bytes.into_inner();
    let source_metadata=manga_node::series_metadata::SeriesMetadata{titles:std::collections::BTreeMap::from([("ja".into(),"Owned cover visibility fixture".into())]),alt_titles:None,description:None,categories:None,credits:None,original_language:Some("ja".into()),status:Some("ongoing".into()),content_rating:Some("safe".into()),publication_demographic:None,year:None,cover:Some(manga_node::manifest::Page{hash:expected.clone(),size:bytes.len() as u64,mime:"image/png".into(),width:13,height:17})};
    let source=f.state.protocol.publication_runtime().await.unwrap().publish_operator(weebdex_platform::protocol::publication::PublicationInput{kind:1002,canonical_manifest:manga_node::platform_bridge::encode_series(&source_metadata).unwrap(),originals:std::collections::BTreeMap::from([(expected.clone(),bytes)]),created_at:None}).await.unwrap().event;source.verify().unwrap();
    let source_id:[u8;32]=hex::decode(&source.id).unwrap().try_into().unwrap();tokio::time::timeout(Duration::from_secs(20),async{loop{if f.state.protocol.entry(source_id).await.unwrap().is_some(){break;}tokio::time::sleep(Duration::from_millis(20)).await;}}).await.unwrap();
    db.execute("UPDATE platform.catalog_covers SET protocol_event_id=$2 WHERE manga_id=$1 AND language='ja' AND deleted_at IS NULL",&[&native,&source.id]).await.unwrap();
    db.execute("UPDATE events SET state='withdrawing' WHERE id=$1",&[&&source_id[..]]).await.unwrap();
    f.call(Some(1),"POST",&format!("/api/v1/manga/draft/{public_id}/publish"),Some(json!({"version":version,"expectedAccountId":account})),400).await;
    assert_eq!(db.query_one("SELECT raw_event FROM events WHERE id=$1",&[&&source_id[..]]).await.unwrap().get::<_,Value>(0),serde_json::to_value(&source).unwrap());
    assert_eq!(db.query_one("SELECT count(*) FROM platform.protocol_publication_commands WHERE catalog_id=$1",&[&native]).await.unwrap().get::<_,i64>(0),0);
    // A fresh eligible secondary restores readiness; the newer other-language
    // main and the still-withdrawing prior source both remain ineligible.
    let (mut body,mime)=cover_multipart(version as i32,account.as_str(),&[("locale","ja"),("main","false")],false);
    let mut bytes=Cursor::new(Vec::new());DynamicImage::ImageRgb8(RgbImage::from_pixel(13,17,Rgb([83,29,121]))).write_to(&mut bytes,ImageFormat::Png).unwrap();let bytes=bytes.into_inner();let original=png();let start=body.windows(original.len()).position(|part|part==original).unwrap();body.splice(start..start+original.len(),bytes.clone());
    f.raw("POST",&path,body.clone(),&mime,Some(proof(1,11,&format!("{}{path}",f.origin),"POST",&body)),201).await;expected=auth::hash(&bytes);
    let main: String=db.query_one("SELECT language FROM platform.catalog_covers WHERE manga_id=$1 AND deleted_at IS NULL AND is_main",&[&native]).await.unwrap().get(0);assert_eq!(main,"en");
    expected
}

fn cover_multipart(version:i32, account:&str, fields:&[(&str,&str)], invalid_after_file:bool)->(Vec<u8>,String) {
    let boundary="OwnedSignedCoverBoundary";
    let mut body=format!("--{boundary}\r\nContent-Disposition: form-data; name=\"expectedVersion\"\r\n\r\n{version}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"expectedAccountId\"\r\n\r\n{account}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"synthetic-cover.png\"\r\nContent-Type: image/png\r\n\r\n").into_bytes();
    body.extend(png());
    for (key,value) in fields {body.extend(format!("\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"{key}\"\r\n\r\n{value}").as_bytes());}
    if invalid_after_file {body.extend(format!("\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"unrequestedImageRevision\"\r\n\r\ntrue").as_bytes());}
    body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());(body,format!("multipart/form-data; boundary={boundary}"))
}
fn object_files(root:&std::path::Path)->usize {if !root.is_dir(){return 0;}std::fs::read_dir(root).unwrap().map(|entry|{let path=entry.unwrap().path();if path.is_dir(){object_files(&path)}else{1}}).sum()}
async fn signed_cover_lifecycle(f:&Fixture,native:&str) {
    let db=f.state.db.client().await.unwrap();
    db.execute("UPDATE platform.users SET permissions=permissions || '[\"cover.delete\",\"manga.cover.edit\"]'::jsonb WHERE id='PublicUploadUser8'",&[]).await.unwrap();
    let public=PostgresPublicIdResolver::new(f.state.db.clone()).preferred(ResourceKind::Manga,native).await.unwrap().unwrap().to_string();
    let version:i32=db.query_one("SELECT version FROM platform.catalog_manga WHERE id=$1",&[&native]).await.unwrap().get(0);
    let account=PostgresPublicIdResolver::new(f.state.db.clone()).preferred(ResourceKind::User,"PublicUploadUser1").await.unwrap().unwrap().to_string();
    let moderator=PostgresPublicIdResolver::new(f.state.db.clone()).preferred(ResourceKind::User,"PublicUploadUser8").await.unwrap().unwrap().to_string();
    let path=format!("/api/v1/cover/{public}");let list=format!("/api/v1/nostr/manga/{public}/covers");
    let (body,mime)=cover_multipart(version,&account,&[("locale","ja"),("main","true"),("description","Signed original cover")],false);
    f.raw("POST",&path,body.clone(),&mime,None,401).await;
    let other=PostgresPublicIdResolver::new(f.state.db.clone()).preferred(ResourceKind::User,"PublicUploadUser2").await.unwrap().unwrap().to_string();
    let (foreign,foreign_mime)=cover_multipart(version,&other,&[("locale","ja")],false);
    f.raw("POST",&path,foreign.clone(),&foreign_mime,Some(proof(2,11,&format!("{}{path}",f.origin),"POST",&foreign)),403).await;
    let mut tampered=body.clone();tampered.push(b' ');
    f.raw("POST",&path,tampered,&mime,Some(proof(1,11,&format!("{}{path}",f.origin),"POST",&body)),401).await;
    let (stale,stale_mime)=cover_multipart(version+1,&account,&[("locale","ja")],false);
    f.raw("POST",&path,stale.clone(),&stale_mime,Some(proof(1,11,&format!("{}{path}",f.origin),"POST",&stale)),409).await;
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser8' WHERE publisher=$1",&[&hex::encode(id(1))]).await.unwrap();
    f.raw("POST",&path,body.clone(),&mime,Some(proof(1,11,&format!("{}{path}",f.origin),"POST",&body)),409).await;
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser1' WHERE publisher=$1",&[&hex::encode(id(1))]).await.unwrap();
    let token=proof(1,11,&format!("{}{path}",f.origin),"POST",&body);
    let response=f.raw("POST",&path,body.clone(),&mime,Some(token.clone()),201).await;
    f.raw("POST",&path,body,&mime,Some(token),401).await;
    let cover=response["data"]["id"].as_str().unwrap();let cover_native=f.native(ResourceKind::Cover,cover).await;
    assert_eq!(response["data"]["type"],"cover_art");assert_eq!(response["data"]["attributes"]["locale"],"ja");assert_eq!(response["data"]["attributes"]["version"],1);
    assert_eq!(response["data"]["attributes"]["nostr"]["original"]["sha256"],auth::hash(&png()));assert_eq!(response["data"]["attributes"]["nostr"]["metadataScope"],"instance-local");
    assert!(response["data"]["relationships"].as_array().unwrap().iter().any(|row|row["type"]=="manga"&&row["id"]==public));
    let variants:Vec<String>=db.query("SELECT variant FROM platform.cover_media WHERE cover_id=$1 ORDER BY variant",&[&cover_native]).await.unwrap().into_iter().map(|r|r.get(0)).collect();assert_eq!(variants,vec!["original"]);
    let key:String=db.query_one("SELECT o.storage_key FROM platform.cover_media c JOIN platform.media_objects o ON o.id=c.object_id WHERE c.cover_id=$1 AND c.variant='original'",&[&cover_native]).await.unwrap().get(0);
    assert_eq!(tokio::fs::read(domains::uploads::storage::object_path(&f.state.config.uploads_dir,&key).unwrap()).await.unwrap(),png());
    f.call(None,"GET",&format!("/api/v1/cover/{cover}"),None,404).await;
    f.call(None,"GET",&list,None,401).await;f.call(Some(2),"GET",&list,None,404).await;
    let own=f.call(Some(1),"GET",&list,None,200).await;assert!(own["data"].as_array().unwrap().iter().any(|r|r["id"]==cover));
    let preview=format!("/api/v1/nostr/covers/{cover}/original");
    let read=f.http.get(format!("{}{preview}",f.origin)).header("origin",&f.origin).header("authorization",proof(1,11,&format!("{}{preview}",f.origin),"GET",&[])).send().await.unwrap();assert_eq!(read.status().as_u16(),200);assert_eq!(read.headers()["cache-control"],"private, no-store");assert_eq!(read.headers()["content-type"],"image/png");let vary=read.headers().get_all("vary").iter().map(|v|v.to_str().unwrap().to_ascii_lowercase()).collect::<Vec<_>>().join(",");assert!(vary.contains("authorization")&&vary.contains("origin"));assert_eq!(read.bytes().await.unwrap().as_ref(),png());
    let forbidden=f.http.get(format!("{}{preview}",f.origin)).header("authorization",proof(2,11,&format!("{}{preview}",f.origin),"GET",&[])).send().await.unwrap();assert_eq!(forbidden.status().as_u16(),404);assert_eq!(forbidden.headers()["cache-control"],"private, no-store");assert!(forbidden.headers().get_all("vary").iter().any(|v|v.to_str().unwrap().to_ascii_lowercase().contains("authorization")));
    f.call(None,"GET",&preview,None,401).await;
    let target=format!("/api/v1/cover/{cover}");
    // Promoting B implicitly changes A: its CAS and audit must move too.
    let (second,second_mime)=cover_multipart(version,&account,&[("locale","ja"),("main","true")],false);
    let second=f.raw("POST",&path,second.clone(),&second_mime,Some(proof(1,11,&format!("{}{path}",f.origin),"POST",&second)),201).await;
    let second_id=second["data"]["id"].as_str().unwrap();let second_native=f.native(ResourceKind::Cover,second_id).await;
    let a=db.query_one("SELECT is_main,version FROM platform.catalog_covers WHERE id=$1",&[&cover_native]).await.unwrap();assert!(!a.get::<_,bool>(0));assert_eq!(a.get::<_,i32>(1),2);
    assert_eq!(db.query_one("SELECT count(*) FROM platform.catalog_audit WHERE entity_type='cover' AND entity_id=$1 AND operation='main-clear' AND old_version=1 AND new_version=2",&[&cover_native]).await.unwrap().get::<_,i64>(0),1);
    f.call(Some(1),"PUT",&target,Some(json!({"version":1,"main":true})),409).await;
    let accepted=f.call(Some(1),"PUT",&target,Some(json!({"version":2,"locale":"ja","description":"Updated annotation","volume":"2","main":true})),200).await;assert_eq!(accepted["data"]["attributes"]["version"],3);
    let b=db.query_one("SELECT is_main,version FROM platform.catalog_covers WHERE id=$1",&[&second_native]).await.unwrap();assert!(!b.get::<_,bool>(0));assert_eq!(b.get::<_,i32>(1),2);
    f.call(Some(1),"PUT",&format!("/api/v1/cover/{second_id}"),Some(json!({"version":1,"main":true})),409).await;
    f.call(Some(1),"PUT",&target,Some(json!({"version":1,"description":"Stale"})),409).await;
    f.call(Some(1),"PUT",&target,Some(json!({"version":3,"fileName":"swap.png"})),400).await;
    f.call(Some(1),"DELETE",&target,Some(json!({"version":3})),403).await;
    let missing=serde_json::to_vec(&json!({"version":3,"description":"Missing account intent"})).unwrap();
    f.raw("PUT",&target,missing.clone(),"application/json",Some(proof(1,11,&format!("{}{target}",f.origin),"PUT",&missing)),400).await;
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser8' WHERE publisher=$1",&[&hex::encode(id(1))]).await.unwrap();
    f.call(Some(1),"PUT",&target,Some(json!({"version":3,"description":"Rebound actor"})),409).await;
    f.call(Some(1),"DELETE",&target,Some(json!({"version":3})),409).await;
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser1' WHERE publisher=$1",&[&hex::encode(id(1))]).await.unwrap();
    // A saved UUID never bypasses a withdrawn parent, including private reads.
    db.execute("UPDATE platform.catalog_manga SET protocol_source_visible=FALSE WHERE id=$1",&[&native]).await.unwrap();
    f.call(Some(1),"GET",&list,None,404).await;
    f.call(Some(1),"GET",&preview,None,404).await;
    f.call(Some(1),"PUT",&target,Some(json!({"version":3,"description":"Hidden"})),404).await;
    let (hidden,hidden_mime)=cover_multipart(version,&account,&[("locale","ja")],false);
    f.raw("POST",&path,hidden.clone(),&hidden_mime,Some(proof(1,11,&format!("{}{path}",f.origin),"POST",&hidden)),404).await;
    db.execute("UPDATE platform.catalog_manga SET protocol_source_visible=TRUE WHERE id=$1",&[&native]).await.unwrap();
    // Failure after native cover/main/alias writes rolls back rows, audit and
    // original object bytes, without replacing the existing main cover.
    let before:i64=db.query_one("SELECT count(*) FROM platform.public_resource_ids WHERE kind='cover'",&[]).await.unwrap().get(0);
    let bytes_before=object_files(&f.state.config.uploads_dir.join("objects"));
    let audit_before:i64=db.query_one("SELECT count(*) FROM platform.catalog_audit WHERE entity_type='cover'",&[]).await.unwrap().get(0);
    let (bad,bad_mime)=cover_multipart(version,&account,&[("locale","ja")],true);
    f.raw("POST",&path,bad.clone(),&bad_mime,Some(proof(1,11,&format!("{}{path}",f.origin),"POST",&bad)),400).await;
    db.batch_execute("CREATE FUNCTION platform.cover_fixture_reject() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'owned cover rollback seam'; END $$; CREATE TRIGGER cover_fixture_reject BEFORE INSERT ON platform.cover_media FOR EACH ROW EXECUTE FUNCTION platform.cover_fixture_reject();").await.unwrap();
    let (rollback,rollback_mime)=cover_multipart(version,&account,&[("locale","ja"),("main","true")],false);
    f.raw("POST",&path,rollback.clone(),&rollback_mime,Some(proof(1,11,&format!("{}{path}",f.origin),"POST",&rollback)),503).await;
    db.batch_execute("DROP TRIGGER cover_fixture_reject ON platform.cover_media;DROP FUNCTION platform.cover_fixture_reject();").await.unwrap();
    assert_eq!(db.query_one("SELECT count(*) FROM platform.public_resource_ids WHERE kind='cover'",&[]).await.unwrap().get::<_,i64>(0),before);assert_eq!(object_files(&f.state.config.uploads_dir.join("objects")),bytes_before);
    let retained=db.query_one("SELECT is_main,version FROM platform.catalog_covers WHERE id=$1",&[&cover_native]).await.unwrap();assert!(retained.get::<_,bool>(0));assert_eq!(retained.get::<_,i32>(1),3);
    assert_eq!(db.query_one("SELECT count(*) FROM platform.catalog_audit WHERE entity_type='cover'",&[]).await.unwrap().get::<_,i64>(0),audit_before);
    let (spare,spare_mime)=cover_multipart(version,&account,&[("locale","ja")],false);
    let spare=f.raw("POST",&path,spare.clone(),&spare_mime,Some(proof(1,11,&format!("{}{path}",f.origin),"POST",&spare)),201).await;
    let spare=spare["data"]["id"].as_str().unwrap();let target=format!("/api/v1/cover/{spare}");
    f.call(Some(8),"DELETE",&target,Some(json!({"version":2})),409).await;
    let bytes=serde_json::to_vec(&json!({"version":1,"expectedAccountId":moderator})).unwrap();
    let deleted=f.http.delete(format!("{}{target}",f.origin)).header("content-type","application/json").header("authorization",proof(8,11,&format!("{}{target}",f.origin),"DELETE",&bytes)).body(bytes).send().await.unwrap();assert_eq!(deleted.status().as_u16(),204);
    f.call(Some(8),"PUT",&target,Some(json!({"version":1,"locale":"en"})),404).await;
    f.repository.set_account_admission(id(1),false).await.unwrap();f.call(Some(1),"GET",&list,None,403).await;f.repository.set_account_admission(id(1),true).await.unwrap();
}

// This stays in the one owned title fixture: no duplicate static accounts,
// runtime database reuse or fabricated source publication IDs.
async fn title_disposition_lifecycle(f:&Fixture,native:&str,source_id:&str){
    let db=f.state.db.client().await.unwrap();
    let source_key:[u8;32]=hex::decode(source_id).unwrap().try_into().unwrap();
    let original=f.state.protocol.entry(source_key).await.unwrap().unwrap();
    let deletions:i64=db.query_one("SELECT count(*) FROM events WHERE kind=5",&[]).await.unwrap().get(0);
    let object_count=object_files(&f.state.config.uploads_dir.join("objects"));
    db.batch_execute(r#"UPDATE platform.users SET roles='["user"]',permissions='["manga.create","manga.update","manga.title.edit","report.create"]' WHERE id IN('PublicUploadUser1','PublicUploadUser2');UPDATE platform.users SET roles='["moderator"]',permissions='["manga.create","manga.update","manga.title.edit","manga.cover.edit","manga.lock.update","manga.delete","report.update","report.create"]' WHERE id='PublicUploadUser8';"#).await.unwrap();
    db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES('title-disposition-reviewer','PublicUploadUser8',$1,clock_timestamp()+interval '1 hour')",&[&Sha256::digest(b"synthetic-title-reviewer-cookie").to_vec()]).await.unwrap();
    let resolver=PostgresPublicIdResolver::new(f.state.db.clone());
    let public=resolver.preferred(ResourceKind::Manga,native).await.unwrap().unwrap().to_string();
    let owner=resolver.preferred(ResourceKind::User,"PublicUploadUser1").await.unwrap().unwrap().to_string();
    let moderator=resolver.preferred(ResourceKind::User,"PublicUploadUser8").await.unwrap().unwrap().to_string();
    let path=format!("/api/v1/manga/{public}");let lock=format!("{path}/lock");let unlock=format!("{path}/unlock");
    let version:i32=db.query_one("SELECT version FROM platform.catalog_manga WHERE id=$1",&[&native]).await.unwrap().get(0);
    // Sign the exact ambiguous bytes. Every duplicate must fail before any
    // catalogue/report effect, even with a genuine moderator's current grants.
    // Value-based parsing previously selected the last account/version/effect;
    // review followed by approve-hide could immediately hide this title.
    let unchanged_sql="SELECT jsonb_build_object('title',to_jsonb(m),'catalogAudit',(SELECT count(*) FROM platform.catalog_audit WHERE entity_type='manga' AND entity_id=$1),'reports',(SELECT count(*) FROM platform.reports WHERE entity_type='manga' AND entity_id=$1),'reportAudit',(SELECT count(*) FROM platform.report_audit a JOIN platform.reports r ON r.id=a.report_id WHERE r.entity_type='manga' AND r.entity_id=$1),'sourceDeletions',(SELECT count(*) FROM events WHERE kind=5)) FROM platform.catalog_manga m WHERE m.id=$1";
    let before:Value=db.query_one(unchanged_sql,&[&native]).await.unwrap().get(0);
    for (method,target,body) in [
        ("POST",&lock,format!(r#"{{"version":{},"version":{version},"expectedAccountId":"{moderator}"}}"#,version-1)),
        ("POST",&unlock,format!(r#"{{"version":{version},"version":{version},"expectedAccountId":"{moderator}"}}"#)),
        ("POST",&lock,format!(r#"{{"version":{version},"expectedAccountId":"{owner}","expectedAccountId":"{moderator}"}}"#)),
        ("POST",&lock,format!(r#"{{"version":{version},"expectedAccountId":"{owner}","expectedAccount\u0049d":"{moderator}"}}"#)),
        ("DELETE",&path,format!(r#"{{"version":{version},"expectedAccountId":"{moderator}","removalMode":"request-review","removalMode":"approve-hide"}}"#)),
        ("DELETE",&path,format!(r#"{{"version":{version},"expectedAccountId":"{moderator}","removalMode":"approve-hide","removalMode":"request-review"}}"#)),
        ("DELETE",&path,format!(r#"{{"version":{version},"expectedAccountId":"{moderator}","removalMode":"request-review","removal\u004dode":"approve-hide"}}"#)),
    ] {
        let body=body.into_bytes();
        let authorization=proof(8,11,&format!("{}{target}",f.origin),method,&body);
        f.raw(method,target,body,"application/json",Some(authorization),400).await;
        assert_eq!(db.query_one(unchanged_sql,&[&native]).await.unwrap().get::<_,Value>(0),before,"Rejected {method} {target} changed disposition state");
    }
    assert_eq!(object_files(&f.state.config.uploads_dir.join("objects")),object_count);
    f.call(None,"POST",&lock,Some(json!({"version":version})),401).await;
    f.call(Some(1),"POST",&lock,Some(json!({"version":version})),403).await;
    f.call(Some(8),"GET",&lock,None,405).await;
    let bytes=serde_json::to_vec(&json!({"version":version,"expectedAccountId":moderator})).unwrap();
    f.raw("POST",&lock,bytes.clone(),"application/json",Some(proof(8,11,&format!("{}{lock}",f.origin),"DELETE",&bytes)),401).await;
    db.execute("UPDATE platform.users SET permissions=permissions-'manga.lock.update' WHERE id='PublicUploadUser8'",&[]).await.unwrap();
    f.call(Some(8),"POST",&lock,Some(json!({"version":version})),403).await;
    db.execute("UPDATE platform.users SET permissions=permissions||'[\"manga.lock.update\"]'::jsonb WHERE id='PublicUploadUser8'",&[]).await.unwrap();
    let locked=f.call(Some(8),"POST",&lock,Some(json!({"version":version})),200).await;
    assert_eq!(locked["data"]["attributes"]["version"],version+1);assert_eq!(locked["data"]["attributes"]["isLocked"],true);assert_eq!(locked["data"]["attributes"]["nostr"]["eventId"],source_id);assert_eq!(locked["data"]["attributes"]["nostr"]["metadataScope"],"instance-local");assert_eq!(locked["data"]["attributes"]["nostr"]["originalEventRewritten"],false);
    f.call(Some(8),"POST",&unlock,Some(json!({"version":version})),409).await;
    f.call(Some(1),"PUT",&path,Some(json!({"version":version+1,"title":{"en":"Locked edit"}})),403).await;
    let unlocked=f.call(Some(8),"POST",&unlock,Some(json!({"version":version+1})),200).await;assert_eq!(unlocked["data"]["attributes"]["isLocked"],false);assert_eq!(unlocked["data"]["attributes"]["version"],version+2);
    let version=version+2;
    // A newly bound, equally granted moderator must not replace signed account intent.
    db.execute("UPDATE platform.users SET roles='[\"moderator\"]',permissions=(SELECT permissions FROM platform.users WHERE id='PublicUploadUser8') WHERE id='PublicUploadUser2'",&[]).await.unwrap();
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser2' WHERE publisher=$1",&[&hex::encode(id(8))]).await.unwrap();
    f.call(Some(8),"POST",&lock,Some(json!({"version":version})),409).await;
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser8' WHERE publisher=$1",&[&hex::encode(id(8))]).await.unwrap();
    db.execute("UPDATE platform.users SET roles='[\"user\"]',permissions='[\"manga.create\",\"manga.update\"]' WHERE id='PublicUploadUser2'",&[]).await.unwrap();
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser8' WHERE publisher=$1",&[&hex::encode(id(1))]).await.unwrap();
    f.call(Some(1),"DELETE",&path,Some(json!({"version":version})),409).await;
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser1' WHERE publisher=$1",&[&hex::encode(id(1))]).await.unwrap();
    f.call(Some(2),"DELETE",&path,Some(json!({"version":version})),403).await;
    f.call(Some(8),"DELETE",&path,Some(json!({"version":version-1})),409).await;
    // Saved UUIDs and staff grants cannot bypass withdrawn source visibility.
    db.execute("UPDATE platform.catalog_manga SET protocol_source_visible=FALSE WHERE id=$1",&[&native]).await.unwrap();
    f.call(Some(1),"DELETE",&path,Some(json!({"version":version})),404).await;
    f.call(Some(8),"POST",&lock,Some(json!({"version":version})),404).await;
    db.execute("UPDATE platform.catalog_manga SET protocol_source_visible=TRUE WHERE id=$1",&[&native]).await.unwrap();
    let state:String=db.query_one("SELECT state FROM platform.protocol_publication_commands WHERE catalog_id=$1 AND resource_kind='manga'",&[&native]).await.unwrap().get(0);
    db.execute("UPDATE platform.protocol_publication_commands SET state='prepared' WHERE catalog_id=$1 AND resource_kind='manga'",&[&native]).await.unwrap();
    f.call(Some(1),"DELETE",&path,Some(json!({"version":version})),409).await;
    f.call(Some(8),"POST",&lock,Some(json!({"version":version})),409).await;
    db.execute("UPDATE platform.protocol_publication_commands SET state=$2 WHERE catalog_id=$1 AND resource_kind='manga'",&[&native,&state]).await.unwrap();
    let reason=resolver.preferred(ResourceKind::ReportReason,"manga_removal_request").await.unwrap().unwrap().to_string();
    let reasons=f.call(Some(1),"GET","/api/v1/report/reasons/manga",None,200).await;assert!(!reasons["data"].as_array().unwrap().iter().any(|r|r["id"]==reason));
    f.call(Some(1),"POST","/api/v1/report",Some(json!({"category":"manga","reason":reason,"objectId":public})),400).await;
    f.call(Some(1),"DELETE",&path,Some(json!({"version":version,"removalMode":"approve-hide"})),403).await;
    let missing_mode=serde_json::to_vec(&json!({"version":version,"expectedAccountId":owner})).unwrap();
    f.raw("DELETE",&path,missing_mode.clone(),"application/json",Some(proof(1,11,&format!("{}{path}",f.origin),"DELETE",&missing_mode)),400).await;
    // Quota refusal is before any local request creation or source side effect.
    db.execute("INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,status,target_version) SELECT 'TtlQ'||lpad(n::text,6,'0'),'PublicUploadUser1','manga',$1,'manga_removal_request','refused',$2 FROM generate_series(1,20) AS g(n)",&[&native,&version]).await.unwrap();
    f.call(Some(1),"DELETE",&path,Some(json!({"version":version})),429).await;
    assert_eq!(db.query_one("SELECT count(*) FROM platform.reports WHERE entity_type='manga' AND entity_id=$1 AND status='waiting'",&[&native]).await.unwrap().get::<_,i64>(0),0);
    db.execute("UPDATE platform.reports SET created_at=clock_timestamp()-interval '2 hours' WHERE id LIKE 'TtlQ%'",&[]).await.unwrap();
    let body=serde_json::to_vec(&json!({"version":version,"expectedAccountId":owner,"removalMode":"request-review"})).unwrap();
    // Promotion after the request's review-only effect was captured must not
    // silently turn that same signed intent into an approval/hide.
    db.execute("UPDATE platform.users SET roles='[\"moderator\"]',permissions=permissions||'[\"manga.delete\"]'::jsonb WHERE id='PublicUploadUser1'",&[]).await.unwrap();
    let token=proof(1,11,&format!("{}{path}",f.origin),"DELETE",&body);
    let requested=f.raw("DELETE",&path,body.clone(),"application/json",Some(token.clone()),202).await;
    f.raw("DELETE",&path,body,"application/json",Some(token),401).await;
    assert_eq!(requested["targetVersion"],version);assert_eq!(requested["sourceEventId"],source_id);assert_eq!(requested["sourceDeletionStatus"],"not-requested");assert_eq!(requested["originalEventRewritten"],false);
    let duplicate=f.call(Some(1),"DELETE",&path,Some(json!({"version":version})),202).await;assert_eq!(duplicate["requestId"],requested["requestId"]);
    db.execute("UPDATE platform.users SET roles='[\"user\"]',permissions=permissions-'manga.delete' WHERE id='PublicUploadUser1'",&[]).await.unwrap();
    let waiting=db.query_one("SELECT deleted_at IS NULL,version FROM platform.catalog_manga WHERE id=$1",&[&native]).await.unwrap();assert!(waiting.get::<_,bool>(0));assert_eq!(waiting.get::<_,i32>(1),version);
    let report=f.native(ResourceKind::Report,requested["requestId"].as_str().unwrap()).await;
    assert_eq!(f.http.get(format!("{}{path}",f.origin)).send().await.unwrap().status().as_u16(),200);
    db.execute("UPDATE platform.users SET permissions=permissions-'manga.delete' WHERE id='PublicUploadUser8'",&[]).await.unwrap();
    let accept=format!("{}/api/report/{report}/accept",f.origin);
    let refused=f.http.post(&accept).header("origin",&f.origin).header("cookie","weebdex_session=synthetic-title-reviewer-cookie").json(&json!({"version":1})).send().await.unwrap();assert_eq!(refused.status().as_u16(),403);
    db.execute("UPDATE platform.users SET permissions=permissions||'[\"manga.delete\"]'::jsonb WHERE id='PublicUploadUser8'",&[]).await.unwrap();
    // A target change makes acceptance stale; rejection remains possible.
    f.call(Some(8),"POST",&lock,Some(json!({"version":version})),200).await;
    let stale=f.http.post(&accept).header("origin",&f.origin).header("cookie","weebdex_session=synthetic-title-reviewer-cookie").json(&json!({"version":1})).send().await.unwrap();assert_eq!(stale.status().as_u16(),409);
    f.call(Some(1),"DELETE",&path,Some(json!({"version":version+1})),409).await;
    let rejected=f.http.post(format!("{}/api/report/{report}/reject",f.origin)).header("origin",&f.origin).header("cookie","weebdex_session=synthetic-title-reviewer-cookie").json(&json!({"version":1,"notes":"Review current target"})).send().await.unwrap();assert_eq!(rejected.status().as_u16(),200);
    let requested=f.call(Some(1),"DELETE",&path,Some(json!({"version":version+1})),202).await;let report=f.native(ResourceKind::Report,requested["requestId"].as_str().unwrap()).await;
    // Failure after local hide but before the report/audit commit is atomic.
    db.batch_execute("CREATE FUNCTION platform.title_review_fixture_reject() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.operation='accept' THEN RAISE EXCEPTION 'owned title review rollback'; END IF; RETURN NEW; END $$; CREATE TRIGGER title_review_fixture_reject BEFORE INSERT ON platform.report_audit FOR EACH ROW EXECUTE FUNCTION platform.title_review_fixture_reject();").await.unwrap();
    let rollback=f.http.post(format!("{}/api/report/{report}/accept",f.origin)).header("origin",&f.origin).header("cookie","weebdex_session=synthetic-title-reviewer-cookie").json(&json!({"version":1})).send().await.unwrap();assert_eq!(rollback.status().as_u16(),503);
    assert!(db.query_one("SELECT deleted_at IS NULL FROM platform.catalog_manga WHERE id=$1",&[&native]).await.unwrap().get::<_,bool>(0));assert_eq!(db.query_one("SELECT status FROM platform.reports WHERE id=$1",&[&report]).await.unwrap().get::<_,String>(0),"waiting");
    db.batch_execute("DROP TRIGGER title_review_fixture_reject ON platform.report_audit;DROP FUNCTION platform.title_review_fixture_reject();").await.unwrap();
    let accepted=f.http.post(format!("{}/api/report/{report}/accept",f.origin)).header("origin",&f.origin).header("cookie","weebdex_session=synthetic-title-reviewer-cookie").json(&json!({"version":1})).send().await.unwrap();assert_eq!(accepted.status().as_u16(),200);
    assert!(db.query_one("SELECT deleted_at IS NOT NULL FROM platform.catalog_manga WHERE id=$1",&[&native]).await.unwrap().get::<_,bool>(0));
    assert_eq!(db.query_one("SELECT status FROM platform.reports WHERE id=$1",&[&report]).await.unwrap().get::<_,String>(0),"accepted");
    assert_eq!(f.http.get(format!("{}{path}",f.origin)).send().await.unwrap().status().as_u16(),404);
    f.call(Some(1),"GET",&format!("/api/v1/nostr/manga/{public}/covers"),None,404).await;
    let repeat=f.http.post(format!("{}/api/report/{report}/accept",f.origin)).header("origin",&f.origin).header("cookie","weebdex_session=synthetic-title-reviewer-cookie").json(&json!({"version":1})).send().await.unwrap();assert_eq!(repeat.status().as_u16(),409);
    // Ordinary draft removal is review-first too; an explicit staff DELETE may
    // immediately soft-hide and close its pending review using current CAS.
    let draft=f.call(Some(1),"POST","/api/v1/manga",Some(json!({"title":{"en":"Disposition draft"},"originalLanguage":"en","status":"ongoing","contentRating":"safe","authors":[],"artists":[]})),200).await;
    let draft=draft["data"]["id"].as_str().unwrap();let draft_path=format!("/api/v1/manga/{draft}");
    let pending=f.call(Some(1),"DELETE",&draft_path,Some(json!({"version":1})),202).await;assert_eq!(pending["sourceEventId"],Value::Null);
    let bytes=serde_json::to_vec(&json!({"version":1,"expectedAccountId":moderator,"removalMode":"approve-hide"})).unwrap();
    let hidden=f.http.delete(format!("{}{draft_path}",f.origin)).header("origin",&f.origin).header("content-type","application/json").header("authorization",proof(8,11,&format!("{}{draft_path}",f.origin),"DELETE",&bytes)).body(bytes).send().await.unwrap();assert_eq!(hidden.status().as_u16(),204);assert_eq!(hidden.headers()["x-weebdex-instance-removal"],"soft-hidden");assert_eq!(hidden.headers()["x-weebdex-source-deletion-status"],"not-requested");assert_eq!(hidden.headers()["cache-control"],"private, no-store");
    let draft_native=f.native(ResourceKind::Manga,draft).await;let report=f.native(ResourceKind::Report,pending["requestId"].as_str().unwrap()).await;
    assert!(db.query_one("SELECT deleted_at IS NOT NULL FROM platform.catalog_manga WHERE id=$1",&[&draft_native]).await.unwrap().get::<_,bool>(0));assert_eq!(db.query_one("SELECT status FROM platform.reports WHERE id=$1",&[&report]).await.unwrap().get::<_,String>(0),"accepted");
    // Existing native cookie draft-owner deletion remains immediate soft hide.
    let legacy=f.call(Some(1),"POST","/api/v1/manga",Some(json!({"title":{"en":"Legacy native draft"},"originalLanguage":"en","status":"ongoing","contentRating":"safe","authors":[],"artists":[]})),200).await;
    let legacy=f.native(ResourceKind::Manga,legacy["data"]["id"].as_str().unwrap()).await;
    let hidden=f.http.delete(format!("{}/api/manga/{legacy}",f.origin)).header("origin",&f.origin).header("cookie","weebdex_session=synthetic-public-upload-cookie").send().await.unwrap();assert_eq!(hidden.status().as_u16(),204);
    assert!(db.query_one("SELECT deleted_at IS NOT NULL FROM platform.catalog_manga WHERE id=$1",&[&legacy]).await.unwrap().get::<_,bool>(0));
    assert_eq!(object_files(&f.state.config.uploads_dir.join("objects")),object_count);
    let retained=f.state.protocol.entry(source_key).await.unwrap().unwrap();assert_eq!(serde_json::to_value(&retained.event).unwrap(),serde_json::to_value(&original.event).unwrap());assert_eq!(retained.metadata,original.metadata);
    assert_eq!(db.query_one("SELECT count(*) FROM events WHERE kind=5",&[]).await.unwrap().get::<_,i64>(0),deletions);
}
