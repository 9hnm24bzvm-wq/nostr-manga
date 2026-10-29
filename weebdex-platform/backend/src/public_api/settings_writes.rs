//! Lossless known-setting merge into an explicitly bound native account.
use super::{PublicApiError,PublicApiResult,PublicUuid,PublicIdTarget,ResourceKind,PostgresPublicIdResolver,auth::{admitted_request,bound_account_write},account_settings};
use crate::{AppState,contracts::library::PreferencesPatch,domains::library::{account_public_id_in,preference_mutations as core}};
use axum::{Json,body::to_bytes,extract::{OriginalUri,Request,State},http::{HeaderMap,header},response::{IntoResponse,Response}};
use serde::Deserialize;
use serde_json::{Value,json};
use std::{collections::BTreeSet,time::Duration};

#[derive(Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub(super) struct Payload {pub(super) settings:PreferencesPatch,pub(super) updated_at:Option<String>,pub(super) expected_account_id:Option<PublicUuid>,pub(super) template:Option<PublicUuid>}

pub(super) fn request_shape(value:&Value)->PublicApiResult<()> {
    let fields=value.as_object().ok_or_else(||PublicApiError::bad_request("Settings body must be an object"))?;
    let settings=value["settings"].as_object().ok_or_else(||PublicApiError::bad_request("settings must be an object"))?;
    if fields.values().any(Value::is_null) || settings.values().any(Value::is_null) {return Err(PublicApiError::bad_request("Settings fields cannot be null"));}
    for values in settings.values().filter_map(Value::as_array) {
        let strings=values.iter().map(Value::as_str).collect::<Option<BTreeSet<_>>>().ok_or_else(||PublicApiError::bad_request("Setting arrays require unique strings"))?;
        if values.len()>100 || strings.len()!=values.len() {return Err(PublicApiError::bad_request("Setting arrays require at most 100 unique values"));}
    }
    Ok(())
}

pub(super) fn expected_time(value:&str)->PublicApiResult<String> {
    let value=value.strip_suffix('Z').or_else(||value.strip_suffix("+00:00")).unwrap_or(value);
    let (base,fraction)=value.split_once('.').unwrap_or((value,""));
    if !super::catalogue::valid_utc_datetime(base) || fraction.len()>6 || (!fraction.is_empty() && !fraction.bytes().all(|b|b.is_ascii_digit())) || value.ends_with('.') {return Err(PublicApiError::bad_request("updatedAt must be a UTC date/time from the settings preview"));}
    let mut fraction=fraction.to_owned();while fraction.len()<3 {fraction.push('0');}
    while fraction.len()>3 && fraction.ends_with('0') {fraction.pop();}
    Ok(format!("{base}.{fraction}Z"))
}
pub(super) async fn references(tx:&tokio_postgres::Transaction<'_>,p:&mut PreferencesPatch)->PublicApiResult<()> {
    for (kind,values) in [(ResourceKind::Tag,p.tags.as_mut()),(ResourceKind::Group,p.groups.as_mut()),(ResourceKind::User,p.uploaders.as_mut())] {
        let Some(values)=values else{continue};
        if values.len()>100 {return Err(PublicApiError::bad_request("At most 100 filter references are supported"));}
        let ids=values.iter().map(|s|PublicUuid::parse(s).map_err(PublicApiError::from)).collect::<PublicApiResult<BTreeSet<_>>>()?.into_iter().collect::<Vec<_>>();
        let resolved=PostgresPublicIdResolver::resolve_many_in(tx,kind,&ids).await?;
        if resolved.len()!=ids.len() || resolved.values().any(|r|r.target!=PublicIdTarget::NativeResource) {return Err(PublicApiError::bad_request("Unknown or wrong-kind native preference UUID"));}
        let mut seen=BTreeSet::new();
        *values=values.iter().map(|id|{let id=PublicUuid::parse(id)?;Ok(resolved[&id].native_id.clone())}).collect::<PublicApiResult<Vec<_>>>()?.into_iter().filter(|id|seen.insert(id.clone())).collect();
    }
    Ok(())
}
pub(super) async fn write(State(state):State<AppState>,OriginalUri(uri):OriginalUri,headers:HeaderMap,request:Request)->PublicApiResult<Response> {
    if uri.query().is_some() {return Err(PublicApiError::bad_request("Settings writes accept no query"));}
    let mut content=headers.get_all(header::CONTENT_TYPE).iter();
    if !content.next().and_then(|h|h.to_str().ok()).is_some_and(|h|h.split(';').next().is_some_and(|h|h.trim().eq_ignore_ascii_case("application/json"))) || content.next().is_some() {return Err(PublicApiError::bad_request("JSON content type required"));}
    let method=request.method().as_str().to_owned();
    let body=tokio::time::timeout(Duration::from_secs(5),to_bytes(request.into_body(),32768)).await.map_err(|_|PublicApiError::bad_request("Request body timeout"))?.map_err(|_|PublicApiError::bad_request("Settings body exceeds 32768 bytes"))?;
    let cap=admitted_request(&state,&uri,&headers,&method,&body).await?;
    let mut p=serde_json::from_slice::<Payload>(&body).map_err(|_|PublicApiError::bad_request("Invalid settings JSON"))?;
    // Typed deserialization above rejects duplicate fields. This separate shape
    // check enforces the template's nonnull optional values and unique arrays.
    request_shape(&serde_json::from_slice::<Value>(&body).map_err(|_|PublicApiError::bad_request("Invalid settings JSON"))?)?;
    if p.template.as_ref().is_some_and(|id|id.as_str()!=account_settings::TEMPLATE_ID) {return Err(PublicApiError::bad_request("Unknown native settings template"));}
    if p.settings.items_per_page.as_deref().is_some_and(|v|v.is_empty() || v.starts_with('0') || !v.bytes().all(|b|b.is_ascii_digit())) {return Err(PublicApiError::bad_request("items_per_page must be canonical decimal text"));}
    let expected_time=p.updated_at.as_deref().map(expected_time).transpose()?;
    let mut db=state.db.client().await?;let tx=db.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;
    let account_id=account_public_id_in(&tx,actor.user_id()).await?;
    if p.expected_account_id.as_ref().is_some_and(|id|id.as_str()!=account_id) {return Err(PublicApiError::conflict("Account binding changed; refresh before submitting"));}
    references(&tx,&mut p.settings).await?;
    let (version,patch)=core::patch(p.settings)?;
    let changed=core::update_in(&tx,&actor,version,patch,core::Mode::PublicPreserving,expected_time.as_deref()).await?;
    // Project before commit: a schema/alias failure cannot follow a committed
    // preference update with an error that falsely suggests nothing changed.
    let mut response=account_settings::read_in(&tx,actor.user_id()).await?;
    response["nostr"]["changed"]=json!(changed);
    response["nostr"]["nativeStorageUnchanged"]=json!(!changed);
    response["nostr"]["mutation"]=json!("known-field-merge");
    response["nostr"]["preservedUnavailableFilterValues"]=json!(true);
    response["nostr"]["preservedUnknownNativeFields"]=json!(true);
    tx.commit().await?;Ok(Json(response).into_response())
}
#[cfg(test)]mod tests {use super::*;#[test]fn timestamps_and_json_settings_are_strict(){assert_eq!(expected_time("2024-02-29T12:00:00").unwrap(),"2024-02-29T12:00:00.000Z");assert_eq!(expected_time("2024-02-29T12:00:00.123000+00:00").unwrap(),"2024-02-29T12:00:00.123Z");for value in ["bad","2025-02-29T00:00:00Z","2024-02-29T12:00:00+01:00","2024-02-29T12:00:00.Z"]{assert!(expected_time(value).is_err());}assert!(serde_json::from_str::<Payload>("{\"settings\":{\"version\":1,\"version\":2}}").is_err());assert!(serde_json::from_str::<Payload>("{\"settings\":{\"version\":1,\"private_secret\":true}}").is_err());}}
