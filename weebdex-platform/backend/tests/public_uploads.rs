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
    manga:String, native_manga:String, _server:Server, _storage:tempfile::TempDir,
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
    async fn replace(&self,key:u8,session:&str,file:&str,version:i32,name:&str,mime:&str,image:&[u8],expected:u16)->Value {
        let version=version.to_string();
        let (content_type,body)=replacement_multipart(&[
            ("expectedVersion",None,None,version.as_bytes()),("file",Some(name),Some(mime),image),
        ]);
        let path=format!("/api/v1/nostr/uploads/{session}/files/{file}/replace");
        let token=proof(key,11,&format!("{}{path}",self.origin),"POST",&body);
        self.raw("POST",&path,body,&content_type,Some(token),expected).await
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
    db.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES($1,'PublicUploadUser1','Synthetic uploader','Public upload fixture','en','ongoing','safe','published')",&[&FIXTURE_MANGA_ID]).await.unwrap();
    let native_manga=FIXTURE_MANGA_ID.to_owned();
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
fn replacement_png()->Vec<u8> {
    let mut seed=0x731ae53cu32;
    let image=RgbImage::from_fn(640,640,|_,_|{
        seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;
        Rgb([(seed>>16)as u8,(seed>>8)as u8,seed as u8])
    });
    let mut bytes=Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image).write_to(&mut bytes,ImageFormat::Png).unwrap();
    let bytes=bytes.into_inner();assert!(bytes.len()>1024*1024);bytes
}
fn multipart(parts:&[(&str,&str,&[u8])])->(String,Vec<u8>) {
    let boundary="owned-public-upload-fixture-2026";let mut bytes=Vec::new();
    for (filename,mime,data) in parts {
        bytes.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: {mime}\r\n\r\n").as_bytes());
        bytes.extend_from_slice(data);bytes.extend_from_slice(b"\r\n");
    }
    bytes.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());(format!("multipart/form-data; boundary={boundary}"),bytes)
}
fn replacement_multipart(parts:&[(&str,Option<&str>,Option<&str>,&[u8])])->(String,Vec<u8>) {
    let boundary="owned-signed-replacement-fixture";let mut bytes=Vec::new();
    for (name,filename,mime,data) in parts {
        bytes.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"").as_bytes());
        if let Some(filename)=filename {bytes.extend_from_slice(format!("; filename=\"{filename}\"").as_bytes());}
        bytes.extend_from_slice(b"\r\n");
        if let Some(mime)=mime {bytes.extend_from_slice(format!("Content-Type: {mime}\r\n").as_bytes());}
        bytes.extend_from_slice(b"\r\n");bytes.extend_from_slice(data);bytes.extend_from_slice(b"\r\n");
    }
    bytes.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());(format!("multipart/form-data; boundary={boundary}"),bytes)
}
fn commit(files:&[Value])->Value {json!({"chapterDraft":{"volume":null,"chapter":"1","title":"Original upload fixture","translatedLanguage":"en"},
    "pageOrder":files.iter().map(|f|f["id"].clone()).collect::<Vec<_>>(),"termsAccepted":true})}

async fn signed_preview(f:&Fixture,key:u8,path:&str,expected:&[u8]) {
    let response=f.http.get(format!("{}{path}",f.origin))
        .header("authorization",proof(key,11,&format!("{}{path}",f.origin),"GET",b""))
        .send().await.unwrap();
    assert_eq!(response.status().as_u16(),200);
    assert_eq!(response.headers()["cache-control"],"private, no-store");
    assert!(response.headers().get_all("vary").iter().any(|v|v.to_str().unwrap().split(',').any(|s|s.trim().eq_ignore_ascii_case("authorization"))));
    assert_eq!(response.headers()["x-content-type-options"],"nosniff");
    assert_eq!(response.bytes().await.unwrap().as_ref(),expected);
}

// This shares the single owned fixture/database with the original transport
// test. It does not seed a second conflicting account or run any external node.
async fn signed_recovery_contracts(f:&Fixture,session:&str,file:&Value,foreign_file:&Value,image:&[u8]) {
    let account="/api/v1/nostr/account-context";
    f.call(None,"GET",account,None,401).await;
    let admission=f.call(Some(3),"GET","/api/v1/auth/check",None,200).await;
    assert_eq!(admission["isAuthenticated"],true);
    f.call(Some(3),"GET",account,None,403).await;
    f.state.db.client().await.unwrap().execute(
        "UPDATE platform.users SET version=7 WHERE id='PublicUploadUser1'",&[],
    ).await.unwrap();
    let context=f.call(Some(1),"GET",account,None,200).await;
    let user=context["user"]["id"].as_str().unwrap();
    PublicUuid::parse(user).unwrap();
    assert_eq!(f.native(ResourceKind::User,user).await,"PublicUploadUser1");
    assert_eq!(context["user"]["username"],"PublicUploadUser1");
    assert_eq!(context["user"]["version"],7);
    assert_eq!(context["user"]["roles"],json!(["user"]));
    assert!(context["user"]["permissions"].as_array().unwrap().contains(&json!("self.chapter.update")));
    PublicUuid::parse(context["nostr"]["catalogNamespace"].as_str().unwrap()).unwrap();
    assert_eq!(context["nostr"]["pubkey"],public(1));
    assert_eq!(context["nostr"]["serviceKey"],public(11));
    assert_eq!(context["nostr"]["nativeSessionAuthority"],false);
    assert_eq!(context["upload"]["conditional"],true);
    assert_eq!(context["upload"]["publication"]["requiresOperatorOrAuthorizedGroup"],true);
    let no_grants=f.call(Some(4),"GET",account,None,200).await;
    assert_eq!(no_grants["upload"]["canBegin"],false);
    assert_eq!(no_grants["upload"]["canManage"],false);
    for key in [5,6,7] {f.call(Some(key),"GET",account,None,403).await;}
    f.call(Some(1),"GET",&format!("{account}?extra=1"),None,400).await;
    f.call(Some(1),"GET",account,Some(json!({})),400).await;
    f.raw("GET",account,vec![],"application/json",Some(proof(1,12,&format!("{}{account}",f.origin),"GET",b"")),401).await;
    f.raw("GET",account,vec![],"application/json",Some(proof(1,11,&format!("{}{account}",f.origin),"POST",b"")),401).await;
    let token=proof(1,11,&format!("{}{account}",f.origin),"GET",b"");
    f.raw("GET",account,vec![],"application/json",Some(token.clone()),200).await;
    f.raw("GET",account,vec![],"application/json",Some(token),401).await;
    let cookie=f.http.get(format!("{}{account}",f.origin)).header("cookie","weebdex_session=synthetic-public-upload-cookie").send().await.unwrap();
    assert_eq!(cookie.status().as_u16(),401);

    let path=format!("/api/v1/nostr/uploads/{session}");
    f.call(Some(2),"GET",&path,None,404).await;
    f.call(Some(1),"GET",&format!("{path}?version=1"),None,400).await;
    let snapshot=f.call(Some(1),"GET",&path,None,200).await;
    let s=&snapshot["session"];
    assert_eq!(s["id"],session);assert_eq!(s["mangaId"],f.manga);
    assert_eq!(s["state"],"open");assert_eq!(s["canMutate"],true);
    let actual=f.state.db.client().await.unwrap().query_one(
        "SELECT platform.catalog_timestamp(s.created_at),platform.catalog_timestamp(s.updated_at) FROM platform.upload_sessions s JOIN platform.public_resource_ids a ON a.kind='upload_session' AND a.native_id=s.id WHERE a.public_id::text=$1 AND s.owner_id='PublicUploadUser1'",&[&session],
    ).await.unwrap();
    assert_eq!(s["createdAt"],actual.get::<_,String>(0));
    assert_eq!(s["updatedAt"],actual.get::<_,String>(1));
    assert_eq!(s["isProcessed"],true);assert_eq!(s["isDeleted"],false);
    assert_eq!(s["receiptStatus"],"none");
    assert_eq!(s["pageOrder"],json!([file["id"]]));
    assert_eq!(s["files"][0]["dimensions"],json!({"width":13,"height":17}));
    assert_eq!(s["files"][0]["fileSize"],image.len());
    assert_eq!(s["files"][0]["fileHash"],auth::hash(image));
    assert!(snapshot.get("relationships").is_none());
    let preview=s["files"][0]["previewUrl"].as_str().unwrap().to_owned();
    signed_preview(f,1,&preview,image).await;
    f.call(None,"GET",&preview,None,401).await;
    f.call(Some(2),"GET",&preview,None,404).await;
    f.call(Some(1),"GET",&format!("{preview}?x=1"),None,400).await;
    f.call(Some(1),"GET",&preview,Some(json!({})),400).await;
    let wrong_file=format!("{path}/files/{}/preview",foreign_file["id"].as_str().unwrap());
    f.call(Some(1),"GET",&wrong_file,None,404).await;
    let range=f.http.get(format!("{}{preview}",f.origin)).header("range","bytes=0-7")
        .header("authorization",proof(1,11,&format!("{}{preview}",f.origin),"GET",b""))
        .send().await.unwrap();
    assert_eq!(range.status().as_u16(),206);
    assert_eq!(range.headers()["cache-control"],"private, no-store");
    assert_eq!(range.bytes().await.unwrap().as_ref(),&image[..8]);

    let version=s["version"].as_i64().unwrap();
    let payload=json!({"version":version,"draft":{"title":"Private recovery draft","volume":"","chapter":"1","language":"en"},"pageOrder":[file["id"]]});
    let saved=f.call(Some(1),"PATCH",&path,Some(payload.clone()),200).await;
    assert_eq!(saved["session"]["version"],version+1);
    assert_eq!(saved["session"]["draft"],payload["draft"]);
    assert_eq!(saved["session"]["pageOrder"],payload["pageOrder"]);
    f.call(Some(1),"PATCH",&path,Some(payload.clone()),409).await;
    let mut current=payload;current["version"]=json!(version+1);
    f.call(Some(2),"PATCH",&path,Some(current.clone()),404).await;
    for order in [json!([]),json!([file["id"],file["id"]]),json!([foreign_file["id"]])] {
        let mut invalid=current.clone();invalid["pageOrder"]=order;
        f.call(Some(1),"PATCH",&path,Some(invalid),400).await;
    }
    for draft in [Value::Null,json!({"language":"invalid_locale"}),json!({"title":"nul\0title"}),json!({"title":"x".repeat(501)}),json!({"owner_id":"other"})] {
        let mut invalid=current.clone();invalid["draft"]=draft;
        f.call(Some(1),"PATCH",&path,Some(invalid),400).await;
    }
    let mut unknown=current.clone();unknown["commit"]=json!(true);
    f.call(Some(1),"PATCH",&path,Some(unknown),400).await;
    assert_eq!(f.call(Some(1),"GET",&path,None,200).await["session"]["version"],version+1);
    let mut oversized_commit=commit(&[file.clone()]);
    oversized_commit["pageOrder"]=json!(vec![file["id"].clone();500]);
    let invalid_order=f.call(Some(1),"POST",&format!("/api/v1/upload/{session}/commit"),Some(oversized_commit),400).await;
    assert_eq!(invalid_order["errors"][0]["detail"],"Page order must contain every session file exactly once");

    // A live native binding is required on every request, even with a valid
    // original admitted proof. Removing native grants only removes write power.
    let native=f.native(ResourceKind::UploadSession,session).await;
    let native_file=f.native(ResourceKind::UploadFile,file["id"].as_str().unwrap()).await;
    let db=f.state.db.client().await.unwrap();
    db.execute("DELETE FROM platform.protocol_identity_bindings WHERE publisher=$1",&[&public(1)]).await.unwrap();
    f.call(Some(1),"GET",&path,None,403).await;
    f.call(Some(1),"GET",&preview,None,403).await;
    f.call(Some(1),"PATCH",&path,Some(current.clone()),403).await;
    db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,'PublicUploadUser1')",&[&public(1)]).await.unwrap();
    db.execute("UPDATE platform.users SET permissions='[]' WHERE id='PublicUploadUser1'",&[]).await.unwrap();
    let revoked=f.call(Some(1),"GET",&path,None,200).await;
    for flag in ["canMutate","canCancel","canRetryCommit"] {assert_eq!(revoked["session"][flag],false);}
    signed_preview(f,1,&preview,image).await;
    f.call(Some(1),"PATCH",&path,Some(current.clone()),403).await;
    f.call(Some(1),"POST",&format!("/api/v1/upload/{session}/commit"),Some(commit(&[file.clone()])),403).await;
    db.execute("UPDATE platform.users SET permissions='[\"chapter.create\",\"self.chapter.update\",\"self.chapter.remove\"]' WHERE id='PublicUploadUser1'",&[]).await.unwrap();
    f.repository.set_account_admission(id(1),false).await.unwrap();
    f.call(Some(1),"GET",&path,None,403).await;
    f.call(Some(1),"GET",&preview,None,403).await;
    f.repository.set_account_admission(id(1),true).await.unwrap();

    // Prepared inputs are immutable even while owner reads remain recoverable.
    db.execute("INSERT INTO platform.protocol_publication_commands(command_id,user_id,input_hash,resource_kind,catalog_id,upload_session_id,created_at,state) VALUES('public-upload-owned-prepared','PublicUploadUser1',$1,'chapter',$2,$3,1,'prepared')",&[&"11".repeat(32),&native_file,&native]).await.unwrap();
    let prepared=f.call(Some(1),"GET",&path,None,200).await;
    assert_eq!(prepared["session"]["canMutate"],false);
    assert_eq!(prepared["session"]["canCancel"],false);
    assert_eq!(prepared["session"]["canRetryCommit"],true);
    f.call(Some(1),"PATCH",&path,Some(current.clone()),409).await;
    db.execute("UPDATE platform.upload_sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE id=$1",&[&native]).await.unwrap();
    let expired_prepared=f.call(Some(1),"GET",&path,None,200).await;
    assert_eq!(expired_prepared["session"]["isExpired"],true);
    assert_eq!(expired_prepared["session"]["canRetryCommit"],true);
    signed_preview(f,1,&preview,image).await;
    db.execute("DELETE FROM platform.protocol_publication_commands WHERE command_id='public-upload-owned-prepared'",&[]).await.unwrap();
    let expired=f.call(Some(1),"GET",&path,None,200).await;
    assert_eq!(expired["session"]["canRetryCommit"],false);
    f.call(Some(1),"PATCH",&path,Some(current),410).await;
    f.call(Some(1),"GET",&preview,None,404).await;
    db.execute("UPDATE platform.upload_sessions SET expires_at=clock_timestamp()+interval '1 hour' WHERE id=$1",&[&native]).await.unwrap();
}

async fn original_group_refusal_is_distinct_from_admission(f:&Fixture,image:&[u8]) {
    let db=f.state.db.client().await.unwrap();
    db.execute("INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES($1,'PublicUploadUser2','Synthetic uploader','Explicit unconfigured signing group')",&[&FIXTURE_GROUP_ID]).await.unwrap();
    db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,group_id) VALUES($1,$2)",&[&public(9),&FIXTURE_GROUP_ID]).await.unwrap();
    let group=PostgresPublicIdResolver::new(f.state.db.clone()).preferred(ResourceKind::Group,FIXTURE_GROUP_ID).await.unwrap().unwrap().to_string();
    drop(db);
    let admitted=f.call(Some(2),"GET","/api/v1/nostr/account-context",None,200).await;
    assert_eq!(admitted["upload"]["canBegin"],true);
    let begun=f.call(Some(2),"POST","/api/v1/upload/begin",Some(json!({"manga":f.manga,"groups":[group]})),200).await;
    let session=begun["id"].as_str().unwrap();
    let files=f.multipart(2,session,&[("group.png","image/png",image)],200).await;
    f.call(Some(2),"POST",&format!("/api/v1/upload/{session}/commit"),Some(commit(files["data"].as_array().unwrap())),403).await;
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.protocol_publication_commands WHERE user_id='PublicUploadUser2'",&[]).await.unwrap().get::<_,i64>(0),0);
    f.call(Some(2),"DELETE",&format!("/api/v1/upload/{session}"),None,200).await;
}

// Use the exact private production lock query without making a write capability
// publicly constructible. This is a real PostgreSQL lock seam, not an HTTP or
// NIP-98 proof test; the ordinary contract test exercises those boundaries.
fn account_write_sql()->&'static str {
    let source=include_str!("../src/public_api/auth.rs");
    let prefix="const BOUND_ACCOUNT_WRITE_SQL: &str = \"";
    assert_eq!(source.matches(prefix).count(),1);
    assert!(source.contains("let sql = if write {\n        BOUND_ACCOUNT_WRITE_SQL\n"));
    let tail=source.split_once(prefix).unwrap().1;
    let (sql,_)=tail.split_once("\";").unwrap();
    assert!(!sql.contains(['\n','\\','\"']));
    sql
}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires owned disposable PostgreSQL runner; exact-schema concurrent transaction seam"]
async fn account_write_lock_allows_publication_fk_and_excludes_authority_changes() {
    let f=fixture().await;
    let publisher=public(1);
    let mut owner=f.state.db.client().await.unwrap();
    let tx=owner.transaction().await.unwrap();
    let actor=tx.query_one(account_write_sql(),&[&publisher]).await.unwrap();
    assert_eq!(actor.get::<_,String>(0),"PublicUploadUser1");
    let mut other=f.state.db.client().await.unwrap();
    let concurrent=other.transaction().await.unwrap();
    concurrent.batch_execute("SET LOCAL lock_timeout='200ms'; SET LOCAL statement_timeout='500ms'").await.unwrap();
    let fk=concurrent.execute("INSERT INTO platform.protocol_publication_commands(command_id,user_id,input_hash,resource_kind,catalog_id,created_at) VALUES('owned-account-lock-fk','PublicUploadUser1',$1,'manga',$2,1)",&[&"22".repeat(32),&FIXTURE_MANGA_ID]).await;
    concurrent.rollback().await.unwrap();
    // Every attempted mutation is rolled back, including an unexpected success.
    // The primary account lock stays held throughout all independent probes.
    for statement in [
        "UPDATE platform.users SET disabled=true WHERE id='PublicUploadUser1'",
        "UPDATE platform.users SET permissions='[]'::jsonb WHERE id='PublicUploadUser1'",
        "UPDATE platform.users SET roles='[\"restricted\"]'::jsonb WHERE id='PublicUploadUser1'",
        "UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser2' WHERE publisher=$1",
        "DELETE FROM platform.protocol_identity_bindings WHERE publisher=$1",
        account_write_sql(),
    ] {
        let mut other=f.state.db.client().await.unwrap();
        let probe=other.transaction().await.unwrap();
        probe.batch_execute("SET LOCAL lock_timeout='200ms'; SET LOCAL statement_timeout='500ms'").await.unwrap();
        let result=if statement.contains("$1") {
            probe.query(statement,&[&publisher]).await
        } else { probe.query(statement,&[]).await };
        probe.rollback().await.unwrap();
        assert_eq!(result.unwrap_err().code(),Some(&tokio_postgres::error::SqlState::LOCK_NOT_AVAILABLE));
    }
    tx.rollback().await.unwrap();
    assert_eq!(fk.expect("independent publication FK must coexist with the account write fence"),1);
    let db=f.state.db.client().await.unwrap();
    let account=db.query_one("SELECT disabled,roles,permissions FROM platform.users WHERE id='PublicUploadUser1'",&[]).await.unwrap();
    assert!(!account.get::<_,bool>(0));
    assert_eq!(account.get::<_,Value>(1),json!(["user"]));
    assert!(account.get::<_,Value>(2).as_array().unwrap().contains(&json!("chapter.create")));
    assert_eq!(db.query_one("SELECT user_id FROM platform.protocol_identity_bindings WHERE publisher=$1",&[&publisher]).await.unwrap().get::<_,String>(0),"PublicUploadUser1");
    assert_eq!(db.query_one("SELECT count(*) FROM platform.protocol_publication_commands WHERE command_id='owned-account-lock-fk'",&[]).await.unwrap().get::<_,i64>(0),0);
}

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
    signed_recovery_contracts(&f,&session,&file,&other_files["data"][0],&image).await;
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}/{}",other_files["data"][0]["id"].as_str().unwrap()),None,404).await;
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}/batch"),Some(json!([file["id"],other_files["data"][0]["id"]])),404).await;
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.upload_files WHERE session_id=$1",&[&native_session]).await.unwrap().get::<_,i64>(0),1);
    f.call(Some(2),"DELETE",&format!("/api/v1/upload/{other}"),None,200).await;
    original_group_refusal_is_distinct_from_admission(&f,&image).await;
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
    assert_eq!(receipt["relationships"].as_array().unwrap().iter().filter(|relation|relation["type"]=="manga").collect::<Vec<_>>(),vec![&json!({"id":f.manga,"type":"manga"})]);
    assert_eq!(receipt["attributes"]["nostr"]["publicationStatus"],"accepted_pending_projection");assert_eq!(receipt["attributes"]["isUnavailable"],true);
    let recovery_path=format!("/api/v1/nostr/uploads/{session}");
    let recovered=f.call(Some(1),"GET",&recovery_path,None,200).await;
    assert_eq!(recovered["session"]["isCommitted"],true);
    assert_eq!(recovered["session"]["canMutate"],false);
    assert_eq!(recovered["session"]["canRetryCommit"],false);
    assert_eq!(recovered["session"]["receiptStatus"],"available");
    assert_eq!(recovered["session"]["receipt"]["id"],receipt["id"]);
    assert_eq!(recovered["session"]["receipt"]["attributes"]["nostr"],receipt["attributes"]["nostr"]);
    assert_eq!(recovered["session"]["receipt"]["relationships"],receipt["relationships"]);
    assert_eq!(recovered["session"]["mangaId"],f.manga);
    assert_eq!(recovered["session"]["targetStatus"],"accepted_pending_projection");
    // Current original publisher authority, rather than an old cached receipt,
    // controls the private pending view on every recovery read.
    f.repository.set_publisher_admission(id(1),false).await.unwrap();
    let revoked_receipt=f.call(Some(1),"GET",&recovery_path,None,200).await;
    assert_eq!(revoked_receipt["session"]["isCommitted"],true);
    assert_eq!(revoked_receipt["session"]["receiptStatus"],"unavailable");
    assert!(revoked_receipt["session"]["receipt"].is_null());
    f.repository.set_publisher_admission(id(1),true).await.unwrap();
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

// Run this exact function in its own owned disposable fixture. Every request
// below reaches the actual TCP router, raw NIP-98 verifier, decoder and database.
// Ignored source is a regression specification until that run is authorized.
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires an independently reviewed owned disposable PostgreSQL/HTTP runner"]
async fn exact_nostr_page_replacement_preserves_identity_order_and_authority_fences() {
    let f=fixture().await;
    let session=f.begin(1).await;
    let image=png();let replacement=replacement_png();
    let attached=f.multipart(1,&session,&[("one.png","image/png",&image),("two.png","image/png",&image),
        ("three.png","image/png",&image)],200).await;
    let files=attached["data"].as_array().unwrap();
    let file=files[0]["id"].as_str().unwrap();
    let foreign=f.begin(2).await;
    let foreign_attached=f.multipart(2,&foreign,&[("foreign.png","image/png",&image)],200).await;
    let foreign_file=foreign_attached["data"][0]["id"].as_str().unwrap();
    let snapshot_path=format!("/api/v1/nostr/uploads/{session}");
    let replace_path=format!("{snapshot_path}/files/{file}/replace");
    let initial=f.call(Some(1),"GET",&snapshot_path,None,200).await;
    let order=json!([files[2]["id"],files[0]["id"],files[1]["id"]]);
    let draft=json!({"title":"Retained private draft","volume":"2","chapter":"4.5","language":"en"});
    let staged=f.call(Some(1),"PATCH",&snapshot_path,Some(json!({"version":initial["session"]["version"],
        "draft":draft,"pageOrder":order})),200).await;
    let version=staged["session"]["version"].as_i64().unwrap()as i32;
    let native_session=f.native(ResourceKind::UploadSession,&session).await;
    let native_file=f.native(ResourceKind::UploadFile,file).await;
    let db=f.state.db.client().await.unwrap();
    let before=db.query("SELECT f.id,f.position,f.object_id,f.original_name,o.digest FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id WHERE f.session_id=$1 ORDER BY f.position",&[&native_session]).await.unwrap();
    let old_key: String=before[0].get(2);

    for (key,status) in [(2,404),(3,403),(4,403),(5,403),(6,403),(7,403)] {
        f.replace(key,&session,file,version,"replacement.png","image/png",&image,status).await;
    }
    f.replace(1,&session,foreign_file,version,"replacement.png","image/png",&image,404).await;
    let version_text=version.to_string();
    let version_part=("expectedVersion",None,None,version_text.as_bytes());
    let file_part=("file",Some("replacement.png"),Some("image/png"),image.as_slice());
    let (ct,body)=replacement_multipart(&[version_part,file_part]);
    let token=proof(1,11,&format!("{}{replace_path}",f.origin),"POST",&body);
    f.raw("POST",&replace_path,body.clone(),&ct,None,401).await;
    f.raw("POST",&replace_path,body.clone(),&ct,
        Some(proof(1,12,&format!("{}{replace_path}",f.origin),"POST",&body)),401).await;
    f.raw("POST",&replace_path,body.clone(),&ct,
        Some(proof(1,11,&format!("{}{replace_path}",f.origin),"GET",&body)),401).await;
    let (_,changed_version_body)=replacement_multipart(&[("expectedVersion",None,None,b"1"),file_part]);
    f.raw("POST",&replace_path,changed_version_body,&ct,Some(token.clone()),401).await;
    let (_,changed_name_body)=replacement_multipart(&[version_part,
        ("file",Some("altered.png"),Some("image/png"),image.as_slice())]);
    f.raw("POST",&replace_path,changed_name_body,&ct,Some(token),401).await;
    let query_path=format!("{replace_path}?expectedVersion={version}");
    f.raw("POST",&query_path,body.clone(),&ct,
        Some(proof(1,11,&format!("{}{query_path}",f.origin),"POST",&body)),400).await;
    f.raw("POST",&replace_path,body.clone(),"application/json",
        Some(proof(1,11,&format!("{}{replace_path}",f.origin),"POST",&body)),415).await;
    let cookie=f.http.post(format!("{}{replace_path}",f.origin)).header("content-type",&ct)
        .header("cookie","weebdex_session=synthetic-public-upload-cookie").body(body.clone()).send().await.unwrap();
    assert_eq!(cookie.status().as_u16(),401);
    let unsigned=f.http.post(format!("{}{replace_path}",f.origin)).header("content-type",&ct)
        .header("authorization",proof(1,11,&format!("{}{replace_path}",f.origin),"POST",&body))
        .header("x-weebdex-upload-version",version.to_string()).body(body).send().await.unwrap();
    assert_eq!(unsigned.status().as_u16(),400);

    // Missing, repeated, out-of-order, unknown and file-shaped version parts
    // never degrade into a default version or a multi-file partial operation.
    let mut invalid_parts=vec![vec![file_part],vec![version_part],vec![file_part,version_part],
        vec![version_part,version_part,file_part],vec![version_part,file_part,file_part],
        vec![version_part,file_part,("unknown",None,None,b"x" as &[u8])],
        vec![("expectedVersion",Some("version.txt"),None,version_text.as_bytes()),file_part],
        vec![("expectedVersion",None,Some("text/plain"),version_text.as_bytes()),file_part],
        vec![version_part,("file",None,Some("image/png"),image.as_slice())]];
    for bad in ["", "0", "01", "+1", "-1", " 1", "1 ", "1\n", "1.0", "2147483648", "00000000001"] {
        invalid_parts.push(vec![("expectedVersion",None,None,bad.as_bytes()),file_part]);
    }
    for parts in invalid_parts {
        let (ct,body)=replacement_multipart(&parts);
        let proof=proof(1,11,&format!("{}{replace_path}",f.origin),"POST",&body);
        f.raw("POST",&replace_path,body,&ct,Some(proof),400).await;
    }
    for (name,mime,bytes) in [("broken.png","image/png",b"not an encoded image" as &[u8]),
        ("mismatch.jpg","image/jpeg",image.as_slice()),("bad.png","text/html",image.as_slice()),
        ("empty.png","image/png",b"" as &[u8])] {
        f.replace(1,&session,file,version,name,mime,bytes,400).await;
    }
    let too_large=vec![0u8;domains::uploads::storage::MAX_FILE_BYTES as usize+1];
    f.replace(1,&session,file,version,"oversized.png","image/png",&too_large,413).await;
    drop(too_large);
    let (ct,mut truncated)=replacement_multipart(&[version_part,file_part]);
    truncated.truncate(truncated.len()-10);
    let truncated_proof=proof(1,11,&format!("{}{replace_path}",f.origin),"POST",&truncated);
    f.raw("POST",&replace_path,truncated,&ct,Some(truncated_proof),400).await;
    f.replace(1,&session,file,version-1,"replacement.png","image/png",&image,409).await;
    db.execute("UPDATE platform.users SET permissions='[]' WHERE id='PublicUploadUser1'",&[]).await.unwrap();
    f.replace(1,&session,file,version,"replacement.png","image/png",&image,403).await;
    db.execute("UPDATE platform.users SET permissions='[\"chapter.create\",\"self.chapter.update\",\"self.chapter.remove\"]' WHERE id='PublicUploadUser1'",&[]).await.unwrap();
    f.repository.set_account_admission(id(1),false).await.unwrap();
    f.replace(1,&session,file,version,"replacement.png","image/png",&image,403).await;
    f.repository.set_account_admission(id(1),true).await.unwrap();
    db.execute("DELETE FROM platform.protocol_identity_bindings WHERE publisher=$1",&[&public(1)]).await.unwrap();
    f.replace(1,&session,file,version,"replacement.png","image/png",&image,403).await;
    db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,'PublicUploadUser1')",&[&public(1)]).await.unwrap();
    for state in ["prepared","source_accepted"] {
        db.execute("INSERT INTO platform.protocol_publication_commands(command_id,user_id,input_hash,resource_kind,catalog_id,upload_session_id,created_at,state) VALUES('replacement-fixed-input','PublicUploadUser1',$1,'chapter',$2,$3,1,$4)",
            &[&"77".repeat(32),&native_file,&native_session,&state]).await.unwrap();
        f.replace(1,&session,file,version,"replacement.png","image/png",&image,409).await;
        db.execute("DELETE FROM platform.protocol_publication_commands WHERE command_id='replacement-fixed-input'",&[]).await.unwrap();
    }
    db.execute("UPDATE platform.upload_sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE id=$1",&[&native_session]).await.unwrap();
    f.replace(1,&session,file,version,"replacement.png","image/png",&image,410).await;
    db.execute("UPDATE platform.upload_sessions SET expires_at=clock_timestamp()+interval '1 hour' WHERE id=$1",&[&native_session]).await.unwrap();
    let unchanged=f.call(Some(1),"GET",&snapshot_path,None,200).await;
    for field in ["version","draft","pageOrder","files"] {assert_eq!(unchanged["session"][field],staged["session"][field],"refusal mutated {field}");}
    signed_preview(&f,1,unchanged["session"]["files"][1]["previewUrl"].as_str().unwrap(),&image).await;

    // More than one MiB proves this path uses its image budget rather than the
    // ordinary JSON limit. The actual file and its native position are stable.
    let replaced=f.replace(1,&session,file,version,"replacement.png","image/png",&replacement,200).await;
    assert_eq!(replaced["result"],"ok");assert_eq!(replaced["file"]["id"],file);
    assert_eq!(replaced["file"]["type"],"upload_session_file");
    assert_eq!(replaced["file"]["attributes"]["originalFileName"],"replacement.png");
    assert_eq!(replaced["file"]["attributes"]["fileHash"],auth::hash(&replacement));
    assert_eq!(replaced["file"]["attributes"]["fileSize"],replacement.len());
    assert_eq!(replaced["file"]["attributes"]["mimeType"],"image/png");
    assert_eq!(replaced["sessionVersion"],version+1);
    assert!(replaced["file"].get("file_name").is_none());
    let after=f.call(Some(1),"GET",&snapshot_path,None,200).await;
    assert_eq!(after["session"]["version"],replaced["sessionVersion"]);
    assert_eq!(after["session"]["pageOrder"],order);assert_eq!(after["session"]["draft"],draft);
    assert_eq!(after["session"]["files"][0],staged["session"]["files"][0]);
    assert_eq!(after["session"]["files"][2],staged["session"]["files"][2]);
    assert_eq!(after["session"]["files"][1]["id"],file);
    assert_eq!(after["session"]["files"][1]["dimensions"],json!({"width":640,"height":640}));
    assert_eq!(f.native(ResourceKind::UploadFile,file).await,native_file);
    let after_rows=db.query("SELECT f.id,f.position,f.object_id,f.original_name,o.digest,o.storage_key FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id WHERE f.session_id=$1 ORDER BY f.position",&[&native_session]).await.unwrap();
    assert_eq!(after_rows.len(),3);
    for (old,new) in before.iter().zip(&after_rows) {
        assert_eq!(old.get::<_,String>(0),new.get::<_,String>(0));assert_eq!(old.get::<_,i32>(1),new.get::<_,i32>(1));
        if new.get::<_,&str>(0)!=native_file {
            for field in [2,3,4] {assert_eq!(old.get::<_,String>(field),new.get::<_,String>(field));}
        }
    }
    assert_ne!(after_rows[0].get::<_,String>(2),old_key);
    assert_eq!(after_rows[0].get::<_,String>(4),auth::hash(&replacement));
    let actual=tokio::fs::read(domains::uploads::storage::object_path(&f.state.config.uploads_dir,&after_rows[0].get::<_,String>(5)).unwrap()).await.unwrap();
    assert_eq!(actual,replacement);
    let preview=after["session"]["files"][1]["previewUrl"].as_str().unwrap();
    assert_eq!(preview,staged["session"]["files"][1]["previewUrl"].as_str().unwrap());
    signed_preview(&f,1,preview,&replacement).await;
    f.call(None,"GET",preview,None,401).await;f.call(Some(2),"GET",preview,None,404).await;
    f.replace(1,&session,file,version,"replacement.png","image/png",&replacement,409).await;
    let identical=f.replace(1,&session,file,version+1,"replacement.png","image/png",&replacement,200).await;
    assert_eq!(identical,replaced,"current-version identical replacement is unchanged");

    // A signature is one-use even for an unchanged replacement; the signed
    // expectedVersion cannot be swapped after the user signed the raw request.
    let actual_version=(version+1).to_string();
    let (ct,body)=replacement_multipart(&[("expectedVersion",None,None,actual_version.as_bytes()),
        ("file",Some("replacement.png"),Some("image/png"),replacement.as_slice())]);
    let token=proof(1,11,&format!("{}{replace_path}",f.origin),"POST",&body);
    f.raw("POST",&replace_path,body.clone(),&ct,Some(token.clone()),200).await;
    f.raw("POST",&replace_path,body,&ct,Some(token),401).await;
    assert_eq!(f.call(Some(1),"GET",&snapshot_path,None,200).await["session"]["version"],version+1);
    assert_eq!(db.query_one("SELECT count(*) FROM platform.protocol_publication_commands",&[]).await.unwrap().get::<_,i64>(0),0);

    // Owning staged files does not delegate the operator's publication signer.
    let foreign_snapshot=f.call(Some(2),"GET",&format!("/api/v1/nostr/uploads/{foreign}"),None,200).await;
    f.replace(2,&foreign,foreign_file,foreign_snapshot["session"]["version"].as_i64().unwrap()as i32,
        "own-replacement.jpg","image/jpeg",&jpeg(),200).await;
    f.call(Some(2),"POST",&format!("/api/v1/upload/{foreign}/commit"),Some(commit(foreign_attached["data"].as_array().unwrap())),403).await;
    assert_eq!(db.query_one("SELECT count(*) FROM platform.protocol_publication_commands WHERE user_id='PublicUploadUser2'",&[]).await.unwrap().get::<_,i64>(0),0);
    f.call(Some(2),"DELETE",&format!("/api/v1/upload/{foreign}"),None,200).await;

    // Genuine repeated-page references to already verified bytes bring this
    // otherwise valid session just below its existing aggregate byte budget.
    // Swapping another small page must reject atomically before crossing it.
    let large_bytes=replacement.len()as i64;
    let initial_bytes=large_bytes+2*image.len()as i64;
    let copies=(domains::uploads::storage::MAX_SESSION_BYTES-initial_bytes)/large_bytes;
    assert!(copies+3<=domains::uploads::storage::MAX_FILES as i64);
    let admitted_bytes=initial_bytes+copies*large_bytes;
    assert!(admitted_bytes<=domains::uploads::storage::MAX_SESSION_BYTES);
    assert!(admitted_bytes-image.len()as i64+large_bytes>domains::uploads::storage::MAX_SESSION_BYTES);
    db.execute("INSERT INTO platform.upload_files(id,session_id,object_id,original_name,position) SELECT 'Rep'||lpad(n::text,7,'0'),$1,$2,'admitted shared replacement.png',n FROM generate_series(3,$3::integer) n",
        &[&native_session,&after_rows[0].get::<_,String>(2),&((copies+2)as i32)]).await.unwrap();
    let budget_file=files[1]["id"].as_str().unwrap();
    let budget_refused=f.replace(1,&session,budget_file,version+1,"over-budget.png","image/png",&replacement,400).await;
    assert_eq!(budget_refused["errors"][0]["detail"],"Upload session exceeds its byte budget");
    let actual=db.query_one("SELECT s.version,f.object_id,(SELECT sum(o.bytes)::BIGINT FROM platform.upload_files all_files JOIN platform.media_objects o ON o.id=all_files.object_id WHERE all_files.session_id=s.id) FROM platform.upload_sessions s JOIN platform.upload_files f ON f.session_id=s.id WHERE s.id=$1 AND f.id=$2",
        &[&native_session,&before[1].get::<_,String>(0)]).await.unwrap();
    assert_eq!(actual.get::<_,i32>(0),version+1);assert_eq!(actual.get::<_,String>(1),before[1].get::<_,String>(2));
    assert_eq!(actual.get::<_,i64>(2),admitted_bytes);
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}"),None,200).await;
    f.replace(1,&session,file,version+1,"replacement.png","image/png",&replacement,409).await;
    assert_eq!(std::fs::read_dir(f.state.config.uploads_dir.join("temporary")).unwrap().count(),0);
}
