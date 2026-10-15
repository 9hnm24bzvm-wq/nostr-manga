//! MangaDex-shaped private settings over the actual native preference row.
//! Nostr request admission plus an explicit instance-account binding supplies
//! authority; no session, public identity alias, or client-supplied owner does.
use super::{PublicApiError, PublicApiResult, PublicUuid, auth, settings_schema};
use crate::{domains::library::preference_core, state::AppState};
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{OriginalUri, Path, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use manga_node::http_auth::AdmittedHttpCapability;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;
use tokio_postgres::Transaction;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/settings/template",
            get(latest_template).post(create_template),
        )
        .route("/settings/template/{version}", get(version_template))
        .route("/settings", get(read_settings).post(write_settings))
        .method_not_allowed_fallback(|| async {
            PublicApiError::new(
                StatusCode::METHOD_NOT_ALLOWED,
                "Method Not Allowed",
                "Method is not supported for this settings endpoint",
            )
        })
}
fn reply(value: Value) -> Response {
    let mut response = Json(value).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
fn template_reply(value: Value, id: &str) -> Response {
    let mut response = reply(value);
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{id}\"")).expect("database UUID"),
    );
    response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&format!("/api/v1/settings/template/{id}")).expect("database UUID"),
    );
    response
}
async fn request(
    state: &AppState,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    request: Request,
    cap: usize,
) -> PublicApiResult<(AdmittedHttpCapability, Vec<u8>)> {
    if uri.query().is_some() {
        return Err(PublicApiError::bad_request(
            "This operation accepts no query parameters",
        ));
    }
    let method = request.method().as_str().to_owned();
    let body = tokio::time::timeout(Duration::from_secs(5), to_bytes(request.into_body(), cap))
        .await
        .map_err(|_| PublicApiError::bad_request("Request body timeout"))?
        .map_err(|_| {
            PublicApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "Payload Too Large",
                "Settings request exceeds its byte budget",
            )
        })?;
    let capability = auth::admitted_request(state, uri, headers, &method, &body).await?;
    if method == "GET" && !body.is_empty() {
        return Err(PublicApiError::bad_request(
            "This GET operation accepts no request body",
        ));
    }
    if method == "POST"
        && !headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| {
                v.split(';')
                    .next()
                    .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
            })
    {
        return Err(PublicApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Unsupported Media Type",
            "application/json is required",
        ));
    }
    Ok((capability, body.to_vec()))
}
async fn latest_template(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    req: Request,
) -> PublicApiResult<Response> {
    let (capability, _) = request(&state, &uri, &headers, req, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    auth::bound_account_read(&tx, &capability).await?;
    let row=tx.query_opt("SELECT t.id::text,t.document FROM platform.settings_template_head h JOIN platform.settings_templates t ON t.id=h.template_id WHERE h.singleton",&[]).await?.ok_or_else(PublicApiError::not_found)?;
    let id: String = row.get(0);
    let schema: Value = row.get(1);
    tx.commit().await?;
    Ok(template_reply(schema, &id))
}
async fn version_template(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(version): Path<String>,
    headers: HeaderMap,
    req: Request,
) -> PublicApiResult<Response> {
    let version = PublicUuid::parse(&version)?;
    let (capability, _) = request(&state, &uri, &headers, req, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    auth::bound_account_read(&tx, &capability).await?;
    let row = tx
        .query_opt(
            "SELECT document FROM platform.settings_templates WHERE id=$1::text::uuid",
            &[&version.as_str()],
        )
        .await?
        .ok_or_else(PublicApiError::not_found)?;
    let schema: Value = row.get(0);
    tx.commit().await?;
    Ok(template_reply(schema, version.as_str()))
}
fn fresh_uuid() -> String {
    let mut bytes = rand::random::<[u8; 16]>();
    bytes[6] = (bytes[6] & 15) | 64;
    bytes[8] = (bytes[8] & 63) | 128;
    let s = hex::encode(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &s[..8],
        &s[8..12],
        &s[12..16],
        &s[16..20],
        &s[20..]
    )
}
async fn create_template(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    req: Request,
) -> PublicApiResult<Response> {
    let (capability, body) =
        request(&state, &uri, &headers, req, settings_schema::TEMPLATE_BYTES).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = auth::bound_account_write(&tx, &capability).await?;
    if !actor.has_role("sysop") {
        return Err(PublicApiError::forbidden(
            "Settings template publication requires the instance sysop role",
        ));
    }
    actor.require_permission("settings.template.create")?;
    let schema: Value = serde_json::from_slice(&body)
        .map_err(|_| PublicApiError::bad_request("Malformed settings template JSON"))?;
    settings_schema::validate(schema.clone(), None).await?;
    tx.query_one(
        "SELECT template_id FROM platform.settings_template_head WHERE singleton FOR UPDATE",
        &[],
    )
    .await?;
    let count: i64 = tx
        .query_one("SELECT count(*) FROM platform.settings_templates", &[])
        .await?
        .get(0);
    if count >= 1024 {
        return Err(PublicApiError::conflict(
            "The instance settings template retention budget is full",
        ));
    }
    let id = fresh_uuid();
    tx.execute("INSERT INTO platform.settings_templates(id,document,created_by) VALUES($1::text::uuid,$2,$3)",&[&id,&schema,&actor.user_id()]).await?;
    tx.execute(
        "UPDATE platform.settings_template_head SET template_id=$1::text::uuid WHERE singleton",
        &[&id],
    )
    .await?;
    tx.commit().await?;
    Ok(template_reply(schema, &id))
}
async fn document(tx: &Transaction<'_>, user: &str) -> PublicApiResult<Value> {
    let row=tx.query_opt("SELECT preferences,preferences_template_id::text,to_char(preferences_updated_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') FROM platform.users WHERE id=$1",&[&user]).await?.ok_or_else(PublicApiError::not_found)?;
    Ok(
        json!({"result":"ok","settings":row.get::<_,Value>(0),"template":row.get::<_,String>(1),"updatedAt":row.get::<_,String>(2)}),
    )
}
async fn read_settings(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    req: Request,
) -> PublicApiResult<Response> {
    let (capability, _) = request(&state, &uri, &headers, req, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = auth::bound_account_read(&tx, &capability).await?;
    let value = document(&tx, actor.user_id()).await?;
    tx.commit().await?;
    Ok(reply(value))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SettingsWrite {
    settings: Value,
    updated_at: String,
}
async fn write_settings(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    req: Request,
) -> PublicApiResult<Response> {
    let (capability, body) =
        request(&state, &uri, &headers, req, settings_schema::SETTINGS_BYTES).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = auth::bound_account_write(&tx, &capability).await?;
    actor.require_permission("self.preferences.update")?;
    let input: SettingsWrite = serde_json::from_slice(&body).map_err(|_| {
        PublicApiError::bad_request("settings object and updatedAt string are required")
    })?;
    if input.updated_at.len() != 27
        || !input.updated_at.bytes().enumerate().all(|(i, b)| match i {
            4 | 7 => b == b'-',
            10 => b == b'T',
            13 | 16 => b == b':',
            19 => b == b'.',
            26 => b == b'Z',
            _ => b.is_ascii_digit(),
        })
    {
        return Err(PublicApiError::bad_request(
            "updatedAt must be the exact UTC microsecond token returned by GET settings",
        ));
    }
    let current = document(&tx, actor.user_id()).await?;
    // Compare the exact canonical UTC microsecond token that GET returned. This
    // avoids lossy date parsing, timezone ambiguities and equal rapid revisions.
    if input.updated_at != current["updatedAt"].as_str().expect("timestamp") {
        return Err(PublicApiError::conflict(
            "Preferences changed; reload before saving",
        ));
    }
    settings_schema::value_budget(&input.settings)?;
    let settings = preference_core::complete(&tx, input.settings, &current["settings"]).await?;
    let template=tx.query_opt("SELECT t.id::text,t.document FROM platform.settings_template_head h JOIN platform.settings_templates t ON t.id=h.template_id WHERE h.singleton FOR SHARE OF h",&[]).await?.ok_or_else(PublicApiError::not_found)?;
    let id: String = template.get(0);
    let schema: Value = template.get(1);
    settings_schema::validate(schema, Some(settings.clone())).await?;
    tx.execute("UPDATE platform.users SET preferences=$2,preferences_template_id=$3::text::uuid,updated_at=clock_timestamp() WHERE id=$1",&[&actor.user_id(),&settings,&id]).await?;
    let result = document(&tx, actor.user_id()).await?;
    tx.commit().await?;
    Ok(reply(result))
}
