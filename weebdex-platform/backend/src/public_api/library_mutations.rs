//! Version1 client-signed library mutation extension. Plain MDex logical fields
//! cannot authorize server signing; the original signed graph remains authority.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdResolver, PublicIdTarget,
    PublicUuid, ResourceKind, auth,
};
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{OriginalUri, Path, RawQuery, Request, State},
    http::{HeaderMap, StatusCode, header},
    routing::post,
};
use manga_node::{
    auth as original_auth,
    db::Id,
    http_auth::{HttpAuthError, HttpService},
    library::HeadConflict,
    library_admission::Busy,
    library_http_mutation::{
        IntentRejected, MAX_HTTP_BODY, MappedHttpIntent, PrincipalMismatch, SignedCommitRequired,
        TargetUnavailable,
    },
    library_view::{SnapshotChanged, ViewBudget},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
pub fn router() -> Router<AppState> {
    Router::new().route("/manga/{p0}/status", post(status))
}
fn unavailable() -> PublicApiError {
    ApiError::unavailable().into()
}
fn mutation_error(error: anyhow::Error) -> PublicApiError {
    if error.is::<HttpAuthError>() {
        return auth::map_error(error.downcast::<HttpAuthError>().unwrap());
    }
    if error.is::<HeadConflict>() || error.is::<SnapshotChanged>() {
        return PublicApiError {
            status: StatusCode::CONFLICT,
            title: "Library head changed",
            detail:
                "Refresh the signed public library head before building another explicit mutation"
                    .into(),
        };
    }
    if error.is::<PrincipalMismatch>() {
        return PublicApiError {
            status: StatusCode::FORBIDDEN,
            title: "Permission denied",
            detail: "The signed library event must belong to the exact request principal".into(),
        };
    }
    if error.is::<TargetUnavailable>() {
        return PublicApiError::not_found();
    }
    if error.is::<SignedCommitRequired>()
        || error.is::<IntentRejected>()
        || error.is::<ViewBudget>()
    {
        return PublicApiError::bad_request(error.to_string());
    }
    if error.is::<Busy>() {
        return unavailable();
    }
    tracing::debug!(error=%error,"original signed library intent rejected or unavailable");
    unavailable()
}
fn original_id(value: &str) -> PublicApiResult<Id> {
    if !original_auth::valid_hash(value) {
        return Err(unavailable());
    }
    hex::decode(value)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(unavailable)
}
fn query(raw: Option<&str>, read: bool) -> PublicApiResult<()> {
    let Some(raw) = raw else { return Ok(()) };
    let mut seen = false;
    for (key, value) in url::form_urlencoded::parse(raw.as_bytes()) {
        if !read || key != "updateHistory" || seen || !matches!(value.as_ref(), "true" | "false") {
            return Err(PublicApiError::bad_request(
                "Unsupported or duplicate mutation query parameter",
            ));
        }
        seen = true;
        if value == "true" {
            return Err(PublicApiError::bad_request(
                "updateHistory=true requires actual read timestamps, which the signed library protocol does not currently record",
            ));
        }
    }
    if !seen {
        return Err(PublicApiError::bad_request(
            "Empty mutation query is not declared",
        ));
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadIds {
    #[serde(rename = "chapterIdsRead")]
    read: Option<Vec<String>>,
    #[serde(rename = "chapterIdsUnread")]
    unread: Option<Vec<String>>,
    nostr: Option<serde::de::IgnoredAny>,
}
pub(super) async fn read(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<(StatusCode, Json<Value>)> {
    query(raw.as_deref(), true)?;
    mutate(state, uri, id, headers, request, true).await
}
async fn status(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<(StatusCode, Json<Value>)> {
    query(raw.as_deref(), false)?;
    mutate(state, uri, id, headers, request, false).await
}
async fn mutate(
    state: AppState,
    uri: axum::http::Uri,
    id: String,
    headers: HeaderMap,
    request: Request,
    read: bool,
) -> PublicApiResult<(StatusCode, Json<Value>)> {
    if headers.contains_key(header::CONTENT_ENCODING) {
        return Err(PublicApiError::bad_request(
            "This signed extension accepts exact uncompressed JSON bytes",
        ));
    }
    if !headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"))
        })
    {
        return Err(PublicApiError::bad_request(
            "Content-Type application/json is required",
        ));
    }
    let mut authorizations = headers.get_all(header::AUTHORIZATION).iter();
    let authorization = authorizations
        .next()
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| auth::map_error(HttpAuthError::Invalid))?;
    if authorizations.next().is_some() {
        return Err(auth::map_error(HttpAuthError::Invalid));
    }
    let publication = state
        .config
        .protocol_publication
        .as_ref()
        .filter(|config| config.native_node.is_some())
        .ok_or_else(|| auth::map_error(HttpAuthError::Service))?;
    let service = HttpService::new(&state.config.public_origin, &publication.storage_holder)
        .map_err(auth::map_error)?;
    let preflight = state
        .protocol
        .preflight_http_request(&service, authorization, &uri.to_string(), "POST")
        .await
        .map_err(auth::map_error)?;
    let declared = headers
        .get(header::CONTENT_LENGTH)
        .map(|value| {
            value
                .to_str()
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .ok_or_else(|| PublicApiError::bad_request("Invalid Content-Length"))
        })
        .transpose()?;
    if declared.is_some_and(|length| length > MAX_HTTP_BODY) {
        return Err(PublicApiError::bad_request(
            "The signed extension request exceeds1MiB",
        ));
    }
    let receipt = state
        .protocol
        .receive_library_http_intent(&preflight, Some(declared.unwrap_or(MAX_HTTP_BODY)))
        .await
        .map_err(mutation_error)?;
    let body = tokio::time::timeout(
        Duration::from_secs(5),
        to_bytes(request.into_body(), receipt.limit.min(MAX_HTTP_BODY)),
    )
    .await
    .map_err(|_| PublicApiError::bad_request("Signed mutation body timeout"))?
    .map_err(|_| PublicApiError::bad_request("Signed mutation body exceeds its receipt limit"))?;
    let capability = auth::admitted_request(&state, &uri, &headers, "POST", &body).await?;
    let resolver = PostgresPublicIdResolver::new(state.db.clone());
    let alias = resolver
        .resolve(ResourceKind::Manga, &PublicUuid::parse(&id)?)
        .await?
        .ok_or_else(PublicApiError::not_found)?;
    if alias.target != PublicIdTarget::NativeResource {
        return Err(PublicApiError::not_found());
    }
    let manga=state.db.client().await?.query_opt("SELECT m.protocol_series_id FROM platform.catalog_manga m WHERE m.id=$1 AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND m.protocol_series_id IS NOT NULL AND platform.protocol_source_is_visible(m.protocol_series_id)",&[&alias.native_id]).await.map_err(ApiError::from)?.ok_or_else(PublicApiError::not_found)?;
    let series = original_id(&manga.get::<_, String>(0))?;
    let mapped = if read {
        let parsed: ReadIds = serde_json::from_slice(&body).map_err(|_| {
            PublicApiError::bad_request("Invalid read-marker logical body or signed extension")
        })?;
        let _extension_present = parsed.nostr;
        let ids = parsed
            .read
            .into_iter()
            .flatten()
            .chain(parsed.unread.into_iter().flatten())
            .collect::<Vec<_>>();
        if ids.len() > 1000 {
            return Err(PublicApiError::bad_request(
                "Too many requested chapter identifiers",
            ));
        }
        let parsed = ids
            .iter()
            .map(|value| PublicUuid::parse(value).map_err(PublicApiError::from))
            .collect::<PublicApiResult<Vec<_>>>()?;
        let unique = parsed.iter().cloned().collect::<BTreeSet<_>>();
        if unique.len() != ids.len() {
            return Err(PublicApiError::bad_request(
                "Duplicate or overlapping chapter identifiers",
            ));
        }
        let mut aliases = BTreeMap::new();
        for batch in parsed.chunks(super::postgres_identity::MAX_PUBLIC_ID_BATCH) {
            aliases.extend(resolver.resolve_many(ResourceKind::Chapter, batch).await?);
        }
        if aliases.len() != ids.len()
            || aliases
                .values()
                .any(|alias| alias.target != PublicIdTarget::NativeResource)
        {
            return Err(PublicApiError::not_found());
        }
        let native = aliases
            .values()
            .map(|alias| alias.native_id.clone())
            .collect::<Vec<_>>();
        let rows=state.db.client().await?.query("SELECT c.id,c.protocol_event_id,m.protocol_series_id FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=ANY($1::text[]) AND m.id=$2 AND c.deleted_at IS NULL AND NOT c.is_unavailable AND c.protocol_event_id IS NOT NULL AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(c.protocol_event_id) AND platform.protocol_source_is_visible(m.protocol_series_id)",&[&native,&alias.native_id]).await.map_err(ApiError::from)?;
        if rows.len() != native.len() {
            return Err(PublicApiError::not_found());
        }
        let mut sources = BTreeMap::new();
        for row in rows {
            if original_id(&row.get::<_, String>(2))? != series {
                return Err(unavailable());
            }
            sources.insert(
                row.get::<_, String>(0),
                original_id(&row.get::<_, String>(1))?,
            );
        }
        let mut chapters = BTreeMap::new();
        for (spelling, uuid) in ids.into_iter().zip(parsed) {
            chapters.insert(
                spelling,
                *sources
                    .get(&aliases[&uuid].native_id)
                    .ok_or_else(unavailable)?,
            );
        }
        MappedHttpIntent::ReadMarkers { series, chapters }
    } else {
        MappedHttpIntent::Status { series }
    };
    let receipt = state
        .protocol
        .apply_library_http_intent(capability, mapped, body.to_vec(), receipt)
        .await
        .map_err(mutation_error)?;
    Ok((
        if receipt.projected {
            StatusCode::OK
        } else {
            StatusCode::ACCEPTED
        },
        Json(
            json!({"result":"ok","nostr":{"pubkey":receipt.event.pubkey,"head":receipt.event.id,"projected":receipt.projected,"scope":"explicit_client_signed_public_library","extensionVersion":1,"changedBuckets":receipt.delta.affected_buckets}}),
        ),
    ))
}
