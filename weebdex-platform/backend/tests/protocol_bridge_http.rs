//! Genuine platform-to-original module seams, using only synthetic isolated fixtures.
//!
//! Run only after the implementation freeze, through scripts/run-auth-tests.sh.
//! Synthetic principals are fixture setup; all catalogue/upload mutations, media reads,
//! ordering/version checks and retry behavior go through the public HTTP Router.
//! These tests use only local generated images and an isolated temporary storage root.
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, Method, Request, StatusCode},
};
use image::{Delay, DynamicImage, Frame, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
use rand::{Rng, distributions::Alphanumeric};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{io::Cursor, path::PathBuf};
use tempfile::TempDir;
use tower::ServiceExt;
use weebdex_platform::{AppState, Config, domains};

const ORIGIN: &str = "https://upload.fixture.invalid";

struct Fixture {
    state: AppState,
    app: Router,
    owner: String,
    owner_id: String,
    foreign: String,
    _storage: TempDir,
}
struct Reply {
    headers: HeaderMap,
    body: Vec<u8>,
}
impl Reply {
    fn json(&self) -> Value {
        if self.body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&self.body).unwrap_or_else(|_| {
                panic!(
                    "Expected JSON, got {:?}",
                    String::from_utf8_lossy(&self.body)
                )
            })
        }
    }
    fn header(&self, name: &str) -> &str {
        self.headers.get(name).unwrap().to_str().unwrap()
    }
}
async fn send(
    app: &Router,
    method: Method,
    path: &str,
    cookie: Option<&str>,
    headers: &[(&str, &str)],
    body: Vec<u8>,
    expected: StatusCode,
) -> Reply {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("origin", ORIGIN);
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let response_headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 40 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    assert_eq!(
        status,
        expected,
        "{path}: {}",
        String::from_utf8_lossy(&body)
    );
    if path.starts_with("/api/") {
        assert_eq!(response_headers.get("cache-control").unwrap(), "no-store");
    }
    Reply {
        headers: response_headers,
        body,
    }
}
async fn api(
    app: &Router,
    method: Method,
    path: &str,
    cookie: Option<&str>,
    payload: Option<Value>,
    expected: StatusCode,
) -> Value {
    let body = payload
        .map(|p| p.to_string().into_bytes())
        .unwrap_or_default();
    send(
        app,
        method,
        path,
        cookie,
        &[("content-type", "application/json")],
        body,
        expected,
    )
    .await
    .json()
}

/// Guardrails deliberately reject arbitrary operator DSNs and storage roots.
async fn fixture() -> Fixture {
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL")
        .expect("Use scripts/run-auth-tests.sh with its owned disposable PostgreSQL fixture");
    let pg: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
    assert!(!pg.get_hosts().is_empty());
    for host in pg.get_hosts() {
        match host {
            tokio_postgres::config::Host::Unix(path) => assert!(
                path.to_string_lossy()
                    .contains("weebdex-platform/test-runtime/")
            ),
            tokio_postgres::config::Host::Tcp(host) => assert_eq!(host, "127.0.0.1"),
        }
    }
    let runtime = PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap());
    assert!(
        runtime
            .to_string_lossy()
            .contains("weebdex-platform/test-runtime/")
    );
    let storage = tempfile::Builder::new()
        .prefix("upload-http-")
        .tempdir_in(runtime)
        .unwrap();
    let state = AppState::connect(
        Config {
            public_origin: ORIGIN.into(),
            database_plaintext_local: true,
            uploads_dir: storage.path().join("media"),
            registration_enabled: false,
            protocol_publication: None,
            database_connections: 4,
            ..Config::default()
        },
        dsn,
    )
    .await
    .unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();
    let client = state.db.client().await.unwrap();
    let suffix: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect();
    let owner_id = format!("Up{suffix}");
    let foreign_id: String = format!("Uf{suffix}");
    let owner_token = format!("synthetic-upload-owner-{suffix}");
    let foreign_token = format!("synthetic-upload-foreign-{suffix}");
    for (id, username, token, roles, permissions) in [
        (
            &owner_id,
            format!("UpOwner{suffix}"),
            &owner_token,
            json!(["staff"]),
            json!([
                "manga.create",
                "manga.update",
                "manga.title.edit",
                "manga.author.edit",
                "manga.state.update",
                "manga.lock.update",
                "manga.delete",
                "manga.cover.edit",
                "author.create",
                "author.update",
                "cover.delete",
                "chapter.create",
                "chapter.update",
                "chapter.remove",
                "self.chapter.update",
                "self.chapter.remove"
            ]),
        ),
        (
            &foreign_id,
            format!("UpForeign{suffix}"),
            &foreign_token,
            json!(["user"]),
            json!([
                "manga.create",
                "chapter.create",
                "self.chapter.update",
                "self.chapter.remove"
            ]),
        ),
    ] {
        let email = format!("{}@fixture.invalid", username.to_ascii_lowercase());
        client.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,roles,permissions) VALUES($1,$2,$3,'synthetic-no-password',TRUE,$4,$5)", &[id, &username, &email, &roles, &permissions]).await.unwrap();
        let hash = Sha256::digest(token.as_bytes()).to_vec();
        client.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES($1,$1,$2,clock_timestamp()+interval '1 hour')", &[id, &hash]).await.unwrap();
    }
    drop(client);
    let app = weebdex_platform::router::router(state.clone());
    Fixture {
        state,
        app,
        owner_id,
        owner: format!("weebdex_session={owner_token}"),
        foreign: format!("weebdex_session={foreign_token}"),
        _storage: storage,
    }
}
async fn create_manga(f: &Fixture, title: &str, publish: bool) -> Value {
    let mut manga = api(&f.app, Method::POST, "/api/manga", Some(&f.owner), Some(json!({
        "title":title, "year":2025, "language":"en", "status":"ongoing", "content_rating":"safe", "authors":[], "artists":[]
    })), StatusCode::CREATED).await;
    if publish {
        let id = manga["id"].as_str().unwrap().to_owned();
        manga = api(
            &f.app,
            Method::POST,
            &format!("/api/manga/draft/{id}/commit"),
            Some(&f.owner),
            Some(json!({"version":manga["version"]})),
            StatusCode::OK,
        )
        .await;
        manga = api(
            &f.app,
            Method::POST,
            &format!("/api/manga/draft/{id}/publish"),
            Some(&f.owner),
            Some(json!({"version":manga["version"]})),
            StatusCode::OK,
        )
        .await;
    }
    manga
}
async fn add_cover(f: &Fixture, manga: &str, bytes: &[u8], expected: StatusCode) -> Value {
    let (content_type, body) = multipart(&[
        image_part("synthetic-cover.png", "image/png", bytes),
        Part {
            name: "main",
            filename: None,
            mime: None,
            bytes: b"true",
        },
    ]);
    send(
        &f.app,
        Method::POST,
        &format!("/api/cover/{manga}"),
        Some(&f.owner),
        &[("content-type", &content_type)],
        body,
        expected,
    )
    .await
    .json()
}
async fn create_covered_manga(f: &Fixture, title: &str) -> Value {
    let manga = create_manga(f, title, false).await;
    let mid = manga["id"].as_str().unwrap();
    add_cover(f, mid, &png(), StatusCode::OK).await;
    let submitted = api(
        &f.app,
        Method::POST,
        &format!("/api/manga/draft/{mid}/commit"),
        Some(&f.owner),
        Some(json!({"version":manga["version"]})),
        StatusCode::OK,
    )
    .await;
    api(
        &f.app,
        Method::POST,
        &format!("/api/manga/draft/{mid}/publish"),
        Some(&f.owner),
        Some(json!({"version":submitted["version"]})),
        StatusCode::OK,
    )
    .await
}
async fn begin(f: &Fixture, manga: &str) -> String {
    let session = api(
        &f.app,
        Method::POST,
        "/api/upload/begin",
        Some(&f.owner),
        Some(json!({"manga":manga,"groups":[]})),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(session["version"], 1);
    assert_eq!(session["is_committed"], false);
    assert!(
        session["relationships"]["files"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    session["id"].as_str().unwrap().to_owned()
}
fn draft(title: &str) -> Value {
    json!({"volume":"1","chapter":"1","title":title,"language":"en"})
}
fn commit_payload(title: &str, files: &[Value]) -> Value {
    json!({"draft":draft(title), "page_order":files.iter().map(|f| f["id"].as_str().unwrap()).collect::<Vec<_>>()})
}

// Complete encoders make valid local images rather than magic-byte-only fixtures.
fn png() -> Vec<u8> {
    let image = RgbImage::from_fn(128, 160, |x, y| {
        let n = (x.wrapping_mul(73_856_093)) ^ (y.wrapping_mul(19_349_663));
        Rgb([(n >> 16) as u8, (n >> 8) as u8, n as u8])
    });
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image)
        .write_to(&mut out, ImageFormat::Png)
        .unwrap();
    out.into_inner()
}
fn jpeg() -> Vec<u8> {
    let image = RgbImage::from_fn(96, 144, |x, y| {
        let n = x.wrapping_mul(2_654_435_761) ^ y.wrapping_mul(2_246_822_519);
        Rgb([(n >> 16) as u8, (n >> 8) as u8, n as u8])
    });
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 100)
        .encode(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
    out
}
fn gif() -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut out);
        encoder
            .set_repeat(image::codecs::gif::Repeat::Infinite)
            .unwrap();
        encoder
            .encode_frames([0u8, 1].into_iter().map(|n| {
                Frame::from_parts(
                    RgbaImage::from_fn(24, 32, |x, y| {
                        Rgba([x as u8 * 5, y as u8 * 3, n * 200, 255])
                    }),
                    0,
                    0,
                    Delay::from_numer_denom_ms(80, 1),
                )
            }))
            .unwrap();
    }
    out
}
struct Part<'a> {
    name: &'a str,
    filename: Option<&'a str>,
    mime: Option<&'a str>,
    bytes: &'a [u8],
}
fn multipart(parts: &[Part<'_>]) -> (String, Vec<u8>) {
    let boundary = "UploadPublicSeamFixtureBoundary71a";
    let mut body = Vec::new();
    for part in parts {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"",
                part.name
            )
            .as_bytes(),
        );
        if let Some(filename) = part.filename {
            body.extend_from_slice(format!("; filename=\"{filename}\"").as_bytes());
        }
        body.extend_from_slice(b"\r\n");
        if let Some(mime) = part.mime {
            body.extend_from_slice(format!("Content-Type: {mime}\r\n").as_bytes());
        }
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(part.bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}
async fn upload(
    f: &Fixture,
    session: &str,
    parts: &[Part<'_>],
    cookie: Option<&str>,
    expected: StatusCode,
) -> Value {
    let (content_type, body) = multipart(parts);
    send(
        &f.app,
        Method::POST,
        &format!("/api/upload/{session}"),
        cookie,
        &[("content-type", &content_type)],
        body,
        expected,
    )
    .await
    .json()
}
fn image_part<'a>(filename: &'a str, mime: &'a str, bytes: &'a [u8]) -> Part<'a> {
    Part {
        name: "file",
        filename: Some(filename),
        mime: Some(mime),
        bytes,
    }
}
fn file_array(value: Value, expected: usize) -> Vec<Value> {
    let files = value.as_array().unwrap();
    assert_eq!(files.len(), expected);
    for file in files {
        assert!(file.get("error").is_none(), "{file}");
        let id = file["id"].as_str().unwrap();
        assert_eq!(id.len(), 10);
        assert!(id.bytes().all(|b| b.is_ascii_alphanumeric()));
        assert!(file["file_name"].as_str().unwrap().starts_with(id));
    }
    files.clone()
}

use manga_node::{
    auth,
    db::{DbConfig, Repository},
};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use weebdex_platform::{
    domains::catalog::{CatalogPage, ChapterPublication},
    protocol::{
        bridge, covers,
        publication::{
            NativeListener, NativeOriginal, OperatorRuntimeConfig, PublicationInput,
            publish_native_upload,
        },
    },
};
fn id(text: &str) -> [u8; 32] {
    hex::decode(text).unwrap().try_into().unwrap()
}
struct ListenerGuard(Option<NativeListener>);
impl Drop for ListenerGuard {
    fn drop(&mut self) {
        if let Some(listener) = &mut self.0 {
            if let Some(stop) = listener.shutdown.take() {
                let _ = stop.send(());
            }
            listener.task.abort();
        }
    }
}
async fn wait_source(f: &Fixture, event: &str) {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if f.state.protocol.entry(id(event)).await.unwrap().is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await
    .expect("Real original backend projection timed out");
}
async fn sweep(f: &Fixture) {
    for _ in 0..10 {
        let report = bridge::import_page(&f.state).await.unwrap();
        if report.completed_sweep {
            return;
        }
    }
    panic!("Bounded fixture source sweep did not finish");
}
fn proof(secret: &str, url: &str, body: &[u8]) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let event = auth::sign(
        secret,
        27235,
        vec![
            vec!["u".into(), url.into()],
            vec!["method".into(), "POST".into()],
            vec!["payload".into(), auth::hash(body)],
            vec!["nonce".into(), hex::encode(rand::random::<[u8; 16]>())],
        ],
        String::new(),
    )
    .unwrap();
    format!(
        "Nostr {}",
        STANDARD.encode(serde_json::to_vec(&event).unwrap())
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires isolated PostgreSQL and the existing real strfry binary; compiler owner runs after source freeze"]
async fn accepted_upload_rollback_recovers_source_media_covers_revocation_and_embedded_owner_lifecycle()
 {
    let mut f = fixture().await;
    let manga = create_manga(&f, "Bridge durable rollback fixture", true).await;
    let mid = manga["id"].as_str().unwrap().to_owned();
    let mut repo = Repository::connect(DbConfig {
        dsn: std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").unwrap(),
        connections: 1,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1024 * 1024,
        plaintext_local: true,
    })
    .await
    .unwrap();
    // Every live component uses the AppState's one shared pool. The short-lived
    // repository constructor's pool is discarded before any source work starts.
    repo.pool = f.state.db.pool().clone();
    repo.migrate().await.unwrap();
    let (secret, publisher) = loop {
        let secret = hex::encode(rand::random::<[u8; 32]>());
        if let Ok(event) = auth::sign(&secret, 1, vec![], String::new()) {
            break (zeroize::Zeroizing::new(secret), event.pubkey);
        }
    };
    repo.set_publisher_admission(id(&publisher), true)
        .await
        .unwrap();
    let key = f._storage.path().join("SYNTHETIC_EPHEMERAL_bridge.key");
    std::fs::write(&key, secret.as_bytes()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let bind = reservation.local_addr().unwrap();
    drop(reservation);
    let origin = format!("http://{bind}");
    let binary = std::env::var_os("STRFRY_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../manga-protocol-lab/vendor/strfry/strfry")
                .canonicalize()
                .unwrap()
        });
    assert!(
        binary.is_absolute() && binary.is_file(),
        "Real installed projection binary is required"
    );
    let cfg = OperatorRuntimeConfig {
        operator_key_file: key.clone(),
        expected_publisher: publisher.clone(),
        storage_owner: publisher.clone(),
        storage_holder: publisher.clone(),
        group_keys_dir: None,
        backend: None,
        storage: json!({"path":f._storage.path().join("source-originals"),"volume":"BridgeOriginalsFixture","physical_bytes":64*1024*1024,"staging_bytes":64*1024*1024,"max_blob_bytes":8*1024*1024,"upload_slots":2}),
        native_node: Some(
            json!({"bind":bind.to_string(),"base_url":origin,"service_key_file":key,"admin_pubkey":publisher,"database_plaintext_local":true,"read_policy":"public","cors_origins":[ORIGIN],"sync_worker_enabled":false,"auto_approve_publication_deletions":false,"backend":{"binary":binary,"data_dir":f._storage.path().join("real-projection"),"map_bytes":64*1024*1024,"timeout_seconds":5}}),
        ),
    };
    let mut config = (*f.state.config).clone();
    config.protocol_publication = Some(cfg.clone());
    config.validate().unwrap();
    f.state.config = Arc::new(config);
    f.state.protocol.verify_schema().await.unwrap();
    f.state
        .protocol
        .prepare_visibility_function()
        .await
        .unwrap();
    f.state
        .protocol
        .configure_publication(cfg.clone())
        .await
        .unwrap();
    f.state
        .db
        .client()
        .await
        .unwrap()
        .execute(
            "INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",
            &[&publisher, &f.owner_id],
        )
        .await
        .unwrap();
    f.app = weebdex_platform::router::router(f.state.clone());
    // The actual staff draft Publish route must create a real signed1002 even
    // before this instance has any chapter upload or running projection worker.
    let published_title = create_covered_manga(&f, "Bridge explicit signed draft review").await;
    let published_mid = published_title["id"].as_str().unwrap().to_owned();
    let published_event = published_title["protocol_event_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(published_title["publication_scope"], "original-protocol");
    assert_eq!(
        published_title["publication_status"],
        "accepted_pending_projection"
    );
    let signed: manga_node::auth::Event = serde_json::from_value(
        repo.pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT raw_event FROM events WHERE id=$1 AND state='hosted'",
                &[&id(&published_event).as_slice()],
            )
            .await
            .unwrap()
            .get(0),
    )
    .unwrap();
    signed.verify().unwrap();
    assert_eq!(signed.kind, 1002);
    assert_eq!(signed.pubkey, publisher);
    let private_pending = api(
        &f.app,
        Method::GET,
        &format!("/api/manga/draft/{published_mid}"),
        Some(&f.owner),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(private_pending["protocol_event_id"], published_event);
    assert_eq!(
        private_pending["publication_status"],
        "accepted_pending_projection"
    );
    assert_eq!(private_pending["is_unavailable"], true);
    assert!(
        private_pending.get("relationships").is_none(),
        "Pending reload must not disclose cached public relationships"
    );
    api(
        &f.app,
        Method::GET,
        &format!("/api/manga/draft/{published_mid}"),
        Some(&f.foreign),
        None,
        StatusCode::FORBIDDEN,
    )
    .await;
    api(
        &f.app,
        Method::GET,
        &format!("/api/manga/draft/{published_mid}"),
        None,
        None,
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let library_key = format!("synthetic-read-only-library-{}", f.owner_id);
    let library_hash = Sha256::digest(library_key.as_bytes()).to_vec();
    let library_id = format!("Bc{}", &f.owner_id[2..]);
    f.state.db.client().await.unwrap().execute(
        "INSERT INTO platform.api_clients(id,user_id,name,secret_hash,scopes) VALUES($1,$2,'Synthetic read-only owner fixture',$3,'[\"self.library.read\"]')",
        &[&library_id, &f.owner_id, &library_hash]).await.unwrap();
    let bearer = format!("Bearer {library_key}");
    send(
        &f.app,
        Method::GET,
        &format!("/api/manga/draft/{published_mid}"),
        None,
        &[("authorization", &bearer)],
        vec![],
        StatusCode::FORBIDDEN,
    )
    .await;
    // A lost materialization response must replay the actual accepted event even
    // after its temporary native cover pins have correctly been released.
    assert_eq!(
        f.state
            .db
            .client()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM platform.protocol_publication_originals WHERE command_id=$1",
                &[&format!("manga:{published_mid}")]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    let replay = api(
        &f.app,
        Method::POST,
        &format!("/api/manga/draft/{published_mid}/publish"),
        Some(&f.owner),
        Some(json!({"version":published_title["version"].as_i64().unwrap()-1})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(replay["protocol_event_id"], published_event);
    assert_eq!(replay["version"], published_title["version"]);
    api(
        &f.app,
        Method::GET,
        &format!("/api/manga/{published_mid}"),
        None,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    let unavailable_mapping = api(
        &f.app,
        Method::POST,
        "/api/protocol/resources",
        None,
        Some(json!({"resources":[{"kind":"manga","id":published_mid}]})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(unavailable_mapping["data"], json!([]));
    // Source acceptance survives a native review rollback, but recovery may
    // only restore the exact submitted native version rather than stale edits.
    let review_author = api(
        &f.app,
        Method::POST,
        "/api/author",
        Some(&f.owner),
        Some(json!({"name":"Original captured credit"})),
        StatusCode::CREATED,
    )
    .await;
    let mut review_draft = create_manga(&f, "Bridge signed title rollback", false).await;
    let review_mid = review_draft["id"].as_str().unwrap().to_owned();
    review_draft = api(
        &f.app,
        Method::PUT,
        &format!("/api/manga/{review_mid}"),
        Some(&f.owner),
        Some(json!({"version":review_draft["version"],"authors":[review_author["id"]]})),
        StatusCode::OK,
    )
    .await;
    add_cover(&f, &review_mid, &png(), StatusCode::OK).await;
    let submitted = api(
        &f.app,
        Method::POST,
        &format!("/api/manga/draft/{review_mid}/commit"),
        Some(&f.owner),
        Some(json!({"version":review_draft["version"]})),
        StatusCode::OK,
    )
    .await;
    let review_version = submitted["version"].as_i64().unwrap() as i32;
    let mut review_headers = HeaderMap::new();
    review_headers.insert("cookie", f.owner.parse().unwrap());
    let review_actor = domains::auth::require_principal(&f.state, &review_headers)
        .await
        .unwrap();
    let mut review_client = f.state.db.client().await.unwrap();
    let review_tx = review_client.transaction().await.unwrap();
    review_tx
        .query_one(
            "SELECT id FROM platform.catalog_manga WHERE id=$1 FOR UPDATE",
            &[&review_mid],
        )
        .await
        .unwrap();
    let review_receipt = weebdex_platform::protocol::publication::publish_native_manga(
        &f.state,
        &review_tx,
        &review_actor,
        &review_mid,
        review_version,
    )
    .await
    .unwrap()
    .unwrap();
    review_receipt.event.verify().unwrap();
    let review_event = review_receipt.event.id.clone();
    review_tx.rollback().await.unwrap();
    drop(review_client);
    assert_eq!(
        f.state
            .db
            .client()
            .await
            .unwrap()
            .query_one(
                "SELECT state FROM platform.catalog_manga WHERE id=$1",
                &[&review_mid]
            )
            .await
            .unwrap()
            .get::<_, String>(0),
        "submitted"
    );
    let prepared = api(
        &f.app,
        Method::GET,
        &format!("/api/manga/draft/{review_mid}"),
        Some(&f.owner),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(prepared["can_mutate"], false);
    assert_eq!(prepared["can_cancel"], false);
    assert_eq!(prepared["can_retry_publication"], true);
    assert_eq!(prepared["publication_retry_version"], review_version);
    assert!(prepared.get("protocol_event_id").is_none() || prepared["protocol_event_id"].is_null());
    for (method, path, payload) in [
        (
            Method::PUT,
            format!("/api/manga/{review_mid}"),
            Some(json!({"version":review_version,"title":"Forbidden prepared edit"})),
        ),
        (
            Method::POST,
            format!("/api/manga/draft/{review_mid}/reject"),
            Some(json!({"version":review_version})),
        ),
        (Method::POST, format!("/api/manga/{review_mid}/lock"), None),
        (Method::DELETE, format!("/api/manga/{review_mid}"), None),
    ] {
        api(
            &f.app,
            method,
            &path,
            Some(&f.owner),
            payload,
            StatusCode::CONFLICT,
        )
        .await;
    }
    add_cover(&f, &review_mid, &png(), StatusCode::CONFLICT).await;
    // Renaming a shared native author remains allowed; the prepared original
    // CBOR and pinned cover remain exactly the original signed input on retry.
    api(
        &f.app,
        Method::PUT,
        &format!("/api/author/{}", review_author["id"].as_str().unwrap()),
        Some(&f.owner),
        Some(json!({"version":review_author["version"],"name":"Later unrelated credit edit"})),
        StatusCode::OK,
    )
    .await;
    let mut review_client = f.state.db.client().await.unwrap();
    let retry_tx = review_client.transaction().await.unwrap();
    let replay = weebdex_platform::protocol::publication::publish_native_manga(
        &f.state,
        &retry_tx,
        &review_actor,
        &review_mid,
        review_version,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        replay.event.id, review_event,
        "Retry must replay immutable captured credit/cover input"
    );
    retry_tx.rollback().await.unwrap();
    drop(review_client);
    assert_eq!(
        f.state
            .db
            .client()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM platform.protocol_publication_originals WHERE command_id=$1",
                &[&format!("manga:{review_mid}")]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1,
        "A source-accepted native rollback must retain its immutable original input pin"
    );
    let session = begin(&f, &mid).await;
    let image = png();
    let jpg = jpeg();
    let files = file_array(
        upload(
            &f,
            &session,
            &[
                image_part("first.png", "image/png", &image),
                image_part("second.jpg", "image/jpeg", &jpg),
                image_part("repeated.png", "image/png", &image),
            ],
            Some(&f.owner),
            StatusCode::OK,
        )
        .await,
        3,
    );
    let ordered = vec![files[1].clone(), files[2].clone(), files[0].clone()];
    let payload = commit_payload("Bridge accepted before rollback", &ordered);
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
            title: Some("Bridge accepted before rollback"),
            language: "en",
        },
        page_order: ordered.iter().map(|v| v["id"].as_str().unwrap()).collect(),
    };
    let commit_hash = auth::hash(&serde_json::to_vec(&canonical).unwrap());
    let mut headers = HeaderMap::new();
    headers.insert("cookie", f.owner.parse().unwrap());
    let actor = domains::auth::require_principal(&f.state, &headers)
        .await
        .unwrap();
    let mut client = f.state.db.client().await.unwrap();
    let tx = client.transaction().await.unwrap();
    tx.query_one(
        "SELECT id FROM platform.upload_sessions WHERE id=$1 FOR UPDATE",
        &[&session],
    )
    .await
    .unwrap();
    let mut originals = Vec::new();
    let mut pages = Vec::new();
    for file in &ordered {
        let fid = file["id"].as_str().unwrap();
        let row=tx.query_one("SELECT o.digest,o.mime,o.bytes,o.width,o.height,o.storage_key FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id WHERE f.id=$1 AND f.session_id=$2",&[&fid,&session]).await.unwrap();
        let original = NativeOriginal {
            hash: row.get(0),
            mime: row.get(1),
            bytes: row.get::<_, i64>(2) as u64,
            width: row.get::<_, i32>(3) as u32,
            height: row.get::<_, i32>(4) as u32,
            storage_key: row.get(5),
        };
        pages.push(CatalogPage {
            name: format!(
                "{fid}.{}",
                if original.mime == "image/png" {
                    "png"
                } else {
                    "jpg"
                }
            ),
            dimensions: [original.width as i32, original.height as i32],
        });
        originals.push(original);
    }
    let mut record = ChapterPublication {
        id: domains::uploads::storage::short_id(),
        manga_id: mid.clone(),
        uploader_id: actor.user_id.clone(),
        uploader_name: actor.username.clone(),
        title: Some("Bridge accepted before rollback".into()),
        volume: Some("1".into()),
        chapter: Some("1".into()),
        language: "en".into(),
        node: ORIGIN.into(),
        pages,
        optimized_pages: vec![],
        groups: vec![],
        source_id: None,
        protocol_event_id: None,
    };
    // Fault point is the actual native hook after original signed acceptance.
    // No fabricated event, manual outbox acknowledgement or source visibility edit.
    let receipt = publish_native_upload(
        &f.state,
        &tx,
        &actor,
        &session,
        &commit_hash,
        &mut record,
        &originals,
        &serde_json::to_value(&canonical).unwrap(),
    )
    .await
    .unwrap()
    .unwrap();
    receipt.event.verify().unwrap();
    assert_eq!(receipt.state, "accepted_pending_projection");
    let cid = record.id.clone();
    let event = receipt.event.id.clone();
    tx.rollback().await.unwrap();
    drop(client);
    let row=f.state.db.client().await.unwrap().query_one("SELECT state,event_id,catalog_id FROM platform.protocol_publication_commands WHERE upload_session_id=$1",&[&session]).await.unwrap();
    assert_eq!(row.get::<_, String>(0), "source_accepted");
    assert_eq!(row.get::<_, String>(1), event);
    assert_eq!(row.get::<_, String>(2), cid);
    upload(
        &f,
        &session,
        &[image_part(
            "forbidden-after-acceptance.png",
            "image/png",
            &image,
        )],
        Some(&f.owner),
        StatusCode::CONFLICT,
    )
    .await;
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/upload/{session}"),
        Some(&f.owner),
        None,
        StatusCode::CONFLICT,
    )
    .await;
    let file = files[0]["id"].as_str().unwrap();
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/upload/{session}/{file}"),
        Some(&f.owner),
        None,
        StatusCode::CONFLICT,
    )
    .await;
    api(
        &f.app,
        Method::GET,
        &format!("/api/chapter/{cid}"),
        None,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    // Expiry cannot discard accepted originals; the commit-only retry window
    // still checks the exact typed payload, even when duplicate hashes are equal.
    f.state.db.client().await.unwrap().execute("UPDATE platform.upload_sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE id=$1", &[&session]).await.unwrap();
    let resumed = api(
        &f.app,
        Method::GET,
        "/api/upload",
        Some(&f.owner),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(resumed["id"], session);
    assert_eq!(resumed["draft"], payload["draft"]);
    assert_eq!(resumed["page_order"], payload["page_order"]);
    assert_eq!(resumed["is_expired"], true);
    assert_eq!(resumed["can_cancel"], false);
    assert_eq!(resumed["can_retry_commit"], true);
    let mut divergent = payload.clone();
    divergent["page_order"].as_array_mut().unwrap().swap(1, 2);
    api(
        &f.app,
        Method::POST,
        &format!("/api/upload/{session}/commit"),
        Some(&f.owner),
        Some(divergent),
        StatusCode::CONFLICT,
    )
    .await;
    let mut no_publication = f.state.clone();
    let mut local_config = (*no_publication.config).clone();
    local_config.protocol_publication = None;
    no_publication.config = Arc::new(local_config);
    let no_publication_app = weebdex_platform::router::router(no_publication);
    api(
        &no_publication_app,
        Method::POST,
        &format!("/api/upload/{session}/commit"),
        Some(&f.owner),
        Some(payload.clone()),
        StatusCode::SERVICE_UNAVAILABLE,
    )
    .await;
    let runtime = f.state.protocol.publication_runtime().await.unwrap();
    let mut listener = ListenerGuard(runtime.start_native_listener().await.unwrap());
    assert!(listener.0.is_some());
    // A second owner cannot attach to the already-owned original volume.
    assert!(
        weebdex_platform::protocol::publication::PublicationRuntime::open(repo.clone(), cfg)
            .await
            .is_err()
    );
    wait_source(&f, &event).await;
    wait_source(&f, &published_event).await;
    wait_source(&f, &review_event).await;
    // Six valid bounded localized descriptions make a fixed16-row aggregate
    // exceed2MiB. All signed siblings must still advance over adaptive pages.
    let mut large_sources = Vec::new();
    for index in 0..16 {
        let metadata = manga_node::series_metadata::SeriesMetadata {
            titles: BTreeMap::from([("en".into(), format!("Bridge byte-budget fixture {index}"))]),
            alt_titles: None,
            description: Some(
                ["en", "ja", "zh", "ko", "fr", "de"]
                    .into_iter()
                    .map(|language| (language.into(), "x".repeat(40_000)))
                    .collect(),
            ),
            categories: None,
            credits: None,
            original_language: Some("en".into()),
            status: Some("ongoing".into()),
            content_rating: Some("safe".into()),
            publication_demographic: None,
            year: None,
            cover: None,
        };
        let receipt = runtime
            .publish_operator(PublicationInput {
                kind: 1002,
                canonical_manifest: manga_node::platform_bridge::encode_series(&metadata).unwrap(),
                originals: BTreeMap::new(),
                created_at: None,
            })
            .await
            .unwrap();
        wait_source(&f, &receipt.event.id).await;
        large_sources.push(receipt.event.id);
    }
    let bounded = repo.platform_catalogue_page(None, 16).await.unwrap();
    assert!(bounded.entries.len() < 16 && !bounded.entries.is_empty());
    assert!(bounded.rejected.is_empty() && !bounded.exhausted);
    let visible_replay = api(
        &f.app,
        Method::POST,
        &format!("/api/manga/draft/{published_mid}/publish"),
        Some(&f.owner),
        Some(json!({"version":published_title["version"].as_i64().unwrap()-1})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(visible_replay["protocol_event_id"], published_event);
    assert_eq!(visible_replay["publication_status"], "locally_hosted");
    api(
        &f.app,
        Method::GET,
        &format!("/api/manga/draft/{published_mid}"),
        Some(&f.owner),
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    f.state.runtime.mark_ready();
    let capabilities = api(
        &f.app,
        Method::GET,
        "/api/protocol/capabilities",
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(capabilities["available"], true);
    assert_eq!(capabilities["service_key"], publisher);
    assert_eq!(capabilities["origin"], origin);
    assert!(
        capabilities["catalog_namespace"]
            .as_str()
            .is_some_and(|namespace| namespace.len() == 36)
    );
    assert!(
        capabilities["connect_origins"]
            .as_array()
            .unwrap()
            .iter()
            .any(|allowed| allowed == &origin)
    );
    assert_eq!(
        api(
            &f.app,
            Method::GET,
            "/api/protocol/capabilities",
            None,
            None,
            StatusCode::OK
        )
        .await["catalog_namespace"],
        capabilities["catalog_namespace"]
    );
    for _ in 0..3 {
        sweep(&f).await;
    }
    assert_eq!(f.state.db.client().await.unwrap().query_one(
        "SELECT count(*) FROM platform.protocol_import_status WHERE event_id=ANY($1) AND status='imported'",
        &[&large_sources]).await.unwrap().get::<_,i64>(0),16,
        "Valid large source siblings must not wedge the durable importer cursor");
    let reviewed = api(
        &f.app,
        Method::GET,
        &format!("/api/manga/{review_mid}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(reviewed["version"], review_version + 1);
    assert_eq!(
        f.state
            .db
            .client()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM platform.protocol_publication_originals WHERE command_id=$1",
                &[&format!("manga:{review_mid}")]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0,
        "Recovered native materialization releases its original input pins"
    );
    let mappings=api(&f.app,Method::POST,"/api/protocol/resources",None,Some(json!({"resources":[{"kind":"manga","id":published_mid},{"kind":"manga","id":review_mid},{"kind":"chapter","id":cid}]})),StatusCode::OK).await;
    assert_eq!(mappings["data"].as_array().unwrap().len(), 3);
    for mapping in mappings["data"].as_array().unwrap() {
        assert_eq!(mapping["publisher"], publisher);
        assert!(auth::valid_hash(mapping["event_id"].as_str().unwrap()));
        assert!(manga_node::mangadex_compat::valid_uuid(
            mapping["compatibility_id"].as_str().unwrap()
        ));
    }
    let public = api(
        &f.app,
        Method::GET,
        &format!("/api/chapter/{cid}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(public["id"], cid);
    assert_eq!(public["protocol_event_id"], event);
    assert_eq!(public["publication_status"], "locally_hosted");
    assert_eq!(public["data"].as_array().unwrap().len(), 3);
    assert_eq!(public["data"][1]["name"], public["data"][2]["name"]);
    let replay = api(
        &f.app,
        Method::POST,
        &format!("/api/upload/{session}/commit"),
        Some(&f.owner),
        Some(payload.clone()),
        StatusCode::OK,
    )
    .await;
    assert_eq!(replay["id"], cid);
    assert_eq!(replay["protocol_event_id"], event);
    for (index, bytes) in [&jpg, &image, &image].into_iter().enumerate() {
        let name = public["data"][index]["name"].as_str().unwrap();
        let path = format!("/data/{cid}/{name}");
        let full = send(
            &f.app,
            Method::GET,
            &path,
            None,
            &[],
            vec![],
            StatusCode::OK,
        )
        .await;
        assert_eq!(full.body, *bytes);
        assert_eq!(full.header("cache-control"), "no-store");
        let range = send(
            &f.app,
            Method::GET,
            &path,
            None,
            &[("range", "bytes=0-7")],
            vec![],
            StatusCode::PARTIAL_CONTENT,
        )
        .await;
        assert_eq!(range.body, bytes[..8]);
        send(
            &f.app,
            Method::HEAD,
            &path,
            None,
            &[],
            vec![],
            StatusCode::OK,
        )
        .await;
    }
    // Real approved-peer transport is exercised through the embedded listener.
    // This is same-instance idempotent synchronization, not multi-node delivery.
    let request = || manga_node::peers::SyncRequest {
        peer: publisher.clone(),
        event: event.clone(),
        filter: String::new(),
        fallbacks: vec![],
    };
    assert!(runtime.synchronize_approved_peer(request()).await.is_err());
    repo.set_peer(id(&publisher), &origin, true).await.unwrap();
    let synced = runtime.synchronize_approved_peer(request()).await.unwrap();
    assert!(synced.hosted && synced.projected);
    assert_eq!(synced.event, event);
    repo.set_peer(id(&publisher), &origin, false).await.unwrap();
    assert!(runtime.synchronize_approved_peer(request()).await.is_err());
    // A separate original authority database/store/listener must fetch and retain
    // actual signed source events and originals, never proxy the primary reader.
    let peer_database = format!("protocol_peer_{}", hex::encode(rand::random::<[u8; 6]>()));
    f.state
        .db
        .client()
        .await
        .unwrap()
        .batch_execute(&format!("CREATE DATABASE {peer_database}"))
        .await
        .unwrap();
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").unwrap();
    assert!(dsn.contains("dbname=weebdex_platform_test"));
    let peer_dsn = dsn.replace(
        "dbname=weebdex_platform_test",
        &format!("dbname={peer_database}"),
    );
    let peer_storage = tempfile::Builder::new()
        .prefix("protocol-distinct-peer-")
        .tempdir_in(f._storage.path())
        .unwrap();
    let peer_state = AppState::connect(
        Config {
            public_origin: ORIGIN.into(),
            database_plaintext_local: true,
            uploads_dir: peer_storage.path().join("native"),
            database_connections: 4,
            registration_enabled: false,
            ..Config::default()
        },
        peer_dsn.clone(),
    )
    .await
    .unwrap();
    peer_state.db.migrate(domains::MIGRATIONS).await.unwrap();
    let mut peer_repo = Repository::connect(DbConfig {
        dsn: peer_dsn,
        connections: 1,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1024 * 1024,
        plaintext_local: true,
    })
    .await
    .unwrap();
    peer_repo.pool = peer_state.db.pool().clone();
    peer_repo.migrate().await.unwrap();
    let (peer_secret, peer_publisher) = loop {
        let secret = hex::encode(rand::random::<[u8; 32]>());
        if let Ok(event) = auth::sign(&secret, 1, vec![], String::new()) {
            break (zeroize::Zeroizing::new(secret), event.pubkey);
        }
    };
    peer_repo
        .set_publisher_admission(id(&peer_publisher), true)
        .await
        .unwrap();
    peer_repo
        .set_publisher_admission(id(&publisher), true)
        .await
        .unwrap();
    let peer_key = peer_storage.path().join("SYNTHETIC_EPHEMERAL_peer.key");
    std::fs::write(&peer_key, peer_secret.as_bytes()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&peer_key, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let peer_bind = reservation.local_addr().unwrap();
    drop(reservation);
    let peer_origin = format!("http://{peer_bind}");
    peer_state.protocol.verify_schema().await.unwrap();
    peer_state
        .protocol
        .prepare_visibility_function()
        .await
        .unwrap();
    peer_state.protocol.configure_publication(OperatorRuntimeConfig{operator_key_file:peer_key.clone(),expected_publisher:peer_publisher.clone(),storage_owner:peer_publisher.clone(),storage_holder:peer_publisher.clone(),storage:json!({"path":peer_storage.path().join("originals"),"volume":"BridgeDistinctPeer","physical_bytes":64*1024*1024,"staging_bytes":64*1024*1024,"max_blob_bytes":8*1024*1024,"upload_slots":2}),group_keys_dir:None,backend:None,native_node:Some(json!({"bind":peer_bind.to_string(),"base_url":peer_origin,"service_key_file":peer_key,"admin_pubkey":peer_publisher,"database_plaintext_local":true,"read_policy":"public","cors_origins":[ORIGIN],"sync_worker_enabled":false,"auto_approve_publication_deletions":false,"backend":{"binary":binary,"data_dir":peer_storage.path().join("real-projection"),"map_bytes":64*1024*1024,"timeout_seconds":5}}))}).await.unwrap();
    let peer_runtime = peer_state.protocol.publication_runtime().await.unwrap();
    let mut peer_listener = ListenerGuard(peer_runtime.start_native_listener().await.unwrap());
    let peer = Fixture {
        app: weebdex_platform::router::router(peer_state.clone()),
        state: peer_state,
        owner: f.owner.clone(),
        owner_id: f.owner_id.clone(),
        foreign: f.foreign.clone(),
        _storage: peer_storage,
    };
    let series_event: String = f
        .state
        .db
        .client()
        .await
        .unwrap()
        .query_one(
            "SELECT protocol_series_id FROM platform.catalog_manga WHERE id=$1",
            &[&mid],
        )
        .await
        .unwrap()
        .get(0);
    let transfer = |event: String| manga_node::peers::SyncRequest {
        peer: publisher.clone(),
        event,
        filter: String::new(),
        fallbacks: vec![],
    };
    assert!(
        peer_runtime
            .synchronize_approved_peer(transfer(series_event.clone()))
            .await
            .is_err(),
        "Unapproved peer must not fetch content"
    );
    peer_repo
        .set_peer(id(&publisher), &origin, true)
        .await
        .unwrap();
    repo.set_peer(id(&peer_publisher), &peer_origin, true)
        .await
        .unwrap();
    let imported = peer_runtime
        .synchronize_approved_peer(transfer(series_event.clone()))
        .await
        .unwrap();
    assert!(imported.hosted && imported.matched);
    wait_source(&peer, &series_event).await;
    let imported = peer_runtime
        .synchronize_approved_peer(transfer(event.clone()))
        .await
        .unwrap();
    assert!(imported.hosted && imported.matched);
    assert_eq!(
        imported.originals_fetched, 2,
        "Duplicate original hashes transfer once"
    );
    wait_source(&peer, &event).await;
    for _ in 0..3 {
        sweep(&peer).await;
    }
    let peer_cid: String = peer
        .state
        .db
        .client()
        .await
        .unwrap()
        .query_one(
            "SELECT id FROM platform.catalog_chapters WHERE protocol_event_id=$1",
            &[&event],
        )
        .await
        .unwrap()
        .get(0);
    let peer_page = public["data"][0]["name"].as_str().unwrap();
    let peer_path = format!("/data/{peer_cid}/{peer_page}");
    assert_eq!(
        send(
            &peer.app,
            Method::GET,
            &peer_path,
            None,
            &[],
            vec![],
            StatusCode::OK
        )
        .await
        .body,
        jpg
    );
    // Revoke the approval and reject further transport without erasing retained
    // local originals; peer policy and local resource moderation are independent.
    peer_repo
        .set_peer(id(&publisher), &origin, false)
        .await
        .unwrap();
    assert!(
        peer_runtime
            .synchronize_approved_peer(transfer(event.clone()))
            .await
            .is_err()
    );
    assert_eq!(
        send(
            &peer.app,
            Method::GET,
            &peer_path,
            None,
            &[],
            vec![],
            StatusCode::OK
        )
        .await
        .body,
        jpg
    );
    // A distinct source-only series checks importer cover aliases/materialization.
    let cover_hash = auth::hash(&image);
    let series = manga_node::series_metadata::SeriesMetadata {
        titles: BTreeMap::from([("en".into(), "Bridge source cover".into())]),
        alt_titles: None,
        description: None,
        categories: Some(vec!["BridgeFixtureTag".into()]),
        credits: None,
        original_language: Some("en".into()),
        status: Some("completed".into()),
        content_rating: Some("safe".into()),
        publication_demographic: None,
        year: Some(2026),
        cover: Some(manga_node::manifest::Page {
            hash: cover_hash.clone(),
            size: image.len() as u64,
            mime: "image/png".into(),
            width: 128,
            height: 160,
        }),
    };
    let cover_event = runtime
        .publish_operator(PublicationInput {
            kind: 1002,
            canonical_manifest: manga_node::platform_bridge::encode_series(&series).unwrap(),
            originals: BTreeMap::from([(cover_hash, image.clone())]),
            created_at: None,
        })
        .await
        .unwrap()
        .event;
    wait_source(&f, &cover_event.id).await;
    sweep(&f).await;
    assert_eq!(covers::materialize_page(&f.state).await.unwrap(), 1);
    let row=f.state.db.client().await.unwrap().query_one("SELECT m.id,c.id FROM platform.catalog_manga m JOIN platform.catalog_covers c ON c.manga_id=m.id WHERE m.protocol_series_id=$1 AND c.protocol_event_id=$1",&[&cover_event.id]).await.unwrap();
    let source_mid: String = row.get(0);
    let cover_id: String = row.get(1);
    let source_cover = api(
        &f.app,
        Method::GET,
        &format!("/api/cover/{cover_id}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(source_cover["ext"], ".png");
    assert_eq!(
        f.state
            .db
            .client()
            .await
            .unwrap()
            .query_one(
                "SELECT ext FROM platform.catalog_covers WHERE id=$1",
                &[&cover_id]
            )
            .await
            .unwrap()
            .get::<_, String>(0),
        "png"
    );
    let source_manga = api(
        &f.app,
        Method::GET,
        &format!("/api/manga/{source_mid}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(source_manga["relationships"]["cover"], source_cover);
    let source_covers = api(
        &f.app,
        Method::GET,
        &format!("/api/manga/{source_mid}/covers"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(source_covers, json!([source_cover.clone()]));
    // Imported and native covers use the same public DTO and retained id+ext URL.
    let original_path = format!(
        "/covers/{source_mid}/{cover_id}{}",
        source_cover["ext"].as_str().unwrap()
    );
    let original = send(
        &f.app,
        Method::GET,
        &original_path,
        None,
        &[],
        vec![],
        StatusCode::OK,
    )
    .await;
    assert_eq!(original.header("content-type"), "image/png");
    assert_eq!(original.body, image);
    let original_etag = original.header("etag").to_owned();
    let head = send(
        &f.app,
        Method::HEAD,
        &original_path,
        None,
        &[],
        vec![],
        StatusCode::OK,
    )
    .await;
    assert!(head.body.is_empty());
    assert_eq!(head.header("content-length"), image.len().to_string());
    let ranged = send(
        &f.app,
        Method::GET,
        &original_path,
        None,
        &[("range", "bytes=0-2")],
        vec![],
        StatusCode::PARTIAL_CONTENT,
    )
    .await;
    assert_eq!(ranged.body, image[..3]);
    send(
        &f.app,
        Method::GET,
        &format!("/covers/{source_mid}/{cover_id}png"),
        None,
        &[],
        vec![],
        StatusCode::BAD_REQUEST,
    )
    .await;
    send(
        &f.app,
        Method::GET,
        &format!("/covers/{source_mid}/{cover_id}.jpg"),
        None,
        &[],
        vec![],
        StatusCode::NOT_FOUND,
    )
    .await;
    let mut derivative_etag = String::new();
    for variant in [256, 512] {
        let path = format!("/covers/{source_mid}/{cover_id}.{variant}.webp");
        let response = send(
            &f.app,
            Method::GET,
            &path,
            None,
            &[],
            vec![],
            StatusCode::OK,
        )
        .await;
        assert_eq!(
            image::guess_format(&response.body).unwrap(),
            ImageFormat::WebP
        );
        assert_eq!(
            image::load_from_memory(&response.body).unwrap().width(),
            128
        );
        derivative_etag = response.header("etag").to_owned();
    }
    // Signed deletion is only a request until explicit original moderator approval.
    let deletion = runtime
        .request_operator_deletion(
            vec![cover_event.id.clone()],
            "Synthetic fixture moderator review".into(),
        )
        .await
        .unwrap();
    api(
        &f.app,
        Method::GET,
        &format!("/api/manga/{source_mid}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    let url = format!("{origin}/admin/deletions/{}/approve", deletion.id);
    let body = b"{}";
    let response = reqwest::Client::new()
        .post(&url)
        .header("authorization", proof(&secret, &url, body))
        .body(body.to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    // No importer sweep between approval and read: immediate canonical policy guard.
    api(
        &f.app,
        Method::GET,
        &format!("/api/manga/{source_mid}"),
        None,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    send(
        &f.app,
        Method::GET,
        &original_path,
        None,
        &[("if-none-match", &original_etag)],
        vec![],
        StatusCode::NOT_FOUND,
    )
    .await;
    api(
        &f.app,
        Method::GET,
        &format!("/api/cover/{cover_id}"),
        None,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    send(
        &f.app,
        Method::GET,
        &format!("/covers/{source_mid}/{cover_id}.512.webp"),
        None,
        &[("if-none-match", &derivative_etag)],
        vec![],
        StatusCode::NOT_FOUND,
    )
    .await;
    let title_deletion = runtime
        .request_operator_deletion(
            vec![published_event.clone()],
            "Synthetic current-authority materialized title replay revocation".into(),
        )
        .await
        .unwrap();
    let title_url = format!("{origin}/admin/deletions/{}/approve", title_deletion.id);
    assert_eq!(
        reqwest::Client::new()
            .post(&title_url)
            .header("authorization", proof(&secret, &title_url, body))
            .body(body.to_vec())
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::ACCEPTED
    );
    api(
        &f.app,
        Method::GET,
        &format!("/api/manga/draft/{published_mid}"),
        Some(&f.owner),
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    api(
        &f.app,
        Method::POST,
        &format!("/api/manga/draft/{published_mid}/publish"),
        Some(&f.owner),
        Some(json!({"version":published_title["version"].as_i64().unwrap()-1})),
        StatusCode::NOT_FOUND,
    )
    .await;
    let deletion = send(
        &f.app,
        Method::DELETE,
        &format!("/api/chapter/{cid}"),
        Some(&f.owner),
        &[],
        vec![],
        StatusCode::NO_CONTENT,
    )
    .await;
    assert_eq!(
        deletion.header("x-weebdex-source-deletion-status"),
        "moderator-review-requested"
    );
    assert_eq!(deletion.header("x-weebdex-instance-removal"), "soft-hidden");
    api(
        &f.app,
        Method::POST,
        &format!("/api/upload/{session}/commit"),
        Some(&f.owner),
        Some(payload.clone()),
        StatusCode::NOT_FOUND,
    )
    .await;
    let hidden_mapping = api(
        &f.app,
        Method::POST,
        "/api/protocol/resources",
        None,
        Some(json!({"resources":[{"kind":"chapter","id":cid}]})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(hidden_mapping["data"], json!([]));
    let deletion_id = deletion.header("x-weebdex-source-deletion-request");
    assert!(auth::valid_hash(deletion_id));
    assert!(
        f.state.protocol.entry(id(&event)).await.unwrap().is_some(),
        "Native explicit hide submits a request without source approval"
    );
    let url = format!("{origin}/admin/deletions/{deletion_id}/approve");
    assert_eq!(
        reqwest::Client::new()
            .post(&url)
            .header("authorization", proof(&secret, &url, body))
            .body(body.to_vec())
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::ACCEPTED
    );
    api(
        &f.app,
        Method::GET,
        &format!("/api/chapter/{cid}"),
        None,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    let page = public["data"][0]["name"].as_str().unwrap();
    send(
        &f.app,
        Method::GET,
        &format!("/data/{cid}/{page}"),
        None,
        &[],
        vec![],
        StatusCode::NOT_FOUND,
    )
    .await;
    assert_eq!(
        send(
            &peer.app,
            Method::GET,
            &peer_path,
            None,
            &[],
            vec![],
            StatusCode::OK
        )
        .await
        .body,
        jpg,
        "Primary moderator removal must not silently delete a peer's local originals"
    );
    let mut peer_native = peer_listener.0.take().unwrap();
    peer_native.shutdown.take().unwrap().send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), &mut peer_native.task)
        .await
        .expect("Distinct peer listener did not stop")
        .unwrap()
        .unwrap();
    assert!(std::net::TcpListener::bind(peer_bind).is_ok());
    // Stage a real original CBOR, then block a genuine signed publication after
    // its owned spawn at the source module's event-verdict lock. Disconnect the
    // client and request shutdown: completion must wait for durable finalization.
    let drain_metadata = manga_node::series_metadata::SeriesMetadata {
        titles: BTreeMap::from([(
            "en".into(),
            "Synthetic disconnected publication drain".into(),
        )]),
        alt_titles: None,
        description: None,
        categories: None,
        credits: None,
        original_language: Some("en".into()),
        status: Some("ongoing".into()),
        content_rating: Some("safe".into()),
        publication_demographic: None,
        year: None,
        cover: None,
    };
    let drain_manifest = manga_node::platform_bridge::encode_series(&drain_metadata).unwrap();
    let drain_hash = auth::hash(&drain_manifest);
    // Blossom's original verifier requires the signed second to be strictly
    // earlier than its receive second, not merely within the expiry window.
    let upload_proof = auth::sign(
        &secret,
        24242,
        vec![
            vec!["t".into(), "upload".into()],
            vec!["server".into(), origin.clone()],
            vec!["x".into(), drain_hash.clone()],
            vec!["expiration".into(), (auth::now() + 120).to_string()],
        ],
        "Synthetic bounded drain staging".into(),
    )
    .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while auth::now() <= upload_proof.created_at {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Synthetic Blossom proof did not reach its valid receive second");
    auth::blossom(&upload_proof, &origin, "upload", &drain_hash).unwrap();
    use base64::{Engine, engine::general_purpose::STANDARD};
    assert_eq!(
        reqwest::Client::new()
            .put(format!("{origin}/upload/{drain_hash}"))
            .header(
                "authorization",
                format!(
                    "Nostr {}",
                    STANDARD.encode(serde_json::to_vec(&upload_proof).unwrap())
                )
            )
            .body(drain_manifest)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let drain_event =
        manga_node::platform_bridge::sign_publication_at(&secret, 1002, &drain_hash, auth::now())
            .unwrap();
    let mut barrier_client = f.state.db.client().await.unwrap();
    let barrier = barrier_client.transaction().await.unwrap();
    barrier.query_one("SELECT pg_advisory_xact_lock(hashtextextended('manga:review-verdict:'||encode($1::bytea,'hex'),0))",&[&id(&drain_event.id).as_slice()]).await.unwrap();
    let drain_body = serde_json::to_vec(&drain_event).unwrap();
    let drain_url = format!("{origin}/publications");
    let drain_proof = proof(&secret, &drain_url, &drain_body);
    let disconnected = tokio::spawn(async move {
        reqwest::Client::new()
            .post(drain_url)
            .header("authorization", drain_proof)
            .body(drain_body)
            .send()
            .await
    });
    tokio::time::timeout(Duration::from_millis(700),async {
        loop {
            let blocked:bool=f.state.db.client().await.unwrap().query_one(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE '%manga:review-verdict:%')",&[]).await.unwrap().get(0);
            if blocked {break;}
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("Genuine signed native publication did not reach its owned blocked mutation");
    disconnected.abort();
    let _ = disconnected.await;
    let mut native = listener.0.take().unwrap();
    native.shutdown.take().unwrap().send(()).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !native.task.is_finished(),
        "Native listener must not claim successful drain while its disconnected mutation remains blocked"
    );
    barrier.commit().await.unwrap();
    drop(barrier_client);
    tokio::time::timeout(Duration::from_secs(10), &mut native.task)
        .await
        .expect("Embedded listener did not stop within bounded grace")
        .unwrap()
        .unwrap();
    assert!(
        repo.pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM events WHERE id=$1 AND state='hosted')",
                &[&id(&drain_event.id).as_slice()]
            )
            .await
            .unwrap()
            .get::<_, bool>(0),
        "Successful native drain must observe the actual durable publication outcome"
    );
    assert_eq!(
        send(
            &peer.app,
            Method::GET,
            &peer_path,
            None,
            &[],
            vec![],
            StatusCode::OK
        )
        .await
        .body,
        jpg,
        "Retained peer bytes must not require the source listener after shutdown"
    );
    assert!(
        std::net::TcpListener::bind(bind).is_ok(),
        "Embedded listener remained bound after shutdown"
    );
}
