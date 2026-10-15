mod blossom_http;
mod catalogue_http;
mod compatibility_http;
mod enrollment_http;
mod read_admission;
mod reader_http;
mod review_http;
mod service_discovery;
use crate::{
    auth,
    db::{DbConfig, Id, Repository},
};
use anyhow::{Context, Result, ensure};
use axum::{
    Json, Router,
    extract::{OriginalUri, Path, Query, RawQuery, Request, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeConfig {
    #[serde(default)]
    pub enrollment: crate::enrollment::Policy,
    #[serde(default)]
    pub read_policy: crate::relay::ReadPolicy,
    #[serde(default = "cors_origins")]
    pub cors_origins: Vec<String>,
    #[serde(default)]
    pub discovery_private_overrides: Vec<String>,
    #[serde(default = "review_defaults")]
    pub submission_review: Option<crate::submission_review::ReviewLimits>,
    #[serde(default)]
    pub public_serving: Option<crate::serving_role::PublicServingConfig>,
    pub bind: SocketAddr,
    pub base_url: String,
    pub service_key_file: PathBuf,
    pub admin_pubkey: String,
    #[serde(default = "pool_size")]
    pub database_connections: usize,
    #[serde(default)]
    pub read_admission: Option<crate::read_admission::Config>,
    #[serde(default = "pool_wait")]
    pub database_wait_ms: u64,
    #[serde(default = "cache_bytes")]
    pub cache_bytes: u64,
    #[serde(default)]
    pub database_plaintext_local: bool,
    #[serde(default)]
    pub storage: Option<crate::upload::StorageConfig>,
    #[serde(default)]
    pub library_concurrency: crate::library_storage::LibraryConcurrency,
    #[serde(default)]
    pub backend: Option<crate::backend::BackendConfig>,
    #[serde(default)]
    pub auto_approve_publication_deletions: bool,
    #[serde(default)]
    pub library_reclamation: bool,
    #[serde(default)]
    pub publication_reclamation: bool,
    #[serde(default = "sync_worker_default")]
    pub staging_reclamation: bool,
    #[serde(default)]
    pub storage_audit_interval_seconds: u64,
    #[serde(default = "sync_worker_default")]
    pub sync_worker_enabled: bool,
    #[serde(default = "sync_worker_default")]
    pub peer_discovery_enabled: bool,
    #[serde(default)]
    pub group_keys_dir: Option<PathBuf>,
}
fn cors_origins() -> Vec<String> {
    vec!["*".into()]
}
fn sync_worker_default() -> bool {
    true
}
fn pool_size() -> usize {
    8
}
fn pool_wait() -> u64 {
    500
}
fn cache_bytes() -> u64 {
    16 * 1024 * 1024
}
struct Signer {
    secret: Zeroizing<String>,
    public: String,
}
impl Signer {
    fn load(path: &std::path::Path) -> Result<Self> {
        let metadata =
            std::fs::metadata(path).context("cannot read configured service-key file")?;
        ensure!(
            metadata.is_file() && metadata.len() <= 128,
            "invalid service-key file"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            ensure!(
                metadata.permissions().mode() & 0o077 == 0,
                "service-key file must not be group/world accessible"
            );
        }
        let raw = Zeroizing::new(std::fs::read_to_string(path).context("cannot read service key")?);
        let secret = Zeroizing::new(raw.trim().to_owned());
        let public = auth::sign(&secret, 1, vec![], String::new())
            .map_err(|_| anyhow::anyhow!("invalid service key"))?
            .pubkey;
        Ok(Self { secret, public })
    }
    fn proof(&self, nonce: String, endpoint: String) -> Result<auth::Event> {
        auth::sign(
            &self.secret,
            20005,
            vec![
                vec!["nonce".into(), nonce],
                vec!["endpoint".into(), endpoint],
            ],
            "Local experimental service proof; not TLS-channel-bound".into(),
        )
        .map_err(anyhow::Error::msg)
    }
}
fn review_defaults() -> Option<crate::submission_review::ReviewLimits> {
    Some(Default::default())
}

struct App {
    enrollment: crate::enrollment::Policy,
    endpoint_policy: crate::endpoint_policy::EndpointPolicy,
    discovery_slots: Arc<tokio::sync::Semaphore>,
    workers: Arc<crate::background_workers::BackgroundWorkers>,
    operations: Arc<EmbeddedOperations>,
    capabilities: Value,
    cors_origins: Vec<String>,
    serving_status: Option<Arc<crate::serving_role::ServingStatus>>,
    proof_slots: Arc<tokio::sync::Semaphore>,
    client_proof_slots: Arc<tokio::sync::Semaphore>,
    relay: Arc<crate::relay::Relay>,
    peer_relay: Arc<crate::relay::Relay>,
    repo: Repository,
    signer: Signer,
    base_url: String,
    admin: String,
    storage: Option<Arc<crate::upload::UploadStore>>,
    projection_enabled: bool,
    publications: Option<Arc<crate::publication::Publications>>,
    reviews: Option<Arc<crate::submission_review::Reviews>>,
    event_slots: Arc<tokio::sync::Semaphore>,
    reads: Arc<crate::read_admission::Admission>,
    auto_approve_publication_deletions: bool,
    peer_sync: Option<Arc<crate::peers::PeerSync>>,
    peer_blob_reads: std::sync::atomic::AtomicU64,
    snapshots: Arc<crate::inventory::Snapshots>,
    groups: Option<Arc<crate::groups::Groups>>,
    library_objects: Option<Arc<crate::library_storage::LibraryObjects>>,
}
type Shared = Arc<App>;
type ApiError = (StatusCode, Json<Value>);
fn forbidden(message: &str) -> ApiError {
    (
        StatusCode::FORBIDDEN,
        Json(json!({"error":"authorization","message":message})),
    )
}
fn account_error(error: anyhow::Error, message: &str) -> ApiError {
    if error.is::<crate::accounts::AccountDenied>() {
        forbidden(message)
    } else {
        tracing::warn!(error=%error,"account authorization unavailable");
        unavailable()
    }
}
fn role_error(error: anyhow::Error, message: &str) -> ApiError {
    if crate::db::is_database_error(&error) {
        unavailable()
    } else {
        forbidden(message)
    }
}
fn validation_error(error: anyhow::Error, message: &str) -> ApiError {
    if crate::db::is_database_error(&error) {
        unavailable()
    } else {
        bad(message)
    }
}
fn publisher_error(error: anyhow::Error) -> ApiError {
    if error.is::<crate::publisher_authority::PublisherDenied>() {
        forbidden("current publisher authority required")
    } else {
        tracing::warn!(error=%error,"publisher operation unavailable");
        unavailable()
    }
}
fn publisher_read_error(error: anyhow::Error) -> ApiError {
    if error.is::<crate::download_budget::Busy>() {
        return download_busy();
    }

    if matches!(
        error.downcast_ref::<crate::publisher_authority::PublisherDenied>(),
        Some(crate::publisher_authority::PublisherDenied::MissingGrant)
    ) || error.is::<crate::upload::BlobUnavailable>()
    {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"blob_unavailable"})),
        )
    } else {
        publisher_error(error)
    }
}
fn download_busy() -> ApiError {
    (
        StatusCode::TOO_MANY_REQUESTS,
        Json(json!({"error":"download_budget_busy"})),
    )
}
fn download_error(error: anyhow::Error) -> ApiError {
    if error.is::<crate::download_budget::Busy>() {
        download_busy()
    } else if error.is::<crate::upload::ReadVerificationFailed>()
        || error.is::<crate::upload::BlobUnavailable>()
    {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"original_unavailable"})),
        )
    } else {
        unavailable()
    }
}
fn unavailable() -> ApiError {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({"error":"unavailable","message":"metadata store unavailable"})),
    )
}
fn bad(message: &str) -> ApiError {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error":"invalid_request","message":message})),
    )
}
fn id(text: &str) -> Result<Id> {
    ensure!(auth::valid_hash(text), "invalid public identifier");
    Ok(hex::decode(text)?.try_into().unwrap())
}
pub(crate) fn normalize_origin(value: &str) -> Result<String> {
    let url = url::Url::parse(value).context("invalid public base URL")?;
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url.path() == "/",
        "base URL must be an origin without credentials or suffix"
    );
    let local = url.host_str().is_some_and(|h| {
        h == "localhost"
            || h.parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    ensure!(
        url.scheme() == "https" || (url.scheme() == "http" && local),
        "public base URL requires HTTPS outside loopback"
    );
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

#[derive(Clone)]
struct ReadIdentity(Id);

async fn read_policy_guard(
    State(a): State<Shared>,
    mut request: Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, ApiError> {
    let route = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|v| v.as_str())
        .unwrap_or("");
    let guarded = (request.method() == axum::http::Method::GET
        || request.method() == axum::http::Method::HEAD)
        && ((route.starts_with("/compat/mangadex/")
            && route != "/compat/mangadex/group/{alias}/members")
            || matches!(
                route,
                "/service/announcements/{key}"
                    | "/events/{hash}"
                    | "/originals/{hash}"
                    | "/query"
                    | "/series"
                    | "/catalogue/{key}"
                    | "/social"
                    | "/libraries/{key}"
                    | "/libraries/{key}/entries"
                    | "/library/blobs/{hash}"
                    | "/accounts/{key}/metadata/{kind}"
                    | "/account-deletions/{hash}"
            ))
        || request.method() == axum::http::Method::POST && route == "/library/read";
    if !guarded {
        return Ok(next.run(request).await);
    }
    let policy = a.relay.read_policy;
    let Some(header) = request.headers().get("authorization") else {
        if policy == crate::relay::ReadPolicy::Public {
            return Ok(next.run(request).await);
        }
        return Err(forbidden("read authentication required"));
    };
    let header = header
        .to_str()
        .map_err(|_| forbidden("invalid authorization"))?;
    let authentication_phase =
        crate::read_diagnostics::phase(crate::read_diagnostics::Phase::Authentication);
    if header.len() > 16384 {
        return Err(forbidden("authorization header budget"));
    }
    let event = auth::token(header).map_err(|_| forbidden("invalid read signature"))?;
    let url = format!("{}{}", a.base_url, request.uri());
    let method = request.method().to_string();
    auth::http_scope(&event, &url, &method).map_err(|_| forbidden("invalid read scope"))?;
    if policy == crate::relay::ReadPolicy::Admitted {
        a.repo
            .account_permit(id(&event.pubkey).map_err(|_| forbidden("invalid key"))?)
            .await
            .map_err(|error| account_error(error, "reader not admitted"))?;
    }
    if method == "POST" {
        let (parts, body) = request.into_parts();
        let bytes = tokio::time::timeout(Duration::from_secs(5), axum::body::to_bytes(body, 8192))
            .await
            .map_err(|_| bad("read body timeout"))?
            .map_err(|_| bad("read body budget"))?;
        auth::http(&event, &url, &method, &bytes).map_err(|_| forbidden("read body binding"))?;
        request = Request::from_parts(parts, axum::body::Body::from(bytes));
    }
    consume_authorization(&a, &event).await?;
    request.extensions_mut().insert(ReadIdentity(
        id(&event.pubkey).map_err(|_| forbidden("invalid reader key"))?,
    ));
    drop(authentication_phase);
    Ok(next.run(request).await)
}

async fn root(
    State(a): State<Shared>,
    headers: HeaderMap,
    ws: std::result::Result<
        axum::extract::ws::WebSocketUpgrade,
        axum::extract::ws::rejection::WebSocketUpgradeRejection,
    >,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if let Ok(ws) = ws {
        let Ok(slot) = a.relay.connections.clone().try_acquire_owned() else {
            return (StatusCode::SERVICE_UNAVAILABLE, "connection budget").into_response();
        };
        let relay = a.relay.clone();
        let operations = a.operations.clone();
        let lease = match operations.acquire() {
            Ok(lease) => lease,
            Err(error) => return error.into_response(),
        };
        return ws
            .max_message_size(crate::protocol_limits::WS_MESSAGE_BYTES)
            .max_frame_size(crate::protocol_limits::WS_MESSAGE_BYTES)
            .max_write_buffer_size(2_097_152)
            .on_upgrade(move |socket| async move {
                let _lease = lease;
                if operations.enabled {
                    OWNED_OPERATIONS
                        .scope(operations, relay.session(socket, slot))
                        .await
                } else {
                    relay.session(socket, slot).await
                }
            });
    }
    if headers
        .get("accept")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|x| x.trim() == "application/nostr+json"))
    {
        return ([("content-type","application/nostr+json"),("access-control-allow-origin","*"),("access-control-allow-headers","Accept, Content-Type"),("access-control-allow-methods","GET, HEAD, OPTIONS")],Json(json!({
            "name":"manga-node","pubkey":a.admin,"self":a.signer.public,"supported_nips":[1,11,42],
            "description":"Experimental guarded manga relay; admission and verified local storage required for writes.",
            "limitation":crate::capabilities::relay_limits(a.relay.read_policy!=crate::relay::ReadPolicy::Public),
            "manga":a.capabilities
        }))).into_response();
    }
    capabilities(State(a)).await.into_response()
}
async fn peer_websocket(
    State(a): State<Shared>,
    ws: std::result::Result<
        axum::extract::ws::WebSocketUpgrade,
        axum::extract::ws::rejection::WebSocketUpgradeRejection,
    >,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if !a.projection_enabled {
        return unavailable().into_response();
    }
    let Ok(ws) = ws else {
        return (StatusCode::UPGRADE_REQUIRED,[("upgrade","websocket")],Json(json!({"error":"websocket_required","authentication":"NIP42 approved peer","read_only":true}))).into_response();
    };
    let Ok(slot) = a.peer_relay.connections.clone().try_acquire_owned() else {
        return unavailable().into_response();
    };
    let relay = a.peer_relay.clone();
    let operations = a.operations.clone();
    let lease = match operations.acquire() {
        Ok(lease) => lease,
        Err(error) => return error.into_response(),
    };
    ws.max_message_size(crate::protocol_limits::WS_MESSAGE_BYTES)
        .max_frame_size(crate::protocol_limits::WS_MESSAGE_BYTES)
        .max_write_buffer_size(2_097_152)
        .on_upgrade(move |socket| async move {
            let _lease = lease;
            if operations.enabled {
                OWNED_OPERATIONS
                    .scope(operations, relay.session(socket, slot))
                    .await
            } else {
                relay.session(socket, slot).await
            }
        })
}
async fn root_options() -> impl axum::response::IntoResponse {
    (
        StatusCode::NO_CONTENT,
        [
            ("access-control-allow-origin", "*"),
            ("access-control-allow-headers", "Accept, Content-Type"),
            ("access-control-allow-methods", "GET, HEAD, OPTIONS"),
        ],
    )
}
async fn capabilities(State(a): State<Shared>) -> Json<Value> {
    Json(
        json!({"name":"manga-node","experimental":true,"service_pubkey":a.signer.public,"endpoint":a.base_url,"database":"postgresql","cache":"moka","features":a.capabilities["features"],"manga":a.capabilities}),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityQuery {
    nonce: String,
}
async fn identity(
    State(a): State<Shared>,
    Query(query): Query<IdentityQuery>,
) -> Result<Json<auth::Event>, ApiError> {
    if !auth::valid_hash(&query.nonce) {
        return Err(bad("nonce must be 32-byte lowercase hex"));
    }
    Ok(Json(
        a.signer
            .proof(query.nonce, a.base_url.clone())
            .map_err(|_| unavailable())?,
    ))
}
fn operator_preflight(
    a: &App,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    method: &str,
) -> Result<auth::Event, ApiError> {
    let header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| forbidden("authorization required"))?;
    if header.len() > 16_384 {
        return Err(forbidden("authorization header budget"));
    }
    let event = auth::token(header).map_err(|_| forbidden("invalid authorization event"))?;
    auth::http_scope(&event, &format!("{}{}", a.base_url, uri), method)
        .map_err(|_| forbidden("invalid URL, method, signature or freshness"))?;
    if event.pubkey != a.admin {
        return Err(forbidden("operator authorization required"));
    }
    if method == "POST" && !event.one("payload").is_ok_and(auth::valid_hash) {
        return Err(forbidden("signed body hash required"));
    }
    Ok(event)
}
async fn consume_authorization(a: &App, event: &auth::Event) -> Result<(), ApiError> {
    let fresh = a
        .repo
        .consume_http_auth(
            id(&event.id).map_err(|_| forbidden("invalid authorization identifier"))?,
            event.created_at,
        )
        .await
        .map_err(|_| unavailable())?;
    if !fresh {
        return Err(forbidden("expired or replayed authorization"));
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityTarget {
    endpoint: String,
    expected_key: Option<String>,
}
async fn verify_service_identity(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let value = operator_json(&a, &uri, &headers, request).await?;
    let target: IdentityTarget = serde_json::from_value(value)
        .map_err(|_| bad("endpoint and optional expected_key required"))?;
    let _slot = a.discovery_slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"discovery_busy"})),
        )
    })?;
    let normalized = crate::endpoint_policy::canonical_origin(&target.endpoint)
        .map_err(|_| bad("invalid endpoint origin"))?;
    let remembered = a
        .repo
        .remembered_service_key(&normalized)
        .await
        .map_err(|_| unavailable())?;
    let expected = target
        .expected_key
        .as_deref()
        .map(id)
        .transpose()
        .map_err(|_| bad("invalid expected service key"))?;
    if matches!((remembered,expected),(Some(old),Some(new)) if old != new) {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({"error":"remembered_service_key_changed"})),
        ));
    }
    let verified = a
        .endpoint_policy
        .connect(&normalized, remembered.or(expected))
        .await
        .map_err(|error| {
            if error.is::<crate::service_identity::IdentityChanged>() {
                (
                    StatusCode::CONFLICT,
                    Json(json!({"error":"service_key_changed"})),
                )
            } else if error.is::<crate::endpoint_policy::DestinationDenied>() {
                forbidden("endpoint destination policy rejected")
            } else {
                unavailable()
            }
        })?;
    let first = a
        .repo
        .remember_service_identity(&verified)
        .await
        .map_err(|error| {
            if error.is::<crate::service_identity::IdentityChanged>() {
                (
                    StatusCode::CONFLICT,
                    Json(json!({"error":"remembered_service_key_changed"})),
                )
            } else if error.is::<crate::service_identity::IdentityBudget>() {
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({"error":"remembered_endpoint_budget"})),
                )
            } else {
                unavailable()
            }
        })?;
    Ok(Json(
        json!({"endpoint":verified.origin(),"service_pubkey":hex::encode(verified.service_key()),"first_observation":first,"peer_permissions_granted":false}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupAuditQuery {
    before_id: Option<i64>,
    limit: Option<i64>,
}
async fn group_action_audit(
    State(a): State<Shared>,
    Path(key): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(query): Query<GroupAuditQuery>,
) -> Result<Json<Value>, ApiError> {
    let event = operator_preflight(&a, &uri, &headers, "GET")?;
    consume_authorization(&a, &event).await?;
    let group = id(&key).map_err(|_| bad("invalid group"))?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=100).contains(&limit) || query.before_id.is_some_and(|id| id <= 0) {
        return Err(bad("invalid audit page bounds"));
    }
    let client = a.repo.pool.get().await.map_err(|_| unavailable())?;
    let rows = client.query("SELECT id,member,action,event_id,permission_version,recorded_at::text FROM group_action_audit WHERE group_key=$1 AND ($2::bigint IS NULL OR id<$2) ORDER BY id DESC LIMIT $3", &[&group.as_slice(), &query.before_id, &(limit+1)]).await.map_err(|_| unavailable())?;
    let more = rows.len() > limit as usize;
    let items: Vec<Value> = rows
        .into_iter()
        .take(limit as usize)
        .map(|row| {
            json!({
                "id":row.get::<_,i64>(0),"member":hex::encode(row.get::<_,Vec<u8>>(1)),
                "action":row.get::<_,String>(2),"event_id":hex::encode(row.get::<_,Vec<u8>>(3)),
                "permission_version":row.get::<_,i64>(4),"recorded_at":row.get::<_,String>(5)
            })
        })
        .collect();
    let next = if more {
        items.last().and_then(|v| v["id"].as_i64())
    } else {
        None
    };
    Ok(Json(
        json!({"group":key,"items":items,"next_before_id":next,"scope":"local_successful_first_commits"}),
    ))
}

async fn operator_status(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let event = operator_preflight(&a, &uri, &headers, "GET")?;
    consume_authorization(&a, &event).await?;
    let status = a.repo.pool.status();
    let oldest_due_seconds = match a.storage.as_ref() {
        Some(storage) => storage
            .oldest_cleanup_due_seconds()
            .await
            .map_err(|_| unavailable())?,
        None => None,
    };
    let mut result = json!({"service_pubkey":a.signer.public,"database":"ready","background_workers":a.workers.snapshot(),"cleanup":{"oldest_due_seconds":oldest_due_seconds},"relay_feed_ready":a.relay.feed.ready.load(std::sync::atomic::Ordering::Acquire)&&a.relay.dispatcher.is_ready(),"public_serving":a.serving_status.as_ref().map(|s|s.snapshot()),"peer_blob_reads":a.peer_blob_reads.load(std::sync::atomic::Ordering::Relaxed),"pool":{"size":status.size,"available":status.available,"waiting":status.waiting},"experimental":true});
    if let Some(diagnostics) = a.reads.diagnostic_totals() {
        result["read_admission_diagnostics"] =
            serde_json::to_value(diagnostics).map_err(|_| unavailable())?;
    }
    if let Some(holders) = a.reads.diagnostic_holders() {
        result["read_holder_diagnostics"] =
            serde_json::to_value(holders).map_err(|_| unavailable())?;
    }
    Ok(Json(result))
}
async fn operator_check(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    // Validate all body-independent authorization before receiving a byte of body.
    let event = operator_preflight(&a, &uri, &headers, "POST")?;
    if headers
        .get("content-length")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.parse::<u64>().ok())
        .is_some_and(|n| n > 1_000_000)
    {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error":"body_budget"})),
        ));
    }
    let body = tokio::time::timeout(
        Duration::from_secs(5),
        axum::body::to_bytes(request.into_body(), 1_000_000),
    )
    .await
    .map_err(|_| {
        (
            StatusCode::REQUEST_TIMEOUT,
            Json(json!({"error":"body_timeout"})),
        )
    })?
    .map_err(|_| {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error":"body_budget"})),
        )
    })?;
    auth::http(&event, &format!("{}{}", a.base_url, uri), "POST", &body)
        .map_err(|_| forbidden("body binding or freshness failed"))?;
    consume_authorization(&a, &event).await?;
    Ok(Json(json!({"ok":true,"body_sha256":auth::hash(&body)})))
}
pub async fn open_repository(config: &NodeConfig, dsn: String) -> Result<Repository> {
    config.library_concurrency.validate()?;
    config
        .read_admission
        .clone()
        .unwrap_or_else(|| crate::read_admission::Config::for_database(config.database_connections))
        .validate(config.database_connections)?;
    config.enrollment.validate()?;
    ensure!(
        config.storage_audit_interval_seconds <= 86400,
        "audit interval exceeds one day"
    );
    ensure!(
        config.storage_audit_interval_seconds == 0 || config.storage.is_some(),
        "periodic audit requires storage"
    );
    Repository::connect(DbConfig {
        dsn,
        connections: config.database_connections,
        acquisition_timeout: Duration::from_millis(config.database_wait_ms),
        cache_weight: config.cache_bytes,
        plaintext_local: config.database_plaintext_local,
    })
    .await
}
// Embedded handler work has an independent terminal lifetime: losing the HTTP
// caller does not lose the mutations/SQL/spawn_blocking children it must await.
#[derive(Default)]
struct OperationState {
    closed: bool,
    active: usize,
    infrastructure: usize,
    terminal_unconfirmed: bool,
}
struct EmbeddedOperations {
    enabled: bool,
    state: std::sync::Mutex<OperationState>,
    closing: tokio::sync::Notify,
}
struct OperationLease(Arc<EmbeddedOperations>, bool);
impl Drop for OperationLease {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        let count = if self.1 {
            &mut state.infrastructure
        } else {
            &mut state.active
        };
        *count = count.checked_sub(1).expect("owned native operation lease");
    }
}
impl EmbeddedOperations {
    fn new(enabled: bool) -> Arc<Self> {
        Arc::new(Self {
            enabled,
            state: Default::default(),
            closing: Default::default(),
        })
    }
    fn acquire(self: &Arc<Self>) -> Result<Option<OperationLease>, ApiError> {
        if !self.enabled {
            return Ok(None);
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed || state.active >= 128 {
            return Err(unavailable());
        }
        state.active += 1;
        Ok(Some(OperationLease(self.clone(), false)))
    }
    fn inherited(self: &Arc<Self>, infrastructure: bool) -> OperationLease {
        // Only an existing admitted task-local parent can reach this method.
        // Its bounded child may finish after new public admissions close.
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if infrastructure {
            state.infrastructure += 1;
        } else {
            state.active += 1;
        }
        OperationLease(self.clone(), infrastructure)
    }
    fn close(&self) {
        if self.enabled {
            self.state.lock().unwrap_or_else(|e| e.into_inner()).closed = true;
            self.closing.notify_waiters();
        }
    }
    async fn drain_kind(&self, deadline: Duration, all: bool) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        self.close();
        tokio::time::timeout(deadline,async {
            loop {
                let done={let state=self.state.lock().unwrap_or_else(|e|e.into_inner());state.active==0 && (!all || state.infrastructure==0)};
                if done {break;}
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.context("embedded original owned-operation drain unconfirmed; durable mutation outcome may require reconciliation")?;
        ensure!(
            !self
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .terminal_unconfirmed,
            "embedded original subprocess cleanup unconfirmed; durable projection requires reconciliation"
        );
        Ok(())
    }
    async fn drain(&self, deadline: Duration) -> Result<()> {
        self.drain_kind(deadline, true).await
    }
}
tokio::task_local! {static OWNED_OPERATIONS:Arc<EmbeddedOperations>;}
/// Opaque export of the existing embedded node owner to its exact injected
/// LibraryObjects. No public constructor, second tracker, store, or signing key.
#[derive(Clone)]
pub(crate) struct EmbeddedLibraryOperations {
    operations: Arc<EmbeddedOperations>,
    library: std::sync::Weak<crate::library_storage::LibraryObjects>,
    projection_enabled: bool,
    service: Id,
}
impl EmbeddedLibraryOperations {
    fn attach(
        library: &Arc<crate::library_storage::LibraryObjects>,
        operations: &Arc<EmbeddedOperations>,
        projection_enabled: bool,
        service: Id,
    ) -> Result<()> {
        ensure!(operations.enabled, "embedded library owner is disabled");
        let handle = Self {
            operations: operations.clone(),
            library: Arc::downgrade(library),
            projection_enabled,
            service,
        };
        ensure!(
            library.embedded_operations.set(handle).is_ok(),
            "library already has an embedded operation owner; restart requires a new runtime"
        );
        Ok(())
    }
    pub(crate) fn check_receipt(
        &self,
        library: &Arc<crate::library_storage::LibraryObjects>,
        service: Id,
    ) -> Result<()> {
        ensure!(
            self.service == service,
            crate::http_auth::HttpAuthError::Service
        );
        ensure!(
            self.projection_enabled
                && self
                    .library
                    .upgrade()
                    .is_some_and(|owner| Arc::ptr_eq(&owner, library)),
            "original library publication owner unavailable"
        );
        ensure!(
            !self
                .operations
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .closed,
            "original library owner is closing"
        );
        Ok(())
    }
    /// A caller disconnect drops only the outer JoinHandle. The original owner
    /// retains the whole accepted mutation and every inherited storage child
    /// through terminal completion, with its unchanged close/drain policy.
    pub(crate) async fn run<F, T>(
        &self,
        library: &Arc<crate::library_storage::LibraryObjects>,
        future: F,
    ) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>> + Send + 'static,
        T: Send + 'static,
    {
        ensure!(
            self.projection_enabled,
            "embedded original publication backend unavailable"
        );
        ensure!(
            self.library
                .upgrade()
                .is_some_and(|owner| Arc::ptr_eq(&owner, library)),
            "embedded library owner identity mismatch"
        );
        let lease = self
            .operations
            .acquire()
            .map_err(|_| {
                anyhow::anyhow!(crate::library_admission::Busy {
                    stage: "embedded_operations"
                })
            })?
            .context("embedded library owner is disabled")?;
        let operations = self.operations.clone();
        // Acquisition and spawn occur before the first suspension point.
        tokio::spawn(async move {
            let _lease = lease;
            OWNED_OPERATIONS.scope(operations, future).await
        })
        .await
        .context("embedded library mutation task terminated unexpectedly")?
    }
}
/// Capture a bounded child lease synchronously before its executor fork. No
/// task handles are retained by the owner, so detached caller handles cannot
/// form ownership cycles. The child guard drops only with the actual future.
pub(crate) fn owned_future_scope<F>(
    future: F,
) -> impl std::future::Future<Output = F::Output> + Send
where
    F: std::future::Future + Send,
    F::Output: Send,
{
    let inherited = OWNED_OPERATIONS
        .try_with(|operations| (operations.clone(), operations.inherited(false)))
        .ok();
    async move {
        if let Some((operations, lease)) = inherited {
            let _lease = lease;
            OWNED_OPERATIONS.scope(operations, future).await
        } else {
            future.await
        }
    }
}
pub(crate) fn spawn_owned_scope<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::spawn(owned_future_scope(future))
}
pub(crate) fn spawn_blocking_owned_scope<F, T>(operation: F) -> tokio::task::JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let lease = OWNED_OPERATIONS
        .try_with(|operations| operations.inherited(false))
        .ok();
    tokio::task::spawn_blocking(move || {
        let _lease = lease;
        operation()
    })
}
pub(crate) fn spawn_infrastructure_owned_scope<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    let inherited = OWNED_OPERATIONS
        .try_with(|operations| (operations.clone(), operations.inherited(true)))
        .ok();
    tokio::spawn(async move {
        if let Some((operations, lease)) = inherited {
            let _lease = lease;
            OWNED_OPERATIONS.scope(operations, future).await
        } else {
            future.await
        }
    })
}
pub(crate) fn owned_context_active() -> bool {
    OWNED_OPERATIONS
        .try_with(|operations| operations.enabled)
        .unwrap_or(false)
}
/// A failed process-group terminal observation must remain a shutdown failure
/// even if its Rust task has returned and dropped its ordinary operation lease.
pub(crate) fn owned_terminal_unconfirmed() {
    let _ = OWNED_OPERATIONS.try_with(|operations| {
        if operations.enabled {
            operations
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .terminal_unconfirmed = true;
            operations.close();
        }
    });
}
pub(crate) struct AbortChildOnDrop(pub(crate) tokio::task::AbortHandle);
impl Drop for AbortChildOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
/// Relay observes shutdown only between commands; an already admitted signed
/// mutation and its scoped children finish before the session closes.
pub(crate) async fn embedded_shutdown() {
    let operations = OWNED_OPERATIONS.try_with(Clone::clone).ok();
    let Some(operations) = operations.filter(|o| o.enabled) else {
        std::future::pending::<()>().await;
        return;
    };
    loop {
        let notified = operations.closing.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if operations
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .closed
        {
            return;
        }
        notified.await;
    }
}
async fn owned_operation_guard(
    State(a): State<Shared>,
    request: Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, ApiError> {
    let operations = a.operations.clone();
    let Some(lease) = operations.acquire()? else {
        return Ok(next.run(request).await);
    };
    let read = matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    ) || request.method() == axum::http::Method::POST
        && matches!(request.uri().path(), "/library/read" | "/reconcile");
    if read {
        // The original outer read-admission worker already owns cancellation and
        // its RESPONSE task-local deadline. Preserve both while retaining ours.
        let _lease = lease;
        Ok(OWNED_OPERATIONS.scope(operations, next.run(request)).await)
    } else {
        // Retain the whole mutation future (including every awaited nested task),
        // rather than tracking only a subset of today's internal spawn sites.
        tokio::spawn(async move {
            let _lease = lease;
            OWNED_OPERATIONS.scope(operations, next.run(request)).await
        })
        .await
        .map_err(|_| unavailable())
    }
}

// Only the injected path owns cancellation of its original background tasks.
// Dropping its listener future cannot leave detached mutation workers running.
struct EmbeddedWorkerOwner {
    operations: Arc<EmbeddedOperations>,
    registry: Arc<crate::background_workers::BackgroundWorkers>,
    handles: std::sync::Mutex<Vec<(&'static str, tokio::task::AbortHandle)>>,
    cancel_on_drop: bool,
}
impl EmbeddedWorkerOwner {
    fn new(cancel_on_drop: bool) -> Self {
        Self {
            operations: EmbeddedOperations::new(cancel_on_drop),
            registry: Arc::new(Default::default()),
            handles: Default::default(),
            cancel_on_drop,
        }
    }
    fn spawn(
        &self,
        name: &'static str,
        future: impl std::future::Future<Output = ()> + Send + 'static,
    ) -> tokio::task::AbortHandle {
        let operations = self.operations.clone();
        let handle = self.registry.spawn(name, async move {
            if operations.enabled {
                OWNED_OPERATIONS.scope(operations, future).await
            } else {
                future.await
            }
        });
        self.handles
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((name, handle.clone()));
        handle
    }
    async fn stop_workers(&self, all: bool) -> Result<()> {
        if !self.cancel_on_drop {
            return Ok(());
        }
        let handles = self
            .handles
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|(name, _)| {
                all || !matches!(
                    *name,
                    "backend_projection" | "relay_notifications" | "relay_dispatch"
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        for (_, handle) in &handles {
            handle.abort();
        }
        // The original registry's observers await the actual JoinHandles. Wait
        // for both task termination and those terminal observations; an abort
        // request alone is never reported as completed cleanup.
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let snapshot = self.registry.snapshot();
                if handles.iter().all(|(_, handle)| handle.is_finished())
                    && snapshot["workers"].as_array().is_some_and(|workers| {
                        workers
                            .iter()
                            .filter(|worker| {
                                handles.iter().any(|(name, _)| worker["name"] == *name)
                            })
                            .all(|worker| worker["state"] != "running")
                    })
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .context("embedded original worker termination unconfirmed")?;
        Ok(())
    }
    async fn shutdown(&self) -> Result<()> {
        self.stop_workers(true).await
    }
    async fn finish(&self, serving: Option<&mut crate::serving_role::ServingRole>) -> Result<()> {
        self.operations.close();
        // Stop admitting maintenance quanta, but keep projection/feed services
        // available while finite accepted mutations and storage children settle.
        let maintenance = self.stop_workers(false).await;
        let finite = self
            .operations
            .drain_kind(Duration::from_secs(5), false)
            .await;
        // Stop cooperative serving before the infrastructure drain. Its actual
        // loop retains an infrastructure lease, and any timed-out/cancelled
        // storage descendant retains a separate finite terminal lease.
        let serving = if let Some(role) = serving {
            role.shutdown().await
        } else {
            Ok(())
        };
        let infrastructure = self.shutdown().await;
        let terminal = self.operations.drain(Duration::from_secs(5)).await;
        maintenance?;
        finite?;
        serving?;
        infrastructure?;
        terminal
    }
    async fn unexpected_exit(&self) {
        loop {
            if self.registry.snapshot()["unexpected_exit"] == true
                || self
                    .operations
                    .state
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .terminal_unconfirmed
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
impl Drop for EmbeddedWorkerOwner {
    fn drop(&mut self) {
        if self.cancel_on_drop {
            for (_, handle) in self.handles.get_mut().unwrap_or_else(|e| e.into_inner()) {
                handle.abort();
            }
        }
    }
}

pub async fn serve(config: NodeConfig, repo: Repository) -> Result<()> {
    let serving = crate::serving_role::ServingRole::prepare(&config).await?;
    serve_prepared(config, repo, serving).await
}
pub async fn serve_prepared(
    config: NodeConfig,
    repo: Repository,
    serving: Option<crate::serving_role::ServingRole>,
) -> Result<()> {
    serve_prepared_with_store(config, repo, serving, None, None, None).await
}

/// Embedded instances inject their existing storage owner and supervise the
/// ready/shutdown channels. Ordinary node startup keeps its original behavior.
pub async fn serve_prepared_with_store(
    config: NodeConfig,
    repo: Repository,
    serving: Option<crate::serving_role::ServingRole>,
    injected_storage: Option<Arc<crate::upload::UploadStore>>,
    ready: Option<tokio::sync::oneshot::Sender<()>>,
    shutdown_receiver: Option<tokio::sync::oneshot::Receiver<()>>,
) -> Result<()> {
    serve_prepared_with_shared_library(
        config,
        repo,
        serving,
        injected_storage,
        None,
        ready,
        shutdown_receiver,
    )
    .await
}

/// One exact original LibraryObjects/cache/admission/UploadStore owner shared by
/// the embedded native listener and trusted in-process compatibility adapters.
pub async fn serve_prepared_with_shared_library(
    config: NodeConfig,
    repo: Repository,
    serving: Option<crate::serving_role::ServingRole>,
    injected_storage: Option<Arc<crate::upload::UploadStore>>,
    injected_library: Option<Arc<crate::library_storage::LibraryObjects>>,
    ready: Option<tokio::sync::oneshot::Sender<()>>,
    shutdown_receiver: Option<tokio::sync::oneshot::Receiver<()>>,
) -> Result<()> {
    let worker_owner = EmbeddedWorkerOwner::new(injected_storage.is_some());
    let operation = serve_prepared_inner(
        config,
        repo,
        serving,
        injected_storage,
        injected_library,
        ready,
        shutdown_receiver,
        &worker_owner,
    );
    let result = if worker_owner.cancel_on_drop {
        OWNED_OPERATIONS
            .scope(worker_owner.operations.clone(), operation)
            .await
    } else {
        operation.await
    };
    worker_owner.operations.close();
    // Mutation infrastructure stays alive until admitted durable work terminates.
    let cleanup = worker_owner.finish(None).await;
    result?;
    cleanup
}
async fn serve_prepared_inner(
    config: NodeConfig,
    repo: Repository,
    mut serving: Option<crate::serving_role::ServingRole>,
    injected_storage: Option<Arc<crate::upload::UploadStore>>,
    injected_library: Option<Arc<crate::library_storage::LibraryObjects>>,
    ready: Option<tokio::sync::oneshot::Sender<()>>,
    shutdown_receiver: Option<tokio::sync::oneshot::Receiver<()>>,
    worker_owner: &EmbeddedWorkerOwner,
) -> Result<()> {
    let reads = Arc::new(crate::read_admission::Admission::new(
        config.read_admission.clone().unwrap_or_else(|| {
            crate::read_admission::Config::for_database(config.database_connections)
        }),
        config.database_connections,
    )?);
    ensure!(
        config.bind.ip().is_loopback(),
        "node binds loopback; use an explicitly configured reverse proxy"
    );
    ensure!(config.cors_origins.len() <= 64, "CORS origin budget");
    for origin in &config.cors_origins {
        ensure!(
            origin == "*" || normalize_origin(origin).is_ok_and(|normalized| normalized == *origin),
            "invalid CORS origin"
        );
    }
    let endpoint_policy =
        crate::endpoint_policy::EndpointPolicy::new(&config.discovery_private_overrides)?;
    let signer = Signer::load(&config.service_key_file)?;
    let service = id(&signer.public)?;
    let admin = id(&config.admin_pubkey)?;
    let base_url = normalize_origin(&config.base_url)?;
    repo.bind_instance(service, admin).await?;
    if repo.service_announcement(service).await?.is_none() {
        repo.publish_service_endpoints(&signer.secret, vec![base_url.clone()])
            .await?;
    }
    let workers = worker_owner.registry.clone();
    let storage = if let Some(store) = injected_storage {
        ensure!(
            store.owner() == admin,
            "injected original storage owner differs from node administrator"
        );
        Some(store)
    } else {
        match config.storage {
            Some(cfg) => {
                Some(crate::upload::UploadStore::open(repo.clone(), cfg, admin, service).await?)
            }
            None => None,
        }
    };
    if let Some(store) = storage.clone() {
        let _ = worker_owner.spawn("staged_upload_recovery", async move {
            let mut cursor = None;
            loop {
                match store.recover_page(cursor).await {
                    Ok(Some(last)) => {
                        cursor = Some(last);
                        tokio::task::yield_now().await;
                    }
                    Ok(None) => {
                        cursor = None;
                        tokio::time::sleep(Duration::from_secs(30)).await;
                    }
                    Err(error) => {
                        tracing::warn!(error=%error,"staged upload recovery deferred; charges retained");
                        tokio::time::sleep(Duration::from_secs(30)).await;
                    }
                }
            }
        });
    }
    if let Some(store) = storage.clone() {
        let enabled = config.library_reclamation;
        let publications_enabled = config.publication_reclamation;
        let staging_enabled = config.staging_reclamation;
        let _ = worker_owner.spawn("blob_reclamation", async move {
            loop {
                match store
                    .reclaim_objects_page(enabled, publications_enabled, staging_enabled)
                    .await
                {
                    Ok(0) => tokio::time::sleep(Duration::from_millis(250)).await,
                    Ok(_) => tokio::task::yield_now().await,
                    Err(error) => {
                        tracing::warn!(error=%error,"library reclamation deferred; charges retained");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });
    }
    if config.library_reclamation {
        let history = repo.clone();
        let _ = worker_owner.spawn("library_history_pruning", async move {
            loop {
                match history.prune_library_history().await {
                    Ok(batch) if batch.more => tokio::task::yield_now().await,
                    Ok(_) => tokio::time::sleep(Duration::from_millis(250)).await,
                    Err(error) => {
                        tracing::warn!(error=%error,"obsolete library history pruning deferred");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });
    }
    if config.storage_audit_interval_seconds > 0 {
        let store = storage.clone().context("periodic audit requires storage")?;
        let interval = Duration::from_secs(config.storage_audit_interval_seconds);
        let _ = worker_owner.spawn("storage_audit", async move {
            loop {
                if let Err(error) = store.audit_page(32).await {
                    tracing::warn!(error=%error,"storage audit deferred");
                }
                tokio::time::sleep(interval).await;
            }
        });
    }
    let projection_enabled = config.backend.is_some();
    if let Some(cfg) = config.backend {
        let backend = crate::backend::Backend::open(repo.clone(), cfg).await?;
        let _ = worker_owner.spawn("backend_projection", backend.run());
    }
    let publications = if projection_enabled {
        storage
            .clone()
            .map(|s| crate::publication::Publications::new(repo.clone(), s))
    } else {
        None
    };
    let reviews = match (
        storage.clone(),
        publications.clone(),
        config.submission_review.clone(),
    ) {
        (Some(storage), Some(publications), Some(limits)) => Some(Arc::new(
            crate::submission_review::Reviews::new(repo.clone(), storage, publications, limits)?,
        )),
        _ => None,
    };
    if reviews.is_some() {
        let review_repo = repo.clone();
        let _ = worker_owner.spawn("review_expiry", async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                if let Err(error) = review_repo.expire_reviews().await {
                    tracing::warn!(error=%error,"review expiry deferred");
                }
            }
        });
    }
    let peer_sync = match (storage.clone(), publications.clone()) {
        (Some(storage), Some(publications)) => Some(crate::peers::PeerSync::with_endpoint_policy(
            repo.clone(),
            storage,
            publications,
            signer.secret.clone(),
            endpoint_policy.clone(),
        )?),
        _ => None,
    };
    let gc = repo.clone();
    let _ = worker_owner.spawn("authorization_expiry", async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            let start = tokio::time::Instant::now();
            for _ in 0..32 {
                match gc.purge_expired_auth().await {
                    Ok(n) if n == 1000 && start.elapsed() < Duration::from_millis(100) => {}
                    Ok(_) => break,
                    Err(_) => {
                        tracing::warn!("authorization expiry cleanup unavailable");
                        break;
                    }
                }
            }
        }
    });
    let groups = if let Some(path) = config.group_keys_dir {
        Some(crate::groups::Groups::open(
            repo.clone(),
            path,
            storage
                .clone()
                .context("managed groups require storage role")?,
            publications
                .clone()
                .context("managed groups require publication role")?,
        )?)
    } else {
        None
    };
    let snapshots = crate::inventory::Snapshots::new(repo.clone());
    let library_objects = if let Some(library) = injected_library {
        let store = storage
            .as_ref()
            .context("injected library requires its original storage owner")?;
        ensure!(
            library.shares_storage(store),
            "injected library/store ownership mismatch"
        );
        Some(library)
    } else {
        storage
            .clone()
            .map(|store| {
                crate::library_storage::LibraryObjects::configured(
                    repo.clone(),
                    store,
                    config.library_concurrency.clone(),
                )
            })
            .transpose()?
    };
    if worker_owner.cancel_on_drop
        && let Some(library) = &library_objects
    {
        EmbeddedLibraryOperations::attach(
            library,
            &worker_owner.operations,
            projection_enabled,
            service,
        )?;
    }
    if config.sync_worker_enabled
        && let Some(sync) = peer_sync.clone()
    {
        let _ = worker_owner.spawn("sync_schedules", crate::sync_schedules::run(repo.clone()));
        let _ = worker_owner.spawn(
            "sync_jobs",
            crate::sync_jobs::SyncRunner::new(repo.clone(), sync, library_objects.clone()).run(),
        );
    }
    if config.sync_worker_enabled && config.peer_discovery_enabled {
        let _ = worker_owner.spawn(
            "peer_discovery",
            crate::peer_discovery::run(repo.clone(), endpoint_policy.clone()),
        );
    }
    if let Some(store) = storage.clone() {
        let _ = worker_owner.spawn(
            "catalogue_index",
            crate::catalogue::run(repo.clone(), store),
        );
    }
    {
        let repo = repo.clone();
        let _ = worker_owner.spawn("compatibility_index", async move {
            loop {
                match repo.compatibility_backfill().await {
                    Ok(64) => tokio::time::sleep(Duration::from_millis(100)).await,
                    Ok(_) => tokio::time::sleep(Duration::from_secs(10)).await,
                    Err(error) => {
                        tracing::warn!(error=%error,"compatibility indexing deferred");
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        });
    }
    let event_slots = Arc::new(tokio::sync::Semaphore::new(16));

    let mut relay_url = url::Url::parse(&base_url)?;
    relay_url
        .set_scheme(if base_url.starts_with("https:") {
            "wss"
        } else {
            "ws"
        })
        .map_err(|_| anyhow::anyhow!("relay URL"))?;
    let feed = crate::relay_feed::Feed::new();
    let _ = worker_owner.spawn("relay_notifications", feed.clone().run(repo.clone()));
    let dispatcher = Arc::new(crate::relay_dispatch::Dispatcher::default());
    let _ = worker_owner.spawn(
        "relay_dispatch",
        dispatcher.clone().run(
            feed.sender.subscribe(),
            crate::relay_dispatch::RepositorySource::new(repo.clone(), reads.clone()),
        ),
    );
    let relay = Arc::new(crate::relay::Relay {
        peer_only: false,
        read_policy: config.read_policy,
        repo: repo.clone(),
        feed,
        dispatcher,
        url: relay_url.to_string(),
        connections: Arc::new(tokio::sync::Semaphore::new(32)),
        writes: event_slots.clone(),
        reads: reads.clone(),
        publications: publications.clone(),
        libraries: library_objects.clone(),
        projection_enabled,
    });
    let mut peer_relay_url = relay_url.clone();
    peer_relay_url.set_path("/federation/relay");
    let peer_relay = Arc::new(crate::relay::Relay {
        peer_only: true,
        url: peer_relay_url.to_string(),
        connections: Arc::new(tokio::sync::Semaphore::new(8)),
        ..(*relay).clone()
    });
    let app = Router::new()
        .route("/", get(root).options(root_options))
        .route("/federation/relay", get(peer_websocket))
        .route("/identity", get(identity))
        .route(
            "/compat/mangadex/nostr/publications",
            post(compatibility_http::publication),
        )
        .route(
            "/compat/mangadex/group",
            get(compatibility_http::groups).post(compatibility_http::group_create),
        )
        .route(
            "/compat/mangadex/group/{alias}/members",
            get(compatibility_http::group_members_get).post(compatibility_http::group_members_set),
        )
        .route(
            "/compat/mangadex/group/{alias}/publish",
            post(compatibility_http::group_publication),
        )
        .route(
            "/compat/mangadex/group/{alias}/upload/{hash}",
            post(compatibility_http::group_blob),
        )
        .route(
            "/compat/mangadex/group/{alias}/deletion-requests",
            post(compatibility_http::group_deletion),
        )
        .route("/compat/mangadex/cover", get(compatibility_http::covers))
        .route("/compat/mangadex/author", get(compatibility_http::authors))
        .route(
            "/compat/mangadex/author/{key}",
            get(compatibility_http::author_one),
        )
        .route(
            "/compat/mangadex/group/{alias}",
            get(compatibility_http::group_one),
        )
        .route(
            "/compat/mangadex/manga/{key}/aggregate",
            get(compatibility_http::aggregate),
        )
        .route("/compat/mangadex", get(compatibility_http::capability))
        .route(
            "/enrollment",
            get(enrollment_http::policy).post(enrollment_http::enroll),
        )
        .route(
            "/compat/mangadex/nostr/enrollment",
            post(enrollment_http::enroll),
        )
        .route("/groups/{key}/profile", post(set_group_profile))
        .route(
            "/compat/mangadex/group/{alias}/profile",
            post(compatibility_http::group_profile),
        )
        .route("/compat/mangadex/user/me", get(reader_http::me))
        .route("/compat/mangadex/user/list", get(reader_http::my_lists))
        .route("/compat/mangadex/user/{key}", get(reader_http::user))
        .route(
            "/compat/mangadex/user/{key}/list",
            get(reader_http::user_lists),
        )
        .route(
            "/compat/mangadex/user/{key}/library",
            get(reader_http::entries),
        )
        .route("/compat/mangadex/list/{key}", get(reader_http::list))
        .route(
            "/compat/mangadex/list/{key}/members",
            get(reader_http::members),
        )
        .route("/compat/mangadex/manga/status", get(reader_http::statuses))
        .route(
            "/compat/mangadex/manga/read",
            get(reader_http::read_markers_many),
        )
        .route(
            "/compat/mangadex/manga/{key}/status",
            get(reader_http::status),
        )
        .route(
            "/compat/mangadex/manga/{key}/read",
            get(reader_http::read_markers),
        )
        .route(
            "/compat/mangadex/nostr/account-metadata",
            post(submit_account_metadata),
        )
        .route(
            "/compat/mangadex/nostr/account-deletions",
            post(delete_account_events),
        )
        .route(
            "/compat/mangadex/nostr/library/batches",
            post(upload_library_batch),
        )
        .route("/compat/mangadex/nostr/libraries", post(commit_library))
        .route("/compat/mangadex/manga", get(compatibility_http::manga))
        .route(
            "/compat/mangadex/chapter",
            get(compatibility_http::chapters),
        )
        .route("/compat/mangadex/manga/tag", get(compatibility_http::tags))
        .route(
            "/compat/mangadex/manga/{key}",
            get(compatibility_http::manga_one).put(compatibility_http::unsupported),
        )
        .route(
            "/compat/mangadex/chapter/{key}",
            get(compatibility_http::chapter_one).put(compatibility_http::unsupported),
        )
        .route(
            "/compat/mangadex/manga/{key}/feed",
            get(compatibility_http::feed),
        )
        .route(
            "/compat/mangadex/at-home/server/{key}",
            get(compatibility_http::at_home),
        )
        .route(
            "/compat/mangadex/data/{event}/{file}",
            get(compatibility_http::image),
        )
        .route(
            "/compat/mangadex/data-saver/{event}/{file}",
            get(compatibility_http::image),
        )
        .route(
            "/compat/mangadex/cover/{key}",
            get(compatibility_http::cover),
        )
        .route(
            "/compat/mangadex/covers/{key}/{file}",
            get(compatibility_http::cover_image),
        )
        .route(
            "/admin/compatibility/aliases",
            post(compatibility_http::aliases),
        )
        .route("/series", get(catalogue_http::series))
        .route("/catalogue/{key}", get(catalogue_http::entry))
        .route("/service/announcement", get(service_discovery::own))
        .route(
            "/service/announcements/{key}",
            get(service_discovery::cached),
        )
        .route(
            "/federation/service-announcements/{key}",
            get(service_discovery::peer_cached),
        )
        .route(
            "/admin/service-announcement",
            post(service_discovery::publish),
        )
        .route(
            "/admin/service-announcements",
            post(service_discovery::cache),
        )
        .route(
            "/admin/service-announcements/fetch",
            post(service_discovery::fetch),
        )
        .route("/admin/groups/{key}/audit", get(group_action_audit))
        .route("/admin/status", get(operator_status))
        .route("/admin/catalogue/errors", get(catalogue_http::errors))
        .route("/admin/catalogue/reindex", post(catalogue_http::reindex))
        .route("/admin/identities/verify", post(verify_service_identity))
        .route(
            "/admin/identities/discover",
            post(service_discovery::discover),
        )
        .route("/admin/check", post(operator_check))
        .route("/admin/storage/audit", post(audit_storage))
        .route("/admin/storage/repair", post(repair_storage))
        .route("/admin/accounts/{key}", post(configure_account))
        .route("/admin/publishers/{key}", post(configure_publisher))
        .route("/account-metadata", post(submit_account_metadata))
        .route("/social", post(submit_social).get(query_social))
        .route("/account-deletions", post(delete_account_events))
        .route("/account-deletions/{hash}", get(read_account_deletion))
        .route("/libraries", post(commit_library))
        .route("/libraries/{key}/entries", get(read_library_entries))
        .route("/library/batches", post(upload_library_batch))
        .route("/library/read", post(read_library_batch))
        .route("/libraries/{key}", get(read_library_head))
        .route("/library/blobs/{hash}", get(read_library_object))
        .route("/library/objects/{kind}", post(stage_library_object))
        .route("/library/staging/{hash}", get(read_library_staging))
        .route("/accounts/{key}/metadata/{kind}", get(account_head))
        .route("/upload", put(blossom_http::upload))
        .route(
            "/{blob}",
            get(blossom_http::read).delete(blossom_http::delete),
        )
        .route("/upload/{hash}", put(upload_blob))
        .route("/blobs/{hash}", get(read_blob).delete(delete_blob))
        .route("/events", post(submit_event))
        .route("/events/{hash}", get(read_event))
        .route("/publications", post(publish_event))
        .route(
            "/submissions",
            post(review_http::declare).get(review_http::list_own),
        )
        .route("/submissions/{submission}", get(review_http::own))
        .route(
            "/submissions/{submission}/blobs/{hash}",
            post(review_http::upload).get(review_http::read_blob),
        )
        .route(
            "/submissions/{submission}/withdraw",
            post(review_http::withdraw),
        )
        .route("/admin/submissions", get(review_http::list_admin))
        .route("/admin/submissions/{submission}", get(review_http::admin))
        .route(
            "/admin/submissions/{submission}/decision",
            post(review_http::decide),
        )
        .route("/originals/{hash}", get(public_original))
        .route("/query", get(query_chapters))
        .route(
            "/compat/mangadex/nostr/deletion-requests",
            post(deletion_request),
        )
        .route("/deletion-requests", post(deletion_request))
        .route("/admin/deletions/{hash}/approve", post(approve_deletion))
        .route(
            "/admin/publications/{hash}/restore",
            post(restore_publication),
        )
        .route("/admin/peers", post(configure_peer))
        .route("/admin/sync", post(sync_peer_chapter))
        .route("/admin/sync-library", post(sync_peer_library))
        .route("/admin/sync-account", post(sync_peer_account))
        .route("/federation/account-events/{hash}", get(peer_account_event))
        .route("/federation/libraries/{key}", get(peer_library_head))
        .route(
            "/federation/library-routing/{hash}",
            get(peer_library_routing),
        )
        .route("/federation/library/blobs/{hash}", get(peer_library_blob))
        .route("/admin/sync-jobs", post(enqueue_sync))
        .route(
            "/admin/sync-schedules",
            post(save_sync_schedule).get(list_sync_schedules),
        )
        .route(
            "/admin/sync-schedules/{hash}/enabled",
            post(enable_sync_schedule),
        )
        .route("/admin/sync-jobs/{hash}", get(sync_status))
        .route("/admin/sync-jobs/{hash}/cancel", post(cancel_sync))
        .route("/federation/events/{hash}", get(peer_event))
        .route("/federation/blobs/{hash}", get(peer_blob))
        .route("/reconcile", post(reconcile_inventory))
        .route("/admin/groups", post(create_group))
        .route("/admin/groups/import", post(import_group))
        .route("/admin/groups/{key}/bootstrap", post(bootstrap_group))
        .route("/admin/groups/{key}/enabled", post(enable_group))
        .route(
            "/groups/{key}/members",
            post(set_group_member).get(list_group_members),
        )
        .route("/groups/{key}/upload/{hash}", post(group_upload))
        .route(
            "/groups/{key}/delete-upload/{hash}",
            post(group_delete_upload),
        )
        .route("/groups/{key}/publish", post(group_publish))
        .route(
            "/groups/{key}/deletion-requests",
            post(group_deletion_request),
        );
    let features = json!({"identity":true,"remembered_service_identity":true,"automatic_endpoint_discovery":true,"automatic_peer_relocation":config.peer_discovery_enabled&&config.sync_worker_enabled,"signed_endpoint_announcements":true,"signed_federation_metadata":true,"signed_authenticated_metadata":true,"operator_http_auth":true,"upload":storage.is_some(),"storage_audit":storage.is_some(),"verified_peer_repair":peer_sync.is_some(),"regular_event_submission":projection_enabled,"publication":publications.is_some(),"replication":peer_sync.is_some(),"inventory_reconciliation":true,"bulk_reconciliation":peer_sync.is_some(),"managed_groups":groups.is_some(),"submission_review":reviews.is_some(),"public_libraries":library_objects.is_some()&&projection_enabled,"public_social":projection_enabled,"ordinary_author_deletion":projection_enabled,"approved_library_replication":library_objects.is_some()&&peer_sync.is_some()});
    let mut capabilities = crate::capabilities::Profile {
        base_url: &base_url,
        service_key: &signer.public,
        formats: storage
            .as_ref()
            .map(|s| s.formats.as_slice())
            .unwrap_or(&[]),
        max_blob_bytes: storage.as_ref().map(|s| s.max_blob_bytes),
        concurrency: &config.library_concurrency,
        features,
        staging_reclamation: config.staging_reclamation && storage.is_some(),
        read_policy: config.read_policy,
        cors_origins: &config.cors_origins,
        review_limits: if reviews.is_some() {
            config.submission_review.as_ref()
        } else {
            None
        },
    }
    .document();
    capabilities["peer_websocket"] = json!({"enabled":projection_enabled,"url":peer_relay.url,"path":"/federation/relay","authentication":"NIP42 plus current approved-peer allowlist","read_only":true,"connections":8,"authentication_timeout_seconds":15,"manifest_reconciliation":"HTTP /reconcile; not NIP77"});
    capabilities["enrollment"] = enrollment_http::description(&config.enrollment);
    let state = Arc::new(App {
        enrollment: config.enrollment.clone(),
        endpoint_policy,
        discovery_slots: Arc::new(tokio::sync::Semaphore::new(4)),
        workers,
        operations: worker_owner.operations.clone(),
        capabilities,
        cors_origins: config.cors_origins,
        serving_status: serving.as_ref().map(|s| s.status()),
        proof_slots: Arc::new(tokio::sync::Semaphore::new(4)),
        client_proof_slots: Arc::new(tokio::sync::Semaphore::new(32)),
        relay,
        peer_relay,
        repo: repo.clone(),
        signer,
        base_url,
        admin: config.admin_pubkey,
        storage: storage.clone(),
        projection_enabled,
        publications,
        reviews,
        event_slots,
        reads,
        auto_approve_publication_deletions: config.auto_approve_publication_deletions,
        peer_sync,
        peer_blob_reads: std::sync::atomic::AtomicU64::new(0),
        snapshots,
        groups,
        library_objects,
    });
    let app = app
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            read_policy_guard,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            read_admission::guard,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            owned_operation_guard,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            federation_response_proof,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            blossom_http::cors,
        ))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    if let Some(role) = serving.as_mut() {
        role.run(
            storage.clone().context("serving storage missing")?,
            repo.clone(),
        )
        .await?;
    }
    tracing::info!(address=%config.bind,"node HTTP listener ready");
    if let Some(ready) = ready {
        let _ = ready.send(());
    }
    if worker_owner.cancel_on_drop {
        let failure_registry = worker_owner.registry.clone();
        let closing_operations = worker_owner.operations.clone();
        let server=axum::serve(listener,app).with_graceful_shutdown(async move {
            let stop=async move { if let Some(receiver)=shutdown_receiver {let _=receiver.await;} else {shutdown().await;} };
            tokio::select! {
                _=stop=>{},
                _=async { loop {if failure_registry.snapshot()["unexpected_exit"]==true || closing_operations.state.lock().unwrap_or_else(|e|e.into_inner()).terminal_unconfirmed {break;} tokio::time::sleep(Duration::from_millis(100)).await;} }=>{},
            }
            closing_operations.close();
        });
        let server = std::future::IntoFuture::into_future(server);
        tokio::pin!(server);
        let result = tokio::select! {
            result=&mut server=>result.map_err(anyhow::Error::from),
            _=worker_owner.unexpected_exit()=>{
                // Withdraw the required listener after a bounded graceful drain;
                // timeout reports uncertainty rather than claiming completion.
                let drained=tokio::time::timeout(Duration::from_secs(5),&mut server).await;
                if drained.is_err() {tracing::error!("embedded original HTTP drain unconfirmed after worker failure");}
                Err(anyhow::anyhow!("required embedded original worker exited unexpectedly"))
            },
        };
        worker_owner.operations.close();
        let operation_drain = worker_owner.finish(serving.as_mut()).await;
        result?;
        operation_drain?;
    } else {
        // The standalone node retains its original shutdown behavior.
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                if let Some(receiver) = shutdown_receiver {
                    let _ = receiver.await;
                } else {
                    shutdown().await;
                }
                if let Some(role) = serving.as_mut()
                    && let Err(error) = role.shutdown().await
                {
                    tracing::error!(error=%error,"serving shutdown failed");
                }
            })
            .await?;
    }
    Ok(())
}
async fn shutdown() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("signal handler");
        tokio::select! {_ = tokio::signal::ctrl_c()=>{},_ = term.recv()=>{}}
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

async fn blob_actor(
    a: &App,
    headers: &HeaderMap,
    action: &str,
    hash: &str,
) -> Result<crate::publisher_authority::PublisherPermit, ApiError> {
    let header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| forbidden("authorization required"))?;
    if header.len() > 16384 {
        return Err(forbidden("authorization header budget"));
    }
    let event = auth::token(header).map_err(|_| forbidden("invalid authorization event"))?;
    auth::blossom(&event, &a.base_url, action, hash)
        .map_err(|_| forbidden("blob authorization scope"))?;
    if !a
        .repo
        .authorization_clock_allowed(event.created_at)
        .await
        .map_err(|_| unavailable())?
    {
        return Err(forbidden("authorization predates restoration"));
    }
    a.repo
        .publisher_permit(id(&event.pubkey).map_err(|_| forbidden("invalid actor"))?)
        .await
        .map_err(publisher_error)
}
async fn upload_blob(
    State(a): State<Shared>,
    Path(hash): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let hash_id = id(&hash).map_err(|_| bad("invalid hash"))?;
    let authority = blob_actor(&a, &headers, "upload", &hash).await?;
    let storage = a.storage.clone().ok_or_else(unavailable)?;
    let permit = storage.admit_original_upload().await.map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"upload_slots_busy"})),
        )
    })?;
    if headers
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .is_some_and(|n| n > storage.max_blob_bytes)
    {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error":"body_budget"})),
        ));
    }
    let body = tokio::time::timeout(
        Duration::from_secs(30),
        axum::body::to_bytes(request.into_body(), storage.max_blob_bytes as usize),
    )
    .await
    .map_err(|_| {
        (
            StatusCode::REQUEST_TIMEOUT,
            Json(json!({"error":"body_timeout"})),
        )
    })?
    .map_err(|_| {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error":"body_budget"})),
        )
    })?;
    blob_actor(&a, &headers, "upload", &hash).await?;
    if body.is_empty() || auth::hash(&body) != hash {
        return Err(bad("body hash or size mismatch"));
    }
    // Owned task retains the permit and hash guard even if the HTTP client leaves.
    let intent = spawn_owned_scope(async move {
        let _permit = permit;
        storage.put_for_publisher(&authority, hash_id, &body).await
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(publisher_error)?;
    Ok(Json(
        json!({"sha256":hash,"size":intent.size,"url":format!("{}/blobs/{}",a.base_url,hash),"staged":true}),
    ))
}
async fn read_blob(
    State(a): State<Shared>,
    Path(hash): Path<String>,
    headers: HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    let hash_id = id(&hash).map_err(|_| bad("invalid hash"))?;
    let actor = blob_actor(&a, &headers, "get", &hash).await?;
    let storage = a.storage.clone().ok_or_else(unavailable)?;
    let snapshot = storage
        .download_for_publisher(&actor, hash_id)
        .await
        .map_err(publisher_read_error)?;
    Ok(snapshot.response(None, "application/octet-stream", true))
}

fn staged_delete_error(error: anyhow::Error) -> ApiError {
    use crate::staged_deletion::DeleteDenied;
    if error.is::<crate::publisher_authority::PublisherDenied>() {
        return publisher_error(error);
    }
    match error.downcast_ref::<DeleteDenied>() {
        Some(DeleteDenied::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"staged_blob_not_found","deleted":false})),
        ),
        Some(DeleteDenied::Unauthorized) => forbidden("current upload grant required"),
        Some(DeleteDenied::Protected) => (
            StatusCode::CONFLICT,
            Json(json!({"error":"staged_blob_in_use","deleted":false})),
        ),
        None if error.is::<tokio::sync::TryAcquireError>() => (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"storage_slots_busy","deleted":false})),
        ),
        None => {
            tracing::warn!(error=%error,"staged deletion incomplete; durable cleanup may remain pending");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error":"staged_deletion_unconfirmed","cleanup_may_be_pending":true})),
            )
        }
    }
}
async fn delete_blob(
    State(a): State<Shared>,
    Path(hash): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let hash_id = id(&hash).map_err(|_| bad("invalid hash"))?;
    let authority = blob_actor(&a, &headers, "delete", &hash).await?;
    let body = tokio::time::timeout(
        Duration::from_secs(5),
        axum::body::to_bytes(request.into_body(), 0),
    )
    .await
    .map_err(|_| bad("delete body timeout"))?
    .map_err(|_| bad("delete body must be empty"))?;
    if !body.is_empty() {
        return Err(bad("delete body must be empty"));
    }
    blob_actor(&a, &headers, "delete", &hash).await?;
    a.storage
        .as_ref()
        .ok_or_else(unavailable)?
        .delete_staged(
            crate::staged_deletion::DeleteAuthority::Publisher(authority),
            hash_id,
        )
        .await
        .map_err(staged_delete_error)?;
    Ok(Json(json!({"sha256":hash,"deleted":true})))
}
async fn submit_event(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let authorization = operator_preflight(&a, &uri, &headers, "POST")?;
    if !a.projection_enabled {
        return Err(unavailable());
    }
    let _permit = a.event_slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"event_slots_busy"})),
        )
    })?;
    let body = tokio::time::timeout(
        Duration::from_secs(5),
        axum::body::to_bytes(request.into_body(), 1048576),
    )
    .await
    .map_err(|_| {
        (
            StatusCode::REQUEST_TIMEOUT,
            Json(json!({"error":"body_timeout"})),
        )
    })?
    .map_err(|_| {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error":"body_budget"})),
        )
    })?;
    auth::http(
        &authorization,
        &format!("{}{}", a.base_url, uri),
        "POST",
        &body,
    )
    .map_err(|_| forbidden("body binding or freshness failed"))?;
    let event: auth::Event =
        serde_json::from_slice(&body).map_err(|_| bad("invalid signed event"))?;
    event.verify().map_err(|_| bad("invalid signed event"))?;
    if event.pubkey != a.admin {
        return Err(forbidden("publisher is not admitted"));
    }
    if !matches!(event.kind, 1 | 7 | 1111)
        || !event.values("expiration").is_empty()
        || event.tags.iter().flatten().any(|v| v.len() > 1024)
    {
        return Err(bad(
            "event requires a dedicated lifecycle or exceeds tag policy",
        ));
    }
    consume_authorization(&a, &authorization).await?;
    let inserted = a
        .repo
        .stage_regular_event(&event)
        .await
        .map_err(|_| unavailable())?;
    let event_id = id(&event.id).map_err(|_| bad("invalid event ID"))?;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
    loop {
        if a.repo
            .projection_complete(event_id)
            .await
            .map_err(|_| unavailable())?
        {
            return Ok((
                StatusCode::OK,
                Json(json!({"id":event.id,"projected":true,"inserted":inserted})),
            ));
        }
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({"id":event.id,"projected":false,"pending":true,"inserted":inserted})),
    ))
}
async fn read_event(
    State(a): State<Shared>,
    Path(hash): Path<String>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let hash = id(&hash).map_err(|_| bad("invalid hash"))?;
    let Some(event) = a
        .repo
        .visible_event(hash)
        .await
        .map_err(|_| unavailable())?
    else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error":"event_unavailable"})),
        ));
    };
    if !a
        .repo
        .projection_complete(hash)
        .await
        .map_err(|_| unavailable())?
    {
        return Ok((
            StatusCode::ACCEPTED,
            Json(json!({"id":event.id,"projected":false,"pending":true})),
        ));
    }
    Ok((
        StatusCode::OK,
        Json(serde_json::to_value(&*event).map_err(|_| unavailable())?),
    ))
}

async fn publish_event(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let (authorization, authority) = publisher_preflight(&a, &uri, &headers, "POST").await?;
    let publisher = a.publications.clone().ok_or_else(unavailable)?;
    let permit = publisher.slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"validation_slots_busy"})),
        )
    })?;
    let body = tokio::time::timeout(
        Duration::from_secs(5),
        axum::body::to_bytes(request.into_body(), 1048576),
    )
    .await
    .map_err(|_| {
        (
            StatusCode::REQUEST_TIMEOUT,
            Json(json!({"error":"body_timeout"})),
        )
    })?
    .map_err(|_| {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error":"body_budget"})),
        )
    })?;
    auth::http(
        &authorization,
        &format!("{}{}", a.base_url, uri),
        "POST",
        &body,
    )
    .map_err(|_| forbidden("body binding or freshness failed"))?;
    let event: auth::Event =
        serde_json::from_slice(&body).map_err(|_| bad("invalid signed publication"))?;
    event
        .verify()
        .map_err(|_| bad("invalid signed publication"))?;
    if event.pubkey != authorization.pubkey {
        return Err(forbidden("publication author must match request signer"));
    }
    consume_authorization(&a, &authorization).await?;
    let hash = id(&event.id).map_err(|_| bad("invalid event ID"))?;
    let inserted = spawn_owned_scope(async move {
        let _permit = permit;
        publisher.publish_with_permit(&event, authority).await
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|error| {
        tracing::warn!(error=%error,"publication not admitted");
        bad("publication incomplete, invalid, removed or not permitted")
    })?;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
    loop {
        if a.repo
            .projection_complete(hash)
            .await
            .map_err(|_| unavailable())?
        {
            return Ok((
                StatusCode::OK,
                Json(
                    json!({"id":hex::encode(hash),"hosted":true,"projected":true,"inserted":inserted}),
                ),
            ));
        }
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({"id":hex::encode(hash),"hosted":true,"projected":false,"pending":true})),
    ))
}
async fn public_original(
    State(a): State<Shared>,
    Path(hash): Path<String>,
) -> Result<axum::response::Response, ApiError> {
    let hash = id(&hash).map_err(|_| bad("invalid hash"))?;
    let storage = a.storage.clone().ok_or_else(unavailable)?;
    let (snapshot, mime) = storage
        .download_public_optional(hash)
        .await
        .map_err(download_error)?
        .ok_or_else(|| download_error(crate::upload::BlobUnavailable.into()))?;
    Ok(snapshot.response(None, &mime, false))
}

async fn query_chapters(
    State(a): State<Shared>,
    RawQuery(raw): RawQuery,
) -> Result<Json<crate::discovery::Page>, ApiError> {
    let q = crate::discovery::QueryOptions::parse(raw.as_deref())
        .map_err(|_| bad("invalid or overbudget query"))?;
    a.repo.discover(q).await.map(Json).map_err(|error|{tracing::warn!(error=%error,"discovery request failed");(StatusCode::SERVICE_UNAVAILABLE,Json(json!({"error":"query_budget_or_unavailable","message":"no partial result returned"})))})
}

async fn operator_json(
    a: &App,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    request: Request,
) -> Result<Value, ApiError> {
    let authorization = operator_preflight(a, uri, headers, "POST")?;
    let _permit = a.event_slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"request_slots_busy"})),
        )
    })?;
    let body = tokio::time::timeout(
        Duration::from_secs(5),
        axum::body::to_bytes(request.into_body(), 1048576),
    )
    .await
    .map_err(|_| {
        (
            StatusCode::REQUEST_TIMEOUT,
            Json(json!({"error":"body_timeout"})),
        )
    })?
    .map_err(|_| {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error":"body_budget"})),
        )
    })?;
    auth::http(
        &authorization,
        &format!("{}{}", a.base_url, uri),
        "POST",
        &body,
    )
    .map_err(|_| forbidden("body binding or freshness failed"))?;
    let value = serde_json::from_slice(&body).map_err(|_| bad("JSON body required"))?;
    consume_authorization(a, &authorization).await?;
    Ok(value)
}
async fn deletion_request(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let value = operator_json(&a, &uri, &headers, request).await?;
    let event: auth::Event =
        serde_json::from_value(value).map_err(|_| bad("signed deletion request required"))?;
    let inserted = a
        .repo
        .record_deletion_request(&event)
        .await
        .map_err(|_| bad("invalid or overbudget deletion request"))?;
    let request_id = id(&event.id).map_err(|_| bad("invalid request ID"))?;
    let automatic = if a.auto_approve_publication_deletions {
        a.repo.approve_deletion(request_id, None).await.ok()
    } else {
        None
    };
    Ok((
        StatusCode::ACCEPTED,
        Json(
            json!({"id":event.id,"recorded":true,"inserted":inserted,"automatic_approval":automatic.is_some(),"new_withdrawals":automatic,"default_requires_moderator":!a.auto_approve_publication_deletions}),
        ),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Approval {
    targets: Option<Vec<String>>,
}
async fn approve_deletion(
    State(a): State<Shared>,
    Path(hash): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let request_id = id(&hash).map_err(|_| bad("invalid request ID"))?;
    let value = operator_json(&a, &uri, &headers, request).await?;
    let approval: Approval =
        serde_json::from_value(value).map_err(|_| bad("invalid approval body"))?;
    let selected = approval
        .targets
        .map(|v| v.iter().map(|v| id(v)).collect::<Result<Vec<_>>>())
        .transpose()
        .map_err(|_| bad("invalid target ID"))?;
    let changed = a
        .repo
        .approve_deletion(request_id, selected)
        .await
        .map_err(|error| {
            tracing::warn!(error=%error,"local deletion approval rejected");
            bad("request targets missing, inapplicable or not owned by author")
        })?;
    Ok((
        StatusCode::ACCEPTED,
        Json(
            json!({"request":hash,"new_withdrawals":changed,"projection_completion_pending":changed>0}),
        ),
    ))
}
async fn restore_publication(
    State(a): State<Shared>,
    Path(hash): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let event_id = id(&hash).map_err(|_| bad("invalid event ID"))?;
    let value = operator_json(&a, &uri, &headers, request).await?;
    if value != json!({}) {
        return Err(bad("restore body must be empty object"));
    }
    a.repo
        .restore_publication(event_id)
        .await
        .map_err(|_| bad("completed protected removal required"))?;
    Ok(Json(
        json!({"id":hash,"state":"candidate","requires_validation":true}),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PeerPolicy {
    key: String,
    endpoint: String,
    allowed: bool,
}
async fn configure_peer(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let value = operator_json(&a, &uri, &headers, request).await?;
    let policy: PeerPolicy =
        serde_json::from_value(value).map_err(|_| bad("invalid peer policy"))?;
    let key = id(&policy.key).map_err(|_| bad("invalid peer key"))?;
    let version = a
        .repo
        .set_peer(key, &policy.endpoint, policy.allowed)
        .await
        .map_err(|_| bad("invalid endpoint or peer policy unavailable"))?;
    Ok(Json(
        json!({"key":policy.key,"allowed":policy.allowed,"version":version}),
    ))
}
async fn peer_authorized(
    a: &App,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
) -> Result<(), ApiError> {
    let header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| forbidden("approved peer required"))?;
    if header.len() > 16384 {
        return Err(forbidden("authorization header budget"));
    }
    let event = auth::token(header).map_err(|_| forbidden("invalid authorization event"))?;
    auth::http_scope(&event, &format!("{}{}", a.base_url, uri), "GET")
        .map_err(|_| forbidden("invalid URL, method, signature or freshness"))?;
    a.repo
        .approved_peer(id(&event.pubkey).map_err(|_| forbidden("invalid peer key"))?)
        .await
        .map_err(|error| role_error(error, "peer is not approved"))?;
    consume_authorization(a, &event).await
}
async fn peer_event(
    State(a): State<Shared>,
    Path(hash): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    peer_authorized(&a, &uri, &headers).await?;
    let hash = id(&hash).map_err(|_| bad("invalid event ID"))?;
    if !a
        .repo
        .projection_complete(hash)
        .await
        .map_err(|_| unavailable())?
    {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error":"event_not_retained"})),
        ));
    }
    let event = a
        .repo
        .visible_event(hash)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error":"event_not_retained"})),
            )
        })?;
    Ok(Json(
        serde_json::to_value(&*event).map_err(|_| unavailable())?,
    ))
}
async fn peer_blob(
    State(a): State<Shared>,
    Path(hash): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    peer_authorized(&a, &uri, &headers).await?;
    let hash = id(&hash).map_err(|_| bad("invalid blob hash"))?;
    let storage = a.storage.clone().ok_or_else(unavailable)?;
    let (snapshot, mime) = storage
        .download_public_optional(hash)
        .await
        .map_err(download_error)?
        .ok_or_else(|| download_error(crate::upload::BlobUnavailable.into()))?;
    a.peer_blob_reads
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(snapshot.response(None, &mime, false))
}

fn sync_rejection_error(error: &anyhow::Error) -> Option<ApiError> {
    if let Some(rejection) = error.downcast_ref::<crate::transfer_rejection::TransferRejection>() {
        Some((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({"error":"content_not_admitted","hosted":false,"category":rejection.class,"reason":rejection.reason}),
            ),
        ))
    } else if error.is::<crate::peers::PeerUnavailable>() {
        Some((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"peer_temporarily_unavailable","retryable":true})),
        ))
    } else {
        None
    }
}
async fn sync_peer_chapter(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<crate::peers::SyncResult>), ApiError> {
    let value = operator_json(&a, &uri, &headers, request).await?;
    let request: crate::peers::SyncRequest =
        serde_json::from_value(value).map_err(|_| bad("invalid synchronization request"))?;
    let sync = a.peer_sync.clone().ok_or_else(unavailable)?;
    let permit = sync.slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"sync_slots_busy"})),
        )
    })?;
    let result = spawn_owned_scope(async move {
        let _permit = permit;
        sync.synchronize(request).await
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|error| {
        if let Some(response) = sync_rejection_error(&error) {
            return response;
        }
        tracing::warn!(error=%error,"approved-peer synchronization failed");
        bad("peer, content, filter, quota or approval validation failed")
    })?;
    Ok((
        if result.hosted && !result.projected {
            StatusCode::ACCEPTED
        } else {
            StatusCode::OK
        },
        Json(result),
    ))
}

async fn reconcile_inventory(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<crate::inventory::ReconcileReply>, ApiError> {
    let header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| forbidden("approved peer required"))?;
    if header.len() > 16384 {
        return Err(forbidden("authorization header budget"));
    }
    let authorization =
        auth::token(header).map_err(|_| forbidden("invalid authorization event"))?;
    auth::http_scope(&authorization, &format!("{}{}", a.base_url, uri), "POST")
        .map_err(|_| forbidden("invalid URL, method or freshness"))?;
    if !authorization.one("payload").is_ok_and(auth::valid_hash) {
        return Err(forbidden("signed body hash required"));
    }
    let peer = id(&authorization.pubkey).map_err(|_| forbidden("invalid peer key"))?;
    let approval = a
        .repo
        .approved_peer(peer)
        .await
        .map_err(|error| role_error(error, "peer is not approved"))?;
    let permit = a.snapshots.slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"reconciliation_slots_busy"})),
        )
    })?;
    let body = tokio::time::timeout(
        Duration::from_secs(5),
        axum::body::to_bytes(request.into_body(), 262144),
    )
    .await
    .map_err(|_| bad("body timeout"))?
    .map_err(|_| bad("body budget"))?;
    auth::http(
        &authorization,
        &format!("{}{}", a.base_url, uri),
        "POST",
        &body,
    )
    .map_err(|_| forbidden("body binding or freshness failed"))?;
    let request: crate::inventory::ReconcileRequest =
        serde_json::from_slice(&body).map_err(|_| bad("invalid reconciliation request"))?;
    consume_authorization(&a, &authorization).await?;
    let snapshots = a.snapshots.clone();
    spawn_owned_scope(async move {
        let _permit = permit;
        snapshots.reconcile(peer, approval.version, request).await
    })
    .await
    .map_err(|_| unavailable())?
    .map(Json)
    .map_err(|error| {
        if error.is::<crate::inventory::PartitionRequired>() {
            (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(json!({"error":"partition_required","message":"split event-ID prefix"})),
            )
        } else {
            tracing::warn!(error=%error,"bounded reconciliation rejected");
            bad("session expired, invalid, out of scope or over budget")
        }
    })
}

async fn enqueue_sync(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let value = operator_json(&a, &uri, &headers, request).await?;
    if a.peer_sync.is_none() {
        return Err(unavailable());
    }
    let spec: crate::sync_jobs::SyncSpec =
        serde_json::from_value(value).map_err(|_| bad("invalid sync job request"))?;
    let id = a
        .repo
        .enqueue_sync(spec)
        .await
        .map_err(|_| bad("peer, filter or active queue budget rejected"))?;
    Ok((
        StatusCode::ACCEPTED,
        Json(a.repo.sync_status(id).await.map_err(|_| unavailable())?),
    ))
}
async fn sync_status(
    State(a): State<Shared>,
    Path(hash): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let authorization = operator_preflight(&a, &uri, &headers, "GET")?;
    consume_authorization(&a, &authorization).await?;
    let id = id(&hash).map_err(|_| bad("invalid job ID"))?;
    a.repo.sync_status(id).await.map(Json).map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"job_not_found"})),
        )
    })
}
async fn cancel_sync(
    State(a): State<Shared>,
    Path(hash): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let value = operator_json(&a, &uri, &headers, request).await?;
    if value != json!({}) {
        return Err(bad("cancel body must be empty object"));
    }
    let id = id(&hash).map_err(|_| bad("invalid job ID"))?;
    let changed = a.repo.cancel_sync(id).await.map_err(|_| unavailable())?;
    Ok(Json(
        json!({"id":hash,"cancelled":changed,"already_admitted_content_is_retained":true}),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupAdministrator {
    administrator: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupImport {
    key: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupEnabled {
    enabled: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupMember {
    member: String,
    role: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupDeletion {
    targets: Vec<String>,
    #[serde(default)]
    reason: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupProfile {
    metadata: Value,
    expected_head: Option<String>,
}
async fn set_group_profile(
    State(a): State<Shared>,
    Path(key): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let group = id(&key).map_err(|_| bad("invalid group key"))?;
    let (groups, authorization, actor, version) =
        group_preflight(&a, group, "profile", &uri, &headers, "POST").await?;
    let body = group_body(&a, &uri, &authorization, request, 32768).await?;
    let profile: GroupProfile =
        serde_json::from_slice(&body).map_err(|_| bad("profile metadata object required"))?;
    let expected = profile
        .expected_head
        .as_deref()
        .map(id)
        .transpose()
        .map_err(|_| bad("invalid expected profile head"))?;
    let event = spawn_owned_scope(async move {
        groups
            .profile(group, actor, version, profile.metadata, expected)
            .await
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|error| {
        if error.is::<crate::groups::ProfileChanged>() {
            (
                StatusCode::CONFLICT,
                Json(json!({"error":"group_profile_changed"})),
            )
        } else if error.is::<crate::groups::ProfileClockAhead>() {
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({"error":"group_profile_clock_ahead"})),
            )
        } else if error.is::<crate::groups::AuditUnavailable>()
            || error.is::<tokio_postgres::Error>()
            || error.is::<deadpool_postgres::PoolError>()
        {
            unavailable()
        } else {
            bad("profile rejected or group permission changed")
        }
    })?;
    Ok(Json(
        json!({"event":event,"public":true,"private_key_returned":false}),
    ))
}
async fn create_group(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let value = operator_json(&a, &uri, &headers, request).await?;
    let request: GroupAdministrator =
        serde_json::from_value(value).map_err(|_| bad("administrator public key required"))?;
    let administrator =
        id(&request.administrator).map_err(|_| bad("invalid administrator public key"))?;
    let groups = a.groups.clone().ok_or_else(unavailable)?;
    let group = spawn_owned_scope(async move { groups.create(administrator).await })
        .await
        .map_err(|_| unavailable())?
        .map_err(|_| bad("group creation failed"))?;
    Ok(Json(
        json!({"group":hex::encode(group),"administrator":request.administrator,"custody":"instance","private_key_returned":false}),
    ))
}
async fn import_group(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let value = operator_json(&a, &uri, &headers, request).await?;
    let request: GroupImport =
        serde_json::from_value(value).map_err(|_| bad("group public key required"))?;
    let group = id(&request.key).map_err(|_| bad("invalid group key"))?;
    a.groups
        .as_ref()
        .ok_or_else(unavailable)?
        .register_recovered_key(group)
        .await
        .map_err(|_| bad("valid protected key file required in configured key store"))?;
    Ok(Json(
        json!({"group":request.key,"key_registered":true,"membership_recreated":false}),
    ))
}
async fn bootstrap_group(
    State(a): State<Shared>,
    Path(key): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let value = operator_json(&a, &uri, &headers, request).await?;
    let request: GroupAdministrator =
        serde_json::from_value(value).map_err(|_| bad("administrator public key required"))?;
    a.groups
        .as_ref()
        .ok_or_else(unavailable)?
        .bootstrap_recovered(
            id(&key).map_err(|_| bad("invalid group"))?,
            id(&request.administrator).map_err(|_| bad("invalid administrator"))?,
        )
        .await
        .map_err(|_| {
            bad("explicit bootstrap requires disabled key-only recovery without members")
        })?;
    Ok(Json(
        json!({"group":key,"administrator":request.administrator,"enabled":true}),
    ))
}
async fn enable_group(
    State(a): State<Shared>,
    Path(key): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let value = operator_json(&a, &uri, &headers, request).await?;
    let request: GroupEnabled =
        serde_json::from_value(value).map_err(|_| bad("enabled boolean required"))?;
    a.groups
        .as_ref()
        .ok_or_else(unavailable)?
        .set_enabled(id(&key).map_err(|_| bad("invalid group"))?, request.enabled)
        .await
        .map_err(|_| bad("group policy unavailable"))?;
    Ok(Json(json!({"group":key,"enabled":request.enabled})))
}
async fn group_preflight(
    a: &App,
    group: Id,
    action: &str,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    method: &str,
) -> Result<(Arc<crate::groups::Groups>, auth::Event, Id, i64), ApiError> {
    let header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| forbidden("member authorization required"))?;
    if header.len() > 16384 {
        return Err(forbidden("authorization header budget"));
    }
    let event = auth::token(header).map_err(|_| forbidden("invalid authorization event"))?;
    auth::http_scope(&event, &format!("{}{}", a.base_url, uri), method)
        .map_err(|_| forbidden("invalid URL, method, signature or freshness"))?;
    if method == "POST" && !event.one("payload").is_ok_and(auth::valid_hash) {
        return Err(forbidden("signed body hash required"));
    }
    let actor = id(&event.pubkey).map_err(|_| forbidden("invalid member key"))?;
    let groups = a.groups.clone().ok_or_else(unavailable)?;
    let version = groups
        .authorize(group, actor, action)
        .await
        .map_err(|error| role_error(error, "current group role required"))?;
    Ok((groups, event, actor, version))
}
async fn group_body(
    a: &App,
    uri: &axum::http::Uri,
    event: &auth::Event,
    request: Request,
    limit: usize,
) -> Result<axum::body::Bytes, ApiError> {
    if request
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
        .is_some_and(|n| n > limit)
    {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error":"body_budget"})),
        ));
    }
    let bytes = tokio::time::timeout(
        Duration::from_secs(30),
        axum::body::to_bytes(request.into_body(), limit),
    )
    .await
    .map_err(|_| bad("body timeout"))?
    .map_err(|_| {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error":"body_budget"})),
        )
    })?;
    auth::http(event, &format!("{}{}", a.base_url, uri), "POST", &bytes)
        .map_err(|_| forbidden("body binding or freshness failed"))?;
    consume_authorization(a, event).await?;
    Ok(bytes)
}
async fn set_group_member(
    State(a): State<Shared>,
    Path(key): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let group = id(&key).map_err(|_| bad("invalid group"))?;
    let (groups, event, actor, version) =
        group_preflight(&a, group, "admin", &uri, &headers, "POST").await?;
    let _slot = a
        .event_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| unavailable())?;
    let body = group_body(&a, &uri, &event, request, 65536).await?;
    let request: GroupMember =
        serde_json::from_slice(&body).map_err(|_| bad("invalid member policy"))?;
    let role = match request.role {
        Value::Null => None,
        Value::String(role) => Some(role),
        _ => return Err(bad("role must be text or explicit null")),
    };
    groups
        .set_member(
            group,
            actor,
            version,
            id(&request.member).map_err(|_| bad("invalid member key"))?,
            role,
        )
        .await
        .map_err(|_| {
            bad("role change rejected; current administrator and valid member key required")
        })?;
    Ok(Json(json!({"group":key,"updated":true})))
}
async fn list_group_members(
    State(a): State<Shared>,
    Path(key): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let group = id(&key).map_err(|_| bad("invalid group"))?;
    let (groups, event, actor, version) =
        group_preflight(&a, group, "admin", &uri, &headers, "GET").await?;
    consume_authorization(&a, &event).await?;
    groups
        .members(group, actor, version)
        .await
        .map(Json)
        .map_err(|_| bad("member policy changed or listing budget exceeded"))
}
async fn group_upload(
    State(a): State<Shared>,
    Path((key, hash)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let group = id(&key).map_err(|_| bad("invalid group"))?;
    let hash_id = id(&hash).map_err(|_| bad("invalid hash"))?;
    let (groups, event, actor, version) =
        group_preflight(&a, group, "publish", &uri, &headers, "POST").await?;
    let permit = groups.upload_slots().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"upload_slots_busy"})),
        )
    })?;
    let bytes = group_body(&a, &uri, &event, request, groups.max_blob_bytes() as usize).await?;
    let size = bytes.len();
    if size == 0 || auth::hash(&bytes) != hash {
        return Err(bad("body hash or size mismatch"));
    }
    spawn_owned_scope(async move {
        let _permit = permit;
        groups.upload(group, actor, version, hash_id, &bytes).await
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|_| bad("group upload failed or authorization changed"))?;
    Ok(Json(
        json!({"group":key,"sha256":hash,"size":size,"staged":true}),
    ))
}
async fn group_delete_upload(
    State(a): State<Shared>,
    Path((key, hash)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let group = id(&key).map_err(|_| bad("invalid group"))?;
    let hash_id = id(&hash).map_err(|_| bad("invalid hash"))?;
    let (groups, event, actor, version) =
        group_preflight(&a, group, "remove", &uri, &headers, "POST").await?;
    let bytes = group_body(&a, &uri, &event, request, 4096).await?;
    if serde_json::from_slice::<Value>(&bytes).map_err(|_| bad("empty object required"))?
        != json!({})
    {
        return Err(bad("empty object required"));
    }
    groups
        .delete_upload(group, actor, version, hash_id)
        .await
        .map_err(staged_delete_error)?;
    Ok(Json(json!({"group":key,"sha256":hash,"deleted":true})))
}
async fn group_publish(
    State(a): State<Shared>,
    Path(key): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let group = id(&key).map_err(|_| bad("invalid group"))?;
    let (groups, event, actor, version) =
        group_preflight(&a, group, "publish", &uri, &headers, "POST").await?;
    let permit = groups
        .publication_slots()
        .try_acquire_owned()
        .map_err(|_| {
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({"error":"validation_slots_busy"})),
            )
        })?;
    let body = group_body(&a, &uri, &event, request, 65536).await?;
    let request: crate::groups::GroupPublication = serde_json::from_slice(&body)
        .map_err(|_| bad("only kind, manifest, version and optional created_at are accepted"))?;
    let event = spawn_owned_scope(async move {
        let _permit = permit;
        groups.publish(group, actor, version, request).await
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|error| {
        tracing::warn!(error=%error,"group publication rejected");
        if error.is::<crate::groups::AuditUnavailable>() {
            unavailable()
        } else {
            bad("group policy, manifest or original validation failed")
        }
    })?;
    let projected = a
        .repo
        .projection_complete(id(&event.id).map_err(|_| bad("invalid signed event"))?)
        .await
        .map_err(|_| unavailable())?;
    Ok((
        if projected {
            StatusCode::OK
        } else {
            StatusCode::ACCEPTED
        },
        Json(json!({"event":event,"projected":projected,"hosted":true})),
    ))
}
async fn group_deletion_request(
    State(a): State<Shared>,
    Path(key): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let group = id(&key).map_err(|_| bad("invalid group"))?;
    let (groups, event, actor, version) =
        group_preflight(&a, group, "remove", &uri, &headers, "POST").await?;
    let _permit = a
        .event_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| unavailable())?;
    let body = group_body(&a, &uri, &event, request, 65536).await?;
    let request: GroupDeletion =
        serde_json::from_slice(&body).map_err(|_| bad("targets and reason required"))?;
    let event = groups
        .deletion_request(group, actor, version, request.targets, request.reason)
        .await
        .map_err(|error| {
            if error.is::<crate::groups::AuditUnavailable>() {
                unavailable()
            } else {
                bad("group deletion request failed or policy changed")
            }
        })?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({"event":event,"requires_instance_moderator":true})),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountAdmission {
    allowed: bool,
}
async fn configure_publisher(
    State(a): State<Shared>,
    Path(key): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let publisher = id(&key).map_err(|_| bad("invalid publisher key"))?;
    let value = operator_json(&a, &uri, &headers, request).await?;
    let admission: AccountAdmission =
        serde_json::from_value(value).map_err(|_| bad("allowed boolean required"))?;
    a.repo
        .set_publisher_admission(publisher, admission.allowed)
        .await
        .map_err(|_| unavailable())?;
    Ok(Json(json!({"publisher":key,"allowed":admission.allowed})))
}
async fn publisher_preflight(
    a: &App,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    method: &str,
) -> Result<(auth::Event, crate::publisher_authority::PublisherPermit), ApiError> {
    let header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| forbidden("publisher authorization required"))?;
    if header.len() > 16384 {
        return Err(forbidden("authorization header budget"));
    }
    let authorization =
        auth::token(header).map_err(|_| forbidden("invalid authorization event"))?;
    auth::http_scope(&authorization, &format!("{}{}", a.base_url, uri), method)
        .map_err(|_| forbidden("invalid URL, method, signature or freshness"))?;
    if method == "POST" && !authorization.one("payload").is_ok_and(auth::valid_hash) {
        return Err(forbidden("signed body hash required"));
    }
    let permit = a
        .repo
        .publisher_permit(
            id(&authorization.pubkey).map_err(|_| forbidden("invalid publisher key"))?,
        )
        .await
        .map_err(publisher_error)?;
    Ok((authorization, permit))
}
async fn configure_account(
    State(a): State<Shared>,
    Path(key): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let publisher = id(&key).map_err(|_| bad("invalid account key"))?;
    let value = operator_json(&a, &uri, &headers, request).await?;
    let admission: AccountAdmission =
        serde_json::from_value(value).map_err(|_| bad("allowed boolean required"))?;
    a.repo
        .set_account_admission(publisher, admission.allowed)
        .await
        .map_err(|_| unavailable())?;
    Ok(Json(json!({"publisher":key,"allowed":admission.allowed})))
}
async fn submit_account_metadata(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if !a.projection_enabled {
        return Err(unavailable());
    }
    let (authorization, authority) = account_preflight(&a, &uri, &headers, "POST").await?;
    let _slot = a.event_slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"event_slots_busy"})),
        )
    })?;
    let body = group_body(&a, &uri, &authorization, request, 1_048_576).await?;
    let event: auth::Event =
        serde_json::from_slice(&body).map_err(|_| bad("invalid signed metadata"))?;
    if event.pubkey != authorization.pubkey {
        return Err(forbidden("request signer must be the event author"));
    }
    if !matches!(event.kind, 0 | 3 | 10002) {
        return Err(bad("event requires a dedicated workflow"));
    }
    let outcome = a
        .repo
        .stage_account_metadata(&authority, &event)
        .await
        .map_err(|error| {
            tracing::debug!(error=%error,"account metadata rejected");
            validation_error(error, "invalid metadata or changed account permission")
        })?;
    if outcome == crate::accounts::StageOutcome::Superseded {
        return Ok((
            StatusCode::CONFLICT,
            Json(json!({"id":event.id,"superseded":true})),
        ));
    }
    let projected = a
        .repo
        .projection_complete(id(&event.id).map_err(|_| bad("invalid event ID"))?)
        .await
        .map_err(|_| unavailable())?;
    Ok((
        if projected {
            StatusCode::OK
        } else {
            StatusCode::ACCEPTED
        },
        Json(
            json!({"id":event.id,"projected":projected,"inserted":outcome==crate::accounts::StageOutcome::Accepted}),
        ),
    ))
}
async fn account_head(
    State(a): State<Shared>,
    Path((key, kind)): Path<(String, u32)>,
) -> Result<Json<Value>, ApiError> {
    let publisher = id(&key).map_err(|_| bad("invalid account key"))?;
    if !matches!(kind, 0 | 3 | 10002) {
        return Err(bad("unsupported metadata kind"));
    }
    let client = a.repo.pool.get().await.map_err(|_| unavailable())?;
    let kind = kind as i32;
    let row=client.query_opt("SELECT h.event_id,h.winner_at,NOT EXISTS(SELECT 1 FROM backend_outbox o WHERE o.event_id=h.event_id AND o.completed_at IS NULL) FROM account_heads h JOIN events e ON e.id=h.event_id WHERE h.publisher=$1 AND h.kind=$2 AND e.state='hosted'", &[&publisher.as_slice(),&kind]).await.map_err(|_|unavailable())?.ok_or_else(||(StatusCode::NOT_FOUND,Json(json!({"error":"no_current_head"}))))?;
    Ok(Json(
        json!({"publisher":key,"kind":kind,"id":hex::encode(row.get::<_,Vec<u8>>(0)),"created_at":row.get::<_,i64>(1),"projected":row.get::<_,bool>(2)}),
    ))
}

async fn account_preflight(
    a: &App,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    method: &str,
) -> Result<(auth::Event, crate::accounts::AccountPermit), ApiError> {
    let header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| forbidden("account authorization required"))?;
    if header.len() > 16_384 {
        return Err(forbidden("authorization header budget"));
    }
    let authorization =
        auth::token(header).map_err(|_| forbidden("invalid authorization event"))?;
    auth::http_scope(&authorization, &format!("{}{}", a.base_url, uri), method)
        .map_err(|_| forbidden("invalid URL, method, signature or freshness"))?;
    if method == "POST" && !authorization.one("payload").is_ok_and(auth::valid_hash) {
        return Err(forbidden("signed body hash required"));
    }
    let publisher = id(&authorization.pubkey).map_err(|_| forbidden("invalid account key"))?;
    let authority = a
        .repo
        .account_permit(publisher)
        .await
        .map_err(|error| account_error(error, "account is not admitted"))?;
    Ok((authorization, authority))
}
async fn stage_library_object(
    State(a): State<Shared>,
    Path(kind): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let (authorization, authority) = account_preflight(&a, &uri, &headers, "POST").await?;
    let library = a.library_objects.clone().ok_or_else(unavailable)?;
    let kind = match kind.as_str() {
        "chunk" => crate::library_storage::ObjectType::Chunk,
        "root" => crate::library_storage::ObjectType::Root,
        _ => return Err(bad("unknown library object type")),
    };
    let limits = crate::library_manifest::Limits::default();
    let bound = match kind {
        crate::library_storage::ObjectType::Chunk => limits.max_chunk_bytes,
        crate::library_storage::ObjectType::Root => limits.max_root_bytes,
    };
    let _slot = a.event_slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"body_slots_busy"})),
        )
    })?;
    let body = group_body(&a, &uri, &authorization, request, bound).await?;
    let intent = library
        .stage(authority, kind, body.to_vec())
        .await
        .map_err(|error| library_error(error, StatusCode::BAD_REQUEST))?;
    Ok(Json(
        json!({"hash":hex::encode(intent.key.hash),"size":intent.size,"staged":true,"public":false}),
    ))
}
async fn read_library_staging(
    State(a): State<Shared>,
    Path(hash): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    let (authorization, authority) = account_preflight(&a, &uri, &headers, "GET").await?;
    let library = a.library_objects.clone().ok_or_else(unavailable)?;
    let hash = id(&hash).map_err(|_| bad("invalid object hash"))?;
    consume_authorization(&a, &authorization).await?;
    let lease = library
        .response_bytes
        .clone()
        .try_acquire_many_owned(1_000_000)
        .map_err(|e| library_error(e.into(), StatusCode::TOO_MANY_REQUESTS))?;
    let bytes = library
        .read(&authority, hash)
        .await
        .map_err(|_| forbidden("staged object unavailable to account"))?;
    Ok(crate::bounded_response::response(
        bytes,
        lease,
        "application/cbor",
        true,
        false,
    ))
}

fn library_error(error: anyhow::Error, fallback: StatusCode) -> ApiError {
    let (status, code) = if error.is::<crate::library::HeadConflict>() {
        (StatusCode::CONFLICT, "library_head_changed")
    } else if error.is::<tokio::sync::TryAcquireError>()
        || error.is::<crate::library_admission::Busy>()
    {
        (StatusCode::TOO_MANY_REQUESTS, "library_busy")
    } else if error.is::<deadpool_postgres::PoolError>() || error.is::<tokio_postgres::Error>() {
        (StatusCode::SERVICE_UNAVAILABLE, "library_store_unavailable")
    } else {
        (fallback, "library_unavailable_or_invalid")
    };
    tracing::debug!(error=%error,"library operation rejected");
    let mut body = json!({"error":code});
    if let Some(busy) = error.downcast_ref::<crate::library_admission::Busy>() {
        body["stage"] = json!(busy.stage);
    }
    (status, Json(body))
}
fn gzip_body(headers: &HeaderMap) -> Result<bool, ApiError> {
    match headers
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
    {
        None | Some("identity") => Ok(false),
        Some("gzip") => Ok(true),
        _ => Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Json(json!({"error":"unsupported_content_encoding"})),
        )),
    }
}
async fn commit_library(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let (authorization, authority) = account_preflight(&a, &uri, &headers, "POST").await?;
    if !a.projection_enabled {
        return Err(unavailable());
    }
    let library = a.library_objects.clone().ok_or_else(unavailable)?;
    let declared = headers
        .get("content-length")
        .map(|v| {
            v.to_str()
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .ok_or_else(|| bad("invalid content length"))
        })
        .transpose()?;
    let receipt = library
        .admission
        .receive(declared)
        .map_err(|e| library_error(e, StatusCode::PAYLOAD_TOO_LARGE))?;
    let gzip = gzip_body(&headers)?;
    let body = group_body(&a, &uri, &authorization, request, receipt.limit).await?;
    let slot = library
        .admission
        .process(receipt)
        .await
        .map_err(|e| library_error(e, StatusCode::TOO_MANY_REQUESTS))?;
    let (parsed, slot) = spawn_blocking_owned_scope(move || -> Result<_> {
        let input = body.to_vec();
        drop(body);
        let decoded = crate::library_wire::decode_body(input, gzip)?;
        Ok((crate::library_wire::commit(&decoded)?, slot))
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|e| library_error(e, StatusCode::BAD_REQUEST))?;
    if parsed.event.pubkey != authorization.pubkey {
        return Err(forbidden("request signer must be the library author"));
    }
    crate::library_manifest::pointer(&parsed.event)
        .map_err(|e| library_error(e, StatusCode::BAD_REQUEST))?;
    let event_id = parsed.event.id.clone();
    let outcome = spawn_owned_scope(async move {
        let _slot = slot;
        for (kind, bytes) in parsed.objects {
            library.stage(authority.clone(), kind, bytes).await?;
        }
        library
            .commit(authority, parsed.event, parsed.expected)
            .await
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|e| library_error(e, StatusCode::BAD_REQUEST))?;
    if outcome == crate::accounts::StageOutcome::Superseded {
        return Ok((
            StatusCode::CONFLICT,
            Json(json!({"id":event_id,"superseded":true})),
        ));
    }
    let projected = a
        .repo
        .projection_complete(id(&event_id).map_err(|_| bad("invalid event ID"))?)
        .await
        .map_err(|_| unavailable())?;
    Ok((
        if projected {
            StatusCode::OK
        } else {
            StatusCode::ACCEPTED
        },
        Json(
            json!({"id":event_id,"inserted":outcome==crate::accounts::StageOutcome::Accepted,"projected":projected}),
        ),
    ))
}
async fn upload_library_batch(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let (authorization, authority) = account_preflight(&a, &uri, &headers, "POST").await?;
    let library = a.library_objects.clone().ok_or_else(unavailable)?;
    let declared = headers
        .get("content-length")
        .map(|v| {
            v.to_str()
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .ok_or_else(|| bad("invalid content length"))
        })
        .transpose()?;
    let receipt = library
        .admission
        .receive(declared)
        .map_err(|e| library_error(e, StatusCode::PAYLOAD_TOO_LARGE))?;
    let gzip = gzip_body(&headers)?;
    let body = group_body(&a, &uri, &authorization, request, receipt.limit).await?;
    let slot = library
        .admission
        .process(receipt)
        .await
        .map_err(|e| library_error(e, StatusCode::TOO_MANY_REQUESTS))?;
    let (objects, slot) = spawn_blocking_owned_scope(move || -> Result<_> {
        let input = body.to_vec();
        drop(body);
        let decoded = crate::library_wire::decode_body(input, gzip)?;
        Ok((crate::library_wire::batch(&decoded)?, slot))
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|e| library_error(e, StatusCode::BAD_REQUEST))?;
    let hashes = spawn_owned_scope(async move {
        let _slot = slot;
        let mut hashes = vec![];
        for (kind, bytes) in objects {
            hashes.push(hex::encode(
                library
                    .stage(authority.clone(), kind, bytes)
                    .await?
                    .key
                    .hash,
            ));
        }
        Ok::<_, anyhow::Error>(hashes)
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|e| library_error(e, StatusCode::BAD_REQUEST))?;
    Ok(Json(json!({"hashes":hashes,"staged":true,"public":false})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryQuery {
    family: String,
    offset: Option<u64>,
    limit: Option<usize>,
    head: Option<String>,
}
async fn read_library_entries(
    State(a): State<Shared>,
    Path(key): Path<String>,
    Query(q): Query<EntryQuery>,
) -> Result<Json<Value>, ApiError> {
    let publisher = id(&key).map_err(|_| bad("invalid account key"))?;
    if let Some(head) = &q.head {
        id(head).map_err(|_| bad("invalid library head"))?;
    }
    let family =
        crate::library_view::family(&q.family).map_err(|_| bad("invalid library family"))?;
    let limit = q.limit.unwrap_or(100);
    if limit > 100 {
        return Err(bad("entry page limit"));
    }
    let library = a.library_objects.clone().ok_or_else(unavailable)?;
    let operation = async {
        let view = library
            .public_view(publisher, q.head.as_deref())
            .await?
            .ok_or_else(|| anyhow::anyhow!("library unavailable"))?;
        let page = library
            .view_page(&view, family, q.offset.unwrap_or(0), limit)
            .await?;
        Ok::<_, anyhow::Error>(
            json!({"event":view.event,"root":hex::encode(view.root_hash),"family":family.name(),"entries":page.records.iter().map(crate::library_view::record_json).collect::<Vec<_>>(),"chunks":page.chunks.iter().map(hex::encode).collect::<Vec<_>>(),"offset":q.offset.unwrap_or(0),"limit":limit,"total":page.total,"public":true}),
        )
    };
    let value = read_admission::deadline(
        Duration::from_secs(10),
        operation,
        bad("library view deadline"),
    )
    .await?
    .map_err(|error| {
        if error.is::<crate::library_view::SnapshotChanged>() {
            (
                StatusCode::CONFLICT,
                Json(json!({"error":"library_snapshot_changed"})),
            )
        } else if let Some(b) = error.downcast_ref::<crate::library_view::ViewBudget>() {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"library_view_budget","resource":b.0})),
            )
        } else {
            library_error(error, StatusCode::NOT_FOUND)
        }
    })?;
    Ok(Json(value))
}
async fn read_library_head(
    State(a): State<Shared>,
    Path(key): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let publisher = id(&key).map_err(|_| bad("invalid account key"))?;
    let head = a
        .repo
        .current_library_head(publisher)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error":"no_library_head"})),
            )
        })?;
    Ok(Json(
        json!({"event":&*head.event,"projected":head.projected}),
    ))
}
async fn read_library_object(
    State(a): State<Shared>,
    Path(hash): Path<String>,
) -> Result<axum::response::Response, ApiError> {
    let hash = id(&hash).map_err(|_| bad("invalid object hash"))?;
    let library = a.library_objects.clone().ok_or_else(unavailable)?;
    let size = library
        .public_sizes(&[hash])
        .await
        .map_err(|e| library_error(e, StatusCode::NOT_FOUND))?[&hash];
    let lease = library
        .response_bytes
        .clone()
        .try_acquire_many_owned(size as u32 + 16_384)
        .map_err(|e| library_error(e.into(), StatusCode::TOO_MANY_REQUESTS))?;
    let bytes = library
        .public_object(hash)
        .await
        .map_err(|e| library_error(e, StatusCode::NOT_FOUND))?;
    Ok(crate::bounded_response::response(
        bytes,
        lease,
        "application/cbor",
        false,
        false,
    ))
}
async fn read_library_batch(
    State(a): State<Shared>,
    headers: HeaderMap,
    request: Request,
) -> Result<axum::response::Response, ApiError> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let library = a.library_objects.clone().ok_or_else(unavailable)?;
    let slot = library
        .reads
        .clone()
        .try_acquire_owned()
        .map_err(|e| library_error(e.into(), StatusCode::TOO_MANY_REQUESTS))?;
    let gzip = crate::library_wire::requested_gzip(
        headers.get("accept-encoding").and_then(|v| v.to_str().ok()),
    );
    let body = tokio::time::timeout(
        Duration::from_secs(5),
        axum::body::to_bytes(request.into_body(), 8192),
    )
    .await
    .map_err(|_| bad("read body timeout"))?
    .map_err(|_| {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error":"body_budget"})),
        )
    })?;
    let hashes = crate::library_wire::hashes(&body)
        .map_err(|e| library_error(e, StatusCode::BAD_REQUEST))?;
    let sizes = library
        .public_sizes(&hashes)
        .await
        .map_err(|e| library_error(e, StatusCode::NOT_FOUND))?;
    let mut bound = 128usize;
    for hash in &hashes {
        bound = bound
            .checked_add((sizes[hash] as usize).div_ceil(3) * 4 + 256)
            .ok_or_else(|| bad("response size overflow"))?;
    }
    if bound > 8 * 1024 * 1024 {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error":"batch_response_budget"})),
        ));
    }
    let mut lease = library
        .response_bytes
        .clone()
        .try_acquire_many_owned((2 * bound + 1_000_000) as u32)
        .map_err(|e| library_error(e.into(), StatusCode::TOO_MANY_REQUESTS))?;
    let (bytes, lease) = spawn_owned_scope(async move {
        let _slot = slot;
        let mut objects = Vec::with_capacity(hashes.len());
        for hash in hashes {
            let bytes = library.public_object(hash).await?;
            objects.push(json!({"hash":hex::encode(hash),"data":STANDARD.encode(bytes)}));
        }
        spawn_blocking_owned_scope(move || -> Result<_> {
            let mut bytes = Vec::with_capacity(bound);
            serde_json::to_writer(&mut bytes, &json!({"objects":objects}))?;
            ensure!(bytes.len() <= bound, "response encoding budget");
            if gzip {
                bytes = crate::library_wire::compress(bytes)?;
            }
            ensure!(
                bytes.len() <= bound + 1_000_000,
                "compressed response budget"
            );
            // Keep enough output credit including bounded gzip framing overhead.
            bytes.shrink_to_fit();
            let output = lease
                .split(bytes.capacity())
                .context("response byte lease")?;
            drop(lease);
            Ok((bytes, output))
        })
        .await?
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|e| library_error(e, StatusCode::NOT_FOUND))?;
    Ok(crate::bounded_response::response(
        bytes,
        lease,
        "application/json",
        false,
        gzip,
    ))
}

async fn submit_social(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if !a.projection_enabled {
        return Err(unavailable());
    }
    let (authorization, authority) = account_preflight(&a, &uri, &headers, "POST").await?;
    let _slot = a.event_slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"event_slots_busy"})),
        )
    })?;
    let body = group_body(&a, &uri, &authorization, request, 131072).await?;
    let event: auth::Event =
        serde_json::from_slice(&body).map_err(|_| bad("invalid signed social event"))?;
    let inserted = a
        .repo
        .stage_social(&authority, &event)
        .await
        .map_err(|error| {
            tracing::debug!(error=%error,"social event rejected");
            validation_error(error, "invalid social event, target or account permission")
        })?;
    let projected = a
        .repo
        .projection_complete(id(&event.id).map_err(|_| bad("invalid event ID"))?)
        .await
        .map_err(|_| unavailable())?;
    Ok((
        if projected {
            StatusCode::OK
        } else {
            StatusCode::ACCEPTED
        },
        Json(json!({"id":event.id,"inserted":inserted,"projected":projected,"public":true})),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SocialQuery {
    target: String,
    limit: Option<i64>,
    offset: Option<i64>,
    since: Option<i64>,
    until: Option<i64>,
}
async fn query_social(
    State(a): State<Shared>,
    Query(q): Query<SocialQuery>,
) -> Result<Json<Value>, ApiError> {
    let events = a
        .repo
        .social_events(
            id(&q.target).map_err(|_| bad("invalid target"))?,
            q.limit.unwrap_or(50),
            q.offset.unwrap_or(0),
            q.since.unwrap_or(0),
            q.until.unwrap_or(i64::MAX),
        )
        .await
        .map_err(|_| bad("social query failed or outside bounds"))?;
    Ok(Json(
        json!({"events":events,"order":"created_at_desc_id_asc","public":true}),
    ))
}

async fn delete_account_events(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if !a.projection_enabled {
        return Err(unavailable());
    }
    let (authorization, authority) = account_preflight(&a, &uri, &headers, "POST").await?;
    let _slot = a.event_slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"event_slots_busy"})),
        )
    })?;
    let body = group_body(&a, &uri, &authorization, request, 131072).await?;
    let event: auth::Event =
        serde_json::from_slice(&body).map_err(|_| bad("signed deletion request required"))?;
    let changed = a
        .repo
        .delete_ordinary(&authority, &event)
        .await
        .map_err(|e| {
            tracing::debug!(error=%e,"account deletion rejected");
            validation_error(e, "invalid deletion request or changed account permission")
        })?;
    Ok((
        StatusCode::ACCEPTED,
        Json(
            json!({"id":event.id,"ordinary_withdrawals":changed,"protected_publications_require_local_policy":true,"public_request":true}),
        ),
    ))
}
async fn read_account_deletion(
    State(a): State<Shared>,
    Path(hash): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let request = a
        .repo
        .ordinary_deletion_request(id(&hash).map_err(|_| bad("invalid request ID"))?)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error":"request_unavailable"})),
            )
        })?;
    Ok(Json(request))
}

async fn peer_library_head(
    State(a): State<Shared>,
    Path(key): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    peer_authorized(&a, &uri, &headers).await?;
    read_library_head(State(a), Path(key)).await
}
async fn peer_library_blob(
    State(a): State<Shared>,
    Path(hash): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    peer_authorized(&a, &uri, &headers).await?;
    read_library_object(State(a), Path(hash)).await
}
async fn sync_peer_library(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<crate::peers::LibrarySyncResult>), ApiError> {
    let value = operator_json(&a, &uri, &headers, request).await?;
    let request: crate::peers::LibrarySyncRequest =
        serde_json::from_value(value).map_err(|_| bad("peer and publisher required"))?;
    let sync = a.peer_sync.clone().ok_or_else(unavailable)?;
    let library = a.library_objects.clone().ok_or_else(unavailable)?;
    let permit = sync.slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"sync_slots_busy"})),
        )
    })?;
    let result = spawn_owned_scope(async move {
        let _permit = permit;
        tokio::time::timeout(
            Duration::from_secs(600),
            sync.synchronize_library(library, request),
        )
        .await?
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|e| {
        if let Some(response) = sync_rejection_error(&e) {
            return response;
        }
        tracing::warn!(error=%e,"peer library transfer failed");
        bad("peer library transfer failed verification, approval or completeness")
    })?;
    Ok((
        if result.projected {
            StatusCode::OK
        } else {
            StatusCode::ACCEPTED
        },
        Json(result),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuditRequest {
    limit: Option<usize>,
}
async fn audit_storage(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<crate::upload::AuditResult>, ApiError> {
    let value = operator_json(&a, &uri, &headers, request).await?;
    let request: AuditRequest =
        serde_json::from_value(value).map_err(|_| bad("invalid audit request"))?;
    let store = a.storage.clone().ok_or_else(unavailable)?;
    Ok(Json(
        store
            .audit_page(request.limit.unwrap_or(32))
            .await
            .map_err(|e| {
                tracing::warn!(error=%e,"storage audit failed");
                bad("audit failed or invalid page bounds")
            })?,
    ))
}
async fn repair_storage(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let value = operator_json(&a, &uri, &headers, request).await?;
    let request: crate::peers::RepairRequest =
        serde_json::from_value(value).map_err(|_| bad("peer and hash required"))?;
    let sync = a.peer_sync.clone().ok_or_else(unavailable)?;
    let permit = sync.slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"sync_slots_busy"})),
        )
    })?;
    let repaired = spawn_owned_scope(async move {
        let _permit = permit;
        sync.repair_original(request).await
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|e| {
        tracing::warn!(error=%e,"verified repair failed");
        bad("repair failed verification, approval or generation check")
    })?;
    Ok(Json(
        json!({"repaired":repaired,"accounting_preserved":true}),
    ))
}

async fn peer_account_event(
    State(a): State<Shared>,
    Path(hash): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    peer_authorized(&a, &uri, &headers).await?;
    let key = id(&hash).map_err(|_| bad("invalid event ID"))?;
    if let Some(request) = a
        .repo
        .ordinary_deletion_request(key)
        .await
        .map_err(|_| unavailable())?
    {
        return Ok(Json(request));
    }
    let event = a
        .repo
        .visible_event(key)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error":"account_event_unavailable"})),
            )
        })?;
    if !matches!(event.kind, 0 | 1 | 3 | 7 | 1111 | 10002) {
        return Err(bad("event requires another transfer workflow"));
    }
    if !a
        .repo
        .projection_complete(key)
        .await
        .map_err(|_| unavailable())?
    {
        return Err(unavailable());
    }
    Ok(Json(
        serde_json::to_value(&*event).map_err(|_| unavailable())?,
    ))
}
async fn sync_peer_account(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let value = operator_json(&a, &uri, &headers, request).await?;
    let request: crate::peers::AccountSyncRequest =
        serde_json::from_value(value).map_err(|_| bad("peer and event required"))?;
    let sync = a.peer_sync.clone().ok_or_else(unavailable)?;
    let permit = sync.slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"sync_slots_busy"})),
        )
    })?;
    let result = spawn_owned_scope(async move {
        let _permit = permit;
        sync.synchronize_account_event(request).await
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|e| {
        if let Some(response) = sync_rejection_error(&e) {
            return response;
        }
        tracing::warn!(error=%e,"account event transfer rejected");
        bad("account transfer failed verification, approval or target policy")
    })?;
    Ok((StatusCode::ACCEPTED, Json(result)))
}

// Approved peers can recover an author's identity from a recently retained
// signed pointer. This never authorizes old-root or old-chunk serving.
async fn peer_library_routing(
    State(a): State<Shared>,
    Path(hash): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Json<auth::Event>, ApiError> {
    peer_authorized(&a, &uri, &headers).await?;
    let key = id(&hash).map_err(|_| bad("invalid pointer ID"))?;
    let row = a
        .repo
        .pool
        .get()
        .await
        .map_err(|_| unavailable())?
        .query_opt(
            "SELECT raw_event FROM events WHERE id=$1 AND kind=11999",
            &[&key.as_slice()],
        )
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error":"routing_pointer_pruned_or_unknown"})),
            )
        })?;
    let event: auth::Event = serde_json::from_value(row.get(0)).map_err(|_| unavailable())?;
    // Routing identifies an author; it must not depend on the local root codec.
    // The receiving node classifies the pointer version before fetching objects.
    event.verify().map_err(|_| unavailable())?;
    if event.kind != 11999 || event.id != hash {
        return Err(unavailable());
    }
    Ok(Json(event))
}

async fn save_sync_schedule(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let value = operator_json(&a, &uri, &headers, request).await?;
    if a.peer_sync.is_none() {
        return Err(unavailable());
    }
    let request: crate::sync_schedules::ScheduleSpec = serde_json::from_value(value)
        .map_err(|_| bad("spec,interval_seconds and explicit enabled required"))?;
    let id = a
        .repo
        .save_sync_schedule(request)
        .await
        .map_err(|_| bad("schedule scope,approval or budget rejected"))?;
    Ok(Json(
        json!({"id":hex::encode(id),"existing_batches_are_not_cancelled":true,"scheduled_history_limit":512}),
    ))
}
async fn list_sync_schedules(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let auth = operator_preflight(&a, &uri, &headers, "GET")?;
    consume_authorization(&a, &auth).await?;
    Ok(Json(
        json!({"schedules":a.repo.sync_schedules().await.map_err(|_|unavailable())?}),
    ))
}
async fn enable_sync_schedule(
    State(a): State<Shared>,
    Path(hash): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let value = operator_json(&a, &uri, &headers, request).await?;
    let body: GroupEnabled =
        serde_json::from_value(value).map_err(|_| bad("explicit enabled boolean required"))?;
    a.repo
        .enable_sync_schedule(
            id(&hash).map_err(|_| bad("invalid schedule ID"))?,
            body.enabled,
        )
        .await
        .map_err(|_| bad("schedule missing,approval or budget rejected"))?;
    Ok(Json(
        json!({"id":hash,"enabled":body.enabled,"existing_batches_are_not_cancelled":true}),
    ))
}

async fn federation_response_proof(
    State(a): State<Shared>,
    request: Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, ApiError> {
    let route = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|p| p.as_str())
        .unwrap_or("");
    let federation = route == "/reconcile" || route.starts_with("/federation/");
    let addressed = (request.method() == axum::http::Method::GET
        || request.method() == axum::http::Method::HEAD)
        && (route.contains("/blobs/")
            || matches!(
                route,
                "/blobs/{hash}"
                    | "/originals/{hash}"
                    | "/library/staging/{hash}"
                    | "/{blob}"
                    | "/compat/mangadex/data/{event}/{file}"
                    | "/compat/mangadex/data-saver/{event}/{file}"
                    | "/compat/mangadex/covers/{key}/{file}"
            ))
        || route == "/library/read";
    if addressed || request.method() == axum::http::Method::HEAD {
        return Ok(next.run(request).await);
    }
    let authorization = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .filter(|s| s.len() <= 16384)
        .and_then(|s| auth::token(s).ok());
    let Some(authorization) = authorization else {
        return Ok(next.run(request).await);
    };
    // Federation read admission is bounded before work. Client write replies use
    // a separate post-handler budget, so signing does not serialize save admission.
    let early = if federation {
        Some(
            a.proof_slots
                .clone()
                .try_acquire_owned()
                .map_err(|_| unavailable())?,
        )
    } else {
        None
    };
    let url = format!("{}{}", a.base_url, request.uri());
    let method = request.method().to_string();
    let response = next.run(request).await;
    let lease = match early {
        Some(lease) => lease,
        None => a
            .client_proof_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| unavailable())?,
    };
    let (mut parts, body) = response.into_parts();
    let bytes = tokio::time::timeout(
        Duration::from_secs(15),
        axum::body::to_bytes(
            body,
            if federation {
                crate::service_proof::LIMIT
            } else {
                4 * 1024 * 1024
            },
        ),
    )
    .await
    .map_err(|_| unavailable())?
    .map_err(|_| unavailable())?;
    let proof = crate::service_proof::sign(
        &a.signer.secret,
        &crate::service_proof::Scope {
            url: &url,
            method: &method,
            request: &authorization.id,
            status: parts.status.as_u16(),
        },
        &bytes,
    )
    .map_err(|_| unavailable())?;
    parts.headers.insert(
        crate::service_proof::HEADER,
        proof.parse().map_err(|_| unavailable())?,
    );
    parts
        .headers
        .insert(axum::http::header::CONTENT_LENGTH, bytes.len().into());
    parts.headers.insert(
        axum::http::header::CACHE_CONTROL,
        "private, no-store".parse().unwrap(),
    );
    let bounded =
        crate::bounded_response::response(bytes.to_vec(), lease, "application/json", true, false);
    Ok(axum::response::Response::from_parts(
        parts,
        bounded.into_body(),
    ))
}

#[cfg(test)]
mod injected_worker_owner_tests {
    use super::{EmbeddedOperations, EmbeddedWorkerOwner, OWNED_OPERATIONS, spawn_owned_scope};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    #[tokio::test]
    async fn disconnected_owned_work_and_nested_children_remain_in_drain_until_terminal() {
        let operations = EmbeddedOperations::new(true);
        let parent = operations.acquire().unwrap().unwrap();
        let (started, ready) = tokio::sync::oneshot::channel();
        let (finish, released) = tokio::sync::oneshot::channel();
        let task = OWNED_OPERATIONS
            .scope(operations.clone(), async {
                spawn_owned_scope(async move {
                    let _ = started.send(());
                    let _ = released.await;
                })
            })
            .await;
        ready.await.unwrap();
        drop(task); // A disconnected caller loses only its JoinHandle.
        drop(parent);
        operations.close();
        assert!(operations.acquire().is_err());
        assert!(
            operations
                .drain(std::time::Duration::from_millis(20))
                .await
                .is_err()
        );
        finish.send(()).unwrap();
        operations
            .drain(std::time::Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(operations.state.lock().unwrap().active, 0);
    }
    #[tokio::test]
    async fn panic_drops_owned_operation_lease_without_a_handle_cycle() {
        let operations = EmbeddedOperations::new(true);
        let parent = operations.acquire().unwrap().unwrap();
        let task = OWNED_OPERATIONS
            .scope(operations.clone(), async {
                spawn_owned_scope(async {
                    panic!("Synthetic owned operation panic");
                })
            })
            .await;
        assert!(task.await.unwrap_err().is_panic());
        drop(parent);
        operations
            .drain(std::time::Duration::from_secs(1))
            .await
            .unwrap();
        let standalone = EmbeddedOperations::new(false);
        standalone.close();
        assert!(standalone.acquire().unwrap().is_none());
    }
    #[tokio::test]
    async fn injected_shutdown_observes_actual_worker_termination() {
        struct Finished(Arc<AtomicBool>);
        impl Drop for Finished {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let finished = Arc::new(AtomicBool::new(false));
        let worker_finished = finished.clone();
        let (ready, started) = tokio::sync::oneshot::channel();
        let owner = EmbeddedWorkerOwner::new(true);
        let handle = owner.spawn("synthetic_owned_worker", async move {
            let _finished = Finished(worker_finished);
            let _ = ready.send(());
            std::future::pending::<()>().await;
        });
        started.await.unwrap();
        owner.shutdown().await.unwrap();
        assert!(handle.is_finished());
        assert!(finished.load(Ordering::Acquire));
        assert_eq!(
            owner.registry.snapshot()["workers"][0]["state"],
            "cancelled"
        );
    }
    #[tokio::test]
    async fn cancelled_background_and_async_parent_do_not_hide_running_blocking_child() {
        let owner = EmbeddedWorkerOwner::new(true);
        let (ready, started) = tokio::sync::oneshot::channel();
        let (release, released) = std::sync::mpsc::channel::<()>();
        let (child_ready, child_started) = tokio::sync::oneshot::channel();
        let terminal = Arc::new(AtomicBool::new(false));
        let child_terminal = terminal.clone();
        owner.spawn("synthetic_storage_audit", async move {
            let child = spawn_owned_scope(async move {
                super::spawn_blocking_owned_scope(move || {
                    let _ = ready.send(());
                    released.recv().unwrap();
                    child_terminal.store(true, Ordering::Release);
                })
                .await
                .unwrap();
            });
            let _ = child_ready.send(child.abort_handle());
            let _ = child.await;
        });
        let child = child_started.await.unwrap();
        started.await.unwrap();
        owner.shutdown().await.unwrap(); // actual audit parent has terminated
        child.abort(); // cancelling an await cannot cancel already-running blocking work
        assert!(
            owner
                .operations
                .drain(std::time::Duration::from_millis(20))
                .await
                .is_err()
        );
        assert!(!terminal.load(Ordering::Acquire));
        release.send(()).unwrap();
        owner
            .operations
            .drain(std::time::Duration::from_secs(1))
            .await
            .unwrap();
        assert!(terminal.load(Ordering::Acquire));
        assert_eq!(owner.operations.state.lock().unwrap().active, 0);
    }
    #[tokio::test]
    async fn infrastructure_parent_closes_its_idle_descendant_before_terminal_drain() {
        let owner = EmbeddedWorkerOwner::new(true);
        let (ready, started) = tokio::sync::oneshot::channel();
        owner.spawn("relay_notifications", async move {
            let child = super::spawn_infrastructure_owned_scope(std::future::pending::<()>());
            let _child = super::AbortChildOnDrop(child.abort_handle());
            let _ = ready.send(());
            std::future::pending::<()>().await;
        });
        started.await.unwrap();
        assert_eq!(owner.operations.state.lock().unwrap().infrastructure, 1);
        owner.finish(None).await.unwrap();
        let state = owner.operations.state.lock().unwrap();
        assert_eq!(state.active, 0);
        assert_eq!(state.infrastructure, 0);
    }
    #[tokio::test]
    async fn unconfirmed_process_cleanup_cannot_be_hidden_by_rust_task_completion() {
        let operations = EmbeddedOperations::new(true);
        OWNED_OPERATIONS
            .scope(operations.clone(), async {
                super::owned_terminal_unconfirmed();
            })
            .await;
        assert!(operations.acquire().is_err());
        assert_eq!(operations.state.lock().unwrap().active, 0);
        assert!(
            operations
                .drain(std::time::Duration::from_secs(1))
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn idle_session_shutdown_signal_and_already_closed_scope_both_terminate() {
        let operations = EmbeddedOperations::new(true);
        let (ready, started) = tokio::sync::oneshot::channel();
        let task = OWNED_OPERATIONS
            .scope(operations.clone(), async {
                spawn_owned_scope(async move {
                    let _ = ready.send(());
                    super::embedded_shutdown().await;
                })
            })
            .await;
        started.await.unwrap();
        operations.close();
        tokio::time::timeout(std::time::Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            OWNED_OPERATIONS.scope(operations.clone(), super::embedded_shutdown()),
        )
        .await
        .unwrap();
        operations
            .drain(std::time::Duration::from_secs(1))
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn injected_worker_failure_is_observed_and_standalone_default_is_preserved() {
        let owner = EmbeddedWorkerOwner::new(true);
        owner.spawn("synthetic_unexpected_return", async {});
        tokio::time::timeout(std::time::Duration::from_secs(2), owner.unexpected_exit())
            .await
            .unwrap();
        owner.shutdown().await.unwrap();
        assert_eq!(owner.registry.snapshot()["workers"][0]["state"], "returned");
        let standalone = EmbeddedWorkerOwner::new(false);
        let handle = standalone.spawn("synthetic_standalone_worker", std::future::pending());
        standalone.shutdown().await.unwrap();
        assert!(!handle.is_finished());
        assert_eq!(
            standalone.registry.snapshot()["workers"][0]["state"],
            "running"
        );
        handle.abort();
    }
}
