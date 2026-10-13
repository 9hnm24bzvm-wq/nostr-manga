//! Private submission intake and local operator decisions; no public relay drafts.
use super::*;
use crate::submission_review::{ReviewDenied, Reviews};
use base64::{Engine, engine::general_purpose::STANDARD};
fn service(a: &App) -> Result<Arc<Reviews>, ApiError> {
    a.reviews.clone().ok_or_else(unavailable)
}
fn failure(error: anyhow::Error) -> ApiError {
    if error.is::<crate::download_budget::Busy>() {
        return download_busy();
    }
    if let Some(rejection) = error.downcast_ref::<crate::transfer_rejection::TransferRejection>() {
        return (
            StatusCode::CONFLICT,
            Json(
                json!({"error":"review_publication_not_admitted","category":rejection.class,"reason":rejection.reason}),
            ),
        );
    }
    match error.downcast_ref::<ReviewDenied>() {
        Some(ReviewDenied::Budget) => (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"review_budget"})),
        ),
        Some(ReviewDenied::Authority) => forbidden("review account or owner permission changed"),
        Some(ReviewDenied::Closed) => (
            StatusCode::CONFLICT,
            Json(json!({"error":"review_closed_expired_or_changed"})),
        ),
        Some(ReviewDenied::Object) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"review_object_unavailable"})),
        ),
        None if error.is::<tokio_postgres::Error>()
            || error.is::<deadpool_postgres::PoolError>()
            || error.is::<std::io::Error>() =>
        {
            tracing::warn!(error=%error,"submission review unavailable");
            unavailable()
        }
        None => bad("invalid submission, manifest, original or review bounds"),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declaration {
    event: auth::Event,
    manifest: String,
}
pub(super) async fn declare(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let (authorization, account) = account_preflight(&a, &uri, &headers, "POST").await?;
    let review = service(&a)?;
    let slot = a
        .event_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| unavailable())?;
    let body = group_body(&a, &uri, &authorization, request, 512 * 1024).await?;
    let request: Declaration = serde_json::from_slice(&body)
        .map_err(|_| bad("signed event and base64 manifest required"))?;
    if request.manifest.len() > 350_000 {
        return Err(bad("manifest byte budget"));
    }
    let bytes = STANDARD
        .decode(&request.manifest)
        .map_err(|_| bad("manifest base64"))?;
    let id = request.event.id.clone();
    let inserted = crate::node::spawn_owned_scope(async move {
        let _slot = slot;
        review.declare(&account, request.event, &bytes).await
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(failure)?;
    Ok((
        if inserted {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(
            json!({"id":id,"state":"pending","publisher_permission_granted":false,"public":false}),
        ),
    ))
}
pub(super) async fn upload(
    State(a): State<Shared>,
    Path((submission, hash)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let (authorization, account) = account_preflight(&a, &uri, &headers, "POST").await?;
    let submission = id(&submission).map_err(|_| bad("submission ID"))?;
    let hash = id(&hash).map_err(|_| bad("object hash"))?;
    let review = service(&a)?;
    let size = review
        .expected_size(&account, submission, hash)
        .await
        .map_err(failure)?;
    let slot = review
        .storage
        .slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| unavailable())?;
    let bytes = group_body(&a, &uri, &authorization, request, size).await?;
    crate::node::spawn_owned_scope(async move {
        let _slot = slot;
        review.upload(&account, submission, hash, &bytes).await
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(failure)?;
    Ok(Json(
        json!({"hash":hex::encode(hash),"staged":true,"public":false}),
    ))
}
pub(super) async fn read_blob(
    State(a): State<Shared>,
    Path((submission, hash)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    let (authorization, account) = account_preflight(&a, &uri, &headers, "GET").await?;
    consume_authorization(&a, &authorization).await?;
    let review = service(&a)?;
    let snapshot = review
        .download(
            &account,
            id(&submission).map_err(|_| bad("submission ID"))?,
            id(&hash).map_err(|_| bad("object hash"))?,
        )
        .await
        .map_err(failure)?;
    Ok(snapshot.response(None, "application/octet-stream", true))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Page {
    after: Option<String>,
    limit: Option<i64>,
    #[serde(default)]
    pending_only: bool,
}
async fn listing(a: &App, author: Option<Id>, query: Page) -> Result<Json<Value>, ApiError> {
    let limit = query.limit.unwrap_or(32);
    if !(1..=64).contains(&limit) {
        return Err(bad("review page limit"));
    }
    let after = query
        .after
        .as_deref()
        .map(id)
        .transpose()
        .map_err(|_| bad("review cursor"))?;
    a.repo
        .list_reviews(author, after, limit, query.pending_only)
        .await
        .map(Json)
        .map_err(failure)
}
pub(super) async fn list_own(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(query): Query<Page>,
) -> Result<Json<Value>, ApiError> {
    let (authorization, account) = account_preflight(&a, &uri, &headers, "GET").await?;
    consume_authorization(&a, &authorization).await?;
    listing(&a, Some(account.publisher), query).await
}
pub(super) async fn list_admin(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(query): Query<Page>,
) -> Result<Json<Value>, ApiError> {
    let event = operator_preflight(&a, &uri, &headers, "GET")?;
    consume_authorization(&a, &event).await?;
    listing(&a, None, query).await
}
async fn detail(a: &App, submission: &str, author: Option<Id>) -> Result<Json<Value>, ApiError> {
    a.repo
        .review_status(id(submission).map_err(|_| bad("submission ID"))?, author)
        .await
        .map_err(failure)?
        .map(Json)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error":"submission_unavailable"})),
            )
        })
}
pub(super) async fn own(
    State(a): State<Shared>,
    Path(submission): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let (event, account) = account_preflight(&a, &uri, &headers, "GET").await?;
    consume_authorization(&a, &event).await?;
    detail(&a, &submission, Some(account.publisher)).await
}
pub(super) async fn admin(
    State(a): State<Shared>,
    Path(submission): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let event = operator_preflight(&a, &uri, &headers, "GET")?;
    consume_authorization(&a, &event).await?;
    detail(&a, &submission, None).await
}
pub(super) async fn withdraw(
    State(a): State<Shared>,
    Path(submission): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let (authorization, account) = account_preflight(&a, &uri, &headers, "POST").await?;
    let review = service(&a)?;
    let body = group_body(&a, &uri, &authorization, request, 1024).await?;
    let body: Value = serde_json::from_slice(&body).map_err(|_| bad("empty object required"))?;
    if body != json!({}) {
        return Err(bad("empty object required"));
    }
    review
        .withdraw(id(&submission).map_err(|_| bad("submission ID"))?, &account)
        .await
        .map_err(failure)?;
    Ok(Json(json!({"id":submission,"state":"withdrawn"})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    action: String,
    #[serde(default)]
    reason: String,
}
pub(super) async fn decide(
    State(a): State<Shared>,
    Path(submission): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let body = operator_json(&a, &uri, &headers, request).await?;
    let request: Decision = serde_json::from_value(body)
        .map_err(|_| bad("decision action and optional reason required"))?;
    if request.reason.len() > 4096 || request.reason.contains('\0') {
        return Err(bad("review reason budget"));
    }
    let review = service(&a)?;
    let key = id(&submission).map_err(|_| bad("submission ID"))?;
    let moderator = id(&a.admin).map_err(|_| unavailable())?;
    match request.action.as_str() {
        "approve" => {
            if !request.reason.is_empty() {
                return Err(bad("approval does not accept a rejection reason"));
            }
            let slot = review
                .publications
                .slots
                .clone()
                .try_acquire_owned()
                .map_err(|_| unavailable())?;
            crate::node::spawn_owned_scope(async move {
                let _slot = slot;
                review.approve(key, moderator).await
            })
            .await
            .map_err(|_| unavailable())?
            .map_err(failure)?;
        }
        "reject" => review
            .reject(key, moderator, request.reason)
            .await
            .map_err(failure)?,
        _ => return Err(bad("approve or reject required")),
    }
    detail(&a, &submission, None).await
}
