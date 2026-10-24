//! Compatibility DELETE prepares reversible disabling only. The confirmation
//! code identifies the challenge; neither it nor its token is account authority.
use super::{PublicApiError,PublicApiResult,PublicUuid,auth::{admitted_request,bound_account_write},query::PublicQuery};
use crate::{AppState,domains::{auth::closure as core,library::account_public_id_in}};
use axum::{Router,body::to_bytes,extract::{OriginalUri,Path,Request,State},http::{HeaderMap,header},response::Response,routing::{get,post}};
use std::time::Duration;
pub fn router()->Router<AppState> {
    Router::new().route("/nostr/user/{p0}/closure",get(snapshot))
        .route("/user/delete/{p0}",post(confirm))
}
async fn bytes(request:Request,headers:&HeaderMap,write:bool)->PublicApiResult<axum::body::Bytes> {
    if write {
        let mut types=headers.get_all(header::CONTENT_TYPE).iter();
        if !types.next().and_then(|v|v.to_str().ok()).is_some_and(|v|v.split(';').next().is_some_and(|v|v.trim().eq_ignore_ascii_case("application/json"))) || types.next().is_some() {
            return Err(PublicApiError::bad_request("JSON content type required"));
        }
    }
    let body=tokio::time::timeout(Duration::from_secs(5),to_bytes(request.into_body(),4096)).await.map_err(|_|PublicApiError::bad_request("Account confirmation body timeout"))?.map_err(|_|PublicApiError::bad_request("Account confirmation exceeds 4096 bytes"))?;
    if !write&&!body.is_empty(){return Err(PublicApiError::bad_request("Account preview GET accepts no body"));}
    Ok(body)
}
fn capture(c:&core::Capture,account:&str)->PublicApiResult<()> {
    c.validate()?;
    if PublicUuid::parse(&c.expected_account_id)?.as_str()!=account || PublicUuid::parse(&c.expected_user_id)?.as_str()!=account {
        return Err(PublicApiError::conflict("The explicitly selected bound account changed"));
    }
    Ok(())
}
async fn snapshot(State(state):State<AppState>,OriginalUri(uri):OriginalUri,Path(id):Path<String>,headers:HeaderMap,request:Request)->PublicApiResult<Response> {
    let q=PublicQuery::parse_unchecked(uri.query())?;q.require_allowed(&["expectedAccountId"])?;
    let expected=PublicUuid::parse(q.first("expectedAccountId").ok_or_else(||PublicApiError::bad_request("expectedAccountId is required"))?)?;
    let id=PublicUuid::parse(&id)?;let method=request.method().as_str().to_owned();let body=bytes(request,&headers,false).await?;
    let cap=admitted_request(&state,&uri,&headers,&method,&body).await?;
    let mut db=state.db.client().await?;let tx=db.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;let account=account_public_id_in(&tx,actor.user_id()).await?;
    if id.as_str()!=account || expected.as_str()!=account {return Err(PublicApiError::conflict("Select only your explicitly bound native account"));}
    let (name,version)=core::lock_account(&tx,actor.user_id()).await?;
    let result=core::snapshot_in(&tx,actor.user_id(),&account,&name,version).await?;
    tx.commit().await?;Ok(core::private(result))
}
pub(super) async fn prepare(State(state):State<AppState>,OriginalUri(uri):OriginalUri,Path(id):Path<String>,headers:HeaderMap,request:Request)->PublicApiResult<Response> {
    if uri.query().is_some(){return Err(PublicApiError::bad_request("Disable preparation accepts no query"));}
    let id=PublicUuid::parse(&id)?;let method=request.method().as_str().to_owned();let body=bytes(request,&headers,true).await?;
    let cap=admitted_request(&state,&uri,&headers,&method,&body).await?;
    let command:core::Capture=serde_json::from_slice(&body).map_err(|_|PublicApiError::bad_request("Invalid captured account-disable preparation"))?;
    let mut db=state.db.client().await?;let tx=db.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;let account=account_public_id_in(&tx,actor.user_id()).await?;capture(&command,&account)?;
    if id.as_str()!=account{return Err(PublicApiError::forbidden("Only the explicitly bound account can be disabled"));}
    let (_,version)=core::lock_account(&tx,actor.user_id()).await?;
    let result=core::prepare_in(&tx,actor.user_id(),&format!("publisher:{}",hex::encode(cap.principal())),&command,version).await?;
    tx.commit().await?;Ok(core::private(result))
}
async fn confirm(State(state):State<AppState>,OriginalUri(uri):OriginalUri,Path(code):Path<String>,headers:HeaderMap,request:Request)->PublicApiResult<Response> {
    if uri.query().is_some(){return Err(PublicApiError::bad_request("Disable confirmation accepts no query"));}
    let method=request.method().as_str().to_owned();let body=bytes(request,&headers,true).await?;
    let cap=admitted_request(&state,&uri,&headers,&method,&body).await?;
    let command:core::Confirm=serde_json::from_slice(&body).map_err(|_|PublicApiError::bad_request("Invalid account-disable confirmation"))?;
    if command.confirmation_id!=code{return Err(PublicApiError::conflict("The account-disable challenge changed"));}
    let mut db=state.db.client().await?;let tx=db.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;let account=account_public_id_in(&tx,actor.user_id()).await?;capture(&command.capture,&account)?;
    let (_,version)=core::lock_account(&tx,actor.user_id()).await?;
    let result=core::confirm_in(&tx,actor.user_id(),&format!("publisher:{}",hex::encode(cap.principal())),&command,version).await?;
    tx.commit().await?;Ok(core::private(result))
}
