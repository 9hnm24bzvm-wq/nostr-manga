//! Supported publication-only CLI + real loopback HTTP, disposable PostgreSQL only.
//! Synthetic original admission/key setup is test infrastructure, never product bootstrap.
use base64::{Engine, engine::general_purpose::STANDARD};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use weebdex_platform::{
    AppState, Config, domains,
    domains::publication_access::cli,
    protocol::publication::OperatorRuntimeConfig,
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
    repo: Repository,
    http: reqwest::Client,
    origin: String,
    dsn: String,
    config: PathBuf,
    publisher: String,
    secret: zeroize::Zeroizing<String>,
    _server: Server,
}
impl Fixture {
    fn req(&self, method: &str, path: &str, user: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method.parse().unwrap(), format!("{}{path}", self.origin))
            .header("origin", &self.origin)
            .header("cookie", format!("weebdex_session=synthetic-pub-{user}"))
            .header("x-weebdex-expected-account", user)
    }
    async fn reply(&self, r: reqwest::RequestBuilder, status: u16) -> Value {
        let request = r.build().unwrap();
        let method = request.method().clone();
        let path = request.url().path().to_owned();
        let r = self.http.execute(request).await.unwrap();
        let actual = r.status().as_u16();
        let text = r.text().await.unwrap();
        let v = serde_json::from_str(&text).unwrap_or(Value::Null);
        assert_eq!(actual, status, "{method} {path}: {text}");
        v
    }
    async fn call(
        &self,
        method: &str,
        path: &str,
        user: &str,
        body: Option<Value>,
        status: u16,
    ) -> Value {
        let r = self
            .req(method, path, user)
            .header("content-type", "application/json");
        self.reply(if let Some(b) = body { r.json(&b) } else { r }, status)
            .await
    }
    async fn alias(&self, kind: ResourceKind, id: &str) -> String {
        PostgresPublicIdResolver::new(self.state.db.clone())
            .preferred(kind, id)
            .await
            .unwrap()
            .unwrap()
            .to_string()
    }
    async fn review(&self, op: &str, user: &str) -> Value {
        cli::review(
            &self.state,
            op,
            user,
            &self.publisher,
            "fixture operator",
            "Disposable publication-only review",
        )
        .await
        .unwrap()
    }
    async fn settle(&self, r: &Value, cancel: bool) -> Value {
        cli::settle(
            &self.state,
            r["reviewId"].as_str().unwrap(),
            "fixture operator",
            r["digest"].as_str().unwrap(),
            cancel,
        )
        .await
        .unwrap()
    }
    async fn counts(&self) -> (i64, i64, i64) {
        let row=self.state.db.client().await.unwrap().query_one("SELECT (SELECT count(*) FROM platform.protocol_publication_commands),(SELECT count(*) FROM events WHERE kind IN(1001,1002)),(SELECT count(*) FROM platform.scheduled_uploads)",&[]).await.unwrap();
        (row.get(0), row.get(1), row.get(2))
    }
}
async fn fixture() -> Fixture {
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Owned isolated runner only");
    let pg: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
    assert!(!pg.get_hosts().is_empty());
    assert!(
        pg.get_hosts()
            .iter()
            .all(|h| matches!(h,tokio_postgres::config::Host::Tcp(s)if s=="127.0.0.1"))
    );
    let root = PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap());
    assert!(
        root.to_string_lossy()
            .contains("/weebdex-platform/test-runtime/auth-")
    );
    let evidence: Value =
        serde_json::from_slice(&std::fs::read(root.join("fixture.json")).unwrap()).unwrap();
    assert_eq!(evidence["fixture_only"], true);
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
    let secret = zeroize::Zeroizing::new("12".repeat(32));
    let publisher = auth::sign(&secret, 1, vec![], String::new())
        .unwrap()
        .pubkey;
    let public: [u8; 32] = hex::decode(&publisher).unwrap().try_into().unwrap();
    repo.set_publisher_admission(public, true).await.unwrap();
    repo.bind_instance(public, public).await.unwrap();
    repo.set_account_admission(public, true).await.unwrap();
    let key = root.join("SYNTHETIC_EPHEMERAL_publication_access.key");
    std::fs::write(&key, secret.as_bytes()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let publication = json!({"operator_key_file":key,"expected_publisher":publisher,"storage_owner":publisher,"storage_holder":publisher,"storage":{"path":root.join("publication-source"),"volume":"PublicationAccessFixture","physical_bytes":64*1024*1024,"staging_bytes":64*1024*1024,"max_blob_bytes":8*1024*1024,"upload_slots":2},"group_keys_dir":null,"backend":null,"native_node":{"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":key,"admin_pubkey":publisher,"database_plaintext_local":true,"read_policy":"public","cors_origins":[origin],"sync_worker_enabled":false}});
    let config = root.join("publication-access-cli.json");
    std::fs::write(&config,json!({"production":false,"public_origin":origin,"database_plaintext_local":true,"database_connections":12,"uploads_dir":root.join("publication-media"),"ui_dir":root.join("no-ui"),"protocol_publication":publication}).to_string()).unwrap();
    let cfg: OperatorRuntimeConfig = serde_json::from_value(publication).unwrap();
    let state = AppState::connect(
        Config {
            public_origin: origin.clone(),
            database_plaintext_local: true,
            database_connections: 12,
            uploads_dir: root.join("publication-media"),
            ui_dir: root.join("no-ui"),
            protocol_publication: Some(cfg.clone()),
            ..Config::default()
        },
        dsn.clone(),
    )
    .await
    .unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();
    state.protocol.verify_schema().await.unwrap();
    state.protocol.install_visibility_function().await.unwrap();
    state.protocol.configure_publication(cfg).await.unwrap();
    let db = state.db.client().await.unwrap();
    for user in [
        "PubOwner",
        "PubOther",
        "PubInvalid",
        "PubAdmin",
        "PubPending",
        "PubExpiry",
    ] {
        let roles = if user == "PubOwner" {
            json!(["staff"])
        } else if user == "PubAdmin" {
            json!(["sysop"])
        } else {
            json!(["user"])
        };
        let mut permissions = json!([
            "manga.create",
            "manga.state.update",
            "chapter.create",
            "self.chapter.update",
            "self.chapter.remove",
            "chapter.remove",
            "manga.delete",
            "manga.cover.edit"
        ]);
        if user == "PubAdmin" {
            permissions = json!([
                "admin.users.read",
                "admin.roles.manage",
                "admin.accounts.manage"
            ])
        }
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,roles,permissions) VALUES($1,$1,$2,'synthetic-no-password',$3,$4,$5)",&[&user,&format!("{user}@example.invalid"),&(user!="PubInvalid"),&roles,&permissions]).await.unwrap();
        db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES($1,$1,$2,clock_timestamp()+interval '1 hour')",&[&user,&Sha256::digest(format!("synthetic-pub-{user}").as_bytes()).to_vec()]).await.unwrap();
    }
    for (id, user) in [
        ("PubManga01", "PubOwner"),
        ("PubManga02", "PubOwner"),
        ("PubManga03", "PubOwner"),
        ("PubManga04", "PubPending"),
        ("PubManga05", "PubExpiry"),
    ] {
        db.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES($1,$2,$2,$1,'en','ongoing','safe','published')",&[&id,&user]).await.unwrap();
    }
    drop(db);
    let app = weebdex_platform::router::router(state.clone());
    let server = Server(tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap()
    }));
    let http = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap();
    Fixture {
        state,
        repo,
        http,
        origin,
        dsn,
        config,
        publisher,
        secret,
        _server: server,
    }
}
async fn run_cli(f: &Fixture, args: &[&str], confirm: bool) -> (bool, String) {
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_weebdex-admin"))
        .arg("publication-access")
        .args(args)
        .env("PLATFORM_DATABASE_URL", &f.dsn)
        .env("PLATFORM_CONFIG", &f.config)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut output = String::new();
    while let Some(line) = lines.next_line().await.unwrap() {
        output.push_str(&line);
        output.push('\n');
        if line.starts_with("PUBLICATION-ACCESS ") {
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(
                    if confirm {
                        format!("{line}\n")
                    } else {
                        "no\n".into()
                    }
                    .as_bytes(),
                )
                .await
                .unwrap();
        }
    }
    let status = child.wait().await.unwrap();
    (status.success(), output)
}
fn last_json(output: &str) -> Value {
    serde_json::from_str(&output[output.rfind("\n{\n").map(|n| n + 1).unwrap_or(0)..]).unwrap()
}
fn png() -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        32,
        48,
        image::Rgb([12, 34, 56]),
    ))
    .write_to(&mut out, image::ImageFormat::Png)
    .unwrap();
    out.into_inner()
}
async fn upload(f: &Fixture, manga: &str) -> (String, Value) {
    upload_user(f, manga, "PubOwner").await
}
async fn upload_user(f: &Fixture, manga: &str, user: &str) -> (String, Value) {
    let begun = f
        .call(
            "POST",
            "/api/upload/begin",
            user,
            Some(json!({"manga":manga,"groups":[]})),
            201,
        )
        .await;
    let id = begun["id"].as_str().unwrap().to_owned();
    let boundary = "PublicationAccessBoundary";
    let mut body=format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"fixture.png\"\r\nContent-Type: image/png\r\n\r\n").into_bytes();
    body.extend(png());
    body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
    let files = f
        .reply(
            f.req("POST", &format!("/api/upload/{id}"), user)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(body),
            200,
        )
        .await;
    assert!(files[0]["error"].is_null(), "{files}");
    (
        id,
        json!({"draft":{"volume":"1","chapter":"1","title":"Publication access fixture","language":"en"},"page_order":[files[0]["id"]]}),
    )
}
async fn accept_then_rollback(
    f: &Fixture,
    user: &str,
    manga: &str,
    uncertain: &str,
    uncertain_body: &Value,
) -> String {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        "cookie",
        format!("weebdex_session=synthetic-pub-{user}")
            .parse()
            .unwrap(),
    );
    let principal = domains::auth::require_principal(&f.state, &headers)
        .await
        .unwrap();
    let mut connection = f.state.db.client().await.unwrap();
    let tx = connection.transaction().await.unwrap();
    tx.query_one(
        "SELECT id FROM platform.upload_sessions WHERE id=$1 FOR UPDATE",
        &[&uncertain],
    )
    .await
    .unwrap();
    let fid = uncertain_body["page_order"][0].as_str().unwrap();
    let row=tx.query_one("SELECT o.digest,o.mime,o.bytes,o.width,o.height,o.storage_key FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id WHERE f.id=$1 AND f.session_id=$2",&[&fid,&uncertain]).await.unwrap();
    let original = weebdex_platform::protocol::publication::NativeOriginal {
        hash: row.get(0),
        mime: row.get(1),
        bytes: row.get::<_, i64>(2) as u64,
        width: row.get::<_, i32>(3) as u32,
        height: row.get::<_, i32>(4) as u32,
        storage_key: row.get(5),
    };
    let mut record = domains::catalog::ChapterPublication {
        id: domains::uploads::storage::short_id(),
        manga_id: manga.into(),
        uploader_id: principal.user_id.clone(),
        uploader_name: principal.username.clone(),
        title: Some("Publication access fixture".into()),
        volume: Some("1".into()),
        chapter: Some("1".into()),
        language: "en".into(),
        node: f.origin.clone(),
        pages: vec![domains::catalog::CatalogPage {
            name: format!("{fid}.png"),
            dimensions: [original.width as i32, original.height as i32],
        }],
        optimized_pages: vec![],
        groups: vec![],
        source_id: None,
        protocol_event_id: None,
    };
    #[derive(serde::Serialize)]
    struct CanonicalDraft<'a> {
        volume: Option<&'a str>,
        chapter: Option<&'a str>,
        title: Option<&'a str>,
        language: &'a str,
    }
    #[derive(serde::Serialize)]
    struct CanonicalCommit<'a> {
        draft: CanonicalDraft<'a>,
        page_order: Vec<&'a str>,
    }
    let canonical = CanonicalCommit {
        draft: CanonicalDraft {
            volume: Some("1"),
            chapter: Some("1"),
            title: Some("Publication access fixture"),
            language: "en",
        },
        page_order: vec![fid],
    };
    let hash = auth::hash(&serde_json::to_vec(&canonical).unwrap());
    let accepted = weebdex_platform::protocol::publication::publish_native_upload(
        &f.state,
        &tx,
        &principal,
        &uncertain,
        &hash,
        &mut record,
        &[original],
        &serde_json::to_value(&canonical).unwrap(),
    )
    .await
    .unwrap()
    .unwrap();
    accepted.event.verify().unwrap();
    tx.rollback().await.unwrap();
    drop(connection);
    accepted.event.id
}
async fn close(f: &Fixture, user: &str, status: u16) -> Value {
    let snap = f
        .call(
            "GET",
            &format!("/api/account/closure/{user}"),
            user,
            None,
            200,
        )
        .await;
    let capture = json!({"expectedAccountId":user,"expectedUserId":user,"expectedLifecycleId":snap["lifecycleId"],"version":snap["version"]});
    let review = f
        .call(
            "POST",
            "/api/account/closure/prepare",
            user,
            Some(capture.clone()),
            200,
        )
        .await;
    assert_eq!(review["result"], "prepared", "{review}");
    f.call("POST","/api/account/closure/confirm",user,Some(json!({"capture":capture,"confirmationId":review["confirmationId"],"confirmationToken":review["confirmationToken"],"acknowledge":"DISABLE THIS ACCOUNT"})),status).await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Owned isolated scripts/run-auth-tests.sh database; verifier owns Cargo/runtime"]
async fn reviewed_cli_native_only_publication_recovery_and_lifecycle() {
    let f = fixture().await;
    let db = f.state.db.client().await.unwrap();
    let baseline:Value=db.query_one("SELECT jsonb_agg(jsonb_build_object('id',id,'roles',roles,'permissions',permissions) ORDER BY id) FROM platform.users",&[]).await.unwrap().get(0);
    let account_count: i64 = db
        .query_one("SELECT count(*) FROM account_permissions", &[])
        .await
        .unwrap()
        .get(0);
    drop(db);
    let context = f
        .call(
            "GET",
            "/api/v1/account/contribution-context",
            "PubOwner",
            None,
            200,
        )
        .await;
    assert_eq!(context["upload"]["preflight"]["eligible"], false);
    // Real documented CLI review/apply, exact digest confirmation, no keys or runtime opened.
    let (ok, out) = run_cli(
        &f,
        &[
            "review",
            "--operation",
            "grant",
            "--user",
            "PubOwner",
            "--publisher",
            &f.publisher,
            "--operator",
            "fixture operator",
            "--reason",
            "Disposable publication-only review",
        ],
        false,
    )
    .await;
    assert!(ok, "{out}");
    let reviewed: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        reviewed["summary"]["capabilities"]["signerAuthenticatesAsNativeUser"],
        false
    );
    for private in [
        "synthetic-pub-",
        "password_hash",
        "@example.invalid",
        f.secret.as_str(),
    ] {
        assert!(!out.contains(private));
    }
    let id = reviewed["reviewId"].as_str().unwrap();
    let digest = reviewed["digest"].as_str().unwrap();
    let (ok, _) = run_cli(
        &f,
        &[
            "apply",
            "--review",
            id,
            "--digest",
            digest,
            "--operator",
            "fixture operator",
        ],
        false,
    )
    .await;
    assert!(!ok);
    assert_eq!(
        cli::receipt(&f.state, id, "fixture operator", digest)
            .await
            .unwrap()["pending"],
        true
    );
    let (ok, out) = run_cli(
        &f,
        &[
            "apply",
            "--review",
            id,
            "--digest",
            digest,
            "--operator",
            "fixture operator",
        ],
        true,
    )
    .await;
    assert!(ok, "{out}");
    let granted = last_json(&out);
    assert_eq!(granted["delegation"]["revision"], 1);
    assert_eq!(f.settle(&reviewed, false).await, granted);
    assert_eq!(f.settle(&reviewed, true).await, granted);
    let context = f
        .call(
            "GET",
            "/api/v1/account/contribution-context",
            "PubOwner",
            None,
            200,
        )
        .await;
    assert_eq!(
        context["upload"]["preflight"]["eligible"], true,
        "{context}"
    );
    assert_eq!(
        context["upload"]["preflight"]["scheduling"]["available"],
        false
    );
    let db = f.state.db.client().await.unwrap();
    assert_eq!(
        db.query_one(
            "SELECT count(*) FROM platform.protocol_identity_bindings",
            &[]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        0
    );
    assert_eq!(
        db.query_one("SELECT count(*) FROM account_permissions", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        account_count
    );
    assert_eq!(db.query_one("SELECT jsonb_agg(jsonb_build_object('id',id,'roles',roles,'permissions',permissions) ORDER BY id) FROM platform.users",&[]).await.unwrap().get::<_,Value>(0),baseline);
    drop(db);
    // Valid original HTTP signature/admission still cannot authenticate as native account.
    let path = "/api/v1/user/me";
    let proof = auth::sign(
        &f.secret,
        27235,
        vec![
            vec!["u".into(), format!("{}{path}", f.origin)],
            vec!["method".into(), "GET".into()],
            vec!["service".into(), f.publisher.clone()],
            vec!["payload".into(), auth::hash(b"")],
            vec!["nonce".into(), "publication-access-negative".into()],
        ],
        String::new(),
    )
    .unwrap();
    let signed_refusal = f
        .reply(
            f.http.get(format!("{}{path}", f.origin)).header(
                "authorization",
                format!(
                    "Nostr {}",
                    STANDARD.encode(serde_json::to_vec(&proof).unwrap())
                ),
            ),
            403,
        )
        .await;
    assert!(
        signed_refusal.to_string().contains("binding"),
        "Signed request must reach the missing native binding gate: {signed_refusal}"
    );
    // Reviewed cancel is a durable barrier, including no-op and late exact apply.
    let noop = f.review("grant", "PubOwner").await;
    assert!(
        cli::review(
            &f.state,
            "grant",
            "PubOwner",
            &f.publisher,
            "fixture operator",
            "Other review"
        )
        .await
        .is_err()
    );
    assert!(
        cli::settle(
            &f.state,
            noop["reviewId"].as_str().unwrap(),
            "other operator",
            noop["digest"].as_str().unwrap(),
            false
        )
        .await
        .is_err()
    );
    assert!(
        cli::settle(
            &f.state,
            noop["reviewId"].as_str().unwrap(),
            "fixture operator",
            &"0".repeat(64),
            false
        )
        .await
        .is_err()
    );
    let cancelled = f.settle(&noop, true).await;
    assert_eq!(cancelled["outcome"], "cancelled-before-apply");
    assert_eq!(f.settle(&noop, false).await, cancelled);
    let noop = f.review("grant", "PubOwner").await;
    assert_eq!(f.settle(&noop, false).await["changed"], false);
    assert!(
        cli::review(
            &f.state,
            "grant",
            "PubInvalid",
            &f.publisher,
            "fixture operator",
            "Unverified refusal"
        )
        .await
        .is_err()
    );
    assert!(
        cli::review(
            &f.state,
            "grant",
            "PubOther",
            &"0".repeat(64),
            "fixture operator",
            "Wrong configured key"
        )
        .await
        .is_err()
    );
    // Stale account review and held locks cause no audit/receipt/authority effect.
    let r = f.review("grant", "PubOther").await;
    let db = f.state.db.client().await.unwrap();
    db.execute(
        "UPDATE platform.users SET version=version+1 WHERE id='PubOther'",
        &[],
    )
    .await
    .unwrap();
    drop(db);
    assert_eq!(
        cli::settle(
            &f.state,
            r["reviewId"].as_str().unwrap(),
            "fixture operator",
            r["digest"].as_str().unwrap(),
            false
        )
        .await
        .unwrap_err()
        .status
        .as_u16(),
        409
    );
    f.settle(&r, true).await;
    let r = f.review("grant", "PubOther").await;
    let mut holder = f.state.db.client().await.unwrap();
    let held = holder.transaction().await.unwrap();
    held.query_one(
        "SELECT id FROM platform.users WHERE id='PubOther' FOR UPDATE",
        &[],
    )
    .await
    .unwrap();
    let start = std::time::Instant::now();
    assert_eq!(
        cli::settle(
            &f.state,
            r["reviewId"].as_str().unwrap(),
            "fixture operator",
            r["digest"].as_str().unwrap(),
            false
        )
        .await
        .unwrap_err()
        .status
        .as_u16(),
        409
    );
    assert!(start.elapsed() < Duration::from_secs(2));
    held.rollback().await.unwrap();
    drop(holder);
    f.settle(&r, true).await;
    // Expired immutable synthetic reviewed record exercises the final clock gate,
    // without changing delegation or weakening the production ten-minute lifetime.
    let r = f.review("grant", "PubOther").await;
    f.settle(&r, true).await;
    let mut expired = r.clone();
    expired["reviewId"] = json!("11200000-0000-4000-8000-000000000001");
    expired.as_object_mut().unwrap().remove("digest");
    let hash = hex::encode(Sha256::digest(serde_json::to_vec(&expired).unwrap()));
    let db = f.state.db.client().await.unwrap();
    db.execute("INSERT INTO platform.publication_access_reviews(id,operator_label,user_id,operation,digest,review,expires_at) VALUES($1::text::uuid,'fixture operator','PubOther','grant',$2,$3,clock_timestamp()-interval '1 second')",&[&expired["reviewId"].as_str().unwrap(),&hash,&expired]).await.unwrap();
    drop(db);
    expired["digest"] = json!(hash);
    assert!(
        cli::settle(
            &f.state,
            expired["reviewId"].as_str().unwrap(),
            "fixture operator",
            expired["digest"].as_str().unwrap(),
            false
        )
        .await
        .unwrap_err()
        .message
        .contains("expired")
    );
    f.settle(&expired, true).await;
    // The reviewed revoke workflow is independent of original admission. A
    // retained tombstone cannot fall back to a later broad legacy binding.
    let grant_other = f.review("grant", "PubOther").await;
    f.settle(&grant_other, false).await;
    let revoke_other = f.review("revoke", "PubOther").await;
    let (ok, out) = run_cli(
        &f,
        &[
            "apply",
            "--review",
            revoke_other["reviewId"].as_str().unwrap(),
            "--digest",
            revoke_other["digest"].as_str().unwrap(),
            "--operator",
            "fixture operator",
        ],
        true,
    )
    .await;
    assert!(ok, "{out}");
    assert_eq!(last_json(&out)["delegation"]["revoked"], true);
    let c = f
        .call(
            "GET",
            "/api/v1/account/contribution-context",
            "PubOther",
            None,
            200,
        )
        .await;
    assert_eq!(c["upload"]["preflight"]["eligible"], false);
    // Synthetic selected-group fixture is deliberately NOT authorized in the
    // original module. Dedicated operator access must not silently fall back.
    let db = f.state.db.client().await.unwrap();
    db.execute("INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES('PubGroup01','PubOwner','PubOwner','Publication-only group negative')",&[]).await.unwrap();
    db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,group_id) VALUES($1,'PubGroup01')",&[&"a".repeat(64)]).await.unwrap();
    drop(db);
    let gid = f.alias(ResourceKind::Group, "PubGroup01").await;
    let manga = f.alias(ResourceKind::Manga, "PubManga01").await;
    let account = f.alias(ResourceKind::User, "PubOwner").await;
    let group=f.call("GET",&format!("/api/v1/account/upload/preflight/{manga}?expectedAccountId={account}&groups%5B%5D={gid}"),"PubOwner",None,200).await;
    assert_eq!(group["preflight"]["eligible"], false);
    assert!(
        group["preflight"]["reason"]
            .as_str()
            .unwrap()
            .contains("groups")
    );
    f.state
        .db
        .client()
        .await
        .unwrap()
        .execute(
            "DELETE FROM platform.protocol_identity_bindings WHERE group_id='PubGroup01'",
            &[],
        )
        .await
        .unwrap();

    // Future publishAt is an explicit refusal before allocating source/schedule work.
    let (session, body) = upload(&f, "PubManga01").await;
    let account = f.alias(ResourceKind::User, "PubOwner").await;
    let sid = f.alias(ResourceKind::UploadSession, &session).await;
    let file = f
        .alias(
            ResourceKind::UploadFile,
            body["page_order"][0].as_str().unwrap(),
        )
        .await;
    let future = f
        .state
        .db
        .client()
        .await
        .unwrap()
        .query_one(
            "SELECT to_char((clock_timestamp()+interval '1 hour') AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS')",
            &[],
        )
        .await
        .unwrap()
        .get::<_, String>(0);
    let timing=f.state.db.client().await.unwrap().query_one("SELECT ($1::text::timestamp AT TIME ZONE 'UTC')>clock_timestamp(),current_setting('TimeZone'),clock_timestamp()::text",&[&future]).await.unwrap();
    assert!(
        timing.get::<_, bool>(0),
        "Generated UTC publishAt {future} must be future; database zone={} clock={}",
        timing.get::<_, String>(1),
        timing.get::<_, String>(2)
    );
    let counts = f.counts().await;
    f.call("POST",&format!("/api/v1/account/upload/{sid}/commit?expectedAccountId={account}"),"PubOwner",Some(json!({"chapterDraft":{"volume":"1","chapter":"1","title":"Publication access fixture","translatedLanguage":"en","publishAt":future},"pageOrder":[file],"termsAccepted":true})),403).await;
    assert_eq!(f.counts().await, counts);
    // A concurrent revoke's delegation lock fences fresh publication before
    // durable command allocation. Staged native originals remain available.
    let mut revoke_holder = f.state.db.client().await.unwrap();
    let revoke_lock = revoke_holder.transaction().await.unwrap();
    revoke_lock.query_one("SELECT user_id FROM platform.native_publication_delegations WHERE user_id='PubOwner' FOR UPDATE",&[]).await.unwrap();
    f.call(
        "POST",
        &format!("/api/upload/{session}/commit"),
        "PubOwner",
        Some(body.clone()),
        409,
    )
    .await;
    assert_eq!(f.counts().await, counts);
    revoke_lock.rollback().await.unwrap();
    drop(revoke_holder);

    // Actual publication holds account/delegation authority through an independent
    // source-intent wait. Revoke cannot pass that boundary or partly audit itself.
    let revoke = f.review("revoke", "PubOwner").await;
    let mut holder = f.state.db.client().await.unwrap();
    let held = holder.transaction().await.unwrap();
    held.batch_execute("LOCK TABLE platform.protocol_publication_commands IN SHARE MODE")
        .await
        .unwrap();
    let request = f
        .req("POST", &format!("/api/upload/{session}/commit"), "PubOwner")
        .json(&body);
    let task = tokio::spawn(async move { request.send().await.unwrap() });
    let observer = f.state.db.client().await.unwrap();
    let start = std::time::Instant::now();
    loop {
        if observer.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE wait_event_type='Lock' AND query LIKE 'INSERT INTO platform.protocol_publication_commands%')",&[]).await.unwrap().get::<_,bool>(0){break}
        assert!(
            start.elapsed() < Duration::from_millis(800),
            "publication never reached intent wait"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(
        cli::settle(
            &f.state,
            revoke["reviewId"].as_str().unwrap(),
            "fixture operator",
            revoke["digest"].as_str().unwrap(),
            false
        )
        .await
        .unwrap_err()
        .status
        .as_u16(),
        409
    );
    held.rollback().await.unwrap();
    drop(holder);
    drop(observer);
    let response = task.await.unwrap();
    let status = response.status().as_u16();
    let chapter: Value = response.json().await.unwrap();
    assert_eq!(status, 200, "{chapter}");
    assert_eq!(chapter["publication_status"], "accepted_pending_projection");
    f.settle(&revoke, true).await;
    let counts = f.counts().await;
    assert_eq!(counts, (2, 2, 0));
    let chapter_id = chapter["id"].as_str().unwrap();
    let replay = f
        .call(
            "POST",
            &format!("/api/upload/{session}/commit"),
            "PubOwner",
            Some(body.clone()),
            200,
        )
        .await;
    assert_eq!(replay["protocol_event_id"], chapter["protocol_event_id"]);
    assert_eq!(f.counts().await, counts);
    let saved = f
        .call(
            "GET",
            &format!("/api/v1/account/uploads/{sid}?expectedAccountId={account}"),
            "PubOwner",
            None,
            200,
        )
        .await;
    assert!(!saved["session"]["receipt"].is_null(), "{saved}");
    let foreign = f.alias(ResourceKind::User, "PubOther").await;
    f.call(
        "GET",
        &format!("/api/v1/account/uploads/{sid}?expectedAccountId={foreign}"),
        "PubOther",
        None,
        404,
    )
    .await;
    let db = f.state.db.client().await.unwrap();
    let pins=db.query("SELECT authority_kind,delegation_revision,delegation_publisher,delegation_admission_version,delegation_lifecycle_id IS NOT NULL FROM platform.protocol_publication_commands ORDER BY command_id",&[]).await.unwrap();
    for p in pins {
        assert_eq!(p.get::<_, String>(0), "native-delegation");
        assert_eq!(p.get::<_, Option<i64>>(1), Some(1));
        assert_eq!(p.get::<_, Option<String>>(2), Some(f.publisher.clone()));
        assert!(p.get::<_, bool>(4));
    }
    assert!(
        db.execute(
            "UPDATE platform.protocol_publication_commands SET delegation_revision=99",
            &[]
        )
        .await
        .is_err()
    );
    drop(db);
    // Unprojected source remains unavailable even to native removal grants;
    // no source-deletion identity/request is manufactured by this delegation.
    f.call(
        "DELETE",
        &format!("/api/chapter/{chapter_id}"),
        "PubOwner",
        Some(json!({"version":chapter["version"]})),
        404,
    )
    .await;
    assert_eq!(f.counts().await, counts);
    // Actual original acceptance followed by native transaction rollback is
    // unresolved, never cancellation. The CLI refuses regrant until exact HTTP
    // recovery materializes the same signed event, with no duplicated source.
    let (uncertain, uncertain_body) = upload(&f, "PubManga02").await;
    let accepted =
        accept_then_rollback(&f, "PubOwner", "PubManga02", &uncertain, &uncertain_body).await;
    let error = cli::review(
        &f.state,
        "grant",
        "PubOwner",
        &f.publisher,
        "fixture operator",
        "Cannot adopt accepted source",
    )
    .await
    .unwrap_err();
    assert_eq!(error.status.as_u16(), 409);
    assert!(error.message.contains("irreversible"));
    let accepted_counts = f.counts().await;
    let recovered = f
        .call(
            "POST",
            &format!("/api/upload/{uncertain}/commit"),
            "PubOwner",
            Some(uncertain_body),
            200,
        )
        .await;
    assert_eq!(recovered["protocol_event_id"], accepted);
    assert_eq!(f.counts().await, accepted_counts);
    let counts = f.counts().await;

    // Original revoke/re-admit increments its epoch and does not revive delegation.
    let public: [u8; 32] = hex::decode(&f.publisher).unwrap().try_into().unwrap();
    f.repo.set_publisher_admission(public, false).await.unwrap();
    f.repo.set_publisher_admission(public, true).await.unwrap();
    let c = f
        .call(
            "GET",
            "/api/v1/account/contribution-context",
            "PubOwner",
            None,
            200,
        )
        .await;
    assert_eq!(c["upload"]["preflight"]["eligible"], false);
    let fresh = f.review("grant", "PubOwner").await;
    let regrant = f.settle(&fresh, false).await;
    assert_eq!(regrant["delegation"]["revision"], 2);
    let c = f
        .call(
            "GET",
            "/api/v1/account/contribution-context",
            "PubOwner",
            None,
            200,
        )
        .await;
    assert_eq!(c["upload"]["preflight"]["eligible"], true);
    f.call(
        "POST",
        &format!("/api/upload/{session}/commit"),
        "PubOwner",
        Some(body),
        404,
    )
    .await;
    assert_eq!(f.counts().await, counts);
    // Containment-only revoke remains supported for an already source-accepted,
    // unmaterialized command. It neither cancels/deletes source nor adopts it.
    let pending_grant = f.review("grant", "PubPending").await;
    f.settle(&pending_grant, false).await;
    let (pending_session, pending_body) = upload_user(&f, "PubManga04", "PubPending").await;
    let accepted_pending = accept_then_rollback(
        &f,
        "PubPending",
        "PubManga04",
        &pending_session,
        &pending_body,
    )
    .await;
    let pending_counts = f.counts().await;
    let revoke = f.review("revoke", "PubPending").await;
    assert!(
        revoke["summary"]["unresolvedWork"]["sourceAcceptedPublicationCommands"]
            .as_i64()
            .unwrap()
            > 0
    );
    let revoked = f.settle(&revoke, false).await;
    assert_eq!(revoked["outcome"], "applied");
    assert_eq!(revoked["sourceEventsRetracted"], false);
    assert_eq!(
        revoked["unresolvedWork"],
        revoke["summary"]["unresolvedWork"]
    );
    assert_eq!(f.counts().await, pending_counts);
    assert!(
        cli::review(
            &f.state,
            "grant",
            "PubPending",
            &f.publisher,
            "fixture operator",
            "Do not rebind old command"
        )
        .await
        .unwrap_err()
        .message
        .contains("Unresolved")
    );
    f.call(
        "POST",
        &format!("/api/upload/{pending_session}/commit"),
        "PubPending",
        Some(pending_body),
        403,
    )
    .await;
    assert_eq!(f.counts().await, pending_counts);
    let db = f.state.db.client().await.unwrap();
    let accepted_bytes = hex::decode(&accepted_pending).unwrap();
    assert!(
        db.query_one(
            "SELECT EXISTS(SELECT 1 FROM events WHERE id=$1 AND kind=1001)",
            &[&accepted_bytes]
        )
        .await
        .unwrap()
        .get::<_, bool>(0)
    );
    drop(db);

    // Advancing-clock expiry AFTER actual original staging, not before command
    // insertion. Hold its logical blob-grant write, observe the wait, cross the
    // DB expiry time, then release. No accepted event may be created afterward.
    let expiry_grant = f.review("grant", "PubExpiry").await;
    f.settle(&expiry_grant, false).await;
    let (expiry_session, expiry_body) = upload_user(&f, "PubManga05", "PubExpiry").await;
    let before_expiry = f.counts().await;
    let observer = f.state.db.client().await.unwrap();
    let mut staging_holder = f.state.db.client().await.unwrap();
    let staging_lock = staging_holder.transaction().await.unwrap();
    staging_lock
        .batch_execute("LOCK TABLE publisher_blob_grants IN SHARE MODE")
        .await
        .unwrap();
    observer.execute("UPDATE platform.sessions SET expires_at=clock_timestamp()+interval '600 milliseconds' WHERE id='PubExpiry'",&[]).await.unwrap();
    let request = f
        .req(
            "POST",
            &format!("/api/upload/{expiry_session}/commit"),
            "PubExpiry",
        )
        .json(&expiry_body);
    let pending = tokio::spawn(async move { request.send().await.unwrap() });
    let started = std::time::Instant::now();
    loop {
        let waiting:bool=observer.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE wait_event_type='Lock' AND query LIKE 'INSERT INTO publisher_blob_grants%')",&[]).await.unwrap().get(0);
        if waiting {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "source staging did not reach held original grant before expiry"
        );
        tokio::time::sleep(Duration::from_millis(3)).await;
    }
    loop {
        if observer
            .query_one(
                "SELECT expires_at<=clock_timestamp() FROM platform.sessions WHERE id='PubExpiry'",
                &[],
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
        {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_millis(850),
            "session clock did not cross within original lock budget"
        );
        tokio::time::sleep(Duration::from_millis(3)).await;
    }
    staging_lock.rollback().await.unwrap();
    drop(staging_holder);
    drop(observer);
    let response = pending.await.unwrap();
    let status = response.status().as_u16();
    let refused: Value = response.json().await.unwrap();
    assert_eq!(status, 401, "{refused}");
    let after_expiry = f.counts().await;
    assert_eq!(after_expiry.1, before_expiry.1);
    assert_eq!(after_expiry.2, before_expiry.2);
    assert!(after_expiry.0 > before_expiry.0);
    let db = f.state.db.client().await.unwrap();
    assert!(db.query_one("SELECT EXISTS(SELECT 1 FROM platform.protocol_publication_commands WHERE user_id='PubExpiry' AND state='prepared' AND authority_kind='native-delegation' AND source_manifest IS NOT NULL)",&[]).await.unwrap().get::<_,bool>(0));
    drop(db);

    // Sole-administrator closure rolls delegation and its audit back atomically.
    let admin = f.review("grant", "PubAdmin").await;
    f.settle(&admin, false).await;
    let audit_before: i64 = f
        .state
        .db
        .client()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM platform.publication_access_audit",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    close(&f, "PubAdmin", 409).await;
    let db = f.state.db.client().await.unwrap();
    assert!(!db.query_one("SELECT revoked_at IS NOT NULL FROM platform.native_publication_delegations WHERE user_id='PubAdmin'",&[]).await.unwrap().get::<_,bool>(0));
    assert_eq!(
        db.query_one(
            "SELECT count(*) FROM platform.publication_access_audit",
            &[]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        audit_before
    );
    drop(db);
    close(&f, "PubOwner", 200).await;
    let target = f
        .call(
            "GET",
            "/api/staff/admin/users/PubOwner",
            "PubAdmin",
            None,
            200,
        )
        .await;
    let command = json!({"requestId":"11200000-0000-4000-8000-000000000002","targetId":"PubOwner","authorityVersion":target["target"]["authorityVersion"],"operation":"reactivate","reason":"Disposable owner requested reactivation","ownerRequested":true});
    f.call(
        "POST",
        "/api/staff/admin/users/PubOwner/review",
        "PubAdmin",
        Some(command.clone()),
        200,
    )
    .await;
    f.call(
        "POST",
        "/api/staff/admin/users/PubOwner/commands",
        "PubAdmin",
        Some(command),
        200,
    )
    .await;
    let db = f.state.db.client().await.unwrap();
    let closed=db.query_one("SELECT revoked_at IS NOT NULL,revision FROM platform.native_publication_delegations WHERE user_id='PubOwner'",&[]).await.unwrap();
    assert!(closed.get::<_, bool>(0));
    assert_eq!(closed.get::<_, i64>(1), 3);
    assert_eq!(
        db.query_one(
            "SELECT count(*) FROM platform.protocol_identity_bindings",
            &[]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        0
    );
    assert!(
        db.execute("DELETE FROM platform.native_publication_delegations", &[])
            .await
            .is_err()
    );
    assert!(
        db.execute(
            "UPDATE platform.publication_access_audit SET reason='tamper'",
            &[]
        )
        .await
        .is_err()
    );
    assert!(
        db.execute("DELETE FROM platform.publication_access_receipts", &[])
            .await
            .is_err()
    );
    drop(db);
    println!(
        "Dedicated publication CLI: real exact confirmation/cancel/receipts; no broad binding/grant; native immediate auto-parent/chapter; signed-auth and withdrawal refusal; schedule no-effect refusal; held publish/revoke boundary; immutable capture/epoch; closure/last-admin rollback/reactivation passed"
    );
}
