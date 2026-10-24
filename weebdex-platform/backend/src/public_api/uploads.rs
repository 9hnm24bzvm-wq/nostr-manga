//! MangaDex upload transport over the authoritative native upload core.
//! UUIDs select native rows, never files, principals, signers or permissions.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdTarget, PublicUuid,
    ResourceKind,
    auth::{admitted_request, bound_account_read, bound_account_write},
    catalogue_dto::AliasIndex,
    dto::{Relationship, ResourceType},
};
use crate::{
    AppState,
    domains::{catalog::CatalogRepository, uploads::{sessions as core, storage}},
};
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{DefaultBodyLimit, OriginalUri, Path, Request, State},
    http::{HeaderMap, StatusCode, header},
    routing::{delete, get, post},
    response::{IntoResponse, Response},
};
use manga_node::http_auth::AdmittedHttpCapability;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{collections::BTreeSet, time::Duration};
use tokio_postgres::Transaction;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/upload", get(get_current))
        .route("/upload/begin", post(begin))
        .route("/upload/begin/{p0}", post(begin_edit))
        .route("/upload/check-approval-required", post(check_approval))
        .route("/upload/{p0}", post(upload).delete(cancel)
            .layer(DefaultBodyLimit::max(super::uploads_body::BODY_LIMIT)))
        .route("/upload/{p0}/commit", post(commit))
        .route("/upload/{p0}/batch", delete(delete_batch))
        .route("/upload/{p0}/{p1}", delete(delete_file))
}

pub(super) fn no_query(uri: &axum::http::Uri) -> PublicApiResult<()> {
    if uri.query().is_some() {
        Err(PublicApiError::bad_request("Upload operations accept no query parameters"))
    } else { Ok(()) }
}
fn content_type(headers: &HeaderMap) -> PublicApiResult<()> {
    let values = headers.get_all(header::CONTENT_TYPE).iter().collect::<Vec<_>>();
    if values.len() != 1 || values[0].to_str().ok()
        .and_then(|v| v.split(';').next()).is_none_or(|v| !v.trim().eq_ignore_ascii_case("application/json")) {
        return Err(PublicApiError::new(StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Unsupported Media Type", "Expected application/json"));
    }
    Ok(())
}
pub(super) async fn authorize(
    state: &AppState, uri: &axum::http::Uri, headers: &HeaderMap,
    request: Request, maximum: usize, empty: bool,
) -> PublicApiResult<(AdmittedHttpCapability, Vec<u8>)> {
    no_query(uri)?;
    authorize_body(state, uri, headers, request, maximum, empty).await
}
// Only the explicitly opted-in empty-body operations accept this signed URL
// extension. Ordinary upload/cover JSON and multipart queries remain closed.
pub(super) async fn authorize_account_query(
    state: &AppState, uri: &axum::http::Uri, headers: &HeaderMap, request: Request,
) -> PublicApiResult<(AdmittedHttpCapability, Option<Value>)> {
    let expected = account_query(uri)?;
    let (cap, _)=authorize_body(state,uri,headers,request,1,true).await?;
    Ok((cap,expected))
}
fn account_query(uri: &axum::http::Uri) -> PublicApiResult<Option<Value>> {
    let expected = match uri.query() {
        None => None,
        Some(query) => {
            let fields=url::form_urlencoded::parse(query.as_bytes()).collect::<Vec<_>>();
            if fields.len()!=1 || fields[0].0!="expectedAccountId" {
                return Err(PublicApiError::bad_request("Only one signed expectedAccountId query is supported"));
            }
            Some(json!(PublicUuid::parse(&fields[0].1)?))
        }
    };
    Ok(expected)
}
pub(super) async fn authorize_body(
    state: &AppState, uri: &axum::http::Uri, headers: &HeaderMap,
    request: Request, maximum: usize, empty: bool,
) -> PublicApiResult<(AdmittedHttpCapability, Vec<u8>)> {
    if !empty { content_type(headers)?; }
    let method = request.method().as_str().to_owned();
    let body = tokio::time::timeout(Duration::from_secs(5), to_bytes(request.into_body(), maximum))
        .await.map_err(|_| PublicApiError::bad_request("Request body timeout"))?
        .map_err(|_| PublicApiError::bad_request("Request body exceeds the operation budget"))?;
    if empty && !body.is_empty() {
        return Err(PublicApiError::bad_request("This operation does not accept a request body"));
    }
    let cap = admitted_request(state, uri, headers, &method, &body).await?;
    Ok((cap, body.to_vec()))
}
// Missing remains compatible; an explicitly present null is not an account UUID.
pub(super) fn deserialize_optional_account<'de,D:serde::Deserializer<'de>>(input:D)->Result<Option<PublicUuid>,D::Error> {
    PublicUuid::deserialize(input).map(Some)
}
pub(super) fn parse<T: DeserializeOwned>(body: &[u8]) -> PublicApiResult<T> {
    serde_json::from_slice(body).map_err(|_| PublicApiError::bad_request("Invalid upload request body"))
}
pub(super) async fn resolve(tx: &Transaction<'_>, kind: ResourceKind, public: &PublicUuid) -> PublicApiResult<String> {
    let resolved = PostgresPublicIdResolver::resolve_many_in(tx, kind, std::slice::from_ref(public))
        .await?.remove(public).ok_or_else(PublicApiError::not_found)?;
    if resolved.target != PublicIdTarget::NativeResource { return Err(PublicApiError::not_found()); }
    Ok(resolved.native_id)
}
pub(super) async fn preferred(tx: &Transaction<'_>, kind: ResourceKind, native: &str) -> PublicApiResult<PublicUuid> {
    let row = tx.query_opt("SELECT public_id::text FROM platform.public_resource_ids WHERE kind=$1 AND native_id=$2 AND preferred", &[&kind.as_str(), &native]).await?
        .ok_or_else(PublicApiError::unavailable)?;
    PublicUuid::parse(row.get::<_, &str>(0)).map_err(|_| PublicApiError::unavailable())
}
async fn session_resource(tx: &Transaction<'_>, native: &Value) -> PublicApiResult<Value> {
    let id = native["id"].as_str().ok_or_else(PublicApiError::unavailable)?;
    Ok(json!({"id":preferred(tx, ResourceKind::UploadSession, id).await?,"type":"upload_session","attributes":{
        "isCommitted":native["is_committed"],"isProcessed":native["is_processed"],"isDeleted":native["is_deleted"],
        "version":native["version"],"createdAt":native["created_at"],"updatedAt":native["updated_at"]}}))
}
pub(super) async fn file_resource(tx: &Transaction<'_>, native: &Value) -> PublicApiResult<Value> {
    let id = native["id"].as_str().ok_or_else(PublicApiError::unavailable)?;
    let row=tx.query_one("SELECT f.original_name,o.digest,o.bytes,o.mime FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id WHERE f.id=$1", &[&id]).await?;
    Ok(json!({"id":preferred(tx,ResourceKind::UploadFile,id).await?,"type":"upload_session_file","attributes":{
        "originalFileName":row.get::<_,String>(0),"fileHash":row.get::<_,String>(1),"fileSize":row.get::<_,i64>(2),
        "mimeType":row.get::<_,String>(3),"source":"local","version":1}}))
}
pub(super) async fn chapter_resource(tx: &Transaction<'_>, native: &Value) -> PublicApiResult<Value> {
    optional_chapter_resource(tx,native).await?.ok_or_else(PublicApiError::unavailable)
}
// An authenticated pending receipt has no readable parent projection. Keep only
// the real parent ID, using a transaction-held chapter binding and persisted
// preferred alias. Saved receipt fields may confirm that binding, never replace it.
fn pending_manga_relationship(native:&Value, target:Option<(&str,Option<&str>)>) -> Option<Relationship> {
    let (manga,alias)=target?;
    let id=PublicUuid::parse(alias?).ok()?;
    if native.get("manga_id").is_some_and(|value|value.as_str()!=Some(manga)) { return None; }
    if let Some(relationships)=native.get("relationships") {
        let relationships=relationships.as_object()?;
        if relationships.get("manga").is_some_and(|value|!value.is_null() && value["id"].as_str()!=Some(manga)) { return None; }
    }
    Some(Relationship{id,resource_type:ResourceType::Manga,related:None,attributes:None})
}
/// Only the pure presentation step can be unavailable here. SQL/authentication
/// failures while loading aliases still propagate, including after publication.
pub(super) async fn optional_chapter_resource(tx: &Transaction<'_>, native: &Value) -> PublicApiResult<Option<Value>> {
    let pending=matches!(native["publication_status"].as_str(),Some("accepted_pending_projection"|"pending_projection"));
    let pending_manga=if pending {
        let Some(chapter)=native["id"].as_str() else { return Ok(None); };
        let row=tx.query_opt("SELECT c.manga_id,a.public_id::text FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id LEFT JOIN platform.public_resource_ids a ON a.kind='manga' AND a.native_id=c.manga_id AND a.preferred WHERE c.id=$1 AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.state='published' FOR SHARE OF c,m",&[&chapter]).await?;
        let target=row.as_ref().map(|row|(row.get::<_,&str>(0),row.get::<_,Option<&str>>(1)));
        let Some(relationship)=pending_manga_relationship(native,target) else { return Ok(None); };
        Some(relationship)
    } else { None };
    let aliases=AliasIndex::load_in(tx,&[(ResourceKind::Chapter,native.clone())]).await?;
    let mut resource=match aliases.resource(ResourceKind::Chapter,native,&BTreeSet::new()) {
        Ok(resource)=>resource,
        Err(error) if error.status==StatusCode::SERVICE_UNAVAILABLE=>return Ok(None),
        Err(error)=>return Err(error),
    };
    if let Some(manga)=pending_manga {
        resource.relationships.retain(|relationship|relationship.resource_type!=ResourceType::Manga);
        resource.relationships.push(manga);
    }
    if let Some(status)=native["publication_status"].as_str() {
        resource.attributes["nostr"]=json!({"publicationScope":native["publication_scope"],"publicationStatus":status,"eventId":native["protocol_event_id"]});
        if matches!(status,"accepted_pending_projection"|"pending_projection") { resource.attributes["isUnavailable"]=json!(true); }
    }
    serde_json::to_value(resource).map(Some).map_err(|_|PublicApiError::unavailable())
}

async fn get_current(State(state): State<AppState>, OriginalUri(uri): OriginalUri,
    headers: HeaderMap, request: Request) -> PublicApiResult<Json<Value>> {
    let (cap, expected) = authorize_account_query(&state,&uri,&headers,request).await?;
    let mut client=state.db.client().await?; let tx=client.transaction().await?;
    let actor=bound_account_read(&tx,&cap).await?;
    if let Some(expected)=expected {
        let id=PublicUuid::parse(expected.as_str().ok_or_else(PublicApiError::unavailable)?)?;
        if resolve(&tx,ResourceKind::User,&id).await?!=actor.user_id(){return Err(PublicApiError::conflict("Upload account binding changed; reload the intended account"));}
    }
    let row=tx.query_opt("SELECT id FROM platform.upload_sessions s WHERE owner_id=$1 AND ((state='open' AND (expires_at>clock_timestamp() OR EXISTS(SELECT 1 FROM platform.protocol_publication_commands p WHERE p.upload_session_id=s.id AND p.user_id=s.owner_id AND p.state IN ('prepared','source_accepted')))) OR (state='committed' AND committed_result->>'publication_status' IN ('accepted_pending_projection','pending_projection') AND EXISTS(SELECT 1 FROM platform.catalog_chapters c WHERE c.id=s.committed_chapter_id AND c.deleted_at IS NULL))) ORDER BY (state='open') DESC,updated_at DESC,id LIMIT 1", &[&actor.user_id()]).await?
        .ok_or_else(PublicApiError::not_found)?;
    let native=core::session_json(&tx,&row.get::<_,String>(0)).await?;
    let result=session_resource(&tx,&native).await?;
    tx.commit().await?; Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Begin { manga: PublicUuid, groups: Vec<PublicUuid>, #[serde(default,rename="expectedAccountId",deserialize_with="deserialize_optional_account")] expected_account_id:Option<PublicUuid> }
async fn begin(State(state): State<AppState>, OriginalUri(uri): OriginalUri,
    headers: HeaderMap, request: Request) -> PublicApiResult<Json<Value>> {
    let (cap,body)=authorize(&state,&uri,&headers,request,4*1024,false).await?;
    let payload:Begin=parse(&body)?;
    if payload.groups.len()>10 || payload.groups.iter().collect::<BTreeSet<_>>().len()!=payload.groups.len() {
        return Err(PublicApiError::bad_request("Select at most ten distinct groups"));
    }
    let mut client=state.db.client().await?; let tx=client.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;
    let expected=payload.expected_account_id.as_ref().map(|id|json!(id));
    super::editorial::expected_account(&tx,&actor,expected.as_ref()).await?;
    let manga=resolve(&tx,ResourceKind::Manga,&payload.manga).await?;
    let mut groups=Vec::new(); for group in &payload.groups { groups.push(resolve(&tx,ResourceKind::Group,group).await?); }
    let native=core::begin_in(&tx,&actor,&core::Begin{manga,groups}).await?;
    let result=session_resource(&tx,&native).await?;
    tx.commit().await?; core::cleanup(&state).await; Ok(Json(result))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit { version:i32, #[serde(default)] replace:bool, #[serde(default,rename="expectedAccountId",deserialize_with="deserialize_optional_account")] expected_account_id:Option<PublicUuid> }
async fn begin_edit(State(state): State<AppState>, Path(chapter): Path<String>,
    OriginalUri(uri): OriginalUri, headers: HeaderMap, request: Request) -> PublicApiResult<Json<Value>> {
    let chapter=PublicUuid::parse(&chapter)?;
    let (cap,body)=authorize(&state,&uri,&headers,request,1024,false).await?;
    let payload:Edit=parse(&body)?;
    if payload.version<1 { return Err(PublicApiError::bad_request("Invalid chapter version")); }
    let mut client=state.db.client().await?; let tx=client.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;
    let expected=payload.expected_account_id.as_ref().map(|id|json!(id));
    super::editorial::expected_account(&tx,&actor,expected.as_ref()).await?;
    let public_chapter=chapter.clone();
    let chapter=resolve(&tx,ResourceKind::Chapter,&chapter).await?;
    let native=core::begin_edit_in(&tx,&actor,&chapter,&core::Edit{version:payload.version,replace:payload.replace}).await?;
    let mut result=session_resource(&tx,&native).await?;
    // Bind successful admission to the signed target, version and chosen mode.
    result["attributes"]["edit"] = json!({"chapterId":public_chapter,"version":payload.version,"replace":payload.replace});
    tx.commit().await?; core::cleanup(&state).await; Ok(Json(result))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Approval { manga:PublicUuid, locale:String }
async fn check_approval(State(state): State<AppState>, OriginalUri(uri): OriginalUri,
    headers: HeaderMap, request: Request) -> PublicApiResult<Json<Value>> {
    let (cap,body)=authorize(&state,&uri,&headers,request,4*1024,false).await?;
    let payload:Approval=parse(&body)?;
    crate::domains::catalog::validation::public_language(&payload.locale)?;
    let mut client=state.db.client().await?; let tx=client.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;
    let manga=resolve(&tx,ResourceKind::Manga,&payload.manga).await?;
    CatalogRepository::ensure_upload_target(&tx,&manga,&actor,&[]).await?;
    // This instance's explicit chapter.create grant permits immediate upload.
    // No MangaDex moderation queue or global approval is invented here.
    tx.commit().await?; Ok(Json(json!({"result":"ok","requiresApproval":false})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
struct Commit { chapter_draft:ChapterDraft, page_order:Vec<PublicUuid>, terms_accepted:bool }
#[derive(Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
struct ChapterDraft {
    #[serde(deserialize_with="required_nullable")] volume:Option<String>,
    #[serde(deserialize_with="required_nullable")] chapter:Option<String>,
    #[serde(deserialize_with="required_nullable")] title:Option<String>,
    translated_language:String,
    #[serde(default)] external_url:Option<String>, #[serde(default,deserialize_with="optional_publish_at")] publish_at:Option<String>,
}
fn optional_publish_at<'de,D:serde::Deserializer<'de>>(deserializer:D)->Result<Option<String>,D::Error> {
    let value=String::deserialize(deserializer)?;
    if !super::publish_at::valid_publish_at(&value) {return Err(serde::de::Error::custom("publishAt must be a real YYYY-MM-DDTHH:MM:SS UTC timestamp"));}
    Ok(Some(value))
}
fn required_nullable<'de,D:serde::Deserializer<'de>>(deserializer:D) -> Result<Option<String>,D::Error> {
    Option::<String>::deserialize(deserializer)
}
fn draft(payload:ChapterDraft, terms:bool) -> PublicApiResult<core::Draft> {
    if !terms { return Err(PublicApiError::bad_request("termsAccepted must be true for an upload commit")); }
    if payload.external_url.is_some() {
        return Err(PublicApiError::bad_request("External chapters are unavailable in this instance's image upload module"));
    }
    if payload.title.as_ref().is_some_and(|v|v.chars().count()>255) {
        return Err(PublicApiError::bad_request("Chapter title exceeds 255 characters"));
    }
    crate::domains::catalog::validation::public_language(&payload.translated_language)?;
    Ok(core::Draft{volume:payload.volume,chapter:payload.chapter,title:payload.title,language:payload.translated_language})
}
async fn commit(State(state): State<AppState>, Path(session): Path<String>,
    OriginalUri(uri): OriginalUri, headers: HeaderMap, request: Request) -> PublicApiResult<Response> {
    let session=PublicUuid::parse(&session)?;
    // 500 public UUIDs alone need about 20 KiB, before the chapter draft.
    let (cap,body)=authorize(&state,&uri,&headers,request,32*1024,false).await?;
    let payload:Commit=parse(&body)?;
    if payload.page_order.is_empty() || payload.page_order.len()>500 {
        return Err(PublicApiError::bad_request("pageOrder must contain one to 500 files"));
    }
    let publish_at=payload.chapter_draft.publish_at.clone();
    let draft=draft(payload.chapter_draft,payload.terms_accepted)?;
    let mut client=state.db.client().await?; let tx=client.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;
    let session=resolve(&tx,ResourceKind::UploadSession,&session).await?;
    let mut page_order=Vec::new();
    for file in payload.page_order { page_order.push(resolve(&tx,ResourceKind::UploadFile,&file).await?); }
    let payload=core::Commit{draft,page_order};
    if let Some(receipt)=super::scheduled_uploads::admit(&state,&tx,&actor,&cap,&session,publish_at.as_deref(),&payload).await? {
        tx.commit().await?;
        return Ok((StatusCode::ACCEPTED,Json(receipt)).into_response());
    }
    let native=core::commit_in(&state,&tx,&actor,&session,&payload).await?;
    let result=chapter_resource(&tx,&native).await?;
    tx.commit().await?; core::cleanup(&state).await; Ok(Json(result).into_response())
}
async fn cancel(State(state): State<AppState>, Path(session): Path<String>,
    OriginalUri(uri): OriginalUri, headers: HeaderMap, request: Request) -> PublicApiResult<Json<Value>> {
    let session=PublicUuid::parse(&session)?;
    let (cap,expected)=authorize_account_query(&state,&uri,&headers,request).await?;
    let mut client=state.db.client().await?; let tx=client.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;
    // Optional for existing callers, mandatory in the schedule owner UI. Compare
    // the signed account before resolving or accessing any private session.
    super::editorial::expected_account(&tx,&actor,expected.as_ref()).await?;
    let session=resolve(&tx,ResourceKind::UploadSession,&session).await?;
    if !super::scheduled_uploads::cancel_in(&tx,&actor,&session).await? {core::cancel_in(&tx,&actor,&session).await?;}
    tx.commit().await?; core::cleanup(&state).await; Ok(Json(json!({"result":"ok"})))
}
async fn delete_file(State(state): State<AppState>, Path((session,file)): Path<(String,String)>,
    OriginalUri(uri): OriginalUri, headers: HeaderMap, request: Request) -> PublicApiResult<Json<Value>> {
    let session=PublicUuid::parse(&session)?;
    let file=PublicUuid::parse(&file)?;
    let (cap,_)=authorize(&state,&uri,&headers,request,1,true).await?;
    remove(&state,&cap,&session,&[file]).await
}
async fn delete_batch(State(state): State<AppState>, Path(session): Path<String>,
    OriginalUri(uri): OriginalUri, headers: HeaderMap, request: Request) -> PublicApiResult<Json<Value>> {
    let session=PublicUuid::parse(&session)?;
    let (cap,body)=authorize(&state,&uri,&headers,request,20*1024,false).await?;
    let files:Vec<PublicUuid>=parse(&body)?;
    if files.is_empty() || files.len()>500 || files.iter().collect::<BTreeSet<_>>().len()!=files.len() {
        return Err(PublicApiError::bad_request("Select one to 500 distinct files"));
    }
    remove(&state,&cap,&session,&files).await
}
async fn remove(state:&AppState, cap:&AdmittedHttpCapability, session:&PublicUuid, files:&[PublicUuid]) -> PublicApiResult<Json<Value>> {
    let mut client=state.db.client().await?; let tx=client.transaction().await?;
    let actor=bound_account_write(&tx,cap).await?;
    let session=resolve(&tx,ResourceKind::UploadSession,session).await?;
    let mut native=Vec::new(); for file in files { native.push(resolve(&tx,ResourceKind::UploadFile,file).await?); }
    core::remove_files_in(&tx,&actor,&session,&native).await?;
    tx.commit().await?; core::cleanup(state).await; Ok(Json(json!({"result":"ok"})))
}

async fn upload(State(state): State<AppState>, Path(session): Path<String>,
    OriginalUri(uri): OriginalUri, headers: HeaderMap, request: Request) -> PublicApiResult<Json<Value>> {
    let session=PublicUuid::parse(&session)?;
    no_query(&uri)?;
    let (cap,mut multipart,_permit)=super::uploads_body::receive(&state,&uri,&headers,request).await?;
    let native_session;
    {
        let mut client=state.db.client().await?; let tx=client.transaction().await?;
        let actor=bound_account_write(&tx,&cap).await?;
        actor.require_permission("self.chapter.update")?;
        native_session=resolve(&tx,ResourceKind::UploadSession,&session).await?;
        core::open_session(&tx,&native_session,actor.user_id()).await?;
        core::require_mutable_session(&tx,&native_session).await?;
        tx.commit().await?;
    }
    let mut data=Vec::new(); let mut errors=Vec::new(); let mut fields=0usize;
    while let Some(field)=tokio::time::timeout(Duration::from_secs(30),multipart.next_field()).await
        .map_err(|_|PublicApiError::bad_request("Multipart upload timed out"))?
        .map_err(|_|PublicApiError::bad_request("Invalid multipart payload"))? {
        fields+=1;
        if fields>5 || field.name()!=Some("file") {
            return Err(PublicApiError::bad_request("Upload one to five multipart fields named file"));
        }
        let original=field.file_name().unwrap_or("image").chars().take(255).collect::<String>();
        let received=storage::receive(&state.config.uploads_dir,field,false).await;
        let (mut image,mut variants)=match received {
            Ok(received)=>received,
            Err(error)=>{ errors.push(file_error(error.into(),&original)); continue; }
        };
        let attached=async {
            let mut client=state.db.client().await?; let tx=client.transaction().await?;
            // Intake/decoding may take time. Recheck the original account/service,
            // explicit binding, current native grants, ownership and immutable
            // pending publication fence in this same attachment transaction.
            let actor=bound_account_write(&tx,&cap).await?;
            let native=core::attach_in(&state,&tx,&native_session,&actor,&original,&mut image,&mut variants).await?;
            let result=file_resource(&tx,&native).await?;
            tx.commit().await?;
            Ok::<_,PublicApiError>(result)
        }.await;
        match attached {
            Ok(file)=>{
                image.confirm_persisted().await;
                for (_,variant) in &mut variants { variant.confirm_persisted().await; }
                data.push(file);
            }
            Err(error)=>{
                // Authorization/ownership failures are not per-image validation
                // errors and may never become a successful upload response.
                if error.status!=StatusCode::BAD_REQUEST { return Err(error); }
                errors.push(file_error(error,&original));
            }
        }
    }
    if fields==0 { return Err(PublicApiError::bad_request("At least one image is required")); }
    Ok(Json(json!({"result":if errors.is_empty(){"ok"}else{"error"},"errors":errors,"data":data})))
}
fn file_error(error:PublicApiError, original:&str) -> Value {
    // No native identifiers, storage paths or SQL details cross this boundary.
    json!({"id":random_error_id(),"status":error.status.as_u16(),"title":error.title,"detail":error.detail,"context":{"originalFileName":original}})
}
fn random_error_id() -> String {
    let mut bytes=rand::random::<[u8;16]>(); bytes[6]=(bytes[6]&15)|64; bytes[8]=(bytes[8]&63)|128;
    let value=hex::encode(bytes); format!("{}-{}-{}-{}-{}",&value[..8],&value[8..12],&value[12..16],&value[16..20],&value[20..])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_receipt_target_is_actual_id_only_and_does_not_imply_readability() {
        let native=json!({"id":"Chapter001","manga_id":"UploadMng1","is_unavailable":true,"publication_status":"accepted_pending_projection"});
        let alias="11111111-1111-8111-8111-111111111111";
        let relation=pending_manga_relationship(&native,Some(("UploadMng1",Some(alias)))).unwrap();
        let relation=serde_json::to_value(relation).unwrap();
        assert_eq!(relation,json!({"id":alias,"type":"manga"}));
        assert_eq!(native["is_unavailable"],true);
        assert_eq!(native["publication_status"],"accepted_pending_projection");
        let existing=json!({"manga_id":"UploadMng1","relationships":{"manga":{"id":"UploadMng1","title":"not a public profile"}}});
        assert!(pending_manga_relationship(&existing,Some(("UploadMng1",Some(alias)))).unwrap().attributes.is_none());
    }
    #[test]
    fn pending_receipt_target_mismatches_and_missing_aliases_fail_closed() {
        let alias="11111111-1111-8111-8111-111111111111";
        for native in [
            json!({"manga_id":"OtherMng01"}),
            json!({"manga_id":null}),
            json!({"manga_id":"UploadMng1","relationships":{"manga":{"id":"OtherMng01"}}}),
            json!({"relationships":{"manga":"malformed"}}),
            json!({"relationships":[]}),
        ] { assert!(pending_manga_relationship(&native,Some(("UploadMng1",Some(alias)))).is_none()); }
        let native=json!({"manga_id":"UploadMng1"});
        assert!(pending_manga_relationship(&native,None).is_none());
        assert!(pending_manga_relationship(&native,Some(("UploadMng1",None))).is_none());
        assert!(pending_manga_relationship(&native,Some(("UploadMng1",Some("not-a-public-uuid")))).is_none());
    }
    #[test]
    fn captured_account_extensions_are_optional_but_strict_when_present() {
        let id="11111111-1111-8111-8111-111111111111";
        assert!(account_query(&"/upload".parse().unwrap()).unwrap().is_none());
        assert_eq!(account_query(&format!("/upload?expectedAccountId={id}").parse().unwrap()).unwrap(),Some(json!(id)));
        for query in ["", "unknown=1", "expectedAccountId=native", "expectedAccountId=", "expectedAccountId=bad&expectedAccountId=bad"] {
            assert!(account_query(&format!("/upload?{query}").parse().unwrap()).is_err());
        }
        for raw in [format!(r#"{{"manga":"{id}","groups":[]}}"#),format!(r#"{{"manga":"{id}","groups":[],"expectedAccountId":"{id}"}}"#)] {assert!(parse::<Begin>(raw.as_bytes()).is_ok());}
        for field in ["null", "false", "[]", "\"native\""] {
            assert!(parse::<Begin>(format!(r#"{{"manga":"{id}","groups":[],"expectedAccountId":{field}}}"#).as_bytes()).is_err());
            assert!(parse::<Edit>(format!(r#"{{"version":1,"expectedAccountId":{field}}}"#).as_bytes()).is_err());
        }
        assert!(parse::<Edit>(format!(r#"{{"version":1,"expectedAccountId":"{id}","expectedAccountId":"{id}"}}"#).as_bytes()).is_err());
    }
    #[test]
    fn exact_transport_names_and_unsupported_features_are_explicit() {
        assert!(parse::<Begin>(br#"{"manga":"11111111-1111-8111-8111-111111111111"}"#).is_err());
        assert!(parse::<Edit>(br#"{"version":1,"replace":true}"#).is_err());
        assert!(parse::<Commit>(br#"{"draft":{},"page_order":[]}"#).is_err());
        for key in ["externalUrl"] {
            let mut value=json!({"volume":null,"chapter":null,"title":null,"translatedLanguage":"en"});
            value[key]=json!(if key=="externalUrl"{"https://fixture.invalid/chapter"}else{"2030-01-01T00:00:00"});
            assert!(draft(serde_json::from_value(value).unwrap(),true).is_err());
        }
    }
    #[test]
    fn publish_at_is_optional_nonnullable_exact_calendar_utc() {
        let base=json!({"volume":null,"chapter":null,"title":null,"translatedLanguage":"en"});
        assert!(serde_json::from_value::<ChapterDraft>(base.clone()).unwrap().publish_at.is_none());
        for value in [json!(null),json!(false),json!(0),json!("2026-02-29T12:00:00"),json!("2028-02-29T12:00:00Z"),json!("2028-02-29T12:00:00+00:00"),json!("2028-02-29T12:00:00.000")] {
            let mut input=base.clone();input["publishAt"]=value;assert!(serde_json::from_value::<ChapterDraft>(input).is_err());
        }
        let mut input=base;input["publishAt"]=json!("2028-02-29T12:00:00");
        assert!(draft(serde_json::from_value(input).unwrap(),true).is_ok());
        assert!(serde_json::from_str::<ChapterDraft>(r#"{"volume":null,"chapter":null,"title":null,"translatedLanguage":"en","publishAt":"2028-02-29T12:00:00","publishAt":"2028-02-29T12:00:00"}"#).is_err());
    }
    /// Mapper/error-propagation coverage, not an HTTP missing-alias result.
    /// Normal alias identity triggers remain enabled throughout this fixture.
    #[tokio::test]
    #[ignore="Requires separately audited owned disposable PostgreSQL runner"]
    async fn unavailable_chapter_presentation_does_not_hide_database_errors() {
        let dsn=std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use the disposable runner");
        let pg:tokio_postgres::Config=dsn.parse().unwrap();
        assert_eq!(pg.get_dbname(),Some("weebdex_platform_test"));
        assert!(!pg.get_hosts().is_empty());
        assert!(pg.get_hosts().iter().all(|h|matches!(h,tokio_postgres::config::Host::Tcp(h) if h=="127.0.0.1")));
        let state=AppState::connect(crate::Config{
            database_plaintext_local:true,database_connections:6,..crate::Config::default()
        },dsn.clone()).await.unwrap();
        let mut repository=manga_node::db::Repository::connect(manga_node::db::DbConfig{
            dsn,connections:1,acquisition_timeout:Duration::from_secs(2),cache_weight:1048576,plaintext_local:true,
        }).await.unwrap();
        repository.pool=state.db.pool().clone();
        repository.migrate().await.unwrap();
        state.db.migrate(crate::domains::MIGRATIONS).await.unwrap();
        let mut client=state.db.client().await.unwrap();
        let tx=client.transaction().await.unwrap();
        let synthetic=json!({"id":"MapperOnlyMissingChapterAlias","language":"en"});
        assert!(optional_chapter_resource(&tx,&synthetic).await.unwrap().is_none());
        assert_eq!(chapter_resource(&tx,&synthetic).await.unwrap_err().status,StatusCode::SERVICE_UNAVAILABLE);
        // A genuine SQL failure aborts this transaction. The async alias load
        // must propagate it rather than report a successful unavailable receipt.
        assert!(tx.simple_query("SELECT 1/0").await.is_err());
        assert_eq!(optional_chapter_resource(&tx,&synthetic).await.unwrap_err().status,StatusCode::SERVICE_UNAVAILABLE);
        tx.rollback().await.unwrap();
    }
}
