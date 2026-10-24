//! Approved reversible disabling: disposable real TCP/native+signed authority and SQL races.
//! Never run outside the owned isolated fixture runner. No production account action.
//! Every key/account below is synthetic and belongs only to this disposable DB.
use base64::{Engine, engine::general_purpose::STANDARD};
use manga_node::{
    auth::{self, Event},
    db::{DbConfig, Id, Repository},
};
use secp256k1::{Keypair, Message, Secp256k1, SecretKey};
use serde_json::{Value, json};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use weebdex_platform::{AppState, Config, domains, protocol::publication::OperatorRuntimeConfig};
#[path="support/account_closure_contract.rs"]
mod bounded_contract;

fn public(key: u8) -> String {
    auth::sign(&format!("{key:064x}"), 1, vec![], String::new())
        .unwrap()
        .pubkey
}
fn id(key: u8) -> Id {
    hex::decode(public(key)).unwrap().try_into().unwrap()
}
fn proof_for(key: u8, service: u8, url: &str, method: &str, body: &[u8]) -> String {
    static NONCE: AtomicUsize = AtomicUsize::new(0);
    let secret = SecretKey::from_slice(&hex::decode(format!("{key:064x}")).unwrap()).unwrap();
    let secp = Secp256k1::new();
    let pair = Keypair::from_secret_key(&secp, &secret);
    let mut event = Event {
        id: String::new(),
        pubkey: public(key),
        created_at: auth::now(),
        kind: 27235,
        tags: vec![
            vec!["u".into(), url.into()],
            vec!["method".into(), method.into()],
            vec!["service".into(), public(service)],
            vec!["payload".into(), auth::hash(body)],
            vec![
                "nonce".into(),
                NONCE.fetch_add(1, Ordering::Relaxed).to_string(),
            ],
        ],
        content: String::new(),
        sig: String::new(),
    };
    let digest = event.digest();
    event.id = hex::encode(&digest);
    event.sig = secp
        .sign_schnorr_no_aux_rand(&Message::from_digest_slice(&digest).unwrap(), &pair)
        .to_string();
    format!(
        "Nostr {}",
        STANDARD.encode(serde_json::to_vec(&event).unwrap())
    )
}
fn proof(key: u8, url: &str, method: &str, body: &[u8]) -> String {
    proof_for(key, 11, url, method, body)
}
async fn call(
    client: &reqwest::Client,
    origin: &str,
    method: &str,
    path: &str,
    payload: Option<Value>,
    key: Option<u8>,
    expected: u16,
) -> Value {
    let url = format!("{origin}{path}");
    let body = payload.map_or_else(Vec::new, |value| serde_json::to_vec(&value).unwrap());
    let mut request = client
        .request(method.parse().unwrap(), &url)
        .header("content-type", "application/json");
    if let Some(key) = key {
        request = request.header("authorization", proof(key, &url, method, &body));
    }
    let response = request.body(body).send().await.unwrap();
    let status = response.status().as_u16();
    if method=="DELETE"||path.contains("/user/delete/")||path.contains("/closure") {assert_eq!(response.headers()["referrer-policy"],"no-referrer");}
    assert!(
        response.headers()["cache-control"]
            .to_str()
            .unwrap()
            .contains("no-store")
    );
    assert!(response.headers()["cache-control"].to_str().unwrap().contains("private"));
    assert!(response.headers().get_all("vary").iter().any(|v|v.to_str().unwrap().contains("Authorization")));
    let value: Value = response.json().await.unwrap();
    assert_eq!(status, expected, "{method} {path}: {value}");
    value
}
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

use sha2::{Digest,Sha256};
async fn native(client:&reqwest::Client,origin:&str,token:Option<&str>,method:&str,path:&str,payload:Option<Value>,expected:u16)->Value{
 let mut request=client.request(method.parse().unwrap(),format!("{origin}{path}")).header("origin",origin);
 if let Some(token)=token{request=request.header("cookie",format!("weebdex_session={token}"));}
 if let Some(payload)=payload{request=request.json(&payload);}
 let response=request.send().await.unwrap();assert!(response.headers()["cache-control"].to_str().unwrap().contains("private"));assert_eq!(response.headers()["referrer-policy"],"no-referrer");assert!(response.headers().get_all("vary").iter().any(|v|v.to_str().unwrap().contains("Cookie")));let status=response.status().as_u16();let value:Value=response.json().await.unwrap();assert_eq!(status,expected,"{method} {path}: {value}");value
}
fn capture(user:&str,version:i32,lifecycle:&Value)->Value{json!({"expectedAccountId":user,"expectedUserId":user,"expectedLifecycleId":lifecycle,"version":version})}
fn confirm(user:&str,prepared:&Value)->Value{json!({"capture":capture(user,prepared["version"].as_i64().unwrap() as i32,&prepared["lifecycleId"]),"confirmationId":prepared["confirmationId"],"confirmationToken":prepared["confirmationToken"],"acknowledge":"DISABLE THIS ACCOUNT"})}
async fn wait_pause(db:&tokio_postgres::Client){tokio::time::timeout(Duration::from_secs(5),async{loop{if db.query_one("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND objid=760531 AND NOT granted)",&[]).await.unwrap().get::<_,bool>(0){break;}tokio::time::sleep(Duration::from_millis(10)).await;}}).await.expect("actual confirmation never reached its final transactional update");}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Owned isolated scripts/run-auth-tests.sh PostgreSQL and coordinated heavy slot required"]
async fn reversible_disable_native_signed_atomic_revocation_and_concurrent_grants(){
 use weebdex_platform::public_api::{PostgresPublicIdResolver,PublicIdResolver,ResourceKind};
 let dsn=std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use owned isolated fixture runner");let pg:tokio_postgres::Config=dsn.parse().unwrap();assert_eq!(pg.get_dbname(),Some("weebdex_platform_test"));assert!(pg.get_hosts().iter().all(|host|matches!(host,tokio_postgres::config::Host::Tcp(h) if h=="127.0.0.1")));
 let runtime=std::path::PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap());assert!(runtime.to_str().unwrap().contains("/weebdex-platform/test-runtime/auth-"));let receipt:Value=serde_json::from_slice(&std::fs::read(runtime.join("fixture.json")).unwrap()).unwrap();assert_eq!(receipt["fixture_only"],true);assert_eq!(receipt["database"],"weebdex_platform_test");
 let original=Repository::connect(DbConfig{dsn:dsn.clone(),connections:2,acquisition_timeout:Duration::from_secs(2),cache_weight:1048576,plaintext_local:true}).await.unwrap();original.migrate().await.unwrap();original.bind_instance(id(11),id(1)).await.unwrap();for key in [2,3,4]{original.set_account_admission(id(key),true).await.unwrap();}
 let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let origin=format!("http://{}",listener.local_addr().unwrap());
 let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:8,uploads_dir:runtime.join("closure-media"),ui_dir:runtime.join("no-ui"),protocol_publication:Some(OperatorRuntimeConfig{operator_key_file:runtime.join("unused/operator.key"),expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),storage:json!({"path":runtime.join("unused/storage"),"volume":"closurefixture","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144}),group_keys_dir:None,backend:None,native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":runtime.join("unused/service.key"),"admin_pubkey":public(1)}))}),..Config::default()},dsn).await.unwrap();state.db.migrate(domains::MIGRATIONS).await.unwrap();state.protocol.verify_schema().await.unwrap();
 let db=state.db.client().await.unwrap();for key in [2,3,4]{let user=format!("CloseUser{key}");let token=format!("closure-token-{key}");db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,permissions,profile) VALUES($1,$1,$2,'synthetic-unused',TRUE,$3,$4)",&[&user,&format!("close{key}@fixture.invalid"),&json!(["self.user.update","manga.create","author.create","self.chapter.update","chapter.create","thread.reply.create","thread.vote.create"]),&json!({"description":"PRIVATE_BIO","discord":"PRIVATE_CONTACT","email":"PRIVATE_EMAIL","unknown":"PRIVATE_EXTENSION"})]).await.unwrap();db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await.unwrap();db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES($1,$2,$3,clock_timestamp()+interval '1 hour')",&[&format!("closure-session-{key}"),&user,&Sha256::digest(token.as_bytes()).to_vec()]).await.unwrap();}
 db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at,purpose) VALUES('closure-extra','CloseUser2',$1,clock_timestamp()+interval '1 hour','normal'),('closure-recovery','CloseUser2',$2,clock_timestamp()+interval '1 hour','recovery')",&[&Sha256::digest(b"closure-extra-token").to_vec(),&Sha256::digest(b"closure-recovery-token").to_vec()]).await.unwrap();
 db.execute("INSERT INTO platform.api_clients(id,user_id,name,secret_hash,scopes) VALUES('closure-client','CloseUser2','fixture',$1,'[\"self.library.read\"]')",&[&Sha256::digest(b"wd_closure-client.synthetic").to_vec()]).await.unwrap();
 db.execute("INSERT INTO platform.auth_flows(id,kind,browser_hash,csrf_token,session_id,return_to,expires_at) VALUES('closure-flow','settings',$1,'synthetic-csrf','closure-session-2','/',clock_timestamp()+interval '1 hour')",&[&vec![1_u8;32]]).await.unwrap();db.execute("INSERT INTO platform.auth_email_tokens(id,kind,user_id,token_hash,recipient_hash,flow_id,return_to,delivery_state,expires_at) VALUES('closure-email','recovery','CloseUser2',$1,$2,'closure-flow','/','sent',clock_timestamp()+interval '1 hour')",&[&vec![2_u8;32],&vec![3_u8;32]]).await.unwrap();
 db.batch_execute("INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES('CloseGroup','CloseUser2','CloseUser2','Closure fixture group'); INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader) VALUES('CloseGroup','CloseUser2','CloseUser2',TRUE); INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES('CloseManga','CloseUser2','CloseUser2','Retained contribution','en','ongoing','safe','published'); INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,language,metadata) VALUES('CloseChap1','CloseManga','CloseUser2','CloseUser2','en','{}');").await.unwrap();
 let user_thread:String=db.query_one("SELECT id FROM platform.catalog_threads WHERE entity_type='user' AND entity_id='CloseUser2'",&[]).await.unwrap().get(0);db.execute("INSERT INTO platform.community_replies(thread_id,reply_number,author_id,body) VALUES($1,1,'CloseUser2','Retained discussion')",&[&user_thread]).await.unwrap();
 let resolver=PostgresPublicIdResolver::new(state.db.clone());let public2=resolver.preferred(ResourceKind::User,"CloseUser2").await.unwrap().unwrap();let public4=resolver.preferred(ResourceKind::User,"CloseUser4").await.unwrap().unwrap();drop(db);
 let app=weebdex_platform::router::router(state.clone());let _server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap();}));let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build().unwrap();let token=Some("closure-token-2");let preview="/api/account/closure/CloseUser2";let prepare="/api/account/closure/prepare";let finalize="/api/account/closure/confirm";
 let lifecycle2=json!(state.db.client().await.unwrap().query_one("SELECT account_lifecycle_id::text FROM platform.users WHERE id='CloseUser2'",&[]).await.unwrap().get::<_,String>(0));
 native(&client,&origin,None,"GET",preview,None,401).await;native(&client,&origin,Some("closure-recovery-token"),"POST",prepare,Some(capture("CloseUser2",1,&lifecycle2)),403).await;
 native(&client,&origin,token,"POST",prepare,Some(capture("CloseUser3",1,&lifecycle2)),409).await;
 let first=native(&client,&origin,token,"GET",preview,None,200).await;assert_eq!(first["blockers"][0]["kind"],"sole-group-leader");let blocked=native(&client,&origin,token,"POST",prepare,Some(capture("CloseUser2",1,&lifecycle2)),200).await;assert_eq!(blocked["result"],"blocked");
 // Both transports resolve the same immutable lifecycle identity, not a public alias.
 let cross=call(&client,&origin,"GET",&format!("/api/v1/nostr/user/{public2}/closure?expectedAccountId={public2}"),None,Some(2),200).await;assert_eq!(first["lifecycleId"],cross["lifecycleId"]);assert_eq!(first["lifecycleId"],lifecycle2);assert_ne!(first["accountId"],cross["accountId"]);
 let db=state.db.client().await.unwrap();let immutable=db.execute("UPDATE platform.users SET account_lifecycle_id=gen_random_uuid() WHERE id='CloseUser2'",&[]).await.unwrap_err();assert_eq!(immutable.code().unwrap().code(),"42501");drop(db);
 let mut wrong_lifecycle=capture("CloseUser2",1,&lifecycle2);wrong_lifecycle["expectedLifecycleId"]=json!("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa");native(&client,&origin,token,"POST",prepare,Some(wrong_lifecycle),409).await;
 // Real outer signed NO KEY UPDATE plus independent command KEY SHARE must not
 // deadlock. The command commits before closure and is then a visible blocker.
 let mut outer_db=state.db.client().await.unwrap();let outer=outer_db.transaction().await.unwrap();outer.query_one("SELECT id FROM platform.users WHERE id='CloseUser2' FOR NO KEY UPDATE",&[]).await.unwrap();
 let inner=state.db.client().await.unwrap();tokio::time::timeout(Duration::from_secs(2),inner.execute("INSERT INTO platform.protocol_publication_commands(command_id,user_id,input_hash,resource_kind,catalog_id,created_at,state) VALUES('closure-compatible','CloseUser2',repeat('c',64),'chapter','CloseChap1',1,'materialized')",&[])).await.expect("independent intent must be compatible with signed outer authority").unwrap();drop(inner);outer.rollback().await.unwrap();drop(outer_db);
 let db=state.db.client().await.unwrap();db.execute("INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader) VALUES('CloseGroup','CloseUser3','CloseUser3',TRUE)",&[]).await.unwrap();db.execute("UPDATE platform.sessions SET authenticated_at=clock_timestamp()-interval '11 minutes' WHERE id='closure-session-2'",&[]).await.unwrap();drop(db);native(&client,&origin,token,"POST",prepare,Some(capture("CloseUser2",1,&lifecycle2)),403).await;
 let db=state.db.client().await.unwrap();db.execute("UPDATE platform.sessions SET authenticated_at=clock_timestamp() WHERE id='closure-session-2'",&[]).await.unwrap();db.execute("INSERT INTO platform.protocol_publication_commands(command_id,user_id,input_hash,resource_kind,catalog_id,created_at) VALUES('closure-existing','CloseUser2',$1,'chapter','CloseChap1',1)",&[&"a".repeat(64)]).await.unwrap();drop(db);
 let blocked=native(&client,&origin,token,"POST",prepare,Some(capture("CloseUser2",1,&lifecycle2)),200).await;assert!(blocked["blockers"].as_array().unwrap().iter().any(|b|b["kind"]=="publication-unresolved"));
 let db=state.db.client().await.unwrap();assert_eq!(db.query_one("SELECT state FROM platform.protocol_publication_commands WHERE command_id='closure-existing'",&[]).await.unwrap().get::<_,String>(0),"prepared");db.execute("UPDATE platform.protocol_publication_commands SET state='materialized' WHERE command_id='closure-existing'",&[]).await.unwrap();db.execute("INSERT INTO platform.upload_sessions(id,owner_id,manga_id) VALUES('CloseUpl01','CloseUser2','CloseManga')",&[]).await.unwrap();drop(db);
 let blocked=native(&client,&origin,token,"POST",prepare,Some(capture("CloseUser2",1,&lifecycle2)),200).await;assert!(blocked["blockers"].as_array().unwrap().iter().any(|b|b["kind"]=="upload-active"));let db=state.db.client().await.unwrap();db.execute("UPDATE platform.upload_sessions SET state='deleted' WHERE id='CloseUpl01'",&[]).await.unwrap();drop(db);
 let proof=native(&client,&origin,token,"POST",prepare,Some(capture("CloseUser2",1,&lifecycle2)),200).await;assert_eq!(proof["result"],"prepared");let mut wrong=confirm("CloseUser2",&proof);wrong["acknowledge"]=json!("DELETE");native(&client,&origin,token,"POST",finalize,Some(wrong),409).await;
 let db=state.db.client().await.unwrap();let stored:Vec<u8>=db.query_one("SELECT token_hash FROM platform.account_disable_confirmations WHERE id=$1",&[&proof["confirmationId"].as_str().unwrap()]).await.unwrap().get(0);assert_eq!(stored,Sha256::digest(proof["confirmationToken"].as_str().unwrap().as_bytes()).to_vec());assert_eq!(db.query_one("SELECT count(*) FROM platform.account_lifecycle_audit",&[]).await.unwrap().get::<_,i64>(0),0);drop(db);
 // Existing in-flight actor KEY SHARE excludes closure without waiting/deadlock.
 let mut holder=state.db.client().await.unwrap();let tx=holder.transaction().await.unwrap();tx.query_one("SELECT platform.require_active_account('CloseUser2')",&[]).await.unwrap();native(&client,&origin,token,"POST",finalize,Some(confirm("CloseUser2",&proof)),409).await;tx.rollback().await.unwrap();drop(holder);
 // An already-admitted normal status/role write cannot invalidate the chosen alternate.
 for change in ["disabled=TRUE","verified=FALSE","roles='[\"restricted\"]'::jsonb"] {
  let mut holder=state.db.client().await.unwrap();let tx=holder.transaction().await.unwrap();
  tx.execute(&format!("UPDATE platform.users SET {change} WHERE id='CloseUser3'"),&[]).await.unwrap();
  native(&client,&origin,token,"POST",finalize,Some(confirm("CloseUser2",&proof)),409).await;
  tx.rollback().await.unwrap();
 }
 // A transaction failure after revocation/audit must roll everything back.
 let db=state.db.client().await.unwrap();db.batch_execute("CREATE FUNCTION platform.closure_fixture_fail() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.id='CloseUser2' AND NEW.disabled THEN RAISE EXCEPTION 'fixture rollback'; END IF; RETURN NEW; END $$; CREATE TRIGGER closure_fixture_fail BEFORE UPDATE ON platform.users FOR EACH ROW EXECUTE FUNCTION platform.closure_fixture_fail();").await.unwrap();drop(db);
 native(&client,&origin,token,"POST",finalize,Some(confirm("CloseUser2",&proof)),503).await;
 let db=state.db.client().await.unwrap();assert!(!db.query_one("SELECT disabled FROM platform.users WHERE id='CloseUser2'",&[]).await.unwrap().get::<_,bool>(0));assert_eq!(db.query_one("SELECT count(*) FROM platform.sessions WHERE user_id='CloseUser2' AND revoked_at IS NULL",&[]).await.unwrap().get::<_,i64>(0),3);assert_eq!(db.query_one("SELECT count(*) FROM platform.account_lifecycle_audit",&[]).await.unwrap().get::<_,i64>(0),0);db.batch_execute("DROP TRIGGER closure_fixture_fail ON platform.users; DROP FUNCTION platform.closure_fixture_fail();").await.unwrap();drop(db);
 // Pause actual final UPDATE while its real closure locks remain held.
 let controller=state.db.client().await.unwrap();controller.query_one("SELECT pg_advisory_lock(760531)",&[]).await.unwrap();controller.batch_execute("CREATE FUNCTION platform.closure_fixture_pause() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.id='CloseUser2' AND NEW.disabled THEN PERFORM pg_advisory_xact_lock(760531); END IF; RETURN NEW; END $$; CREATE TRIGGER closure_fixture_pause BEFORE UPDATE ON platform.users FOR EACH ROW EXECUTE FUNCTION platform.closure_fixture_pause();").await.unwrap();
 let c=client.clone();let o=origin.clone();let body=confirm("CloseUser2",&proof);let pending=tokio::spawn(async move{native(&c,&o,Some("closure-token-2"),"POST","/api/account/closure/confirm",Some(body),200).await});wait_pause(&controller).await;
 let db=state.db.client().await.unwrap();for sql in ["INSERT INTO platform.catalog_group_members(group_id,user_id,name) VALUES('CloseGroup','CloseUser4','late')","UPDATE platform.catalog_group_members SET is_leader=FALSE WHERE group_id='CloseGroup' AND user_id='CloseUser3'","DELETE FROM platform.catalog_group_members WHERE group_id='CloseGroup' AND user_id='CloseUser3'","INSERT INTO platform.protocol_publication_commands(command_id,user_id,input_hash,resource_kind,catalog_id,created_at) VALUES('closure-late','CloseUser2',repeat('b',64),'chapter','CloseChap1',1)"]{let error=db.execute(sql,&[]).await.unwrap_err();assert_eq!(error.code().unwrap().code(),"55P03", "{sql}");}drop(db);
 // Reverse ordering: actual ordinary status updates wait for the closure's SHARE
 // eligibility lock, and a bounded timeout proves they cannot commit underneath it.
 for change in ["disabled=TRUE","verified=FALSE","roles='[\"restricted\"]'::jsonb"] {
  let mut writer=state.db.client().await.unwrap();let tx=writer.transaction().await.unwrap();
  tx.batch_execute("SET LOCAL lock_timeout='100ms'; SET LOCAL statement_timeout='1s';").await.unwrap();
  let error=tx.execute(&format!("UPDATE platform.users SET {change} WHERE id='CloseUser3'"),&[]).await.unwrap_err();assert_eq!(error.code().unwrap().code(),"55P03");tx.rollback().await.unwrap();
 }
 controller.query_one("SELECT pg_advisory_unlock(760531)",&[]).await.unwrap();let closed=pending.await.unwrap();assert_eq!(closed["disabled"],true);assert_eq!(closed["version"],2);controller.batch_execute("DROP TRIGGER closure_fixture_pause ON platform.users; DROP FUNCTION platform.closure_fixture_pause();").await.unwrap();drop(controller);
 for old in ["closure-token-2","closure-extra-token","closure-recovery-token"]{native(&client,&origin,Some(old),"GET",preview,None,401).await;}
 call(&client,&origin,"GET","/api/v1/user/me",None,Some(2),403).await;original.publisher_permit(id(2)).await.unwrap();
 let public_profile=client.get(format!("{origin}/api/v1/user/{public2}")).send().await.unwrap();assert_eq!(public_profile.status().as_u16(),200);let tombstone:Value=public_profile.json().await.unwrap();assert_eq!(tombstone["data"]["attributes"]["accountState"],"disabled");assert!(!tombstone.to_string().contains("PRIVATE_"));assert_eq!(tombstone["data"]["attributes"]["username"],"CloseUser2");
 let db=state.db.client().await.unwrap();let counts=db.query_one("SELECT (SELECT count(*) FROM platform.sessions WHERE user_id='CloseUser2' AND revoked_at IS NULL),(SELECT count(*) FROM platform.api_clients WHERE user_id='CloseUser2' AND revoked_at IS NULL),(SELECT count(*) FROM platform.protocol_identity_bindings WHERE user_id='CloseUser2' AND revoked_at IS NULL),(SELECT count(*) FROM platform.auth_email_tokens WHERE user_id='CloseUser2' AND used_at IS NULL),(SELECT count(*) FROM platform.account_lifecycle_audit WHERE user_id='CloseUser2' AND operation='disable'),(SELECT count(*) FROM platform.community_replies WHERE author_id='CloseUser2'),(SELECT count(*) FROM platform.catalog_chapters WHERE uploader_id='CloseUser2')",&[]).await.unwrap();for i in 0..4{assert_eq!(counts.get::<_,i64>(i),0);}for i in 4..7{assert_eq!(counts.get::<_,i64>(i),1);}let attribution:Value=db.query_one("SELECT platform.community_user_json('CloseUser2')",&[]).await.unwrap().get(0);assert_eq!(attribution["name"],"CloseUser2");assert_eq!(attribution["account_state"],"disabled");assert!(!attribution.to_string().contains("PRIVATE_"));
 for sql in ["SELECT platform.require_active_account('CloseUser2')","INSERT INTO platform.protocol_publication_commands(command_id,user_id,input_hash,resource_kind,catalog_id,created_at) VALUES('closure-late','CloseUser2',repeat('b',64),'chapter','CloseChap1',1)","UPDATE platform.catalog_group_members SET is_officer=TRUE WHERE group_id='CloseGroup' AND user_id='CloseUser2'"]{let error=db.execute(sql,&[]).await.unwrap_err();assert_eq!(error.code().unwrap().code(),"42501");}drop(db);
 // Stale actor native core cannot mutate after closure even without re-running HTTP auth.
 let actor=weebdex_platform::domains::auth::SessionPrincipal{user_id:"CloseUser2".into(),username:"CloseUser2".into(),email:"close2@fixture.invalid".into(),verified:true,roles:vec!["user".into()],permissions:vec!["self.chapter.update".into()],session_id:"closure-session-2".into(),is_api_client:false,authenticated_age_seconds:0,recovery_session:false};let mut db=state.db.client().await.unwrap();let tx=db.transaction().await.unwrap();let denied=weebdex_platform::domains::catalog::CatalogRepository::update_chapter(&tx,"CloseChap1",&actor,&json!({"version":1,"title":"stale"})).await.unwrap_err();assert_eq!(denied.status.as_u16(),403);tx.rollback().await.unwrap();drop(db);
 // The only remaining active leader cannot close; historical closed leader is not counted.
 let blocker=native(&client,&origin,Some("closure-token-3"),"GET","/api/account/closure/CloseUser3",None,200).await;assert_eq!(blocker["blockers"][0]["kind"],"sole-group-leader");
 // Signed preparation is not disable, token is not authority, and actual signed closure revokes native sessions.
 let signed_preview=format!("/api/v1/nostr/user/{public4}/closure?expectedAccountId={public4}");call(&client,&origin,"GET",&signed_preview,None,None,401).await;let observed=call(&client,&origin,"GET",&signed_preview,None,Some(4),200).await;assert_eq!(observed["accountId"],public4.as_str());
 let path=format!("/api/v1/user/{public4}");let signed=call(&client,&origin,"DELETE",&path,Some(capture(public4.as_str(),1,&observed["lifecycleId"])),Some(4),200).await;assert_eq!(signed["result"],"prepared");let finish=format!("/api/v1/user/delete/{}",signed["confirmationId"].as_str().unwrap());call(&client,&origin,"POST",&finish,Some(confirm(public4.as_str(),&signed)),None,401).await;call(&client,&origin,"POST",&finish,Some(confirm(public4.as_str(),&signed)),Some(3),409).await;let result=call(&client,&origin,"POST",&finish,Some(confirm(public4.as_str(),&signed)),Some(4),200).await;assert_eq!(result["disabled"],true);call(&client,&origin,"POST",&finish,Some(confirm(public4.as_str(),&signed)),Some(4),403).await;native(&client,&origin,Some("closure-token-4"),"GET","/api/account/closure/CloseUser4",None,401).await;
 // Synthetic existing-administrator recovery follows the documented exact CAS
 // and audit procedure. No old credential/binding is revived; a fresh ordinary
 // session is necessary for a second lifecycle.
 let mut admin=state.db.client().await.unwrap();let tx=admin.transaction().await.unwrap();
 assert_eq!(tx.query_one("SELECT version FROM platform.users WHERE id='CloseUser4' AND disabled AND closed_at IS NOT NULL FOR UPDATE NOWAIT",&[]).await.unwrap().get::<_,i32>(0),2);
 assert_eq!(tx.execute("UPDATE platform.users SET disabled=FALSE,closed_at=NULL,version=version+1,updated_at=clock_timestamp() WHERE id='CloseUser4' AND version=2 AND disabled AND closed_at IS NOT NULL",&[]).await.unwrap(),1);
 tx.execute("INSERT INTO platform.account_lifecycle_audit(user_id,operation,authority,old_version,new_version) VALUES('CloseUser4','reactivate','fixture-existing-administrator:case-1',2,3)",&[]).await.unwrap();tx.commit().await.unwrap();drop(admin);
 native(&client,&origin,Some("closure-token-4"),"GET","/api/account/closure/CloseUser4",None,401).await;call(&client,&origin,"GET","/api/v1/user/me",None,Some(4),403).await;
 let db=state.db.client().await.unwrap();assert_eq!(db.query_one("SELECT count(*) FROM platform.protocol_identity_bindings WHERE user_id='CloseUser4' AND revoked_at IS NULL",&[]).await.unwrap().get::<_,i64>(0),0);
 db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES('closure-new-session-4','CloseUser4',$1,clock_timestamp()+interval '1 hour')",&[&Sha256::digest(b"closure-new-token-4").to_vec()]).await.unwrap();drop(db);
 let fresh=native(&client,&origin,Some("closure-new-token-4"),"POST",prepare,Some(capture("CloseUser4",3,&observed["lifecycleId"])),200).await;assert_ne!(fresh["confirmationId"],signed["confirmationId"]);let again=native(&client,&origin,Some("closure-new-token-4"),"POST",finalize,Some(confirm("CloseUser4",&fresh)),200).await;assert_eq!(again["version"],4);assert_eq!(fresh["lifecycleId"],observed["lifecycleId"]);assert_eq!(again["lifecycleId"],observed["lifecycleId"]);
 bounded_contract::revision_headroom(&state,&original,&client,&origin).await;

}
