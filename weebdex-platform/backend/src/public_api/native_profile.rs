//! Explicit self-only instance account profile, separate from original kind0.
use super::{PublicApiError,PublicApiResult,PublicUuid,PublicIdTarget,ResourceKind,PostgresPublicIdResolver,auth::{admitted_request,bound_account_read,bound_account_write},query::PublicQuery};
use crate::{AppState,contracts::library::ProfilePatch,domains::library::{account_public_id_in,profile_mutations as core}};
use axum::{Json,Router,body::to_bytes,extract::{OriginalUri,Path,Request,State},http::{HeaderMap,HeaderValue,header},middleware::Next,response::{IntoResponse,Response},routing::get};
use serde::Deserialize;
use serde_json::{Value,json};
use std::time::Duration;

#[derive(Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub(super) struct Command {
    pub(super) expected_account_id:PublicUuid,pub(super) expected_user_id:PublicUuid,version:i32,
    description:Option<String>,discord:Option<String>,twitter:Option<String>,website:Option<String>,
}
impl Command {
    pub(super) fn patch(self)->ProfilePatch {ProfilePatch{version:self.version,description:self.description,discord:self.discord,twitter:self.twitter,website:self.website}}
}
pub fn router()->Router<AppState> {
    Router::new().route("/nostr/user/{p0}/profile",get(preview))
        .layer(axum::middleware::from_fn(private_headers))
}
async fn private_headers(request:Request,next:Next)->Response {
    let mut response=next.run(request).await;
    response.headers_mut().insert(header::CACHE_CONTROL,HeaderValue::from_static("private, no-store"));
    response.headers_mut().append(header::VARY,HeaderValue::from_static("Authorization, Cookie"));response
}
pub(super) fn envelope(attributes:Value,account:&str,user:&PublicUuid,changed:Option<bool>)->Value {
    let mut meta=json!({"scope":"instance-account","source":"persisted-native-account-profile","accountId":account,"userId":user,"versionSemantics":"shared-native-user-revision"});
    if let Some(changed)=changed {meta["changed"]=json!(changed);}
    let mut provenance=meta.clone();provenance["identity"]=json!("native_public_user");
    json!({"result":"ok","response":"entity","data":{"id":user,"type":"user","attributes":attributes,"relationships":[],"nostr":provenance},"nostr":meta})
}
pub(super) async fn target(tx:&tokio_postgres::Transaction<'_>,id:&PublicUuid,actor:&str)->PublicApiResult<super::ResolvedPublicId> {
    let target=PostgresPublicIdResolver::resolve_many_in(&*tx,ResourceKind::User,&[id.clone()]).await?.into_values().next()
        .filter(|r|r.target==PublicIdTarget::NativeResource).ok_or_else(PublicApiError::not_found)?;
    if target.native_id!=actor {return Err(PublicApiError::forbidden("Only your explicitly bound native profile can be edited"));}
    Ok(target)
}
async fn body(request:Request)->PublicApiResult<axum::body::Bytes> {
    tokio::time::timeout(Duration::from_secs(5),to_bytes(request.into_body(),196608)).await.map_err(|_|PublicApiError::bad_request("Profile body timeout"))?.map_err(|_|PublicApiError::bad_request("Profile body exceeds196608bytes"))
}
async fn preview(State(state):State<AppState>,OriginalUri(uri):OriginalUri,Path(id):Path<String>,headers:HeaderMap,request:Request)->PublicApiResult<Response> {
    let q=PublicQuery::parse_unchecked(uri.query())?;q.require_allowed(&["expectedAccountId"])?;
    let expected=PublicUuid::parse(q.first("expectedAccountId").ok_or_else(||PublicApiError::bad_request("expectedAccountId is required"))?)?;
    let id=PublicUuid::parse(&id)?;let method=request.method().as_str().to_owned();let bytes=body(request).await?;
    if !bytes.is_empty() {return Err(PublicApiError::bad_request("Profile GET accepts no body"));}
    let cap=admitted_request(&state,&uri,&headers,&method,&bytes).await?;
    let mut db=state.db.client().await?;let tx=db.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).start().await?;
    let actor=bound_account_read(&tx,&cap).await?;actor.require_permission("self.user.update")?;
    let account=account_public_id_in(&tx,actor.user_id()).await?;
    if expected.as_str()!=account {return Err(PublicApiError::conflict("Account binding changed; refresh before reading profile"));}
    let resolved=target(&tx,&id,actor.user_id()).await?;
    let result=envelope(core::read_in(&*tx,&resolved.native_id).await?,&account,&resolved.preferred_id,None);
    tx.commit().await?;Ok(Json(result).into_response())
}
pub(super) async fn write(State(state):State<AppState>,OriginalUri(uri):OriginalUri,Path(id):Path<String>,headers:HeaderMap,request:Request)->PublicApiResult<Response> {
    if uri.query().is_some() {return Err(PublicApiError::bad_request("Profile writes accept no query"));}
    let id=PublicUuid::parse(&id)?;let method=request.method().as_str().to_owned();let bytes=body(request).await?;
    let mut types=headers.get_all(header::CONTENT_TYPE).iter();
    if !types.next().and_then(|v|v.to_str().ok()).is_some_and(|v|v.split(';').next().is_some_and(|v|v.trim().eq_ignore_ascii_case("application/json")))||types.next().is_some() {return Err(PublicApiError::bad_request("JSON content type required"));}
    let cap=admitted_request(&state,&uri,&headers,&method,&bytes).await?;
    // Typed parse rejects duplicate/unknown keys; separate shape refuses null
    // rather than conflating it with omitted (preserved) native fields.
    let command:Command=serde_json::from_slice(&bytes).map_err(|_|PublicApiError::bad_request("Invalid native profile command"))?;
    let shape:Value=serde_json::from_slice(&bytes).map_err(|_|PublicApiError::bad_request("Invalid native profile JSON"))?;
    if shape.as_object().ok_or_else(||PublicApiError::bad_request("Profile must be an object"))?.values().any(Value::is_null) {return Err(PublicApiError::bad_request("Profile fields cannot be null; empty strings clear"));}
    let mut db=state.db.client().await?;let tx=db.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;
    let account=account_public_id_in(&tx,actor.user_id()).await?;
    if command.expected_account_id.as_str()!=account {return Err(PublicApiError::conflict("Account binding changed; refresh before submitting"));}
    let resolved=target(&tx,&id,actor.user_id()).await?;
    if command.expected_user_id!=resolved.preferred_id {return Err(PublicApiError::conflict("User identity changed; refresh before submitting"));}
    core::update_in(&tx,&actor,&resolved.native_id,&command.patch()).await?;
    // Project before commit so malformed stored/public metadata cannot turn an
    // already committed update into an ambiguous projection failure.
    let result=envelope(core::read_in(&*tx,&resolved.native_id).await?,&account,&resolved.preferred_id,Some(true));
    tx.commit().await?;Ok(Json(result).into_response())
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn profile_command_requires_exact_capture_and_rejects_duplicate_authority() {
        let id="11111111-1111-8111-8111-111111111111";
        let valid=json!({"expectedAccountId":id,"expectedUserId":id,"version":1,"description":"Biography"});
        assert!(serde_json::from_value::<Command>(valid.clone()).is_ok());
        for field in ["expectedAccountId","expectedUserId","version"] {let mut x=valid.clone();x.as_object_mut().unwrap().remove(field);assert!(serde_json::from_value::<Command>(x).is_err());}
        for (field,value) in [("version",json!(1.5)),("version",json!(2147483648_i64)),("expectedUserId",json!("native-id")),("email",json!("private@example.invalid")),("permissions",json!([]))] {let mut x=valid.clone();x[field]=value;assert!(serde_json::from_value::<Command>(x).is_err());}
        assert!(serde_json::from_str::<Command>(&format!("{{\"expectedAccountId\":\"{id}\",\"expectedUserId\":\"{id}\",\"version\":1,\"version\":2}}")).is_err());
    }
}
