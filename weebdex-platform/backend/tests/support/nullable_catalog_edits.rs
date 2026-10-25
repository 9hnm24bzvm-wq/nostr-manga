//! Pinned ChapterRequest/CoverEdit null semantics, exercised through the real
//! validators, transaction writers, signed TCP router and PostgreSQL projections.
//! All identities and originals are disposable test fixtures.
use super::*;
use weebdex_platform::domains::{auth::optional_principal, catalog::CatalogRepository};

async fn snapshot(f: &Fixture) -> Value {
    f.state.db.client().await.unwrap().query_one(
        "SELECT jsonb_build_object('chapters',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM platform.catalog_chapters c),'covers',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM platform.catalog_covers c),'groups',(SELECT jsonb_agg(to_jsonb(g) ORDER BY chapter_id,group_id) FROM platform.catalog_chapter_groups g),'audit',(SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM platform.catalog_audit a))", &[]
    ).await.unwrap().get(0)
}
async fn read(f: &Fixture, path: &str) -> Value {
    let response=f.http.get(format!("{}{path}",f.origin)).send().await.unwrap();
    assert_eq!(response.status().as_u16(),200);
    response.json::<Value>().await.unwrap()["data"]["attributes"].clone()
}
async fn refused(f: &Fixture, key: Option<u8>, path: &str, patch: Value, status: u16) {
    let before=snapshot(f).await;
    f.call(key,"PUT",path,Some(patch),status).await;
    assert_eq!(snapshot(f).await,before,"refused nullable edit changed catalogue or audit");
}

async fn transactions_preserve_presence_and_rollback(f: &Fixture) {
    let mut headers=axum::http::HeaderMap::new();
    headers.insert("cookie","weebdex_session=synthetic-public-upload-cookie".parse().unwrap());
    let actor=optional_principal(&f.state,&headers).await.unwrap().unwrap();
    let before=snapshot(f).await;
    let mut client=f.state.db.client().await.unwrap();
    let tx=client.transaction().await.unwrap();
    let omitted=CatalogRepository::update_chapter(&tx,"NullCh0001",&actor,&json!({"version":1})).await.unwrap();
    assert_eq!(omitted["title"],"Before");assert_eq!(omitted["volume"],"1");assert_eq!(omitted["chapter"],"2");
    let cleared=CatalogRepository::update_chapter(&tx,"NullCh0001",&actor,&json!({"version":2,"title":null,"volume":null,"chapter":null})).await.unwrap();
    for field in ["title","volume","chapter"] {assert!(cleared[field].is_null());}
    let row=tx.query_one("SELECT title IS NULL,volume IS NULL,chapter IS NULL,language,version FROM platform.catalog_chapters WHERE id='NullCh0001'",&[]).await.unwrap();
    for i in 0..3 {assert!(row.get::<_,bool>(i));}assert_eq!(row.get::<_,String>(3),"en");assert_eq!(row.get::<_,i32>(4),3);
    let omitted=CatalogRepository::update_cover(&tx,"NullCv0001",&actor,&json!({"version":1})).await.unwrap();
    assert_eq!(omitted["description"],"Before");assert_eq!(omitted["language"],"ja");assert_eq!(omitted["volume"],"1");
    CatalogRepository::update_cover(&tx,"NullCv0001",&actor,&json!({"version":2,"description":null,"language":null})).await.unwrap();
    let row=tx.query_one("SELECT description IS NULL,language,volume,version FROM platform.catalog_covers WHERE id='NullCv0001'",&[]).await.unwrap();
    assert!(row.get::<_,bool>(0));assert_eq!(row.get::<_,String>(1),"");assert_eq!(row.get::<_,String>(2),"1");assert_eq!(row.get::<_,i32>(3),3);
    // No writer commits for its caller, including clear operations and audits.
    tx.rollback().await.unwrap();
    assert_eq!(snapshot(f).await,before);
}

async fn signed_http_preserves_null_omission_and_fences(f: &Fixture) {
    let account=alias(f,ResourceKind::User,"PublicUploadUser1").await;
    let other=alias(f,ResourceKind::User,"PublicUploadUser2").await;
    let chapter=format!("/api/v1/chapter/{}",alias(f,ResourceKind::Chapter,"NullCh0001").await);
    let cover=format!("/api/v1/cover/{}",alias(f,ResourceKind::Cover,"NullCv0001").await);
    // Exercise each clear independently so an unrelated missing field can never
    // be accidentally cleared. Public responses and later GETs must agree.
    for (index,field) in ["title","volume","chapter"].into_iter().enumerate() {
        let result=f.call(Some(1),"PUT",&chapter,Some(json!({"version":index+1,"expectedAccountId":account,field:null})),200).await;
        let attrs=&result["data"]["attributes"];
        assert!(attrs[field].is_null());assert_eq!(attrs["version"],index+2);
        if index==0 {assert_eq!(attrs["volume"],"1");assert_eq!(attrs["chapter"],"2");}
        if index==1 {assert_eq!(attrs["chapter"],"2");}
        assert_eq!(attrs["translatedLanguage"],"en");
        assert_eq!(*attrs,read(f,&chapter).await);
    }
    let omitted=f.call(Some(1),"PUT",&chapter,Some(json!({"version":4,"expectedAccountId":account})),200).await;
    for field in ["title","volume","chapter"] {assert!(omitted["data"]["attributes"][field].is_null());}
    let restored=f.call(Some(1),"PUT",&chapter,Some(json!({"version":5,"expectedAccountId":account,"title":"Restored","volume":"3","chapter":"4"})),200).await;
    let held=f.call(Some(1),"PUT",&chapter,Some(json!({"version":6,"expectedAccountId":account,"translatedLanguage":"ja"})),200).await;
    for field in ["title","volume","chapter"] {assert_eq!(held["data"]["attributes"][field],restored["data"]["attributes"][field]);}
    assert_eq!(held["data"]["attributes"]["translatedLanguage"],"ja");

    let description=f.call(Some(1),"PUT",&cover,Some(json!({"version":1,"expectedAccountId":account,"description":null})),200).await;
    assert!(description["data"]["attributes"]["description"].is_null());assert_eq!(description["data"]["attributes"]["locale"],"ja");assert_eq!(description["data"]["attributes"]["volume"],"1");
    let locale=f.call(Some(1),"PUT",&cover,Some(json!({"version":2,"expectedAccountId":account,"locale":null})),200).await;
    assert!(locale["data"]["attributes"]["locale"].is_null());assert!(locale["data"]["attributes"]["description"].is_null());assert_eq!(locale["data"]["attributes"]["volume"],"1");
    let volume=f.call(Some(1),"PUT",&cover,Some(json!({"version":3,"expectedAccountId":account,"volume":null})),200).await;
    for field in ["description","locale","volume"] {assert!(volume["data"]["attributes"][field].is_null());}
    let omitted=f.call(Some(1),"PUT",&cover,Some(json!({"version":4,"expectedAccountId":account})),200).await;
    for field in ["description","locale","volume"] {assert!(omitted["data"]["attributes"][field].is_null());}
    let current=read(f,&cover).await;
    for field in ["description","locale","volume","version","fileName","dimensions"] {assert_eq!(omitted["data"]["attributes"][field],current[field]);}
    let restored=f.call(Some(1),"PUT",&cover,Some(json!({"version":5,"expectedAccountId":account,"description":"Restored","locale":"en","volume":"3"})),200).await;
    let held=f.call(Some(1),"PUT",&cover,Some(json!({"version":6,"expectedAccountId":account})),200).await;
    for field in ["description","locale","volume"] {assert_eq!(held["data"]["attributes"][field],restored["data"]["attributes"][field]);}
    assert_eq!(held["data"]["attributes"]["fileName"],description["data"]["attributes"]["fileName"]);
    assert_eq!(held["data"]["attributes"]["dimensions"],json!([13,17]));
    assert_eq!(held["data"]["attributes"]["nostr"]["originalEventRewritten"],false);

    for (path,field) in [(&chapter,"title"),(&cover,"description")] {
        refused(f,Some(1),path,json!({"version":6,"expectedAccountId":account,field:null}),409).await;
        refused(f,None,path,json!({"version":7,"expectedAccountId":account,field:null}),401).await;
        refused(f,Some(2),path,json!({"version":7,"expectedAccountId":other,field:null}),403).await;
        refused(f,Some(1),path,json!({"version":7,"expectedAccountId":other,field:null}),409).await;
        refused(f,Some(1),path,json!({"version":7,"expectedAccountId":account,field:42}),400).await;
        refused(f,Some(1),path,json!({"version":null,"expectedAccountId":account,field:null}),400).await;
    }
    refused(f,Some(1),&chapter,json!({"version":7,"expectedAccountId":account,"title":null,"translatedLanguage":null}),400).await;
    refused(f,Some(1),&chapter,json!({"version":7,"expectedAccountId":account,"title":null,"groups":[account]}),404).await;
    refused(f,Some(1),&cover,json!({"version":7,"expectedAccountId":account,"description":null,"locale":""}),400).await;
    refused(f,Some(1),&cover,json!({"version":7,"description":null}),400).await;
    for kind in ["chapter","cover"] {refused(f,Some(1),&format!("/api/v1/{kind}/{account}"),json!({"version":1,"expectedAccountId":account}),404).await;}
    let db=f.state.db.client().await.unwrap();
    db.execute("UPDATE platform.catalog_manga SET protocol_source_visible=FALSE WHERE id='UploadMng1'",&[]).await.unwrap();
    for (path,field) in [(&chapter,"title"),(&cover,"description")] {refused(f,Some(1),path,json!({"version":7,"expectedAccountId":account,field:null}),404).await;}
    db.execute("UPDATE platform.catalog_manga SET protocol_source_visible=TRUE WHERE id='UploadMng1'",&[]).await.unwrap();
    let row=db.query_one("SELECT c.metadata,c.uploader_id,c.language,c.version,(SELECT count(*) FROM platform.catalog_audit WHERE entity_id=c.id) FROM platform.catalog_chapters c WHERE c.id='NullCh0001'",&[]).await.unwrap();
    assert_eq!(row.get::<_,Value>(0),json!({"data":[{"name":"immutable.png","dimensions":[13,17]}],"protocol_source_metadata":{"preserved":true}}));
    assert_eq!(row.get::<_,String>(1),"PublicUploadUser1");assert_eq!(row.get::<_,String>(2),"ja");assert_eq!(row.get::<_,i32>(3),7);assert_eq!(row.get::<_,i64>(4),6);
    let row=db.query_one("SELECT c.ext,c.dimensions,c.is_main,c.version,(SELECT count(*) FROM platform.catalog_audit WHERE entity_id=c.id) FROM platform.catalog_covers c WHERE c.id='NullCv0001'",&[]).await.unwrap();
    assert_eq!(row.get::<_,String>(0),"png");assert_eq!(row.get::<_,Vec<i32>>(1),vec![13,17]);assert!(!row.get::<_,bool>(2));assert_eq!(row.get::<_,i32>(3),7);assert_eq!(row.get::<_,i64>(4),6);
}

async fn signed_originals_and_source_metadata_remain_immutable(f: &Fixture) {
    use std::collections::BTreeMap;
    use weebdex_platform::protocol::{bridge,publication::PublicationInput};
    let runtime=f.state.protocol.publication_runtime().await.unwrap();
    let bytes=original_fixture::png();
    let page=original_fixture::page(&bytes);
    let series=runtime.publish_operator(PublicationInput{kind:1002,canonical_manifest:original_fixture::cbor(json!({"type":"series","version":1,"titles":{"en":"Nullable source"},"contentRating":"safe","required":["series-cover-v1"],"cover":page})),originals:BTreeMap::from([(auth::hash(&bytes),bytes.clone())]),created_at:None}).await.unwrap().event;
    let source=runtime.publish_operator(PublicationInput{kind:1001,canonical_manifest:original_fixture::cbor(json!({"type":"chapter","version":1,"series":series.id,"language":"en","title":"Signed title","volume":"1","chapter":"2","pages":[page]})),originals:BTreeMap::from([(auth::hash(&bytes),bytes)]),created_at:None}).await.unwrap().event;
    for event in [&series.id,&source.id] {
        let id:[u8;32]=hex::decode(event).unwrap().try_into().unwrap();
        tokio::time::timeout(Duration::from_secs(20),async {loop {if f.state.protocol.entry(id).await.unwrap().is_some(){break;}tokio::time::sleep(Duration::from_millis(30)).await;}}).await.unwrap();
    }
    for _ in 0..8 {let result=bridge::import_page(&f.state).await.unwrap();assert_eq!(result.rejected,0);if result.completed_sweep && result.deferred==0 && result.scanned>0 {break;}}
    let db=f.state.db.client().await.unwrap();
    let native:String=db.query_one("SELECT id FROM platform.catalog_chapters WHERE protocol_event_id=$1",&[&source.id]).await.unwrap().get(0);
    let cover:String=db.query_one("SELECT id FROM platform.catalog_covers WHERE protocol_event_id=$1",&[&series.id]).await.unwrap().get(0);
    let before:Value=db.query_one("SELECT metadata - 'instance_overlay' FROM platform.catalog_chapters WHERE id=$1",&[&native]).await.unwrap().get(0);
    let events_before:Value=db.query_one("SELECT jsonb_agg(raw_event ORDER BY id) FROM events",&[]).await.unwrap().get(0);
    let account=alias(f,ResourceKind::User,"PublicUploadUser1").await;
    let chapter_path=format!("/api/v1/chapter/{}",alias(f,ResourceKind::Chapter,&native).await);
    let cover_path=format!("/api/v1/cover/{}",alias(f,ResourceKind::Cover,&cover).await);
    f.call(Some(1),"PUT",&chapter_path,Some(json!({"version":1,"expectedAccountId":account,"title":null,"volume":null,"chapter":null})),200).await;
    f.call(Some(1),"PUT",&cover_path,Some(json!({"version":1,"expectedAccountId":account,"locale":null,"description":null})),200).await;
    let row=db.query_one("SELECT title IS NULL,volume IS NULL,chapter IS NULL,metadata - 'instance_overlay',metadata->'instance_overlay' FROM platform.catalog_chapters WHERE id=$1",&[&native]).await.unwrap();
    for i in 0..3 {assert!(row.get::<_,bool>(i));}assert_eq!(row.get::<_,Value>(3),before);
    let overlay:Value=row.get(4);for field in ["title","volume","chapter"] {assert_eq!(overlay.get(field),Some(&Value::Null));}assert_eq!(overlay["scope"],"instance-local");
    for _ in 0..8 {let result=bridge::import_page(&f.state).await.unwrap();assert_eq!(result.rejected,0);if result.completed_sweep {break;}}
    let attrs=read(f,&chapter_path).await;for field in ["title","volume","chapter"] {assert!(attrs[field].is_null());}
    let attrs=read(f,&cover_path).await;assert!(attrs["locale"].is_null());assert!(attrs["description"].is_null());
    let events_after:Value=db.query_one("SELECT jsonb_agg(raw_event ORDER BY id) FROM events",&[]).await.unwrap().get(0);assert_eq!(events_after,events_before);
    let row=db.query_one("SELECT metadata - 'instance_overlay',protocol_event_id FROM platform.catalog_chapters WHERE id=$1",&[&native]).await.unwrap();assert_eq!(row.get::<_,Value>(0),before);assert_eq!(row.get::<_,String>(1),source.id);
}

async fn nullable_fixture() -> Fixture {
    let f=fixture().await;
    f.state.db.client().await.unwrap().batch_execute(r#"
        UPDATE platform.users SET permissions=permissions || '["manga.cover.edit"]'::jsonb WHERE id='PublicUploadUser1';
        INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,language,title,volume,chapter,metadata) VALUES('NullCh0001','UploadMng1','PublicUploadUser1','Fixture','en','Before','1','2','{"data":[{"name":"immutable.png","dimensions":[13,17]}],"protocol_source_metadata":{"preserved":true}}');
        INSERT INTO platform.catalog_covers(id,manga_id,uploader_id,uploader_name,ext,dimensions,volume,language,description) VALUES('NullCv0001','UploadMng1','PublicUploadUser1','Fixture','png',ARRAY[13,17],'1','ja','Before');
    "#).await.unwrap();
    f
}

async fn stop_native(f: &mut Fixture) {
    let mut native=f.native.take().unwrap();if let Some(shutdown)=native.shutdown.take(){let _=shutdown.send(());}
    tokio::time::timeout(Duration::from_secs(30),native.task).await.unwrap().unwrap().unwrap();
}

#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires scripts/run-auth-tests.sh and owned isolated PostgreSQL"]
async fn nullable_catalog_edits_preserve_clears_omission_authority_and_originals() {
    let mut f=nullable_fixture().await;
    transactions_preserve_presence_and_rollback(&f).await;
    signed_http_preserves_null_omission_and_fences(&f).await;
    signed_originals_and_source_metadata_remain_immutable(&f).await;
    stop_native(&mut f).await;
    println!("NULLABLE_CATALOG_EDITS_VERIFIED=explicit clears, omission, transaction rollback, signed HTTP and GET, stale version, account intent, foreign scope, hidden parent, unchanged original events and metadata");
}

// Separately selectable real HTTP regressions also serve as old-source
// controls: each must fail at the unchanged route's rejection, not at an
// unrelated fixture setup, missing helper, or the transaction regression.
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires scripts/run-auth-tests.sh and owned isolated PostgreSQL"]
async fn nullable_chapter_http_clear() {
    let mut f=nullable_fixture().await;
    let path=format!("/api/v1/chapter/{}",alias(&f,ResourceKind::Chapter,"NullCh0001").await);
    let account=alias(&f,ResourceKind::User,"PublicUploadUser1").await;
    let result=f.call(Some(1),"PUT",&path,Some(json!({"version":1,"expectedAccountId":account,"title":null,"volume":null,"chapter":null})),200).await;
    for field in ["title","volume","chapter"] {assert!(result["data"]["attributes"][field].is_null());}
    stop_native(&mut f).await;
}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires scripts/run-auth-tests.sh and owned isolated PostgreSQL"]
async fn nullable_cover_http_clear() {
    let mut f=nullable_fixture().await;
    let path=format!("/api/v1/cover/{}",alias(&f,ResourceKind::Cover,"NullCv0001").await);
    let account=alias(&f,ResourceKind::User,"PublicUploadUser1").await;
    let result=f.call(Some(1),"PUT",&path,Some(json!({"version":1,"expectedAccountId":account,"description":null,"locale":null})),200).await;
    for field in ["description","locale"] {assert!(result["data"]["attributes"][field].is_null());}
    stop_native(&mut f).await;
}
