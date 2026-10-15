//! Trusted in-process adapter into the single original runtime. No native
//! account translation, server signature, URL forwarding or new storage owner.
use super::ProtocolFacade;
use manga_node::{
    http_auth::{AdmittedHttpCapability, HttpRequestPreflight},
    library_admission::Receipt,
    library_http_mutation::{HttpMutationReceipt, MappedHttpIntent},
};
impl ProtocolFacade {
    pub async fn receive_library_http_intent(
        &self,
        preflight: &HttpRequestPreflight,
        declared: Option<usize>,
    ) -> anyhow::Result<Receipt> {
        let runtime = self
            .publication_runtime()
            .await
            .map_err(|_| anyhow::anyhow!("original library runtime unavailable"))?;
        runtime
            .library_objects
            .receive_http_intent(preflight, declared)
    }
    pub async fn apply_library_http_intent(
        &self,
        capability: AdmittedHttpCapability,
        mapped: MappedHttpIntent,
        body: Vec<u8>,
        receipt: Receipt,
    ) -> anyhow::Result<HttpMutationReceipt> {
        let runtime = self
            .publication_runtime()
            .await
            .map_err(|_| anyhow::anyhow!("original library runtime unavailable"))?;
        runtime
            .library_objects
            .apply_http_intent(capability, mapped, body, receipt)
            .await
    }
}
