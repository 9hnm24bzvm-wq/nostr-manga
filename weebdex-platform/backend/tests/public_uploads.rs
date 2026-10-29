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
struct OriginalServer { task:tokio::task::JoinHandle<anyhow::Result<()>>, shutdown:Option<tokio::sync::oneshot::Sender<()>> }
impl Drop for OriginalServer {fn drop(&mut self){if let Some(stop)=self.shutdown.take(){let _=stop.send(());}self.task.abort();}}
struct Fixture {
    state:AppState, repository:Repository, http:reqwest::Client, origin:String,
    manga:String, native_manga:String, _server:Server, _original:Option<OriginalServer>,
    original_store:Option<Arc<manga_node::upload::UploadStore>>, original_origin:Option<String>, _storage:tempfile::TempDir,
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
        let schedule_path=path.split('?').next().unwrap();
        let schedule_private=(method=="GET"&&(schedule_path=="/api/v1/nostr/uploads/schedules"||schedule_path.ends_with("/schedule")))
            ||(method=="POST"&&schedule_path.ends_with("/schedule/retry"));
        if schedule_private {assert!(response.headers().get_all("vary").iter().any(|v|v.to_str().unwrap().split(',').any(|v|v.trim().eq_ignore_ascii_case("authorization"))));}
        assert_eq!(response.headers()["cache-control"],if private_upload||schedule_private {"private, no-store"}else{"no-store"});
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
async fn fixture()->Fixture {fixture_options(Config::default(),false).await}
async fn fixture_options(limits:Config,original:bool)->Fixture {fixture_full(limits,original,false).await}
async fn fixture_full(limits:Config,original:bool,projection:bool)->Fixture {
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
        operator_key_file:signer.clone(),expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),
        storage:json!({"path":storage.path().join("protocol-originals"),"volume":"PublicUploadFixture","physical_bytes":64*1024*1024,
            "staging_bytes":64*1024*1024,"max_blob_bytes":8*1024*1024,"upload_slots":2}),group_keys_dir:None,backend:None,
        native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":service,"admin_pubkey":public(1),
            "database_plaintext_local":true,"read_policy":"public","sync_worker_enabled":false})) };
    let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:10,
        scheduled_upload_maximum:limits.scheduled_upload_maximum,scheduled_upload_bytes:limits.scheduled_upload_bytes,
        scheduled_upload_retry_seconds:limits.scheduled_upload_retry_seconds,
        uploads_dir:storage.path().join("native-images"),protocol_publication:Some(publication.clone()),..Config::default()},dsn.clone()).await.unwrap();
    let mut repository=Repository::connect(DbConfig{dsn,connections:1,acquisition_timeout:Duration::from_secs(2),cache_weight:1048576,plaintext_local:true}).await.unwrap();
    repository.pool=state.db.pool().clone();repository.migrate().await.unwrap();repository.bind_instance(id(11),id(1)).await.unwrap();
    for key in 1..=8 {repository.set_account_admission(id(key),true).await.unwrap();}
    repository.set_publisher_admission(id(1),true).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();state.protocol.verify_schema().await.unwrap();
    state.protocol.install_visibility_function().await.unwrap();
    let (original_store,original_origin,original_server)=if original {
        let store=manga_node::upload::UploadStore::open(repository.clone(),serde_json::from_value(publication.storage.clone()).unwrap(),id(1),id(11)).await.unwrap();
        let runtime=weebdex_platform::protocol::publication::PublicationRuntime::from_owned_store(repository.clone(),store.clone(),&signer,&public(1),None).unwrap();
        state.protocol.attach_publication_runtime(runtime).await;
        let reservation=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let address=reservation.local_addr().unwrap();drop(reservation);
        let origin=format!("http://{address}");let mut config=publication.native_node.clone().unwrap();
        config["bind"]=json!(address.to_string());config["base_url"]=json!(origin);config["database_connections"]=json!(10);
        if projection {
            let binary=std::env::var_os("STRFRY_BIN").map(std::path::PathBuf::from).expect("Use the existing verified STRFRY_BIN; never download a fixture binary");
            assert!(binary.is_absolute()&&binary.is_file());
            config["backend"]=json!({"binary":binary,"data_dir":storage.path().join("real-projection"),"map_bytes":64*1024*1024,"timeout_seconds":5});
        }
        let config=serde_json::from_value(config).unwrap();
        let (ready,wait)=tokio::sync::oneshot::channel();let (stop,shutdown)=tokio::sync::oneshot::channel();
        let repo=repository.clone();let shared=store.clone();
        let task=tokio::spawn(async move{manga_node::node::serve_prepared_with_store(config,repo,None,Some(shared),Some(ready),Some(shutdown)).await});
        tokio::time::timeout(Duration::from_secs(30),wait).await.unwrap().unwrap();
        store.configure_public_export_role().await.unwrap();
        (Some(store),Some(origin),Some(OriginalServer{task,shutdown:Some(stop)}))
    } else {state.protocol.configure_publication(publication).await.unwrap();(None,None,None)};
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
    let manga=PostgresPublicIdResolver::new(state.db.clone()).preferred(ResourceKind::Manga,&native_manga).await.unwrap().unwrap().to_string();
    drop(db);
    let app=weebdex_platform::router::router(state.clone());
    let server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap()}));
    Fixture{state,repository,http:reqwest::Client::new(),origin,manga,native_manga,_server:server,_original:original_server,original_store,original_origin,_storage:storage}
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

async fn schedule_account(f:&Fixture,owner:&str)->String {
    PostgresPublicIdResolver::new(f.state.db.clone()).preferred(ResourceKind::User,owner).await.unwrap().unwrap().to_string()
}
async fn schedule_time(f:&Fixture,seconds:i64)->String {
    f.state.db.client().await.unwrap().query_one("SELECT to_char((clock_timestamp()+$1::bigint*interval '1 second') AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS')",&[&seconds]).await.unwrap().get(0)
}
async fn scheduled_upload(f:&Fixture,bytes:&[u8],seconds:i64)->(String,String,Value,Value) {
    let session=f.begin(1).await;
    let files=f.multipart(1,&session,&[("scheduled.png","image/png",bytes)],200).await;
    let mut payload=commit(files["data"].as_array().unwrap());payload["chapterDraft"]["publishAt"]=json!(schedule_time(f,seconds).await);
    let receipt=f.call(Some(1),"POST",&format!("/api/v1/upload/{session}/commit"),Some(payload.clone()),202).await;
    assert_eq!(receipt["nostr"]["publicationStatus"],"scheduled");
    assert_eq!(receipt["nostr"]["sourceAccepted"],false);assert_eq!(receipt["nostr"]["readerAvailable"],false);
    assert!(receipt["nostr"]["eventId"].is_null());assert!(receipt["attributes"]["readableAt"].is_null());
    let native=f.native(ResourceKind::UploadSession,&session).await;
    (session,native,receipt,payload)
}
async fn await_schedule_due(f:&Fixture,id:&str) {
    let seconds:f64=f.state.db.client().await.unwrap().query_one("SELECT greatest(0,extract(epoch FROM due_at-clock_timestamp()))::float8 FROM platform.scheduled_uploads WHERE session_id=$1",&[&id]).await.unwrap().get(0);
    assert!(seconds<30.0,"fixture must use a near deadline, not wait on real future work");
    tokio::time::sleep(Duration::from_secs_f64(seconds+0.05)).await;
}
fn export_path(f:&Fixture,hash:&str,extension:&str)->std::path::PathBuf {
    f._storage.path().join("protocol-originals/public").join(&hash[..2]).join(&hash[2..4]).join(format!("{hash}.{extension}"))
}
async fn original_get(f:&Fixture,hash:&str,method:reqwest::Method,range:bool,status:u16)->Vec<u8> {
    let mut request=f.http.request(method,format!("{}/originals/{hash}",f.original_origin.as_ref().unwrap()));
    if range {request=request.header("Range","bytes=0-15");}
    let response=request.send().await.unwrap();assert_eq!(response.status().as_u16(),status);
    response.bytes().await.unwrap().to_vec()
}

#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Owned disposable PostgreSQL, existing real STRFRY_BIN and loopback original node; run alone"]
async fn scheduled_private_original_export_expiry_cancel_and_rebind() {
    let f=fixture_full(Config::default(),true,true).await;
    let account=schedule_account(&f,"PublicUploadUser1").await;
    let list=format!("/api/v1/nostr/uploads/schedules?expectedAccountId={account}");
    assert_eq!(f.call(Some(1),"GET",&list,None,200).await["data"],json!([]));
    f.call(Some(1),"GET","/api/v1/nostr/uploads/schedules",None,400).await;
    // Publish an unrelated synthetic old original first. Its successful GET,
    // Range and real export prove the negative scheduled controls are meaningful.
    let old_parent="UploadOld1";
    f.state.db.client().await.unwrap().execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES($1,'PublicUploadUser1','Synthetic uploader','Old original control','en','ongoing','safe','published')",&[&old_parent]).await.unwrap();
    let old_manga=PostgresPublicIdResolver::new(f.state.db.clone()).preferred(ResourceKind::Manga,old_parent).await.unwrap().unwrap().to_string();
    let old=f.call(Some(1),"POST","/api/v1/upload/begin",Some(json!({"manga":old_manga,"groups":[]})),200).await;
    let old_session=old["id"].as_str().unwrap();let old_bytes=png();let old_hash=hex::encode(Sha256::digest(&old_bytes));
    let old_files=f.multipart(1,old_session,&[("old.png","image/png",&old_bytes)],200).await;
    let old_receipt=f.call(Some(1),"POST",&format!("/api/v1/upload/{old_session}/commit"),Some(commit(old_files["data"].as_array().unwrap())),200).await;
    wait_original_projection(&f,old_receipt["attributes"]["nostr"]["eventId"].as_str().unwrap()).await;
    assert_eq!(original_get(&f,&old_hash,reqwest::Method::GET,false,200).await,old_bytes);
    assert_eq!(original_get(&f,&old_hash,reqwest::Method::GET,true,200).await,old_bytes);
    for _ in 0..16 {f.original_store.as_ref().unwrap().reconcile_public_exports().await.unwrap();if export_path(&f,&old_hash,"png").exists(){break;}}
    assert_eq!(std::fs::read(export_path(&f,&old_hash,"png")).unwrap(),old_bytes);
    let before:i64=f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.protocol_publication_commands",&[]).await.unwrap().get(0);
    let bytes=replacement_png();let hash=hex::encode(Sha256::digest(&bytes));
    let (session,native,receipt,payload)=scheduled_upload(&f,&bytes,2*24*60*60).await;
    let reserved=receipt["id"].as_str().unwrap();
    let status=format!("/api/v1/nostr/uploads/{session}/schedule?expectedAccountId={account}");
    let repeated=f.call(Some(1),"POST",&format!("/api/v1/upload/{session}/commit"),Some(payload.clone()),202).await;
    assert_eq!(repeated["id"],receipt["id"]);
    let mut changed=payload;changed["chapterDraft"]["title"]=json!("Forbidden changed intent");
    f.call(Some(1),"POST",&format!("/api/v1/upload/{session}/commit"),Some(changed),409).await;
    assert_eq!(f.call(Some(1),"GET",&list,None,200).await["data"].as_array().unwrap().len(),1);
    assert_eq!(f.call(Some(1),"GET",&status,None,200).await["receipt"]["id"],receipt["id"]);
    for path in [format!("/api/v1/chapter/{reserved}"),format!("/api/v1/at-home/server/{reserved}")] {
        f.call(None,"GET",&path,None,404).await;
    }
    assert!(!weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap());
    let db=f.state.db.client().await.unwrap();
    assert_eq!(db.query_one("SELECT count(*) FROM platform.protocol_publication_commands",&[]).await.unwrap().get::<_,i64>(0),before);
    assert_eq!(db.query_one("SELECT count(*) FROM platform.public_resource_ids WHERE kind='chapter' AND public_id=$1::text::uuid",&[&reserved]).await.unwrap().get::<_,i64>(0),0);
    assert_eq!(db.query_one("SELECT count(*) FROM platform.catalog_chapters c JOIN platform.scheduled_uploads i ON c.id=i.chapter_id WHERE i.session_id=$1",&[&native]).await.unwrap().get::<_,i64>(0),0);
    let digests=db.query("SELECT o.digest FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id WHERE f.session_id=$1 UNION SELECT o.digest FROM platform.upload_files f JOIN platform.upload_file_derivatives d ON d.file_id=f.id JOIN platform.media_objects o ON o.id=d.object_id WHERE f.session_id=$1",&[&native]).await.unwrap();
    assert!(digests.len()>1,"fixture must actually exercise retained derivatives");
    for row in digests {
        let digest:String=row.get(0);let binary=hex::decode(&digest).unwrap();
        for table in ["blob_intents","publication_blobs","public_export_holds","public_export_queue"] {
            assert_eq!(db.query_one(&format!("SELECT count(*) FROM {table} WHERE hash=$1"),&[&binary]).await.unwrap().get::<_,i64>(0),0,"{table}");
        }
        for extension in ["png","jpg","webp"] {assert!(!export_path(&f,&digest,extension).exists());}
        original_get(&f,&digest,reqwest::Method::GET,false,404).await;
        original_get(&f,&digest,reqwest::Method::GET,true,404).await;
        original_get(&f,&digest,reqwest::Method::HEAD,true,404).await;
    }
    // Ordinary 24-hour session expiry must not erase a two-day schedule or occupy
    // the user's one OPEN slot. Native cancel must not delete private references.
    db.execute("UPDATE platform.upload_sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE id=$1",&[&native]).await.unwrap();
    let files_before:i64=db.query_one("SELECT count(*) FROM platform.upload_files WHERE session_id=$1",&[&native]).await.unwrap().get(0);
    let missing_origin=f.http.delete(format!("{}/api/upload/{native}",f.origin)).header("cookie","weebdex_session=synthetic-public-upload-cookie").send().await.unwrap();
    assert_eq!(missing_origin.status().as_u16(),403,"native CSRF protection remains separate from the scheduling fence");
    let cancel=f.http.delete(format!("{}/api/upload/{native}",f.origin)).header("origin",&f.origin).header("cookie","weebdex_session=synthetic-public-upload-cookie").send().await.unwrap();
    assert_eq!(cancel.status().as_u16(),409);
    drop(db);domains::uploads::recover_storage(&f.state).await;
    let next=f.begin(1).await;assert_ne!(next,session);
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{next}"),None,200).await;
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.upload_files WHERE session_id=$1",&[&native]).await.unwrap().get::<_,i64>(0),files_before);
    // A genuine rebind must fail before returning even an otherwise-empty list.
    let db=f.state.db.client().await.unwrap();
    db.batch_execute("INSERT INTO platform.users(id,username,email,password_hash,verified,permissions) VALUES('ScheduledRebound','ScheduledRebound','scheduled-rebound@fixture.invalid','synthetic',true,'[\"chapter.create\",\"self.chapter.update\",\"self.chapter.remove\"]')").await.unwrap();
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='ScheduledRebound' WHERE publisher=$1",&[&public(1)]).await.unwrap();drop(db);
    let rebound=schedule_account(&f,"ScheduledRebound").await;
    f.call(Some(1),"GET",&list,None,409).await;f.call(Some(1),"GET",&status,None,409).await;
    f.call(Some(1),"POST",&format!("/api/v1/nostr/uploads/{session}/schedule/retry?expectedAccountId={account}"),None,409).await;
    assert_eq!(f.call(Some(1),"GET",&format!("/api/v1/nostr/uploads/schedules?expectedAccountId={rebound}"),None,200).await["data"],json!([]));
    let retained=retained_snapshot(&f,&native).await;
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}?expectedAccountId={account}"),None,409).await;
    assert_eq!(retained_snapshot(&f,&native).await,retained,"rebound cancellation must not mutate private rows or pins");
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}"),None,404).await;
    f.state.db.client().await.unwrap().execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser1' WHERE publisher=$1",&[&public(1)]).await.unwrap();
    let cancel=format!("/api/v1/upload/{session}?expectedAccountId={account}");
    for query in [format!("expectedAccountId={account}&extra=1"),format!("expectedAccountId={account}&expectedAccountId={account}"),"expectedAccountId=null".to_owned()] {
        f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}?{query}"),None,400).await;
    }
    let wrong_url=proof(1,11,&format!("{}/api/v1/upload/{session}",f.origin),"DELETE",b"");
    f.raw("DELETE",&cancel,vec![],"application/json",Some(wrong_url),401).await;
    assert_eq!(retained_snapshot(&f,&native).await,retained);
    f.call(Some(1),"DELETE",&cancel,None,200).await;
    assert_eq!(f.call(Some(1),"GET",&status,None,200).await["receipt"]["nostr"]["scheduleState"],"cancelled");
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.upload_files WHERE session_id=$1",&[&native]).await.unwrap().get::<_,i64>(0),0);
    original_get(&f,&hash,reqwest::Method::GET,true,404).await;
    assert_eq!(original_get(&f,&old_hash,reqwest::Method::GET,false,200).await,old_bytes);
}

async fn install_acceptance_fault(f:&Fixture,kind:&str) {
    assert!(matches!(kind,"manga"|"chapter"));
    f.state.db.client().await.unwrap().batch_execute(&format!("CREATE FUNCTION platform.scheduled_acceptance_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.resource_kind='{kind}' AND NEW.state='source_accepted' AND OLD.state='prepared' THEN RAISE EXCEPTION 'synthetic post-acceptance durable acknowledgement fault' USING ERRCODE='58000'; END IF; RETURN NEW; END $$; CREATE TRIGGER scheduled_acceptance_fault BEFORE UPDATE ON platform.protocol_publication_commands FOR EACH ROW EXECUTE FUNCTION platform.scheduled_acceptance_fault()" )).await.unwrap();
}
async fn remove_acceptance_fault(f:&Fixture) {
    f.state.db.client().await.unwrap().batch_execute("DROP TRIGGER scheduled_acceptance_fault ON platform.protocol_publication_commands; DROP FUNCTION platform.scheduled_acceptance_fault()").await.unwrap();
}
async fn wait_original_projection(f:&Fixture,event:&str) {
    let binary:[u8;32]=hex::decode(event).unwrap().try_into().unwrap();
    tokio::time::timeout(Duration::from_secs(30),async {
        loop {if f.state.protocol.entry(binary).await.unwrap().is_some(){break;}tokio::time::sleep(Duration::from_millis(30)).await;}
    }).await.expect("actual original backend did not project the signed receipt");
}
async fn restart_platform_over_owned_source(f:&mut Fixture) {
    let config=f.state.config.as_ref().clone();restart_platform_with_config(f,config).await;
}
// Application recreation in the same OS process, not process-crash or power-loss proof.
async fn restart_platform_with_config(f:&mut Fixture,config:Config) {
    // Recreate the application, database pool, facade and signer runtime, retaining
    // only the separately owned original node/store and durable database/files.
    // This is an application restart, not a claim to have restarted PostgreSQL.
    f._server.0.abort();let _=(&mut f._server.0).await;
    let state=AppState::connect(config,std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").unwrap()).await.unwrap();
    state.protocol.verify_schema().await.unwrap();state.protocol.install_visibility_function().await.unwrap();
    let mut repo=f.repository.clone();repo.pool=state.db.pool().clone();
    let config=state.config.protocol_publication.as_ref().unwrap().clone();
    // Release the previous application's sole runtime owner before reopening its
    // store when there is no separately running original node in this fixture.
    f.state=state.clone();
    if let Some(store)=f.original_store.as_ref() {
        let runtime=weebdex_platform::protocol::publication::PublicationRuntime::from_owned_store(repo.clone(),store.clone(),&config.operator_key_file,&config.expected_publisher,None).unwrap();
        state.protocol.attach_publication_runtime(runtime).await;
    } else {state.protocol.configure_publication(config).await.unwrap();}
    let address=f.origin.strip_prefix("http://").unwrap();let listener=tokio::net::TcpListener::bind(address).await.unwrap();
    let app=weebdex_platform::router::router(state.clone());f._server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap()}));
    f.state=state;f.repository=repo;
}
async fn owner_schedule_retry(f:&Fixture,session:&str) {
    let account=schedule_account(f,"PublicUploadUser1").await;
    f.call(Some(1),"POST",&format!("/api/v1/nostr/uploads/{session}/schedule/retry?expectedAccountId={account}"),None,202).await;
}

#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Owned disposable PostgreSQL and existing real STRFRY_BIN; run alone with fixture guard"]
async fn scheduled_transient_retry_parent_acceptance_restart_and_receipts() {
    let mut f=fixture_full(Config::default(),true,true).await;
    let (session,native,initial,_)=scheduled_upload(&f,&png(),3).await;
    let account=schedule_account(&f,"PublicUploadUser1").await;
    let status=format!("/api/v1/nostr/uploads/{session}/schedule?expectedAccountId={account}");
    f.state.db.client().await.unwrap().batch_execute("CREATE FUNCTION platform.scheduled_start_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF OLD.started_at IS NULL AND NEW.started_at IS NOT NULL THEN RAISE EXCEPTION 'synthetic pre-source infrastructure fault' USING ERRCODE='58000'; END IF; RETURN NEW; END $$; CREATE TRIGGER scheduled_start_fault BEFORE UPDATE ON platform.scheduled_uploads FOR EACH ROW EXECUTE FUNCTION platform.scheduled_start_fault()").await.unwrap();
    await_schedule_due(&f,&native).await;
    let error=weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap_err();assert_eq!(error.status.as_u16(),503);
    let row=f.state.db.client().await.unwrap().query_one("SELECT state,started_at IS NULL,retry_attempts,failure_code,next_attempt_at>clock_timestamp() FROM platform.scheduled_uploads WHERE session_id=$1",&[&native]).await.unwrap();
    assert_eq!(row.get::<_,String>(0),"scheduled");assert!(row.get::<_,bool>(1));assert_eq!(row.get::<_,i32>(2),1);assert_eq!(row.get::<_,String>(3),"transient_retry");assert!(row.get::<_,bool>(4));
    assert!(!weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap());
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.protocol_publication_commands",&[]).await.unwrap().get::<_,i64>(0),0);
    f.state.db.client().await.unwrap().batch_execute("DROP TRIGGER scheduled_start_fault ON platform.scheduled_uploads; DROP FUNCTION platform.scheduled_start_fault()").await.unwrap();
    install_acceptance_fault(&f,"manga").await;
    wait_retry_due(&f,&native).await;
    assert_eq!(weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap_err().status.as_u16(),503);
    let row=f.state.db.client().await.unwrap().query_one("SELECT i.state,i.started_at IS NOT NULL,i.failure_code,c.event_id,c.scheduled_session_id,c.source_manifest=i.parent_manifest,m.version FROM platform.scheduled_uploads i JOIN platform.upload_sessions s ON s.id=i.session_id JOIN platform.catalog_manga m ON m.id=s.manga_id JOIN platform.protocol_publication_commands c ON c.command_id='manga:'||m.id WHERE i.session_id=$1",&[&native]).await.unwrap();
    assert_eq!(row.get::<_,String>(0),"blocked");assert!(row.get::<_,bool>(1));assert_eq!(row.get::<_,String>(2),"publication_outcome_uncertain");
    let parent:String=row.get(3);assert_eq!(row.get::<_,String>(4),native);assert!(row.get::<_,bool>(5));let version:i32=row.get(6);
    assert!(!weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap(),"post-command503 must not automatically replay");
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}"),None,409).await;
    let pending=f.call(Some(1),"GET",&status,None,200).await["receipt"].clone();
    assert_eq!(pending["nostr"]["readerAvailable"],false);assert!(pending["nostr"]["eventId"].is_null());
    // A normal immediate upload cannot adopt a scheduled parent's durable command.
    let other=f.begin(1).await;let files=f.multipart(1,&other,&[("other.png","image/png",&png())],200).await;
    f.call(Some(1),"POST",&format!("/api/v1/upload/{other}/commit"),Some(commit(files["data"].as_array().unwrap())),409).await;
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{other}"),None,200).await;
    remove_acceptance_fault(&f).await;
    wait_original_projection(&f,&parent).await;
    weebdex_platform::protocol::bridge::import_page(&f.state).await.unwrap();
    let associated=f.state.db.client().await.unwrap().query_one("SELECT protocol_series_id,version FROM platform.catalog_manga WHERE id=$1",&[&f.native_manga]).await.unwrap();
    assert_eq!(associated.get::<_,String>(0),parent);assert_eq!(associated.get::<_,i32>(1),version);
    restart_platform_over_owned_source(&mut f).await;
    owner_schedule_retry(&f,&session).await;
    assert!(weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap());
    let commands=f.state.db.client().await.unwrap().query("SELECT resource_kind,event_id,scheduled_session_id FROM platform.protocol_publication_commands ORDER BY resource_kind",&[]).await.unwrap();
    assert_eq!(commands.len(),2);let chapter:String=commands.iter().find(|r|r.get::<_,String>(0)=="chapter").unwrap().get(1);
    assert_eq!(commands.iter().find(|r|r.get::<_,String>(0)=="manga").unwrap().get::<_,String>(1),parent);
    assert!(commands.iter().all(|r|r.get::<_,String>(2)==native));
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT state FROM platform.scheduled_uploads WHERE session_id=$1",&[&native]).await.unwrap().get::<_,String>(0),"complete");
    let complete=f.call(Some(1),"GET",&status,None,200).await["receipt"].clone();
    assert_ne!(complete["nostr"]["publicationStatus"],"scheduled");assert_eq!(complete["id"],initial["id"]);
    wait_original_projection(&f,&chapter).await;
    for _ in 0..3 {weebdex_platform::protocol::bridge::import_page(&f.state).await.unwrap();weebdex_platform::protocol::covers::materialize_page(&f.state).await.unwrap();}
    let readable=f.call(Some(1),"GET",&status,None,200).await["receipt"].clone();
    assert_eq!(readable["nostr"]["publicationStatus"],"readable");assert_eq!(readable["nostr"]["readerAvailable"],true);
    f.call(None,"GET",&format!("/api/v1/chapter/{}",initial["id"].as_str().unwrap()),None,200).await;
    // Admission controls future publication, not historical reader visibility.
    f.repository.set_publisher_admission(id(1),false).await.unwrap();
    let historical=f.call(Some(1),"GET",&status,None,200).await["receipt"].clone();
    assert_eq!(historical["nostr"]["readerAvailable"],true);
    f.repository.set_publisher_admission(id(1),true).await.unwrap();
    // Inject only the actual source visibility seam for this owned original.
    // This is not a claim that a public signed deletion or moderation UI ran.
    let source_id=hex::decode(&chapter).unwrap();
    let db=f.state.db.client().await.unwrap();
    let original:Value=db.query_one("SELECT raw_event FROM events WHERE id=$1",&[&&source_id[..]]).await.unwrap().get(0);
    assert_eq!(db.execute("UPDATE events SET state='withdrawing' WHERE id=$1 AND state='hosted'",&[&&source_id[..]]).await.unwrap(),1);
    assert_eq!(db.query_one("SELECT raw_event FROM events WHERE id=$1",&[&&source_id[..]]).await.unwrap().get::<_,Value>(0),original);
    drop(db);
    assert!(f.state.protocol.entry(source_id.try_into().unwrap()).await.unwrap().is_none());
    f.call(None,"GET",&format!("/api/v1/chapter/{}",initial["id"].as_str().unwrap()),None,404).await;
    let unavailable=f.call(Some(1),"GET",&status,None,200).await["receipt"].clone();
    assert_eq!(unavailable["nostr"]["readerAvailable"],false);assert_ne!(unavailable["nostr"]["publicationStatus"],"readable");
}

#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Owned disposable PostgreSQL; real two-second configured deadline, run alone"]
async fn scheduled_captured_deadline_restart_and_blocked_quota() {
    let mut f=fixture_options(Config{scheduled_upload_retry_seconds:2,scheduled_upload_maximum:1,..Config::default()},false).await;
    let (session,native,initial,_)=scheduled_upload(&f,&png(),2).await;
    assert_eq!(initial["nostr"]["retryWindowSeconds"],2);
    let saved:f64=f.state.db.client().await.unwrap().query_one("SELECT extract(epoch FROM retry_until)::float8 FROM platform.scheduled_uploads WHERE session_id=$1",&[&native]).await.unwrap().get(0);
    let mut config=f.state.config.as_ref().clone();config.scheduled_upload_retry_seconds=86400;
    restart_platform_with_config(&mut f,config).await;
    let now:f64=f.state.db.client().await.unwrap().query_one("SELECT extract(epoch FROM retry_until)::float8 FROM platform.scheduled_uploads WHERE session_id=$1",&[&native]).await.unwrap().get(0);assert_eq!(saved,now);
    await_schedule_due(&f,&native).await;tokio::time::sleep(Duration::from_millis(2100)).await;
    assert_eq!(weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap_err().status.as_u16(),409);
    let db=f.state.db.client().await.unwrap();
    let row=db.query_one("SELECT state,started_at IS NULL FROM platform.scheduled_uploads WHERE session_id=$1",&[&native]).await.unwrap();assert_eq!(row.get::<_,String>(0),"blocked");assert!(row.get::<_,bool>(1));
    assert_eq!(db.query_one("SELECT count(*) FROM platform.protocol_publication_commands",&[]).await.unwrap().get::<_,i64>(0),0);
    db.execute("UPDATE platform.upload_sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE id=$1",&[&native]).await.unwrap();drop(db);
    domains::uploads::recover_storage(&f.state).await;
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.upload_files WHERE session_id=$1",&[&native]).await.unwrap().get::<_,i64>(0),1);
    let account=schedule_account(&f,"PublicUploadUser1").await;
    let status=format!("/api/v1/nostr/uploads/{session}/schedule?expectedAccountId={account}");
    assert_eq!(f.call(Some(1),"GET",&status,None,200).await["receipt"]["nostr"]["retryWindowSeconds"],2);
    let list=f.call(Some(1),"GET",&format!("/api/v1/nostr/uploads/schedules?expectedAccountId={account}"),None,200).await;
    assert_eq!(list["limits"]["retryWindowSeconds"],86400);
    let next=f.begin(1).await;let files=f.multipart(1,&next,&[("next.png","image/png",&png())],200).await;
    let mut payload=commit(files["data"].as_array().unwrap());payload["chapterDraft"]["publishAt"]=json!(schedule_time(&f,30).await);
    f.call(Some(1),"POST",&format!("/api/v1/upload/{next}/commit"),Some(payload.clone()),409).await;
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}"),None,200).await;
    let accepted=f.call(Some(1),"POST",&format!("/api/v1/upload/{next}/commit"),Some(payload),202).await;
    assert_eq!(accepted["nostr"]["retryWindowSeconds"],86400);
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{next}"),None,200).await;
}

async fn wait_blocked_on(f:&Fixture,blocker:i32) {
    tokio::time::timeout(Duration::from_secs(5),async {
        loop {
            let waiting:bool=f.state.db.client().await.unwrap().query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND $1=ANY(pg_blocking_pids(pid)))",&[&blocker]).await.unwrap().get(0);
            if waiting {break;}tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("real competing transaction never reached the expected lock");
}
async fn wait_claim(f:&Fixture,id:&str,previous:Option<&str>)->String {
    tokio::time::timeout(Duration::from_secs(5),async {
        loop {
            let token:Option<String>=f.state.db.client().await.unwrap().query_one("SELECT lease_token FROM platform.scheduled_uploads WHERE session_id=$1",&[&id]).await.unwrap().get(0);
            if let Some(token)=token {if previous!=Some(token.as_str()){break token;}}
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("real worker did not commit its claim before actor/session locks")
}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Owned disposable PostgreSQL; actual HTTP cancel versus worker locks, run alone"]
async fn scheduled_claim_cancel_serialization_and_stale_lease_cas() {
    let f=fixture().await;
    let (session,native,_,_)=scheduled_upload(&f,&png(),2).await;await_schedule_due(&f,&native).await;
    let mut holder=f.state.db.client().await.unwrap();let tx=holder.transaction().await.unwrap();
    let pid:i32=tx.query_one("SELECT pg_backend_pid()",&[]).await.unwrap().get(0);
    tx.query_one("SELECT id FROM platform.upload_sessions WHERE id=$1 FOR UPDATE",&[&native]).await.unwrap();
    let path=format!("/api/v1/upload/{session}");let url=format!("{}{path}",f.origin);let client=f.http.clone();let token=proof(1,11,&url,"DELETE",b"");
    let cancel=tokio::spawn(async move{client.delete(url).header("authorization",token).send().await.unwrap().status().as_u16()});
    wait_blocked_on(&f,pid).await;
    let state=f.state.clone();let worker=tokio::spawn(async move{weebdex_platform::public_api::scheduled_uploads::run_due(&state).await});
    wait_claim(&f,&native,None).await;tx.commit().await.unwrap();drop(holder);
    assert_eq!(tokio::time::timeout(Duration::from_secs(10),cancel).await.unwrap().unwrap(),200);
    assert!(tokio::time::timeout(Duration::from_secs(10),worker).await.unwrap().unwrap().is_err());
    let row=f.state.db.client().await.unwrap().query_one("SELECT state,started_at IS NULL,lease_token IS NULL FROM platform.scheduled_uploads WHERE session_id=$1",&[&native]).await.unwrap();assert_eq!(row.get::<_,String>(0),"cancelled");assert!(row.get::<_,bool>(1)&&row.get::<_,bool>(2));
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.protocol_publication_commands",&[]).await.unwrap().get::<_,i64>(0),0);
    let (_,native,_,_)=scheduled_upload(&f,&png(),2).await;await_schedule_due(&f,&native).await;
    let mut holder=f.state.db.client().await.unwrap();let tx=holder.transaction().await.unwrap();
    tx.query_one("SELECT publisher FROM account_permissions WHERE publisher=$1 FOR UPDATE",&[&&id(1)[..]]).await.unwrap();
    let state=f.state.clone();let first=tokio::spawn(async move{weebdex_platform::public_api::scheduled_uploads::run_due(&state).await});
    let old=wait_claim(&f,&native,None).await;
    // Accelerate only the mutable operational lease of a genuinely claimed row.
    // Publication deadlines and immutable intent/source inputs remain untouched.
    f.state.db.client().await.unwrap().execute("UPDATE platform.scheduled_uploads SET lease_until=clock_timestamp()-interval '1 second' WHERE session_id=$1 AND lease_token=$2",&[&native,&old]).await.unwrap();
    let state=f.state.clone();let second=tokio::spawn(async move{weebdex_platform::public_api::scheduled_uploads::run_due(&state).await});
    let current=wait_claim(&f,&native,Some(&old)).await;assert_ne!(old,current);tx.commit().await.unwrap();drop(holder);
    assert!(tokio::time::timeout(Duration::from_secs(20),first).await.unwrap().unwrap().is_err());
    assert!(tokio::time::timeout(Duration::from_secs(20),second).await.unwrap().unwrap().unwrap());
    let row=f.state.db.client().await.unwrap().query_one("SELECT state,generation FROM platform.scheduled_uploads WHERE session_id=$1",&[&native]).await.unwrap();assert_eq!(row.get::<_,String>(0),"complete");assert_eq!(row.get::<_,i64>(1),2);
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.protocol_publication_commands WHERE scheduled_session_id=$1",&[&native]).await.unwrap().get::<_,i64>(0),2);
}

#[tokio::test]
#[ignore="requires isolated guarded PostgreSQL database"]
async fn scheduled_derivatives_count_toward_private_budget() {
    let image=replacement_png();let mut config=Config::default();config.scheduled_upload_bytes=image.len() as i64;
    let f=fixture_options(config,false).await;let session=f.begin(1).await;
    let uploaded=f.multipart(1,&session,&[("page.png","image/png",&image)],200).await;
    let native=f.native(ResourceKind::UploadSession,&session).await;
    let row=f.state.db.client().await.unwrap().query_one("SELECT (SELECT sum(o.bytes)::bigint FROM platform.upload_files x JOIN platform.media_objects o ON o.id=x.object_id WHERE x.session_id=$1),(SELECT sum(o.bytes)::bigint FROM platform.upload_files x JOIN platform.upload_file_derivatives d ON d.file_id=x.id JOIN platform.media_objects o ON o.id=d.object_id WHERE x.session_id=$1)",&[&native]).await.unwrap();
    assert!(row.get::<_,i64>(0)<=image.len() as i64);assert!(row.get::<_,i64>(1)>0);
    let mut payload=commit(uploaded["data"].as_array().unwrap());payload["chapterDraft"]["publishAt"]=json!(schedule_time(&f,3600).await);
    f.call(Some(1),"POST",&format!("/api/v1/upload/{session}/commit"),Some(payload),409).await;
    let row=f.state.db.client().await.unwrap().query_one("SELECT state,(SELECT count(*) FROM platform.scheduled_uploads),(SELECT count(*) FROM platform.protocol_publication_commands) FROM platform.upload_sessions WHERE id=$1",&[&native]).await.unwrap();
    assert_eq!(row.get::<_,String>(0),"open");assert_eq!(row.get::<_,i64>(1),0);assert_eq!(row.get::<_,i64>(2),0);
}

// Historical unsigned local chapters use the real catalogue writer and real staged
// media. No scheduled state, command, signed source, or publication result is mocked.
async fn local_chapter(f:&Fixture,id:&str,owner:&str)->(String,String) {
    use weebdex_platform::domains::catalog::{CatalogRepository,CatalogPage,ChapterPublication};
    let session=f.begin(1).await;let uploaded=f.multipart(1,&session,&[("old.png","image/png",&png())],200).await;
    let file=f.native(ResourceKind::UploadFile,uploaded["data"][0]["id"].as_str().unwrap()).await;
    let mut db=f.state.db.client().await.unwrap();let tx=db.transaction().await.unwrap();
    let row=tx.query_one("SELECT o.id,o.width,o.height FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id WHERE f.id=$1",&[&file]).await.unwrap();
    let name=format!("{file}.png");
    CatalogRepository::publish_chapter(&tx,&ChapterPublication{id:id.into(),manga_id:f.native_manga.clone(),uploader_id:owner.into(),uploader_name:owner.into(),title:Some("Historical local chapter".into()),volume:None,chapter:Some("1".into()),language:"en".into(),node:f.origin.clone(),pages:vec![CatalogPage{name:name.clone(),dimensions:[row.get(1),row.get(2)]}],optimized_pages:vec![],groups:vec![],source_id:None,protocol_event_id:None}).await.unwrap();
    tx.execute("INSERT INTO platform.chapter_pages(chapter_id,position,name,object_id) VALUES($1,0,$2,$3)",&[&id,&name,&row.get::<_,String>(0)]).await.unwrap();
    tx.commit().await.unwrap();drop(db);let public=PostgresPublicIdResolver::new(f.state.db.clone()).preferred(ResourceKind::Chapter,id).await.unwrap().unwrap().to_string();
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}"),None,200).await;
    (public,name)
}
async fn schedule_local_edit(f:&Fixture,chapter:&str,seconds:i64)->(String,String,String) {
    let opened=f.call(Some(1),"POST",&format!("/api/v1/upload/begin/{chapter}"),Some(json!({"version":1})),200).await;
    let session=opened["id"].as_str().unwrap().to_owned();let native=f.native(ResourceKind::UploadSession,&session).await;
    let mut db=f.state.db.client().await.unwrap();let tx=db.transaction().await.unwrap();
    let mut old=Vec::new();for row in tx.query("SELECT id FROM platform.upload_files WHERE session_id=$1",&[&native]).await.unwrap(){old.push(row.get::<_,String>(0));}tx.commit().await.unwrap();drop(db);
    for native_file in old {let file=PostgresPublicIdResolver::new(f.state.db.clone()).preferred(ResourceKind::UploadFile,&native_file).await.unwrap().unwrap();f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}/{file}"),None,200).await;}
    let files=f.multipart(1,&session,&[("replacement.png","image/png",&replacement_png())],200).await;
    let file=f.native(ResourceKind::UploadFile,files["data"][0]["id"].as_str().unwrap()).await;
    let mut payload=commit(files["data"].as_array().unwrap());payload["chapterDraft"]["publishAt"]=json!(schedule_time(f,seconds).await);
    f.call(Some(1),"POST",&format!("/api/v1/upload/{session}/commit"),Some(payload),202).await;
    (session,native,format!("{file}.png"))
}
async fn native_edit(f:&Fixture,chapter:&str,version:i32)->reqwest::Response {
    f.http.post(format!("{}/api/upload/begin/{chapter}",f.origin)).header("origin",&f.origin).header("cookie","weebdex_session=synthetic-public-upload-cookie").json(&json!({"version":version,"replace":false})).send().await.unwrap()
}
async fn media_response(f:&Fixture,chapter:&str,name:&str,status:u16)->Vec<u8> {
    let response=f.http.get(format!("{}/data/{chapter}/{name}",f.origin)).send().await.unwrap();assert_eq!(response.status().as_u16(),status);response.bytes().await.unwrap().to_vec()
}

#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="requires isolated guarded PostgreSQL database"]
async fn scheduled_local_edit_preserves_old_reader_and_native_lock_orders() {
    let f=fixture().await;
    for (id,native_first) in [("SchedEdit1",true),("SchedEdit2",false)] {
        let (chapter,old)=local_chapter(&f,id,"PublicUploadUser1").await;
        let (session,native,new)=schedule_local_edit(&f,&chapter,2).await;
        assert_eq!(media_response(&f,&chapter,&old,200).await,png());media_response(&f,&chapter,&new,404).await;
        let before=f.call(None,"GET",&format!("/api/v1/chapter/{chapter}"),None,200).await;assert_eq!(before["data"]["attributes"]["version"],1);
        await_schedule_due(&f,&native).await;
        // The durable-start gate releases its chapter lock before execution.
        // Pause on the existing due-only alias fence after that gate commits, so
        // the ordering below tests the execution transaction, not mere claiming.
        let mut barrier=f.state.db.client().await.unwrap();let alias_tx=barrier.transaction().await.unwrap();
        let barrier_pid:i32=alias_tx.query_one("SELECT pg_backend_pid()",&[]).await.unwrap().get(0);
        alias_tx.query_one("SELECT pg_advisory_xact_lock(hashtextextended($1,0))",&[&format!("platform-public-id:chapter:{id}")]).await.unwrap();
        let state=f.state.clone();let worker=tokio::spawn(async move{weebdex_platform::public_api::scheduled_uploads::run_due(&state).await});
        wait_blocked_on(&f,barrier_pid).await;
        assert!(f.state.db.client().await.unwrap().query_one("SELECT state='starting' AND started_at IS NOT NULL FROM platform.scheduled_uploads WHERE session_id=$1",&[&native]).await.unwrap().get::<_,bool>(0));
        let mut holder=f.state.db.client().await.unwrap();let tx=holder.transaction().await.unwrap();let pid:i32=tx.query_one("SELECT pg_backend_pid()",&[]).await.unwrap().get(0);
        tx.query_one("SELECT id FROM platform.catalog_chapters WHERE id=$1 FOR UPDATE",&[&id]).await.unwrap();
        let http=f.http.clone();let origin=f.origin.clone();let url=format!("{}/api/upload/begin/{id}",f.origin);
        let request=async move{http.post(url).header("origin",origin).header("cookie","weebdex_session=synthetic-public-upload-cookie").json(&json!({"version":1,"replace":false})).send().await.unwrap()};
        let native_task=if native_first {
            let native_task=tokio::spawn(request);wait_blocked_on(&f,pid).await;
            alias_tx.commit().await.unwrap();drop(barrier);native_task
        } else {
            alias_tx.commit().await.unwrap();drop(barrier);wait_blocked_on(&f,pid).await;
            tokio::spawn(request)
        };
        // Both contenders execute their real parent/chapter/session FK paths.
        // The first native request must acquire the parent's FK KEY SHARE while
        // the worker holds NO KEY UPDATE; the reverse order must fail stale CAS.
        wait_chapter_contenders(&f).await;tx.commit().await.unwrap();drop(holder);
        let response=tokio::time::timeout(Duration::from_secs(20),native_task).await.unwrap().unwrap();let status=response.status().as_u16();let body=response.text().await.unwrap();assert_eq!(status,if native_first{201}else{409},"{body}");
        assert!(tokio::time::timeout(Duration::from_secs(20),worker).await.unwrap().unwrap().unwrap());
        let after=f.call(None,"GET",&format!("/api/v1/chapter/{chapter}"),None,200).await;assert_eq!(after["data"]["attributes"]["version"],2);
        assert_eq!(media_response(&f,&chapter,&new,200).await,replacement_png());
        if native_first {
            let open=f.state.db.client().await.unwrap().query_one("SELECT id FROM platform.upload_sessions WHERE owner_id='PublicUploadUser1' AND state='open'",&[]).await.unwrap().get::<_,String>(0);
            assert!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.upload_files WHERE session_id=$1",&[&open]).await.unwrap().get::<_,i64>(0)>0);
            let response=f.http.delete(format!("{}/api/upload/{open}",f.origin)).header("origin",&f.origin).header("cookie","weebdex_session=synthetic-public-upload-cookie").send().await.unwrap();assert_eq!(response.status().as_u16(),204);
        }
        let account=schedule_account(&f,"PublicUploadUser1").await;let receipt=f.call(Some(1),"GET",&format!("/api/v1/nostr/uploads/{session}/schedule?expectedAccountId={account}"),None,200).await;assert_eq!(receipt["receipt"]["nostr"]["readerAvailable"],true);
    }
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.protocol_publication_commands",&[]).await.unwrap().get::<_,i64>(0),0);
    let (chapter,old)=local_chapter(&f,"SchedEdit3","PublicUploadUser1").await;let (_,native,new)=schedule_local_edit(&f,&chapter,2).await;
    f.call(Some(1),"PUT",&format!("/api/v1/chapter/{chapter}"),Some(json!({"version":1,"title":"Concurrent actual edit"})),200).await;
    await_schedule_due(&f,&native).await;assert!(weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.is_err());
    assert_eq!(media_response(&f,&chapter,&old,200).await,png());media_response(&f,&chapter,&new,404).await;
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT state FROM platform.scheduled_uploads WHERE session_id=$1",&[&native]).await.unwrap().get::<_,String>(0),"blocked");
}
async fn wait_retry_due(f:&Fixture,id:&str) {
    let deadline=tokio::time::Instant::now()+Duration::from_secs(5);
    loop {if f.state.db.client().await.unwrap().query_one("SELECT next_attempt_at<=clock_timestamp() FROM platform.scheduled_uploads WHERE session_id=$1",&[&id]).await.unwrap().get::<_,bool>(0){return}assert!(tokio::time::Instant::now()<deadline,"retry deadline did not arrive");tokio::time::sleep(Duration::from_millis(25)).await;}
}
async fn wait_chapter_contenders(f:&Fixture) {
    let deadline=tokio::time::Instant::now()+Duration::from_secs(5);
    loop {let count:i64=f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE '%platform.catalog_chapters%'",&[]).await.unwrap().get(0);if count>=2{return}assert!(tokio::time::Instant::now()<deadline,"both real chapter contenders did not reach row locks");tokio::time::sleep(Duration::from_millis(20)).await;}
}
async fn retained_snapshot(f:&Fixture,id:&str)->Value {
    f.state.db.client().await.unwrap().query_one("SELECT jsonb_build_object('session',(SELECT to_jsonb(s) FROM platform.upload_sessions s WHERE id=$1),'files',(SELECT jsonb_agg(to_jsonb(f) ORDER BY f.id) FROM platform.upload_files f WHERE session_id=$1),'objects',(SELECT jsonb_agg(id ORDER BY id) FROM platform.media_objects),'cleanup',(SELECT jsonb_agg(storage_key ORDER BY storage_key) FROM platform.media_cleanup),'pins',(SELECT jsonb_agg(object_id ORDER BY object_id) FROM platform.protocol_publication_originals))",&[&id]).await.unwrap().get(0)
}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="requires isolated guarded PostgreSQL database"]
async fn native_begin_edit_expiry_lock_order_and_authorization_rollback() {
    let f=fixture().await;let (chapter,old)=local_chapter(&f,"ExpiryOwn1","PublicUploadUser1").await;local_chapter(&f,"ExpiryOth1","PublicUploadUser2").await;
    let session=f.begin(1).await;f.multipart(1,&session,&[("uncommitted.png","image/png",&replacement_png())],200).await;
    let native=f.native(ResourceKind::UploadSession,&session).await;
    f.state.db.client().await.unwrap().execute("UPDATE platform.upload_sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE id=$1",&[&native]).await.unwrap();
    let before=retained_snapshot(&f,&native).await;
    for (target,version,status) in [("ExpiryOwn1",999,409),("ExpiryOth1",1,403)] {
        let response=native_edit(&f,target,version).await;let actual=response.status().as_u16();let text=response.text().await.unwrap();assert_eq!(actual,status,"{text}");assert_eq!(retained_snapshot(&f,&native).await,before,"failed authorization/version must roll back expiry, files, pins and cleanup queue");
    }
    let mut holder=f.state.db.client().await.unwrap();let tx=holder.transaction().await.unwrap();let pid:i32=tx.query_one("SELECT pg_backend_pid()",&[]).await.unwrap().get(0);
    tx.query_one("SELECT id FROM platform.upload_sessions WHERE id=$1 FOR UPDATE",&[&native]).await.unwrap();
    let http=f.http.clone();let origin=f.origin.clone();let url=format!("{}/api/upload/begin/ExpiryOwn1",f.origin);
    let pending=tokio::spawn(async move{http.post(url).header("origin",origin).header("cookie","weebdex_session=synthetic-public-upload-cookie").json(&json!({"version":1,"replace":false})).send().await.unwrap()});wait_blocked_on(&f,pid).await;
    // While real begin_edit is blocked expiring a media-bearing OPEN session,
    // it must not already own the chapter lock. NOWAIT proves the revised order.
    tx.query_one("SELECT id FROM platform.catalog_chapters WHERE id='ExpiryOwn1' FOR UPDATE NOWAIT",&[]).await.unwrap();tx.commit().await.unwrap();drop(holder);
    let response=tokio::time::timeout(Duration::from_secs(20),pending).await.unwrap().unwrap();let status=response.status().as_u16();let text=response.text().await.unwrap();assert_eq!(status,201,"{text}");
    let row=f.state.db.client().await.unwrap().query_one("SELECT state,(SELECT count(*) FROM platform.upload_files WHERE session_id=$1) FROM platform.upload_sessions WHERE id=$1",&[&native]).await.unwrap();assert_eq!(row.get::<_,String>(0),"deleted");assert_eq!(row.get::<_,i64>(1),0);
    assert_eq!(media_response(&f,&chapter,&old,200).await,png());
}
async fn new_fixture_parent(f:&mut Fixture,id:&str) {
    // Distinct native parents must have distinct source manifests even when
    // signed in the same second; identical originals correctly share an event.
    let title=format!("Independent fixture parent {id}");
    f.state.db.client().await.unwrap().execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES($1,'PublicUploadUser1','Synthetic uploader',$2,'en','ongoing','safe','published')",&[&id,&title]).await.unwrap();
    f.native_manga=id.to_owned();f.manga=PostgresPublicIdResolver::new(f.state.db.clone()).preferred(ResourceKind::Manga,id).await.unwrap().unwrap().to_string();
}
#[tokio::test]
#[ignore="requires isolated guarded PostgreSQL database"]
async fn scheduled_command_provenance_accepted_pins_and_legacy_refusal() {
    let mut limits=Config::default();limits.scheduled_upload_maximum=2;let mut f=fixture_options(limits,true).await;
    let a=scheduled_upload(&f,&replacement_png(),2).await;let b=scheduled_upload(&f,&png(),2).await;
    install_acceptance_fault(&f,"chapter").await;await_schedule_due(&f,&a.1).await;await_schedule_due(&f,&b.1).await;
    assert_eq!(weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap_err().status.as_u16(),503);
    let owner=f.state.db.client().await.unwrap().query_one("SELECT session_id FROM platform.scheduled_uploads WHERE state='blocked'",&[]).await.unwrap().get::<_,String>(0);
    let (accepted,other)=if owner==a.1{(&a,&b)}else{assert_eq!(owner,b.1);(&b,&a)};
    let commands=f.state.db.client().await.unwrap().query("SELECT command_id,event_id,scheduled_session_id,state FROM platform.protocol_publication_commands ORDER BY command_id",&[]).await.unwrap();assert_eq!(commands.len(),2);for row in &commands{assert_eq!(row.get::<_,Option<String>>(2).as_deref(),Some(owner.as_str()));}
    let events:Vec<(String,String)>=commands.iter().map(|r|(r.get(0),r.get::<_,Option<String>>(1).unwrap())).collect();
    // Same owner and identical source-less parent metadata do not confer ownership
    // of another schedule's parent command. This second intent cannot adopt it.
    assert_eq!(weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap_err().status.as_u16(),409);
    let row=f.state.db.client().await.unwrap().query_one("SELECT state,started_at IS NULL FROM platform.scheduled_uploads WHERE session_id=$1",&[&other.1]).await.unwrap();assert_eq!(row.get::<_,String>(0),"blocked");assert!(row.get::<_,bool>(1));
    let open=f.begin(1).await;let files=f.multipart(1,&open,&[("quota.jpg","image/jpeg",&jpeg())],200).await;let mut payload=commit(files["data"].as_array().unwrap());payload["chapterDraft"]["publishAt"]=json!(schedule_time(&f,60).await);
    f.call(Some(1),"POST",&format!("/api/v1/upload/{open}/commit"),Some(payload),409).await;f.call(Some(1),"DELETE",&format!("/api/v1/upload/{open}"),None,200).await;
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{}",accepted.0),None,409).await;
    f.state.db.client().await.unwrap().execute("UPDATE platform.upload_sessions SET expires_at=clock_timestamp()-interval '1 day' WHERE id=$1",&[&owner]).await.unwrap();
    let before=retained_snapshot(&f,&owner).await;assert!(!before["pins"].as_array().unwrap().is_empty());weebdex_platform::domains::uploads::recover_storage(&f.state).await;assert_eq!(retained_snapshot(&f,&owner).await,before);
    // Negative corruption control: the accepted source does not excuse a different
    // immutable stored manifest. Restore exact original bytes only after refusal.
    remove_acceptance_fault(&f).await;
    let parent=format!("manga:{}",f.native_manga);let manifest=f.state.db.client().await.unwrap().query_one("SELECT source_manifest FROM platform.protocol_publication_commands WHERE command_id=$1",&[&parent]).await.unwrap().get::<_,Vec<u8>>(0);let mut corrupt=manifest.clone();corrupt.push(b' ');
    f.state.db.client().await.unwrap().execute("UPDATE platform.protocol_publication_commands SET source_manifest=$2 WHERE command_id=$1",&[&parent,&corrupt]).await.unwrap();
    owner_schedule_retry(&f,&accepted.0).await;assert_eq!(weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap_err().status.as_u16(),409);
    f.state.db.client().await.unwrap().execute("UPDATE platform.protocol_publication_commands SET source_manifest=$2 WHERE command_id=$1",&[&parent,&manifest]).await.unwrap();
    owner_schedule_retry(&f,&accepted.0).await;assert!(weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap());
    let recovered:Vec<(String,String)>=f.state.db.client().await.unwrap().query("SELECT command_id,event_id FROM platform.protocol_publication_commands ORDER BY command_id",&[]).await.unwrap().iter().map(|r|(r.get(0),r.get::<_,Option<String>>(1).unwrap())).collect();assert_eq!(recovered,events,"accepted parent and chapter must recover their original events without signing replacements");
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{}",other.0),None,200).await;
    // A legacy native parent-only command cannot be adopted even before any
    // chapter command exists. Exercise the admission refusal, not only the
    // ordinary immutable-upload-command refusal used by the next control.
    new_fixture_parent(&mut f,"LegacyPar1").await;
    let legacy_parent=f.begin(1).await;let parent_files=f.multipart(1,&legacy_parent,&[("parent-only.png","image/png",&png())],200).await;
    install_acceptance_fault(&f,"manga").await;
    f.call(Some(1),"POST",&format!("/api/v1/upload/{legacy_parent}/commit"),Some(commit(parent_files["data"].as_array().unwrap())),503).await;
    let legacy_native=f.native(ResourceKind::UploadSession,&legacy_parent).await;
    let row=f.state.db.client().await.unwrap().query_one("SELECT scheduled_session_id,(SELECT count(*) FROM platform.protocol_publication_commands WHERE upload_session_id=$1) FROM platform.protocol_publication_commands WHERE command_id='manga:LegacyPar1'",&[&legacy_native]).await.unwrap();assert!(row.get::<_,Option<String>>(0).is_none());assert_eq!(row.get::<_,i64>(1),0);
    let mut future=commit(parent_files["data"].as_array().unwrap());future["chapterDraft"]["publishAt"]=json!(schedule_time(&f,60).await);f.call(Some(1),"POST",&format!("/api/v1/upload/{legacy_parent}/commit"),Some(future),409).await;
    remove_acceptance_fault(&f).await;f.call(Some(1),"POST",&format!("/api/v1/upload/{legacy_parent}/commit"),Some(commit(parent_files["data"].as_array().unwrap())),200).await;
    // Legacy NULL provenance stays native. Actual source acceptance and a failed
    // acknowledgement leave a real OPEN session with source-pinned page bytes.
    new_fixture_parent(&mut f,"LegacyPar2").await;local_chapter(&f,"LegacyOld2","PublicUploadUser1").await;
    let legacy=f.begin(1).await;let files=f.multipart(1,&legacy,&[("native.jpg","image/jpeg",&jpeg())],200).await;let native=f.native(ResourceKind::UploadSession,&legacy).await;
    install_acceptance_fault(&f,"chapter").await;
    f.call(Some(1),"POST",&format!("/api/v1/upload/{legacy}/commit"),Some(commit(files["data"].as_array().unwrap())),503).await;
    let row=f.state.db.client().await.unwrap().query_one("SELECT scheduled_session_id,state FROM platform.protocol_publication_commands WHERE upload_session_id=$1",&[&native]).await.unwrap();assert!(row.get::<_,Option<String>>(0).is_none());assert_eq!(row.get::<_,String>(1),"prepared");
    let mut future=commit(files["data"].as_array().unwrap());future["chapterDraft"]["publishAt"]=json!(schedule_time(&f,60).await);f.call(Some(1),"POST",&format!("/api/v1/upload/{legacy}/commit"),Some(future),409).await;
    f.state.db.client().await.unwrap().execute("UPDATE platform.upload_sessions SET expires_at=clock_timestamp()-interval '1 day' WHERE id=$1",&[&native]).await.unwrap();let before=retained_snapshot(&f,&native).await;
    assert_eq!(native_edit(&f,"LegacyOld2",1).await.status().as_u16(),409);weebdex_platform::domains::uploads::recover_storage(&f.state).await;assert_eq!(retained_snapshot(&f,&native).await,before);
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.scheduled_uploads WHERE session_id=$1",&[&native]).await.unwrap().get::<_,i64>(0),0);
}
#[tokio::test]
#[ignore="requires isolated guarded PostgreSQL database"]
async fn scheduled_unknown_prepared_source_is_never_replayed_or_purged() {
    let mut limits=Config::default();limits.scheduled_upload_maximum=1;let f=fixture_options(limits,true).await;
    let (session,native,_,_)=scheduled_upload(&f,&replacement_png(),2).await;
    // Fail the actual original transaction after durable platform preparation,
    // before any original chapter acceptance. This is not a forged command row.
    f.state.db.client().await.unwrap().batch_execute("CREATE FUNCTION public.scheduled_original_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.kind=1001 THEN RAISE EXCEPTION 'synthetic original admission failure' USING ERRCODE='58000'; END IF; RETURN NEW; END $$; CREATE TRIGGER scheduled_original_fault BEFORE INSERT ON public.events FOR EACH ROW EXECUTE FUNCTION public.scheduled_original_fault()").await.unwrap();
    await_schedule_due(&f,&native).await;assert_eq!(weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap_err().status.as_u16(),503);
    let row=f.state.db.client().await.unwrap().query_one("SELECT c.event_id,c.state,EXISTS(SELECT 1 FROM public.events e WHERE e.id=decode(c.event_id,'hex')) FROM platform.protocol_publication_commands c WHERE upload_session_id=$1",&[&native]).await.unwrap();let event=row.get::<_,String>(0);assert_eq!(row.get::<_,String>(1),"prepared");assert!(!row.get::<_,bool>(2));
    f.state.db.client().await.unwrap().batch_execute("DROP TRIGGER scheduled_original_fault ON public.events; DROP FUNCTION public.scheduled_original_fault()").await.unwrap();
    let hash=hex::encode(Sha256::digest(replacement_png()));original_get(&f,&hash,reqwest::Method::GET,true,404).await;
    owner_schedule_retry(&f,&session).await;let failure=weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap_err();assert_eq!(failure.status.as_u16(),409);assert!(failure.detail.contains("uncertain"),"{}",failure.detail);
    assert!(!weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap());f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}"),None,409).await;
    f.state.db.client().await.unwrap().execute("UPDATE platform.upload_sessions SET expires_at=clock_timestamp()-interval '1 day' WHERE id=$1",&[&native]).await.unwrap();let before=retained_snapshot(&f,&native).await;assert!(!before["pins"].as_array().unwrap().is_empty());domains::uploads::recover_storage(&f.state).await;assert_eq!(retained_snapshot(&f,&native).await,before);
    let row=f.state.db.client().await.unwrap().query_one("SELECT c.event_id,c.state,EXISTS(SELECT 1 FROM public.events e WHERE e.id=decode(c.event_id,'hex')) FROM platform.protocol_publication_commands c WHERE upload_session_id=$1",&[&native]).await.unwrap();assert_eq!(row.get::<_,String>(0),event);assert_eq!(row.get::<_,String>(1),"prepared");assert!(!row.get::<_,bool>(2));
    original_get(&f,&hash,reqwest::Method::HEAD,true,404).await;
    let open=f.begin(1).await;let files=f.multipart(1,&open,&[("next.png","image/png",&png())],200).await;let mut payload=commit(files["data"].as_array().unwrap());payload["chapterDraft"]["publishAt"]=json!(schedule_time(&f,60).await);f.call(Some(1),"POST",&format!("/api/v1/upload/{open}/commit"),Some(payload),409).await;
}
async fn upload_fixture_cover(f:&Fixture,bytes:&[u8],mime:&str,name:&str)->Value {
    let version=f.state.db.client().await.unwrap().query_one("SELECT version FROM platform.catalog_manga WHERE id=$1",&[&f.native_manga]).await.unwrap().get::<_,i32>(0).to_string();let account=schedule_account(f,"PublicUploadUser1").await;
    let (content_type,body)=replacement_multipart(&[("expectedVersion",None,None,version.as_bytes()),("expectedAccountId",None,None,account.as_bytes()),("main",None,None,b"true"),("file",Some(name),Some(mime),bytes)]);
    let path=format!("/api/v1/cover/{}",f.manga);let token=proof(1,11,&format!("{}{path}",f.origin),"POST",&body);
    let response=f.http.post(format!("{}{path}",f.origin)).header("content-type",content_type).header("authorization",token).body(body).send().await.unwrap();let status=response.status().as_u16();let text=response.text().await.unwrap();assert_eq!(status,201,"{text}");serde_json::from_str(&text).unwrap()
}
#[tokio::test]
#[ignore="requires isolated guarded PostgreSQL database"]
async fn scheduled_parent_cover_removal_replacement_and_metadata_edit_block() {
    let f=fixture().await;
    f.state.db.client().await.unwrap().batch_execute("UPDATE platform.users SET permissions=permissions||'[\"manga.cover.edit\",\"manga.update\",\"manga.title.edit\"]'::jsonb WHERE id='PublicUploadUser1'; UPDATE platform.users SET roles='[\"moderator\"]',permissions=permissions||'[\"cover.delete\"]'::jsonb WHERE id='PublicUploadUser2'").await.unwrap();
    let cover=upload_fixture_cover(&f,&jpeg(),"image/jpeg","old.jpg").await;let cover_id=cover["data"]["id"].as_str().unwrap();let native_cover=f.native(ResourceKind::Cover,cover_id).await;
    let (_,native,_,_)=scheduled_upload(&f,&png(),2).await;
    let row=f.state.db.client().await.unwrap().query_one("SELECT parent_cover_id,retained_bytes,(SELECT sum(o.bytes)::bigint FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id WHERE f.session_id=i.session_id) FROM platform.scheduled_uploads i WHERE session_id=$1",&[&native]).await.unwrap();assert_eq!(row.get::<_,String>(0),native_cover);assert!(row.get::<_,i64>(1)>=row.get::<_,i64>(2)+jpeg().len() as i64,"parent cover bytes must be reserved in the scheduled pool");
    let account=schedule_account(&f,"PublicUploadUser2").await;let path=format!("/api/v1/cover/{cover_id}");let body=serde_json::to_vec(&json!({"version":cover["data"]["attributes"]["version"],"expectedAccountId":account})).unwrap();
    let response=f.http.delete(format!("{}{path}",f.origin)).header("content-type","application/json").header("authorization",proof(2,11,&format!("{}{path}",f.origin),"DELETE",&body)).body(body).send().await.unwrap();let status=response.status().as_u16();let text=response.text().await.unwrap();assert_eq!(status,204,"{text}");
    domains::uploads::recover_storage(&f.state).await;await_schedule_due(&f,&native).await;assert_eq!(weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap_err().status.as_u16(),409);
    // Reservation is not a physical cover reference pin. Removal is allowed by
    // normal GC, but the saved schedule must block, never select another cover.
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.cover_media WHERE cover_id=$1",&[&native_cover]).await.unwrap().get::<_,i64>(0),0);
    upload_fixture_cover(&f,&png(),"image/png","first.png").await;let (_,second,_,_)=scheduled_upload(&f,&png(),2).await;
    upload_fixture_cover(&f,&jpeg(),"image/jpeg","replacement.jpg").await;await_schedule_due(&f,&second).await;assert_eq!(weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap_err().status.as_u16(),409);
    let (_,third,_,_)=scheduled_upload(&f,&png(),2).await;let version=f.state.db.client().await.unwrap().query_one("SELECT version FROM platform.catalog_manga WHERE id=$1",&[&f.native_manga]).await.unwrap().get::<_,i32>(0);
    let response=f.http.put(format!("{}/api/manga/{}",f.origin,f.native_manga)).header("origin",&f.origin).header("cookie","weebdex_session=synthetic-public-upload-cookie").json(&json!({"version":version,"title":"Unrelated real parent metadata edit"})).send().await.unwrap();let status=response.status().as_u16();let text=response.text().await.unwrap();assert_eq!(status,200,"{text}");
    await_schedule_due(&f,&third).await;assert_eq!(weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap_err().status.as_u16(),409);
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.protocol_publication_commands",&[]).await.unwrap().get::<_,i64>(0),0);
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.scheduled_uploads WHERE state='blocked' AND started_at IS NULL",&[]).await.unwrap().get::<_,i64>(0),3);
}

#[path="support/account_closure_schedules.rs"]
mod account_closure_schedules;

// Authored local-edit controls. Execution requires the existing isolated fixture
// guard; these are not evidence of a run merely because this source is present.
#[tokio::test]
#[ignore="requires isolated guarded PostgreSQL database"]
async fn signed_local_page_edit_modes_preserve_originals_until_atomic_commit() {
    let f=fixture().await;let (chapter,old)=local_chapter(&f,"LocalEdit1","PublicUploadUser1").await;
    let account=schedule_account(&f,"PublicUploadUser1").await;
    // SQL projection wins over imported, attacker-controlled metadata and is not
    // a publicly writable authority field.
    f.state.db.client().await.unwrap().execute("UPDATE platform.catalog_chapters SET metadata=metadata || '{\"_local_page_source_mutable\":false,\"localPageSource\":\"protocol\",\"nostr\":{\"eventId\":\"forged\"}}'::jsonb WHERE id='LocalEdit1'",&[]).await.unwrap();
    let detail=f.call(None,"GET",&format!("/api/v1/chapter/{chapter}"),None,200).await;
    assert_eq!(detail["data"]["attributes"]["localPageSource"],"local");
    f.call(Some(1),"PUT",&format!("/api/v1/chapter/{chapter}"),Some(json!({"version":1,"localPageSource":"local","expectedAccountId":account})),400).await;
    for replace in [None,Some(false),Some(true)] {
        let mut payload=json!({"version":1,"expectedAccountId":account});if let Some(replace)=replace{payload["replace"]=json!(replace);}
        let opened=f.call(Some(1),"POST",&format!("/api/v1/upload/begin/{chapter}"),Some(payload),200).await;
        assert_eq!(opened["attributes"]["edit"],json!({"chapterId":chapter,"version":1,"replace":replace.unwrap_or(false)}));
        let session=opened["id"].as_str().unwrap();let snapshot=f.call(Some(1),"GET",&format!("/api/v1/nostr/uploads/{session}"),None,200).await;
        assert_eq!(snapshot["session"]["chapterId"],chapter);assert_eq!(snapshot["session"]["editVersion"],1);
        assert_eq!(snapshot["session"]["files"].as_array().unwrap().len(),if replace==Some(true){0}else{1});
        if replace!=Some(true){signed_preview(&f,1,snapshot["session"]["files"][0]["previewUrl"].as_str().unwrap(),&png()).await;}
        assert_eq!(media_response(&f,&chapter,&old,200).await,png());
        f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}?expectedAccountId={account}"),None,200).await;
        assert_eq!(media_response(&f,&chapter,&old,200).await,png());
    }
    // Native cookie route exercises the same core, while its transport remains
    // distinct from signed public admission.
    for replace in [false,true] {
        let response=f.http.post(format!("{}/api/upload/begin/LocalEdit1",f.origin)).header("origin",&f.origin).header("cookie","weebdex_session=synthetic-public-upload-cookie").json(&json!({"version":1,"replace":replace})).send().await.unwrap();assert_eq!(response.status().as_u16(),201);
        let body:Value=response.json().await.unwrap();let native=body["id"].as_str().unwrap();
        let session=PostgresPublicIdResolver::new(f.state.db.clone()).preferred(ResourceKind::UploadSession,native).await.unwrap().unwrap().to_string();
        let snapshot=f.call(Some(1),"GET",&format!("/api/v1/nostr/uploads/{session}"),None,200).await;
        assert_eq!(snapshot["session"]["editVersion"],1);assert_eq!(snapshot["session"]["files"].as_array().unwrap().len(),if replace{0}else{1});
        f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}"),None,200).await;
    }
    for (key,version,status) in [(1,2,409),(2,1,403),(4,1,403)] {f.call(Some(key),"POST",&format!("/api/v1/upload/begin/{chapter}"),Some(json!({"version":version,"replace":true})),status).await;}
    for replace in [Value::Null,json!("true"),json!(1),json!({})] {f.call(Some(1),"POST",&format!("/api/v1/upload/begin/{chapter}"),Some(json!({"version":1,"replace":replace})),400).await;}
    assert_eq!(media_response(&f,&chapter,&old,200).await,png());
    let opened=f.call(Some(1),"POST",&format!("/api/v1/upload/begin/{chapter}"),Some(json!({"version":1,"replace":true,"expectedAccountId":account})),200).await;
    let session=opened["id"].as_str().unwrap();let bytes=replacement_png();let files=f.multipart(1,session,&[("replacement.png","image/png",&bytes)],200).await;
    assert_eq!(media_response(&f,&chapter,&old,200).await,png());
    let permissions:Value=f.state.db.client().await.unwrap().query_one("SELECT permissions FROM platform.users WHERE id='PublicUploadUser1'",&[]).await.unwrap().get(0);
    f.state.db.client().await.unwrap().execute("UPDATE platform.users SET permissions='[]'::jsonb WHERE id='PublicUploadUser1'",&[]).await.unwrap();
    f.call(Some(1),"POST",&format!("/api/v1/upload/{session}/commit"),Some(commit(files["data"].as_array().unwrap())),403).await;
    assert_eq!(media_response(&f,&chapter,&old,200).await,png());
    f.state.db.client().await.unwrap().execute("UPDATE platform.users SET permissions=$1 WHERE id='PublicUploadUser1'",&[&permissions]).await.unwrap();
    let committed=f.call(Some(1),"POST",&format!("/api/v1/upload/{session}/commit"),Some(commit(files["data"].as_array().unwrap())),200).await;
    assert_eq!(committed["id"],chapter);assert_eq!(committed["attributes"]["version"],2);
    let native_file=f.native(ResourceKind::UploadFile,files["data"][0]["id"].as_str().unwrap()).await;
    assert_eq!(media_response(&f,&chapter,&format!("{native_file}.png"),200).await,bytes);
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.protocol_publication_commands",&[]).await.unwrap().get::<_,i64>(0),0);
}

#[tokio::test]
#[ignore="requires isolated guarded PostgreSQL database and existing STRFRY_BIN"]
async fn signed_original_page_edits_refuse_both_modes_despite_forged_local_metadata() {
    let f=fixture_full(Config::default(),true,true).await;let session=f.begin(1).await;
    let files=f.multipart(1,&session,&[("original.png","image/png",&png())],200).await;
    let receipt=f.call(Some(1),"POST",&format!("/api/v1/upload/{session}/commit"),Some(commit(files["data"].as_array().unwrap())),200).await;
    let chapter=receipt["id"].as_str().unwrap();let native=f.native(ResourceKind::Chapter,chapter).await;
    let db=f.state.db.client().await.unwrap();let before:Value=db.query_one("SELECT to_jsonb(c) FROM platform.catalog_chapters c WHERE id=$1",&[&native]).await.unwrap().get(0);
    assert!(before["protocol_event_id"].is_string());
    db.execute("UPDATE platform.catalog_chapters SET metadata=metadata || '{\"_local_page_source_mutable\":true,\"localPageSource\":\"local\"}'::jsonb WHERE id=$1",&[&native]).await.unwrap();drop(db);
    let detail=f.call(None,"GET",&format!("/api/v1/chapter/{chapter}"),None,200).await;assert_eq!(detail["data"]["attributes"]["localPageSource"],"protocol");
    let version=detail["data"]["attributes"]["version"].as_i64().unwrap();
    for replace in [false,true] {f.call(Some(1),"POST",&format!("/api/v1/upload/begin/{chapter}"),Some(json!({"version":version,"replace":replace})),409).await;}
    let after:Value=f.state.db.client().await.unwrap().query_one("SELECT to_jsonb(c) FROM platform.catalog_chapters c WHERE id=$1",&[&native]).await.unwrap().get(0);
    for key in ["protocol_event_id","version","data","updated_at"]{assert_eq!(after[key],before[key]);}
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.upload_sessions WHERE state='open'",&[]).await.unwrap().get::<_,i64>(0),0);
}

// Explicit native scheduling uses an already operator-provisioned account role;
// it never asks the caller for a signer or manufactures an HTTP capability.
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="owned isolated PostgreSQL and configured synthetic publication runtime; run alone"]
async fn native_cookie_schedule_captures_configured_authority_without_user_signature() {
    let f=fixture_full(Config::default(),true,false).await;
    let account=schedule_account(&f,"PublicUploadUser1").await;
    async fn native(f:&Fixture,method:&str,path:&str,body:Vec<u8>,mime:&str,status:u16)->Value {
        let response=f.http.request(method.parse().unwrap(),format!("{}{path}",f.origin))
            .header("origin",&f.origin).header("cookie","weebdex_session=synthetic-public-upload-cookie")
            .header("x-weebdex-expected-account","PublicUploadUser1").header("content-type",mime).body(body).send().await.unwrap();
        let actual=response.status().as_u16();assert_eq!(response.headers()["cache-control"],"private, no-store");
        let value:Value=response.json().await.unwrap();assert_eq!(actual,status,"{method} {path}: {value}");
        if status<300 {assert_eq!(value["nostr"]["authentication"],"native-session");assert_eq!(value["nostr"]["scope"],"instance-account");}
        value
    }
    let path=|suffix:&str|format!("/api/v1/account/{suffix}?expectedAccountId={account}");
    let before=f.state.db.client().await.unwrap().query_one("SELECT (SELECT count(*) FROM platform.protocol_identity_bindings),(SELECT count(*) FROM account_permissions),(SELECT count(*) FROM platform.protocol_publication_commands)",&[]).await.unwrap();
    let start=native(&f,"POST",&path("upload/begin"),serde_json::to_vec(&json!({"manga":f.manga,"groups":[],"expectedAccountId":account})).unwrap(),"application/json",200).await;
    let session=start["id"].as_str().unwrap();
    let (mime,bytes)=multipart(&[("scheduled.png","image/png",&png())]);
    let files=native(&f,"POST",&path(&format!("upload/{session}")),bytes,&mime,200).await;
    let mut payload=commit(files["data"].as_array().unwrap());payload["chapterDraft"]["publishAt"]=json!(schedule_time(&f,3600).await);
    let commit_path=path(&format!("upload/{session}/commit"));let bytes=serde_json::to_vec(&payload).unwrap();
    let receipt=native(&f,"POST",&commit_path,bytes.clone(),"application/json",202).await;
    assert_eq!(receipt["nostr"]["scheduleAccepted"],true);assert_eq!(receipt["nostr"]["sourceAccepted"],false);
    let replay=native(&f,"POST",&commit_path,bytes,"application/json",202).await;assert_eq!(replay["id"],receipt["id"]);
    let list=native(&f,"GET",&path("uploads/schedules"),vec![],"application/json",200).await;assert_eq!(list["data"].as_array().unwrap().len(),1);
    native(&f,"GET",&path(&format!("uploads/{session}/schedule")),vec![],"application/json",200).await;
    // Source admission revocation is authoritative even though native login
    // remains valid; failed requests create no alternate binding or permission.
    f.repository.set_account_admission(id(1),false).await.unwrap();
    native(&f,"POST",&commit_path,serde_json::to_vec(&payload).unwrap(),"application/json",403).await;
    let native_id=f.native(ResourceKind::UploadSession,session).await;
    let db=f.state.db.client().await.unwrap();
    let after=db.query_one("SELECT (SELECT count(*) FROM platform.protocol_identity_bindings),(SELECT count(*) FROM account_permissions),(SELECT count(*) FROM platform.protocol_publication_commands)",&[]).await.unwrap();
    for index in 0..3 {assert_eq!(after.get::<_,i64>(index),before.get::<_,i64>(index));}
    let capture=db.query_one("SELECT author,service,owner_id,payload_hash,state FROM platform.scheduled_uploads WHERE session_id=$1",&[&native_id]).await.unwrap();
    assert_eq!(capture.get::<_,String>(0),public(1));assert_eq!(capture.get::<_,String>(1),public(11));assert_eq!(capture.get::<_,String>(2),"PublicUploadUser1");assert_eq!(capture.get::<_,String>(4),"scheduled");
    drop(db);
    // Owner cancellation remains available after publication admission is
    // revoked, provided no source attempt has started. It cannot impersonate one.
    native(&f,"DELETE",&path(&format!("upload/{session}")),vec![],"application/json",200).await;
    let db=f.state.db.client().await.unwrap();assert_eq!(db.query_one("SELECT state FROM platform.scheduled_uploads WHERE session_id=$1",&[&native_id]).await.unwrap().get::<_,String>(0),"cancelled");
    db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES('native-other-upload','PublicUploadUser2',$1,clock_timestamp()+interval '1 hour')",&[&Sha256::digest(b"synthetic-other-native-cookie").to_vec()]).await.unwrap();drop(db);
    async fn other(f:&Fixture,method:&str,path:&str,body:Vec<u8>,mime:&str,status:u16)->Value {
        let response=f.http.request(method.parse().unwrap(),format!("{}{path}",f.origin)).header("origin",&f.origin).header("cookie","weebdex_session=synthetic-other-native-cookie").header("x-weebdex-expected-account","PublicUploadUser2").header("content-type",mime).body(body).send().await.unwrap();let actual=response.status().as_u16();let value:Value=response.json().await.unwrap();assert_eq!(actual,status,"{value}");value
    }
    let other_account=schedule_account(&f,"PublicUploadUser2").await;
    let other_path=|suffix:&str|format!("/api/v1/account/{suffix}?expectedAccountId={other_account}");
    let begin=other(&f,"POST",&other_path("upload/begin"),serde_json::to_vec(&json!({"manga":f.manga,"groups":[],"expectedAccountId":other_account})).unwrap(),"application/json",200).await;let other_session=begin["id"].as_str().unwrap();
    let (mime,bytes)=multipart(&[("ordinary.png","image/png",&png())]);let files=other(&f,"POST",&other_path(&format!("upload/{other_session}")),bytes,&mime,200).await;
    let denied=other(&f,"POST",&other_path(&format!("upload/{other_session}/commit")),serde_json::to_vec(&commit(files["data"].as_array().unwrap())).unwrap(),"application/json",403).await;
    assert!(denied["errors"][0]["detail"].is_string());
    let kept=other(&f,"GET",&other_path(&format!("uploads/{other_session}")),vec![],"application/json",200).await;assert_eq!(kept["session"]["state"],"open");assert_eq!(kept["session"]["files"].as_array().unwrap().len(),1);
    other(&f,"DELETE",&other_path(&format!("upload/{other_session}")),vec![],"application/json",200).await;
}

// Actual native-cookie HTTP with deliberately provisioned synthetic publication
// authority. No user account, external destination, or automatic grant is used.
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Owned disposable PostgreSQL fixture; run alone"]
async fn native_upload_preflight_and_configured_completion_are_current_and_read_only() {
    let f=fixture().await;
    let account=schedule_account(&f,"PublicUploadUser1").await;
    let path=format!("/api/v1/account/upload/preflight/{}?expectedAccountId={account}",f.manga);
    let native=|method:&str,path:&str|f.http.request(method.parse().unwrap(),format!("{}{path}",f.origin)).header("cookie",format!("{}=synthetic-public-upload-cookie",f.state.config.session_cookie_name)).header("x-weebdex-expected-account","PublicUploadUser1").header("origin",&f.origin);
    let before:i64=f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.protocol_publication_commands",&[]).await.unwrap().get(0);
    let response=native("GET",&path).send().await.unwrap();assert_eq!(response.status().as_u16(),200);let first:Value=response.json().await.unwrap();
    assert_eq!(first["preflight"]["eligible"],true);assert_eq!(first["preflight"]["configured"],true);
    assert_eq!(first["preflight"]["formats"],json!(["image/jpeg","image/png","image/webp"]));
    assert_eq!(first["preflight"]["limits"]["maxUniqueBytes"],256*1024*1024);
    assert_eq!(first["preflight"]["limits"]["maxFileBytes"],8*1024*1024);
    let db=f.state.db.client().await.unwrap();assert_eq!(db.query_one("SELECT count(*) FROM platform.protocol_publication_commands",&[]).await.unwrap().get::<_,i64>(0),before);
    db.execute("UPDATE platform.protocol_identity_bindings SET revoked_at=clock_timestamp() WHERE user_id='PublicUploadUser1'",&[]).await.unwrap();drop(db);
    let denied:Value=native("GET",&path).send().await.unwrap().json().await.unwrap();assert_eq!(denied["preflight"]["eligible"],false);assert!(denied["preflight"]["reason"].as_str().unwrap().contains("site operator"));
    let db=f.state.db.client().await.unwrap();assert_eq!(db.query_one("SELECT count(*) FROM platform.protocol_publication_commands",&[]).await.unwrap().get::<_,i64>(0),before);db.execute("UPDATE platform.protocol_identity_bindings SET revoked_at=NULL WHERE user_id='PublicUploadUser1'",&[]).await.unwrap();drop(db);
    let begin:Value=native("POST",&format!("/api/v1/account/upload/begin?expectedAccountId={account}")).json(&json!({"manga":f.manga,"groups":[],"expectedAccountId":account})).send().await.unwrap().json().await.unwrap();let session=begin["id"].as_str().unwrap();
    let boundary="six-persona-native-synthetic";let mut body=format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"page.png\"\r\nContent-Type: image/png\r\n\r\n").into_bytes();body.extend(png());
    let mut webp=Cursor::new(Vec::new());DynamicImage::ImageRgb8(RgbImage::from_pixel(13,17,Rgb([12,34,56]))).write_to(&mut webp,ImageFormat::WebP).unwrap();
    body.extend(format!("\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"page-2.webp\"\r\nContent-Type: image/webp\r\n\r\n").as_bytes());body.extend(webp.into_inner());body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
    let attached=native("POST",&format!("/api/v1/account/upload/{session}?expectedAccountId={account}")).header("content-type",format!("multipart/form-data; boundary={boundary}")).body(body).send().await.unwrap();assert_eq!(attached.status().as_u16(),200);let attached:Value=attached.json().await.unwrap();assert_eq!(attached["data"].as_array().unwrap().len(),2);
    let committed=native("POST",&format!("/api/v1/account/upload/{session}/commit?expectedAccountId={account}")).json(&commit(attached["data"].as_array().unwrap())).send().await.unwrap();assert_eq!(committed.status().as_u16(),200);let receipt:Value=committed.json().await.unwrap();assert_eq!(receipt["type"],"chapter");assert_eq!(receipt["attributes"]["pages"],2);assert_eq!(receipt["nostr"]["accountId"],account);
    let read:Value=native("GET",&format!("/api/v1/account/uploads/{session}?expectedAccountId={account}")).send().await.unwrap().json().await.unwrap();assert_eq!(read["session"]["state"],"committed");assert_eq!(read["session"]["receipt"]["id"],receipt["id"]);
}
