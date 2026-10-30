//! Ordinary verified browser contributors, not source signers. These canonical
//! adapters use the native catalogue cores and preserve their role/owner gates.
use super::{
    PublicApiError, PublicApiResult, PublicUuid, ResourceKind, manga_writes,
    native_account_profile_settings as account,
    uploads::{preferred, resolve},
};
use crate::{
    AppState,
    domains::{
        auth::SessionPrincipal,
        catalog::{
            entity_mutations::{self, CatalogMutationActor},
            mutations as core,
        },
        uploads::storage,
    },
};
use axum::{
    Json, Router,
    extract::{OriginalUri, Path, Request, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
};
use serde_json::{Value, json};
use tokio_postgres::Transaction;

/// Public language/UUID presentation with genuine native session authority. No
/// invented user, protocol identity, binding, grant or private key is created.
pub(super) struct NativeActor(pub(super) SessionPrincipal);
impl entity_mutations::sealed::Sealed for NativeActor {}
impl CatalogMutationActor for NativeActor {
    fn native_publication_session(&self) -> Option<&SessionPrincipal> { Some(&self.0) }
    fn language_contract(&self) -> crate::domains::catalog::validation::LanguageContract {
        crate::domains::catalog::validation::LanguageContract::Public
    }
    fn user_id(&self) -> &str {
        &self.0.user_id
    }
    fn username(&self) -> &str {
        &self.0.username
    }
    fn is_moderator(&self) -> bool {
        self.0.is_moderator()
    }
    fn has_perm(&self, p: &str) -> bool {
        self.0.has_perm(p)
    }
    fn require_permission(&self, p: &str) -> crate::error::ApiResult<()> {
        self.0.require_permission(p)
    }
}
impl NativeActor {
    pub(super) fn user_id(&self) -> &str {
        &self.0.user_id
    }
    pub(super) fn draft_owner_filter(&self) -> PublicApiResult<Option<&str>> {
        if self.0.is_moderator() {
            self.0.require_permission("manga.state.update")?;
            Ok(None)
        } else {
            self.0.require_permission("manga.create")?;
            Ok(Some(self.user_id()))
        }
    }
    pub(super) fn require_draft_owner(&self, owner: &str) -> PublicApiResult<()> {
        if owner == self.user_id() {
            self.0.require_permission("manga.create")?;
        } else if self.0.is_moderator() {
            self.0.require_permission("manga.state.update")?;
        } else {
            return Err(PublicApiError::forbidden(
                "This draft belongs to another account",
            ));
        }
        Ok(())
    }
}
pub(super) async fn actor(
    state: &AppState,
    headers: &HeaderMap,
    tx: &Transaction<'_>,
    write: bool,
) -> PublicApiResult<NativeActor> {
    let actor = account::actor(state, headers, tx, write).await?;
    actor.require_verified()?;
    Ok(NativeActor(actor))
}
pub(super) async fn capture(
    tx: &Transaction<'_>,
    actor: &NativeActor,
    input: &mut Value,
) -> PublicApiResult<String> {
    let id = super::editorial::take_expected_account(input)?
        .ok_or_else(|| PublicApiError::bad_request("expectedAccountId required"))?;
    let id = PublicUuid::parse(
        id.as_str()
            .ok_or_else(|| PublicApiError::bad_request("Invalid expectedAccountId"))?,
    )?;
    account::expected(tx, &actor.0, &id).await
}
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/account/contribution-context", get(context))
        .route("/account/manga", post(create))
        .route("/account/manga/{p0}", put(update))
        .route("/account/manga/draft/{id}/commit", post(submit))
        .route("/account/manga/draft/{id}/publish", post(publish))
        .route("/account/manga/draft/{id}/reject", post(reject))
}
async fn context(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    super::uploads::no_query(&uri)?;
    account::body(request, &headers, false, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, false).await?;
    let user = preferred(&tx, ResourceKind::User, actor.user_id()).await?;
    let version: i32 = tx
        .query_one(
            "SELECT version FROM platform.users WHERE id=$1",
            &[&actor.user_id()],
        )
        .await?
        .get(0);
    let namespace: String = tx
        .query_one(
            "SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton",
            &[],
        )
        .await?
        .get(0);
    let configured = state.config.protocol_publication.is_some();
    let preflight = crate::protocol::publication::native_upload_preflight(&state, &tx, &actor, &[]).await?;
    let permissions = actor
        .0
        .permissions
        .iter()
        .filter(|p| actor.0.has_perm(p))
        .collect::<Vec<_>>();
    let mut value = json!({"result":"ok","user":{"id":user,"version":version,"username":actor.0.username,"roles":actor.0.roles,"permissions":permissions},"nostr":{"catalogueScope":"instance-local","catalogNamespace":PublicUuid::parse(&namespace)?,"nativeSessionAuthority":true},"upload":{"canBegin":actor.has_perm("chapter.create"),"canManage":actor.has_perm("self.chapter.update"),"conditional":true,"publication":{"scope":if configured{"original-protocol"}else{"instance-local"},"configured":configured,"requiresOperatorOrAuthorizedGroup":true},"limits":{"maxFiles":storage::MAX_FILES,"maxFilesPerRequest":5,"maxFileBytes":storage::MAX_FILE_BYTES,"maxSessionBytes":storage::MAX_SESSION_BYTES,"maxPixels":storage::MAX_PIXELS,"maxDimension":storage::MAX_DIMENSION}}});
    value["upload"]["preflight"] = preflight;
    value["groups"] = super::native_collaboration::own_groups(&tx, &actor).await?;
    account::provenance(&mut value, user.as_str());
    tx.commit().await?;
    Ok(Json(value))
}
async fn project(tx: &Transaction<'_>, native: Value, account: &str, include_feedback: bool) -> PublicApiResult<Value> {
    let native_id = native["id"].as_str().ok_or_else(PublicApiError::unavailable)?.to_owned();
    let mut value = serde_json::to_value(manga_writes::resource(tx, native).await?.0)
        .map_err(|_| PublicApiError::unavailable())?;
    if include_feedback {
        value["data"]["attributes"]["reviewTimeline"] = super::native_draft_reviews::timeline(tx, &native_id).await?;
    }
    account::provenance(&mut value, account);
    Ok(value)
}
async fn create(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    super::uploads::no_query(&uri)?;
    let bytes = account::body(request, &headers, true, 65536).await?;
    let mut input: Value = super::native_json::parse(&bytes)
        .map_err(|_| PublicApiError::bad_request("Invalid title command"))?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, true).await?;
    let account = capture(&tx, &actor, &mut input).await?;
    let (mut native, metadata) = manga_writes::normalize(input, false, "")?;
    manga_writes::ids(&tx, &mut native).await?;
    let result = project(
        &tx,
        core::manga_create_in(&tx, &actor, &native, &metadata).await?,
        &account,
        true, // Creation already authorized this draft's owner.
    )
    .await?;
    tx.commit().await?;
    Ok(Json(result))
}
async fn update(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    super::uploads::no_query(&uri)?;
    let id = PublicUuid::parse(&id)?;
    let bytes = account::body(request, &headers, true, 65536).await?;
    let mut input: Value = super::native_json::parse(&bytes)
        .map_err(|_| PublicApiError::bad_request("Invalid title command"))?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, true).await?;
    let account = capture(&tx, &actor, &mut input).await?;
    let native = resolve(&tx, ResourceKind::Manga, &id).await?;
    let row=tx.query_opt("SELECT language,owner_id FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id)",&[&native]).await?.ok_or_else(PublicApiError::not_found)?;
    let language: String = row.get(0);
    // Editing catalogue metadata does not confer private review visibility.
    // Reuse the exact author/reviewer rule applied to private draft reads.
    let include_feedback = actor.require_draft_owner(row.get::<_, &str>(1)).is_ok();
    let (mut patch, metadata) = manga_writes::normalize(input, true, &language)?;
    manga_writes::ids(&tx, &mut patch).await?;
    let result = project(
        &tx,
        core::manga_update_in(&tx, &actor, &native, &patch, &metadata).await?,
        &account,
        include_feedback,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(result))
}
async fn submit(
    s: State<AppState>,
    u: OriginalUri,
    p: Path<String>,
    h: HeaderMap,
    r: Request,
) -> PublicApiResult<(StatusCode, Json<Value>)> {
    Ok((
        StatusCode::CREATED,
        transition(s, u, p, h, r, "submitted").await?,
    ))
}
async fn publish(
    s: State<AppState>,
    u: OriginalUri,
    p: Path<String>,
    h: HeaderMap,
    r: Request,
) -> PublicApiResult<Json<Value>> {
    transition(s, u, p, h, r, "published").await
}
async fn reject(
    s: State<AppState>,
    u: OriginalUri,
    p: Path<String>,
    h: HeaderMap,
    r: Request,
) -> PublicApiResult<Json<Value>> {
    transition(s, u, p, h, r, "rejected").await
}
async fn transition(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Request,
    next: &str,
) -> PublicApiResult<Json<Value>> {
    super::uploads::no_query(&uri)?;
    let id = PublicUuid::parse(&id)?;
    let bytes = account::body(request, &headers, true, 12000).await?;
    let mut input: Value = super::native_json::parse(&bytes)
        .map_err(|_| PublicApiError::bad_request("Invalid title transition"))?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, true).await?;
    let account = capture(&tx, &actor, &mut input).await?;
    crate::domains::catalog::validation::allowed(&input, &["version", "reason", "affectedFields"])?;
    crate::domains::catalog::validation::version(&input)?;
    let id = resolve(&tx, ResourceKind::Manga, &id).await?;
    let visible:bool=tx.query_opt("SELECT protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id) FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL",&[&id]).await?.ok_or_else(PublicApiError::not_found)?.get(0);
    if !visible
        && !(next == "published"
            && crate::protocol::publication::pending_native_manga_receipt_allowed(
                &state, &tx, &id, &actor, true,
            )
            .await?)
    {
        return Err(PublicApiError::not_found());
    }
    // Native owner submission and moderator review retain their exact gates. A
    // review action still cannot borrow the configured source publisher identity.
    let native = core::manga_transition_in(&state, &tx, &actor, &id, &input, next, true).await?;
    let result = project(&tx, native, &account, true).await?;
    tx.commit().await?;
    Ok(Json(result))
}
