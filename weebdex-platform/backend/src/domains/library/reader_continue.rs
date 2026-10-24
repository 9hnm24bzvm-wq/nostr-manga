//! Exact native-account Continue in one authority-fenced catalogue snapshot.
//! This accepts filters, never device history, and never changes completion.
use super::validation;
use crate::{
    AppState,
    domains::{auth, catalog::{
        query::{QueryParams, Sql, chapter_filters, manga_filters},
        reader_selector::{CatalogIdentity, native_continue_identity, deadline, native_continue,
            native_continue_database_error, validate_wire},
    }},
    error::{ApiError, ApiResult},
};
use axum::{Json, body::HttpBody, extract::{Path, RawQuery, Request as HttpRequest, State}, http::{HeaderMap, StatusCode}};
use serde_json::Value;
use std::{collections::BTreeMap, time::Duration};

const MAX_QUERY_BYTES: usize = 16 * 1024;
const MAX_CLIENT_EPOCH: u64 = 9_007_199_254_740_991;
const FILTERS: &[&str] = &["tlang", "tlangx", "uploader", "uploaderx", "group", "groupx", "id", "manga"];

struct Request {
    account_epoch: u64,
    identity: CatalogIdentity,
    request_id: String,
    filters: QueryParams,
}
impl Request {
    fn parse(raw: Option<&str>) -> ApiResult<Self> {
        let raw = raw.unwrap_or("");
        if raw.len() > MAX_QUERY_BYTES { return Err(ApiError::bad_request("Continue query is too long")); }
        validate_wire(raw)?;
        let mut controls = BTreeMap::new();
        let mut filters = url::form_urlencoded::Serializer::new(String::new());
        for (key, value) in url::form_urlencoded::parse(raw.as_bytes()) {
            match key.as_ref() {
                "accountEpoch" | "catalogNamespace" | "serviceKey" | "requestId" => {
                    if value.len() > 64 || controls.insert(key.into_owned(), value.into_owned()).is_some() {
                        return Err(ApiError::bad_request("Invalid or duplicate Continue control"));
                    }
                }
                _ if FILTERS.contains(&key.strip_suffix("[]").unwrap_or(&key)) => { filters.append_pair(&key, &value); }
                _ => return Err(ApiError::bad_request("Unknown Continue control or filter")),
            }
        }
        let required = |key: &str| controls.get(key).map(String::as_str)
            .ok_or_else(|| ApiError::bad_request(format!("{key} is required")));
        let epoch = required("accountEpoch")?;
        let account_epoch = epoch.parse::<u64>().ok()
            .filter(|n| *n <= MAX_CLIENT_EPOCH && n.to_string() == epoch)
            .ok_or_else(|| ApiError::bad_request("Invalid account epoch"))?;
        let identity = CatalogIdentity::expected(required("catalogNamespace")?, required("serviceKey")?)?;
        let request_id = required("requestId")?;
        if request_id.is_empty() || !request_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
            return Err(ApiError::bad_request("Invalid Continue request ID"));
        }
        let filters = QueryParams::parse(Some(filters.finish()))?;
        Ok(Self { account_epoch, identity, request_id: request_id.to_owned(), filters })
    }
}

pub async fn get(
    State(state): State<AppState>, Path(manga): Path<String>,
    RawQuery(raw): RawQuery, request: HttpRequest,
) -> ApiResult<Json<Value>> {
    // Like completion, refuse unfinished bodies WITHOUT polling. A zero size
    // hint or Content-Length is not proof that trailers/body have ended.
    if !request.body().is_end_stream() {
        return Err(ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, "body_not_empty",
            "Continue requires an already-ended empty request body"));
    }
    let headers = request.into_parts().0.headers;
    // Router admission owns one LibraryHead permit. Include pool acquisition,
    // principal locks, catalogue work and serialization in this operation cap.
    tokio::time::timeout(Duration::from_secs(8), get_in(state, manga, raw, headers))
        .await.map_err(|_| ApiError::unavailable())?
}

async fn get_in(state: AppState, manga: String, raw: Option<String>, headers: HeaderMap) -> ApiResult<Json<Value>> {
    validation::id(&manga)?;
    let request = Request::parse(raw.as_deref())?;
    let mut db = state.db.client().await?;
    // SHARE authority locks forbid PostgreSQL READ ONLY. No statement mutates
    // history, progress, sessions, users or catalogue despite writable RR.
    let tx = db.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start().await.map_err(native_continue_database_error)?;
    deadline(&tx).await?;
    let actor = auth::require_native_library_principal_in(&state, &headers, &tx).await?;
    let identity = native_continue_identity(&tx, &state).await?;
    if identity != request.identity {
        return Err(ApiError::conflict("Catalogue identity changed; reload before continuing"));
    }
    let mut sql = Sql::default();
    sql.conditions.push("m.deleted_at IS NULL AND m.state='published'".into());
    manga_filters(&QueryParams { values: BTreeMap::new(), limit: 1, page: 1 }, &mut sql, "m", None)?;
    let parent = sql.bind(manga.clone());
    sql.conditions.push(format!("m.id={parent}"));
    if tx.query_opt(&format!("SELECT m.id FROM platform.catalog_manga m WHERE {}", sql.predicate()), &sql.params())
        .await.map_err(native_continue_database_error)?.is_none() {
        return Err(ApiError::not_found("Manga not found"));
    }
    sql.conditions.push("c.deleted_at IS NULL".into());
    // Exactly the native aggregate vocabulary and eligibility. No public
    // defaults, page limits, caller history or progress.completed substitution.
    chapter_filters(&request.filters, &mut sql, "c")?;
    let value = native_continue(&tx, &identity, &manga, &actor.user_id, request.account_epoch,
        &request.request_id, &request.filters, &mut sql).await?;
    tx.commit().await.map_err(native_continue_database_error)?;
    Ok(Json(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn query() -> String {
        "accountEpoch=7&catalogNamespace=aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa&serviceKey=null&requestId=click_1-A".into()
    }
    #[test]
    fn continue_accepts_only_capture_and_existing_bounded_native_filters() {
        let valid = query();
        let parsed = Request::parse(Some(&valid)).unwrap();
        assert_eq!(parsed.request_id, "click_1-A");
        assert_eq!(parsed.account_epoch, 7);
        for key in FILTERS {
            let value = if key.starts_with("tlang") { "en" } else { "id" };
            let request = Request::parse(Some(&format!("{valid}&{key}={value}&{key}[]={value}"))).unwrap();
            assert_eq!(request.filters.list(key), [value, value]);
        }
        for suffix in ["history=x", "completed=true", "readIds=x", "sort=id", "direction=oldest", "current=c", "groupChapter=c", "cursor=x", "after=x", "offset=0", "page=1", "limit=1", "operation=first", "accountEpoch=7", "requestId=two", "requestId[]=two", "id=%FF", "id=%Q1", "id=%00", "&id=c"] {
            assert!(Request::parse(Some(&format!("{valid}&{suffix}"))).is_err(), "{suffix}");
        }
        for value in ["", "a b", "a/b", "é", &"a".repeat(65)] {
            assert!(Request::parse(Some(&valid.replace("click_1-A", value))).is_err());
        }
        for value in ["-1", "01", "1.0", "9007199254740992"] {
            assert!(Request::parse(Some(&valid.replace("accountEpoch=7", &format!("accountEpoch={value}")))).is_err());
        }
        for pair in valid.split('&') {
            let without = valid.split('&').filter(|field| *field != pair).collect::<Vec<_>>().join("&");
            assert!(Request::parse(Some(&without)).is_err());
        }
        assert!(Request::parse(Some(&format!("{valid}{}", "&id=c".repeat(101)))).is_err());
        assert!(Request::parse(Some(&"x".repeat(MAX_QUERY_BYTES + 1))).is_err());
    }
}
