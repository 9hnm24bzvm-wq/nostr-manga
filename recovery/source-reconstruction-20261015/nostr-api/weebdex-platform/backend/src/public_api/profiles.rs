//! Typed public presentation only; never principal, admission or signing authority.
use super::{PostgresPublicIdResolver,PublicApiError,PublicApiResult,PublicIdResolver,PublicIdTarget,PublicUuid,ResourceKind};
use crate::{domains::library::public_user_profile,error::ApiError,state::AppState};
use axum::{Json,Router,extract::{Path,RawQuery,State},routing::get};
use serde_json::{Value,json};
pub fn router()->Router<AppState>{Router::new().route("/user/{id}",get(one))}
async fn one(State(state):State<AppState>,Path(value):Path<String>,RawQuery(raw):RawQuery)->PublicApiResult<Json<Value>>{
    if raw.as_deref().is_some_and(|raw|!raw.is_empty()){return Err(PublicApiError::bad_request("User profile GET does not declare query parameters"));}
    let resolver=PostgresPublicIdResolver::new(state.db.clone());
    let target=resolver.resolve(ResourceKind::User,&PublicUuid::parse(&value)?).await?.ok_or_else(PublicApiError::not_found)?;
    let (attributes,provenance)=match target.target {
        PublicIdTarget::ProtocolPublisher{pubkey}=>{
            let publisher=hex::decode(&pubkey).ok().and_then(|value|value.try_into().ok()).ok_or_else(||PublicApiError::from(ApiError::unavailable()))?;
            let profile=state.protocol.public_account_profile(publisher).await?;
            if !profile.known{return Err(PublicApiError::not_found());}
            let name=profile.display_name().unwrap_or(&pubkey).to_owned();
            let fallback=profile.display_name().is_none();
            (json!({"username":name,"roles":[]}),json!({"pubkey":pubkey,"known":true,"profile":profile.event.as_deref(),"metadata":profile.metadata,"nameIsPublicKeyFallback":fallback,"identity":"protocol_publisher","versionSemantics":"No mutable revision counter exists in original kind0 metadata; version is omitted"}))
        }
        PublicIdTarget::NativeResource=>{
            let profile=public_user_profile(&state.db,&target.native_id).await?.ok_or_else(PublicApiError::not_found)?;
            (json!({"username":profile.username,"roles":profile.roles,"version":profile.version}),json!({"identity":"native_public_user"}))
        }
        PublicIdTarget::ProtocolList{..}=>return Err(PublicApiError::not_found()),
    };
    Ok(Json(json!({"result":"ok","response":"entity","data":{"id":target.preferred_id,"type":"user","attributes":attributes,"relationships":[],"nostr":provenance}})))
}
