use super::*;
pub(super) async fn series(
    State(a): State<Shared>,
    RawQuery(raw): RawQuery,
) -> Result<Json<crate::catalogue::Page>, ApiError> {
    let query = crate::catalogue::SeriesQuery::parse(raw.as_deref())
        .map_err(|_| bad("invalid catalogue query"))?;
    a.repo
        .catalogue_series(query)
        .await
        .map(Json)
        .map_err(|_| unavailable())
}
pub(super) async fn entry(
    State(a): State<Shared>,
    Path(key): Path<String>,
) -> Result<Json<crate::catalogue::Entry>, ApiError> {
    a.repo
        .catalogue_entry(id(&key).map_err(|_| bad("publication ID"))?)
        .await
        .map_err(|_| unavailable())?
        .map(Json)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error":"catalogue_entry_unavailable"})),
            )
        })
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ErrorPage {
    after: Option<String>,
    limit: Option<i64>,
}
pub(super) async fn errors(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(q): Query<ErrorPage>,
) -> Result<Json<Value>, ApiError> {
    let auth = operator_preflight(&a, &uri, &headers, "GET")?;
    consume_authorization(&a, &auth).await?;
    let after = q
        .after
        .as_deref()
        .map(id)
        .transpose()
        .map_err(|_| bad("after ID"))?
        .map(|id| id.to_vec());
    let limit = q.limit.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(bad("page bounds"));
    }
    let rows=a.repo.pool.get().await.map_err(|_|unavailable())?.query("SELECT event_id,reason,observed_at::text FROM catalogue_index_errors WHERE ($1::bytea IS NULL OR event_id>$1) ORDER BY event_id LIMIT $2",&[&after,&(limit+1)]).await.map_err(|_|unavailable())?;
    let more = rows.len() > limit as usize;
    let items:Vec<_>=rows.into_iter().take(limit as usize).map(|r|json!({"event":hex::encode(r.get::<_,Vec<u8>>(0)),"reason":r.get::<_,String>(1),"observed_at":r.get::<_,String>(2)})).collect();
    let next = if more {
        items.last().map(|v| v["event"].clone())
    } else {
        None
    };
    Ok(Json(json!({"items":items,"next_after":next})))
}
pub(super) async fn reindex(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Retry {
        event: String,
    }
    let value = operator_json(&a, &uri, &headers, request).await?;
    let r: Retry = serde_json::from_value(value).map_err(|_| bad("event required"))?;
    let event = id(&r.event).map_err(|_| bad("event ID"))?;
    a.repo
        .pool
        .get()
        .await
        .map_err(|_| unavailable())?
        .execute(
            "DELETE FROM catalogue_index_errors WHERE event_id=$1",
            &[&event.as_slice()],
        )
        .await
        .map_err(|_| unavailable())?;
    Ok(Json(
        json!({"event":r.event,"eligible_for_reindex":true,"publication_mutated":false}),
    ))
}
