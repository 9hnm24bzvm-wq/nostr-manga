//! Real public Router + owned isolated PostgreSQL seams. No external account,
//! production database, relay publication, or live credential is used.
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use weebdex_platform::{AppState, Config};
const ORIGIN: &str = "https://library-test.example.invalid";
const OWNER: &str = "libowner01";
const OTHER: &str = "libother01";
const M1: &str = "libmanga01";
const M2: &str = "libmanga02";
const C1: &str = "libchap001";
const C2: &str = "libchap002";
const C3: &str = "libchap003";
const GROUP: &str = "libgroup01";
const TOKEN: &str = "isolated-library-owner-token";
const OTHER_TOKEN: &str = "isolated-library-other-token";
async fn request(
    app: &Router,
    token: Option<&str>,
    method: Method,
    path: &str,
    payload: Option<Value>,
    origin: Option<&str>,
) -> (StatusCode, Value) {
    let mut r = Request::builder().method(method).uri(path);
    if let Some(t) = token {
        r = r.header(header::COOKIE, format!("weebdex_session={t}"));
    }
    if let Some(origin) = origin {
        r = r.header(header::ORIGIN, origin);
    }
    let body = if let Some(p) = payload {
        r = r.header(header::CONTENT_TYPE, "application/json");
        Body::from(p.to_string())
    } else {
        Body::empty()
    };
    let response = app.clone().oneshot(r.body(body).unwrap()).await.unwrap();
    let status = response.status();
    if path.starts_with("/api/user") || path.starts_with("/api/list/") || path == "/api/settings" {
        assert!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .is_some_and(|h| h.to_str().unwrap().contains("no-store")),
            "private response cache guard missing for {path}"
        );
    }
    let content = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let bytes = to_bytes(response.into_body(), 8 * 1024 * 1024)
        .await
        .unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else if content.starts_with("application/json") {
        serde_json::from_slice(&bytes).unwrap()
    } else {
        Value::String(String::from_utf8(bytes.to_vec()).unwrap())
    };
    (status, value)
}
async fn ok(
    app: &Router,
    token: Option<&str>,
    method: Method,
    path: &str,
    p: Option<Value>,
    expected: StatusCode,
) -> Value {
    let (status, value) = request(app, token, method, path, p, Some(ORIGIN)).await;
    assert_eq!(status, expected, "{path}: {value}");
    value
}
async fn state() -> AppState {
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL")
        .expect("Run only through scripts/run-auth-tests.sh with an owned isolated cluster");
    let pg: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
    assert!(!pg.get_hosts().is_empty());
    for host in pg.get_hosts() {
        match host {
            #[cfg(unix)]
            tokio_postgres::config::Host::Unix(p) => assert!(
                p.to_string_lossy()
                    .contains("/weebdex-platform/test-runtime/")
            ),
            tokio_postgres::config::Host::Tcp(host) => assert_eq!(host, "127.0.0.1"),
        }
    }
    let runtime =
        std::env::var("PLATFORM_TEST_RUNTIME_DIR").expect("Owned runtime receipt is required");
    let path = std::path::Path::new(&runtime);
    assert!(
        path.to_string_lossy()
            .contains("/weebdex-platform/test-runtime/auth-")
    );
    let receipt: Value = serde_json::from_slice(
        &std::fs::read(path.join("fixture.json")).expect("Owned disposable fixture receipt"),
    )
    .unwrap();
    assert_eq!(receipt["database"], "weebdex_platform_test");
    assert_eq!(receipt["fixture_only"], true);
    let state = AppState::connect(
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
    state
        .db
        .migrate(weebdex_platform::domains::MIGRATIONS)
        .await
        .unwrap();
    state
}
async fn seed(state: &AppState) {
    let db = state.db.client().await.unwrap();
    let permissions = json!(["self.preferences.update", "self.user.update"]);
    for (id, name, token) in [
        (OWNER, "library-fixture-owner", TOKEN),
        (OTHER, "library-fixture-other", OTHER_TOKEN),
    ] {
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,permissions) VALUES($1,$2,$3,'not-a-login-password-hash',TRUE,$4)",&[&id,&name,&format!("{id}@example.invalid"),&permissions]).await.unwrap();
        let hash = Sha256::digest(token.as_bytes()).to_vec();
        db.execute("INSERT INTO platform.sessions(id,token_hash,user_id,expires_at) VALUES($1,$2,$3,clock_timestamp()+interval '1 hour')",&[&format!("session_{id}"),&hash,&id]).await.unwrap();
    }
    db.execute("INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES($1,$2,'library-fixture-owner','Library fixture group')",&[&GROUP,&OWNER]).await.unwrap();
    db.execute("INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader) VALUES($1,$2,'library-fixture-owner',TRUE)",&[&GROUP,&OWNER]).await.unwrap();
    db.execute("INSERT INTO platform.catalog_tags(id,name,tag_group) VALUES('library-tag','Fixture tag','genre')",&[]).await.unwrap();
    for (id, title, language, rating, mal) in [
        (M1, "日本語, \"Quoted\" title", "ja", "safe", "1001"),
        (M2, "Second fixture title", "en", "suggestive", "1002"),
    ] {
        let metadata = json!({"links":{"mal":mal}});
        db.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state,published_at,metadata) VALUES($1,$2,'library-fixture-owner',$3,$4,'ongoing',$5,'published',clock_timestamp(),$6)",&[&id,&OWNER,&title,&language,&rating,&metadata]).await.unwrap();
        db.execute(
            "INSERT INTO platform.catalog_manga_counters(manga_id) VALUES($1)",
            &[&id],
        )
        .await
        .unwrap();
    }
    for (id, manga, language, n) in [
        (C1, M1, "en", 1_i32),
        (C2, M1, "zh-hk", 2_i32),
        (C3, M2, "pt-br", 3_i32),
    ] {
        let metadata = json!({"node":"https://media.example.invalid","data":[{"name":"p1.png","dimensions":[10,10]},{"name":"p2.png","dimensions":[10,10]}],"data_optimized":[]});
        db.execute("INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,language,chapter,metadata,published_at) VALUES($1,$2,$3,'library-fixture-owner',$4,$5,$6,clock_timestamp()+($7::INTEGER * interval '1 second'))",&[&id,&manga,&OWNER,&language,&n.to_string(),&metadata,&n]).await.unwrap();
        db.execute(
            "INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES($1,$2)",
            &[&id, &GROUP],
        )
        .await
        .unwrap();
    }
}

#[tokio::test]
#[ignore = "Requires owned disposable PostgreSQL cluster; scripts/run-auth-tests.sh cargo test --test library_http -- --ignored --test-threads=1"]
async fn actual_account_library_profiles_follows_history_preferences_import_export_and_private_lists()
 {
    let state = state().await;
    seed(&state).await;
    let app = weebdex_platform::router::router(state.clone());
    // Authentication, CSRF, source profile DTO and actual persisted user-thread/member relationships.
    ok(
        &app,
        None,
        Method::GET,
        "/api/user/follows/manga",
        None,
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let public = ok(
        &app,
        None,
        Method::GET,
        &format!("/api/user/{OWNER}"),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(public["name"], "library-fixture-owner");
    assert!(public.get("email").is_none());
    assert!(public.get("password_hash").is_none());
    assert_eq!(public["relationships"]["groups"][0]["id"], GROUP);
    assert!(public["relationships"]["thread"]["id"].is_string());
    let users = ok(
        &app,
        None,
        Method::GET,
        "/api/user?name=library-fixture&limit=1&page=2",
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(users["total"], 2);
    assert_eq!(users["data"].as_array().unwrap().len(), 1);
    ok(
        &app,
        Some(TOKEN),
        Method::PUT,
        &format!("/api/user/{OTHER}"),
        Some(json!({"version":1,"description":"Unauthorized"})),
        StatusCode::FORBIDDEN,
    )
    .await;
    let profile=ok(&app,Some(TOKEN),Method::PUT,&format!("/api/user/{OWNER}"),Some(json!({"version":1,"description":"説明","website":"https://example.invalid/profile","discord":"invite_42"})),StatusCode::OK).await;
    assert_eq!(profile["version"], 2);
    assert_eq!(profile["description"], "説明");
    ok(
        &app,
        Some(TOKEN),
        Method::PUT,
        &format!("/api/user/{OWNER}"),
        Some(json!({"version":1,"description":"stale"})),
        StatusCode::CONFLICT,
    )
    .await;
    ok(
        &app,
        Some(TOKEN),
        Method::PUT,
        &format!("/api/user/{OWNER}"),
        Some(json!({"version":2,"website":"javascript:alert(1)"})),
        StatusCode::BAD_REQUEST,
    )
    .await;
    let (status, _) = request(
        &app,
        Some(TOKEN),
        Method::POST,
        &format!("/api/user/follows/manga/{M1}"),
        Some(json!({"status":"reading"})),
        Some("https://evil.example.invalid"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    for _ in 0..2 {
        ok(
            &app,
            Some(TOKEN),
            Method::POST,
            &format!("/api/user/follows/manga/{M1}"),
            Some(json!({"status":"reading","notification":true})),
            StatusCode::NO_CONTENT,
        )
        .await;
    }
    ok(
        &app,
        Some(TOKEN),
        Method::POST,
        &format!("/api/user/follows/manga/{M2}"),
        Some(json!({"status":"completed","notification":false})),
        StatusCode::NO_CONTENT,
    )
    .await;
    let followed=ok(&app,Some(TOKEN),Method::GET,"/api/user/follows/manga?reading_status=reading&reading_status=completed&lang=ja&lang=en&sort=title&order=asc&limit=1&page=2",None,StatusCode::OK).await;
    assert_eq!(followed["total"], 2);
    assert_eq!(followed["data"].as_array().unwrap().len(), 1);
    assert!(
        followed["data"][0]["followed_at"]
            .as_str()
            .unwrap()
            .ends_with('Z')
    );
    assert_eq!(followed["data"][0]["private"], true);
    assert_eq!(
        ok(
            &app,
            Some(OTHER_TOKEN),
            Method::GET,
            "/api/user/follows/manga",
            None,
            StatusCode::OK
        )
        .await["total"],
        0
    );
    let check = ok(
        &app,
        Some(TOKEN),
        Method::GET,
        "/api/auth/check",
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(check["follows"][M1], json!(["reading", 1]));
    assert_eq!(check["follows"][M2], json!(["completed", 0]));
    assert_eq!(check["groups"][0]["id"], GROUP);
    let count: i64 = state
        .db
        .client()
        .await
        .unwrap()
        .query_one(
            "SELECT follows FROM platform.catalog_manga_counters WHERE manga_id=$1",
            &[&M1],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 1);
    let feed = ok(
        &app,
        Some(TOKEN),
        Method::GET,
        "/api/user/follows/chapter/feed?tlang=en&tlang=zh-hk&group=libgroup01&limit=1&page=2",
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(feed["total"], 2);
    assert_eq!(feed["data"].as_array().unwrap().len(), 1);
    assert_eq!(feed["map"]["manga"][M1]["id"], M1);
    let latest = ok(
        &app,
        Some(TOKEN),
        Method::GET,
        "/api/user/follows/chapter/updates?limit=10",
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(latest["total"], 1);
    assert_eq!(latest["data"][0]["id"], C2);
    ok(
        &app,
        Some(TOKEN),
        Method::GET,
        "/api/user/follows/manga?page=1&page=2",
        None,
        StatusCode::BAD_REQUEST,
    )
    .await;
    // Exact nested history, strict manga ownership, duplicate/retried writes preserve original read time.
    let first = ok(
        &app,
        Some(TOKEN),
        Method::POST,
        &format!("/api/user/history/manga/{M1}"),
        Some(json!({"read_ids":[C1,C1,C2]})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(first, json!({"read_updated":2,"unread_updated":0}));
    let repeated = ok(
        &app,
        Some(TOKEN),
        Method::POST,
        &format!("/api/user/history/manga/{M1}"),
        Some(json!({"read_ids":[C1,C2]})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(repeated["read_updated"], 0);
    ok(
        &app,
        Some(TOKEN),
        Method::POST,
        &format!("/api/user/history/manga/{M1}"),
        Some(json!({"unread_ids":[C1,C3]})),
        StatusCode::BAD_REQUEST,
    )
    .await;
    let nested = ok(
        &app,
        Some(TOKEN),
        Method::GET,
        &format!("/api/user/history/manga?id={M1}&id={M2}"),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(nested[M1], json!({C1:null,C2:null}));
    assert_eq!(nested[M2], json!({}));
    assert_eq!(
        ok(
            &app,
            Some(TOKEN),
            Method::GET,
            &format!("/api/user/history/manga/{M1}"),
            None,
            StatusCode::OK
        )
        .await[M1],
        nested[M1]
    );
    assert_eq!(
        ok(
            &app,
            Some(OTHER_TOKEN),
            Method::GET,
            &format!("/api/user/history/manga/{M1}"),
            None,
            StatusCode::OK
        )
        .await[M1],
        json!({})
    );
    let history = ok(
        &app,
        Some(TOKEN),
        Method::GET,
        "/api/user/history/manga/feed?limit=1&page=2",
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(history["total"], 2);
    assert!(
        history["data"][0]["read_at"]
            .as_str()
            .unwrap()
            .ends_with('Z')
    );
    assert_eq!(history["map"]["manga"][M1]["id"], M1);
    ok(
        &app,
        Some(TOKEN),
        Method::POST,
        &format!("/api/user/history/manga/{M1}"),
        Some(json!({"read_ids":[C1],"unread_ids":[C1]})),
        StatusCode::BAD_REQUEST,
    )
    .await;
    let removed = ok(
        &app,
        Some(TOKEN),
        Method::POST,
        &format!("/api/user/history/manga/{M1}"),
        Some(json!({"unread_ids":[C1]})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(removed["unread_updated"], 1);
    // User/group follows: exact 204 status contract, real list DTO and owner isolation.
    for (kind, id) in [("users", OTHER), ("groups", GROUP)] {
        let path = format!("/api/user/follows/{kind}/{id}");
        ok(
            &app,
            Some(TOKEN),
            Method::GET,
            &path,
            None,
            StatusCode::NOT_FOUND,
        )
        .await;
        for _ in 0..2 {
            ok(
                &app,
                Some(TOKEN),
                Method::POST,
                &path,
                None,
                StatusCode::NO_CONTENT,
            )
            .await;
        }
        ok(
            &app,
            Some(TOKEN),
            Method::GET,
            &path,
            None,
            StatusCode::NO_CONTENT,
        )
        .await;
        let feed = ok(
            &app,
            Some(TOKEN),
            Method::GET,
            &format!("/api/user/follows/{kind}?limit=1&page=1"),
            None,
            StatusCode::OK,
        )
        .await;
        assert_eq!(feed["total"], 1);
        assert_eq!(feed["data"][0]["id"], id);
        assert!(feed["data"][0]["followed_at"].is_string());
        assert_eq!(
            ok(
                &app,
                Some(OTHER_TOKEN),
                Method::GET,
                &format!("/api/user/follows/{kind}"),
                None,
                StatusCode::OK
            )
            .await["total"],
            0
        );
        for _ in 0..2 {
            ok(
                &app,
                Some(TOKEN),
                Method::DELETE,
                &path,
                None,
                StatusCode::NO_CONTENT,
            )
            .await;
        }
        ok(
            &app,
            Some(TOKEN),
            Method::GET,
            &path,
            None,
            StatusCode::NOT_FOUND,
        )
        .await;
    }
    ok(
        &app,
        Some(TOKEN),
        Method::POST,
        &format!("/api/user/follows/users/{OWNER}"),
        None,
        StatusCode::BAD_REQUEST,
    )
    .await;
    // Preferences persist canonical IDs and return real DTO expansions; version conflict is atomic.
    let prefs=ok(&app,Some(TOKEN),Method::PUT,"/api/settings",Some(json!({"version":1,"items_per_page":"42","languages":["ja","zh-hk"],"translated_languages":["pt-br","es-la"],"tags":["library-tag"],"uploaders":[OTHER],"groups":[GROUP],"show_safe":true,"show_suggestive":false,"show_erotic":false,"show_hentai":false,"data_saver":true})),StatusCode::OK).await;
    assert_eq!(prefs["version"], 2);
    assert_eq!(prefs["uploaders"][0]["id"], OTHER);
    assert_eq!(prefs["groups"][0]["id"], GROUP);
    assert_eq!(prefs["data_saver"], true);
    ok(
        &app,
        Some(TOKEN),
        Method::PUT,
        "/api/settings",
        Some(json!({"version":1,"items_per_page":"10"})),
        StatusCode::CONFLICT,
    )
    .await;
    ok(
        &app,
        Some(TOKEN),
        Method::PUT,
        "/api/settings",
        Some(json!({"version":2,"uploaders":["missing-user"]})),
        StatusCode::BAD_REQUEST,
    )
    .await;
    assert_eq!(
        ok(
            &app,
            Some(TOKEN),
            Method::GET,
            "/api/auth/check",
            None,
            StatusCode::OK
        )
        .await["preferences"],
        prefs
    );
    // Real import, unique external mappings, all-or-nothing invalid batch, and CSV response.
    ok(
        &app,
        Some(TOKEN),
        Method::POST,
        "/api/user/follows/manga/import",
        Some(json!({"weebdex":{M1:"on_hold",M2:"reading"}})),
        StatusCode::NO_CONTENT,
    )
    .await;
    ok(
        &app,
        Some(TOKEN),
        Method::POST,
        "/api/user/follows/manga/import",
        Some(json!({"myanimelist":{"1001":"completed"}})),
        StatusCode::NO_CONTENT,
    )
    .await;
    ok(
        &app,
        Some(TOKEN),
        Method::POST,
        "/api/user/follows/manga/import",
        Some(json!({"weebdex":{M1:"dropped","missing":"reading"}})),
        StatusCode::BAD_REQUEST,
    )
    .await;
    assert_eq!(
        ok(
            &app,
            Some(TOKEN),
            Method::GET,
            "/api/auth/check",
            None,
            StatusCode::OK
        )
        .await["follows"][M1][0],
        "completed"
    );
    let csv = ok(
        &app,
        Some(TOKEN),
        Method::GET,
        "/api/user/follows/manga/export",
        None,
        StatusCode::OK,
    )
    .await;
    assert!(
        csv.as_str()
            .unwrap()
            .starts_with("weebdex_id,title,reading_status\r\n")
    );
    assert!(csv.as_str().unwrap().contains("\"\"Quoted\"\""));
    assert!(
        !ok(
            &app,
            Some(OTHER_TOKEN),
            Method::GET,
            "/api/user/follows/manga/export",
            None,
            StatusCode::OK
        )
        .await
        .as_str()
        .unwrap()
        .contains(M1)
    );
    for _ in 0..2 {
        ok(
            &app,
            Some(TOKEN),
            Method::DELETE,
            &format!("/api/user/follows/manga/{M1}"),
            None,
            StatusCode::NO_CONTENT,
        )
        .await;
    }
    let count: i64 = state
        .db
        .client()
        .await
        .unwrap()
        .query_one(
            "SELECT follows FROM platform.catalog_manga_counters WHERE manga_id=$1",
            &[&M1],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0);
    // Additive contracts are tested separately from the 22 observed source operations.
    let list = ok(
        &app,
        Some(TOKEN),
        Method::POST,
        "/api/user/lists",
        Some(json!({"name":"Private fixture list","manga_ids":[M2,M1,M2]})),
        StatusCode::CREATED,
    )
    .await;
    let list_id = list["id"].as_str().unwrap();
    let path = format!("/api/list/{list_id}");
    assert_eq!(list["visibility"], "private");
    assert_eq!(list["relationships"]["manga"][0]["id"], M2);
    assert_eq!(list["relationships"]["manga"].as_array().unwrap().len(), 2);
    ok(&app, None, Method::GET, &path, None, StatusCode::NOT_FOUND).await;
    ok(
        &app,
        Some(OTHER_TOKEN),
        Method::GET,
        &path,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    ok(
        &app,
        Some(OTHER_TOKEN),
        Method::PUT,
        &path,
        Some(json!({"version":1,"visibility":"public"})),
        StatusCode::NOT_FOUND,
    )
    .await;
    assert_eq!(
        ok(
            &app,
            Some(TOKEN),
            Method::GET,
            "/api/user/lists",
            None,
            StatusCode::OK
        )
        .await["total"],
        1
    );
    assert_eq!(
        ok(
            &app,
            None,
            Method::GET,
            &format!("/api/user/{OWNER}/lists"),
            None,
            StatusCode::OK
        )
        .await["total"],
        0
    );
    let public_list = ok(
        &app,
        Some(TOKEN),
        Method::PUT,
        &path,
        Some(json!({"version":1,"visibility":"public","manga_ids":[M1,M2]})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(public_list["version"], 2);
    assert_eq!(public_list["relationships"]["manga"][0]["id"], M1);
    ok(&app, None, Method::GET, &path, None, StatusCode::OK).await;
    ok(
        &app,
        Some(TOKEN),
        Method::PUT,
        &path,
        Some(json!({"version":1,"name":"stale"})),
        StatusCode::CONFLICT,
    )
    .await;
    assert_eq!(
        ok(
            &app,
            None,
            Method::GET,
            &format!("/api/user/{OWNER}/lists"),
            None,
            StatusCode::OK
        )
        .await["total"],
        1
    );
    ok(
        &app,
        Some(TOKEN),
        Method::PUT,
        &path,
        Some(json!({"version":2,"visibility":"unlisted"})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        ok(
            &app,
            None,
            Method::GET,
            &format!("/api/user/{OWNER}/lists"),
            None,
            StatusCode::OK
        )
        .await["total"],
        0
    );
    ok(&app, None, Method::GET, &path, None, StatusCode::OK).await;
    ok(
        &app,
        Some(TOKEN),
        Method::DELETE,
        &path,
        Some(json!({"version":3})),
        StatusCode::NO_CONTENT,
    )
    .await;
    let progress_path = format!("/api/user/history/chapter/{C1}/progress");
    ok(
        &app,
        Some(TOKEN),
        Method::GET,
        &progress_path,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    let progress = ok(
        &app,
        Some(TOKEN),
        Method::PUT,
        &progress_path,
        Some(json!({"version":0,"page":1,"completed":true})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(progress["version"], 1);
    assert_eq!(progress["page"], 1);
    ok(
        &app,
        Some(OTHER_TOKEN),
        Method::GET,
        &progress_path,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    assert_eq!(
        ok(
            &app,
            Some(TOKEN),
            Method::GET,
            &progress_path,
            None,
            StatusCode::OK
        )
        .await,
        progress
    );
    ok(
        &app,
        Some(TOKEN),
        Method::PUT,
        &progress_path,
        Some(json!({"version":1,"page":2})),
        StatusCode::BAD_REQUEST,
    )
    .await;
    ok(
        &app,
        Some(TOKEN),
        Method::PUT,
        &progress_path,
        Some(json!({"version":0,"page":0})),
        StatusCode::CONFLICT,
    )
    .await;
    // Correlated public/private contents must share the authorization snapshot.
    let list = ok(&app, Some(TOKEN), Method::POST, "/api/user/lists",
        Some(json!({"name":"Public snapshot","description":"public snapshot","visibility":"public","manga_ids":[M1]})), StatusCode::CREATED).await;
    let coherent_path = format!("/api/list/{}", list["id"].as_str().unwrap());
    let writer = async {
        let mut version = 1;
        for _ in 0..16 {
            ok(&app, Some(TOKEN), Method::PUT, &coherent_path,
                Some(json!({"version":version,"name":"Private snapshot","description":"private-only contents","visibility":"private","manga_ids":[M2]})), StatusCode::OK).await;
            version += 1;
            ok(&app, Some(TOKEN), Method::PUT, &coherent_path,
                Some(json!({"version":version,"name":"Public snapshot","description":"public snapshot","visibility":"public","manga_ids":[M1]})), StatusCode::OK).await;
            version += 1;
        }
        ok(&app, Some(TOKEN), Method::PUT, &coherent_path,
            Some(json!({"version":version,"description":"private-only contents","visibility":"private","manga_ids":[M2]})), StatusCode::OK).await;
    };
    let readers = async {
        for _ in 0..64 {
            let (status, value) =
                request(&app, None, Method::GET, &coherent_path, None, None).await;
            if status == StatusCode::OK {
                assert_eq!(value["visibility"], "public");
                assert_eq!(value["description"], "public snapshot");
                assert_eq!(value["relationships"]["manga"][0]["id"], M1);
            } else {
                assert_eq!(status, StatusCode::NOT_FOUND);
            }
            tokio::task::yield_now().await;
        }
    };
    tokio::join!(writer, readers);
    ok(
        &app,
        None,
        Method::GET,
        &coherent_path,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    let private = ok(
        &app,
        Some(TOKEN),
        Method::GET,
        &coherent_path,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(private["description"], "private-only contents");
    assert_eq!(private["relationships"]["manga"][0]["id"], M2);
    // The only newly written state is platform.* instance rows, never signed protocol events.
    let principal = ok(
        &app,
        Some(TOKEN),
        Method::GET,
        "/api/auth/check",
        None,
        StatusCode::OK,
    )
    .await;
    assert!(principal.get("nostr_private_key").is_none());
}
