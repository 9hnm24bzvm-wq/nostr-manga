//! App-owned popularity extensions; never a fabricated MangaDex ranking.
use super::{catalogue, catalogue_dto::AliasIndex, dto::{CollectionResponse, Resource}, query::{PublicQuery,QuerySchema}, PublicApiError, PublicApiResult, ResourceKind};
use crate::{AppState, client_peer::ClientPeer, domains::{analytics, catalog::{query::{QueryParams, Sql}, ranking_at}}};
use axum::{Extension, Json, Router, body::to_bytes, extract::{Path, RawQuery, Request, State}, http::{HeaderMap,HeaderValue,StatusCode,header}, response::{Response,IntoResponse}, routing::{get, post}};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/manga/top", get(manga))
        .route("/chapter/top", get(chapter))
        .route("/analytics/manga/{p0}/views", post(manga_view))
        .route("/analytics/chapter/{p0}/views", post(chapter_view))
}
fn query(kind: ResourceKind, raw: Option<&str>) -> PublicApiResult<(PublicQuery, String, i64)> {
    let base=catalogue::schema(kind);
    let scalar=base.scalar.iter().copied().chain(["rank","time"]).collect::<Vec<_>>();
    let schema=QuerySchema{default_limit:10,max_limit:10,max_offset:0,scalar:&scalar,arrays:base.arrays,includes:base.includes,order:&[]};
    let q=PublicQuery::parse(raw,&schema)?;
    if q.first("rank").unwrap_or("read")!="read" {return Err(PublicApiError::bad_request("Popularity requires the genuine read metric"));}
    let time=q.first("time").unwrap_or("30d").to_owned();
    let seconds=match time.as_str(){"24h"=>86400,"7d"=>604800,"30d"=>2592000,_=>return Err(PublicApiError::bad_request("Popularity time must be 24h, 7d or 30d"))};
    Ok((q,time,seconds))
}
async fn manga(State(state): State<AppState>,RawQuery(raw):RawQuery)->PublicApiResult<Json<CollectionResponse<Resource>>>{read(state,ResourceKind::Manga,raw).await}
async fn chapter(State(state): State<AppState>,RawQuery(raw):RawQuery)->PublicApiResult<Json<CollectionResponse<Resource>>>{read(state,ResourceKind::Chapter,raw).await}
async fn read(state:AppState,kind:ResourceKind,raw:Option<String>)->PublicApiResult<Json<CollectionResponse<Resource>>>{
    let (q,time,seconds)=query(kind,raw.as_deref())?;
    let mut db=state.db.client().await?;
    let tx=db.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).read_only(true).start().await?;
    let mut sql=Sql::default();catalogue::filters(&state,Some(&tx),kind,&q,&mut sql,None,None).await?;
    let (alias,table,projection,_)=catalogue::parts(kind)?;
    let metric_kind=if kind==ResourceKind::Manga{"manga"}else{"chapter"};
    let mut values=BTreeMap::new();values.insert("rank".into(),vec!["read".into()]);values.insert("time".into(),vec![time.clone()]);
    let native=QueryParams{values,limit:i64::from(q.limit),page:1};
    let score=ranking_at(&native,&mut sql,metric_kind,&format!("{alias}.id"),"(SELECT observed_at FROM moment)")?;
    let limit=sql.bind(i64::from(q.limit));
    let statement=format!("WITH moment AS MATERIALIZED(SELECT statement_timestamp() observed_at),scored AS MATERIALIZED(SELECT {alias}.id,{score} views FROM {table} WHERE {}),matched AS MATERIALIZED(SELECT * FROM scored WHERE views>0),selected AS(SELECT * FROM matched ORDER BY views DESC,id ASC LIMIT {limit}) SELECT (SELECT count(*) FROM matched),to_char((SELECT observed_at FROM moment) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),COALESCE((SELECT jsonb_agg((SELECT {projection} FROM {table} WHERE {alias}.id=selected.id) || jsonb_build_object('_popularity_views',selected.views::text) ORDER BY selected.views DESC,selected.id ASC) FROM selected),'[]'::jsonb)",sql.predicate());
    let row=tx.query_one(&statement,&sql.params()).await?;let total:i64=row.get(0);let observed:String=row.get(1);
    let native:Vec<Value>=serde_json::from_value(row.get(2)).map_err(|_|PublicApiError::unavailable())?;
    let resources=native.into_iter().map(|v|(kind,v)).collect::<Vec<_>>();let index=AliasIndex::load_in(&tx,&resources).await?;
    let data=resources.iter().map(|(kind,value)|{
        let mut resource=index.resource(*kind,value,&q.includes)?;
        let views=value.get("_popularity_views").and_then(Value::as_str).ok_or_else(PublicApiError::unavailable)?;
        let parsed=views.parse::<i64>().map_err(|_|PublicApiError::unavailable())?;if parsed<=0{return Err(PublicApiError::unavailable());}
        let attributes=resource.attributes.as_object_mut().ok_or_else(PublicApiError::unavailable)?;
        let nostr=attributes.entry("nostr").or_insert_with(||json!({})).as_object_mut().ok_or_else(PublicApiError::unavailable)?;
        nostr.insert("popularity".into(),json!({"views":views,"rank":"read","time":time}));Ok(resource)
    }).collect::<PublicApiResult<Vec<_>>>()?;
    let mut result=CollectionResponse::new(data,q.limit,0,total as u64);
    result.extensions.insert("nostr".into(),json!({"scope":"instance-catalogue","source":"persisted-native-view-events","metric":"deduplicated-views","rank":"read","time":time,"windowSeconds":seconds,"observedAt":observed,"order":"window-views-desc-native-id-asc"}));
    tx.commit().await?;Ok(Json(result))
}
async fn manga_view(State(state):State<AppState>,Path(id):Path<String>,RawQuery(raw):RawQuery,headers:HeaderMap,peer:Option<Extension<ClientPeer>>,request:Request)->PublicApiResult<Response>{view(state,ResourceKind::Manga,id,raw,headers,peer,request).await}
async fn chapter_view(State(state):State<AppState>,Path(id):Path<String>,RawQuery(raw):RawQuery,headers:HeaderMap,peer:Option<Extension<ClientPeer>>,request:Request)->PublicApiResult<Response>{view(state,ResourceKind::Chapter,id,raw,headers,peer,request).await}
async fn view(state:AppState,kind:ResourceKind,id:String,raw:Option<String>,headers:HeaderMap,peer:Option<Extension<ClientPeer>>,request:Request)->PublicApiResult<Response>{
    if raw.is_some(){return Err(PublicApiError::bad_request("View recording accepts no query"));}
    let body=tokio::time::timeout(std::time::Duration::from_secs(5),to_bytes(request.into_body(),1)).await.map_err(|_|PublicApiError::bad_request("View request body timeout"))?.map_err(|_|PublicApiError::bad_request("View recording accepts no body"))?;
    if !body.is_empty(){return Err(PublicApiError::bad_request("View recording accepts no body"));}
    let native=catalogue::resolve(&state,kind,&id).await?;
    match analytics::record_view(&state,&headers,peer.map(|p|p.0),if kind==ResourceKind::Manga{"manga"}else{"chapter"},&native,"/api/v1/analytics").await{
        Ok(response)=>Ok(response),
        Err(error) if error.status==StatusCode::TOO_MANY_REQUESTS=>{let mut response=PublicApiError::from(error).into_response();response.headers_mut().insert(header::RETRY_AFTER,HeaderValue::from_static("1"));Ok(response)},
        Err(error)=>Err(error.into()),
    }
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn popularity_is_bounded_and_cannot_substitute_metrics(){
        for kind in [ResourceKind::Manga,ResourceKind::Chapter]{
            let (q,time,seconds)=query(kind,None).unwrap();assert_eq!((q.limit,q.offset,time.as_str(),seconds),(10,0,"30d",2592000));
            for raw in ["rank=follow","rank=views","time=all","time=1d","time=7d&time=30d","limit=11","offset=1","order[title]=asc","sort=views"]{assert!(query(kind,Some(raw)).is_err(),"{raw}");}
            for time in ["24h","7d","30d"]{assert!(query(kind,Some(&format!("rank=read&time={time}&limit=0&offset=0"))).is_ok());}
        }
    }
}
