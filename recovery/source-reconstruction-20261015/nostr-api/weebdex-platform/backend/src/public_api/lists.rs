//! RECONSTRUCTED UNVERIFIED after reset. Public signed graph only; GET never writes aliases.
use super::{PostgresPublicIdResolver,PublicApiError,PublicApiResult,PublicIdResolver,PublicIdTarget,PublicUuid,ResourceKind,auth,query::{PublicQuery,QuerySchema}};
use crate::{error::ApiError,state::AppState};
use axum::{Json,Router,body::to_bytes,extract::{OriginalUri,Path,RawQuery,Request,State},http::{HeaderMap,StatusCode},routing::get};
use manga_node::{db::Id,http_auth::{AdmittedHttpCapability,HttpAuthError},library_lists::PublicListSnapshot,library_view::{SnapshotChanged,ViewBudget}};
use serde_json::{Value,json};
use std::{collections::{BTreeMap,BTreeSet},time::Duration};
pub fn router()->Router<AppState>{Router::new().route("/user/list",get(mine)).route("/user/{id}/list",get(owner)).route("/list/{id}",get(one)).route("/list/{id}/feed",get(feed))}
fn unavailable()->PublicApiError{ApiError::unavailable().into()}
fn error(error:anyhow::Error)->PublicApiError{
 if error.is::<SnapshotChanged>(){return PublicApiError{status:StatusCode::CONFLICT,title:"Library snapshot changed",detail:"The current signed public library changed; refresh its head".into()};}
 if error.is::<ViewBudget>(){return PublicApiError::bad_request("The original public-list read budget was exceeded");}
 if error.is::<HttpAuthError>(){return auth::map_error(error.downcast::<HttpAuthError>().unwrap());}unavailable()
}
fn id(value:&str)->PublicApiResult<Id>{if !manga_node::auth::valid_hash(value){return Err(unavailable());}hex::decode(value).ok().and_then(|b|b.try_into().ok()).ok_or_else(unavailable)}
fn list_key(owner:Id,list:[u8;16])->String{format!("protocol-list:{}:{}",hex::encode(owner),hex::encode(list))}
fn query(raw:Option<&str>,paged:bool)->PublicApiResult<PublicQuery>{
 for(key,_)in url::form_urlencoded::parse(raw.unwrap_or("").as_bytes()){if key!="head"&&!(paged&&matches!(key.as_ref(),"limit"|"offset")){return Err(PublicApiError::bad_request("Unsupported public-list query parameter"));}}
 let q=PublicQuery::parse(raw,&QuerySchema{default_limit:10,max_limit:100,max_offset:u32::MAX,scalar:&["head"],arrays:&[],includes:&[],order:&[]})?;
 if q.first("head").is_some_and(|v|!manga_node::auth::valid_hash(v)){return Err(PublicApiError::bad_request("Invalid original library head"));}Ok(q)
}
async fn mine(State(state):State<AppState>,OriginalUri(uri):OriginalUri,RawQuery(raw):RawQuery,headers:HeaderMap,request:Request)->PublicApiResult<Json<Value>>{
 let q=query(raw.as_deref(),true)?;let method=request.method().to_string();
 let body=tokio::time::timeout(Duration::from_secs(5),to_bytes(request.into_body(),10240)).await.map_err(|_|PublicApiError::bad_request("Request body timeout"))?.map_err(|_|PublicApiError::bad_request("Request body budget"))?;
 if !body.is_empty(){return Err(PublicApiError::bad_request("GET public-list requests do not declare a body"));}
 let capability=auth::admitted_request(&state,&uri,&headers,&method,&body).await?;
 listing(&state,capability.principal(),q,Some(&capability)).await
}
async fn owner(State(state):State<AppState>,Path(value):Path<String>,RawQuery(raw):RawQuery)->PublicApiResult<Json<Value>>{
 let q=query(raw.as_deref(),true)?;let resolver=PostgresPublicIdResolver::new(state.db.clone());
 let target=resolver.resolve(ResourceKind::User,&PublicUuid::parse(&value)?).await?.ok_or_else(PublicApiError::not_found)?;
 let PublicIdTarget::ProtocolPublisher{pubkey}=target.target else{return Err(PublicApiError::not_found());};
 listing(&state,id(&pubkey)?,q,None).await
}
async fn snapshot(state:&AppState,publisher:Id,head:Option<&str>,offset:u64,limit:usize,target:Option<[u8;16]>)->PublicApiResult<PublicListSnapshot>{
 let future=state.protocol.public_lists(publisher,head,offset,limit,target);tokio::pin!(future);
 match tokio::time::timeout(Duration::from_secs(15),&mut future).await{Ok(r)=>r.map_err(error),Err(_)=>{let _=future.await;Err(unavailable())}}
}
async fn projected(state:&AppState,snapshot:&PublicListSnapshot)->PublicApiResult<()>{
 if let Some(head)=&snapshot.head{let owner=hex::encode(snapshot.publisher);let ready:bool=state.db.client().await?.query_one("SELECT EXISTS(SELECT 1 FROM platform.protocol_public_list_alias_projection WHERE publisher=$1 AND head=$2 AND completed)",&[&owner,head]).await.map_err(ApiError::from)?.get(0);if !ready{return Err(PublicApiError{status:StatusCode::SERVICE_UNAVAILABLE,title:"Public-list alias projection pending",detail:"The current signed library has not completed bounded alias projection; retry with a fresh request after projection".into()});}}
 Ok(())
}
fn metadata(snapshot:&PublicListSnapshot)->Value{json!({"pubkey":hex::encode(snapshot.publisher),"head":snapshot.head,"root":snapshot.root.map(hex::encode),"scope":"published_library_locally_projected_manga"})}
async fn listing(state:&AppState,publisher:Id,q:PublicQuery,capability:Option<&AdmittedHttpCapability>)->PublicApiResult<Json<Value>>{
 let snapshot=snapshot(state,publisher,q.first("head"),q.offset as u64,q.limit as usize,None).await?;
 projected(state,&snapshot).await?;let data=entities(state,&snapshot).await?;
 state.protocol.verify_public_list_snapshot(&snapshot,capability).await.map_err(error)?;
 Ok(Json(json!({"result":"ok","response":"collection","data":data,"limit":q.limit,"offset":q.offset,"total":snapshot.total,"nostr":metadata(&snapshot)})))
}
async fn target(state:&AppState,value:&str)->PublicApiResult<(Id,[u8;16])>{
 let resolver=PostgresPublicIdResolver::new(state.db.clone());let target=resolver.resolve(ResourceKind::List,&PublicUuid::parse(value)?).await?.ok_or_else(PublicApiError::not_found)?;
 let PublicIdTarget::ProtocolList{owner,list_id}=target.target else{return Err(PublicApiError::not_found());};
 let list=hex::decode(list_id).ok().and_then(|v|v.try_into().ok()).ok_or_else(unavailable)?;Ok((id(&owner)?,list))
}
async fn one(State(state):State<AppState>,Path(value):Path<String>,RawQuery(raw):RawQuery)->PublicApiResult<Json<Value>>{
 let q=query(raw.as_deref(),false)?;let(owner,list)=target(&state,&value).await?;
 let snapshot=snapshot(&state,owner,q.first("head"),0,1,Some(list)).await?;
 if snapshot.lists.is_empty(){return Err(PublicApiError::not_found());}projected(&state,&snapshot).await?;
 let data=entities(&state,&snapshot).await?.into_iter().next().ok_or_else(unavailable)?;
 state.protocol.verify_public_list_snapshot(&snapshot,None).await.map_err(error)?;
 Ok(Json(json!({"result":"ok","response":"entity","data":data,"nostr":metadata(&snapshot)})))
}
/// Bounded native/canonical scope for actual original-visible member identities.
async fn manga(state:&AppState,snapshot:&PublicListSnapshot)->PublicApiResult<BTreeMap<String,(String,PublicUuid)>>{
 let sources=snapshot.lists.iter().flat_map(|l|l.members.iter().map(|m|hex::encode(m.series))).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
 if sources.len()>5000{return Err(PublicApiError::bad_request("Public-list member mapping budget exceeded"));}
 if sources.is_empty(){return Ok(BTreeMap::new());}
 let rows=state.db.client().await?.query("SELECT m.id,m.protocol_series_id FROM platform.catalog_manga m WHERE m.protocol_series_id=ANY($1::text[]) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)",&[&sources]).await.map_err(ApiError::from)?;
 let natives=rows.iter().map(|r|r.get::<_,String>(0)).collect::<Vec<_>>();let resolver=PostgresPublicIdResolver::new(state.db.clone());let mut aliases=BTreeMap::new();
 for batch in natives.chunks(super::postgres_identity::MAX_PUBLIC_ID_BATCH){aliases.extend(resolver.preferred_many(ResourceKind::Manga,batch).await?);}
 let mut out=BTreeMap::new();for row in rows{let native:String=row.get(0);let source:String=row.get(1);let alias=aliases.get(&native).cloned().ok_or_else(unavailable)?;out.insert(source,(native,alias));}Ok(out)
}
async fn entities(state:&AppState,snapshot:&PublicListSnapshot)->PublicApiResult<Vec<Value>>{
 if snapshot.lists.is_empty(){return Ok(vec![]);}
 let resolver=PostgresPublicIdResolver::new(state.db.clone());let owner=hex::encode(snapshot.publisher);let owner_key=format!("protocol:{owner}");
 let owners=resolver.describe_many(ResourceKind::User,&[owner_key.clone()]).await?;let owner_alias=owners.get(&owner_key).ok_or_else(unavailable)?;
 if owner_alias.target!=(PublicIdTarget::ProtocolPublisher{pubkey:owner.clone()}){return Err(unavailable());}
 let keys=snapshot.lists.iter().map(|l|list_key(snapshot.publisher,l.id)).collect::<Vec<_>>();let aliases=resolver.describe_many(ResourceKind::List,&keys).await?;let manga=manga(state,snapshot).await?;let mut data=vec![];
 for list in &snapshot.lists{let key=list_key(snapshot.publisher,list.id);let alias=aliases.get(&key).ok_or_else(unavailable)?;
  if alias.target!=(PublicIdTarget::ProtocolList{owner:owner.clone(),list_id:hex::encode(list.id)}){return Err(unavailable());}
  let mut relationships=vec![json!({"id":owner_alias.preferred_id,"type":"user"})];
  for member in &list.members{let event=hex::encode(member.series);if let Some((_,id))=manga.get(&event){relationships.push(json!({"id":id,"type":"manga","nostr":{"eventId":event,"position":member.position}}));}}
  data.push(json!({"id":alias.preferred_id,"type":"custom_list","attributes":{"name":list.name,"visibility":"public"},"relationships":relationships,"nostr":{"listId":hex::encode(list.id),"pubkey":owner,"head":snapshot.head,"root":snapshot.root.map(hex::encode),"updatedAt":list.updated_at,"scope":"published_library_locally_projected_manga"}}));
 }Ok(data)
}
fn feed_query(raw:Option<String>)->PublicApiResult<(Option<String>,Option<String>)>{
 let Some(raw)=raw else{return Ok((None,None));};let mut head=None;let mut remaining=vec![];
 for field in raw.split('&'){let mut parsed=url::form_urlencoded::parse(field.as_bytes());if let Some((key,value))=parsed.next(){if key=="head"{if head.is_some()||!manga_node::auth::valid_hash(&value){return Err(PublicApiError::bad_request("Invalid or duplicate original library head"));}head=Some(value.into_owned());continue;}}remaining.push(field);}
 Ok((head,(!remaining.is_empty()).then(||remaining.join("&"))))
}
async fn feed(State(state):State<AppState>,Path(value):Path<String>,RawQuery(raw):RawQuery)->PublicApiResult<Json<Value>>{
 let(head,query)=feed_query(raw)?;let(owner,list)=target(&state,&value).await?;
 let snapshot=snapshot(&state,owner,head.as_deref(),0,1,Some(list)).await?;
 if snapshot.lists.is_empty(){return Err(PublicApiError::not_found());}projected(&state,&snapshot).await?;
 let scope=manga(&state,&snapshot).await?.into_values().map(|(native,_)|native).collect::<BTreeSet<_>>().into_iter().collect();
 let Json(response)=super::catalogue::feed_for_manga_ids(state.clone(),query,scope).await?;
 state.protocol.verify_public_list_snapshot(&snapshot,None).await.map_err(error)?;
 let mut response=serde_json::to_value(response).map_err(|_|unavailable())?;
 response.as_object_mut().ok_or_else(unavailable)?.insert("nostr".into(),metadata(&snapshot));Ok(Json(response))
}
#[cfg(test)]mod tests{use super::*;#[test]fn queries_reject_private_native_and_unimplemented_extensions(){assert!(query(Some("limit=0&offset=100"),true).is_ok());for q in["privacy=private","includes[]=user","owner=other","head=bad"]{assert!(query(Some(q),true).is_err());}assert!(query(Some("limit=1"),false).is_err());let(head,rest)=feed_query(Some(format!("head={}&translatedLanguage[]=en&limit=%ZZ","a".repeat(64)))).unwrap();assert!(head.is_some());assert_eq!(rest.as_deref(),Some("translatedLanguage[]=en&limit=%ZZ"));}}
