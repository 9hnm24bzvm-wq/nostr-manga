//! Private native-account follows of actual original signed public lists.
//! The follower's account UUID never becomes the list publisher or graph owner.
use super::{PublicApiError,PublicApiResult,PublicUuid,PublicIdTarget,ResourceKind,PostgresPublicIdResolver,auth::{admitted_request,bound_account_read,bound_account_write},query::{PublicQuery,QuerySchema}};
use crate::{AppState,domains::library::account_public_id_in as account_id_in};
use axum::{Json,Router,body::to_bytes,extract::{OriginalUri,Path,Request,State},http::{HeaderMap,HeaderValue,header},middleware::Next,response::{IntoResponse,Response},routing::{get,post}};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

const MAX_ACTIVE_FOLLOWS:usize=1000;
#[derive(Default,Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
struct FollowPayload {version:Option<i32>,expected_account_id:Option<PublicUuid>,expected_head:Option<String>}

pub fn router()->Router<AppState> {
    Router::new().route("/user/follows/list",get(collection))
        .route("/user/follows/list/{p0}",get(status))
        .route("/list/{p0}/follow",post(follow).delete(unfollow))
        .layer(axum::middleware::from_fn(private_headers))
}
async fn private_headers(request:Request,next:Next)->Response {
    let mut response=next.run(request).await;
    response.headers_mut().insert(header::CACHE_CONTROL,HeaderValue::from_static("private, no-store"));
    response.headers_mut().insert(header::VARY,HeaderValue::from_static("Authorization, Cookie"));response
}
fn query(raw:Option<&str>,paged:bool)->PublicApiResult<PublicQuery> {
    let q=PublicQuery::parse(raw,&QuerySchema {default_limit:10,max_limit:100,max_offset:10000,scalar:&[],arrays:&[],includes:&[],order:&[]})?;
    q.require_allowed(if paged{&["limit","offset"]}else{&[]})?;
    if paged && q.limit==0 {return Err(PublicApiError::bad_request("Follow list limit must be 1..100"));} Ok(q)
}
async fn bytes(request:Request,headers:&HeaderMap,write:bool)->PublicApiResult<Vec<u8>> {
    let body=tokio::time::timeout(Duration::from_secs(5),to_bytes(request.into_body(),1024)).await.map_err(|_|PublicApiError::bad_request("Request body timeout"))?.map_err(|_|PublicApiError::bad_request("Follow body exceeds 1024 bytes"))?;
    if !body.is_empty() {
        if !write {return Err(PublicApiError::bad_request("GET accepts no body"));}
        let mut content=headers.get_all(header::CONTENT_TYPE).iter();
        if !content.next().and_then(|h|h.to_str().ok()).is_some_and(|h|h.split(';').next().is_some_and(|h|h.trim().eq_ignore_ascii_case("application/json"))) || content.next().is_some() {return Err(PublicApiError::bad_request("JSON content type required"));}
    } Ok(body.to_vec())
}
async fn collection(State(state):State<AppState>,OriginalUri(uri):OriginalUri,headers:HeaderMap,request:Request)->PublicApiResult<Response> {read(state,uri,headers,request,None).await}
async fn status(State(state):State<AppState>,OriginalUri(uri):OriginalUri,Path(id):Path<String>,headers:HeaderMap,request:Request)->PublicApiResult<Response> {read(state,uri,headers,request,Some(id)).await}
async fn read(state:AppState,uri:axum::http::Uri,headers:HeaderMap,request:Request,id:Option<String>)->PublicApiResult<Response> {
    tokio::time::timeout(Duration::from_secs(30),read_inner(state,uri,headers,request,id)).await.map_err(|_|PublicApiError::unavailable())?
}
async fn read_inner(state:AppState,uri:axum::http::Uri,headers:HeaderMap,request:Request,id:Option<String>)->PublicApiResult<Response> {
    let q=query(uri.query(),id.is_none())?;
    let id=id.as_deref().map(PublicUuid::parse).transpose()?;
    let method=request.method().as_str().to_owned(); let body=bytes(request,&headers,false).await?;
    let cap=admitted_request(&state,&uri,&headers,&method,&body).await?;
    let mut db=state.db.client().await?;
    let tx=db.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).start().await?;
    let actor=bound_account_read(&tx,&cap).await?;
    let account_id=account_id_in(&tx,actor.user_id()).await?;
    if let Some(id)=id {
        let target=PostgresPublicIdResolver::resolve_many_in(&*tx,ResourceKind::List,&[id]).await?.into_values().next().filter(|r|matches!(r.target,PublicIdTarget::ProtocolList{..})).ok_or_else(PublicApiError::not_found)?;
        let row=tx.query_opt("SELECT version,platform.catalog_timestamp(followed_at),platform.catalog_timestamp(updated_at),source_head_at_follow FROM platform.library_public_list_follows WHERE user_id=$1 AND list_key=$2 AND followed",&[&actor.user_id(),&target.native_id]).await?.ok_or_else(PublicApiError::not_found)?;
        let (snapshot,_)=super::lists::followed_snapshot(&state,&target.preferred_id,None).await?.ok_or_else(PublicApiError::not_found)?;
        super::lists::verify_follow_snapshot(&state,&snapshot).await?;
        tx.commit().await?;
        return Ok(Json(json!({"result":"ok","nostr":{"scope":"instance-account","source":"persisted-native-account-protocol-list-follows","accountId":account_id,"listId":target.preferred_id,"version":row.get::<_,i32>(0),"followedAt":row.get::<_,String>(1),"updatedAt":row.get::<_,String>(2),"sourceHeadAtFollow":row.get::<_,String>(3),"head":snapshot.head}})).into_response());
    }
    let rows=tx.query("SELECT f.list_key,f.version,platform.catalog_timestamp(f.followed_at),platform.catalog_timestamp(f.updated_at),f.source_head_at_follow,p.public_id::text FROM platform.library_public_list_follows f LEFT JOIN platform.public_resource_ids p ON p.kind='list' AND p.native_id=f.list_key AND p.preferred AND p.principal_type='protocol_list' WHERE f.user_id=$1 AND f.followed ORDER BY f.followed_at DESC,p.public_id ASC LIMIT 1001",&[&actor.user_id()]).await?;
    if rows.len()>MAX_ACTIVE_FOLLOWS {return Err(PublicApiError::unavailable());}
    let mut data=Vec::new(); let mut snapshots=Vec::new(); let mut unavailable=0; let mut total=0; let mut relationships=0;
    for row in rows {
        let value:Option<&str>=row.get(5);
        let id=PublicUuid::parse(value.ok_or_else(PublicApiError::unavailable)?).map_err(|_|PublicApiError::unavailable())?;
        if let Some((mut snapshot,mut entity))=super::lists::followed_snapshot(&state,&id,None).await? {
            if total>=q.offset as usize && data.len()<q.limit as usize {
                relationships+=entity["relationships"].as_array().map_or(0,|r|r.len().saturating_sub(1));
                if relationships>5000 {return Err(PublicApiError::bad_request("Follow page exceeds 5000 manga relationships; reduce limit"));}
                entity["nostr"]["follow"]=json!({"version":row.get::<_,i32>(1),"followedAt":row.get::<_,String>(2),"updatedAt":row.get::<_,String>(3),"sourceHeadAtFollow":row.get::<_,String>(4)});
                data.push(entity);
            }
            total+=1;
            // Original verification checks only publisher/head. Do not retain
            // unselected source member expansions merely to recheck that head.
            snapshot.lists.clear(); snapshots.push(snapshot);
        } else {unavailable+=1;}
    }
    for snapshot in &snapshots {super::lists::verify_follow_snapshot(&state,snapshot).await?;}
    tx.commit().await?;
    Ok(Json(json!({"result":"ok","response":"collection","data":data,"limit":q.limit,"offset":q.offset,"total":total,"nostr":{"scope":"instance-account","source":"persisted-native-account-protocol-list-follows","accountId":account_id,"unavailableFollowCount":unavailable}})).into_response())
}
async fn follow(State(state):State<AppState>,OriginalUri(uri):OriginalUri,Path(id):Path<String>,headers:HeaderMap,request:Request)->PublicApiResult<Response> {mutate(state,uri,id,headers,request,true).await}
async fn unfollow(State(state):State<AppState>,OriginalUri(uri):OriginalUri,Path(id):Path<String>,headers:HeaderMap,request:Request)->PublicApiResult<Response> {mutate(state,uri,id,headers,request,false).await}
async fn mutate(state:AppState,uri:axum::http::Uri,id:String,headers:HeaderMap,request:Request,follow:bool)->PublicApiResult<Response> {
    if uri.query().is_some() {return Err(PublicApiError::bad_request("Follow writes accept no query"));}
    let id=PublicUuid::parse(&id)?;
    let method=request.method().as_str().to_owned(); let body=bytes(request,&headers,true).await?;
    let cap=admitted_request(&state,&uri,&headers,&method,&body).await?;
    let p=if body.is_empty(){FollowPayload::default()}else{serde_json::from_slice::<FollowPayload>(&body).map_err(|_|PublicApiError::bad_request("Invalid follow JSON"))?};
    if p.version.is_some_and(|v|v<0) || p.expected_head.as_deref().is_some_and(|h|!manga_node::auth::valid_hash(h)) {return Err(PublicApiError::bad_request("Invalid follow version or expected head"));}
    if !follow && p.expected_head.is_some() {return Err(PublicApiError::bad_request("Unfollow accepts no expected head; it also works after withdrawal"));}
    let mut db=state.db.client().await?;let tx=db.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;actor.require_permission("self.preferences.update")?;
    let account_id=account_id_in(&tx,actor.user_id()).await?;
    if p.expected_account_id.as_ref().is_some_and(|id|id.as_str()!=account_id) {return Err(PublicApiError::conflict("Account binding changed; refresh before submitting"));}
    let target=PostgresPublicIdResolver::resolve_many_in(&*tx,ResourceKind::List,&[id]).await?.into_values().next().filter(|r|matches!(r.target,PublicIdTarget::ProtocolList{..})).ok_or_else(PublicApiError::not_found)?;
    let snapshot=if follow {Some(super::lists::followed_snapshot(&state,&target.preferred_id,p.expected_head.as_deref()).await?.ok_or_else(PublicApiError::not_found)?.0)}else{None};
    // Unfollow remains possible after source withdrawal, with the same account
    // authority. Its typed persistent alias is still required.
    let previous=tx.query_opt("SELECT followed,version FROM platform.library_public_list_follows WHERE user_id=$1 AND list_key=$2 FOR UPDATE",&[&actor.user_id(),&target.native_id]).await?;
    let version=previous.as_ref().map_or(0,|r|r.get::<_,i32>(1));
    if p.version.is_some_and(|e|e!=version) {return Err(PublicApiError::conflict("Follow version changed"));}
    let changed=previous.as_ref().is_some_and(|r|r.get::<_,bool>(0))!=follow;
    let version=if changed {
        if follow {let count:i64=tx.query_one("SELECT count(*) FROM platform.library_public_list_follows WHERE user_id=$1 AND followed",&[&actor.user_id()]).await?.get(0);if count>=MAX_ACTIVE_FOLLOWS as i64{return Err(PublicApiError::bad_request("At most 1000 active public-list follows are supported"));}}
        let next=version.checked_add(1).ok_or_else(||PublicApiError::conflict("Follow version exhausted"))?;
        let head=snapshot.as_ref().and_then(|s|s.head.as_deref());
        tx.execute("INSERT INTO platform.library_public_list_follows(user_id,list_key,followed,version,followed_at,source_head_at_follow) VALUES($1,$2,$3,$4,CASE WHEN $3 THEN clock_timestamp() END,$5) ON CONFLICT(user_id,list_key) DO UPDATE SET followed=EXCLUDED.followed,version=EXCLUDED.version,followed_at=EXCLUDED.followed_at,source_head_at_follow=COALESCE(EXCLUDED.source_head_at_follow,platform.library_public_list_follows.source_head_at_follow),updated_at=clock_timestamp()",&[&actor.user_id(),&target.native_id,&follow,&next,&head]).await?;next
    }else{version};
    if let Some(snapshot)=&snapshot {super::lists::verify_follow_snapshot(&state,snapshot).await?;}
    let head=snapshot.as_ref().and_then(|s|s.head.as_ref());
    let result=json!({"result":"ok","nostr":{"scope":"instance-account","source":"persisted-native-account-protocol-list-follows","accountId":account_id,"listId":target.preferred_id,"version":version,"changed":changed,"followed":follow,"head":head}});
    tx.commit().await?;Ok(Json(result).into_response())
}
#[cfg(test)]mod tests {use super::*;#[test]fn follow_queries_and_preconditions_are_strict(){assert_eq!(query(None,true).unwrap().limit,10);for raw in ["limit=0","limit=101","offset=10001","includes[]=user","head=bad"]{assert!(query(Some(raw),true).is_err());}assert!(query(Some("limit=1"),false).is_err());assert!(serde_json::from_str::<FollowPayload>("{\"version\":1,\"version\":2}").is_err());}}
