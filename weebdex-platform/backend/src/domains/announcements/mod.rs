//! Additive source-missing announcement functionality. No original parity is claimed.
use crate::{
    domains::{
        auth::{optional_principal, require_principal, validate_mutation, SessionPrincipal},
        catalog::{
            query::{QueryParams, Sql},
            validation as v,
        },
    },
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    extract::{Path, RawQuery, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use tokio_postgres::Transaction;
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/announcement", get(search).post(create))
        .route("/announcement/{id}", get(detail).put(update).delete(remove))
}
fn staff(a: &SessionPrincipal, permission: &str) -> ApiResult<()> {
    if !a.is_staff() {
        return Err(ApiError::forbidden(
            "Staff announcement permission required",
        ));
    }
    a.require_permission(permission)
}
fn can_read_drafts(a: &SessionPrincipal) -> bool {
    a.is_staff() && (a.has_perm("announcement.create") || a.has_perm("announcement.update"))
}
async fn audit(
    tx: &Transaction<'_>,
    a: &SessionPrincipal,
    id: &str,
    operation: &str,
    old: Option<i32>,
    new: i32,
    published: bool,
    pinned: bool,
) -> ApiResult<()> {
    tx.execute("INSERT INTO platform.announcement_audit(announcement_id,actor_id,operation,old_version,new_version,published,pinned) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&id,&a.user_id,&operation,&old,&new,&published,&pinned]).await?;
    Ok(())
}
async fn search(
    State(s): State<AppState>,
    h: HeaderMap,
    RawQuery(raw): RawQuery,
) -> ApiResult<Json<Value>> {
    let q = QueryParams::parse(raw)?;
    let order = q.order("desc")?;
    let drafts = q.bool("include_drafts")?.unwrap_or(false);
    if drafts {
        let a = require_principal(&s, &h).await?;
        if !can_read_drafts(&a) {
            return Err(ApiError::forbidden("Staff draft permission required"));
        }
    }
    let mut sql = Sql::default();
    sql.conditions.push("a.deleted_at IS NULL".into());
    if !drafts {
        sql.conditions.push("a.published".into());
    }
    if let Some(pinned) = q.bool("pinned")? {
        let p = sql.bind(pinned);
        sql.conditions.push(format!("a.pinned={p}"));
    }
    if let Some(name) = q.first("name").filter(|s| !s.trim().is_empty()) {
        let p = sql.bind(name.to_owned());
        sql.conditions
            .push(format!("position(lower({p}) in lower(a.title))>0"));
    }
    let predicate = sql.predicate();
    let limit = sql.bind(q.limit);
    let offset = sql.bind((q.page - 1) * q.limit);
    let db = s.db.client().await?;
    let row=db.query_one(&format!("SELECT (SELECT count(*) FROM platform.announcements a WHERE {predicate}),COALESCE((SELECT jsonb_agg(x.dto ORDER BY x.pinned DESC,x.published_at {order} NULLS LAST,x.created_at {order},x.id {order}) FROM (SELECT a.id,a.pinned,a.published_at,a.created_at,platform.announcement_json(a.id) dto FROM platform.announcements a WHERE {predicate} ORDER BY a.pinned DESC,a.published_at {order} NULLS LAST,a.created_at {order},a.id {order} LIMIT {limit} OFFSET {offset})x),'[]'::jsonb)"),&sql.params()).await?;
    Ok(Json(
        json!({"total":row.get::<_,i64>(0),"limit":q.limit,"page":q.page,"data":row.get::<_,Value>(1)}),
    ))
}
async fn detail(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let a = optional_principal(&s, &h).await?;
    let drafts = a.as_ref().is_some_and(can_read_drafts);
    let db = s.db.client().await?;
    let row=db.query_opt("SELECT platform.announcement_json(id) FROM platform.announcements WHERE id=$1 AND deleted_at IS NULL AND (published OR $2)",&[&id,&drafts]).await?.ok_or_else(||ApiError::not_found("Announcement not found"))?;
    Ok(Json(row.get(0)))
}
async fn create(
    State(s): State<AppState>,
    h: HeaderMap,
    Json(p): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let a = require_principal(&s, &h).await?;
    validate_mutation(&s, &h)?;
    staff(&a, "announcement.create")?;
    v::allowed(&p, &["title", "body", "pinned", "published"])?;
    let title = v::string(&p, "title", 300, true)?.unwrap();
    let body = v::string(&p, "body", 100000, true)?.unwrap();
    let pinned = v::boolean(&p, "pinned")?.unwrap_or(false);
    let published = v::boolean(&p, "published")?.unwrap_or(false);
    let id = v::new_id("announcement");
    let mut db = s.db.client().await?;
    let tx = db.transaction().await?;
    crate::domains::auth::closure::active_in(&tx,&a.user_id).await?;
    tx.execute("INSERT INTO platform.announcements(id,author_id,title,body,pinned,published,published_at) VALUES($1,$2,$3,$4,$5,$6,CASE WHEN $6 THEN clock_timestamp() END)",&[&id,&a.user_id,&title,&body,&pinned,&published]).await?;
    audit(&tx, &a, &id, "create", None, 1, published, pinned).await?;
    let data = tx
        .query_one("SELECT platform.announcement_json($1)", &[&id])
        .await?
        .get(0);
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(data)))
}
async fn update(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(p): Json<Value>,
) -> ApiResult<Json<Value>> {
    let a = require_principal(&s, &h).await?;
    validate_mutation(&s, &h)?;
    staff(&a, "announcement.update")?;
    v::id(&id)?;
    v::allowed(&p, &["title", "body", "pinned", "published", "version"])?;
    let version = v::version(&p)?;
    let title = v::string(&p, "title", 300, false)?;
    let body = v::string(&p, "body", 100000, false)?;
    if title.as_deref() == Some("") || body.as_deref() == Some("") {
        return Err(ApiError::bad_request("Title and body cannot be empty"));
    }
    let pinned = v::boolean(&p, "pinned")?;
    let published = v::boolean(&p, "published")?;
    let mut db = s.db.client().await?;
    let tx = db.transaction().await?;
    crate::domains::auth::closure::active_in(&tx,&a.user_id).await?;
    let row=tx.query_opt("SELECT version,published,pinned FROM platform.announcements WHERE id=$1 AND deleted_at IS NULL FOR UPDATE",&[&id]).await?.ok_or_else(||ApiError::not_found("Announcement not found"))?;
    if row.get::<_, i32>(0) != version {
        return Err(ApiError::conflict(
            "Announcement changed; reload before editing",
        ));
    }
    let new_published = published.unwrap_or(row.get(1));
    let new_pinned = pinned.unwrap_or(row.get(2));
    tx.execute("UPDATE platform.announcements SET title=COALESCE($2,title),body=COALESCE($3,body),pinned=$4,published=$5,published_at=CASE WHEN $5 AND NOT published THEN clock_timestamp() WHEN NOT $5 THEN NULL ELSE published_at END,version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id,&title,&body,&new_pinned,&new_published]).await?;
    audit(
        &tx,
        &a,
        &id,
        "update",
        Some(version),
        version + 1,
        new_published,
        new_pinned,
    )
    .await?;
    let data = tx
        .query_one("SELECT platform.announcement_json($1)", &[&id])
        .await?
        .get(0);
    tx.commit().await?;
    Ok(Json(data))
}
async fn remove(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(p): Json<Value>,
) -> ApiResult<StatusCode> {
    let a = require_principal(&s, &h).await?;
    validate_mutation(&s, &h)?;
    staff(&a, "announcement.delete")?;
    v::id(&id)?;
    v::allowed(&p, &["version"])?;
    let version = v::version(&p)?;
    let mut db = s.db.client().await?;
    let tx = db.transaction().await?;
    crate::domains::auth::closure::active_in(&tx,&a.user_id).await?;
    let row=tx.query_opt("SELECT version,published,pinned FROM platform.announcements WHERE id=$1 AND deleted_at IS NULL FOR UPDATE",&[&id]).await?.ok_or_else(||ApiError::not_found("Announcement not found"))?;
    if row.get::<_, i32>(0) != version {
        return Err(ApiError::conflict(
            "Announcement changed; reload before deleting",
        ));
    }
    tx.execute("UPDATE platform.announcements SET deleted_at=clock_timestamp(),version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id]).await?;
    audit(
        &tx,
        &a,
        &id,
        "delete",
        Some(version),
        version + 1,
        row.get(1),
        row.get(2),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
