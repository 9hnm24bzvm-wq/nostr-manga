//! Namespace-scoped recovery extension for retained native links/device data.
//! This is separate from MangaDex's numeric /legacy/mapping operation.
use super::{PublicApiError, PublicApiResult, PublicUuid};
use crate::AppState;
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{OriginalUri, Request, State},
    http::{StatusCode, header},
    routing::post,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestBody {
    catalog_namespace: PublicUuid,
    resources: Vec<LegacyResource>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LegacyResource {
    kind: String,
    id: String,
}
pub fn router() -> Router<AppState> {
    Router::new().route("/legacy/resolve", post(resolve))
}
async fn resolve(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    if uri.query().is_some() {
        return Err(PublicApiError::bad_request(
            "Legacy resolution accepts no query",
        ));
    }
    if request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_none_or(|v| !v.trim().eq_ignore_ascii_case("application/json"))
    {
        return Err(PublicApiError::bad_request(
            "Content-Type must be application/json",
        ));
    }
    let body = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        to_bytes(request.into_body(), 64 * 1024),
    )
    .await
    .map_err(|_| PublicApiError::bad_request("Body timeout"))?
    .map_err(|_| PublicApiError::bad_request("Body exceeds64KiB"))?;
    let payload: RequestBody = serde_json::from_slice(&body)
        .map_err(|_| PublicApiError::bad_request("Invalid namespace-scoped legacy request"))?;
    if payload.resources.len() > 256 {
        return Err(PublicApiError::bad_request(
            "At most256 resources are allowed",
        ));
    }
    let mut seen = BTreeSet::new();
    for resource in &payload.resources {
        if !["manga", "chapter", "cover", "author", "group"].contains(&resource.kind.as_str())
            || resource.id.is_empty()
            || resource.id.len() > 128
            || resource
                .id
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\'))
            || !seen.insert((resource.kind.clone(), resource.id.clone()))
        {
            return Err(PublicApiError::bad_request(
                "Invalid or duplicate legacy resource",
            ));
        }
    }
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    tx.batch_execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await?;
    let namespace: String = tx
        .query_one(
            "SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton",
            &[],
        )
        .await?
        .get(0);
    if payload.catalog_namespace.as_str() != namespace {
        return Err(PublicApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Catalogue Namespace Mismatch",
            "Legacy identities belong to a different catalogue namespace",
        ));
    }
    let resources = Value::Array(
        payload
            .resources
            .iter()
            .map(|r| json!({"kind":r.kind,"id":r.id,"public_id":PublicUuid::parse(&r.id).ok()}))
            .collect(),
    );
    let rows=tx.query("SELECT r.kind,r.id,canonical.public_id::text FROM jsonb_to_recordset($1::jsonb)r(kind text,id text,public_id text) JOIN platform.public_resource_ids a ON a.kind=r.kind AND a.principal_type='native' AND CASE WHEN r.public_id IS NOT NULL THEN a.public_id=r.public_id::uuid ELSE a.native_id=r.id AND a.preferred END JOIN platform.public_resource_ids canonical ON canonical.kind=a.kind AND canonical.native_id=a.native_id AND canonical.preferred WHERE CASE r.kind WHEN 'manga' THEN EXISTS(SELECT 1 FROM platform.catalog_manga m WHERE m.id=a.native_id AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)) WHEN 'chapter' THEN EXISTS(SELECT 1 FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=a.native_id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)) WHEN 'cover' THEN EXISTS(SELECT 1 FROM platform.catalog_covers c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=a.native_id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)) WHEN 'author' THEN EXISTS(SELECT 1 FROM platform.catalog_authors author WHERE author.id=a.native_id AND author.deleted_at IS NULL AND platform.protocol_source_is_visible(author.protocol_event_id)) WHEN 'group' THEN EXISTS(SELECT 1 FROM platform.catalog_groups g WHERE g.id=a.native_id AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id)) ELSE false END",&[&resources]).await?;
    let mapped = rows
        .into_iter()
        .map(|r| {
            let id: String = r.get(2);
            Ok((
                (r.get::<_, String>(0), r.get::<_, String>(1)),
                PublicUuid::parse(&id).map_err(|_| PublicApiError::unavailable())?,
            ))
        })
        .collect::<PublicApiResult<BTreeMap<_, _>>>()?;
    let mut data = vec![];
    let mut unresolved = vec![];
    for resource in payload.resources {
        if let Some(id) = mapped.get(&(resource.kind.clone(), resource.id.clone())) {
            data.push(json!({"kind":resource.kind,"legacy_id":resource.id,"id":id}));
        } else {
            unresolved.push(resource);
        }
    }
    tx.commit().await?;
    Ok(Json(
        json!({"result":"ok","data":data,"unresolved":unresolved}),
    ))
}
