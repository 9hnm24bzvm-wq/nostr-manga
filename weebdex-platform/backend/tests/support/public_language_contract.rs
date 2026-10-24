//! Genuine signed HTTP/PG language qualification. Reuses only the actual owned
//! fixture, never substitutes the production validators or write repository.
use super::*;
use std::collections::BTreeMap;

async fn alias(f:&Fixture, kind:ResourceKind, native:&str)->String {
    PostgresPublicIdResolver::new(f.state.db.clone()).preferred(kind,native).await.unwrap().unwrap().to_string()
}
async fn native_put(f:&Fixture, path:&str, payload:Value, expected:u16)->Value {
    let response=f.http.put(format!("{}{path}",f.origin)).header("origin",&f.origin)
        .header("cookie","weebdex_session=synthetic-public-upload-cookie").json(&payload).send().await.unwrap();
    let status=response.status().as_u16();let text=response.text().await.unwrap();
    assert_eq!(status,expected,"native {path}: {text}");serde_json::from_str(&text).unwrap()
}
async fn attach(f:&Fixture, session:&str)->String {
    let boundary="OwnedLanguageUploadBoundary";
    let mut body=format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"language.png\"\r\nContent-Type: image/png\r\n\r\n").into_bytes();
    body.extend(png());body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
    let path=format!("/api/v1/upload/{session}");
    let value=f.raw("POST",&path,body.clone(),&format!("multipart/form-data; boundary={boundary}"),
        Some(proof(1,11,&format!("{}{path}",f.origin),"POST",&body)),200).await;
    value["data"][0]["id"].as_str().unwrap().to_owned()
}
fn chapter_draft(language:&str)->Value {
    json!({"volume":null,"chapter":"1","title":"Language chapter","translatedLanguage":language})
}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires owned isolated PostgreSQL and verified pinned strfry"]
async fn public_language_writes_reach_durable_storage_without_changing_native_contracts() {
    let mut f=fixture().await;
    let db=f.state.db.client().await.unwrap();
    let permissions=json!(["manga.create","manga.update","manga.title.edit","manga.cover.edit","scanlation_group.create","scanlation_group.update","self.scanlation_group.update","chapter.create","self.chapter.update","self.chapter.remove"]);
    db.execute("UPDATE platform.users SET permissions=$1 WHERE id IN ('PublicUploadUser1','PublicUploadUser2')", &[&permissions]).await.unwrap();
    let account=alias(&f,ResourceKind::User,"PublicUploadUser1").await;
    let other=alias(&f,ResourceKind::User,"PublicUploadUser2").await;
    let manga=alias(&f,ResourceKind::Manga,FIXTURE_MANGA_ID).await;
    // A valid public code absent from the retained native enum persists, not
    // merely normalizes successfully at the DTO boundary.
    let title=json!({"title":{"ta":"தமிழ்","opaque label":"Preserved text"},"altTitles":[{"zh-Hant":"原文"}],"originalLanguage":"ta","status":"ongoing","contentRating":"safe"});
    let created=f.call(Some(1),"POST","/api/v1/manga",Some(title.clone()),200).await;
    let title_id=created["data"]["id"].as_str().unwrap();let native=f.native(ResourceKind::Manga,title_id).await;
    assert_eq!(created["data"]["attributes"]["originalLanguage"],"ta");
    assert_eq!(db.query_one("SELECT language FROM platform.catalog_manga WHERE id=$1",&[&native]).await.unwrap().get::<_,String>(0),"ta");
    let before:i64=db.query_one("SELECT count(*) FROM platform.catalog_manga",&[]).await.unwrap().get(0);
    for language in ["tam","und","zh-hant","TA","en-US","ta "," ta","e","en_uk",""] {
        let mut invalid=title.clone();invalid["originalLanguage"]=json!(language);
        f.call(Some(1),"POST","/api/v1/manga",Some(invalid),400).await;
    }
    assert_eq!(db.query_one("SELECT count(*) FROM platform.catalog_manga",&[]).await.unwrap().get::<_,i64>(0),before);
    // Omitted unsupported labels are retained while ordinary edits remain usable.
    db.execute("UPDATE platform.catalog_manga SET language='tam' WHERE id=$1",&[&FIXTURE_MANGA_ID]).await.unwrap();
    let path=format!("/api/v1/manga/{manga}");
    let changed=f.call(Some(1),"PUT",&path,Some(json!({"version":1,"title":{"tam":"Native label, edited title"}})),200).await;
    assert_eq!(changed["data"]["attributes"]["originalLanguage"],"tam");
    // Key 4 is bound, admitted and verified but has no permissions. Key 2
    // has manga.update and intentionally may edit this published title.
    f.call(Some(4),"PUT",&path,Some(json!({"version":2,"originalLanguage":"ta"})),403).await;
    f.call(Some(1),"PUT",&path,Some(json!({"version":1,"originalLanguage":"ta"})),409).await;
    f.call(Some(1),"PUT",&path,Some(json!({"version":2,"originalLanguage":"ta","expectedAccountId":other})),409).await;
    let bytes=serde_json::to_vec(&json!({"version":2,"originalLanguage":"ta"})).unwrap();
    f.raw("PUT",&path,bytes.clone(),"application/json",Some(proof(1,12,&format!("{}{path}",f.origin),"PUT",&bytes)),401).await;
    f.call(Some(1),"PUT",&path,Some(json!({"version":2,"originalLanguage":"ta"})),200).await;
    native_put(&f,&format!("/api/manga/{FIXTURE_MANGA_ID}"),json!({"version":3,"language":"ta"}),400).await;
    native_put(&f,&format!("/api/manga/{FIXTURE_MANGA_ID}"),json!({"version":3,"language":"tam"}),200).await;
    assert_eq!(db.query_one("SELECT language FROM platform.catalog_manga WHERE id=$1",&[&FIXTURE_MANGA_ID]).await.unwrap().get::<_,String>(0),"tam");
    // Group write parser and durable metadata must not reapply the native enum.
    let group=f.call(Some(1),"POST","/api/v1/group",Some(json!({"name":"Language fixture"})),200).await;
    let group=group["data"]["id"].as_str().unwrap();let group_native=f.native(ResourceKind::Group,group).await;
    f.call(Some(1),"PUT",&format!("/api/v1/group/{group}"),Some(json!({"version":1,"focusedLanguages":["ta"]})),200).await;
    assert_eq!(db.query_one("SELECT metadata->'focused_languages' FROM platform.catalog_groups WHERE id=$1",&[&group_native]).await.unwrap().get::<_,Value>(0),json!(["ta"]));
    f.call(Some(1),"PUT",&format!("/api/v1/group/{group}"),Some(json!({"version":2,"focusedLanguages":["tam"]})),400).await;
    native_put(&f,&format!("/api/group/{group_native}"),json!({"version":2,"focused_languages":["tam"]}),200).await;
    native_put(&f,&format!("/api/group/{group_native}"),json!({"version":3,"focused_languages":["ta"]}),400).await;
    // Cover publication and update both reach repository validation.
    let cover_path=format!("/api/v1/cover/{manga}");
    let (body,mime)=cover_multipart(4,&account,&[("locale","ta")],false);
    let cover=f.raw("POST",&cover_path,body.clone(),&mime,Some(proof(1,11,&format!("{}{cover_path}",f.origin),"POST",&body)),201).await;
    let cover=cover["data"]["id"].as_str().unwrap();let cover_native=f.native(ResourceKind::Cover,cover).await;
    assert_eq!(db.query_one("SELECT language FROM platform.catalog_covers WHERE id=$1",&[&cover_native]).await.unwrap().get::<_,String>(0),"ta");
    f.call(Some(1),"PUT",&format!("/api/v1/cover/{cover}"),Some(json!({"version":1,"locale":"tam"})),400).await;
    native_put(&f,&format!("/api/cover/{cover_native}"),json!({"version":1,"language":"tam"}),200).await;
    let cover_edit=f.call(Some(1),"PUT",&format!("/api/v1/cover/{cover}"),Some(json!({"version":2,"description":"Language untouched"})),200).await;
    assert_eq!(cover_edit["data"]["attributes"]["locale"],"tam");
    f.call(Some(1),"PUT",&format!("/api/v1/cover/{cover}"),Some(json!({"version":3,"locale":"ta"})),200).await;
    // Chapter metadata omission, authority, version and native defaults survive.
    db.execute("INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,language,title,metadata) VALUES('LangChap01',$1,'PublicUploadUser1','Fixture','tam','Before','{}')",&[&FIXTURE_MANGA_ID]).await.unwrap();
    let chapter=alias(&f,ResourceKind::Chapter,"LangChap01").await;let chapter_path=format!("/api/v1/chapter/{chapter}");
    let edited=f.call(Some(1),"PUT",&chapter_path,Some(json!({"version":1,"title":"Changed only title"})),200).await;
    assert_eq!(edited["data"]["attributes"]["translatedLanguage"],"tam");
    f.call(Some(1),"PUT",&chapter_path,Some(json!({"version":2,"translatedLanguage":"ta "})),400).await;
    f.call(Some(2),"PUT",&chapter_path,Some(json!({"version":2,"translatedLanguage":"ta"})),403).await;
    f.call(Some(1),"PUT",&chapter_path,Some(json!({"version":1,"translatedLanguage":"ta"})),409).await;
    f.call(Some(1),"PUT",&chapter_path,Some(json!({"version":2,"translatedLanguage":"ta","expectedAccountId":other})),409).await;
    f.call(Some(1),"PUT",&chapter_path,Some(json!({"version":2,"translatedLanguage":"ta"})),200).await;
    native_put(&f,"/api/chapter/LangChap01",json!({"version":3,"language":"ta"}),400).await;
    native_put(&f,"/api/chapter/LangChap01",json!({"version":3,"language":"tam"}),200).await;
    // Search intentionally admits wider syntax; cover locales remain strict.
    for filter in ["tam","TA","EN-us","--","zz"] {
        for path in [format!("/api/v1/manga?originalLanguage[]={filter}"),format!("/api/v1/manga?excludedOriginalLanguage[]={filter}&availableTranslatedLanguage[]={filter}"),format!("/api/v1/chapter?translatedLanguage[]={filter}"),format!("/api/v1/manga/{manga}/feed?translatedLanguage[]={filter}"),format!("/api/v1/manga/{manga}/aggregate?translatedLanguage[]={filter}")] {
            f.call(None,"GET",&path,None,200).await;
        }
    }
    for path in ["/api/v1/manga?originalLanguage[]=zh-hant","/api/v1/chapter?translatedLanguage[]=1a","/api/v1/cover?locales[]=tam","/api/v1/cover?locales[]=TA"] {f.call(None,"GET",path,None,400).await;}
    f.call(None,"GET","/api/v1/cover?locales[]=ta",None,200).await;
    f.call(None,"GET","/api/v1/group?focusedLanguage=zh-hant",None,200).await;
    let group_matches=f.call(None,"GET","/api/v1/group?focusedLanguage=tam",None,200).await;assert_eq!(group_matches["total"],1);
    // Signed upload recovery and replacement use the same selected contract.
    let session=f.call(Some(1),"POST",&format!("/api/v1/upload/begin/{chapter}"),Some(json!({"version":4})),200).await;
    let session=session["id"].as_str().unwrap();let file=attach(&f,session).await;
    let native_session=f.native(ResourceKind::UploadSession,session).await;
    let version:i32=db.query_one("SELECT version FROM platform.upload_sessions WHERE id=$1",&[&native_session]).await.unwrap().get(0);
    let stage=json!({"version":version,"draft":{"language":"ta","title":"Recovery"},"pageOrder":[file]});
    f.call(Some(1),"PATCH",&format!("/api/v1/nostr/uploads/{session}"),Some(stage.clone()),200).await;
    assert_eq!(db.query_one("SELECT resume_draft->>'language' FROM platform.upload_sessions WHERE id=$1",&[&native_session]).await.unwrap().get::<_,String>(0),"ta");
    let mut invalid_stage=stage;invalid_stage["version"]=json!(version+1);invalid_stage["draft"]["language"]=json!("tam");
    f.call(Some(1),"PATCH",&format!("/api/v1/nostr/uploads/{session}"),Some(invalid_stage),400).await;
    let committed=f.call(Some(1),"POST",&format!("/api/v1/upload/{session}/commit"),Some(json!({"chapterDraft":chapter_draft("ta"),"pageOrder":[file],"termsAccepted":true})),200).await;
    assert_eq!(committed["attributes"]["translatedLanguage"],"ta");
    assert_eq!(db.query_one("SELECT language FROM platform.catalog_chapters WHERE id='LangChap01'",&[]).await.unwrap().get::<_,String>(0),"ta");
    // New public upload commits an actual original source and durably associates
    // the same ta label through verified-publication repository insertion.
    let session=f.call(Some(1),"POST","/api/v1/upload/begin",Some(json!({"manga":manga,"groups":[]})),200).await;
    let session=session["id"].as_str().unwrap();let file=attach(&f,session).await;
    f.call(Some(1),"POST","/api/v1/upload/check-approval-required",Some(json!({"manga":manga,"locale":"ta"})),200).await;
    f.call(Some(1),"POST","/api/v1/upload/check-approval-required",Some(json!({"manga":manga,"locale":"tam"})),400).await;
    f.call(Some(1),"POST",&format!("/api/v1/upload/{session}/commit"),Some(json!({"chapterDraft":chapter_draft("tam"),"pageOrder":[file],"termsAccepted":true})),400).await;
    let committed=f.call(Some(1),"POST",&format!("/api/v1/upload/{session}/commit"),Some(json!({"chapterDraft":chapter_draft("ta"),"pageOrder":[file],"termsAccepted":true})),200).await;
    assert_eq!(committed["attributes"]["translatedLanguage"],"ta");
    let native_session=f.native(ResourceKind::UploadSession,session).await;
    let row=db.query_one("SELECT c.language,c.protocol_event_id FROM platform.upload_sessions s JOIN platform.catalog_chapters c ON c.id=s.committed_chapter_id WHERE s.id=$1",&[&native_session]).await.unwrap();
    assert_eq!(row.get::<_,String>(0),"ta");let event:String=row.get(1);
    let source:[u8;32]=hex::decode(event).unwrap().try_into().unwrap();
    tokio::time::timeout(Duration::from_secs(20),async {loop {if f.state.protocol.entry(source).await.unwrap().is_some(){break;}tokio::time::sleep(Duration::from_millis(30)).await;}}).await.unwrap();
    let entry=f.state.protocol.entry(source).await.unwrap().unwrap();entry.event.verify().unwrap();assert_eq!(entry.metadata["language"],"ta");
    if let Some(mut listener)=f.native.take() {if let Some(shutdown)=listener.shutdown.take(){let _=shutdown.send(());}tokio::time::timeout(Duration::from_secs(30),listener.task).await.unwrap().unwrap().unwrap();}
}

#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires owned isolated PostgreSQL and verified pinned strfry"]
async fn imported_language_extensions_preserve_current_projection_and_original_source() {
    let mut f=fixture().await;let runtime=f.state.protocol.publication_runtime().await.unwrap();
    let db=f.state.db.client().await.unwrap();
    for label in ["und","tam","zh-Hant"] {
        let image=png();let page=manga_node::manifest::Page{hash:auth::hash(&image),size:image.len() as u64,mime:"image/png".into(),width:13,height:17};
        let metadata=manga_node::series_metadata::SeriesMetadata{titles:BTreeMap::from([("en".into(),format!("Original {label}"))]),alt_titles:None,description:None,categories:None,credits:None,original_language:Some(label.into()),status:Some("ongoing".into()),content_rating:Some("safe".into()),publication_demographic:None,year:None,cover:Some(page.clone())};
        let series=runtime.publish_operator(weebdex_platform::protocol::publication::PublicationInput{kind:1002,canonical_manifest:manga_node::platform_bridge::encode_series(&metadata).unwrap(),originals:BTreeMap::from([(page.hash.clone(),image.clone())]),created_at:None}).await.unwrap().event;
        let chapter=manga_node::manifest::Manifest{series:series.id.clone(),language:label.into(),title:Some(format!("Original {label}")),volume:None,chapter:Some("1".into()),pages:vec![page.clone()]};
        let chapter=runtime.publish_operator(weebdex_platform::protocol::publication::PublicationInput{kind:1001,canonical_manifest:manga_node::platform_bridge::encode_chapter(&chapter).unwrap(),originals:BTreeMap::from([(page.hash.clone(),image.clone())]),created_at:None}).await.unwrap().event;
        for event in [&series,&chapter] {let id:[u8;32]=hex::decode(&event.id).unwrap().try_into().unwrap();tokio::time::timeout(Duration::from_secs(20),async{loop{if f.state.protocol.entry(id).await.unwrap().is_some(){break;}tokio::time::sleep(Duration::from_millis(30)).await;}}).await.unwrap();}
        for _ in 0..8 {let result=weebdex_platform::protocol::bridge::import_page(&f.state).await.unwrap();assert_eq!(result.rejected,0);if result.completed_sweep&&result.deferred==0&&result.scanned>0{break;}}
        let expected=label.to_ascii_lowercase();
        for (kind,table,field,event_id,attribute) in [(ResourceKind::Manga,"catalog_manga","protocol_series_id",series.id.as_str(),"originalLanguage"),(ResourceKind::Chapter,"catalog_chapters","protocol_event_id",chapter.id.as_str(),"translatedLanguage"),(ResourceKind::Cover,"catalog_covers","protocol_event_id",series.id.as_str(),"locale")] {
            let row=db.query_one(&format!("SELECT id,language FROM platform.{table} WHERE {field}=$1"),&[&event_id]).await.unwrap();let native:String=row.get(0);assert_eq!(row.get::<_,String>(1),expected);
            let public=alias(&f,kind,&native).await;let route=match kind{ResourceKind::Manga=>"manga",ResourceKind::Chapter=>"chapter",_=>"cover"};
            let result=f.call(None,"GET",&format!("/api/v1/{route}/{public}"),None,200).await;assert_eq!(result["data"]["attributes"][attribute],expected);
        }
        for (event,field) in [(&series,"originalLanguage"),(&chapter,"language")] {
            let id:[u8;32]=hex::decode(&event.id).unwrap().try_into().unwrap();let retained=f.state.protocol.entry(id).await.unwrap().unwrap();
            assert_eq!(serde_json::to_value(&retained.event).unwrap(),serde_json::to_value(event).unwrap());assert_eq!(retained.metadata[field],label);retained.event.verify().unwrap();
        }
    }
    if let Some(mut listener)=f.native.take() {if let Some(shutdown)=listener.shutdown.take(){let _=shutdown.send(());}tokio::time::timeout(Duration::from_secs(30),listener.task).await.unwrap().unwrap().unwrap();}
}

#[path = "public_language_restart.rs"]
mod restart;
