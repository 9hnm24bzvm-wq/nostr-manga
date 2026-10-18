//! Private persisted native-account ratings, never inferred from aggregates.
use super::{PublicApiError, PublicApiResult, PublicIdTarget, PublicUuid, ResourceKind, PostgresPublicIdResolver, auth::{admitted_request,bound_account_read,bound_account_write}, query::{PublicQuery,QuerySchema}};
use crate::{AppState, domains::library::ratings as core};
use crate::domains::library::account_public_id_in;
use axum::{Json,Router,body::to_bytes,extract::{OriginalUri,Path,Request,State},http::{HeaderMap,HeaderValue,header},middleware::Next,response::{IntoResponse,Response},routing::{get,post}};
use serde_json::json;
use std::{collections::{BTreeMap,BTreeSet},time::Duration};

pub fn router() -> Router<AppState> {
    Router::new().route("/rating",get(read)).route("/rating/{p0}",post(set).delete(delete))
        .layer(axum::middleware::from_fn(private_headers))
}
async fn private_headers(request:Request,next:Next)->Response {
    let mut response=next.run(request).await;
    response.headers_mut().insert(header::CACHE_CONTROL,HeaderValue::from_static("private, no-store"));
    response.headers_mut().insert(header::VARY,HeaderValue::from_static("Authorization, Cookie")); response
}
fn selected(raw:Option<&str>)->PublicApiResult<Vec<PublicUuid>> {
    let q=PublicQuery::parse(raw,&QuerySchema {default_limit:0,max_limit:0,max_offset:0,scalar:&[],arrays:&["manga","manga[]"],includes:&[],order:&[]})?;
    q.require_allowed(&["manga","manga[]"])?;
    let plain=q.list("manga"); let brackets=q.list("manga[]");
    if !plain.is_empty() && !brackets.is_empty() {return Err(PublicApiError::bad_request("Use one manga array spelling"));}
    let values=if plain.is_empty(){brackets}else{plain};
    if values.is_empty() {return Err(PublicApiError::bad_request("Select 1..100 manga UUIDs"));}
    values.iter().map(|id|PublicUuid::parse(id).map_err(PublicApiError::from)).collect::<PublicApiResult<BTreeSet<_>>>().map(|ids|ids.into_iter().collect())
}
async fn body(request:Request,headers:&HeaderMap,allow_json:bool)->PublicApiResult<Vec<u8>> {
    let bytes=tokio::time::timeout(Duration::from_secs(5),to_bytes(request.into_body(),1024)).await
        .map_err(|_|PublicApiError::bad_request("Request body timeout"))?
        .map_err(|_|PublicApiError::bad_request("Rating body exceeds 1024 bytes"))?;
    if !bytes.is_empty() {
        if !allow_json {return Err(PublicApiError::bad_request("GET accepts no body"));}
        let mut types=headers.get_all(header::CONTENT_TYPE).iter();
        let valid=types.next().and_then(|v|v.to_str().ok()).is_some_and(|v|v.split(';').next().is_some_and(|v|v.trim().eq_ignore_ascii_case("application/json")));
        if !valid || types.next().is_some() {return Err(PublicApiError::bad_request("JSON content type required"));}
    }
    Ok(bytes.to_vec())
}
async fn read(State(state):State<AppState>,OriginalUri(uri):OriginalUri,headers:HeaderMap,request:Request)->PublicApiResult<Response> {
    let ids=selected(uri.query())?;
    let method=request.method().as_str().to_owned(); let bytes=body(request,&headers,false).await?;
    let cap=admitted_request(&state,&uri,&headers,&method,&bytes).await?;
    let mut db=state.db.client().await?;
    let tx=db.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).start().await?;
    let actor=bound_account_read(&tx,&cap).await?;
    let account_id=account_public_id_in(&tx,actor.user_id()).await?;
    let aliases=PostgresPublicIdResolver::resolve_many_in(&*tx,ResourceKind::Manga,&ids).await?;
    if aliases.len()!=ids.len() || aliases.values().any(|r|r.target!=PublicIdTarget::NativeResource) {return Err(PublicApiError::not_found());}
    let targets=aliases.into_values().map(|r|(r.native_id,r.preferred_id)).collect::<BTreeMap<_,_>>();
    let natives=targets.keys().cloned().collect::<Vec<_>>();
    let visible:i64=tx.query_one(&format!("SELECT count(*) FROM platform.catalog_manga m WHERE m.id=ANY($1::text[]) AND {}",super::catalogue::MANGA_VISIBLE), &[&natives]).await?.get(0);
    if visible as usize!=natives.len() {return Err(PublicApiError::not_found());}
    let ratings=core::read_in(&tx,actor.user_id(),&natives).await?.into_iter().map(|(native,value)|(targets[&native].clone(),value)).collect::<BTreeMap<_,_>>();
    let mut versions=targets.values().map(|id|(id.clone(),0)).collect::<BTreeMap<_,_>>();
    for row in tx.query("SELECT manga_id,version FROM platform.library_manga_ratings WHERE user_id=$1 AND manga_id=ANY($2::text[])", &[&actor.user_id(),&natives]).await? {
        versions.insert(targets[&row.get::<_,String>(0)].clone(),row.get::<_,i32>(1));
    }
    tx.commit().await?;
    Ok(Json(json!({"result":"ok","ratings":ratings,"nostr":{"scope":"instance-account","source":"persisted-native-account-ratings","accountId":account_id,"versions":versions,"zeroVersionMeans":"no-persisted-rating-record"}})).into_response())
}
async fn set(State(state):State<AppState>,OriginalUri(uri):OriginalUri,Path(id):Path<String>,headers:HeaderMap,request:Request)->PublicApiResult<Response> {mutate(state,uri,id,headers,request,false).await}
async fn delete(State(state):State<AppState>,OriginalUri(uri):OriginalUri,Path(id):Path<String>,headers:HeaderMap,request:Request)->PublicApiResult<Response> {mutate(state,uri,id,headers,request,true).await}
async fn mutate(state:AppState,uri:axum::http::Uri,id:String,headers:HeaderMap,request:Request,delete:bool)->PublicApiResult<Response> {
    if uri.query().is_some() {return Err(PublicApiError::bad_request("Rating writes accept no query"));}
    let id=PublicUuid::parse(&id)?;
    let method=request.method().as_str().to_owned(); let bytes=body(request,&headers,true).await?;
    let cap=admitted_request(&state,&uri,&headers,&method,&bytes).await?;
    let (rating,version,expected_account)=if delete {
        let p=if bytes.is_empty(){core::DeletePayload::default()}else{serde_json::from_slice::<core::DeletePayload>(&bytes).map_err(|_|PublicApiError::bad_request("Invalid rating delete JSON"))?}; (None,p.version,p.expected_account_id)
    } else {
        let p=serde_json::from_slice::<core::RatingPayload>(&bytes).map_err(|_|PublicApiError::bad_request("Invalid rating JSON"))?; (Some(p.rating),p.version,p.expected_account_id)
    };
    core::validate(rating,version)?;
    let mut db=state.db.client().await?; let tx=db.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;
    let target=PostgresPublicIdResolver::resolve_many_in(&*tx,ResourceKind::Manga,&[id]).await?.into_values().next().filter(|r|r.target==PublicIdTarget::NativeResource).ok_or_else(PublicApiError::not_found)?;
    let mut result=core::mutate_in(&tx,&actor,&target.native_id,rating,version,expected_account.as_deref()).await?;
    result["nostr"]["mangaId"]=json!(target.preferred_id);
    tx.commit().await?; Ok(Json(result).into_response())
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn rating_selection_is_typed_and_bounded() {
        let id="11111111-1111-8111-8111-111111111111";
        assert_eq!(selected(Some(&format!("manga[]={id}&manga[]={id}"))).unwrap().len(),1);
        assert!(selected(Some(&format!("manga={id}"))).is_ok());
        for raw in ["".into(),"manga[]=native-id".into(),format!("manga[]={id}&limit=0"),format!("manga={id}&manga[]={id}"),std::iter::repeat_n(format!("manga[]={id}"),101).collect::<Vec<_>>().join("&")] {assert!(selected(Some(&raw)).is_err(),"{raw}");}
    }
}
