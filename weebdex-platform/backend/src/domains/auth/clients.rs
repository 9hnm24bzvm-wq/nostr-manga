use super::{crypto, flows, principal};
use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_postgres::Row;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/client", get(list).post(create))
        .route("/client/{id}", post(rename).delete(remove))
        .route("/client/{id}/secret", post(rotate))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Name {
    name: String,
}
const CLIENT_FIELDS: &str = "id,name,version,to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),to_char(updated_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')";
fn dto(row: Row, key: Option<&str>) -> Value {
    let mut value = json!({"id":row.get::<_,String>(0),"name":row.get::<_,String>(1),"version":row.get::<_,i32>(2),"created_at":row.get::<_,String>(3),"updated_at":row.get::<_,String>(4)});
    if let Some(key) = key {
        value["key"] = json!(key);
    }
    value
}
fn reply(body: Value) -> Response {
    let mut response = Json(body).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.chars().count())
        && name.len() <= 256
        && !name.chars().any(char::is_control)
}

async fn list(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Response> {
    let user = principal::require_principal(&state, &headers).await?;
    // A password-reset cookie is a restricted capability, never private
    // account/client access. The same browser boundary also denies API tokens.
    user.require_browser_session()?;
    let sql = format!(
        "SELECT {CLIENT_FIELDS} FROM platform.api_clients WHERE user_id=$1 AND revoked_at IS NULL ORDER BY created_at,id LIMIT 10"
    );
    let rows = state
        .db
        .client()
        .await?
        .query(&sql, &[&user.user_id])
        .await?;
    Ok(reply(Value::Array(
        rows.into_iter().map(|r| dto(r, None)).collect(),
    )))
}

async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Name>,
) -> ApiResult<Response> {
    principal::validate_mutation(&state, &headers)?;
    let user = principal::require_principal(&state, &headers).await?;
    user.require_permission("client.create")?;
    user.require_recent_browser_auth()?;
    let name = payload.name.trim();
    if !valid_name(name) {
        return Err(ApiError::bad_request(
            "Client name must contain 1 to 64 printable characters",
        ));
    }
    flows::throttle(
        &state,
        &format!("api-client:create:{}", user.user_id),
        5,
        300,
    )
    .await?;
    let id = crypto::id();
    let key = format!("wd_{}.{}", id, crypto::token());
    let hash = crypto::digest(&key);
    // The unchanged client UI has no scope editor. Issue read-only owner scopes,
    // never arbitrary account/admin permissions or ambient browser grants.
    let scopes = json!(["self.library.read"]);
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    revalidate_credential_authority(&tx, &user, "client.create").await?;
    let count: i64 = tx
        .query_one(
            "SELECT count(*) FROM platform.api_clients WHERE user_id=$1 AND revoked_at IS NULL",
            &[&user.user_id],
        )
        .await?
        .get(0);
    if count >= 10 {
        return Err(ApiError::conflict(
            "At most 10 active API clients are allowed",
        ));
    }
    let sql = format!(
        "INSERT INTO platform.api_clients(id,user_id,name,secret_hash,scopes) VALUES($1,$2,$3,$4,$5) RETURNING {CLIENT_FIELDS}"
    );
    let row = tx
        .query_one(&sql, &[&id, &user.user_id, &name, &hash, &scopes])
        .await?;
    tx.commit().await?;
    // The retained creation dialog consumes the one-time secret only on201.
    // Reuse private no-store headers, but do not change list/update/rotation200.
    let mut response = reply(dto(row, Some(&key)));
    *response.status_mut() = StatusCode::CREATED;
    Ok(response)
}

async fn rename(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Name>,
) -> ApiResult<Response> {
    principal::validate_mutation(&state, &headers)?;
    let user = principal::require_principal(&state, &headers).await?;
    user.require_permission("self.client.update")?;
    if user.is_api_client {
        return Err(ApiError::forbidden(
            "Use your browser session to manage API clients",
        ));
    }
    let name = payload.name.trim();
    if !valid_name(name) {
        return Err(ApiError::bad_request(
            "Client name must contain 1 to 64 printable characters",
        ));
    }
    let sql = format!(
        "UPDATE platform.api_clients SET name=$3,version=version+1,updated_at=clock_timestamp() WHERE id=$1 AND user_id=$2 AND revoked_at IS NULL RETURNING {CLIENT_FIELDS}"
    );
    let row = state
        .db
        .client()
        .await?
        .query_opt(&sql, &[&id, &user.user_id, &name])
        .await?
        .ok_or_else(|| ApiError::not_found("API client not found"))?;
    Ok(reply(dto(row, None)))
}

async fn remove(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    principal::validate_mutation(&state, &headers)?;
    let user = principal::require_principal(&state, &headers).await?;
    user.require_permission("self.client.delete")?;
    if user.is_api_client {
        return Err(ApiError::forbidden(
            "Use your browser session to manage API clients",
        ));
    }
    let changed=state.db.client().await?.execute("UPDATE platform.api_clients SET revoked_at=clock_timestamp(),version=version+1,updated_at=clock_timestamp() WHERE id=$1 AND user_id=$2 AND revoked_at IS NULL",&[&id,&user.user_id]).await?;
    if changed == 0 {
        return Err(ApiError::not_found("API client not found"));
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn rotate(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    principal::validate_mutation(&state, &headers)?;
    let user = principal::require_principal(&state, &headers).await?;
    user.require_permission("self.client.update")?;
    user.require_recent_browser_auth()?;
    flows::throttle(
        &state,
        &format!("api-client:rotate:{}", user.user_id),
        5,
        300,
    )
    .await?;
    let key = format!("wd_{}.{}", id, crypto::token());
    let hash = crypto::digest(&key);
    let sql = format!(
        "UPDATE platform.api_clients SET secret_hash=$3,version=version+1,updated_at=clock_timestamp(),expires_at=clock_timestamp()+interval '90 days' WHERE id=$1 AND user_id=$2 AND revoked_at IS NULL RETURNING {CLIENT_FIELDS}"
    );
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    revalidate_credential_authority(&tx, &user, "self.client.update").await?;
    let row = tx
        .query_opt(&sql, &[&id, &user.user_id, &hash])
        .await?
        .ok_or_else(|| ApiError::not_found("API client not found"))?;
    tx.commit().await?;
    Ok(reply(dto(row, Some(&key))))
}

/// Credential issuance must serialize with password/recovery rotation. An
/// earlier resolved cookie does not survive waiting for the account lock.
async fn revalidate_credential_authority(
    tx: &tokio_postgres::Transaction<'_>,
    user: &principal::SessionPrincipal,
    permission: &str,
) -> ApiResult<()> {
    user.require_browser_session()?;
    let account = tx.query_opt(
        "SELECT roles,permissions FROM platform.users WHERE id=$1 AND verified AND NOT disabled FOR UPDATE",
        &[&user.user_id],
    ).await?.ok_or_else(|| ApiError::forbidden("A verified active account is required"))?;
    let mut fresh = user.clone();
    fresh.roles =
        serde_json::from_value(account.get::<_, Value>(0)).map_err(|_| ApiError::unavailable())?;
    fresh.permissions =
        serde_json::from_value(account.get::<_, Value>(1)).map_err(|_| ApiError::unavailable())?;
    fresh.require_permission(permission)?;
    if tx.query_opt(
        "SELECT id FROM platform.sessions WHERE id=$1 AND user_id=$2 AND purpose='normal' AND revoked_at IS NULL AND expires_at>clock_timestamp() AND authenticated_at>clock_timestamp()-interval '10 minutes' FOR UPDATE",
        &[&user.session_id,&user.user_id],
    ).await?.is_none() {
        return Err(ApiError::unauthorized());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn client_name_is_bounded() {
        assert!(valid_name("Fixture client"));
        assert!(!valid_name(""));
        assert!(!valid_name("bad\nname"));
        assert!(!valid_name(&"a".repeat(65)));
    }
}
