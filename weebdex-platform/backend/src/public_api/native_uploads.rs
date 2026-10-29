//! MangaDex upload transport over the authoritative native upload core.
//! UUIDs select native rows, never files, principals, signers or permissions.
use super::uploads::{chapter_resource, file_resource, resolve, session_resource};
use super::{PublicApiError, PublicApiResult, PublicUuid, ResourceKind};
use crate::domains::catalog::entity_mutations::CatalogMutationActor;
use crate::{
    AppState,
    domains::{
        catalog::CatalogRepository,
        uploads::{sessions as core, storage},
    },
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, OriginalUri, Path, Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{collections::BTreeSet, time::Duration};
use tokio_postgres::Transaction;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/account/upload", get(get_current))
        .route("/account/upload/begin", post(begin))
        .route("/account/upload/begin/{p0}", post(begin_edit))
        .route(
            "/account/upload/check-approval-required",
            post(check_approval),
        )
        .route(
            "/account/upload/{p0}",
            post(upload)
                .delete(cancel)
                .layer(DefaultBodyLimit::max(super::uploads_body::BODY_LIMIT)),
        )
        .route("/account/upload/{p0}/commit", post(commit))
        .route("/account/upload/{p0}/batch", delete(delete_batch))
        .route("/account/upload/{p0}/{p1}", delete(delete_file))
}

pub(super) struct NativeRequest {
    headers: HeaderMap,
    account: PublicUuid,
}
pub(super) fn no_query(uri: &axum::http::Uri) -> PublicApiResult<()> {
    account_query(uri).map(|_| ())
}
fn account_query(uri: &axum::http::Uri) -> PublicApiResult<PublicUuid> {
    let q = super::query::PublicQuery::parse_unchecked(uri.query())?;
    q.require_allowed(&["expectedAccountId"])?;
    PublicUuid::parse(
        q.first("expectedAccountId")
            .ok_or_else(|| PublicApiError::bad_request("expectedAccountId query is required"))?,
    )
    .map_err(Into::into)
}
pub(super) async fn authority(
    state: &AppState,
    tx: &Transaction<'_>,
    cap: &NativeRequest,
    write: bool,
) -> PublicApiResult<super::native_contributions::NativeActor> {
    let actor = super::native_contributions::actor(state, &cap.headers, tx, write).await?;
    super::native_account_profile_settings::expected(tx, &actor.0, &cap.account).await?;
    Ok(actor)
}
pub(super) async fn authorize_headers(
    state: &AppState,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    write: bool,
) -> PublicApiResult<NativeRequest> {
    let cap = NativeRequest {
        headers: headers.clone(),
        account: account_query(uri)?,
    };
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    authority(state, &tx, &cap, write).await?;
    tx.commit().await?;
    Ok(cap)
}
pub(super) async fn authorize(
    state: &AppState,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    request: Request,
    maximum: usize,
    empty: bool,
) -> PublicApiResult<(NativeRequest, Vec<u8>)> {
    let write = request.method() != axum::http::Method::GET;
    let cap = authorize_headers(state, uri, headers, write).await?;
    let bytes =
        super::native_account_profile_settings::body(request, headers, !empty, maximum).await?;
    if empty && !bytes.is_empty() {
        return Err(PublicApiError::bad_request(
            "This operation accepts no body",
        ));
    }
    Ok((cap, bytes))
}
pub(super) async fn authorize_account_query(
    state: &AppState,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    request: Request,
) -> PublicApiResult<(NativeRequest, Option<Value>)> {
    let (cap, _) = authorize(state, uri, headers, request, 1, true).await?;
    let expected = Some(json!(cap.account));
    Ok((cap, expected))
}
// Missing remains compatible; an explicitly present null is not an account UUID.
pub(super) fn deserialize_optional_account<'de, D: serde::Deserializer<'de>>(
    input: D,
) -> Result<Option<PublicUuid>, D::Error> {
    PublicUuid::deserialize(input).map(Some)
}
pub(super) fn parse<T: DeserializeOwned>(body: &[u8]) -> PublicApiResult<T> {
    super::native_json::parse(body)?;
    serde_json::from_slice(body)
        .map_err(|_| PublicApiError::bad_request("Invalid upload request body"))
}
async fn get_current(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    let (cap, expected) = authorize_account_query(&state, &uri, &headers, request).await?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let actor = authority(&state, &tx, &cap, false).await?;
    if let Some(expected) = expected {
        let id = PublicUuid::parse(expected.as_str().ok_or_else(PublicApiError::unavailable)?)?;
        if resolve(&tx, ResourceKind::User, &id).await? != actor.user_id() {
            return Err(PublicApiError::conflict(
                "Upload account changed; reload the intended account",
            ));
        }
    }
    let row=tx.query_opt("SELECT id FROM platform.upload_sessions s WHERE owner_id=$1 AND ((state='open' AND (expires_at>clock_timestamp() OR EXISTS(SELECT 1 FROM platform.protocol_publication_commands p WHERE p.upload_session_id=s.id AND p.user_id=s.owner_id AND p.state IN ('prepared','source_accepted')))) OR (state='committed' AND committed_result->>'publication_status' IN ('accepted_pending_projection','pending_projection') AND EXISTS(SELECT 1 FROM platform.catalog_chapters c WHERE c.id=s.committed_chapter_id AND c.deleted_at IS NULL))) ORDER BY (state='open') DESC,updated_at DESC,id LIMIT 1", &[&actor.user_id()]).await?
        .ok_or_else(PublicApiError::not_found)?;
    let native = core::session_json(&tx, &row.get::<_, String>(0)).await?;
    let mut result = session_resource(&tx, &native).await?;
    super::native_account_profile_settings::provenance(&mut result, cap.account.as_str());
    tx.commit().await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Begin {
    manga: PublicUuid,
    groups: Vec<PublicUuid>,
    #[serde(
        default,
        rename = "expectedAccountId",
        deserialize_with = "deserialize_optional_account"
    )]
    expected_account_id: Option<PublicUuid>,
}
async fn begin(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    let (cap, body) = authorize(&state, &uri, &headers, request, 4 * 1024, false).await?;
    let payload: Begin = parse(&body)?;
    if payload.groups.len() > 10
        || payload.groups.iter().collect::<BTreeSet<_>>().len() != payload.groups.len()
    {
        return Err(PublicApiError::bad_request(
            "Select at most ten distinct groups",
        ));
    }
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let actor = authority(&state, &tx, &cap, true).await?;
    let expected = payload.expected_account_id.as_ref().map(|id| json!(id));
    if let Some(expected) = expected {
        if expected.as_str() != Some(cap.account.as_str()) {
            return Err(PublicApiError::conflict("Captured upload account changed"));
        }
    }
    let manga = resolve(&tx, ResourceKind::Manga, &payload.manga).await?;
    let mut groups = Vec::new();
    for group in &payload.groups {
        groups.push(resolve(&tx, ResourceKind::Group, group).await?);
    }
    let native = core::begin_in(&tx, &actor, &core::Begin { manga, groups }).await?;
    let mut result = session_resource(&tx, &native).await?;
    super::native_account_profile_settings::provenance(&mut result, cap.account.as_str());
    tx.commit().await?;
    core::cleanup(&state).await;
    Ok(Json(result))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    version: i32,
    #[serde(default)]
    replace: bool,
    #[serde(
        default,
        rename = "expectedAccountId",
        deserialize_with = "deserialize_optional_account"
    )]
    expected_account_id: Option<PublicUuid>,
}
async fn begin_edit(
    State(state): State<AppState>,
    Path(chapter): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    let chapter = PublicUuid::parse(&chapter)?;
    let (cap, body) = authorize(&state, &uri, &headers, request, 1024, false).await?;
    let payload: Edit = parse(&body)?;
    if payload.version < 1 {
        return Err(PublicApiError::bad_request("Invalid chapter version"));
    }
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let actor = authority(&state, &tx, &cap, true).await?;
    let expected = payload.expected_account_id.as_ref().map(|id| json!(id));
    if let Some(expected) = expected {
        if expected.as_str() != Some(cap.account.as_str()) {
            return Err(PublicApiError::conflict("Captured upload account changed"));
        }
    }
    let public_chapter = chapter.clone();
    let chapter = resolve(&tx, ResourceKind::Chapter, &chapter).await?;
    let native = core::begin_edit_in(
        &tx,
        &actor,
        &chapter,
        &core::Edit {
            version: payload.version,
            replace: payload.replace,
        },
    )
    .await?;
    let mut result = session_resource(&tx, &native).await?;
    super::native_account_profile_settings::provenance(&mut result, cap.account.as_str());
    // Bind successful admission to the signed target, version and chosen mode.
    result["attributes"]["edit"] =
        json!({"chapterId":public_chapter,"version":payload.version,"replace":payload.replace});
    tx.commit().await?;
    core::cleanup(&state).await;
    Ok(Json(result))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Approval {
    manga: PublicUuid,
    locale: String,
}
async fn check_approval(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    let (cap, body) = authorize(&state, &uri, &headers, request, 4 * 1024, false).await?;
    let payload: Approval = parse(&body)?;
    crate::domains::catalog::validation::public_language(&payload.locale)?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let actor = authority(&state, &tx, &cap, true).await?;
    let manga = resolve(&tx, ResourceKind::Manga, &payload.manga).await?;
    CatalogRepository::ensure_upload_target(&tx, &manga, &actor, &[]).await?;
    // This instance's explicit chapter.create grant permits immediate upload.
    // No MangaDex moderation queue or global approval is invented here.
    tx.commit().await?;
    Ok(Json(
        json!({"result":"ok","requiresApproval":false,"nostr":{"scope":"instance-account","authentication":"native-session","accountId":cap.account}}),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Commit {
    chapter_draft: ChapterDraft,
    page_order: Vec<PublicUuid>,
    terms_accepted: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ChapterDraft {
    #[serde(deserialize_with = "required_nullable")]
    volume: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    chapter: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    title: Option<String>,
    translated_language: String,
    #[serde(default)]
    external_url: Option<String>,
    #[serde(default, deserialize_with = "optional_publish_at")]
    publish_at: Option<String>,
}
fn optional_publish_at<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    let value = String::deserialize(deserializer)?;
    if !super::publish_at::valid_publish_at(&value) {
        return Err(serde::de::Error::custom(
            "publishAt must be a real YYYY-MM-DDTHH:MM:SS UTC timestamp",
        ));
    }
    Ok(Some(value))
}
fn required_nullable<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}
fn draft(payload: ChapterDraft, terms: bool) -> PublicApiResult<core::Draft> {
    if !terms {
        return Err(PublicApiError::bad_request(
            "termsAccepted must be true for an upload commit",
        ));
    }
    if payload.external_url.is_some() {
        return Err(PublicApiError::bad_request(
            "External chapters are unavailable in this instance's image upload module",
        ));
    }
    if payload
        .title
        .as_ref()
        .is_some_and(|v| v.chars().count() > 255)
    {
        return Err(PublicApiError::bad_request(
            "Chapter title exceeds 255 characters",
        ));
    }
    crate::domains::catalog::validation::public_language(&payload.translated_language)?;
    Ok(core::Draft {
        volume: payload.volume,
        chapter: payload.chapter,
        title: payload.title,
        language: payload.translated_language,
    })
}
async fn commit(
    State(state): State<AppState>,
    Path(session): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Response> {
    let session = PublicUuid::parse(&session)?;
    // 500 public UUIDs alone need about 20 KiB, before the chapter draft.
    let (cap, body) = authorize(&state, &uri, &headers, request, 32 * 1024, false).await?;
    let payload: Commit = parse(&body)?;
    if payload.page_order.is_empty() || payload.page_order.len() > 500 {
        return Err(PublicApiError::bad_request(
            "pageOrder must contain one to 500 files",
        ));
    }
    let publish_at = payload.chapter_draft.publish_at.clone();
    let draft = draft(payload.chapter_draft, payload.terms_accepted)?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let actor = authority(&state, &tx, &cap, true).await?;
    let session = resolve(&tx, ResourceKind::UploadSession, &session).await?;
    let mut page_order = Vec::new();
    for file in payload.page_order {
        page_order.push(resolve(&tx, ResourceKind::UploadFile, &file).await?);
    }
    let payload = core::Commit { draft, page_order };
    if let Some(mut receipt) = super::scheduled_uploads::admit_native(
        &state,
        &tx,
        &actor,
        &session,
        publish_at.as_deref(),
        &payload,
    )
    .await?
    {
        super::native_account_profile_settings::provenance(&mut receipt, cap.account.as_str());
        tx.commit().await?;
        return Ok((StatusCode::ACCEPTED, Json(receipt)).into_response());
    }
    let native = core::commit_in(&state, &tx, &actor, &session, &payload).await?;
    let mut result = chapter_resource(&tx, &native).await?;
    super::native_account_profile_settings::provenance(&mut result, cap.account.as_str());
    tx.commit().await?;
    core::cleanup(&state).await;
    Ok(Json(result).into_response())
}
async fn cancel(
    State(state): State<AppState>,
    Path(session): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    let session = PublicUuid::parse(&session)?;
    let (cap, expected) = authorize_account_query(&state, &uri, &headers, request).await?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let actor = authority(&state, &tx, &cap, true).await?;
    // Optional for existing callers, mandatory in the schedule owner UI. Compare
    // the signed account before resolving or accessing any private session.
    if let Some(expected) = expected {
        if expected.as_str() != Some(cap.account.as_str()) {
            return Err(PublicApiError::conflict("Captured upload account changed"));
        }
    }
    let session = resolve(&tx, ResourceKind::UploadSession, &session).await?;
    if !super::scheduled_uploads::cancel_in(&tx, &actor, &session).await? {
        core::cancel_in(&tx, &actor, &session).await?;
    }
    tx.commit().await?;
    core::cleanup(&state).await;
    Ok(Json(
        json!({"result":"ok","nostr":{"scope":"instance-account","authentication":"native-session","accountId":cap.account}}),
    ))
}
async fn delete_file(
    State(state): State<AppState>,
    Path((session, file)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    let session = PublicUuid::parse(&session)?;
    let file = PublicUuid::parse(&file)?;
    let (cap, _) = authorize(&state, &uri, &headers, request, 1, true).await?;
    remove(&state, &cap, &session, &[file]).await
}
async fn delete_batch(
    State(state): State<AppState>,
    Path(session): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    let session = PublicUuid::parse(&session)?;
    let (cap, body) = authorize(&state, &uri, &headers, request, 20 * 1024, false).await?;
    let files: Vec<PublicUuid> = parse(&body)?;
    if files.is_empty()
        || files.len() > 500
        || files.iter().collect::<BTreeSet<_>>().len() != files.len()
    {
        return Err(PublicApiError::bad_request(
            "Select one to 500 distinct files",
        ));
    }
    remove(&state, &cap, &session, &files).await
}
async fn remove(
    state: &AppState,
    cap: &NativeRequest,
    session: &PublicUuid,
    files: &[PublicUuid],
) -> PublicApiResult<Json<Value>> {
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let actor = authority(state, &tx, cap, true).await?;
    let session = resolve(&tx, ResourceKind::UploadSession, session).await?;
    let mut native = Vec::new();
    for file in files {
        native.push(resolve(&tx, ResourceKind::UploadFile, file).await?);
    }
    core::remove_files_in(&tx, &actor, &session, &native).await?;
    tx.commit().await?;
    core::cleanup(state).await;
    Ok(Json(
        json!({"result":"ok","nostr":{"scope":"instance-account","authentication":"native-session","accountId":cap.account}}),
    ))
}

async fn upload(
    State(state): State<AppState>,
    Path(session): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    let session = PublicUuid::parse(&session)?;
    no_query(&uri)?;
    let (cap, mut multipart, _permit) =
        super::uploads_body::receive_native(&state, &uri, &headers, request).await?;
    let native_session;
    {
        let mut client = state.db.client().await?;
        let tx = client.transaction().await?;
        let actor = authority(&state, &tx, &cap, true).await?;
        actor.require_permission("self.chapter.update")?;
        native_session = resolve(&tx, ResourceKind::UploadSession, &session).await?;
        core::open_session(&tx, &native_session, actor.user_id()).await?;
        core::require_mutable_session(&tx, &native_session).await?;
        tx.commit().await?;
    }
    let mut data = Vec::new();
    let mut errors = Vec::new();
    let mut fields = 0usize;
    while let Some(field) = tokio::time::timeout(Duration::from_secs(30), multipart.next_field())
        .await
        .map_err(|_| PublicApiError::bad_request("Multipart upload timed out"))?
        .map_err(|_| PublicApiError::bad_request("Invalid multipart payload"))?
    {
        fields += 1;
        if fields > 5 || field.name() != Some("file") {
            return Err(PublicApiError::bad_request(
                "Upload one to five multipart fields named file",
            ));
        }
        let original = field
            .file_name()
            .unwrap_or("image")
            .chars()
            .take(255)
            .collect::<String>();
        let received = storage::receive(&state.config.uploads_dir, field, false).await;
        let (mut image, mut variants) = match received {
            Ok(received) => received,
            Err(error) => {
                errors.push(file_error(error.into(), &original));
                continue;
            }
        };
        let attached = async {
            let mut client = state.db.client().await?;
            let tx = client.transaction().await?;
            // Intake/decoding may take time. Recheck the original account/service,
            // explicit binding, current native grants, ownership and immutable
            // pending publication fence in this same attachment transaction.
            let actor = authority(&state, &tx, &cap, true).await?;
            let native = core::attach_in(
                &state,
                &tx,
                &native_session,
                &actor,
                &original,
                &mut image,
                &mut variants,
            )
            .await?;
            let result = file_resource(&tx, &native).await?;
            tx.commit().await?;
            Ok::<_, PublicApiError>(result)
        }
        .await;
        match attached {
            Ok(file) => {
                image.confirm_persisted().await;
                for (_, variant) in &mut variants {
                    variant.confirm_persisted().await;
                }
                data.push(file);
            }
            Err(error) => {
                // Authorization/ownership failures are not per-image validation
                // errors and may never become a successful upload response.
                if error.status != StatusCode::BAD_REQUEST {
                    return Err(error);
                }
                errors.push(file_error(error, &original));
            }
        }
    }
    if fields == 0 {
        return Err(PublicApiError::bad_request(
            "At least one image is required",
        ));
    }
    Ok(Json(
        json!({"result":if errors.is_empty(){"ok"}else{"error"},"errors":errors,"data":data,"nostr":{"scope":"instance-account","authentication":"native-session","accountId":cap.account}}),
    ))
}
fn file_error(error: PublicApiError, original: &str) -> Value {
    // No native identifiers, storage paths or SQL details cross this boundary.
    json!({"id":random_error_id(),"status":error.status.as_u16(),"title":error.title,"detail":error.detail,"context":{"originalFileName":original}})
}
fn random_error_id() -> String {
    let mut bytes = rand::random::<[u8; 16]>();
    bytes[6] = (bytes[6] & 15) | 64;
    bytes[8] = (bytes[8] & 63) | 128;
    let value = hex::encode(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &value[..8],
        &value[8..12],
        &value[12..16],
        &value[16..20],
        &value[20..]
    )
}
