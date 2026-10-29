use super::{
    crypto,
    principal::{self, SessionPrincipal},
};
use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_postgres::{GenericClient, Row};

pub(super) const BROWSER_COOKIE: &str = "weebdex_browser";
const FLOW_LIFETIME: i64 = 600;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/self-service/logout/browser", get(create_logout))
        .route("/self-service/logout", get(update_logout).post(update_logout_post))
        .route("/self-service/errors", get(flow_error))
        .route("/self-service/{kind}/browser", get(create))
        .route("/self-service/{kind}/flows", get(read))
        .route(
            "/self-service/recovery",
            get(super::mail::recovery).post(update_recovery),
        )
        .route(
            "/self-service/verification",
            get(super::mail::verification).post(update_verification),
        )
        .route("/self-service/{kind}", axum::routing::post(update))
        .route("/sessions/whoami", get(whoami))
}

pub(super) struct FlowError(ApiError);
impl From<ApiError> for FlowError {
    fn from(e: ApiError) -> Self {
        Self(e)
    }
}
impl From<tokio_postgres::Error> for FlowError {
    fn from(e: tokio_postgres::Error) -> Self {
        Self(e.into())
    }
}
impl IntoResponse for FlowError {
    fn into_response(self) -> Response {
        let e = self.0;
        let mut response = reply(
            e.status,
            json!({"error":{"id":e.code,"code":e.status.as_u16(),"message":e.message,"reason":e.message}}),
        );
        if e.status == StatusCode::TOO_MANY_REQUESTS {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("60"));
        }
        response
    }
}

#[derive(Deserialize, Default)]
struct FlowQuery {
    return_to: Option<String>,
    refresh: Option<bool>,
    id: Option<String>,
    flow: Option<String>,
    token: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateBody {
    method: String,
    csrf_token: String,
    identifier: Option<String>,
    password: Option<String>,
    traits: Option<Traits>,
    email: Option<String>,
    provider: Option<String>,
    link: Option<String>,
    unlink: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Traits {
    email: String,
    username: String,
}

pub(super) struct Flow {
    pub(super) id: String,
    pub(super) kind: String,
    browser_hash: Vec<u8>,
    csrf_token: String,
    session_id: Option<String>,
    pub(super) return_to: String,
    issued_at: String,
    expires_at: String,
    expired: bool,
    used: bool,
    attempts: i32,
}

const FLOW_SELECT: &str = "SELECT id,kind,browser_hash,csrf_token,session_id,return_to,to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),to_char(expires_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),expires_at<=clock_timestamp(),used_at IS NOT NULL,attempts FROM platform.auth_flows WHERE id=$1";
fn flow_row(row: Row) -> Flow {
    Flow {
        id: row.get(0),
        kind: row.get(1),
        browser_hash: row.get(2),
        csrf_token: row.get(3),
        session_id: row.get(4),
        return_to: row.get(5),
        issued_at: row.get(6),
        expires_at: row.get(7),
        expired: row.get(8),
        used: row.get(9),
        attempts: row.get(10),
    }
}

fn reply(status: StatusCode, body: Value) -> Response {
    let mut response = (status, Json(body)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

pub(super) fn set_cookie(
    response: &mut Response,
    state: &AppState,
    name: &str,
    value: &str,
    lifetime: u64,
) -> ApiResult<()> {
    let secure = if state.config.public_origin.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };
    let value =
        format!("{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={lifetime}{secure}");
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&value).map_err(|_| ApiError::unavailable())?,
    );
    Ok(())
}

pub(super) fn safe_return_to(origin: &str, target: Option<&str>) -> ApiResult<String> {
    let target = target.unwrap_or("/");
    if target.len() > 2048
        || target.chars().any(|c| c.is_control() || c == '\\')
        || target.starts_with("//")
        || target.to_ascii_lowercase().contains("%5c")
    {
        return Err(ApiError::bad_request("Invalid return target"));
    }
    let origin = url::Url::parse(origin).map_err(|_| ApiError::unavailable())?;
    let result = origin
        .join(target)
        .map_err(|_| ApiError::bad_request("Invalid return target"))?;
    if result.origin() != origin.origin()
        || !result.username().is_empty()
        || result.password().is_some()
    {
        return Err(ApiError::bad_request("Return target must be on this site"));
    }
    Ok(result.into())
}

fn validate_kind(kind: &str) -> ApiResult<()> {
    if matches!(
        kind,
        "login" | "registration" | "recovery" | "verification" | "settings"
    ) {
        Ok(())
    } else {
        Err(ApiError::not_found("Unknown browser flow"))
    }
}

fn validate_creation_origin(state: &AppState, headers: &HeaderMap) -> ApiResult<()> {
    if headers.contains_key(header::ORIGIN)
        || headers.contains_key(header::REFERER)
        || headers.get("sec-fetch-site").and_then(|h| h.to_str().ok()) == Some("cross-site")
    {
        principal::validate_browser_mutation(&state.config.public_origin, headers)?;
    }
    Ok(())
}

pub(super) async fn throttle(
    state: &AppState,
    key: &str,
    limit: i32,
    seconds: i64,
) -> ApiResult<()> {
    let hash = crypto::digest(key);
    let db = state.db.client().await?;
    let row=db.query_one("INSERT INTO platform.auth_rate_limits(key_hash,window_ends,count) VALUES($1,clock_timestamp()+$2::BIGINT*interval '1 second',1) ON CONFLICT(key_hash) DO UPDATE SET count=CASE WHEN platform.auth_rate_limits.window_ends<=clock_timestamp() THEN 1 ELSE platform.auth_rate_limits.count+1 END,window_ends=CASE WHEN platform.auth_rate_limits.window_ends<=clock_timestamp() THEN clock_timestamp()+$2::BIGINT*interval '1 second' ELSE platform.auth_rate_limits.window_ends END RETURNING count",&[&hash,&seconds]).await?;
    let count: i32 = row.get(0);
    if count > limit {
        Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limit_exceeded",
            "Too many attempts; wait before trying again",
        ))
    } else {
        Ok(())
    }
}

async fn new_flow(
    state: &AppState,
    headers: &HeaderMap,
    kind: &str,
    target: Option<&str>,
    session: Option<&SessionPrincipal>,
) -> ApiResult<(Flow, Option<String>)> {
    let return_to = safe_return_to(&state.config.public_origin, target)?;
    let old_browser = principal::cookie(headers, BROWSER_COOKIE).filter(|s| {
        s.len() == 43
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    });
    let browser = old_browser.map(str::to_owned).unwrap_or_else(crypto::token);
    throttle(state, "global:flow-create", 300, 60).await?;
    throttle(
        state,
        &format!(
            "browser:flow-create:{}",
            crypto::digest(&browser)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        ),
        30,
        60,
    )
    .await?;
    let id = crypto::id();
    let csrf = crypto::token();
    let hash = crypto::digest(&browser);
    let session_id = session.map(|s| s.session_id.clone());
    let db = state.db.client().await?;
    // Bounded opportunistic cleanup; flow rows never carry session credentials.
    db.execute("DELETE FROM platform.auth_flows WHERE id IN (SELECT id FROM platform.auth_flows WHERE expires_at<clock_timestamp()-interval '1 hour' ORDER BY expires_at LIMIT 100)",&[]).await?;
    db.execute("DELETE FROM platform.auth_rate_limits WHERE key_hash IN (SELECT key_hash FROM platform.auth_rate_limits WHERE window_ends<clock_timestamp()-interval '1 hour' LIMIT 100)",&[]).await?;
    db.execute("INSERT INTO platform.auth_flows(id,kind,browser_hash,csrf_token,session_id,return_to,expires_at) VALUES($1,$2,$3,$4,$5,$6,clock_timestamp()+$7::BIGINT*interval '1 second')",&[&id,&kind,&hash,&csrf,&session_id,&return_to,&FLOW_LIFETIME]).await?;
    let flow = flow_row(db.query_one(FLOW_SELECT, &[&id]).await?);
    Ok((flow, old_browser.is_none().then_some(browser)))
}

fn ui_node(
    name: &str,
    input_type: &str,
    label: &str,
    group: &str,
    value: &str,
    required: bool,
) -> Value {
    json!({"type":"input","group":group,"attributes":{"name":name,"node_type":"input","type":input_type,"value":value,"required":required,"disabled":false,"autocomplete":match name {"identifier"|"traits.username"=>"username","traits.email"|"email"=>"email","password"=>if group=="password" {"current-password"} else {"new-password"},_=>"off"}},"messages":[],"meta":{"label":{"id":1070000,"text":label,"type":"info"}}})
}

fn flow_body(flow: &Flow, principal: Option<&SessionPrincipal>) -> Value {
    let mut nodes = vec![ui_node(
        "csrf_token",
        "hidden",
        "CSRF token",
        "default",
        &flow.csrf_token,
        true,
    )];
    match flow.kind.as_str() {
        "login" => {
            nodes.push(ui_node(
                "identifier",
                "text",
                "Email or username",
                "password",
                "",
                true,
            ));
            nodes.push(ui_node(
                "password", "password", "Password", "password", "", true,
            ));
        }
        "registration" => {
            nodes.push(ui_node(
                "traits.email",
                "email",
                "Email",
                "default",
                "",
                true,
            ));
            nodes.push(ui_node(
                "traits.username",
                "text",
                "Username",
                "default",
                "",
                true,
            ));
            nodes.push(ui_node(
                "password", "password", "Password", "password", "", true,
            ));
        }
        "settings" => {
            if !principal.is_some_and(|p| p.recovery_session) {
                nodes.push(ui_node(
                    "traits.email",
                    "email",
                    "Email",
                    "profile",
                    principal.map(|p| p.email.as_str()).unwrap_or(""),
                    true,
                ));
                nodes.push(ui_node(
                    "traits.username",
                    "text",
                    "Username",
                    "profile",
                    principal.map(|p| p.username.as_str()).unwrap_or(""),
                    true,
                ));
            }
            nodes.push(ui_node(
                "password",
                "password",
                "New password",
                "password",
                "",
                true,
            ));
        }
        "recovery" | "verification" => nodes.push(ui_node(
            "email",
            "email",
            "Email",
            "link",
            principal.map(|p| p.email.as_str()).unwrap_or(""),
            true,
        )),
        _ => {}
    }
    json!({"id":flow.id,"type":"browser","issued_at":flow.issued_at,"expires_at":flow.expires_at,"return_to":flow.return_to,"state":"choose_method","ui":{"action":format!("/auth/self-service/{}?flow={}",flow.kind,flow.id),"method":"POST","nodes":nodes,"messages":[]}})
}

fn flow_problem(
    flow: &Flow,
    principal: Option<&SessionPrincipal>,
    status: StatusCode,
    field: Option<&str>,
    text: &str,
) -> Response {
    let mut body = flow_body(flow, principal);
    let message = json!({"id":4000000,"text":text,"type":"error"});
    body["ui"]["messages"] = json!([message]);
    if let Some(field) = field {
        if let Some(nodes) = body["ui"]["nodes"].as_array_mut() {
            for node in nodes {
                if node["attributes"]["name"] == field {
                    node["messages"] = json!([message]);
                }
            }
        }
    }
    reply(status, body)
}

async fn create(
    State(state): State<AppState>,
    Path(kind): Path<String>,
    Query(query): Query<FlowQuery>,
    headers: HeaderMap,
) -> Result<Response, FlowError> {
    validate_kind(&kind)?;
    validate_creation_origin(&state, &headers)?;
    let current = principal::optional_principal(&state, &headers).await?;
    if current.as_ref().is_some_and(|p| p.recovery_session) && kind != "settings" {
        return Err(ApiError::forbidden("Recovery sessions may only set a new password").into());
    }
    if current.as_ref().is_some_and(|p| p.is_api_client) {
        return Err(ApiError::forbidden("Browser flows require a browser session").into());
    }
    if kind == "settings" && current.is_none() {
        return Err(ApiError::unauthorized().into());
    }
    if current.is_some()
        && (kind == "registration"
            || kind == "recovery"
            || kind == "login" && query.refresh != Some(true))
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "session_already_available",
            "A session is already available",
        )
        .into());
    }
    if kind == "registration" && !state.config.registration_enabled {
        return Err(
            ApiError::forbidden("Account registration is disabled on this instance").into(),
        );
    }
    let (flow, browser) = new_flow(
        &state,
        &headers,
        &kind,
        query.return_to.as_deref(),
        current.as_ref(),
    )
    .await?;
    let mut response = reply(StatusCode::OK, flow_body(&flow, current.as_ref()));
    if let Some(browser) = browser {
        set_cookie(
            &mut response,
            &state,
            BROWSER_COOKIE,
            &browser,
            state.config.session_lifetime_seconds,
        )?;
    }
    Ok(response)
}

async fn load<C: GenericClient + Sync>(
    db: &C,
    id: &str,
    kind: &str,
    headers: &HeaderMap,
    lock: bool,
) -> Result<Flow, FlowError> {
    if id.len() != 36 {
        return Err(ApiError::not_found("Browser flow not found").into());
    }
    let sql = if lock {
        format!("{FLOW_SELECT} FOR UPDATE")
    } else {
        FLOW_SELECT.into()
    };
    let flow = db
        .query_opt(&sql, &[&id])
        .await?
        .map(flow_row)
        .ok_or_else(|| ApiError::not_found("Browser flow not found"))?;
    let browser = principal::cookie(headers, BROWSER_COOKIE)
        .ok_or_else(|| ApiError::forbidden("Browser flow belongs to another browser"))?;
    if flow.kind != kind
        || !crypto::constant_time_equal(&flow.browser_hash, &crypto::digest(browser))
    {
        return Err(ApiError::forbidden("Browser flow belongs to another browser").into());
    }
    Ok(flow)
}

fn expired(flow: &Flow) -> Option<Response> {
    if flow.expired || flow.used || flow.attempts >= 10 {
        Some(reply(
            StatusCode::GONE,
            json!({"error":{"id":"self_service_flow_expired","message":"This browser flow has expired; start a new flow"},"return_to":flow.return_to}),
        ))
    } else {
        None
    }
}

async fn read(
    State(state): State<AppState>,
    Path(kind): Path<String>,
    Query(query): Query<FlowQuery>,
    headers: HeaderMap,
) -> Result<Response, FlowError> {
    validate_kind(&kind)?;
    validate_creation_origin(&state, &headers)?;
    let id = query
        .id
        .ok_or_else(|| ApiError::bad_request("Flow id is required"))?;
    let db = state.db.client().await?;
    let flow = load(&**db, &id, &kind, &headers, false).await?;
    if let Some(response) = expired(&flow) {
        return Ok(response);
    }
    let current = principal::optional_principal(&state, &headers).await?;
    if current.as_ref().is_some_and(|p| p.recovery_session) && kind != "settings" {
        return Err(ApiError::forbidden("Recovery sessions may only set a new password").into());
    }
    if kind == "settings"
        && (current.is_none()
            || flow.session_id.as_deref() != current.as_ref().map(|p| p.session_id.as_str()))
    {
        return Err(ApiError::unauthorized().into());
    }
    if current.as_ref().is_some_and(|p| p.is_api_client) {
        return Err(ApiError::forbidden("Browser flows require a browser session").into());
    }
    Ok(reply(StatusCode::OK, flow_body(&flow, current.as_ref())))
}

fn parse_body(headers: &HeaderMap, bytes: &[u8]) -> ApiResult<UpdateBody> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim();
    match content_type {
        "application/json" => serde_json::from_slice(bytes)
            .map_err(|_| ApiError::bad_request("Invalid browser flow payload")),
        "application/x-www-form-urlencoded" => {
            let values: Vec<(String, String)> = serde_urlencoded::from_bytes(bytes)
                .map_err(|_| ApiError::bad_request("Invalid browser flow payload"))?;
            let mut map = serde_json::Map::new();
            let mut traits = serde_json::Map::new();
            for (key, value) in values {
                let target = if let Some(key) = key.strip_prefix("traits.") {
                    (&mut traits, key.to_owned())
                } else {
                    (&mut map, key)
                };
                if target.0.insert(target.1, Value::String(value)).is_some() {
                    return Err(ApiError::bad_request("Duplicate browser flow field"));
                }
            }
            if !traits.is_empty() {
                map.insert("traits".into(), Value::Object(traits));
            }
            serde_json::from_value(Value::Object(map))
                .map_err(|_| ApiError::bad_request("Invalid browser flow payload"))
        }
        _ => Err(ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "invalid_content_type",
            "Use JSON or form data for browser flows",
        )),
    }
}

pub(super) fn valid_password(password: &str) -> bool {
    let count = password.chars().count();
    (8..=64).contains(&count) && password.len() <= 256 && !password.contains('\0')
}
pub(super) fn valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.rsplit_once('@') else {
        return false;
    };
    (3..=320).contains(&email.len())
        && !local.is_empty()
        && local.len() <= 64
        && !local.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && email
            .bytes()
            .all(|b| b.is_ascii() && !b.is_ascii_whitespace() && !b.is_ascii_control())
}
pub(super) fn valid_username(username: &str) -> bool {
    (3..=32).contains(&username.chars().count())
        && username.len() <= 128
        && username
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-'))
}

async fn update_recovery(
    State(state): State<AppState>,
    Query(query): Query<FlowQuery>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Result<Response, FlowError> {
    update(
        State(state),
        Path("recovery".into()),
        Query(query),
        headers,
        bytes,
    )
    .await
}
async fn update_verification(
    State(state): State<AppState>,
    Query(query): Query<FlowQuery>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Result<Response, FlowError> {
    update(
        State(state),
        Path("verification".into()),
        Query(query),
        headers,
        bytes,
    )
    .await
}

async fn update(
    State(state): State<AppState>,
    Path(kind): Path<String>,
    Query(query): Query<FlowQuery>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Result<Response, FlowError> {
    validate_kind(&kind)?;
    principal::validate_browser_mutation(&state.config.public_origin, &headers)?;
    let id = query
        .flow
        .ok_or_else(|| ApiError::bad_request("Flow id is required"))?;
    let mut body = parse_body(&headers, &bytes)?;
    let db = state.db.client().await?;
    let flow = load(&**db, &id, &kind, &headers, false).await?;
    drop(db);
    if let Some(response) = expired(&flow) {
        return Ok(response);
    }
    if !crypto::constant_time_equal(body.csrf_token.as_bytes(), flow.csrf_token.as_bytes()) {
        return Err(ApiError::forbidden("Invalid browser flow CSRF token").into());
    }
    let current = principal::optional_principal(&state, &headers).await?;
    if current.as_ref().is_some_and(|p| p.recovery_session) && kind != "settings" {
        return Err(ApiError::forbidden("Recovery sessions may only set a new password").into());
    }
    if current.as_ref().is_some_and(|p| p.is_api_client) {
        return Err(ApiError::forbidden("Browser flows cannot use API credentials").into());
    }
    if flow.session_id.as_deref() != current.as_ref().map(|p| p.session_id.as_str()) {
        return Err(ApiError::forbidden("Browser session changed; start a new flow").into());
    }
    throttle(&state, &format!("global:update:{kind}"), 128, 60).await?;
    throttle(
        &state,
        &format!("browser:update:{}", hex::encode(&flow.browser_hash)),
        30,
        60,
    )
    .await?;
    state
        .db
        .client()
        .await?
        .execute(
            "UPDATE platform.auth_flows SET attempts=attempts+1 WHERE id=$1 AND used_at IS NULL",
            &[&id],
        )
        .await?;
    if body.method == "oidc"
        || body.provider.is_some()
        || body.link.is_some()
        || body.unlink.is_some()
    {
        return Ok(flow_problem(
            &flow,
            current.as_ref(),
            StatusCode::BAD_REQUEST,
            None,
            "No OpenID Connect provider is configured on this instance",
        ));
    }
    match kind.as_str() {
        "login" => login(&state, &headers, flow, &mut body, current.as_ref()).await,
        "registration" => registration(&state, &headers, flow, &mut body).await,
        "settings" => {
            settings(
                &state,
                &headers,
                flow,
                &mut body,
                current.as_ref().ok_or_else(ApiError::unauthorized)?,
            )
            .await
        }
        "recovery" | "verification" => {
            if body.method != "link" {
                return Ok(flow_problem(
                    &flow,
                    current.as_ref(),
                    StatusCode::BAD_REQUEST,
                    None,
                    "Use the link method",
                ));
            }
            let email = body.email.as_deref().unwrap_or("").trim();
            if !valid_email(email) {
                return Ok(flow_problem(
                    &flow,
                    current.as_ref(),
                    StatusCode::BAD_REQUEST,
                    Some("email"),
                    "Enter a valid email address",
                ));
            }
            if let Err(error) = super::mail::send_link(&state, &flow, email).await {
                let mut response =
                    flow_problem(&flow, current.as_ref(), error.status, None, &error.message);
                if error.code == "email_provider_required" {
                    response.headers_mut().insert(
                        "x-weebdex-feature-state",
                        HeaderValue::from_static("email-provider-required"),
                    );
                }
                return Ok(response);
            }
            let mut result = flow_body(&flow, current.as_ref());
            result["state"] = json!("sent_email");
            result["ui"]["messages"] = json!([{"id":1060001,"text":"If an account is associated with that email, a one-time link has been sent. Check your inbox.","type":"info"}]);
            Ok(reply(StatusCode::OK, result))
        }
        _ => Err(ApiError::not_found("Unknown browser flow").into()),
    }
}

async fn issue_session<C: GenericClient + Sync>(
    db: &C,
    state: &AppState,
    user_id: &str,
    flow: &Flow,
    previous: Option<&SessionPrincipal>,
) -> ApiResult<(String, Value)> {
    let token = crypto::token();
    let token_hash = crypto::digest(&token);
    let session_id = crypto::id();
    let lifetime = state.config.session_lifetime_seconds as i64;
    if let Some(previous) = previous {
        db.execute(
            "UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE id=$1 AND user_id=$2",
            &[&previous.session_id, &user_id],
        )
        .await?;
    }
    db.execute("INSERT INTO platform.sessions(id,token_hash,user_id,expires_at) VALUES($1,$2,$3,clock_timestamp()+$4::BIGINT*interval '1 second')",&[&session_id,&token_hash,&user_id,&lifetime]).await?;
    db.execute(
        "UPDATE platform.auth_flows SET used_at=clock_timestamp() WHERE id=$1",
        &[&flow.id],
    )
    .await?;
    let row=db.query_one("SELECT u.username,u.email,u.verified,to_char(s.created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),to_char(s.expires_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') FROM platform.users u JOIN platform.sessions s ON s.user_id=u.id WHERE s.id=$1",&[&session_id]).await?;
    let body = json!({"session":{"id":session_id,"active":true,"issued_at":row.get::<_,String>(3),"expires_at":row.get::<_,String>(4),"authenticated_at":row.get::<_,String>(3),"authentication_methods":[{"method":"password","aal":"aal1"}],"authenticator_assurance_level":"aal1","identity":{"id":user_id,"schema_id":"default","state":"active","traits":{"email":row.get::<_,String>(1),"username":row.get::<_,String>(0)},"verifiable_addresses":[{"value":row.get::<_,String>(1),"verified":row.get::<_,bool>(2),"via":"email","status":if row.get::<_,bool>(2){"completed"}else{"pending"}}]}},"continue_with":[],"return_to":flow.return_to});
    Ok((token, body))
}

async fn login(
    state: &AppState,
    headers: &HeaderMap,
    flow: Flow,
    body: &mut UpdateBody,
    current: Option<&SessionPrincipal>,
) -> Result<Response, FlowError> {
    if body.method != "password" {
        return Ok(flow_problem(
            &flow,
            current,
            StatusCode::BAD_REQUEST,
            None,
            "Use the password method",
        ));
    }
    let identifier = body
        .identifier
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    let password = body.password.take().unwrap_or_default();
    if !(3..=320).contains(&identifier.len()) || password.is_empty() || password.len() > 256 {
        return Ok(flow_problem(
            &flow,
            current,
            StatusCode::BAD_REQUEST,
            None,
            "Enter your identifier and password",
        ));
    }
    throttle(state, &format!("login:identifier:{identifier}"), 8, 300).await?;
    let db = state.db.client().await?;
    let row=db.query_opt("SELECT id,password_hash,disabled FROM platform.users WHERE lower(email)=$1 OR lower(username)=$1",&[&identifier]).await?;
    let hash = row.as_ref().map(|r| r.get::<_, String>(1));
    let user_id = row.as_ref().map(|r| r.get::<_, String>(0));
    let disabled = row.as_ref().is_some_and(|r| r.get::<_, bool>(2));
    drop(db);
    if !crypto::verify_password(password, hash.clone()).await? || disabled {
        return Ok(flow_problem(
            &flow,
            current,
            StatusCode::BAD_REQUEST,
            None,
            "The identifier or password is incorrect",
        ));
    }
    let user_id = user_id.ok_or_else(ApiError::unauthorized)?;
    if current.is_some_and(|p| p.user_id != user_id) {
        return Ok(flow_problem(
            &flow,
            current,
            StatusCode::BAD_REQUEST,
            None,
            "Sign out before signing in to another account",
        ));
    }
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let locked = load(&*tx, &flow.id, "login", headers, true).await?;
    if let Some(response) = expired(&locked) {
        return Ok(response);
    }
    let user = tx
        .query_one(
            "SELECT password_hash,disabled FROM platform.users WHERE id=$1 FOR UPDATE",
            &[&user_id],
        )
        .await?;
    if user.get::<_, bool>(1) || Some(user.get::<_, String>(0)) != hash {
        return Err(ApiError::unauthorized().into());
    }
    let (token, result) = issue_session(&*tx, state, &user_id, &locked, current).await?;
    tx.commit().await?;
    let mut response = reply(StatusCode::OK, result);
    set_cookie(
        &mut response,
        state,
        &state.config.session_cookie_name,
        &token,
        state.config.session_lifetime_seconds,
    )?;
    Ok(response)
}

fn normal_permissions() -> Value {
    json!([
        "self.library.read",
        "self.user.update",
        "self.preferences.update",
        "client.create",
        "self.client.update",
        "self.client.delete",
        "manga.create",
        "author.create",
        "scanlation_group.create",
        "self.scanlation_group.update",
        "chapter.create",
        "self.chapter.update",
        "self.chapter.remove",
        "reply.create",
        "self.reply.update",
        "self.reply.remove",
        "report.create"
    ])
}

async fn registration(
    state: &AppState,
    headers: &HeaderMap,
    flow: Flow,
    body: &mut UpdateBody,
) -> Result<Response, FlowError> {
    if !state.config.registration_enabled {
        return Err(
            ApiError::forbidden("Account registration is disabled on this instance").into(),
        );
    }
    if body.method != "password" {
        return Ok(flow_problem(
            &flow,
            None,
            StatusCode::BAD_REQUEST,
            None,
            "Use the password method",
        ));
    }
    let Some(traits) = body.traits.take() else {
        return Ok(flow_problem(
            &flow,
            None,
            StatusCode::BAD_REQUEST,
            None,
            "Email and username are required",
        ));
    };
    let email = traits.email.trim().to_lowercase();
    let username = traits.username.trim().to_owned();
    let password = body.password.take().unwrap_or_default();
    if !valid_email(&email) {
        return Ok(flow_problem(
            &flow,
            None,
            StatusCode::BAD_REQUEST,
            Some("traits.email"),
            "Enter a valid email address",
        ));
    }
    if !valid_username(&username) {
        return Ok(flow_problem(
            &flow,
            None,
            StatusCode::BAD_REQUEST,
            Some("traits.username"),
            "Use 3 to 32 letters, digits, underscores or hyphens",
        ));
    }
    if !valid_password(&password) {
        return Ok(flow_problem(
            &flow,
            None,
            StatusCode::BAD_REQUEST,
            Some("password"),
            "Password must contain 8 to 64 characters",
        ));
    }
    let password_hash = crypto::hash_password(password).await?;
    let user_id = crypto::id();
    let permissions = normal_permissions();
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let locked = load(&*tx, &flow.id, "registration", headers, true).await?;
    if let Some(response) = expired(&locked) {
        return Ok(response);
    }
    let inserted=tx.execute("INSERT INTO platform.users(id,username,email,password_hash,permissions) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING",&[&user_id,&username,&email,&password_hash,&permissions]).await?;
    if inserted != 1 {
        return Ok(flow_problem(
            &flow,
            None,
            StatusCode::BAD_REQUEST,
            None,
            "That email or username is unavailable",
        ));
    }
    let (token, result) = issue_session(&*tx, state, &user_id, &locked, None).await?;
    tx.commit().await?;
    let mut response = reply(StatusCode::OK, result);
    set_cookie(
        &mut response,
        state,
        &state.config.session_cookie_name,
        &token,
        state.config.session_lifetime_seconds,
    )?;
    Ok(response)
}

async fn settings(
    state: &AppState,
    headers: &HeaderMap,
    flow: Flow,
    body: &mut UpdateBody,
    current: &SessionPrincipal,
) -> Result<Response, FlowError> {
    current.require_recent_browser_auth()?;
    if current.recovery_session && body.method != "password" {
        return Err(ApiError::forbidden("Recovery sessions may only set a new password").into());
    }
    let mut db = state.db.client().await?;
    // Password work happens before locking state; concurrent changes are still serialized below.
    let new_hash = if body.method == "password" {
        let password = body.password.take().unwrap_or_default();
        if !valid_password(&password) {
            return Ok(flow_problem(
                &flow,
                Some(current),
                StatusCode::BAD_REQUEST,
                Some("password"),
                "Password must contain 8 to 64 characters",
            ));
        }
        Some(crypto::hash_password(password).await?)
    } else {
        None
    };
    let tx = db.transaction().await?;
    let locked = load(&*tx, &flow.id, "settings", headers, true).await?;
    if let Some(response) = expired(&locked) {
        return Ok(response);
    }
    if locked.session_id.as_deref() != Some(&current.session_id) {
        return Err(ApiError::unauthorized().into());
    }
    // All credential rotation, recovery issuance and redemption serialize on
    // the account before taking token/session locks, avoiding stale reset links.
    if tx
        .query_opt(
            "SELECT id FROM platform.users WHERE id=$1 AND NOT disabled FOR UPDATE",
            &[&current.user_id],
        )
        .await?
        .is_none()
    {
        return Err(ApiError::unauthorized().into());
    }
    let row=tx.query_opt("SELECT id FROM platform.sessions WHERE id=$1 AND user_id=$2 AND revoked_at IS NULL AND expires_at>clock_timestamp() AND authenticated_at>clock_timestamp()-interval '10 minutes' FOR UPDATE",&[&current.session_id,&current.user_id]).await?;
    if row.is_none() {
        return Err(ApiError::unauthorized().into());
    }
    let mut updated = current.clone();
    match body.method.as_str() {
        "password" => {
            tx.execute("UPDATE platform.users SET password_hash=$2,updated_at=clock_timestamp(),version=version+1 WHERE id=$1",&[&current.user_id,&new_hash]).await?;
            tx.execute("UPDATE platform.auth_email_tokens SET used_at=clock_timestamp() WHERE user_id=$1 AND kind='recovery' AND used_at IS NULL",&[&current.user_id]).await?;
            tx.execute("UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE user_id=$1 AND id<>$2 AND revoked_at IS NULL",&[&current.user_id,&current.session_id]).await?;
            tx.execute("UPDATE platform.api_clients SET revoked_at=clock_timestamp(),updated_at=clock_timestamp(),version=version+1 WHERE user_id=$1 AND revoked_at IS NULL",&[&current.user_id]).await?;
        }
        "profile" => {
            let Some(traits) = body.traits.take() else {
                return Ok(flow_problem(
                    &flow,
                    Some(current),
                    StatusCode::BAD_REQUEST,
                    None,
                    "Email and username are required",
                ));
            };
            let email = traits.email.trim().to_lowercase();
            let username = traits.username.trim().to_owned();
            if !valid_email(&email) || !valid_username(&username) {
                return Ok(flow_problem(
                    &flow,
                    Some(current),
                    StatusCode::BAD_REQUEST,
                    None,
                    "Enter a valid email and username",
                ));
            }
            let duplicate=tx.query_opt("SELECT id FROM platform.users WHERE id<>$1 AND (lower(email)=$2 OR lower(username)=lower($3))",&[&current.user_id,&email,&username]).await?;
            if duplicate.is_some() {
                return Ok(flow_problem(
                    &flow,
                    Some(current),
                    StatusCode::BAD_REQUEST,
                    None,
                    "That email or username is unavailable",
                ));
            }
            let email_changed = email != current.email;
            let changed=tx.execute("UPDATE platform.users SET username=$2,email=$3,verified=CASE WHEN email<>$3 THEN false ELSE verified END,updated_at=clock_timestamp(),version=version+1 WHERE id=$1",&[&current.user_id,&username,&email]).await;
            match changed {
                Ok(_) => {}
                Err(error)
                    if error.code() == Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION) =>
                {
                    return Ok(flow_problem(
                        &flow,
                        Some(current),
                        StatusCode::BAD_REQUEST,
                        None,
                        "That email or username is unavailable",
                    ));
                }
                Err(error) => return Err(error.into()),
            }
            if email_changed {
                tx.execute("UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE user_id=$1 AND id<>$2 AND revoked_at IS NULL",&[&current.user_id,&current.session_id]).await?;
                tx.execute("UPDATE platform.api_clients SET revoked_at=clock_timestamp(),updated_at=clock_timestamp(),version=version+1 WHERE user_id=$1 AND revoked_at IS NULL",&[&current.user_id]).await?;
                updated.verified = false;
            }
            updated.email = email;
            updated.username = username;
        }
        _ => {
            return Ok(flow_problem(
                &flow,
                Some(current),
                StatusCode::BAD_REQUEST,
                None,
                "Use the profile or password method",
            ));
        }
    }
    // Consume the submitted flow and rotate the current session. A copied old
    // cookie must not survive a password/profile credential change. Preserve
    // authentication freshness rather than extending the reauthentication grant.
    let session_token = crypto::token();
    let session_hash = crypto::digest(&session_token);
    let session_id = crypto::id();
    let lifetime = state.config.session_lifetime_seconds as i64;
    tx.execute("INSERT INTO platform.sessions(id,token_hash,user_id,authenticated_at,expires_at) SELECT $1,$2,user_id,authenticated_at,clock_timestamp()+$4::BIGINT*interval '1 second' FROM platform.sessions WHERE id=$3",&[&session_id,&session_hash,&current.session_id,&lifetime]).await?;
    tx.execute(
        "UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE id=$1",
        &[&current.session_id],
    )
    .await?;
    tx.execute(
        "UPDATE platform.auth_flows SET used_at=clock_timestamp() WHERE id=$1",
        &[&flow.id],
    )
    .await?;
    updated.session_id = session_id.clone();
    let next_id = crypto::id();
    let csrf = crypto::token();
    tx.execute("INSERT INTO platform.auth_flows(id,kind,browser_hash,csrf_token,session_id,return_to,expires_at) VALUES($1,'settings',$2,$3,$4,$5,clock_timestamp()+$6::BIGINT*interval '1 second')",&[&next_id,&flow.browser_hash,&csrf,&session_id,&flow.return_to,&FLOW_LIFETIME]).await?;
    let next = flow_row(tx.query_one(FLOW_SELECT, &[&next_id]).await?);
    tx.commit().await?;
    let mut result = flow_body(&next, Some(&updated));
    result["state"] = json!("success");
    result["ui"]["messages"] =
        json!([{"id":1050001,"text":"Your changes have been saved","type":"success"}]);
    let mut response = reply(StatusCode::OK, result);
    set_cookie(
        &mut response,
        state,
        &state.config.session_cookie_name,
        &session_token,
        state.config.session_lifetime_seconds,
    )?;
    Ok(response)
}

async fn create_logout(
    State(state): State<AppState>,
    Query(query): Query<FlowQuery>,
    headers: HeaderMap,
) -> Result<Response, FlowError> {
    validate_creation_origin(&state, &headers)?;
    let current = principal::optional_principal(&state, &headers)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    if current.is_api_client {
        return Err(ApiError::forbidden("Browser logout requires a browser session").into());
    }
    let (flow, browser) = new_flow(
        &state,
        &headers,
        "logout",
        query.return_to.as_deref(),
        Some(&current),
    )
    .await?;
    let mut response = reply(
        StatusCode::OK,
        json!({"logout_token":flow.csrf_token,"logout_url":format!("{}/auth/self-service/logout?token={}",state.config.public_origin.trim_end_matches('/'),flow.csrf_token),"session_id":current.session_id,"account_id":current.user_id}),
    );
    if let Some(browser) = browser {
        set_cookie(
            &mut response,
            &state,
            BROWSER_COOKIE,
            &browser,
            state.config.session_lifetime_seconds,
        )?;
    }
    Ok(response)
}

async fn update_logout(
    State(state): State<AppState>,
    Query(query): Query<FlowQuery>,
    headers: HeaderMap,
) -> Result<Response, FlowError> {
    complete_logout(state, headers, query.token, None).await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LogoutCommand {
    token: String,
    expected_session_id: String,
}

async fn update_logout_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, FlowError> {
    // POST is needed when private-page no-referrer policy prevents the legacy
    // GET client from supplying a request source. Never weaken the origin check.
    principal::validate_browser_mutation(&state.config.public_origin, &headers)?;
    if body.len() > 1024 || headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).and_then(|v| v.split(';').next()) != Some("application/json") {
        return Err(ApiError::bad_request("A bounded JSON logout command is required").into());
    }
    let command: LogoutCommand = serde_json::from_slice(&body).map_err(|_| ApiError::bad_request("Invalid logout command"))?;
    if command.expected_session_id.is_empty() || command.expected_session_id.len() > 128 {
        return Err(ApiError::bad_request("The captured browser session is required").into());
    }
    complete_logout(state, headers, Some(command.token), Some(command.expected_session_id)).await
}

async fn complete_logout(
    state: AppState,
    headers: HeaderMap,
    token: Option<String>,
    expected_session_id: Option<String>,
) -> Result<Response, FlowError> {
    principal::validate_browser_mutation(&state.config.public_origin, &headers)?;
    let token = token
        .filter(|t| t.len() == 43)
        .ok_or_else(|| ApiError::bad_request("Logout token is required"))?;
    let current = principal::optional_principal(&state, &headers)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    if current.is_api_client {
        return Err(ApiError::forbidden("Browser logout requires a browser session").into());
    }
    if expected_session_id.as_deref().is_some_and(|expected| expected != current.session_id) {
        return Err(ApiError::conflict("The browser session changed; review sign out again").into());
    }
    let browser = principal::cookie(&headers, BROWSER_COOKIE)
        .ok_or_else(|| ApiError::forbidden("Logout flow belongs to another browser"))?;
    let hash = crypto::digest(browser);
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let row=tx.query_opt("SELECT id,return_to FROM platform.auth_flows WHERE kind='logout' AND csrf_token=$1 AND browser_hash=$2 AND session_id=$3 AND used_at IS NULL AND expires_at>clock_timestamp() FOR UPDATE",&[&token,&hash,&current.session_id]).await?.ok_or_else(||ApiError::forbidden("Logout token is invalid or expired"))?;
    let id: String = row.get(0);
    let target: String = row.get(1);
    tx.execute(
        "UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE id=$1 AND user_id=$2",
        &[&current.session_id, &current.user_id],
    )
    .await?;
    tx.execute(
        "UPDATE platform.auth_flows SET used_at=clock_timestamp() WHERE id=$1",
        &[&id],
    )
    .await?;
    tx.commit().await?;
    let mut response = reply(StatusCode::OK, json!({"redirect_browser_to":target}));
    set_cookie(
        &mut response,
        &state,
        &state.config.session_cookie_name,
        "",
        0,
    )?;
    Ok(response)
}

async fn whoami(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, FlowError> {
    let current = principal::optional_principal(&state, &headers)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    current.require_browser_session()?;
    Ok(reply(
        StatusCode::OK,
        json!({"id":current.session_id,"active":true,"identity":{"id":current.user_id,"schema_id":"default","traits":{"email":current.email,"username":current.username},"verifiable_addresses":[{"value":current.email,"verified":current.verified,"via":"email","status":if current.verified{"completed"}else{"pending"}}]}}),
    ))
}

async fn flow_error(
    State(state): State<AppState>,
    Query(query): Query<FlowQuery>,
    headers: HeaderMap,
) -> Result<Response, FlowError> {
    let id = query
        .id
        .ok_or_else(|| ApiError::bad_request("Flow error id is required"))?;
    let db = state.db.client().await?;
    let row = db
        .query_opt("SELECT kind FROM platform.auth_flows WHERE id=$1", &[&id])
        .await?
        .ok_or_else(|| ApiError::not_found("Flow error not found"))?;
    let kind: String = row.get(0);
    let flow = load(&**db, &id, &kind, &headers, false).await?;
    Ok(reply(
        StatusCode::OK,
        json!({"id":flow.id,"error":{"id":"browser_flow_error","message":if flow.expired||flow.used{"This browser flow has expired; start a new flow"}else{"The browser flow could not be completed"}}}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_settings_successor_matches_frontend_contract_fixture() {
        // No database or HTTP fixture: exercise the native response serializer
        // with a synthetic fresh settings flow and restricted principal. The
        // frontend executes this same fixture against its actual reset handler.
        let successor = Flow {
            id: "b".repeat(32),
            kind: "settings".into(),
            browser_hash: vec![0; 32],
            csrf_token: "synthetic-successor-csrf".into(),
            session_id: Some("synthetic-rotated-session".into()),
            return_to: "https://recovery.example.invalid/auth/recovery".into(),
            issued_at: "2026-10-14T00:00:00.000000Z".into(),
            expires_at: "2026-10-14T00:10:00.000000Z".into(),
            expired: false,
            used: false,
            attempts: 0,
        };
        let principal = SessionPrincipal {
            user_id: "synthetic-user".into(),
            username: "synthetic-user".into(),
            email: "synthetic@example.invalid".into(),
            verified: false,
            roles: vec![],
            permissions: vec![],
            session_id: "synthetic-rotated-session".into(),
            is_api_client: false,
            authenticated_age_seconds: 0,
            recovery_session: true,
        };
        let mut body = flow_body(&successor, Some(&principal));
        // These are the native settings handler's two acknowledgement fields.
        body["state"] = json!("success");
        body["ui"]["messages"] =
            json!([{"id":1050001,"text":"Your changes have been saved","type":"success"}]);
        let expected: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/recovery-settings-success.json"
        ))
        .unwrap();
        assert_eq!(body, expected);
        assert_ne!(body["id"], "a".repeat(32));
        assert!(body.get("identity").is_none());
    }
    #[test]
    fn return_targets_are_same_origin_and_normalized() {
        assert_eq!(
            safe_return_to("https://example.test", Some("/manga/one?a=1")).unwrap(),
            "https://example.test/manga/one?a=1"
        );
        for target in [
            "https://evil.test/",
            "//evil.test/",
            "/\\evil.test",
            "javascript:alert(1)",
            "https://example.test.evil.test/",
            "/a%5Cb",
        ] {
            assert!(
                safe_return_to("https://example.test", Some(target)).is_err(),
                "{target}"
            );
        }
    }
    #[test]
    fn browser_payload_and_field_validation() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            "application/x-www-form-urlencoded".parse().unwrap(),
        );
        let p=parse_body(&headers,b"method=password&csrf_token=abc&password=isolated-fixture&traits.email=fixture%40example.invalid&traits.username=fixture").unwrap();
        assert_eq!(p.traits.unwrap().username, "fixture");
        assert!(parse_body(&headers, b"method=password&method=oidc&csrf_token=abc").is_err());
        assert!(valid_email("fixture@example.invalid"));
        assert!(!valid_email("bad@email"));
        assert!(valid_username("test-user"));
        assert!(!valid_username("a"));
        assert!(valid_password("01234567"));
        assert!(!valid_password("short"));
    }
}
