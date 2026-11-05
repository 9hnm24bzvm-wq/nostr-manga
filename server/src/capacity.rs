//! Durable batched byte credits. No expiry automatically releases uncertain I/O.
use crate::db::{Id, Repository};
use anyhow::{Result, ensure};
use deadpool_postgres::Transaction;
use std::collections::HashSet;

impl Repository {
    /// Operator configuration; reducing a limit below allocated credits fails.
    pub async fn configure_budget(&self, key: &str, limit: i64) -> Result<()> {
        ensure!(
            !key.is_empty() && key.len() <= 160 && limit > 0,
            "invalid budget"
        );
        self.pool.get().await?.execute("INSERT INTO capacity_budgets(key,limit_bytes) VALUES($1,$2) ON CONFLICT(key) DO UPDATE SET limit_bytes=EXCLUDED.limit_bytes",&[&key,&limit]).await?;
        Ok(())
    }
    pub async fn allocate_grant(&self, budget: &str, holder: Id, bytes: i64) -> Result<Id> {
        ensure!(bytes > 0, "grant size must be positive");
        let id: Id = rand::random();
        self.pool.get().await?.execute("INSERT INTO capacity_grants(id,budget_key,holder,capacity,available) VALUES($1,$2,$3,$4,$4)",&[&id.as_slice(),&budget,&holder.as_slice(),&bytes]).await?;
        Ok(id)
    }
    /// Seal a worker's grant and return only credits not used or reserved. In-flight
    /// reservations remain accounted and may settle; no new reservation can start.
    pub async fn seal_and_release_unused(&self, id: Id) -> Result<i64> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row = tx
            .query_one(
                &tx.prepare_cached("SELECT available FROM capacity_grants WHERE id=$1 FOR UPDATE")
                    .await?,
                &[&id.as_slice()],
            )
            .await?;
        let available: i64 = row.get(0);
        tx.execute(&tx.prepare_cached("UPDATE capacity_grants SET state='sealed',capacity=capacity-available,available=0 WHERE id=$1").await?,&[&id.as_slice()]).await?;
        tx.commit().await?;
        Ok(available)
    }
}
pub(crate) async fn settle_tx(tx: &Transaction<'_>, id: Id, actual: i64) -> Result<bool> {
    let row = tx
        .query_one(
            &tx.prepare_cached(
                "SELECT requested,actual,state FROM capacity_reservations WHERE id=$1 FOR UPDATE",
            )
            .await?,
            &[&id.as_slice()],
        )
        .await?;
    let requested: i64 = row.get(0);
    ensure!(
        (0..=requested).contains(&actual),
        "actual bytes exceed reservation"
    );
    if row.get::<_, String>(2) == "settled" {
        ensure!(
            row.get::<_, Option<i64>>(1) == Some(actual),
            "settlement differs from previous result"
        );
        return Ok(false);
    }
    let rows=tx.query(&tx.prepare_cached("SELECT g.id FROM capacity_charges c JOIN capacity_grants g ON g.id=c.grant_id WHERE c.reservation=$1 ORDER BY g.id FOR UPDATE OF g").await?,&[&id.as_slice()]).await?;
    ensure!(!rows.is_empty(), "reservation has no grants");
    // Every grant is already locked in sorted order. One set update preserves
    // the paired-budget transition without serial per-grant round trips.
    let grants: Vec<Vec<u8>> = rows.iter().map(|row| row.get(0)).collect();
    let changed = tx.execute(&tx.prepare_cached("UPDATE capacity_grants SET reserved=reserved-$2,used=used+$3,available=available+($2-$3) WHERE id=ANY($1)").await?, &[&grants, &requested, &actual]).await?;
    ensure!(
        changed as usize == grants.len(),
        "settlement grants changed"
    );
    tx.execute(
        &tx.prepare_cached("UPDATE capacity_charges SET charged=$2 WHERE reservation=$1")
            .await?,
        &[&id.as_slice(), &actual],
    )
    .await?;
    tx.execute(
        &tx.prepare_cached(
            "UPDATE capacity_reservations SET state='settled',actual=$2 WHERE id=$1",
        )
        .await?,
        &[&id.as_slice(), &actual],
    )
    .await?;
    Ok(true)
}
/// Refund all still-charged budgets of one settled reservation. Caller has a
/// verified cleanup receipt and owns its blob-generation lock. This function
/// owns reservation then sorted grant locking; retries cannot refund twice.
pub(crate) async fn refund_reservation_tx(tx: &Transaction<'_>, id: Id) -> Result<()> {
    let row = tx
        .query_one(
            &tx.prepare_cached("SELECT state FROM capacity_reservations WHERE id=$1 FOR UPDATE")
                .await?,
            &[&id.as_slice()],
        )
        .await?;
    ensure!(
        row.get::<_, String>(0) == "settled",
        "unsettled reservation cannot be refunded"
    );
    let charges = tx
        .query(
            &tx.prepare_cached(
                "SELECT grant_id,refunded FROM capacity_charges WHERE reservation=$1 ORDER BY grant_id",
            )
            .await?,
            &[&id.as_slice()],
        )
        .await?;
    ensure!(!charges.is_empty(), "reservation charges missing");
    // The settled reservation lock serializes both refund paths. Refunded flags
    // can only change false -> true under that lock, so an already-returned
    // staging charge cannot become writable again. Do not block physical
    // cleanup on a staging grant now serving unrelated foreground admissions.
    let grants: Vec<Vec<u8>> = charges
        .iter()
        .filter(|row| !row.get::<_, bool>(1))
        .map(|row| row.get(0))
        .collect();
    tx.query(
        &tx.prepare_cached(
            "SELECT id FROM capacity_grants WHERE id=ANY($1) ORDER BY id FOR UPDATE",
        )
        .await?,
        &[&grants],
    )
    .await?;
    tx.execute(&tx.prepare_cached(
        "WITH refunded AS (UPDATE capacity_charges SET refunded=TRUE WHERE reservation=$1 AND NOT refunded RETURNING grant_id,charged), totals AS (SELECT grant_id,sum(charged)::bigint AS bytes FROM refunded GROUP BY grant_id) UPDATE capacity_grants g SET used=g.used-totals.bytes,available=g.available+totals.bytes FROM totals WHERE g.id=totals.grant_id"
    ).await?, &[&id.as_slice()]).await?;
    Ok(())
}

pub(crate) async fn reserve_tx(
    tx: &Transaction<'_>,
    id: Id,
    grants: &[Id],
    bytes: i64,
) -> Result<bool> {
    ensure!(
        bytes > 0 && !grants.is_empty() && grants.len() <= 8,
        "reservation bounds"
    );
    let mut keys: Vec<Vec<u8>> = grants.iter().map(|g| g.to_vec()).collect();
    keys.sort();
    keys.dedup();
    ensure!(keys.len() == grants.len(), "duplicate grant");
    let created=tx.execute(&tx.prepare_cached("INSERT INTO capacity_reservations(id,requested) VALUES($1,$2) ON CONFLICT(id) DO NOTHING").await?,&[&id.as_slice(),&bytes]).await?==1;
    if !created {
        let row = tx
            .query_one(
                &tx.prepare_cached("SELECT requested FROM capacity_reservations WHERE id=$1")
                    .await?,
                &[&id.as_slice()],
            )
            .await?;
        ensure!(
            row.get::<_, i64>(0) == bytes,
            "reservation ID reused with different size"
        );
        let old: Vec<Vec<u8>> = tx
            .query(
                &tx.prepare_cached(
                    "SELECT grant_id FROM capacity_charges WHERE reservation=$1 ORDER BY grant_id",
                )
                .await?,
                &[&id.as_slice()],
            )
            .await?
            .iter()
            .map(|r| r.get(0))
            .collect();
        ensure!(old == keys, "reservation ID reused with different grants");
        return Ok(false);
    }
    let rows=tx.query(&tx.prepare_cached("SELECT id,budget_key,available,state FROM capacity_grants WHERE id=ANY($1) ORDER BY id FOR UPDATE").await?,&[&keys]).await?;
    ensure!(rows.len() == keys.len(), "unknown grant");
    let mut budgets = HashSet::new();
    for row in &rows {
        ensure!(row.get::<_, String>(3) == "active", "grant is sealed");
        ensure!(row.get::<_, i64>(2) >= bytes, "grant capacity exhausted");
        ensure!(
            budgets.insert(row.get::<_, String>(1)),
            "multiple grants for one budget"
        );
    }
    // The full grant set was validated and locked above. Derive charge identity
    // from those immutable rows, so any insertion fault rolls back every debit.
    let changed = tx.execute(&tx.prepare_cached(
        "WITH debited AS (UPDATE capacity_grants SET available=available-$2,reserved=reserved+$2 WHERE id=ANY($1) RETURNING id,budget_key) INSERT INTO capacity_charges(reservation,grant_id,budget_key) SELECT $3,id,budget_key FROM debited"
    ).await?, &[&keys, &bytes, &id.as_slice()]).await?;
    ensure!(changed as usize == keys.len(), "reservation grants changed");
    Ok(true)
}
