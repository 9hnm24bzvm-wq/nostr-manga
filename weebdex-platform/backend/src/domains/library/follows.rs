use super::{query, repository as r, validation as v};
use crate::{
    contracts::library::{ImportFollows, UpdateFollow},
    domains::{
        auth,
        catalog::query::{chapter_filters, manga_filters, QueryParams, Sql},
    },
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    extract::{Path, RawQuery, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(p): Json<UpdateFollow>,
) -> ApiResult<StatusCode> {
    v::id(&id)?;
    let actor = auth::require_principal(&state, &headers).await?;
    auth::validate_mutation(&state, &headers)?;
    actor.require_permission("self.preferences.update")?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    r::lock_user(&tx, &actor.user_id).await?;
    r::published_manga(&tx, &id).await?;
    r::upsert_follow(
        &tx,
        &actor.user_id,
        &id,
        p.status.as_str(),
        p.notification,
        p.private,
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn unfollow(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<StatusCode> {
    v::id(&id)?;
    let actor = auth::require_principal(&state, &headers).await?;
    auth::validate_mutation(&state, &headers)?;
    actor.require_permission("self.preferences.update")?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    r::lock_user(&tx, &actor.user_id).await?;
    let deleted = tx
        .execute(
            "DELETE FROM platform.library_manga_follows WHERE user_id=$1 AND manga_id=$2",
            &[&actor.user_id, &id],
        )
        .await?;
    if deleted > 0 {
        tx.execute("UPDATE platform.catalog_manga_counters SET follows=GREATEST(0,follows-1) WHERE manga_id=$1",&[&id]).await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn search(
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(raw): RawQuery,
) -> ApiResult<Json<Value>> {
    let actor = auth::require_library_principal(&state, &headers).await?;
    let q = query::parse(raw)?;
    let mut sql = Sql::default();
    let user = sql.bind(actor.user_id.clone());
    sql.conditions.push(format!(
        "f.user_id={user} AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id) AND m.deleted_at IS NULL"
    ));
    manga_filters(&q, &mut sql, "m", Some(&actor))?;
    let ids = q.list("id");
    sql.array("m.id", ids, false);
    let sort = match q.first("sort").unwrap_or("followedAt") {
        "followedAt" => "f.followed_at",
        "createdAt" => "m.created_at",
        "updatedAt" => "m.updated_at",
        "title" => "lower(m.title)",
        "year" => "m.year",
        "lastUploadedChapterAt" => {
            "(SELECT max(c.published_at) FROM platform.catalog_chapters c WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id))"
        }
        "rating" => "COALESCE(s.rating,0)",
        "views" => "COALESCE(s.views,0)",
        "follows" => "COALESCE(s.follows,0)",
        "chapters" => {
            "(SELECT count(*) FROM platform.catalog_chapters c WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id))"
        }
        _ => return Err(ApiError::bad_request("Invalid library sort")),
    };
    let order = q.order("desc")?;
    let predicate = sql.predicate();
    let from = "platform.library_manga_follows f JOIN platform.catalog_manga m ON m.id=f.manga_id LEFT JOIN platform.catalog_manga_counters s ON s.manga_id=m.id";
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    tx.batch_execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await?;
    let total: i64 = tx
        .query_one(
            &format!("SELECT count(*) FROM {from} WHERE {predicate}"),
            &sql.params(),
        )
        .await?
        .get(0);
    let limit = sql.bind(q.limit);
    let offset = sql.bind((q.page - 1) * q.limit);
    let rows=tx.query(&format!("SELECT platform.catalog_manga_json(m.id) || jsonb_build_object('reading_status',f.reading_status,'followed_at',platform.catalog_timestamp(f.followed_at),'private',f.is_private,'notification',f.notification) FROM {from} WHERE {predicate} ORDER BY {sort} {order} NULLS LAST,m.id ASC LIMIT {limit} OFFSET {offset}"),&sql.params()).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"total":total,"limit":q.limit,"page":q.page,"data":rows.iter().map(|r|r.get::<_,Value>(0)).collect::<Vec<_>>() }),
    ))
}
pub async fn import(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(p): Json<ImportFollows>,
) -> ApiResult<StatusCode> {
    let actor = auth::require_principal(&state, &headers).await?;
    auth::validate_mutation(&state, &headers)?;
    actor.require_permission("self.preferences.update")?;
    let entries = p.weebdex.unwrap_or_default();
    let mal = p.myanimelist.unwrap_or_default();
    if entries.len() + mal.len() > 400 || entries.is_empty() && mal.is_empty() {
        return Err(ApiError::bad_request(
            "Import must contain between 1 and 400 entries",
        ));
    }
    for id in entries.keys() {
        v::id(id)?;
    }
    if mal
        .keys()
        .any(|id| id.is_empty() || id.len() > 20 || !id.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(ApiError::bad_request("Invalid MyAnimeList identifier"));
    }
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    r::lock_user(&tx, &actor.user_id).await?;
    let mut mapped = BTreeMap::new();
    for (id, status) in mal {
        let rows=tx.query("SELECT id FROM platform.catalog_manga WHERE state='published' AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id) AND (metadata->'links'->>'mal'=$1 OR metadata->'links'->>'myanimelist'=$1)",&[&id]).await?;
        if rows.len() != 1 {
            return Err(ApiError::bad_request(
                "MyAnimeList entry has no unique persisted catalog mapping",
            ));
        }
        let manga: String = rows[0].get(0);
        mapped.insert(manga, status);
    }
    for (id, status) in entries {
        mapped.insert(id, status);
    }
    let ids: Vec<String> = mapped.keys().cloned().collect();
    r::validate_manga_ids(&tx, &ids).await?;
    for (id, status) in mapped {
        r::upsert_follow(&tx, &actor.user_id, &id, status.as_str(), None, None).await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
fn spreadsheet_safe_title(s: &str) -> String {
    if s.trim_start().starts_with(['=', '+', '-', '@']) || s.starts_with(['\t', '\r']) {
        format!("'{s}")
    } else {
        s.to_owned()
    }
}

fn csv_cell(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
pub async fn export(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Response> {
    let actor = auth::require_library_principal(&state, &headers).await?;
    let rows=state.db.client().await?.query("SELECT m.id,m.title,f.reading_status FROM platform.library_manga_follows f JOIN platform.catalog_manga m ON m.id=f.manga_id WHERE f.user_id=$1 AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id) ORDER BY f.followed_at,m.id LIMIT 10001",&[&actor.user_id]).await?;
    if rows.len() > 10_000 {
        return Err(ApiError::bad_request(
            "Library export exceeds 10000 entries",
        ));
    }
    let mut csv = String::from("weebdex_id,title,reading_status\r\n");
    for row in rows {
        let id: String = row.get(0);
        let title: String = row.get(1);
        let status: String = row.get(2);
        csv.push_str(&format!(
            "{},{},{}\r\n",
            csv_cell(&id),
            csv_cell(&spreadsheet_safe_title(&title)),
            csv_cell(&status)
        ));
    }
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"weebdex-library.csv\"",
            ),
            (header::CACHE_CONTROL, "private, no-store"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        csv,
    )
        .into_response())
}
pub async fn chapter_feed(
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(raw): RawQuery,
) -> ApiResult<Json<Value>> {
    let actor = auth::require_library_principal(&state, &headers).await?;
    let q = query::parse(raw)?;
    Ok(Json(chapters(&state, &actor, &q, false, false).await?))
}
pub async fn chapter_updates(
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(raw): RawQuery,
) -> ApiResult<Json<Value>> {
    let actor = auth::require_library_principal(&state, &headers).await?;
    let q = query::parse(raw)?;
    Ok(Json(chapters(&state, &actor, &q, false, true).await?))
}
pub async fn chapters(
    state: &AppState,
    actor: &auth::SessionPrincipal,
    q: &QueryParams,
    history: bool,
    latest: bool,
) -> ApiResult<Value> {
    let mut sql = Sql::default();
    let user = sql.bind(actor.user_id.clone());
    let join = if history {
        format!("JOIN platform.library_chapter_history h ON h.chapter_id=c.id AND h.user_id={user}")
    } else {
        format!(
            "JOIN platform.library_manga_follows f ON f.manga_id=m.id AND f.user_id={user} AND f.notification"
        )
    };
    sql.conditions
        .push("c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)".into());
    let mut manga_q = q.clone();
    manga_q.values.remove("id"); // Chapter feed IDs must not also filter manga IDs.
    manga_filters(&manga_q, &mut sql, "m", Some(actor))?;
    chapter_filters(q, &mut sql, "c")?;
    let predicate = sql.predicate();
    let from = format!(
        "platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id {join}"
    );
    let count = if latest {
        "count(DISTINCT c.manga_id)"
    } else {
        "count(*)"
    };
    let sort = match q
        .first("sort")
        .unwrap_or(if history { "readAt" } else { "publishedAt" })
    {
        "readAt" if history => "h.read_at",
        "publishedAt" => "c.published_at",
        "createdAt" => "c.created_at",
        "updatedAt" => "c.updated_at",
        "chapter" => "c.chapter",
        "volume" => "c.volume",
        _ => return Err(ApiError::bad_request("Invalid chapter feed sort")),
    };
    let order = q.order("desc")?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    tx.batch_execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await?;
    let total: i64 = tx
        .query_one(
            &format!("SELECT {count} FROM {from} WHERE {predicate}"),
            &sql.params(),
        )
        .await?
        .get(0);
    let limit = sql.bind(q.limit);
    let offset = sql.bind((q.page - 1) * q.limit);
    let dto = if history {
        "platform.catalog_chapter_json(c.id) || jsonb_build_object('read_at',platform.catalog_timestamp(h.read_at))"
    } else {
        "platform.catalog_chapter_json(c.id)"
    };
    let query = if latest {
        format!(
            "SELECT platform.catalog_chapter_json(x.id) FROM (SELECT DISTINCT ON (c.manga_id) c.id,c.published_at FROM {from} WHERE {predicate} ORDER BY c.manga_id,c.published_at DESC,c.id ASC) x ORDER BY x.published_at DESC,x.id ASC LIMIT {limit} OFFSET {offset}"
        )
    } else {
        format!(
            "SELECT {dto} FROM {from} WHERE {predicate} ORDER BY {sort} {order} NULLS LAST,c.id ASC LIMIT {limit} OFFSET {offset}"
        )
    };
    let rows = tx.query(&query, &sql.params()).await?;
    let data: Vec<Value> = rows.iter().map(|r| r.get(0)).collect();
    let mut manga = serde_json::Map::new();
    for chapter in &data {
        if let Some(value) = chapter.pointer("/relationships/manga") {
            if let Some(id) = value.get("id").and_then(Value::as_str) {
                manga.insert(id.to_owned(), value.clone());
            }
        }
    }
    tx.commit().await?;
    Ok(json!({"total":total,"limit":q.limit,"page":q.page,"data":data,"map":{"manga":manga}}))
}
#[derive(Clone, Copy)]
enum Target {
    User,
    Group,
}
async fn target_exists(state: &AppState, target: Target, id: &str) -> ApiResult<()> {
    v::id(id)?;
    let sql = match target {
        Target::User => "SELECT id FROM platform.users WHERE id=$1 AND NOT disabled",
        Target::Group => {
            "SELECT id FROM platform.catalog_groups WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_group_is_visible(id)"
        }
    };
    if state
        .db
        .client()
        .await?
        .query_opt(sql, &[&id])
        .await?
        .is_none()
    {
        return Err(ApiError::not_found("Follow target not found"));
    }
    Ok(())
}
async fn change_target(
    state: &AppState,
    headers: &HeaderMap,
    id: &str,
    target: Target,
    follow: bool,
) -> ApiResult<StatusCode> {
    let actor = auth::require_principal(state, headers).await?;
    auth::validate_mutation(state, headers)?;
    actor.require_permission("self.preferences.update")?;
    if matches!(target, Target::User) && id == actor.user_id {
        return Err(ApiError::bad_request("An account cannot follow itself"));
    }
    target_exists(state, target, id).await?;
    let (table, column) = match target {
        Target::User => ("platform.library_user_follows", "target_id"),
        Target::Group => ("platform.library_group_follows", "group_id"),
    };
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    r::lock_user(&tx, &actor.user_id).await?;
    if follow {
        tx.execute(&format!("INSERT INTO {table}(user_id,{column}) VALUES($1,$2) ON CONFLICT(user_id,{column}) DO NOTHING"),&[&actor.user_id,&id]).await?;
    } else {
        tx.execute(
            &format!("DELETE FROM {table} WHERE user_id=$1 AND {column}=$2"),
            &[&actor.user_id, &id],
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn target_status(
    state: &AppState,
    headers: &HeaderMap,
    id: &str,
    target: Target,
) -> ApiResult<StatusCode> {
    let actor = auth::require_library_principal(state, headers).await?;
    target_exists(state, target, id).await?;
    let (table, column) = match target {
        Target::User => ("platform.library_user_follows", "target_id"),
        Target::Group => ("platform.library_group_follows", "group_id"),
    };
    let exists: bool = state
        .db
        .client()
        .await?
        .query_one(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE user_id=$1 AND {column}=$2)"),
            &[&actor.user_id, &id],
        )
        .await?
        .get(0);
    if exists {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(
            "This account does not follow that target",
        ))
    }
}
async fn target_feed(
    state: &AppState,
    headers: &HeaderMap,
    raw: Option<String>,
    target: Target,
) -> ApiResult<Value> {
    let actor = auth::require_library_principal(state, headers).await?;
    let q = query::parse(raw)?;
    let mut sql = Sql::default();
    let user = sql.bind(actor.user_id);
    sql.conditions.push(format!("f.user_id={user}"));
    let (from, dto, name, id) = match target {
        Target::User => {
            sql.conditions.push("NOT t.disabled".into());
            (
                "platform.library_user_follows f JOIN platform.users t ON t.id=f.target_id",
                "platform.library_user_json(t.id)",
                "t.username",
                "t.id",
            )
        }
        Target::Group => {
            sql.conditions
                .push("t.deleted_at IS NULL AND platform.protocol_group_is_visible(t.id)".into());
            (
                "platform.library_group_follows f JOIN platform.catalog_groups t ON t.id=f.group_id",
                "platform.catalog_group_json(t.id)",
                "t.name",
                "t.id",
            )
        }
    };
    sql.array(id, q.list("id"), false);
    if let Some(text) = q.first("name") {
        let p = sql.bind(v::text(text, 200, false)?);
        sql.conditions
            .push(format!("position(lower({p}) in lower({name}))>0"));
    }
    let sort = match q.first("sort").unwrap_or("followedAt") {
        "followedAt" => "f.followed_at",
        "name" => name,
        _ => return Err(ApiError::bad_request("Invalid follow sort")),
    };
    let order = q.order("desc")?;
    let predicate = sql.predicate();
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    tx.batch_execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await?;
    let total: i64 = tx
        .query_one(
            &format!("SELECT count(*) FROM {from} WHERE {predicate}"),
            &sql.params(),
        )
        .await?
        .get(0);
    let limit = sql.bind(q.limit);
    let offset = sql.bind((q.page - 1) * q.limit);
    let rows=tx.query(&format!("SELECT {dto} || jsonb_build_object('followed_at',platform.catalog_timestamp(f.followed_at)) FROM {from} WHERE {predicate} ORDER BY {sort} {order},{id} ASC LIMIT {limit} OFFSET {offset}"),&sql.params()).await?;
    tx.commit().await?;
    Ok(
        json!({"total":total,"limit":q.limit,"page":q.page,"data":rows.iter().map(|r|r.get::<_,Value>(0)).collect::<Vec<_>>() }),
    )
}
macro_rules! target_handlers {
    ($feed:ident,$status:ident,$follow:ident,$unfollow:ident,$target:expr) => {
        pub async fn $feed(
            State(state): State<AppState>,
            headers: HeaderMap,
            RawQuery(raw): RawQuery,
        ) -> ApiResult<Json<Value>> {
            Ok(Json(target_feed(&state, &headers, raw, $target).await?))
        }
        pub async fn $status(
            State(state): State<AppState>,
            Path(id): Path<String>,
            headers: HeaderMap,
        ) -> ApiResult<StatusCode> {
            target_status(&state, &headers, &id, $target).await
        }
        pub async fn $follow(
            State(state): State<AppState>,
            Path(id): Path<String>,
            headers: HeaderMap,
        ) -> ApiResult<StatusCode> {
            change_target(&state, &headers, &id, $target, true).await
        }
        pub async fn $unfollow(
            State(state): State<AppState>,
            Path(id): Path<String>,
            headers: HeaderMap,
        ) -> ApiResult<StatusCode> {
            change_target(&state, &headers, &id, $target, false).await
        }
    };
}
target_handlers!(users, user_status, user_follow, user_unfollow, Target::User);
target_handlers!(
    groups,
    group_status,
    group_follow,
    group_unfollow,
    Target::Group
);
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csv_quotes_are_roundtrip_safe() {
        assert_eq!(csv_cell("a,\"b\"\n日本語"), "\"a,\"\"b\"\"\n日本語\"");
    }
}
