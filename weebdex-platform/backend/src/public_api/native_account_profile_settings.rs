//! Explicit normal-browser adapters. Shared profile/settings cores keep their
//! native permissions and storage. No signed route falls back to cookies.
use super::{
    PublicApiError, PublicApiResult, PublicUuid, ResourceKind, account_settings, native_profile,
    query::{PublicQuery, QuerySchema, SortDirection},
    settings_writes,
};
use crate::{
    AppState,
    domains::{
        auth,
        catalog::query::Sql,
        library::{account_public_id_in, preference_mutations, profile_mutations},
    },
};
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{OriginalUri, Path, Request, State},
    http::{HeaderMap, Uri, header},
    routing::get,
};
use serde_json::{Value, json};
use std::time::Duration;
use tokio_postgres::Transaction;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/account/user/{id}/profile",
            get(profile_read).put(profile_write),
        )
        .route("/account/settings", get(settings_read).post(settings_write))
        .route("/account/settings/template", get(template_read))
        .route(
            "/account/settings/reconcile-block",
            axum::routing::post(reconcile_block),
        )
        .route("/account/user", get(directory))
}
pub(super) async fn body(
    request: Request,
    headers: &HeaderMap,
    write: bool,
    limit: usize,
) -> PublicApiResult<Vec<u8>> {
    if write {
        let mut types = headers.get_all(header::CONTENT_TYPE).iter();
        if !types.next().and_then(|v| v.to_str().ok()).is_some_and(|v| {
            v.split(';')
                .next()
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
        }) || types.next().is_some()
        {
            return Err(PublicApiError::bad_request("JSON content type required"));
        }
    }
    let bytes = tokio::time::timeout(Duration::from_secs(5), to_bytes(request.into_body(), limit))
        .await
        .map_err(|_| PublicApiError::bad_request("Request body timeout"))?
        .map_err(|_| PublicApiError::bad_request("Request body exceeds operation budget"))?;
    if !write && !bytes.is_empty() {
        return Err(PublicApiError::bad_request("GET accepts no body"));
    }
    Ok(bytes.to_vec())
}
pub(super) async fn actor(
    state: &AppState,
    headers: &HeaderMap,
    tx: &Transaction<'_>,
    write: bool,
) -> PublicApiResult<auth::SessionPrincipal> {
    let actor = auth::require_native_account_principal_in(state, headers, tx, write).await?;
    if write {
        auth::validate_mutation(state, headers)?;
    }
    Ok(actor)
}
pub(super) async fn expected(
    tx: &Transaction<'_>,
    actor: &auth::SessionPrincipal,
    id: &PublicUuid,
) -> PublicApiResult<String> {
    let account = account_public_id_in(tx, &actor.user_id).await?;
    if id.as_str() != account {
        return Err(PublicApiError::conflict(
            "Account changed; reload before continuing",
        ));
    }
    Ok(account)
}
pub(super) fn provenance(value: &mut Value, account: &str) {
    if !value["nostr"].is_object() {
        value["nostr"] = json!({});
    }
    value["nostr"]["scope"] = json!("instance-account");
    value["nostr"]["authentication"] = json!("native-session");
    value["nostr"]["accountId"] = json!(account);
}
fn no_query(uri: &Uri) -> PublicApiResult<()> {
    super::uploads::no_query(uri)
}
async fn profile_read(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    let query = PublicQuery::parse_unchecked(uri.query())?;
    query.require_allowed(&["expectedAccountId"])?;
    let capture = PublicUuid::parse(
        query
            .first("expectedAccountId")
            .ok_or_else(|| PublicApiError::bad_request("expectedAccountId required"))?,
    )?;
    let id = PublicUuid::parse(&id)?;
    body(request, &headers, false, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = actor(&state, &headers, &tx, false).await?;
    actor.require_permission("self.user.update")?;
    let account = expected(&tx, &actor, &capture).await?;
    let target = native_profile::target(&tx, &id, &actor.user_id).await?;
    let mut value = native_profile::envelope(
        profile_mutations::read_in(&*tx, &target.native_id).await?,
        &account,
        &target.preferred_id,
        None,
    );
    provenance(&mut value, &account);
    value["data"]["nostr"]["authentication"] = json!("native-session");
    tx.commit().await?;
    Ok(Json(value))
}
async fn profile_write(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    no_query(&uri)?;
    let id = PublicUuid::parse(&id)?;
    let bytes = body(request, &headers, true, 32768).await?;
    let command: native_profile::Command = serde_json::from_slice(&bytes)
        .map_err(|_| PublicApiError::bad_request("Invalid native profile command"))?;
    let shape: Value = serde_json::from_slice(&bytes)
        .map_err(|_| PublicApiError::bad_request("Invalid profile JSON"))?;
    if shape
        .as_object()
        .ok_or_else(|| PublicApiError::bad_request("Expected profile object"))?
        .values()
        .any(Value::is_null)
    {
        return Err(PublicApiError::bad_request(
            "Profile fields cannot be null; empty strings clear",
        ));
    }
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, true).await?;
    let account = expected(&tx, &actor, &command.expected_account_id).await?;
    let target = native_profile::target(&tx, &id, &actor.user_id).await?;
    if command.expected_user_id != target.preferred_id {
        return Err(PublicApiError::conflict(
            "Profile identity changed; reload before continuing",
        ));
    }
    profile_mutations::update_in(&tx, &actor, &target.native_id, &command.patch()).await?;
    let mut value = native_profile::envelope(
        profile_mutations::read_in(&*tx, &target.native_id).await?,
        &account,
        &target.preferred_id,
        Some(true),
    );
    provenance(&mut value, &account);
    value["data"]["nostr"]["authentication"] = json!("native-session");
    tx.commit().await?;
    Ok(Json(value))
}
async fn settings_read(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    no_query(&uri)?;
    body(request, &headers, false, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = auth::require_native_library_principal_in(&state, &headers, &tx).await?;
    let mut value = account_settings::read_native_in(&tx, &actor.user_id).await?;
    provenance(
        &mut value,
        &account_public_id_in(&tx, &actor.user_id).await?,
    );
    tx.commit().await?;
    Ok(Json(value))
}
async fn template_read(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    no_query(&uri)?;
    body(request, &headers, false, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = auth::require_native_library_principal_in(&state, &headers, &tx).await?;
    let mut value = account_settings::template()?;
    provenance(
        &mut value,
        &account_public_id_in(&tx, &actor.user_id).await?,
    );
    tx.commit().await?;
    Ok(Json(value))
}
async fn settings_write(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    no_query(&uri)?;
    let bytes = body(request, &headers, true, 32768).await?;
    let mut command: settings_writes::Payload = serde_json::from_slice(&bytes)
        .map_err(|_| PublicApiError::bad_request("Invalid settings command"))?;
    settings_writes::request_shape(
        &serde_json::from_slice(&bytes)
            .map_err(|_| PublicApiError::bad_request("Invalid settings JSON"))?,
    )?;
    let capture = command
        .expected_account_id
        .as_ref()
        .ok_or_else(|| PublicApiError::bad_request("expectedAccountId required"))?;
    let time = settings_writes::expected_time(
        command
            .updated_at
            .as_deref()
            .ok_or_else(|| PublicApiError::bad_request("updatedAt required"))?,
    )?;
    if command
        .template
        .as_ref()
        .is_some_and(|id| id.as_str() != account_settings::TEMPLATE_ID)
    {
        return Err(PublicApiError::bad_request("Unknown settings template"));
    }
    if command.settings.items_per_page.as_deref().is_some_and(|v| {
        v.is_empty() || v.starts_with('0') || !v.bytes().all(|b| b.is_ascii_digit())
    }) {
        return Err(PublicApiError::bad_request(
            "items_per_page must be canonical decimal text",
        ));
    }
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, true).await?;
    let account = expected(&tx, &actor, capture).await?;
    settings_writes::references(&tx, &mut command.settings).await?;
    if command
        .settings
        .uploaders
        .as_ref()
        .is_some_and(|ids| ids.contains(&actor.user_id))
    {
        return Err(PublicApiError::bad_request(
            "You cannot exclude your own uploads",
        ));
    }
    let (version, patch) = preference_mutations::patch(command.settings)?;
    let changed = preference_mutations::update_in(
        &tx,
        &actor,
        version,
        patch,
        preference_mutations::Mode::NativePreserving,
        Some(&time),
    )
    .await?;
    let mut value = account_settings::read_native_in(&tx, &actor.user_id).await?;
    provenance(&mut value, &account);
    value["nostr"]["changed"] = json!(changed);
    value["nostr"]["nativeStorageUnchanged"] = json!(!changed);
    value["nostr"]["mutation"] = json!("known-field-merge");
    value["nostr"]["preservedUnavailableFilterValues"] = json!(true);
    value["nostr"]["preservedUnknownNativeFields"] = json!(true);
    tx.commit().await?;
    Ok(Json(value))
}
async fn directory(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    let mut q = PublicQuery::parse(
        uri.query(),
        &QuerySchema {
            default_limit: 10,
            max_limit: 100,
            max_offset: 10000,
            scalar: &["username"],
            arrays: &["ids[]"],
            includes: &[],
            order: &["username"],
        },
    )?;
    if q.order.is_empty() {
        q.order.push(("username".into(), SortDirection::Asc));
    }
    if q.first("username")
        .is_some_and(|v| v.len() > 200 || v.contains('\0'))
    {
        return Err(PublicApiError::bad_request("Invalid username search"));
    }
    body(request, &headers, false, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = actor(&state, &headers, &tx, false).await?;
    // The native user directory is public. Require a current ordinary account
    // here, but do not invent a role or expose private account fields.
    let mut sql = Sql::default();
    super::catalogue::filters(
        &state,
        Some(&tx),
        ResourceKind::User,
        &q,
        &mut sql,
        None,
        None,
    )
    .await?;
    if let Some(username) = q.first("username") {
        let name = sql.bind(username.to_owned());
        sql.conditions
            .push(format!("position(lower({name}) in lower(u.username))>0"));
    }
    let response = super::catalogue::collection_in(&tx, ResourceKind::User, &q, &mut sql).await?;
    let mut value = serde_json::to_value(response).map_err(|_| PublicApiError::unavailable())?;
    provenance(
        &mut value,
        &account_public_id_in(&tx, &actor.user_id).await?,
    );
    value["nostr"]["source"] = json!("persisted-native-account-library");
    value["nostr"]["requires"] = json!("native-session-account");
    tx.commit().await?;
    Ok(Json(value))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BlockBarrier {
    expected_account_id: PublicUuid,
    target_id: PublicUuid,
    command_id: String,
    expected_version: i32,
    expected_updated_at: String,
}
async fn reconcile_block(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    no_query(&uri)?;
    let bytes = body(request, &headers, true, 2048).await?;
    let command: BlockBarrier = serde_json::from_slice(&bytes)
        .map_err(|_| PublicApiError::bad_request("Invalid block reconciliation command"))?;
    if command.expected_version < 1
        || command.command_id.is_empty()
        || command.command_id.len() > 128
        || !command
            .command_id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(PublicApiError::bad_request(
            "Invalid reconciliation capture",
        ));
    }
    let timestamp = settings_writes::expected_time(&command.expected_updated_at)?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, true).await?;
    actor.require_permission("self.preferences.update")?;
    let account = expected(&tx, &actor, &command.expected_account_id).await?;
    let target = super::uploads::resolve(&tx, ResourceKind::User, &command.target_id).await?;
    let target_id = super::uploads::preferred(&tx, ResourceKind::User, &target).await?;
    if target == actor.user_id {
        return Err(PublicApiError::bad_request(
            "You cannot exclude your own uploads",
        ));
    }
    if target_id != command.target_id {
        return Err(PublicApiError::conflict(
            "Block target changed; reopen the original account view",
        ));
    }
    let row = tx
        .query_one(
            "SELECT preferences FROM platform.users WHERE id=$1 FOR NO KEY UPDATE",
            &[&actor.user_id],
        )
        .await?;
    let current: Value = row.get(0);
    let version = current["version"]
        .as_i64()
        .filter(|v| (1..=i64::from(i32::MAX)).contains(v))
        .ok_or_else(PublicApiError::unavailable)? as i32;
    if version < command.expected_version {
        return Err(PublicApiError::conflict(
            "Settings revision is older than this captured command",
        ));
    }
    let advanced = if version == command.expected_version {
        preference_mutations::update_in(
            &tx,
            &actor,
            version,
            json!({}),
            preference_mutations::Mode::NativeBarrier,
            Some(&timestamp),
        )
        .await?
    } else {
        false
    };
    let mut value = account_settings::read_native_in(&tx, &actor.user_id).await?;
    provenance(&mut value, &account);
    value["nostr"]["barrierEstablished"] = json!(true);
    value["nostr"]["commandId"] = json!(command.command_id);
    value["nostr"]["targetId"] = json!(target_id);
    value["nostr"]["expectedVersion"] = json!(command.expected_version);
    value["nostr"]["barrierVersion"] = value["settings"]["version"].clone();
    value["nostr"]["mutation"] = json!("version-barrier-only");
    value["nostr"]["changed"] = json!(false);
    value["nostr"]["nativeStorageUnchanged"] = json!(!advanced);
    tx.commit().await?;
    Ok(Json(value))
}
