//! Real HTTP and PostgreSQL coverage of native-account library adapters.
//! Synthetic cookies only, no protocol identity or real user secrets.
use manga_node::db::{DbConfig, Repository};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::Duration;
use weebdex_platform::{
    AppState, Config, domains,
    public_api::{PostgresPublicIdResolver, PublicIdResolver, PublicUuid, ResourceKind},
};

struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}
#[allow(dead_code)]
struct Fixture {
    state: AppState,
    http: reqwest::Client,
    origin: String,
    account: String,
    other: String,
    manga: String,
    alias: String,
    hidden: String,
    reason: String,
    group: String,
    chapters: Vec<String>,
    second: String,
    _server: Server,
}
#[allow(dead_code)]
impl Fixture {
    fn request(&self, method: &str, path: &str, user: Option<&str>) -> reqwest::RequestBuilder {
        let mut request = self
            .http
            .request(method.parse().unwrap(), format!("{}{path}", self.origin))
            .header("content-type", "application/json");
        if let Some(user) = user {
            request = request
                .header(
                    "cookie",
                    format!(
                        "{}=synthetic-cookie-{user}",
                        self.state.config.session_cookie_name
                    ),
                )
                .header("x-weebdex-expected-account", user);
        }
        request
    }
    async fn response(&self, request: reqwest::RequestBuilder, expected: u16) -> Value {
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        assert_eq!(response.headers()["cache-control"], "private, no-store");
        let vary = response
            .headers()
            .get_all("vary")
            .iter()
            .map(|v| v.to_str().unwrap().to_lowercase())
            .collect::<Vec<_>>()
            .join(",");
        for header in ["cookie", "authorization", "x-weebdex-expected-account"] {
            assert!(
                vary.split(',').any(|v| v.trim() == header),
                "Missing {header}: {vary}"
            );
        }
        let value: Value = response.json().await.unwrap();
        assert_eq!(status, expected, "{value}");
        if expected == 200 && value.get("revision").is_some() {
            assert_eq!(value["authentication"], "native-session");
            assert_eq!(value["scope"], "instance-account");
        }
        let text = value.to_string();
        for private in [
            "synthetic-cookie-",
            "@example.invalid",
            "password_hash",
            "token_hash",
            "session_id",
        ] {
            assert!(!text.contains(private), "Private field escaped");
        }
        value
    }
    async fn call(
        &self,
        method: &str,
        path: &str,
        user: Option<&str>,
        value: Option<Value>,
        status: u16,
    ) -> Value {
        let request = self
            .request(method, path, user)
            .header("origin", &self.origin);
        self.response(
            if let Some(value) = value {
                request.body(serde_json::to_vec(&value).unwrap())
            } else {
                request
            },
            status,
        )
        .await
    }
    fn rating(&self, value: i32, version: i32) -> Value {
        json!({"rating":value,"version":version,"expectedAccountId":self.account})
    }
    fn report(&self) -> Value {
        json!({"category":"manga","objectId":self.manga,"reason":self.reason,"details":"Synthetic ordinary account report","expectedAccountId":self.account,"expectedReasonVersion":1})
    }
    async fn counts(&self) -> (i64, i64, i64, i64) {
        let row = self.state.db.client().await.unwrap().query_one("SELECT (SELECT count(*) FROM platform.library_manga_ratings),(SELECT count(*) FROM platform.reports),(SELECT count(*) FROM platform.protocol_identity_bindings),(SELECT count(*) FROM account_permissions)",&[]).await.unwrap();
        (row.get(0), row.get(1), row.get(2), row.get(3))
    }
}
async fn fixture() -> Fixture {
    let dsn =
        std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use isolated run-auth-tests.sh");
    let pg: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
    assert!(!pg.get_hosts().is_empty());
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
    let repository = Repository::connect(DbConfig {
        dsn: dsn.clone(),
        connections: 2,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await
    .unwrap();
    repository.migrate().await.unwrap();
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
    for user in [
        "NativeActor",
        "NativeOther",
        "NativeUnverified",
        "NativeBanned",
        "NativeRestricted",
        "NativeNoPerm",
        "NativeDisabled",
        "NativeRecovery",
        "NativeExpired",
        "NativeRevoked",
    ] {
        let roles = if user == "NativeBanned" {
            json!(["banned"])
        } else if user == "NativeRestricted" {
            json!(["restricted"])
        } else {
            json!(["user"])
        };
        let permissions = if user == "NativeNoPerm" {
            json!([])
        } else {
            json!(["self.preferences.update", "report.create"])
        };
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,roles,permissions) VALUES($1,$1,$2,'synthetic-no-password',$3,$4,$5)",&[&user,&format!("{user}@example.invalid"),&(user!="NativeUnverified"),&roles,&permissions]).await.unwrap();
        let hash = Sha256::digest(format!("synthetic-cookie-{user}").as_bytes()).to_vec();
        db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at,purpose) VALUES($1,$1,$2,clock_timestamp()+interval '1 hour',$3)",&[&user,&hash,&if user=="NativeRecovery" {"recovery"}else{"normal"}]).await.unwrap();
    }
    db.batch_execute("UPDATE platform.users SET disabled=true WHERE id='NativeDisabled'; UPDATE platform.sessions SET created_at=clock_timestamp()-interval '2 hours',expires_at=clock_timestamp()-interval '1 hour' WHERE id='NativeExpired'; UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE id='NativeRevoked'; INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES('NativeMng1','NativeActor','Fixture','Visible native title','en','ongoing','safe','published'),('NativeHid1','NativeActor','Fixture','Hidden title','en','ongoing','safe','published'); UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='NativeHid1'; INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES('NativeGrp1','NativeActor','Fixture','Visible group');").await.unwrap();
    db.batch_execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES('NativeMng2','NativeActor','Fixture','Second title','ja','ongoing','suggestive','published');").await.unwrap();
    for (id, manga, chapter, language) in [
        ("NativeCh1", "NativeMng1", "1", "en"),
        ("NativeCh2", "NativeMng1", "2", "ja"),
        ("NativeCh3", "NativeMng2", "1", "en"),
    ] {
        let metadata = json!({"node":"https://media.example.invalid","data":[{"name":"p1.png","dimensions":[10,10]},{"name":"p2.png","dimensions":[10,10]}],"data_optimized":[]});
        db.execute("INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,language,chapter,metadata) VALUES($1,$2,'NativeActor','Fixture',$3,$4,$5)",&[&id,&manga,&language,&chapter,&metadata]).await.unwrap();
    }
    drop(db);
    let resolver = PostgresPublicIdResolver::new(state.db.clone());
    async fn preferred(
        resolver: &PostgresPublicIdResolver,
        kind: ResourceKind,
        native: &str,
    ) -> String {
        resolver
            .preferred(kind, native)
            .await
            .unwrap()
            .unwrap()
            .to_string()
    }
    let account = preferred(&resolver, ResourceKind::User, "NativeActor").await;
    let other = preferred(&resolver, ResourceKind::User, "NativeOther").await;
    let manga = preferred(&resolver, ResourceKind::Manga, "NativeMng1").await;
    let hidden = preferred(&resolver, ResourceKind::Manga, "NativeHid1").await;
    let reason = preferred(&resolver, ResourceKind::ReportReason, "manga_spam").await;
    let group = preferred(&resolver, ResourceKind::Group, "NativeGrp1").await;
    let second = preferred(&resolver, ResourceKind::Manga, "NativeMng2").await;
    let mut chapters = vec![];
    for id in ["NativeCh1", "NativeCh2", "NativeCh3"] {
        chapters.push(preferred(&resolver, ResourceKind::Chapter, id).await);
    }
    let alias = "aaaaaaaa-aaaa-8aaa-8aaa-aaaaaaaaaaaa".to_owned();
    resolver
        .register_alias(
            ResourceKind::Manga,
            "NativeMng1",
            &PublicUuid::parse(&alias).unwrap(),
        )
        .await
        .unwrap();
    let app = weebdex_platform::router::router(state.clone());
    let server = Server(tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap()
    }));
    Fixture {
        state,
        http: reqwest::Client::new(),
        origin,
        account,
        other,
        manga,
        alias,
        hidden,
        reason,
        group,
        chapters,
        second,
        _server: server,
    }
}

const BASE: &str = "/api/v1/account/library";
impl Fixture {
    async fn get(&self, path: &str) -> Value {
        self.call(
            "GET",
            &format!("{BASE}{path}"),
            Some("NativeActor"),
            None,
            200,
        )
        .await
    }
    async fn revision(&self) -> i64 {
        self.get("/lists").await["revision"].as_i64().unwrap()
    }
    async fn command(&self, method: &str, path: &str, mut fields: Value, status: u16) -> Value {
        fields["expectedAccountId"] = json!(self.account);
        fields["revision"] = json!(self.revision().await);
        self.call(
            method,
            &format!("{BASE}{path}"),
            Some("NativeActor"),
            Some(fields),
            status,
        )
        .await
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires isolated scripts/run-auth-tests.sh PostgreSQL"]
async fn real_account_library_cas_privacy_history_progress_lists_and_legacy_writers() {
    let f = fixture().await;
    assert_eq!(f.get("/follows").await["total"], 0);
    assert_eq!(f.revision().await, 0);
    let continue_path = format!("/continue/{}?translatedLanguage[]=en", f.manga);
    let first = f.get(&continue_path).await;
    assert_eq!(first["data"]["mangaId"], f.manga);
    assert_eq!(first["data"]["orderingProfile"], "public-retained-v1");
    assert_eq!(first["data"]["reason"], "oldest-unread");
    assert_eq!(first["data"]["winner"]["id"], f.chapters[0]);
    assert!(!first.to_string().contains("NativeCh"));
    assert_eq!(
        f.get(&format!("/continue/{}?translatedLanguage[]=fr", f.manga))
            .await["data"]["reason"],
        "empty"
    );
    f.call(
        "GET",
        &format!("{BASE}/continue/{}?history=forged", f.manga),
        Some("NativeActor"),
        None,
        400,
    )
    .await;
    let state_path = format!("/follows/state?ids[]={}", f.manga);
    let empty = f.get(&state_path).await;
    assert_eq!(empty["data"][0]["saved"], false);
    // Boundary rejects missing/ambiguous ambient authority and explicit owner switch.
    f.call("GET", &format!("{BASE}/lists"), None, None, 400)
        .await;
    f.response(
        f.request("GET", &format!("{BASE}/lists"), Some("NativeActor"))
            .header("authorization", "Bearer synthetic"),
        403,
    )
    .await;
    f.response(
        f.request("GET", &format!("{BASE}/lists"), Some("NativeActor"))
            .header(
                "cookie",
                format!(
                    "{}=synthetic-cookie-NativeOther",
                    f.state.config.session_cookie_name
                ),
            ),
        401,
    )
    .await;
    f.response(
        f.request("GET", &format!("{BASE}/lists"), Some("NativeActor"))
            .header("x-weebdex-expected-account", "NativeOther"),
        400,
    )
    .await;
    for (user, status) in [
        ("NativeDisabled", 401),
        ("NativeExpired", 401),
        ("NativeRevoked", 401),
        ("NativeRecovery", 403),
        ("NativeUnverified", 200),
        ("NativeRestricted", 200),
        ("NativeBanned", 200),
    ] {
        f.call("GET", &format!("{BASE}/lists"), Some(user), None, status)
            .await;
    }
    let follow = format!("/follows/{}", f.alias);
    let initial = f
        .command(
            "POST",
            &follow,
            json!({"status":"reading","notification":true}),
            200,
        )
        .await;
    assert_eq!(initial["data"]["mangaId"], f.manga);
    assert_eq!(initial["data"]["saved"], true);
    let rev = initial["revision"].as_i64().unwrap();
    assert!(rev > 0);
    // Missing/wrong canonical ownership, CSRF and stale revisions fail without mutation.
    let write_path = format!("{BASE}{follow}");
    let command = json!({"expectedAccountId":f.account,"revision":rev,"status":"completed","notification":true});
    f.response(
        f.request("POST", &write_path, Some("NativeActor"))
            .json(&command),
        403,
    )
    .await;
    let mut wrong = command.clone();
    wrong["expectedAccountId"] = json!(f.other);
    f.call("POST", &write_path, Some("NativeActor"), Some(wrong), 409)
        .await;
    f.call(
        "POST",
        &write_path,
        Some("NativeOther"),
        Some(command.clone()),
        409,
    )
    .await;
    let changed = f
        .command(
            "POST",
            &follow,
            json!({"status":"completed","notification":false}),
            200,
        )
        .await;
    assert!(changed["revision"].as_i64().unwrap() > rev);
    f.call("POST", &write_path, Some("NativeActor"), Some(command), 409)
        .await;
    let same = f
        .command(
            "POST",
            &follow,
            json!({"status":"completed","notification":false}),
            200,
        )
        .await;
    assert_eq!(same["revision"], changed["revision"]);
    let list = f.get("/follows?limit=1&sort=title&order=asc").await;
    assert_eq!(list["total"], 1);
    assert_eq!(list["data"][0]["manga"]["id"], f.manga);
    assert!(!list.to_string().contains("NativeMng"));
    assert_eq!(
        f.get("/follows?title=absent&page=3&limit=1").await["total"],
        0
    );
    for bad in [
        "/follows?limit=0",
        "/follows?limit=101",
        "/follows?page=1000000",
        "/follows?wat=true",
        "/follows?page=1&page=2",
        "/lists?sort=name",
    ] {
        f.call(
            "GET",
            &format!("{BASE}{bad}"),
            Some("NativeActor"),
            None,
            400,
        )
        .await;
    }
    for id in [
        &f.hidden,
        &f.account,
        &"00000000-0000-8000-8000-000000000000".to_owned(),
    ] {
        f.call(
            "GET",
            &format!("{BASE}/follows/state?ids[]={id}"),
            Some("NativeActor"),
            None,
            404,
        )
        .await;
    }
    f.call(
        "GET",
        &format!("{BASE}/follows/state?ids[]=NativeMng1"),
        Some("NativeActor"),
        None,
        400,
    )
    .await;
    assert_eq!(f.get("/updates").await["total"], 0); // notifications disabled, real source not fake sync
    f.command(
        "POST",
        &follow,
        json!({"status":"reading","notification":true}),
        200,
    )
    .await;
    assert_eq!(f.get("/updates").await["total"], 2);
    assert_eq!(f.get("/updates?latest=true").await["total"], 1);
    assert_eq!(f.get("/updates?tlang=en").await["total"], 1);
    let progress = format!("/progress/{}", f.chapters[0]);
    let p = f.get(&progress).await;
    assert!(p["data"]["page"].is_null());
    assert_eq!(p["data"]["version"], 0);
    assert_eq!(p["data"]["pageCount"], 2);
    assert_eq!(p["data"]["chapterId"], f.chapters[0]);
    assert_eq!(p["data"]["mangaId"], f.manga);
    assert_eq!(
        p["catalogIdentity"],
        f.get(&continue_path).await["data"]["catalogIdentity"]
    );
    f.command(
        "POST",
        &progress,
        json!({"page":2,"completed":false,"progressVersion":0}),
        400,
    )
    .await;
    let saved = f
        .command(
            "POST",
            &progress,
            json!({"page":1,"completed":true,"progressVersion":0}),
            200,
        )
        .await;
    assert_eq!(saved["data"]["completed"], true);
    assert_eq!(saved["data"]["version"], 1);
    assert_eq!(
        f.get(&continue_path).await["data"]["reason"],
        "all-completed-newest"
    );
    assert_eq!(
        f.get(&format!("/continue/{}", f.manga)).await["data"]["winner"]["id"],
        f.chapters[1]
    );

    assert_eq!(f.get("/history/feed").await["total"], 1);
    f.command(
        "POST",
        &progress,
        json!({"page":0,"completed":false,"progressVersion":0}),
        409,
    )
    .await;
    let history = format!("/history/{}", f.manga);
    let before = f.revision().await;
    f.command(
        "POST",
        &history,
        json!({"readIds":[f.chapters[1],f.chapters[2]],"unreadIds":[]}),
        404,
    )
    .await;
    assert_eq!(f.revision().await, before); // cross-parent batch atomic
    f.command(
        "POST",
        &history,
        json!({"readIds":[f.chapters[0]],"unreadIds":[f.chapters[0]]}),
        400,
    )
    .await;
    f.command(
        "POST",
        &history,
        json!({"readIds":[f.chapters[1]],"unreadIds":[f.chapters[0]]}),
        200,
    )
    .await;
    let p = f.get(&progress).await;
    assert_eq!(p["data"]["completed"], false);
    assert_eq!(p["data"]["page"], 1);
    assert_eq!(p["data"]["version"], 2);
    assert_eq!(p["data"]["pageCount"], 2);
    assert_eq!(
        p["catalogIdentity"],
        f.get(&continue_path).await["data"]["catalogIdentity"]
    );
    assert_eq!(
        f.get(&format!("/history/state?manga[]={}&limit=1", f.manga))
            .await["total"],
        1
    );
    assert_eq!(
        f.get(&format!("/history/state?ids[]={}", f.chapters[0]))
            .await["total"],
        0
    );
    let rev = f.revision().await;
    f.command(
        "POST",
        &history,
        json!({"readIds":[f.chapters[1]],"unreadIds":[]}),
        200,
    )
    .await;
    assert_eq!(f.revision().await, rev); // read checks idempotent
    assert_eq!(
        f.get(&format!("/progress/{}", f.chapters[1])).await["data"]["page"],
        Value::Null
    ); // no invented position
    let created = f
        .command(
            "POST",
            "/lists",
            json!({"name":"Private list","mangaIds":[f.manga,f.second]}),
            200,
        )
        .await;
    let id = created["data"]["id"].as_str().unwrap().to_owned();
    let path = format!("/lists/{id}");
    let titles = f.get(&format!("{path}/titles?limit=1&page=2")).await;
    assert_eq!(titles["listId"], id);
    assert_eq!(titles["listVersion"], 1);
    assert_eq!(titles["total"], 2);
    assert_eq!(titles["data"][0]["manga"]["id"], f.second);
    assert_eq!(titles["data"][0]["saved"], false);
    assert_eq!(titles["data"][0]["status"], Value::Null);
    f.call(
        "GET",
        &format!("{BASE}{path}/titles"),
        Some("NativeOther"),
        None,
        404,
    )
    .await;

    assert_eq!(created["data"]["version"], 1);
    assert_eq!(created["data"]["mangaIds"], json!([f.manga, f.second]));
    f.call(
        "GET",
        &format!("{BASE}{path}"),
        Some("NativeOther"),
        None,
        404,
    )
    .await;
    f.call(
        "POST",
        &format!("{BASE}{path}"),
        Some("NativeOther"),
        Some(json!({"expectedAccountId":f.other,"revision":0,"listVersion":1,"name":"Hijack"})),
        404,
    )
    .await;
    let reordered = f
        .command(
            "POST",
            &path,
            json!({"listVersion":1,"name":"Ordered list","mangaIds":[f.second,f.manga]}),
            200,
        )
        .await;
    assert_eq!(reordered["data"]["version"], 2);
    assert_eq!(reordered["data"]["mangaIds"], json!([f.second, f.manga]));
    f.command("POST", &path, json!({"listVersion":1,"name":"Stale"}), 409)
        .await;
    let before = f.revision().await;
    f.command(
        "POST",
        &path,
        json!({"listVersion":2,"name":"Do not save","mangaIds":[f.second,f.hidden]}),
        404,
    )
    .await;
    assert_eq!(f.revision().await, before);
    assert_eq!(f.get(&path).await["data"]["name"], "Ordered list");
    // Account and session locks fence every private read/write and fail promptly.
    let mut blocker = f.state.db.client().await.unwrap();
    let hold = blocker.transaction().await.unwrap();
    hold.query_one(
        "SELECT id FROM platform.users WHERE id='NativeActor' FOR NO KEY UPDATE",
        &[],
    )
    .await
    .unwrap();
    f.call(
        "GET",
        &format!("{BASE}/lists"),
        Some("NativeActor"),
        None,
        409,
    )
    .await;
    f.call(
        "POST",
        &format!("{BASE}/lists"),
        Some("NativeActor"),
        Some(json!({"expectedAccountId":f.account,"revision":before,"name":"Busy"})),
        409,
    )
    .await;
    hold.rollback().await.unwrap();
    let hold = blocker.transaction().await.unwrap();
    hold.query_one(
        "SELECT id FROM platform.sessions WHERE id='NativeActor' FOR UPDATE",
        &[],
    )
    .await
    .unwrap();
    f.call(
        "GET",
        &format!("{BASE}/lists"),
        Some("NativeActor"),
        None,
        409,
    )
    .await;
    hold.rollback().await.unwrap();
    drop(blocker);
    // Two writes with the exact same account revision cannot both win.
    let revision = f.revision().await;
    let request = |name: &str| {
        f.request("POST", &format!("{BASE}/lists"), Some("NativeActor"))
            .header("origin", &f.origin)
            .json(&json!({"expectedAccountId":f.account,"revision":revision,"name":name}))
    };
    let (a, b) = tokio::join!(
        request("Concurrent one").send(),
        request("Concurrent two").send()
    );
    let mut statuses = vec![a.unwrap().status().as_u16(), b.unwrap().status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, [200, 409]);
    let selected = f
        .get(&format!("/history/state?ids[]={}&limit=100", f.chapters[0]))
        .await;
    assert_eq!(selected["selection"], json!([f.chapters[0]]));
    assert!(selected["catalogIdentity"]["catalogNamespace"].is_string());
    // Every legacy source writer advances same revision including deletion/ABA.
    let db = f.state.db.client().await.unwrap();
    let old = f.revision().await;
    db.batch_execute("UPDATE platform.library_manga_follows SET notification=false WHERE user_id='NativeActor' AND manga_id='NativeMng1'").await.unwrap();
    assert!(f.revision().await > old);
    let before = f.revision().await;
    db.batch_execute("DELETE FROM platform.library_chapter_history WHERE user_id='NativeActor'; INSERT INTO platform.library_chapter_history(user_id,manga_id,chapter_id) VALUES('NativeActor','NativeMng1','NativeCh2');").await.unwrap();
    assert!(f.revision().await > before);
    let stale = json!({"expectedAccountId":f.account,"revision":before,"listVersion":2,"name":"Stale after ABA"});
    f.call(
        "POST",
        &format!("{BASE}{path}"),
        Some("NativeActor"),
        Some(stale),
        409,
    )
    .await;
    drop(db);
    f.command("DELETE", &path, json!({"listVersion":1}), 409)
        .await;
    f.command("DELETE", &path, json!({"listVersion":2}), 200)
        .await;
    f.call(
        "GET",
        &format!("{BASE}{path}"),
        Some("NativeActor"),
        None,
        404,
    )
    .await;
    let before = f.get("/history/feed").await["total"].clone();
    f.command("DELETE", &follow, json!({}), 200).await;
    assert_eq!(f.get("/follows").await["total"], 0);
    assert_eq!(f.get("/history/feed").await["total"], before);
    let row=f.state.db.client().await.unwrap().query_one("SELECT follows,(SELECT count(*) FROM platform.protocol_identity_bindings),(SELECT count(*) FROM account_permissions) FROM platform.catalog_manga_counters WHERE manga_id='NativeMng1'",&[]).await.unwrap();
    assert_eq!(row.get::<_, i64>(0), 0);
    assert_eq!(row.get::<_, i64>(1), 0);
    assert_eq!(row.get::<_, i64>(2), 0);
    // Unverified private writes still work, restrictive accounts cannot write.
    let r = f
        .call(
            "GET",
            "/api/v1/account/me",
            Some("NativeUnverified"),
            None,
            200,
        )
        .await;
    let id = r["data"]["id"].as_str().unwrap();
    f.call(
        "POST",
        &format!("{BASE}/lists"),
        Some("NativeUnverified"),
        Some(json!({"expectedAccountId":id,"revision":0,"name":"Unverified private"})),
        200,
    )
    .await;
    for user in ["NativeRestricted", "NativeBanned", "NativeNoPerm"] {
        let r = f
            .call("GET", "/api/v1/account/me", Some(user), None, 200)
            .await;
        f.call(
            "POST",
            &format!("{BASE}/lists"),
            Some(user),
            Some(json!({"expectedAccountId":r["data"]["id"],"revision":0,"name":"Denied"})),
            403,
        )
        .await;
    }
}
