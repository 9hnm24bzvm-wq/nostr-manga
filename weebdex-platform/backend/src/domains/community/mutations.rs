//! Real transaction-owned native discussion mutations. Callers authenticate and
//! own the transaction; only genuine sealed mutation actors enter this core.
use crate::{
    domains::catalog::{entity_mutations::CatalogMutationActor, validation as v},
    error::{ApiError, ApiResult},
};
use serde_json::Value;
use tokio_postgres::Transaction;

pub(crate) async fn visible(db: &impl tokio_postgres::GenericClient, id: &str) -> ApiResult<()> {
    v::id(id)?;
    if !db.query_one("SELECT platform.community_thread_visible($1)", &[&id]).await?.get::<_, bool>(0) {
        return Err(ApiError::not_found("Thread not found"));
    }
    Ok(())
}
async fn locked(tx: &Transaction<'_>, id: &str) -> ApiResult<bool> {
    visible(tx, id).await?;
    let row = tx.query_opt("SELECT locked FROM platform.catalog_threads WHERE id=$1 FOR UPDATE", &[&id]).await?
        .ok_or_else(|| ApiError::not_found("Thread not found"))?;
    // Visibility is rechecked after waiting for the native thread lock.
    visible(tx, id).await?;
    Ok(row.get(0))
}
fn authority(a: &impl CatalogMutationActor, owner: &str, action: &str) -> ApiResult<()> {
    if a.is_moderator() && a.has_perm(&format!("reply.{action}")) { return Ok(()); }
    if a.user_id() == owner { a.require_permission(&format!("self.reply.{action}")) }
    else { Err(ApiError::forbidden("Only the reply owner or an authorized moderator may change it")) }
}
async fn audit(tx: &Transaction<'_>, a: &impl CatalogMutationActor, id: &str, reply: i64, operation: &str, old: Option<i32>, new: Option<i32>) -> ApiResult<()> {
    crate::domains::auth::closure::active_in(tx,a.user_id()).await?;
    tx.execute("INSERT INTO platform.community_audit(actor_id,thread_id,reply_id,operation,old_version,new_version) VALUES($1,$2,$3,$4,$5,$6)", &[&a.user_id(), &id, &reply, &operation, &old, &new]).await?;
    Ok(())
}
fn check_version(actual: i32, expected: Option<i32>) -> ApiResult<()> {
    if expected.is_some_and(|v| v != actual) { return Err(ApiError::conflict("Reply changed; reload before submitting")); }
    Ok(())
}
fn check_vote(actual: i16, expected: Option<i16>) -> ApiResult<()> {
    if expected.is_some_and(|v| v != actual) { return Err(ApiError::conflict("Vote changed; reload before submitting")); }
    Ok(())
}
pub(crate) async fn reply_json(tx: &Transaction<'_>, reply: i64, actor: &impl CatalogMutationActor) -> ApiResult<Value> {
    crate::domains::auth::closure::active_in(tx,actor.user_id()).await?;
    Ok(tx.query_one("SELECT platform.community_reply_json($1,$2)", &[&reply, &actor.user_id()]).await?.get(0))
}
pub(crate) async fn create_in(tx: &Transaction<'_>, a: &impl CatalogMutationActor, id: &str, body: &str, parent: Option<i64>) -> ApiResult<Value> {
    crate::domains::auth::closure::active_in(tx,a.user_id()).await?;
    a.require_permission("reply.create")?;
    if locked(tx, id).await? { return Err(ApiError::forbidden("Thread is locked")); }
    if let Some(parent) = parent {
        if tx.query_opt("SELECT id FROM platform.community_replies WHERE id=$1 AND thread_id=$2 AND state='active' FOR SHARE", &[&parent, &id]).await?.is_none() {
            return Err(ApiError::bad_request("Parent reply must be active and belong to this thread"));
        }
    }
    let n: i32 = tx.query_one("UPDATE platform.catalog_threads SET next_reply_number=next_reply_number+1,updated_at=clock_timestamp() WHERE id=$1 RETURNING next_reply_number-1", &[&id]).await?.get(0);
    let r: i64 = tx.query_one("INSERT INTO platform.community_replies(thread_id,reply_number,parent_id,author_id,body) VALUES($1,$2,$3,$4,$5) RETURNING id", &[&id, &n, &parent, &a.user_id(), &body]).await?.get(0);
    audit(tx, a, id, r, "create", None, Some(1)).await?;
    reply_json(tx, r, a).await
}
pub(crate) async fn update_in(tx: &Transaction<'_>, a: &impl CatalogMutationActor, id: &str, r: i64, body: &str, version: i32) -> ApiResult<Value> {
    crate::domains::auth::closure::active_in(tx,a.user_id()).await?;
    let thread_locked = locked(tx, id).await?;
    let row = tx.query_opt("SELECT author_id,version,state FROM platform.community_replies WHERE thread_id=$1 AND id=$2 FOR UPDATE", &[&id, &r]).await?.ok_or_else(|| ApiError::not_found("Reply not found"))?;
    authority(a, &row.get::<_, String>(0), "update")?;
    if thread_locked && !a.is_moderator() { return Err(ApiError::forbidden("Thread is locked")); }
    if row.get::<_, String>(2) != "active" { return Err(ApiError::conflict("Only active replies may be edited")); }
    check_version(row.get(1), Some(version))?;
    tx.execute("UPDATE platform.community_replies SET body=$3,version=version+1,updated_at=clock_timestamp() WHERE thread_id=$1 AND id=$2", &[&id, &r, &body]).await?;
    audit(tx, a, id, r, "update", Some(version), Some(version + 1)).await?;
    reply_json(tx, r, a).await
}
pub(crate) async fn remove_in(tx: &Transaction<'_>, a: &impl CatalogMutationActor, id: &str, r: i64, expected_version: Option<i32>) -> ApiResult<bool> {
    crate::domains::auth::closure::active_in(tx,a.user_id()).await?;
    locked(tx, id).await?;
    let row = tx.query_opt("SELECT author_id,version,state FROM platform.community_replies WHERE thread_id=$1 AND id=$2 FOR UPDATE", &[&id, &r]).await?.ok_or_else(|| ApiError::not_found("Reply not found"))?;
    let owner: String = row.get(0);
    authority(a, &owner, "remove")?;
    let version: i32 = row.get(1);
    check_version(version, expected_version)?;
    if row.get::<_, String>(2).starts_with("deleted_") { return Ok(false); }
    let state = if owner == a.user_id() { "deleted_by_user" } else { "deleted_by_moderator" };
    tx.execute("UPDATE platform.community_replies SET state=$3,version=version+1,updated_at=clock_timestamp() WHERE thread_id=$1 AND id=$2", &[&id, &r, &state]).await?;
    tx.execute("DELETE FROM platform.community_reply_votes WHERE reply_id=$1", &[&r]).await?;
    audit(tx, a, id, r, "remove", Some(version), Some(version + 1)).await?;
    Ok(true)
}
pub(crate) async fn delete_in(tx: &Transaction<'_>, a: &impl CatalogMutationActor, id: &str, r: i64, expected_version: Option<i32>) -> ApiResult<()> {
    crate::domains::auth::closure::active_in(tx,a.user_id()).await?;
    if !a.is_moderator() { return Err(ApiError::forbidden("Moderator deletion permission required")); }
    a.require_permission("reply.delete")?;
    locked(tx, id).await?;
    let row = tx.query_opt("SELECT version,reply_number FROM platform.community_replies WHERE thread_id=$1 AND id=$2 FOR UPDATE", &[&id, &r]).await?.ok_or_else(|| ApiError::not_found("Reply not found"))?;
    check_version(row.get(0), expected_version)?;
    tx.execute("DELETE FROM platform.community_replies WHERE thread_id=$1 AND id=$2", &[&id, &r]).await?;
    // Retained native deep links calculate pages from dense reply ordinals.
    let removed_number: i32 = row.get(1);
    tx.execute("UPDATE platform.community_replies SET reply_number=reply_number-1 WHERE thread_id=$1 AND reply_number>$2", &[&id, &removed_number]).await?;
    tx.execute("UPDATE platform.catalog_threads SET next_reply_number=next_reply_number-1,updated_at=clock_timestamp() WHERE id=$1", &[&id]).await?;
    audit(tx, a, id, r, "delete", Some(row.get(0)), None).await
}
pub(crate) async fn thread_vote_in(tx: &Transaction<'_>, a: &impl CatalogMutationActor, id: &str, vote: i16, expected: Option<i16>) -> ApiResult<bool> {
    crate::domains::auth::closure::active_in(tx,a.user_id()).await?;
    a.require_permission("reply.create")?;
    let thread_locked = locked(tx, id).await?;
    if vote != 0 {
        if thread_locked { return Err(ApiError::forbidden("Thread is locked")); }
        let owner: String = tx.query_one("SELECT author_id FROM platform.catalog_threads WHERE id=$1", &[&id]).await?.get(0);
        if owner == a.user_id() { return Err(ApiError::forbidden("You cannot vote on your own thread")); }
    }
    let old: i16 = tx.query_one("SELECT COALESCE((SELECT vote FROM platform.community_thread_votes WHERE thread_id=$1 AND user_id=$2),0)::SMALLINT", &[&id, &a.user_id()]).await?.get(0);
    check_vote(old, expected)?;
    if old == vote && expected.is_some() { return Ok(false); }
    if vote == 0 { tx.execute("DELETE FROM platform.community_thread_votes WHERE thread_id=$1 AND user_id=$2", &[&id, &a.user_id()]).await?; }
    else { tx.execute("INSERT INTO platform.community_thread_votes(thread_id,user_id,vote) VALUES($1,$2,$3) ON CONFLICT(thread_id,user_id) DO UPDATE SET vote=EXCLUDED.vote,updated_at=clock_timestamp()", &[&id, &a.user_id(), &vote]).await?; }
    Ok(old != vote)
}
pub(crate) async fn reply_vote_in(tx: &Transaction<'_>, a: &impl CatalogMutationActor, id: &str, r: i64, vote: i16, expected: Option<i16>, expected_version: Option<i32>) -> ApiResult<bool> {
    crate::domains::auth::closure::active_in(tx,a.user_id()).await?;
    a.require_permission("reply.create")?;
    let thread_locked = locked(tx, id).await?;
    // As in the native API, clearing an existing vote remains allowed after locking/removal.
    if vote != 0 && thread_locked { return Err(ApiError::forbidden("Thread is locked")); }
    let row = tx.query_opt("SELECT author_id,state,version FROM platform.community_replies WHERE thread_id=$1 AND id=$2 FOR SHARE", &[&id, &r]).await?.ok_or_else(|| ApiError::not_found("Reply not found"))?;
    if vote != 0 {
        if row.get::<_, String>(0) == a.user_id() { return Err(ApiError::forbidden("You cannot vote on your own reply")); }
        if row.get::<_, String>(1) != "active" { return Err(ApiError::conflict("Reply cannot receive votes")); }
    }
    check_version(row.get(2), expected_version)?;
    let old: i16 = tx.query_one("SELECT COALESCE((SELECT vote FROM platform.community_reply_votes WHERE reply_id=$1 AND user_id=$2),0)::SMALLINT", &[&r, &a.user_id()]).await?.get(0);
    check_vote(old, expected)?;
    if old == vote && expected.is_some() { return Ok(false); }
    if vote == 0 { tx.execute("DELETE FROM platform.community_reply_votes WHERE reply_id=$1 AND user_id=$2", &[&r, &a.user_id()]).await?; }
    else { tx.execute("INSERT INTO platform.community_reply_votes(reply_id,user_id,vote) VALUES($1,$2,$3) ON CONFLICT(reply_id,user_id) DO UPDATE SET vote=EXCLUDED.vote,updated_at=clock_timestamp()", &[&r, &a.user_id(), &vote]).await?; }
    Ok(old != vote)
}
