//! Narrow owner-only native report projection. Never expands report_json, a
//! moderator queue, the reported content, reviewer notes, or an audit record.
use super::{REPORT_STATUSES, sealed};
use crate::{domains::catalog::{query::Sql, validation as v}, error::{ApiError, ApiResult}};
use serde_json::{Value, json};
use tokio_postgres::Transaction;

pub(crate) const PAGE_SIZE: i64 = 5;
pub(crate) const MAX_PAGE: usize = 2001;
pub(crate) trait PersonalReportReader: sealed::Sealed {
    fn user_id(&self) -> &str;
}
#[derive(Default)]
pub(crate) struct Filters {
    pub category: Option<String>,
    pub status: Option<String>,
    pub object_id: Option<String>,
    pub reason_id: Option<String>,
    pub report_id: Option<String>,
    pub page: usize,
}
/// Caller owns authentication, captured-account comparison and the repeatable
/// read transaction. Ownership cannot be selected by a query or moderator role.
pub(crate) async fn read(
    tx: &Transaction<'_>, actor: &impl PersonalReportReader, filters: &Filters,
) -> ApiResult<(i64, Vec<Value>)> {
    if !(1..=MAX_PAGE).contains(&filters.page) { return Err(ApiError::bad_request("Invalid report page")); }
    let mut sql = Sql::default();
    let owner = sql.bind(actor.user_id().to_owned());
    sql.conditions.push(format!("r.submitter_id={owner} AND r.deleted_at IS NULL AND r.entity_type IN ('thread','reply')"));
    for (column, value) in [
        ("entity_type", filters.category.as_ref()), ("status", filters.status.as_ref()),
        ("entity_id", filters.object_id.as_ref()), ("reason_id", filters.reason_id.as_ref()),
        ("id", filters.report_id.as_ref()),
    ] {
        if let Some(value) = value {
            match column {
                "entity_type" if !["thread", "reply"].contains(&value.as_str()) => return Err(ApiError::bad_request("Invalid native report category")),
                "status" if !REPORT_STATUSES.contains(&value.as_str()) => return Err(ApiError::bad_request("Invalid report status")),
                "entity_type" | "status" => {},
                _ => v::id(value)?,
            }
            let parameter = sql.bind(value.clone());
            sql.conditions.push(format!("r.{column}={parameter}"));
        }
    }
    let predicate = sql.predicate();
    let total: i64 = tx.query_one(&format!("SELECT count(*) FROM platform.reports r WHERE {predicate}"), &sql.params()).await?.get(0);
    if total < 0 || total > 9_007_199_254_740_991 { return Err(ApiError::unavailable()); }
    let limit = sql.bind(if filters.report_id.is_some() { 1_i64 } else { PAGE_SIZE });
    let offset = sql.bind((filters.page as i64 - 1) * PAGE_SIZE);
    // Guard all variable-width persisted TEXT before the driver materializes it.
    // LEFT JOIN preserves malformed/missing reason rows so the entire selected
    // page fails closed instead of hiding rows or disagreeing with its count.
    let rows = tx.query(&format!(r#"SELECT
        CASE WHEN octet_length(r.id) BETWEEN 1 AND 128 THEN r.id END,
        r.entity_type,
        CASE WHEN octet_length(r.entity_id) BETWEEN 1 AND 128 THEN r.entity_id END,
        r.status,
        to_char(r.created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
        CASE WHEN octet_length(r.body)<=20000 THEN r.body END,
        CASE WHEN octet_length(rr.id) BETWEEN 1 AND 128 THEN rr.id END,
        CASE WHEN octet_length(rr.reason) BETWEEN 1 AND 8000 THEN rr.reason END,
        rr.version,rr.details_required,rr.category
        FROM platform.reports r LEFT JOIN platform.report_reasons rr ON rr.id=r.reason_id
        WHERE {predicate} ORDER BY r.created_at DESC,r.id COLLATE "C" DESC LIMIT {limit} OFFSET {offset}"#), &sql.params()).await?;
    let mut data = Vec::with_capacity(rows.len());
    for row in rows {
        let bounded = |column| row.get::<_, Option<String>>(column).ok_or_else(ApiError::unavailable);
        let id = bounded(0)?;
        let category: String = row.get(1);
        let object = bounded(2)?;
        let status: String = row.get(3);
        let details = bounded(5)?;
        let reason_id = bounded(6)?;
        let reason = bounded(7)?;
        for value in [&id, &object, &reason_id] { v::id(value).map_err(|_| ApiError::unavailable())?; }
        let version = row.get::<_, Option<i32>>(8).filter(|v| *v > 0).ok_or_else(ApiError::unavailable)?;
        let required = row.get::<_, Option<bool>>(9).ok_or_else(ApiError::unavailable)?;
        if row.get::<_, Option<String>>(10).as_deref() != Some(category.as_str()) || !REPORT_STATUSES.contains(&status.as_str()) {
            return Err(ApiError::unavailable());
        }
        data.push(json!({"id":id,"category":category,"objectId":object,"status":status,"createdAt":row.get::<_,String>(4),"details":details,
            "reason":{"id":reason_id,"reason":reason,"version":version,"detailsRequired":required}}));
    }
    Ok((total, data))
}
