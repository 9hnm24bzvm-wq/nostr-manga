//! Bounded native-account history membership, not a history upload or map.
//! One request is coherent; repeated calls are live observations, not a proof
//! of globally oldest unread/all-read under catalogue or history drift.
use super::validation;
use crate::{
    AppState,
    domains::{auth, catalog::{
        query::{QueryParams, Sql, chapter_filters, manga_filters},
        reader_selector::{CatalogIdentity, catalog_identity, deadline, validate_wire},
    }},
    error::{ApiError, ApiResult},
};
use axum::{Json, body::HttpBody, extract::{Path, RawQuery, Request as HttpRequest, State}, http::{HeaderMap, StatusCode}};
use serde_json::{Value, json};
use std::{collections::{BTreeMap, BTreeSet}, time::Duration};

const MAX_IDS: usize = 50;
const MAX_QUERY_BYTES: usize = 24 * 1024;
const MAX_RESPONSE_BYTES: usize = 16 * 1024;
const MAX_CLIENT_EPOCH: u64 = 9_007_199_254_740_991;

struct Request {
    ids: Vec<String>,
    account_epoch: u64,
    identity: CatalogIdentity,
}
impl Request {
    fn parse(raw: Option<&str>) -> ApiResult<Self> {
        let raw = raw.unwrap_or("");
        if raw.len() > MAX_QUERY_BYTES {
            return Err(ApiError::bad_request("Completion query is too long; request fewer chapters"));
        }
        validate_wire(raw)?;
        let mut ids = Vec::new();
        let mut seen = BTreeSet::new();
        let mut controls = BTreeMap::new();
        for (key, value) in url::form_urlencoded::parse(raw.as_bytes()) {
            match key.as_ref() {
                "id" => {
                    validation::id(&value)?;
                    if ids.len() >= MAX_IDS || !seen.insert(value.to_string()) {
                        return Err(ApiError::bad_request("Request 1..50 distinct chapter IDs"));
                    }
                    ids.push(value.into_owned());
                }
                "accountEpoch" | "catalogNamespace" | "serviceKey" => {
                    if value.len() > 64 || controls.insert(key.into_owned(), value.into_owned()).is_some() {
                        return Err(ApiError::bad_request("Invalid or duplicate completion control"));
                    }
                }
                _ => return Err(ApiError::bad_request("Unknown completion control")),
            }
        }
        if ids.is_empty() { return Err(ApiError::bad_request("Request 1..50 distinct chapter IDs")); }
        let required = |key: &str| controls.get(key).map(String::as_str)
            .ok_or_else(|| ApiError::bad_request(format!("{key} is required")));
        let epoch = required("accountEpoch")?;
        let account_epoch = epoch.parse::<u64>().ok()
            .filter(|n| *n <= MAX_CLIENT_EPOCH && n.to_string() == epoch)
            .ok_or_else(|| ApiError::bad_request("Invalid account epoch"))?;
        let identity = CatalogIdentity::expected(required("catalogNamespace")?, required("serviceKey")?)?;
        Ok(Self { ids, account_epoch, identity })
    }
}

fn envelope(manga: &str, identity: &CatalogIdentity, account: &str, epoch: u64, data: Vec<Value>) -> ApiResult<Value> {
    let value = json!({
        "version": 1, "mangaId": manga, "catalogIdentity": identity,
        "account": {"id": account, "epoch": epoch},
        "consistency": "live-request-v1", "continueProof": "observed-only", "data": data,
    });
    if serde_json::to_vec(&value).map_err(|_| ApiError::unavailable())?.len() > MAX_RESPONSE_BYTES {
        // Never truncate exact coverage or turn unreturned keys into unread.
        return Err(ApiError::unavailable());
    }
    Ok(value)
}

pub async fn get(
    State(state): State<AppState>, Path(manga): Path<String>,
    RawQuery(raw): RawQuery, request: HttpRequest,
) -> ApiResult<Json<Value>> {
    // GET is deliberately not wrapped by the shared intake deadline. Do not
    // extract Bytes: even a zero-byte cap can await a no-frame stream forever
    // before reaching this handler. Inspect the actual body, not Content-Length
    // or size_hint, and refuse anything not already ended without polling it.
    if !request.body().is_end_stream() {
        return Err(ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, "body_not_empty",
            "Completion lookup requires an already-ended empty request body"));
    }
    let headers = request.into_parts().0.headers;
    // The ended body has now been dropped. Router admission still owns exactly
    // one LibraryHead permit; all remaining work is inside this deadline.
    tokio::time::timeout(Duration::from_secs(8), get_in(state, manga, raw, headers))
        .await.map_err(|_| ApiError::unavailable())?
}

async fn get_in(state: AppState, manga: String, raw: Option<String>, headers: HeaderMap) -> ApiResult<Json<Value>> {
    validation::id(&manga)?;
    let request = Request::parse(raw.as_deref())?;
    let mut db = state.db.client().await?;
    // This performs no durable writes. READ ONLY would prohibit the authority
    // row locks, so use a writable RR transaction with no mutation statements.
    let tx = db.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).start().await?;
    deadline(&tx).await?;
    let actor = auth::require_native_library_principal_in(&state, &headers, &tx).await?;
    let identity = catalog_identity(&tx, &state).await?;
    if identity != request.identity {
        return Err(ApiError::conflict("Catalogue identity changed; reload before continuing"));
    }
    // Reuse native eligibility. Do not inherit public availability/content/
    // future-time defaults or the user's current frontend filter selection.
    let empty = QueryParams { values: BTreeMap::new(), limit: 1, page: 1 };
    let mut sql = Sql::default();
    sql.conditions.push("m.deleted_at IS NULL AND m.state='published'".into());
    manga_filters(&empty, &mut sql, "m", None)?;
    let parent = sql.bind(manga.clone());
    sql.conditions.push(format!("m.id={parent}"));
    if tx.query_opt(&format!("SELECT m.id FROM platform.catalog_manga m WHERE {}", sql.predicate()), &sql.params()).await?.is_none() {
        return Err(ApiError::not_found("Manga not found"));
    }
    sql.conditions.push("c.deleted_at IS NULL".into());
    chapter_filters(&empty, &mut sql, "c")?;
    let ids = sql.bind(request.ids.clone());
    let account = sql.bind(actor.user_id.clone());
    let rows = tx.query(&format!(
        "SELECT c.id,EXISTS(SELECT 1 FROM platform.library_chapter_history h WHERE h.user_id={account} AND h.manga_id=m.id AND h.chapter_id=c.id) \
         FROM unnest({ids}::TEXT[]) WITH ORDINALITY requested(id,ordinal) \
         JOIN platform.catalog_chapters c ON c.id=requested.id \
         JOIN platform.catalog_manga m ON m.id=c.manga_id \
         WHERE {} ORDER BY requested.ordinal LIMIT 51", sql.predicate()), &sql.params()).await?;
    if rows.len() != request.ids.len() {
        return Err(ApiError::conflict("Requested chapters changed or are not visible in this manga"));
    }
    let mut data = Vec::with_capacity(rows.len());
    for (row, expected) in rows.iter().zip(&request.ids) {
        let id: String = row.get(0);
        if &id != expected { return Err(ApiError::unavailable()); }
        data.push(json!({"id": id, "completed": row.get::<_, bool>(1)}));
    }
    let value = envelope(&manga, &identity, &actor.user_id, request.account_epoch, data)?;
    tx.commit().await?;
    Ok(Json(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    const NAMESPACE: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    fn query(ids: &[String], epoch: &str, service_key: &str) -> String {
        let mut out = url::form_urlencoded::Serializer::new(String::new());
        out.append_pair("accountEpoch", epoch).append_pair("catalogNamespace", NAMESPACE).append_pair("serviceKey", service_key);
        for id in ids { out.append_pair("id", id); }
        out.finish()
    }
    #[test]
    fn completion_controls_are_exact_bounded_and_not_uploaded_history() {
        let valid = query(&["chapter".into()], "0", "null");
        assert_eq!(Request::parse(Some(&valid)).unwrap().ids, ["chapter"]);
        for suffix in ["&id=chapter", "&id[]=other", "&readIds=chapter", "&completed=true", "&offset=0", "&accountEpoch=0", "&serviceKey=null", "&catalogNamespace=bad", "&&id=other", "&id=%FF", "&id=%00", "&id=%Q1"] {
            assert!(Request::parse(Some(&format!("{valid}{suffix}"))).is_err(), "{suffix}");
        }
        for epoch in ["", "-1", "+1", "01", "1.0", "1e3", "9007199254740992"] {
            assert!(Request::parse(Some(&query(&["c".into()], epoch, "null"))).is_err(), "{epoch}");
        }
        for key in ["", "NULL", "none", &"A".repeat(64), &"a".repeat(63)] {
            assert!(Request::parse(Some(&query(&["c".into()], "1", key))).is_err());
        }
        for removed in ["accountEpoch=0&", &format!("catalogNamespace={NAMESPACE}&"), "serviceKey=null&"] {
            assert!(Request::parse(Some(&valid.replace(removed, ""))).is_err());
        }
        assert!(Request::parse(Some(&valid.replace(NAMESPACE, &NAMESPACE.to_uppercase()))).is_err());
        assert!(Request::parse(Some(&query(&[], "0", "null"))).is_err());
        assert!(Request::parse(Some(&query(&(0..51).map(|n| format!("c{n}")).collect::<Vec<_>>(), "0", "null"))).is_err());
        assert!(Request::parse(Some(&"x".repeat(MAX_QUERY_BYTES + 1))).is_err());
    }
    #[test]
    fn fifty_maximum_native_ids_fit_full_query_and_response_without_trimming() {
        let ids = (0..MAX_IDS).map(|n| format!("{n:02}{}", ":".repeat(126))).collect::<Vec<_>>();
        let key = "a".repeat(64);
        let raw = query(&ids, &MAX_CLIENT_EPOCH.to_string(), &key);
        // Even percent-encoding every byte of each legal ID is below the cap.
        let encoded = ids.iter().map(|id| format!("&id={}", id.bytes().map(|b| format!("%{b:02X}")).collect::<String>())).collect::<String>();
        let worst = format!("accountEpoch={MAX_CLIENT_EPOCH}&catalogNamespace={NAMESPACE}&serviceKey={key}{encoded}");
        assert!(worst.len() <= MAX_QUERY_BYTES);
        assert_eq!(Request::parse(Some(&worst)).unwrap().ids, ids);
        let parsed = Request::parse(Some(&raw)).unwrap();
        let data = ids.iter().map(|id| json!({"id": id, "completed": false})).collect();
        let response = envelope(&":".repeat(128), &parsed.identity, &"a".repeat(128), MAX_CLIENT_EPOCH, data).unwrap();
        assert_eq!(response["data"].as_array().unwrap().len(), MAX_IDS);
        assert!(serde_json::to_vec(&response).unwrap().len() <= MAX_RESPONSE_BYTES);
        assert!(envelope("m", &parsed.identity, "a", 0, vec![json!({"id":"x".repeat(MAX_RESPONSE_BYTES)})]).is_err());
    }
}
