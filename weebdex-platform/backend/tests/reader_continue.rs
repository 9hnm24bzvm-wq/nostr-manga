//! Source-authored, ignored acceptance controls for native automatic Continue.
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
const RESPONSE_LIMIT: usize = 32 * 1024;
const QUERY_LIMIT: usize = 16 * 1024;
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
        assert!(value.get("target").is_none() && value.get("winner").is_none(), "No target on refusal");
        assert!(!String::from_utf8_lossy(&self.body).contains("\"completed\""), "No history flags on refusal");
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
        let origin = "https://reader-continue.fixture.invalid".to_owned();
        let state = AppState::connect(Config {
            database_plaintext_local: true,
            database_connections: 4,
            public_origin: origin.clone(),
            uploads_dir: runtime.join(format!("continue-{name}-media")),
            ui_dir: runtime.join("no-ui"),
            ..Config::default()
        }, dsn).await.unwrap();
        state.db.migrate(domains::MIGRATIONS).await.unwrap();
        state.db.verify_schema(domains::MIGRATIONS).await.unwrap();
        let user = format!("NC{name}User");
        let session = format!("NC{name}Session");
        let token = format!("synthetic-continue-session-{name}");
        let hash = Sha256::digest(token.as_bytes()).to_vec();
        let db = state.db.client().await.unwrap();
        db.execute(
            "INSERT INTO platform.users(id,username,email,password_hash,verified) VALUES($1,$2,$3,'synthetic-no-password',true)",
            &[&user, &format!("continue-{}", name.to_lowercase()), &format!("continue-{name}@fixture.invalid")],
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
            "INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES($1,$2,'Continue fixture',$1,'ja','ongoing','safe','published')",
            &[&id, &self.user],
        ).await.unwrap();
    }
    async fn chapter(&self, id: &str, manga: &str, volume: Option<&str>, chapter: Option<&str>) {
        self.state.db.client().await.unwrap().execute(
            "INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,volume,chapter,language,published_at) VALUES($1,$2,$3,'Continue fixture',$4,$5,'en','2024-01-01')",
            &[&id, &manga, &self.user, &volume, &chapter],
        ).await.unwrap();
    }
    async fn mark(&self, manga: &str, id: &str) {
        self.state.db.client().await.unwrap().execute(
            "INSERT INTO platform.library_chapter_history(user_id,manga_id,chapter_id) VALUES($1,$2,$3)",
            &[&self.user, &manga, &id],
        ).await.unwrap();
    }
    fn path(&self, manga: &str) -> String {
        encode(&format!("/api/user/history/manga/{manga}/continue"), &[
            ("accountEpoch".into(), EPOCH.to_string()),
            ("catalogNamespace".into(), self.namespace.clone()),
            ("serviceKey".into(), "null".into()),
            ("requestId".into(), "click_1-A".into()),
        ])
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
    fn success(&self, wire: &Wire, manga: &str, winner: Option<&str>, reason: &str, multiple: bool) -> Value {
        wire.private();
        assert_eq!(wire.status, 200, "{}", String::from_utf8_lossy(&wire.body));
        let value = wire.value();
        exact_keys(&value, &["version", "operation", "orderingProfile", "mangaId", "catalogIdentity", "account", "requestId", "scope", "consistency", "continueProof", "crossRequestSnapshot", "state", "reason", "target", "winner", "hasMultipleEditions"]);
        assert_eq!(value["version"], 1);
        assert_eq!(value["operation"], "native-continue");
        assert_eq!(value["orderingProfile"], "native-aggregate-v1");
        assert_eq!(value["mangaId"], manga);
        assert_eq!(value["catalogIdentity"], json!({"catalogNamespace":self.namespace,"serviceKey":null}));
        assert_eq!(value["account"], json!({"id":self.user,"epoch":EPOCH}));
        assert_eq!(value["requestId"], "click_1-A");
        assert_eq!(value["consistency"], "live-request-v1");
        assert_eq!(value["continueProof"], "native-account-snapshot-v1");
        assert_eq!(value["crossRequestSnapshot"], false);
        assert_eq!(value["reason"], reason);
        assert_eq!(value["hasMultipleEditions"], multiple);
        let scope = value["scope"].as_str().unwrap();
        assert_eq!(scope.len(), 64);
        assert!(scope.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        if let Some(winner) = winner {
            assert_eq!(value["state"], "ready");
            assert_eq!(value["winner"]["id"], winner);
            exact_keys(&value["target"], &["volume", "chapter", "anchorChapterId"]);
            assert_eq!(value["target"], json!({"volume":value["winner"]["volume"], "chapter":value["winner"]["chapter"], "anchorChapterId":winner}));
            exact_keys(&value["winner"], &["id", "volume", "chapter", "language", "publishedAt", "version", "groupIds", "sharedGroupScore", "witness"]);
            assert_eq!(value["winner"]["sharedGroupScore"], 0);
            assert!(value["winner"]["groupIds"].as_array().unwrap().len() <= 100);
            assert_eq!(value["winner"]["witness"].as_str().unwrap().len(), 64);
        } else {
            assert_eq!(value["state"], "boundary");
            assert!(value["target"].is_null() && value["winner"].is_null());
            assert!(!multiple);
        }
        value
    }
    async fn reference(&self, manga: &str, filters: &str) -> Value {
        // Both real HTTP implementations see a quiescent fixture here. The
        // independent oracle runs the ACTUAL composed retained selection block.
        let aggregate = self.send(self.bare(Method::GET, &format!("/api/manga/{manga}/aggregate?{filters}"))).await;
        assert_eq!(aggregate.status, 200, "Retained aggregate fixture must fit its unchanged 10K cap");
        let history: Value = self.state.db.client().await.unwrap().query_one(
            "SELECT COALESCE(jsonb_object_agg(chapter_id,NULL),'{}'::jsonb) FROM platform.library_chapter_history WHERE user_id=$1 AND manga_id=$2", &[&self.user,&manga],
        ).await.unwrap().get(0);
        composed_reference(&aggregate.value(), &history)
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


fn composed_reference(aggregate: &Value, history: &Value) -> Value {
    use std::{io::Write, process::{Command, Stdio}};
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/native_continue_reference.mjs");
    let mut process = Command::new("node").arg(script).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
        .expect("The separately admitted parity runner requires Node and the composed frontend source");
    process.stdin.take().unwrap().write_all(serde_json::to_string(&json!({"aggregate":aggregate,"history":history})).unwrap().as_bytes()).unwrap();
    let output = process.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).unwrap()
}
fn parity(actual: &Value, reference: &Value) {
    assert_eq!(actual["target"], reference["target"]);
    assert_eq!(actual["hasMultipleEditions"], reference["hasMultipleEditions"]);
    assert_eq!(actual["winner"]["id"], reference["winnerId"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Source-authored only; isolated PostgreSQL and composed-source runner required"]
async fn no_partial_all_history_and_alternate_editions_match_actual_composed_algorithm() {
    let f = Fixture::new("Parity").await;
    let manga = "NCParityManga";
    f.manga(manga).await;
    for (id, volume, chapter) in [("NCParityOldA","1","2"),("NCParityOldZ","1","2"),("NCParityMiddle","1","10"),("NCParityNew","2","1")] {
        f.chapter(id, manga, Some(volume), Some(chapter)).await;
    }
    let path = f.path(manga);
    let before = f.own_snapshot().await;
    let empty = f.success(&f.get(&path).await, manga, Some("NCParityOldA"), "oldest-unread", true);
    parity(&empty, &f.reference(manga, "").await);
    // No history and an empty history object are both the actual old algorithm.
    let aggregate = f.send(f.bare(Method::GET, &format!("/api/manga/{manga}/aggregate"))).await.value();
    assert_eq!(composed_reference(&aggregate, &Value::Null), composed_reference(&aggregate, &json!({})));
    assert_eq!(f.own_snapshot().await, before, "GET is not a progress/history/session mutation");
    f.mark(manga, "NCParityOldZ").await; // non-winner alternate edition completes the group
    f.sql("INSERT INTO platform.library_chapter_progress(user_id,chapter_id,page,completed) VALUES('NCParityUser','NCParityOldZ',0,false),('NCParityUser','NCParityMiddle',9,true)").await;
    let partial = f.success(&f.get(&path).await, manga, Some("NCParityMiddle"), "oldest-unread", false);
    parity(&partial, &f.reference(manga, "").await);
    f.mark(manga, "NCParityMiddle").await;
    let partial = f.success(&f.get(&path).await, manga, Some("NCParityNew"), "oldest-unread", false);
    parity(&partial, &f.reference(manga, "").await);
    f.mark(manga, "NCParityNew").await;
    let all = f.success(&f.get(&path).await, manga, Some("NCParityNew"), "all-completed-newest", false);
    parity(&all, &f.reference(manga, "").await);
    assert_eq!(empty["scope"], all["scope"], "Scope is not a history revision");
    // No client-provided capture can turn the observed request into a durable snapshot.
    let later = f.get(&replace(&replace(&path, "accountEpoch", Some("8")), "requestId", Some("next-click"))).await.value();
    assert_eq!(later["account"]["epoch"], 8);
    assert_eq!(later["requestId"], "next-click");
    assert_eq!(later["scope"], all["scope"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Source-authored only; isolated PostgreSQL and composed-source runner required"]
async fn exact_nullable_special_numeric_equivalent_labels_and_js_entry_keys_match_native_order() {
    let f = Fixture::new("Labels").await;
    let manga = "NCLabelsManga";
    f.manga(manga).await;
    let labels = [None, Some(""), Some("special"), Some("2a"), Some("02"), Some("2"), Some("2.0"), Some("9007199254740992"), Some("9007199254740993"), Some("2.0000000000000000000000000001"), Some("2.0000000000000000000000000002"), Some("😀"), Some("\u{e000}")];
    for (index, label) in labels.iter().enumerate() {
        f.chapter(&format!("NCLabels{index:02}"), manga, None, *label).await;
    }
    // Same chapter label in different raw volumes must remain separate.
    for (index, volume) in [None, Some(""), Some("02"), Some("2"), Some("2.0")].iter().enumerate() {
        f.chapter(&format!("NCVolume{index:02}"), manga, *volume, Some("volume-case")).await;
    }
    let path = f.path(manga);
    for _ in 0..labels.len()+5 {
        let reference = f.reference(manga, "").await;
        let value = f.get(&path).await;
        let winner = reference["winnerId"].as_str().unwrap();
        let actual = f.success(&value, manga, Some(winner), "oldest-unread", false);
        parity(&actual, &reference);
        f.mark(manga, winner).await;
    }
    let reference = f.reference(manga, "").await;
    parity(&f.success(&f.get(&path).await, manga, reference["winnerId"].as_str(), "all-completed-newest", false), &reference);
    let editions = "NCLabelsEditions";
    f.manga(editions).await;
    for id in ["4294967295", "4294967294", "10", "2", "0", "01", "NCordinaryA", "NCordinarya"] {
        f.chapter(id, editions, None, Some("")).await;
    }
    for id in ["0", "2", "10", "4294967294", "01", "4294967295", "NCordinaryA", "NCordinarya"] {
        let reference = f.reference(editions, "").await;
        assert_eq!(reference["winnerId"], id);
        parity(&f.success(&f.get(&f.path(editions)).await, editions, Some(id), "oldest-unread", id != "NCordinarya"), &reference);
        f.state.db.client().await.unwrap().execute("UPDATE platform.catalog_chapters SET deleted_at=clock_timestamp() WHERE id=$1", &[&id]).await.unwrap();
    }
    f.success(&f.get(&f.path(editions)).await, editions, None, "empty", false);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Source-authored only; isolated PostgreSQL runner required; no performance claim"]
async fn complete_sets_exceed_ten_thousand_groups_and_editions_without_preliminary_caps() {
    let f = Fixture::new("Large").await;
    let manga = "NCLargeManga";
    f.manga(manga).await;
    f.sql("INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,volume,chapter,language,published_at) SELECT 'NCLarge'||lpad(n::text,5,'0'),'NCLargeManga','NCLargeUser','Continue fixture','1',n::text,'en','2024-01-01' FROM generate_series(1,10020)n").await;
    f.success(&f.get(&f.path(manga)).await, manga, Some("NCLarge00001"), "oldest-unread", false);
    f.sql("INSERT INTO platform.library_chapter_history(user_id,manga_id,chapter_id) SELECT 'NCLargeUser',manga_id,id FROM platform.catalog_chapters WHERE manga_id='NCLargeManga' AND chapter::integer<=10010").await;
    f.success(&f.get(&f.path(manga)).await, manga, Some("NCLarge10011"), "oldest-unread", false);
    f.sql("INSERT INTO platform.library_chapter_history(user_id,manga_id,chapter_id) SELECT 'NCLargeUser',manga_id,id FROM platform.catalog_chapters WHERE manga_id='NCLargeManga' ON CONFLICT DO NOTHING").await;
    f.success(&f.get(&f.path(manga)).await, manga, Some("NCLarge10020"), "all-completed-newest", false);
    let editions = "NCLargeEditions";
    f.manga(editions).await;
    f.sql("INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,volume,chapter,language,published_at) SELECT 'NCEdition'||lpad(n::text,5,'0'),'NCLargeEditions','NCLargeUser','Continue fixture','1','1','en','2025-01-01' FROM generate_series(1,10020)n").await;
    f.chapter("1", editions, Some("1"), Some("1")).await;
    f.chapter("NCLargeNew", editions, Some("1"), Some("2")).await;
    // Oldest publication inserted last still wins via native Object.entries.
    f.success(&f.get(&f.path(editions)).await, editions, Some("1"), "oldest-unread", true);
    // Membership of an edition beyond 10K completes the entire eligible group.
    f.mark(editions, "NCEdition10020").await;
    f.success(&f.get(&f.path(editions)).await, editions, Some("NCLargeNew"), "oldest-unread", false);
    f.mark(editions, "NCLargeNew").await;
    f.success(&f.get(&f.path(editions)).await, editions, Some("NCLargeNew"), "all-completed-newest", false);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Source-authored only; isolated PostgreSQL and composed-source runner required"]
async fn native_filters_exclude_only_their_own_history_and_do_not_inherit_public_defaults() {
    let f = Fixture::new("Filters").await;
    let manga = "NCFiltersManga";
    f.manga(manga).await;
    f.manga("NCFiltersForeign").await;
    for (id, chapter) in [("NCFiltersEligible","1"),("NCFiltersFrench","1"),("NCFiltersDeleted","1"),("NCFiltersHidden","1"),("NCFiltersNewest","2")] {
        f.chapter(id, manga, Some("1"), Some(chapter)).await;
    }
    f.chapter("NCFiltersWrongTitle", "NCFiltersForeign", Some("1"), Some("1")).await;
    for id in ["NCFiltersFrench", "NCFiltersDeleted", "NCFiltersHidden"] { f.mark(manga, id).await; }
    f.mark("NCFiltersForeign", "NCFiltersWrongTitle").await;
    f.mark("NCFiltersForeign", "NCFiltersEligible").await; // deliberately mismatched parent history cannot complete this group
    f.sql("UPDATE platform.catalog_chapters SET language='fr' WHERE id='NCFiltersFrench'; UPDATE platform.catalog_chapters SET deleted_at=clock_timestamp() WHERE id='NCFiltersDeleted'; UPDATE platform.catalog_chapters SET protocol_event_id=repeat('ed',32) WHERE id='NCFiltersHidden'; INSERT INTO platform.users(id,username,email,password_hash) VALUES('NCFiltersOtherUser','continue-other','continue-other@fixture.invalid','synthetic'); INSERT INTO platform.library_chapter_history(user_id,manga_id,chapter_id) VALUES('NCFiltersOtherUser','NCFiltersManga','NCFiltersEligible'); UPDATE platform.catalog_chapters SET is_unavailable=true,published_at=clock_timestamp()+interval '10 years',metadata='{\"externalUrl\":\"https://external.fixture.invalid/chapter\",\"data\":[]}' WHERE id='NCFiltersEligible'; INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES('NCFiltersGroup','NCFiltersUser','Continue fixture','Group'),('NCFiltersExcluded','NCFiltersUser','Continue fixture','Excluded'); INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES('NCFiltersEligible','NCFiltersGroup'),('NCFiltersFrench','NCFiltersExcluded');").await;
    let path = f.path(manga);
    for filters in ["tlang=en", "tlang[]=en&tlangx=fr", "group=NCFiltersGroup", "groupx=NCFiltersExcluded", "id=NCFiltersEligible", "uploader=NCFiltersUser&tlang=en", "uploaderx=NCFiltersOtherUser&tlang=en", "manga=NCFiltersManga&tlang=en"] {
        let actual = f.success(&f.get(&format!("{path}&{filters}")).await, manga, Some("NCFiltersEligible"), "oldest-unread", false);
        parity(&actual, &f.reference(manga, filters).await);
    }
    // Unfiltered eligible alternate history completes group 1; neither history
    // from other accounts nor excluded/deleted editions can do so on their own.
    f.success(&f.get(&path).await, manga, Some("NCFiltersNewest"), "oldest-unread", false);
    for filters in ["tlang=ja", "tlang=en&tlangx=en", "group=NCFiltersGroup&groupx=NCFiltersGroup", "manga=NCFiltersForeign", "id=NCFiltersWrongTitle"] {
        f.success(&f.get(&format!("{path}&{filters}")).await, manga, None, "empty", false);
    }
    // Native group matching uses relation membership even for a deleted group;
    // visible projection identities are independently filtered, just as today.
    f.sql("UPDATE platform.catalog_groups SET deleted_at=clock_timestamp() WHERE id='NCFiltersGroup'").await;
    let actual = f.success(&f.get(&format!("{path}&group=NCFiltersGroup")).await, manga, Some("NCFiltersEligible"), "oldest-unread", false);
    assert_eq!(actual["winner"]["groupIds"], json!([]));
    parity(&actual, &f.reference(manga, "group=NCFiltersGroup").await);
    f.get(&f.path("NCFiltersAbsent")).await.refused(404);
    for (hide, restore) in [("state='draft'","state='published'"),("deleted_at=clock_timestamp()","deleted_at=NULL"),("protocol_source_visible=false","protocol_source_visible=true"),("protocol_series_id=repeat('ef',32)","protocol_series_id=NULL")] {
        f.sql(&format!("UPDATE platform.catalog_manga SET {hide} WHERE id='{manga}'")).await;
        f.get(&path).await.refused(404);
        f.sql(&format!("UPDATE platform.catalog_manga SET {restore} WHERE id='{manga}'")).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Source-authored only; isolated PostgreSQL runner required"]
async fn capture_cookie_only_authority_and_bounded_query_refusals_never_return_targets() {
    let f = Fixture::new("Authority").await;
    let manga = "NCAuthorityManga";
    f.manga(manga).await;
    f.chapter("NCAuthorityOld", manga, Some("1"), Some("1")).await;
    f.chapter("NCAuthorityNew", manga, Some("1"), Some("2")).await;
    f.mark(manga, "NCAuthorityOld").await;
    let path = f.path(manga);
    f.send(f.bare(Method::GET, &path).header(EXPECTED_ACCOUNT, &f.user)).await.refused(401);
    f.send(f.bare(Method::GET, &path).header(header::COOKIE, &f.cookie)).await.refused(400);
    f.send(f.bare(Method::GET, &path).header(header::COOKIE, &f.cookie).header(EXPECTED_ACCOUNT, "")).await.refused(400);
    f.send(f.bare(Method::GET, &path).header(header::COOKIE, &f.cookie).header(EXPECTED_ACCOUNT, "different")).await.refused(409);
    f.send(f.request(&path).header(EXPECTED_ACCOUNT, &f.user)).await.refused(400);
    for cookie in [format!("{}; {}=", f.cookie, f.state.config.session_cookie_name), format!("{}; {}", f.cookie, f.cookie)] {
        f.send(f.bare(Method::GET, &path).header(header::COOKIE, cookie).header(EXPECTED_ACCOUNT, &f.user)).await.refused(401);
    }
    let api_hash = Sha256::digest(b"synthetic-continue-api").to_vec();
    f.state.db.client().await.unwrap().execute("INSERT INTO platform.api_clients(id,user_id,name,secret_hash,scopes) VALUES('NCAuthorityApi',$1,'Continue fixture',$2,'[\"self.library.read\"]')", &[&f.user,&api_hash]).await.unwrap();
    for value in ["Bearer synthetic-continue-api", "Nostr synthetic-invalid", ""] {
        f.send(f.request(&path).header(header::AUTHORIZATION, value)).await.refused(403);
    }
    let b_hash = Sha256::digest(b"synthetic-continue-account-b").to_vec();
    f.sql("INSERT INTO platform.users(id,username,email,password_hash,verified) VALUES('NCAuthorityB','continue-authority-b','continue-authority-b@fixture.invalid','synthetic',true)").await;
    f.state.db.client().await.unwrap().execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES('NCAuthorityBSession','NCAuthorityB',$1,clock_timestamp()+interval '1 hour')", &[&b_hash]).await.unwrap();
    let b_cookie = format!("{}=synthetic-continue-account-b", f.state.config.session_cookie_name);
    f.send(f.bare(Method::GET, &path).header(header::COOKIE, &b_cookie).header(EXPECTED_ACCOUNT, &f.user)).await.refused(409);
    let switched = f.send(f.bare(Method::GET, &path).header(header::COOKIE, &b_cookie).header(EXPECTED_ACCOUNT, "NCAuthorityB")).await;
    switched.private(); assert_eq!(switched.status, 200);
    assert_eq!(switched.value()["winner"]["id"], "NCAuthorityOld");
    assert_eq!(switched.value()["account"]["id"], "NCAuthorityB");
    for (change, restore, status) in [
        ("UPDATE platform.sessions SET purpose='recovery' WHERE id='NCAuthoritySession'", "UPDATE platform.sessions SET purpose='normal' WHERE id='NCAuthoritySession'", 403),
        ("UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE id='NCAuthoritySession'", "UPDATE platform.sessions SET revoked_at=NULL WHERE id='NCAuthoritySession'", 401),
        ("UPDATE platform.sessions SET created_at=clock_timestamp()-interval '2 hours',expires_at=clock_timestamp()-interval '1 minute' WHERE id='NCAuthoritySession'", "UPDATE platform.sessions SET expires_at=clock_timestamp()+interval '1 hour' WHERE id='NCAuthoritySession'", 401),
        ("UPDATE platform.users SET disabled=true WHERE id='NCAuthorityUser'", "UPDATE platform.users SET disabled=false WHERE id='NCAuthorityUser'", 401),
    ] {
        f.sql(change).await; f.get(&path).await.refused(status); f.sql(restore).await;
    }
    f.sql("UPDATE platform.users SET verified=false,roles='[\"user\",\"banned\",\"restricted\"]',permissions='[]' WHERE id='NCAuthorityUser'").await;
    f.success(&f.get(&path).await, manga, Some("NCAuthorityNew"), "oldest-unread", false);
    for key in ["accountEpoch", "catalogNamespace", "serviceKey", "requestId"] {
        f.get(&replace(&path, key, None)).await.refused(400);
    }
    for pair in ["history=old", "completed=true", "sort=id", "direction=oldest", "current=x", "groupChapter=x", "cursor=x", "after=x", "offset=0", "limit=1", "page=1", "accountEpoch=7", "requestId=again", "id=%FF", "id=%Q1", "id=%00", ""] {
        f.get(&format!("{path}&{pair}")).await.refused(400);
    }
    for (key, value) in [("requestId", ""), ("requestId", "a/b"), ("accountEpoch", "01"), ("accountEpoch", "9007199254740992"), ("serviceKey", "NULL")] {
        f.get(&replace(&path, key, Some(value))).await.refused(400);
    }
    f.get(&replace(&path, "requestId", Some(&"a".repeat(65)))).await.refused(400);
    f.get(&format!("{path}&unknown={}", "x".repeat(QUERY_LIMIT))).await.refused(400);
    let other = if f.namespace == "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa" { "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb" } else { "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa" };
    f.get(&replace(&path, "catalogNamespace", Some(other))).await.refused(409);
    f.get(&replace(&path, "serviceKey", Some(&"a".repeat(64)))).await.refused(409);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Source-authored only; isolated PostgreSQL runner required"]
async fn pending_authority_writers_conflict_and_unrelated_sessions_do_not() {
    let f = Fixture::new("Locks").await;
    let manga = "NCLocksManga";
    f.manga(manga).await;
    f.chapter("NCLocksChapter", manga, None, None).await;
    let path = f.path(manga);
    for statement in [
        "UPDATE platform.users SET disabled=true WHERE id='NCLocksUser'",
        "UPDATE platform.users SET roles='[\"restricted\"]' WHERE id='NCLocksUser'",
        "UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE id='NCLocksSession'",
        "UPDATE platform.sessions SET purpose='recovery' WHERE id='NCLocksSession'",
    ] {
        let mut connection = f.state.db.client().await.unwrap();
        let pending = connection.transaction().await.unwrap();
        pending.batch_execute(statement).await.unwrap();
        let response = tokio::time::timeout(Duration::from_secs(2), f.get(&path)).await.unwrap();
        response.refused(409);
        pending.rollback().await.unwrap();
        f.success(&f.get(&path).await, manga, Some("NCLocksChapter"), "oldest-unread", false);
    }
    let hash = Sha256::digest(b"synthetic-continue-unrelated").to_vec();
    f.state.db.client().await.unwrap().execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES('NCLocksUnrelated',$1,$2,clock_timestamp()+interval '1 hour')", &[&f.user,&hash]).await.unwrap();
    let mut connection = f.state.db.client().await.unwrap();
    let pending = connection.transaction().await.unwrap();
    pending.batch_execute("UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE id='NCLocksUnrelated'").await.unwrap();
    f.success(&f.get(&path).await, manga, Some("NCLocksChapter"), "oldest-unread", false);
    pending.rollback().await.unwrap();
}

// These causal lock controls use real PostgreSQL blocking relationships, not a
// sleep claimed as proof of a transaction boundary. Run serially: the gates
// briefly lock shared fixture tables. Production's 250ms lock budget is NOT
// relaxed; a runner too slow to release a gate within that budget fails closed.
async fn waiting_reader(db: &tokio_postgres::Client, blocker: i32, query: &str) -> i32 {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(row) = db.query_opt("SELECT pid FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND wait_event_type='Lock' AND $1=ANY(pg_blocking_pids(pid)) AND query LIKE $2", &[&blocker,&query]).await.unwrap() { return row.get(0); }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }).await.expect("Observe the actual blocked statement before proceeding")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Source-authored only; isolated PostgreSQL runner, --test-threads=1 required"]
async fn reader_first_holds_user_and_exact_session_share_locks_until_snapshot_commit() {
    let f = Fixture::new("ReaderFirst").await;
    let manga = "NCReaderFirstManga";
    f.manga(manga).await;
    f.chapter("NCReaderFirstChapter", manga, None, None).await;
    let path = f.path(manga);
    for (table, key) in [("platform.users", f.user.as_str()), ("platform.sessions", f.session.as_str())] {
        let mut gate = f.state.db.client().await.unwrap();
        let gate_pid: i32 = gate.query_one("SELECT pg_backend_pid()", &[]).await.unwrap().get(0);
        let gate_tx = gate.transaction().await.unwrap();
        gate_tx.batch_execute("LOCK TABLE platform.catalogue_namespace IN ACCESS EXCLUSIVE MODE").await.unwrap();
        let request = f.request(&path);
        let reader = tokio::spawn(async move { request.send().await.unwrap() });
        let mut observer = f.state.db.client().await.unwrap();
        let reader_pid = waiting_reader(&observer, gate_pid, "%SELECT namespace::text FROM platform.catalogue_namespace%").await;
        // SHARE, unlike KEY SHARE, conflicts even with a non-key UPDATE lock.
        let probe = observer.transaction().await.unwrap();
        let conflict = probe.query_opt(&format!("SELECT id FROM {table} WHERE id=$1 FOR NO KEY UPDATE NOWAIT"), &[&key]).await.unwrap_err();
        assert_eq!(conflict.code().map(|c| c.code()), Some("55P03"));
        probe.rollback().await.unwrap();
        let still_waiting: bool = observer.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1 AND $2=ANY(pg_blocking_pids(pid)))", &[&reader_pid,&gate_pid]).await.unwrap().get(0);
        assert!(still_waiting, "Authority lock observation must precede the reader's catalogue gate release");
        gate_tx.commit().await.unwrap();
        let response = reader.await.unwrap();
        let wire = Wire { status:response.status().as_u16(), headers:response.headers().clone(), body:response.bytes().await.unwrap().to_vec() };
        f.success(&wire, manga, Some("NCReaderFirstChapter"), "oldest-unread", false);
        let probe = observer.transaction().await.unwrap();
        assert!(probe.query_opt(&format!("SELECT id FROM {table} WHERE id=$1 FOR NO KEY UPDATE NOWAIT"), &[&key]).await.unwrap().is_some());
        probe.rollback().await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Source-authored only; isolated PostgreSQL runner, --test-threads=1 required"]
async fn namespace_and_history_drift_share_one_snapshot_and_changed_authority_maps_serialization_conflict() {
    let f = Fixture::new("Snapshot").await;
    let manga = "NCSnapshotManga";
    f.manga(manga).await;
    f.chapter("NCSnapshotOld", manga, Some("1"), Some("1")).await;
    f.chapter("NCSnapshotNew", manga, Some("1"), Some("2")).await;
    let path = f.path(manga);
    let replacement = if f.namespace == "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa" { "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb" } else { "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa" };
    let mut gate = f.state.db.client().await.unwrap();
    let gate_pid: i32 = gate.query_one("SELECT pg_backend_pid()", &[]).await.unwrap().get(0);
    let gate_tx = gate.transaction().await.unwrap();
    gate_tx.batch_execute("LOCK TABLE platform.catalogue_namespace IN ACCESS EXCLUSIVE MODE").await.unwrap();
    gate_tx.execute("UPDATE platform.catalogue_namespace SET namespace=$1::text::uuid WHERE singleton", &[&replacement]).await.unwrap();
    // INSERT history's FK locks are compatible with the reader's SHARE locks.
    gate_tx.execute("INSERT INTO platform.library_chapter_history(user_id,manga_id,chapter_id) VALUES($1,$2,'NCSnapshotOld')", &[&f.user,&manga]).await.unwrap();
    let request = f.request(&path);
    let reader = tokio::spawn(async move { request.send().await.unwrap() });
    let observer = f.state.db.client().await.unwrap();
    waiting_reader(&observer, gate_pid, "%SELECT namespace::text FROM platform.catalogue_namespace%").await;
    gate_tx.commit().await.unwrap();
    let response = reader.await.unwrap();
    let wire = Wire { status:response.status().as_u16(), headers:response.headers().clone(), body:response.bytes().await.unwrap().to_vec() };
    f.success(&wire, manga, Some("NCSnapshotOld"), "oldest-unread", false);
    drop(observer);
    f.get(&path).await.refused(409);
    let latest = f.get(&replace(&path, "catalogNamespace", Some(replacement))).await;
    latest.private(); assert_eq!(latest.status, 200);
    assert_eq!(latest.value()["winner"]["id"], "NCSnapshotNew");
    // Restore namespace before the separate authority race and other tests.
    gate.execute("UPDATE platform.catalogue_namespace SET namespace=$1::text::uuid WHERE singleton", &[&f.namespace]).await.unwrap();
    let gate_tx = gate.transaction().await.unwrap();
    gate_tx.batch_execute("LOCK TABLE platform.users IN ACCESS EXCLUSIVE MODE").await.unwrap();
    gate_tx.execute("UPDATE platform.users SET roles='[\"restricted\"]' WHERE id=$1", &[&f.user]).await.unwrap();
    let request = f.request(&path);
    let reader = tokio::spawn(async move { request.send().await.unwrap() });
    let observer = f.state.db.client().await.unwrap();
    waiting_reader(&observer, gate_pid, "%SELECT id,username,email,verified,roles,permissions FROM platform.users%").await;
    gate_tx.commit().await.unwrap();
    // Candidate lookup established the snapshot before user-row commit. FOR
    // SHARE now sees a concurrent row version: actual 40001 maps to HTTP 409.
    let response = reader.await.unwrap();
    Wire { status:response.status().as_u16(), headers:response.headers().clone(), body:response.bytes().await.unwrap().to_vec() }.refused(409);
    drop(observer);
    f.success(&f.get(&path).await, manga, Some("NCSnapshotNew"), "oldest-unread", false);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn route_local_get_preflight_is_credentialed_but_grants_no_other_header_origin_route_or_method() {
    let f = Fixture::new("Cors").await;
    let manga = "NCCorsManga";
    f.manga(manga).await;
    f.chapter("NCCorsChapter", manga, None, None).await;
    let path = f.path(manga);
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
    f.success(&actual, manga, Some("NCCorsChapter"), "oldest-unread", false);
    assert_eq!(actual.headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(), f.origin.as_str());
    for method in ["HEAD", "POST", "PUT", "PATCH", "DELETE"] {
        f.send(preflight(&path, &f.origin, method, EXPECTED_ACCOUNT)).await.refused(403);
    }
    for headers in ["X-WeebDex-Expected-Account, X-Private-Override", "X-Private-Override"] {
        f.send(preflight(&path, &f.origin, "GET", headers)).await.refused(403);
    }
    f.send(preflight(&path, "https://not-approved.fixture.invalid", "GET", EXPECTED_ACCOUNT)).await.refused(403);
    f.send(f.request(&path).header(header::ORIGIN, "https://not-approved.fixture.invalid")).await.refused(403);
    for other in ["/api/user/history/manga", "/api/manga/NCCorsManga/reader", "/api/v1/user/history/manga/NCCorsManga/continue"] {
        let refused = f.send(preflight(other, &f.origin, "GET", EXPECTED_ACCOUNT)).await;
        assert_eq!(refused.status, 403);
        assert!(!refused.headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
        assert!(!refused.headers.contains_key(header::ACCESS_CONTROL_ALLOW_HEADERS));
    }
    let public = f.send(f.bare(Method::GET, &path.replacen("/api/", "/api/v1/", 1))
        .header(header::COOKIE, &f.cookie).header(EXPECTED_ACCOUNT, &f.user)).await;
    assert_eq!(public.status, 404, "There is deliberately no public-ID continue endpoint");
    // Retained HTTP HEAD dispatch executes GET admission but returns no entity.
    let head = f.send(f.bare(Method::HEAD, &path).header(header::COOKIE, &f.cookie).header(EXPECTED_ACCOUNT, &f.user)).await;
    head.private();
    assert_eq!(head.status, 200);
    assert!(head.body.is_empty());
    let bad_head = f.send(f.bare(Method::HEAD, &path).header(header::COOKIE, &f.cookie).header(EXPECTED_ACCOUNT, "NCCorsDifferent")).await;
    bad_head.private();
    assert_eq!(bad_head.status, 409);
    assert!(bad_head.body.is_empty());
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
    let manga = "NCIntakeRouterManga";
    let chapter = "NCIntakeRouterChapter";
    f.manga(manga).await;
    f.chapter(chapter, manga, None, None).await;
    let path = f.path(manga);
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
            .await.expect("Ordinary empty GET must reach the real continue query").unwrap();
        let wire = router_wire(response).await;
        if method == "GET" {
            f.success(&wire, manga, Some(chapter), "oldest-unread", false);
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
    let manga = "NCIntakeWireManga";
    let chapter = "NCIntakeWireChapter";
    f.manga(manga).await;
    f.chapter(chapter, manga, None, None).await;
    let path = f.path(manga);
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
    f.success(&f.get(&path).await, manga, Some(chapter), "oldest-unread", false);
    // HTTP/2 end-stream behavior still needs transport qualification. The
    // full-router no-frame body above is the causal stream-contract control,
    // not a claim that this HTTP/1.1 fixture executed an HTTP/2 connection.
}


#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Source-authored only; isolated PostgreSQL runner required"]
async fn visible_group_identity_budget_refuses_instead_of_truncating_and_pool_wait_is_bounded() {
    let f = Fixture::new("Budget").await;
    let manga = "NCBudgetManga";
    f.manga(manga).await;
    f.chapter("NCBudgetChapter", manga, None, None).await;
    let db = f.state.db.client().await.unwrap();
    db.batch_execute("INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) SELECT 'NCBudget'||lpad(n::text,3,'0')||repeat('g',117),'NCBudgetUser','Continue fixture','Group' FROM generate_series(1,101)n; INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) SELECT 'NCBudgetChapter',id FROM platform.catalog_groups WHERE owner_id='NCBudgetUser'").await.unwrap();
    drop(db);
    let path = f.path(manga);
    f.get(&path).await.refused(503);
    f.sql("UPDATE platform.catalog_groups SET deleted_at=clock_timestamp() WHERE id='NCBudget101'||repeat('g',117)").await;
    let response = f.get(&path).await;
    let value = f.success(&response, manga, Some("NCBudgetChapter"), "oldest-unread", false);
    assert_eq!(value["winner"]["groupIds"].as_array().unwrap().len(), 100);
    assert!(response.body.len() > 100*128, "Count complete envelope and winner, not only identities");
    let mut occupied = Vec::new();
    for _ in 0..f.state.config.database_connections { occupied.push(f.state.db.client().await.unwrap()); }
    let start = Instant::now();
    tokio::time::timeout(Duration::from_secs(10), f.get(&path)).await.unwrap().refused(503);
    assert!(start.elapsed() < Duration::from_secs(10), "Pool acquisition is inside operation deadline");
    drop(occupied);
    f.success(&f.get(&path).await, manga, Some("NCBudgetChapter"), "oldest-unread", false);
    // Admission refusals happen before the library handler and must still be private.
    let request_permits = f.state.requests.clone().acquire_many_owned(f.state.requests.available_permits() as u32).await.unwrap();
    f.get(&path).await.refused(429);
    drop(request_permits);
    let mut read_permits = Vec::new();
    for _ in 0..f.state.config.admission_config().library_head.active {
        read_permits.push(f.state.reads.acquire(manga_node::read_admission::Class::LibraryHead).await.unwrap());
    }
    f.get(&path).await.refused(429);
    drop(read_permits);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Source-authored only; isolated PostgreSQL runner, --test-threads=1 required"]
async fn real_protocol_source_withdrawal_removes_alternate_completion_without_erasing_history() {
    let f = Fixture::new("Withdrawal").await;
    let manga = "NCWithdrawalManga";
    f.manga(manga).await;
    f.chapter("NCWithdrawalNative", manga, Some("1"), Some("1")).await;
    f.chapter("NCWithdrawalSource", manga, Some("1"), Some("1")).await;
    f.chapter("NCWithdrawalNewest", manga, Some("1"), Some("2")).await;
    let mut repository = manga_node::db::Repository::connect(manga_node::db::DbConfig {
        dsn:std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").unwrap(), connections:1,
        acquisition_timeout:Duration::from_secs(2), cache_weight:1024*1024, plaintext_local:true,
    }).await.unwrap();
    repository.pool = f.state.db.pool().clone();
    repository.migrate().await.unwrap();
    f.state.protocol.verify_schema().await.unwrap();
    f.state.protocol.install_visibility_function().await.unwrap();
    // Synthetic SQL ledger rows, not fabricated signature/publication evidence.
    // They exercise the REAL installed source policy and its state transition.
    f.sql("INSERT INTO events(id,publisher,kind,created_at,raw_event,state) VALUES(decode(repeat('ed',32),'hex'),decode(repeat('ea',32),'hex'),1002,1,'{}','hosted'); UPDATE platform.catalog_chapters SET protocol_event_id=repeat('ed',32) WHERE id='NCWithdrawalSource'").await;
    f.mark(manga, "NCWithdrawalSource").await;
    let path = f.path(manga);
    f.success(&f.get(&path).await, manga, Some("NCWithdrawalNewest"), "oldest-unread", false);
    for state in ["withdrawing", "removed"] {
        f.state.db.client().await.unwrap().execute("UPDATE events SET state=$1 WHERE id=decode(repeat('ed',32),'hex')", &[&state]).await.unwrap();
        f.success(&f.get(&path).await, manga, Some("NCWithdrawalNative"), "oldest-unread", false);
        assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.library_chapter_history WHERE user_id=$1 AND chapter_id='NCWithdrawalSource'", &[&f.user]).await.unwrap().get::<_,i64>(0), 1);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Source-authored only; isolated PostgreSQL runner required"]
async fn configured_service_capture_matches_capabilities_without_starting_publication() {
    use weebdex_platform::protocol::publication::OperatorRuntimeConfig;
    let f = Fixture::new("Service").await;
    let manga = "NCServiceManga";
    f.manga(manga).await;
    f.chapter("NCServiceChapter", manga, None, None).await;
    let key = "b".repeat(64);
    let mut config = f.state.config.as_ref().clone();
    // Public fixture identity only. No signer file is read/generated and no
    // native service is started; this is the same immutable capability input.
    config.protocol_publication = Some(OperatorRuntimeConfig {
        operator_key_file:"fixture-unused/operator.key".into(), expected_publisher:"a".repeat(64),
        storage_owner:"a".repeat(64), storage_holder:key.clone(),
        storage:json!({"path":"fixture-unused/storage","volume":"continuefixture","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144}),
        group_keys_dir:None, backend:None,
        native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":"fixture-unused/service.key","admin_pubkey":"a".repeat(64)})),
    });
    config.validate().unwrap();
    let mut state = f.state.clone(); state.config = Arc::new(config);
    let app = weebdex_platform::router::router(state);
    let capabilities = router_wire(app.clone().oneshot(axum::http::Request::builder().uri("/api/protocol/capabilities").body(axum::body::Body::empty()).unwrap()).await.unwrap()).await.value();
    assert_eq!(capabilities["service_key"], key);
    for (captured, status) in [(&key,200), (&"null".to_owned(),409), (&"c".repeat(64),409)] {
        let path = replace(&f.path(manga), "serviceKey", Some(captured));
        let response = app.clone().oneshot(axum::http::Request::builder().uri(path)
            .header(header::COOKIE, f.cookie.as_str()).header(EXPECTED_ACCOUNT, f.user.as_str())
            .body(axum::body::Body::empty()).unwrap()).await.unwrap();
        let wire = router_wire(response).await;
        if status == 200 {
            wire.private(); assert_eq!(wire.status,200);
            assert_eq!(wire.value()["catalogIdentity"]["serviceKey"], capabilities["service_key"]);
            assert_eq!(wire.value()["winner"]["id"], "NCServiceChapter");
        } else { wire.refused(status); }
    }
}
