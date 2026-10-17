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
            .header("content-type",content_type);
        if let Some(value)=authorization { request=request.header("authorization",value); }
        let response=request.body(body).send().await.unwrap();
        let status=response.status().as_u16();
        let private_upload=path.split('?').next().unwrap().starts_with("/api/v1/nostr/uploads/")
            && ["/preview","/replace"].iter().any(|suffix|path.split('?').next().unwrap().ends_with(suffix));
        assert_eq!(response.headers()["cache-control"],if private_upload {"private, no-store"}else{"no-store"});
        let text=response.text().await.unwrap();
        let value:Value=serde_json::from_str(&text).unwrap_or_else(|_|panic!("{method} {path}: non-JSON {text}"));
        assert_eq!(status,expected,"{method} {path}: {value}");
        if status>=400 { assert_eq!(value["result"],"error","{value}"); assert_eq!(value["errors"][0]["status"],status,"{value}"); }
        value
    }
    async fn call(&self,key:Option<u8>,method:&str,path:&str,payload:Option<Value>,expected:u16)->Value {
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
    cover(&f,&native).await;
    // Native cover upload changes the title version. Read its real optimistic token.
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
    let published=f.call(Some(1),"POST",&publish,Some(json!({"version":v})),200).await;
    assert_eq!(published["data"]["attributes"]["state"],"published");assert_eq!(published["data"]["attributes"]["title"]["en"],"Edited title");
    assert_eq!(published["data"]["attributes"]["nostr"]["publicationScope"],"original-protocol");
    let event=published["data"]["attributes"]["nostr"]["eventId"].as_str().unwrap();let eid:[u8;32]=hex::decode(event).unwrap().try_into().unwrap();
    tokio::time::timeout(Duration::from_secs(20),async{loop{if f.state.protocol.entry(eid).await.unwrap().is_some(){break;}tokio::time::sleep(Duration::from_millis(30)).await;}}).await.unwrap();
    for _ in 0..8 {let r=bridge::import_page(&f.state).await.unwrap();assert_eq!(r.rejected,0);if r.completed_sweep && r.deferred==0 && r.scanned>0 {break;}}
    let source=f.state.protocol.entry(eid).await.unwrap().unwrap();source.event.verify().unwrap();assert_eq!(source.event.pubkey,public(1));assert_eq!(source.event.kind,1002);
    let source_metadata=&source.metadata;assert_eq!(source_metadata["titles"]["ja"],"編集した題");assert_eq!(source_metadata["titles"]["en"],"Edited title");assert_eq!(source_metadata["description"]["ja"],"編集した説明");assert_eq!(source_metadata["description"]["en"],"Edited description");assert_eq!(source_metadata["altTitles"][0],input["altTitles"][0]);
    assert!(source_metadata["cover"]["sha256"].is_string());
    let public_read:Value=f.http.get(format!("{}{path}",f.origin)).send().await.unwrap().json().await.unwrap();assert_eq!(public_read["data"]["id"],manga);assert_eq!(public_read["data"]["attributes"]["nostr"]["publicationStatus"],"locally_hosted");
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
    drop(db);let mut native=f.native.take().unwrap();if let Some(shutdown)=native.shutdown.take(){let _=shutdown.send(());}
    tokio::time::timeout(Duration::from_secs(30),native.task).await.unwrap().unwrap().unwrap();
}
