//! Real TCP, exact raw NIP-98 multipart proofs and original publication receipts.
//! All accounts, keys, PNG/JPEG bytes, paths and databases are owned test data.
use base64::{Engine,engine::general_purpose::STANDARD};
use image::{DynamicImage,ImageFormat,Rgb,RgbImage};
use manga_node::{auth,db::{DbConfig,Repository}};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use std::{io::Cursor,sync::Arc,time::Duration};
use weebdex_platform::{AppState,Config,domains,protocol::publication::OperatorRuntimeConfig,
    public_api::{PostgresPublicIdResolver,PublicIdResolver,PublicUuid,ResourceKind}};
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
    manga:String, native_manga:String, _server:Server, _storage:tempfile::TempDir,
}
impl Fixture {
    async fn raw(&self,method:&str,path:&str,body:Vec<u8>,content_type:&str,authorization:Option<String>,expected:u16)->Value {
        let mut request=self.http.request(method.parse().unwrap(),format!("{}{path}",self.origin))
            .header("content-type",content_type);
        if let Some(value)=authorization { request=request.header("authorization",value); }
        let response=request.body(body).send().await.unwrap();
        let status=response.status().as_u16();
        assert_eq!(response.headers()["cache-control"],"no-store");
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
    async fn begin(&self,key:u8)->String {
        let value=self.call(Some(key),"POST","/api/v1/upload/begin",Some(json!({"manga":self.manga,"groups":[]})),200).await;
        assert_eq!(value["type"],"upload_session"); assert!(value.get("result").is_none());
        assert_eq!(value["attributes"]["isCommitted"],false); assert_eq!(value["attributes"]["version"],1);
        let session=value["id"].as_str().unwrap().to_owned();PublicUuid::parse(&session).unwrap();session
    }
    async fn multipart(&self,key:u8,session:&str,parts:&[(&str,&str,&[u8])],expected:u16)->Value {
        let (content_type,body)=multipart(parts);
        let path=format!("/api/v1/upload/{session}");
        let token=proof(key,11,&format!("{}{path}",self.origin),"POST",&body);
        self.raw("POST",&path,body,&content_type,Some(token),expected).await
    }
    async fn native(&self,kind:ResourceKind,public:&str)->String {
        PostgresPublicIdResolver::new(self.state.db.clone()).resolve(kind,&PublicUuid::parse(public).unwrap())
            .await.unwrap().unwrap().native_id
    }
}
async fn fixture()->Fixture {
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
    let publication=OperatorRuntimeConfig {
        operator_key_file:signer,expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),
        storage:json!({"path":storage.path().join("protocol-originals"),"volume":"PublicUploadFixture","physical_bytes":64*1024*1024,
            "staging_bytes":64*1024*1024,"max_blob_bytes":8*1024*1024,"upload_slots":2}),group_keys_dir:None,backend:None,
        native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":service,"admin_pubkey":public(1),
            "database_plaintext_local":true,"read_policy":"public","sync_worker_enabled":false})) };
    let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:6,
        uploads_dir:storage.path().join("native-images"),protocol_publication:Some(publication.clone()),..Config::default()},dsn.clone()).await.unwrap();
    let mut repository=Repository::connect(DbConfig{dsn,connections:1,acquisition_timeout:Duration::from_secs(2),cache_weight:1048576,plaintext_local:true}).await.unwrap();
    repository.pool=state.db.pool().clone();repository.migrate().await.unwrap();repository.bind_instance(id(11),id(1)).await.unwrap();
    for key in 1..=8 {repository.set_account_admission(id(key),true).await.unwrap();}
    repository.set_publisher_admission(id(1),true).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();state.protocol.verify_schema().await.unwrap();
    state.protocol.install_visibility_function().await.unwrap();state.protocol.configure_publication(publication).await.unwrap();
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
    db.batch_execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES('UploadManga','PublicUploadUser1','Synthetic uploader','Public upload fixture','en','ongoing','safe','published')").await.unwrap();
    let native_manga="UploadManga".to_owned();
    let manga=PostgresPublicIdResolver::new(state.db.clone()).preferred(ResourceKind::Manga,&native_manga).await.unwrap().unwrap().to_string();
    drop(db);
    let app=weebdex_platform::router::router(state.clone());
    let server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap()}));
    Fixture{state,repository,http:reqwest::Client::new(),origin,manga,native_manga,_server:server,_storage:storage}
}
fn png()->Vec<u8> {
    let mut bytes=Cursor::new(Vec::new());DynamicImage::ImageRgb8(RgbImage::from_pixel(13,17,Rgb([91,22,171])))
        .write_to(&mut bytes,ImageFormat::Png).unwrap();bytes.into_inner()
}
fn jpeg()->Vec<u8> {
    let mut bytes=Cursor::new(Vec::new());DynamicImage::ImageRgb8(RgbImage::from_pixel(19,11,Rgb([111,62,27])))
        .write_to(&mut bytes,ImageFormat::Jpeg).unwrap();bytes.into_inner()
}
fn multipart(parts:&[(&str,&str,&[u8])])->(String,Vec<u8>) {
    let boundary="owned-public-upload-fixture-2026";let mut bytes=Vec::new();
    for (filename,mime,data) in parts {
        bytes.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: {mime}\r\n\r\n").as_bytes());
        bytes.extend_from_slice(data);bytes.extend_from_slice(b"\r\n");
    }
    bytes.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());(format!("multipart/form-data; boundary={boundary}"),bytes)
}
fn commit(files:&[Value])->Value {json!({"chapterDraft":{"volume":null,"chapter":"1","title":"Original upload fixture","translatedLanguage":"en"},
    "pageOrder":files.iter().map(|f|f["id"].clone()).collect::<Vec<_>>(),"termsAccepted":true})}

#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires owned disposable PostgreSQL runner"]
async fn exact_nostr_upload_contracts_ownership_validation_and_original_commit() {
    let f=fixture().await;
    f.call(None,"GET","/api/v1/upload",None,401).await;
    f.call(Some(1),"GET","/api/v1/upload",None,404).await;
    for key in [3,5,6,7] { f.call(Some(key),"POST","/api/v1/upload/begin",Some(json!({"manga":f.manga,"groups":[]})),403).await; }
    f.call(Some(4),"POST","/api/v1/upload/begin",Some(json!({"manga":f.manga,"groups":[]})),403).await;
    let cookie=f.http.get(format!("{}/api/v1/upload",f.origin)).header("cookie","weebdex_session=synthetic-public-upload-cookie").send().await.unwrap();assert_eq!(cookie.status().as_u16(),401);
    f.call(Some(1),"POST","/api/v1/upload/begin",Some(json!({"manga":f.manga})),400).await;
    f.call(Some(1),"POST","/api/v1/upload/check-approval-required",Some(json!({"manga":f.manga,"locale":"en"})),200).await;
    f.call(Some(1),"POST","/api/v1/upload/check-approval-required",Some(json!({"manga":f.manga,"locale":"bad_locale"})),400).await;
    let session=f.begin(1).await;
    let native_session=f.native(ResourceKind::UploadSession,&session).await;assert_eq!(native_session.len(),10);
    assert_eq!(f.call(Some(1),"GET","/api/v1/upload",None,200).await["id"],session);
    f.call(Some(1),"POST","/api/v1/upload/begin",Some(json!({"manga":f.manga,"groups":[]})),409).await;
    let upload_path=format!("/api/v1/upload/{session}");
    let image=png();let (ct,body)=multipart(&[("signed.png","image/png",&image)]);
    let token=proof(1,11,&format!("{}{upload_path}",f.origin),"POST",&body);
    let mut altered=body.clone();let n=altered.iter().position(|n|*n==91).unwrap_or(10);altered[n]^=1;
    f.raw("POST",&upload_path,altered,&ct,Some(token.clone()),401).await;
    f.raw("POST",&upload_path,body.clone(),&ct,Some(proof(1,12,&format!("{}{upload_path}",f.origin),"POST",&body)),401).await;
    let uploaded=f.raw("POST",&upload_path,body.clone(),&ct,Some(token.clone()),200).await;
    assert_eq!(uploaded["result"],"ok");assert_eq!(uploaded["data"][0]["type"],"upload_session_file");
    assert_eq!(uploaded["data"][0]["attributes"]["fileHash"],auth::hash(&image));
    assert_eq!(uploaded["data"][0]["attributes"]["fileSize"],image.len());
    assert_eq!(uploaded["data"][0]["attributes"]["mimeType"],"image/png");
    assert!(uploaded["data"][0]["attributes"].get("file_name").is_none());
    f.raw("POST",&upload_path,body,&ct,Some(token),401).await;
    let file=uploaded["data"][0].clone();let native_file=f.native(ResourceKind::UploadFile,file["id"].as_str().unwrap()).await;assert_eq!(native_file.len(),10);
    // Genuine field/body bytes are bound, not just decoded file bytes.
    let (ct,body)=multipart(&[("renamed.png","image/png",&image)]);
    let (_,original_body)=multipart(&[("signed.png","image/png",&image)]);
    f.raw("POST",&upload_path,body,&ct,Some(proof(1,11,&format!("{}{upload_path}",f.origin),"POST",&original_body)),401).await;
    f.multipart(2,&session,&[("foreign.png","image/png",&image)],404).await;
    let invalid=f.multipart(1,&session,&[("malicious.png","image/png",b"not an image")],200).await;
    assert_eq!(invalid["result"],"error");assert!(invalid["data"].as_array().unwrap().is_empty());assert!(!invalid["errors"].as_array().unwrap().is_empty());
    let other=f.begin(2).await;
    let other_files=f.multipart(2,&other,&[("other.jpg","image/jpeg",&jpeg())],200).await;
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}/{}",other_files["data"][0]["id"].as_str().unwrap()),None,404).await;
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}/batch"),Some(json!([file["id"],other_files["data"][0]["id"]])),404).await;
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.upload_files WHERE session_id=$1",&[&native_session]).await.unwrap().get::<_,i64>(0),1);
    f.call(Some(2),"DELETE",&format!("/api/v1/upload/{other}"),None,200).await;
    let jpg=jpeg();let second=f.multipart(1,&session,&[("actual.jpg","image/jpeg",&jpg)],200).await;
    let second_file=second["data"][0].clone();
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}/{}",second_file["id"].as_str().unwrap()),None,200).await;
    let third=f.multipart(1,&session,&[("actual.jpg","image/jpeg",&jpg)],200).await;
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}/batch"),Some(json!([third["data"][0]["id"]])),200).await;
    let mut rejected=commit(&[file.clone()]);rejected["termsAccepted"]=json!(false);
    f.call(Some(1),"POST",&format!("{upload_path}/commit"),Some(rejected),400).await;
    for (key,value) in [("externalUrl","https://fixture.invalid/chapter"),("publishAt","2030-01-01T00:00:00")] {
        let mut value_body=commit(&[file.clone()]);value_body["chapterDraft"][key]=json!(value);
        f.call(Some(1),"POST",&format!("{upload_path}/commit"),Some(value_body),400).await;
    }
    // A real account grant does not authorize using the operator's source signer.
    let blocked=f.begin(2).await;
    let blocked_files=f.multipart(2,&blocked,&[("blocked.png","image/png",&image)],200).await;
    f.call(Some(2),"POST",&format!("/api/v1/upload/{blocked}/commit"),Some(commit(blocked_files["data"].as_array().unwrap())),403).await;
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.protocol_publication_commands WHERE user_id='PublicUploadUser2'",&[]).await.unwrap().get::<_,i64>(0),0);
    f.call(Some(2),"DELETE",&format!("/api/v1/upload/{blocked}"),None,200).await;
    let actual_files=f.multipart(1,&session,&[("second.jpg","image/jpeg",&jpg),("repeat.png","image/png",&image)],200).await;
    let mut files=vec![actual_files["data"][0].clone(),actual_files["data"][1].clone(),file.clone()];
    let payload=commit(&files);
    let receipt=f.call(Some(1),"POST",&format!("{upload_path}/commit"),Some(payload.clone()),200).await;
    assert_eq!(receipt["type"],"chapter");assert!(receipt.get("result").is_none());assert_eq!(receipt["attributes"]["pages"],3);
    assert_eq!(receipt["attributes"]["nostr"]["publicationStatus"],"accepted_pending_projection");assert_eq!(receipt["attributes"]["isUnavailable"],true);
    let event_id=receipt["attributes"]["nostr"]["eventId"].as_str().unwrap();
    let event:auth::Event=serde_json::from_value(f.state.db.client().await.unwrap().query_one("SELECT raw_event FROM public.events WHERE id=decode($1,'hex')",&[&event_id]).await.unwrap().get(0)).unwrap();
    event.verify().unwrap();assert_eq!(event.kind,1001);assert_eq!(event.pubkey,public(1));
    let native_chapter=f.native(ResourceKind::Chapter,receipt["id"].as_str().unwrap()).await;
    let pages=f.state.db.client().await.unwrap().query("SELECT p.name,o.digest,o.bytes,o.storage_key FROM platform.chapter_pages p JOIN platform.media_objects o ON o.id=p.object_id WHERE p.chapter_id=$1 ORDER BY p.position",&[&native_chapter]).await.unwrap();
    assert_eq!(pages.len(),3);
    for (row,expected) in pages.iter().zip([&jpg,&image,&image]) {
        assert_eq!(row.get::<_,String>(0),auth::hash(expected));assert_eq!(row.get::<_,String>(1),auth::hash(expected));
        let bytes=tokio::fs::read(weebdex_platform::domains::uploads::storage::object_path(&f.state.config.uploads_dir,&row.get::<_,String>(3)).unwrap()).await.unwrap();assert_eq!(&bytes,expected);
    }
    let retry=f.call(Some(1),"POST",&format!("{upload_path}/commit"),Some(payload.clone()),200).await;assert_eq!(retry["id"],receipt["id"]);assert_eq!(retry["attributes"]["nostr"],receipt["attributes"]["nostr"]);
    files.swap(0,1);f.call(Some(1),"POST",&format!("{upload_path}/commit"),Some(commit(&files)),409).await;
    f.call(Some(2),"POST",&format!("{upload_path}/commit"),Some(payload),404).await;
    f.call(Some(1),"DELETE",&upload_path,None,409).await;
    f.call(None,"GET",&format!("/api/v1/chapter/{}",receipt["id"].as_str().unwrap()),None,404).await;
    assert_eq!(std::fs::read_dir(f.state.config.uploads_dir.join("temporary")).unwrap().count(),0);
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.upload_files WHERE session_id=$1",&[&native_session]).await.unwrap().get::<_,i64>(0),0);
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.protocol_identity_bindings WHERE publisher=$1",&[&public(3)]).await.unwrap().get::<_,i64>(0),0);
}
