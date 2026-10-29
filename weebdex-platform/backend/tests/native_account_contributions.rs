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
                "author.create",
                "scanlation_group.create",
                "manga.title.edit",
                "manga.description.edit",
                "manga.cover.edit",
                "manga.relation.edit",
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

fn png() -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        13,
        17,
        image::Rgb([71, 28, 149]),
    ))
    .write_to(&mut bytes, image::ImageFormat::Png)
    .unwrap();
    bytes.into_inner()
}
async fn multipart(
    f: &Fixture,
    path: &str,
    fields: &[(&str, String)],
    account: &str,
    status: u16,
) -> Value {
    let boundary = "native-account-fixture";
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"page.png\"\r\nContent-Type: image/png\r\n\r\n").as_bytes());
    body.extend(png());
    body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
    let request = f
        .http
        .post(format!("{}{path}", f.origin))
        .header("origin", &f.origin)
        .header(
            "cookie",
            format!(
                "{}=synthetic-cookie-{account}",
                f.state.config.session_cookie_name
            ),
        )
        .header("x-weebdex-expected-account", account)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body);
    f.response(request, status).await
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL fixture"]
async fn native_contributors_use_native_cores_without_signer_or_binding() {
    let f = fixture().await;
    let user = "NativeActor";
    let context = f
        .call(
            "GET",
            "/api/v1/account/contribution-context",
            Some(user),
            None,
            200,
        )
        .await;
    assert_eq!(context["nostr"]["authentication"], "native-session");
    assert_eq!(context["nostr"]["nativeSessionAuthority"], true);
    assert!(context["nostr"].get("pubkey").is_none());
    f.call(
        "GET",
        "/api/v1/account/contribution-context",
        Some("NativeUnverified"),
        None,
        403,
    )
    .await;
    let input = json!({"expectedAccountId":f.account,"title":{"en":"Native contribution"},"description":{"en":"Native description"},"originalLanguage":"en","status":"ongoing","contentRating":"safe","authors":[],"artists":[]});
    f.call(
        "POST",
        "/api/v1/account/manga",
        Some("NativeNoPerm"),
        Some(json!({"expectedAccountId":f.account})),
        409,
    )
    .await;
    let created = f
        .call(
            "POST",
            "/api/v1/account/manga",
            Some(user),
            Some(input.clone()),
            200,
        )
        .await;
    let title = created["data"]["id"].as_str().unwrap();
    assert_eq!(created["data"]["attributes"]["state"], "draft");
    assert_eq!(created["nostr"]["accountId"], f.account);
    let edit = format!("/api/v1/account/manga/{title}");
    f.call("PUT",&edit,Some(user),Some(json!({"expectedAccountId":f.account,"version":1,"title":{"en":"Changed native title"}})),200).await;
    f.call(
        "PUT",
        &edit,
        Some(user),
        Some(json!({"expectedAccountId":f.account,"version":1,"title":{"en":"Stale title"}})),
        409,
    )
    .await;
    let draft = format!("/api/v1/account/manga/draft/{title}");
    f.call("GET", &draft, Some("NativeOther"), None, 403).await;
    let read = f.call("GET", &draft, Some(user), None, 200).await;
    assert_eq!(
        read["data"]["attributes"]["title"]["en"],
        "Changed native title"
    );
    let list = f
        .call(
            "GET",
            &format!(
                "/api/v1/account/manga/draft?expectedAccountId={}",
                f.account
            ),
            Some(user),
            None,
            200,
        )
        .await;
    assert_eq!(list["total"], 1);
    let cover_path = format!(
        "/api/v1/account/cover/{title}?expectedAccountId={}",
        f.account
    );
    let cover = multipart(
        &f,
        &cover_path,
        &[
            ("expectedVersion", "2".into()),
            ("expectedAccountId", f.account.clone()),
            ("locale", "en".into()),
            ("main", "true".into()),
        ],
        user,
        201,
    )
    .await;
    assert_eq!(cover["nostr"]["authentication"], "native-session");
    let current = f.call("GET", &draft, Some(user), None, 200).await;
    let version = current["data"]["attributes"]["version"].clone();
    let submitted = f
        .call(
            "POST",
            &format!("{draft}/commit"),
            Some(user),
            Some(json!({"version":version,"expectedAccountId":f.account})),
            201,
        )
        .await;
    f.call("POST",&format!("{draft}/publish"),Some(user),Some(json!({"version":submitted["data"]["attributes"]["version"],"expectedAccountId":f.account})),403).await;
    let db = f.state.db.client().await.unwrap();
    db.batch_execute("UPDATE platform.users SET roles='[\"moderator\"]'::jsonb,permissions=permissions || '[\"manga.state.update\"]'::jsonb WHERE id='NativeOther'").await.unwrap();
    drop(db);
    let published=f.call("POST",&format!("{draft}/publish"),Some("NativeOther"),Some(json!({"version":submitted["data"]["attributes"]["version"],"expectedAccountId":f.other})),200).await;
    assert_eq!(published["data"]["attributes"]["state"], "published");
    let author = f
        .call(
            "POST",
            "/api/v1/account/author",
            Some(user),
            Some(json!({"name":"Native author","expectedAccountId":f.account})),
            200,
        )
        .await;
    assert_eq!(author["nostr"]["authentication"], "native-session");
    // Upload storage and commit: browser owner remains uploader, source config is
    // absent, so the core legitimately publishes only this local chapter.
    let upload = |suffix: &str| {
        format!(
            "/api/v1/account/upload{suffix}?expectedAccountId={}",
            f.account
        )
    };
    let begin = f
        .call(
            "POST",
            &upload("/begin"),
            Some(user),
            Some(json!({"manga":title,"groups":[],"expectedAccountId":f.account})),
            200,
        )
        .await;
    let session = begin["id"].as_str().unwrap();
    f.call(
        "POST",
        &upload("/begin"),
        Some(user),
        Some(json!({"manga":title,"groups":[],"expectedAccountId":f.account})),
        409,
    )
    .await;
    let attached = multipart(&f, &upload(&format!("/{session}")), &[], user, 200).await;
    let file = attached["data"][0]["id"].as_str().unwrap();
    let snapshot = format!(
        "/api/v1/account/uploads/{session}?expectedAccountId={}",
        f.account
    );
    let staged = f.call("GET", &snapshot, Some(user), None, 200).await;
    assert_eq!(staged["session"]["files"][0]["id"], file);
    assert_eq!(staged["nostr"]["authentication"], "native-session");
    f.call("GET", &snapshot, Some("NativeOther"), None, 409)
        .await;
    let save = json!({"version":staged["session"]["version"],"draft":{"language":"en","title":"Saved chapter"},"pageOrder":[file]});
    f.call("PATCH", &snapshot, Some(user), Some(save.clone()), 200)
        .await;
    f.call("PATCH", &snapshot, Some(user), Some(save), 409)
        .await;
    let commit = json!({"chapterDraft":{"volume":null,"chapter":"1","title":"Saved chapter","translatedLanguage":"en"},"pageOrder":[file],"termsAccepted":true});
    let committed = f
        .call(
            "POST",
            &upload(&format!("/{session}/commit")),
            Some(user),
            Some(commit.clone()),
            200,
        )
        .await;
    assert_eq!(committed["type"], "chapter");
    assert_eq!(committed["nostr"]["accountId"], f.account);
    let replay = f
        .call(
            "POST",
            &upload(&format!("/{session}/commit")),
            Some(user),
            Some(commit),
            200,
        )
        .await;
    assert_eq!(replay["id"], committed["id"]);
    let recovered = f.call("GET", &snapshot, Some(user), None, 200).await;
    assert_eq!(recovered["session"]["state"], "committed");
    assert_eq!(recovered["session"]["receipt"]["id"], committed["id"]);
    // Same core storage proves the browser user, not a synthesized signing
    // persona, owns every original page and published local chapter.
    let resolver = PostgresPublicIdResolver::new(f.state.db.clone());
    let chapter = resolver
        .resolve(
            ResourceKind::Chapter,
            &PublicUuid::parse(committed["id"].as_str().unwrap()).unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    let db = f.state.db.client().await.unwrap();
    assert_eq!(
        db.query_one(
            "SELECT uploader_id FROM platform.catalog_chapters WHERE id=$1",
            &[&chapter.native_id]
        )
        .await
        .unwrap()
        .get::<_, String>(0),
        user
    );
    drop(db);
    let invalid_commit = json!({"chapterDraft":{"volume":null,"chapter":"2","title":"Different command","translatedLanguage":"en"},"pageOrder":[file],"termsAccepted":true});
    f.call(
        "POST",
        &upload(&format!("/{session}/commit")),
        Some(user),
        Some(invalid_commit),
        409,
    )
    .await;
    let duplicate = format!(
        r#"{{"expectedAccountId":"{}","expectedAccountId":"{}","title":{{"en":"duplicate"}},"originalLanguage":"en","status":"ongoing","contentRating":"safe"}}"#,
        f.account, f.account
    );
    f.response(
        f.request("POST", "/api/v1/account/manga", Some(user))
            .header("origin", &f.origin)
            .body(duplicate),
        400,
    )
    .await;
    assert_eq!(f.counts().await.2, 0);
    assert_eq!(f.counts().await.3, 0);
}
