//! Shared request shape, projection and execution for actual native discussions.
//! Authentication and typed target selection remain with each transport adapter.
use super::{PublicApiError, PublicApiResult, PublicUuid, auth::{BoundAccountRead, BoundAccountWrite}};
use crate::domains::{catalog::validation as v, community::mutations as core};
use axum::{body::to_bytes, extract::Request};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::time::Duration;
use tokio_postgres::Transaction;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Operation { Create, Update, Remove, Delete, ThreadVote, ReplyVote }
impl Operation {
    pub(super) fn name(self) -> &'static str { match self {
        Self::Create => "create", Self::Update => "update", Self::Remove => "remove",
        Self::Delete => "delete", Self::ThreadVote => "thread_vote", Self::ReplyVote => "reply_vote",
    } }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct Command {
    pub(super) expected_account_id: PublicUuid,
    pub(super) expected_user_id: Option<PublicUuid>,
    pub(super) expected_target_id: Option<PublicUuid>,
    pub(super) expected_thread_id: String,
    pub(super) operation: Operation,
    pub(super) body: Option<String>,
    pub(super) reply_id: Option<i64>,
    pub(super) parent_id: Option<i64>,
    pub(super) version: Option<i32>,
    pub(super) vote: Option<i16>,
    pub(super) expected_vote: Option<i16>,
}
impl Command {
    pub(super) fn parse(bytes: &[u8]) -> PublicApiResult<Self> { Self::parse_for(bytes, true) }
    pub(super) fn parse_target(bytes: &[u8]) -> PublicApiResult<Self> { Self::parse_for(bytes, false) }
    fn parse_for(bytes: &[u8], user: bool) -> PublicApiResult<Self> {
        // Typed parsing rejects duplicates before Value parsing loses them.
        let mut c: Self = serde_json::from_slice(bytes).map_err(|_| PublicApiError::bad_request("Invalid native discussion command"))?;
        let shape: Value = serde_json::from_slice(bytes).map_err(|_| PublicApiError::bad_request("Invalid native discussion JSON"))?;
        if shape.as_object().ok_or_else(|| PublicApiError::bad_request("Command must be an object"))?.values().any(Value::is_null) {
            return Err(PublicApiError::bad_request("Discussion fields cannot be null"));
        }
        if c.expected_user_id.is_some() != user || c.expected_target_id.is_some() == user {
            return Err(PublicApiError::bad_request("Exactly the current typed target binding is required"));
        }
        v::id(&c.expected_thread_id)?;
        let (body, reply, parent, version, vote) = match c.operation {
            Operation::Create => (true, false, true, false, false),
            Operation::Update => (true, true, false, true, false),
            Operation::Remove | Operation::Delete => (false, true, false, true, false),
            Operation::ThreadVote => (false, false, false, false, true),
            Operation::ReplyVote => (false, true, false, true, true),
        };
        if c.body.is_some() != body || c.reply_id.is_some() != reply || c.version.is_some() != version
            || c.vote.is_some() != vote || c.expected_vote.is_some() != vote || (!parent && c.parent_id.is_some()) {
            return Err(PublicApiError::bad_request("Missing or irrelevant discussion operation fields"));
        }
        if c.reply_id.into_iter().chain(c.parent_id).any(|n| n <= 0 || n > i32::MAX as i64)
            || c.version.is_some_and(|n| n <= 0)
            || c.vote.into_iter().chain(c.expected_vote).any(|n| !(-1..=1).contains(&n)) {
            return Err(PublicApiError::bad_request("Invalid reply, version or vote"));
        }
        if let Some(body) = &mut c.body {
            // Preserve the native trim and NUL policy, and reject the actual
            // persisted octet limit as 400 rather than a late DB constraint error.
            if body.chars().count() > 50_000 { return Err(PublicApiError::bad_request("Body exceeds native 50000 character limit")); }
            *body = body.trim().to_owned();
            if body.is_empty() || body.len() > 50_000 || body.contains('\0') {
                return Err(PublicApiError::bad_request("Body must contain 1 to 50000 UTF-8 bytes"));
            }
        }
        Ok(c)
    }
}
pub(super) async fn body(request: Request) -> PublicApiResult<axum::body::Bytes> {
    tokio::time::timeout(Duration::from_secs(5), to_bytes(request.into_body(), 393_216)).await
        .map_err(|_| PublicApiError::bad_request("Discussion body timeout"))?
        .map_err(|_| PublicApiError::bad_request("Discussion body exceeds 393216 bytes"))
}
fn permitted(a: &BoundAccountRead, owner: &str, action: &str) -> bool {
    (a.is_moderator() && a.has_perm(&format!("reply.{action}")))
        || (a.user_id() == owner && a.has_perm(&format!("self.reply.{action}")))
}
pub(super) async fn view(tx: &Transaction<'_>, actor: &BoundAccountRead, thread: &str, page: i64, order: &str) -> PublicApiResult<Value> {
    let row = tx.query_one("SELECT platform.community_thread_json(id),locked,author_id FROM platform.catalog_threads WHERE id=$1", &[&thread]).await?;
    let dto: Value = row.get(0);
    let locked: bool = row.get(1);
    let owner: String = row.get(2);
    let thread_vote: i16 = tx.query_one("SELECT COALESCE((SELECT vote FROM platform.community_thread_votes WHERE thread_id=$1 AND user_id=$2),0)::SMALLINT", &[&thread, &actor.user_id()]).await?.get(0);
    let total: i64 = tx.query_one("SELECT count(*) FROM platform.community_replies WHERE thread_id=$1", &[&thread]).await?.get(0);
    let offset = (page - 1) * 20;
    let rows = tx.query(&format!("SELECT platform.community_reply_json(id,$2),author_id,state FROM platform.community_replies WHERE thread_id=$1 ORDER BY reply_number {order} LIMIT 20 OFFSET $3"), &[&thread, &actor.user_id(), &offset]).await?;
    let mut data = Vec::new(); let mut actions = Map::new();
    for row in rows {
        let reply: Value = row.get(0); let owner: String = row.get(1); let state: String = row.get(2);
        let id = reply["id"].as_i64().ok_or_else(PublicApiError::unavailable)?;
        let existing_vote = reply["relationships"]["vote_state"].as_i64().unwrap_or(0) != 0;
        actions.insert(id.to_string(), json!({
            "update": state == "active" && (!locked || actor.is_moderator()) && permitted(actor,&owner,"update"),
            "remove": !state.starts_with("deleted_") && permitted(actor,&owner,"remove"),
            "delete": actor.is_moderator() && actor.has_perm("reply.delete"),
            "vote": actor.has_perm("reply.create") && (existing_vote || (!locked && state == "active" && owner != actor.user_id()))
        }));
        data.push(reply);
    }
    let result = json!({"thread":dto,"feed":{"total":total,"limit":20,"page":page,"data":data},"threadVote":thread_vote,"actions":{
        "create": !locked && actor.has_perm("reply.create"),
        "threadVote": actor.has_perm("reply.create") && (thread_vote != 0 || (!locked && owner != actor.user_id())),
        "replies":actions
    }});
    Ok(result)
}
pub(super) async fn execute(tx: &Transaction<'_>, actor: &BoundAccountWrite, thread: &str, c: &Command) -> PublicApiResult<(Option<Value>, bool)> {
    let mut reply = None;
    let changed = match c.operation {
        Operation::Create => { reply = Some(core::create_in(&tx,actor,&thread,c.body.as_deref().unwrap(),c.parent_id).await?); true },
        Operation::Update => { reply = Some(core::update_in(&tx,actor,&thread,c.reply_id.unwrap(),c.body.as_deref().unwrap(),c.version.unwrap()).await?); true },
        Operation::Remove => { let changed = core::remove_in(&tx,actor,&thread,c.reply_id.unwrap(),c.version).await?; reply = Some(core::reply_json(tx,c.reply_id.unwrap(),actor).await?); changed },
        Operation::Delete => { core::delete_in(&tx,actor,&thread,c.reply_id.unwrap(),c.version).await?; true },
        Operation::ThreadVote => core::thread_vote_in(&tx,actor,&thread,c.vote.unwrap(),c.expected_vote).await?,
        Operation::ReplyVote => { let changed = core::reply_vote_in(&tx,actor,&thread,c.reply_id.unwrap(),c.vote.unwrap(),c.expected_vote,c.version).await?; reply = Some(core::reply_json(tx,c.reply_id.unwrap(),actor).await?); changed },
    };
    Ok((reply, changed))
}
