//! Public upload/reader acceptance seams backed by the owned disposable PostgreSQL runner.
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
use image::{
    AnimationDecoder, Delay, DynamicImage, Frame, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage,
};
use rand::{Rng, distributions::Alphanumeric};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{io::Cursor, path::PathBuf};
use tempfile::TempDir;
use tower::ServiceExt;
use weebdex_platform::{AppState, Config, domains};

const ORIGIN: &str = "https://upload.fixture.invalid";
const MAX_FILE_BYTES: usize = 32 * 1024 * 1024;

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
                "manga.state.update",
                "manga.cover.edit",
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

/// Assert full bodies and public delivery semantics, including revalidation and
/// conditional Range/HEAD behavior. The expected hash comes from actual bytes.
async fn media_contract(
    f: &Fixture,
    path: &str,
    cookie: Option<&str>,
    original: &[u8],
    mime: &str,
    private: bool,
) -> String {
    let reply = send(
        &f.app,
        Method::GET,
        path,
        cookie,
        &[],
        vec![],
        StatusCode::OK,
    )
    .await;
    assert_eq!(reply.body, original);
    assert_eq!(reply.header("content-type"), mime);
    assert_eq!(reply.header("content-length"), original.len().to_string());
    assert_eq!(reply.header("accept-ranges"), "bytes");
    assert_eq!(reply.header("x-content-type-options"), "nosniff");
    assert_eq!(
        reply.header("cache-control"),
        if private {
            "private, no-store"
        } else {
            "no-cache, must-revalidate"
        }
    );
    let etag = format!("\"{}\"", hex::encode(Sha256::digest(original)));
    assert_eq!(reply.header("etag"), etag);
    let head = send(
        &f.app,
        Method::HEAD,
        path,
        cookie,
        &[],
        vec![],
        StatusCode::OK,
    )
    .await;
    assert!(head.body.is_empty());
    assert_eq!(head.header("content-length"), original.len().to_string());
    assert_eq!(head.header("etag"), etag);
    let conditional = send(
        &f.app,
        Method::GET,
        path,
        cookie,
        &[("if-none-match", &etag)],
        vec![],
        StatusCode::NOT_MODIFIED,
    )
    .await;
    assert!(conditional.body.is_empty());
    assert_eq!(conditional.header("etag"), etag);
    let weak = format!("W/{etag}");
    let conditional_head = send(
        &f.app,
        Method::HEAD,
        path,
        cookie,
        &[("if-none-match", &weak)],
        vec![],
        StatusCode::NOT_MODIFIED,
    )
    .await;
    assert!(conditional_head.body.is_empty());
    let range = send(
        &f.app,
        Method::GET,
        path,
        cookie,
        &[("range", "bytes=2-9"), ("if-range", &etag)],
        vec![],
        StatusCode::PARTIAL_CONTENT,
    )
    .await;
    assert_eq!(range.body, original[2..10]);
    assert_eq!(range.header("content-length"), "8");
    assert_eq!(
        range.header("content-range"),
        format!("bytes 2-9/{}", original.len())
    );
    let suffix = send(
        &f.app,
        Method::GET,
        path,
        cookie,
        &[("range", "bytes=-7")],
        vec![],
        StatusCode::PARTIAL_CONTENT,
    )
    .await;
    assert_eq!(suffix.body, original[original.len() - 7..]);
    let range_head = send(
        &f.app,
        Method::HEAD,
        path,
        cookie,
        &[("range", "bytes=2-9")],
        vec![],
        StatusCode::PARTIAL_CONTENT,
    )
    .await;
    assert!(range_head.body.is_empty());
    assert_eq!(range_head.header("content-length"), "8");
    let stale = send(
        &f.app,
        Method::GET,
        path,
        cookie,
        &[("range", "bytes=2-9"), ("if-range", "\"stale\"")],
        vec![],
        StatusCode::OK,
    )
    .await;
    assert_eq!(stale.body, original);
    for bad_range in [
        format!("bytes={}-", original.len()),
        "bytes=9-2".into(),
        "bytes=0-1,3-4".into(),
    ] {
        let invalid = send(
            &f.app,
            Method::GET,
            path,
            cookie,
            &[("range", &bad_range)],
            vec![],
            StatusCode::RANGE_NOT_SATISFIABLE,
        )
        .await;
        assert!(invalid.body.is_empty());
        assert_eq!(
            invalid.header("content-range"),
            format!("bytes */{}", original.len())
        );
    }
    etag
}

#[tokio::test]
#[ignore = "Requires the explicitly isolated PostgreSQL runner after implementation freeze"]
async fn persisted_upload_commit_reader_originals_derivatives_and_retries() {
    let f = fixture().await;
    let manga = create_manga(&f, "Up multipart reader fixture", true).await;
    let mid = manga["id"].as_str().unwrap();
    api(
        &f.app,
        Method::GET,
        "/api/upload",
        None,
        None,
        StatusCode::UNAUTHORIZED,
    )
    .await;
    assert_eq!(
        api(
            &f.app,
            Method::GET,
            "/api/upload",
            Some(&f.owner),
            None,
            StatusCode::NO_CONTENT
        )
        .await,
        Value::Null
    );
    api(
        &f.app,
        Method::GET,
        "/api/upload",
        Some(&f.foreign),
        None,
        StatusCode::NO_CONTENT,
    )
    .await;
    api(
        &f.app,
        Method::POST,
        "/api/upload/begin",
        None,
        Some(json!({"manga":mid})),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    api(
        &f.app,
        Method::POST,
        "/api/upload/check-approval-required",
        Some(&f.owner),
        Some(json!({"language":"en"})),
        StatusCode::NO_CONTENT,
    )
    .await;
    let session = begin(&f, mid).await;
    assert_eq!(
        api(
            &f.app,
            Method::GET,
            "/api/upload",
            Some(&f.owner),
            None,
            StatusCode::OK
        )
        .await["id"],
        session
    );
    api(
        &f.app,
        Method::POST,
        "/api/upload/begin",
        Some(&f.owner),
        Some(json!({"manga":mid})),
        StatusCode::CONFLICT,
    )
    .await;
    let png = png();
    let jpg = jpeg();
    let files = file_array(
        upload(
            &f,
            &session,
            &[
                image_part("page-one.png", "image/png", &png),
                image_part("page-two.jpeg", "image/jpeg", &jpg),
                image_part("same-original-again.png", "image/png", &png),
            ],
            Some(&f.owner),
            StatusCode::OK,
        )
        .await,
        3,
    );
    assert_ne!(files[0]["id"], files[2]["id"]);
    assert_eq!(files[0]["file_hash"], files[2]["file_hash"]);
    assert_eq!(files[0]["file_hash"], hex::encode(Sha256::digest(&png)));
    assert_eq!(files[1]["file_ext"], ".jpg");
    assert_eq!(files[1]["file_size"], jpg.len().to_string());
    let preview = format!("/uploads/{}", files[0]["file_name"].as_str().unwrap());
    send(
        &f.app,
        Method::GET,
        &preview,
        None,
        &[],
        vec![],
        StatusCode::UNAUTHORIZED,
    )
    .await;
    send(
        &f.app,
        Method::GET,
        &preview,
        Some(&f.foreign),
        &[],
        vec![],
        StatusCode::NOT_FOUND,
    )
    .await;
    media_contract(&f, &preview, Some(&f.owner), &png, "image/png", true).await;
    upload(
        &f,
        &session,
        &[image_part("foreign.png", "image/png", &png)],
        Some(&f.foreign),
        StatusCode::NOT_FOUND,
    )
    .await;
    upload(
        &f,
        &session,
        &[image_part("anonymous.png", "image/png", &png)],
        None,
        StatusCode::UNAUTHORIZED,
    )
    .await;
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/upload/{session}/{}", files[0]["id"].as_str().unwrap()),
        Some(&f.foreign),
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/upload/{session}"),
        Some(&f.foreign),
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    let ordered = vec![files[1].clone(), files[2].clone(), files[0].clone()];
    let payload = commit_payload("Up exact ordered page set", &ordered);
    // A page reload restores the private form and exact file order. Saving is
    // versioned independently of chapter publication and cannot overwrite a
    // newer acknowledgement from another tab.
    let current = api(
        &f.app,
        Method::GET,
        "/api/upload",
        Some(&f.owner),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(current["state"], "open");
    assert_eq!(current["can_mutate"], true);
    assert_eq!(current["can_cancel"], true);
    assert_eq!(current["target_manga"]["id"], mid);
    let saved_input = json!({"version":current["version"],"draft":payload["draft"],"page_order":payload["page_order"]});
    api(
        &f.app,
        Method::PATCH,
        &format!("/api/upload/{session}"),
        Some(&f.foreign),
        Some(saved_input.clone()),
        StatusCode::NOT_FOUND,
    )
    .await;
    let saved = api(
        &f.app,
        Method::PATCH,
        &format!("/api/upload/{session}"),
        Some(&f.owner),
        Some(saved_input.clone()),
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        saved["version"].as_i64(),
        current["version"].as_i64().map(|v| v + 1)
    );
    assert_eq!(saved["draft"], payload["draft"]);
    assert_eq!(saved["page_order"], payload["page_order"]);
    assert_eq!(saved["relationships"]["files"], json!(ordered));
    api(
        &f.app,
        Method::PATCH,
        &format!("/api/upload/{session}"),
        Some(&f.owner),
        Some(saved_input),
        StatusCode::CONFLICT,
    )
    .await;
    let resumed = api(
        &f.app,
        Method::GET,
        "/api/upload",
        Some(&f.owner),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(resumed["draft"], saved["draft"]);
    assert_eq!(resumed["page_order"], saved["page_order"]);
    assert_eq!(
        resumed["relationships"]["files"],
        saved["relationships"]["files"]
    );
    assert_eq!(resumed["version"], saved["version"]);
    api(&f.app, Method::PATCH, &format!("/api/upload/{session}"), Some(&f.owner), Some(json!({"version":saved["version"],"draft":payload["draft"],"page_order":[files[0]["id"]]})), StatusCode::BAD_REQUEST).await;
    api(
        &f.app,
        Method::POST,
        &format!("/api/upload/{session}/commit"),
        Some(&f.foreign),
        Some(payload.clone()),
        StatusCode::NOT_FOUND,
    )
    .await;
    for bad in [
        vec![
            files[0]["id"].clone(),
            files[0]["id"].clone(),
            files[2]["id"].clone(),
        ],
        vec![files[0]["id"].clone()],
        vec![
            files[0]["id"].clone(),
            files[1]["id"].clone(),
            json!("Otherf0001"),
        ],
        vec![],
    ] {
        api(
            &f.app,
            Method::POST,
            &format!("/api/upload/{session}/commit"),
            Some(&f.owner),
            Some(json!({"draft":draft("Up invalid order"),"page_order":bad})),
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    send(
        &f.app,
        Method::GET,
        &preview,
        Some(&f.owner),
        &[],
        vec![],
        StatusCode::OK,
    )
    .await;
    let chapter = api(
        &f.app,
        Method::POST,
        &format!("/api/upload/{session}/commit"),
        Some(&f.owner),
        Some(payload.clone()),
        StatusCode::OK,
    )
    .await;
    assert_eq!(chapter["publication_scope"], "instance-local");
    assert_eq!(chapter["publication_status"], "local");
    assert!(chapter.get("protocol_event_id").is_none());
    let cid = chapter["id"].as_str().unwrap();
    assert_eq!(chapter["data"].as_array().unwrap().len(), 3);
    for (page, file) in chapter["data"].as_array().unwrap().iter().zip(&ordered) {
        assert_eq!(page["name"], file["file_name"]);
    }
    assert_eq!(chapter["data"][0]["dimensions"], json!([96, 144]));
    assert_eq!(chapter["data"][1]["dimensions"], json!([128, 160]));
    assert_eq!(
        api(
            &f.app,
            Method::POST,
            &format!("/api/upload/{session}/commit"),
            Some(&f.owner),
            Some(payload.clone()),
            StatusCode::OK
        )
        .await,
        chapter
    );
    let mut changed = payload.clone();
    changed["draft"]["title"] = json!("Different retry is rejected");
    api(
        &f.app,
        Method::POST,
        &format!("/api/upload/{session}/commit"),
        Some(&f.owner),
        Some(changed),
        StatusCode::CONFLICT,
    )
    .await;
    upload(
        &f,
        &session,
        &[image_part("late.png", "image/png", &png)],
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
    api(
        &f.app,
        Method::GET,
        "/api/upload",
        Some(&f.owner),
        None,
        StatusCode::NO_CONTENT,
    )
    .await;
    send(
        &f.app,
        Method::GET,
        &preview,
        Some(&f.owner),
        &[],
        vec![],
        StatusCode::NOT_FOUND,
    )
    .await;
    assert_eq!(
        api(
            &f.app,
            Method::GET,
            &format!("/api/chapter/{cid}"),
            None,
            None,
            StatusCode::OK
        )
        .await,
        chapter
    );
    let listing = api(
        &f.app,
        Method::GET,
        &format!("/api/manga/{mid}/chapters"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert!(
        listing["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == cid)
    );
    let originals: [&[u8]; 3] = [&jpg, &png, &png];
    let mut first_etag = String::new();
    let mut first_path = String::new();
    for (index, (page, bytes)) in chapter["data"]
        .as_array()
        .unwrap()
        .iter()
        .zip(originals)
        .enumerate()
    {
        let path = format!("/data/{cid}/{}", page["name"].as_str().unwrap());
        let mime = if index == 0 {
            "image/jpeg"
        } else {
            "image/png"
        };
        let etag = media_contract(&f, &path, None, bytes, mime, false).await;
        if index == 0 {
            first_etag = etag;
            first_path = path;
        }
    }
    let optimized = chapter["data_optimized"].as_array().expect("The noisy lossless/high-quality fixtures must yield a complete genuinely smaller derivative set");
    assert_eq!(optimized.len(), originals.len());
    for (page, original) in optimized.iter().zip(originals) {
        let path = format!("/data/{cid}/{}", page["name"].as_str().unwrap());
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
        assert!(!response.body.is_empty());
        assert!(
            response.body.len() < original.len(),
            "Advertised optimization must save real bytes"
        );
        assert_ne!(response.body, original);
        let decoded = image::load_from_memory(&response.body).unwrap();
        assert_eq!(
            json!([decoded.width(), decoded.height()]),
            page["dimensions"]
        );
        media_contract(&f, &path, None, &response.body, "image/jpeg", false).await;
    }
    api(
        &f.app,
        Method::PUT,
        &format!("/api/chapter/{cid}"),
        Some(&f.foreign),
        Some(json!({"version":1,"title":"Foreign overwrite"})),
        StatusCode::FORBIDDEN,
    )
    .await;
    let updated = api(
        &f.app,
        Method::PUT,
        &format!("/api/chapter/{cid}"),
        Some(&f.owner),
        Some(json!({"version":1,"title":"Up persisted chapter update"})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(updated["version"], 2);
    api(
        &f.app,
        Method::PUT,
        &format!("/api/chapter/{cid}"),
        Some(&f.owner),
        Some(json!({"version":1,"title":"Stale overwrite"})),
        StatusCode::CONFLICT,
    )
    .await;
    api(
        &f.app,
        Method::POST,
        &format!("/api/upload/begin/{cid}"),
        Some(&f.owner),
        Some(json!({"version":1,"replace":false})),
        StatusCode::CONFLICT,
    )
    .await;
    let edit = api(
        &f.app,
        Method::POST,
        &format!("/api/upload/begin/{cid}"),
        Some(&f.owner),
        Some(json!({"version":2,"replace":false})),
        StatusCode::CREATED,
    )
    .await;
    let edit_id = edit["id"].as_str().unwrap();
    let copied = edit["relationships"]["files"].as_array().unwrap();
    assert_eq!(copied.len(), 3);
    api(
        &f.app,
        Method::PUT,
        &format!("/api/chapter/{cid}"),
        Some(&f.owner),
        Some(json!({"version":2,"title":"Concurrent newer update"})),
        StatusCode::OK,
    )
    .await;
    api(
        &f.app,
        Method::POST,
        &format!("/api/upload/{edit_id}/commit"),
        Some(&f.owner),
        Some(commit_payload("Stale page replacement", copied)),
        StatusCode::CONFLICT,
    )
    .await;
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/upload/{edit_id}"),
        Some(&f.owner),
        None,
        StatusCode::NO_CONTENT,
    )
    .await;
    send(
        &f.app,
        Method::GET,
        &first_path,
        None,
        &[],
        vec![],
        StatusCode::OK,
    )
    .await;
    let edit = api(
        &f.app,
        Method::POST,
        &format!("/api/upload/begin/{cid}"),
        Some(&f.owner),
        Some(json!({"version":3,"replace":false})),
        StatusCode::CREATED,
    )
    .await;
    let edit_id = edit["id"].as_str().unwrap();
    let replaced = api(
        &f.app,
        Method::POST,
        &format!("/api/upload/{edit_id}/commit"),
        Some(&f.owner),
        Some(commit_payload(
            "Up authorized page replacement",
            edit["relationships"]["files"].as_array().unwrap(),
        )),
        StatusCode::OK,
    )
    .await;
    assert_eq!(replaced["version"], 4);
    assert_eq!(replaced["id"], cid);
    let replaced_path = format!(
        "/data/{cid}/{}",
        replaced["data"][0]["name"].as_str().unwrap()
    );
    media_contract(&f, &replaced_path, None, &jpg, "image/jpeg", false).await;
    send(
        &f.app,
        Method::GET,
        &first_path,
        None,
        &[("if-none-match", &first_etag)],
        vec![],
        StatusCode::NOT_FOUND,
    )
    .await;
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/chapter/{cid}"),
        Some(&f.foreign),
        None,
        StatusCode::FORBIDDEN,
    )
    .await;
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/chapter/{cid}"),
        Some(&f.owner),
        None,
        StatusCode::NO_CONTENT,
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
    send(
        &f.app,
        Method::GET,
        &replaced_path,
        None,
        &[("if-none-match", &first_etag)],
        vec![],
        StatusCode::NOT_FOUND,
    )
    .await;
    let listing = api(
        &f.app,
        Method::GET,
        &format!("/api/manga/{mid}/chapters"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert!(
        !listing["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == cid)
    );

    // A GIF stays animated and a mixed set must not advertise incomplete optimization.
    let session = begin(&f, mid).await;
    let gif = gif();
    let files = file_array(
        upload(
            &f,
            &session,
            &[
                image_part("animation.gif", "image/gif", &gif),
                image_part("static.png", "image/png", &png),
            ],
            Some(&f.owner),
            StatusCode::OK,
        )
        .await,
        2,
    );
    let chapter = api(
        &f.app,
        Method::POST,
        &format!("/api/upload/{session}/commit"),
        Some(&f.owner),
        Some(commit_payload("Up real GIF original", &files)),
        StatusCode::OK,
    )
    .await;
    assert!(
        chapter.get("data_optimized").is_none(),
        "Partial/empty optimized arrays break the retained reader fallback"
    );
    let cid = chapter["id"].as_str().unwrap();
    let path = format!(
        "/data/{cid}/{}",
        chapter["data"][0]["name"].as_str().unwrap()
    );
    let etag = media_contract(&f, &path, None, &gif, "image/gif", false).await;
    let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(gif)).unwrap();
    assert_eq!(decoder.into_frames().collect_frames().unwrap().len(), 2);
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/chapter/{cid}"),
        Some(&f.owner),
        None,
        StatusCode::NO_CONTENT,
    )
    .await;
    send(
        &f.app,
        Method::HEAD,
        &path,
        None,
        &[("if-none-match", &etag)],
        vec![],
        StatusCode::NOT_FOUND,
    )
    .await;
}

#[tokio::test]
#[ignore = "Requires the explicitly isolated PostgreSQL runner after implementation freeze"]
async fn persisted_upload_validation_file_removal_cancel_and_body_bounds() {
    let f = fixture().await;
    let manga = create_manga(&f, "Up invalid intake fixture", true).await;
    let mid = manga["id"].as_str().unwrap();
    let session = begin(&f, mid).await;
    let png = png();
    let malformed = b"\x89PNG\r\n\x1a\nnot-a-complete-image";
    let oversized = vec![b'x'; MAX_FILE_BYTES + 1];
    let invalids = upload(
        &f,
        &session,
        &[
            image_part("malformed.png", "image/png", malformed),
            image_part("wrong-type.png", "image/jpeg", &png),
            image_part("wrong-extension.jpg", "image/png", &png),
            image_part("too-large.png", "image/png", &oversized),
        ],
        Some(&f.owner),
        StatusCode::OK,
    )
    .await;
    let invalids = invalids.as_array().unwrap();
    assert_eq!(invalids.len(), 4);
    for invalid in invalids {
        assert!(invalid["error"].as_str().is_some_and(|e| !e.is_empty()));
        assert!(
            invalid.get("id").is_none(),
            "Rejected content must not become an upload file"
        );
    }
    assert!(invalids[3]["error"].as_str().unwrap().contains("32 MiB"));
    upload(
        &f,
        &session,
        &[Part {
            name: "metadata",
            filename: None,
            mime: None,
            bytes: b"not-file",
        }],
        Some(&f.owner),
        StatusCode::BAD_REQUEST,
    )
    .await;
    upload(&f, &session, &[], Some(&f.owner), StatusCode::BAD_REQUEST).await;
    send(
        &f.app,
        Method::POST,
        &format!("/api/upload/{session}"),
        Some(&f.owner),
        &[("content-type", "multipart/form-data; boundary=missing")],
        b"not-multipart".to_vec(),
        StatusCode::BAD_REQUEST,
    )
    .await;
    api(
        &f.app,
        Method::POST,
        &format!("/api/upload/{session}/commit"),
        Some(&f.owner),
        Some(json!({"draft":draft("No accepted files"),"page_order":[]})),
        StatusCode::BAD_REQUEST,
    )
    .await;
    let files = file_array(
        upload(
            &f,
            &session,
            &[
                image_part("accepted-after-errors.png", "image/png", &png),
                image_part("remove-batch.png", "image/png", &png),
            ],
            Some(&f.owner),
            StatusCode::OK,
        )
        .await,
        2,
    );
    let first_preview = format!("/uploads/{}", files[0]["file_name"].as_str().unwrap());
    let second_preview = format!("/uploads/{}", files[1]["file_name"].as_str().unwrap());
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/upload/{session}/batch"),
        Some(&f.foreign),
        Some(json!([files[1]["id"]])),
        StatusCode::NOT_FOUND,
    )
    .await;
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/upload/{session}/batch"),
        Some(&f.owner),
        Some(json!([files[1]["id"], "Missing001"])),
        StatusCode::NOT_FOUND,
    )
    .await;
    send(
        &f.app,
        Method::GET,
        &second_preview,
        Some(&f.owner),
        &[],
        vec![],
        StatusCode::OK,
    )
    .await;
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/upload/{session}/batch"),
        Some(&f.owner),
        Some(json!([files[1]["id"]])),
        StatusCode::NO_CONTENT,
    )
    .await;
    send(
        &f.app,
        Method::GET,
        &second_preview,
        Some(&f.owner),
        &[],
        vec![],
        StatusCode::NOT_FOUND,
    )
    .await;
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/upload/{session}/{}", files[0]["id"].as_str().unwrap()),
        Some(&f.owner),
        None,
        StatusCode::NO_CONTENT,
    )
    .await;
    send(
        &f.app,
        Method::GET,
        &first_preview,
        Some(&f.owner),
        &[],
        vec![],
        StatusCode::NOT_FOUND,
    )
    .await;
    api(
        &f.app,
        Method::POST,
        &format!("/api/upload/{session}/commit"),
        Some(&f.owner),
        Some(commit_payload("Deleted pages cannot commit", &files)),
        StatusCode::BAD_REQUEST,
    )
    .await;
    let files = file_array(
        upload(
            &f,
            &session,
            &[image_part("cancel-this.png", "image/png", &png)],
            Some(&f.owner),
            StatusCode::OK,
        )
        .await,
        1,
    );
    let preview = format!("/uploads/{}", files[0]["file_name"].as_str().unwrap());
    let etag = media_contract(&f, &preview, Some(&f.owner), &png, "image/png", true).await;
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/upload/{session}"),
        Some(&f.owner),
        None,
        StatusCode::NO_CONTENT,
    )
    .await;
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/upload/{session}"),
        Some(&f.owner),
        None,
        StatusCode::NO_CONTENT,
    )
    .await;
    api(
        &f.app,
        Method::GET,
        "/api/upload",
        Some(&f.owner),
        None,
        StatusCode::NO_CONTENT,
    )
    .await;
    send(
        &f.app,
        Method::GET,
        &preview,
        Some(&f.owner),
        &[("if-none-match", &etag)],
        vec![],
        StatusCode::NOT_FOUND,
    )
    .await;
    api(
        &f.app,
        Method::POST,
        &format!("/api/upload/{session}/commit"),
        Some(&f.owner),
        Some(commit_payload("Cancelled session cannot commit", &files)),
        StatusCode::CONFLICT,
    )
    .await;
    upload(
        &f,
        &session,
        &[image_part("closed.png", "image/png", &png)],
        Some(&f.owner),
        StatusCode::CONFLICT,
    )
    .await;
    let next = begin(&f, mid).await;
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/upload/{next}"),
        Some(&f.owner),
        None,
        StatusCode::NO_CONTENT,
    )
    .await;

    // Whole-request budgets are checked at the HTTP boundary before multipart intake.
    send(
        &f.app,
        Method::POST,
        &format!("/api/upload/{next}"),
        Some(&f.owner),
        &[
            ("content-type", "multipart/form-data; boundary=unused"),
            ("content-length", "100663297"),
        ],
        vec![],
        StatusCode::PAYLOAD_TOO_LARGE,
    )
    .await;
    send(
        &f.app,
        Method::POST,
        &format!("/api/cover/{mid}"),
        Some(&f.owner),
        &[
            ("content-type", "multipart/form-data; boundary=unused"),
            ("content-length", "41943041"),
        ],
        vec![],
        StatusCode::PAYLOAD_TOO_LARGE,
    )
    .await;
    // Rejected originals and removed previews are not recoverable by guessing paths.
    assert!(
        f.state
            .config
            .uploads_dir
            .join("temporary")
            .read_dir()
            .unwrap()
            .next()
            .is_none()
    );
}

#[tokio::test]
#[ignore = "Requires the explicitly isolated PostgreSQL runner after implementation freeze"]
async fn persisted_cover_upload_draft_privacy_list_update_variants_and_delete() {
    let f = fixture().await;
    let manga = create_manga(&f, "Up private cover fixture", false).await;
    let mid = manga["id"].as_str().unwrap();
    let png = png();
    let gif = gif();
    let path = format!("/api/cover/{mid}");
    let parts = [
        image_part("cover.png", "image/png", &png),
        Part {
            name: "language",
            filename: None,
            mime: None,
            bytes: b"en",
        },
        Part {
            name: "volume",
            filename: None,
            mime: None,
            bytes: b"1",
        },
        Part {
            name: "description",
            filename: None,
            mime: None,
            bytes: b"Synthetic cover",
        },
        Part {
            name: "main",
            filename: None,
            mime: None,
            bytes: b"true",
        },
    ];
    let (content_type, body) = multipart(&parts);
    send(
        &f.app,
        Method::POST,
        &path,
        None,
        &[("content-type", &content_type)],
        body.clone(),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    send(
        &f.app,
        Method::POST,
        &path,
        Some(&f.foreign),
        &[("content-type", &content_type)],
        body.clone(),
        StatusCode::FORBIDDEN,
    )
    .await;
    let cover = send(
        &f.app,
        Method::POST,
        &path,
        Some(&f.owner),
        &[("content-type", &content_type)],
        body,
        StatusCode::OK,
    )
    .await
    .json();
    assert_eq!(cover["ext"], ".png");
    assert_eq!(cover["dimensions"], json!([128, 160]));
    assert_eq!(cover["description"], "Synthetic cover");
    let id = cover["id"].as_str().unwrap();
    // Match the retained Manga/editor/lightbox URL expression exactly, rather
    // than hiding a DTO mismatch by hardcoding an extra separator in the test.
    let original = format!("/covers/{mid}/{id}{}", cover["ext"].as_str().unwrap());
    assert_eq!(
        f.state
            .db
            .client()
            .await
            .unwrap()
            .query_one(
                "SELECT ext FROM platform.catalog_covers WHERE id=$1",
                &[&id]
            )
            .await
            .unwrap()
            .get::<_, String>(0),
        "png",
        "Public extension adaptation must not change storage metadata"
    );
    send(
        &f.app,
        Method::GET,
        &original,
        None,
        &[],
        vec![],
        StatusCode::NOT_FOUND,
    )
    .await;
    send(
        &f.app,
        Method::GET,
        &original,
        Some(&f.foreign),
        &[],
        vec![],
        StatusCode::NOT_FOUND,
    )
    .await;
    api(
        &f.app,
        Method::GET,
        &format!("/api/cover/{id}"),
        None,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    api(
        &f.app,
        Method::GET,
        &format!("/api/cover/{id}"),
        Some(&f.foreign),
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    assert_eq!(
        api(
            &f.app,
            Method::GET,
            &format!("/api/cover/{id}"),
            Some(&f.owner),
            None,
            StatusCode::OK
        )
        .await,
        cover
    );
    let etag = media_contract(&f, &original, Some(&f.owner), &png, "image/png", true).await;
    for variant in [256, 512] {
        let path = format!("/covers/{mid}/{id}.{variant}.webp");
        let reply = send(
            &f.app,
            Method::GET,
            &path,
            Some(&f.owner),
            &[],
            vec![],
            StatusCode::OK,
        )
        .await;
        assert_eq!(reply.header("content-type"), "image/webp");
        let decoded = image::load_from_memory_with_format(&reply.body, ImageFormat::WebP).unwrap();
        assert!(decoded.width() <= variant && decoded.height() <= variant * 4);
        assert!(decoded.width() > 0 && decoded.height() > 0);
        media_contract(&f, &path, Some(&f.owner), &reply.body, "image/webp", true).await;
        send(
            &f.app,
            Method::GET,
            &path,
            None,
            &[],
            vec![],
            StatusCode::NOT_FOUND,
        )
        .await;
    }
    let (content_type, body) = multipart(&[image_part("unsupported-cover.gif", "image/gif", &gif)]);
    send(
        &f.app,
        Method::POST,
        &path,
        Some(&f.owner),
        &[("content-type", &content_type)],
        body,
        StatusCode::BAD_REQUEST,
    )
    .await;
    let (content_type, body) = multipart(&[image_part("broken.png", "image/png", b"bad bytes")]);
    send(
        &f.app,
        Method::POST,
        &path,
        Some(&f.owner),
        &[("content-type", &content_type)],
        body,
        StatusCode::BAD_REQUEST,
    )
    .await;
    api(
        &f.app,
        Method::PUT,
        &format!("/api/cover/{id}"),
        Some(&f.foreign),
        Some(json!({"version":1,"description":"Foreign update"})),
        StatusCode::FORBIDDEN,
    )
    .await;
    let cover = api(
        &f.app,
        Method::PUT,
        &format!("/api/cover/{id}"),
        Some(&f.owner),
        Some(json!({"version":1,"volume":"2","description":"Up saved cover update","main":true})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(cover["version"], 2);
    assert_eq!(cover["volume"], "2");
    api(
        &f.app,
        Method::PUT,
        &format!("/api/cover/{id}"),
        Some(&f.owner),
        Some(json!({"version":1,"description":"Stale cover update"})),
        StatusCode::CONFLICT,
    )
    .await;
    let manga = api(
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
        Some(json!({"version":manga["version"]})),
        StatusCode::OK,
    )
    .await;
    let listed = api(
        &f.app,
        Method::GET,
        &format!("/api/manga/{mid}/covers"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(listed.as_array().unwrap(), &vec![cover.clone()]);
    let published = api(
        &f.app,
        Method::GET,
        &format!("/api/manga/{mid}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(published["relationships"]["cover"], cover);
    media_contract(&f, &original, None, &png, "image/png", false).await;
    send(
        &f.app,
        Method::GET,
        &format!("/covers/{mid}/{id}png"),
        None,
        &[],
        vec![],
        StatusCode::BAD_REQUEST,
    )
    .await;
    send(
        &f.app,
        Method::GET,
        &format!("/covers/{mid}/{id}.jpg"),
        None,
        &[],
        vec![],
        StatusCode::NOT_FOUND,
    )
    .await;
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/cover/{id}"),
        Some(&f.foreign),
        None,
        StatusCode::FORBIDDEN,
    )
    .await;
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/cover/{id}"),
        Some(&f.owner),
        None,
        StatusCode::NO_CONTENT,
    )
    .await;
    api(
        &f.app,
        Method::GET,
        &format!("/api/cover/{id}"),
        None,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    assert!(
        api(
            &f.app,
            Method::GET,
            &format!("/api/manga/{mid}/covers"),
            None,
            None,
            StatusCode::OK
        )
        .await
        .as_array()
        .unwrap()
        .is_empty()
    );
    send(
        &f.app,
        Method::GET,
        &original,
        None,
        &[("if-none-match", &etag)],
        vec![],
        StatusCode::NOT_FOUND,
    )
    .await;
    for variant in [256, 512] {
        send(
            &f.app,
            Method::HEAD,
            &format!("/covers/{mid}/{id}.{variant}.webp"),
            None,
            &[],
            vec![],
            StatusCode::NOT_FOUND,
        )
        .await;
    }
}

#[tokio::test]
#[ignore = "Requires the explicitly isolated PostgreSQL runner after implementation freeze"]
async fn signed_upload_pending_projection_is_truthful_owner_scoped_and_retry_stable() {
    signed_upload_projection_journey(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires isolated PostgreSQL and the real strfry projection binary"]
async fn committed_signed_upload_becomes_readable_after_original_projection() {
    signed_upload_projection_journey(true).await;
}

/// Exercise both an intentionally paused source and its real eventual delivery.
/// No test writes source visibility, catalogue metadata, or outbox completion.
async fn signed_upload_projection_journey(materialize: bool) {
    use manga_node::{
        auth,
        db::{DbConfig, Repository},
    };
    use std::{sync::Arc, time::Duration};
    use weebdex_platform::protocol::publication::OperatorRuntimeConfig;

    let mut f = fixture().await;
    let manga = create_manga(&f, &format!("Up signed pending fixture {}", f.owner_id), true).await;
    let mid = manga["id"].as_str().unwrap().to_owned();
    let original_repo = Repository::connect(DbConfig {
        dsn: std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").unwrap(),
        connections: 4,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1024 * 1024,
        plaintext_local: true,
    })
    .await
    .unwrap();
    original_repo.migrate().await.unwrap();
    // Fresh synthetic test material is never printed or retained in evidence.
    // It exists only in this owned temporary fixture and is deleted with it.
    let (synthetic_secret, publisher) = loop {
        let material: [u8; 32] = rand::random();
        let secret = hex::encode(material);
        if let Ok(event) = auth::sign(&secret, 1, vec![], String::new()) {
            break (zeroize::Zeroizing::new(secret), event.pubkey);
        }
    };
    let publisher_bytes: [u8; 32] = hex::decode(&publisher).unwrap().try_into().unwrap();
    original_repo
        .set_publisher_admission(publisher_bytes, true)
        .await
        .unwrap();
    let signer = f._storage.path().join("SYNTHETIC_EPHEMERAL_signer.key");
    std::fs::write(&signer, synthetic_secret.as_bytes()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&signer, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let runtime = OperatorRuntimeConfig {
        operator_key_file: signer.clone(),
        expected_publisher: publisher.clone(),
        storage_owner: publisher.clone(),
        storage_holder: publisher.clone(),
        storage: json!({
            "path":f._storage.path().join("protocol-originals"),
            "volume":format!("UpPendingFixture{}", f.owner_id), "physical_bytes":32*1024*1024,
            "staging_bytes":32*1024*1024, "max_blob_bytes":8*1024*1024,
            "upload_slots":2, "reclamation_slots":2, "credit_lanes":16
        }),
        group_keys_dir: None,
        backend: None,
        native_node: if materialize {
            let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let bind = reservation.local_addr().unwrap();
            drop(reservation);
            let binary = std::env::var_os("STRFRY_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../manga-protocol-lab/vendor/strfry/strfry"));
            assert!(binary.is_absolute() && binary.is_file(), "Real original projection binary is required");
            Some(json!({
                "bind":bind.to_string(), "base_url":format!("http://{bind}"),
                "service_key_file":signer, "admin_pubkey":publisher,
                "database_plaintext_local":true, "read_policy":"public",
                "cors_origins":[ORIGIN], "sync_worker_enabled":false,
                "auto_approve_publication_deletions":false,
                "backend":{"binary":binary,"data_dir":f._storage.path().join("real-projection"),
                    "map_bytes":64*1024*1024,"timeout_seconds":5}
            }))
        } else { None },
    };
    let mut config = (*f.state.config).clone();
    config.protocol_publication = Some(runtime.clone());
    config.validate().unwrap();
    f.state.config = Arc::new(config);
    f.state.protocol.verify_schema().await.unwrap();
    f.state
        .protocol
        .install_visibility_function()
        .await
        .unwrap();
    f.state
        .protocol
        .configure_publication(runtime)
        .await
        .unwrap();
    f.app = weebdex_platform::router::router(f.state.clone());
    let client = f.state.db.client().await.unwrap();
    client
        .execute(
            "INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",
            &[&publisher, &f.owner_id],
        )
        .await
        .unwrap();
    drop(client);

    // Real GIF upload remains valid intake, but source publication has no GIF capability.
    // Refusal leaves the session recoverable and does not publish a fictitious signed chapter.
    let session = begin(&f, &mid).await;
    let animation = gif();
    let files = file_array(
        upload(
            &f,
            &session,
            &[image_part(
                "unsupported-source.gif",
                "image/gif",
                &animation,
            )],
            Some(&f.owner),
            StatusCode::OK,
        )
        .await,
        1,
    );
    api(
        &f.app,
        Method::POST,
        &format!("/api/upload/{session}/commit"),
        Some(&f.owner),
        Some(commit_payload("Unsupported signed GIF", &files)),
        StatusCode::BAD_REQUEST,
    )
    .await;
    assert_eq!(
        api(
            &f.app,
            Method::GET,
            "/api/upload",
            Some(&f.owner),
            None,
            StatusCode::OK
        )
        .await["id"],
        session
    );
    api(
        &f.app,
        Method::DELETE,
        &format!("/api/upload/{session}"),
        Some(&f.owner),
        None,
        StatusCode::NO_CONTENT,
    )
    .await;

    let session = begin(&f, &mid).await;
    // Source storage deliberately owns a hash globally in this database.
    // Distinct fixture authorities need distinct originals when both journeys
    // run together; repeated positions still use the same exact PNG below.
    let unique_original = |bytes: Vec<u8>, format: ImageFormat| {
        let mut image = image::load_from_memory(&bytes).unwrap().to_rgb8();
        image.put_pixel(0, 0, Rgb(rand::random::<[u8; 3]>()));
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image).write_to(&mut encoded, format).unwrap();
        encoded.into_inner()
    };
    let png = unique_original(png(), ImageFormat::Png);
    let jpg = unique_original(jpeg(), ImageFormat::Jpeg);
    let files = file_array(
        upload(
            &f,
            &session,
            &[
                image_part("signed-first.png", "image/png", &png),
                image_part("signed-second.jpeg", "image/jpeg", &jpg),
                image_part("signed-repeat.png", "image/png", &png),
            ],
            Some(&f.owner),
            StatusCode::OK,
        )
        .await,
        3,
    );
    let ordered = vec![files[1].clone(), files[2].clone(), files[0].clone()];
    let payload = commit_payload("Up real signed pending upload", &ordered);
    let path = format!("/api/upload/{session}/commit");
    let receipt = api(
        &f.app,
        Method::POST,
        &path,
        Some(&f.owner),
        Some(payload.clone()),
        StatusCode::OK,
    )
    .await;
    assert_eq!(receipt["publication_scope"], "original-protocol");
    assert_eq!(receipt["publication_status"], "accepted_pending_projection");
    assert_eq!(receipt["is_unavailable"], true);
    let cid = receipt["id"].as_str().unwrap();
    let event_id = receipt["protocol_event_id"].as_str().unwrap();
    assert!(auth::valid_hash(event_id));
    assert_eq!(receipt["data"].as_array().unwrap().len(), 3);
    for (page, file) in receipt["data"].as_array().unwrap().iter().zip(&ordered) {
        assert_eq!(
            page["name"], file["file_hash"],
            "The signed original name is its canonical content hash"
        );
    }
    assert_eq!(
        receipt["data"][1]["name"], receipt["data"][2]["name"],
        "Repeated original hashes retain separate page positions"
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
    api(
        &f.app,
        Method::GET,
        &format!("/api/manga/{mid}"),
        None,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    api(
        &f.app,
        Method::GET,
        &format!("/api/manga/{mid}/chapters"),
        None,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    let public_feed = api(
        &f.app,
        Method::GET,
        "/api/chapter?limit=100",
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert!(
        !public_feed["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == cid)
    );
    for page in receipt["data"].as_array().unwrap() {
        send(
            &f.app,
            Method::GET,
            &format!("/data/{cid}/{}", page["name"].as_str().unwrap()),
            None,
            &[],
            vec![],
            StatusCode::NOT_FOUND,
        )
        .await;
    }
    if let Some(optimized) = receipt["data_optimized"].as_array() {
        for page in optimized {
            send(
                &f.app,
                Method::HEAD,
                &format!("/data/{cid}/{}", page["name"].as_str().unwrap()),
                None,
                &[],
                vec![],
                StatusCode::NOT_FOUND,
            )
            .await;
        }
    }
    api(
        &f.app,
        Method::POST,
        &path,
        None,
        Some(payload.clone()),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    api(
        &f.app,
        Method::POST,
        &path,
        Some(&f.foreign),
        Some(payload.clone()),
        StatusCode::NOT_FOUND,
    )
    .await;
    assert_eq!(
        api(
            &f.app,
            Method::POST,
            &path,
            Some(&f.owner),
            Some(payload.clone()),
            StatusCode::OK
        )
        .await,
        receipt
    );
    let mut changed = payload.clone();
    changed["draft"]["title"] = json!("Different signed retry");
    api(
        &f.app,
        Method::POST,
        &path,
        Some(&f.owner),
        Some(changed),
        StatusCode::CONFLICT,
    )
    .await;
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
    assert_eq!(resumed["state"], "committed");
    assert_eq!(resumed["can_mutate"], false);
    assert_eq!(resumed["can_cancel"], false);
    assert_eq!(resumed["committed_result"], receipt);
    assert_eq!(resumed["receipt"], receipt);
    assert_eq!(resumed["draft"], payload["draft"]);
    assert_eq!(resumed["target_status"], "accepted_pending_projection");
    api(
        &f.app,
        Method::GET,
        "/api/upload",
        Some(&f.foreign),
        None,
        StatusCode::NO_CONTENT,
    )
    .await;

    // Read-only persisted evidence proves this HTTP receipt refers to a real signed
    // original publication and genuinely uncompleted projection, rather than a stub.
    // No authority, backend state or outbox completion is forged by this test.
    let client = original_repo.pool.get().await.unwrap();
    let event_bytes = hex::decode(event_id).unwrap();
    let row = client.query_one("SELECT raw_event,state,(SELECT count(*) FROM backend_outbox o WHERE o.event_id=e.id AND o.completed_at IS NULL) FROM events e WHERE e.id=$1", &[&event_bytes]).await.unwrap();
    let event: auth::Event = serde_json::from_value(row.get::<_, Value>(0)).unwrap();
    event.verify().unwrap();
    assert_eq!(event.id, event_id);
    assert_eq!(event.pubkey, publisher);
    assert_eq!(event.kind, 1001);
    assert_eq!(row.get::<_, String>(1), "hosted");
    assert!(
        row.get::<_, i64>(2) > 0,
        "Pending means source projection has actually not completed"
    );
    let count: i64 = client
        .query_one(
            "SELECT count(*) FROM events WHERE kind=1001 AND publisher=$1",
            &[&publisher_bytes.as_slice()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        count, 1,
        "Identical HTTP commit replay must not create another signed chapter"
    );
    drop(client);
    if !materialize {
        return;
    }

    struct ProjectionGuard {
        listener: Option<weebdex_platform::protocol::publication::NativeListener>,
        importer: tokio::task::JoinHandle<()>,
    }
    impl Drop for ProjectionGuard {
        fn drop(&mut self) {
            self.importer.abort();
            if let Some(listener) = &mut self.listener {
                if let Some(stop) = listener.shutdown.take() { let _ = stop.send(()); }
                listener.task.abort();
            }
        }
    }
    let runtime = f.state.protocol.publication_runtime().await.unwrap();
    let mut projection = ProjectionGuard {
        listener: runtime.start_native_listener().await.unwrap(),
        importer: weebdex_platform::protocol::bridge::spawn_importer(f.state.clone()),
    };
    assert!(projection.listener.is_some());
    // The real original worker drains its outbox and the normal supervised
    // importer discovers the now-visible immutable source without commit replay.
    let public = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let response = f.app.clone().oneshot(Request::builder()
                .uri(format!("/api/chapter/{cid}"))
                .body(Body::empty()).unwrap()).await.unwrap();
            let status = response.status();
            let body = to_bytes(response.into_body(), 1024*1024).await.unwrap();
            if status == StatusCode::OK { break serde_json::from_slice::<Value>(&body).unwrap(); }
            assert_eq!(status, StatusCode::NOT_FOUND, "{}", String::from_utf8_lossy(&body));
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    }).await.expect("Real signed publication did not become readable");
    assert_eq!(public["id"], cid);
    assert_eq!(public["protocol_event_id"], event_id);
    assert_eq!(public["publication_status"], "locally_hosted");
    assert_eq!(public["is_unavailable"], false);
    assert_eq!(public["data"], receipt["data"]);
    assert_eq!(public["data"][1]["name"], public["data"][2]["name"]);
    for (index, bytes) in [&jpg, &png, &png].into_iter().enumerate() {
        let name = public["data"][index]["name"].as_str().unwrap();
        let media = format!("/data/{cid}/{name}");
        let original = send(&f.app, Method::GET, &media, None, &[], vec![], StatusCode::OK).await;
        assert_eq!(original.body, *bytes);
        assert_eq!(original.header("etag"), format!("\"{name}\""));
        assert_eq!(original.header("cache-control"), "no-store");
        let range = send(&f.app, Method::GET, &media, None, &[("range","bytes=0-7")], vec![], StatusCode::PARTIAL_CONTENT).await;
        assert_eq!(range.body, bytes[..8]);
        let head = send(&f.app, Method::HEAD, &media, None, &[], vec![], StatusCode::OK).await;
        assert!(head.body.is_empty());
        assert_eq!(head.header("content-length"), bytes.len().to_string());
    }
    assert_eq!(public["data_optimized"].as_array().unwrap().len(), 3);
    for page in public["data_optimized"].as_array().unwrap() {
        let media = format!("/data-saver/{cid}/{}", page["name"].as_str().unwrap());
        let optimized = send(&f.app, Method::GET, &media, None, &[], vec![], StatusCode::OK).await;
        assert_eq!(optimized.header("content-type"), "image/jpeg");
        assert_eq!(image::guess_format(&optimized.body).unwrap(), ImageFormat::Jpeg);
        let decoded = image::load_from_memory(&optimized.body).unwrap();
        assert_eq!(json!([decoded.width(),decoded.height()]), page["dimensions"]);
    }
    let recovered = api(&f.app, Method::POST, &path, Some(&f.owner), Some(payload.clone()), StatusCode::OK).await;
    assert_eq!(recovered, public, "Owner commit replay must return current readable state");
    api(&f.app, Method::GET, "/api/upload", Some(&f.owner), None, StatusCode::NO_CONTENT).await;
    let chapters = api(&f.app, Method::GET, &format!("/api/manga/{mid}/chapters"), None, None, StatusCode::OK).await;
    assert!(chapters["data"].as_array().unwrap().iter().any(|chapter| chapter["id"] == cid));
    let client = original_repo.pool.get().await.unwrap();
    assert_eq!(client.query_one("SELECT count(*) FROM backend_outbox WHERE event_id=$1 AND completed_at IS NULL", &[&event_bytes]).await.unwrap().get::<_,i64>(0), 0);
    assert_eq!(client.query_one("SELECT count(*) FROM events WHERE kind=1001 AND publisher=$1", &[&publisher_bytes.as_slice()]).await.unwrap().get::<_,i64>(0), 1);
    let session_version: i32 = client.query_one("SELECT version FROM platform.upload_sessions WHERE id=$1 AND committed_result->>'publication_status'='locally_hosted'", &[&session]).await.unwrap().get(0);
    drop(client);
    projection.importer.abort();
    let _ = (&mut projection.importer).await;
    for _ in 0..2 {
        assert!(weebdex_platform::protocol::bridge::import_page(&f.state).await.unwrap().completed_sweep);
    }
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT version FROM platform.upload_sessions WHERE id=$1", &[&session]).await.unwrap().get::<_,i32>(0), session_version, "Repeated source sweeps must not rewrite a completed receipt");
    if let Some(mut listener) = projection.listener.take() {
        let _ = listener.shutdown.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(20), listener.task).await.unwrap().unwrap().unwrap();
    }
}

#[tokio::test]
#[ignore = "Requires the explicitly isolated PostgreSQL runner after implementation freeze"]
async fn atomic_page_replacement_expiry_reclamation_and_media_admission() {
    let f = fixture().await;
    let manga = create_manga(&f, "Atomic replacement and abandonment fixture", true).await;
    let mid = manga["id"].as_str().unwrap();
    let session = begin(&f, mid).await;
    let png = png();
    let files = file_array(
        upload(
            &f,
            &session,
            &[image_part("original.png", "image/png", &png)],
            Some(&f.owner),
            StatusCode::OK,
        )
        .await,
        1,
    );
    let file = files[0]["id"].as_str().unwrap();
    // Seed the maximum already-admitted file count with genuine shared bytes.
    // Replacement must not need a temporary 501st upload-file row.
    let db = f.state.db.client().await.unwrap();
    let object: String = db
        .query_one(
            "SELECT object_id FROM platform.upload_files WHERE id=$1",
            &[&file],
        )
        .await
        .unwrap()
        .get(0);
    for position in 1..500i32 {
        let duplicate = domains::uploads::storage::short_id();
        db.execute("INSERT INTO platform.upload_files(id,session_id,object_id,original_name,position) VALUES($1,$2,$3,'admitted duplicate.png',$4)", &[&duplicate,&session,&object,&position]).await.unwrap();
    }
    drop(db);
    let initial = api(
        &f.app,
        Method::GET,
        "/api/upload",
        Some(&f.owner),
        None,
        StatusCode::OK,
    )
    .await;
    let version = initial["version"].as_i64().unwrap().to_string();
    let path = format!("/api/upload/{session}/files/{file}/replace");
    let (mime, body) = multipart(&[image_part("broken.png", "image/png", b"not a valid image")]);
    send(
        &f.app,
        Method::POST,
        &path,
        Some(&f.owner),
        &[
            ("content-type", &mime),
            ("x-weebdex-upload-version", &version),
        ],
        body,
        StatusCode::BAD_REQUEST,
    )
    .await;
    media_contract(
        &f,
        &format!("/uploads/{}", files[0]["file_name"].as_str().unwrap()),
        Some(&f.owner),
        &png,
        "image/png",
        true,
    )
    .await;
    let mut seed = 0x731ae53cu32;
    let large = RgbImage::from_fn(640, 640, |_, _| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        Rgb([(seed >> 16) as u8, (seed >> 8) as u8, seed as u8])
    });
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(large)
        .write_to(&mut encoded, ImageFormat::Png)
        .unwrap();
    let replacement = encoded.into_inner();
    assert!(
        replacement.len() > 1024 * 1024,
        "Valid replacement exercises upload rather than JSON body limits"
    );
    let (mime, body) = multipart(&[image_part("replacement.png", "image/png", &replacement)]);
    send(
        &f.app,
        Method::POST,
        &path,
        Some(&f.foreign),
        &[
            ("content-type", &mime),
            ("x-weebdex-upload-version", &version),
        ],
        body.clone(),
        StatusCode::NOT_FOUND,
    )
    .await;
    let replaced = send(
        &f.app,
        Method::POST,
        &path,
        Some(&f.owner),
        &[
            ("content-type", &mime),
            ("x-weebdex-upload-version", &version),
        ],
        body.clone(),
        StatusCode::OK,
    )
    .await
    .json();
    assert_eq!(replaced["id"], file);
    assert_eq!(
        replaced["version"].as_i64(),
        initial["version"].as_i64().map(|v| v + 1)
    );
    assert_eq!(
        replaced["file_hash"],
        hex::encode(Sha256::digest(&replacement))
    );
    let replay = send(
        &f.app,
        Method::POST,
        &path,
        Some(&f.owner),
        &[
            ("content-type", &mime),
            ("x-weebdex-upload-version", &version),
        ],
        body,
        StatusCode::OK,
    )
    .await
    .json();
    assert_eq!(
        replay, replaced,
        "An identical lost-response retry must not change version or file identity"
    );
    let (mime, body) = multipart(&[image_part("divergent.png", "image/png", &png)]);
    send(
        &f.app,
        Method::POST,
        &path,
        Some(&f.owner),
        &[
            ("content-type", &mime),
            ("x-weebdex-upload-version", &version),
        ],
        body,
        StatusCode::CONFLICT,
    )
    .await;
    let current = api(
        &f.app,
        Method::GET,
        "/api/upload",
        Some(&f.owner),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        current["relationships"]["files"].as_array().unwrap().len(),
        500
    );
    assert_eq!(current["page_order"][0], file);
    media_contract(
        &f,
        &format!("/uploads/{}", replaced["file_name"].as_str().unwrap()),
        Some(&f.owner),
        &replacement,
        "image/png",
        true,
    )
    .await;

    // Expiration is maintenance-owned, not dependent on a returning owner.
    let db = f.state.db.client().await.unwrap();
    let object: String = db.query_one("SELECT o.storage_key FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id WHERE f.id=$1", &[&file]).await.unwrap().get(0);
    db.execute("UPDATE platform.upload_sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE id=$1", &[&session]).await.unwrap();
    drop(db);
    let mut held = f.state.db.client().await.unwrap();
    let tx = held.transaction().await.unwrap();
    tx.query_one(
        "SELECT id FROM platform.upload_sessions WHERE id=$1 FOR UPDATE",
        &[&session],
    )
    .await
    .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        domains::uploads::recover_storage(&f.state),
    )
    .await
    .unwrap();
    assert_eq!(
        tx.query_one(
            "SELECT state FROM platform.upload_sessions WHERE id=$1",
            &[&session]
        )
        .await
        .unwrap()
        .get::<_, String>(0),
        "open",
        "Maintenance skips the active session lock"
    );
    tx.rollback().await.unwrap();
    drop(held);
    domains::uploads::recover_storage(&f.state).await;
    let db = f.state.db.client().await.unwrap();
    assert_eq!(
        db.query_one(
            "SELECT state FROM platform.upload_sessions WHERE id=$1",
            &[&session]
        )
        .await
        .unwrap()
        .get::<_, String>(0),
        "deleted"
    );
    assert_eq!(
        db.query_one(
            "SELECT count(*) FROM platform.upload_files WHERE session_id=$1",
            &[&session]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        0
    );
    drop(db);
    assert!(
        !domains::uploads::storage::object_path(&f.state.config.uploads_dir, &object)
            .unwrap()
            .exists()
    );

    // A durable command is an intentional synthetic maintenance fixture, not a
    // fabricated signed-publication success. The real bridge suite signs one.
    let protected = begin(&f, mid).await;
    let protected_files = file_array(
        upload(
            &f,
            &protected,
            &[image_part("held.png", "image/png", &png)],
            Some(&f.owner),
            StatusCode::OK,
        )
        .await,
        1,
    );
    let protected_file = protected_files[0]["id"].as_str().unwrap();
    let db = f.state.db.client().await.unwrap();
    let command = format!("maintenance:{protected}");
    db.execute("INSERT INTO platform.protocol_publication_commands(command_id,user_id,input_hash,resource_kind,catalog_id,upload_session_id,created_at,state) VALUES($1,$2,$3,'chapter',$4,$5,1,'prepared')", &[&command,&f.owner_id,&"11".repeat(32),&protected_file,&protected]).await.unwrap();
    db.execute("UPDATE platform.upload_sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE id=$1", &[&protected]).await.unwrap();
    drop(db);
    domains::uploads::recover_storage(&f.state).await;
    let db = f.state.db.client().await.unwrap();
    assert_eq!(
        db.query_one(
            "SELECT state FROM platform.upload_sessions WHERE id=$1",
            &[&protected]
        )
        .await
        .unwrap()
        .get::<_, String>(0),
        "open"
    );
    assert_eq!(
        db.query_one(
            "SELECT count(*) FROM platform.upload_files WHERE session_id=$1",
            &[&protected]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        1
    );
    drop(db);

    // Even a no-longer-prepared command may still pin an immutable original;
    // maintenance cannot reclaim that reference until the owner releases it.
    let db = f.state.db.client().await.unwrap();
    let pinned_object: String = db
        .query_one(
            "SELECT object_id FROM platform.upload_files WHERE id=$1",
            &[&protected_file],
        )
        .await
        .unwrap()
        .get(0);
    db.execute("UPDATE platform.protocol_publication_commands SET state='materialized' WHERE command_id=$1", &[&command]).await.unwrap();
    db.execute("INSERT INTO platform.protocol_publication_originals(command_id,hash,object_id) VALUES($1,$2,$3)", &[&command,&hex::encode(Sha256::digest(&png)),&pinned_object]).await.unwrap();
    drop(db);
    domains::uploads::recover_storage(&f.state).await;
    let db = f.state.db.client().await.unwrap();
    assert_eq!(
        db.query_one(
            "SELECT state FROM platform.upload_sessions WHERE id=$1",
            &[&protected]
        )
        .await
        .unwrap()
        .get::<_, String>(0),
        "open"
    );
    db.execute(
        "DELETE FROM platform.protocol_publication_originals WHERE command_id=$1",
        &[&command],
    )
    .await
    .unwrap();
    drop(db);
    domains::uploads::recover_storage(&f.state).await;
    assert_eq!(
        f.state
            .db
            .client()
            .await
            .unwrap()
            .query_one(
                "SELECT state FROM platform.upload_sessions WHERE id=$1",
                &[&protected]
            )
            .await
            .unwrap()
            .get::<_, String>(0),
        "deleted"
    );

    // With every request and pool slot held, media must reject before choosing
    // protocol/local bytes or adding a database waiter. No nested admission.
    let lease = f
        .state
        .requests
        .clone()
        .try_acquire_many_owned(f.state.config.maximum_requests as u32)
        .unwrap();
    let mut clients = Vec::new();
    for _ in 0..f.state.config.database_connections {
        clients.push(f.state.db.client().await.unwrap());
    }
    for path in [
        format!("/data/Saturated1/{}", "22".repeat(32)),
        format!("/covers/{mid}/Saturated1.png"),
    ] {
        tokio::time::timeout(
            std::time::Duration::from_millis(200),
            send(
                &f.app,
                Method::GET,
                &path,
                None,
                &[],
                vec![],
                StatusCode::TOO_MANY_REQUESTS,
            ),
        )
        .await
        .unwrap();
    }
    drop(clients);
    drop(lease);
}

#[tokio::test]
#[ignore = "Requires the explicitly isolated PostgreSQL runner after implementation freeze"]
async fn restart_reconciliation_respects_journal_leases_references_and_proxy_streams() {
    let f = fixture().await;
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(301);
    let root = &f.state.config.uploads_dir;
    let key = "bb".repeat(32);
    let object = domains::uploads::storage::object_path(root, &key).unwrap();
    std::fs::create_dir_all(object.parent().unwrap()).unwrap();
    std::fs::create_dir_all(root.join("pending")).unwrap();
    std::fs::write(
        &object,
        b"crash-window original without a database reference",
    )
    .unwrap();
    let marker = root.join("pending").join(&key);
    std::fs::write(&marker, &key).unwrap();
    let lease = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&marker)
        .unwrap();
    lease.set_modified(old).unwrap();
    lease.try_lock().unwrap();
    domains::uploads::recover_storage(&f.state).await;
    assert!(
        object.exists() && marker.exists(),
        "Live prewrite lease must prevent orphan reconciliation"
    );
    drop(lease);
    domains::uploads::recover_storage(&f.state).await;
    assert!(
        !object.exists() && !marker.exists(),
        "Released old intent permits bounded restart reclamation"
    );

    let manga = create_manga(&f, "Referenced journal fixture", true).await;
    let session = begin(&f, manga["id"].as_str().unwrap()).await;
    let png = png();
    let files = file_array(
        upload(
            &f,
            &session,
            &[image_part("referenced.png", "image/png", &png)],
            Some(&f.owner),
            StatusCode::OK,
        )
        .await,
        1,
    );
    let db = f.state.db.client().await.unwrap();
    let referenced: String = db
        .query_one(
            "SELECT object_id FROM platform.upload_files WHERE id=$1",
            &[&files[0]["id"].as_str().unwrap()],
        )
        .await
        .unwrap()
        .get(0);
    drop(db);
    let marker = root.join("pending").join(&referenced);
    std::fs::write(&marker, &referenced).unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&marker)
        .unwrap()
        .set_modified(old)
        .unwrap();
    domains::uploads::recover_storage(&f.state).await;
    assert!(
        domains::uploads::storage::object_path(root, &referenced)
            .unwrap()
            .exists()
    );
    assert!(
        !marker.exists(),
        "Persisted references retain bytes and clear stale uncertain-commit markers"
    );
    media_contract(
        &f,
        &format!("/uploads/{}", files[0]["file_name"].as_str().unwrap()),
        Some(&f.owner),
        &png,
        "image/png",
        true,
    )
    .await;

    // These synthetic files model restart residue, not a claimed hard-stop or
    // power-loss experiment. Active raw/proxy leases must survive maintenance.
    let ancient = std::time::SystemTime::now() - std::time::Duration::from_secs(24 * 60 * 60 + 1);
    for directory in ["temporary", "proxy-temporary"] {
        let directory = root.join(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("cc".repeat(32));
        std::fs::write(&path, b"unshared intake/proxy bytes").unwrap();
        let live = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        live.set_modified(ancient).unwrap();
        live.try_lock().unwrap();
        domains::uploads::recover_storage(&f.state).await;
        assert!(path.exists());
        drop(live);
        domains::uploads::recover_storage(&f.state).await;
        assert!(!path.exists());
    }
}

/// Real user-requested data stays local: immutable cached Japanese originals,
/// both supplied license notices, explicit attribution, no external publication.
#[tokio::test]
#[ignore = "Owned PostgreSQL and the locally verified noncommercial Ubunchu cache required"]
async fn licensed_ubunchu_originals_upload_cover_reader_hashes_and_attribution() {
    use std::path::Component;
    const REVISION: &str = "9f065a3fde464be893dfba9fb5fc6abc93722cf9";
    const ATTRIBUTION: &str = "Ubunchu! Japanese original, chapters1–2. Author: Hiroshi Seo (Seotch). Publisher: ASCII MEDIA WORKS Inc. Source: https://gitlab.com/ubunchu-translators/ubunchu at9f065a3fde464be893dfba9fb5fc6abc93722cf9 and https://seotch.wordpress.com/ubunchu/. Supplied notices: CC BY-NC3.0 (English), CC BY-NC2.1JP (Japanese). Unchanged downloaded originals; any optimized local test variants are derivatives. Noncommercial local disposable fixture only; no author/publisher endorsement.";
    let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let manifest_bytes = std::fs::read(project.join("tests/fixtures/ubunchu-manifest.json"))
        .expect("Reviewed local Ubunchu manifest required; no network download in this test");
    assert_eq!(
        hex::encode(Sha256::digest(&manifest_bytes)),
        "279dff36348bfef4b60b2923c33b82de80c4ee812756eb53567be4ac64afeb92"
    );
    let manifest: Value = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(manifest["revision"], REVISION);
    assert_eq!(manifest["source_modified"], false);
    let cache = project
        .join(".cache/fixtures/ubunchu")
        .join(REVISION)
        .canonicalize()
        .expect(
            "Local verified Ubunchu cache is missing; do not silently skip real-image acceptance",
        );
    let entries = manifest["files"].as_array().unwrap();
    assert_eq!(entries.len(), 28);
    let mut total = 0u64;
    let mut notices = 0;
    let mut originals = Vec::new();
    for entry in entries {
        let relative = std::path::Path::new(entry["path"].as_str().unwrap());
        assert!(
            relative
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
        );
        let path = cache.join(relative);
        assert!(path.canonicalize().unwrap().starts_with(&cache));
        let metadata = std::fs::symlink_metadata(&path).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        let is_png = relative.extension().is_some_and(|ext| ext == "png");
        assert!(
            metadata.len() > 0
                && metadata.len() <= if is_png { 4 * 1024 * 1024 } else { 128 * 1024 }
        );
        let bytes = std::fs::read(path).unwrap();
        assert_eq!(bytes.len() as u64, entry["bytes"].as_u64().unwrap());
        assert_eq!(
            hex::encode(Sha256::digest(&bytes)),
            entry["sha256"].as_str().unwrap()
        );
        total += bytes.len() as u64;
        assert!(total <= 32 * 1024 * 1024);
        if is_png {
            let decoded = image::load_from_memory_with_format(&bytes, ImageFormat::Png).unwrap();
            assert_eq!(decoded.width(), entry["width"].as_u64().unwrap() as u32);
            assert_eq!(decoded.height(), entry["height"].as_u64().unwrap() as u32);
            assert_eq!(entry["declared_license_urls"].as_array().unwrap().len(), 2);
            assert_eq!(entry["license_notice_paths"].as_array().unwrap().len(), 2);
            originals.push((entry.clone(), bytes));
        } else {
            assert_eq!(entry["role"], "license_notice");
            notices += 1;
        }
    }
    assert_eq!(notices, 4);
    assert_eq!(originals.len(), 24);
    assert_eq!(
        originals
            .iter()
            .map(|(_, bytes)| bytes.len())
            .sum::<usize>(),
        9_499_508
    );
    let f = fixture().await;
    let mut manga = api(&f.app, Method::POST, "/api/manga", Some(&f.owner),
        Some(json!({"title":"Ubunchu! — verified local noncommercial fixture", "description":ATTRIBUTION,
            "year":2009,"language":"ja","status":"completed","content_rating":"safe","authors":[],"artists":[]})), StatusCode::CREATED).await;
    let mid = manga["id"].as_str().unwrap().to_owned();
    let (cover_entry, cover_bytes) = &originals[0];
    let filename = std::path::Path::new(cover_entry["path"].as_str().unwrap())
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let parts = [
        image_part(filename, "image/png", cover_bytes),
        Part {
            name: "description",
            filename: None,
            mime: None,
            bytes: ATTRIBUTION.as_bytes(),
        },
        Part {
            name: "language",
            filename: None,
            mime: None,
            bytes: b"ja",
        },
        Part {
            name: "main",
            filename: None,
            mime: None,
            bytes: b"true",
        },
    ];
    let (content_type, body) = multipart(&parts);
    let cover = send(
        &f.app,
        Method::POST,
        &format!("/api/cover/{mid}"),
        Some(&f.owner),
        &[("content-type", &content_type)],
        body,
        StatusCode::OK,
    )
    .await
    .json();
    assert_eq!(cover["description"], ATTRIBUTION);
    assert_eq!(cover["ext"], ".png");
    manga = api(
        &f.app,
        Method::POST,
        &format!("/api/manga/draft/{mid}/commit"),
        Some(&f.owner),
        Some(json!({"version":manga["version"]})),
        StatusCode::OK,
    )
    .await;
    manga = api(
        &f.app,
        Method::POST,
        &format!("/api/manga/draft/{mid}/publish"),
        Some(&f.owner),
        Some(json!({"version":manga["version"]})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(manga["publication_scope"], "instance-local");
    assert_eq!(manga["description"], ATTRIBUTION);
    assert_eq!(manga["language"], "ja");
    let cover_path = format!(
        "/covers/{mid}/{}{}",
        cover["id"].as_str().unwrap(),
        cover["ext"].as_str().unwrap()
    );
    media_contract(&f, &cover_path, None, cover_bytes, "image/png", false).await;
    for number in [1u64, 2] {
        let selected: Vec<_> = originals
            .iter()
            .filter(|(entry, _)| entry["chapter"].as_u64() == Some(number))
            .collect();
        assert_eq!(selected.len(), 12);
        let session = begin(&f, &mid).await;
        let mut files = Vec::new();
        for (entry, bytes) in &selected {
            let filename = std::path::Path::new(entry["path"].as_str().unwrap())
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();
            let result = file_array(
                upload(
                    &f,
                    &session,
                    &[image_part(filename, "image/png", bytes)],
                    Some(&f.owner),
                    StatusCode::OK,
                )
                .await,
                1,
            );
            files.push(result[0].clone());
        }
        let payload = json!({"draft":{"volume":"1","chapter":number.to_string(),
            "title":format!("Ubunchu Japanese original chapter {number}; local attributed fixture"),"language":"ja"},
            "page_order":files.iter().map(|file|file["id"].clone()).collect::<Vec<_>>()});
        let chapter = api(
            &f.app,
            Method::POST,
            &format!("/api/upload/{session}/commit"),
            Some(&f.owner),
            Some(payload),
            StatusCode::OK,
        )
        .await;
        assert_eq!(chapter["publication_scope"], "instance-local");
        assert!(chapter.get("protocol_event_id").is_none());
        assert_eq!(chapter["language"], "ja");
        let cid = chapter["id"].as_str().unwrap();
        let reader = api(
            &f.app,
            Method::GET,
            &format!("/api/chapter/{cid}"),
            None,
            None,
            StatusCode::OK,
        )
        .await;
        let pages = reader["data"].as_array().unwrap();
        assert_eq!(pages.len(), 12);
        assert_eq!(reader["relationships"]["manga"]["description"], ATTRIBUTION);
        for (index, (entry, bytes)) in selected.iter().enumerate() {
            assert_eq!(
                pages[index]["dimensions"],
                json!([entry["width"], entry["height"]])
            );
            assert_eq!(pages[index]["name"], files[index]["file_name"]);
            let path = format!("/data/{cid}/{}", pages[index]["name"].as_str().unwrap());
            media_contract(&f, &path, None, bytes, "image/png", false).await;
        }
    }
    println!(
        "Real local Ubunchu acceptance:24 unchangedPNG originals,2chapters,4 verified notices,1 attributed cover; HTTP bytes/HEAD/ranges/ETags; no browser or external publication claim."
    );
}
