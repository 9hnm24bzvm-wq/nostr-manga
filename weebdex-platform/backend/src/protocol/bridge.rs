//! Durable, restart-safe derived catalogue import over the original module.
use super::ProtocolFacade;
use crate::{
    domains::catalog::CatalogRepository,
    error::{ApiError, ApiResult},
    state::AppState,
};
use manga_node::db::Id;
use serde::Serialize;
use tokio::task::JoinHandle;

#[derive(Debug, Default, Serialize)]
pub struct ImportReport {
    pub scanned: usize,
    pub imported: usize,
    pub hidden: usize,
    pub deferred: usize,
    pub rejected: usize,
    pub completed_sweep: bool,
}
impl ProtocolFacade {
    pub async fn source_page(
        &self,
        after: Option<Id>,
        limit: i64,
    ) -> ApiResult<manga_node::platform_bridge::SourcePage> {
        self.require_schema()?;
        self.repository
            .platform_catalogue_page(after, limit)
            .await
            .map_err(|_| ApiError::unavailable())
    }
}
/// One bounded page. Cursor and materialization share a platform transaction.
/// Replicas serialize on the singleton row; no protocol tables are written.
/// Failed mappings are recorded and revisited on every subsequent full sweep.
pub async fn import_page(state: &AppState) -> ApiResult<ImportReport> {
    if !state.protocol.is_ready() {
        return Err(ApiError::unavailable());
    }
    let observed: Option<String> = {
        let client = state.db.client().await?;
        client
            .query_one(
                "SELECT after_event FROM platform.protocol_import_cursor WHERE singleton",
                &[],
            )
            .await?
            .get(0)
    };
    let after = observed
        .clone()
        .map(|text| {
            hex::decode(text)
                .ok()
                .and_then(|v| v.try_into().ok())
                .ok_or_else(ApiError::unavailable)
        })
        .transpose()?;
    // No checkout is held while the source reads, including on a one-slot pool.
    let page = state.protocol.source_page(after, 16).await?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let actual: Option<String> = tx
        .query_one(
            "SELECT after_event FROM platform.protocol_import_cursor WHERE singleton FOR UPDATE",
            &[],
        )
        .await?
        .get(0);
    if actual != observed {
        return Err(ApiError::conflict(
            "Another importer advanced the source sweep",
        ));
    }
    let mut report = ImportReport {
        scanned: page.entries.len() + page.rejected.len(),
        completed_sweep: page.exhausted,
        ..Default::default()
    };
    for rejection in &page.rejected {
        CatalogRepository::reject_protocol_source(&tx, &rejection.event_id).await?;
        tx.execute("INSERT INTO platform.protocol_import_status(event_id,source_version,source_visible,status,reason) VALUES($1,$2,FALSE,'rejected',$3) ON CONFLICT(event_id) DO UPDATE SET source_version=EXCLUDED.source_version,source_visible=FALSE,status='rejected',reason=EXCLUDED.reason,checked_at=clock_timestamp()", &[&rejection.event_id,&rejection.source_version,&rejection.reason]).await?;
        report.rejected += 1;
        report.hidden += 1;
    }
    for source in &page.entries {
        // Savepoints avoid a rejected descriptor rolling back successful siblings.
        tx.batch_execute("SAVEPOINT source_projection").await?;
        let result =
            CatalogRepository::materialize_protocol(&tx, source, &state.config.public_origin).await;
        let (status, reason) = match result {
            Ok(changed) => {
                tx.batch_execute("RELEASE SAVEPOINT source_projection")
                    .await?;
                if source.visible {
                    if changed {
                        report.imported += 1;
                    }
                    ("imported", None)
                } else {
                    report.hidden += 1;
                    ("hidden", None)
                }
            }
            Err(error) => {
                tx.batch_execute(
                    "ROLLBACK TO SAVEPOINT source_projection; RELEASE SAVEPOINT source_projection",
                )
                .await?;
                if error.status == axum::http::StatusCode::CONFLICT {
                    report.deferred += 1;
                    ("deferred", Some(error.message))
                } else if error.status == axum::http::StatusCode::BAD_REQUEST {
                    report.rejected += 1;
                    ("rejected", Some(error.message))
                } else {
                    return Err(error);
                }
            }
        };
        tx.execute("INSERT INTO platform.protocol_import_status(event_id,source_version,source_visible,status,reason) VALUES($1,$2,$3,$4,$5) ON CONFLICT(event_id) DO UPDATE SET source_version=EXCLUDED.source_version,source_visible=EXCLUDED.source_visible,status=EXCLUDED.status,reason=EXCLUDED.reason,checked_at=clock_timestamp()",&[&source.event.id,&source.source_version,&source.visible,&status,&reason]).await?;
    }
    let next = next_cursor(page.exhausted, page.next_after);
    tx.execute("UPDATE platform.protocol_import_cursor SET after_event=$1,completed_sweeps=completed_sweeps+$2,updated_at=clock_timestamp() WHERE singleton",&[&next,&i64::from(page.exhausted)]).await?;
    tx.commit().await?;
    if report.imported > 0 || report.hidden > 0 {
        state.public_cache.invalidate_all();
    }
    Ok(report)
}
/// Caller owns the task and must abort/await it during graceful shutdown.
/// No per-request spawning, unbounded queue, or second database pool.
pub fn spawn_importer(state: AppState) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match import_page(&state).await {
                Ok(report) => {
                    if let Err(error) = super::covers::materialize_page(&state).await {
                        tracing::warn!(
                            code = error.code,
                            "self-hosted protocol cover derivative remains pending"
                        );
                    }
                    if report.imported > 0 || report.rejected > 0 {
                        tracing::info!(
                            scanned = report.scanned,
                            imported = report.imported,
                            deferred = report.deferred,
                            rejected = report.rejected,
                            "protocol catalogue sweep"
                        );
                    }
                    if report.completed_sweep {
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    } else {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
                Err(_) => {
                    tracing::warn!(
                        "protocol catalogue import unavailable; source remains authoritative"
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }
    })
}
fn next_cursor(exhausted: bool, next_after: Option<Id>) -> Option<String> {
    if exhausted {
        None
    } else {
        next_after.map(hex::encode)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completing_a_sweep_restarts_before_late_lower_hash_commits() {
        let high = [0xf0; 32];
        assert_eq!(next_cursor(false, Some(high)), Some(hex::encode(high)));
        assert_eq!(next_cursor(true, Some(high)), None);
        assert_eq!(next_cursor(true, None), None);
        // A late event sorting before an old cursor is included by the next sweep.
        let late = [0x01; 32];
        let after = next_cursor(true, Some(high));
        assert!(after.is_none());
        assert!(after.as_ref().is_none_or(|a| hex::encode(late) > *a));
    }
}
