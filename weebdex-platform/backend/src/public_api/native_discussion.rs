//! Signed native manga/chapter/group discussions, never protocol-source comments.
//! A real typed target and its actual persisted thread remain inseparable.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdTarget,
    PublicUuid, ResolvedPublicId, ResourceKind,
    auth::{admitted_request, bound_account_read, bound_account_write},
    native_discussion_common::{self as common, Command},
    native_user_discussion::private_headers, query::PublicQuery,
};
use crate::{AppState, domains::{catalog::validation as v, community::mutations as core, library::account_public_id_in}};
use axum::{Json, Router, extract::{OriginalUri, Path, Request, State}, http::{HeaderMap, header}, response::{IntoResponse, Response}, routing::get};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tokio_postgres::Transaction;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Kind { Manga, Chapter, Group }
impl Kind {
    pub(crate) fn parse(value: &str) -> PublicApiResult<Self> { match value {
        "manga" => Ok(Self::Manga), "chapter" => Ok(Self::Chapter), "group" => Ok(Self::Group), _ => Err(PublicApiError::not_found())
    } }
    pub(crate) fn name(self) -> &'static str { match self { Self::Manga => "manga", Self::Chapter => "chapter", Self::Group => "group" } }
    fn resource(self) -> ResourceKind { match self { Self::Manga => ResourceKind::Manga, Self::Chapter => ResourceKind::Chapter, Self::Group => ResourceKind::Group } }
}
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/nostr/{kind}/{id}/discussion", get(read).post(write))
        .route("/nostr/native-forum/threads/{thread}/context", get(context))
        .layer(axum::middleware::from_fn(private_headers))
}
fn lock_error(error: tokio_postgres::Error) -> PublicApiError {
    if matches!(error.code(), Some(&tokio_postgres::error::SqlState::LOCK_NOT_AVAILABLE) | Some(&tokio_postgres::error::SqlState::T_R_SERIALIZATION_FAILURE)) {
        PublicApiError::conflict("Discussion target is being changed; reload before retrying")
    } else { error.into() }
}
fn metadata(account: &str, kind: Kind, target: &PublicUuid, thread: &str) -> Value {
    json!({"scope":"instance-account","source":"persisted-native-discussion","identity":"native_catalogue_resource","accountId":account,"targetKind":kind.name(),"targetId":target,"threadId":thread})
}
/// Protect live protocol visibility through the rows that all relevant writers
/// must first lock: event lifecycle/outbox writers lock events, and quarantine
/// insertion/removal locks blob_intents. Merely locking existing negative rows
/// would not fence a newly inserted quarantine or outbox entry.
pub(super) async fn source_fences(tx: &Transaction<'_>, sources: Vec<Option<String>>) -> PublicApiResult<()> {
    let mut events: Vec<String> = sources.into_iter().flatten().collect(); events.sort(); events.dedup();
    if events.is_empty() { return Ok(()); }
    let event_ids: Vec<Vec<u8>> = events.iter().map(|event| hex::decode(event).ok().filter(|id| id.len()==32).ok_or_else(PublicApiError::not_found)).collect::<PublicApiResult<_>>()?;
    let mut manifests = Vec::new();
    for (event,event_id) in events.iter().zip(&event_ids) {
        let row = tx.query_opt("SELECT manifest FROM public.events WHERE id=$1 AND kind IN(1001,1002) FOR SHARE NOWAIT", &[&event_id]).await.map_err(lock_error)?.ok_or_else(PublicApiError::not_found)?;
        let manifest: Option<Vec<u8>> = row.get(0);
        manifests.push((event.clone(), manifest.ok_or_else(PublicApiError::not_found)?));
    }
    // Event SHARE keeps publication_blobs generation membership stable; every
    // legitimate insertion/removal takes event UPDATE before changing it.
    let references = tx.query("SELECT encode(event_id,'hex'),hash,generation FROM public.publication_blobs WHERE event_id=ANY($1::bytea[]) ORDER BY hash,generation,event_id LIMIT 16385", &[&event_ids]).await?;
    if references.len() > 16384 { return Err(PublicApiError::conflict("Discussion source witness exceeds its bounded read budget")); }
    for (event, manifest) in &manifests {
        if !references.iter().any(|row| row.get::<_, String>(0) == *event && row.get::<_, Vec<u8>>(1) == *manifest) { return Err(PublicApiError::not_found()); }
    }
    let hashes: Vec<Vec<u8>> = references.iter().map(|row| row.get(1)).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    let blobs = tx.query("SELECT hash,generation,state FROM public.blob_intents WHERE hash=ANY($1) ORDER BY hash FOR SHARE NOWAIT", &[&hashes]).await.map_err(lock_error)?;
    let blobs: std::collections::BTreeMap<Vec<u8>,(Vec<u8>,String)> = blobs.into_iter().map(|row| (row.get(0),(row.get(1),row.get(2)))).collect();
    for reference in &references {
        let hash: Vec<u8> = reference.get(1); let generation: Vec<u8> = reference.get(2);
        if !blobs.get(&hash).is_some_and(|(actual,state)| *actual == generation && state == "available") { return Err(PublicApiError::not_found()); }
    }
    for event in &events {
        if !tx.query_one("SELECT platform.protocol_source_is_visible($1)", &[&event]).await?.get::<_, bool>(0) { return Err(PublicApiError::not_found()); }
    }
    Ok(())
}
pub(super) async fn manga(tx: &Transaction<'_>, id: &str) -> PublicApiResult<Option<String>> {
    Ok(tx.query_opt("SELECT protocol_series_id FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND state='published' AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id) FOR SHARE NOWAIT", &[&id]).await.map_err(lock_error)?.ok_or_else(PublicApiError::not_found)?.get(0))
}
pub(super) async fn chapter(tx: &Transaction<'_>, id: &str) -> PublicApiResult<Vec<Option<String>>> {
    let row = tx.query_opt("SELECT manga_id,protocol_event_id FROM platform.catalog_chapters WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_source_is_visible(protocol_event_id) FOR SHARE NOWAIT", &[&id]).await.map_err(lock_error)?.ok_or_else(PublicApiError::not_found)?;
    Ok(vec![row.get(1), manga(tx, &row.get::<_, String>(0)).await?])
}
pub(crate) async fn target(tx: &Transaction<'_>, kind: Kind, id: &PublicUuid, expected_thread: &str, write: bool) -> PublicApiResult<ResolvedPublicId> {
    v::id(expected_thread)?;
    let resolved = PostgresPublicIdResolver::resolve_many_in(tx, kind.resource(), &[id.clone()]).await?.into_values().next().filter(|r| r.target == PublicIdTarget::NativeResource).ok_or_else(PublicApiError::not_found)?;
    // Alias preference is presentation, not authority, but must not move between
    // review and command. No missing alias or relation is ever manufactured.
    let aliases = tx.query("SELECT public_id::text FROM platform.public_resource_ids WHERE kind=$1 AND native_id=$2 AND (preferred OR public_id=$3::text::uuid) ORDER BY public_id FOR SHARE NOWAIT", &[&kind.name(), &resolved.native_id, &id.as_str()]).await.map_err(lock_error)?;
    if aliases.is_empty() { return Err(PublicApiError::not_found()); }
    let again = PostgresPublicIdResolver::resolve_many_in(tx, kind.resource(), &[id.clone()]).await?.into_values().next().ok_or_else(PublicApiError::not_found)?;
    if again.native_id != resolved.native_id || again.preferred_id != resolved.preferred_id || again.target != PublicIdTarget::NativeResource { return Err(PublicApiError::conflict("Discussion target mapping changed; reload")); }
    let sources = match kind {
        Kind::Manga => vec![manga(tx, &resolved.native_id).await?],
        Kind::Chapter => chapter(tx, &resolved.native_id).await?,
        Kind::Group => {
            tx.query_opt("SELECT id FROM platform.catalog_groups WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_group_is_visible(id) FOR SHARE NOWAIT", &[&resolved.native_id]).await.map_err(lock_error)?.ok_or_else(PublicApiError::not_found)?;
            let imported = tx.query_opt("SELECT catalog_id FROM platform.protocol_resource_ids WHERE kind='group' AND catalog_id=$1 FOR SHARE NOWAIT", &[&resolved.native_id]).await.map_err(lock_error)?.is_some();
            if imported {
                // One stable, still-visible real witness is sufficient. Do not
                // lock all chapters attributed to a potentially large group.
                let row = tx.query_opt("SELECT cg.chapter_id FROM platform.catalog_chapter_groups cg JOIN platform.catalog_chapters c ON c.id=cg.chapter_id JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE cg.group_id=$1 AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(c.protocol_event_id) AND platform.protocol_source_is_visible(m.protocol_series_id) ORDER BY c.id LIMIT 1 FOR SHARE OF cg NOWAIT", &[&resolved.native_id]).await.map_err(lock_error)?.ok_or_else(PublicApiError::not_found)?;
                chapter(tx, &row.get::<_, String>(0)).await?
            } else { vec![] }
        }
    };
    source_fences(tx, sources).await?;
    let sql = if write { "SELECT id FROM platform.catalog_threads WHERE entity_type=$1 AND entity_id=$2 FOR UPDATE NOWAIT" } else { "SELECT id FROM platform.catalog_threads WHERE entity_type=$1 AND entity_id=$2 FOR SHARE NOWAIT" };
    let thread: String = tx.query_opt(sql, &[&kind.name(), &resolved.native_id]).await.map_err(lock_error)?.ok_or_else(PublicApiError::not_found)?.get(0);
    if thread != expected_thread { return Err(PublicApiError::conflict("Actual discussion changed; reload before continuing")); }
    core::visible(tx, &thread).await?;
    Ok(resolved)
}
async fn context(State(state): State<AppState>, OriginalUri(uri): OriginalUri, Path(thread): Path<String>, request: Request) -> PublicApiResult<Response> {
    if uri.query().is_some() { return Err(PublicApiError::bad_request("Thread context accepts no query")); }
    if !common::body(request).await?.is_empty() { return Err(PublicApiError::bad_request("Thread context GET accepts no body")); }
    v::id(&thread)?;
    let mut db = state.db.client().await?; let tx = db.transaction().await?;
    let row = tx.query_opt("SELECT t.entity_type,a.public_id::text FROM platform.catalog_threads t JOIN platform.public_resource_ids a ON a.kind=t.entity_type AND a.native_id=t.entity_id AND a.preferred AND a.principal_type='native' WHERE t.id=$1 AND t.entity_type IN('manga','chapter','group')", &[&thread]).await?.ok_or_else(PublicApiError::not_found)?;
    let kind = Kind::parse(&row.get::<_, String>(0))?; let id = PublicUuid::parse(&row.get::<_, String>(1))?;
    let resolved = target(&tx, kind, &id, &thread, false).await?;
    let value = json!({"result":"ok","data":{"kind":kind.name(),"id":resolved.preferred_id,"threadId":thread},"nostr":{"source":"persisted-native-discussion-target"}});
    tx.commit().await?; Ok(Json(value).into_response())
}
async fn read(State(state): State<AppState>, OriginalUri(uri): OriginalUri, Path((kind,id)): Path<(String,String)>, headers: HeaderMap, request: Request) -> PublicApiResult<Response> {
    let kind = Kind::parse(&kind)?; let id = PublicUuid::parse(&id)?;
    let q = PublicQuery::parse_unchecked(uri.query())?; q.require_allowed(&["expectedAccountId","expectedThreadId","page","order"])?;
    let expected_account = PublicUuid::parse(q.first("expectedAccountId").ok_or_else(|| PublicApiError::bad_request("expectedAccountId is required"))?)?;
    let expected_thread = q.first("expectedThreadId").ok_or_else(|| PublicApiError::bad_request("expectedThreadId is required"))?;
    let page = q.bounded_usize("page",1,1,1_000_000)? as i64;
    let order = match q.first("order").unwrap_or("desc") { "asc" => "ASC", "desc" => "DESC", _ => return Err(PublicApiError::bad_request("Invalid reply order")) };
    let method = request.method().as_str().to_owned(); let bytes = common::body(request).await?;
    if !bytes.is_empty() { return Err(PublicApiError::bad_request("Discussion GET accepts no body")); }
    let cap = admitted_request(&state, &uri, &headers, &method, &bytes).await?;
    let mut db = state.db.client().await?; let tx = db.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).start().await?;
    let actor = bound_account_read(&tx, &cap).await?; let account = account_public_id_in(&tx, actor.user_id()).await?;
    if expected_account.as_str() != account { return Err(PublicApiError::conflict("Account binding changed; refresh before reading")); }
    let resolved = target(&tx, kind, &id, expected_thread, false).await?;
    let data = common::view(&tx, &actor, expected_thread, page, order).await?;
    let result = json!({"result":"ok","data":data,"nostr":metadata(&account,kind,&resolved.preferred_id,expected_thread)});
    tx.commit().await?; Ok(Json(result).into_response())
}
async fn write(State(state): State<AppState>, OriginalUri(uri): OriginalUri, Path((kind,id)): Path<(String,String)>, headers: HeaderMap, request: Request) -> PublicApiResult<Response> {
    let kind = Kind::parse(&kind)?; let id = PublicUuid::parse(&id)?;
    if uri.query().is_some() { return Err(PublicApiError::bad_request("Discussion writes accept no query")); }
    let method = request.method().as_str().to_owned(); let bytes = common::body(request).await?;
    let mut types = headers.get_all(header::CONTENT_TYPE).iter();
    if !types.next().and_then(|v|v.to_str().ok()).is_some_and(|v|v.split(';').next().is_some_and(|v|v.trim().eq_ignore_ascii_case("application/json"))) || types.next().is_some() { return Err(PublicApiError::bad_request("JSON content type required")); }
    let cap = admitted_request(&state, &uri, &headers, &method, &bytes).await?; let c = Command::parse_target(&bytes)?;
    let mut db = state.db.client().await?; let tx = db.transaction().await?;
    let actor = bound_account_write(&tx, &cap).await?; let account = account_public_id_in(&tx, actor.user_id()).await?;
    if c.expected_account_id.as_str() != account { return Err(PublicApiError::conflict("Account binding changed; refresh before submitting")); }
    let resolved = target(&tx, kind, &id, &c.expected_thread_id, true).await?;
    if c.expected_target_id.as_ref() != Some(&resolved.preferred_id) { return Err(PublicApiError::conflict("Target identity changed; refresh before submitting")); }
    let (reply,changed) = common::execute(&tx, &actor, &c.expected_thread_id, &c).await?;
    let mut meta = metadata(&account,kind,&resolved.preferred_id,&c.expected_thread_id);
    meta["operation"]=json!(c.operation.name()); meta["changed"]=json!(changed); meta["requestHash"]=json!(hex::encode(Sha256::digest(&bytes))); meta["requestMethod"]=json!(method); meta["requestPath"]=json!(uri.to_string());
    let mut data = Map::new(); if let Some(reply)=reply { data.insert("reply".into(),reply); }
    let result=json!({"result":"ok","data":data,"nostr":meta});
    tx.commit().await?; Ok(Json(result).into_response())
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn routes_compose_without_shadowing_existing_user_discussion() { let _: Router<AppState> = super::super::router(); }
    #[test] fn generic_target_never_accepts_user_or_unknown_kind() {
        for kind in ["manga","chapter","group"] { assert!(Kind::parse(kind).is_ok()); }
        for kind in ["user","author","publisher","thread",""] { assert!(Kind::parse(kind).is_err()); }
        let mut c=json!({"expectedAccountId":"11111111-1111-8111-8111-111111111111","expectedTargetId":"22222222-2222-8222-8222-222222222222","expectedThreadId":"NativeThread","operation":"create","body":"reply"});
        assert!(Command::parse_target(&serde_json::to_vec(&c).unwrap()).is_ok()); assert!(Command::parse(&serde_json::to_vec(&c).unwrap()).is_err());
        c["expectedUserId"]=c["expectedTargetId"].clone(); assert!(Command::parse_target(&serde_json::to_vec(&c).unwrap()).is_err());
    }
}
