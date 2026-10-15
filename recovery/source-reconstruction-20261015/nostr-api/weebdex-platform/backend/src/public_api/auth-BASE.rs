//! RECONSTRUCTED UNVERIFIED auth/check and principal verifier base. Merge with
//! foundation's recovered BoundAccountRead/BoundAccountWrite; do not replace it.
use super::{PublicApiError,PublicApiResult};use crate::state::AppState;
use axum::{Json,Router,body::to_bytes,extract::{OriginalUri,Request,State},http::{HeaderMap,StatusCode,header},routing::get};
use manga_node::http_auth::{AdmittedHttpCapability,HttpAuthError,HttpService};use serde_json::{Value,json};use std::time::Duration;
pub fn router()->Router<AppState>{Router::new().route("/auth/check",get(check))}
fn error(status:StatusCode,title:&'static str,detail:&'static str)->PublicApiError{PublicApiError{status,title,detail:detail.into()}}
pub(super)fn map_error(source:HttpAuthError)->PublicApiError{match source{
 HttpAuthError::Invalid|HttpAuthError::Replay=>error(StatusCode::UNAUTHORIZED,"Authentication required","A fresh Nostr proof bound to this exact request is required"),
 HttpAuthError::Denied=>error(StatusCode::FORBIDDEN,"Permission denied","The Nostr principal is not admitted on this instance"),
 HttpAuthError::Service|HttpAuthError::Unavailable=>error(StatusCode::SERVICE_UNAVAILABLE,"Service unavailable","The configured Nostr authorization service is unavailable"),}}
pub async fn admitted_request(state:&AppState,uri:&axum::http::Uri,headers:&HeaderMap,method:&str,body:&[u8])->PublicApiResult<AdmittedHttpCapability>{
 let mut values=headers.get_all(header::AUTHORIZATION).iter();let authorization=values.next().and_then(|v|v.to_str().ok()).ok_or_else(||error(StatusCode::UNAUTHORIZED,"Authentication required","Nostr request authorization is required"))?;
 if values.next().is_some(){return Err(map_error(HttpAuthError::Invalid));}
 let publication=state.config.protocol_publication.as_ref().filter(|c|c.native_node.is_some()).ok_or_else(||map_error(HttpAuthError::Service))?;
 let service=HttpService::new(&state.config.public_origin,&publication.storage_holder).map_err(map_error)?;
 state.protocol.verify_http_request(&service,authorization,&uri.to_string(),method,body).await.map_err(map_error)
}
async fn check(State(state):State<AppState>,OriginalUri(uri):OriginalUri,headers:HeaderMap,request:Request)->PublicApiResult<Json<Value>>{
 if uri.query().is_some(){return Err(PublicApiError::bad_request("This operation accepts no query parameters"));}
 let method=request.method().as_str().to_owned();let body=tokio::time::timeout(Duration::from_secs(5),to_bytes(request.into_body(),10240)).await.map_err(|_|PublicApiError::bad_request("Request body timeout"))?.map_err(|_|PublicApiError::bad_request("Request body exceeds the operation budget"))?;
 let principal=admitted_request(&state,&uri,&headers,&method,&body).await?;
 Ok(Json(json!({"result":"ok","isAuthenticated":true,"roles":[],"permissions":[],"nostr":{"pubkey":hex::encode(principal.principal()),"serviceKey":hex::encode(principal.service()),"authentication":"NIP-98","requestScoped":true,"nativeSessionAuthority":false}})))
}
