//! Real HTTP and PostgreSQL coverage of ordinary profile/settings/contribution adapters.
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
            json!([
                "self.preferences.update",
                "self.user.update",
                "manga.create",
                "chapter.create",
                "self.chapter.update",
                "report.create"
            ])
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

const PROFILE: &str = "/api/v1/account/user";
const SETTINGS: &str = "/api/v1/account/settings";
#[tokio::test]
#[ignore = "requires isolated PostgreSQL fixture"]
async fn normal_profile_preferences_and_directory_share_native_storage() {
    let f = fixture().await;
    let user = "NativeActor";
    let path = format!("{PROFILE}/{}/profile", f.account);
    let read_path = format!("{path}?expectedAccountId={}", f.account);
    let before = f.call("GET", &read_path, Some(user), None, 200).await;
    assert_eq!(before["nostr"]["authentication"], "native-session");
    let command = json!({"expectedAccountId":f.account,"expectedUserId":f.account,"version":before["data"]["attributes"]["version"],"description":"Ordinary profile","website":"https://example.invalid/about"});
    for (header, value, expected) in [
        ("authorization", "Bearer invalid", 403),
        ("origin", "https://wrong.invalid", 403),
        ("x-weebdex-expected-account", "NativeOther", 400),
    ] {
        f.response(
            f.request("PUT", &path, Some(user))
                .header("origin", &f.origin)
                .header(header, value)
                .json(&command),
            expected,
        )
        .await;
    }
    f.call(
        "PUT",
        &path,
        Some("NativeOther"),
        Some(command.clone()),
        409,
    )
    .await;
    let after = f
        .call("PUT", &path, Some(user), Some(command.clone()), 200)
        .await;
    assert_eq!(
        after["data"]["attributes"]["description"],
        "Ordinary profile"
    );
    assert_eq!(after["data"]["nostr"]["authentication"], "native-session");
    f.call("PUT", &path, Some(user), Some(command.clone()), 409)
        .await;
    for bad in [
        json!({"expectedAccountId":f.account,"expectedUserId":f.account,"version":2,"description":null}),
        json!({"expectedAccountId":f.account,"expectedUserId":f.account,"version":2,"email":"not-permitted"}),
    ] {
        f.call("PUT", &path, Some(user), Some(bad), 400).await;
    }
    let other_path = format!(
        "{PROFILE}/{}/profile?expectedAccountId={}",
        f.other, f.account
    );
    f.call("GET", &other_path, Some(user), None, 403).await;
    let resolver = PostgresPublicIdResolver::new(f.state.db.clone());
    let unverified = resolver
        .preferred(ResourceKind::User, "NativeUnverified")
        .await
        .unwrap()
        .unwrap()
        .to_string();
    let uvpath = format!("{PROFILE}/{unverified}/profile");
    f.call("PUT",&uvpath,Some("NativeUnverified"),Some(json!({"expectedAccountId":unverified,"expectedUserId":unverified,"version":1,"description":"Unverified own profile"})),200).await;
    let db = f.state.db.client().await.unwrap();
    db.batch_execute("UPDATE platform.users SET preferences=preferences || '{\"private_unknown\":{\"keep\":true},\"uploaders\":[\"NativeDisabled\",\"UnavailableLegacy\"]}'::jsonb WHERE id='NativeActor'; INSERT INTO platform.library_user_follows(user_id,target_id) VALUES('NativeActor','NativeOther');").await.unwrap();
    drop(db);
    let settings = f.call("GET", SETTINGS, Some(user), None, 200).await;
    assert_eq!(settings["nostr"]["unavailableFilterCounts"]["uploaders"], 2);
    assert!(settings["settings"].get("private_unknown").is_none());
    let cmd = json!({"expectedAccountId":f.account,"updatedAt":settings["updatedAt"],"settings":{"version":settings["settings"]["version"],"uploaders":[f.other],"data_saver":true}});
    let saved = f
        .call("POST", SETTINGS, Some(user), Some(cmd.clone()), 200)
        .await;
    assert_eq!(saved["settings"]["uploaders"], json!([f.other]));
    assert_eq!(saved["nostr"]["preservedUnavailableFilterValues"], true);
    f.call("POST", SETTINGS, Some(user), Some(cmd), 409).await;
    let db = f.state.db.client().await.unwrap();
    let stored: Value = db
        .query_one(
            "SELECT preferences FROM platform.users WHERE id='NativeActor'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(stored["private_unknown"], json!({"keep":true}));
    assert_eq!(
        stored["uploaders"],
        json!(["NativeOther", "NativeDisabled", "UnavailableLegacy"])
    );
    assert_eq!(db.query_one("SELECT count(*) FROM platform.library_user_follows WHERE user_id='NativeActor' AND target_id='NativeOther'",&[]).await.unwrap().get::<_,i64>(0),1);
    drop(db);
    f.call("POST",SETTINGS,Some(user),Some(json!({"expectedAccountId":f.account,"updatedAt":saved["updatedAt"],"settings":{"version":saved["settings"]["version"],"uploaders":[f.account]}})),400).await;
    let us = f
        .call("GET", SETTINGS, Some("NativeUnverified"), None, 200)
        .await;
    f.call("POST",SETTINGS,Some("NativeUnverified"),Some(json!({"expectedAccountId":unverified,"updatedAt":us["updatedAt"],"settings":{"version":us["settings"]["version"],"data_saver":true}})),200).await;
    for who in [
        "NativeRecovery",
        "NativeRevoked",
        "NativeExpired",
        "NativeDisabled",
    ] {
        f.call(
            "GET",
            SETTINGS,
            Some(who),
            None,
            if who == "NativeRecovery" { 403 } else { 401 },
        )
        .await;
    }
    let directory = f
        .call(
            "GET",
            "/api/v1/account/user?limit=100&username=Native",
            Some(user),
            None,
            200,
        )
        .await;
    assert_eq!(directory["nostr"]["authentication"], "native-session");
    assert!(!directory.to_string().contains("NativeDisabled"));
    assert!(!directory.to_string().contains("private_unknown"));
    f.call(
        "GET",
        "/api/v1/account/user?limit=101",
        Some(user),
        None,
        400,
    )
    .await;
    f.call(
        "GET",
        "/api/v1/account/user?offset=10001",
        Some(user),
        None,
        400,
    )
    .await;
    f.call(
        "GET",
        "/api/v1/account/user?username=a&username=b",
        Some(user),
        None,
        400,
    )
    .await;
    // Recovery establishes a metadata-only CAS barrier. A delayed original
    // block request can no longer arrive after a read and contradict it.
    let before_barrier = f.call("GET", SETTINGS, Some(user), None, 200).await;
    let barrier = json!({"expectedAccountId":f.account,"targetId":f.other,"commandId":"synthetic-block-attempt-1","expectedVersion":before_barrier["settings"]["version"],"expectedUpdatedAt":before_barrier["updatedAt"]});
    let reconciled = f
        .call(
            "POST",
            "/api/v1/account/settings/reconcile-block",
            Some(user),
            Some(barrier.clone()),
            200,
        )
        .await;
    assert_eq!(reconciled["nostr"]["barrierEstablished"], true);
    assert_eq!(
        reconciled["nostr"]["commandId"],
        "synthetic-block-attempt-1"
    );
    assert_eq!(reconciled["nostr"]["changed"], false);
    assert_eq!(
        reconciled["settings"]["version"].as_i64().unwrap(),
        before_barrier["settings"]["version"].as_i64().unwrap() + 1
    );
    let mut original = before_barrier["settings"].clone();
    original["version"] = reconciled["settings"]["version"].clone();
    assert_eq!(original, reconciled["settings"]);
    f.call("POST",SETTINGS,Some(user),Some(json!({"expectedAccountId":f.account,"updatedAt":before_barrier["updatedAt"],"settings":{"version":before_barrier["settings"]["version"],"uploaders":[]}})),409).await;
    let repeated = f
        .call(
            "POST",
            "/api/v1/account/settings/reconcile-block",
            Some(user),
            Some(barrier.clone()),
            200,
        )
        .await;
    assert_eq!(
        repeated["settings"]["version"],
        reconciled["settings"]["version"]
    );
    assert_eq!(repeated["nostr"]["nativeStorageUnchanged"], true);
    f.call(
        "POST",
        "/api/v1/account/settings/reconcile-block",
        Some("NativeOther"),
        Some(barrier.clone()),
        409,
    )
    .await;
    let mut self_target = barrier.clone();
    self_target["targetId"] = json!(f.account);
    f.call(
        "POST",
        "/api/v1/account/settings/reconcile-block",
        Some(user),
        Some(self_target),
        400,
    )
    .await;
    let mut future = barrier;
    future["expectedVersion"] = json!(reconciled["settings"]["version"].as_i64().unwrap() + 1);
    f.call(
        "POST",
        "/api/v1/account/settings/reconcile-block",
        Some(user),
        Some(future),
        409,
    )
    .await;
    // An unrelated profile edit changes the shared users.updated_at but not
    // preferences.version. Recovery must use the timestamp from its explicit
    // current-state read, while retaining the original command/account/target
    // and preference version. It must not apply the original block/unblock.
    {
        let captured = f.call("GET", SETTINGS, Some(user), None, 200).await;
        let delayed_original = json!({
            "expectedAccountId": f.account,
            "updatedAt": captured["updatedAt"],
            "settings": {
                "version": captured["settings"]["version"],
                "uploaders": []
            }
        });
        let db = f.state.db.client().await.unwrap();
        let stored_before: Value = db
            .query_one(
                "SELECT preferences FROM platform.users WHERE id='NativeActor'",
                &[],
            )
            .await
            .unwrap()
            .get(0);
        drop(db);
        let profile_before = f.call("GET", &read_path, Some(user), None, 200).await;
        let edited = f
            .call(
                "PUT",
                &path,
                Some(user),
                Some(json!({
                    "expectedAccountId": f.account,
                    "expectedUserId": f.account,
                    "version": profile_before["data"]["attributes"]["version"],
                    "description": "Unrelated profile change while block outcome is unknown"
                })),
                200,
            )
            .await;
        let checked = f.call("GET", SETTINGS, Some(user), None, 200).await;
        assert_eq!(checked["settings"], captured["settings"]);
        assert_ne!(checked["updatedAt"], captured["updatedAt"]);
        let original_barrier = json!({
            "expectedAccountId": f.account,
            "targetId": f.other,
            "commandId": "synthetic-profile-timestamp-block-attempt",
            "expectedVersion": captured["settings"]["version"],
            "expectedUpdatedAt": captured["updatedAt"]
        });
        f.call(
            "POST",
            "/api/v1/account/settings/reconcile-block",
            Some(user),
            Some(original_barrier.clone()),
            409,
        )
        .await;
        let mut checked_barrier = original_barrier.clone();
        checked_barrier["expectedUpdatedAt"] = checked["updatedAt"].clone();
        // A second profile edit after Check invalidates even that fresh CAS.
        // Nothing may silently refresh and resubmit the recovery confirmation.
        f.call(
            "PUT",
            &path,
            Some(user),
            Some(json!({
                "expectedAccountId": f.account,
                "expectedUserId": f.account,
                "version": edited["data"]["attributes"]["version"],
                "description": "Another unrelated edit after the explicit status check"
            })),
            200,
        )
        .await;
        f.call(
            "POST",
            "/api/v1/account/settings/reconcile-block",
            Some(user),
            Some(checked_barrier),
            409,
        )
        .await;
        let latest = f.call("GET", SETTINGS, Some(user), None, 200).await;
        assert_eq!(latest["settings"], captured["settings"]);
        assert_ne!(latest["updatedAt"], checked["updatedAt"]);
        let mut fresh_barrier = original_barrier.clone();
        fresh_barrier["expectedUpdatedAt"] = latest["updatedAt"].clone();
        let resolved = f
            .call(
                "POST",
                "/api/v1/account/settings/reconcile-block",
                Some(user),
                Some(fresh_barrier),
                200,
            )
            .await;
        assert_eq!(resolved["nostr"]["authentication"], "native-session");
        assert_eq!(resolved["nostr"]["accountId"], f.account);
        assert_eq!(resolved["nostr"]["targetId"], f.other);
        assert_eq!(
            resolved["nostr"]["commandId"],
            original_barrier["commandId"]
        );
        assert_eq!(
            resolved["nostr"]["expectedVersion"],
            captured["settings"]["version"]
        );
        assert_eq!(resolved["nostr"]["barrierEstablished"], true);
        assert_eq!(resolved["nostr"]["mutation"], "version-barrier-only");
        assert_eq!(resolved["nostr"]["changed"], false);
        assert_eq!(
            resolved["settings"]["version"].as_i64().unwrap(),
            captured["settings"]["version"].as_i64().unwrap() + 1
        );
        let mut projected_before = captured["settings"].clone();
        projected_before["version"] = resolved["settings"]["version"].clone();
        assert_eq!(projected_before, resolved["settings"]);
        f.call("POST", SETTINGS, Some(user), Some(delayed_original), 409)
            .await;
        // Even borrowing the current timestamp cannot rescue the old revision.
        f.call(
            "POST",
            SETTINGS,
            Some(user),
            Some(json!({
                "expectedAccountId": f.account,
                "updatedAt": resolved["updatedAt"],
                "settings": {
                    "version": captured["settings"]["version"],
                    "uploaders": []
                }
            })),
            409,
        )
        .await;
        let db = f.state.db.client().await.unwrap();
        let stored_after: Value = db
            .query_one(
                "SELECT preferences FROM platform.users WHERE id='NativeActor'",
                &[],
            )
            .await
            .unwrap()
            .get(0);
        let mut expected_stored = stored_before;
        expected_stored["version"] = resolved["settings"]["version"].clone();
        assert_eq!(stored_after, expected_stored);
        assert_eq!(
            stored_after["uploaders"],
            json!(["NativeOther", "NativeDisabled", "UnavailableLegacy"])
        );
        drop(db);
    }
    // Two explicit same-version settings writes cannot both win, and the
    // native account lock makes a concurrent revocation/write fail closed.
    let current = f.call("GET", SETTINGS, Some(user), None, 200).await;
    let command = |field: &str, value: bool| json!({"expectedAccountId":f.account,"updatedAt":current["updatedAt"],"settings":{"version":current["settings"]["version"],field:value}});
    let left = f
        .request("POST", SETTINGS, Some(user))
        .header("origin", &f.origin)
        .json(&command("data_saver", false))
        .send();
    let right = f
        .request("POST", SETTINGS, Some(user))
        .header("origin", &f.origin)
        .json(&command("show_safe", false))
        .send();
    let (left, right) = tokio::join!(left, right);
    let mut status = vec![
        left.unwrap().status().as_u16(),
        right.unwrap().status().as_u16(),
    ];
    status.sort();
    assert_eq!(status, vec![200, 409]);
    let mut db = f.state.db.client().await.unwrap();
    let lock = db.transaction().await.unwrap();
    lock.query_one(
        "SELECT id FROM platform.users WHERE id='NativeActor' FOR NO KEY UPDATE",
        &[],
    )
    .await
    .unwrap();
    f.call("GET", &read_path, Some(user), None, 409).await;
    lock.rollback().await.unwrap();
    drop(db);
    // Duplicate authority bytes are refused, rather than accepted after Value
    // normalization silently selects one of them.
    let duplicate = format!(
        r#"{{"expectedAccountId":"{}","expectedAccountId":"{}","expectedUserId":"{}","version":2,"description":"bad"}}"#,
        f.account, f.account, f.account
    );
    f.response(
        f.request("PUT", &path, Some(user))
            .header("origin", &f.origin)
            .body(duplicate),
        400,
    )
    .await;
    assert_eq!(f.counts().await.2, 0);
    assert_eq!(f.counts().await.3, 0);
}
