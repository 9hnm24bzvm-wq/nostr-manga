//! Read-only signed library facade over the exact shared original runtime owner.
use super::ProtocolFacade;
use manga_node::{
    db::Id, http_auth::AdmittedHttpCapability, library_lists::PublicListSnapshot,
    library_view::ReadMarkerSnapshot,
};
impl ProtocolFacade {
    pub async fn public_read_markers(
        &self,
        capability: &AdmittedHttpCapability,
        series: &[Id],
        expected_head: Option<&str>,
    ) -> anyhow::Result<ReadMarkerSnapshot> {
        let runtime = self
            .publication_runtime()
            .await
            .map_err(|_| anyhow::anyhow!("original library runtime unavailable"))?;
        runtime
            .library_objects
            .public_read_markers(capability, series, expected_head)
            .await
    }
    pub async fn verify_read_marker_snapshot(
        &self,
        capability: &AdmittedHttpCapability,
        snapshot: &ReadMarkerSnapshot,
    ) -> anyhow::Result<()> {
        let runtime = self
            .publication_runtime()
            .await
            .map_err(|_| anyhow::anyhow!("original library runtime unavailable"))?;
        runtime
            .library_objects
            .verify_read_marker_snapshot(capability, snapshot)
            .await
    }
    pub async fn public_lists(
        &self,
        publisher: Id,
        expected: Option<&str>,
        offset: u64,
        limit: usize,
        target: Option<[u8; 16]>,
    ) -> anyhow::Result<PublicListSnapshot> {
        let runtime = self
            .publication_runtime()
            .await
            .map_err(|_| anyhow::anyhow!("original library runtime unavailable"))?;
        runtime
            .library_objects
            .public_lists(publisher, expected, offset, limit, target)
            .await
    }
    pub async fn verify_public_list_snapshot(
        &self,
        snapshot: &PublicListSnapshot,
        capability: Option<&AdmittedHttpCapability>,
    ) -> anyhow::Result<()> {
        let runtime = self
            .publication_runtime()
            .await
            .map_err(|_| anyhow::anyhow!("original library runtime unavailable"))?;
        if let Some(capability) = capability {
            runtime
                .library_objects
                .verify_admitted_public_head(
                    capability,
                    snapshot.publisher,
                    snapshot.head.as_deref(),
                )
                .await?;
        } else {
            runtime
                .library_objects
                .verify_public_list_snapshot(snapshot)
                .await?;
        }
        Ok(())
    }
}
