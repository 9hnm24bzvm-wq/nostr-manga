//! Request-scoped capabilities for trusted, in-process HTTP adapters.
//! Configured external origin + exact original URI, never Host/Forwarded or a
//! rewritten upstream path. No credentials, native accounts or server signing.
use crate::{
    accounts::AccountPermit,
    auth,
    db::{Id, Repository},
};

pub struct HttpService {
    origin: String,
    key: Id,
}
impl HttpService {
    pub fn new(origin: &str, key: &str) -> Result<Self, HttpAuthError> {
        let origin = crate::node::normalize_origin(origin).map_err(|_| HttpAuthError::Service)?;
        let key = identifier(key).ok_or(HttpAuthError::Service)?;
        Ok(Self { origin, key })
    }
}
/// Private non-Clone fields make this a verified one-request capability, not a
/// UUID/native-session principal or a reusable bearer token.
pub struct AdmittedHttpCapability {
    principal: Id,
    service: Id,
    request_url: String,
    method: String,
    body_hash: Id,
    authorization: Id,
    permit: AccountPermit,
}
impl AdmittedHttpCapability {
    pub fn principal(&self) -> Id {
        self.principal
    }
    pub fn service(&self) -> Id {
        self.service
    }
    pub fn request_url(&self) -> &str {
        &self.request_url
    }
    pub fn method(&self) -> &str {
        &self.method
    }
    pub fn body_hash(&self) -> Id {
        self.body_hash
    }
    pub fn authorization(&self) -> Id {
        self.authorization
    }
    pub fn account_permit(&self) -> &AccountPermit {
        &self.permit
    }
    pub async fn recheck_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
    ) -> Result<(), HttpAuthError> {
        self.recheck_service_transaction(tx).await?;
        crate::accounts::check_account_tx(tx, &self.permit)
            .await
            .map_err(|error| {
                if error.is::<tokio_postgres::Error>() {
                    HttpAuthError::Unavailable
                } else {
                    HttpAuthError::Denied
                }
            })
    }
    /// Service fence can precede the original author FOR UPDATE lock, avoiding
    /// an account SHARE→UPDATE upgrade and preserving service→author ordering.
    pub(crate) async fn recheck_service_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
    ) -> Result<(), HttpAuthError> {
        let service: Option<Vec<u8>> = tx
            .query_opt(
                "SELECT service_key FROM node_identity WHERE singleton FOR SHARE",
                &[],
            )
            .await
            .map_err(|_| HttpAuthError::Unavailable)?
            .map(|row| row.get(0));
        if service.as_deref() != Some(self.service.as_slice()) {
            return Err(HttpAuthError::Service);
        }
        Ok(())
    }
}
#[derive(Debug, thiserror::Error)]
pub enum HttpAuthError {
    #[error("invalid request authorization")]
    Invalid,
    #[error("account is not admitted")]
    Denied,
    #[error("expired or replayed request authorization")]
    Replay,
    #[error("selected service is unavailable or does not own this repository")]
    Service,
    #[error("authorization store unavailable")]
    Unavailable,
}
fn identifier(value: &str) -> Option<Id> {
    auth::valid_hash(value)
        .then(|| hex::decode(value).ok()?.try_into().ok())
        .flatten()
}
/// Header-only admission check for bounded body receipt. It never consumes
/// replay, grants a write capability, or permits a storage mutation. Exact body
/// verification and one durable replay consume still mint the final capability.
pub struct HttpRequestPreflight {
    service: Id,
}
impl HttpRequestPreflight {
    pub(crate) fn service(&self) -> Id {
        self.service
    }
}
fn verified_scope(
    service: &HttpService,
    authorization: &str,
    external_uri: &str,
    method: &str,
    body: Option<(Id, bool)>,
) -> Result<(auth::Event, String), HttpAuthError> {
    if authorization.len() > 16384 || external_uri.len() > 32768 {
        return Err(HttpAuthError::Invalid);
    }
    let uri: axum::http::Uri = external_uri.parse().map_err(|_| HttpAuthError::Invalid)?;
    if !external_uri.starts_with('/')
        || external_uri.starts_with("//")
        || uri.scheme().is_some()
        || uri.authority().is_some()
        || uri.path_and_query().map(|v| v.as_str()) != Some(external_uri)
    {
        return Err(HttpAuthError::Invalid);
    }
    let event = auth::token(authorization).map_err(|_| HttpAuthError::Invalid)?;
    let request_url = format!("{}{}", service.origin, external_uri);
    auth::http_scope(&event, &request_url, method).map_err(|_| HttpAuthError::Invalid)?;
    if let Some((digest, empty)) = body {
        if empty != (digest == body_digest(b"")) {
            return Err(HttpAuthError::Invalid);
        }
    }
    // Empty GET proofs may omit payload under the original NIP-98 policy.
    // If supplied, the public adapter extension never ignores an ambiguous
    // or inconsistent raw-body binding. Native auth::http is unchanged.
    let payloads: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some("payload"))
        .collect();
    if !payloads.is_empty()
        && (payloads.len() != 1
            || payloads[0].len() != 2
            || !auth::valid_hash(&payloads[0][1])
            || body.is_some_and(|(digest, _)| payloads[0][1] != hex::encode(digest)))
    {
        return Err(HttpAuthError::Invalid);
    }

    if (["POST", "PUT", "PATCH", "DELETE"].contains(&method)
        || body.is_some_and(|(_, empty)| !empty))
        && payloads.is_empty()
    {
        return Err(HttpAuthError::Invalid);
    }
    let services: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some("service"))
        .collect();
    if services.len() != 1 || services[0].len() != 2 || services[0][1] != hex::encode(service.key) {
        return Err(HttpAuthError::Invalid);
    }
    Ok((event, request_url))
}
fn body_digest(body: &[u8]) -> Id {
    use sha2::Digest;
    sha2::Sha256::digest(body).into()
}
impl Repository {
    pub async fn preflight_admitted_http_request(
        &self,
        service: &HttpService,
        authorization: &str,
        external_uri: &str,
        method: &str,
    ) -> Result<HttpRequestPreflight, HttpAuthError> {
        let (event, _) = verified_scope(service, authorization, external_uri, method, None)?;
        let principal = identifier(&event.pubkey).ok_or(HttpAuthError::Invalid)?;
        self.verify_http_service_account(service, principal).await?;
        Ok(HttpRequestPreflight {
            service: service.key,
        })
    }
    pub async fn verify_admitted_http_request(
        &self,
        service: &HttpService,
        authorization: &str,
        external_uri: &str,
        method: &str,
        body: &[u8],
    ) -> Result<AdmittedHttpCapability, HttpAuthError> {
        self.verify_admitted_http_request_digest(
            service,
            authorization,
            external_uri,
            method,
            body_digest(body),
            body.is_empty(),
        )
        .await
    }
    /// For trusted bounded frame/spool adapters only: digest and empty flag must
    /// be computed over every actual raw request byte. This changes no proof,
    /// admission, selected-service, freshness, or durable replay policy.
    pub async fn verify_admitted_http_request_digest(
        &self,
        service: &HttpService,
        authorization: &str,
        external_uri: &str,
        method: &str,
        body_hash: Id,
        body_empty: bool,
    ) -> Result<AdmittedHttpCapability, HttpAuthError> {
        let (event, request_url) = verified_scope(
            service,
            authorization,
            external_uri,
            method,
            Some((body_hash, body_empty)),
        )?;
        let principal = identifier(&event.pubkey).ok_or(HttpAuthError::Invalid)?;
        let authorization = identifier(&event.id).ok_or(HttpAuthError::Invalid)?;
        let permit = self.verify_http_service_account(service, principal).await?;
        if !self
            .consume_http_auth(authorization, event.created_at)
            .await
            .map_err(|_| HttpAuthError::Unavailable)?
        {
            return Err(HttpAuthError::Replay);
        }
        Ok(AdmittedHttpCapability {
            principal,
            service: service.key,
            request_url,
            method: method.to_owned(),
            body_hash,
            authorization,
            permit,
        })
    }
    async fn verify_http_service_account(
        &self,
        service: &HttpService,
        principal: Id,
    ) -> Result<AccountPermit, HttpAuthError> {
        let actual: Option<Vec<u8>> = self
            .pool
            .get()
            .await
            .map_err(|_| HttpAuthError::Unavailable)?
            .query_opt("SELECT service_key FROM node_identity WHERE singleton", &[])
            .await
            .map_err(|_| HttpAuthError::Unavailable)?
            .map(|row| row.get(0));
        if actual.as_deref() != Some(service.key.as_slice()) {
            return Err(HttpAuthError::Service);
        }
        self.account_permit(principal).await.map_err(|error| {
            if error.is::<crate::accounts::AccountDenied>() {
                HttpAuthError::Denied
            } else {
                HttpAuthError::Unavailable
            }
        })
    }
}
