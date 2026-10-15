//! Original verification; never platform account translation.
use super::ProtocolFacade;
use manga_node::http_auth::{AdmittedHttpCapability, HttpAuthError, HttpService};
impl ProtocolFacade {
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
