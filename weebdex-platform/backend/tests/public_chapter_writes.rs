//! Real signed HTTP chapter writes and durable review-first local removal.
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
async fn alias(f:&Fixture,kind:ResourceKind,native:&str)->String { PostgresPublicIdResolver::new(f.state.db.clone()).preferred(kind,native).await.unwrap().unwrap().to_string() }
async fn review(f:&Fixture,report:&str,expected:u16)->Value {
    let response=f.http.post(format!("{}/api/report/{report}/accept",f.origin)).header("origin",&f.origin)
        .header("cookie","weebdex_session=synthetic-public-chapter-moderator").json(&json!({"version":1,"notes":"Explicit owned fixture review"})).send().await.unwrap();
    let status=response.status().as_u16();let value:Value=response.json().await.unwrap();assert_eq!(status,expected,"native moderator review: {value}");value
}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires scripts/run-auth-tests.sh and owned isolated PostgreSQL"]
async fn signed_chapter_metadata_claim_and_review_first_removal_preserve_authority() {
    let mut f=fixture().await;
    let db=f.state.db.client().await.unwrap();
    db.batch_execute(r#"INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES('ChapterGp1','PublicUploadUser1','Fixture','Owned chapter group'); INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,language,title,metadata) SELECT x,'UploadMng1','PublicUploadUser1','Fixture','en','Before',jsonb_build_object('data',jsonb_build_array(jsonb_build_object('name','immutable.png','dimensions',jsonb_build_array(13,17))),'protocol_source_metadata',jsonb_build_object('preserved',true)) FROM unnest(ARRAY['ChapterA01','ChapterB01','ChapterC01']) x; INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES('ChapterA01','ChapterGp1'); UPDATE platform.users SET roles='["moderator"]',permissions='["chapter.update","chapter.remove","report.update"]' WHERE id='PublicUploadUser8'"#).await.unwrap();
    db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES('public-chapter-mod-session','PublicUploadUser8',$1,clock_timestamp()+interval '1 hour')",&[&Sha256::digest(b"synthetic-public-chapter-moderator").to_vec()]).await.unwrap();
    let chapter=alias(&f,ResourceKind::Chapter,"ChapterA01").await;
    let group=alias(&f,ResourceKind::Group,"ChapterGp1").await;
    let path=format!("/api/v1/chapter/{chapter}");
    let operation_reason=alias(&f,ResourceKind::ReportReason,"chapter_removal_request").await;
    let reasons=f.call(Some(1),"GET","/api/v1/report/reasons/chapter",None,200).await;
    assert!(reasons["data"].as_array().unwrap().iter().all(|reason|reason["id"]!=operation_reason));
    let reasons:Value=f.http.get(format!("{}/api/report/reasons/chapter",f.origin)).send().await.unwrap().json().await.unwrap();
    assert!(reasons.as_array().unwrap().iter().all(|reason|reason["id"]!="chapter_removal_request"));
    db.execute("UPDATE platform.users SET permissions=permissions || '[\"report.create\"]'::jsonb WHERE id='PublicUploadUser1'",&[]).await.unwrap();
    f.call(Some(1),"POST","/api/v1/report",Some(json!({"category":"chapter","reason":operation_reason,"objectId":chapter,"details":"Generic removal must not bypass its dedicated preflight"})),400).await;

    for key in [None,Some(3),Some(4),Some(2)] {f.call(key,"PUT",&path,Some(json!({"version":1,"title":"Refused"})),if key.is_none(){401}else{403}).await;}
    let cookie=f.http.put(format!("{}{path}",f.origin)).header("cookie","weebdex_session=synthetic-public-upload-cookie").json(&json!({"version":1,"title":"No cookie authority"})).send().await.unwrap();assert_eq!(cookie.status().as_u16(),401);
    let result=f.call(Some(1),"PUT",&path,Some(json!({"version":1,"title":"Updated","volume":"2.5a","chapter":"3","translatedLanguage":"es","groups":[group]})),200).await;
    assert_eq!(result["result"],"ok");assert_eq!(result["response"],"entity");assert_eq!(result["data"]["id"],chapter);assert_eq!(result["data"]["attributes"]["version"],2);assert_eq!(result["data"]["attributes"]["translatedLanguage"],"es");
    for payload in [json!({"version":1,"title":"Stale"}),json!({"version":2,"data":[]}),json!({"version":2,"language":"en"}),json!({"version":2,"groups":[group,group]}),json!({"version":2,"groups":["ChapterGp1"]})] {
        let expected=if payload["version"]==1 {409}else{400};f.call(Some(1),"PUT",&path,Some(payload),expected).await;
    }
    f.call(Some(1),"PUT",&format!("{path}?unexpected=1"),Some(json!({"version":2})),400).await;
    let body=serde_json::to_vec(&json!({"version":2,"title":"Signed exactly"})).unwrap();
    f.raw("PUT",&path,body.clone(),"application/json",Some(proof(1,11,&format!("{}{path}",f.origin),"POST",&body)),401).await;
    let token=proof(1,11,&format!("{}{path}",f.origin),"PUT",&body);
    f.raw("PUT",&path,body.clone(),"application/json",Some(token.clone()),200).await;
    f.raw("PUT",&path,body.clone(),"application/json",Some(token),401).await;
    f.raw("PUT",&path,b"{\"version\":3}".to_vec(),"application/json",Some(proof(1,11,&format!("{}{path}",f.origin),"PUT",&body)),401).await;
    let claim=format!("{path}/claim");f.call(Some(2),"POST",&claim,None,403).await;
    db.execute("INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader) VALUES('ChapterGp1','PublicUploadUser2','Leader',TRUE)",&[]).await.unwrap();
    let claimed=f.call(Some(2),"POST",&claim,None,200).await;assert_eq!(claimed["data"]["attributes"]["version"],4);
    f.call(Some(2),"POST",&claim,None,403).await;
    f.call(Some(1),"DELETE",&path,None,403).await;
    f.call(Some(2),"DELETE",&path,Some(json!({})),400).await;
    let removal=f.call(Some(2),"DELETE",&path,None,202).await;
    assert_eq!(removal["status"],"pending-moderation");assert_eq!(removal["sourceDeletionRequestId"],Value::Null);
    let again=f.call(Some(2),"DELETE",&path,None,202).await;assert_eq!(again["requestId"],removal["requestId"]);
    let live=f.http.get(format!("{}{path}",f.origin)).send().await.unwrap();assert_eq!(live.status().as_u16(),200);
    let audit=db.query_one("SELECT c.version,c.deleted_at IS NULL,c.metadata->'data',c.metadata->'protocol_source_metadata',(SELECT count(*) FROM platform.reports WHERE entity_id=c.id AND reason_id='chapter_removal_request') FROM platform.catalog_chapters c WHERE c.id='ChapterA01'",&[]).await.unwrap();
    assert_eq!(audit.get::<_,i32>(0),4);assert!(audit.get::<_,bool>(1));assert_eq!(audit.get::<_,Value>(2)[0]["name"],"immutable.png");assert_eq!(audit.get::<_,Value>(3),json!({"preserved":true}));assert_eq!(audit.get::<_,i64>(4),1);
    // Moderator status without its action grant still cannot approve a hide.
    db.execute("UPDATE platform.users SET permissions='[]' WHERE id='PublicUploadUser8'",&[]).await.unwrap();f.call(Some(8),"DELETE",&path,None,403).await;
    db.execute(r#"UPDATE platform.users SET permissions='["chapter.update","chapter.remove","report.update"]' WHERE id='PublicUploadUser8'"#,&[]).await.unwrap();
    let response=f.http.delete(format!("{}{path}",f.origin)).header("authorization",proof(8,11,&format!("{}{path}",f.origin),"DELETE",b"")).send().await.unwrap();
    assert_eq!(response.status().as_u16(),204);assert_eq!(response.headers()["x-weebdex-instance-removal"],"soft-hidden");assert!(response.bytes().await.unwrap().is_empty());
    assert_eq!(f.http.get(format!("{}{path}",f.origin)).send().await.unwrap().status().as_u16(),404);
    let status:String=db.query_one("SELECT status FROM platform.reports WHERE entity_id='ChapterA01'",&[]).await.unwrap().get(0);assert_eq!(status,"accepted");
    // Existing native review is a deliberate, audited disposition of the queue.
    for (native,stale) in [("ChapterB01",false),("ChapterC01",true)] {
        let id=alias(&f,ResourceKind::Chapter,native).await;let path=format!("/api/v1/chapter/{id}");
        let pending=f.call(Some(1),"DELETE",&path,None,202).await;
        let report=f.native(ResourceKind::Report,pending["requestId"].as_str().unwrap()).await;
        if stale {f.call(Some(1),"PUT",&path,Some(json!({"version":1,"title":"Changed after request"})),200).await;}
        let reviewed=review(&f,&report,if stale{409}else{200}).await;
        if !stale {assert_eq!(reviewed["status"],"accepted");}
        let hidden:bool=db.query_one("SELECT deleted_at IS NOT NULL FROM platform.catalog_chapters WHERE id=$1",&[&native]).await.unwrap().get(0);assert_eq!(hidden,!stale);
    }
    captured_account_intent_refuses_eligible_rebind(&f).await;
    captured_chapter_removal_effect_is_preserved(&f).await;
    signed_original_deletion_request_remains_review_only(&f).await;
    f.repository.set_account_admission(id(1),false).await.unwrap();
    let c=alias(&f,ResourceKind::Chapter,"ChapterC01").await;f.call(Some(1),"PUT",&format!("/api/v1/chapter/{c}"),Some(json!({"version":2,"title":"Revoked"})),403).await;
    drop(db);
    let mut native=f.native.take().unwrap();if let Some(shutdown)=native.shutdown.take(){let _=shutdown.send(());}
    tokio::time::timeout(Duration::from_secs(30),native.task).await.unwrap().unwrap().unwrap();
}

#[path="../../../manga-protocol-lab/server/tests/support/mod.rs"]
mod original_fixture;

// These are actual proof/body captures followed by an eligible native-account
// rebind, not simulated authorization failures. Missing extensions remain
// compatible in the existing journey above; first-party commands always bind it.
async fn captured_account_intent_refuses_eligible_rebind(f:&Fixture) {
    let db=f.state.db.client().await.unwrap();
    db.batch_execute(r#"UPDATE platform.users SET permissions='["chapter.create","self.chapter.update","self.chapter.remove","report.create"]' WHERE id IN('PublicUploadUser1','PublicUploadUser2');
      INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES('IntentGp01','PublicUploadUser1','Fixture','Intent group A'),('IntentGp02','PublicUploadUser2','Fixture','Intent group B');
      INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader) VALUES('IntentGp01','PublicUploadUser1','A',TRUE),('IntentGp02','PublicUploadUser2','B',TRUE);
      INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,language,title) SELECT x,'UploadMng1','PublicUploadUser4','Other uploader','en','Before' FROM unnest(ARRAY['IntentUp01','IntentCl01','IntentDl01','IntentEd01']) x;
      INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) SELECT c,g FROM unnest(ARRAY['IntentUp01','IntentCl01','IntentDl01','IntentEd01']) c CROSS JOIN unnest(ARRAY['IntentGp01','IntentGp02']) g;"#).await.unwrap();
    let manga=alias(f,ResourceKind::Manga,FIXTURE_MANGA_ID).await;
    for group in ["IntentGp01","IntentGp02"] {alias(f,ResourceKind::Group,group).await;}
    for user in ["PublicUploadUser2","PublicUploadUser4"] {alias(f,ResourceKind::User,user).await;}
    let intended=alias(f,ResourceKind::User,"PublicUploadUser1").await;
    let other=alias(f,ResourceKind::User,"PublicUploadUser2").await;
    let chapter=alias(f,ResourceKind::Chapter,"IntentUp01").await;
    let claim=alias(f,ResourceKind::Chapter,"IntentCl01").await;
    let remove=alias(f,ResourceKind::Chapter,"IntentDl01").await;
    let edit=alias(f,ResourceKind::Chapter,"IntentEd01").await;
    let reason=alias(f,ResourceKind::ReportReason,"manga_spam").await;
    let snapshot=r#"SELECT jsonb_build_object(
      'chapters',(SELECT jsonb_agg(to_jsonb(c) ORDER BY c.id) FROM platform.catalog_chapters c WHERE id LIKE 'Intent%'),
      'sessions',(SELECT jsonb_agg(to_jsonb(s) ORDER BY s.id) FROM platform.upload_sessions s WHERE owner_id IN('PublicUploadUser1','PublicUploadUser2')),
      'reports',(SELECT jsonb_agg(to_jsonb(r) ORDER BY r.id) FROM platform.reports r),
      'audit',(SELECT jsonb_agg(to_jsonb(a) ORDER BY a.id) FROM platform.catalog_audit a))"#;
    let mut verified=Vec::new();
    for (method,path,payload,status,observation_sql) in [
      ("PUT",format!("/api/v1/chapter/{chapter}"),Some(json!({"version":1,"title":"Captured account A intent","expectedAccountId":intended})),200,"SELECT actor_id FROM platform.catalog_audit WHERE entity_id='IntentUp01' ORDER BY id DESC LIMIT 1"),
      ("POST",format!("/api/v1/chapter/{claim}/claim?expectedAccountId={intended}"),None,200,"SELECT uploader_id FROM platform.catalog_chapters WHERE id='IntentCl01'"),
      ("DELETE",format!("/api/v1/chapter/{remove}?expectedAccountId={intended}"),None,202,"SELECT submitter_id FROM platform.reports WHERE entity_id='IntentDl01'"),
      ("POST","/api/v1/upload/begin".into(),Some(json!({"manga":manga,"groups":[],"expectedAccountId":intended})),200,"SELECT owner_id FROM platform.upload_sessions WHERE state='open'"),
      ("POST",format!("/api/v1/upload/begin/{edit}"),Some(json!({"version":1,"expectedAccountId":intended})),200,"SELECT owner_id FROM platform.upload_sessions WHERE state='open'"),
      ("POST","/api/v1/report".into(),Some(json!({"category":"manga","reason":reason,"objectId":manga,"details":"Captured account A report","expectedAccountId":intended})),201,"SELECT submitter_id FROM platform.reports WHERE entity_id='UploadMng1' AND reason_id='manga_spam'"),
    ] {
      let context=f.call(Some(1),"GET","/api/v1/nostr/account-context",None,200).await;assert_eq!(context["user"]["id"],intended);
      let body=payload.clone().map_or_else(Vec::new,|v|serde_json::to_vec(&v).unwrap());
      let token=proof(1,11,&format!("{}{path}",f.origin),method,&body);
      let before:Value=db.query_one(snapshot,&[]).await.unwrap().get(0);
      db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser2' WHERE publisher=$1",&[&public(1)]).await.unwrap();
      f.raw(method,&path,body,"application/json",Some(token),409).await;
      let after:Value=db.query_one(snapshot,&[]).await.unwrap().get(0);assert_eq!(after,before,"refused captured-account command changed native state: {method} {path}");
      db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser1' WHERE publisher=$1",&[&public(1)]).await.unwrap();
      let result=f.call(Some(1),method,&path,payload,status).await;
      let actor:String=db.query_one(observation_sql,&[]).await.unwrap().get(0);assert_eq!(actor,"PublicUploadUser1");
      verified.push(json!({"method":method,"path":path,"rebindStatus":409,"sameAccountStatus":status,"actualActor":actor,"rollback":true}));
      if path.starts_with("/api/v1/upload/begin") {let session=result["id"].as_str().unwrap();f.call(Some(1),"DELETE",&format!("/api/v1/upload/{session}"),None,200).await;}
    }
    // Signed account-bound discovery cannot adopt another account's session.
    let opened=f.call(Some(2),"POST","/api/v1/upload/begin",Some(json!({"manga":manga,"groups":[],"expectedAccountId":other})),200).await;
    let other_session=opened["id"].as_str().unwrap();
    let path=format!("/api/v1/upload?expectedAccountId={intended}");
    let token=proof(1,11,&format!("{}{path}",f.origin),"GET",b"");
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser2' WHERE publisher=$1",&[&public(1)]).await.unwrap();
    f.raw("GET",&path,vec![],"application/json",Some(token),409).await;
    assert_eq!(f.call(Some(2),"GET",&format!("/api/v1/upload?expectedAccountId={other}"),None,200).await["id"],other_session);
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser1' WHERE publisher=$1",&[&public(1)]).await.unwrap();
    f.call(Some(1),"GET",&path,None,404).await;
    let own=f.call(Some(1),"POST","/api/v1/upload/begin",Some(json!({"manga":manga,"groups":[],"expectedAccountId":intended})),200).await;
    let own_session=own["id"].as_str().unwrap();
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser2' WHERE publisher=$1",&[&public(1)]).await.unwrap();
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{own_session}"),None,404).await;
    let open:i64=db.query_one("SELECT count(*) FROM platform.upload_sessions WHERE state='open'",&[]).await.unwrap().get(0);assert_eq!(open,2,"session owner fences must remain independent of discovery intent");
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser1' WHERE publisher=$1",&[&public(1)]).await.unwrap();
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{own_session}"),None,200).await;
    f.call(Some(2),"DELETE",&format!("/api/v1/upload/{other_session}"),None,200).await;
    // The signed URL includes its exact sole account query; no proof stripping.
    let wrong_proof=proof(1,11,&format!("{}/api/v1/upload",f.origin),"GET",b"");
    f.raw("GET",&path,vec![],"application/json",Some(wrong_proof),401).await;
    for query in [format!("expectedAccountId={manga}"),"expectedAccountId=not-a-uuid".into(),format!("expectedAccountId={intended}&expectedAccountId={other}"),"unexpected=1".into()] {
      f.call(Some(1),"GET",&format!("/api/v1/upload?{query}"),None,if query==format!("expectedAccountId={manga}"){404}else{400}).await;
    }
    let claim_path=format!("/api/v1/chapter/{claim}/claim?expectedAccountId={intended}");
    f.call(Some(1),"POST",&claim_path,Some(json!({})),400).await;
    println!("ACCOUNT_INTENT_VERIFIED={}",json!({"writes":verified,"discoveryRebindStatus":409,"retainedSessionOwnerStatus":404,"foreignSessionsUnchanged":true}));
}
// Staged real grant-gain/loss regression. The compatibility request deliberately
// demonstrates the legacy inferred effect; the first-party signed mode must not.
async fn captured_chapter_removal_effect_is_preserved(f:&Fixture) {
    let db=f.state.db.client().await.unwrap();
    db.batch_execute("INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,language,title) SELECT x,'UploadMng1','PublicUploadUser1','Fixture','en','Captured removal effect' FROM unnest(ARRAY['ModeOld001','ModeGain01','ModeDeny01','ModeLost01']) x").await.unwrap();
    let account=alias(f,ResourceKind::User,"PublicUploadUser1").await;
    let mut paths=Vec::new();for native in ["ModeOld001","ModeGain01","ModeDeny01","ModeLost01"] {paths.push(format!("/api/v1/chapter/{}?expectedAccountId={account}",alias(f,ResourceKind::Chapter,native).await));}
    let ordinary=f.call(Some(1),"GET","/api/v1/nostr/account-context",None,200).await;
    assert!(!ordinary["user"]["roles"].as_array().unwrap().iter().any(|r|r=="moderator"));
    let snapshot="SELECT jsonb_build_object('chapters',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM platform.catalog_chapters c WHERE id LIKE 'Mode%'),'reports',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM platform.reports r),'audit',(SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM platform.catalog_audit a))";
    let before:Value=db.query_one(snapshot,&[]).await.unwrap().get(0);
    f.call(Some(1),"DELETE",&format!("{}&removalMode=approve-hide",paths[2]),None,403).await;
    let after:Value=db.query_one(snapshot,&[]).await.unwrap().get(0);assert_eq!(before,after,"ordinary approval must not fall back to a review report");
    let review_path=format!("{}&removalMode=request-review",paths[1]);
    let legacy=proof(1,11,&format!("{}{}",f.origin,paths[0]),"DELETE",b"");
    let review=proof(1,11,&format!("{}{review_path}",f.origin),"DELETE",b"");
    db.execute(r#"UPDATE platform.users SET roles='["moderator"]',permissions=permissions || '["chapter.remove"]'::jsonb WHERE id='PublicUploadUser1'"#,&[]).await.unwrap();
    async fn hide(f:&Fixture,path:&str,token:String) {
        let response=f.http.delete(format!("{}{path}",f.origin)).header("authorization",token).send().await.unwrap();
        assert_eq!(response.status().as_u16(),204);assert_eq!(response.headers()["cache-control"],"no-store");
        assert_eq!(response.headers()["x-weebdex-instance-removal"],"soft-hidden");assert_eq!(response.headers()["x-weebdex-source-deletion-status"],"instance-local-only");
        assert!(response.bytes().await.unwrap().is_empty());
    }
    hide(f,&paths[0],legacy).await; // Explicitly retained legacy API compatibility.
    let queued=f.raw("DELETE",&review_path,vec![],"application/json",Some(review),202).await;
    assert_eq!(queued["instanceRemoval"],"pending-moderation");assert_eq!(queued["sourceDeletionStatus"],"instance-local-only");
    let visible:bool=db.query_one("SELECT deleted_at IS NULL FROM platform.catalog_chapters WHERE id='ModeGain01'",&[]).await.unwrap().get(0);assert!(visible,"captured review must stay visible after grant gain");
    let approve_path=format!("{}&removalMode=approve-hide",paths[3]);
    let approve=proof(1,11,&format!("{}{approve_path}",f.origin),"DELETE",b"");
    db.execute(r#"UPDATE platform.users SET permissions=permissions - 'chapter.remove' WHERE id='PublicUploadUser1'"#,&[]).await.unwrap();
    let before:Value=db.query_one(snapshot,&[]).await.unwrap().get(0);
    f.raw("DELETE",&approve_path,vec![],"application/json",Some(approve),403).await;
    let after:Value=db.query_one(snapshot,&[]).await.unwrap().get(0);assert_eq!(before,after,"revoked approval must not hide or silently queue");
    db.execute(r#"UPDATE platform.users SET permissions=permissions || '["chapter.remove"]'::jsonb WHERE id='PublicUploadUser1'"#,&[]).await.unwrap();
    let approve_path=format!("{}&removalMode=approve-hide",paths[1]);
    hide(f,&approve_path,proof(1,11,&format!("{}{approve_path}",f.origin),"DELETE",b"")).await;
    let report= f.native(ResourceKind::Report,queued["requestId"].as_str().unwrap()).await;
    let status:String=db.query_one("SELECT status FROM platform.reports WHERE id=$1",&[&report]).await.unwrap().get(0);assert_eq!(status,"accepted");
    for suffix in ["&removalMode=bad","&removalMode=request-review&removalMode=approve-hide"] {f.call(Some(1),"DELETE",&format!("{}{suffix}",paths[2]),None,400).await;}
    db.execute(r#"UPDATE platform.users SET roles='["user"]',permissions=permissions - 'chapter.remove' WHERE id='PublicUploadUser1'"#,&[]).await.unwrap();
    println!("CHAPTER_REMOVAL_EFFECT_VERIFIED={}",json!({"grantGainSignedReview":202,"grantGainLegacyCompatibility":204,"ordinaryApproval":403,"revokedApproval":403,"freshExplicitApproval":204,"refusalNativeStateUnchanged":true}));
}
async fn signed_original_deletion_request_remains_review_only(f:&Fixture) {
    use std::collections::BTreeMap;
    use weebdex_platform::protocol::{bridge,publication::PublicationInput};
    let runtime=f.state.protocol.publication_runtime().await.unwrap();
    let original=original_fixture::png();
    let series=runtime.publish_operator(PublicationInput{kind:1002,canonical_manifest:original_fixture::cbor(json!({"type":"series","version":1,"titles":{"en":"Public chapter deletion source"},"contentRating":"safe","required":["series-cover-v1"],"cover":original_fixture::page(&original)})),originals:BTreeMap::from([(auth::hash(&original),original.clone())]),created_at:None}).await.unwrap().event;
    let mut bytes=Cursor::new(Vec::new());DynamicImage::ImageRgb8(RgbImage::from_pixel(13,17,Rgb([57,12,188]))).write_to(&mut bytes,ImageFormat::Png).unwrap();let bytes=bytes.into_inner();
    let page=json!({"sha256":auth::hash(&bytes),"size":bytes.len(),"mime":"image/png","width":13,"height":17});
    let chapter=runtime.publish_operator(PublicationInput{kind:1001,canonical_manifest:original_fixture::cbor(json!({"type":"chapter","version":1,"series":series.id,"language":"en","chapter":"1","pages":[page]})),originals:BTreeMap::from([(auth::hash(&bytes),bytes)]),created_at:None}).await.unwrap().event;
    for event in [&series.id,&chapter.id] {
        let id:[u8;32]=hex::decode(event).unwrap().try_into().unwrap();
        tokio::time::timeout(Duration::from_secs(20),async {loop {if f.state.protocol.entry(id).await.unwrap().is_some(){break;}tokio::time::sleep(Duration::from_millis(30)).await;}}).await.unwrap();
    }
    for _ in 0..8 {let result=bridge::import_page(&f.state).await.unwrap();assert_eq!(result.rejected,0);if result.completed_sweep && result.deferred==0 && result.scanned>0 {break;}}
    let db=f.state.db.client().await.unwrap();
    let row=db.query_one("SELECT id,version FROM platform.catalog_chapters WHERE protocol_event_id=$1",&[&chapter.id]).await.unwrap();let native:String=row.get(0);let version:i32=row.get(1);
    let public=alias(f,ResourceKind::Chapter,&native).await;let path=format!("/api/v1/chapter/{public}");
    let group=alias(f,ResourceKind::Group,"ChapterGp1").await;
    f.call(Some(1),"PUT",&path,Some(json!({"version":version,"title":"Local annotation over immutable source","groups":[group]})),200).await;
    let saved:Value=db.query_one("SELECT metadata->'instance_overlay' FROM platform.catalog_chapters WHERE id=$1",&[&native]).await.unwrap().get(0);assert_eq!(saved["title"],"Local annotation over immutable source");
    let pending=f.call(Some(1),"DELETE",&path,None,202).await;let request=pending["sourceDeletionRequestId"].as_str().unwrap();assert_eq!(request.len(),64);
    let again=f.call(Some(1),"DELETE",&path,None,202).await;assert_eq!(again,pending);
    let source:[u8;32]=hex::decode(&chapter.id).unwrap().try_into().unwrap();assert!(f.state.protocol.entry(source).await.unwrap().is_some());
    let visible=f.http.get(format!("{}{path}",f.origin)).send().await.unwrap();assert_eq!(visible.status().as_u16(),200);
    let row=db.query_one("SELECT c.deleted_at IS NULL,c.protocol_event_id,i.state,i.request_event,e.raw_event FROM platform.catalog_chapters c JOIN platform.protocol_deletion_intents i ON i.chapter_id=c.id JOIN events e ON encode(e.id,'hex')=c.protocol_event_id WHERE c.id=$1",&[&native]).await.unwrap();
    assert!(row.get::<_,bool>(0));assert_eq!(row.get::<_,String>(1),chapter.id);assert_eq!(row.get::<_,String>(2),"requested");
    let event:auth::Event=serde_json::from_value(row.get(3)).unwrap();event.verify().unwrap();assert_eq!(event.kind,5);assert_eq!(event.id,request);assert_eq!(event.values("e"),vec![chapter.id.as_str()]);
    let unchanged:auth::Event=serde_json::from_value(row.get(4)).unwrap();assert_eq!(serde_json::to_value(unchanged).unwrap(),serde_json::to_value(chapter).unwrap());
    // A local group leader can claim and request LOCAL removal, without ever
    // gaining the original publisher's signing authority or fabricating kind5.
    f.call(Some(2),"POST",&format!("{path}/claim"),None,200).await;
    let claimed=f.call(Some(2),"DELETE",&path,None,202).await;
    assert_eq!(claimed["sourceDeletionStatus"],"not-authorized");assert_eq!(claimed["sourceDeletionRequestId"],Value::Null);
    let count:i64=db.query_one("SELECT count(*) FROM platform.protocol_deletion_intents WHERE chapter_id=$1 AND actor_user_id='PublicUploadUser2'",&[&native]).await.unwrap().get(0);assert_eq!(count,0);
    assert_eq!(f.http.get(format!("{}{path}",f.origin)).send().await.unwrap().status().as_u16(),200);
    db.execute("INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader) VALUES('ChapterGp1','PublicUploadUser1','Original source owner',TRUE)",&[]).await.unwrap();
    let intent_before:String=db.query_one("SELECT updated_at::text FROM platform.protocol_deletion_intents WHERE chapter_id=$1 AND actor_user_id='PublicUploadUser1'",&[&native]).await.unwrap().get(0);
    // The original owner's earlier local request has a stale target version.
    f.call(Some(1),"DELETE",&path,None,409).await;
    let intent_after:String=db.query_one("SELECT updated_at::text FROM platform.protocol_deletion_intents WHERE chapter_id=$1 AND actor_user_id='PublicUploadUser1'",&[&native]).await.unwrap().get(0);assert_eq!(intent_after,intent_before);
    // A new request exceeding the bounded report quota also cannot resubmit the
    // already durable original event before returning its local rate refusal.
    db.execute("UPDATE platform.reports SET status='refused' WHERE entity_id=$1 AND submitter_id='PublicUploadUser1'",&[&native]).await.unwrap();
    db.execute("INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,body,status,target_version) SELECT 'OwnedRateFixture'||n,'PublicUploadUser1','chapter',$1,'chapter_removal_request','Owned quota fixture','refused',1 FROM generate_series(1,GREATEST(0,20-(SELECT count(*)::integer FROM platform.reports WHERE submitter_id='PublicUploadUser1' AND created_at>clock_timestamp()-interval '1 hour')))n",&[&native]).await.unwrap();
    f.call(Some(1),"DELETE",&path,None,429).await;
    let intent_after:String=db.query_one("SELECT updated_at::text FROM platform.protocol_deletion_intents WHERE chapter_id=$1 AND actor_user_id='PublicUploadUser1'",&[&native]).await.unwrap().get(0);assert_eq!(intent_after,intent_before);

}
