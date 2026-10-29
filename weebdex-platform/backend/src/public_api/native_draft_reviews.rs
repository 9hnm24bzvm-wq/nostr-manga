//! Author-visible review feedback, separate from private staff notes.
use super::PublicApiResult;
use serde_json::{Value, json};
use tokio_postgres::Transaction;

pub(super) async fn timeline(tx: &Transaction<'_>, manga: &str) -> PublicApiResult<Value> {
    history(tx, manga, 100).await
}
/// List cards need only the latest event; never multiply a full timeline by 100 drafts.
pub(super) async fn summary(tx: &Transaction<'_>, manga: &str) -> PublicApiResult<Value> {
    history(tx, manga, 1).await
}
async fn history(tx: &Transaction<'_>, manga: &str, limit: i64) -> PublicApiResult<Value> {
    let rows = tx.query("SELECT version,action,reason,affected_fields,platform.catalog_timestamp(created_at) FROM platform.native_draft_reviews WHERE manga_id=$1 ORDER BY version DESC LIMIT $2", &[&manga, &(limit + 1)]).await?;
    let more = rows.len() > limit as usize;
    let events = rows.into_iter().take(limit as usize).map(|r| json!({"version":r.get::<_,i32>(0),"action":r.get::<_,String>(1),"reason":r.get::<_,String>(2),"affectedFields":r.get::<_,Vec<String>>(3),"createdAt":r.get::<_,String>(4)})).rev().collect::<Vec<_>>();
    Ok(json!({"events":events,"hasEarlier":more}))
}
