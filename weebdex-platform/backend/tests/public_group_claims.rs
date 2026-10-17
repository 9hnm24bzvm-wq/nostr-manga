//! Signed local group-claim review requests, actual native moderation and no source authority.
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
    let response = request.body(body).send().await.unwrap();
    let status = response.status().as_u16();
    assert!(
        response.headers()["cache-control"]
            .to_str()
            .unwrap()
            .contains("no-store")
    );
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
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires scripts/run-auth-tests.sh and its owned isolated PostgreSQL"]
async fn signed_group_claim_is_review_only_versioned_and_preserves_original_authority() {
    use sha2::{Digest,Sha256};
    use weebdex_platform::public_api::{PostgresPublicIdResolver,PublicIdResolver,PublicUuid,ResourceKind};
    let dsn=std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use isolated runner");
    let pg:tokio_postgres::Config=dsn.parse().unwrap();assert_eq!(pg.get_dbname(),Some("weebdex_platform_test"));
    assert!(!pg.get_hosts().is_empty());assert!(pg.get_hosts().iter().all(|h|matches!(h,tokio_postgres::config::Host::Tcp(h)if h=="127.0.0.1")));
    let repository=Repository::connect(DbConfig{dsn:dsn.clone(),connections:2,acquisition_timeout:Duration::from_secs(2),cache_weight:1048576,plaintext_local:true}).await.unwrap();
    repository.migrate().await.unwrap();repository.bind_instance(id(11),id(1)).await.unwrap();
    for key in [2,3,4,5] {repository.set_account_admission(id(key),true).await.unwrap();}
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let origin=format!("http://{}",listener.local_addr().unwrap());
    let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:6,
        protocol_publication:Some(OperatorRuntimeConfig{operator_key_file:"fixture-unused/operator.key".into(),expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),storage:json!({"path":"fixture-unused/storage","volume":"editorialfixture","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144}),group_keys_dir:None,backend:None,native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":"fixture-unused/service.key","admin_pubkey":public(1)}))}),..Config::default()},dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();state.protocol.verify_schema().await.unwrap();
    let db=state.db.client().await.unwrap();
    for key in [2,4,5] {
        let user=format!("ClaimUser{key}");let roles=if key==5 {json!(["moderator"])}else{json!(["user"])};
        let permissions=if key==4 {json!([])}else{json!(["report.create","report.update","scanlation_group.create","scanlation_group.update","self.scanlation_group.update"])};
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,roles,permissions) VALUES($1,$1,$2,'synthetic-no-password',TRUE,$3,$4)",&[&user,&format!("claim-{key}@fixture.invalid"),&roles,&permissions]).await.unwrap();
        db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await.unwrap();
    }
    db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES('owned-claim-reviewer','ClaimUser5',$1,clock_timestamp()+interval '1 hour')",&[&Sha256::digest(b"synthetic-claim-reviewer").to_vec()]).await.unwrap();
    for group in ["ClaimGrp01","ClaimGrp02","ClaimGrp03","ClaimGrp04"] {
        db.execute("INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES($1,'ClaimUser5','Fixture',$1)",&[&group]).await.unwrap();
    }
    let resolver=PostgresPublicIdResolver::new(state.db.clone());
    let group=resolver.preferred(ResourceKind::Group,"ClaimGrp01").await.unwrap().unwrap();
    let path=format!("/api/v1/group/{group}/claim");
    let app=weebdex_platform::router::router(state.clone());let _server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap()}));
    let client=reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(10)).build().unwrap();
    let claimant=resolver.preferred(ResourceKind::User,"ClaimUser2").await.unwrap().unwrap();
    let captured=json!({"version":1,"details":"Captured claim intent","expectedAccountId":claimant.as_str()});let bytes=serde_json::to_vec(&captured).unwrap();let target=format!("{origin}{path}");let token=proof(2,&target,"POST",&bytes);
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='ClaimUser5' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    let response=client.post(&target).header("content-type","application/json").header("authorization",token).body(bytes).send().await.unwrap();assert_eq!(response.status().as_u16(),409);
    let claims:i64=db.query_one("SELECT count(*) FROM platform.reports WHERE entity_id='ClaimGrp01'",&[]).await.unwrap().get(0);assert_eq!(claims,0);
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='ClaimUser2' WHERE publisher=$1",&[&public(2)]).await.unwrap();
    let mut payload=json!({"version":1,"details":"Method: Verification Code\nURL: https://fixture.invalid/owned-proof\nNotes: Synthetic local claim"});
    call(&client,&origin,"POST",&path,Some(payload.clone()),None,401).await;
    for key in [3,4] {call(&client,&origin,"POST",&path,Some(payload.clone()),Some(key),403).await;}
    for bad in [json!({"version":1,"details":""}),json!({"version":1,"details":"Proof","publisher":public(2)}),json!({"version":0,"details":"Proof"})] {call(&client,&origin,"POST",&path,Some(bad),Some(2),400).await;}
    call(&client,&origin,"POST",&path,Some(json!({"version":2,"details":"Stale"})),Some(2),409).await;
    call(&client,&origin,"POST",&format!("{path}?unexpected=1"),Some(payload.clone()),Some(2),400).await;
    payload["expectedAccountId"]=json!(claimant.as_str());
    let pending=call(&client,&origin,"POST",&path,Some(payload.clone()),Some(2),202).await;
    assert_eq!(pending["status"],"pending-moderation");assert_eq!(pending["scope"],"instance-local");assert_eq!(pending["originalSigningAuthorityChanged"],false);
    let again=call(&client,&origin,"POST",&path,Some(payload),Some(2),202).await;assert_eq!(again,pending);
    call(&client,&origin,"POST",&path,Some(json!({"version":1,"details":"Changed evidence must not be silently discarded"})),Some(2),409).await;
    let report=resolver.resolve(ResourceKind::Report,&PublicUuid::parse(pending["requestId"].as_str().unwrap()).unwrap()).await.unwrap().unwrap().native_id;
    let row=db.query_one("SELECT (SELECT count(*) FROM platform.catalog_group_members WHERE group_id='ClaimGrp01' AND is_leader),(SELECT count(*) FROM platform.reports WHERE entity_id='ClaimGrp01'),(SELECT count(*) FROM events)",&[]).await.unwrap();assert_eq!(row.get::<_,i64>(0),0);assert_eq!(row.get::<_,i64>(1),1);assert_eq!(row.get::<_,i64>(2),0);
    // Only a deliberate native moderator review grants local leadership.
    let response=client.post(format!("{origin}/api/report/{report}/accept")).header("origin",&origin).header("cookie","weebdex_session=synthetic-claim-reviewer").json(&json!({"version":1,"notes":"Explicit owned proof review"})).send().await.unwrap();let status=response.status().as_u16();let reviewed:Value=response.json().await.unwrap();assert_eq!(status,200,"{reviewed}");
    let owner=resolver.preferred(ResourceKind::User,"ClaimUser2").await.unwrap().unwrap();
    let updated=call(&client,&origin,"PUT",&format!("/api/v1/group/{group}"),Some(json!({"version":2,"description":"Reviewed local leader annotation"})),Some(2),200).await;assert_eq!(updated["data"]["attributes"]["version"],3);assert!(updated["data"]["relationships"].as_array().unwrap().iter().any(|r|r["type"]=="leader"&&r["id"]==owner.as_str()));
    call(&client,&origin,"POST",&path,Some(json!({"version":3,"details":"Cannot claim an already-led group"})),Some(2),409).await;
    // Changed groups and revoked claimant eligibility cannot acquire authority.
    for (native,change) in [("ClaimGrp02","stale"),("ClaimGrp03","revoked")] {
        let group=resolver.preferred(ResourceKind::Group,native).await.unwrap().unwrap();let path=format!("/api/v1/group/{group}/claim");
        let pending=call(&client,&origin,"POST",&path,Some(json!({"version":1,"details":"Synthetic proof"})),Some(2),202).await;
        let report=resolver.resolve(ResourceKind::Report,&PublicUuid::parse(pending["requestId"].as_str().unwrap()).unwrap()).await.unwrap().unwrap().native_id;
        if change=="stale" {db.execute("UPDATE platform.catalog_groups SET version=version+1 WHERE id=$1",&[&native]).await.unwrap();}else{db.execute("UPDATE platform.users SET verified=FALSE WHERE id='ClaimUser2'",&[]).await.unwrap();}
        let response=client.post(format!("{origin}/api/report/{report}/accept")).header("origin",&origin).header("cookie","weebdex_session=synthetic-claim-reviewer").json(&json!({"version":1})).send().await.unwrap();assert_eq!(response.status().as_u16(),409);
        let leaders:i64=db.query_one("SELECT count(*) FROM platform.catalog_group_members WHERE group_id=$1 AND is_leader",&[&native]).await.unwrap().get(0);assert_eq!(leaders,0);
        db.execute("UPDATE platform.users SET verified=TRUE WHERE id='ClaimUser2'",&[]).await.unwrap();
    }
    let group=resolver.preferred(ResourceKind::Group,"ClaimGrp04").await.unwrap().unwrap();let path=format!("/api/v1/group/{group}/claim");
    repository.set_account_admission(id(2),false).await.unwrap();call(&client,&origin,"POST",&path,Some(json!({"version":1,"details":"Revoked"})),Some(2),403).await;repository.set_account_admission(id(2),true).await.unwrap();
    let row=db.query_one("SELECT roles,(SELECT count(*) FROM platform.protocol_identity_bindings WHERE group_id IS NOT NULL),(SELECT count(*) FROM events),(SELECT count(*) FROM platform.protocol_publication_commands) FROM platform.users WHERE id='ClaimUser2'",&[]).await.unwrap();assert_eq!(row.get::<_,Value>(0),json!(["user"]));for n in 1..4 {assert_eq!(row.get::<_,i64>(n),0);}
}
