//! Ordinary browser account actions over canonical catalogue UUIDs. These
//! routes are explicitly cookie-only: signed and bearer credentials never fall
//! back to a cookie. They reuse the same persisted account mutation cores as the
//! separate signed adapter, without enrollment, key custody or protocol events.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdTarget, PublicUuid,
    ResourceKind,
};
use crate::{
    AppState,
    domains::{
        auth,
        library::{
            account_public_id_in,
            follow_mutations::{self, Target},
            ratings,
        },
    },
};
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{OriginalUri, Path, Request, State},
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Duration};
use tokio_postgres::Transaction;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/account/me", get(me))
        .route("/account/rating", get(rating_read))
        .route(
            "/account/rating/{id}",
            post(rating_set).delete(rating_delete),
        )
        .route("/account/report/reasons/{category}", get(report_reasons))
        .route("/account/report", post(report_create))
        .route(
            "/account/{kind}/{id}/follow",
            get(follow_read).post(follow_set).delete(follow_delete),
        )
}
fn no_query(uri: &Uri) -> PublicApiResult<()> {
    if uri.query().is_some() {
        Err(PublicApiError::bad_request(
            "This operation accepts no query parameters",
        ))
    } else {
        Ok(())
    }
}
async fn body(
    request: Request,
    headers: &HeaderMap,
    write: bool,
    limit: usize,
) -> PublicApiResult<Vec<u8>> {
    if write {
        let mut types = headers.get_all(header::CONTENT_TYPE).iter();
        if !types.next().and_then(|v| v.to_str().ok()).is_some_and(|v| {
            v.split(';')
                .next()
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
        }) || types.next().is_some()
        {
            return Err(PublicApiError::bad_request("JSON content type required"));
        }
    }
    let bytes = tokio::time::timeout(Duration::from_secs(5), to_bytes(request.into_body(), limit))
        .await
        .map_err(|_| PublicApiError::bad_request("Request body timeout"))?
        .map_err(|_| PublicApiError::bad_request("Request body exceeds the operation budget"))?;
    if !write && !bytes.is_empty() {
        return Err(PublicApiError::bad_request("GET accepts no body"));
    }
    Ok(bytes.to_vec())
}
async fn actor(
    state: &AppState,
    headers: &HeaderMap,
    tx: &Transaction<'_>,
    write: bool,
    verified: bool,
) -> PublicApiResult<auth::SessionPrincipal> {
    // The cookie-only extractor rejects Authorization before validate_mutation's
    // bearer exemption could apply. Both source and persisted account/session
    // authority are checked before any private data or mutation is accessed.
    let actor = auth::require_native_account_principal_in(state, headers, tx, write).await?;
    if write {
        auth::validate_mutation(state, headers)?;
    }
    if verified {
        actor.require_verified()?;
    }
    Ok(actor)
}
async fn target(
    tx: &Transaction<'_>,
    kind: ResourceKind,
    id: &PublicUuid,
) -> PublicApiResult<super::ResolvedPublicId> {
    PostgresPublicIdResolver::resolve_many_in(tx, kind, std::slice::from_ref(id))
        .await?
        .remove(id)
        .filter(|r| r.target == PublicIdTarget::NativeResource)
        .ok_or_else(PublicApiError::not_found)
}
fn expected(value: Option<&str>, version: Option<i32>) -> PublicApiResult<()> {
    let value =
        value.ok_or_else(|| PublicApiError::bad_request("expectedAccountId is required"))?;
    PublicUuid::parse(value)?;
    if version.is_none_or(|v| v < 0) {
        return Err(PublicApiError::bad_request(
            "A nonnegative version is required",
        ));
    }
    Ok(())
}
async fn me(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    no_query(&uri)?;
    body(request, &headers, false, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    // Account context is a private owner read, not a social permission grant.
    // Preserve native library access for restricted/unverified normal sessions;
    // every social operation independently enforces its stricter current policy.
    let actor = auth::require_native_library_principal_in(&state, &headers, &tx).await?;
    let id = account_public_id_in(&tx, &actor.user_id).await?;
    let version: i32 = tx
        .query_one(
            "SELECT version FROM platform.users WHERE id=$1",
            &[&actor.user_id],
        )
        .await?
        .get(0);
    let permissions = actor
        .permissions
        .iter()
        .filter(|p| actor.has_perm(p))
        .collect::<Vec<_>>();
    let value = json!({"result":"ok","response":"entity","data":{"id":id,"type":"user","attributes":{"username":actor.username,"roles":actor.roles,"permissions":permissions,"version":version,"isVerified":actor.verified},"relationships":[]},"nostr":{"scope":"instance-account","identity":"native-session-account","authentication":"native-session","nativeSessionAuthority":true}});
    tx.commit().await?;
    Ok(Json(value))
}
async fn rating_read(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    let ids = super::ratings::selected(uri.query())?;
    body(request, &headers, false, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = actor(&state, &headers, &tx, false, true).await?;
    let account = account_public_id_in(&tx, &actor.user_id).await?;
    let aliases =
        PostgresPublicIdResolver::resolve_many_in(&*tx, ResourceKind::Manga, &ids).await?;
    if aliases.len() != ids.len()
        || aliases
            .values()
            .any(|r| r.target != PublicIdTarget::NativeResource)
    {
        return Err(PublicApiError::not_found());
    }
    let targets = aliases
        .into_values()
        .map(|r| (r.native_id, r.preferred_id))
        .collect::<BTreeMap<_, _>>();
    let natives = targets.keys().cloned().collect::<Vec<_>>();
    let visible: i64 = tx
        .query_one(
            &format!(
                "SELECT count(*) FROM platform.catalog_manga m WHERE m.id=ANY($1::text[]) AND {}",
                super::catalogue::MANGA_VISIBLE
            ),
            &[&natives],
        )
        .await?
        .get(0);
    if visible as usize != natives.len() {
        return Err(PublicApiError::not_found());
    }
    let values = ratings::read_in(&tx, &actor.user_id, &natives)
        .await?
        .into_iter()
        .map(|(id, v)| (targets[&id].clone(), v))
        .collect::<BTreeMap<_, _>>();
    let mut versions = targets
        .values()
        .map(|id| (id.clone(), 0))
        .collect::<BTreeMap<_, _>>();
    for row in tx.query("SELECT manga_id,version FROM platform.library_manga_ratings WHERE user_id=$1 AND manga_id=ANY($2::text[])", &[&actor.user_id,&natives]).await? { versions.insert(targets[&row.get::<_,String>(0)].clone(),row.get::<_,i32>(1)); }
    tx.commit().await?;
    Ok(Json(
        json!({"result":"ok","ratings":values,"nostr":{"scope":"instance-account","source":"persisted-native-account-ratings","authentication":"native-session","accountId":account,"versions":versions,"zeroVersionMeans":"no-persisted-rating-record"}}),
    ))
}
async fn rating_set(
    State(s): State<AppState>,
    OriginalUri(u): OriginalUri,
    Path(id): Path<String>,
    h: HeaderMap,
    r: Request,
) -> PublicApiResult<Json<Value>> {
    rating_write(s, u, id, h, r, false).await
}
async fn rating_delete(
    State(s): State<AppState>,
    OriginalUri(u): OriginalUri,
    Path(id): Path<String>,
    h: HeaderMap,
    r: Request,
) -> PublicApiResult<Json<Value>> {
    rating_write(s, u, id, h, r, true).await
}
async fn rating_write(
    state: AppState,
    uri: Uri,
    id: String,
    headers: HeaderMap,
    request: Request,
    delete: bool,
) -> PublicApiResult<Json<Value>> {
    no_query(&uri)?;
    let id = PublicUuid::parse(&id)?;
    let bytes = body(request, &headers, true, 1024).await?;
    let (rating, version, account) = if delete {
        let p: ratings::DeletePayload = serde_json::from_slice(&bytes)
            .map_err(|_| PublicApiError::bad_request("Invalid rating delete JSON"))?;
        (None, p.version, p.expected_account_id)
    } else {
        let p: ratings::RatingPayload = serde_json::from_slice(&bytes)
            .map_err(|_| PublicApiError::bad_request("Invalid rating JSON"))?;
        (Some(p.rating), p.version, p.expected_account_id)
    };
    expected(account.as_deref(), version)?;
    ratings::validate(rating, version)?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, true, true).await?;
    let target = target(&tx, ResourceKind::Manga, &id).await?;
    let mut value = ratings::mutate_in(
        &tx,
        &actor,
        &target.native_id,
        rating,
        version,
        account.as_deref(),
    )
    .await?;
    value["nostr"]["mangaId"] = json!(target.preferred_id);
    value["nostr"]["authentication"] = json!("native-session");
    tx.commit().await?;
    Ok(Json(value))
}
async fn report_reasons(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(category): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Response> {
    no_query(&uri)?;
    super::reports::kind(&category)?;
    body(request, &headers, false, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, false, true).await?;
    actor.require_permission("report.create")?;
    let mut value = super::reports::reasons_in(&tx, &category).await?;
    value.extensions.insert("nostr".into(), json!({"scope":"instance-account","authentication":"native-session","accountId":account_public_id_in(&tx,&actor.user_id).await?}));
    tx.commit().await?;
    Ok(Json(value).into_response())
}
async fn report_create(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Response> {
    no_query(&uri)?;
    let bytes = body(request, &headers, true, 8192).await?;
    let payload: super::reports::CreateReport = serde_json::from_slice(&bytes)
        .map_err(|_| PublicApiError::bad_request("Invalid report JSON"))?;
    if payload.expected_account_id.is_none() || payload.expected_reason_version.is_none() {
        return Err(PublicApiError::bad_request(
            "Expected account and reason version are required",
        ));
    }
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, true, true).await?;
    let account = account_public_id_in(&tx, &actor.user_id).await?;
    if payload.expected_account_id.as_ref().map(PublicUuid::as_str) != Some(account.as_str()) {
        return Err(PublicApiError::conflict(
            "Account changed; refresh before submitting",
        ));
    }
    super::reports::create_in(&tx, &actor, &payload).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(json!({"result":"ok","nostr":{"scope":"instance-account","authentication":"native-session","accountId":account}}))).into_response())
}
fn follow_kind(kind: &str) -> PublicApiResult<(ResourceKind, Target)> {
    match kind {
        "manga" => Ok((ResourceKind::Manga, Target::Manga)),
        "group" => Ok((ResourceKind::Group, Target::Group)),
        "user" => Ok((ResourceKind::User, Target::User)),
        _ => Err(PublicApiError::bad_request("Unsupported follow kind")),
    }
}
async fn follow_read(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path((kind, id)): Path<(String, String)>,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    no_query(&uri)?;
    let (resource, kind) = follow_kind(&kind)?;
    let id = PublicUuid::parse(&id)?;
    body(request, &headers, false, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = auth::require_native_library_principal_in(&state, &headers, &tx).await?;
    let target = target(&tx, resource, &id).await?;
    let (alias, table, _, visible) = super::catalogue::parts(resource)?;
    if !tx
        .query_one(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE {alias}.id=$1 AND {visible})"),
            &[&target.native_id],
        )
        .await?
        .get::<_, bool>(0)
    {
        return Err(PublicApiError::not_found());
    }
    let mut meta = follow_mutations::state_in(&tx, &actor.user_id, kind, &target.native_id).await?;
    meta["accountId"] = json!(account_public_id_in(&tx, &actor.user_id).await?);
    meta["targetId"] = json!(target.preferred_id);
    meta["kind"] = json!(resource.as_str());
    meta["scope"] = json!("instance-account");
    meta["source"] = json!("persisted-native-account-follows");
    meta["versionSemantics"] = json!("membership-change-revision");
    meta["authentication"] = json!("native-session");
    tx.commit().await?;
    Ok(Json(json!({"result":"ok","nostr":meta})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FollowCommand {
    expected_account_id: PublicUuid,
    version: i32,
}
async fn follow_set(
    State(s): State<AppState>,
    OriginalUri(u): OriginalUri,
    Path((k, id)): Path<(String, String)>,
    h: HeaderMap,
    r: Request,
) -> PublicApiResult<Json<Value>> {
    follow_write(s, u, k, id, h, r, true).await
}
async fn follow_delete(
    State(s): State<AppState>,
    OriginalUri(u): OriginalUri,
    Path((k, id)): Path<(String, String)>,
    h: HeaderMap,
    r: Request,
) -> PublicApiResult<Json<Value>> {
    follow_write(s, u, k, id, h, r, false).await
}
async fn follow_write(
    state: AppState,
    uri: Uri,
    kind: String,
    id: String,
    headers: HeaderMap,
    request: Request,
    follow: bool,
) -> PublicApiResult<Json<Value>> {
    no_query(&uri)?;
    let (resource, kind) = follow_kind(&kind)?;
    let id = PublicUuid::parse(&id)?;
    let bytes = body(request, &headers, true, 1024).await?;
    let command: FollowCommand = serde_json::from_slice(&bytes).map_err(|_| {
        PublicApiError::bad_request("Expected account and follow version are required")
    })?;
    if command.version < 0 {
        return Err(PublicApiError::bad_request(
            "Follow version must be nonnegative",
        ));
    }
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, true, resource != ResourceKind::Manga).await?;
    let target = target(&tx, resource, &id).await?;
    let mut meta = if resource == ResourceKind::Manga {
        follow_mutations::mutate_native_manga_in(
            &tx,
            &actor,
            &target.native_id,
            follow,
            command.version,
            command.expected_account_id.as_str(),
        )
        .await?
    } else {
        follow_mutations::mutate_in(
            &tx,
            &actor,
            kind,
            &target.native_id,
            follow,
            Some(command.version),
            Some(command.expected_account_id.as_str()),
        )
        .await?
    };
    meta["targetId"] = json!(target.preferred_id);
    meta["authentication"] = json!("native-session");
    tx.commit().await?;
    Ok(Json(json!({"result":"ok","nostr":meta})))
}
