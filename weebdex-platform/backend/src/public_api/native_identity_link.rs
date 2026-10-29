//! Private native-account ↔ portable-identity proof. This module must never write
//! protocol_identity_bindings, account_permissions, user grants, or public events.
//! The cookie and embedded signature are both required for linking; neither is a
//! fallback authentication mechanism. Unlink is fresh native owner recovery.
use super::{PublicApiError, PublicApiResult, PublicUuid};
use crate::{
    AppState,
    domains::{auth, library::account_public_id_in},
};
use axum::{
    Json, Router,
    extract::{OriginalUri, Request, State},
    http::HeaderMap,
    routing::{get, post},
};
use manga_node::auth::{Event, hash, now, valid_hash};
use rand::{RngCore, rngs::OsRng};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_postgres::Transaction;
const CONFIRM_PATH: &str = "/api/v1/account/identity-link/confirm";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/account/identity-link", get(read))
        .route("/account/identity-link/prepare", post(prepare))
        .route("/account/identity-link/confirm", post(confirm))
}
async fn actor(
    state: &AppState,
    headers: &HeaderMap,
    tx: &Transaction<'_>,
    write: bool,
) -> PublicApiResult<auth::SessionPrincipal> {
    let actor = auth::require_native_account_principal_in(state, headers, tx, write).await?;
    if write {
        auth::validate_mutation(state, headers)?;
        actor.require_verified()?;
        actor.require_permission("self.user.update")?;
        require_fresh_session(tx, &actor).await?;
    }
    Ok(actor)
}
async fn require_fresh_session(
    tx: &Transaction<'_>,
    actor: &auth::SessionPrincipal,
) -> PublicApiResult<()> {
    // EXISTS gives a clean refusal if expiry is crossed after extraction. User
    // and session locks prevent revocation races, but time still advances.
    let fresh: bool = tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.sessions WHERE id=$1 AND user_id=$2 AND purpose='normal' AND revoked_at IS NULL AND expires_at>clock_timestamp() AND authenticated_at<=clock_timestamp() AND authenticated_at>clock_timestamp()-interval '10 minutes')", &[&actor.session_id,&actor.user_id]).await?.get(0);
    if !fresh {
        return Err(PublicApiError::forbidden(
            "Sign in again before connecting or disconnecting a portable identity. Your existing connection has not changed.",
        ));
    }
    Ok(())
}
async fn capture(
    tx: &Transaction<'_>,
    actor: &auth::SessionPrincipal,
    expected: &str,
) -> PublicApiResult<String> {
    PublicUuid::parse(expected)?;
    let id = account_public_id_in(tx, &actor.user_id).await?;
    if id != expected {
        return Err(PublicApiError::conflict(
            "Account changed. Reload Account settings before continuing.",
        ));
    }
    Ok(id)
}
fn envelope(account: &str, key: Option<String>, revision: i64) -> Value {
    json!({"result":"ok","accountId":account,"publicKey":key,"revision":revision,"scope":"private-account-association","authentication":"native-session","publiclyListed":false,"grantsAuthority":false,"changesPublisherAuthority":false})
}
async fn read(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    super::uploads::no_query(&uri)?;
    super::native_account_profile_settings::body(request, &headers, false, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, false).await?;
    let account = account_public_id_in(&tx, &actor.user_id).await?;
    let row = tx
        .query_opt(
            "SELECT public_key,revision FROM platform.native_identity_links WHERE user_id=$1",
            &[&actor.user_id],
        )
        .await?;
    let value = row
        .map(|r| envelope(&account, r.get(0), r.get(1)))
        .unwrap_or_else(|| envelope(&account, None, 0));
    tx.commit().await?;
    Ok(Json(value))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Prepare {
    expected_account_id: String,
    expected_revision: i64,
    action: String,
    public_key: Option<String>,
}
async fn prepare(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    super::uploads::no_query(&uri)?;
    let bytes = super::native_account_profile_settings::body(request, &headers, true, 4096).await?;
    let command: Prepare = serde_json::from_slice(&bytes)
        .map_err(|_| PublicApiError::bad_request("Invalid identity connection review"))?;
    if !["link", "unlink"].contains(&command.action.as_str()) || command.expected_revision < 0 {
        return Err(PublicApiError::bad_request(
            "Choose connect or disconnect and reload the current connection",
        ));
    }
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, true).await?;
    let account = capture(&tx, &actor, &command.expected_account_id).await?;
    tx.execute(
        "INSERT INTO platform.native_identity_links(user_id) VALUES($1) ON CONFLICT DO NOTHING",
        &[&actor.user_id],
    )
    .await?;
    let row=tx.query_one("SELECT public_key,revision FROM platform.native_identity_links WHERE user_id=$1 FOR UPDATE",&[&actor.user_id]).await?;
    let current: Option<String> = row.get(0);
    let revision: i64 = row.get(1);
    if revision != command.expected_revision {
        return Err(PublicApiError::conflict(
            "Connection changed. Reload it before reviewing again.",
        ));
    }
    let key = if command.action == "link" {
        if current.is_some() {
            return Err(PublicApiError::conflict(
                "Disconnect the existing identity explicitly before choosing another. No identities were merged.",
            ));
        }
        command
            .public_key
            .filter(|key| valid_hash(key))
            .ok_or_else(|| PublicApiError::bad_request("Choose a valid public identity"))?
    } else {
        if command.public_key.is_some() {
            return Err(PublicApiError::bad_request(
                "Disconnect uses the current connection, not another supplied identity",
            ));
        }
        current.ok_or_else(|| {
            PublicApiError::conflict("There is no connected identity to disconnect")
        })?
    };
    let mut random = [0u8; 32];
    OsRng.fill_bytes(&mut random);
    let id = hex::encode(random);
    let expires = now() + 300;
    let url = format!(
        "{}{}",
        state.config.public_origin.trim_end_matches('/'),
        CONFIRM_PATH
    );
    let payload=json!({"purpose":"weebdex-private-identity-link-v1","challenge":id,"accountId":account,"publicKey":key,"action":command.action,"revision":revision,"origin":state.config.public_origin,"expiresAt":expires}).to_string();
    // A new review invalidates earlier unconfirmed reviews for this account.
    tx.execute("UPDATE platform.native_identity_link_challenges SET used_at=clock_timestamp(),outcome='replaced' WHERE user_id=$1 AND used_at IS NULL",&[&actor.user_id]).await?;
    tx.execute("DELETE FROM platform.native_identity_link_challenges WHERE user_id=$1 AND created_at<clock_timestamp()-interval '30 days'",&[&actor.user_id]).await?;
    tx.execute("INSERT INTO platform.native_identity_link_challenges(id,user_id,session_id,action,public_key,revision,payload,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,to_timestamp($8::double precision))",&[&id,&actor.user_id,&actor.session_id,&command.action,&key,&revision,&payload,&(expires as f64)]).await?;
    tx.execute("DELETE FROM platform.native_identity_link_challenges WHERE user_id=$1 AND id NOT IN (SELECT id FROM platform.native_identity_link_challenges WHERE user_id=$1 ORDER BY created_at DESC,id DESC LIMIT 100)",&[&actor.user_id]).await?;
    require_fresh_session(&tx, &actor).await?;
    let value = json!({"result":"ok","accountId":account,"challengeId":id,"action":command.action,"publicKey":key,"revision":revision,"expiresAt":expires,"payload":payload,"proofUrl":url,"proofPayloadHash":hash(payload.as_bytes()),"privateProofOnly":true,"grantsAuthority":false});
    tx.commit().await?;
    Ok(Json(value))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Confirm {
    expected_account_id: String,
    challenge_id: String,
    confirmation: String,
    proof: Option<Event>,
}
fn verify_proof(event: &Event, key: &str, url: &str, payload: &str) -> PublicApiResult<()> {
    let expected = vec![
        vec!["u".to_string(), url.to_string()],
        vec!["method".to_string(), "POST".to_string()],
        vec!["payload".to_string(), hash(payload.as_bytes())],
    ];
    if event.verify().is_err()
        || event.pubkey != key
        || event.kind != 27235
        || !event.content.is_empty()
        || event.tags != expected
        || event.created_at.abs_diff(now()) > 60
    {
        return Err(PublicApiError::unauthorized(
            "The identity proof is invalid, expired, or for a different connection. Review again; nothing was connected.",
        ));
    }
    Ok(())
}
async fn confirm(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    super::uploads::no_query(&uri)?;
    let bytes =
        super::native_account_profile_settings::body(request, &headers, true, 16384).await?;
    let command: Confirm = serde_json::from_slice(&bytes)
        .map_err(|_| PublicApiError::bad_request("Invalid identity connection confirmation"))?;
    if !valid_hash(&command.challenge_id) {
        return Err(PublicApiError::bad_request("Invalid connection review"));
    }
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, true).await?;
    let account = capture(&tx, &actor, &command.expected_account_id).await?;
    // Uniform user/session -> link -> challenge order keeps revoke/link races serialized.
    let row=tx.query_opt("SELECT public_key,revision FROM platform.native_identity_links WHERE user_id=$1 FOR UPDATE",&[&actor.user_id]).await?.ok_or_else(||PublicApiError::conflict("Connection review is no longer current"))?;
    let current: Option<String> = row.get(0);
    let revision: i64 = row.get(1);
    let challenge=tx.query_opt("SELECT action,public_key,revision,payload FROM platform.native_identity_link_challenges WHERE id=$1 AND user_id=$2 AND session_id=$3 AND used_at IS NULL AND expires_at>clock_timestamp() FOR UPDATE",&[&command.challenge_id,&actor.user_id,&actor.session_id]).await?.ok_or_else(||PublicApiError::conflict("Connection review expired, was replaced, or belongs to another session. Review again."))?;
    let action: String = challenge.get(0);
    let key: String = challenge.get(1);
    let expected_revision: i64 = challenge.get(2);
    let payload: String = challenge.get(3);
    if revision != expected_revision
        || (action == "link" && current.is_some())
        || (action == "unlink" && current.as_ref() != Some(&key))
    {
        return Err(PublicApiError::conflict(
            "Connection changed. Reload and review again.",
        ));
    }
    let next = if action == "link" {
        if command.confirmation != "connect-private-identity" {
            return Err(PublicApiError::bad_request(
                "Explicit connection approval is required",
            ));
        }
        let proof = command.proof.as_ref().ok_or_else(|| {
            PublicApiError::unauthorized("Approval from your portable identity is required")
        })?;
        verify_proof(
            proof,
            &key,
            &format!(
                "{}{}",
                state.config.public_origin.trim_end_matches('/'),
                CONFIRM_PATH
            ),
            &payload,
        )?;
        Some(key)
    } else {
        if command.confirmation != "disconnect-private-identity" || command.proof.is_some() {
            return Err(PublicApiError::bad_request(
                "Explicit owner disconnect approval is required",
            ));
        }
        None
    };
    require_fresh_session(&tx, &actor).await?;
    match tx.execute("UPDATE platform.native_identity_links SET public_key=$2,revision=revision+1,updated_at=clock_timestamp() WHERE user_id=$1",&[&actor.user_id,&next]).await {
        Ok(_) => {}, Err(error) if error.code().is_some_and(|c|c.code()=="23505") => return Err(PublicApiError::conflict("This identity cannot be connected to this account. It may already be connected elsewhere; no accounts were merged.")), Err(error)=>return Err(error.into())
    }
    // The unique public-key index can wait on another account after the earlier
    // checks. Recheck all expiring proofs after that wait; any refusal rolls the
    // association update back together with the untouched challenge.
    require_fresh_session(&tx, &actor).await?;
    let challenge_current: bool = tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.native_identity_link_challenges WHERE id=$1 AND user_id=$2 AND session_id=$3 AND used_at IS NULL AND expires_at>clock_timestamp())",&[&command.challenge_id,&actor.user_id,&actor.session_id]).await?.get(0);
    if !challenge_current {
        return Err(PublicApiError::conflict(
            "Connection review expired while waiting. Review again; nothing was connected.",
        ));
    }
    if let Some(proof) = command.proof.as_ref() {
        verify_proof(
            proof,
            next.as_deref().expect("link proof has a key"),
            &format!(
                "{}{}",
                state.config.public_origin.trim_end_matches('/'),
                CONFIRM_PATH
            ),
            &payload,
        )?;
    }
    tx.execute("UPDATE platform.native_identity_link_challenges SET used_at=clock_timestamp(),outcome=$2 WHERE id=$1",&[&command.challenge_id,&if action=="link"{"linked"}else{"unlinked"}]).await?;
    tx.commit().await?;
    Ok(Json(envelope(&account, next, revision + 1)))
}
