use super::validation as v;
use crate::{
    contracts::library::ProfilePatch,
    domains::{
        auth,
        catalog::query::{QueryParams, Sql},
    },
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    extract::{Path, RawQuery, State},
    http::HeaderMap,
    Json,
};
use serde_json::{json, Value};
pub async fn get(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let row=state.db.client().await?.query_opt("SELECT platform.library_user_json(id) FROM platform.users WHERE id=$1 AND NOT disabled",&[&id]).await?.ok_or_else(||ApiError::not_found("User not found"))?;
    Ok(Json(row.get(0)))
}
pub async fn search(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
) -> ApiResult<Json<Value>> {
    let q = QueryParams::parse(raw)?;
    let mut sql = Sql::default();
    sql.conditions.push("NOT u.disabled".into());
    let ids = q.list("id");
    if !ids.is_empty() {
        let ids = v::ids(&ids, 100)?;
        sql.array("u.id", ids, false);
    }
    if let Some(name) = q.first("name").or_else(|| q.first("username")) {
        let name = v::text(name, 200, false)?;
        let arg = sql.bind(name);
        sql.conditions
            .push(format!("position(lower({arg}) in lower(u.username))>0"));
    }
    let sort = match q.first("sort").unwrap_or("name") {
        "name" => "lower(u.username)",
        "createdAt" => "u.created_at",
        _ => return Err(ApiError::bad_request("Invalid user sort")),
    };
    let order = q.order("asc")?;
    let db = state.db.client().await?;
    let predicate = sql.predicate();
    let total: i64 = db
        .query_one(
            &format!("SELECT count(*) FROM platform.users u WHERE {predicate}"),
            &sql.params(),
        )
        .await?
        .get(0);
    let limit = sql.bind(q.limit);
    let offset = sql.bind((q.page - 1) * q.limit);
    let rows=db.query(&format!("SELECT platform.library_user_json(u.id) FROM platform.users u WHERE {predicate} ORDER BY {sort} {order},u.id ASC LIMIT {limit} OFFSET {offset}"),&sql.params()).await?;
    Ok(Json(
        json!({"total":total,"limit":q.limit,"page":q.page,"data":rows.iter().map(|r|r.get::<_,Value>(0)).collect::<Vec<_>>() }),
    ))
}
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(p): Json<ProfilePatch>,
) -> ApiResult<Json<Value>> {
    v::id(&id)?;
    let actor = auth::require_principal(&state, &headers).await?;
    auth::validate_mutation(&state, &headers)?;
    if id == actor.user_id {
        actor.require_permission("self.user.update")?;
    } else {
        if !actor.is_staff() {
            return Err(ApiError::forbidden("Only staff can edit another profile"));
        }
        actor.require_permission("user.update")?;
    }
    let patch = v::profile(&p)?;
    if patch.as_object().expect("profile object").is_empty() {
        return Err(ApiError::bad_request("No profile changes supplied"));
    }
    let next = p
        .version
        .checked_add(1)
        .ok_or_else(|| ApiError::conflict("Profile version limit reached"))?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let row = tx
        .query_opt(
            "SELECT version FROM platform.users WHERE id=$1 AND NOT disabled FOR UPDATE",
            &[&id],
        )
        .await?
        .ok_or_else(|| ApiError::not_found("User not found"))?;
    if row.get::<_, i32>(0) != p.version {
        return Err(ApiError::conflict("Profile changed; reload before editing"));
    }
    // Empty strings remove contact/biography fields; profile metadata cannot overwrite identity or authority.
    tx.execute("UPDATE platform.users SET profile=(profile || $2),version=$3,updated_at=clock_timestamp() WHERE id=$1",&[&id,&patch,&next]).await?;
    for (key, value) in patch.as_object().expect("profile object") {
        if value.as_str() == Some("") {
            tx.execute(
                "UPDATE platform.users SET profile=profile-$2 WHERE id=$1",
                &[&id, &key],
            )
            .await?;
        }
    }
    tx.execute("INSERT INTO platform.catalog_audit(actor_id,entity_type,entity_id,operation,old_version,new_version) VALUES($1,'user',$2,'update',$3,$4)",&[&actor.user_id,&id,&p.version,&next]).await?;
    let result: Value = tx
        .query_one("SELECT platform.library_user_json($1)", &[&id])
        .await?
        .get(0);
    tx.commit().await?;
    Ok(Json(result))
}
