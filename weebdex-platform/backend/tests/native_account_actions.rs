//! Isolated real HTTP tests for ordinary login over canonical UUIDs. Synthetic
//! cookie sessions only; no key, enrollment, binding or protocol write is needed.
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
    _server: Server,
}
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
        _server: server,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires isolated scripts/run-auth-tests.sh PostgreSQL"]
async fn ordinary_login_owns_canonical_ratings_follows_reports_without_protocol_identity() {
    let f = fixture().await;
    let read = format!("/api/v1/account/rating?manga[]={}", f.manga);
    let write = format!("/api/v1/account/rating/{}", f.manga);
    let me = f
        .call("GET", "/api/v1/account/me", Some("NativeActor"), None, 200)
        .await;
    assert_eq!(me["data"]["id"], f.account);
    assert_eq!(me["data"]["attributes"]["username"], "NativeActor");
    assert_eq!(me["nostr"]["identity"], "native-session-account");
    assert_eq!(me["nostr"]["nativeSessionAuthority"], true);
    assert_eq!(f.counts().await, (0, 0, 0, 0));

    // No implicit signed/browser fallback, no bearer inheritance, no recovery.
    f.call("GET", "/api/v1/account/me", None, None, 400).await;
    for (user, status) in [
        ("NativeDisabled", 401),
        ("NativeExpired", 401),
        ("NativeRevoked", 401),
    ] {
        f.call("GET", "/api/v1/account/me", Some(user), None, status)
            .await;
        f.call("POST", &write, Some(user), Some(f.rating(7, 0)), status)
            .await;
    }
    f.call(
        "GET",
        "/api/v1/account/me",
        Some("NativeRecovery"),
        None,
        403,
    )
    .await;
    f.call(
        "POST",
        &write,
        Some("NativeRecovery"),
        Some(f.rating(7, 0)),
        401,
    )
    .await;
    for user in ["NativeBanned", "NativeRestricted"] {
        let me = f
            .call("GET", "/api/v1/account/me", Some(user), None, 200)
            .await;
        assert_eq!(me["data"]["attributes"]["permissions"], json!([]));
        f.call("POST", &write, Some(user), Some(f.rating(7, 0)), 403)
            .await;
    }
    f.call(
        "GET",
        "/api/v1/account/me",
        Some("NativeUnverified"),
        None,
        200,
    )
    .await;
    f.call("GET", &read, Some("NativeUnverified"), None, 403)
        .await;
    f.call(
        "POST",
        &write,
        Some("NativeUnverified"),
        Some(f.rating(7, 0)),
        403,
    )
    .await;
    f.call(
        "POST",
        &write,
        Some("NativeNoPerm"),
        Some(f.rating(7, 0)),
        403,
    )
    .await;
    for authorization in ["Bearer synthetic-client", "Nostr synthetic-proof", ""] {
        f.response(
            f.request("GET", "/api/v1/account/me", Some("NativeActor"))
                .header("authorization", authorization),
            403,
        )
        .await;
    }
    f.response(
        f.request("GET", "/api/v1/account/me", Some("NativeActor"))
            .header("x-weebdex-expected-account", "NativeOther"),
        400,
    )
    .await;
    f.response(
        f.request("GET", "/api/v1/account/me", None)
            .header(
                "cookie",
                format!(
                    "{}=synthetic-cookie-NativeActor",
                    f.state.config.session_cookie_name
                ),
            )
            .header("x-weebdex-expected-account", "NativeOther"),
        409,
    )
    .await;
    f.response(
        f.request("GET", "/api/v1/account/me", Some("NativeActor"))
            .header(
                "cookie",
                format!(
                    "{}=synthetic-cookie-NativeActor; {}=",
                    f.state.config.session_cookie_name, f.state.config.session_cookie_name
                ),
            ),
        401,
    )
    .await;
    // Public signed API still rejects a cookie. Merely switching URLs does not
    // grant either public signer admission or native account binding.
    let signed = f
        .request("GET", "/api/v1/user/me", Some("NativeActor"))
        .send()
        .await
        .unwrap();
    assert_eq!(signed.status(), 401);

    let bytes = serde_json::to_vec(&f.rating(8, 0)).unwrap();
    for (name, first, second) in [
        ("origin", f.origin.as_str(), "https://attacker.invalid"),
        ("sec-fetch-site", "same-origin", "cross-site"),
        ("referer", f.origin.as_str(), "https://attacker.invalid/"),
    ] {
        let mut request = f.request("POST", &write, Some("NativeActor"));
        if name != "origin" {
            request = request.header("origin", &f.origin);
        }
        f.response(
            request
                .header(name, first)
                .header(name, second)
                .body(bytes.clone()),
            403,
        )
        .await;
    }
    f.response(
        f.request("POST", &write, Some("NativeActor"))
            .header("origin", &f.origin)
            .header("sec-fetch-site", "same-origin, cross-site")
            .body(bytes.clone()),
        403,
    )
    .await;
    f.response(
        f.request("POST", &write, Some("NativeActor"))
            .body(bytes.clone()),
        403,
    )
    .await;
    f.response(
        f.request("POST", &write, Some("NativeActor"))
            .header("origin", "https://attacker.invalid")
            .body(bytes.clone()),
        403,
    )
    .await;
    f.response(
        f.request("POST", &write, Some("NativeActor"))
            .header("origin", &f.origin)
            .header("sec-fetch-site", "cross-site")
            .body(bytes.clone()),
        403,
    )
    .await;
    f.call(
        "POST",
        &write,
        Some("NativeOther"),
        Some(f.rating(8, 0)),
        409,
    )
    .await;
    for invalid in [
        json!({"rating":8,"version":0}),
        json!({"rating":8,"expectedAccountId":f.account}),
        json!({"rating":0,"version":0,"expectedAccountId":f.account}),
        json!({"rating":8,"version":-1,"expectedAccountId":f.account}),
        json!({"rating":8,"version":0,"expectedAccountId":f.account,"owner":"NativeOther"}),
    ] {
        f.call("POST", &write, Some("NativeActor"), Some(invalid), 400)
            .await;
    }
    f.call(
        "POST",
        &format!("/api/v1/account/rating/{}", f.hidden),
        Some("NativeActor"),
        Some(f.rating(8, 0)),
        404,
    )
    .await;
    f.call(
        "POST",
        &format!("/api/v1/account/rating/{}", f.other),
        Some("NativeActor"),
        Some(f.rating(8, 0)),
        404,
    )
    .await;
    assert_eq!(f.counts().await, (0, 0, 0, 0));

    let empty = f.call("GET", &read, Some("NativeActor"), None, 200).await;
    assert_eq!(empty["nostr"]["versions"][&f.manga], 0);
    let result = f
        .call(
            "POST",
            &format!("/api/v1/account/rating/{}", f.alias),
            Some("NativeActor"),
            Some(f.rating(8, 0)),
            200,
        )
        .await;
    assert_eq!(result["nostr"]["mangaId"], f.manga);
    assert_eq!(result["nostr"]["version"], 1);
    f.call(
        "POST",
        &write,
        Some("NativeActor"),
        Some(f.rating(5, 0)),
        409,
    )
    .await;
    let read_alias = f
        .call(
            "GET",
            &format!("/api/v1/account/rating?manga[]={}", f.alias),
            Some("NativeActor"),
            None,
            200,
        )
        .await;
    assert_eq!(read_alias["ratings"][&f.manga]["rating"], 8);
    assert!(read_alias["ratings"].get(&f.alias).is_none());
    let other = f.call("GET", &read, Some("NativeOther"), None, 200).await;
    assert_eq!(other["nostr"]["accountId"], f.other);
    assert_eq!(other["nostr"]["versions"][&f.manga], 0);
    assert_eq!(other["ratings"], json!({}));
    // Legacy native endpoint sees the same row, not a parallel account identity.
    let native = f
        .request("GET", "/api/rating?manga=NativeMng1", Some("NativeActor"))
        .send()
        .await
        .unwrap();
    assert_eq!(native.status(), 200);
    assert_eq!(
        native.json::<Value>().await.unwrap()["ratings"]["NativeMng1"]["rating"],
        8
    );
    let a = f
        .request("POST", &write, Some("NativeActor"))
        .header("origin", &f.origin)
        .json(&f.rating(6, 1))
        .send();
    let b = f
        .request("POST", &write, Some("NativeActor"))
        .header("origin", &f.origin)
        .json(&f.rating(7, 1))
        .send();
    let (a, b) = tokio::join!(a, b);
    let mut statuses = [a.unwrap().status().as_u16(), b.unwrap().status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, [200, 409]);
    f.call(
        "DELETE",
        &write,
        Some("NativeActor"),
        Some(json!({"version":2,"expectedAccountId":f.account})),
        200,
    )
    .await;
    assert_eq!(
        f.call("GET", &read, Some("NativeActor"), None, 200).await["ratings"],
        json!({})
    );

    // Legacy unverified private save -> canonical own collection observes the
    // same row. Verification still gates ratings/reports/social mutations.
    let saved = f
        .request(
            "POST",
            "/api/user/follows/manga/NativeMng1",
            Some("NativeUnverified"),
        )
        .header("origin", &f.origin)
        .json(&json!({"status":"reading"}))
        .send()
        .await
        .unwrap();
    assert_eq!(saved.status(), 204);
    let saved = f
        .call(
            "GET",
            "/api/v1/account/user/follows/manga?limit=1",
            Some("NativeUnverified"),
            None,
            200,
        )
        .await;
    assert_eq!(saved["total"], 1);
    assert_eq!(saved["data"][0]["id"], f.manga);
    let status_path = format!("/api/v1/account/manga/{}/follow", f.manga);
    let saved_state = f
        .call("GET", &status_path, Some("NativeUnverified"), None, 200)
        .await;
    assert_eq!(saved_state["nostr"]["followed"], true);
    f.call("DELETE",&status_path,Some("NativeUnverified"),Some(json!({"expectedAccountId":saved["nostr"]["accountId"],"version":saved_state["nostr"]["version"]})),200).await;
    for kind in ["manga", "group", "user"] {
        f.call(
            "GET",
            &format!("/api/v1/account/user/follows/{kind}"),
            Some("NativeRestricted"),
            None,
            200,
        )
        .await;
    }
    for (kind, target) in [("manga", &f.manga), ("group", &f.group), ("user", &f.other)] {
        let path = format!("/api/v1/account/{kind}/{target}/follow");
        let first = f.call("GET", &path, Some("NativeActor"), None, 200).await;
        assert_eq!(first["nostr"]["followed"], false);
        assert_eq!(first["nostr"]["version"], 0);
        let command = json!({"expectedAccountId":f.account,"version":0});
        if kind != "manga" {
            f.call(
                "POST",
                &path,
                Some("NativeUnverified"),
                Some(command.clone()),
                403,
            )
            .await;
        }

        f.call(
            "POST",
            &path,
            Some("NativeOther"),
            Some(command.clone()),
            409,
        )
        .await;
        let followed = f
            .call(
                "POST",
                &path,
                Some("NativeActor"),
                Some(command.clone()),
                200,
            )
            .await;
        assert_eq!(followed["nostr"]["followed"], true);
        assert_eq!(followed["nostr"]["version"], 1);
        let collection = format!("/api/v1/account/user/follows/{kind}?limit=1&offset=0");
        let page = f
            .call("GET", &collection, Some("NativeActor"), None, 200)
            .await;
        assert_eq!(page["total"], 1);
        assert_eq!(page["data"][0]["id"], *target);
        assert_eq!(page["nostr"]["authentication"], "native-session");
        assert_eq!(page["nostr"]["accountId"], f.account);
        assert_eq!(
            f.call("GET", &collection, Some("NativeOther"), None, 200)
                .await["total"],
            0
        );
        f.call("GET", &collection, None, None, 400).await;
        f.call("GET", &collection, Some("NativeUnverified"), None, 200)
            .await;
        f.call(
            "GET",
            &format!("/api/v1/account/user/follows/{kind}?limit=101"),
            Some("NativeActor"),
            None,
            400,
        )
        .await;
        if kind == "manga" {
            let feed = f
                .call(
                    "GET",
                    "/api/v1/account/user/follows/manga/feed?limit=1",
                    Some("NativeActor"),
                    None,
                    200,
                )
                .await;
            assert_eq!(feed["nostr"]["authentication"], "native-session");
            assert_eq!(feed["total"], 0);
        }

        f.call("DELETE", &path, Some("NativeActor"), Some(command), 409)
            .await;
        f.call(
            "DELETE",
            &path,
            Some("NativeActor"),
            Some(json!({"expectedAccountId":f.account,"version":1})),
            200,
        )
        .await;
    }
    f.call(
        "POST",
        &format!("/api/v1/account/user/{}/follow", f.account),
        Some("NativeActor"),
        Some(json!({"expectedAccountId":f.account,"version":0})),
        400,
    )
    .await;

    let reasons = f
        .call(
            "GET",
            "/api/v1/account/report/reasons/manga",
            Some("NativeActor"),
            None,
            200,
        )
        .await;
    assert_eq!(
        reasons["nostr"],
        json!({"scope":"instance-account","authentication":"native-session","accountId":f.account})
    );
    assert!(
        reasons["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == f.reason)
    );
    f.call(
        "GET",
        "/api/v1/account/report/reasons/manga",
        Some("NativeNoPerm"),
        None,
        403,
    )
    .await;
    f.call(
        "POST",
        "/api/v1/account/report",
        Some("NativeOther"),
        Some(f.report()),
        409,
    )
    .await;
    f.call(
        "POST",
        "/api/v1/account/report",
        Some("NativeNoPerm"),
        Some(f.report()),
        409,
    )
    .await;
    let mut changed = f.report();
    changed["expectedReasonVersion"] = json!(2);
    f.call(
        "POST",
        "/api/v1/account/report",
        Some("NativeActor"),
        Some(changed),
        409,
    )
    .await;
    let mut hidden = f.report();
    hidden["objectId"] = json!(f.hidden);
    f.call(
        "POST",
        "/api/v1/account/report",
        Some("NativeActor"),
        Some(hidden),
        404,
    )
    .await;
    f.call(
        "POST",
        "/api/v1/account/report",
        Some("NativeActor"),
        Some(f.report()),
        201,
    )
    .await;
    f.call(
        "POST",
        "/api/v1/account/report",
        Some("NativeActor"),
        Some(f.report()),
        409,
    )
    .await;
    assert_eq!(f.counts().await, (1, 1, 0, 0));
    let row = f.state.db.client().await.unwrap().query_one("SELECT submitter_id,(SELECT count(*) FROM platform.report_audit WHERE operation='create') FROM platform.reports",&[]).await.unwrap();
    assert_eq!(row.get::<_, String>(0), "NativeActor");
    assert_eq!(row.get::<_, i64>(1), 1);

    // Deterministic in-flight authority fences: an account/credential mutation
    // already holding its row must not allow private reads or queued writes
    // based on a previously captured session. NOWAIT yields safe retry, not leak.
    let mut db = f.state.db.client().await.unwrap();
    let tx = db.transaction().await.unwrap();
    tx.query_one(
        "SELECT id FROM platform.users WHERE id='NativeActor' FOR UPDATE",
        &[],
    )
    .await
    .unwrap();
    f.call("GET", &read, Some("NativeActor"), None, 409).await;
    f.call(
        "POST",
        &write,
        Some("NativeActor"),
        Some(f.rating(9, 3)),
        409,
    )
    .await;
    tx.rollback().await.unwrap();
    let tx = db.transaction().await.unwrap();
    tx.query_one(
        "SELECT id FROM platform.sessions WHERE id='NativeActor' FOR UPDATE",
        &[],
    )
    .await
    .unwrap();
    f.call("GET", &read, Some("NativeActor"), None, 409).await;
    tx.execute(
        "UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE id='NativeActor'",
        &[],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    f.call("GET", &read, Some("NativeActor"), None, 401).await;
    f.call(
        "POST",
        &write,
        Some("NativeActor"),
        Some(f.rating(9, 3)),
        401,
    )
    .await;
    let value: i64 = db
        .query_one(
            "SELECT count(*) FROM platform.library_manga_ratings WHERE rating IS NOT NULL",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(value, 0);
    assert_eq!(f.counts().await, (1, 1, 0, 0));
}
