//! One-time staging transfer, separate from receipt/health row locks. Callers
//! hold shared generation locks and validate health before retaining content.
use crate::db::Id;
use anyhow::{Result, ensure};
use deadpool_postgres::Transaction;
pub(crate) async fn retain_blobs_tx(
    tx: &Transaction<'_>,
    hashes: &[Vec<u8>],
    owner: Id,
) -> Result<()> {
    // Both callers already hold current blob-generation SHARE locks and have
    // checked owner/availability. The composite FK guarantees credit generations
    // belong to those locked blobs; avoid repeating an estimate-sensitive join.
    let count: i64 = tx
        .query_one(
            &tx.prepare_cached("SELECT count(*) FROM blob_retention_credits WHERE hash=ANY($1)")
                .await?,
            &[&hashes],
        )
        .await?
        .get(0);
    ensure!(
        count as usize == hashes.len(),
        "retention accounting missing"
    );
    // Sparse partial-index lookup. Separate primary-key lookups keep an empty or
    // newly bulk-loaded catalog from selecting a whole-table hash join here.
    let rows=tx.query(&tx.prepare_cached("SELECT hash FROM blob_retention_credits WHERE hash=ANY($1) AND NOT staging_released ORDER BY hash FOR UPDATE").await?, &[&hashes]).await?;
    let changed: Vec<Vec<u8>> = rows.iter().map(|r| r.get(0)).collect();
    if changed.is_empty() {
        return Ok(());
    }
    let rows = tx
        .query(
            &tx.prepare_cached(
                "SELECT reservation FROM blob_intents WHERE hash=ANY($1) ORDER BY hash",
            )
            .await?,
            &[&changed],
        )
        .await?;
    ensure!(rows.len() == changed.len(), "retained blob receipt missing");
    let mut reservations: Vec<Vec<u8>> = rows.iter().map(|r| r.get(0)).collect();
    reservations.sort();
    reservations.dedup();
    let settled = tx.query(
        &tx.prepare_cached(
            "SELECT id,state FROM capacity_reservations WHERE id=ANY($1) ORDER BY id FOR UPDATE",
        )
        .await?,
        &[&reservations],
    )
    .await?;
    ensure!(
        settled.len() == reservations.len()
            && settled
                .iter()
                .all(|row| row.get::<_, String>(1) == "settled"),
        "unsettled or missing retention reservation"
    );
    let staging = format!("staging:{}", hex::encode(owner));
    let rows = tx
        .query(
            &tx.prepare_cached(
                "SELECT grant_id FROM capacity_charges WHERE reservation=ANY($1) AND budget_key=$2",
            )
            .await?,
            &[&reservations, &staging],
        )
        .await?;
    ensure!(rows.len() == reservations.len(), "staging charge missing");
    let mut grants: Vec<Vec<u8>> = rows.iter().map(|r| r.get(0)).collect();
    grants.sort();
    grants.dedup();
    tx.query(
        &tx.prepare_cached(
            "SELECT id FROM capacity_grants WHERE id=ANY($1) ORDER BY id FOR UPDATE",
        )
        .await?,
        &[&grants],
    )
    .await?;
    // Reservation and grant rows are already locked in global order. Change
    // only previously unrefunded charges and return each grant's exact aggregate
    // once; repeated retention cannot manufacture staging capacity.
    tx.execute(&tx.prepare_cached(
        "WITH refunded AS (UPDATE capacity_charges SET refunded=TRUE WHERE reservation=ANY($1) AND budget_key=$2 AND NOT refunded RETURNING grant_id,charged), totals AS (SELECT grant_id,sum(charged)::bigint AS bytes FROM refunded GROUP BY grant_id) UPDATE capacity_grants g SET used=g.used-totals.bytes,available=g.available+totals.bytes FROM totals WHERE g.id=totals.grant_id"
    ).await?, &[&reservations, &staging]).await?;
    tx.execute(
        &tx.prepare_cached(
            "UPDATE blob_retention_credits SET staging_released=TRUE WHERE hash=ANY($1)",
        )
        .await?,
        &[&changed],
    )
    .await?;
    Ok(())
}
