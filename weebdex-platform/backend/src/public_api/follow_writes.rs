//! Native account follows, separate from original publisher graph membership.
use super::{PublicApiError,PublicApiResult,PublicUuid,PublicIdTarget,ResourceKind,PostgresPublicIdResolver,auth::{admitted_request,bound_account_write}};
use crate::{AppState,domains::library::follow_mutations::{self,Target}};
use axum::{Json,Router,body::to_bytes,extract::{OriginalUri,Path,Request,State},http::{HeaderMap,HeaderValue,header},middleware::Next,response::{IntoResponse,Response},routing::post};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

#[derive(Default,Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
struct Payload {
    #[serde(default,deserialize_with="present")] version:Option<i32>,
    #[serde(default,deserialize_with="present")] expected_account_id:Option<PublicUuid>
}
fn present<'de,D:serde::Deserializer<'de>,T:Deserialize<'de>>(d:D)->Result<Option<T>,D::Error> {T::deserialize(d).map(Some)}
pub fn router()->Router<AppState> {
    Router::new().route("/manga/{p0}/follow",post(manga_follow).delete(manga_unfollow))
        .route("/group/{p0}/follow",post(group_follow).delete(group_unfollow))
        // Upstream5.13.1 has user-follow reads, but no user follow write path.
        // This explicit instance extension uses the same native account domain.
        .route("/user/{p0}/follow",post(user_follow).delete(user_unfollow))
        .layer(axum::middleware::from_fn(private_headers))
}
async fn private_headers(request:Request,next:Next)->Response {
    let mut response=next.run(request).await;
    response.headers_mut().insert(header::CACHE_CONTROL,HeaderValue::from_static("private, no-store"));
    response.headers_mut().insert(header::VARY,HeaderValue::from_static("Authorization, Cookie"));response
}
macro_rules! handler {($name:ident,$target:ident,$kind:ident,$follow:expr)=>{
    async fn $name(State(state):State<AppState>,OriginalUri(uri):OriginalUri,Path(id):Path<String>,headers:HeaderMap,request:Request)->PublicApiResult<Response> {
        mutate(state,uri,id,headers,request,Target::$target,ResourceKind::$kind,$follow).await
    }
}}
handler!(manga_follow,Manga,Manga,true);handler!(manga_unfollow,Manga,Manga,false);
handler!(group_follow,Group,Group,true);handler!(group_unfollow,Group,Group,false);
handler!(user_follow,User,User,true);handler!(user_unfollow,User,User,false);
async fn mutate(state:AppState,uri:axum::http::Uri,id:String,headers:HeaderMap,request:Request,target:Target,kind:ResourceKind,follow:bool)->PublicApiResult<Response> {
    if uri.query().is_some() {return Err(PublicApiError::bad_request("Follow writes accept no query"));}
    let id=PublicUuid::parse(&id)?;
    let method=request.method().as_str().to_owned();
    let body=tokio::time::timeout(Duration::from_secs(5),to_bytes(request.into_body(),1024)).await.map_err(|_|PublicApiError::bad_request("Request body timeout"))?.map_err(|_|PublicApiError::bad_request("Follow body exceeds 1024 bytes"))?;
    if !body.is_empty() {
        let mut values=headers.get_all(header::CONTENT_TYPE).iter();
        if !values.next().and_then(|v|v.to_str().ok()).is_some_and(|v|v.split(';').next().is_some_and(|v|v.trim().eq_ignore_ascii_case("application/json"))) || values.next().is_some() {return Err(PublicApiError::bad_request("JSON content type required"));}
    }
    let cap=admitted_request(&state,&uri,&headers,&method,&body).await?;
    let payload=if body.is_empty(){Payload::default()}else{serde_json::from_slice::<Payload>(&body).map_err(|_|PublicApiError::bad_request("Invalid follow JSON"))?};
    if payload.version.is_some_and(|v|v<0) {return Err(PublicApiError::bad_request("Follow version must be nonnegative"));}
    let mut db=state.db.client().await?;let tx=db.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;actor.require_permission("self.preferences.update")?;
    let resolved=PostgresPublicIdResolver::resolve_many_in(&*tx,kind,&[id]).await?.into_values().next().filter(|v|v.target==PublicIdTarget::NativeResource).ok_or_else(PublicApiError::not_found)?;
    let mut metadata=follow_mutations::mutate_in(&tx,&actor,target,&resolved.native_id,follow,payload.version,payload.expected_account_id.as_ref().map(PublicUuid::as_str)).await?;
    metadata["targetId"]=json!(resolved.preferred_id);
    tx.commit().await?;
    Ok(Json(json!({"result":"ok","nostr":metadata})).into_response())
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn follow_payloads_are_strict() {
        for text in ["{\"version\":null}","{\"expectedAccountId\":null}","{\"version\":1,\"version\":2}","{\"version\":1.5}","{\"expectedAccountId\":\"native-id\"}","{\"userId\":\"other\"}"] {assert!(serde_json::from_str::<Payload>(text).is_err());}
        assert!(serde_json::from_str::<Payload>("{\"version\":0}").is_ok());
    }
}
