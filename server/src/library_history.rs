//! Bounded removal of obsolete library event bodies. Winner clocks survive.
use crate::db::Repository;
use anyhow::Result;
const BATCH_LIMIT: i64 = 16;
/// Result of one bounded maintenance transaction. Continuation is derived from
/// the selected page, not the number surviving a concurrent eligibility recheck.
/// Callers must not duplicate the private SQL page size to choose idle pacing.
pub struct HistoryPrune {
    pub removed: usize,
    pub more: bool,
}
impl Repository {
    /// Library-only history, after backend-confirmed retirement and local grace.
    /// Does not remove current heads, winner watermarks, pending work or manga.
    pub async fn prune_library_history(&self) -> Result<HistoryPrune> {
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        // No queue/event locks before author locks: same order as head admission
        // and backend acknowledgement. Recheck everything after taking locks.
        let candidates=tx.query(&tx.prepare_cached("SELECT e.id,e.publisher FROM events e WHERE e.kind=11999 AND e.state='removed' AND e.retired_at IS NOT NULL AND e.retired_at<=statement_timestamp()-interval '5 minutes' AND NOT EXISTS(SELECT 1 FROM account_heads h WHERE h.event_id=e.id) AND NOT EXISTS(SELECT 1 FROM backend_outbox o WHERE o.event_id=e.id AND o.completed_at IS NULL) AND NOT EXISTS(SELECT 1 FROM library_object_holds h WHERE h.wait_event=e.id) AND NOT EXISTS(SELECT 1 FROM publication_blobs p WHERE p.event_id=e.id) AND NOT EXISTS(SELECT 1 FROM publication_tombstones t WHERE t.event_id=e.id) ORDER BY e.retired_at,e.id LIMIT $1").await?,&[&BATCH_LIMIT]).await?;
        if candidates.is_empty() {
            return Ok(HistoryPrune {
                removed: 0,
                more: false,
            });
        }
        let more = candidates.len() == BATCH_LIMIT as usize;
        let ids: Vec<Vec<u8>> = candidates.iter().map(|r| r.get(0)).collect();
        let authors: Vec<Vec<u8>> = candidates.iter().map(|r| r.get(1)).collect();
        tx.query(&tx.prepare_cached("SELECT publisher FROM account_permissions WHERE publisher=ANY($1) ORDER BY publisher FOR UPDATE").await?,&[&authors]).await?;
        let eligible=tx.query(&tx.prepare_cached("SELECT e.id FROM events e WHERE e.id=ANY($1) AND e.kind=11999 AND e.state='removed' AND e.retired_at<=statement_timestamp()-interval '5 minutes' AND NOT EXISTS(SELECT 1 FROM account_heads h WHERE h.event_id=e.id) AND NOT EXISTS(SELECT 1 FROM backend_outbox o WHERE o.event_id=e.id AND o.completed_at IS NULL) AND NOT EXISTS(SELECT 1 FROM library_object_holds h WHERE h.wait_event=e.id) AND NOT EXISTS(SELECT 1 FROM publication_blobs p WHERE p.event_id=e.id) AND NOT EXISTS(SELECT 1 FROM publication_tombstones t WHERE t.event_id=e.id) ORDER BY e.id FOR UPDATE OF e").await?,&[&ids]).await?;
        let ids: Vec<Vec<u8>> = eligible.iter().map(|r| r.get(0)).collect();
        tx.execute(
            &tx.prepare_cached(
                "DELETE FROM backend_outbox WHERE event_id=ANY($1) AND completed_at IS NOT NULL",
            )
            .await?,
            &[&ids],
        )
        .await?;
        let count = tx
            .execute(
                &tx.prepare_cached("DELETE FROM events WHERE id=ANY($1)")
                    .await?,
                &[&ids],
            )
            .await?;
        tx.commit().await?;
        Ok(HistoryPrune {
            removed: count as usize,
            more,
        })
    }
}
