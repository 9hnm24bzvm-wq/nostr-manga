//! Instance-account observations shared by native sessions and explicitly bound
//! public Nostr requests. No signing/publication or ambient UUID write authority.
use crate::{domains::{auth, catalog::entity_mutations::CatalogMutationActor}, error::{ApiError, ApiResult}, AppState};
use axum::{Json, body::to_bytes, extract::{Path, RawQuery, Request, State}, http::HeaderMap};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tokio_postgres::Transaction;
use super::account_public_id_in as account_id_in;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RatingPayload { pub rating: i16, pub version: Option<i32>, #[serde(rename="expectedAccountId")] pub expected_account_id: Option<String> }
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeletePayload { pub version: Option<i32>, #[serde(rename="expectedAccountId")] pub expected_account_id: Option<String> }

pub(crate) fn validate(rating: Option<i16>, version: Option<i32>) -> ApiResult<()> {
    if rating.is_some_and(|v| !(1..=10).contains(&v)) || version.is_some_and(|v| v<0) {
        return Err(ApiError::bad_request("rating must be an integer 1..10 and version a nonnegative integer"));
    }
    Ok(())
}

pub(crate) async fn read_in(tx: &Transaction<'_>, user: &str, ids: &[String]) -> ApiResult<BTreeMap<String, Value>> {
    let rows = tx.query("SELECT r.manga_id,r.rating,platform.catalog_timestamp(r.rated_at),r.version,platform.catalog_timestamp(r.updated_at) FROM platform.library_manga_ratings r JOIN platform.catalog_manga m ON m.id=r.manga_id WHERE r.user_id=$1 AND r.manga_id=ANY($2::text[]) AND r.rating IS NOT NULL AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)", &[&user,&ids]).await?;
    Ok(rows.into_iter().map(|r| (r.get(0),json!({"rating":r.get::<_,i16>(1),"createdAt":r.get::<_,String>(2),"nostr":{"version":r.get::<_,i32>(3),"updatedAt":r.get::<_,String>(4)}}))).collect())
}

pub(crate) async fn mutate_in(tx: &Transaction<'_>, actor: &impl CatalogMutationActor, manga: &str, rating: Option<i16>, expected: Option<i32>, expected_account: Option<&str>) -> ApiResult<Value> {
    validate(rating,expected)?;
    actor.require_permission("self.preferences.update")?;
    // Recheck the genuine actor's current native account inside this transaction.
    let user=tx.query_opt("SELECT verified,disabled,roles,permissions FROM platform.users WHERE id=$1 FOR NO KEY UPDATE", &[&actor.user_id()]).await?.ok_or_else(ApiError::unauthorized)?;
    let roles:Vec<String>=serde_json::from_value(user.get(2)).map_err(|_|ApiError::unavailable())?;
    let permissions:Vec<String>=serde_json::from_value(user.get(3)).map_err(|_|ApiError::unavailable())?;
    if !user.get::<_,bool>(0) || user.get::<_,bool>(1) || roles.iter().any(|r| matches!(r.as_str(),"banned"|"restricted")) || !permissions.iter().any(|p|p=="self.preferences.update") {
        return Err(ApiError::forbidden("This account cannot change ratings"));
    }
    let account_id=account_id_in(tx,actor.user_id()).await?;
    if let Some(expected)=expected_account {
        let expected=crate::public_api::PublicUuid::parse(expected).map_err(|_|ApiError::bad_request("Expected account must be a canonical user UUID"))?;
        if expected.as_str()!=account_id { return Err(ApiError::conflict("Account binding changed; refresh before submitting")); }
    }
    // Different owners of one title serialize here before observing the votes.
    tx.query_opt("SELECT id FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND state='published' AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id) FOR NO KEY UPDATE", &[&manga]).await?.ok_or_else(||ApiError::not_found("Published manga not found"))?;
    let previous=tx.query_opt("SELECT rating,version FROM platform.library_manga_ratings WHERE user_id=$1 AND manga_id=$2 FOR UPDATE", &[&actor.user_id(),&manga]).await?;
    let version=previous.as_ref().map_or(0,|r|r.get::<_,i32>(1));
    if expected.is_some_and(|e|e!=version) { return Err(ApiError::conflict("Rating version changed")); }
    let current=previous.as_ref().and_then(|r|r.get::<_,Option<i16>>(0));
    let changed=current!=rating;
    let version=if changed {
        let next=version.checked_add(1).ok_or_else(||ApiError::conflict("Rating version exhausted"))?;
        tx.execute("INSERT INTO platform.library_manga_ratings(user_id,manga_id,rating,version,rated_at) VALUES($1,$2,$3,$4,CASE WHEN $3::smallint IS NULL THEN NULL ELSE clock_timestamp() END) ON CONFLICT(user_id,manga_id) DO UPDATE SET rating=EXCLUDED.rating,version=EXCLUDED.version,rated_at=CASE WHEN EXCLUDED.rating IS NULL THEN NULL ELSE COALESCE(platform.library_manga_ratings.rated_at,EXCLUDED.rated_at) END,updated_at=clock_timestamp()", &[&actor.user_id(),&manga,&rating,&next]).await?;
        // Persist the exact mean for existing native/public catalogue ordering.
        tx.execute("INSERT INTO platform.catalog_manga_counters(manga_id,rating) SELECT $1,COALESCE(avg(rating),0) FROM platform.library_manga_ratings WHERE manga_id=$1 AND rating IS NOT NULL ON CONFLICT(manga_id) DO UPDATE SET rating=EXCLUDED.rating", &[&manga]).await?;
        next
    } else { version };
    Ok(json!({"result":"ok","nostr":{"scope":"instance-account","source":"persisted-native-account-ratings","accountId":account_id,"version":version,"changed":changed}}))
}

pub(crate) async fn get(State(state): State<AppState>, headers: HeaderMap, RawQuery(raw): RawQuery) -> ApiResult<Json<Value>> {
    let actor=auth::require_library_principal(&state,&headers).await?;
    let q=super::query::parse(raw)?;
    if q.values.keys().any(|k|k!="manga") { return Err(ApiError::bad_request("Rating reads accept only manga identifiers")); }
    let ids=super::validation::ids(&q.list("manga"),100)?;
    if ids.is_empty() || ids.len()>100 { return Err(ApiError::bad_request("Select 1..100 manga identifiers")); }
    for id in &ids { super::validation::id(id)?; }
    let mut db=state.db.client().await?;
    let tx=db.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).read_only(true).start().await?;
    let visible:i64=tx.query_one("SELECT count(*) FROM platform.catalog_manga WHERE id=ANY($1::text[]) AND deleted_at IS NULL AND state='published' AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id)",&[&ids]).await?.get(0);
    if visible as usize!=ids.len() {return Err(ApiError::not_found("Published manga not found"));}
    let rows=read_in(&tx,&actor.user_id,&ids).await?;
    let account_id=account_id_in(&tx,&actor.user_id).await?;
    let mut versions=ids.iter().map(|id|(id.clone(),0)).collect::<BTreeMap<_,_>>();
    for row in tx.query("SELECT r.manga_id,r.version FROM platform.library_manga_ratings r JOIN platform.catalog_manga m ON m.id=r.manga_id WHERE r.user_id=$1 AND r.manga_id=ANY($2::text[]) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)",&[&actor.user_id,&ids]).await? { versions.insert(row.get::<_,String>(0),row.get::<_,i32>(1)); }
    tx.commit().await?;
    Ok(Json(json!({"result":"ok","ratings":rows,"nostr":{"scope":"instance-account","source":"persisted-native-account-ratings","accountId":account_id,"versions":versions,"zeroVersionMeans":"no-persisted-rating-record"}})))
}
pub(crate) async fn set(State(state): State<AppState>, Path(id): Path<String>, headers: HeaderMap, Json(p): Json<RatingPayload>) -> ApiResult<Json<Value>> {
    native_mutate(state,headers,id,Some(p.rating),p.version,p.expected_account_id.as_deref()).await
}
pub(crate) async fn delete(State(state): State<AppState>, Path(id): Path<String>, headers: HeaderMap, request: Request) -> ApiResult<Json<Value>> {
    let body=tokio::time::timeout(std::time::Duration::from_secs(5),to_bytes(request.into_body(),1024)).await.map_err(|_|ApiError::bad_request("Request body timeout"))?.map_err(|_|ApiError::bad_request("Rating body exceeds 1024 bytes"))?;
    let p=if body.is_empty(){DeletePayload::default()}else{serde_json::from_slice::<DeletePayload>(&body).map_err(|_|ApiError::bad_request("Invalid rating delete JSON"))?};
    native_mutate(state,headers,id,None,p.version,p.expected_account_id.as_deref()).await
}
async fn native_mutate(state: AppState, headers: HeaderMap, id: String, rating: Option<i16>, version: Option<i32>, expected_account: Option<&str>) -> ApiResult<Json<Value>> {
    super::validation::id(&id)?;
    let actor=auth::require_principal(&state,&headers).await?;
    actor.require_browser_session()?; actor.require_verified()?;
    auth::validate_mutation(&state,&headers)?;
    let mut db=state.db.client().await?; let tx=db.transaction().await?;
    let result=mutate_in(&tx,&actor,&id,rating,version,expected_account).await?; tx.commit().await?;
    Ok(Json(result))
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn strict_rating_json_and_versions() {
        for value in [json!({"rating":0}),json!({"rating":11}),json!({"rating":1.5}),json!({"rating":"8"}),json!({"rating":8,"user":"other"})] {
            let valid=serde_json::from_value::<RatingPayload>(value).ok().is_some_and(|p|validate(Some(p.rating),p.version).is_ok()); assert!(!valid);
        }
        assert!(validate(Some(10),Some(0)).is_ok()); assert!(validate(None,Some(-1)).is_err());
        assert!(serde_json::from_str::<RatingPayload>("{\"rating\":1,\"rating\":2}").is_err());
    }
}
