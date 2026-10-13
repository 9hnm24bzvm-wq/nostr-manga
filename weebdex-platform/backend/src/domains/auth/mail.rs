//! Configurable synchronous HTTPS delivery with a durable hashed issuance and
//! delivery ledger. Disabled by default; no simulated delivery in production.
use super::{crypto, flows};
use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};
use anyhow::{Result, ensure};
use axum::{
    extract::{Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use zeroize::Zeroizing;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MailConfig {
    /// A provider/webhook implementing POST {from,to,subject,text}, returning 2xx
    /// only when it has accepted delivery. Redirects and HTTP proxies are disabled.
    pub endpoint: String,
    pub from: String,
    #[serde(default)]
    pub bearer_token_env: Option<String>,
    /// Explicit fixture opt-in: HTTP is accepted only on a loopback IP. Fixture
    /// delivery is additionally restricted to the reserved example.invalid domain.
    #[serde(default)]
    pub allow_loopback_test_sink: bool,
}

impl MailConfig {
    pub fn validate(&self) -> Result<()> {
        let url = url::Url::parse(&self.endpoint)?;
        ensure!(
            url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none()
                && url.query().is_none(),
            "Mail delivery URL must not contain credentials, query or fragment"
        );
        let local = url.host_str().is_some_and(|host| {
            host.parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
        });
        ensure!(
            url.scheme() == "https"
                || self.allow_loopback_test_sink && local && url.scheme() == "http",
            "Mail delivery requires HTTPS or the explicit loopback fixture sink"
        );
        if self.allow_loopback_test_sink {
            ensure!(local, "Fixture mail sink must be a loopback IP");
        }
        ensure!(
            flows::valid_email(&self.from),
            "Mail sender must be a valid email address"
        );
        if let Some(name) = &self.bearer_token_env {
            ensure!(
                !name.is_empty()
                    && name.len() <= 64
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'),
                "Mail credential environment variable name is invalid"
            );
        }
        Ok(())
    }
    async fn deliver(&self, to: &str, kind: &str, link: &str) -> ApiResult<()> {
        self.validate().map_err(|_| ApiError::unavailable())?;
        if self.allow_loopback_test_sink && !to.ends_with("@example.invalid") {
            return Err(ApiError::forbidden(
                "Fixture delivery accepts reserved synthetic recipients only",
            ));
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()
            .map_err(|_| ApiError::unavailable())?;
        let (subject, intro) = if kind == "verification" {
            (
                "Verify your WeebDex email",
                "Use this one-time link to verify your email address.",
            )
        } else {
            (
                "Recover your WeebDex account",
                "Use this one-time link to open a restricted password-recovery session.",
            )
        };
        let mut request=client.post(&self.endpoint).json(&json!({"from":self.from,"to":to,"subject":subject,"text":format!("{intro}\n\n{link}\n\nThis link expires in 15 minutes. If you did not request this message, ignore it.")}));
        if let Some(name) = &self.bearer_token_env {
            let credential =
                Zeroizing::new(std::env::var(name).map_err(|_| ApiError::unavailable())?);
            if credential.is_empty() || credential.len() > 4096 {
                return Err(ApiError::unavailable());
            }
            request = request.bearer_auth(credential.as_str());
        }
        let response = request.send().await.map_err(|_| {
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "email_delivery_unavailable",
                "Email delivery is unavailable; try again later",
            )
        })?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "email_delivery_unavailable",
                "Email delivery is unavailable; try again later",
            ))
        }
    }
}

pub(super) async fn send_link(state: &AppState, flow: &flows::Flow, email: &str) -> ApiResult<()> {
    let mail=state.config.auth_mail.as_ref().ok_or_else(||ApiError::new(StatusCode::SERVICE_UNAVAILABLE,"email_provider_required","Email delivery is not configured. Recovery and verification are unavailable; contact the instance administrator."))?;
    mail.validate().map_err(|_| ApiError::unavailable())?;
    let email = email.trim().to_lowercase();
    flows::throttle(state, &format!("email:{}:{}", flow.kind, email), 4, 900).await?;
    let raw_token = Zeroizing::new(crypto::token());
    let hash = crypto::digest(&raw_token);
    let recipient_hash = crypto::digest(&email);
    let id = crypto::id();
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    // Unknown accounts receive an indistinguishable decoy link. Delivery, status,
    // timing and UI remain equivalent; a decoy can never authenticate or verify.
    let user = tx
        .query_opt(
            "SELECT id FROM platform.users WHERE lower(email)=$1 AND NOT disabled FOR UPDATE",
            &[&email],
        )
        .await?
        .map(|r| r.get::<_, String>(0));
    tx.execute("DELETE FROM platform.auth_email_tokens WHERE id IN (SELECT id FROM platform.auth_email_tokens WHERE expires_at<clock_timestamp()-interval '1 hour' FOR UPDATE SKIP LOCKED LIMIT 100)",&[]).await?;
    tx.execute("INSERT INTO platform.auth_email_tokens(id,kind,user_id,token_hash,recipient_hash,flow_id,return_to,delivery_state,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,'pending',clock_timestamp()+interval '15 minutes')",&[&id,&flow.kind,&user,&hash,&recipient_hash,&flow.id,&flow.return_to]).await?;
    tx.commit().await?;
    drop(db);
    let link = format!(
        "{}/auth/self-service/{}?token={}",
        state.config.public_origin.trim_end_matches('/'),
        flow.kind,
        raw_token.as_str()
    );
    let delivered = mail.deliver(&email, &flow.kind, &link).await;
    let db = state.db.client().await?;
    db.execute("UPDATE platform.auth_email_tokens SET delivery_state=$2,delivered_at=CASE WHEN $2='sent' THEN clock_timestamp() ELSE NULL END WHERE id=$1",&[&id,&if delivered.is_ok(){"sent"}else{"failed"}]).await?;
    delivered
}

#[derive(Deserialize)]
pub(super) struct RedeemQuery {
    token: Option<String>,
}

pub(super) async fn verification(
    State(state): State<AppState>,
    Query(query): Query<RedeemQuery>,
) -> Result<Response, flows::FlowError> {
    redeem(&state, query, "verification").await
}
pub(super) async fn recovery(
    State(state): State<AppState>,
    Query(query): Query<RedeemQuery>,
) -> Result<Response, flows::FlowError> {
    redeem(&state, query, "recovery").await
}

async fn redeem(
    state: &AppState,
    query: RedeemQuery,
    kind: &str,
) -> Result<Response, flows::FlowError> {
    let token = Zeroizing::new(
        query
            .token
            .filter(|t| t.len() == 43)
            .ok_or_else(invalid_link)?,
    );
    let hash = crypto::digest(&token);
    flows::throttle(state, &format!("email:redeem-global:{kind}"), 128, 60).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    // Discover the account without locking a token first. Issuance, rotation
    // and redemption all use account -> token/session lock ordering.
    let user_id: Option<String> = tx.query_opt("SELECT user_id FROM platform.auth_email_tokens WHERE token_hash=$1 AND kind=$2 AND delivery_state='sent' AND used_at IS NULL AND expires_at>clock_timestamp()", &[&hash,&kind]).await?.ok_or_else(invalid_link)?.get(0);
    let user_id = user_id.ok_or_else(invalid_link)?;
    let user = tx
        .query_opt(
            "SELECT email,disabled FROM platform.users WHERE id=$1 FOR UPDATE",
            &[&user_id],
        )
        .await?
        .ok_or_else(invalid_link)?;
    // Recheck token validity after the current account lock. A password change
    // that won the lock has already consumed every outstanding recovery link.
    let row=tx.query_opt("SELECT t.id,t.recipient_hash,t.return_to FROM platform.auth_email_tokens t WHERE t.token_hash=$1 AND t.kind=$2 AND t.user_id=$3 AND t.delivery_state='sent' AND t.used_at IS NULL AND t.expires_at>clock_timestamp() FOR UPDATE OF t",&[&hash,&kind,&user_id]).await?.ok_or_else(invalid_link)?;
    let id: String = row.get(0);
    let recipient_hash: Vec<u8> = row.get(1);
    let target: String = row.get(2);
    let email: String = user.get(0);
    let disabled: bool = user.get(1);
    if disabled
        || !crypto::constant_time_equal(&recipient_hash, &crypto::digest(&email.to_lowercase()))
    {
        return Err(invalid_link().into());
    }
    let target = flows::safe_return_to(&state.config.public_origin, Some(&target))?;
    tx.execute(
        "UPDATE platform.auth_email_tokens SET used_at=clock_timestamp() WHERE id=$1",
        &[&id],
    )
    .await?;
    let mut response = StatusCode::SEE_OTHER.into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        "no-store".parse().map_err(|_| ApiError::unavailable())?,
    );
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        "no-referrer".parse().map_err(|_| ApiError::unavailable())?,
    );
    response.headers_mut().insert(
        header::LOCATION,
        target.parse().map_err(|_| ApiError::unavailable())?,
    );
    if kind == "verification" {
        tx.execute("UPDATE platform.users SET verified=true,roles=CASE WHEN roles ? 'verified' THEN roles ELSE roles || '[\"verified\"]'::JSONB END,updated_at=clock_timestamp(),version=version+1 WHERE id=$1",&[&user_id]).await?;
    } else {
        let session_id = crypto::id();
        let session_token = Zeroizing::new(crypto::token());
        let session_hash = crypto::digest(&session_token);
        let browser = crypto::token();
        tx.execute("INSERT INTO platform.sessions(id,token_hash,user_id,purpose,expires_at) VALUES($1,$2,$3,'recovery',clock_timestamp()+interval '15 minutes')",&[&session_id,&session_hash,&user_id]).await?;
        flows::set_cookie(
            &mut response,
            state,
            &state.config.session_cookie_name,
            &session_token,
            900,
        )?;
        flows::set_cookie(
            &mut response,
            state,
            flows::BROWSER_COOKIE,
            &browser,
            state.config.session_lifetime_seconds,
        )?;
        let destination = format!(
            "{}/auth/recovery",
            state.config.public_origin.trim_end_matches('/')
        );
        response.headers_mut().insert(
            header::LOCATION,
            destination.parse().map_err(|_| ApiError::unavailable())?,
        );
    }
    tx.commit().await?;
    Ok(response)
}
fn invalid_link() -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "invalid_email_link",
        "This email link is invalid, already used, or expired",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mail_configuration_rejects_insecure_production_and_nonlocal_test_sinks() {
        let mut cfg = MailConfig {
            endpoint: "http://provider.example.invalid/messages".into(),
            from: "service@example.invalid".into(),
            bearer_token_env: None,
            allow_loopback_test_sink: false,
        };
        assert!(cfg.validate().is_err());
        cfg.endpoint = "https://provider.example.invalid/messages".into();
        assert!(cfg.validate().is_ok());
        cfg.allow_loopback_test_sink = true;
        assert!(cfg.validate().is_err());
        cfg.endpoint = "http://127.0.0.1:8089/messages".into();
        assert!(cfg.validate().is_ok());
    }
}
