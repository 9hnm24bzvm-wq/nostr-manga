//! Explicit native-cookie, immediate-only publication delegation. This table is
//! never an original-signer login binding or a native role/permission grant.
pub mod cli;
use crate::{ApiError, ApiResult, AppState, domains::auth::SessionPrincipal};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use tokio_postgres::Transaction;
pub(super) const LOCK: i64 = 697241112;
pub(crate) fn lock_error(e: tokio_postgres::Error) -> ApiError {
    match e.code().map(|v| v.code()) {
        Some("55P03" | "40P01" | "40001" | "57014" | "P6012") => ApiError::conflict(
            "Publication access or account activity is changing. No access change was applied; inspect the exact receipt before retrying.",
        ),
        Some("P6013") => ApiError::conflict(
            "Publication access revision exhausted; contact the instance operator",
        ),
        _ => e.into(),
    }
}
pub(super) async fn bounded(tx: &Transaction<'_>) -> ApiResult<()> {
    tx.batch_execute("SET LOCAL lock_timeout='500ms';SET LOCAL statement_timeout='5s';SET LOCAL idle_in_transaction_session_timeout='10s'").await.map_err(lock_error)?;
    Ok(())
}
pub(super) async fn serialize(tx: &Transaction<'_>) -> ApiResult<()> {
    bounded(tx).await?;
    if !tx
        .query_one("SELECT pg_try_advisory_xact_lock($1)", &[&LOCK])
        .await
        .map_err(lock_error)?
        .get::<_, bool>(0)
    {
        return Err(ApiError::conflict(
            "Another publication access operation is running; nothing was changed",
        ));
    }
    Ok(())
}
pub(super) fn key(s: &str) -> ApiResult<Vec<u8>> {
    if s.len() != 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(ApiError::bad_request(
            "Select an exact lowercase public publisher key",
        ));
    }
    hex::decode(s).map_err(|_| ApiError::bad_request("Invalid public publisher key"))
}
pub(super) fn user_id(s: &str) -> ApiResult<()> {
    if s.is_empty()
        || s.len() > 128
        || s.starts_with("protocol:")
        || s.chars().any(|c| c.is_control())
    {
        return Err(ApiError::bad_request(
            "Select an existing native account ID",
        ));
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Delegation {
    pub user_id: String,
    pub publisher: String,
    pub publisher_admission_version: i64,
    pub revision: i64,
    pub revoked: bool,
}
pub(crate) async fn delegation_in(
    tx: &Transaction<'_>,
    user: &str,
    lock: bool,
) -> ApiResult<Option<Delegation>> {
    let sql = format!(
        "SELECT user_id,publisher,publisher_admission_version,revision,revoked_at IS NOT NULL FROM platform.native_publication_delegations WHERE user_id=$1 {}",
        if lock { "FOR SHARE NOWAIT" } else { "" }
    );
    Ok(tx
        .query_opt(&sql, &[&user])
        .await
        .map_err(lock_error)?
        .map(|r| Delegation {
            user_id: r.get(0),
            publisher: r.get(1),
            publisher_admission_version: r.get(2),
            revision: r.get(3),
            revoked: r.get(4),
        }))
}
/// Original publisher authority remains original-module owned. This read-only
/// seam uses its current allowed/version policy and locks the existing row; it
/// never opens a signer, UploadStore, runtime, or admission mutation interface.
pub(crate) async fn publisher_admission_in(
    tx: &Transaction<'_>,
    publisher: &str,
    lock: bool,
) -> ApiResult<Option<(bool, i64)>> {
    let bytes = key(publisher)?;
    let sql = format!(
        "SELECT allowed,version FROM publisher_permissions WHERE publisher=$1 {}",
        if lock { "FOR SHARE NOWAIT" } else { "" }
    );
    Ok(tx
        .query_opt(&sql, &[&bytes])
        .await
        .map_err(lock_error)?
        .map(|r| (r.get(0), r.get(1))))
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Capture {
    pub user_id: String,
    pub lifecycle_id: String,
    pub publisher: String,
    pub delegation_revision: i64,
    pub publisher_admission_version: i64,
}
fn same_set(a: &[String], b: &[String]) -> bool {
    a.iter().collect::<BTreeSet<_>>() == b.iter().collect::<BTreeSet<_>>()
}
fn same_permissions(a: &[String], b: &[String]) -> bool {
    // The retained ordinary principal adds this read-only baseline for older
    // accounts; native transaction principals carry the raw stored permission
    // list. Normalize only that documented baseline, never mutation grants.
    let mut a = a.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let mut b = b.iter().map(String::as_str).collect::<BTreeSet<_>>();
    a.insert("self.library.read");
    b.insert("self.library.read");
    a == b
}
/// `write` is the caller's existing account lock mode. NEVER turn a mutation's
/// NO KEY UPDATE fence into SHARE/UPDATE: the independent command needs FK KEY SHARE.
/// A cached principal/boolean alone is insufficient; recheck its ordinary native
/// session and current account authority in this very transaction.
pub(crate) async fn resolve_native_in(
    state: &AppState,
    tx: &Transaction<'_>,
    principal: Option<&SessionPrincipal>,
    write: bool,
) -> ApiResult<Option<Capture>> {
    let Some(a) = principal else { return Ok(None) };
    if delegation_in(tx, &a.user_id, false).await?.is_none() {
        return Ok(None);
    }
    if a.is_api_client || a.recovery_session {
        return Err(ApiError::forbidden(
            "A current ordinary native session is required for delegated publication",
        ));
    }
    user_id(&a.user_id)?;
    let sql = format!(
        "SELECT verified,disabled,roles,permissions,account_lifecycle_id::text FROM platform.users WHERE id=$1 {}",
        if write {
            "FOR NO KEY UPDATE NOWAIT"
        } else {
            "FOR SHARE NOWAIT"
        }
    );
    let user = tx
        .query_opt(&sql, &[&a.user_id])
        .await
        .map_err(lock_error)?
        .ok_or_else(ApiError::unauthorized)?;
    let normal=tx.query_opt("SELECT id FROM platform.sessions WHERE id=$1 AND user_id=$2 AND purpose='normal' AND revoked_at IS NULL AND expires_at>clock_timestamp() FOR SHARE NOWAIT",&[&a.session_id,&a.user_id]).await.map_err(lock_error)?.is_some();
    if !normal {
        return Err(ApiError::unauthorized());
    }
    // No dedicated history means legacy behavior remains unchanged. A retained
    // revoked/stale delegation below never falls back to a legacy identity.
    let Some(d) = delegation_in(tx, &a.user_id, true).await? else {
        return Ok(None);
    };
    let roles: Vec<String> =
        serde_json::from_value(user.get::<_, Value>(2)).map_err(|_| ApiError::unavailable())?;
    let permissions: Vec<String> =
        serde_json::from_value(user.get::<_, Value>(3)).map_err(|_| ApiError::unavailable())?;
    if !user.get::<_, bool>(0)
        || user.get::<_, bool>(1)
        || roles.iter().any(|r| r == "banned" || r == "restricted")
        || !same_set(&roles, &a.roles)
        || !same_permissions(&permissions, &a.permissions)
        || !a.verified
    {
        return Err(ApiError::forbidden(
            "Current native account authority is required; reload the account before publishing",
        ));
    }
    if d.revoked {
        return Err(ApiError::forbidden(
            "Native publish-now access was revoked. A fresh operator review is required; no legacy fallback is allowed.",
        ));
    }
    let configured = state.config.protocol_publication.as_ref().ok_or_else(|| {
        ApiError::forbidden("The site's original publication service is not configured")
    })?;
    if configured.expected_publisher != d.publisher {
        return Err(ApiError::forbidden(
            "Configured publisher changed; ask the operator to review publication access",
        ));
    }
    let legacy=tx.query("SELECT publisher FROM platform.protocol_identity_bindings WHERE user_id=$1 AND revoked_at IS NULL ORDER BY publisher LIMIT 2 FOR SHARE NOWAIT",&[&a.user_id]).await.map_err(lock_error)?;
    if !legacy.is_empty() {
        return Err(ApiError::conflict(
            "Dedicated publication access overlaps a legacy signer binding; the operator must resolve this ambiguity",
        ));
    }
    if publisher_admission_in(tx, &d.publisher, true).await?
        != Some((true, d.publisher_admission_version))
    {
        return Err(ApiError::forbidden(
            "Original publisher admission changed. Re-admission alone does not restore this delegation; ask the operator for a fresh review.",
        ));
    }
    Ok(Some(Capture {
        user_id: a.user_id.clone(),
        lifecycle_id: user.get(4),
        publisher: d.publisher,
        delegation_revision: d.revision,
        publisher_admission_version: d.publisher_admission_version,
    }))
}
pub(crate) async fn capture_matches_in(
    tx: &Transaction<'_>,
    command: &str,
    capture: Option<&Capture>,
) -> ApiResult<bool> {
    let row=tx.query_opt("SELECT authority_kind,delegation_revision,delegation_publisher,delegation_admission_version,delegation_lifecycle_id::text FROM platform.protocol_publication_commands WHERE command_id=$1",&[&command]).await?;
    Ok(row.is_none_or(|r| match capture {
        None => r.get::<_, String>(0) == "legacy",
        Some(c) => {
            r.get::<_, String>(0) == "native-delegation"
                && r.get::<_, Option<i64>>(1) == Some(c.delegation_revision)
                && r.get::<_, Option<String>>(2).as_deref() == Some(c.publisher.as_str())
                && r.get::<_, Option<i64>>(3) == Some(c.publisher_admission_version)
                && r.get::<_, Option<String>>(4).as_deref() == Some(c.lifecycle_id.as_str())
        }
    }))
}
pub(super) async fn audit_in(
    tx: &Transaction<'_>,
    user: &str,
    authority: &str,
    label: &str,
    review: Option<&str>,
    operation: &str,
    reason: &str,
    before: &Option<Delegation>,
    after: &Option<Delegation>,
) -> ApiResult<i64> {
    Ok(tx.query_one("INSERT INTO platform.publication_access_audit(user_id,authority,operator_label,review_id,operation,reason,before_access,after_access) VALUES($1,$2,$3,$4::text::uuid,$5,$6,$7,$8) RETURNING id",&[&user,&authority,&label,&review,&operation,&reason,&json!(before),&json!(after)]).await.map_err(lock_error)?.get(0))
}
/// Called only after closure holds the account UPDATE lock, and before its final
/// disable. Everything, including this audit/revision, rolls back on any blocker.
pub(crate) async fn revoke_for_account_closure_in(
    tx: &Transaction<'_>,
    user: &str,
    confirmation: &str,
) -> ApiResult<()> {
    if delegation_in(tx, user, false)
        .await?
        .is_none_or(|d| d.revoked)
    {
        return Ok(());
    }
    serialize(tx).await?;
    tx.query("SELECT user_id FROM platform.native_publication_delegations WHERE user_id=$1 FOR UPDATE NOWAIT",&[&user]).await.map_err(lock_error)?;
    let before = delegation_in(tx, user, false).await?;
    if before.as_ref().is_some_and(|d| !d.revoked) {
        tx.execute("UPDATE platform.native_publication_delegations SET revoked_at=clock_timestamp() WHERE user_id=$1",&[&user]).await.map_err(lock_error)?;
        let after = delegation_in(tx, user, false).await?;
        audit_in(tx,user,"account-closure",confirmation,None,"revoke","Owner disabled this native account; publication access is not restored by reactivation",&before,&after).await?;
    }
    Ok(())
}
