//! Synthetic-only signed moderator queue/context/decision HTTP qualification.
//! Actual TCP, original request authorization and private instance moderation.
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
    let response = request.body(body.clone()).send().await.unwrap();
    let status = response.status().as_u16();
    assert!(
        response.headers()["cache-control"]
            .to_str()
            .unwrap()
            .contains("no-store")
    );
    assert!(response.headers()["cache-control"].to_str().unwrap().contains("private"));
    assert!(response.headers().get_all("vary").iter().any(|v|v.to_str().unwrap().contains("Authorization")));
    assert!(response.headers().get_all("vary").iter().any(|v|v.to_str().unwrap().contains("Cookie")));
    let value: Value = response.json().await.unwrap();
    assert_eq!(status, expected, "{method} {path}: {value}");
    if status == 200 && method == "POST" {
        assert_eq!(value["nostr"]["requestHash"],auth::hash(&body));
        assert_eq!(value["nostr"]["requestMethod"],method);
        assert_eq!(value["nostr"]["requestPath"],path);
    }
    value
}
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires isolated scripts/run-auth-tests.sh PostgreSQL"]
async fn signed_moderator_decisions_are_private_atomic_and_fresh() {
    use weebdex_platform::public_api::{PostgresPublicIdResolver, PublicIdResolver, ResourceKind};
    let dsn=std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use isolated runner");
    let pg:tokio_postgres::Config=dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(),Some("weebdex_platform_test"));
    assert!(!pg.get_hosts().is_empty());
    assert!(pg.get_hosts().iter().all(|h|matches!(h,tokio_postgres::config::Host::Tcp(h) if h=="127.0.0.1")));
    assert!(pg.get_ports().iter().all(|p| *p>1024 && *p!=5432));
    let runtime=std::path::PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap());
    assert!(runtime.to_str().unwrap().contains("/weebdex-platform/test-runtime/"));
    assert!(runtime.join("fixture.json").is_file());
    let original=Repository::connect(DbConfig{dsn:dsn.clone(),connections:2,acquisition_timeout:Duration::from_secs(2),cache_weight:1048576,plaintext_local:true}).await.unwrap();
    original.migrate().await.unwrap();original.bind_instance(id(11),id(1)).await.unwrap();
    for key in [2,3,4,5,6,7,8] {original.set_account_admission(id(key),true).await.unwrap();}
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let origin=format!("http://{}",listener.local_addr().unwrap());
    let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:10,uploads_dir:runtime.join("media"),ui_dir:runtime.join("no-ui"),protocol_publication:Some(OperatorRuntimeConfig{operator_key_file:runtime.join("unused/operator.key"),expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),storage:json!({"path":runtime.join("unused/storage"),"volume":"reportsfixture","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144}),group_keys_dir:None,backend:None,native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":runtime.join("unused/service.key"),"admin_pubkey":public(1)}))}),..Config::default()},dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();state.protocol.verify_schema().await.unwrap();
    // Match the explicit production migration bootstrap, not its fail-closed placeholder.
    state.protocol.install_visibility_function().await.unwrap();
    let db=state.db.client().await.unwrap();
    for (key,user,roles,permissions,disabled) in [
        (2,"ModReviewer",json!(["moderator"]),json!(["report.update","scanlation_group.update","manga.delete","chapter.remove"]),false),
        (4,"ModSubmitter",json!(["user"]),json!(["report.update"]),false),
        (5,"ModDisabled",json!(["moderator"]),json!(["report.update"]),true),
        (6,"ModLimited",json!(["moderator"]),json!(["report.update"]),false),
        (7,"ModNoGrant",json!(["moderator"]),json!([]),false),
        (8,"ModDeleteOnly",json!(["moderator"]),json!(["report.delete"]),false),
    ] {
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,disabled,roles,permissions) VALUES($1,$1,$2,'synthetic',TRUE,FALSE,$3,$4)",&[&user,&format!("{user}@fixture.invalid"),&roles,&permissions]).await.unwrap();
        db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await.unwrap();
        if disabled { db.execute("UPDATE platform.users SET disabled=TRUE WHERE id=$1",&[&user]).await.unwrap(); }
    }
    db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified) VALUES('ModTarget','ModTarget','target@fixture.invalid','synthetic',TRUE)",&[]).await.unwrap();
    db.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES('ModManga','ModSubmitter','Submitter','Synthetic target','ja','ongoing','safe','published')",&[]).await.unwrap();
    db.execute("INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,title,language) VALUES('ModChapter','ModManga','ModSubmitter','Submitter','Synthetic chapter','ja')",&[]).await.unwrap();
    db.execute("INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES('ModGroup','ModSubmitter','Submitter','Unclaimed group')",&[]).await.unwrap();
    db.execute("INSERT INTO platform.catalog_threads(id,entity_type,entity_id,author_id,author_name,subject,body) VALUES('ModThread','chapter','ModChapter','ModSubmitter','Submitter','Native thread','Actual bounded thread body')",&[]).await.unwrap();
    let reply:i64=db.query_one("INSERT INTO platform.community_replies(thread_id,reply_number,author_id,body) VALUES('ModThread',1,'ModSubmitter','Actual reply') RETURNING id",&[]).await.unwrap().get(0);
    for (report,category,target,reason,version) in [
        ("generic","user","ModTarget","user_spam",None),
        ("refuse","user","ModTarget","user_other",None),
        ("claim","scanlation_group","ModGroup","scanlation_group_claim",Some(1_i32)),
        ("chapter","chapter","ModChapter","chapter_removal_request",Some(1_i32)),
        ("manga","manga","ModManga","manga_removal_request",Some(1_i32)),
        ("thread","thread","ModThread","thread_spam",None),
    ] {db.execute("INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,body,target_version) VALUES($1,'ModSubmitter',$2,$3,$4,'Private synthetic evidence',$5)",&[&report,&category,&target,&reason,&version]).await.unwrap();}
    db.execute("INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,body) VALUES('reply','ModSubmitter','reply',$1,'reply_spam','Private reply evidence')",&[&reply.to_string()]).await.unwrap();
    // > old offset ceiling, same category/status and microsecond timestamp ties.
    // Keyset traversal is exercised near the far boundary, without huge output.
    // Preserve all 10,010 pagination witnesses without retaining 10,010 unique
    // revision/advisory locks in one seed transaction under the normal PG budget.
    for first in (1_i32..=10010).step_by(128) {
        let last=(first+127).min(10010);
        db.execute("INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,body,status,created_at) SELECT 'old_'||lpad(n::text,5,'0'),'ModSubmitter','user','ModTarget','user_incorrect','Older history','refused','2026-01-01T00:00:00.123456Z'::timestamptz FROM generate_series($1::integer,$2::integer)n",&[&first,&last]).await.unwrap();
    }
    assert_eq!(db.query_one("SELECT count(*) FROM platform.reports WHERE id LIKE 'old_%'",&[]).await.unwrap().get::<_,i64>(0),10010);
    db.batch_execute("INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,title,language,protocol_event_id) VALUES('ModSourceChapter','ModManga','ModSubmitter','Submitter','Sourced synthetic chapter','ja',repeat('45',32)); INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,body) VALUES('source','ModSubmitter','chapter','ModSourceChapter','chapter_spam','Source witness report'); INSERT INTO capacity_reservations(id,requested,actual,state) VALUES(decode(repeat('41',32),'hex'),1,1,'settled'); INSERT INTO blob_intents(hash,generation,volume,owner,size,state,reservation,verified_at) VALUES(decode(repeat('42',32),'hex'),decode(repeat('43',16),'hex'),'moderator-fixture',decode(repeat('44',32),'hex'),1,'available',decode(repeat('41',32),'hex'),clock_timestamp()); INSERT INTO events(id,publisher,kind,created_at,raw_event,state,manifest) VALUES(decode(repeat('45',32),'hex'),decode(repeat('44',32),'hex'),1002,1,'{}','hosted',decode(repeat('42',32),'hex')); INSERT INTO publication_blobs(event_id,hash,generation,mime) VALUES(decode(repeat('45',32),'hex'),decode(repeat('42',32),'hex'),decode(repeat('43',16),'hex'),'application/cbor');").await.unwrap();
    let resolver=PostgresPublicIdResolver::new(state.db.clone());let actor=resolver.preferred(ResourceKind::User,"ModReviewer").await.unwrap().unwrap();let ordinary=resolver.preferred(ResourceKind::User,"ModSubmitter").await.unwrap().unwrap();let limited=resolver.preferred(ResourceKind::User,"ModLimited").await.unwrap().unwrap();drop(db);
    let _server=Server(tokio::spawn({let state=state.clone();async move{axum::serve(listener,weebdex_platform::router::router(state)).await.unwrap();}}));
    let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build().unwrap();let base="/api/v1/nostr/moderation/reports";let list=format!("{base}?expectedAccountId={actor}");let detail=|id:&str|format!("{base}/{id}?expectedAccountId={actor}");
    call(&client,&origin,"GET",&list,None,None,401).await;
    for key in [3,4,5,7] {call(&client,&origin,"GET",&list,None,Some(key),if key==4||key==7 {409} else {403}).await;}
    call(&client,&origin,"GET",&format!("{base}?expectedAccountId={ordinary}"),None,Some(4),403).await;
    call(&client,&origin,"GET",base,None,Some(2),400).await;
    for suffix in ["&page=2001","&cursor=bad","&category=source_event","&category=user&category=reply","&status=resolved","&owner=Other","&scope=mine"] {call(&client,&origin,"GET",&format!("{list}{suffix}"),None,Some(2),400).await;}
    let first=call(&client,&origin,"GET",&list,None,Some(2),200).await;assert_eq!(first["limit"],5);assert_eq!(first["data"].as_array().unwrap().len(),5);assert!(first["nextCursor"].is_string());assert!(!first.to_string().contains("Private synthetic evidence"));assert!(!first.to_string().contains("fixture.invalid"));
    let next=first["nextCursor"].as_str().unwrap();let mut query=url::form_urlencoded::Serializer::new(String::new());query.append_pair("expectedAccountId",&actor.to_string()).append_pair("cursor",next);let second=call(&client,&origin,"GET",&format!("{base}?{}",query.finish()),None,Some(2),200).await;assert!(second["data"].as_array().unwrap().iter().all(|row|!first["data"].as_array().unwrap().iter().any(|other|row["id"]==other["id"])));
    // Canonical server cursor field order is explicit, not serde Value map order.
    let far=format!("{{\"at\":\"2026-01-01T00:00:00.123456Z\",\"id\":\"old_00006\",\"accountId\":\"{actor}\",\"category\":\"user\",\"status\":\"refused\",\"object\":null,\"reason\":null,\"order\":\"desc\"}}");let mut q=url::form_urlencoded::Serializer::new(String::new());q.append_pair("expectedAccountId",&actor.to_string()).append_pair("category","user").append_pair("status","refused").append_pair("cursor",&far);let tail=call(&client,&origin,"GET",&format!("{base}?{}",q.finish()),None,Some(2),200).await;assert_eq!(tail["data"][0]["id"],"old_00005");assert_eq!(tail["data"].as_array().unwrap().len(),5);assert!(tail["nextCursor"].is_null());
    for (category,status,account,c) in [("chapter","waiting",actor.to_string(),next.to_owned()),("user","accepted",actor.to_string(),far.clone()),("user","refused",ordinary.to_string(),far.clone()),("user","refused",actor.to_string(),far.replace(".123456Z",".12345Z"))] {let mut q=url::form_urlencoded::Serializer::new(String::new());q.append_pair("expectedAccountId",&account).append_pair("category",category).append_pair("status",status).append_pair("cursor",&c);call(&client,&origin,"GET",&format!("{base}?{}",q.finish()),None,Some(2),400).await;}
    let current=call(&client,&origin,"GET",&detail("generic"),None,Some(2),200).await;let command=|context:&Value,action:&str|json!({"expectedAccountId":actor,"expectedReportVersion":context["data"]["report"]["version"],"expectedReasonVersion":context["data"]["reason"]["version"],"expectedTargetRevision":context["data"]["targetRevision"],"expectedContextHash":context["data"]["contextHash"],"expectedEffect":if action=="accept"{context["data"]["acceptEffect"].clone()}else{json!({})},"action":action,"notes":"Exact moderator notes"});
    let good=command(&current,"accept");let post=format!("{base}/generic");
    for (field,value) in [("expectedAccountId",json!(ordinary)),("expectedReportVersion",json!(2)),("expectedReasonVersion",json!(999)),("expectedTargetRevision",json!("a".repeat(64))),("expectedContextHash",json!("b".repeat(64))),("expectedEffect",json!({"action":"other"}))] {let mut bad=good.clone();bad[field]=value;call(&client,&origin,"POST",&post,Some(bad),Some(2),409).await;}
    for (sql,restore) in [
        ("UPDATE platform.users SET roles='[\"user\"]' WHERE id='ModReviewer'","UPDATE platform.users SET roles='[\"moderator\"]' WHERE id='ModReviewer'"),
        ("UPDATE platform.users SET permissions='[]' WHERE id='ModReviewer'","UPDATE platform.users SET permissions='[\"report.update\",\"scanlation_group.update\",\"manga.delete\",\"chapter.remove\"]' WHERE id='ModReviewer'"),
        ("UPDATE platform.users SET disabled=TRUE WHERE id='ModReviewer'","UPDATE platform.users SET disabled=FALSE WHERE id='ModReviewer'"),
    ] {state.db.client().await.unwrap().execute(sql,&[]).await.unwrap();call(&client,&origin,"POST",&post,Some(good.clone()),Some(2),403).await;state.db.client().await.unwrap().execute(restore,&[]).await.unwrap();}
    original.set_account_admission(id(2),false).await.unwrap();call(&client,&origin,"POST",&post,Some(good.clone()),Some(2),403).await;original.set_account_admission(id(2),true).await.unwrap();
    // Target lock conflict is bounded before any native effect or audit.
    let mut blocker=state.db.client().await.unwrap();let blocked=blocker.transaction().await.unwrap();blocked.query_one("SELECT id FROM platform.users WHERE id='ModTarget' FOR UPDATE",&[]).await.unwrap();let start=std::time::Instant::now();call(&client,&origin,"POST",&post,Some(good.clone()),Some(2),409).await;assert!(start.elapsed()<Duration::from_secs(3));blocked.rollback().await.unwrap();
    state.db.client().await.unwrap().execute("UPDATE platform.users SET version=version+1 WHERE id='ModTarget'",&[]).await.unwrap();call(&client,&origin,"POST",&post,Some(good),Some(2),409).await;
    let refreshed=call(&client,&origin,"GET",&detail("generic"),None,Some(2),200).await;let success=call(&client,&origin,"POST",&post,Some(command(&refreshed,"accept")),Some(2),200).await;assert_eq!(success["data"]["effect"],json!({}));assert_eq!(success["data"]["version"],2);call(&client,&origin,"POST",&post,Some(command(&refreshed,"accept")),Some(2),409).await;
    let thread=call(&client,&origin,"GET",&detail("thread"),None,Some(2),200).await;assert_eq!(thread["data"]["target"]["parent"]["category"],"chapter");assert_eq!(thread["data"]["target"]["parent"]["parent"]["id"],"ModManga");let r=call(&client,&origin,"GET",&detail("reply"),None,Some(2),200).await;assert_eq!(r["data"]["target"]["parent"]["parent"]["id"],"ModManga");
    // Read-only personal inspection remains unconditional owner scope for staff.
    let personal=format!("/api/v1/nostr/account/reports/native?expectedAccountId={actor}");let url=format!("{origin}{personal}");let personal=client.get(&url).header("authorization",proof(2,&url,"GET",b"")).send().await.unwrap();assert_eq!(personal.status().as_u16(),200);assert_eq!(personal.json::<Value>().await.unwrap()["total"],0);
    let no_grant=call(&client,&origin,"GET",&format!("{base}/claim?expectedAccountId={limited}"),None,Some(6),200).await;assert_eq!(no_grant["data"]["acceptAllowed"],false);
    let claim=call(&client,&origin,"GET",&detail("claim"),None,Some(2),200).await;state.db.client().await.unwrap().execute("UPDATE platform.users SET verified=FALSE WHERE id='ModSubmitter'",&[]).await.unwrap();call(&client,&origin,"POST",&format!("{base}/claim"),Some(command(&claim,"accept")),Some(2),409).await;state.db.client().await.unwrap().execute("UPDATE platform.users SET verified=TRUE WHERE id='ModSubmitter'",&[]).await.unwrap();
    // Exact live source/negative-membership fences and nested grandparent authority.
    let sourced=call(&client,&origin,"GET",&detail("source"),None,Some(2),200).await;
    assert_eq!(sourced["data"]["target"]["available"],true);
    for (lock,withdraw,restore) in [
        ("SELECT id FROM public.events WHERE id=decode(repeat('45',32),'hex') FOR UPDATE", "UPDATE public.events SET state='withdrawing' WHERE id=decode(repeat('45',32),'hex'); INSERT INTO backend_outbox(event_id,action,payload,event_version) VALUES(decode(repeat('45',32),'hex'),'delete','{}',1)", "UPDATE public.events SET state='hosted' WHERE id=decode(repeat('45',32),'hex'); UPDATE backend_outbox SET completed_at=clock_timestamp() WHERE event_id=decode(repeat('45',32),'hex')"),
        ("SELECT hash FROM public.blob_intents WHERE hash=decode(repeat('42',32),'hex') FOR UPDATE", "INSERT INTO quarantined_blobs(hash,generation,reason) VALUES(decode(repeat('42',32),'hex'),decode(repeat('43',16),'hex'),'audit_failed')", "DELETE FROM quarantined_blobs WHERE hash=decode(repeat('42',32),'hex')"),
        ("SELECT id FROM platform.catalog_manga WHERE id='ModManga' FOR UPDATE", "UPDATE platform.catalog_manga SET protocol_source_visible=FALSE WHERE id='ModManga'", "UPDATE platform.catalog_manga SET protocol_source_visible=TRUE WHERE id='ModManga'"),
    ] {
        let mut blocker=state.db.client().await.unwrap();let blocked=blocker.transaction().await.unwrap();blocked.query(lock,&[]).await.unwrap();let start=std::time::Instant::now();call(&client,&origin,"POST",&format!("{base}/source"),Some(command(&sourced,"accept")),Some(2),409).await;assert!(start.elapsed()<Duration::from_secs(3));blocked.batch_execute(withdraw).await.unwrap();blocked.commit().await.unwrap();drop(blocker);
        call(&client,&origin,"POST",&format!("{base}/source"),Some(command(&sourced,"accept")),Some(2),409).await;
        let unchanged=state.db.client().await.unwrap().query_one("SELECT status,version,(SELECT count(*) FROM platform.report_audit WHERE report_id='source') FROM platform.reports WHERE id='source'",&[]).await.unwrap();
        assert_eq!(unchanged.get::<_,String>(0),"waiting");assert_eq!(unchanged.get::<_,i32>(1),1);assert_eq!(unchanged.get::<_,i64>(2),0);
        let hidden=call(&client,&origin,"GET",&detail("source"),None,Some(2),200).await;assert_eq!(hidden["data"]["target"]["available"],false);assert!(!hidden.to_string().contains("Sourced synthetic chapter"));state.db.client().await.unwrap().batch_execute(restore).await.unwrap();
    }
    for report in ["claim","chapter","manga"] {let c=call(&client,&origin,"GET",&detail(report),None,Some(2),200).await;assert_eq!(c["data"]["acceptAllowed"],true,"{report}: {c}");let receipt=call(&client,&origin,"POST",&format!("{base}/{report}"),Some(command(&c,"accept")),Some(2),200).await;assert_eq!(receipt["data"]["effect"],c["data"]["acceptEffect"]);}
    let refusal=call(&client,&origin,"GET",&detail("refuse"),None,Some(2),200).await;let mut notes=command(&refusal,"refuse");notes["notes"]=json!("\u{1}".repeat(20000));call(&client,&origin,"POST",&format!("{base}/refuse"),Some(notes),Some(2),200).await;
    let db=state.db.client().await.unwrap();assert!(db.query_one("SELECT is_leader FROM platform.catalog_group_members WHERE group_id='ModGroup' AND user_id='ModSubmitter'",&[]).await.unwrap().get::<_,bool>(0));assert!(db.query_one("SELECT deleted_at IS NOT NULL FROM platform.catalog_chapters WHERE id='ModChapter'",&[]).await.unwrap().get::<_,bool>(0));assert!(db.query_one("SELECT deleted_at IS NOT NULL FROM platform.catalog_manga WHERE id='ModManga'",&[]).await.unwrap().get::<_,bool>(0));assert_eq!(db.query_one("SELECT count(*) FROM platform.report_audit WHERE actor_id='ModReviewer' AND operation IN('accept','reject')",&[]).await.unwrap().get::<_,i64>(0),5);
    assert_eq!(db.query_one("SELECT count(*) FROM public.events",&[]).await.unwrap().get::<_,i64>(0),1,"No moderation event published beyond the source fixture");
    // Report-only deletion parity, using only synthetic disposable rows. The
    // native shared core must not depend on live target/reason expansion.
    let delete_actor=resolver.preferred(ResourceKind::User,"ModDeleteOnly").await.unwrap().unwrap();
    let deletion_path=|report:&str|format!("{base}/{report}?expectedAccountId={delete_actor}&intent=delete");
    let deletion_command=|value:&Value|json!({"action":"delete","expectedAccountId":delete_actor,"expectedAuthorityRevision":value["data"]["authority"]["revision"],"expectedReportVersion":value["data"]["report"]["version"],"expectedContextHash":value["data"]["contextHash"],"expectedEffect":value["data"]["effect"]});
    call(&client,&origin,"GET",&format!("{base}?expectedAccountId={delete_actor}"),None,Some(8),403).await;
    call(&client,&origin,"GET",&format!("{base}/generic?expectedAccountId={delete_actor}"),None,Some(8),403).await;
    for status in ["waiting","accepted","refused","autoresolved"]{
        let report=format!("delete_{status}");
        db.execute("INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,status,body,notes,reviewed_by) VALUES($1,'ModSubmitter','user','MissingHistoricalTarget','user_spam',$2,'HIDDEN_DELETE_BODY','HIDDEN_OLD_NOTES','ModReviewer')",&[&report,&status]).await.unwrap();
        let value=call(&client,&origin,"GET",&deletion_path(&report),None,Some(8),200).await;
        assert_eq!(value["data"]["report"]["status"],status);assert_eq!(value["data"]["intent"],"delete");
        for hidden in ["HIDDEN_DELETE_BODY","HIDDEN_OLD_NOTES","submitterId","claimant","targetRevision","fixture.invalid"]{assert!(!value.to_string().contains(hidden));}
        let mut wrong=deletion_command(&value);wrong["action"]=json!("accept");call(&client,&origin,"POST",&format!("{base}/{report}"),Some(wrong),Some(8),400).await;
        let response=call(&client,&origin,"POST",&format!("{base}/{report}"),Some(deletion_command(&value)),Some(8),200).await;
        assert_eq!(response["data"]["oldStatus"],status);assert_eq!(response["data"]["status"],status);assert_eq!(response["data"]["deleted"],true);assert_eq!(response["data"]["version"],2);
        let row=db.query_one("SELECT status,notes,reviewed_by,body,deleted_at IS NOT NULL,version FROM platform.reports WHERE id=$1",&[&report]).await.unwrap();
        assert_eq!(row.get::<_,String>(0),status);assert_eq!(row.get::<_,String>(1),"HIDDEN_OLD_NOTES");assert_eq!(row.get::<_,String>(2),"ModReviewer");assert_eq!(row.get::<_,String>(3),"HIDDEN_DELETE_BODY");assert!(row.get::<_,bool>(4));assert_eq!(row.get::<_,i32>(5),2);
        let audit=db.query_one("SELECT operation,old_status,new_status,old_version,new_version,effect,notes FROM platform.report_audit WHERE report_id=$1",&[&report]).await.unwrap();assert_eq!(audit.get::<_,String>(0),"delete");assert_eq!(audit.get::<_,String>(1),status);assert_eq!(audit.get::<_,String>(2),status);assert_eq!(audit.get::<_,i32>(3),1);assert_eq!(audit.get::<_,i32>(4),2);assert_eq!(audit.get::<_,Value>(5),json!({}));assert!(audit.get::<_,Option<String>>(6).is_none());
        call(&client,&origin,"POST",&format!("{base}/{report}"),Some(deletion_command(&value)),Some(8),404).await;
        call(&client,&origin,"GET",&deletion_path(&report),None,Some(8),404).await;
    }
    db.execute("INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,status) VALUES('delete_fresh','ModSubmitter','manga','NoSuchTitle','manga_removal_request','accepted')",&[]).await.unwrap();
    call(&client,&origin,"GET",&format!("{base}/delete_fresh?expectedAccountId={limited}&intent=delete"),None,Some(6),403).await;
    db.execute("UPDATE platform.users SET permissions='[\"report.delete\"]' WHERE id='ModSubmitter'",&[]).await.unwrap();
    call(&client,&origin,"GET",&format!("{base}/delete_fresh?expectedAccountId={ordinary}&intent=delete"),None,Some(4),403).await;
    let stale=call(&client,&origin,"GET",&deletion_path("delete_fresh"),None,Some(8),200).await;
    db.execute("UPDATE platform.users SET version=version+1 WHERE id='ModDeleteOnly'",&[]).await.unwrap();
    call(&client,&origin,"POST",&format!("{base}/delete_fresh"),Some(deletion_command(&stale)),Some(8),409).await;
    let stale=call(&client,&origin,"GET",&deletion_path("delete_fresh"),None,Some(8),200).await;
    db.execute("UPDATE platform.users SET roles='[\"staff\"]' WHERE id='ModDeleteOnly'",&[]).await.unwrap();
    call(&client,&origin,"POST",&format!("{base}/delete_fresh"),Some(deletion_command(&stale)),Some(8),409).await;
    let fresh=call(&client,&origin,"GET",&deletion_path("delete_fresh"),None,Some(8),200).await;
    db.execute("UPDATE platform.users SET permissions='[]' WHERE id='ModDeleteOnly'",&[]).await.unwrap();call(&client,&origin,"POST",&format!("{base}/delete_fresh"),Some(deletion_command(&fresh)),Some(8),403).await;
    db.execute("UPDATE platform.users SET permissions='[\"report.delete\"]' WHERE id='ModDeleteOnly'",&[]).await.unwrap();
    let mut blocker=state.db.client().await.unwrap();let blocked=blocker.transaction().await.unwrap();blocked.query_one("SELECT id FROM platform.reports WHERE id='delete_fresh' FOR UPDATE",&[]).await.unwrap();let start=std::time::Instant::now();call(&client,&origin,"POST",&format!("{base}/delete_fresh"),Some(deletion_command(&fresh)),Some(8),409).await;assert!(start.elapsed()<Duration::from_secs(3));blocked.rollback().await.unwrap();
    db.execute("UPDATE platform.reports SET version=version+1 WHERE id='delete_fresh'",&[]).await.unwrap();call(&client,&origin,"POST",&format!("{base}/delete_fresh"),Some(deletion_command(&fresh)),Some(8),409).await;
    let fresh=call(&client,&origin,"GET",&deletion_path("delete_fresh"),None,Some(8),200).await;call(&client,&origin,"POST",&format!("{base}/delete_fresh"),Some(deletion_command(&fresh)),Some(8),200).await;
    // Hidden source and held unrelated target locks must not obstruct deletion.
    db.execute("UPDATE platform.catalog_manga SET protocol_source_visible=FALSE WHERE id='ModManga'",&[]).await.unwrap();
    let target_before:Value=db.query_one("SELECT to_jsonb(m) FROM platform.catalog_manga m WHERE id='ModManga'",&[]).await.unwrap().get(0);
    let blocked=blocker.transaction().await.unwrap();blocked.query_one("SELECT id FROM platform.catalog_chapters WHERE id='ModSourceChapter' FOR UPDATE",&[]).await.unwrap();
    let source=call(&client,&origin,"GET",&deletion_path("source"),None,Some(8),200).await;call(&client,&origin,"POST",&format!("{base}/source"),Some(deletion_command(&source)),Some(8),200).await;blocked.rollback().await.unwrap();
    assert_eq!(target_before,db.query_one("SELECT to_jsonb(m) FROM platform.catalog_manga m WHERE id='ModManga'",&[]).await.unwrap().get::<_,Value>(0));assert_eq!(db.query_one("SELECT count(*) FROM public.events",&[]).await.unwrap().get::<_,i64>(0),1);
    let asc=format!("{list}&category=user&status=refused&object=ModTarget&reason=user_incorrect&order=asc");let first=call(&client,&origin,"GET",&asc,None,Some(2),200).await;assert_eq!(first["data"][0]["id"],"old_00001");assert_eq!(first["data"][4]["id"],"old_00005");
    let next=first["nextCursor"].as_str().unwrap();let mut q=url::form_urlencoded::Serializer::new(String::new());q.append_pair("cursor",next);let suffix=q.finish();let second=call(&client,&origin,"GET",&format!("{asc}&{suffix}"),None,Some(2),200).await;assert_eq!(second["data"][0]["id"],"old_00006");
    for wrong in [asc.replace("order=asc","order=desc"),asc.replace("object=ModTarget","object=Other"),asc.replace("reason=user_incorrect","reason=user_spam")]{call(&client,&origin,"GET",&format!("{wrong}&{suffix}"),None,Some(2),400).await;}
    for suffix in ["&object=x&object=y","&reason=x&reason=y","&order=asc&order=desc","&object=bad/id","&reason=","&intent=delete"]{call(&client,&origin,"GET",&format!("{list}{suffix}"),None,Some(2),400).await;}

}
