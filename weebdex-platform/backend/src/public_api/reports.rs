//! MangaDex report transport for private instance-local moderation. Every route
//! requires a genuine request-scoped Nostr proof and explicit native binding.
//! Nothing here signs/publishes report data or grants global moderator authority.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdTarget, PublicUuid,
    ResourceKind,
    auth::{BoundAccountWrite, admitted_request, bound_account_read, bound_account_write},
    dto::{CollectionResponse, Relationship, Resource, ResourceType},
    query::{Direction, PublicQuery, QuerySchema},
};
use crate::{
    domains::{catalog::query::Sql, moderation as core},
    state::AppState,
};
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{OriginalUri, Path, Request, State},
    http::{HeaderMap, StatusCode, header},
    routing::get,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

const CATEGORIES: &[&str] = &["manga", "chapter", "scanlation_group", "user", "author"];
const STATUSES: &[&str] = &["waiting", "accepted", "refused", "autoresolved"];
const BODY_LIMIT: usize = 8 * 1024;
const QUERY: QuerySchema<'static> = QuerySchema {
    default_limit: 10,
    max_limit: 100,
    max_offset: 10_000,
    scalar: &["category", "reasonId", "objectId", "status", "scope", "expectedAccountId"],
    arrays: &[],
    includes: &["user", "reason"],
    order: &["createdAt"],
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/report", get(list).post(create))
        .route("/report/reasons/{category}", get(reasons))
}
fn kind(category: &str) -> PublicApiResult<ResourceKind> {
    match category {
        "manga" => Ok(ResourceKind::Manga),
        "chapter" => Ok(ResourceKind::Chapter),
        "scanlation_group" => Ok(ResourceKind::Group),
        "user" => Ok(ResourceKind::User),
        "author" => Ok(ResourceKind::Author),
        _ => Err(PublicApiError::bad_request("Unsupported report category")),
    }
}
async fn bytes(request: Request, read: bool) -> PublicApiResult<Vec<u8>> {
    let body = tokio::time::timeout(
        Duration::from_secs(5),
        to_bytes(request.into_body(), BODY_LIMIT),
    )
    .await
    .map_err(|_| PublicApiError::bad_request("Request body timeout"))?
    .map_err(|_| PublicApiError::bad_request("Request body exceeds 8KB"))?;
    if read && !body.is_empty() {
        return Err(PublicApiError::bad_request(
            "Report reads do not accept a request body",
        ));
    }
    Ok(body.to_vec())
}
fn uuid(value: Option<String>) -> PublicApiResult<PublicUuid> {
    value
        .and_then(|value| PublicUuid::parse(&value).ok())
        .ok_or_else(|| crate::error::ApiError::unavailable().into())
}
fn reason_attributes(reason: String, required: bool, category: String, version: i32) -> Value {
    json!({"reason":{"en":reason},"detailsRequired":required,"category":category,"version":version})
}
async fn reasons(
    State(state): State<AppState>,
    Path(category): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<CollectionResponse<Value>>> {
    kind(&category)?;
    if uri.query().is_some() {
        return Err(PublicApiError::bad_request(
            "Report reasons accept no query parameters",
        ));
    }
    let method = request.method().to_string();
    let body = bytes(request, true).await?;
    let cap = admitted_request(&state, &uri, &headers, &method, &body).await?;
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    bound_account_read(&tx, &cap).await?;
    let rows = tx.query("SELECT a.public_id::text,r.reason,r.details_required,r.category,r.version FROM platform.report_reasons r LEFT JOIN platform.public_resource_ids a ON a.kind='report_reason' AND a.native_id=r.id AND a.preferred WHERE r.category=$1 AND r.enabled ORDER BY r.reason,r.id LIMIT 1001", &[&category]).await?;
    if rows.len() > 1000 {
        return Err(crate::error::ApiError::unavailable().into());
    }
    let data = rows.iter().map(|row| Ok(json!({"id":uuid(row.get(0))?,"type":"report_reason","attributes":reason_attributes(row.get(1),row.get(2),row.get(3),row.get(4))})))
        .collect::<PublicApiResult<Vec<_>>>()?;
    tx.commit().await?;
    let count = data.len() as u32;
    Ok(Json(CollectionResponse::new(
        data,
        count,
        0,
        u64::from(count),
    )))
}
fn query(raw: Option<&str>) -> PublicApiResult<PublicQuery> {
    let mut query = PublicQuery::parse(raw, &QUERY)?;
    if query.limit == 0 {
        return Err(PublicApiError::bad_request(
            "Report limit must be between 1 and 100",
        ));
    }
    if let Some(category) = query.first("category") {
        kind(category)?;
    }
    if query.first("scope").is_some_and(|scope| scope != "mine") {
        return Err(PublicApiError::bad_request("Invalid report scope"));
    }
    if query
        .first("status")
        .is_some_and(|status| !STATUSES.contains(&status))
    {
        return Err(PublicApiError::bad_request("Invalid report status"));
    }
    for field in ["reasonId", "objectId", "expectedAccountId"] {
        if let Some(value) = query.first(field) {
            PublicUuid::parse(value)?;
        }
    }
    if query.order.is_empty() {
        query.order.push(("createdAt".into(), Direction::Desc));
    }
    Ok(query)
}
async fn list(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<CollectionResponse<Resource>>> {
    let query = query(uri.query())?;
    let method = request.method().to_string();
    let body = bytes(request, true).await?;
    let cap = admitted_request(&state, &uri, &headers, &method, &body).await?;
    let mut client = state.db.client().await?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = bound_account_read(&tx, &cap).await?;
    // The captured account is a stale-read fence, never an authority selector.
    if let Some(expected) = query.first("expectedAccountId") {
        let expected = PublicUuid::parse(expected)?;
        let current = super::uploads::preferred(&tx, ResourceKind::User, actor.user_id()).await?;
        if expected != current { return Err(PublicApiError::conflict("The selected signer is bound to another account; reload report access")); }
    }
    let mut sql = Sql::default();
    // Preserve the native instance moderation queue permission boundary. Native
    // thread/reply reports have no counterpart in the pinned public categories.
    sql.conditions.push("r.deleted_at IS NULL AND r.entity_type IN ('manga','chapter','scanlation_group','user','author')".into());
    // Instance extension: explicit personal inspection narrows even a moderator
    // to the transaction-validated actor. No client-supplied owner is trusted.
    if query.first("scope") == Some("mine")
        || !(actor.is_moderator() && actor.require_permission("report.update").is_ok())
    {
        let owner = sql.bind(actor.user_id().to_owned());
        sql.conditions.push(format!("r.submitter_id={owner}"));
    }
    if let Some(category) = query.first("category") {
        let value = sql.bind(category.to_owned());
        sql.conditions.push(format!("r.entity_type={value}"));
    }
    if let Some(status) = query.first("status") {
        let value = sql.bind(status.to_owned());
        sql.conditions.push(format!("r.status={value}"));
    }
    if let Some(reason) = query.first("reasonId") {
        let id = PublicUuid::parse(reason)?;
        let resolved = PostgresPublicIdResolver::resolve_many_in(
            &*tx,
            ResourceKind::ReportReason,
            std::slice::from_ref(&id),
        )
        .await?
        .remove(&id);
        if let Some(reason) = resolved {
            let value = sql.bind(reason.native_id);
            sql.conditions.push(format!("r.reason_id={value}"));
        } else {
            sql.conditions.push("FALSE".into());
        }
    }
    if let Some(object) = query.first("objectId") {
        let id = PublicUuid::parse(object)?;
        let mut alternatives = Vec::new();
        for category in CATEGORIES.iter().copied().filter(|category| {
            query
                .first("category")
                .is_none_or(|wanted| wanted == *category)
        }) {
            let resolved = PostgresPublicIdResolver::resolve_many_in(
                &*tx,
                kind(category)?,
                std::slice::from_ref(&id),
            )
            .await?
            .remove(&id);
            if let Some(target) =
                resolved.filter(|target| target.target == PublicIdTarget::NativeResource)
            {
                let category = sql.bind(category.to_owned());
                let target = sql.bind(target.native_id);
                alternatives.push(format!(
                    "(r.entity_type={category} AND r.entity_id={target})"
                ));
            }
        }
        sql.conditions.push(if alternatives.is_empty() {
            "FALSE".into()
        } else {
            format!("({})", alternatives.join(" OR "))
        });
    }
    let predicate = sql.predicate();
    let total: i64 = tx
        .query_one(
            &format!("SELECT count(*) FROM platform.reports r WHERE {predicate}"),
            &sql.params(),
        )
        .await?
        .get(0);
    let limit = sql.bind(i64::from(query.limit));
    let offset = sql.bind(i64::from(query.offset));
    let order = query.order[0].1.sql();
    // One bounded join, no report_json or per-row expansion/resolver calls. A
    // missing persisted alias is an availability error, never a made-up UUID.
    let rows = tx.query(&format!("SELECT rp.public_id::text,oa.public_id::text,r.body,r.status,platform.catalog_timestamp(r.created_at),ua.public_id::text,CASE WHEN NOT u.disabled THEN jsonb_build_object('username',u.username,'roles',u.roles,'version',u.version) END,ra.public_id::text,rr.reason,rr.details_required,rr.category,rr.version FROM platform.reports r JOIN platform.report_reasons rr ON rr.id=r.reason_id JOIN platform.users u ON u.id=r.submitter_id LEFT JOIN platform.public_resource_ids rp ON rp.kind='report' AND rp.native_id=r.id AND rp.preferred LEFT JOIN platform.public_resource_ids oa ON oa.kind=CASE r.entity_type WHEN 'scanlation_group' THEN 'group' ELSE r.entity_type END AND oa.native_id=r.entity_id AND oa.preferred LEFT JOIN platform.public_resource_ids ua ON ua.kind='user' AND ua.native_id=r.submitter_id AND ua.preferred LEFT JOIN platform.public_resource_ids ra ON ra.kind='report_reason' AND ra.native_id=r.reason_id AND ra.preferred WHERE {predicate} ORDER BY r.created_at {order},r.id {order} LIMIT {limit} OFFSET {offset}"), &sql.params()).await?;
    let data = rows.into_iter().map(|row| {
        let mut relationships = vec![Relationship { id: uuid(row.get(5))?, resource_type: ResourceType::User, related: None, attributes: if query.includes.contains("user") { row.get(6) } else { None } }];
        relationships.push(Relationship { id: uuid(row.get(7))?, resource_type: ResourceType::ReportReason, related: None, attributes: query.includes.contains("reason").then(|| reason_attributes(row.get(8),row.get(9),row.get(10),row.get(11))) });
        Ok(Resource { id: uuid(row.get(0))?, resource_type: ResourceType::Report, attributes: json!({"details":row.get::<_,String>(2),"objectId":uuid(row.get(1))?,"status":row.get::<_,String>(3),"createdAt":row.get::<_,String>(4)}), relationships })
    }).collect::<PublicApiResult<Vec<_>>>()?;
    tx.commit().await?;
    Ok(Json(CollectionResponse::new(
        data,
        query.limit,
        query.offset,
        total as u64,
    )))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CreateReport {
    category: String,
    reason: PublicUuid,
    object_id: PublicUuid,
    #[serde(default)]
    details: String,
    #[serde(default,deserialize_with="super::uploads::deserialize_optional_account")]
    expected_account_id: Option<PublicUuid>,
    // Optional compatibility extension. The signed UI always pins the exact
    // reason the user reviewed, not merely its stable public identifier.
    #[serde(default, deserialize_with="deserialize_reason_version")]
    expected_reason_version: Option<i32>,
}
fn deserialize_reason_version<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<i32>, D::Error> {
    let version = i32::deserialize(deserializer)?;
    if version < 1 { return Err(serde::de::Error::custom("Expected positive reason version")); }
    Ok(Some(version))
}
fn check_reason_version(actual: i32, expected: i32) -> PublicApiResult<()> {
    if actual != expected { return Err(PublicApiError::conflict("Report reason changed. Reload reasons before reporting.")); }
    Ok(())
}
impl core::sealed::Sealed for BoundAccountWrite {}
impl core::ReportMutationActor for BoundAccountWrite {
    fn user_id(&self) -> &str {
        BoundAccountWrite::user_id(self)
    }
    fn require_permission(&self, permission: &str) -> crate::error::ApiResult<()> {
        BoundAccountWrite::require_permission(self, permission)
    }
}
async fn create(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<(StatusCode, Json<Value>)> {
    if uri.query().is_some() {
        return Err(PublicApiError::bad_request(
            "Report creation accepts no query parameters",
        ));
    }
    if headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_none_or(|value| !value.trim().eq_ignore_ascii_case("application/json"))
    {
        return Err(PublicApiError::bad_request(
            "Content-Type must be application/json",
        ));
    }
    let method = request.method().to_string();
    let body = bytes(request, false).await?;
    let cap = admitted_request(&state, &uri, &headers, &method, &body).await?;
    let payload: CreateReport = serde_json::from_slice(&body).map_err(|_| {
        PublicApiError::bad_request(
            "A valid report object with category, reason UUID and objectId UUID is required",
        )
    })?;
    let target_kind = kind(&payload.category)?;
    if payload.details.contains('\0') {
        return Err(PublicApiError::bad_request("Invalid report details"));
    }
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let actor = bound_account_write(&tx, &cap).await?;
    let expected=payload.expected_account_id.as_ref().map(|id|json!(id));
    super::editorial::expected_account(&tx,&actor,expected.as_ref()).await?;
    actor.require_permission("report.create")?;
    let reason = PostgresPublicIdResolver::resolve_many_in(
        &*tx,
        ResourceKind::ReportReason,
        std::slice::from_ref(&payload.reason),
    )
    .await?
    .remove(&payload.reason)
    .ok_or_else(|| PublicApiError::bad_request("Reason does not belong to this report category"))?;
    if let Some(expected) = payload.expected_reason_version {
        // Hold this lock through core validation and insert; an administrative
        // reason edit cannot change text/required-details after this check.
        let actual = tx.query_opt("SELECT version FROM platform.report_reasons WHERE id=$1 FOR SHARE", &[&reason.native_id]).await?
            .ok_or_else(|| PublicApiError::bad_request("Report reason is unavailable"))?;
        check_reason_version(actual.get(0), expected)?;
    }
    let target = PostgresPublicIdResolver::resolve_many_in(
        &*tx,
        target_kind,
        std::slice::from_ref(&payload.object_id),
    )
    .await?
    .remove(&payload.object_id)
    .filter(|target| target.target == PublicIdTarget::NativeResource)
    .ok_or_else(PublicApiError::not_found)?;
    // The public catalogue also honors its persisted withdrawal cache. Keep
    // the parent row locked until commit so a local withdrawal cannot race the
    // report insert. Native operator moderation keeps its own broader policy.
    let visibility_sql = match target_kind {
        ResourceKind::Manga => Some(
            "SELECT 1 FROM platform.catalog_manga m WHERE m.id=$1 AND m.protocol_source_visible AND platform.report_entity_exists('manga',m.id) FOR SHARE OF m",
        ),
        ResourceKind::Chapter => Some(
            "SELECT 1 FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=$1 AND m.protocol_source_visible AND platform.report_entity_exists('chapter',c.id) FOR SHARE OF c,m",
        ),
        // Authors use the live source helper; groups use the original group
        // visibility helper. Neither resource has a cached withdrawal column.
        _ => None,
    };
    if let Some(sql) = visibility_sql {
        if tx.query_opt(sql, &[&target.native_id]).await?.is_none() {
            return Err(PublicApiError::not_found());
        }
    }
    core::create_report(
        &tx,
        &actor,
        &payload.category,
        &target.native_id,
        &reason.native_id,
        payload.details.trim(),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(json!({"result":"ok"}))))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_report_contract_and_bounds() {
        for value in [
            "scope=all",
            "expectedAccountId=native",
            "expectedAccountId=11111111-1111-8111-8111-111111111111&expectedAccountId=11111111-1111-8111-111111111111",
            "scope=",
            "scope=mine&scope=mine",
            "scope=33333333-3333-8333-8333-333333333333",
            "limit=0",
            "limit=101",
            "offset=10001",
            "status=reviewing",
            "category=reply",
            "reasonId=manga_spam",
            "objectId=abc",
            "order[status]=asc",
            "includes[]=manga",
            "reason=manga_spam",
            "limit=1&limit=2",
        ] {
            assert!(query(Some(value)).is_err(), "{value}");
        }
        assert_eq!(query(Some("scope=mine")).unwrap().first("scope"), Some("mine"));
        assert_eq!(query(Some("expectedAccountId=11111111-1111-8111-8111-111111111111")).unwrap().first("expectedAccountId"), Some("11111111-1111-8111-8111-111111111111"));
        let q = query(Some(
            "offset=7&limit=1&order[createdAt]=asc&includes[]=reason",
        ))
        .unwrap();
        assert_eq!((q.limit, q.offset), (1, 7));
        assert_eq!(q.order[0].1, Direction::Asc);
        let valid = r#"{"category":"manga","reason":"11111111-1111-8111-8111-111111111111","objectId":"22222222-2222-8222-8222-222222222222"}"#;
        assert!(serde_json::from_str::<CreateReport>(valid).is_ok());
        let pinned=valid.replace('}', ",\"expectedReasonVersion\":3}");
        assert_eq!(serde_json::from_str::<CreateReport>(&pinned).unwrap().expected_reason_version, Some(3));
        for value in ["null", "false", "[]", "\"3\"", "0", "-1", "1.5", "2147483648"] {
            assert!(serde_json::from_str::<CreateReport>(&valid.replace('}', &format!(",\"expectedReasonVersion\":{value}}}"))).is_err());
        }
        assert!(serde_json::from_str::<CreateReport>(&pinned.replace('}', ",\"expectedReasonVersion\":3}")).is_err());
        assert!(check_reason_version(3,3).is_ok());
        assert!(check_reason_version(4,3).is_err());
        let captured=valid.replace('}',",\"expectedAccountId\":\"33333333-3333-8333-8333-333333333333\"}");
        assert!(serde_json::from_str::<CreateReport>(&captured).unwrap().expected_account_id.is_some());
        for field in ["null","false","[]","\"native\""] {
            assert!(serde_json::from_str::<CreateReport>(&valid.replace('}',&format!(",\"expectedAccountId\":{field}}}"))).is_err());
        }
        assert!(serde_json::from_str::<CreateReport>(&captured.replace('}',",\"expectedAccountId\":\"33333333-3333-8333-8333-333333333333\"}")).is_err());
        for invalid in [
            valid.replace("\"manga\"", "null"),
            valid.replace(
                "\"category\":\"manga\"",
                "\"category\":\"manga\",\"category\":\"user\"",
            ),
            valid.replace("\"objectId\"", "\"entity_id\""),
            valid.replace("11111111-1111-8111-8111-111111111111", "manga_spam"),
            valid.replace('}', ",\"version\":1}"),
        ] {
            assert!(
                serde_json::from_str::<CreateReport>(&invalid).is_err(),
                "{invalid}"
            );
        }
    }
}
