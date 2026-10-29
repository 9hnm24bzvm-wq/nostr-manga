use crate::{domains, error::ApiError, runtime::BoundedBody, state::AppState};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, OriginalUri, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
};
use manga_node::read_admission::Class;
use serde_json::json;
use std::time::Duration;
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

const FRONTEND_AUTH_ROUTES: &[&str] = &[
    "/auth/login",
    "/auth/register",
    "/auth/logout",
    "/auth/recovery",
    "/auth/verification",
    "/auth/error",
];

pub fn router(state: AppState) -> Router {
    let ui_html = std::fs::read_to_string(state.config.ui_dir.join(&state.config.ui_fallback))
        .unwrap_or_default();
    let mut protocol_connections = state.config.protocol_connect_origins.clone();
    if let Some(origin) = state
        .config
        .protocol_publication
        .as_ref()
        .and_then(|p| p.native_node.as_ref())
        .and_then(|n| n.get("base_url"))
        .and_then(serde_json::Value::as_str)
    {
        protocol_connections.push(origin.to_owned());
    }
    let native_media = if state.config.protocol_origin_self_hosted {
        state
            .config
            .protocol_origin
            .clone()
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let privacy_policy = crate::privacy::content_security_policy_with_origins(
        &ui_html,
        &protocol_connections,
        &native_media,
    );
    let api = domains::api_router()
        .nest("/v1", crate::public_api::router())
        .route("/health", get(health))
        .fallback(api_not_found)
        .layer(DefaultBodyLimit::max(state.config.maximum_json_bytes))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            admitted,
        ));
    let auth = domains::auth_router()
        .fallback(api_not_found)
        .layer(DefaultBodyLimit::max(state.config.maximum_json_bytes))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            admitted,
        ));
    let mut inner = Router::new()
        .nest("/api", api)
        .nest("/auth", auth)
        .merge(domains::media_router())
        .merge(domains::previews::router())
        .route(
            "/healthz",
            get(health).layer(axum::middleware::from_fn_with_state(
                state.clone(),
                admitted,
            )),
        )
        .route("/livez", get(live))
        .route("/readyz", get(ready))
        // Reserved integration surfaces never fall through to SPA HTML.
        .route(
            "/proxy",
            get(domains::uploads::proxy).head(domains::uploads::proxy),
        );
    // These are frontend pages, while /auth/self-service/** remains JSON API.
    for path in FRONTEND_AUTH_ROUTES {
        inner = inner.route(path, get(static_fallback));
    }
    inner
        .fallback(static_fallback)
        .layer(axum::middleware::from_fn_with_state(
            state.config.clone(),
            crate::client_peer::install,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            request_boundary,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            response_headers,
        ))
        .layer(axum::middleware::from_fn(
            move |request: Request, next: Next| {
                let policy = privacy_policy.clone();
                async move {
                    let mut response = next.run(request).await;
                    if !response
                        .headers()
                        .contains_key(header::CONTENT_SECURITY_POLICY)
                    {
                        response
                            .headers_mut()
                            .insert(header::CONTENT_SECURITY_POLICY, policy);
                    }
                    response
                }
            },
        ))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    state.health().await?;
    Ok(Json(
        json!({"database":"ready","protocol_schema_ready":state.protocol.is_ready(),"draining":state.runtime.is_draining()}),
    ))
}
async fn live() -> Json<serde_json::Value> {
    Json(json!({"status":"alive"}))
}
async fn ready(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    if !state.runtime.is_ready() || state.runtime.is_draining() {
        return Err(ApiError::unavailable());
    }
    let _request = state
        .requests
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::busy())?;
    state.health().await?;
    if state.config.production && !state.protocol.is_ready() {
        return Err(ApiError::unavailable());
    }
    if state.config.production && !state.config.protocol_read_only {
        state.protocol.publication_runtime().await?;
    }
    Ok(Json(
        json!({"status":"ready","protocol_schema_ready":state.protocol.is_ready(),"protocol_read_only":state.config.protocol_read_only,"protocol_publication_configured":state.config.protocol_publication.is_some()}),
    ))
}
async fn api_not_found() -> ApiError {
    ApiError::not_found("Unknown API endpoint")
}

fn signed_upload_schedule_private(parts:&[&str],method:&Method)->bool {
    match parts {
        ["api","v1","nostr","uploads","schedules"] => *method==Method::GET,
        ["api","v1","nostr","uploads",session,"schedule"] => *method==Method::GET&&crate::public_api::PublicUuid::parse(session).is_ok(),
        ["api","v1","nostr","uploads",session,"schedule","retry"] => *method==Method::POST&&crate::public_api::PublicUuid::parse(session).is_ok(),
        _=>false,
    }
}

fn segment_prefix(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with('/'))
}
fn spa_navigation(path: &str, headers: &HeaderMap) -> bool {
    // File-looking/reserved asset routes never receive an HTML-success fallback.
    if [
        "/api",
        "/data",
        "/data-saver",
        "/covers",
        "/originals",
        "/uploads",
        "/proxy",
        "/og-image",
        "/_app",
        "/fonts",
        "/images",
        "/templates",
        "/icons",
        "/flags",
    ]
    .iter()
    .any(|prefix| segment_prefix(path, prefix))
    {
        return false;
    }
    if segment_prefix(path, "/auth") && !FRONTEND_AUTH_ROUTES.contains(&path) {
        return false;
    }
    if path
        .rsplit('/')
        .next()
        .is_some_and(|part| part.contains('.'))
    {
        return false;
    }
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(',')
                .any(|kind| kind.trim().split(';').next() == Some("text/html"))
        })
        || headers
            .get("sec-fetch-mode")
            .is_some_and(|v| v == "navigate")
}
async fn static_fallback(State(state): State<AppState>, request: Request) -> Response {
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        return ApiError::not_found("Unknown endpoint").into_response();
    }
    let path = request.uri().path();
    if [
        "/api",
        "/data",
        "/data-saver",
        "/covers",
        "/originals",
        "/uploads",
        "/proxy",
        "/og-image",
    ]
    .iter()
    .any(|prefix| segment_prefix(path, prefix))
    {
        return ApiError::not_found("Endpoint unavailable").into_response();
    }
    if segment_prefix(path, "/auth") && !FRONTEND_AUTH_ROUTES.contains(&path) {
        return api_not_found().await.into_response();
    }
    // Build helper preserves original Go templates for recovery; they are not a
    // public static interface and should not be downloadable at runtime.
    if segment_prefix(path, "/templates") {
        return ApiError::not_found("Asset not found").into_response();
    }
    let navigation = spa_navigation(path, request.headers());
    serve_ui(
        &state.config.ui_dir,
        &state.config.ui_fallback,
        request,
        navigation,
    )
    .await
}

async fn serve_ui(
    ui_dir: &std::path::Path,
    fallback: &str,
    request: Request,
    navigation: bool,
) -> Response {
    let response = if navigation {
        // not_found_service forces404 even for a successful file response.
        // This branch is already constrained to genuine HTML navigation.
        ServeDir::new(ui_dir)
            .fallback(ServeFile::new(ui_dir.join(fallback)))
            .oneshot(request)
            .await
    } else {
        ServeDir::new(ui_dir).oneshot(request).await
    };
    match response {
        Ok(response) => response.into_response(),
        Err(error) => match error {},
    }
}

/// The original external path survives nested routers and middleware. It also
/// keeps public envelopes/classification independent of stripped adapter URIs.
fn original_path(request: &Request) -> &str {
    request.extensions().get::<OriginalUri>().map(|uri| uri.0.path())
        .unwrap_or_else(|| request.uri().path())
}
fn public_request(request: &Request) -> bool {
    segment_prefix(original_path(request), "/api/v1")
}
fn boundary_error(public: bool, error: ApiError) -> Response {
    if public {
        crate::public_api::PublicApiError::new(error.status,
            error.status.canonical_reason().unwrap_or("Request Failed"),error.message).into_response()
    } else { error.into_response() }
}
fn read_class(request: &Request) -> Class {
    let path=original_path(request);
    let public=path.strip_prefix("/api/v1");
    let path=public.or_else(||path.strip_prefix("/api")).unwrap_or(path);
    let segments=path.split('/').filter(|s|!s.is_empty()).collect::<Vec<_>>();
    let private=path.starts_with("/user/history") || path.starts_with("/user/follows")
        || public.is_some() && segment_prefix(path, "/account/library")
        || (public.is_some() && matches!(segments.as_slice(),["nostr","manga",_,"reading-status"] | ["nostr","user",_,"profile"]))
        || (public.is_some() && (path=="/manga/read" || path=="/manga/status"
            || matches!(segments.as_slice(),["manga",_,"read"|"status"])
            || path=="/user/list" || matches!(segments.as_slice(),["user",_,"list"])
            || matches!(segments.as_slice(),["list",_] | ["list",_,"feed"])));
    if private { Class::LibraryHead } else { Class::Catalogue }
}

/// Owned work keeps both request and read permits after client disconnect.
/// Body deadlines are streaming intake limits, not cancellation of SQL work.
async fn admitted(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let public=public_request(&request);
    let request_permit = match state.requests.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => return boundary_error(public, ApiError::busy()),
    };
    let read_permit = if matches!(*request.method(), Method::GET | Method::HEAD) {
        let class = read_class(&request);
        match state.reads.acquire(class).await {
            Ok(permit) => Some(permit),
            Err(_) => return boundary_error(public, ApiError::busy()),
        }
    } else {
        None
    };
    let (sender, receiver) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let _request_permit = request_permit;
        let _read_permit = read_permit;
        let response = next.run(request).await;
        let _ = sender.send(response);
    });
    receiver
        .await
        .unwrap_or_else(|_| boundary_error(public, ApiError::unavailable()))
}

fn request_origin(headers: &HeaderMap, expected: &str) -> Result<Option<HeaderValue>, ApiError> {
    let mut values = headers.get_all(header::ORIGIN).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(ApiError::forbidden("Invalid request origin"));
    }
    let source = url::Url::parse(
        value
            .to_str()
            .map_err(|_| ApiError::forbidden("Invalid request origin"))?,
    )
    .map_err(|_| ApiError::forbidden("Invalid request origin"))?;
    let expected = url::Url::parse(expected).map_err(|_| ApiError::unavailable())?;
    if source.username().is_empty()
        && source.password().is_none()
        && source.query().is_none()
        && source.fragment().is_none()
        && source.path() == "/"
        && source.origin() == expected.origin()
    {
        Ok(Some(value.clone()))
    } else {
        Err(ApiError::forbidden("Cross-origin requests are not allowed"))
    }
}
/// Only the native completion and Continue seams get the account preflight
/// header. This is not a global native API or public/signed CORS expansion.
fn native_account_reader_path(path: &str) -> bool {
    path.strip_prefix("/api/user/history/manga/")
        .and_then(|rest| rest.strip_suffix("/completion").or_else(|| rest.strip_suffix("/continue")))
        .is_some_and(|id| !id.is_empty() && !id.contains('/'))
}
fn preflight_headers_allowed(value: &str, native_account_reader: bool) -> bool {
    value.split(',').all(|h| {
        let h = h.trim().to_ascii_lowercase();
        ["accept", "authorization", "content-type", "x-csrf-token"].contains(&h.as_str())
            || native_account_reader && h == "x-weebdex-expected-account"
    })
}
fn signed_upload_replacement_path(parts: &[&str]) -> bool {
    matches!(parts, ["api", "v1", "nostr", "uploads", session, "files", file, "replace"]
        if crate::public_api::PublicUuid::parse(session).is_ok()
            && crate::public_api::PublicUuid::parse(file).is_ok())
}
fn signed_native_user_report_budget(path: &str, method: &Method) -> Option<usize> {
    let (kind, rest) = path.strip_prefix("/api/v1/nostr/")?.split_once('/')?;
    if !matches!(kind, "user" | "manga" | "chapter" | "group") { return None; }
    let (user, suffix) = rest.split_once('/')?;
    (suffix == "discussion/reports" && *method == Method::POST
        && crate::public_api::PublicUuid::parse(user).is_ok())
        .then_some(crate::public_api::native_user_reports::BODY_LIMIT)
}
fn signed_moderator_decision_budget(path:&str,method:&Method)->Option<usize> {
    let id=path.strip_prefix("/api/v1/nostr/moderation/reports/")?;
    // encodeURIComponent uses %3A for the only escaped character in native IDs.
    let decoded=id.replace("%3A",":");
    (*method==Method::POST && crate::domains::catalog::validation::id(&decoded).is_ok())
        .then_some(crate::public_api::native_user_reports::BODY_LIMIT)
}
fn body_budget(path: &str, method: &Method, state: &AppState) -> (usize, Duration) {
    if let Some(maximum) = signed_native_user_report_budget(path, method).or_else(||signed_moderator_decision_budget(path,method)) {
        return (maximum, Duration::from_secs(state.config.json_body_timeout_seconds));
    }
    let parts = path
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if *method == Method::POST && signed_upload_replacement_path(&parts) {
        return (40 * 1024 * 1024, Duration::from_secs(state.config.upload_body_timeout_seconds));
    }
    if *method == Method::POST && parts.len() == 4 && parts[..3] == ["api", "v1", "cover"]
        && crate::public_api::PublicUuid::parse(parts[3]).is_ok() {
        return (40 * 1024 * 1024, Duration::from_secs(state.config.upload_body_timeout_seconds));
    }
    if *method == Method::POST && parts.len() == 4
        && parts[..3] == ["api", "v1", "upload"]
        && crate::public_api::PublicUuid::parse(parts[3]).is_ok()
    {
        return (96 * 1024 * 1024, Duration::from_secs(state.config.upload_body_timeout_seconds));
    }
    if *method == Method::POST
        && parts.len() == 6
        && parts[0] == "api"
        && parts[1] == "upload"
        && parts[3] == "files"
        && parts[5] == "replace"
    {
        return (
            40 * 1024 * 1024,
            Duration::from_secs(state.config.upload_body_timeout_seconds),
        );
    }
    if *method == Method::POST && parts.len() == 3 && parts[0] == "api" {
        match parts[1] {
            "upload" if !matches!(parts[2], "begin" | "check-approval-required") => {
                return (
                    96 * 1024 * 1024,
                    Duration::from_secs(state.config.upload_body_timeout_seconds),
                );
            }
            "cover" => {
                return (
                    40 * 1024 * 1024,
                    Duration::from_secs(state.config.upload_body_timeout_seconds),
                );
            }
            _ => {}
        }
    }
    (
        state.config.maximum_json_bytes,
        Duration::from_secs(state.config.json_body_timeout_seconds),
    )
}
async fn request_boundary(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let public=public_request(&request);
    let probe = matches!(request.uri().path(), "/livez" | "/readyz" | "/healthz");
    if state.runtime.is_draining() && !probe {
        return boundary_error(public, ApiError::unavailable());
    }
    if state.config.production && !probe {
        let expected = match url::Url::parse(&state.config.public_origin) {
            Ok(url) => url,
            Err(_) => return boundary_error(public, ApiError::unavailable()),
        };
        let authority = &expected[url::Position::BeforeHost..url::Position::AfterPort];
        let mut hosts = request.headers().get_all(header::HOST).iter();
        if hosts.next().and_then(|value| value.to_str().ok()) != Some(authority)
            || hosts.next().is_some()
        {
            return boundary_error(public, ApiError::forbidden("Invalid request host"));
        }
    }
    let origin = match request_origin(request.headers(), &state.config.public_origin) {
        Ok(origin) => origin,
        Err(error) => return boundary_error(public, error),
    };
    if request.method() == Method::OPTIONS {
        if let Some(origin) = origin {
            let native_account_reader = native_account_reader_path(original_path(&request));
            let allowed = ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE"];
            let method = request
                .headers()
                .get(header::ACCESS_CONTROL_REQUEST_METHOD)
                .and_then(|v| v.to_str().ok());
            let headers_allowed = request
                .headers()
                .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
                .and_then(|v| v.to_str().ok())
                .is_none_or(|v| preflight_headers_allowed(v, native_account_reader));
            if !method.is_some_and(|m| allowed.contains(&m))
                || native_account_reader && method != Some("GET") || !headers_allowed {
                return boundary_error(public, ApiError::forbidden("Unsupported cross-origin request"));
            }
            let mut response = StatusCode::NO_CONTENT.into_response();
            response
                .headers_mut()
                .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
            response.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                HeaderValue::from_static("true"),
            );
            response.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static(if native_account_reader { "GET" } else { "GET, HEAD, POST, PUT, PATCH, DELETE" }),
            );
            response.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                HeaderValue::from_static(if native_account_reader {
                    "Accept, Authorization, Content-Type, X-CSRF-Token, X-WeebDex-Expected-Account"
                } else { "Accept, Authorization, Content-Type, X-CSRF-Token" }),
            );
            response.headers_mut().insert(
                header::VARY,
                HeaderValue::from_static(
                    "Origin, Access-Control-Request-Method, Access-Control-Request-Headers",
                ),
            );
            return response;
        }
    }
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        let (maximum, deadline) = body_budget(request.uri().path(), request.method(), &state);
        if request
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .is_some_and(|n| n > maximum as u64)
        {
            return boundary_error(public, ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "body_too_large",
                "Request body exceeds the route limit",
            ));
        }
        let (parts, body) = request.into_parts();
        request = Request::from_parts(parts, BoundedBody::wrap(body, maximum, deadline));
    }
    let mut response = next.run(request).await;
    if let Some(origin) = origin {
        response
            .headers_mut()
            .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
            HeaderValue::from_static("true"),
        );
        response
            .headers_mut()
            .append(header::VARY, HeaderValue::from_static("Origin"));
    }
    response
}

fn signed_relation_page_private(parts: &[&str], method: &Method) -> bool {
    matches!(*method,Method::GET|Method::HEAD) && match parts {
        ["api","v1","nostr","manga",id,"relations"]
        | ["api","v1","manga","draft",id,"relation"] => crate::public_api::PublicUuid::parse(id).is_ok(),
        _ => false,
    }
}
fn signed_native_user_discussion_private(path: &str, method: &Method) -> bool {
    let Some((user, suffix)) = path.strip_prefix("/api/v1/nostr/user/").and_then(|rest| rest.split_once('/')) else { return false; };
    suffix == "discussion" && crate::public_api::PublicUuid::parse(user).is_ok()
        && matches!(*method, Method::GET | Method::HEAD | Method::POST)
}
fn signed_native_user_report_private(path: &str, method: &Method) -> bool {
    if signed_native_user_report_budget(path, method).is_some() { return true; }
    let Some((kind, rest)) = path.strip_prefix("/api/v1/nostr/").and_then(|rest| rest.split_once('/')) else { return false; };
    if !matches!(kind, "user" | "manga" | "chapter" | "group") { return false; }
    let Some((user, suffix)) = rest.split_once('/') else { return false; };
    suffix == "discussion/reports/context" && crate::public_api::PublicUuid::parse(user).is_ok()
        && matches!(*method, Method::GET | Method::HEAD)
}
fn account_closure_private(path:&str, method:&Method)->bool {
    let parts=path.split('/').filter(|part|!part.is_empty()).collect::<Vec<_>>();
    matches!(parts.as_slice(),["api","account","closure",_]
        | ["api","v1","nostr","user",_,"closure"]
        | ["api","v1","user","delete",_])
        || matches!(parts.as_slice(),["api","v1","user",_]) && *method==Method::DELETE
}
async fn response_headers(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let service_worker = request.uri().path() == "/service-worker.js";
    let native_account_reader = native_account_reader_path(original_path(&request))
        || segment_prefix(original_path(&request), "/api/v1/account");
    let closure_private=account_closure_private(original_path(&request),request.method());
    let signed_report_private = signed_native_user_report_private(original_path(&request), request.method())
        || segment_prefix(original_path(&request), "/api/v1/nostr/moderation/reports");
    let signed_upload_private = {
        let parts = original_path(&request).split('/').filter(|part| !part.is_empty()).collect::<Vec<_>>();
        closure_private || signed_upload_schedule_private(&parts, request.method()) || signed_report_private || signed_native_user_discussion_private(original_path(&request), request.method())
            || signed_relation_page_private(&parts, request.method()) || matches!(parts.as_slice(), ["api", "v1", "cover", _]) && matches!(*request.method(), Method::POST | Method::PUT | Method::DELETE)
            || matches!(parts.as_slice(),["api","v1","nostr","manga",_,"reading-status"] | ["api","v1","nostr","user",_,"profile"])
            || matches!(parts.as_slice(),["api","v1","user",_]) && *request.method()==Method::PUT
            || matches!(parts.as_slice(),["api","v1","manga",_,"status"]) && *request.method()==Method::POST
            || matches!(parts.as_slice(), ["api", "v1", "manga", _, "lock" | "unlock"])
            || matches!(parts.as_slice(), ["api", "v1", "manga", _]) && *request.method()==Method::DELETE
            || matches!(parts.as_slice(), ["api", "v1", "nostr", "manga", _, "covers"])
            || matches!(parts.as_slice(), ["api", "v1", "nostr", "covers", _, "original"])
            || signed_upload_replacement_path(&parts) || matches!(parts.as_slice(), ["api", "v1", "nostr", "uploads", session, "files", file, "preview"]
            if crate::public_api::PublicUuid::parse(session).is_ok()
                && crate::public_api::PublicUuid::parse(file).is_ok())
    };
    let private_response = segment_prefix(request.uri().path(), "/api")
        || segment_prefix(request.uri().path(), "/auth")
        || matches!(request.uri().path(), "/healthz" | "/livez" | "/readyz");
    let mut response = next.run(request).await;
    if private_response {
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static(
                if signed_upload_private || native_account_reader { "private, no-store" } else { "no-store" }));
    }
    if native_account_reader {
        response.headers_mut().append(header::VARY, HeaderValue::from_static("Cookie, Authorization, X-WeebDex-Expected-Account"));
    }
    if signed_upload_private {
        response.headers_mut().append(header::VARY, HeaderValue::from_static(if signed_report_private || closure_private { "Authorization, Cookie" } else { "Authorization" }));
    }
    if service_worker {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache, must-revalidate"),
        );
    }
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
        .headers_mut()
        .insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    response.headers_mut().insert(
        "cross-origin-opener-policy",
        HeaderValue::from_static("same-origin"),
    );
    response.headers_mut().insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
    response.headers_mut().insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=(), usb=()"),
    );
    if state.config.production {
        response.headers_mut().insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000"),
        );
    }
    if closure_private {response.headers_mut().insert(header::REFERRER_POLICY,HeaderValue::from_static("no-referrer"));}
    if !response.headers().contains_key(header::REFERRER_POLICY) {
        response.headers_mut().insert(
            header::REFERRER_POLICY,
            HeaderValue::from_static("same-origin"),
        );
    }
    if response.status() == StatusCode::SERVICE_UNAVAILABLE {
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static(if signed_upload_private || native_account_reader { "private, no-store" } else { "no-store" }));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_moderator_budget_does_not_widen_other_native_routes() {
        for id in ["NativeReport","report_1","report:1","report%3A1"] {assert_eq!(signed_moderator_decision_budget(&format!("/api/v1/nostr/moderation/reports/{id}"),&Method::POST),Some(131072));}
        for id in ["","bad/id","bad%2Fid","bad%253Aid","../other","bad?query", "bad%3aid"] {assert_eq!(signed_moderator_decision_budget(&format!("/api/v1/nostr/moderation/reports/{id}"),&Method::POST),None);}
        for method in [Method::GET,Method::HEAD,Method::PUT,Method::PATCH,Method::DELETE] {assert_eq!(signed_moderator_decision_budget("/api/v1/nostr/moderation/reports/report_1",&method),None);}
    }
    #[test]
    fn signed_native_user_report_budget_and_privacy_are_exact() {
        let path = "/api/v1/nostr/user/11111111-1111-8111-8111-111111111111/discussion/reports";
        assert_eq!(signed_native_user_report_budget(path, &Method::POST), Some(131_072));
        assert!(signed_native_user_report_private(path, &Method::POST));
        let context = format!("{path}/context");
        for method in [Method::GET, Method::HEAD] {
            assert!(signed_native_user_report_private(&context, &method));
            assert_eq!(signed_native_user_report_budget(&context, &method), None);
            assert_eq!(signed_native_user_report_budget(path, &method), None);
            assert!(!signed_native_user_report_private(path, &method));
        }
        for bad in [format!("{path}/"),format!("{path}/extra"),context,path.replace("/user/","/user//"),path.replace("11111111-1111-8111-8111-111111111111","native1234"),path.replace("/v1/","/v10/"),path.replace("/reports","")] {
            assert_eq!(signed_native_user_report_budget(&bad, &Method::POST), None, "{bad}");
            assert!(!signed_native_user_report_private(&bad, &Method::POST), "{bad}");
        }
        for method in [Method::PUT,Method::DELETE,Method::OPTIONS] {
            assert_eq!(signed_native_user_report_budget(path, &method), None);
            assert!(!signed_native_user_report_private(path, &method));
        }
    }
    #[test]
    fn generic_native_report_budget_and_privacy_are_exact() {
        let id = "11111111-1111-8111-8111-111111111111";
        for kind in ["manga", "chapter", "group"] {
            let path = format!("/api/v1/nostr/{kind}/{id}/discussion/reports");
            assert_eq!(signed_native_user_report_budget(&path, &Method::POST), Some(131_072));
            assert!(signed_native_user_report_private(&path, &Method::POST));
            for method in [Method::GET, Method::HEAD] {
                assert!(signed_native_user_report_private(&format!("{path}/context"), &method));
                assert!(!signed_native_user_report_private(&path, &method));
                assert_eq!(signed_native_user_report_budget(&path, &method), None);
            }
            for bad in [format!("{path}/"),format!("{path}/context"),format!("{path}/extra"),path.replace(id,"native-id"),path.replace(kind,"publisher"),path.replace(kind,"author"),path.replace("/reports","")] {
                assert_eq!(signed_native_user_report_budget(&bad, &Method::POST), None);
                assert!(!signed_native_user_report_private(&bad, &Method::POST));
            }
        }
    }
    #[test]
    fn signed_native_user_discussion_private_policy_is_exact_typed() {
        let path = "/api/v1/nostr/user/11111111-1111-8111-8111-111111111111/discussion";
        for method in [Method::GET, Method::HEAD, Method::POST] {
            assert!(signed_native_user_discussion_private(path, &method));
        }
        for method in [Method::PUT, Method::DELETE, Method::OPTIONS] {
            assert!(!signed_native_user_discussion_private(path, &method));
        }
        for bad in [format!("{path}/"), format!("{path}/extra"), path.replace("/user/", "/user//"), path.replace("11111111-1111-8111-8111-111111111111", "native1234"), path.replace("/v1/", "/v10/"), path.replace("/discussion", "/profile")] {
            assert!(!signed_native_user_discussion_private(&bad, &Method::POST), "{bad}");
        }
    }
    #[test]
    fn schedule_private_routes_are_exact_typed_and_method_bound() {
        let id="11111111-1111-8111-8111-111111111111";
        assert!(signed_upload_schedule_private(&["api","v1","nostr","uploads","schedules"],&Method::GET));
        assert!(signed_upload_schedule_private(&["api","v1","nostr","uploads",id,"schedule"],&Method::GET));
        assert!(signed_upload_schedule_private(&["api","v1","nostr","uploads",id,"schedule","retry"],&Method::POST));
        for (parts,method) in [
            (vec!["api","v1","nostr","uploads","schedules"],Method::POST),
            (vec!["api","v1","nostr","uploads",id,"schedule"],Method::HEAD),
            (vec!["api","v1","nostr","uploads",id,"schedule","retry"],Method::GET),
            (vec!["api","v1","nostr","uploads","native-id","schedule"],Method::GET),
            (vec!["api","v1","nostr","uploads",id,"schedule","extra"],Method::GET),
        ] {assert!(!signed_upload_schedule_private(&parts,&method));}
    }

    #[test]
    fn signed_replacement_body_budget_and_private_cache_select_exact_public_path() {
        let session="11111111-1111-4111-8111-111111111111";
        let file="22222222-2222-4222-8222-222222222222";
        assert!(signed_upload_replacement_path(&["api","v1","nostr","uploads",session,"files",file,"replace"]));
        for path in [
            vec!["api","v1","nostr","uploads","native1234","files",file,"replace"],
            vec!["api","v1","nostr","uploads",session,"files","native1234","replace"],
            vec!["api","v1","nostr","uploads",session,"files",file,"preview"],
            vec!["api","v1","nostr","uploads",session,"files",file,"replace","extra"],
            vec!["api","upload",session,"files",file,"replace"],
        ] { assert!(!signed_upload_replacement_path(&path)); }
    }
    #[test]
    fn signed_relation_page_cache_policy_is_exact_typed_and_read_only() {
        let id="11111111-1111-8111-8111-111111111111";
        for path in [vec!["api","v1","nostr","manga",id,"relations"],vec!["api","v1","manga","draft",id,"relation"]] {
            assert!(signed_relation_page_private(&path,&Method::GET));assert!(signed_relation_page_private(&path,&Method::HEAD));
            assert!(!signed_relation_page_private(&path,&Method::POST));
        }
        for path in [vec!["api","v1","nostr","manga","native1234","relations"],vec!["api","v1","nostr","manga",id,"relations","extra"],vec!["api","v1","manga",id,"relation"],vec!["api","manga",id,"relations"]] {
            assert!(!signed_relation_page_private(&path,&Method::GET));
        }
    }
    #[test]
    fn account_disable_privacy_includes_errors_and_wrong_methods() {
        for path in ["/api/account/closure/prepare","/api/account/closure/confirm","/api/account/closure/user", "/api/v1/nostr/user/bad-id/closure","/api/v1/user/delete/bad-code"] {
            for method in [Method::GET,Method::POST,Method::DELETE] {assert!(account_closure_private(path,&method));}
        }
        assert!(account_closure_private("/api/v1/user/bad-id",&Method::DELETE));
        assert!(!account_closure_private("/api/v1/user/user",&Method::GET));
        assert!(!account_closure_private("/api/v1/nostr/user/user/profile",&Method::GET));
    }
    #[tokio::test]
    async fn unknown_api_is_a_real_error() {
        let response = api_not_found().await.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
    #[tokio::test]
    async fn navigation_fallback_returns200_but_missing_assets_remain404() {
        let directory = tempfile::tempdir().unwrap();
        tokio::fs::write(
            directory.path().join("index.html"),
            "<html>synthetic fallback</html>",
        )
        .await
        .unwrap();
        let request = Request::builder()
            .uri("/library")
            .header(header::ACCEPT, "text/html")
            .body(axum::body::Body::empty())
            .unwrap();
        let navigation = spa_navigation(request.uri().path(), request.headers());
        let response = serve_ui(directory.path(), "index.html", request, navigation).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap(),
            "<html>synthetic fallback</html>"
        );
        let request = Request::builder()
            .uri("/_app/missing.js")
            .header(header::ACCEPT, "text/html")
            .body(axum::body::Body::empty())
            .unwrap();
        let navigation = spa_navigation(request.uri().path(), request.headers());
        assert!(!navigation);
        assert_eq!(
            serve_ui(directory.path(), "index.html", request, navigation)
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    #[test]
    fn spa_fallback_is_only_for_navigation_and_never_missing_assets() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT, HeaderValue::from_static("text/html"));
        for path in [
            "/library",
            "/title/Abcdef1234",
            "/auth/login",
            "/announcements/manage",
        ] {
            assert!(spa_navigation(path, &headers), "{path}");
        }
        for path in [
            "/_app/missing.js",
            "/fonts/missing",
            "/templates/index.html",
            "/api/manga",
            "/auth/self-service/missing",
            "/service-worker.js",
            "/manifest.webmanifest",
            "/data/chapter/page",
            "/originals/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            assert!(!spa_navigation(path, &headers), "{path}");
        }
        assert!(!spa_navigation("/library", &HeaderMap::new()));
    }
    #[test]
    fn cors_never_allows_external_null_or_duplicate_origins() {
        for value in [
            "https://attacker.invalid",
            "null",
            "https://example.invalid/path",
            "https://user@example.invalid",
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(header::ORIGIN, value.parse().unwrap());
            assert!(request_origin(&headers, "https://example.invalid").is_err());
        }
        let mut headers = HeaderMap::new();
        headers.insert(header::ORIGIN, "https://example.invalid".parse().unwrap());
        assert!(
            request_origin(&headers, "https://example.invalid/")
                .unwrap()
                .is_some()
        );
        headers.append(header::ORIGIN, "https://example.invalid".parse().unwrap());
        assert!(request_origin(&headers, "https://example.invalid").is_err());
    }
    #[test]
    fn account_reader_header_scope_is_native_and_route_local() {
        for (id, operation) in ["chapter", "native%3A128", "%61"].into_iter().flat_map(|id| ["completion", "continue"].map(|operation| (id, operation))) {
            let path = format!("/api/user/history/manga/{id}/{operation}");
            assert!(native_account_reader_path(&path));
            for method in [Method::GET, Method::HEAD] {
                let request = Request::builder().method(method).uri(&path).body(axum::body::Body::empty()).unwrap();
                assert!(matches!(read_class(&request), Class::LibraryHead));
            }
        }
        for path in ["/api/user/history/manga/id", "/api/v1/user/history/manga/id/completion", "/api/user/history/manga//completion", "/api/user/history/manga/id/completion/extra", "/api/user/history/manga/a/b/completion", "/api/v1/user/history/manga/id/continue", "/api/user/history/manga//continue", "/api/user/history/manga/id/continue/extra", "/api/user/history/manga/a/b/continue"] {
            assert!(!native_account_reader_path(path), "{path}");
        }
        let header = "Accept, X-WeebDex-Expected-Account";
        assert!(preflight_headers_allowed(header, true));
        assert!(!preflight_headers_allowed(header, false));
        assert!(!preflight_headers_allowed("X-Other", true));
        assert!(!preflight_headers_allowed("X-WeebDex-Expected-Account, X-Other", true));
        assert!(preflight_headers_allowed("Accept, Content-Type", false));
    }
}

#[cfg(test)]
mod public_boundary_tests {
    use super::*;
    use axum::body::{Body,to_bytes};
    #[tokio::test]
    async fn outer_errors_keep_public_status_and_native_envelopes() {
        for (public,status) in [(true,StatusCode::PAYLOAD_TOO_LARGE),(true,StatusCode::TOO_MANY_REQUESTS),(true,StatusCode::SERVICE_UNAVAILABLE),(true,StatusCode::FORBIDDEN),(false,StatusCode::PAYLOAD_TOO_LARGE)] {
            let response=boundary_error(public,ApiError::new(status,"fixture_error","Fixture boundary"));
            assert_eq!(response.status(),status);
            let body:serde_json::Value=serde_json::from_slice(&to_bytes(response.into_body(),8192).await.unwrap()).unwrap();
            if public { assert_eq!(body["result"],"error"); assert_eq!(body["errors"][0]["status"],status.as_u16()); assert!(body.get("code").is_none()); }
            else { assert_eq!(body["code"],"fixture_error"); assert!(body.get("errors").is_none()); }
        }
    }
    #[test]
    fn nested_private_reads_use_original_external_uri() {
        for path in ["/api/v1/nostr/user/id/profile","/api/v1/nostr/manga/id/reading-status","/api/v1/manga/read","/api/v1/manga/status","/api/v1/manga/id/read","/api/v1/manga/id/status","/api/v1/user/list","/api/v1/user/id/list","/api/v1/list/id","/api/v1/list/id/feed","/api/user/history","/api/user/follows/manga"] {
            let mut request=Request::builder().uri("/stripped").body(Body::empty()).unwrap();
            request.extensions_mut().insert(OriginalUri(path.parse().unwrap()));
            assert!(matches!(read_class(&request),Class::LibraryHead),"{path}");
            assert_eq!(public_request(&request),path.starts_with("/api/v1/"));
        }
        for path in ["/api/v1/manga","/api/v1/chapter/id","/api/v10/manga/read","/api/manga/status"] {
            let request=Request::builder().uri(path).body(Body::empty()).unwrap();
            assert!(matches!(read_class(&request),Class::Catalogue),"{path}");
        }
    }
}
