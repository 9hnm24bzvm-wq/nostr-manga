//! Versioned, read-only UUID-to-original mapping. Local saves never need this
//! endpoint. Explicit original publication/social actions use only live verified
//! source identities; aliases never become keys, signatures or permissions.
use super::{PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdTarget, PublicUuid, ResourceKind};
use crate::AppState;
use axum::{Json,Router,body::to_bytes,extract::{OriginalUri,Request,State},http::{HeaderMap,StatusCode,header},routing::post};
use serde::Deserialize;
use serde_json::{Value,json};
use std::{collections::{BTreeMap,BTreeSet},time::Duration};

pub fn router()->Router<AppState>{Router::new().route("/nostr/resources",post(resolve))}
#[derive(Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
struct MappingRequest { schema_version:u8, catalog_namespace:PublicUuid, resources:Vec<Reference> }
#[derive(Clone,Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference { kind:String,id:PublicUuid }
fn validate(request:&MappingRequest)->PublicApiResult<()> {
    if request.schema_version!=2 {return Err(PublicApiError::bad_request("Use resource mapping schemaVersion2"));}
    if request.resources.len()>256 {return Err(PublicApiError::bad_request("At most256 explicit resources may be mapped"));}
    let mut seen=BTreeSet::new();
    for row in &request.resources {if !matches!(row.kind.as_str(),"manga"|"chapter") || !seen.insert((&row.kind,&row.id)){return Err(PublicApiError::bad_request("Invalid or duplicate typed UUID resource"));}}
    Ok(())
}
fn hash(text:&str)->PublicApiResult<[u8;32]>{if !manga_node::auth::valid_hash(text){return Err(PublicApiError::unavailable());}hex::decode(text).ok().and_then(|v|v.try_into().ok()).ok_or_else(PublicApiError::unavailable)}
async fn resolve(State(state):State<AppState>,OriginalUri(uri):OriginalUri,headers:HeaderMap,request:Request)->PublicApiResult<Json<Value>>{
    if uri.query().is_some(){return Err(PublicApiError::bad_request("Resource mapping accepts no query parameters"));}
    let types=headers.get_all(header::CONTENT_TYPE).iter().collect::<Vec<_>>();
    if types.len()!=1 || types[0].to_str().ok().and_then(|v|v.split(';').next()).is_none_or(|v|!v.trim().eq_ignore_ascii_case("application/json")) {return Err(PublicApiError::new(StatusCode::UNSUPPORTED_MEDIA_TYPE,"Unsupported Media Type","Expected application/json"));}
    let bytes=tokio::time::timeout(Duration::from_secs(5),to_bytes(request.into_body(),48*1024)).await.map_err(|_|PublicApiError::bad_request("Mapping body timeout"))?.map_err(|_|PublicApiError::bad_request("Mapping request exceeds48KiB"))?;
    let request:MappingRequest=serde_json::from_slice(&bytes).map_err(|_|PublicApiError::bad_request("Invalid versioned UUID mapping request"))?;validate(&request)?;
    if !state.protocol.is_ready(){return Err(PublicApiError::unavailable());}
    let mut client=state.db.client().await?;
    let tx=client.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).read_only(true).start().await?;
    tx.batch_execute("SET LOCAL statement_timeout='2000ms';SET LOCAL lock_timeout='500ms'").await?;
    let namespace:String=tx.query_one("SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton",&[]).await?.get(0);
    if request.catalog_namespace.as_str()!=namespace {return Err(PublicApiError::conflict("Catalogue namespace changed; no saved UUID was reinterpreted"));}
    let mut aliases=BTreeMap::new();
    for (name,kind) in [("manga",ResourceKind::Manga),("chapter",ResourceKind::Chapter)]{
        let ids=request.resources.iter().filter(|r|r.kind==name).map(|r|r.id.clone()).collect::<Vec<_>>();
        for (id,resolved) in PostgresPublicIdResolver::resolve_many_in(&*tx,kind,&ids).await? {if resolved.target==PublicIdTarget::NativeResource{aliases.insert((name.to_owned(),id),resolved);}}
    }
    let manga=aliases.iter().filter(|((kind,_),_)|kind=="manga").map(|(_,r)|r.native_id.clone()).collect::<Vec<_>>();
    let chapter=aliases.iter().filter(|((kind,_),_)|kind=="chapter").map(|(_,r)|r.native_id.clone()).collect::<Vec<_>>();
    let rows=tx.query("SELECT 'manga'::text,m.id,m.protocol_series_id,NULL::text,NULL::text FROM platform.catalog_manga m WHERE m.id=ANY($1) AND m.state='published' AND m.deleted_at IS NULL AND m.protocol_source_visible AND m.protocol_series_id IS NOT NULL AND platform.protocol_source_is_visible(m.protocol_series_id) UNION ALL SELECT 'chapter',c.id,c.protocol_event_id,a.public_id::text,m.protocol_series_id FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id JOIN platform.public_resource_ids a ON a.kind='manga' AND a.native_id=m.id AND a.preferred WHERE c.id=ANY($2) AND c.deleted_at IS NULL AND NOT c.is_unavailable AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND c.protocol_event_id IS NOT NULL AND m.protocol_series_id IS NOT NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND platform.protocol_source_is_visible(m.protocol_series_id)",&[&manga,&chapter]).await?;
    let mut local=BTreeMap::new();let mut sources=BTreeSet::new();
    for row in rows {let event:String=row.get(2);sources.insert(hash(&event)?);let parent:Option<String>=row.get(4);if let Some(parent)=&parent{sources.insert(hash(parent)?);}local.insert((row.get::<_,String>(0),row.get::<_,String>(1)),(event,row.get::<_,Option<String>>(3),parent));}
    let mut verified=BTreeMap::new();let sources=sources.into_iter().collect::<Vec<_>>();
    for batch in sources.chunks(256){for source in manga_node::platform_bridge::visible_resource_ids(&*tx,batch).await.map_err(|_|PublicApiError::unavailable())?{verified.insert(source.event.id.clone(),source);}}
    let mut data=Vec::new();let mut unresolved=Vec::new();
    for reference in request.resources {
        let resolved=aliases.get(&(reference.kind.clone(),reference.id.clone()));
        let eligible=resolved.and_then(|alias|local.get(&(reference.kind.clone(),alias.native_id.clone())).map(|row|(alias,row))).and_then(|(alias,row)|verified.get(&row.0).map(|source|(alias,row,source)));
        let Some((alias,(event,parent_id,parent_event),source))=eligible else {unresolved.push(json!({"kind":reference.kind,"id":reference.id,"reason":"no-visible-original-source"}));continue;};
        if source.event.kind!=if reference.kind=="manga"{1002}else{1001}{return Err(PublicApiError::unavailable());}
        if reference.kind=="chapter" {
            let Some(parent)=parent_event.as_ref().and_then(|id|verified.get(id)) else {unresolved.push(json!({"kind":reference.kind,"id":reference.id,"reason":"no-visible-original-parent"}));continue;};
            if parent.event.kind!=1002 || source.series.as_ref()!=parent_event.as_ref(){return Err(PublicApiError::unavailable());}
        }
        data.push(json!({"kind":reference.kind,"id":reference.id,"canonical_id":alias.preferred_id,"event_id":event,"publisher":source.event.pubkey,"source_kind":source.event.kind,"compatibility_id":source.compatibility_id,
            "parent":parent_id.as_ref().map(|id|json!({"id":id,"event_id":parent_event}))}));
    }
    tx.commit().await?;
    let publication=state.config.protocol_publication.as_ref();let origin=publication.and_then(|p|p.native_node.as_ref()).and_then(|n|n.get("base_url")).and_then(Value::as_str);
    let service=publication.map(|p|p.storage_holder.as_str());
    Ok(Json(json!({"schemaVersion":2,"idFormat":"public_uuid","origin":origin,"service_key":service,"catalog_namespace":namespace,"data":data,"unresolved":unresolved})))
}
#[cfg(test)] mod tests {use super::*;#[test]fn mapping_contract_is_explicit_bounded_and_type_scoped(){let id="11111111-1111-8111-8111-111111111111";let good=json!({"schemaVersion":2,"catalogNamespace":id,"resources":[{"kind":"manga","id":id},{"kind":"chapter","id":id}]});assert!(validate(&serde_json::from_value(good.clone()).unwrap()).is_ok());for change in [json!({"schemaVersion":1,"catalogNamespace":id,"resources":[]}),json!({"schemaVersion":2,"catalogNamespace":id,"resources":[{"kind":"user","id":id}]}),json!({"schemaVersion":2,"catalogNamespace":id,"resources":[{"kind":"manga","id":id},{"kind":"manga","id":id}]})]{assert!(validate(&serde_json::from_value(change).unwrap()).is_err());}for value in [json!({"schemaVersion":2,"catalogNamespace":id,"resources":[{"kind":"manga","id":"NativeID01"}]}),json!({"schemaVersion":2,"catalogNamespace":id,"resources":[],"nativeFallback":true})]{assert!(serde_json::from_value::<MappingRequest>(value).is_err());}}}
