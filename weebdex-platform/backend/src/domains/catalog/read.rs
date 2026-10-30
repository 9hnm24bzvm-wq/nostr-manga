use super::{
    feed_diagnostics::FeedError,
    feed_projection::{CHAPTER_PARENT_LOOKUP, chapter_page_projection},
    query::{QueryParams, Sql, chapter_filters, manga_filters, window_seconds},
    repository::CatalogRepository,
    validation as v,
};
use crate::{
    domains::auth::{require_library_principal, require_principal},
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    Json,
    extract::{Path, RawQuery, State},
    http::HeaderMap,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportMappingRequest {
    myanimelist: Vec<String>,
}
/// Public, read-only mapping for local device imports. Ambiguity never invents
/// an association, and the two-row lateral cap bounds each requested identifier.
pub async fn manga_import_mappings(
    State(state): State<AppState>,
    Json(payload): Json<ImportMappingRequest>,
) -> ApiResult<Json<Value>> {
    if payload.myanimelist.is_empty()
        || payload.myanimelist.len() > 400
        || payload
            .myanimelist
            .iter()
            .any(|id| id.is_empty() || id.len() > 20 || !id.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(ApiError::bad_request(
            "Mapping requires between 1 and 400 bounded decimal MyAnimeList IDs",
        ));
    }
    let _read = state
        .reads
        .acquire(manga_node::read_admission::Class::Catalogue)
        .await
        .map_err(|_| ApiError::busy())?;
    let ids = payload
        .myanimelist
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let rows=state.db.client().await?.query("SELECT r.mal,m.id,m.title FROM unnest($1::text[]) r(mal) LEFT JOIN LATERAL(SELECT id,title FROM platform.catalog_manga WHERE state='published' AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id) AND (metadata->'links'->>'mal'=r.mal OR metadata->'links'->>'myanimelist'=r.mal) ORDER BY id LIMIT 2) m ON TRUE ORDER BY r.mal,m.id", &[&ids]).await?;
    let mut matches: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for row in rows {
        let entries = matches.entry(row.get(0)).or_default();
        if let Some(id) = row.get::<_, Option<String>>(1) {
            entries.push(json!({"myanimelist":row.get::<_,String>(0),"id":id,"title":row.get::<_,String>(2)}));
        }
    }
    let mut data = Vec::new();
    let mut missing = Vec::new();
    let mut ambiguous = Vec::new();
    for (id, mut entries) in matches {
        match entries.len() {
            0 => missing.push(id),
            1 => data.push(entries.remove(0)),
            _ => ambiguous.push(id),
        }
    }
    Ok(Json(
        json!({"data":data,"missing":missing,"ambiguous":ambiguous}),
    ))
}

pub async fn manga_search(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    manga_feed(state, raw, headers, false, false).await
}
pub async fn manga_top(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    manga_feed(state, raw, headers, true, false).await
}
pub async fn draft_search(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    manga_feed(state, raw, headers, false, true).await
}
async fn manga_feed(
    state: AppState,
    raw: Option<String>,
    headers: HeaderMap,
    top: bool,
    draft: bool,
) -> ApiResult<Json<Value>> {
    let q = QueryParams::parse(raw)?;
    let actor = if draft {
        Some(require_principal(&state, &headers).await?)
    } else if q.first("reading_status").is_some() {
        Some(require_library_principal(&state, &headers).await?)
    } else {
        None
    };
    let mut sql = Sql::default();
    sql.conditions.push("m.deleted_at IS NULL".to_owned());
    if draft {
        let a = actor.as_ref().unwrap();
        if a.is_api_client || a.recovery_session {
            return Err(ApiError::forbidden(
                "A browser session is required for drafts",
            ));
        }
        if a.is_moderator() {
            a.require_permission("manga.state.update")?;
        } else {
            a.require_permission("manga.create")?;
            let p = sql.bind(a.user_id.clone());
            sql.conditions.push(format!("m.owner_id={p}"));
        }
        sql.conditions.push("m.state<>'published'".to_owned());
    } else {
        sql.conditions.push("m.state='published'".to_owned());
    }
    if !draft {
        public_rating_defaults(&q, &mut sql, "m");
    }
    manga_filters(&q, &mut sql, "m", actor.as_ref())?;
    let order = q.order("desc")?;
    let sort = if top {
        ranking(&q, &mut sql, "manga", "m.id")?
    } else {
        match q.first("sort").unwrap_or("lastUploadedChapterAt") {"createdAt"=>"m.created_at".to_owned(),"updatedAt"=>"m.updated_at".to_owned(),"title"=>"lower(m.title)".to_owned(),"year"=>"m.year".to_owned(),"lastUploadedChapterAt"=>"(SELECT max(c.published_at) FROM platform.catalog_chapters c WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id))".to_owned(),"chapters"=>"(SELECT count(*) FROM platform.catalog_chapters c WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id))".to_owned(),"views"=>"COALESCE(s.views,0)".to_owned(),"follows"=>"COALESCE(s.follows,0)".to_owned(),"rating"=>"COALESCE(s.rating,0)".to_owned(),"relevance"=>{let p=sql.bind(q.first("title").unwrap_or("").to_owned());format!("CASE WHEN lower(m.title)=lower({p}) THEN 3 WHEN starts_with(lower(m.title),lower({p})) THEN 2 ELSE 1 END")},_=>return Err(ApiError::bad_request("Invalid manga sort"))}
    };
    let limit = sql.bind(q.limit);
    let offset = sql.bind((q.page - 1) * q.limit);
    let statement = format!(
        "WITH matched AS(SELECT m.id,{sort} AS sort_value FROM platform.catalog_manga m LEFT JOIN platform.catalog_manga_counters s ON s.manga_id=m.id WHERE {}),selected AS(SELECT * FROM matched ORDER BY sort_value {order} NULLS LAST,id ASC LIMIT {limit} OFFSET {offset}) SELECT jsonb_build_object('total',(SELECT count(*) FROM matched),'limit',{},'page',{},'data',COALESCE((SELECT jsonb_agg(platform.catalog_manga_json(id) || jsonb_build_object('_sort_value',sort_value) ORDER BY sort_value {order} NULLS LAST,id ASC) FROM selected),'[]'::jsonb))",
        sql.predicate(),
        q.limit,
        q.page
    );
    let data = state
        .db
        .client()
        .await?
        .query_one(&statement, &sql.params())
        .await?
        .get(0);
    Ok(Json(data))
}
fn ranking(q: &QueryParams, sql: &mut Sql, kind: &str, id: &str) -> ApiResult<String> {
    ranking_at(q, sql, kind, id, "clock_timestamp()")
}
/// Shared persisted ranking expression. Every SQL identifier/clock is supplied
/// by a trusted internal caller; request values remain bound parameters.
pub(crate) fn ranking_at(q: &QueryParams, sql: &mut Sql, kind: &str, id: &str, clock: &str) -> ApiResult<String> {
    let rank = q.first("rank").unwrap_or("read");
    if kind == "manga" && matches!(rank, "follow" | "follows") {
        return Ok("COALESCE(s.follows,0)".to_owned());
    }
    if !matches!(rank, "read" | "views") {
        return Err(ApiError::bad_request("Invalid ranking"));
    }
    if let Some(seconds) = window_seconds(q)? {
        let p = sql.bind(seconds);
        Ok(format!(
            "(SELECT count(*) FROM platform.catalog_view_events ve WHERE ve.entity_type='{kind}' AND ve.entity_id={id} AND ve.viewed_at>={clock}-({p}::bigint * interval '1 second') AND ve.viewed_at<={clock})"
        ))
    } else {
        Ok("COALESCE(s.views,0)".to_owned())
    }
}
pub async fn manga_get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(CatalogRepository::new(state.db).get_manga(&id).await?))
}
pub async fn draft_get(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let actor = require_principal(&state, &headers).await?;
    if actor.is_api_client || actor.recovery_session {
        return Err(ApiError::forbidden(
            "A browser session is required for drafts",
        ));
    }
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let row=tx.query_opt("SELECT owner_id,state,title,version,protocol_series_id,platform.catalog_manga_json(id) FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL",&[&id]).await?.ok_or_else(||ApiError::not_found("Draft not found"))?;
    if actor.is_moderator() {
        actor.require_permission("manga.state.update")?;
    } else if row.get::<_, String>(0) == actor.user_id {
        actor.require_permission("manga.create")?;
    } else {
        return Err(ApiError::forbidden("Only the draft owner may read it"));
    }
    let data = if row.get::<_, String>(1) == "published" {
        if !crate::protocol::publication::pending_native_manga_receipt_allowed(
            &state, &tx, &id, &actor, false,
        )
        .await?
        {
            return Err(ApiError::not_found("Draft not found"));
        }
        json!({"id":id,"title":row.get::<_,String>(2),"version":row.get::<_,i32>(3),"state":"published","protocol_event_id":row.get::<_,Option<String>>(4),"publication_scope":"original-protocol","publication_status":"accepted_pending_projection","is_unavailable":true,"can_mutate":false,"can_cancel":false,"can_retry_publication":false})
    } else {
        let mut data = row
            .get::<_, Option<Value>>(5)
            .ok_or_else(|| ApiError::not_found("Draft not found"))?;
        if let Some(command)=tx.query_opt("SELECT native_commit_payload FROM platform.protocol_publication_commands WHERE command_id=$1 AND catalog_id=$2 AND resource_kind='manga' AND state IN ('prepared','source_accepted')", &[&format!("manga:{id}"), &id]).await? {
            data["publication_scope"]=json!("original-protocol");
            // A durable checkpoint is not proof of current source acceptance.
            data["publication_status"]=json!("prepared_original_command");
            data["can_mutate"]=json!(false);
            data["can_cancel"]=json!(false);
            let allowed=actor.is_moderator() && actor.has_perm("manga.state.update") && crate::protocol::publication::native_manga_publication_retry_allowed(&state,&tx,&id,&actor).await?;
            data["can_retry_publication"]=json!(allowed);
            if allowed {
                let saved:Value=command.get::<_,Option<Value>>(0).ok_or_else(ApiError::unavailable)?;
                let version=saved["submitted_version"].as_i64().filter(|n|*n>0 && *n<=i32::MAX as i64).ok_or_else(ApiError::unavailable)?;
                data["publication_retry_version"]=json!(version);
            }
        }
        data
    };
    tx.commit().await?;
    Ok(Json(data))
}
pub async fn manga_random(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    let q = QueryParams::parse(raw)?;
    let actor = if q.first("reading_status").is_some() {
        Some(require_library_principal(&state, &headers).await?)
    } else {
        None
    };
    let mut sql = Sql::default();
    sql.conditions
        .push("m.state='published' AND m.deleted_at IS NULL".to_owned());
    public_rating_defaults(&q, &mut sql, "m");
    manga_filters(&q, &mut sql, "m", actor.as_ref())?;
    let statement = format!(
        "SELECT platform.catalog_manga_json(m.id) FROM platform.catalog_manga m WHERE {} ORDER BY random() LIMIT 1",
        sql.predicate()
    );
    let value = state
        .db
        .client()
        .await?
        .query_opt(&statement, &sql.params())
        .await?
        .map(|r| r.get(0))
        .ok_or_else(|| ApiError::not_found("No manga matches these filters"))?;
    Ok(Json(value))
}
pub async fn manga_relations(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    CatalogRepository::new(state.db.clone())
        .get_manga(&id)
        .await?;
    Ok(Json(
        state
            .db
            .client()
            .await?
            .query_one("SELECT platform.catalog_relations_json($1)", &[&id])
            .await?
            .get(0),
    ))
}
pub async fn manga_recommendations(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> ApiResult<Json<Value>> {
    CatalogRepository::new(state.db.clone())
        .get_manga(&id)
        .await?;
    let q = QueryParams::parse(raw)?;
    let offset = (q.page - 1) * q.limit;
    let value=state.db.client().await?.query_one("WITH matched AS(SELECT m.id,r.score FROM platform.catalog_recommendations r JOIN platform.catalog_manga m ON m.id=r.recommended_id WHERE r.manga_id=$1 AND EXISTS(SELECT 1 FROM platform.catalog_manga root WHERE root.id=$1 AND root.deleted_at IS NULL AND root.state='published' AND root.protocol_source_visible AND platform.protocol_source_is_visible(root.protocol_series_id)) AND m.state='published' AND m.deleted_at IS NULL AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)),selected AS(SELECT * FROM matched ORDER BY score DESC,id LIMIT $2 OFFSET $3) SELECT jsonb_build_object('total',(SELECT count(*) FROM matched),'limit',$2::bigint,'page',$4::bigint,'data',COALESCE((SELECT jsonb_agg(platform.catalog_manga_json(id) ORDER BY score DESC,id) FROM selected),'[]'::jsonb))",&[&id,&q.limit,&offset,&q.page]).await?.get(0);
    Ok(Json(value))
}
pub async fn author_search(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
) -> ApiResult<Json<Value>> {
    people_feed(state, raw, "author").await
}
pub async fn group_search(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
) -> ApiResult<Json<Value>> {
    people_feed(state, raw, "group").await
}
pub async fn tag_search(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
) -> ApiResult<Json<Value>> {
    people_feed(state, raw, "tag").await
}
async fn people_feed(state: AppState, raw: Option<String>, kind: &str) -> ApiResult<Json<Value>> {
    let mut q = QueryParams::parse(raw)?;
    // populateTagList requests no pagination and groups the complete taxonomy.
    // Keep this bootstrap bounded without silently applying the people page size.
    if kind == "tag" && q.first("limit").is_none() {
        q.limit = 10_000;
    }
    let mut sql = Sql::default();
    let (table, projection, deleted) = match kind {
        "author" => (
            "platform.catalog_authors",
            "platform.catalog_author_json(id)",
            "deleted_at IS NULL AND platform.protocol_source_is_visible(protocol_event_id)",
        ),
        "group" => (
            "platform.catalog_groups",
            "platform.catalog_group_json(id)",
            "deleted_at IS NULL AND platform.protocol_group_is_visible(id)",
        ),
        _ => (
            "platform.catalog_tags",
            "jsonb_build_object('id',id,'name',name,'group',tag_group)",
            "TRUE",
        ),
    };
    sql.conditions.push(deleted.to_owned());
    if let Some(name) = q.first("name").or(q.first("title")) {
        let p = sql.bind(name.to_owned());
        sql.conditions
            .push(format!("position(lower({p}) in lower(name))>0"));
    }
    sql.array("id", q.list("id"), false);
    if kind == "tag" {
        sql.array("tag_group", q.list("group"), false);
    }
    let limit = sql.bind(q.limit);
    let offset = sql.bind((q.page - 1) * q.limit);
    let order = q.order("asc")?;
    let query = format!(
        "WITH matched AS(SELECT * FROM {table} WHERE {}),selected AS(SELECT * FROM matched ORDER BY lower(name) {order},id LIMIT {limit} OFFSET {offset}) SELECT jsonb_build_object('total',(SELECT count(*) FROM matched),'limit',{},'page',{},'data',COALESCE((SELECT jsonb_agg({projection} ORDER BY lower(name) {order},id) FROM selected),'[]'::jsonb))",
        sql.predicate(),
        q.limit,
        q.page
    );
    let value: Value = state
        .db
        .client()
        .await?
        .query_one(&query, &sql.params())
        .await?
        .get(0);
    if kind == "tag"
        && q.first("limit").is_none()
        && value["total"].as_i64().is_some_and(|total| total > q.limit)
    {
        return Err(ApiError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "taxonomy_limit",
            "The complete tag taxonomy exceeds the bounded bootstrap limit",
        ));
    }
    Ok(Json(value))
}
pub async fn author_get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    person_get(state, id, "author").await
}
pub async fn group_get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    person_get(state, id, "group").await
}
async fn person_get(state: AppState, id: String, kind: &str) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let statement = if kind == "author" {
        "SELECT platform.catalog_author_json(id) FROM platform.catalog_authors WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_source_is_visible(protocol_event_id)"
    } else {
        "SELECT platform.catalog_group_json(id) FROM platform.catalog_groups WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_group_is_visible(id)"
    };
    let value = state
        .db
        .client()
        .await?
        .query_opt(statement, &[&id])
        .await?
        .map(|r| r.get(0))
        .ok_or_else(|| ApiError::not_found("Entity not found"))?;
    Ok(Json(value))
}
pub async fn chapter_get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        CatalogRepository::new(state.db).get_chapter(&id).await?,
    ))
}
pub async fn chapter_search(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, FeedError> {
    chapter_feed(state, raw, None, true, false, false).await
}
pub async fn chapter_latest(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, FeedError> {
    chapter_feed(state, raw, None, true, false, false).await
}
pub async fn chapter_updates(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, FeedError> {
    chapter_feed(state, raw, None, false, false, true).await
}
pub async fn chapter_top(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, FeedError> {
    chapter_feed(state, raw, None, true, true, false).await
}
pub async fn chapter_list(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, FeedError> {
    CatalogRepository::new(state.db.clone())
        .get_manga(&id)
        .await?;
    chapter_feed(state, raw, Some(id), false, false, false).await
}
async fn chapter_feed(
    state: AppState,
    raw: Option<String>,
    manga: Option<String>,
    mapped: bool,
    top: bool,
    updates: bool,
) -> Result<Json<Value>, FeedError> {
    let q = QueryParams::parse(raw)?;
    let is_manga_list = manga.is_some();
    let chapter_parent = manga.clone();
    let mut sql = Sql::default();
    sql.conditions
        .push("c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.state='published'".to_owned());
    if let Some(manga) = manga {
        let p = sql.bind(manga);
        sql.conditions.push(format!("c.manga_id={p}"));
    }
    chapter_filters(&q, &mut sql, "c")?;
    if !is_manga_list {
        public_rating_defaults(&q, &mut sql, "m");
    }
    let mut manga_q = q.clone();
    manga_q.values.remove("id"); // Here id addresses chapters, not their parent titles.
    manga_filters(&manga_q, &mut sql, "m", None)?;
    let order = q.order("desc")?;
    let sort = if top {
        ranking(&q, &mut sql, "chapter", "c.id")?
    } else {
        match q.first("sort").unwrap_or(if is_manga_list{"chapter"}else{"publishedAt"}){"publishedAt"=>"c.published_at".to_owned(),"createdAt"=>"c.created_at".to_owned(),"updatedAt"=>"c.updated_at".to_owned(),"chapter"=>"CASE WHEN c.chapter ~ '^[0-9]' THEN substring(c.chapter FROM '^[0-9]+(?:[.][0-9]+)?')::numeric END".to_owned(),"volume"=>"CASE WHEN c.volume ~ '^[0-9]' THEN substring(c.volume FROM '^[0-9]+(?:[.][0-9]+)?')::numeric END".to_owned(),"views"=>"COALESCE(s.views,0)".to_owned(),_=>return Err(ApiError::bad_request("Invalid chapter sort").into())}
    };
    let sort_text = match q.first("sort").unwrap_or(if is_manga_list {
        "chapter"
    } else {
        "publishedAt"
    }) {
        "chapter" => "lower(COALESCE(c.chapter,''))",
        "volume" => "lower(COALESCE(c.volume,''))",
        _ => "''::text",
    };
    let limit = sql.bind(q.limit);
    let offset = sql.bind((q.page - 1) * q.limit);
    let unique = if updates {
        "DISTINCT ON(c.manga_id)"
    } else {
        ""
    };
    let match_order = if updates {
        "ORDER BY c.manga_id,c.published_at DESC,c.id"
    } else {
        ""
    };
    let projection = chapter_page_projection(order, q.limit, q.page, mapped);
    let query = format!(
        "WITH matched AS(SELECT {unique} c.id,c.manga_id,{sort} AS sort_value,{sort_text} AS sort_text FROM platform.catalog_chapters c {CHAPTER_PARENT_LOOKUP} LEFT JOIN platform.catalog_chapter_counters s ON s.chapter_id=c.id WHERE {} {match_order}),selected AS MATERIALIZED(SELECT * FROM matched ORDER BY sort_value {order} NULLS LAST,sort_text {order},id LIMIT {limit} OFFSET {offset}){projection}",
        sql.predicate(),
    );
    // Bind only this title route. The namespace, rows, total, parent visibility
    // and nested DTOs are read in ONE PostgreSQL statement snapshot. idProfile
    // identifies the ID representation, not a fixed ordering of this dynamic feed.
    let bound_query = chapter_parent.and_then(|parent| {
        let service_key = crate::protocol::adapters::chapter_page_service_key(&state)?;
        let parent = sql.bind(parent);
        let service_key = sql.bind(service_key);
        Some(format!("SELECT page.value || CASE WHEN identity.namespace IS NOT NULL AND EXISTS(SELECT 1 FROM platform.catalog_manga m WHERE m.id={parent} AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)) THEN jsonb_build_object('chapterPage',jsonb_build_object('version',1,'mangaId',{parent}::text,'idProfile','native-id-v1','catalogIdentity',jsonb_build_object('catalogNamespace',identity.namespace,'serviceKey',{service_key}::text),'consistency','response-snapshot-v1')) ELSE '{{}}'::jsonb END FROM ({query}) AS page(value) CROSS JOIN (SELECT (SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton) AS namespace) identity"))
    });
    let client = state.db.pool().get().await
        .map_err(|error| FeedError::pool(error, state.config.fixture_feed_diagnostics))?;
    let params = sql.params();
    // Missing optional namespace/configuration omits the envelope. SQL failures
    // are ordinary feed failures, never retried as an unbound second query.
    let row = client.query_one(bound_query.as_deref().unwrap_or(&query), &params).await
        .map_err(|error| FeedError::query(error, state.config.fixture_feed_diagnostics))?;
    Ok(Json(row.get(0)))
}
pub async fn chapter_aggregate(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> ApiResult<Json<Value>> {
    CatalogRepository::new(state.db.clone())
        .get_manga(&id)
        .await?;
    let q = QueryParams::parse(raw)?;
    let mut sql = Sql::default();
    let p = sql.bind(id);
    sql.conditions.push(format!("c.manga_id={p} AND c.deleted_at IS NULL AND EXISTS(SELECT 1 FROM platform.catalog_manga parent WHERE parent.id=c.manga_id AND parent.deleted_at IS NULL AND parent.state='published' AND parent.protocol_source_visible AND platform.protocol_source_is_visible(parent.protocol_series_id))"));
    chapter_filters(&q, &mut sql, "c")?;
    let query = format!(
        "SELECT c.id,c.volume,c.chapter,c.language,platform.catalog_timestamp(c.published_at),COALESCE((SELECT array_agg(group_id ORDER BY group_id) FROM platform.catalog_chapter_groups cg WHERE cg.chapter_id=c.id AND platform.protocol_group_is_visible(cg.group_id)),ARRAY[]::text[]) FROM platform.catalog_chapters c WHERE {} ORDER BY CASE WHEN c.volume ~ '^[0-9]' THEN substring(c.volume FROM '^[0-9]+(?:[.][0-9]+)?')::numeric END DESC NULLS LAST,c.volume DESC,CASE WHEN c.chapter ~ '^[0-9]' THEN substring(c.chapter FROM '^[0-9]+(?:[.][0-9]+)?')::numeric END DESC NULLS LAST,c.chapter DESC,c.published_at DESC,c.id LIMIT 10001",
        sql.predicate()
    );
    let client = state.db.client().await?;
    let rows = client.query(&query, &sql.params()).await?;
    if rows.len() > 10000 {
        return Err(ApiError::bad_request(
            "Aggregate exceeds 10000 chapters; narrow language/group filters",
        ));
    }
    let mut languages: Vec<String> = rows.iter().map(|r| r.get(3)).collect();
    languages.sort();
    languages.dedup();
    let mut group_ids: Vec<String> = rows
        .iter()
        .flat_map(|r| r.get::<_, Vec<String>>(5))
        .collect();
    group_ids.sort();
    group_ids.dedup();
    let groups=client.query("SELECT platform.catalog_group_json(id) FROM platform.catalog_groups WHERE id=ANY($1) AND deleted_at IS NULL AND platform.protocol_group_is_visible(id) ORDER BY id",&[&group_ids]).await?.iter().map(|r|r.get::<_,Value>(0)).collect::<Vec<_>>();
    let mut positions: BTreeMap<(Option<String>, Option<String>), usize> = BTreeMap::new();
    let mut chapters: Vec<Value> = Vec::new();
    for row in rows {
        let volume: Option<String> = row.get(1);
        let chapter: Option<String> = row.get(2);
        let key = (volume.clone(), chapter.clone());
        let pos = *positions.entry(key).or_insert_with(|| {
            chapters.push(json!({"volume":volume,"chapter":chapter,"entries":{}}));
            chapters.len() - 1
        });
        let lang: String = row.get(3);
        let group_indexes: Vec<usize> = row
            .get::<_, Vec<String>>(5)
            .iter()
            .filter_map(|g| group_ids.binary_search(g).ok())
            .collect();
        let entry = json!({"published_at":row.get::<_,String>(4),"language":languages.binary_search(&lang).unwrap(),"groups":group_indexes});
        chapters[pos]["entries"]
            .as_object_mut()
            .unwrap()
            .insert(row.get::<_, String>(0), entry);
    }
    Ok(Json(
        json!({"chapters":chapters,"groups":groups,"languages":languages}),
    ))
}
pub async fn cover_list(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let q = QueryParams::parse(raw)?;
    if q.bool("draft")? == Some(true) {
        let actor = require_principal(&state, &headers).await?;
        if actor.is_api_client || actor.recovery_session {
            return Err(ApiError::forbidden(
                "A browser session is required for draft covers",
            ));
        }
        let owner=state.db.client().await?.query_opt("SELECT owner_id FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id)",&[&id]).await?.ok_or_else(||ApiError::not_found("Manga not found"))?.get::<_,String>(0);
        if actor.is_moderator() {
            actor.require_permission("manga.cover.edit")?;
        } else if owner == actor.user_id {
            actor.require_permission("manga.create")?;
        } else {
            return Err(ApiError::forbidden("Only the draft owner may read covers"));
        }
    } else {
        CatalogRepository::new(state.db.clone())
            .get_manga(&id)
            .await?;
    }
    let data=state.db.client().await?.query_one("SELECT COALESCE(jsonb_agg(platform.catalog_cover_json(id) ORDER BY is_main DESC,created_at DESC,id),'[]'::jsonb) FROM platform.catalog_covers WHERE manga_id=$1 AND deleted_at IS NULL AND platform.protocol_source_is_visible(protocol_event_id) AND EXISTS(SELECT 1 FROM platform.catalog_manga parent WHERE parent.id=$1 AND parent.deleted_at IS NULL AND parent.protocol_source_visible AND platform.protocol_source_is_visible(parent.protocol_series_id))",&[&id]).await?.get(0);
    Ok(Json(data))
}

fn public_rating_defaults(q: &QueryParams, sql: &mut Sql, alias: &str) {
    if q.list("contentRating").is_empty() {
        sql.array(
            &format!("{alias}.content_rating"),
            vec!["safe".into(), "suggestive".into(), "erotica".into()],
            false,
        );
    }
}
