//! Signed title drafts and instance-local metadata edits over the native core.
//! Submission is review-only. Original publication still requires the exact
//! configured operator binding/admission; a UUID never supplies signing authority.
use super::{PublicApiError, PublicApiResult, PublicUuid, ResourceKind,
    editorial::{expected_account,take_expected_account}, auth::bound_account_write, catalogue_dto::AliasIndex, dto::{EntityResponse, Resource},
    uploads::{authorize, resolve}};
use crate::{AppState, domains::catalog::{mutations as core, validation as v}};
use axum::{Json, Router, extract::{OriginalUri, Path, Request, State},
    http::{HeaderMap, StatusCode}, routing::{post, put}};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use tokio_postgres::Transaction;

pub fn router() -> Router<AppState> {
    Router::new().route("/manga", post(create)).route("/manga/{p0}", put(update))
        .route("/manga/draft/{p0}/commit", post(commit))
        // Explicit retained-platform review extensions, not MangaDex operations.
        .route("/manga/draft/{p0}/publish", post(publish))
        .route("/manga/draft/{p0}/reject", post(reject))
}
fn bad(field: &str) -> PublicApiError { PublicApiError::bad_request(format!("Invalid {field}")) }
fn localized(value: &Value, field: &str, title: bool) -> PublicApiResult<Value> {
    let entries=value.as_object().filter(|m|m.len()<=64 && (!title || !m.is_empty())).ok_or_else(||bad(field))?;
    let mut size=0usize;
    for (language,text) in entries {
        v::language(language)?;
        let text=text.as_str().ok_or_else(||bad(field))?;
        size+=text.chars().count();
        if text.contains('\0') || title && (text.trim().is_empty() || text.chars().count()>500 || text.len()>1024) || size>50_000 {
            return Err(bad(field));
        }
    }
    Ok(value.clone())
}
fn primary(value: &Value, language: &str) -> String {
    value.get(language).or_else(||value.get("en")).or_else(||value.as_object().and_then(|m|m.values().next()))
        .and_then(Value::as_str).unwrap_or("").to_owned()
}
fn normalize(input: Value, edit: bool, language: &str) -> PublicApiResult<(Value,Value)> {
    v::allowed(&input,&["title","altTitles","description","authors","artists","links","originalLanguage",
        "lastVolume","lastChapter","publicationDemographic","status","year","contentRating",
        "chapterNumbersResetOnNewVolume","tags","version"])?;
    if edit { v::version(&input)?; }
    else if input.get("version").is_some() { return Err(PublicApiError::bad_request("Version is assigned by the server")); }
    let mut native=json!({}); let mut metadata=json!({});
    let language=if let Some(language)=input.get("originalLanguage") {
        let language=language.as_str().ok_or_else(||bad("originalLanguage"))?;v::language(language)?;language
    } else if edit {language} else {return Err(bad("originalLanguage"))};
    if !edit || input.get("originalLanguage").is_some() {native["language"]=json!(language);}
    if let Some(title)=input.get("title") {
        let title=localized(title,"title",true)?;
        native["title"]=json!(primary(&title,language));metadata["localized_titles"]=title;
    } else if !edit {return Err(bad("title"));}
    if let Some(description)=input.get("description") {
        let description=localized(description,"description",false)?;
        native["description"]=json!(primary(&description,language));metadata["localized_description"]=description;
    }
    if let Some(alts)=input.get("altTitles") {
        let alts=alts.as_array().filter(|a|a.len()<=100).ok_or_else(||bad("altTitles"))?;
        let mut flat=json!({});
        for alt in alts {
            let alt=localized(alt,"altTitles",true)?;
            for (lang,text) in alt.as_object().unwrap() {
                if flat.get(lang).is_none() {flat[lang]=json!([]);}
                flat[lang].as_array_mut().unwrap().push(text.clone());
            }
        }
        native["alt_titles"]=flat;metadata["localized_alt_titles"]=json!(alts);
    }
    for (wire,key) in [("lastVolume","last_volume"),("lastChapter","last_chapter")] {
        if let Some(value)=input.get(wire) {
            if value.is_null() {metadata[key]=Value::Null;}
            else {native[key]=value.clone();v::string(&native,key,32,false)?;}
        }
    }
    if let Some(value)=input.get("publicationDemographic") {
        if !value.is_null() && !value.as_str().is_some_and(|v|["shounen","shoujo","josei","seinen"].contains(&v)) {return Err(bad("publicationDemographic"));}
        native["demographic"]=if value.is_null(){json!("none")}else{value.clone()};
    }
    if let Some(value)=input.get("year") {
        if !value.is_null() && !value.as_i64().is_some_and(|v|(1..=9999).contains(&v)) {return Err(bad("year"));}
        native["year"]=if value.is_null(){json!(0)}else{value.clone()};
    }
    for (wire,key) in [("status","status"),("contentRating","content_rating"),("links","links"),("version","version")] {
        if let Some(value)=input.get(wire) {native[key]=value.clone();}
    }
    if let Some(flag)=input.get("chapterNumbersResetOnNewVolume") {
        if !flag.is_boolean() {return Err(bad("chapterNumbersResetOnNewVolume"));}
        metadata["chapter_numbers_reset_on_new_volume"]=flag.clone();
    }
    for key in ["tags","authors","artists"] {
        if let Some(values)=input.get(key) {
            let values=values.as_array().filter(|a|a.len()<=100).ok_or_else(||bad(key))?;
            let mut seen=BTreeSet::new();
            for value in values {
                let id=PublicUuid::parse(value.as_str().ok_or_else(||bad(key))?)?;
                if !seen.insert(id) {return Err(bad(key));}
            }
            native[key]=json!(values);
        } else if !edit && key!="tags" {native[key]=json!([]);}
    }
    Ok((native,metadata))
}
async fn ids(tx:&Transaction<'_>, native:&mut Value) -> PublicApiResult<()> {
    for (field,kind) in [("tags",ResourceKind::Tag),("authors",ResourceKind::Author),("artists",ResourceKind::Author)] {
        if let Some(values)=native.get_mut(field) {
            let mut mapped=Vec::new();
            for id in values.as_array().unwrap() {mapped.push(resolve(tx,kind,&PublicUuid::parse(id.as_str().unwrap())?).await?);}
            *values=json!(mapped);
        }
    }
    Ok(())
}
async fn resource(tx:&Transaction<'_>,native:Value)->PublicApiResult<Json<EntityResponse<Resource>>> {
    let aliases=AliasIndex::load_in(tx,&[(ResourceKind::Manga,native.clone())]).await?;
    let includes=["author","artist","tag","creator","cover_art"].into_iter().map(str::to_owned).collect();
    Ok(Json(EntityResponse::new(aliases.resource(ResourceKind::Manga,&native,&includes)?)))
}
async fn create(State(state):State<AppState>,OriginalUri(uri):OriginalUri,headers:HeaderMap,request:Request)
    ->PublicApiResult<Json<EntityResponse<Resource>>> {
    let (cap,body)=authorize(&state,&uri,&headers,request,64*1024,false).await?;
    let mut input=serde_json::from_slice(&body).map_err(|_|PublicApiError::bad_request("Invalid manga request body"))?;
    let expected=take_expected_account(&mut input)?;
    let (mut native,metadata)=normalize(input,false,"")?;
    let mut client=state.db.client().await?;let tx=client.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;expected_account(&tx,&actor,expected.as_ref()).await?;ids(&tx,&mut native).await?;
    let native=core::manga_create_in(&tx,&actor,&native,&metadata).await?;
    let result=resource(&tx,native).await?;tx.commit().await?;Ok(result)
}
async fn update(State(state):State<AppState>,Path(manga):Path<String>,OriginalUri(uri):OriginalUri,headers:HeaderMap,request:Request)
    ->PublicApiResult<Json<EntityResponse<Resource>>> {
    let manga=PublicUuid::parse(&manga)?;
    let (cap,body)=authorize(&state,&uri,&headers,request,64*1024,false).await?;
    let mut input=serde_json::from_slice(&body).map_err(|_|PublicApiError::bad_request("Invalid manga request body"))?;
    let expected=take_expected_account(&mut input)?;
    let mut client=state.db.client().await?;let tx=client.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;expected_account(&tx,&actor,expected.as_ref()).await?;let manga=resolve(&tx,ResourceKind::Manga,&manga).await?;
    let language:String=tx.query_opt("SELECT language FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id)",&[&manga]).await?.ok_or_else(PublicApiError::not_found)?.get(0);
    let (mut native,metadata)=normalize(input,true,&language)?;ids(&tx,&mut native).await?;
    let native=core::manga_update_in(&tx,&actor,&manga,&native,&metadata).await?;
    let result=resource(&tx,native).await?;tx.commit().await?;Ok(result)
}
async fn commit(state:State<AppState>,path:Path<String>,uri:OriginalUri,headers:HeaderMap,request:Request)
    ->PublicApiResult<(StatusCode,Json<EntityResponse<Resource>>)> {
    Ok((StatusCode::CREATED,transition(state,path,uri,headers,request,"submitted").await?))
}
async fn publish(state:State<AppState>,path:Path<String>,uri:OriginalUri,headers:HeaderMap,request:Request)
    ->PublicApiResult<Json<EntityResponse<Resource>>> {transition(state,path,uri,headers,request,"published").await}
async fn reject(state:State<AppState>,path:Path<String>,uri:OriginalUri,headers:HeaderMap,request:Request)
    ->PublicApiResult<Json<EntityResponse<Resource>>> {transition(state,path,uri,headers,request,"rejected").await}
async fn transition(State(state):State<AppState>,Path(manga):Path<String>,OriginalUri(uri):OriginalUri,
    headers:HeaderMap,request:Request,next:&str)->PublicApiResult<Json<EntityResponse<Resource>>> {
    let manga=PublicUuid::parse(&manga)?;
    let (cap,body)=authorize(&state,&uri,&headers,request,1024,false).await?;
    let mut payload:Value=serde_json::from_slice(&body).map_err(|_|PublicApiError::bad_request("Invalid title transition body"))?;
    let expected=take_expected_account(&mut payload)?;
    v::allowed(&payload,&["version"])?;v::version(&payload)?;
    let mut client=state.db.client().await?;let tx=client.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;expected_account(&tx,&actor,expected.as_ref()).await?;let manga=resolve(&tx,ResourceKind::Manga,&manga).await?;
    // A hidden source cannot be reviewed through a saved UUID. Exact pending
    // publication receipts have their own current original-authority predicate.
    let visible:bool=tx.query_opt("SELECT protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id) FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL", &[&manga]).await?.ok_or_else(PublicApiError::not_found)?.get(0);
    if !visible && !(next=="published" && crate::protocol::publication::pending_native_manga_receipt_allowed(&state,&tx,&manga,&actor).await?) {return Err(PublicApiError::not_found());}
    let native=core::manga_transition_in(&state,&tx,&actor,&manga,&payload,next,true).await?;
    let result=resource(&tx,native).await?;tx.commit().await?;Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn localized_maps_and_nullable_fields_are_not_silently_lost() {
        let (n,m)=normalize(json!({"title":{"ja":"題","en":"Title"},"description":{"ja":"説明","en":"Text"},"originalLanguage":"ja","status":"ongoing","contentRating":"safe","year":null,"lastVolume":null,"publicationDemographic":null}),false,"").unwrap();
        assert_eq!(n["title"],"題");assert_eq!(n["year"],0);assert_eq!(n["demographic"],"none");
        assert_eq!(m["localized_titles"]["en"],"Title");assert_eq!(m["localized_description"]["ja"],"説明");assert!(m["last_volume"].is_null());
        for value in [json!({"version":1,"title":{}}),json!({"version":1,"officialLinks":{}}),json!({"version":1,"primaryCover":null}),json!({"version":1,"title":{"en":""}}),json!({"version":1,"year":0}),json!({"version":1,"authors":["native"]}),json!({"version":1,"title":{"ja":"題".repeat(342)}})] {assert!(normalize(value,true,"en").is_err());}
    }
}
