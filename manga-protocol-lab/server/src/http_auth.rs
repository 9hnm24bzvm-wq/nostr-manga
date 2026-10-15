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
impl Repository {
    pub async fn verify_admitted_http_request(
        &self,
        service: &HttpService,
        authorization: &str,
        external_uri: &str,
        method: &str,
        body: &[u8],
    ) -> Result<AdmittedHttpCapability, HttpAuthError> {
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
        auth::http(&event, &request_url, method, body).map_err(|_| HttpAuthError::Invalid)?;
        if event.one("service").map_err(|_| HttpAuthError::Invalid)? != hex::encode(service.key) {
            return Err(HttpAuthError::Invalid);
        }
        let principal = identifier(&event.pubkey).ok_or(HttpAuthError::Invalid)?;
        let authorization = identifier(&event.id).ok_or(HttpAuthError::Invalid)?;
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
        let permit = self.account_permit(principal).await.map_err(|error| {
            if error.is::<crate::accounts::AccountDenied>() {
                HttpAuthError::Denied
            } else {
                HttpAuthError::Unavailable
            }
        })?;
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
            body_hash: identifier(&auth::hash(body)).ok_or(HttpAuthError::Invalid)?,
            authorization,
            permit,
        })
    }
}
