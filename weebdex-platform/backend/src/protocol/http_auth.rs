//! Original verification; never platform account translation.
use super::ProtocolFacade;
use manga_node::http_auth::{
    AdmittedHttpCapability, HttpAuthError, HttpRequestPreflight, HttpService,
};
impl ProtocolFacade {
    pub async fn preflight_http_request(
        &self,
        service: &HttpService,
        authorization: &str,
        external_uri: &str,
        method: &str,
    ) -> Result<HttpRequestPreflight, HttpAuthError> {
        self.require_schema()
            .map_err(|_| HttpAuthError::Unavailable)?;
        self.repository
            .preflight_admitted_http_request(service, authorization, external_uri, method)
            .await
    }
    pub async fn verify_http_request_digest(
        &self,
        service: &HttpService,
        authorization: &str,
        external_uri: &str,
        method: &str,
        body_hash: manga_node::db::Id,
        body_empty: bool,
    ) -> Result<AdmittedHttpCapability, HttpAuthError> {
        self.require_schema()
            .map_err(|_| HttpAuthError::Unavailable)?;
        self.repository
            .verify_admitted_http_request_digest(
                service,
                authorization,
                external_uri,
                method,
                body_hash,
                body_empty,
            )
            .await
    }
    pub async fn verify_http_request(
        &self,
        service: &HttpService,
        authorization: &str,
        external_uri: &str,
        method: &str,
        body: &[u8],
    ) -> Result<AdmittedHttpCapability, HttpAuthError> {
        self.require_schema()
            .map_err(|_| HttpAuthError::Unavailable)?;
        self.repository
            .verify_admitted_http_request(service, authorization, external_uri, method, body)
            .await
    }
}
