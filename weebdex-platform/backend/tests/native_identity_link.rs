//! Real HTTP + disposable PostgreSQL. Synthetic account cookies and fixed test
//! signer only. No real keys, login accounts, publication or authority grants.
use manga_node::{
    auth,
    db::{DbConfig, Repository},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::Duration;
use weebdex_platform::{
    AppState, Config, domains,
    public_api::{PostgresPublicIdResolver, PublicIdResolver, ResourceKind},
};
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}
struct Fixture {
    state: AppState,
    http: reqwest::Client,
    origin: String,
    accounts: Vec<String>,
    _server: Server,
}
impl Fixture {
    async fn start() -> Self {
        let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").unwrap();
        let pg: tokio_postgres::Config = dsn.parse().unwrap();
        assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
        assert!(
            pg.get_hosts()
                .iter()
                .all(|h| matches!(h,tokio_postgres::config::Host::Tcp(h) if h=="127.0.0.1"))
        );
        let runtime = std::path::PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap());
        assert!(
            runtime
                .to_str()
                .unwrap()
                .contains("/weebdex-platform/test-runtime/")
        );
        let repo = Repository::connect(DbConfig {
            dsn: dsn.clone(),
            connections: 2,
            acquisition_timeout: Duration::from_secs(2),
            cache_weight: 1048576,
            plaintext_local: true,
        })
        .await
        .unwrap();
        repo.migrate().await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let state = AppState::connect(
            Config {
                public_origin: origin.clone(),
                database_plaintext_local: true,
                database_connections: 8,
                uploads_dir: runtime.join("media"),
                ui_dir: runtime.join("no-ui"),
                ..Config::default()
            },
            dsn,
        )
        .await
        .unwrap();
        state.db.migrate(domains::MIGRATIONS).await.unwrap();
        state.protocol.verify_schema().await.unwrap();
        let db = state.db.client().await.unwrap();
        for name in ["LinkUserA", "LinkUserB"] {
            db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,roles,permissions) VALUES($1,$1,$2,'synthetic-no-password',true,'[\"user\"]','[\"self.user.update\"]')",&[&name,&format!("{name}@example.invalid")]).await.unwrap();
            for suffix in ["", "-other-session"] {
                let id = format!("{name}{suffix}");
                let hash = Sha256::digest(format!("synthetic-cookie-{id}").as_bytes()).to_vec();
                db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at,purpose) VALUES($1,$2,$3,clock_timestamp()+interval '1 hour','normal')",&[&id,&name,&hash]).await.unwrap();
            }
        }
        drop(db);
        let resolver = PostgresPublicIdResolver::new(state.db.clone());
        let mut accounts = vec![];
        for name in ["LinkUserA", "LinkUserB"] {
            accounts.push(
                resolver
                    .preferred(ResourceKind::User, name)
                    .await
                    .unwrap()
                    .unwrap()
                    .to_string(),
            );
        }
        let app = weebdex_platform::router::router(state.clone());
        let server = Server(tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        }));
        Self {
            state,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            origin,
            accounts,
            _server: server,
        }
    }
    fn request(&self, path: &str, session: &str, body: Option<Value>) -> reqwest::RequestBuilder {
        let mut r = self
            .http
            .request(
                if body.is_some() {
                    reqwest::Method::POST
                } else {
                    reqwest::Method::GET
                },
                format!("{}/api/v1/account/identity-link{path}", self.origin),
            )
            .header("origin", &self.origin)
            .header(
                "cookie",
                format!(
                    "{}=synthetic-cookie-{session}",
                    self.state.config.session_cookie_name
                ),
            )
            .header(
                "x-weebdex-expected-account",
                session.split("-other-session").next().unwrap(),
            );
        if let Some(body) = body {
            r = r.json(&body);
        }
        r
    }
    fn closure(&self, path: &str, body: Option<Value>) -> reqwest::RequestBuilder {
        let mut request = self
            .http
            .request(
                if body.is_some() {
                    reqwest::Method::POST
                } else {
                    reqwest::Method::GET
                },
                format!("{}/api/account/closure{}", self.origin, path),
            )
            .header("origin", &self.origin)
            .header(
                "cookie",
                format!(
                    "{}=synthetic-cookie-LinkUserA",
                    self.state.config.session_cookie_name
                ),
            );
        if let Some(body) = body {
            request = request.json(&body);
        }
        request
    }
    async fn response(&self, r: reqwest::RequestBuilder, status: u16) -> Value {
        let r = r.send().await.unwrap();
        let code = r.status().as_u16();
        assert!(
            r.headers()
                .get("cache-control")
                .unwrap()
                .to_str()
                .unwrap()
                .contains("no-store")
        );
        let value: Value = r.json().await.unwrap();
        assert_eq!(code, status, "{value}");
        value
    }
    async fn prepare(&self, index: usize, key: &str, revision: i64) -> Value {
        self.response(self.request("/prepare",if index==0{"LinkUserA"}else{"LinkUserB"},Some(json!({"expectedAccountId":self.accounts[index],"expectedRevision":revision,"action":"link","publicKey":key}))),200).await
    }
    fn confirmation(&self, index: usize, challenge: &Value, secret: u64) -> Value {
        let proof = auth::sign(
            &format!("{secret:064x}"),
            27235,
            vec![
                vec!["u".into(), challenge["proofUrl"].as_str().unwrap().into()],
                vec!["method".into(), "POST".into()],
                vec![
                    "payload".into(),
                    challenge["proofPayloadHash"].as_str().unwrap().into(),
                ],
            ],
            String::new(),
        )
        .unwrap();
        json!({"expectedAccountId":self.accounts[index],"challengeId":challenge["challengeId"],"confirmation":"connect-private-identity","proof":proof})
    }
}
fn key(secret: u64) -> String {
    auth::sign(&format!("{secret:064x}"), 27235, vec![], String::new())
        .unwrap()
        .pubkey
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires run-auth-tests.sh disposable PostgreSQL"]
async fn private_identity_link_http_dual_proof_conflict_recovery_and_races() {
    let f = Fixture::start().await;
    let key = key(73);
    let initial = f.response(f.request("", "LinkUserA", None), 200).await;
    assert_eq!(initial["publicKey"], Value::Null);
    assert_eq!(initial["revision"], 0);
    let baseline=f.state.db.client().await.unwrap().query_one("SELECT (SELECT count(*) FROM platform.protocol_identity_bindings),(SELECT count(*) FROM account_permissions),(SELECT count(*) FROM events)",&[]).await.unwrap();
    let baseline: (i64, i64, i64) = (baseline.get(0), baseline.get(1), baseline.get(2));
    let a = f.prepare(0, &key, 0).await;
    let confirm = f.confirmation(0, &a, 73);
    f.response(
        f.request("/confirm", "LinkUserA-other-session", Some(confirm.clone())),
        409,
    )
    .await;
    f.response(
        f.request("/confirm", "LinkUserB", Some(confirm.clone())),
        409,
    )
    .await;
    f.response(
        f.request("/confirm", "LinkUserA", Some(confirm.clone()))
            .header("authorization", "Nostr ignored"),
        403,
    )
    .await;
    f.response(
        f.request("/confirm", "LinkUserA", Some(confirm.clone()))
            .header("origin", "https://elsewhere.invalid"),
        403,
    )
    .await;
    let wrong = f.confirmation(0, &a, 74);
    f.response(f.request("/confirm", "LinkUserA", Some(wrong)), 401)
        .await;
    for (kind, url, method, payload) in [
        (
            0,
            a["proofUrl"].as_str().unwrap(),
            "POST",
            a["proofPayloadHash"].as_str().unwrap(),
        ),
        (
            27235,
            "https://elsewhere.invalid/api/v1/account/identity-link/confirm",
            "POST",
            a["proofPayloadHash"].as_str().unwrap(),
        ),
        (
            27235,
            a["proofUrl"].as_str().unwrap(),
            "GET",
            a["proofPayloadHash"].as_str().unwrap(),
        ),
        (
            27235,
            a["proofUrl"].as_str().unwrap(),
            "POST",
            "0000000000000000000000000000000000000000000000000000000000000000",
        ),
    ] {
        let proof = auth::sign(
            &format!("{:064x}", 73),
            kind,
            vec![
                vec!["u".into(), url.into()],
                vec!["method".into(), method.into()],
                vec!["payload".into(), payload.into()],
            ],
            String::new(),
        )
        .unwrap();
        let mut invalid = confirm.clone();
        invalid["proof"] = json!(proof);
        f.response(f.request("/confirm", "LinkUserA", Some(invalid)), 401)
            .await;
    }
    let mut wrong_payload = confirm.clone();
    wrong_payload["proof"]["tags"][2][1] = json!("0".repeat(64));
    f.response(f.request("/confirm", "LinkUserA", Some(wrong_payload)), 401)
        .await;
    // Explicit new review cancels stale challenge even if old confirmation arrives late.
    let replaced = f.prepare(0, &key, 0).await;
    f.response(f.request("/confirm", "LinkUserA", Some(confirm)), 409)
        .await;
    let confirmed = f
        .response(
            f.request(
                "/confirm",
                "LinkUserA",
                Some(f.confirmation(0, &replaced, 73)),
            ),
            200,
        )
        .await;
    assert_eq!(confirmed["publicKey"], key);
    assert_eq!(confirmed["revision"], 1);
    assert_eq!(confirmed["grantsAuthority"], false);
    f.response(
        f.request(
            "/confirm",
            "LinkUserA",
            Some(f.confirmation(0, &replaced, 73)),
        ),
        409,
    )
    .await;
    let b = f.prepare(1, &key, 0).await;
    let conflict = f
        .response(
            f.request("/confirm", "LinkUserB", Some(f.confirmation(1, &b, 73))),
            409,
        )
        .await;
    assert!(!conflict.to_string().contains("LinkUserA"));
    assert!(!conflict.to_string().contains(&f.accounts[0]));
    // Native owner can revoke without the lost portable key, but only after fresh login.
    let db = f.state.db.client().await.unwrap();
    db.execute("UPDATE platform.sessions SET authenticated_at=clock_timestamp()-interval '11 minutes' WHERE id='LinkUserA'",&[]).await.unwrap();
    let unlink = json!({"expectedAccountId":f.accounts[0],"expectedRevision":1,"action":"unlink"});
    f.response(
        f.request("/prepare", "LinkUserA", Some(unlink.clone())),
        403,
    )
    .await;
    db.execute("UPDATE platform.sessions SET authenticated_at=clock_timestamp()+interval '1 minute' WHERE id='LinkUserA'",&[]).await.unwrap();
    f.response(
        f.request("/prepare", "LinkUserA", Some(unlink.clone())),
        403,
    )
    .await;
    db.execute(
        "UPDATE platform.sessions SET authenticated_at=clock_timestamp() WHERE id='LinkUserA'",
        &[],
    )
    .await
    .unwrap();
    let unlink_review = f
        .response(f.request("/prepare", "LinkUserA", Some(unlink)), 200)
        .await;
    let unlink_confirm = json!({"expectedAccountId":f.accounts[0],"challengeId":unlink_review["challengeId"],"confirmation":"disconnect-private-identity"});
    let out = f
        .response(
            f.request("/confirm", "LinkUserA", Some(unlink_confirm)),
            200,
        )
        .await;
    assert_eq!(out["publicKey"], Value::Null);
    assert_eq!(out["revision"], 2);
    // Expired review is refused without mutation.
    let expired = f.prepare(0, &key, 2).await;
    db.execute("UPDATE platform.native_identity_link_challenges SET expires_at=clock_timestamp()-interval '1 second' WHERE id=$1",&[&expired["challengeId"].as_str().unwrap()]).await.unwrap();
    f.response(
        f.request(
            "/confirm",
            "LinkUserA",
            Some(f.confirmation(0, &expired, 73)),
        ),
        409,
    )
    .await;
    // Delay a genuine confirm at the link lock. A concurrent prepare cannot clear
    // uncertainty by claiming to replace a still-running confirmation.
    let delayed = f.prepare(0, &key, 2).await;
    let mut blocker = f.state.db.client().await.unwrap();
    let hold = blocker.transaction().await.unwrap();
    hold.query_one(
        "SELECT user_id FROM platform.native_identity_links WHERE user_id='LinkUserA' FOR UPDATE",
        &[],
    )
    .await
    .unwrap();
    let pending = tokio::spawn(
        f.request(
            "/confirm",
            "LinkUserA",
            Some(f.confirmation(0, &delayed, 73)),
        )
        .send(),
    );
    let mut waiting = false;
    for _ in 0..100 {
        waiting=db.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE wait_event_type='Lock' AND query LIKE 'SELECT public_key,revision FROM platform.native_identity_links%FOR UPDATE')",&[]).await.unwrap().get(0);
        if waiting {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(waiting, "confirm reached the owned link-row barrier");
    f.response(f.request("/prepare","LinkUserA",Some(json!({"expectedAccountId":f.accounts[0],"expectedRevision":2,"action":"link","publicKey":key}))),409).await;
    hold.commit().await.unwrap();
    let response = pending.await.unwrap().unwrap();
    assert_eq!(response.status(), 200);
    let linked: Value = response.json().await.unwrap();
    assert_eq!(linked["revision"], 3);
    f.response(f.request("/prepare","LinkUserA",Some(json!({"expectedAccountId":f.accounts[0],"expectedRevision":2,"action":"link","publicKey":key}))),409).await;
    let reset=f.response(f.request("/prepare","LinkUserA",Some(json!({"expectedAccountId":f.accounts[0],"expectedRevision":3,"action":"unlink"}))),200).await;
    f.response(f.request("/confirm","LinkUserA",Some(json!({"expectedAccountId":f.accounts[0],"challengeId":reset["challengeId"],"confirmation":"disconnect-private-identity"}))),200).await;
    // Crossing native session expiry while waiting on a row lock must refuse
    // cleanly, not turn a previously fresh extractor into durable authority.
    let crossing = f.prepare(0, &key, 4).await;
    db.execute("UPDATE platform.sessions SET expires_at=clock_timestamp()+interval '400 milliseconds' WHERE id='LinkUserA'",&[]).await.unwrap();
    let hold = blocker.transaction().await.unwrap();
    hold.query_one(
        "SELECT user_id FROM platform.native_identity_links WHERE user_id='LinkUserA' FOR UPDATE",
        &[],
    )
    .await
    .unwrap();
    let pending = tokio::spawn(
        f.request(
            "/confirm",
            "LinkUserA",
            Some(f.confirmation(0, &crossing, 73)),
        )
        .send(),
    );
    let mut waiting = false;
    for _ in 0..100 {
        waiting=db.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE wait_event_type='Lock' AND query LIKE 'SELECT public_key,revision FROM platform.native_identity_links%FOR UPDATE')",&[]).await.unwrap().get(0);
        if waiting {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(waiting);
    tokio::time::sleep(Duration::from_millis(450)).await;
    hold.commit().await.unwrap();
    let response = pending.await.unwrap().unwrap();
    assert_eq!(response.status(), 403);
    let refused: Value = response.json().await.unwrap();
    assert!(refused.to_string().contains("Sign in again"));
    assert_eq!(
        db.query_one(
            "SELECT revision FROM platform.native_identity_links WHERE user_id='LinkUserA'",
            &[]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        4
    );
    db.execute("UPDATE platform.sessions SET expires_at=clock_timestamp()+interval '1 hour' WHERE id='LinkUserA'",&[]).await.unwrap();
    // The cross-account unique-key index is a second wait boundary AFTER the
    // pre-update checks. Both session and challenge expiry must roll back the
    // successful update once that index wait is released.
    for expire_session in [true, false] {
        let crossing = f.prepare(0, &key, 4).await;
        if expire_session {
            db.execute("UPDATE platform.sessions SET expires_at=clock_timestamp()+interval '400 milliseconds' WHERE id='LinkUserA'",&[]).await.unwrap();
        } else {
            db.execute("UPDATE platform.native_identity_link_challenges SET expires_at=clock_timestamp()+interval '400 milliseconds' WHERE id=$1",&[&crossing["challengeId"].as_str().unwrap()]).await.unwrap();
        }
        let hold = blocker.transaction().await.unwrap();
        hold.execute(
            "UPDATE platform.native_identity_links SET public_key=$1 WHERE user_id='LinkUserB'",
            &[&key],
        )
        .await
        .unwrap();
        let pending = tokio::spawn(
            f.request(
                "/confirm",
                "LinkUserA",
                Some(f.confirmation(0, &crossing, 73)),
            )
            .send(),
        );
        let mut waiting = false;
        for _ in 0..100 {
            waiting=db.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE wait_event_type='Lock' AND query LIKE 'UPDATE platform.native_identity_links SET public_key=$2%')",&[]).await.unwrap().get(0);
            if waiting {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(waiting, "confirm reached the owned unique-index barrier");
        tokio::time::sleep(Duration::from_millis(450)).await;
        hold.rollback().await.unwrap();
        let response = pending.await.unwrap().unwrap();
        assert_eq!(
            response.status().as_u16(),
            if expire_session { 403 } else { 409 }
        );
        let refused: Value = response.json().await.unwrap();
        assert!(refused.to_string().contains(if expire_session {
            "Sign in again"
        } else {
            "expired while waiting"
        }));
        let row=db.query_one("SELECT revision,public_key FROM platform.native_identity_links WHERE user_id='LinkUserA'",&[]).await.unwrap();
        assert_eq!(row.get::<_, i64>(0), 4);
        assert_eq!(row.get::<_, Option<String>>(1), None);
        let unused: bool = db
            .query_one(
                "SELECT used_at IS NULL FROM platform.native_identity_link_challenges WHERE id=$1",
                &[&crossing["challengeId"].as_str().unwrap()],
            )
            .await
            .unwrap()
            .get(0);
        assert!(
            unused,
            "refusal rolled back both link and challenge consumption"
        );
        db.execute("UPDATE platform.sessions SET expires_at=clock_timestamp()+interval '1 hour' WHERE id='LinkUserA'",&[]).await.unwrap();
    }
    // Prepare storage is bounded even after many cancelled/replaced reviews.
    for _ in 0..105 {
        f.prepare(0, &key, 4).await;
    }
    let retained:i64=db.query_one("SELECT count(*) FROM platform.native_identity_link_challenges WHERE user_id='LinkUserA'",&[]).await.unwrap().get(0);
    assert_eq!(retained, 100);
    // Race two accounts for the same key: at most one mapping is committed.
    let ra = f.prepare(0, &key, 4).await;
    let rb = f.prepare(1, &key, 0).await;
    let (ar, br) = tokio::join!(
        f.request("/confirm", "LinkUserA", Some(f.confirmation(0, &ra, 73)))
            .send(),
        f.request("/confirm", "LinkUserB", Some(f.confirmation(1, &rb, 73)))
            .send()
    );
    let codes = vec![ar.unwrap().status().as_u16(), br.unwrap().status().as_u16()];
    assert_eq!(codes.iter().filter(|&&code| code == 200).count(), 1);
    assert_eq!(codes.iter().filter(|&&code| code == 409).count(), 1);
    let count: i64 = db
        .query_one(
            "SELECT count(*) FROM platform.native_identity_links WHERE public_key=$1",
            &[&key],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 1);
    let after=db.query_one("SELECT (SELECT count(*) FROM platform.protocol_identity_bindings),(SELECT count(*) FROM account_permissions),(SELECT count(*) FROM events)",&[]).await.unwrap();
    assert_eq!(
        (
            after.get::<_, i64>(0),
            after.get::<_, i64>(1),
            after.get::<_, i64>(2)
        ),
        baseline
    );
    let grants: Value = db
        .query_one(
            "SELECT permissions FROM platform.users WHERE id='LinkUserA'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(grants, json!(["self.user.update"]));
    // Fresh requests after revocation/restriction cannot use pending reviews.
    db.execute(
        "UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE id='LinkUserA'",
        &[],
    )
    .await
    .unwrap();
    f.response(
        f.request("/confirm", "LinkUserA", Some(f.confirmation(0, &ra, 73))),
        401,
    )
    .await;
    db.execute(
        "UPDATE platform.users SET roles='[\"restricted\"]' WHERE id='LinkUserB'",
        &[],
    )
    .await
    .unwrap();
    f.response(
        f.request("/confirm", "LinkUserB", Some(f.confirmation(1, &rb, 73))),
        403,
    )
    .await;

    // Owned fixture recovery restores only the synthetic cookie for a separate
    // native-close race. All association/closure operations below use real HTTP.
    db.execute("UPDATE platform.sessions SET revoked_at=NULL,authenticated_at=clock_timestamp(),expires_at=clock_timestamp()+interval '1 hour' WHERE id='LinkUserA'",&[]).await.unwrap();
    let mut current = f.response(f.request("", "LinkUserA", None), 200).await;
    if !current["publicKey"].is_null() {
        let reset=f.response(f.request("/prepare","LinkUserA",Some(json!({"expectedAccountId":f.accounts[0],"expectedRevision":current["revision"],"action":"unlink"}))),200).await;
        current=f.response(f.request("/confirm","LinkUserA",Some(json!({"expectedAccountId":f.accounts[0],"challengeId":reset["challengeId"],"confirmation":"disconnect-private-identity"}))),200).await;
    }
    let closure_key = self::key(75);
    let portable = f
        .prepare(0, &closure_key, current["revision"].as_i64().unwrap())
        .await;
    let preview = f.response(f.closure("/LinkUserA", None), 200).await;
    let capture = json!({"expectedAccountId":"LinkUserA","expectedUserId":"LinkUserA","expectedLifecycleId":preview["lifecycleId"],"version":preview["version"]});
    let close_review = f
        .response(f.closure("/prepare", Some(capture.clone())), 200)
        .await;
    let close_command = json!({"capture":capture,"confirmationId":close_review["confirmationId"],"confirmationToken":close_review["confirmationToken"],"acknowledge":"DISABLE THIS ACCOUNT"});
    let hold = blocker.transaction().await.unwrap();
    hold.query_one(
        "SELECT user_id FROM platform.native_identity_links WHERE user_id='LinkUserA' FOR UPDATE",
        &[],
    )
    .await
    .unwrap();
    let pending = tokio::spawn(
        f.request(
            "/confirm",
            "LinkUserA",
            Some(f.confirmation(0, &portable, 75)),
        )
        .send(),
    );
    let mut waiting = false;
    for _ in 0..100 {
        waiting=db.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE wait_event_type='Lock' AND query LIKE 'SELECT public_key,revision FROM platform.native_identity_links%FOR UPDATE')",&[]).await.unwrap().get(0);
        if waiting {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(
        waiting,
        "portable confirmation holds native authority before closure"
    );
    f.response(f.closure("/confirm", Some(close_command.clone())), 409)
        .await;
    hold.commit().await.unwrap();
    let response = pending.await.unwrap().unwrap();
    assert_eq!(response.status(), 200);
    let connected: Value = response.json().await.unwrap();
    let unfinished=f.response(f.request("/prepare","LinkUserA",Some(json!({"expectedAccountId":f.accounts[0],"expectedRevision":connected["revision"],"action":"unlink"}))),200).await;
    f.response(f.closure("/confirm", Some(close_command)), 200)
        .await;
    let row=db.query_one("SELECT public_key,revision FROM platform.native_identity_links WHERE user_id='LinkUserA'",&[]).await.unwrap();
    assert_eq!(row.get::<_, Option<String>>(0), None);
    assert_eq!(
        row.get::<_, i64>(1),
        connected["revision"].as_i64().unwrap() + 1
    );
    let row=db.query_one("SELECT used_at IS NOT NULL,outcome FROM platform.native_identity_link_challenges WHERE id=$1",&[&unfinished["challengeId"].as_str().unwrap()]).await.unwrap();
    assert!(row.get::<_, bool>(0));
    assert_eq!(row.get::<_, String>(1), "account-closed");
    f.response(f.request("/confirm","LinkUserA",Some(json!({"expectedAccountId":f.accounts[0],"challengeId":unfinished["challengeId"],"confirmation":"disconnect-private-identity"}))),401).await;
}
