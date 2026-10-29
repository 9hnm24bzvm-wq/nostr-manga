//! Real HTTP/PostgreSQL regression for author feedback, voluntary teams and public credits.
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
        let url = response.url().to_string();
        let value: Value = response.json().await.unwrap();
        assert_eq!(status, expected, "{url}: {value}");
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
async fn native_team_feedback_and_credits_keep_narrow_authority() {
    let f = fixture().await;
    let db = f.state.db.client().await.unwrap();
    db.batch_execute("UPDATE platform.users SET permissions=permissions || '[\"self.scanlation_group.update\"]'::jsonb WHERE id='NativeActor'").await.unwrap();
    drop(db);
    let resolver = PostgresPublicIdResolver::new(f.state.db.clone());
    let outsider = resolver
        .preferred(ResourceKind::User, "NativeNoPerm")
        .await
        .unwrap()
        .unwrap()
        .to_string();
    let before = f.counts().await;
    async fn group(f: &Fixture, name: &str) -> String {
        f.call(
            "POST",
            "/api/v1/account/group",
            Some("NativeActor"),
            Some(json!({"name":name,"expectedAccountId":f.account})),
            200,
        )
        .await["data"]["id"]
            .as_str()
            .unwrap()
            .to_owned()
    }
    async fn invite(f: &Fixture, g: &str) -> Value {
        f.call("POST",&format!("/api/v1/account/group/{g}/requests"),Some("NativeActor"),Some(json!({"kind":"invitation","recipient":f.other,"version":1,"message":"Synthetic member invite","expectedAccountId":f.account})),200).await
    }
    async fn decide(
        f: &Fixture,
        g: &str,
        r: &Value,
        action: &str,
        user: &str,
        account: &str,
        status: u16,
    ) -> Value {
        f.call(
            "POST",
            &format!(
                "/api/v1/account/group/{g}/requests/{}/decision",
                r["id"].as_str().unwrap()
            ),
            Some(user),
            Some(json!({"action":action,"version":r["version"],"expectedAccountId":account})),
            status,
        )
        .await
    }
    f.call(
        "POST",
        "/api/v1/account/group",
        Some("NativeActor"),
        Some(
            json!({"name":"No forced bootstrap","members":[f.other],"expectedAccountId":f.account}),
        ),
        400,
    )
    .await;
    let g = group(&f, "Workflow team").await;
    let refusal=f.call("PUT",&format!("/api/v1/account/group/{g}"),Some("NativeActor"),Some(json!({"version":1,"leader":f.account,"officers":[],"members":[f.other],"expectedAccountId":f.account})),409).await;
    assert!(refusal.to_string().contains("Invite new members"));
    let native_group = resolver
        .resolve(ResourceKind::Group, &PublicUuid::parse(&g).unwrap())
        .await
        .unwrap()
        .unwrap()
        .native_id;
    let legacy = f
        .request(
            "PUT",
            &format!("/api/group/{native_group}"),
            Some("NativeActor"),
        )
        .header("origin", &f.origin)
        .json(&json!({"version":1,"leader":"NativeActor","officers":[],"members":["NativeOther"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(legacy.status(), 409);
    assert!(legacy.text().await.unwrap().contains("Invite new members"));
    f.call(
        "GET",
        &format!("/api/v1/account/group/{g}/team?offset=1"),
        Some("NativeActor"),
        None,
        400,
    )
    .await;
    let page = f
        .call(
            "GET",
            &format!("/api/v1/account/group/{g}/team?offset=100"),
            Some("NativeActor"),
            None,
            200,
        )
        .await;
    assert_eq!(page["requests"]["offset"], 100);
    assert_eq!(page["requests"]["data"], json!([]));

    let mine = f
        .call(
            "GET",
            "/api/v1/account/groups",
            Some("NativeActor"),
            None,
            200,
        )
        .await;
    assert!(
        mine["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == g && r["role"] == "leader" && r["version"] == 1)
    );
    assert!(mine["data"].as_array().unwrap().len() <= 100);
    f.call("POST",&format!("/api/v1/account/group/{g}/requests"),Some("NativeOther"),Some(json!({"kind":"invitation","recipient":f.account,"version":1,"expectedAccountId":f.other})),403).await;
    f.call("POST",&format!("/api/v1/account/group/{g}/requests"),Some("NativeActor"),Some(json!({"kind":"invitation","recipient":f.other,"version":1,"role":"officer","expectedAccountId":f.account})),400).await;
    let r = invite(&f, &g).await;
    assert_eq!(r["role"], "member");
    f.call("POST",&format!("/api/v1/account/group/{g}/requests"),Some("NativeActor"),Some(json!({"kind":"invitation","recipient":f.other,"version":1,"expectedAccountId":f.account})),409).await;
    let inbox = f
        .call(
            "GET",
            "/api/v1/account/group-requests",
            Some("NativeOther"),
            None,
            200,
        )
        .await;
    assert_eq!(inbox["data"].as_array().unwrap().len(), 1);
    let hidden = f
        .call(
            "GET",
            "/api/v1/account/group-requests",
            Some("NativeNoPerm"),
            None,
            200,
        )
        .await;
    assert_eq!(hidden["data"], json!([]));
    decide(&f, &g, &r, "accept", "NativeNoPerm", &outsider, 403).await;
    decide(&f, &g, &r, "accept", "NativeOther", &f.account, 409).await;
    decide(&f, &g, &r, "accept", "NativeOther", &f.other, 200).await;
    decide(&f, &g, &r, "accept", "NativeOther", &f.other, 409).await;
    let team = f
        .call(
            "GET",
            &format!("/api/v1/account/group/{g}/team"),
            Some("NativeOther"),
            None,
            200,
        )
        .await;
    assert_eq!(team["ownRole"], "member");
    assert_eq!(team["canManage"], false);
    assert_eq!(team["group"]["version"], 2);
    let promoted=f.call("PUT",&format!("/api/v1/account/group/{g}"),Some("NativeActor"),Some(json!({"version":2,"leader":f.account,"officers":[f.other],"members":[],"expectedAccountId":f.account})),200).await;
    assert_eq!(promoted["data"]["attributes"]["version"], 3);
    let restored=f.call("PUT",&format!("/api/v1/account/group/{g}"),Some("NativeActor"),Some(json!({"version":3,"leader":f.account,"officers":[],"members":[f.other],"expectedAccountId":f.account})),200).await;
    assert_eq!(restored["data"]["attributes"]["version"], 4);

    // A transfer cannot strand the team on an accepted but now-ineligible user.
    let db = f.state.db.client().await.unwrap();
    let saved = db
        .query_one(
            "SELECT verified,disabled,roles,permissions FROM platform.users WHERE id='NativeOther'",
            &[],
        )
        .await
        .unwrap();
    let saved_roles: Value = saved.get(2);
    let saved_permissions: Value = saved.get(3);
    drop(db);
    for change in [
        "UPDATE platform.users SET permissions=permissions-'self.scanlation_group.update' WHERE id='NativeOther'",
        "UPDATE platform.users SET roles='[\"restricted\"]'::jsonb WHERE id='NativeOther'",
        "UPDATE platform.users SET verified=false WHERE id='NativeOther'",
        "UPDATE platform.users SET disabled=true WHERE id='NativeOther'",
    ] {
        let db = f.state.db.client().await.unwrap();
        db.execute("UPDATE platform.users SET verified=true,disabled=false,roles=$1,permissions=$2 || '[\"self.scanlation_group.update\"]'::jsonb WHERE id='NativeOther'", &[&saved_roles,&saved_permissions]).await.unwrap();
        db.batch_execute(change).await.unwrap();
        drop(db);
        eprintln!("Leadership target eligibility case: {change}");
        // Disabled native IDs are intentionally rejected by the adapter before
        // the core target-eligibility check; other ineligible users reach409.
        let disabled_target = change.contains("disabled=true");
        let refused=f.call("PUT",&format!("/api/v1/account/group/{g}"),Some("NativeActor"),Some(json!({"version":4,"leader":f.other,"officers":[],"members":[f.account],"expectedAccountId":f.account})),if disabled_target {400} else {409}).await;
        if disabled_target {assert_eq!(refused["errors"][0]["detail"],"Unknown native group member");}
        else {assert!(refused.to_string().contains("new leader"));}
        let current = f
            .call(
                "GET",
                &format!("/api/v1/account/group/{g}/team"),
                Some("NativeActor"),
                None,
                200,
            )
            .await;
        assert_eq!(current["group"]["version"], 4);
        assert_eq!(current["ownRole"], "leader");
        assert_eq!(current["members"].as_array().unwrap().len(), 2);
    }
    let db = f.state.db.client().await.unwrap();
    db.execute("UPDATE platform.users SET verified=true,disabled=false,roles=$1,permissions=$2 || '[\"self.scanlation_group.update\"]'::jsonb WHERE id='NativeOther'", &[&saved_roles,&saved_permissions]).await.unwrap();
    drop(db);
    for pending_change in [
        "UPDATE platform.users SET disabled=true WHERE id='NativeOther'",
        "UPDATE platform.users SET permissions=permissions-'self.scanlation_group.update' WHERE id='NativeOther'",
    ] {
        let mut db = f.state.db.client().await.unwrap();
        let held = db.transaction().await.unwrap();
        held.batch_execute(pending_change).await.unwrap();
        f.call("PUT",&format!("/api/v1/account/group/{g}"),Some("NativeActor"),Some(json!({"version":4,"leader":f.other,"officers":[],"members":[f.account],"expectedAccountId":f.account})),503).await;
        held.rollback().await.unwrap();
        drop(db);
        let current = f
            .call(
                "GET",
                &format!("/api/v1/account/group/{g}/team"),
                Some("NativeActor"),
                None,
                200,
            )
            .await;
        assert_eq!(current["group"]["version"], 4);
        assert_eq!(current["ownRole"], "leader");
        assert_eq!(current["members"].as_array().unwrap().len(), 2);
    }
    let db = f.state.db.client().await.unwrap();
    db.execute("UPDATE platform.users SET verified=$1,disabled=$2,roles=$3,permissions=$4 WHERE id='NativeOther'", &[&saved.get::<_,bool>(0),&saved.get::<_,bool>(1),&saved_roles,&saved_permissions]).await.unwrap();
    drop(db);

    f.call(
        "PUT",
        &format!("/api/v1/account/group/{g}"),
        Some("NativeActor"),
        Some(json!({"version":1,"description":"Stale overwrite","expectedAccountId":f.account})),
        409,
    )
    .await;
    // Revoke/accept race: one durable terminal result; a NOWAIT authority lock is a safe refusal.
    let race_group = group(&f, "Race team").await;
    let race = invite(&f, &race_group).await;
    let route = format!(
        "/api/v1/account/group/{race_group}/requests/{}/decision",
        race["id"].as_str().unwrap()
    );
    let accept = f
        .request("POST", &route, Some("NativeOther"))
        .header("origin", &f.origin)
        .json(&json!({"version":1,"action":"accept","expectedAccountId":f.other}));
    let revoke = f
        .request("POST", &route, Some("NativeActor"))
        .header("origin", &f.origin)
        .json(&json!({"version":1,"action":"revoke","expectedAccountId":f.account}));
    let (a, b) = tokio::join!(accept.send(), revoke.send());
    let a = a.unwrap().status().as_u16();
    let b = b.unwrap().status().as_u16();
    assert!(a == 200 || b == 200, "race had no terminal result: {a}/{b}");
    assert!(a != 200 || b != 200, "both raced decisions succeeded");
    assert!(
        [200, 409, 503].contains(&a) && [200, 409, 503].contains(&b),
        "{a}/{b}"
    );
    let db = f.state.db.client().await.unwrap();
    let row=db.query_one("SELECT r.status,r.version,EXISTS(SELECT 1 FROM platform.catalog_group_members m WHERE m.group_id=r.group_id AND m.user_id=r.user_id) FROM platform.native_group_requests r WHERE id=$1::text::uuid",&[&race["id"].as_str().unwrap()]).await.unwrap();
    let status: String = row.get(0);
    assert_eq!(row.get::<_, i32>(1), 2);
    assert_eq!(row.get::<_, bool>(2), status == "accepted");
    drop(db);
    // Acceptance racing an explicit leadership transfer uses the same group CAS.
    let db = f.state.db.client().await.unwrap();
    db.batch_execute("UPDATE platform.users SET permissions=permissions || '[\"self.scanlation_group.update\"]'::jsonb WHERE id='NativeNoPerm'").await.unwrap();
    drop(db);
    let transfer_group = group(&f, "Transfer race team").await;
    let member=f.call("POST",&format!("/api/v1/account/group/{transfer_group}/requests"),Some("NativeActor"),Some(json!({"kind":"invitation","recipient":outsider,"version":1,"expectedAccountId":f.account})),200).await;
    decide(
        &f,
        &transfer_group,
        &member,
        "accept",
        "NativeNoPerm",
        &outsider,
        200,
    )
    .await;
    let waiting=f.call("POST",&format!("/api/v1/account/group/{transfer_group}/requests"),Some("NativeActor"),Some(json!({"kind":"invitation","recipient":f.other,"version":2,"expectedAccountId":f.account})),200).await;
    let accept = f
        .request(
            "POST",
            &format!(
                "/api/v1/account/group/{transfer_group}/requests/{}/decision",
                waiting["id"].as_str().unwrap()
            ),
            Some("NativeOther"),
        )
        .header("origin", &f.origin)
        .json(&json!({"version":1,"action":"accept","expectedAccountId":f.other}));
    let transfer=f.request("PUT",&format!("/api/v1/account/group/{transfer_group}"),Some("NativeActor")).header("origin",&f.origin).json(&json!({"version":2,"leader":outsider,"officers":[],"members":[f.account],"expectedAccountId":f.account}));
    let (a, b) = tokio::join!(accept.send(), transfer.send());
    let a = a.unwrap().status().as_u16();
    let b = b.unwrap().status().as_u16();
    assert!(a == 200 || b == 200, "transfer race: {a}/{b}");
    assert!(!(a == 200 && b == 200));
    assert!(
        [200, 403, 409, 503].contains(&a) && [200, 409, 503].contains(&b),
        "transfer race: {a}/{b}"
    );
    // A held global-grant revocation fences acceptance before any membership write.
    let revoked_group = group(&f, "Grant race team").await;
    let waiting = invite(&f, &revoked_group).await;
    let mut db = f.state.db.client().await.unwrap();
    let revoke_tx = db.transaction().await.unwrap();
    revoke_tx.execute("UPDATE platform.users SET permissions=permissions-'self.scanlation_group.update' WHERE id='NativeActor'",&[]).await.unwrap();
    decide(
        &f,
        &revoked_group,
        &waiting,
        "accept",
        "NativeOther",
        &f.other,
        503,
    )
    .await;
    revoke_tx.commit().await.unwrap();
    drop(db);
    decide(
        &f,
        &revoked_group,
        &waiting,
        "accept",
        "NativeOther",
        &f.other,
        403,
    )
    .await;
    let db = f.state.db.client().await.unwrap();
    let member:bool=db.query_one("SELECT EXISTS(SELECT 1 FROM platform.catalog_group_members m JOIN platform.native_group_requests r ON r.group_id=m.group_id AND r.user_id=m.user_id WHERE r.id=$1::text::uuid)",&[&waiting["id"].as_str().unwrap()]).await.unwrap().get(0);
    assert!(!member);
    db.batch_execute("UPDATE platform.users SET permissions=permissions || '[\"self.scanlation_group.update\"]'::jsonb WHERE id='NativeActor'").await.unwrap();
    drop(db);
    // A leader's revoked grant and a transferred leadership both invalidate old invitations.
    let lost = group(&f, "Lost authority team").await;
    let pending = invite(&f, &lost).await;
    let db = f.state.db.client().await.unwrap();
    db.batch_execute("UPDATE platform.users SET permissions=permissions-'self.scanlation_group.update' WHERE id='NativeActor'").await.unwrap();
    drop(db);
    decide(&f, &lost, &pending, "accept", "NativeOther", &f.other, 403).await;
    let db = f.state.db.client().await.unwrap();
    db.batch_execute("UPDATE platform.users SET permissions=permissions || '[\"self.scanlation_group.update\"]'::jsonb WHERE id='NativeActor'").await.unwrap();
    db.execute("UPDATE platform.catalog_group_members SET is_leader=false WHERE group_id=(SELECT group_id FROM platform.native_group_requests WHERE id=$1::text::uuid)",&[&pending["id"].as_str().unwrap()]).await.unwrap();
    drop(db);
    decide(&f, &lost, &pending, "accept", "NativeOther", &f.other, 403).await;
    let db = f.state.db.client().await.unwrap();
    db.execute("UPDATE platform.catalog_group_members SET is_leader=true WHERE group_id=(SELECT group_id FROM platform.native_group_requests WHERE id=$1::text::uuid) AND user_id='NativeActor'",&[&pending["id"].as_str().unwrap()]).await.unwrap();
    drop(db);
    decide(
        &f,
        &lost,
        &pending,
        "revoke",
        "NativeActor",
        &f.account,
        200,
    )
    .await;
    decide(&f, &lost, &pending, "accept", "NativeOther", &f.other, 409).await;
    let expired = group(&f, "Expired invite team").await;
    let pending = invite(&f, &expired).await;
    let db = f.state.db.client().await.unwrap();
    db.execute("UPDATE platform.native_group_requests SET created_at=clock_timestamp()-interval '8 days',expires_at=clock_timestamp()-interval '1 day' WHERE id=$1::text::uuid",&[&pending["id"].as_str().unwrap()]).await.unwrap();
    drop(db);
    decide(
        &f,
        &expired,
        &pending,
        "accept",
        "NativeOther",
        &f.other,
        409,
    )
    .await;
    let application_group = group(&f, "Application team").await;
    let application=f.call("POST",&format!("/api/v1/account/group/{application_group}/requests"),Some("NativeOther"),Some(json!({"kind":"application","version":1,"message":"May I join?","expectedAccountId":f.other})),200).await;
    decide(
        &f,
        &application_group,
        &application,
        "accept",
        "NativeNoPerm",
        &outsider,
        403,
    )
    .await;
    decide(
        &f,
        &application_group,
        &application,
        "accept",
        "NativeActor",
        &f.account,
        200,
    )
    .await;
    // Explicit decline and cancel each consume exactly one request.
    let declined = group(&f, "Declined team").await;
    let pending = invite(&f, &declined).await;
    decide(
        &f,
        &declined,
        &pending,
        "decline",
        "NativeOther",
        &f.other,
        200,
    )
    .await;
    decide(
        &f,
        &declined,
        &pending,
        "accept",
        "NativeOther",
        &f.other,
        409,
    )
    .await;
    let app = f
        .call(
            "POST",
            &format!("/api/v1/account/group/{declined}/requests"),
            Some("NativeOther"),
            Some(json!({"kind":"application","version":1,"expectedAccountId":f.other})),
            200,
        )
        .await;
    decide(&f, &declined, &app, "cancel", "NativeOther", &f.other, 200).await;
    let db = f.state.db.client().await.unwrap();
    db.execute("INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) SELECT 'NativeCh1',group_id FROM platform.native_group_requests WHERE id=$1::text::uuid",&[&r["id"].as_str().unwrap()]).await.unwrap();
    drop(db);
    // Credits are public text, exact-version chapter metadata authority only.
    let chapter = &f.chapters[0];
    let path = format!("/api/v1/account/chapter/{chapter}/credits");
    let attribution = json!([{"role":"translator","name":"Synthetic translator"},{"role":"typesetter","name":"<script>not executable</script>"}]);
    f.call(
        "PUT",
        &path,
        Some("NativeOther"),
        Some(json!({"version":1,"credits":attribution,"expectedAccountId":f.other})),
        403,
    )
    .await;
    f.call("PUT",&path,Some("NativeActor"),Some(json!({"version":1,"credits":[{"role":"publisher","name":"No grant"}],"expectedAccountId":f.account})),400).await;
    let saved = f
        .call(
            "PUT",
            &path,
            Some("NativeActor"),
            Some(json!({"version":1,"credits":attribution,"expectedAccountId":f.account})),
            200,
        )
        .await;
    assert_eq!(saved["version"], 2);
    assert_eq!(saved["credits"], attribution);
    f.call(
        "PUT",
        &path,
        Some("NativeActor"),
        Some(json!({"version":1,"credits":[],"expectedAccountId":f.account})),
        409,
    )
    .await;
    let public = f
        .http
        .get(format!("{}/api/v1/chapter/{chapter}/credits", f.origin))
        .send()
        .await
        .unwrap();
    assert_eq!(public.status(), 200);
    let public: Value = public.json().await.unwrap();
    assert_eq!(public["credits"], attribution);
    assert!(public.get("nostr").is_none());
    // Author feedback retains the same covers/relations through revision and resubmission.
    let created=f.call("POST","/api/v1/account/manga",Some("NativeActor"),Some(json!({"expectedAccountId":f.account,"title":{"en":"Feedback draft"},"description":{"en":"Before"},"originalLanguage":"en","status":"ongoing","contentRating":"safe"})),200).await;
    let title = created["data"]["id"].as_str().unwrap();
    let draft = format!("/api/v1/account/manga/draft/{title}");
    multipart(
        &f,
        &format!(
            "/api/v1/account/cover/{title}?expectedAccountId={}",
            f.account
        ),
        &[
            ("expectedVersion", "1".into()),
            ("expectedAccountId", f.account.clone()),
            ("locale", "en".into()),
            ("main", "true".into()),
        ],
        "NativeActor",
        201,
    )
    .await;
    let read = f.call("GET", &draft, Some("NativeActor"), None, 200).await;
    let version = read["data"]["attributes"]["version"].clone();
    f.call("POST",&format!("/api/v1/account/manga/{title}/relation"),Some("NativeActor"),Some(json!({"version":version,"targetManga":f.manga,"relation":"sequel","expectedAccountId":f.account})),200).await;
    let read = f.call("GET", &draft, Some("NativeActor"), None, 200).await;
    let version = read["data"]["attributes"]["version"].clone();
    let submitted = f
        .call(
            "POST",
            &format!("{draft}/commit"),
            Some("NativeActor"),
            Some(json!({"version":version,"expectedAccountId":f.account})),
            201,
        )
        .await;
    let version = submitted["data"]["attributes"]["version"].clone();
    let db = f.state.db.client().await.unwrap();
    db.batch_execute("UPDATE platform.users SET roles='[\"moderator\"]'::jsonb,permissions=permissions || '[\"manga.state.update\"]'::jsonb WHERE id='NativeOther'").await.unwrap();
    drop(db);
    let native_title = resolver
        .resolve(ResourceKind::Manga, &PublicUuid::parse(title).unwrap())
        .await
        .unwrap()
        .unwrap()
        .native_id;
    let legacy = f
        .request(
            "POST",
            &format!("/api/manga/draft/{native_title}/reject"),
            Some("NativeOther"),
        )
        .header("origin", &f.origin)
        .json(&json!({"version":version}))
        .send()
        .await
        .unwrap();
    assert_eq!(legacy.status(), 400);
    assert!(
        legacy
            .text()
            .await
            .unwrap()
            .contains("author needs to change")
    );
    for reason in [Value::Null, json!("   ")] {
        f.call(
            "POST",
            &format!("{draft}/reject"),
            Some("NativeOther"),
            Some(json!({"version":version,"reason":reason,"expectedAccountId":f.other})),
            400,
        )
        .await;
    }
    let rejected=f.call("POST",&format!("{draft}/reject"),Some("NativeOther"),Some(json!({"version":version,"reason":"Please clarify the description.","affectedFields":["description"],"expectedAccountId":f.other})),200).await;
    assert_eq!(rejected["data"]["attributes"]["state"], "rejected");
    let read = f.call("GET", &draft, Some("NativeActor"), None, 200).await;
    assert_eq!(
        read["data"]["attributes"]["reviewTimeline"]["events"][1]["reason"],
        "Please clarify the description."
    );
    f.call("GET", &draft, Some("NativeNoPerm"), None, 403).await;
    let cover_before = f
        .call(
            "GET",
            &format!(
                "/api/v1/account/manga/{title}/covers?limit=100&expectedAccountId={}",
                f.account
            ),
            Some("NativeActor"),
            None,
            200,
        )
        .await;
    let relations_before=f.call("GET",&format!("/api/v1/account/manga/{title}/relations?limit=32&offset=0&ownerVersion={}&expectedAccountId={}",read["data"]["attributes"]["version"],f.account),Some("NativeActor"),None,200).await;
    let edited=f.call("PUT",&format!("/api/v1/account/manga/{title}"),Some("NativeActor"),Some(json!({"version":read["data"]["attributes"]["version"],"description":{"en":"Clarified description"},"expectedAccountId":f.account})),200).await;
    assert_eq!(edited["data"]["attributes"]["state"], "rejected");
    let resubmitted=f.call("POST",&format!("{draft}/commit"),Some("NativeActor"),Some(json!({"version":edited["data"]["attributes"]["version"],"expectedAccountId":f.account})),201).await;
    assert_eq!(
        resubmitted["data"]["attributes"]["reviewTimeline"]["events"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let cover_after = f
        .call(
            "GET",
            &format!(
                "/api/v1/account/manga/{title}/covers?limit=100&expectedAccountId={}",
                f.account
            ),
            Some("NativeActor"),
            None,
            200,
        )
        .await;
    assert_eq!(cover_before["data"], cover_after["data"]);
    let relations_after=f.call("GET",&format!("/api/v1/account/manga/{title}/relations?limit=32&offset=0&ownerVersion={}&expectedAccountId={}",resubmitted["data"]["attributes"]["version"],f.account),Some("NativeActor"),None,200).await;
    assert_eq!(
        relations_before["data"][0]["id"],
        relations_after["data"][0]["id"]
    );
    assert_eq!(
        relations_before["data"][0]["relationships"],
        relations_after["data"][0]["relationships"]
    );
    assert_eq!(relations_after["total"], 1);
    let legacy=f.request("POST",&format!("/api/manga/draft/{native_title}/reject"),Some("NativeOther")).header("origin",&f.origin).json(&json!({"version":resubmitted["data"]["attributes"]["version"],"reason":"Second revision required","affectedFields":["description"]})).send().await.unwrap();
    assert_eq!(legacy.status(), 200);
    let history = f.call("GET", &draft, Some("NativeActor"), None, 200).await;
    assert_eq!(
        history["data"]["attributes"]["reviewTimeline"]["events"][3]["reason"],
        "Second revision required"
    );
    // Two different leaders cannot race past a recipient's pending-request cap.
    let cap_a = group(&f, "Recipient cap A").await;
    let cap_b = group(&f, "Recipient cap B").await;
    let cap_b_native = resolver
        .resolve(ResourceKind::Group, &PublicUuid::parse(&cap_b).unwrap())
        .await
        .unwrap()
        .unwrap()
        .native_id;
    let db = f.state.db.client().await.unwrap();
    db.batch_execute("UPDATE platform.users SET permissions=permissions || '[\"self.scanlation_group.update\"]'::jsonb WHERE id='NativeOther'").await.unwrap();
    db.execute("UPDATE platform.catalog_group_members SET user_id='NativeOther',name='Other' WHERE group_id=$1 AND is_leader",&[&cap_b_native]).await.unwrap();
    let pending:i64=db.query_one("SELECT count(*) FROM platform.native_group_requests WHERE user_id='NativeNoPerm' AND status='pending' AND expires_at>clock_timestamp()",&[]).await.unwrap().get(0);
    assert!(pending < 99);
    for i in 0..(99 - pending) {
        let id = format!("QueueCapSeed{i}");
        db.execute("INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES($1,'NativeActor','Native Actor','Synthetic cap seed')",&[&id]).await.unwrap();
        db.execute("INSERT INTO platform.native_group_requests(id,group_id,user_id,created_by,kind) VALUES(gen_random_uuid(),$1,'NativeNoPerm','NativeActor','invitation')",&[&id]).await.unwrap();
    }
    drop(db);
    let first=f.request("POST",&format!("/api/v1/account/group/{cap_a}/requests"),Some("NativeActor")).header("origin",&f.origin).json(&json!({"kind":"invitation","recipient":outsider,"version":1,"expectedAccountId":f.account}));
    let second=f.request("POST",&format!("/api/v1/account/group/{cap_b}/requests"),Some("NativeOther")).header("origin",&f.origin).json(&json!({"kind":"invitation","recipient":outsider,"version":1,"expectedAccountId":f.other}));
    let (a, b) = tokio::join!(first.send(), second.send());
    let a = a.unwrap().status().as_u16();
    let b = b.unwrap().status().as_u16();
    assert!(a == 200 || b == 200, "recipient cap race: {a}/{b}");
    assert!(!(a == 200 && b == 200));
    assert!([200, 409, 503].contains(&a) && [200, 409, 503].contains(&b));
    let db = f.state.db.client().await.unwrap();
    let pending:i64=db.query_one("SELECT count(*) FROM platform.native_group_requests WHERE user_id='NativeNoPerm' AND status='pending' AND expires_at>clock_timestamp()",&[]).await.unwrap().get(0);
    assert_eq!(pending, 100);
    drop(db);

    // Explicitly authorized staff administration retains its existing ability.
    let db = f.state.db.client().await.unwrap();
    db.batch_execute("UPDATE platform.users SET permissions=permissions || '[\"scanlation_group.update\"]'::jsonb WHERE id='NativeOther'").await.unwrap();
    drop(db);
    let staff=f.call("PUT",&format!("/api/v1/account/group/{g}"),Some("NativeOther"),Some(json!({"version":4,"leader":f.account,"officers":[],"members":[f.other,outsider],"expectedAccountId":f.other})),200).await;
    assert_eq!(staff["data"]["attributes"]["version"], 5);
    // Metadata-only editors can change public catalogue fields, not read a
    // different author's historical review feedback through the write receipt.
    let db=f.state.db.client().await.unwrap();
    db.batch_execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state,version) VALUES('FeedbackScopeTitle','NativeActor','NativeActor','Published feedback fixture','en','ongoing','safe','published',3); INSERT INTO platform.native_draft_reviews(manga_id,version,action,actor_id,reason,affected_fields) VALUES('FeedbackScopeTitle',2,'rejected','NativeOther','Author-only historical explanation',ARRAY['description']); UPDATE platform.users SET permissions=permissions || '[\"manga.update\"]'::jsonb WHERE id='NativeNoPerm'").await.unwrap();
    let before_history:Value=db.query_one("SELECT to_jsonb(r) FROM platform.native_draft_reviews r WHERE manga_id='FeedbackScopeTitle' AND version=2",&[]).await.unwrap().get(0);drop(db);
    let public=resolver.preferred(ResourceKind::Manga,"FeedbackScopeTitle").await.unwrap().unwrap();
    let edited=f.call("PUT",&format!("/api/v1/account/manga/{public}"),Some("NativeNoPerm"),Some(json!({"version":3,"description":{"en":"Public catalogue correction"},"expectedAccountId":outsider})),200).await;
    assert_eq!(edited["data"]["attributes"]["version"],4);
    assert!(edited["data"]["attributes"].get("reviewTimeline").is_none());
    assert!(!edited.to_string().contains("Author-only historical explanation"));
    // A metadata-only staff grant also does not become a private-review grant.
    let db=f.state.db.client().await.unwrap();db.batch_execute("UPDATE platform.users SET roles='[\"moderator\"]'::jsonb WHERE id='NativeNoPerm'").await.unwrap();drop(db);
    let private_edit=f.call("PUT",&format!("/api/v1/account/manga/{title}"),Some("NativeNoPerm"),Some(json!({"version":history["data"]["attributes"]["version"],"description":{"en":"Metadata-only staff correction"},"expectedAccountId":outsider})),200).await;
    assert!(private_edit["data"]["attributes"].get("reviewTimeline").is_none());
    assert!(!private_edit.to_string().contains("Second revision required"));
    f.call("GET",&draft,Some("NativeNoPerm"),None,403).await;
    let owner_feedback=f.call("GET",&draft,Some("NativeActor"),None,200).await;
    assert_eq!(owner_feedback["data"]["attributes"]["reviewTimeline"]["events"][3]["reason"],"Second revision required");
    let db=f.state.db.client().await.unwrap();let after_history:Value=db.query_one("SELECT to_jsonb(r) FROM platform.native_draft_reviews r WHERE manga_id='FeedbackScopeTitle' AND version=2",&[]).await.unwrap().get(0);assert_eq!(before_history,after_history);drop(db);

    assert_eq!(before.2, f.counts().await.2);
    assert_eq!(before.3, f.counts().await.3);
}
