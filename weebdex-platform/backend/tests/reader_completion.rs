//! Source-authored, ignored acceptance controls for native bounded completion.
//! These are real loopback HTTP / PostgreSQL fixtures, not mock routing proofs.
//! No scenario in this file has been executed merely by authoring this source.
//! Run only in a separately approved, disposable PostgreSQL fixture runner.
use reqwest::{Method, RequestBuilder, header::{self, HeaderMap}};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, pin::Pin, sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}}, task::{Context, Poll}, time::{Duration, Instant}};
use tower::ServiceExt;
use weebdex_platform::{AppState, Config, domains};

const EXPECTED_ACCOUNT: &str = "x-weebdex-expected-account";
const RESPONSE_LIMIT: usize = 16 * 1024;
const QUERY_LIMIT: usize = 24 * 1024;
const EPOCH: u64 = 7;

struct Fixture {
    state: AppState,
    client: reqwest::Client,
    base: String,
    origin: String,
    user: String,
    session: String,
    cookie: String,
    namespace: String,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) { self.server.abort(); }
}
struct Wire {
    status: u16,
    headers: HeaderMap,
    body: Vec<u8>,
}
impl Wire {
    fn value(&self) -> Value {
        serde_json::from_slice(&self.body).expect("Complete response must be valid JSON, not truncated")
    }
    fn private(&self) {
        let cache = self.headers.get(header::CACHE_CONTROL).and_then(|h| h.to_str().ok()).unwrap();
        let directives: Vec<&str> = cache.split(',').map(str::trim).collect();
        assert!(directives.contains(&"no-store"));
        assert!(directives.contains(&"private"));
        assert!(self.body.len() <= RESPONSE_LIMIT, "Count the entire serialized response, including envelope and errors");
    }
    fn refused(&self, status: u16) {
        self.private();
        assert_eq!(self.status, status, "{}", String::from_utf8_lossy(&self.body));
        let value = self.value();
        assert!(value.get("data").is_none_or(|rows| rows.is_null() || rows.as_array().is_some_and(Vec::is_empty)),
            "Refusal must not disclose a successful prefix: {value}");
        assert!(!String::from_utf8_lossy(&self.body).contains("\"completed\""), "No partial completion flags on refusal");
    }
}
impl Fixture {
    async fn new(name: &str) -> Self {
        let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL")
            .expect("Use only the explicitly isolated PostgreSQL fixture runner");
        let pg: tokio_postgres::Config = dsn.parse().unwrap();
        assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
        assert_eq!(pg.get_hosts().len(), 1);
        assert!(pg.get_hosts().iter().all(|host|
            matches!(host, tokio_postgres::config::Host::Tcp(host) if host == "127.0.0.1")));
        assert_eq!(pg.get_ports().len(), 1);
        assert!(pg.get_ports().iter().all(|port| *port > 1024 && *port != 5432));
        let runtime = std::path::PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR")
            .expect("Owned runtime receipt from scripts/run-auth-tests.sh is required"));
        assert!(runtime.to_string_lossy().contains("/weebdex-platform/test-runtime/auth-"));
        // Consume the actual auth_fixture_guard.py receipt, as library_http
        // does, before connecting, migrating, seeding or binding a test server.
        // Process/listener ownership still belongs to that existing launcher;
        // this check is not an alternative fixture owner or authorization.
        let receipt: Value = serde_json::from_slice(
            &std::fs::read(runtime.join("fixture.json")).expect("Owned disposable fixture receipt"),
        ).unwrap();
        assert_eq!(receipt["database"], "weebdex_platform_test");
        assert_eq!(receipt["fixture_only"], true);
        assert_eq!(receipt["transport"], "private loopback TCP");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        // UI origin deliberately differs from the loopback transport origin.
        // This exercises the existing configured public_origin allowance.
        let origin = "https://reader-completion.fixture.invalid".to_owned();
        let state = AppState::connect(Config {
            database_plaintext_local: true,
            database_connections: 4,
            public_origin: origin.clone(),
            uploads_dir: runtime.join(format!("completion-{name}-media")),
            ui_dir: runtime.join("no-ui"),
            ..Config::default()
        }, dsn).await.unwrap();
        state.db.migrate(domains::MIGRATIONS).await.unwrap();
        state.db.verify_schema(domains::MIGRATIONS).await.unwrap();
        let user = format!("RC{name}User");
        let session = format!("RC{name}Session");
        let token = format!("synthetic-completion-session-{name}");
        let hash = Sha256::digest(token.as_bytes()).to_vec();
        let db = state.db.client().await.unwrap();
        db.execute(
            "INSERT INTO platform.users(id,username,email,password_hash,verified) VALUES($1,$2,$3,'synthetic-no-password',true)",
            &[&user, &format!("completion-{}", name.to_lowercase()), &format!("completion-{name}@fixture.invalid")],
        ).await.unwrap();
        db.execute(
            "INSERT INTO platform.sessions(id,user_id,token_hash,purpose,expires_at) VALUES($1,$2,$3,'normal',clock_timestamp()+interval '1 hour')",
            &[&session, &user, &hash],
        ).await.unwrap();
        let namespace: String = db.query_one(
            "SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton", &[],
        ).await.unwrap().get(0);
        drop(db);
        let cookie = format!("{}={token}", state.config.session_cookie_name);
        let app = weebdex_platform::router::router(state.clone());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });
        let client = reqwest::Client::builder().no_proxy()
            .timeout(Duration::from_secs(15)).build().unwrap();
        Self { state, client, base, origin, user, session, cookie, namespace, server }
    }
    async fn sql(&self, statement: &str) {
        self.state.db.client().await.unwrap().batch_execute(statement).await.unwrap();
    }
    async fn manga(&self, id: &str) {
        self.state.db.client().await.unwrap().execute(
            "INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES($1,$2,'Completion fixture',$1,'ja','ongoing','safe','published')",
            &[&id, &self.user],
        ).await.unwrap();
    }
    async fn chapter(&self, id: &str, manga: &str) {
        self.state.db.client().await.unwrap().execute(
            "INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,language,published_at) VALUES($1,$2,$3,'Completion fixture','en','2024-01-01')",
            &[&id, &manga, &self.user],
        ).await.unwrap();
    }
    async fn mark(&self, manga: &str, id: &str) {
        self.state.db.client().await.unwrap().execute(
            "INSERT INTO platform.library_chapter_history(user_id,manga_id,chapter_id) VALUES($1,$2,$3)",
            &[&self.user, &manga, &id],
        ).await.unwrap();
    }
    fn path(&self, manga: &str, ids: &[&str]) -> String {
        let mut pairs = vec![
            ("accountEpoch".to_owned(), EPOCH.to_string()),
            ("catalogNamespace".to_owned(), self.namespace.clone()),
            ("serviceKey".to_owned(), "null".to_owned()),
        ];
        pairs.extend(ids.iter().map(|id| ("id".to_owned(), (*id).to_owned())));
        encode(&format!("/api/user/history/manga/{manga}/completion"), &pairs)
    }
    fn bare(&self, method: Method, path: &str) -> RequestBuilder {
        self.client.request(method, format!("{}{path}", self.base))
    }
    fn request(&self, path: &str) -> RequestBuilder {
        self.bare(Method::GET, path).header(header::COOKIE, &self.cookie).header(EXPECTED_ACCOUNT, &self.user)
    }
    async fn send(&self, request: RequestBuilder) -> Wire {
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let body = response.bytes().await.unwrap().to_vec();
        Wire { status, headers, body }
    }
    async fn get(&self, path: &str) -> Wire { self.send(self.request(path)).await }
    fn success(&self, wire: &Wire, manga: &str, ids: &[&str], completed: &[bool], epoch: u64) -> Value {
        wire.private();
        assert_eq!(wire.status, 200, "{}", String::from_utf8_lossy(&wire.body));
        let value = wire.value();
        exact_keys(&value, &["version", "mangaId", "catalogIdentity", "account", "consistency", "continueProof", "data"]);
        assert_eq!(value["version"], 1);
        assert_eq!(value["mangaId"], manga);
        assert_eq!(value["catalogIdentity"], json!({"catalogNamespace":self.namespace,"serviceKey":null}));
        assert_eq!(value["account"], json!({"id":self.user,"epoch":epoch}));
        assert_eq!(value["consistency"], "live-request-v1");
        assert_eq!(value["continueProof"], "observed-only");
        let rows = value["data"].as_array().unwrap();
        assert_eq!(rows.len(), ids.len(), "No truncation, omission or unsolicited history rows");
        assert_eq!(ids.len(), completed.len());
        assert!((1..=50).contains(&rows.len()));
        for ((row, id), completed) in rows.iter().zip(ids).zip(completed) {
            exact_keys(row, &["id", "completed"]);
            assert_eq!(row["id"], *id, "Preserve exact request order, not lexical/database/history order");
            assert_eq!(row["completed"].as_bool(), Some(*completed));
        }
        value
    }
    async fn own_snapshot(&self) -> Value {
        self.state.db.client().await.unwrap().query_one(
            "SELECT jsonb_build_object('user',(SELECT to_jsonb(u) FROM platform.users u WHERE id=$1),'sessions',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM platform.sessions s WHERE user_id=$1),'history',(SELECT jsonb_agg(to_jsonb(h) ORDER BY chapter_id) FROM platform.library_chapter_history h WHERE user_id=$1),'progress',(SELECT jsonb_agg(to_jsonb(p) ORDER BY chapter_id) FROM platform.library_chapter_progress p WHERE user_id=$1),'follows',(SELECT jsonb_agg(to_jsonb(f) ORDER BY manga_id) FROM platform.library_manga_follows f WHERE user_id=$1))",
            &[&self.user],
        ).await.unwrap().get(0)
    }
}
fn encode(path: &str, pairs: &[(String, String)]) -> String {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    query.extend_pairs(pairs.iter().map(|(key, value)| (key.as_str(), value.as_str())));
    format!("{path}?{}", query.finish())
}
fn replace(path: &str, name: &str, value: Option<&str>) -> String {
    let (route, raw) = path.split_once('?').unwrap();
    let mut pairs: Vec<(String, String)> = url::form_urlencoded::parse(raw.as_bytes())
        .filter(|(key, _)| key != name).map(|(key, value)| (key.into_owned(), value.into_owned())).collect();
    if let Some(value) = value { pairs.push((name.to_owned(), value.to_owned())); }
    encode(route, &pairs)
}
fn exact_keys(value: &Value, expected: &[&str]) {
    let actual = value.as_object().unwrap().keys().map(String::as_str).collect::<BTreeSet<_>>();
    assert_eq!(actual, expected.iter().copied().collect::<BTreeSet<_>>());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn real_cookie_reads_only_requested_membership_amid_more_than_ten_thousand_history_rows() {
    let f = Fixture::new("Bounded").await;
    let manga = "RCBoundedManga";
    f.manga(manga).await;
    f.manga("RCBoundedOther").await;
    f.sql("INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,language) SELECT 'RCBoundedBulk' || n::text,'RCBoundedOther','RCBoundedUser','Completion fixture','en' FROM generate_series(1,10020) n; INSERT INTO platform.library_chapter_history(user_id,manga_id,chapter_id) SELECT 'RCBoundedUser',manga_id,id FROM platform.catalog_chapters WHERE manga_id='RCBoundedOther';").await;
    for id in ["RCBoundedHistoryOnly", "RCBoundedProgressOnly", "RCBoundedFalseProgress", "RCBoundedNeither"] {
        f.chapter(id, manga).await;
    }
    f.mark(manga, "RCBoundedHistoryOnly").await;
    f.mark(manga, "RCBoundedFalseProgress").await;
    f.sql("INSERT INTO platform.library_chapter_progress(user_id,chapter_id,page,completed) VALUES('RCBoundedUser','RCBoundedProgressOnly',3,true),('RCBoundedUser','RCBoundedFalseProgress',0,false)").await;
    let before = f.own_snapshot().await;
    assert!(before["history"].as_array().unwrap().len() > 10_000);
    let requested = ["RCBoundedNeither", "RCBoundedFalseProgress", "RCBoundedHistoryOnly", "RCBoundedProgressOnly"];
    let response = f.get(&f.path(manga, &requested)).await;
    f.success(&response, manga, &requested, &[false, true, true, false], EPOCH);
    assert!(!String::from_utf8_lossy(&response.body).contains("RCBoundedBulk"));
    assert_eq!(f.own_snapshot().await, before, "GET must not mark reads, advance pages, touch sessions or change follows");
    // A later request observes a later membership change; epoch is client context,
    // not a database revision or cross-request snapshot promise.
    f.mark(manga, "RCBoundedProgressOnly").await;
    let changed = f.get(&f.path(manga, &requested)).await;
    f.success(&changed, manga, &requested, &[false, true, true, true], EPOCH);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn cookie_authority_rejects_api_recovery_revoked_expired_disabled_and_silent_account_switches() {
    let f = Fixture::new("Authority").await;
    let manga = "RCAuthorityManga";
    f.manga(manga).await;
    f.chapter("RCAuthorityChapter", manga).await;
    f.mark(manga, "RCAuthorityChapter").await;
    let ids = ["RCAuthorityChapter"];
    let path = f.path(manga, &ids);
    f.success(&f.get(&path).await, manga, &ids, &[true], EPOCH);
    f.send(f.bare(Method::GET, &path).header(EXPECTED_ACCOUNT, &f.user)).await.refused(401);
    f.send(f.bare(Method::GET, &path).header(header::COOKIE, &f.cookie)).await.refused(400);
    for value in ["", "RCAuthorityDifferent"] {
        f.send(f.bare(Method::GET, &path).header(header::COOKIE, &f.cookie).header(EXPECTED_ACCOUNT, value))
            .await.refused(if value.is_empty() { 400 } else { 409 });
    }
    f.send(f.request(&path).header(EXPECTED_ACCOUNT, &f.user)).await.refused(400);
    f.send(f.request(&path).header(EXPECTED_ACCOUNT, "RCAuthorityDifferent")).await.refused(400);
    f.send(f.bare(Method::GET, &path).header(header::COOKIE, format!("{}; {}", f.cookie, f.cookie)).header(EXPECTED_ACCOUNT, &f.user)).await.refused(401);
    f.send(f.request(&path).header(header::COOKIE, &f.cookie)).await.refused(401);
    let empty_cookie = format!("{}=", f.state.config.session_cookie_name);
    f.send(f.bare(Method::GET, &path).header(header::COOKIE, &empty_cookie).header(EXPECTED_ACCOUNT, &f.user)).await.refused(401);
    for cookie in [format!("{}; {empty_cookie}", f.cookie), format!("{empty_cookie}; {}", f.cookie)] {
        f.send(f.bare(Method::GET, &path).header(header::COOKIE, cookie).header(EXPECTED_ACCOUNT, &f.user)).await.refused(401);
    }
    f.send(f.request(&path).header(header::COOKIE, &empty_cookie)).await.refused(401);
    let api_token = "synthetic-completion-api-authority";
    let api_hash = Sha256::digest(api_token.as_bytes()).to_vec();
    let b_hash = Sha256::digest(b"synthetic-completion-account-b").to_vec();
    let db = f.state.db.client().await.unwrap();
    db.execute("INSERT INTO platform.api_clients(id,user_id,name,secret_hash,scopes) VALUES('RCAuthorityApi',$1,'Completion fixture',$2,'[\"self.library.read\"]')", &[&f.user, &api_hash]).await.unwrap();
    db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified) VALUES('RCAuthorityB','completion-authority-b','completion-authority-b@fixture.invalid','synthetic-no-password',true)", &[]).await.unwrap();
    db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES('RCAuthorityBSession','RCAuthorityB',$1,clock_timestamp()+interval '1 hour')", &[&b_hash]).await.unwrap();
    drop(db);
    // Even valid read-scoped API credentials never enter this cookie-only seam.
    for authorization in [format!("Bearer {api_token}"), "Bearer synthetic-invalid".into(), "Basic synthetic".into(), String::new()] {
        f.send(f.request(&path).header(header::AUTHORIZATION, &authorization)).await.refused(403);
        f.send(f.bare(Method::GET, &path).header(EXPECTED_ACCOUNT, &f.user).header(header::AUTHORIZATION, &authorization)).await.refused(403);
    }
    let b_cookie = format!("{}=synthetic-completion-account-b", f.state.config.session_cookie_name);
    f.send(f.bare(Method::GET, &path).header(header::COOKIE, &b_cookie).header(EXPECTED_ACCOUNT, &f.user)).await.refused(409);
    let switched = f.send(f.bare(Method::GET, &path).header(header::COOKIE, &b_cookie).header(EXPECTED_ACCOUNT, "RCAuthorityB")).await;
    switched.private();
    assert_eq!(switched.status, 200);
    let switched = switched.value();
    assert_eq!(switched["account"], json!({"id":"RCAuthorityB","epoch":EPOCH}));
    assert_eq!(switched["data"], json!([{"id":"RCAuthorityChapter","completed":false}]));
    for (change, restore, status) in [
        ("UPDATE platform.sessions SET purpose='recovery' WHERE id='RCAuthoritySession'", "UPDATE platform.sessions SET purpose='normal' WHERE id='RCAuthoritySession'", 403),
        ("UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE id='RCAuthoritySession'", "UPDATE platform.sessions SET revoked_at=NULL WHERE id='RCAuthoritySession'", 401),
        ("UPDATE platform.sessions SET created_at=clock_timestamp()-interval '2 hours',expires_at=clock_timestamp()-interval '1 hour' WHERE id='RCAuthoritySession'", "UPDATE platform.sessions SET expires_at=clock_timestamp()+interval '1 hour' WHERE id='RCAuthoritySession'", 401),
        ("UPDATE platform.users SET disabled=true WHERE id='RCAuthorityUser'", "UPDATE platform.users SET disabled=false WHERE id='RCAuthorityUser'", 401),
    ] {
        f.sql(change).await;
        f.get(&path).await.refused(status);
        f.sql(restore).await;
        f.success(&f.get(&path).await, manga, &ids, &[true], EPOCH);
    }
    f.sql("UPDATE platform.users SET verified=false,roles='[\"user\",\"banned\",\"restricted\"]',permissions='[]' WHERE id='RCAuthorityUser'").await;
    f.success(&f.get(&path).await, manga, &ids, &[true], EPOCH);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn request_contract_preserves_fifty_distinct_ids_and_rejects_noncanonical_or_unbounded_inputs() {
    let f = Fixture::new("Inputs").await;
    let manga = "RCInputsManga";
    f.manga(manga).await;
    let ids: Vec<String> = (0..51).map(|n| format!("RCInputs{n:03}" )).collect();
    for id in &ids { f.chapter(id, manga).await; }
    let first: Vec<&str> = ids.iter().take(50).rev().map(String::as_str).collect();
    let path = f.path(manga, &first);
    f.success(&f.get(&path).await, manga, &first, &[false; 50], EPOCH);
    let all: Vec<&str> = ids.iter().map(String::as_str).collect();
    f.get(&f.path(manga, &all)).await.refused(400);
    f.get(&f.path(manga, &[])).await.refused(400);
    f.get(&f.path(manga, &[&ids[0], &ids[0]])).await.refused(400);
    let one = f.path(manga, &[&ids[0]]);
    for key in ["id", "accountEpoch", "catalogNamespace", "serviceKey"] {
        f.get(&replace(&one, key, None)).await.refused(400);
    }
    for (key, value) in [
        ("id", ""), ("id", "not native"), ("id", "é"), ("id", "chapter/path"), ("id", "chapter.with.dot"),
        ("accountEpoch", ""), ("accountEpoch", "-1"), ("accountEpoch", "+1"), ("accountEpoch", "01"),
        ("accountEpoch", "1.0"), ("accountEpoch", "1e0"), ("accountEpoch", " 1"), ("accountEpoch", "9007199254740992"),
        ("catalogNamespace", ""), ("catalogNamespace", "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA"),
        ("catalogNamespace", "aaaaaaaaaaaa4aaa8aaaaaaaaaaaaaaa"),
        ("serviceKey", ""), ("serviceKey", "NULL"), ("serviceKey", "abc"),
    ] { f.get(&replace(&one, key, Some(value))).await.refused(400); }
    f.get(&replace(&one, "id", Some(&"x".repeat(129)))).await.refused(400);
    f.get(&replace(&one, "serviceKey", Some(&"A".repeat(64)))).await.refused(400);
    for appended in ["accountEpoch=7", "serviceKey=null", "accountId=RCInputsUser", "id[]=RCInputs000", "limit=50", "offset=0", "history=anything", "completed=true", "unknown=%00", "id=%FF", "id=%Q1", "id=%", ""] {
        f.get(&format!("{one}&{appended}")).await.refused(400);
    }
    f.get(&format!("{one}&catalogNamespace={}", f.namespace)).await.refused(400);
    for epoch in [0_u64, 9_007_199_254_740_991] {
        let path = replace(&one, "accountEpoch", Some(&epoch.to_string()));
        f.success(&f.get(&path).await, manga, &[&ids[0]], &[false], epoch);
    }
    // Percent-encoding cannot disguise a duplicate key after strict decoding.
    f.get(&format!("{one}&%69d={}", ids[0])).await.refused(400);
    f.send(f.request(&one).body("x")).await.refused(413);
    f.send(f.request(&one).header(header::CONTENT_TYPE, "application/json").body("{}" )).await.refused(413);
    // Both malformed queries at the cap and a query above it fail closed.
    let (route, raw) = one.split_once('?').unwrap();
    for length in [QUERY_LIMIT, QUERY_LIMIT + 1] {
        let prefix = format!("{raw}&unknown=");
        let query = format!("{prefix}{}", "x".repeat(length - prefix.len()));
        assert_eq!(query.len(), length);
        f.get(&format!("{route}?{query}")).await.refused(400);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn native_eligibility_is_all_or_nothing_and_includes_unavailable_empty_future_and_external() {
    let f = Fixture::new("Eligibility").await;
    let manga = "RCEligibilityManga";
    f.manga(manga).await;
    f.manga("RCEligibilityOther").await;
    let eligible = ["RCEligibilityEmpty", "RCEligibilityUnavailable", "RCEligibilityFuture", "RCEligibilityExternal"];
    for id in eligible { f.chapter(id, manga).await; f.mark(manga, id).await; }
    f.chapter("RCEligibilityDeleted", manga).await;
    f.chapter("RCEligibilityWithdrawn", manga).await;
    f.chapter("RCEligibilityWrongTitle", "RCEligibilityOther").await;
    // This native-only fixture's canonical source helper fails closed for any
    // non-null, unresolved source event. It proves the endpoint honors that
    // predicate, not a live protocol-ledger visible-to-withdrawn transition.
    f.sql("UPDATE platform.catalog_chapters SET is_unavailable=true WHERE id='RCEligibilityUnavailable'; UPDATE platform.catalog_chapters SET published_at=clock_timestamp()+interval '10 years' WHERE id='RCEligibilityFuture'; UPDATE platform.catalog_chapters SET metadata='{\"externalUrl\":\"https://external.fixture.invalid/chapter\",\"data\":[]}' WHERE id='RCEligibilityExternal'; UPDATE platform.catalog_chapters SET deleted_at=clock_timestamp() WHERE id='RCEligibilityDeleted'; UPDATE platform.catalog_chapters SET protocol_event_id=repeat('e',64) WHERE id='RCEligibilityWithdrawn';").await;
    f.success(&f.get(&f.path(manga, &eligible)).await, manga, &eligible, &[true; 4], EPOCH);
    for ineligible in ["RCEligibilityDeleted", "RCEligibilityWithdrawn", "RCEligibilityWrongTitle", "RCEligibilityMissing"] {
        // A valid completed row first must not leak when a later requested row
        // is absent, hidden or belongs to another parent.
        f.get(&f.path(manga, &[eligible[0], ineligible])).await.refused(409);
    }
    f.get(&f.path("RCEligibilityAbsentParent", &[eligible[0]])).await.refused(404);
    for (hide, restore) in [
        ("UPDATE platform.catalog_manga SET state='draft' WHERE id='RCEligibilityManga'", "UPDATE platform.catalog_manga SET state='published' WHERE id='RCEligibilityManga'"),
        ("UPDATE platform.catalog_manga SET deleted_at=clock_timestamp() WHERE id='RCEligibilityManga'", "UPDATE platform.catalog_manga SET deleted_at=NULL WHERE id='RCEligibilityManga'"),
        ("UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='RCEligibilityManga'", "UPDATE platform.catalog_manga SET protocol_source_visible=true WHERE id='RCEligibilityManga'"),
        ("UPDATE platform.catalog_manga SET protocol_series_id=repeat('d',64) WHERE id='RCEligibilityManga'", "UPDATE platform.catalog_manga SET protocol_series_id=NULL WHERE id='RCEligibilityManga'"),
    ] {
        f.sql(hide).await;
        f.get(&f.path(manga, &eligible)).await.refused(404);
        f.sql(restore).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn expected_catalogue_identity_mismatch_refuses_without_account_history() {
    let f = Fixture::new("Identity").await;
    let manga = "RCIdentityManga";
    f.manga(manga).await;
    f.chapter("RCIdentityChapter", manga).await;
    f.mark(manga, "RCIdentityChapter").await;
    let path = f.path(manga, &["RCIdentityChapter"]);
    let other = if f.namespace == "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa" {
        "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"
    } else { "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa" };
    f.get(&replace(&path, "catalogNamespace", Some(other))).await.refused(409);
    f.get(&replace(&path, "serviceKey", Some(&"a".repeat(64)))).await.refused(409);
    f.success(&f.get(&path).await, manga, &["RCIdentityChapter"], &[true], EPOCH);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn pending_user_disable_roles_and_exact_session_revocation_fail_bounded_without_partial_data() {
    let f = Fixture::new("Locks").await;
    let manga = "RCLocksManga";
    f.manga(manga).await;
    f.chapter("RCLocksChapter", manga).await;
    f.mark(manga, "RCLocksChapter").await;
    let path = f.path(manga, &["RCLocksChapter"]);
    for statement in [
        "UPDATE platform.users SET disabled=true WHERE id='RCLocksUser'",
        "UPDATE platform.users SET roles='[\"restricted\"]' WHERE id='RCLocksUser'",
        "UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE id='RCLocksSession'",
        "UPDATE platform.sessions SET purpose='recovery' WHERE id='RCLocksSession'",
    ] {
        let mut connection = f.state.db.client().await.unwrap();
        let pending = connection.transaction().await.unwrap();
        pending.batch_execute(statement).await.unwrap();
        let start = Instant::now();
        // This establishes bounded conflict while the writer stays uncommitted.
        // It does NOT prove NOWAIT: an ordinary SHARE lock with the production
        // 250ms lock_timeout would also satisfy this two-second upper bound.
        // Literal NOWAIT is separately visible in the reviewed auth SQL source.
        let response = tokio::time::timeout(Duration::from_secs(2), f.get(&path)).await
            .expect("Pending authority updates must refuse within the bounded conflict budget");
        response.refused(409);
        assert!(start.elapsed() < Duration::from_secs(2));
        pending.rollback().await.unwrap();
        f.success(&f.get(&path).await, manga, &["RCLocksChapter"], &[true], EPOCH);
    }
    // An unrelated session lock must not turn an exact-session check into a
    // user-wide session scan or allow the exact revoked session to survive.
    let other_hash = Sha256::digest(b"synthetic-completion-unrelated-session").to_vec();
    f.state.db.client().await.unwrap().execute(
        "INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES('RCLocksOtherSession',$1,$2,clock_timestamp()+interval '1 hour')",
        &[&f.user, &other_hash],
    ).await.unwrap();
    let mut connection = f.state.db.client().await.unwrap();
    let pending = connection.transaction().await.unwrap();
    pending.execute("UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE id='RCLocksOtherSession'", &[]).await.unwrap();
    f.success(&f.get(&path).await, manga, &["RCLocksChapter"], &[true], EPOCH);
    pending.rollback().await.unwrap();
    f.state.db.client().await.unwrap().execute(
        "UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE id=$1", &[&f.session],
    ).await.unwrap();
    f.get(&path).await.refused(401);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn route_local_get_preflight_is_credentialed_but_grants_no_other_header_origin_route_or_method() {
    let f = Fixture::new("Cors").await;
    let manga = "RCCorsManga";
    f.manga(manga).await;
    f.chapter("RCCorsChapter", manga).await;
    let path = f.path(manga, &["RCCorsChapter"]);
    let preflight = |route: &str, origin: &str, method: &str, headers: &str| {
        f.bare(Method::OPTIONS, route).header(header::ORIGIN, origin)
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, method)
            .header(header::ACCESS_CONTROL_REQUEST_HEADERS, headers)
    };
    let allowed = f.send(preflight(&path, &f.origin, "GET", "X-WeebDex-Expected-Account")).await;
    allowed.private();
    assert_eq!(allowed.status, 204);
    assert!(allowed.body.is_empty());
    assert_eq!(allowed.headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(), f.origin.as_str());
    assert_eq!(allowed.headers.get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).unwrap(), "true");
    let allowed_methods = allowed.headers.get(header::ACCESS_CONTROL_ALLOW_METHODS).unwrap().to_str().unwrap();
    assert_eq!(allowed_methods.split(',').map(str::trim).collect::<Vec<_>>(), ["GET"]);
    let allowed_headers = allowed.headers.get(header::ACCESS_CONTROL_ALLOW_HEADERS).unwrap().to_str().unwrap().to_ascii_lowercase();
    assert!(allowed_headers.split(',').map(str::trim).any(|h| h == EXPECTED_ACCOUNT));
    assert!(allowed_headers.split(',').map(str::trim).all(|h| ["accept", "authorization", "content-type", "x-csrf-token", EXPECTED_ACCOUNT].contains(&h)),
        "No widening beyond the established boundary headers and this route's expected-account header");
    let actual = f.send(f.request(&path).header(header::ORIGIN, &f.origin)).await;
    f.success(&actual, manga, &["RCCorsChapter"], &[false], EPOCH);
    assert_eq!(actual.headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(), f.origin.as_str());
    for method in ["HEAD", "POST", "PUT", "PATCH", "DELETE"] {
        f.send(preflight(&path, &f.origin, method, EXPECTED_ACCOUNT)).await.refused(403);
    }
    for headers in ["X-WeebDex-Expected-Account, X-Private-Override", "X-Private-Override"] {
        f.send(preflight(&path, &f.origin, "GET", headers)).await.refused(403);
    }
    f.send(preflight(&path, "https://not-approved.fixture.invalid", "GET", EXPECTED_ACCOUNT)).await.refused(403);
    f.send(f.request(&path).header(header::ORIGIN, "https://not-approved.fixture.invalid")).await.refused(403);
    for other in ["/api/user/history/manga", "/api/manga/RCCorsManga/reader", "/api/v1/user/history/manga/RCCorsManga/completion"] {
        let refused = f.send(preflight(other, &f.origin, "GET", EXPECTED_ACCOUNT)).await;
        assert_eq!(refused.status, 403);
        assert!(!refused.headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
        assert!(!refused.headers.contains_key(header::ACCESS_CONTROL_ALLOW_HEADERS));
    }
    let public = f.send(f.bare(Method::GET, &path.replacen("/api/", "/api/v1/", 1))
        .header(header::COOKIE, &f.cookie).header(EXPECTED_ACCOUNT, &f.user)).await;
    assert_eq!(public.status, 404, "There is deliberately no public-ID completion endpoint");
    // Retained HTTP HEAD dispatch executes GET admission but returns no entity.
    let head = f.send(f.bare(Method::HEAD, &path).header(header::COOKIE, &f.cookie).header(EXPECTED_ACCOUNT, &f.user)).await;
    head.private();
    assert_eq!(head.status, 200);
    assert!(head.body.is_empty());
    let bad_head = f.send(f.bare(Method::HEAD, &path).header(header::COOKIE, &f.cookie).header(EXPECTED_ACCOUNT, "RCCorsDifferent")).await;
    bad_head.private();
    assert_eq!(bad_head.status, 409);
    assert!(bad_head.body.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn maximum_native_id_payload_counts_the_whole_wire_envelope_without_truncating_rows() {
    let f = Fixture::new("Bytes").await;
    let manga = format!("RCBytes{}", "m".repeat(121));
    assert_eq!(manga.len(), 128);
    f.manga(&manga).await;
    let ids: Vec<String> = (0..50).map(|n| format!("RCBytes{n:03}{}", "c".repeat(118))).collect();
    for id in &ids { assert_eq!(id.len(), 128); f.chapter(id, &manga).await; }
    let requested: Vec<&str> = ids.iter().rev().map(String::as_str).collect();
    let path = f.path(&manga, &requested);
    assert!(path.split_once('?').unwrap().1.len() <= QUERY_LIMIT);
    let response = f.get(&path).await;
    f.success(&response, &manga, &requested, &[false; 50], EPOCH);
    assert!(response.body.len() > 50 * 128, "Budget must include envelope, keys and booleans, not IDs alone");
    if let Some(length) = response.headers.get(header::CONTENT_LENGTH) {
        assert_eq!(length.to_str().unwrap().parse::<usize>().unwrap(), response.body.len());
    }
    // Admitted scalar limits make a >16KiB success envelope unreachable here.
    // Serializer-overflow refusal belongs to a serializer unit fixture, not a
    // synthetic HTTP case with impossible admitted rows or an altered schema.
}

// A valid body may have no data bytes remaining yet never end (for example,
// pending trailers). Size hints and Content-Length do not prove end-of-stream.
// This body is supplied to the actual complete application router, not to a
// substitute endpoint or mocked handler. It deliberately never yields a frame.
struct NoFrameBody {
    polls: Arc<AtomicUsize>,
    dropped: Arc<AtomicBool>,
}
impl axum::body::HttpBody for NoFrameBody {
    type Data = axum::body::Bytes;
    type Error = std::convert::Infallible;
    fn poll_frame(self: Pin<&mut Self>, _cx: &mut Context<'_>)
        -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        Poll::Pending
    }
    fn is_end_stream(&self) -> bool { false }
    fn size_hint(&self) -> http_body::SizeHint { http_body::SizeHint::with_exact(0) }
}
impl Drop for NoFrameBody {
    fn drop(&mut self) { self.dropped.store(true, Ordering::SeqCst); }
}
fn no_frame_body() -> (axum::body::Body, Arc<AtomicUsize>, Arc<AtomicBool>) {
    let polls = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicBool::new(false));
    (axum::body::Body::new(NoFrameBody { polls: polls.clone(), dropped: dropped.clone() }), polls, dropped)
}
async fn router_wire(response: axum::response::Response) -> Wire {
    let (parts, body) = response.into_parts();
    let body = tokio::time::timeout(Duration::from_secs(2), axum::body::to_bytes(body, RESPONSE_LIMIT))
        .await.expect("Response body must finish").unwrap().to_vec();
    Wire { status: parts.status.as_u16(), headers: parts.headers, body }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn full_router_refuses_never_ending_zero_hint_bodies_without_polling_or_leaking_admission() {
    let f = Fixture::new("IntakeRouter").await;
    let manga = "RCIntakeRouterManga";
    let chapter = "RCIntakeRouterChapter";
    f.manga(manga).await;
    f.chapter(chapter, manga).await;
    let path = f.path(manga, &[chapter]);
    let app = weebdex_platform::router::router(f.state.clone());
    let request_slots = f.state.requests.available_permits();
    // Leave exactly one LibraryHead slot. A leaked owned request would exhaust
    // it, even if the caller stopped waiting for the first response.
    let mut occupied = Vec::new();
    for _ in 1..f.state.config.admission_config().library_head.active {
        occupied.push(f.state.reads.acquire(manga_node::read_admission::Class::LibraryHead).await.unwrap());
    }
    for method in ["GET", "HEAD"] {
      for length_zero in [false, true] {
        let (body, polls, dropped) = no_frame_body();
        let mut request = axum::http::Request::builder().method(method).uri(path.as_str())
            .header(header::COOKIE, f.cookie.as_str()).header(EXPECTED_ACCOUNT, f.user.as_str());
        if length_zero { request = request.header(header::CONTENT_LENGTH, "0"); }
        let response = tokio::time::timeout(Duration::from_secs(2), app.clone().oneshot(request.body(body).unwrap()))
            .await.expect("A no-frame body must not wait for intake or the handler's database deadline").unwrap();
        let wire = router_wire(response).await;
        if method == "GET" {
            wire.refused(413);
        } else {
            wire.private();
            assert_eq!(wire.status, 413);
            assert!(wire.body.is_empty(), "HEAD preserves refusal status and privacy without an entity");
        }
        assert_eq!(polls.load(Ordering::SeqCst), 0, "The body must be refused, not polled under a byte-only cap");
        assert!(dropped.load(Ordering::SeqCst), "Do not retain the unfinished body after refusal");
        let all_requests = tokio::time::timeout(Duration::from_secs(2), f.state.requests.clone().acquire_many_owned(request_slots as u32))
            .await.expect("Every request admission permit must be returned").unwrap();
        drop(all_requests);
        let last_read = f.state.reads.acquire(manga_node::read_admission::Class::LibraryHead)
            .await.expect("The sole remaining LibraryHead slot must be returned");
        drop(last_read);
        let empty = axum::http::Request::builder().method(method).uri(path.as_str())
            .header(header::COOKIE, f.cookie.as_str()).header(EXPECTED_ACCOUNT, f.user.as_str())
            .body(axum::body::Body::empty()).unwrap();
        let response = tokio::time::timeout(Duration::from_secs(8), app.clone().oneshot(empty))
            .await.expect("Ordinary empty GET must reach the real completion query").unwrap();
        let wire = router_wire(response).await;
        if method == "GET" {
            f.success(&wire, manga, &[chapter], &[false], EPOCH);
        } else {
            wire.private();
            assert_eq!(wire.status, 200);
            assert!(wire.body.is_empty(), "Ordinary empty HEAD still reaches GET dispatch");
        }
      }
    }
    drop(occupied);
    // Do not turn this correction into a global GET-body policy. The retained
    // health GET ignores an unused body and must keep doing so.
    let (body, polls, dropped) = no_frame_body();
    let request = axum::http::Request::builder().uri("/api/health").body(body).unwrap();
    let response = tokio::time::timeout(Duration::from_secs(2), app.oneshot(request))
        .await.expect("Unrelated GET semantics remain unchanged").unwrap();
    assert_eq!(router_wire(response).await.status, 200);
    assert_eq!(polls.load(Ordering::SeqCst), 0);
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn real_http_get_with_headers_but_no_body_frames_is_refused_before_the_peer_ends_intake() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let f = Fixture::new("IntakeWire").await;
    let manga = "RCIntakeWireManga";
    let chapter = "RCIntakeWireChapter";
    f.manga(manga).await;
    f.chapter(chapter, manga).await;
    let path = f.path(manga, &[chapter]);
    let address = url::Url::parse(&f.base).unwrap();
    let port = address.port().unwrap();
    let mut peer = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let headers = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nCookie: {}\r\nX-WeebDex-Expected-Account: {}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n", f.cookie, f.user);
    peer.write_all(headers.as_bytes()).await.unwrap();
    // Keep the socket open without sending a chunk, terminator or EOF. A
    // zero-byte Bytes extractor would wait forever here before handler entry.
    let response_headers = tokio::time::timeout(Duration::from_secs(2), async {
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            assert!(bytes.len() < RESPONSE_LIMIT, "Bound the fixture's header capture");
            let mut byte = [0; 1];
            peer.read_exact(&mut byte).await.expect("Receive refusal before closing the request body");
            bytes.push(byte[0]);
        }
        String::from_utf8(bytes).unwrap().to_ascii_lowercase()
    }).await.expect("The real server must refuse a no-frame GET without waiting for client EOF");
    assert!(response_headers.starts_with("http/1.1 413 "), "{response_headers}");
    let cache = response_headers.lines().find_map(|line| line.strip_prefix("cache-control:")).unwrap();
    let cache: Vec<&str> = cache.split(',').map(str::trim).collect();
    assert!(cache.contains(&"private") && cache.contains(&"no-store"));
    drop(peer);
    f.success(&f.get(&path).await, manga, &[chapter], &[false], EPOCH);
    // HTTP/2 end-stream behavior still needs transport qualification. The
    // full-router no-frame body above is the causal stream-contract control,
    // not a claim that this HTTP/1.1 fixture executed an HTTP/2 connection.
}

// Remaining deterministic SQL-race fixtures, not claimed as authored/executed:
// 1. Pause an admitted reader after authority locks, then prove disable/role and
//    exact-session revoke writers serialize until commit (opposite direction).
// 2. Synchronize catalogue namespace, parent/chapter visibility and membership
//    commits between reads to prove one RepeatableRead snapshot, including 40001.
// 3. Exercise expiry crossing while a request is in flight. The contract fences
//    authority at its transaction checks, not at eventual response delivery.
// A protocol-enabled fixture must separately cover an actual source-ledger
// withdrawal transition and a matching non-null configured service key.
// Pending-writer bounded refusal above is real lock-contention fixture source;
// it does not distinguish NOWAIT from lock_timeout and is not a substitute for
// these separately synchronized race controls or a causal NOWAIT control.
