//! Deliberately separate staff editor. The ordinary account editor stays self-only.
use crate::{AppState, ApiError, ApiResult, contracts::library::ProfilePatch, domains::{auth, library::profile_mutations}, public_api::{PostgresPublicIdResolver, PublicIdTarget, PublicUuid, ResourceKind}};
use axum::{Json, extract::{Path, State}, http::HeaderMap};
use serde::Deserialize;
use serde_json::{Value, json};

async fn target(tx: &tokio_postgres::Transaction<'_>, id: &str) -> ApiResult<(String, String)> {
    let uuid = PublicUuid::parse(id).map_err(|_| ApiError::bad_request("Invalid public profile address"))?;
    let row = PostgresPublicIdResolver::resolve_many_in(&*tx, ResourceKind::User, &[uuid]).await
        .map_err(|_| ApiError::not_found("Public account profile not found"))?
        .into_values().next().filter(|r| r.target == PublicIdTarget::NativeResource)
        .ok_or_else(|| ApiError::not_found("This is not an instance account profile"))?;
    Ok((row.native_id, row.preferred_id.to_string()))
}
fn authorize(actor: &auth::SessionPrincipal) -> ApiResult<()> {
    if !actor.is_staff() { return Err(ApiError::forbidden("Staff profile editing permission required")); }
    actor.require_permission("user.update")
}
fn envelope(actor: &str, user: &str, attributes: Value, changed: bool) -> Value {
    json!({"scope":"native-staff-profile","actorId":actor,"userId":user,"attributes":attributes,"changed":changed})
}
pub(super) async fn read(State(state): State<AppState>, Path(id): Path<String>, h: HeaderMap) -> ApiResult<Json<Value>> {
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = auth::require_native_account_principal_in(&state, &h, &tx, false).await?;
    authorize(&actor)?;
    let (native, public) = target(&tx, &id).await?;
    let result = envelope(&actor.user_id, &public, profile_mutations::read_in(&*tx, &native).await?, false);
    tx.commit().await?;
    Ok(Json(result))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all="camelCase")]
pub(super) struct Command { expected_user_id: String, patch: ProfilePatch }
pub(super) async fn write(State(state): State<AppState>, Path(id): Path<String>, h: HeaderMap, Json(command): Json<Command>) -> ApiResult<Json<Value>> {
    auth::validate_mutation(&state, &h)?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    // Final actor write lock is acquired before the foreign target. A stale
    // role, disabled account, revoked session or API credential cannot pass.
    let actor = auth::require_native_account_principal_in(&state, &h, &tx, true).await?;
    authorize(&actor)?;
    let (native, public) = target(&tx, &id).await?;
    if command.expected_user_id != public { return Err(ApiError::conflict("The selected profile changed; reopen the editor")); }
    profile_mutations::update_in(&tx, &actor, &native, &command.patch).await?;
    let result = envelope(&actor.user_id, &public, profile_mutations::read_in(&*tx, &native).await?, true);
    tx.commit().await?;
    Ok(Json(result))
}
