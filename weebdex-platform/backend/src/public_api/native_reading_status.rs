//! Explicit bound-native-account status preview/write. Existing signed-library
//! GET /manga/status and device defaults retain their separate meanings.
use super::{PublicApiError,PublicApiResult,PublicUuid,PublicIdTarget,ResourceKind,PostgresPublicIdResolver,auth::{admitted_request,bound_account_read,bound_account_write},query::PublicQuery};
use crate::{AppState,contracts::library::ReadingStatus,domains::library::{account_public_id_in,reading_status}};
use axum::{Json,Router,body::to_bytes,extract::{OriginalUri,Path,Request,State},http::{HeaderMap,HeaderValue,header},middleware::Next,response::{IntoResponse,Response},routing::{get,post}};
use serde::Deserialize;
use serde_json::{Value,json};
use std::time::Duration;

#[derive(Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
struct Command {status:ReadingStatus,expected_account_id:PublicUuid,expected_manga_id:PublicUuid,version:i32}
pub fn router()->Router<AppState> {
    Router::new().route("/nostr/manga/{p0}/reading-status",get(preview))
        .route("/manga/{p0}/status",post(write))
        .layer(axum::middleware::from_fn(private_headers))
}
async fn private_headers(request:Request,next:Next)->Response {
    let mut response=next.run(request).await;
    response.headers_mut().insert(header::CACHE_CONTROL,HeaderValue::from_static("private, no-store"));
    response.headers_mut().append(header::VARY,HeaderValue::from_static("Authorization, Cookie"));response
}
fn envelope(state:Value,account:&str,manga:&PublicUuid)->Value {
    let mut metadata=state.as_object().expect("internal status object").clone();
    let status=metadata.remove("status").expect("internal status");
    metadata.insert("scope".into(),json!("instance-account"));metadata.insert("source".into(),json!("persisted-native-account-reading-status"));
    metadata.insert("accountId".into(),json!(account));metadata.insert("mangaId".into(),json!(manga));
    metadata.insert("versionSemantics".into(),json!("reading-status-change-revision"));
    json!({"result":"ok","status":status,"nostr":metadata})
}
async fn target(tx:&tokio_postgres::Transaction<'_>,id:&PublicUuid)->PublicApiResult<super::ResolvedPublicId> {
    PostgresPublicIdResolver::resolve_many_in(&*tx,ResourceKind::Manga,&[id.clone()]).await?.into_values().next()
        .filter(|r|r.target==PublicIdTarget::NativeResource).ok_or_else(PublicApiError::not_found)
}
async fn visible(tx:&tokio_postgres::Transaction<'_>,native:&str,write:bool)->PublicApiResult<()> {
    let suffix=if write {" FOR SHARE"} else {""};
    tx.query_opt(&format!("SELECT id FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND state='published' AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id){suffix}"),&[&native]).await?.ok_or_else(PublicApiError::not_found)?;Ok(())
}
async fn body(request:Request)->PublicApiResult<axum::body::Bytes> {
    tokio::time::timeout(Duration::from_secs(5),to_bytes(request.into_body(),1024)).await.map_err(|_|PublicApiError::bad_request("Status body timeout"))?.map_err(|_|PublicApiError::bad_request("Status body exceeds1024bytes"))
}
async fn preview(State(state):State<AppState>,OriginalUri(uri):OriginalUri,Path(id):Path<String>,headers:HeaderMap,request:Request)->PublicApiResult<Response> {
    let q=PublicQuery::parse_unchecked(uri.query())?;q.require_allowed(&["expectedAccountId"])?;
    let expected=PublicUuid::parse(q.first("expectedAccountId").ok_or_else(||PublicApiError::bad_request("expectedAccountId is required"))?)?;
    let id=PublicUuid::parse(&id)?;let method=request.method().as_str().to_owned();let bytes=body(request).await?;
    if !bytes.is_empty() {return Err(PublicApiError::bad_request("Status GET accepts no body"));}
    let cap=admitted_request(&state,&uri,&headers,&method,&bytes).await?;
    let mut db=state.db.client().await?;let tx=db.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).start().await?;
    let actor=bound_account_read(&tx,&cap).await?;actor.require_permission("self.preferences.update")?;
    let account=account_public_id_in(&tx,actor.user_id()).await?;
    if expected.as_str()!=account {return Err(PublicApiError::conflict("Account binding changed; refresh before reading status"));}
    let resolved=target(&tx,&id).await?;visible(&tx,&resolved.native_id,false).await?;
    let result=envelope(reading_status::state_in(&tx,actor.user_id(),&resolved.native_id,false).await?,&account,&resolved.preferred_id);
    tx.commit().await?;Ok(Json(result).into_response())
}
async fn write(State(state):State<AppState>,OriginalUri(uri):OriginalUri,Path(id):Path<String>,headers:HeaderMap,request:Request)->PublicApiResult<Response> {
    if uri.query().is_some() {return Err(PublicApiError::bad_request("Status writes accept no query"));}
    let id=PublicUuid::parse(&id)?;let method=request.method().as_str().to_owned();let bytes=body(request).await?;
    let mut types=headers.get_all(header::CONTENT_TYPE).iter();
    if !types.next().and_then(|v|v.to_str().ok()).is_some_and(|v|v.split(';').next().is_some_and(|v|v.trim().eq_ignore_ascii_case("application/json")))||types.next().is_some() {return Err(PublicApiError::bad_request("JSON content type required"));}
    let cap=admitted_request(&state,&uri,&headers,&method,&bytes).await?;
    let command:Command=serde_json::from_slice(&bytes).map_err(|_|PublicApiError::bad_request("Status requires one of six native values, expected account/manga UUIDs and an integer version; null clear is unsupported"))?;
    if command.version<0 {return Err(PublicApiError::bad_request("Status version must be nonnegative"));}
    let mut db=state.db.client().await?;let tx=db.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;actor.require_permission("self.preferences.update")?;
    let account=account_public_id_in(&tx,actor.user_id()).await?;
    if command.expected_account_id.as_str()!=account {return Err(PublicApiError::conflict("Account binding changed; refresh before submitting"));}
    let resolved=target(&tx,&id).await?;
    if command.expected_manga_id!=resolved.preferred_id {return Err(PublicApiError::conflict("Manga identity changed; refresh before submitting"));}
    visible(&tx,&resolved.native_id,true).await?;
    let result=envelope(reading_status::mutate_in(&tx,&actor,&resolved.native_id,command.status,command.version).await?,&account,&resolved.preferred_id);
    tx.commit().await?;Ok(Json(result).into_response())
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn status_commands_require_typed_capture_and_no_null_clear() {
        let id="11111111-1111-8111-8111-111111111111";
        let valid=json!({"status":"reading","expectedAccountId":id,"expectedMangaId":id,"version":1});
        assert!(serde_json::from_value::<Command>(valid.clone()).is_ok());
        for field in ["status","expectedAccountId","expectedMangaId","version"] {let mut x=valid.clone();x.as_object_mut().unwrap().remove(field);assert!(serde_json::from_value::<Command>(x).is_err());}
        for (field,value) in [("status",Value::Null),("status",json!("finished")),("version",Value::Null),("version",json!(1.5)),("version",json!(2147483648_i64)),("expectedAccountId",json!("native-id"))] {let mut x=valid.clone();x[field]=value;assert!(serde_json::from_value::<Command>(x).is_err());}
        assert!(serde_json::from_str::<Command>(&format!("{{\"status\":\"reading\",\"status\":\"completed\",\"expectedAccountId\":\"{id}\",\"expectedMangaId\":\"{id}\",\"version\":1}}")).is_err());
    }
}
