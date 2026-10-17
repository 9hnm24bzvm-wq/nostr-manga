//! Instance-local recovery extensions. These fields are deliberately separate
//! from the official public upload transport and never confer cookie authority.
use super::{
    PublicApiError, PublicApiResult, PublicUuid, ResourceKind,
    auth::{bound_account_read, bound_account_write},
    uploads::{authorize, file_resource, no_query, optional_chapter_resource, parse, preferred, resolve},
};
use crate::{
    AppState,
    domains::uploads::{media, sessions as core, storage},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, OriginalUri, Path, Request, State, multipart::Field},
    http::{HeaderMap, HeaderValue, Method, header},
    middleware,
    response::Response,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeSet, time::Duration};
use tokio_postgres::Transaction;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/nostr/account-context", get(account_context))
        .route("/nostr/uploads/{session}", get(snapshot).patch(stage))
        .route("/nostr/uploads/{session}/files/{file}/preview", get(preview))
        .route("/nostr/uploads/{session}/files/{file}/replace", post(replace)
            .layer(DefaultBodyLimit::max(40 * 1024 * 1024)))
        .layer(middleware::map_response(private_response))
}

pub(super) async fn private_response(mut response: Response) -> Response {
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    response.headers_mut().append(header::VARY, HeaderValue::from_static("Authorization"));
    response
}

fn require_get(request: &Request) -> PublicApiResult<()> {
    if request.method() != Method::GET {
        return Err(PublicApiError::new(
            axum::http::StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed", "Use a signed GET request",
        ));
    }
    Ok(())
}

async fn account_context(
    State(state): State<AppState>, OriginalUri(uri): OriginalUri,
    headers: HeaderMap, request: Request,
) -> PublicApiResult<Json<Value>> {
    require_get(&request)?;
    let (cap, _) = authorize(&state, &uri, &headers, request, 1, true).await?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let actor = bound_account_read(&tx, &cap).await?;
    let user = preferred(&tx, ResourceKind::User, actor.user_id()).await?;
    let user_version: i32 = tx.query_one(
        "SELECT version FROM platform.users WHERE id=$1", &[&actor.user_id()],
    ).await?.get(0);
    let namespace: String = tx.query_one(
        "SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton", &[],
    ).await?.get(0);
    let namespace = PublicUuid::parse(&namespace).map_err(|_| PublicApiError::unavailable())?;
    let publication = state.config.protocol_publication.as_ref();
    let result = json!({
        "result":"ok",
        "user":{"id":user,"version":user_version,"username":actor.username(),"roles":actor.roles(),"permissions":actor.permissions()},
        "nostr":{"pubkey":hex::encode(cap.principal()),"serviceKey":hex::encode(cap.service()),
            "authentication":"NIP-98","requestScoped":true,"nativeSessionAuthority":false,
            "catalogueScope":"instance-local","catalogNamespace":namespace},
        "upload":{
            "canBegin":actor.has_perm("chapter.create"),"canManage":actor.has_perm("self.chapter.update"),
            "conditional":true,
            "publication":{"scope":"original-protocol","configured":publication.is_some(),
                "operatorPublisher":publication.map(|p|p.expected_publisher.as_str()),
                "requiresOperatorOrAuthorizedGroup":true,
                "limits":{"maxUniqueOriginalBytes":256*1024*1024,
                    "maxBlobBytes":publication.and_then(|p|p.storage.get("max_blob_bytes")).and_then(Value::as_u64),
                    "mimeTypes":["image/jpeg","image/png","image/webp"]}},
            "limits":{"maxFiles":storage::MAX_FILES,"maxFilesPerRequest":5,
                "maxFileBytes":storage::MAX_FILE_BYTES,"maxSessionBytes":storage::MAX_SESSION_BYTES,
                "maxPixels":storage::MAX_PIXELS,"maxDimension":storage::MAX_DIMENSION}
        }
    });
    tx.commit().await?;
    Ok(Json(result))
}

async fn snapshot(
    State(state): State<AppState>, Path(session): Path<String>, OriginalUri(uri): OriginalUri,
    headers: HeaderMap, request: Request,
) -> PublicApiResult<Json<Value>> {
    require_get(&request)?;
    let session = PublicUuid::parse(&session)?;
    let (cap, _) = authorize(&state, &uri, &headers, request, 1, true).await?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let actor = bound_account_read(&tx, &cap).await?;
    let native_id = resolve(&tx, ResourceKind::UploadSession, &session).await?;
    let native = core::owner_snapshot_in(&state, &tx, &native_id, actor.user_id(),
        actor.has_perm("self.chapter.update")).await?;
    let result = snapshot_resource(&tx, &native).await?;
    tx.commit().await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all="camelCase")]
struct Stage {
    version: i32,
    draft: Value,
    page_order: Vec<PublicUuid>,
}
async fn stage(
    State(state): State<AppState>, Path(session): Path<String>, OriginalUri(uri): OriginalUri,
    headers: HeaderMap, request: Request,
) -> PublicApiResult<Json<Value>> {
    let session = PublicUuid::parse(&session)?;
    let (cap, body) = authorize(&state, &uri, &headers, request, 32*1024, false).await?;
    let payload: Stage = parse(&body)?;
    if payload.page_order.len() > storage::MAX_FILES
        || payload.page_order.iter().collect::<BTreeSet<_>>().len() != payload.page_order.len()
    {
        return Err(PublicApiError::bad_request("Saved order must contain at most 500 distinct files"));
    }
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let actor = bound_account_write(&tx, &cap).await?;
    actor.require_permission("self.chapter.update")?;
    let native_id = resolve(&tx, ResourceKind::UploadSession, &session).await?;
    let mut page_order = Vec::with_capacity(payload.page_order.len());
    for file in payload.page_order {
        page_order.push(resolve(&tx, ResourceKind::UploadFile, &file).await?);
    }
    core::save_resume_in(&tx, &actor, &native_id, &core::ResumePatch{
        version:payload.version, draft:payload.draft, page_order,
    }).await?;
    let native = core::owner_snapshot_in(&state, &tx, &native_id, actor.user_id(), true).await?;
    let result = snapshot_resource(&tx, &native).await?;
    tx.commit().await?;
    Ok(Json(result))
}

/// Both the concurrency fence and the image are part of the exact raw NIP-98
/// multipart payload. No cookie or unsigned version header can confer authority.
async fn replace(
    State(state): State<AppState>, Path((session, file)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri, headers: HeaderMap, request: Request,
) -> PublicApiResult<Json<Value>> {
    let session = PublicUuid::parse(&session)?;
    let file = PublicUuid::parse(&file)?;
    no_query(&uri)?;
    if headers.contains_key("x-weebdex-upload-version") {
        return Err(PublicApiError::bad_request("Use the signed expectedVersion multipart field"));
    }
    let (cap, mut multipart, _permit) = super::uploads_body::receive(&state, &uri, &headers, request).await?;
    // Check current ownership before spending decoder resources. This actor is
    // deliberately discarded; the mutation below constructs another one.
    {
        let mut client = state.db.client().await?;
        let tx = client.transaction().await?;
        let actor = bound_account_write(&tx, &cap).await?;
        actor.require_permission("self.chapter.update")?;
        let native_session = resolve(&tx, ResourceKind::UploadSession, &session).await?;
        let native_file = resolve(&tx, ResourceKind::UploadFile, &file).await?;
        core::open_session(&tx, &native_session, actor.user_id()).await?;
        core::require_mutable_session(&tx, &native_session).await?;
        if tx.query_opt("SELECT id FROM platform.upload_files WHERE id=$1 AND session_id=$2",
            &[&native_file, &native_session]).await?.is_none()
        {
            return Err(PublicApiError::not_found());
        }
        tx.commit().await?;
    }
    let expected = expected_version(next_replacement_field(&mut multipart).await?
        .ok_or_else(||PublicApiError::bad_request("expectedVersion must be the first multipart field"))?).await?;
    let field = next_replacement_field(&mut multipart).await?
        .ok_or_else(||PublicApiError::bad_request("One replacement image is required"))?;
    if field.name() != Some("file") || field.file_name().is_none_or(str::is_empty) {
        return Err(PublicApiError::bad_request("The second multipart field must be a file named file"));
    }
    let original = field.file_name().unwrap().chars().take(255).collect::<String>();
    let (mut image, mut variants) = storage::receive(&state.config.uploads_dir, field, false).await?;
    if next_replacement_field(&mut multipart).await?.is_some() {
        return Err(PublicApiError::bad_request("Exactly expectedVersion and one file are required"));
    }
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    // Raw receipt and decoding can outlive grants, binding or admission changes.
    // Resolve the exact public selectors again and retain all authority locks in
    // this same atomic replacement/version/derivative transaction.
    let actor = bound_account_write(&tx, &cap).await?;
    actor.require_permission("self.chapter.update")?;
    let native_session = resolve(&tx, ResourceKind::UploadSession, &session).await?;
    let native_file = resolve(&tx, ResourceKind::UploadFile, &file).await?;
    let replaced = core::replace_file_in(&state, &tx, &actor, &native_session, &native_file,
        expected, core::ReplacementVersionPolicy::Exact, &original, &mut image, &mut variants).await?;
    let result = json!({"result":"ok", "file":file_resource(&tx, &replaced.file).await?,
        "sessionVersion":replaced.file["version"]});
    tx.commit().await?;
    if replaced.changed {
        image.confirm_persisted().await;
        for (_, derivative) in &mut variants { derivative.confirm_persisted().await; }
    }
    Ok(Json(result))
}

async fn next_replacement_field(multipart: &mut Multipart) -> PublicApiResult<Option<Field<'_>>> {
    tokio::time::timeout(Duration::from_secs(30), multipart.next_field()).await
        .map_err(|_|PublicApiError::bad_request("Multipart upload timed out"))?
        .map_err(|_|PublicApiError::bad_request("Invalid multipart payload"))
}
pub(super) async fn expected_version(mut field: Field<'_>) -> PublicApiResult<i32> {
    if field.name() != Some("expectedVersion") || field.file_name().is_some() || field.content_type().is_some() {
        return Err(PublicApiError::bad_request("expectedVersion must be the first plain multipart field"));
    }
    // A signed i32 needs at most ten ASCII bytes. Never allocate an arbitrary
    // text part from the much larger image body budget.
    let mut bytes = Vec::with_capacity(10);
    while let Some(chunk) = tokio::time::timeout(Duration::from_secs(30), field.chunk()).await
        .map_err(|_|PublicApiError::bad_request("Multipart upload timed out"))?
        .map_err(|_|PublicApiError::bad_request("Invalid multipart payload"))?
    {
        if bytes.len() + chunk.len() > 10 {
            return Err(PublicApiError::bad_request("Invalid expectedVersion"));
        }
        bytes.extend_from_slice(&chunk);
    }
    canonical_expected_version(&bytes)
}
fn canonical_expected_version(bytes: &[u8]) -> PublicApiResult<i32> {
    let expected = std::str::from_utf8(bytes).ok()
        .filter(|v|!v.is_empty() && !v.starts_with('0') && v.bytes().all(|b|b.is_ascii_digit()))
        .and_then(|v|v.parse::<i32>().ok()).filter(|v|*v > 0)
        .ok_or_else(||PublicApiError::bad_request("Invalid expectedVersion"))?;
    Ok(expected)
}

async fn snapshot_resource(tx: &Transaction<'_>, native: &Value) -> PublicApiResult<Value> {
    let native_id = native["id"].as_str().ok_or_else(PublicApiError::unavailable)?;
    let session = preferred(tx, ResourceKind::UploadSession, native_id).await?;
    let manga = preferred(tx, ResourceKind::Manga,
        native["manga_id"].as_str().ok_or_else(PublicApiError::unavailable)?).await?;
    let committed = native["is_committed"] == true;
    let chapter = match native["chapter_id"].as_str() {
        Some(id) if committed => {
            let alias = tx.query_opt(
                "SELECT public_id::text FROM platform.public_resource_ids WHERE kind='chapter' AND native_id=$1 AND preferred",
                &[&id],
            ).await?;
            alias.map(|row| PublicUuid::parse(row.get::<_, &str>(0))
                .map_err(|_|PublicApiError::unavailable())).transpose()?
        },
        Some(id) => Some(preferred(tx, ResourceKind::Chapter, id).await?),
        None => None,
    };
    let groups: Vec<String> = tx.query_one(
        "SELECT groups FROM platform.upload_sessions WHERE id=$1", &[&native_id],
    ).await?.get(0);
    let mut group_ids = Vec::with_capacity(groups.len());
    for group in groups { group_ids.push(preferred(tx, ResourceKind::Group, &group).await?); }
    let rows = tx.query(
        "SELECT f.id,f.original_name,o.digest,o.bytes,o.mime,o.width,o.height FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id WHERE f.session_id=$1 ORDER BY f.position,f.id",
        &[&native_id],
    ).await?;
    let mut files = Vec::with_capacity(rows.len());
    let mut page_order = Vec::with_capacity(rows.len());
    let order = native["page_order"].as_array().ok_or_else(PublicApiError::unavailable)?;
    for native_file in order {
        let native_file = native_file.as_str().ok_or_else(PublicApiError::unavailable)?;
        let row = rows.iter().find(|row|row.get::<_, &str>(0) == native_file)
            .ok_or_else(PublicApiError::unavailable)?;
        let file = preferred(tx, ResourceKind::UploadFile, native_file).await?;
        files.push(json!({"id":file,"originalFileName":row.get::<_,String>(1),
            "fileHash":row.get::<_,String>(2),"fileSize":row.get::<_,i64>(3),"mimeType":row.get::<_,String>(4),
            "dimensions":{"width":row.get::<_,i32>(5),"height":row.get::<_,i32>(6)},
            "previewUrl":format!("/api/v1/nostr/uploads/{session}/files/{file}/preview")}));
        page_order.push(file);
    }
    let draft = native["draft"].as_object().map(|draft| {
        Value::Object(["title","volume","chapter","language"].iter()
            .filter_map(|key|draft.get(*key).map(|value|((*key).to_owned(),value.clone()))).collect())
    });
    let receipt = if chapter.is_some() && native["receipt"].is_object() {
        optional_chapter_resource(tx, &native["receipt"]).await?
    } else { None };
    let receipt_status = if !committed { "none" }
        else if receipt.is_some() { "available" } else { "unavailable" };
    Ok(json!({"result":"ok","session":{
        "id":session,"mangaId":manga,"chapterId":chapter,"groupIds":group_ids,
        "createdAt":native["created_at"],"updatedAt":native["updated_at"],
        "isProcessed":native["is_processed"],"isDeleted":native["is_deleted"],
        "version":native["version"],"draft":draft,"pageOrder":page_order,"files":files,
        "canMutate":native["can_mutate"],"canCancel":native["can_cancel"],"canRetryCommit":native["can_retry_commit"],
        "isCommitted":native["is_committed"],"isExpired":native["is_expired"],"expiresAt":native["expires_at"],
        "state":native["state"],"publicationStatus":native["publication_status"],"targetStatus":native["target_status"],
        "receiptStatus":receipt_status,"receipt":receipt
    }}))
}

async fn preview(
    State(state): State<AppState>, Path((session, file)): Path<(String,String)>,
    OriginalUri(uri): OriginalUri, headers: HeaderMap, request: Request,
) -> PublicApiResult<Response> {
    require_get(&request)?;
    let session = PublicUuid::parse(&session)?;
    let file = PublicUuid::parse(&file)?;
    let (cap, _) = authorize(&state, &uri, &headers, request, 1, true).await?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let actor = bound_account_read(&tx, &cap).await?;
    let session = resolve(&tx, ResourceKind::UploadSession, &session).await?;
    let file = resolve(&tx, ResourceKind::UploadFile, &file).await?;
    // Session and file locks fence replacement/cancel/expiry until the immutable
    // object has been opened. Stream permits remain attached to the response.
    let row = tx.query_opt(
        "SELECT o.storage_key,o.digest,o.mime,o.bytes FROM platform.upload_sessions s JOIN platform.upload_files f ON f.session_id=s.id JOIN platform.media_objects o ON o.id=f.object_id WHERE s.id=$1 AND s.owner_id=$2 AND f.id=$3 AND s.state='open' AND (s.expires_at>clock_timestamp() OR EXISTS(SELECT 1 FROM platform.protocol_publication_commands p WHERE p.upload_session_id=s.id AND p.user_id=s.owner_id AND p.state IN ('prepared','source_accepted'))) FOR SHARE OF s,f,o",
        &[&session, &actor.user_id(), &file],
    ).await?.ok_or_else(PublicApiError::not_found)?;
    let response = media::signed_upload_preview(&state, &row, &headers).await?;
    tx.commit().await?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, extract::FromRequest};

    #[test]
    fn expected_version_accepts_only_canonical_positive_i32() {
        for (raw, value) in [("1",1), ("19",19), ("2147483647",i32::MAX)] {
            assert_eq!(canonical_expected_version(raw.as_bytes()).unwrap(), value);
        }
        for raw in ["", "0", "01", "+1", "-1", " 1", "1 ", "1\n", "1\0", "1.0", "1e0", "2147483648", "١"] {
            assert!(canonical_expected_version(raw.as_bytes()).is_err(), "accepted {raw:?}");
        }
        assert!(canonical_expected_version(&[0xff]).is_err());
    }

    // Actual Axum multipart parsing, with no database/authentication substitute.
    #[tokio::test]
    async fn expected_version_is_a_bounded_plain_first_part() {
        for (disposition, extra, value, valid) in [
            ("name=\"expectedVersion\"", "", "9", true),
            ("name=\"expectedVersion\"; filename=\"version.txt\"", "", "9", false),
            ("name=\"expectedVersion\"", "Content-Type: text/plain\r\n", "9", false),
            ("name=\"version\"", "", "9", false),
            ("name=\"file\"; filename=\"page.png\"", "", "9", false),
            ("name=\"expectedVersion\"", "", "00000000001", false),
            ("name=\"expectedVersion\"", "", "2147483648", false),
        ] {
            let body = format!("--test-boundary\r\nContent-Disposition: form-data; {disposition}\r\n{extra}\r\n{value}\r\n--test-boundary--\r\n");
            let request = Request::builder().method("POST")
                .header(header::CONTENT_TYPE, "multipart/form-data; boundary=test-boundary")
                .body(Body::from(body)).unwrap();
            let mut multipart = Multipart::from_request(request, &()).await.unwrap();
            let result = expected_version(next_replacement_field(&mut multipart).await.unwrap().unwrap()).await;
            assert_eq!(result.is_ok(), valid, "{disposition} {extra} {value}");
            if valid { assert_eq!(result.unwrap(), 9); }
        }
    }
}
