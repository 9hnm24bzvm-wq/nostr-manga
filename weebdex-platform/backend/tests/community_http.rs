//! All25 original community/report/analytics operations through the public Router
//! and an owned disposable PostgreSQL database. Never uses production credentials.
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use tower::ServiceExt;
use weebdex_platform::{AppState, Config};
const ORIGIN: &str = "https://community-test.example.invalid";
const OWNER: &str = "communityowner01";
const OTHER: &str = "communityother01";
const MOD: &str = "communitymod01";
const LIMITED: &str = "communitylimited01";
const STAFF: &str = "communitystaff01";
const MANGA: &str = "communitymanga01";
const CHAPTER: &str = "communitychapter01";
const GROUP: &str = "communitygroup01";
const THREAD: &str = "communitythread01";
const OTHER_THREAD: &str = "communitythread02";
#[derive(Default)]
struct Browser {
    cookies: BTreeMap<String, String>,
}
impl Browser {
    fn session(token: &str) -> Self {
        Self {
            cookies: BTreeMap::from([("weebdex_session".into(), token.into())]),
        }
    }
    async fn request(
        &mut self,
        app: &Router,
        method: Method,
        path: &str,
        p: Option<Value>,
        origin: Option<&str>,
    ) -> (StatusCode, Value) {
        let mut r = Request::builder().method(method).uri(path);
        if !self.cookies.is_empty() {
            r = r.header(
                header::COOKIE,
                self.cookies
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join("; "),
            );
            if path.starts_with("/api/staff/") {
                if let Some(token)=self.cookies.get("weebdex_session") {
                    if let Some(user)=token.strip_prefix("isolated-").and_then(|s|s.strip_suffix("-token")) {
                        r=r.header("x-weebdex-expected-account",user);
                    }
                }
            }
        }
        if let Some(o) = origin {
            r = r.header(header::ORIGIN, o);
        }
        let body = if let Some(p) = p {
            r = r.header(header::CONTENT_TYPE, "application/json");
            Body::from(p.to_string())
        } else {
            Body::empty()
        };
        let response = app.clone().oneshot(r.body(body).unwrap()).await.unwrap();
        assert!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .is_some_and(|v| v.to_str().unwrap().contains("no-store")),
            "cache guard {path}"
        );
        for c in response.headers().get_all(header::SET_COOKIE) {
            let c = c.to_str().unwrap();
            assert!(c.contains("HttpOnly") && c.contains("SameSite=Lax") && c.contains("Secure"));
            let pair = c.split(';').next().unwrap();
            let (k, v) = pair.split_once('=').unwrap();
            self.cookies.insert(k.into(), v.into());
        }
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 16 * 1024 * 1024)
            .await
            .unwrap();
        (
            status,
            if bytes.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&bytes).unwrap()
            },
        )
    }
    async fn expect(
        &mut self,
        app: &Router,
        method: Method,
        path: &str,
        p: Option<Value>,
        expected: StatusCode,
    ) -> Value {
        let (s, v) = self.request(app, method, path, p, Some(ORIGIN)).await;
        assert_eq!(s, expected, "{path}: {v}");
        v
    }
}
async fn state() -> AppState {
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL")
        .expect("Only scripts/run-auth-tests.sh isolated cluster is accepted");
    let pg: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
    let runtime = std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap();
    assert!(
        runtime.contains("/weebdex-platform/test-runtime/")
            && std::path::Path::new(&runtime)
                .join("fixture.json")
                .is_file()
    );
    assert!(!pg.get_hosts().is_empty());
    for h in pg.get_hosts() {
        match h {
            tokio_postgres::config::Host::Tcp(h) => assert_eq!(h, "127.0.0.1"),
            _ => panic!("Only disposable private loopback listener allowed"),
        }
    }
    assert!(pg.get_ports().iter().all(|p| *p > 1024 && *p != 5432));
    let s = AppState::connect(
        Config {
            public_origin: ORIGIN.into(),
            database_plaintext_local: true,
            database_connections: 4,
            ..Config::default()
        },
        dsn,
    )
    .await
    .unwrap();
    s.db.migrate(weebdex_platform::domains::MIGRATIONS)
        .await
        .unwrap();
    s
}
async fn seed(s: &AppState) {
    let db = s.db.client().await.unwrap();
    let normal = json!([
        "reply.create",
        "self.reply.update",
        "self.reply.remove",
        "self.scanlation_group.update",
        "report.create"
    ]);
    let moderator = json!([
        "reply.create",
        "reply.update",
        "reply.remove",
        "reply.delete",
        "report.create",
        "report.update",
        "report.delete",
        "scanlation_group.update"
    ]);
    for (id, name, role, perms, verified) in [
        (
            OWNER,
            "community-fixture-owner",
            "user",
            normal.clone(),
            true,
        ),
        (
            OTHER,
            "community-fixture-other",
            "user",
            normal.clone(),
            true,
        ),
        (
            MOD,
            "community-fixture-moderator",
            "moderator",
            moderator,
            true,
        ),
        (
            LIMITED,
            "community-fixture-limited",
            "moderator",
            json!([]),
            true,
        ),
        (
            STAFF,
            "community-fixture-staff",
            "staff",
            json!([
                "announcement.create",
                "announcement.update",
                "announcement.delete"
            ]),
            true,
        ),
    ] {
        let roles = json!([role]);
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,roles,permissions) VALUES($1,$2,$3,'not-a-login-password-hash',$4,$5,$6)",&[&id,&name,&format!("{id}@example.invalid"),&verified,&roles,&perms]).await.unwrap();
        let hash = Sha256::digest(format!("isolated-{id}-token").as_bytes()).to_vec();
        db.execute("INSERT INTO platform.sessions(id,token_hash,user_id,expires_at) VALUES($1,$2,$3,clock_timestamp()+interval '1 hour')",&[&format!("session_{id}"),&hash,&id]).await.unwrap();
    }
    db.execute("INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES($1,$2,'community-fixture-owner','Community fixture group')",&[&GROUP,&OWNER]).await.unwrap();
    db.execute("INSERT INTO platform.catalog_authors(id,owner_id,owner_name,name) VALUES('communityauthor01',$1,'community-fixture-owner','Community fixture author')",&[&OWNER]).await.unwrap();
    for (id, title, state) in [
        (MANGA, "Community fixture manga", "published"),
        ("communitymanga02", "Second community manga", "published"),
        ("communitydraft01", "Private draft", "draft"),
    ] {
        db.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES($1,$2,'community-fixture-owner',$3,'ja','ongoing','safe',$4)",&[&id,&OWNER,&title,&state]).await.unwrap();
    }
    db.execute("INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,language,metadata) VALUES($1,$2,$3,'community-fixture-owner','en',$4)",&[&CHAPTER,&MANGA,&OWNER,&json!({"node":"https://media.example.invalid","data":[{"name":"p1.png","dimensions":[10,10]}]})]).await.unwrap();
    db.execute(
        "INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES($1,$2)",
        &[&CHAPTER, &GROUP],
    )
    .await
    .unwrap();
    for (id, manga) in [
        (THREAD, MANGA),
        (OTHER_THREAD, "communitymanga02"),
        ("communitydraftthread", "communitydraft01"),
    ] {
        db.execute("INSERT INTO platform.catalog_threads(id,entity_type,entity_id,author_id,author_name,subject) VALUES($1,'manga',$2,$3,'community-fixture-owner','Fixture discussion')",&[&id,&manga,&OWNER]).await.unwrap();
    }
}
// Real public-router/PG seam: a configured proxy must not collapse all clients
// into its120/min bucket, and a direct caller cannot rotate forged XFF values.
async fn trusted_proxy_analytics_seam(s: &AppState, visitor: &str) {
    fn peer_hash(value: &str) -> Vec<u8> {
        Sha256::digest(format!("weebdex-local-peer:{value}").as_bytes()).to_vec()
    }
    let mut config = (*s.config).clone();
    config.trusted_proxy_peers = vec!["127.0.0.1".parse().unwrap()];
    config.trusted_proxy_hops = 1;
    let mut proxy_state = s.clone();
    proxy_state.config = std::sync::Arc::new(config);
    let app = weebdex_platform::router::router(proxy_state);
    let exhausted = peer_hash("198.51.100.4");
    let direct = peer_hash("192.0.2.8");
    let independent = peer_hash("203.0.113.9");
    {
        let db = s.db.client().await.unwrap();
        for key in [&exhausted, &direct] {
            db.execute("INSERT INTO platform.analytics_rate_limits(peer_hash,bucket,count) VALUES($1,floor(extract(epoch FROM clock_timestamp())/60)::BIGINT,120) ON CONFLICT(peer_hash,bucket) DO UPDATE SET count=120",&[key]).await.unwrap();
        }
    }
    async fn call(app: &Router, actual: &str, headers: &[&str], visitor: &str) -> StatusCode {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/analytics/manga/{MANGA}/views"))
            .header(header::ORIGIN, ORIGIN)
            .header(header::COOKIE, format!("weebdex_visitor={visitor}"))
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(axum::extract::ConnectInfo(
            actual.parse::<std::net::SocketAddr>().unwrap(),
        ));
        for value in headers {
            request
                .headers_mut()
                .append("x-forwarded-for", value.parse().unwrap());
        }
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        response.status()
    }
    assert_eq!(
        call(&app, "127.0.0.1:4400", &["198.51.100.4"], visitor).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        call(&app, "127.0.0.1:4400", &["203.0.113.9"], visitor).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app, "192.0.2.8:4400", &["203.0.113.9"], visitor).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        call(&app, "192.0.2.8:4400", &["203.0.113.10"], visitor).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        call(&app, "127.0.0.1:4400", &["unknown"], visitor).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &app,
            "127.0.0.1:4400",
            &["203.0.113.9", "198.51.100.4"],
            visitor
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    let count:i32=s.db.client().await.unwrap().query_one("SELECT count FROM platform.analytics_rate_limits WHERE peer_hash=$1 AND bucket=floor(extract(epoch FROM clock_timestamp())/60)::BIGINT",&[&independent]).await.unwrap().get(0);
    assert_eq!(count, 1);
}

#[tokio::test]
#[ignore = "Owned disposable PostgreSQL required; run scripts/run-auth-tests.sh cargo test --test community_http -- --ignored --test-threads=1"]
async fn all25_original_operations_persist_authorize_page_moderate_and_count_actual_activity() {
    let s = state().await;
    seed(&s).await;
    let app = weebdex_platform::router::router(s.clone());
    let mut owner = Browser::session(&format!("isolated-{OWNER}-token"));
    let mut other = Browser::session(&format!("isolated-{OTHER}-token"));
    let mut moderator = Browser::session(&format!("isolated-{MOD}-token"));
    let mut limited = Browser::session(&format!("isolated-{LIMITED}-token"));
    let mut anon = Browser::default();
    let mut staff = Browser::session(&format!("isolated-{STAFF}-token"));
    let thread = anon
        .expect(
            &app,
            Method::GET,
            &format!("/api/thread/{THREAD}"),
            None,
            StatusCode::OK,
        )
        .await;
    assert_eq!(thread["relationships"]["author"]["id"], OWNER);
    assert_eq!(thread["relationships"]["manga"]["id"], MANGA);
    assert_eq!(thread["relationships"]["stats"]["replies"], 0);
    assert!(thread["created_at"].as_str().unwrap().ends_with('Z'));
    anon.expect(
        &app,
        Method::GET,
        "/api/thread/communitydraftthread",
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    let base = format!("/api/thread/{THREAD}/replies");
    anon.expect(
        &app,
        Method::POST,
        &base,
        Some(json!({"body":"anonymous"})),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let (status, _) = owner
        .request(
            &app,
            Method::POST,
            &base,
            Some(json!({"body":"csrf"})),
            Some("https://evil.example.invalid"),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let reply = owner
        .expect(
            &app,
            Method::POST,
            &base,
            Some(json!({"body":"日本語 **persisted comment**"})),
            StatusCode::CREATED,
        )
        .await;
    let id = reply["id"].as_i64().unwrap();
    assert_eq!(reply["reply_number"], 1);
    assert_eq!(reply["version"], 1);
    let path = format!("{base}/{id}");
    let reply2 = other
        .expect(
            &app,
            Method::POST,
            &base,
            Some(json!({"body":"Second reply","parent_id":id})),
            StatusCode::CREATED,
        )
        .await;
    let id2 = reply2["id"].as_i64().unwrap();
    let path2 = format!("{base}/{id2}");
    owner
        .expect(
            &app,
            Method::POST,
            &format!("/api/thread/{OTHER_THREAD}/replies"),
            Some(json!({"body":"cross-thread parent","parent_id":id})),
            StatusCode::BAD_REQUEST,
        )
        .await;
    owner
        .expect(
            &app,
            Method::GET,
            &format!("/api/thread/{OTHER_THREAD}/replies/{id}"),
            None,
            StatusCode::NOT_FOUND,
        )
        .await;
    let page = owner
        .expect(
            &app,
            Method::GET,
            &format!("{base}?limit=1&page=2&order=asc"),
            None,
            StatusCode::OK,
        )
        .await;
    assert_eq!(page["total"], 2);
    assert_eq!(page["data"][0]["id"], id2);
    assert_eq!(
        owner
            .expect(&app, Method::GET, &path, None, StatusCode::OK)
            .await["body"],
        reply["body"]
    );
    other
        .expect(
            &app,
            Method::PUT,
            &path,
            Some(json!({"body":"unauthorized","version":1})),
            StatusCode::FORBIDDEN,
        )
        .await;
    let edited = owner
        .expect(
            &app,
            Method::PUT,
            &path,
            Some(json!({"body":"edited","version":1})),
            StatusCode::OK,
        )
        .await;
    assert_eq!(edited["version"], 2);
    owner
        .expect(
            &app,
            Method::PUT,
            &path,
            Some(json!({"body":"stale","version":1})),
            StatusCode::CONFLICT,
        )
        .await;
    owner
        .expect(
            &app,
            Method::POST,
            &format!("{path}/vote"),
            Some(json!({"vote":1})),
            StatusCode::FORBIDDEN,
        )
        .await;
    other
        .expect(
            &app,
            Method::POST,
            &format!("{path}/vote"),
            Some(json!({"vote":0})),
            StatusCode::BAD_REQUEST,
        )
        .await;
    for _ in 0..2 {
        other
            .expect(
                &app,
                Method::POST,
                &format!("{path}/vote"),
                Some(json!({"vote":1})),
                StatusCode::NO_CONTENT,
            )
            .await;
    }
    let voted = other
        .expect(&app, Method::GET, &path, None, StatusCode::OK)
        .await;
    assert_eq!(voted["relationships"]["stats"]["up"], 1);
    assert_eq!(voted["relationships"]["vote_state"], 1);
    other
        .expect(
            &app,
            Method::POST,
            &format!("{path}/vote"),
            Some(json!({"vote":-1})),
            StatusCode::NO_CONTENT,
        )
        .await;
    for _ in 0..2 {
        other
            .expect(
                &app,
                Method::DELETE,
                &format!("{path}/vote"),
                None,
                StatusCode::NO_CONTENT,
            )
            .await;
    }
    let tp = format!("/api/thread/{THREAD}/vote");
    assert_eq!(
        other
            .expect(&app, Method::GET, &tp, None, StatusCode::OK)
            .await["vote"],
        0
    );
    for _ in 0..2 {
        other
            .expect(
                &app,
                Method::POST,
                &tp,
                Some(json!({"vote":1})),
                StatusCode::NO_CONTENT,
            )
            .await;
    }
    assert_eq!(
        other
            .expect(&app, Method::GET, &tp, None, StatusCode::OK)
            .await["vote"],
        1
    );
    for _ in 0..2 {
        other
            .expect(&app, Method::DELETE, &tp, None, StatusCode::NO_CONTENT)
            .await;
    }
    // Source enum reasons are dynamic instance policy. All categories have actual persisted reasons.
    for category in [
        "user",
        "manga",
        "author",
        "chapter",
        "scanlation_group",
        "thread",
        "reply",
    ] {
        let reasons = anon
            .expect(
                &app,
                Method::GET,
                &format!("/api/report/reasons/{category}"),
                None,
                StatusCode::OK,
            )
            .await;
        assert!(!reasons.as_array().unwrap().is_empty());
        assert_eq!(reasons[0]["category"], category);
    }
    anon.expect(
        &app,
        Method::GET,
        "/api/report",
        None,
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let report = json!({"entity_type":"reply","entity_id":id.to_string(),"reason_id":"reply_spam","details":"Fixture report"});
    owner.expect(&app,Method::POST,"/api/report",Some(json!({"entity_type":"reply","entity_id":id.to_string(),"reason_id":"manga_spam","details":"wrong category"})),StatusCode::BAD_REQUEST).await;
    owner
        .expect(
            &app,
            Method::POST,
            "/api/report",
            Some(report.clone()),
            StatusCode::NO_CONTENT,
        )
        .await;
    owner
        .expect(
            &app,
            Method::POST,
            "/api/report",
            Some(report),
            StatusCode::CONFLICT,
        )
        .await;
    other.expect(&app,Method::POST,"/api/report",Some(json!({"entity_type":"manga","entity_id":MANGA,"reason_id":"manga_other","details":"Other user's report"})),StatusCode::NO_CONTENT).await;
    let own = owner
        .expect(
            &app,
            Method::GET,
            "/api/report?category=reply&status=waiting&limit=1",
            None,
            StatusCode::OK,
        )
        .await;
    assert_eq!(own["total"], 1);
    assert_eq!(own["data"][0]["relationships"]["submitter"]["id"], OWNER);
    assert_eq!(own["data"][0]["relationships"]["reply"]["id"], id);
    for status in ["accepted", "refused", "autoresolved"] {
        let filtered = owner
            .expect(
                &app,
                Method::GET,
                &format!("/api/report?category=reply&status={status}"),
                None,
                StatusCode::OK,
            )
            .await;
        assert_eq!(filtered["total"], 0);
    }
    for status in ["completed", "not_a_status"] {
        owner
            .expect(
                &app,
                Method::GET,
                &format!("/api/report?status={status}"),
                None,
                StatusCode::BAD_REQUEST,
            )
            .await;
    }
    owner
        .expect(
            &app,
            Method::GET,
            "/api/manga?status=waiting",
            None,
            StatusCode::BAD_REQUEST,
        )
        .await;
    owner
        .expect(
            &app,
            Method::GET,
            "/api/manga?status=ongoing",
            None,
            StatusCode::OK,
        )
        .await;
    let report_id = own["data"][0]["id"].as_str().unwrap();
    assert_eq!(
        owner
            .expect(
                &app,
                Method::GET,
                "/api/report?category=manga",
                None,
                StatusCode::OK
            )
            .await["total"],
        0
    );
    assert_eq!(
        limited
            .expect(&app, Method::GET, "/api/report", None, StatusCode::OK)
            .await["total"],
        0
    );
    let queue = moderator
        .expect(
            &app,
            Method::GET,
            "/api/staff/reports?order=asc&limit=1&page=2",
            None,
            StatusCode::OK,
        )
        .await;
    assert_eq!(queue["total"], 2);
    assert_eq!(queue["data"].as_array().unwrap().len(), 1);
    let accept = format!("/api/report/{report_id}/accept");
    owner
        .expect(
            &app,
            Method::POST,
            &accept,
            Some(json!({"notes":"unauthorized"})),
            StatusCode::FORBIDDEN,
        )
        .await;
    limited
        .expect(
            &app,
            Method::POST,
            &accept,
            Some(json!({"notes":"role alone is insufficient"})),
            StatusCode::FORBIDDEN,
        )
        .await;
    let accepted = moderator
        .expect(
            &app,
            Method::POST,
            &accept,
            Some(json!({"notes":"reviewed fixture","version":1})),
            StatusCode::OK,
        )
        .await;
    assert_eq!(accepted["status"], "accepted");
    assert_eq!(accepted["version"], 2);
    assert_eq!(accepted["notes"], "reviewed fixture");
    moderator
        .expect(
            &app,
            Method::POST,
            &format!("/api/report/{report_id}/reject"),
            Some(json!({"notes":"can't reverse terminal review"})),
            StatusCode::CONFLICT,
        )
        .await;
    let own2 = other
        .expect(&app, Method::GET, "/api/report", None, StatusCode::OK)
        .await;
    let report2 = own2["data"][0]["id"].as_str().unwrap();
    assert_eq!(
        moderator
            .expect(
                &app,
                Method::POST,
                &format!("/api/report/{report2}/reject"),
                Some(json!({"notes":"not actionable"})),
                StatusCode::OK
            )
            .await["status"],
        "refused"
    );
    owner
        .expect(
            &app,
            Method::DELETE,
            &format!("/api/report/{report_id}"),
            None,
            StatusCode::FORBIDDEN,
        )
        .await;
    moderator
        .expect(
            &app,
            Method::DELETE,
            &format!("/api/report/{report_id}"),
            None,
            StatusCode::NO_CONTENT,
        )
        .await;
    assert_eq!(
        owner
            .expect(&app, Method::GET, "/api/report", None, StatusCode::OK)
            .await["total"],
        0
    );
    // Source-missing announcements reconstruction is separately bounded and has no invented seed content.
    assert_eq!(
        anon.expect(&app, Method::GET, "/api/announcement", None, StatusCode::OK)
            .await["total"],
        0
    );
    owner
        .expect(
            &app,
            Method::GET,
            "/api/announcement?include_drafts=1",
            None,
            StatusCode::FORBIDDEN,
        )
        .await;
    moderator.expect(&app,Method::POST,"/api/announcement",Some(json!({"title":"Not authorized","body":"Moderator is not automatically a publisher"})),StatusCode::FORBIDDEN).await;
    let announcement=staff.expect(&app,Method::POST,"/api/announcement",Some(json!({"title":"Isolated announcement","body":"Synthetic **draft**. No external news is invented.","pinned":true})),StatusCode::CREATED).await;
    let announcement_id = announcement["id"].as_str().unwrap();
    let announcement_path = format!("/api/announcement/{announcement_id}");
    assert_eq!(announcement["published"], false);
    assert_eq!(announcement["version"], 1);
    anon.expect(
        &app,
        Method::GET,
        &announcement_path,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    assert_eq!(
        staff
            .expect(
                &app,
                Method::GET,
                "/api/announcement?include_drafts=1",
                None,
                StatusCode::OK
            )
            .await["total"],
        1
    );
    assert_eq!(
        staff
            .expect(&app, Method::GET, &announcement_path, None, StatusCode::OK)
            .await["body"],
        announcement["body"]
    );
    let published = staff
        .expect(
            &app,
            Method::PUT,
            &announcement_path,
            Some(json!({"version":1,"published":true})),
            StatusCode::OK,
        )
        .await;
    assert_eq!(published["version"], 2);
    assert!(published["published_at"].as_str().unwrap().ends_with('Z'));
    assert_eq!(
        anon.expect(
            &app,
            Method::GET,
            "/api/announcement?pinned=true&name=isolated&limit=1&page=1",
            None,
            StatusCode::OK
        )
        .await["total"],
        1
    );
    staff
        .expect(
            &app,
            Method::PUT,
            &announcement_path,
            Some(json!({"version":1,"title":"Stale"})),
            StatusCode::CONFLICT,
        )
        .await;
    staff
        .expect(
            &app,
            Method::DELETE,
            &announcement_path,
            Some(json!({"version":1})),
            StatusCode::CONFLICT,
        )
        .await;
    staff
        .expect(
            &app,
            Method::DELETE,
            &announcement_path,
            Some(json!({"version":2})),
            StatusCode::NO_CONTENT,
        )
        .await;
    anon.expect(
        &app,
        Method::GET,
        &announcement_path,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    // Source GroupClaimDialog requires the exact reason label and an actual accepted ownership transition.
    let reasons = anon
        .expect(
            &app,
            Method::GET,
            "/api/report/reasons/scanlation_group",
            None,
            StatusCode::OK,
        )
        .await;
    assert!(
        reasons
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["reason"] == "Group claim request")
    );
    for claimant in [&mut owner, &mut other] {
        claimant.expect(&app,Method::POST,"/api/report",Some(json!({"entity_type":"scanlation_group","entity_id":GROUP,"reason_id":"scanlation_group_claim","details":"Method: Verification Code\nURL: https://example.invalid/proof\nNotes: Isolated fixture"})),StatusCode::NO_CONTENT).await;
    }
    let claims = moderator
        .expect(
            &app,
            Method::GET,
            "/api/staff/reports?reason=scanlation_group_claim&order=asc",
            None,
            StatusCode::OK,
        )
        .await;
    assert_eq!(claims["total"], 2);
    let first_claim = claims["data"][0]["id"].as_str().unwrap();
    let second_claim = claims["data"][1]["id"].as_str().unwrap();
    moderator
        .expect(
            &app,
            Method::POST,
            &format!("/api/report/{first_claim}/accept"),
            Some(json!({"notes":"Verified synthetic ownership evidence","version":1})),
            StatusCode::OK,
        )
        .await;
    let group = anon
        .expect(
            &app,
            Method::GET,
            &format!("/api/group/{GROUP}"),
            None,
            StatusCode::OK,
        )
        .await;
    assert_eq!(group["version"], 2);
    assert_eq!(group["relationships"]["members"][0]["id"], OWNER);
    assert_eq!(group["relationships"]["members"][0]["is_leader"], true);
    moderator
        .expect(
            &app,
            Method::POST,
            &format!("/api/report/{second_claim}/accept"),
            Some(json!({"notes":"Stale concurrent claim"})),
            StatusCode::CONFLICT,
        )
        .await;
    assert_eq!(
        other
            .expect(
                &app,
                Method::GET,
                "/api/report?reason=scanlation_group_claim",
                None,
                StatusCode::OK
            )
            .await["data"][0]["status"],
        "waiting"
    );
    // Real stat envelopes and hourly visitor dedupe feed the existing top-rank query.
    let mv = format!("/api/analytics/manga/{MANGA}/views");
    let cv = format!("/api/analytics/chapter/{CHAPTER}/views");
    for _ in 0..3 {
        anon.expect(&app, Method::POST, &mv, None, StatusCode::NO_CONTENT)
            .await;
        anon.expect(&app, Method::POST, &cv, None, StatusCode::NO_CONTENT)
            .await;
    }
    assert!(anon.cookies.contains_key("weebdex_visitor"));
    let manga_stats = anon
        .expect(
            &app,
            Method::GET,
            &format!("/api/manga/{MANGA}/statistics"),
            None,
            StatusCode::OK,
        )
        .await;
    assert_eq!(manga_stats["views"], 1);
    assert_eq!(manga_stats["chapters"], 1);
    assert_eq!(manga_stats["follows"], 0);
    assert_eq!(
        anon.expect(
            &app,
            Method::GET,
            &format!("/api/chapter/{CHAPTER}/statistics"),
            None,
            StatusCode::OK
        )
        .await["views"],
        1
    );
    assert_eq!(
        anon.expect(
            &app,
            Method::GET,
            &format!("/api/group/{GROUP}/statistics"),
            None,
            StatusCode::OK
        )
        .await["chapters"],
        1
    );
    let stats = anon
        .expect(
            &app,
            Method::GET,
            &format!("/api/user/{OWNER}/statistics"),
            None,
            StatusCode::OK,
        )
        .await;
    assert_eq!(stats["chapters"], 1);
    assert_eq!(stats["messages"], 1);
    assert_eq!(stats["points"], 0);
    let stats = anon
        .expect(
            &app,
            Method::GET,
            &format!("/api/thread/{THREAD}/statistics"),
            None,
            StatusCode::OK,
        )
        .await;
    assert_eq!(stats, json!({"replies":2,"up":0,"down":0}));
    anon.expect(
        &app,
        Method::POST,
        "/api/analytics/manga/communitydraft01/views",
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    anon.expect(
        &app,
        Method::GET,
        "/api/user/missing/statistics",
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    // Tombstone renders neither old body nor author. Hard delete needs both role and permission.
    other
        .expect(&app, Method::DELETE, &path, None, StatusCode::FORBIDDEN)
        .await;
    for _ in 0..2 {
        owner
            .expect(&app, Method::DELETE, &path, None, StatusCode::NO_CONTENT)
            .await;
    }
    let removed = anon
        .expect(&app, Method::GET, &path, None, StatusCode::OK)
        .await;
    assert_eq!(removed["state"], "deleted_by_user");
    assert!(removed.get("body").is_none());
    assert!(removed["relationships"].get("author").is_none());
    other
        .expect(
            &app,
            Method::POST,
            &format!("{path}/vote"),
            Some(json!({"vote":1})),
            StatusCode::CONFLICT,
        )
        .await;
    owner
        .expect(
            &app,
            Method::DELETE,
            &format!("{path}/hard"),
            None,
            StatusCode::FORBIDDEN,
        )
        .await;
    limited
        .expect(
            &app,
            Method::DELETE,
            &format!("{path}/hard"),
            None,
            StatusCode::FORBIDDEN,
        )
        .await;
    moderator
        .expect(
            &app,
            Method::DELETE,
            &format!("{path}/hard"),
            None,
            StatusCode::NO_CONTENT,
        )
        .await;
    anon.expect(&app, Method::GET, &path, None, StatusCode::NOT_FOUND)
        .await;
    moderator
        .expect(&app, Method::DELETE, &path2, None, StatusCode::NO_CONTENT)
        .await;
    assert_eq!(
        anon.expect(&app, Method::GET, &path2, None, StatusCode::OK)
            .await["state"],
        "deleted_by_moderator"
    );
    let db = s.db.client().await.unwrap();
    let audit:i64=db.query_one("SELECT count(*) FROM platform.report_audit WHERE operation IN ('accept','reject','delete')",&[]).await.unwrap().get(0);
    assert_eq!(audit, 4);
    let view_rows: i64 = db
        .query_one("SELECT count(*) FROM platform.catalog_view_events", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(view_rows, 2);
    drop(db);
    trusted_proxy_analytics_seam(&s, anon.cookies.get("weebdex_visitor").unwrap()).await;
    let db = s.db.client().await.unwrap();
    let raw: Value = db
        .query_one("SELECT platform.catalog_manga_json($1)", &[&MANGA])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        raw["relationships"]["thread"]["relationships"]["stats"]["replies"],
        1
    );
    db.execute(
        "UPDATE platform.catalog_threads SET locked=TRUE WHERE id=$1",
        &[&THREAD],
    )
    .await
    .unwrap();
    owner
        .expect(
            &app,
            Method::POST,
            &base,
            Some(json!({"body":"locked"})),
            StatusCode::FORBIDDEN,
        )
        .await;
}
