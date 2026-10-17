//! Exact signed, account-bound instance cover operations. Originals are stored
//! byte-for-byte; no derivative encoder or original event rewrite is invoked.
use super::{PublicApiError, PublicApiResult, PublicUuid, ResourceKind, auth::{bound_account_read,bound_account_write,BoundAccountRead},
    catalogue_dto::AliasIndex,dto::{CollectionResponse,EntityResponse,Resource}, uploads::{authorize,no_query,resolve}};
use crate::{AppState,domains::{catalog::{CatalogRepository,CoverPublication,entity_mutations::CatalogMutationActor,repository,validation as v},uploads::{media,sessions,storage}}};
use axum::{Json,Router,extract::{DefaultBodyLimit,Multipart,OriginalUri,Path,Request,State,multipart::Field},http::{HeaderMap,StatusCode},middleware,response::Response,routing::{get,post}};
use serde_json::{Value,json};
use std::{collections::BTreeSet,time::Duration};
use tokio_postgres::Transaction;

pub fn router()->Router<AppState>{Router::new()
    .route("/cover/{p0}",post(upload).put(update).delete(remove))
    .route("/nostr/manga/{p0}/covers",get(list))
    .route("/nostr/covers/{p0}/original",get(original))
    .layer(DefaultBodyLimit::max(super::uploads_body::BODY_LIMIT))
    .layer(middleware::map_response(super::upload_extensions::private_response))}
async fn next(multipart:&mut Multipart)->PublicApiResult<Option<Field<'_>>>{tokio::time::timeout(Duration::from_secs(30),multipart.next_field()).await.map_err(|_|PublicApiError::bad_request("Cover multipart timed out"))?.map_err(|_|PublicApiError::bad_request("Invalid cover multipart"))}
async fn text(mut field:Field<'_>)->PublicApiResult<String>{
    if field.file_name().is_some(){return Err(PublicApiError::bad_request("Cover metadata must be plain fields"));}
    let mut bytes=Vec::new();while let Some(chunk)=tokio::time::timeout(Duration::from_secs(30),field.chunk()).await.map_err(|_|PublicApiError::bad_request("Cover metadata timed out"))?.map_err(|_|PublicApiError::bad_request("Invalid cover metadata"))?{
        if bytes.len()+chunk.len()>50_000{return Err(PublicApiError::bad_request("Cover metadata exceeds budget"));}bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes).map_err(|_|PublicApiError::bad_request("Cover metadata must be UTF-8"))
}
fn metadata(fields:Value,version:Option<i32>)->PublicApiResult<Value>{
    v::allowed(&fields,&["locale","volume","description","main"])?;
    let mut output=json!({});
    if let Some(version)=version{output["version"]=json!(version);}
    for key in ["volume","description"]{if let Some(value)=fields.get(key){if value.is_null()&&key=="volume"{output[key]=json!("");}else{output[key]=value.clone();}if let Some(text)=v::string(&output,key,if key=="volume"{32}else{50_000},false)?{output[key]=json!(text);}}}
    if let Some(language)=fields.get("locale"){let language=language.as_str().ok_or_else(||PublicApiError::bad_request("Invalid cover locale"))?;v::language(language)?;output["language"]=json!(language);}
    if let Some(main)=fields.get("main"){if !main.is_boolean(){return Err(PublicApiError::bad_request("Invalid main cover flag"));}output["main"]=main.clone();}
    Ok(output)
}
async fn account_intent(tx:&Transaction<'_>,actor:&impl CatalogMutationActor,expected:&PublicUuid)->PublicApiResult<()> {
    if resolve(tx,ResourceKind::User,expected).await?!=actor.user_id(){return Err(PublicApiError::conflict("Native account binding changed; reopen this cover editor"));}Ok(())
}
fn expected_account(input:&Value)->PublicApiResult<PublicUuid>{Ok(PublicUuid::parse(input.get("expectedAccountId").and_then(Value::as_str).ok_or_else(||PublicApiError::bad_request("Signed expectedAccountId is required"))?)?)}
async fn parent_version(tx:&Transaction<'_>,id:&str,actor:&impl CatalogMutationActor,expected:i32)->PublicApiResult<()> {
    repository::require_manga_edit(tx,id,actor,"manga.cover.edit").await?;
    let actual:i32=tx.query_one("SELECT version FROM platform.catalog_manga WHERE id=$1",&[&id]).await?.get(0);
    if actual!=expected{return Err(PublicApiError::conflict("Title changed; reload before uploading a cover"));}Ok(())
}
async fn value(tx:&Transaction<'_>,id:&str)->PublicApiResult<Value>{
    tx.query_opt("SELECT platform.catalog_cover_json(c.id)||jsonb_build_object('_manga_id',c.manga_id,'_main',c.is_main,'_source',c.protocol_event_id) FROM platform.catalog_covers c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=$1 AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.protocol_source_visible AND platform.protocol_source_is_visible(c.protocol_event_id) AND platform.protocol_source_is_visible(m.protocol_series_id)",&[&id]).await?.map(|r|r.get(0)).ok_or_else(PublicApiError::not_found)
}
async fn resource(tx:&Transaction<'_>,native:Value)->PublicApiResult<Resource>{
    let aliases=AliasIndex::load_in(tx,&[(ResourceKind::Cover,native.clone())]).await?;
    let mut resource=aliases.resource(ResourceKind::Cover,&native,&BTreeSet::from(["user".into()]))?;
    let id=native["id"].as_str().ok_or_else(PublicApiError::unavailable)?;
    let object=tx.query_opt("SELECT o.digest,o.mime,o.bytes,o.width,o.height FROM platform.cover_media c JOIN platform.media_objects o ON o.id=c.object_id WHERE c.cover_id=$1 AND c.variant='original'",&[&id]).await?;
    let original=object.map(|r|json!({"sha256":r.get::<_,String>(0),"mime":r.get::<_,String>(1),"bytes":r.get::<_,i64>(2),"dimensions":[r.get::<_,i32>(3),r.get::<_,i32>(4)]}));
    resource.attributes["nostr"]=json!({"metadataScope":"instance-local","sourceEventId":native["_source"],"main":native["_main"],"original":original,"originalEventRewritten":false});
    Ok(resource)
}
async fn upload(State(state):State<AppState>,Path(manga):Path<String>,OriginalUri(uri):OriginalUri,headers:HeaderMap,request:Request)->PublicApiResult<(StatusCode,Json<EntityResponse<Resource>>)> {
    let manga=PublicUuid::parse(&manga)?;no_query(&uri)?;
    let (cap,mut multipart,_permit)=super::uploads_body::receive(&state,&uri,&headers,request).await?;
    let expected=super::upload_extensions::expected_version(next(&mut multipart).await?.ok_or_else(||PublicApiError::bad_request("First field must be signed expectedVersion"))?).await?;
    let account_field=next(&mut multipart).await?.ok_or_else(||PublicApiError::bad_request("Second field must be signed expectedAccountId"))?;
    if account_field.name()!=Some("expectedAccountId"){return Err(PublicApiError::bad_request("Second field must be signed expectedAccountId"));}
    let expected_account=PublicUuid::parse(&text(account_field).await?)?;
    let initial_actor={
        let mut client=state.db.client().await?;let tx=client.transaction().await?;
        let actor=bound_account_write(&tx,&cap).await?;account_intent(&tx,&actor,&expected_account).await?;let native=resolve(&tx,ResourceKind::Manga,&manga).await?;
        parent_version(&tx,&native,&actor,expected).await?;let actor_id=actor.user_id().to_owned();tx.commit().await?;actor_id
    };
    let mut fields=serde_json::Map::new();let mut image=None;let mut count=2;
    while let Some(field)=next(&mut multipart).await?{
        count+=1;if count>8{return Err(PublicApiError::bad_request("Too many cover fields"));}
        let name=field.name().unwrap_or("").to_owned();
        if name=="file"{if image.is_some(){return Err(PublicApiError::bad_request("Exactly one original cover file is allowed"));}image=Some(storage::receive_cover_original(&state.config.uploads_dir,field).await?);}
        else{if !["locale","volume","description","main"].contains(&name.as_str())||fields.contains_key(&name){return Err(PublicApiError::bad_request("Unknown or repeated cover field"));}let text=text(field).await?;fields.insert(name.clone(),if name=="main"{match text.as_str(){"true"=>json!(true),"false"=>json!(false),_=>return Err(PublicApiError::bad_request("Invalid main cover flag"))}}else{json!(text)});}
    }
    let fields=metadata(Value::Object(fields),None)?;let mut image=image.ok_or_else(||PublicApiError::bad_request("Original cover file is required"))?;
    let mut client=state.db.client().await?;let tx=client.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;account_intent(&tx,&actor,&expected_account).await?;if actor.user_id()!=initial_actor{return Err(PublicApiError::conflict("Cover account changed during image intake"));}let manga=resolve(&tx,ResourceKind::Manga,&manga).await?;parent_version(&tx,&manga,&actor,expected).await?;
    let id=storage::short_id();let cover=CoverPublication{id:id.clone(),manga_id:manga,uploader_id:actor.user_id().into(),uploader_name:actor.username().into(),ext:image.ext.into(),dimensions:[image.width,image.height],volume:fields["volume"].as_str().unwrap_or("").into(),language:fields["language"].as_str().unwrap_or("en").into(),description:fields["description"].as_str().map(str::to_owned),main:fields["main"].as_bool().unwrap_or(false)};
    CatalogRepository::publish_cover(&tx,&cover).await?;
    let object=sessions::insert_object(&tx,&image).await?;tx.execute("INSERT INTO platform.cover_media(cover_id,variant,object_id) VALUES($1,'original',$2)",&[&id,&object]).await?;
    let result=resource(&tx,value(&tx,&id).await?).await?;
    image.protect_commit(&state.config.uploads_dir).await?;tx.commit().await?;image.confirm_persisted().await;
    Ok((StatusCode::CREATED,Json(EntityResponse::new(result))))
}
async fn update(State(state):State<AppState>,Path(cover):Path<String>,OriginalUri(uri):OriginalUri,headers:HeaderMap,request:Request)->PublicApiResult<Json<EntityResponse<Resource>>>{
    let cover=PublicUuid::parse(&cover)?;let (cap,body)=authorize(&state,&uri,&headers,request,64*1024,false).await?;
    let mut input:Value=serde_json::from_slice(&body).map_err(|_|PublicApiError::bad_request("Invalid cover update"))?;v::allowed(&input,&["locale","volume","description","main","version","expectedAccountId"])?;let expected_account=expected_account(&input)?;let version=v::version(&input)?;input.as_object_mut().unwrap().remove("version");input.as_object_mut().unwrap().remove("expectedAccountId");let patch=metadata(input,Some(version))?;
    let mut client=state.db.client().await?;let tx=client.transaction().await?;let actor=bound_account_write(&tx,&cap).await?;account_intent(&tx,&actor,&expected_account).await?;let cover=resolve(&tx,ResourceKind::Cover,&cover).await?;
    // Parent/source visibility must be checked even when editing a saved alias.
    value(&tx,&cover).await?;CatalogRepository::update_cover(&tx,&cover,&actor,&patch).await?;let result=resource(&tx,value(&tx,&cover).await?).await?;tx.commit().await?;Ok(Json(EntityResponse::new(result)))
}
async fn remove(State(state):State<AppState>,Path(cover):Path<String>,OriginalUri(uri):OriginalUri,headers:HeaderMap,request:Request)->PublicApiResult<StatusCode>{
    let cover=PublicUuid::parse(&cover)?;let (cap,body)=authorize(&state,&uri,&headers,request,1024,false).await?;let input:Value=serde_json::from_slice(&body).map_err(|_|PublicApiError::bad_request("Expected signed cover version"))?;v::allowed(&input,&["version","expectedAccountId"])?;let expected_account=expected_account(&input)?;let version=v::version(&input)?;
    let mut client=state.db.client().await?;let tx=client.transaction().await?;let actor=bound_account_write(&tx,&cap).await?;account_intent(&tx,&actor,&expected_account).await?;
    if !actor.is_moderator(){return Err(PublicApiError::forbidden("Instance staff cover deletion approval required"));}actor.require_permission("cover.delete")?;
    let cover=resolve(&tx,ResourceKind::Cover,&cover).await?;let native=value(&tx,&cover).await?;let parent=native["_manga_id"].as_str().ok_or_else(PublicApiError::unavailable)?;
    tx.query_one("SELECT id FROM platform.catalog_manga WHERE id=$1 FOR UPDATE",&[&parent]).await?;
    value(&tx,&cover).await?;
    let actual:i32=tx.query_opt("SELECT version FROM platform.catalog_covers WHERE id=$1 AND deleted_at IS NULL FOR UPDATE",&[&cover]).await?.ok_or_else(PublicApiError::not_found)?.get(0);if actual!=version{return Err(PublicApiError::conflict("Cover changed; reload before removal"));}
    CatalogRepository::delete_cover(&tx,&cover,&actor).await?;tx.execute("DELETE FROM platform.cover_media WHERE cover_id=$1",&[&cover]).await?;sessions::queue_unused(&tx).await?;tx.commit().await?;sessions::cleanup(&state).await;Ok(StatusCode::NO_CONTENT)
}
async fn read_parent(tx:&Transaction<'_>,actor:&BoundAccountRead,manga:&str)->PublicApiResult<()> {
    let row=tx.query_opt("SELECT state,owner_id FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id)",&[&manga]).await?.ok_or_else(PublicApiError::not_found)?;
    if row.get::<_,String>(0)!="published" && !(row.get::<_,String>(1)==actor.user_id()&&actor.has_perm("manga.create")||actor.is_moderator()&&actor.has_perm("manga.cover.edit")){return Err(PublicApiError::not_found());}Ok(())
}
async fn list(State(state):State<AppState>,Path(manga):Path<String>,OriginalUri(uri):OriginalUri,headers:HeaderMap,request:Request)->PublicApiResult<Json<CollectionResponse<Resource>>>{
    let manga=PublicUuid::parse(&manga)?;let (cap,_)=authorize(&state,&uri,&headers,request,1,true).await?;let mut client=state.db.client().await?;let tx=client.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).start().await?;let actor=bound_account_read(&tx,&cap).await?;let manga=resolve(&tx,ResourceKind::Manga,&manga).await?;read_parent(&tx,&actor,&manga).await?;
    let rows=tx.query("SELECT id FROM platform.catalog_covers WHERE manga_id=$1 AND deleted_at IS NULL AND platform.protocol_source_is_visible(protocol_event_id) ORDER BY created_at DESC,id LIMIT 100",&[&manga]).await?;
    let total:i64=tx.query_one("SELECT count(*) FROM platform.catalog_covers WHERE manga_id=$1 AND deleted_at IS NULL AND platform.protocol_source_is_visible(protocol_event_id)",&[&manga]).await?.get(0);
    let mut data=Vec::new();for row in rows{data.push(resource(&tx,value(&tx,row.get(0)).await?).await?);}
    let response=CollectionResponse::new(data,100,0,total as u64);tx.commit().await?;Ok(Json(response))
}
async fn original(State(state):State<AppState>,Path(cover):Path<String>,OriginalUri(uri):OriginalUri,headers:HeaderMap,request:Request)->PublicApiResult<Response>{
    let cover=PublicUuid::parse(&cover)?;let (cap,_)=authorize(&state,&uri,&headers,request,1,true).await?;let mut client=state.db.client().await?;let tx=client.transaction().await?;let actor=bound_account_read(&tx,&cap).await?;let cover=resolve(&tx,ResourceKind::Cover,&cover).await?;let native=value(&tx,&cover).await?;let parent=native["_manga_id"].as_str().ok_or_else(PublicApiError::unavailable)?;read_parent(&tx,&actor,parent).await?;
    let row=tx.query_opt("SELECT o.storage_key,o.digest,o.mime,o.bytes FROM platform.cover_media c JOIN platform.media_objects o ON o.id=c.object_id WHERE c.cover_id=$1 AND c.variant='original'",&[&cover]).await?.ok_or_else(PublicApiError::not_found)?;
    let response=media::signed_upload_preview(&state,&row,&headers).await?;tx.commit().await?;Ok(response)
}
#[cfg(test)]mod tests{use super::*;#[test]fn cover_metadata_is_explicit_and_file_identity_never_changes(){assert_eq!(metadata(json!({"locale":"ja","volume":null,"main":true}),Some(3)).unwrap(),json!({"language":"ja","volume":"","main":true,"version":3}));for input in[json!({"fileName":"injected.png"}),json!({"locale":"?"}),json!({"main":"true"}),json!({"dimensions":[1,2]}),json!({"description":"bad\0"})]{assert!(metadata(input,Some(1)).is_err());}}}
