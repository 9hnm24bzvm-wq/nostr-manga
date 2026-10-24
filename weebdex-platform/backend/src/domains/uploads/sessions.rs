use super::storage::{self, MAX_FILES, MAX_SESSION_BYTES, StoredImage};
use crate::{
    domains::{
        auth::{self, SessionPrincipal},
        catalog::{CatalogPage, CatalogRepository, ChapterPublication, CoverPublication, entity_mutations::CatalogMutationActor},
    },
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use tokio_postgres::{Row, Transaction};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Begin {
    pub(crate) manga: String,
    #[serde(default)]
    pub(crate) groups: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    #[serde(default)]
    pub(crate) replace: bool,
    pub(crate) version: i32,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub(crate) volume: Option<String>,
    pub(crate) chapter: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) language: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Commit {
    pub(crate) draft: Draft,
    pub(crate) page_order: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumePatch {
    pub(crate) version: i32,
    pub(crate) draft: Value,
    pub(crate) page_order: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    language: String,
}

async fn actor(state: &AppState, headers: &HeaderMap) -> ApiResult<SessionPrincipal> {
    auth::validate_mutation(state, headers)?;
    let actor = auth::require_principal(state, headers).await?;
    if actor.is_api_client || actor.recovery_session {
        return Err(ApiError::forbidden(
            "A browser session is required for uploads",
        ));
    }
    actor.require_verified()?;
    Ok(actor)
}
fn session_id(id: &str) -> ApiResult<()> {
    if storage::valid_id(id) {
        Ok(())
    } else {
        Err(ApiError::bad_request("Invalid upload session identifier"))
    }
}
#[cfg(test)]
fn validate_draft(draft: &Draft) -> ApiResult<()> {
    validate_draft_for(draft, crate::domains::catalog::validation::LanguageContract::Native)
}
pub(crate) fn validate_draft_for(draft: &Draft, contract: crate::domains::catalog::validation::LanguageContract) -> ApiResult<()> {
    contract.validate(&draft.language)?;
    for value in [&draft.volume, &draft.chapter] {
        if value.as_ref().is_some_and(|v| !valid_number(v)) {
            return Err(ApiError::bad_request("Invalid chapter or volume number"));
        }
    }
    if draft
        .title
        .as_ref()
        .is_some_and(|v| v.len() > 500 || v.contains('\0'))
    {
        return Err(ApiError::bad_request("Chapter title is too long"));
    }
    Ok(())
}
// Matches supplied frontend /^((0|[1-9]\d*)(\.\d+)?[a-z]?)?$/ without regex allocation.
fn valid_number(value: &str) -> bool {
    if value.len() > 32 {
        return false;
    }
    if value.is_empty() {
        return true;
    }
    let body = if value.as_bytes().last().is_some_and(u8::is_ascii_lowercase) {
        &value[..value.len() - 1]
    } else {
        value
    };
    let mut parts = body.split('.');
    let integer = parts.next().unwrap_or("");
    if integer.is_empty()
        || !integer.bytes().all(|b| b.is_ascii_digit())
        || (integer.len() > 1 && integer.starts_with('0'))
    {
        return false;
    }
    if let Some(fraction) = parts.next() {
        if fraction.is_empty() || !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    parts.next().is_none()
}
pub(crate) async fn open_session(tx: &Transaction<'_>, id: &str, user: &str) -> ApiResult<Row> {
    open_session_for(tx, id, user, false).await
}
async fn open_session_for(
    tx: &Transaction<'_>,
    id: &str,
    user: &str,
    retry_prepared_commit: bool,
) -> ApiResult<Row> {
    crate::domains::auth::closure::active_in(tx,user).await?;
    session_id(id)?;
    let row=tx.query_opt("SELECT manga_id,chapter_id,edit_version,groups,state,committed_chapter_id,commit_hash,expires_at>clock_timestamp() AS active FROM platform.upload_sessions WHERE id=$1 AND owner_id=$2 FOR UPDATE",&[&id,&user]).await?.ok_or_else(||ApiError::not_found("Upload session not found"))?;
    if row.get::<_, String>(4) != "open" {
        return Err(ApiError::conflict("Upload session is no longer open"));
    }
    if !row.get::<_, bool>(7) {
        let recoverable = retry_prepared_commit && tx.query_one(
            "SELECT EXISTS(SELECT 1 FROM platform.protocol_publication_commands WHERE upload_session_id=$1 AND user_id=$2 AND state IN ('prepared','source_accepted'))",
            &[&id, &user],
        ).await?.get::<_, bool>(0);
        if recoverable {
            // Only commit uses this exception. Its durable command still verifies
            // the exact native commit hash, signed input and current authority.
            return Ok(row);
        }
        return Err(ApiError::new(
            StatusCode::GONE,
            "upload_expired",
            "Upload session expired",
        ));
    }
    Ok(row)
}
/// An independently prepared original publication fixes its signed input.
/// Preserve those native files for exact commit retry and rollback recovery.
pub(crate) async fn require_mutable_session(tx: &Transaction<'_>, id: &str) -> ApiResult<()> {
    let prepared: bool = tx.query_one(
        "SELECT EXISTS(SELECT 1 FROM platform.protocol_publication_commands WHERE upload_session_id=$1 AND state IN ('prepared','source_accepted'))",
        &[&id],
    ).await?.get(0);
    if prepared {
        Err(ApiError::conflict(
            "Publication has been prepared; retry the identical commit instead of editing or cancelling its inputs",
        ))
    } else {
        Ok(())
    }
}
pub(crate) async fn session_json(tx: &Transaction<'_>, id: &str) -> ApiResult<Value> {
    let row=tx.query_one("SELECT id,platform.catalog_timestamp(created_at),platform.catalog_timestamp(updated_at),version,state,manga_id,chapter_id,owner_id,groups,platform.catalog_timestamp(expires_at),expires_at>clock_timestamp(),resume_draft,resume_page_order,committed_chapter_id FROM platform.upload_sessions WHERE id=$1",&[&id]).await?;
    let manga: Option<Value> = tx
        .query_opt(
            "SELECT platform.catalog_manga_json(id) FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND state='published' AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id)",
            &[&row.get::<_, String>(5)],
        )
        .await?.and_then(|r|r.get(0));
    let chapter_id: Option<String> = row.get::<_, Option<String>>(13).or_else(|| row.get(6));
    let chapter: Option<Value> = match &chapter_id {
        Some(chapter) => tx
            .query_opt("SELECT platform.catalog_chapter_json(c.id) FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=$1 AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(c.protocol_event_id) AND platform.protocol_source_is_visible(m.protocol_series_id)", &[&chapter])
            .await?.and_then(|r|r.get(0)),
        None => None,
    };
    let owner: String = row.get(7);
    let command=tx.query_opt("SELECT state,native_commit_payload,event_id FROM platform.protocol_publication_commands WHERE upload_session_id=$1 AND user_id=$2 ORDER BY CASE resource_kind WHEN 'chapter' THEN 0 ELSE 1 END,created_at DESC,command_id LIMIT 1", &[&id,&owner]).await?;
    let publication_status = command
        .as_ref()
        .map(|r| r.get::<_, String>(0))
        .unwrap_or_else(|| "unprepared".into());
    let fixed = matches!(publication_status.as_str(), "prepared" | "source_accepted");
    let command_payload: Option<Value> = command.as_ref().and_then(|r| r.get(1));
    let draft: Option<Value> = command_payload
        .as_ref()
        .and_then(|p| p.get("draft").cloned())
        .or_else(|| row.get(11))
        .or_else(||chapter.as_ref().map(|c|json!({"title":c.get("title"),"volume":c.get("volume"),"chapter":c.get("chapter"),"language":c.get("language")})));
    let stored_order: Vec<String> = command_payload
        .as_ref()
        .and_then(|p| serde_json::from_value(p["page_order"].clone()).ok())
        .unwrap_or_else(|| row.get(12));
    let groups: Vec<String> = row.get(8);
    let mut group_json = Vec::new();
    for group in groups {
        if let Some(value) = tx
            .query_one("SELECT platform.catalog_group_json($1)", &[&group])
            .await?
            .get::<_, Option<Value>>(0)
        {
            group_json.push(value);
        }
    }
    let mut files = file_rows(tx, id)
        .await?
        .iter()
        .map(file_json)
        .collect::<Vec<_>>();
    let mut page_order = stored_order
        .into_iter()
        .filter(|id| files.iter().any(|f| f["id"].as_str() == Some(id)))
        .collect::<Vec<_>>();
    for file in &files {
        let id = file["id"].as_str().expect("stored file ID").to_owned();
        if !page_order.contains(&id) {
            page_order.push(id);
        }
    }
    files.sort_by_key(|f| {
        page_order
            .iter()
            .position(|id| f["id"].as_str() == Some(id))
            .unwrap_or(usize::MAX)
    });
    let session_state: String = row.get(4);
    let active: bool = row.get(10);
    Ok(
        json!({"id":row.get::<_,String>(0),"created_at":row.get::<_,String>(1),"updated_at":row.get::<_,String>(2),"version":row.get::<_,i32>(3),"state":session_state,"is_committed":session_state=="committed","is_processed":true,"is_deleted":session_state=="deleted","expires_at":row.get::<_,String>(9),"is_expired":!active,"can_mutate":session_state=="open"&&active&&!fixed,"can_cancel":session_state=="open"&&active&&!fixed,"can_retry_commit":session_state=="open"&&(active||fixed),"publication_status":publication_status,"publication_event_id":command.as_ref().and_then(|r|r.get::<_,Option<String>>(2)),"manga_id":row.get::<_,String>(5),"chapter_id":chapter_id,"draft":draft,"page_order":page_order,"receipt":if session_state=="committed"{chapter.clone()}else{None},"committed_result":if session_state=="committed"{chapter.clone()}else{None},"relationships":{"manga":manga,"chapter":chapter,"uploader":{"id":owner},"groups":group_json,"files":files}}),
    )
}
pub(crate) async fn file_rows(tx: &Transaction<'_>, id: &str) -> ApiResult<Vec<Row>> {
    Ok(tx.query("SELECT f.id,o.digest,o.extension,o.bytes,f.original_name,o.width,o.height,o.id FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id WHERE f.session_id=$1 ORDER BY f.position,f.id",&[&id]).await?)
}
fn file_json(row: &Row) -> Value {
    let id: String = row.get(0);
    let ext: String = row.get(2);
    json!({"id":id,"file_hash":row.get::<_,String>(1),"file_name":format!("{id}.{ext}"),"file_ext":format!(".{ext}"),"file_size":row.get::<_,i64>(3).to_string(),"original_file_name":row.get::<_,String>(4)})
}
async fn current_committed_result(
    state: &AppState,
    tx: &Transaction<'_>,
    chapter: &str,
    session: &str,
    actor: &impl CatalogMutationActor,
) -> ApiResult<Value> {
    crate::domains::auth::closure::active_in(tx,actor.user_id()).await?;
    current_committed_result_for_owner(state, tx, chapter, session, actor.user_id()).await
}
async fn current_committed_result_for_owner(
    state: &AppState,
    tx: &Transaction<'_>,
    chapter: &str,
    session: &str,
    owner: &str,
) -> ApiResult<Value> {
    if let Some(value) = tx.query_opt(
        "SELECT platform.catalog_chapter_json(c.id) FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=$1 AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(c.protocol_event_id) AND platform.protocol_source_is_visible(m.protocol_series_id)",
        &[&chapter],
    ).await?.and_then(|r|r.get::<_,Option<Value>>(0)) { return Ok(value); }
    if !crate::protocol::publication::pending_native_receipt_allowed_for_owner(
        state, tx, chapter, session, owner,
    )
    .await?
    {
        return Err(ApiError::not_found(
            "Committed chapter is no longer available",
        ));
    }
    let row = tx.query_one(
        "SELECT s.committed_result,c.protocol_event_id FROM platform.upload_sessions s JOIN platform.catalog_chapters c ON c.id=s.committed_chapter_id WHERE s.id=$1 AND s.owner_id=$2 AND c.id=$3",
        &[&session,&owner,&chapter],
    ).await?;
    let saved: Value = row
        .get::<_, Option<Value>>(0)
        .ok_or_else(ApiError::unavailable)?;
    let source: String = row
        .get::<_, Option<String>>(1)
        .ok_or_else(ApiError::unavailable)?;
    if saved["id"] != chapter
        || saved["protocol_event_id"] != source
        || saved["publication_scope"] != "original-protocol"
        || !matches!(
            saved["publication_status"].as_str(),
            Some("accepted_pending_projection" | "pending_projection")
        )
    {
        return Err(ApiError::unavailable());
    }
    // Only the authenticated uploader's small pending receipt is retained. Never
    // replay historical relationship profiles, counters, threads or full DTOs.
    let mut receipt = serde_json::Map::new();
    for key in [
        "id",
        "manga_id",
        "source_id",
        "protocol_event_id",
        "publication_scope",
        "publication_status",
        "version",
        "node",
        "data",
        "data_optimized",
        "title",
        "language",
        "volume",
        "chapter",
    ] {
        if let Some(value) = saved.get(key) {
            receipt.insert(key.to_owned(), value.clone());
        }
    }
    receipt.insert("is_unavailable".into(), json!(true));
    Ok(Value::Object(receipt))
}
/// Owner-only resumable view shared by native and signed-public reads. The
/// caller rechecks authentication/binding in this transaction; this helper locks
/// ownership and preserves current receipt visibility and capability downgrades.
/// It never turns a read principal into a catalogue mutation capability.
pub(crate) async fn owner_snapshot_in(
    state: &AppState,
    tx: &Transaction<'_>,
    id: &str,
    owner: &str,
    can_manage: bool,
) -> ApiResult<Value> {
    session_id(id)?;
    tx.query_opt(
        "SELECT id FROM platform.upload_sessions WHERE id=$1 AND owner_id=$2 FOR SHARE",
        &[&id, &owner],
    ).await?.ok_or_else(|| ApiError::not_found("Upload session not found"))?;
    let mut value = session_json(tx, id).await?;
    value["target_manga"] = value["relationships"]["manga"]
        .as_object()
        .map(|m| json!({"id":m.get("id"),"title":m.get("title")}))
        .unwrap_or(Value::Null);
    value["target_status"] = json!(if value["target_manga"].is_null() {
        "unavailable"
    } else {
        "available"
    });
    if value["is_committed"] == true {
        let session_id = value["id"].as_str().ok_or_else(ApiError::unavailable)?;
        let chapter_id = value["chapter_id"]
            .as_str()
            .ok_or_else(ApiError::unavailable)?;
        match current_committed_result_for_owner(state, tx, chapter_id, session_id, owner).await {
            Ok(receipt) => {
                if value["target_manga"].is_null() && receipt["is_unavailable"] == true {
                    if let Some(target)=tx.query_opt("SELECT id,title FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND state='published'", &[&value["manga_id"].as_str().ok_or_else(ApiError::unavailable)?]).await? {
                        value["target_manga"] = json!({"id":target.get::<_,String>(0),"title":target.get::<_,String>(1),"scope":"instance-local","is_unavailable":true});
                        value["target_status"] = json!("accepted_pending_projection");
                    }
                }
                value["committed_result"] = receipt.clone();
                value["receipt"] = receipt;
            }
            Err(error) if error.status == StatusCode::NOT_FOUND => {
                value["committed_result"] = Value::Null;
                value["receipt"] = Value::Null;
                value["source_unavailable"] = json!(true);
            }
            Err(error) => return Err(error),
        }
    }
    if value["target_status"] == "unavailable" {
        value["can_retry_commit"] = json!(false);
    }
    if !can_manage {
        value["can_mutate"] = json!(false);
        value["can_cancel"] = json!(false);
        value["can_retry_commit"] = json!(false);
    }
    if value["publication_status"] != "unprepared"
        && state.config.protocol_publication.is_none()
    {
        value["can_retry_commit"] = json!(false);
        value["publication_configuration_available"] = json!(false);
    }
    Ok(value)
}
pub async fn get(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Response> {
    let actor = auth::require_principal(&state, &headers).await?;
    if actor.is_api_client || actor.recovery_session {
        return Err(ApiError::forbidden(
            "A browser session is required for upload sessions",
        ));
    }
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let row=tx.query_opt("SELECT id FROM platform.upload_sessions s WHERE owner_id=$1 AND ((state='open' AND (expires_at>clock_timestamp() OR EXISTS(SELECT 1 FROM platform.protocol_publication_commands p WHERE p.upload_session_id=s.id AND p.user_id=s.owner_id AND p.state IN ('prepared','source_accepted')))) OR (state='committed' AND committed_result->>'publication_status' IN ('accepted_pending_projection','pending_projection') AND EXISTS(SELECT 1 FROM platform.catalog_chapters c WHERE c.id=s.committed_chapter_id AND c.deleted_at IS NULL))) ORDER BY (state='open') DESC,updated_at DESC,id LIMIT 1",&[&actor.user_id]).await?;
    let response = match row {
        Some(row) => {
            let value = owner_snapshot_in(&state, &tx, &row.get::<_, String>(0),
                &actor.user_id, actor.verified && actor.has_perm("self.chapter.update")).await?;
            Json(value).into_response()
        }
        None => StatusCode::NO_CONTENT.into_response(),
    };
    tx.commit().await?;
    Ok(response)
}
/// Persist private form metadata/order without changing a chapter or signed input.
pub async fn save_resume(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<ResumePatch>,
) -> ApiResult<Json<Value>> {
    let actor = actor(&state, &headers).await?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let result = save_resume_in(&tx, &actor, &id, &payload).await?;
    tx.commit().await?;
    Ok(Json(result))
}
/// Persist only private draft/order metadata under the same version, ownership,
/// expiry and prepared-command fences for native and public transports.
pub(crate) async fn save_resume_in(
    tx: &Transaction<'_>,
    actor: &impl CatalogMutationActor,
    id: &str,
    payload: &ResumePatch,
) -> ApiResult<Value> {
    crate::domains::auth::closure::active_in(tx,actor.user_id()).await?;
    actor.require_permission("self.chapter.update")?;
    let v = &payload.draft;
    if payload.version <= 0 {
        return Err(ApiError::bad_request("Invalid upload session version"));
    }
    crate::domains::catalog::validation::allowed(v, &["title", "volume", "chapter", "language"])?;
    for (field, max) in [
        ("title", 500),
        ("volume", 32),
        ("chapter", 32),
        ("language", 12),
    ] {
        crate::domains::catalog::validation::string(v, field, max, false)?;
    }
    if let Some(language) = v["language"].as_str().filter(|lang| !lang.is_empty()) {
        actor.language_contract().validate(language)?;
    }
    open_session(tx, id, actor.user_id()).await?;
    require_mutable_session(tx, id).await?;
    let version: i32 = tx
        .query_one(
            "SELECT version FROM platform.upload_sessions WHERE id=$1",
            &[&id],
        )
        .await?
        .get(0);
    if version != payload.version {
        return Err(ApiError::conflict(
            "Upload session changed; reload its current files and version",
        ));
    }
    let files = file_rows(tx, id)
        .await?
        .iter()
        .map(|r| r.get::<_, String>(0))
        .collect::<Vec<_>>();
    if payload.page_order.len() > MAX_FILES
        || payload.page_order.len() != files.len()
        || payload.page_order.iter().collect::<BTreeSet<_>>().len() != files.len()
        || payload.page_order.iter().any(|id| !files.contains(id))
    {
        return Err(ApiError::bad_request(
            "Saved order must contain every current session file exactly once",
        ));
    }
    tx.execute("UPDATE platform.upload_sessions SET resume_draft=$2,resume_page_order=$3,version=version+1,updated_at=clock_timestamp() WHERE id=$1", &[&id,&payload.draft,&payload.page_order]).await?;
    session_json(tx, id).await
}

pub async fn begin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Begin>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let actor = actor(&state, &headers).await?;
    actor.require_permission("chapter.create")?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let result = begin_in(&tx, &actor, &payload).await?;
    tx.commit().await?;
    cleanup(&state).await;
    Ok((StatusCode::CREATED, Json(result)))
}
pub(crate) async fn begin_in(
    tx: &Transaction<'_>,
    actor: &impl CatalogMutationActor,
    payload: &Begin,
) -> ApiResult<Value> {
    crate::domains::auth::closure::active_in(tx,actor.user_id()).await?;
    actor.require_permission("chapter.create")?;
    CatalogRepository::ensure_upload_target(&tx, &payload.manga, actor, &payload.groups).await?;
    expire_owner(&tx, &actor.user_id()).await?;
    if tx
        .query_opt(
            "SELECT id FROM platform.upload_sessions WHERE owner_id=$1 AND state='open'",
            &[&actor.user_id()],
        )
        .await?
        .is_some()
    {
        return Err(ApiError::conflict(
            "Cancel the existing upload session before beginning another",
        ));
    }
    let id = storage::short_id();
    tx.execute(
        "INSERT INTO platform.upload_sessions(id,owner_id,manga_id,groups) VALUES($1,$2,$3,$4)",
        &[&id, &actor.user_id(), &payload.manga, &payload.groups],
    )
    .await?;
    let result = session_json(&tx, &id).await?;
    Ok(result)
}
pub async fn begin_edit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(chapter): Path<String>,
    Json(payload): Json<Edit>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let actor = actor(&state, &headers).await?;
    actor.require_permission("self.chapter.update")?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let result = begin_edit_in(&tx, &actor, &chapter, &payload).await?;
    tx.commit().await?;
    cleanup(&state).await;
    Ok((StatusCode::CREATED, Json(result)))
}
pub(crate) async fn begin_edit_in(
    tx: &Transaction<'_>,
    actor: &impl CatalogMutationActor,
    chapter: &str, payload: &Edit,
) -> ApiResult<Value> {
    crate::domains::auth::closure::active_in(tx,actor.user_id()).await?;
    actor.require_permission("self.chapter.update")?;
    // Expiry may lock the owner's old OPEN session and media references. Take
    // those locks before the chapter, matching commit/scheduling admission.
    // Authorization/version failures roll this entire transaction back; no
    // filesystem cleanup runs before the handler commits successfully.
    expire_owner(&tx, &actor.user_id()).await?;
    let old =
        CatalogRepository::authorize_chapter_edit(&tx, &chapter, actor, payload.version).await?;
    if tx
        .query_opt(
            "SELECT id FROM platform.upload_sessions WHERE owner_id=$1 AND state='open'",
            &[&actor.user_id()],
        )
        .await?
        .is_some()
    {
        return Err(ApiError::conflict(
            "Cancel the existing upload session before beginning another",
        ));
    }
    let manga = old["relationships"]["manga"]["id"]
        .as_str()
        .ok_or_else(ApiError::unavailable)?;
    let groups: Vec<String> = old["relationships"]["groups"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|g| g["id"].as_str().map(str::to_owned))
        .collect();
    let id = storage::short_id();
    tx.execute("INSERT INTO platform.upload_sessions(id,owner_id,manga_id,chapter_id,edit_version,groups) VALUES($1,$2,$3,$4,$5,$6)",&[&id,&actor.user_id(),&manga,&chapter,&payload.version,&groups]).await?;
    if !payload.replace {
        for row in tx.query("SELECT p.object_id,p.position,o.extension FROM platform.chapter_pages p JOIN platform.media_objects o ON o.id=p.object_id WHERE p.chapter_id=$1 ORDER BY p.position",&[&chapter]).await? {
   let file=storage::short_id();let position:i32=row.get(1);let original=format!("{}.{}",position+1,row.get::<_,String>(2));
   tx.execute("INSERT INTO platform.upload_files(id,session_id,object_id,position,original_name) VALUES($1,$2,$3,$4,$5)",&[&file,&id,&row.get::<_,String>(0),&position,&original]).await?;
   tx.execute("INSERT INTO platform.upload_file_derivatives(file_id,name,object_id) SELECT $1,d.name,d.object_id FROM platform.chapter_page_derivatives d WHERE d.chapter_id=$2 AND d.position=$3",&[&file,&chapter,&position]).await?;
  }
    }
    let result = session_json(&tx, &id).await?;
    Ok(result)
}
pub async fn check_approval(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Approval>,
) -> ApiResult<StatusCode> {
    let actor = actor(&state, &headers).await?;
    actor.require_permission("chapter.create")?;
    crate::domains::catalog::validation::language(&payload.language)?;
    // Local publication is permitted by the persisted chapter.create grant. No fictitious approval queue.
    Ok(StatusCode::NO_CONTENT)
}
pub async fn upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> ApiResult<Json<Value>> {
    let actor = actor(&state, &headers).await?;
    actor.require_permission("self.chapter.update")?;
    {
        let mut client = state.db.client().await?;
        let tx = client.transaction().await?;
        open_session(&tx, &id, &actor.user_id).await?;
        require_mutable_session(&tx, &id).await?;
        tx.commit().await?;
    }
    let mut result = Vec::new();
    let mut field_count = 0usize;
    while let Some(field) =
        tokio::time::timeout(std::time::Duration::from_secs(30), multipart.next_field())
            .await
            .map_err(|_| ApiError::bad_request("Multipart upload timed out"))?
            .map_err(|_| ApiError::bad_request("Invalid multipart payload"))?
    {
        field_count += 1;
        if field_count > 5 {
            return Err(ApiError::bad_request(
                "At most five files may be uploaded per request",
            ));
        }
        if field.name() != Some("file") {
            return Err(ApiError::bad_request("Multipart fields must be named file"));
        }
        let original = field
            .file_name()
            .unwrap_or("image")
            .chars()
            .take(255)
            .collect::<String>();
        let received = storage::receive(&state.config.uploads_dir, field, false).await;
        let entry = match received {
            Err(error) => json!({"original_file_name":original,"error":error.message}),
            Ok((mut image, mut variants)) => {
                match attach(&state, &id, &actor, &original, &mut image, &mut variants).await {
                    Ok(file) => file,
                    Err(error) => json!({"original_file_name":original,"error":error.message}),
                }
            }
        };
        result.push(entry);
    }
    if result.is_empty() {
        return Err(ApiError::bad_request("At least one image is required"));
    }
    Ok(Json(Value::Array(result)))
}
/// Replace a page in place. Validation and filesystem staging finish before
/// the old reference changes; the session row lock makes the swap/version and
/// derivative installation atomic. Stable file IDs also make identical retries
/// safe after a lost response without adding another page.
pub async fn replace_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, file)): Path<(String, String)>,
    mut multipart: Multipart,
) -> ApiResult<Json<Value>> {
    let actor = actor(&state, &headers).await?;
    actor.require_permission("self.chapter.update")?;
    session_id(&id)?;
    if !storage::valid_id(&file) {
        return Err(ApiError::bad_request("Invalid upload file identifier"));
    }
    let values = headers
        .get_all("x-weebdex-upload-version")
        .iter()
        .collect::<Vec<_>>();
    if values.len() != 1 {
        return Err(ApiError::bad_request(
            "Exactly one upload version is required",
        ));
    }
    let expected: i32 = values[0]
        .to_str()
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| ApiError::bad_request("Invalid upload version"))?;
    {
        let mut client = state.db.client().await?;
        let tx = client.transaction().await?;
        open_session(&tx, &id, &actor.user_id).await?;
        require_mutable_session(&tx, &id).await?;
        if tx
            .query_opt(
                "SELECT id FROM platform.upload_files WHERE id=$1 AND session_id=$2",
                &[&file, &id],
            )
            .await?
            .is_none()
        {
            return Err(ApiError::not_found("Upload file not found in this session"));
        }
        tx.commit().await?;
    }
    let field = tokio::time::timeout(std::time::Duration::from_secs(30), multipart.next_field())
        .await
        .map_err(|_| ApiError::bad_request("Multipart upload timed out"))?
        .map_err(|_| ApiError::bad_request("Invalid multipart payload"))?
        .ok_or_else(|| ApiError::bad_request("One replacement image is required"))?;
    if field.name() != Some("file") {
        return Err(ApiError::bad_request("Multipart field must be named file"));
    }
    let original = field
        .file_name()
        .unwrap_or("image")
        .chars()
        .take(255)
        .collect::<String>();
    let (mut image, mut variants) =
        storage::receive(&state.config.uploads_dir, field, false).await?;
    if tokio::time::timeout(std::time::Duration::from_secs(30), multipart.next_field())
        .await
        .map_err(|_| ApiError::bad_request("Multipart upload timed out"))?
        .map_err(|_| ApiError::bad_request("Invalid multipart payload"))?
        .is_some()
    {
        return Err(ApiError::bad_request(
            "Exactly one replacement image is allowed",
        ));
    }
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let replaced = replace_file_in(
        &state, &tx, &actor, &id, &file, expected,
        ReplacementVersionPolicy::NativeIdenticalRetry,
        &original, &mut image, &mut variants,
    ).await?;
    tx.commit().await?;
    if replaced.changed {
        image.confirm_persisted().await;
        for (_, derivative) in variants.iter_mut() {
            derivative.confirm_persisted().await;
        }
    }
    Ok(Json(replaced.file))
}

/// Native cookie uploads have always allowed an identical lost-response retry.
/// Signed public replacement instead fences every request on its signed version.
#[derive(Clone, Copy)]
pub(crate) enum ReplacementVersionPolicy {
    NativeIdenticalRetry,
    Exact,
}
pub(crate) struct FileReplacement {
    pub(crate) file: Value,
    pub(crate) changed: bool,
}
/// Caller-owned mutation transaction, shared by native and signed public routes.
/// The public caller must construct its write actor in this same transaction,
/// after raw-body admission and image decoding. No preflight actor is promoted.
pub(crate) async fn replace_file_in(
    state: &AppState,
    tx: &Transaction<'_>,
    actor: &impl CatalogMutationActor,
    id: &str,
    file: &str,
    expected: i32,
    version_policy: ReplacementVersionPolicy,
    original: &str,
    image: &mut StoredImage,
    variants: &mut Vec<(String, StoredImage)>,
) -> ApiResult<FileReplacement> {
    crate::domains::auth::closure::active_in(tx,actor.user_id()).await?;
    actor.require_permission("self.chapter.update")?;
    if !storage::valid_id(file) || expected < 1 {
        return Err(ApiError::bad_request("Invalid replacement file or version"));
    }
    open_session(tx, id, actor.user_id()).await?;
    require_mutable_session(tx, id).await?;
    let old = tx.query_opt("SELECT o.digest,o.bytes,f.original_name FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id WHERE f.id=$1 AND f.session_id=$2 FOR UPDATE OF f", &[&file,&id]).await?
        .ok_or_else(||ApiError::not_found("Upload file not found in this session"))?;
    let version: i32 = tx
        .query_one(
            "SELECT version FROM platform.upload_sessions WHERE id=$1",
            &[&id],
        )
        .await?
        .get(0);
    let identical = old.get::<_, String>(0) == image.digest && old.get::<_, String>(2) == original;
    require_replacement_version(version_policy, version, expected, identical)?;
    if !identical {
        let bytes: i64 = tx.query_one("SELECT COALESCE(sum(o.bytes),0)::BIGINT FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id WHERE f.session_id=$1", &[&id]).await?.get(0);
        if bytes - old.get::<_, i64>(1) + image.bytes > MAX_SESSION_BYTES {
            return Err(ApiError::bad_request(
                "Upload session exceeds its byte budget",
            ));
        }
        let object = insert_object(tx, image).await?;
        tx.execute(
            "DELETE FROM platform.upload_file_derivatives WHERE file_id=$1",
            &[&file],
        )
        .await?;
        tx.execute("UPDATE platform.upload_files SET object_id=$3,original_name=$4 WHERE id=$1 AND session_id=$2", &[&file,&id,&object,&original]).await?;
        for (_, derivative) in variants.iter() {
            let object = insert_object(tx, derivative).await?;
            let name = format!("{}.{}", storage::short_id(), derivative.ext);
            tx.execute("INSERT INTO platform.upload_file_derivatives(file_id,name,object_id) VALUES($1,$2,$3)", &[&file,&name,&object]).await?;
        }
        tx.execute("UPDATE platform.upload_sessions SET version=version+1,updated_at=clock_timestamp() WHERE id=$1", &[&id]).await?;
        queue_unused(tx).await?;
        image.protect_commit(&state.config.uploads_dir).await?;
        for (_, derivative) in variants.iter_mut() {
            derivative.protect_commit(&state.config.uploads_dir).await?;
        }
    }
    let mut result = file_rows(tx, id)
        .await?
        .iter()
        .find(|r| r.get::<_, String>(0) == file)
        .map(file_json)
        .ok_or_else(ApiError::unavailable)?;
    // Report the persisted value while the session lock is still held, including
    // unchanged requests. Never infer a client-visible version from its input.
    result["version"] = json!(tx.query_one(
        "SELECT version FROM platform.upload_sessions WHERE id=$1", &[&id],
    ).await?.get::<_, i32>(0));
    Ok(FileReplacement { file: result, changed: !identical })
}
fn require_replacement_version(
    policy: ReplacementVersionPolicy, actual: i32, expected: i32, identical: bool,
) -> ApiResult<()> {
    if actual != expected && (matches!(policy, ReplacementVersionPolicy::Exact) || !identical) {
        Err(ApiError::conflict(
            "Upload session version changed; reload before replacing the page",
        ))
    } else {
        Ok(())
    }
}
async fn attach(
    state: &AppState,
    id: &str,
    actor: &SessionPrincipal,
    original: &str,
    image: &mut StoredImage,
    variants: &mut Vec<(String, StoredImage)>,
) -> ApiResult<Value> {
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let result = attach_in(state, &tx, id, actor, original, image, variants).await?;
    tx.commit().await?;
    image.confirm_persisted().await;
    for (_, derivative) in variants.iter_mut() {
        derivative.confirm_persisted().await;
    }
    Ok(result)
}
pub(crate) async fn attach_in(
    state: &AppState,
    tx: &Transaction<'_>,
    id: &str,
    actor: &impl CatalogMutationActor,
    original: &str,
    image: &mut StoredImage,
    variants: &mut Vec<(String, StoredImage)>,
) -> ApiResult<Value> {
    crate::domains::auth::closure::active_in(tx,actor.user_id()).await?;
    actor.require_permission("self.chapter.update")?;
    open_session(&tx, id, &actor.user_id()).await?;
    require_mutable_session(&tx, id).await?;
    let row=tx.query_one("SELECT count(*) AS count,COALESCE(sum(o.bytes),0)::BIGINT AS bytes,COALESCE(max(f.position),-1)::INTEGER AS position FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id WHERE f.session_id=$1",&[&id]).await?;
    if row.get::<_, i64>(0) >= MAX_FILES as i64
        || row.get::<_, i64>(1) + image.bytes > MAX_SESSION_BYTES
    {
        return Err(ApiError::bad_request(
            "Upload session exceeds its file or byte budget",
        ));
    }
    let object = insert_object(&tx, image).await?;
    let file = storage::short_id();
    let position = row.get::<_, i32>(2) + 1;
    tx.execute("INSERT INTO platform.upload_files(id,session_id,object_id,original_name,position) VALUES($1,$2,$3,$4,$5)",&[&file,&id,&object,&original,&position]).await?;
    for (_, derivative) in variants.iter() {
        let object = insert_object(&tx, derivative).await?;
        let name = format!("{}.{}", storage::short_id(), derivative.ext);
        tx.execute(
            "INSERT INTO platform.upload_file_derivatives(file_id,name,object_id) VALUES($1,$2,$3)",
            &[&file, &name, &object],
        )
        .await?;
    }
    tx.execute("UPDATE platform.upload_sessions SET version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id]).await?;
    image.protect_commit(&state.config.uploads_dir).await?;
    for (_, derivative) in variants.iter_mut() {
        derivative.protect_commit(&state.config.uploads_dir).await?;
    }
    Ok(
        json!({"id":file,"file_hash":image.digest,"file_name":format!("{}.{}",file,image.ext),"file_ext":format!(".{}",image.ext),"file_size":image.bytes.to_string(),"original_file_name":original}),
    )
}
pub(crate) async fn insert_object(tx: &Transaction<'_>, image: &StoredImage) -> ApiResult<String> {
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,724194))",
        &[&image.key],
    )
    .await?;
    tx.execute("INSERT INTO platform.media_objects(id,digest,storage_key,mime,extension,bytes,width,height) VALUES($1,$2,$1,$3,$4,$5,$6,$7)",&[&image.key,&image.digest,&image.mime,&image.ext,&image.bytes,&image.width,&image.height]).await?;
    Ok(image.key.clone())
}
pub async fn commit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<Commit>,
) -> ApiResult<Json<Value>> {
    let actor = actor(&state, &headers).await?;
    actor.require_permission("self.chapter.update")?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let result = commit_in(&state, &tx, &actor, &id, &payload).await?;
    tx.commit().await?;
    cleanup(&state).await;
    Ok(Json(result))
}
pub(crate) async fn commit_in(
    state: &AppState,
    tx: &Transaction<'_>,
    actor: &impl CatalogMutationActor,
    id: &str,
    payload: &Commit,
) -> ApiResult<Value> {
    commit_impl(state, tx, actor, id, payload, None).await
}
/// Only the private durable scheduling gate can construct this actor. Ordinary
/// native/public commits never acquire permission to mutate scheduled sessions.
pub(crate) async fn commit_scheduled_in(
    state: &AppState, tx: &Transaction<'_>,
    actor: &crate::public_api::scheduled_uploads::ScheduledUploadActor,
    id: &str, payload: &Commit,
) -> ApiResult<Value> {
    commit_impl(state, tx, actor, id, payload, Some(actor)).await
}
async fn commit_impl(
    state: &AppState, tx: &Transaction<'_>, actor: &impl CatalogMutationActor,
    id: &str, payload: &Commit,
    scheduled: Option<&crate::public_api::scheduled_uploads::ScheduledUploadActor>,
) -> ApiResult<Value> {
    crate::domains::auth::closure::active_in(tx,actor.user_id()).await?;
    actor.require_permission("self.chapter.update")?;
    session_id(&id)?;
    validate_draft_for(&payload.draft, actor.language_contract())?;
    let hash = hex::encode(Sha256::digest(
        serde_json::to_vec(&payload).map_err(|_| ApiError::bad_request("Invalid commit"))?,
    ));
    let row=tx.query_opt("SELECT state,committed_chapter_id,commit_hash,committed_result FROM platform.upload_sessions WHERE id=$1 AND owner_id=$2 FOR UPDATE",&[&id,&actor.user_id()]).await?.ok_or_else(||ApiError::not_found("Upload session not found"))?;
    if row.get::<_, String>(0) == "committed" {
        if row.get::<_, Option<String>>(2).as_deref() != Some(&hash) {
            return Err(ApiError::conflict(
                "This session was committed with a different payload",
            ));
        }
        let chapter: String = row
            .get::<_, Option<String>>(1)
            .ok_or_else(ApiError::unavailable)?;
        let value = current_committed_result(&state, &tx, &chapter, &id, actor).await?;
        return Ok(value);
    }
    let session = match scheduled {
        Some(schedule) => schedule.session_for(tx, id, &hash).await?,
        None => open_session_for(tx, id, actor.user_id(), true).await?,
    };
    let native_commit_payload =
        serde_json::to_value(&payload).map_err(|_| ApiError::bad_request("Invalid commit"))?;
    let rows = file_rows(&tx, &id).await?;
    validate_order(
        &payload.page_order,
        &rows
            .iter()
            .map(|r| r.get::<_, String>(0))
            .collect::<Vec<_>>(),
    )?;
    let mut pages = Vec::new();
    let mut objects = Vec::new();
    for file in &payload.page_order {
        let row = rows
            .iter()
            .find(|r| r.get::<_, String>(0) == *file)
            .expect("validated exact page set");
        pages.push(CatalogPage {
            name: format!("{}.{}", file, row.get::<_, String>(2)),
            dimensions: [row.get(5), row.get(6)],
        });
        objects.push(row.get::<_, String>(7));
    }
    let derivative_rows=tx.query("SELECT f.id,d.name,o.width,o.height,o.id FROM platform.upload_files f JOIN platform.upload_file_derivatives d ON d.file_id=f.id JOIN platform.media_objects o ON o.id=d.object_id WHERE f.session_id=$1",&[&id]).await?;
    let optimized = if derivative_rows.len() == pages.len() {
        payload
            .page_order
            .iter()
            .map(|file| {
                let row = derivative_rows
                    .iter()
                    .find(|r| r.get::<_, String>(0) == *file)
                    .expect("complete unique derivative set");
                (
                    CatalogPage {
                        name: row.get(1),
                        dimensions: [row.get(2), row.get(3)],
                    },
                    row.get::<_, String>(4),
                )
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let manga: String = session.get(0);
    let groups: Vec<String> = session.get(3);
    let draft = &payload.draft;
    let old: Option<String> = session.get(1);
    let (chapter, initial_result) = if let Some(chapter) = old {
        let version: i32 = session
            .get::<_, Option<i32>>(2)
            .ok_or_else(ApiError::unavailable)?;
        let patch = json!({"title":draft.title.clone().unwrap_or_default(),"volume":draft.volume.clone().unwrap_or_default(),"chapter":draft.chapter.clone().unwrap_or_default(),"language":draft.language,"groups":groups,"version":version});
        let value = CatalogRepository::replace_chapter_pages(
            &tx,
            &chapter,
            actor,
            version,
            &patch,
            &state.config.public_origin,
            &pages,
        )
        .await?;
        (chapter, value)
    } else {
        actor.require_permission("chapter.create")?;
        CatalogRepository::ensure_upload_target(&tx, &manga, actor, &groups).await?;
        let chapter = scheduled.map(|s| s.chapter_id().to_owned()).unwrap_or_else(storage::short_id);
        let mut record = ChapterPublication {
            id: chapter.clone(),
            manga_id: manga.clone(),
            uploader_id: actor.user_id().to_owned(),
            uploader_name: actor.username().to_owned(),
            title: draft.title.clone(),
            volume: draft.volume.clone(),
            chapter: draft.chapter.clone(),
            language: draft.language.clone(),
            node: state.config.public_origin.clone(),
            pages: pages.clone(),
            optimized_pages: Vec::new(),
            groups,
            source_id: None,
            protocol_event_id: None,
        };
        let originals = payload
            .page_order
            .iter()
            .map(|file| {
                let row = rows
                    .iter()
                    .find(|row| row.get::<_, String>(0) == *file)
                    .expect("validated exact original set");
                let extension: String = row.get(2);
                let mime = match extension.as_str() {
                    "jpg" => "image/jpeg",
                    "png" => "image/png",
                    "webp" => "image/webp",
                    "gif" => "image/gif",
                    _ => unreachable!("validated image extension"),
                };
                crate::protocol::publication::NativeOriginal {
                    hash: row.get(1),
                    mime: mime.to_owned(),
                    bytes: row.get::<_, i64>(3) as u64,
                    width: row.get::<_, i32>(5) as u32,
                    height: row.get::<_, i32>(6) as u32,
                    storage_key: row.get(7),
                }
            })
            .collect::<Vec<_>>();
        let receipt = crate::protocol::publication::publish_native_upload_fenced(
            &state,
            &tx,
            actor,
            &id,
            &hash,
            &mut record,
            &originals,
            &native_commit_payload,
            scheduled.map(|s| s.source_identity()),
            scheduled.and_then(|s| s.parent_manifest()),
        )
        .await?;
        let value = match receipt {
            Some(receipt) => {
                CatalogRepository::publish_verified_protocol_chapter(&tx, &record, &receipt, actor.language_contract()).await?
            }
            None => CatalogRepository::publish_chapter_for(&tx, &record, actor.language_contract()).await?,
        };
        // Original protocol names are canonical hashes; keep all positions,
        // including repeated identical original bytes, in the instance index.
        pages = record.pages.clone();
        (record.id.clone(), value)
    };
    tx.execute(
        "DELETE FROM platform.chapter_page_derivatives WHERE chapter_id=$1",
        &[&chapter],
    )
    .await?;
    tx.execute(
        "DELETE FROM platform.chapter_pages WHERE chapter_id=$1",
        &[&chapter],
    )
    .await?;
    for (index, (page, object)) in pages.iter().zip(objects).enumerate() {
        let position = index as i32;
        tx.execute("INSERT INTO platform.chapter_pages(chapter_id,name,object_id,position) VALUES($1,$2,$3,$4)",&[&chapter,&page.name,&object,&position]).await?;
    }
    for (index, (page, object)) in optimized.iter().enumerate() {
        let position = index as i32;
        tx.execute("INSERT INTO platform.chapter_page_derivatives(chapter_id,name,object_id,position) VALUES($1,$2,$3,$4)",&[&chapter,&page.name,object,&position]).await?;
    }
    if !optimized.is_empty() {
        CatalogRepository::install_optimized_pages(
            &tx,
            &chapter,
            &optimized
                .iter()
                .map(|(page, _)| page.clone())
                .collect::<Vec<_>>(),
        )
        .await?;
    }
    // A real signed publication can be accepted while its source projection is
    // still pending. Return its explicit authenticated receipt rather than NULL
    // or pretend the chapter is publicly readable before source admission.
    let result: Value = tx
        .query_one("SELECT platform.catalog_chapter_json($1)", &[&chapter])
        .await?
        .get::<_, Option<Value>>(0)
        .unwrap_or(initial_result);
    tx.execute("UPDATE platform.upload_sessions SET state='committed',committed_chapter_id=$2,commit_hash=$3,committed_result=$4,resume_draft=$5,resume_page_order=$6,version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id,&chapter,&hash,&result,&native_commit_payload["draft"],&payload.page_order]).await?;
    tx.execute(
        "DELETE FROM platform.upload_files WHERE session_id=$1",
        &[&id],
    )
    .await?;
    queue_unused(&tx).await?;
    crate::protocol::publication::finalize_native_upload(&tx, &id, &manga).await?;
    Ok(result)
}
pub fn validate_order(order: &[String], files: &[String]) -> ApiResult<()> {
    let unique: BTreeSet<_> = order.iter().collect();
    let file_set: BTreeSet<_> = files.iter().collect();
    if order.is_empty()
        || order.len() > MAX_FILES
        || unique.len() != order.len()
        || unique != file_set
        || order.iter().any(|id| !storage::valid_id(id))
    {
        Err(ApiError::bad_request(
            "Page order must contain every session file exactly once",
        ))
    } else {
        Ok(())
    }
}
pub async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let actor = actor(&state, &headers).await?;
    actor.require_permission("self.chapter.update")?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    cancel_in(&tx, &actor, &id).await?;
    tx.commit().await?;
    cleanup(&state).await;
    Ok(StatusCode::NO_CONTENT)
}
pub(crate) async fn cancel_in(tx: &Transaction<'_>, actor: &impl CatalogMutationActor, id: &str) -> ApiResult<()> {
    crate::domains::auth::closure::active_in(tx,actor.user_id()).await?;
    actor.require_permission("self.chapter.update")?;
    session_id(&id)?;
    let row = tx
        .query_opt(
            "SELECT state FROM platform.upload_sessions WHERE id=$1 AND owner_id=$2 FOR UPDATE",
            &[&id, &actor.user_id()],
        )
        .await?
        .ok_or_else(|| ApiError::not_found("Upload session not found"))?;
    if row.get::<_, String>(0) == "scheduled" {
        return Err(ApiError::conflict("Use the signed owner schedule cancellation operation"));
    }
    require_mutable_session(&tx, &id).await?;
    if row.get::<_, String>(0) == "committed" {
        return Err(ApiError::conflict(
            "Committed chapters cannot be cancelled through an upload session",
        ));
    }
    tx.execute("UPDATE platform.upload_sessions SET state='deleted',version=version+1,updated_at=clock_timestamp() WHERE id=$1 AND state='open'",&[&id]).await?;
    tx.execute(
        "DELETE FROM platform.upload_files WHERE session_id=$1",
        &[&id],
    )
    .await?;
    queue_unused(&tx).await?;
    Ok(())
}
pub async fn delete_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, file)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    remove_files(&state, &headers, &id, &[file]).await
}
pub async fn delete_files(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(files): Json<Vec<String>>,
) -> ApiResult<StatusCode> {
    remove_files(&state, &headers, &id, &files).await
}
async fn remove_files(
    state: &AppState,
    headers: &HeaderMap,
    id: &str,
    files: &[String],
) -> ApiResult<StatusCode> {
    let actor = actor(state, headers).await?;
    actor.require_permission("self.chapter.update")?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    remove_files_in(&tx, &actor, id, files).await?;
    tx.commit().await?;
    cleanup(state).await;
    Ok(StatusCode::NO_CONTENT)
}
pub(crate) async fn remove_files_in(tx: &Transaction<'_>, actor: &impl CatalogMutationActor, id: &str, files: &[String]) -> ApiResult<()> {
    crate::domains::auth::closure::active_in(tx,actor.user_id()).await?;
    actor.require_permission("self.chapter.update")?;
    if files.len() > MAX_FILES || files.iter().any(|f| !storage::valid_id(f)) {
        return Err(ApiError::bad_request("Invalid upload file identifiers"));
    }
    open_session(&tx, id, &actor.user_id()).await?;
    require_mutable_session(&tx, id).await?;
    let count: i64 = tx
        .query_one(
            "SELECT count(*) FROM platform.upload_files WHERE session_id=$1 AND id=ANY($2)",
            &[&id, &files],
        )
        .await?
        .get(0);
    if count as usize != files.iter().collect::<BTreeSet<_>>().len() {
        return Err(ApiError::not_found("Upload file not found in this session"));
    }
    tx.execute(
        "DELETE FROM platform.upload_files WHERE session_id=$1 AND id=ANY($2)",
        &[&id, &files],
    )
    .await?;
    tx.execute("UPDATE platform.upload_sessions SET version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id]).await?;
    queue_unused(&tx).await?;
    Ok(())
}
async fn expire_owner(tx: &Transaction<'_>, owner: &str) -> ApiResult<()> {
    tx.execute("UPDATE platform.upload_sessions s SET state='deleted',version=version+1,updated_at=clock_timestamp() WHERE owner_id=$1 AND state='open' AND expires_at<=clock_timestamp() AND NOT EXISTS(SELECT 1 FROM platform.protocol_publication_commands p WHERE p.upload_session_id=s.id AND p.state IN ('prepared','source_accepted')) AND NOT EXISTS(SELECT 1 FROM platform.upload_files retained JOIN platform.protocol_publication_originals pins ON pins.object_id=retained.object_id WHERE retained.session_id=s.id) AND NOT EXISTS(SELECT 1 FROM platform.upload_files retained JOIN platform.upload_file_derivatives derivative ON derivative.file_id=retained.id JOIN platform.protocol_publication_originals pins ON pins.object_id=derivative.object_id WHERE retained.session_id=s.id)",&[&owner]).await?;
    tx.execute("DELETE FROM platform.upload_files f USING platform.upload_sessions s WHERE f.session_id=s.id AND s.owner_id=$1 AND s.state='deleted' AND NOT EXISTS(SELECT 1 FROM platform.protocol_publication_commands p WHERE p.upload_session_id=s.id AND p.state IN ('prepared','source_accepted')) AND NOT EXISTS(SELECT 1 FROM platform.upload_files retained JOIN platform.protocol_publication_originals pins ON pins.object_id=retained.object_id WHERE retained.session_id=s.id) AND NOT EXISTS(SELECT 1 FROM platform.upload_files retained JOIN platform.upload_file_derivatives derivative ON derivative.file_id=retained.id JOIN platform.protocol_publication_originals pins ON pins.object_id=derivative.object_id WHERE retained.session_id=s.id)",&[&owner]).await?;
    queue_unused(tx).await
}
pub(crate) async fn queue_unused(tx: &Transaction<'_>) -> ApiResult<()> {
    // Foreign keys serialize concurrent reference insertion against object deletion.
    let rows=tx.query("WITH unused AS (SELECT o.id FROM platform.media_objects o WHERE NOT EXISTS(SELECT 1 FROM platform.upload_files f WHERE f.object_id=o.id) AND NOT EXISTS(SELECT 1 FROM platform.chapter_pages p WHERE p.object_id=o.id) AND NOT EXISTS(SELECT 1 FROM platform.cover_media c WHERE c.object_id=o.id) AND NOT EXISTS(SELECT 1 FROM platform.upload_file_derivatives d WHERE d.object_id=o.id) AND NOT EXISTS(SELECT 1 FROM platform.chapter_page_derivatives d WHERE d.object_id=o.id) AND NOT EXISTS(SELECT 1 FROM platform.protocol_publication_originals p WHERE p.object_id=o.id) ORDER BY o.created_at,o.id LIMIT 100 FOR UPDATE OF o SKIP LOCKED) DELETE FROM platform.media_objects o USING unused u WHERE o.id=u.id RETURNING o.storage_key",&[]).await?;
    for row in rows {
        tx.execute(
            "INSERT INTO platform.media_cleanup(storage_key) VALUES($1) ON CONFLICT DO NOTHING",
            &[&row.get::<_, String>(0)],
        )
        .await?;
    }
    Ok(())
}
/// A bounded indexed batch reclaims ordinary abandoned sessions without an
/// owner returning. The session lock serializes attachment/commit/cancel, and
/// durable original commands or original-object pins retain recoverable inputs.
async fn expire_abandoned(state: &AppState) -> ApiResult<()> {
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let rows = tx.query("SELECT s.id FROM platform.upload_sessions s WHERE s.state='open' AND s.expires_at<=clock_timestamp() AND NOT EXISTS(SELECT 1 FROM platform.protocol_publication_commands p WHERE p.upload_session_id=s.id AND p.state IN ('prepared','source_accepted')) AND NOT EXISTS(SELECT 1 FROM platform.upload_files f JOIN platform.protocol_publication_originals p ON p.object_id=f.object_id WHERE f.session_id=s.id) AND NOT EXISTS(SELECT 1 FROM platform.upload_files f JOIN platform.upload_file_derivatives d ON d.file_id=f.id JOIN platform.protocol_publication_originals p ON p.object_id=d.object_id WHERE f.session_id=s.id) ORDER BY s.expires_at,s.id LIMIT 8 FOR UPDATE OF s SKIP LOCKED", &[]).await?;
    let ids: Vec<String> = rows.iter().map(|r| r.get(0)).collect();
    if !ids.is_empty() {
        tx.execute("UPDATE platform.upload_sessions SET state='deleted',version=version+1,updated_at=clock_timestamp() WHERE id=ANY($1)", &[&ids]).await?;
        tx.execute(
            "DELETE FROM platform.upload_files WHERE session_id=ANY($1)",
            &[&ids],
        )
        .await?;
    }
    queue_unused(&tx).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn cleanup(state: &AppState) {
    reconcile_pending(state).await;
    if expire_abandoned(state).await.is_err() {
        return;
    }
    let Ok(client) = state.db.client().await else {
        return;
    };
    let Ok(rows) = client
        .query(
            "SELECT storage_key FROM platform.media_cleanup ORDER BY queued_at LIMIT 100",
            &[],
        )
        .await
    else {
        return;
    };
    for row in rows {
        let key: String = row.get(0);
        let Ok(path) = storage::object_path(&state.config.uploads_dir, &key) else {
            continue;
        };
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => continue,
        };
        if storage::sync_directory(path.parent().expect("object parent"))
            .await
            .is_err()
        {
            continue;
        }
        let _ = client
            .execute(
                "DELETE FROM platform.media_cleanup WHERE storage_key=$1",
                &[&key],
            )
            .await;
    }
}

pub async fn get_cover(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let client = state.db.client().await?;
    let row=client.query_opt("SELECT m.state,m.owner_id FROM platform.catalog_covers c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=$1 AND c.deleted_at IS NULL AND m.deleted_at IS NULL",&[&id]).await?.ok_or_else(||ApiError::not_found("Cover not found"))?;
    drop(client);
    if row.get::<_, String>(0) != "published" {
        let actor = auth::optional_principal(&state, &headers)
            .await?
            .ok_or_else(|| ApiError::not_found("Cover not found"))?;
        let owner: String = row.get(1);
        if !(actor.user_id == owner && actor.has_perm("manga.create")
            || actor.is_moderator() && actor.has_perm("manga.cover.edit"))
        {
            return Err(ApiError::not_found("Cover not found"));
        }
    }
    Ok(Json(CatalogRepository::new(state.db).get_cover(&id).await?))
}
pub async fn upload_cover(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(manga): Path<String>,
    mut multipart: Multipart,
) -> ApiResult<Json<Value>> {
    let actor = actor(&state, &headers).await?;
    {
        let mut client = state.db.client().await?;
        let tx = client.transaction().await?;
        crate::domains::catalog::repository::require_manga_edit(
            &tx,
            &manga,
            &actor,
            "manga.cover.edit",
        )
        .await?;
        tx.commit().await?;
    }
    let mut original: Option<StoredImage> = None;
    let mut variants = Vec::new();
    let mut fields = serde_json::Map::new();
    let mut count = 0;
    while let Some(mut field) =
        tokio::time::timeout(std::time::Duration::from_secs(30), multipart.next_field())
            .await
            .map_err(|_| ApiError::bad_request("Multipart upload timed out"))?
            .map_err(|_| ApiError::bad_request("Invalid cover multipart payload"))?
    {
        count += 1;
        if count > 6 {
            return Err(ApiError::bad_request("Too many cover fields"));
        }
        let name = field.name().unwrap_or("").to_owned();
        if name == "file" {
            if original.is_some() {
                return Err(ApiError::bad_request("Only one cover file is allowed"));
            }
            let (image, generated) =
                storage::receive(&state.config.uploads_dir, field, true).await?;
            original = Some(image);
            variants = generated;
        } else {
            if !["language", "volume", "description", "main"].contains(&name.as_str())
                || fields.contains_key(&name)
            {
                return Err(ApiError::bad_request("Invalid or repeated cover field"));
            }
            let mut text = Vec::new();
            while let Some(chunk) =
                tokio::time::timeout(std::time::Duration::from_secs(30), field.chunk())
                    .await
                    .map_err(|_| ApiError::bad_request("Cover metadata upload timed out"))?
                    .map_err(|_| ApiError::bad_request("Invalid cover metadata"))?
            {
                if text.len() + chunk.len() > 50_000 {
                    return Err(ApiError::bad_request("Cover metadata is too long"));
                }
                text.extend_from_slice(&chunk);
            }
            let text = String::from_utf8(text)
                .map_err(|_| ApiError::bad_request("Cover metadata must be UTF-8"))?;
            if text.contains('\0') {
                return Err(ApiError::bad_request("Invalid cover metadata"));
            }
            fields.insert(name, Value::String(text));
        }
    }
    let mut original = original.ok_or_else(|| ApiError::bad_request("Cover image is required"))?;
    let language = fields
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or("en")
        .to_owned();
    crate::domains::catalog::validation::language(&language)?;
    let volume = fields
        .get("volume")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    if volume.len() > 32 {
        return Err(ApiError::bad_request("Cover volume is too long"));
    }
    let description = fields
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let main = match fields.get("main").and_then(Value::as_str) {
        None | Some("false") => false,
        Some("true") => true,
        _ => return Err(ApiError::bad_request("Invalid main cover flag")),
    };
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    crate::domains::catalog::repository::require_manga_edit(
        &tx,
        &manga,
        &actor,
        "manga.cover.edit",
    )
    .await?;
    let id = storage::short_id();
    let record = CoverPublication {
        id: id.clone(),
        manga_id: manga,
        uploader_id: actor.user_id.clone(),
        uploader_name: actor.username.clone(),
        ext: original.ext.into(),
        dimensions: [original.width, original.height],
        volume,
        language,
        description,
        main,
    };
    let result = CatalogRepository::publish_cover(&tx, &record).await?;
    let key = insert_object(&tx, &original).await?;
    tx.execute(
        "INSERT INTO platform.cover_media(cover_id,variant,object_id) VALUES($1,'original',$2)",
        &[&id, &key],
    )
    .await?;
    for (variant, image) in &variants {
        let key = insert_object(&tx, image).await?;
        tx.execute(
            "INSERT INTO platform.cover_media(cover_id,variant,object_id) VALUES($1,$2,$3)",
            &[&id, &variant, &key],
        )
        .await?;
    }
    original.protect_commit(&state.config.uploads_dir).await?;
    for (_, image) in &mut variants {
        image.protect_commit(&state.config.uploads_dir).await?;
    }
    tx.commit().await?;
    original.confirm_persisted().await;
    for (_, image) in &mut variants {
        image.confirm_persisted().await;
    }
    Ok(Json(result))
}
pub async fn update_cover(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(patch): Json<Value>,
) -> ApiResult<Json<Value>> {
    let actor = actor(&state, &headers).await?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let value = CatalogRepository::update_cover(&tx, &id, &actor, &patch).await?;
    tx.commit().await?;
    Ok(Json(value))
}
pub async fn delete_cover(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let actor = actor(&state, &headers).await?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    CatalogRepository::delete_cover(&tx, &id, &actor).await?;
    tx.execute("DELETE FROM platform.cover_media WHERE cover_id=$1", &[&id])
        .await?;
    queue_unused(&tx).await?;
    tx.commit().await?;
    cleanup(&state).await;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn update_chapter(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(patch): Json<Value>,
) -> ApiResult<Json<Value>> {
    let actor = actor(&state, &headers).await?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let value = CatalogRepository::update_chapter(&tx, &id, &actor, &patch).await?;
    tx.commit().await?;
    Ok(Json(value))
}
pub async fn delete_chapter(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    let actor = actor(&state, &headers).await?;
    // Sign/request before taking native row locks. The durable original request
    // survives a later native transaction failure and is replayed on retry.
    // This is a request for moderator review, never automatic source approval.
    let receipt =
        crate::protocol::deletions::request_native_chapter_deletion(&state, &actor, &id).await?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        "x-weebdex-instance-removal",
        HeaderValue::from_static("soft-hidden"),
    );
    response.headers_mut().insert(
        "x-weebdex-source-deletion-status",
        HeaderValue::from_static(if receipt.is_some() {
            "moderator-review-requested"
        } else {
            "instance-local-only"
        }),
    );
    if let Some(receipt) = &receipt {
        response.headers_mut().insert(
            "x-weebdex-source-deletion-request",
            HeaderValue::from_str(&receipt.request_id).map_err(|_| ApiError::unavailable())?,
        );
    }
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    CatalogRepository::delete_chapter(&tx, &id, &actor).await?;
    if let Some(receipt) = &receipt {
        let annotation = json!({
            "source_deletion_request_id": receipt.request_id,
            "source_deletion_status": receipt.source_status,
            "instance_removal_status": receipt.instance_effect,
        });
        tx.execute(
            "UPDATE platform.catalog_chapters SET metadata=metadata || $2::jsonb WHERE id=$1",
            &[&id, &annotation],
        )
        .await?;
    }
    // Soft removal keeps originals for recoverability. The public media join denies removed chapters.
    tx.commit().await?;
    Ok(response)
}
pub async fn claim_chapter(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let actor = actor(&state, &headers).await?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let value = CatalogRepository::claim_chapter(&tx, &id, &actor).await?;
    tx.commit().await?;
    Ok(Json(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_replacement_always_fences_version_and_native_identical_retry_is_preserved() {
        for identical in [false, true] {
            assert!(require_replacement_version(ReplacementVersionPolicy::Exact, 9, 9, identical).is_ok());
            assert!(require_replacement_version(ReplacementVersionPolicy::Exact, 9, 8, identical).is_err());
            assert!(require_replacement_version(ReplacementVersionPolicy::NativeIdenticalRetry, 9, 9, identical).is_ok());
        }
        assert!(require_replacement_version(ReplacementVersionPolicy::NativeIdenticalRetry, 9, 8, true).is_ok());
        assert!(require_replacement_version(ReplacementVersionPolicy::NativeIdenticalRetry, 9, 8, false).is_err());
    }
    #[test]
    fn commit_order_cannot_duplicate_omit_or_reference_another_session_file() {
        let files = vec!["Abcdef1234".into(), "Defghi5678".into()];
        assert!(validate_order(&files, &files).is_ok());
        assert!(validate_order(&vec![files[1].clone(), files[0].clone()], &files).is_ok());
        for order in [
            vec![],
            vec![files[0].clone()],
            vec![files[0].clone(), files[0].clone()],
            vec![files[0].clone(), "Otheri9999".into()],
            vec!["../secret".into()],
        ] {
            assert!(validate_order(&order, &files).is_err());
        }
    }
    #[test]
    fn draft_accepts_source_language_variants_and_rejects_invalid_numbers() {
        for language in ["en", "zh-hk", "es-la", "pt-br"] {
            assert!(
                validate_draft(&Draft {
                    volume: Some("1.5".into()),
                    chapter: Some("12".into()),
                    title: Some("日本語".into()),
                    language: language.into()
                })
                .is_ok()
            );
        }
        for value in [
            "../../secret",
            "1e10",
            "a",
            "-1",
            "01",
            "1..2",
            &"1".repeat(33),
        ] {
            assert!(
                validate_draft(&Draft {
                    volume: None,
                    chapter: Some(value.into()),
                    title: None,
                    language: "en".into()
                })
                .is_err()
            );
        }
    }
}

/// Recover uncertain commits without deleting an original still referenced by a committed row.
/// The matching object advisory lock prevents a missing-row check racing an active commit.
pub async fn reconcile_pending(state: &AppState) {
    let directory = state.config.uploads_dir.join("pending");
    let Ok(mut entries) = tokio::fs::read_dir(&directory).await else {
        return;
    };
    let mut inspected = 0usize;
    let mut checked = 0usize;
    while let Ok(Some(entry)) = entries.next_entry().await {
        inspected += 1;
        if inspected > 1000 || checked >= 100 {
            break;
        }
        let Some(key) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !storage::valid_key(&key) {
            continue;
        }
        let Ok(metadata) = entry.metadata().await else {
            continue;
        };
        let Ok(age) = metadata
            .modified()
            .and_then(|time| time.elapsed().map_err(std::io::Error::other))
        else {
            continue;
        };
        if age.as_secs() < 300 {
            continue;
        }
        let Ok(marker) = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(entry.path())
            .await
        else {
            continue;
        };
        let marker = marker.into_std().await;
        if marker.try_lock().is_err() {
            continue;
        }
        checked += 1;
        let Ok(mut client) = state.db.client().await else {
            return;
        };
        let Ok(tx) = client.transaction().await else {
            continue;
        };
        let Ok(lock) = tx
            .query_one(
                "SELECT pg_try_advisory_xact_lock(hashtextextended($1,724194))",
                &[&key],
            )
            .await
        else {
            continue;
        };
        if !lock.get::<_, bool>(0) {
            continue;
        }
        let Ok(row) = tx
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM platform.media_objects WHERE storage_key=$1)",
                &[&key],
            )
            .await
        else {
            continue;
        };
        if !row.get::<_, bool>(0) {
            let Ok(path) = storage::object_path(&state.config.uploads_dir, &key) else {
                continue;
            };
            match tokio::fs::remove_file(&path).await {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => continue,
            };
            if storage::sync_directory(path.parent().expect("object parent"))
                .await
                .is_err()
            {
                continue;
            }
        }
        if tx.commit().await.is_ok() {
            if tokio::fs::remove_file(entry.path()).await.is_ok() {
                let _ = storage::sync_directory(&directory).await;
            }
        }
    }
}
