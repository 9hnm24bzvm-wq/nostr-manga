//! Read-only facade reusing the exact shared original LibraryObjects owner.
use super::ProtocolFacade;
use manga_node::{
    db::Id, http_auth::AdmittedHttpCapability, library_statuses::ReadingStatusSnapshot,
};
impl ProtocolFacade {
    pub async fn public_reading_statuses(
        &self,
        cap: &AdmittedHttpCapability,
        series: Option<Id>,
        status: Option<&str>,
        head: Option<&str>,
    ) -> anyhow::Result<ReadingStatusSnapshot> {
        let runtime = self
            .publication_runtime()
            .await
            .map_err(|_| anyhow::anyhow!("original library unavailable"))?;
        runtime
            .library_objects
            .public_reading_statuses(cap, series, status, head)
            .await
    }
    pub async fn verify_reading_status_snapshot(
        &self,
        cap: &AdmittedHttpCapability,
        snapshot: &ReadingStatusSnapshot,
    ) -> anyhow::Result<()> {
        let runtime = self
            .publication_runtime()
            .await
            .map_err(|_| anyhow::anyhow!("original library unavailable"))?;
        runtime
            .library_objects
            .verify_admitted_public_head(cap, snapshot.publisher, snapshot.head.as_deref())
            .await
    }
}
