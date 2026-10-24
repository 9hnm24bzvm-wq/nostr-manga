use super::crypto;
use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::http::{HeaderMap, header};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Serialize, Deserialize)]
pub struct SessionPrincipal {
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub verified: bool,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub session_id: String,
    /// API credentials never inherit a browser's recent-authentication grant.
    pub is_api_client: bool,
    pub authenticated_age_seconds: i64,
    pub recovery_session: bool,
}

impl SessionPrincipal {
    pub fn has_perm(&self, permission: &str) -> bool {
        !self.recovery_session
            && (!self.is_api_client || permission == "self.library.read")
            && !self
                .roles
                .iter()
                .any(|r| r == "banned" || r == "restricted")
            && (self.verified
                || matches!(permission, "self.user.update" | "self.preferences.update"))
            && self.permissions.iter().any(|p| p == permission)
    }
    pub fn is_staff(&self) -> bool {
        !self.is_api_client
            && !self.recovery_session
            && self
                .roles
                .iter()
                .any(|r| matches!(r.as_str(), "staff" | "sysop"))
    }
    pub fn has_role(&self, role: &str) -> bool {
        (!self.is_api_client || matches!(role, "banned" | "restricted"))
            && self.roles.iter().any(|r| r == role)
    }
    pub fn is_moderator(&self) -> bool {
        !self.is_api_client
            && !self.recovery_session
            && self
                .roles
                .iter()
                .any(|r| matches!(r.as_str(), "staff" | "sysop" | "moderator"))
    }
    pub fn require_permission(&self, permission: &str) -> ApiResult<()> {
        if self.has_perm(permission) {
            Ok(())
        } else {
            Err(ApiError::forbidden(
                "This account cannot perform that action",
            ))
        }
    }
    /// A library credential is never an ambient browser/account capability.
    pub fn require_browser_session(&self) -> ApiResult<()> {
        if self.is_api_client || self.recovery_session {
            Err(ApiError::forbidden(
                "Use your browser session for this action",
            ))
        } else {
            Ok(())
        }
    }
    /// Browser owner-library access retains its existing behavior; API clients
    /// must have the current explicit read-only scope and active account grant.
    pub fn require_library_read(&self) -> ApiResult<()> {
        if self.is_api_client {
            self.require_permission("self.library.read")
        } else if self.recovery_session {
            Err(ApiError::forbidden(
                "Recovery sessions may only set a new password",
            ))
        } else {
            Ok(())
        }
    }
    /// Personal projections must never borrow the identity of a password-reset
    /// cookie. Public readers can treat that cookie as an anonymous viewer.
    pub fn can_read_personal_data(&self) -> bool {
        self.require_library_read().is_ok()
    }
    pub fn require_verified(&self) -> ApiResult<()> {
        if self.verified {
            Ok(())
        } else {
            Err(ApiError::forbidden(
                "Verify your email before performing this action",
            ))
        }
    }
    pub fn require_owner_or_permission(&self, owner_id: &str, permission: &str) -> ApiResult<()> {
        self.require_verified()?;
        if self
            .roles
            .iter()
            .any(|r| r == "banned" || r == "restricted")
        {
            return Err(ApiError::forbidden(
                "This account cannot perform that action",
            ));
        }
        if !self.recovery_session && (self.user_id == owner_id && !self.is_api_client)
            || self.has_perm(permission)
        {
            Ok(())
        } else {
            Err(ApiError::forbidden(
                "This resource belongs to another account",
            ))
        }
    }
    pub fn require_recent_browser_auth(&self) -> ApiResult<()> {
        if !self.is_api_client && (0..600).contains(&self.authenticated_age_seconds) {
            Ok(())
        } else {
            Err(ApiError::new(
                axum::http::StatusCode::FORBIDDEN,
                "session_refresh_required",
                "Sign in again before changing account credentials",
            ))
        }
    }
}

pub fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut matches = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|h| h.to_str().ok())
        .flat_map(|s| s.split(';'))
        .filter_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name && !value.is_empty()).then_some(value)
        });
    let value = matches.next()?;
    // Ambiguous duplicate cookies are rejected instead of trusting ordering.
    if matches.next().is_some() {
        None
    } else {
        Some(value)
    }
}

fn strings(value: Value) -> Vec<String> {
    serde_json::from_value(value).unwrap_or_default()
}

fn effective_permissions(mut permissions: Vec<String>, scopes: Option<Vec<String>>) -> Vec<String> {
    // Reading one's own library is a baseline account capability, including
    // accounts registered before scoped clients were introduced. This grants
    // no draft, staged-media, account-email, staff or mutation authority.
    if !permissions.iter().any(|p| p == "self.library.read") {
        permissions.push("self.library.read".into());
    }
    if let Some(scopes) = scopes {
        // The current client UI issues only owner-library readers. A stored
        // legacy/arbitrary scope cannot silently opt into browser authority.
        permissions.retain(|p| p == "self.library.read" && scopes.contains(p));
    }
    permissions
}

pub async fn optional_principal(
    state: &AppState,
    headers: &HeaderMap,
) -> ApiResult<Option<SessionPrincipal>> {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));
    if headers.contains_key(header::AUTHORIZATION) && bearer.is_none() {
        return Ok(None);
    }
    let token = match bearer.or_else(|| cookie(headers, &state.config.session_cookie_name)) {
        Some(token) if token.len() <= 160 => token,
        _ => return Ok(None),
    };
    let hash = crypto::digest(token);
    let db = state.db.client().await?;
    let row = if bearer.is_some() {
        db.query_opt("SELECT u.id,u.username,u.email,u.verified,u.roles,u.permissions,c.id,c.scopes,0::BIGINT AS age,false AS recovery FROM platform.api_clients c JOIN platform.users u ON u.id=c.user_id WHERE c.secret_hash=$1 AND c.revoked_at IS NULL AND c.expires_at>clock_timestamp() AND NOT u.disabled", &[&hash]).await?
    } else {
        db.query_opt("SELECT u.id,u.username,u.email,u.verified,u.roles,u.permissions,s.id,NULL::JSONB AS scopes,GREATEST(0,EXTRACT(EPOCH FROM clock_timestamp()-s.authenticated_at)::BIGINT) AS age,(s.purpose='recovery') AS recovery FROM platform.sessions s JOIN platform.users u ON u.id=s.user_id WHERE s.token_hash=$1 AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp() AND NOT u.disabled", &[&hash]).await?
    };
    let principal = row.and_then(|row| {
        let permissions = effective_permissions(
            strings(row.get(5)),
            row.get::<_, Option<Value>>(7).map(strings),
        );
        let mut roles = strings(row.get(4));
        if bearer.is_some() {
            roles.retain(|role| matches!(role.as_str(), "banned" | "restricted"));
        }
        let principal = SessionPrincipal {
            user_id: row.get(0),
            username: row.get(1),
            email: row.get(2),
            verified: row.get(3),
            roles,
            permissions,
            session_id: row.get(6),
            is_api_client: bearer.is_some(),
            authenticated_age_seconds: row.get(8),
            recovery_session: row.get(9),
        };
        // Optional identity may authorize private-owner projections too. Do
        // not expose an identity whose bearer scope was removed or is invalid.
        if principal.is_api_client && principal.require_library_read().is_err() {
            None
        } else {
            Some(principal)
        }
    });
    if let Some(principal) = &principal {
        constrain_expected_account(headers, principal)?;
    }
    Ok(principal)
}

fn expected_account(headers: &HeaderMap) -> ApiResult<Option<&str>> {
    // This never authenticates or grants a capability. Applying it at the
    // optional identity boundary also protects private preview/read branches.
    let mut expected = headers.get_all("x-weebdex-expected-account").iter();
    if let Some(value) = expected.next() {
        if expected.next().is_some() {
            return Err(ApiError::bad_request("Ambiguous expected account"));
        }
        let value = value
            .to_str()
            .map_err(|_| ApiError::bad_request("Invalid expected account"))?;
        if value.is_empty() || value.len() > 128 {
            return Err(ApiError::bad_request("Invalid expected account"));
        }
        return Ok(Some(value));
    }
    Ok(None)
}

fn constrain_expected_account(headers: &HeaderMap, principal: &SessionPrincipal) -> ApiResult<()> {
    if expected_account(headers)?.is_some_and(|value| value != principal.user_id) {
        return Err(ApiError::conflict(
            "The signed-in account changed; refresh its preview before continuing",
        ));
    }
    Ok(())
}

fn native_library_cookie<'a>(headers: &'a HeaderMap, name: &str) -> ApiResult<&'a str> {
    // This seam is deliberately not a bearer/signed/account-recovery adapter.
    if headers.contains_key(header::AUTHORIZATION) {
        return Err(ApiError::forbidden("Use a normal browser cookie for completion lookup"));
    }
    if expected_account(headers)?.is_none() {
        return Err(ApiError::bad_request("Expected account is required"));
    }
    // The retained cookie helper ignores empty values. At this exact-selection
    // boundary even `sid=valid; sid=` is ambiguous, so count names first without
    // changing the existing extractor or unrelated authentication behavior.
    let mut named = 0;
    for value in headers.get_all(header::COOKIE) {
        let value = value.to_str().map_err(|_| ApiError::unauthorized())?;
        for part in value.split(';') {
            if part.trim().split_once('=').is_some_and(|(key, _)| key == name) {
                named += 1;
                if named > 1 { return Err(ApiError::unauthorized()); }
            }
        }
    }
    if named != 1 { return Err(ApiError::unauthorized()); }
    cookie(headers, name).filter(|token| token.len() <= 160)
        .ok_or_else(ApiError::unauthorized)
}

fn library_authority_error(error: tokio_postgres::Error) -> ApiError {
    if error.code().is_some_and(|code| matches!(code.code(), "55P03" | "40P01" | "40001")) {
        ApiError::conflict("Account authority changed or is busy; refresh before continuing")
    } else { error.into() }
}

/// Native cookie owner-library authority, resolved and fenced in the caller's
/// RepeatableRead transaction. Never use a previously cached/pool principal.
/// User -> session SHARE locks block disable/role changes and revocation until
/// commit. They do not promise authority at response delivery or stop expiry.
pub(crate) async fn require_native_library_principal_in(
    state: &AppState,
    headers: &HeaderMap,
    tx: &tokio_postgres::Transaction<'_>,
) -> ApiResult<SessionPrincipal> {
    let hash = crypto::digest(native_library_cookie(headers, &state.config.session_cookie_name)?);
    let candidate = tx.query_opt(
        "SELECT id,user_id FROM platform.sessions WHERE token_hash=$1", &[&hash],
    ).await.map_err(library_authority_error)?.ok_or_else(ApiError::unauthorized)?;
    let session_id: String = candidate.get(0);
    let user_id: String = candidate.get(1);
    // Separate statements make the lock order explicit, unlike a joined lock.
    let user = tx.query_opt(
        "SELECT id,username,email,verified,roles,permissions FROM platform.users WHERE id=$1 AND NOT disabled FOR SHARE NOWAIT",
        &[&user_id],
    ).await.map_err(library_authority_error)?.ok_or_else(ApiError::unauthorized)?;
    let session = tx.query_opt(
        "SELECT id,GREATEST(0,EXTRACT(EPOCH FROM clock_timestamp()-authenticated_at)::BIGINT),purpose='normal' FROM platform.sessions WHERE id=$1 AND user_id=$2 AND token_hash=$3 AND revoked_at IS NULL AND expires_at>clock_timestamp() FOR SHARE NOWAIT",
        &[&session_id, &user_id, &hash],
    ).await.map_err(library_authority_error)?.ok_or_else(ApiError::unauthorized)?;
    if !session.get::<_, bool>(2) {
        return Err(ApiError::forbidden("Use a normal browser session for completion lookup"));
    }
    let principal = SessionPrincipal {
        user_id: user.get(0), username: user.get(1), email: user.get(2), verified: user.get(3),
        roles: strings(user.get(4)), permissions: effective_permissions(strings(user.get(5)), None),
        session_id: session.get(0), is_api_client: false,
        authenticated_age_seconds: session.get(1), recovery_session: false,
    };
    principal.require_browser_session()?;
    // Preserve normal browser owner-library access, including unverified or
    // restricted accounts. has_perm("self.library.read") is NOT equivalent.
    principal.require_library_read()?;
    constrain_expected_account(headers, &principal)?;
    Ok(principal)
}

pub async fn require_library_principal(
    state: &AppState,
    headers: &HeaderMap,
) -> ApiResult<SessionPrincipal> {
    let principal = require_principal(state, headers).await?;
    principal.require_library_read()?;
    Ok(principal)
}

/// Optional identity for personal read projections, separate from the limited
/// identity still needed by the password-reset flow itself.
pub async fn optional_personal_principal(
    state: &AppState,
    headers: &HeaderMap,
) -> ApiResult<Option<SessionPrincipal>> {
    Ok(optional_principal(state, headers)
        .await?
        .filter(SessionPrincipal::can_read_personal_data))
}

pub async fn require_principal(
    state: &AppState,
    headers: &HeaderMap,
) -> ApiResult<SessionPrincipal> {
    let principal = optional_principal(state, headers)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    if principal.recovery_session {
        return Err(ApiError::forbidden(
            "Recovery sessions may only set a new password",
        ));
    }
    Ok(principal)
}

/// Cookie-authenticated unsafe requests require an explicit same-origin source.
/// Bearer callers are protected by the non-ambient Authorization credential.
pub fn validate_mutation(state: &AppState, headers: &HeaderMap) -> ApiResult<()> {
    if headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| h.starts_with("Bearer ") && h.len() > 7)
    {
        return Ok(());
    }
    validate_browser_mutation(&state.config.public_origin, headers)
}

pub fn validate_browser_mutation(origin: &str, headers: &HeaderMap) -> ApiResult<()> {
    if headers
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == "cross-site")
    {
        return Err(ApiError::forbidden("Cross-site requests are not allowed"));
    }
    let expected = url::Url::parse(origin).map_err(|_| ApiError::unavailable())?;
    let source = if let Some(value) = headers.get(header::ORIGIN) {
        let source = url::Url::parse(
            value
                .to_str()
                .map_err(|_| ApiError::forbidden("Invalid request origin"))?,
        )
        .map_err(|_| ApiError::forbidden("Invalid request origin"))?;
        if source.path() != "/"
            || source.query().is_some()
            || source.fragment().is_some()
            || !source.username().is_empty()
            || source.password().is_some()
        {
            return Err(ApiError::forbidden("Invalid request origin"));
        }
        source
    } else if let Some(value) = headers.get(header::REFERER) {
        url::Url::parse(
            value
                .to_str()
                .map_err(|_| ApiError::forbidden("Invalid request source"))?,
        )
        .map_err(|_| ApiError::forbidden("Invalid request source"))?
    } else {
        return Err(ApiError::forbidden(
            "A same-origin request source is required",
        ));
    };
    if source.origin() == expected.origin() {
        Ok(())
    } else {
        Err(ApiError::forbidden("Cross-origin requests are not allowed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn user(verified: bool, permissions: &[&str], roles: &[&str]) -> SessionPrincipal {
        SessionPrincipal {
            user_id: "owner".into(),
            username: "test".into(),
            email: "test@example.invalid".into(),
            verified,
            roles: roles.iter().map(|s| s.to_string()).collect(),
            permissions: permissions.iter().map(|s| s.to_string()).collect(),
            session_id: "test-session".into(),
            is_api_client: false,
            authenticated_age_seconds: 0,
            recovery_session: false,
        }
    }
    #[test]
    fn anonymous_unverified_owner_moderator_staff_permissions_fail_closed() {
        let unverified = user(false, &["manga.update", "self.user.update"], &["user"]);
        assert!(!unverified.has_perm("manga.update"));
        assert!(unverified.has_perm("self.user.update"));
        assert!(
            unverified
                .require_owner_or_permission("owner", "manga.update")
                .is_err()
        );
        let owner = user(true, &[], &["user"]);
        assert!(
            owner
                .require_owner_or_permission("owner", "manga.update")
                .is_ok()
        );
        assert!(
            owner
                .require_owner_or_permission("other", "manga.update")
                .is_err()
        );
        let moderator = user(true, &["manga.update"], &["moderator"]);
        assert!(
            moderator
                .require_owner_or_permission("other", "manga.update")
                .is_ok()
        );
        assert!(moderator.require_permission("user.update").is_err());
        let staff = user(true, &[], &["staff"]);
        assert!(staff.is_staff());
        assert!(staff.require_permission("manga.update").is_err());
        let banned = user(true, &["manga.update"], &["banned", "staff"]);
        assert!(!banned.has_perm("manga.update"));
        assert!(
            banned
                .require_owner_or_permission("owner", "manga.update")
                .is_err()
        );
    }
    #[test]
    fn csrf_origin_and_duplicate_cookies_are_rejected() {
        let mut h = HeaderMap::new();
        assert!(validate_browser_mutation("https://example.test", &h).is_err());
        h.insert(header::ORIGIN, "https://evil.test".parse().unwrap());
        assert!(validate_browser_mutation("https://example.test", &h).is_err());
        h.insert(header::ORIGIN, "https://example.test".parse().unwrap());
        assert!(validate_browser_mutation("https://example.test", &h).is_ok());
        h.insert(header::COOKIE, "sid=one; sid=two".parse().unwrap());
        assert_eq!(cookie(&h, "sid"), None);
    }
    #[test]
    fn scoped_library_clients_have_no_ambient_browser_or_staff_authority() {
        let mut client = user(
            true,
            &["self.library.read", "manga.update", "self.user.update"],
            &["staff", "moderator"],
        );
        client.is_api_client = true;
        assert!(client.require_library_read().is_ok());
        assert!(client.require_browser_session().is_err());
        assert!(client.require_recent_browser_auth().is_err());
        assert!(!client.is_staff());
        assert!(!client.is_moderator());
        assert!(!client.has_role("staff"));
        assert!(!client.has_perm("manga.update"));
        assert!(!client.has_perm("self.user.update"));
        assert!(
            client
                .require_owner_or_permission("owner", "manga.update")
                .is_err()
        );
        client.permissions.clear();
        assert!(client.require_library_read().is_err());
        client.permissions.push("self.library.read".into());
        client.roles.push("restricted".into());
        assert!(client.require_library_read().is_err());
        client.roles.clear();
        client.verified = false;
        assert!(client.require_library_read().is_err());
    }
    #[test]
    fn explicit_library_scope_survives_legacy_grants_without_expanding_other_scopes() {
        let account = vec!["self.user.update".into(), "manga.update".into()];
        let scoped = effective_permissions(
            account.clone(),
            Some(vec!["self.library.read".into(), "manga.update".into()]),
        );
        assert_eq!(scoped, vec!["self.library.read"]);
        assert!(effective_permissions(account.clone(), Some(Vec::new())).is_empty());
        let browser = effective_permissions(account, None);
        assert!(browser.iter().any(|p| p == "self.user.update"));
        assert!(browser.iter().any(|p| p == "manga.update"));
        assert!(browser.iter().any(|p| p == "self.library.read"));
        let owner = user(false, &[], &["user"]);
        assert!(owner.require_browser_session().is_ok());
        assert!(owner.require_library_read().is_ok());
        let mut recovery = owner;
        recovery.recovery_session = true;
        assert!(recovery.require_browser_session().is_err());
        assert!(recovery.require_library_read().is_err());
        assert!(!recovery.can_read_personal_data());
        recovery.roles = vec!["staff".into(), "moderator".into()];
        assert!(!recovery.is_staff());
        assert!(!recovery.is_moderator());
    }
    #[test]
    fn completion_requires_one_expected_account_and_only_native_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, "sid=synthetic".parse().unwrap());
        assert!(native_library_cookie(&headers, "sid").is_err());
        headers.insert("x-weebdex-expected-account", "owner".parse().unwrap());
        assert_eq!(native_library_cookie(&headers, "sid").unwrap(), "synthetic");
        let actor = user(false, &[], &["restricted"]);
        assert!(actor.require_browser_session().is_ok());
        assert!(actor.require_library_read().is_ok());
        assert!(constrain_expected_account(&headers, &actor).is_ok());
        headers.insert("x-weebdex-expected-account", "other".parse().unwrap());
        assert_eq!(constrain_expected_account(&headers, &actor).unwrap_err().status, axum::http::StatusCode::CONFLICT);
        headers.append("x-weebdex-expected-account", "other".parse().unwrap());
        assert!(native_library_cookie(&headers, "sid").is_err());
        headers.remove("x-weebdex-expected-account");
        headers.insert("x-weebdex-expected-account", "owner".parse().unwrap());
        for authorization in ["Bearer synthetic", "Nostr synthetic", "", "malformed"] {
            headers.insert(header::AUTHORIZATION, authorization.parse().unwrap());
            assert!(native_library_cookie(&headers, "sid").is_err());
        }
        headers.remove(header::AUTHORIZATION);
        for cookies in ["sid=one; sid=two".to_owned(), "sid=one; sid=".to_owned(), "sid=; sid=one".to_owned(), format!("sid={}", "x".repeat(161)), "other=value".to_owned()] {
            headers.insert(header::COOKIE, cookies.parse().unwrap());
            assert!(native_library_cookie(&headers, "sid").is_err());
        }
        headers.insert(header::COOKIE, "sid=synthetic".parse().unwrap());
        for expected in ["".to_owned(), "x".repeat(129)] {
            headers.insert("x-weebdex-expected-account", expected.parse().unwrap());
            assert!(native_library_cookie(&headers, "sid").is_err());
        }
        // Existing other endpoints still permit omission of the correlation
        // header. Mandatory presence is specific to this new completion seam.
        headers.remove("x-weebdex-expected-account");
        assert!(constrain_expected_account(&headers, &actor).is_ok());
    }
}
