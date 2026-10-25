//! Real signed HTTP/PostgreSQL paging through more than 100 covers, with an
//! older main original. Every row, account, signature, image and database is an
//! owned disposable fixture. No existing runtime data or credentials are used.
use super::*;
#[path="../../../../manga-protocol-lab/server/tests/support/mod.rs"]
mod original_fixture;

async fn alias(f: &Fixture, kind: ResourceKind, native: &str) -> String {
    PostgresPublicIdResolver::new(f.state.db.clone()).preferred(kind,native).await.unwrap().unwrap().to_string()
}
async fn seed(f: &Fixture) -> (String,String,String,Value) {
    let db=f.state.db.client().await.unwrap();
    db.batch_execute(r#"
        UPDATE platform.users SET permissions='["manga.create","manga.cover.edit"]' WHERE id='PublicUploadUser1';
        UPDATE platform.users SET roles='["moderator"]',permissions='["cover.delete"]' WHERE id='PublicUploadUser8';
        INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES('PageOther1','PublicUploadUser2','Fixture','Other title','en','ongoing','safe','draft');
    "#).await.unwrap();
    let manga=alias(f,ResourceKind::Manga,FIXTURE_MANGA_ID).await;
    let account=alias(f,ResourceKind::User,"PublicUploadUser1").await;
    let path=format!("/api/v1/cover/{manga}");
    let (body,mime)=cover_multipart(1,&account,&[("locale","en"),("main","true"),("description","Older main exact original")],false);
    let original=f.raw("POST",&path,body.clone(),&mime,Some(proof(1,11,&format!("{}{path}",f.origin),"POST",&body)),201).await;
    let cover=original["data"]["id"].as_str().unwrap().to_owned();let native=f.native(ResourceKind::Cover,&cover).await;
    db.execute("UPDATE platform.catalog_covers SET created_at='2025-01-01' WHERE id=$1",&[&native]).await.unwrap();
    // 204 equal-time newer records exercise deterministic tie ordering. Hidden,
    // removed and foreign records must not leak into either data or totals.
    db.batch_execute(r#"
        INSERT INTO platform.catalog_covers(id,manga_id,uploader_id,uploader_name,ext,dimensions,volume,language,created_at)
        SELECT 'PageCov'||lpad(i::text,3,'0'),'UploadMng1','PublicUploadUser1','Fixture','png',ARRAY[13,17],i::text,'en','2026-01-01'::timestamptz FROM generate_series(1,204) AS i;
        INSERT INTO platform.catalog_covers(id,manga_id,uploader_id,uploader_name,ext,dimensions,language,created_at) VALUES
          ('PageDelete','UploadMng1','PublicUploadUser1','Fixture','png',ARRAY[13,17],'en','2026-02-01'),
          ('PageHidden','UploadMng1','PublicUploadUser1','Fixture','png',ARRAY[13,17],'en','2026-02-01'),
          ('PageOtherC','PageOther1','PublicUploadUser2','Fixture','png',ARRAY[13,17],'en','2026-02-01');
        UPDATE platform.catalog_covers SET deleted_at=clock_timestamp() WHERE id='PageDelete';
        UPDATE platform.catalog_covers SET protocol_event_id=repeat('f',64) WHERE id='PageHidden';
    "#).await.unwrap();
    (manga,account,cover,original)
}
async fn page(f:&Fixture,manga:&str,account:&str,offset:u64,limit:u32)->Value {
    f.call(Some(1),"GET",&format!("/api/v1/nostr/manga/{manga}/covers?limit={limit}&offset={offset}&expectedAccountId={account}"),None,200).await
}
async fn covers_and_intent(f:&Fixture,manga:&str,account:&str,cover:&str,original:&Value) {
    let mut seen=std::collections::BTreeSet::new();
    for (offset,count) in [(0,100),(100,100),(200,5)] {
        let result=page(f,manga,account,offset,100).await;
        assert_eq!(result["limit"],100);assert_eq!(result["offset"],offset);assert_eq!(result["total"],205);
        let rows=result["data"].as_array().unwrap();assert_eq!(rows.len(),count);
        for (index,row) in rows.iter().enumerate() {
            let id=row["id"].as_str().unwrap();assert!(seen.insert(id.to_owned()));
            assert!(row["relationships"].as_array().unwrap().iter().any(|r|r["type"]=="manga"&&r["id"]==manga));
            let rank=offset as usize+index+1;
            if rank<=204 {assert_eq!(f.native(ResourceKind::Cover,id).await,format!("PageCov{rank:03}"));assert_eq!(row["attributes"]["nostr"]["main"],false);}
            else {assert_eq!(id,cover);assert_eq!(row["attributes"]["nostr"]["main"],true);assert_eq!(row["attributes"]["nostr"]["original"],original["data"]["attributes"]["nostr"]["original"]);}
        }
    }
    assert_eq!(seen.len(),205);
    let previous=page(f,manga,account,100,100).await;assert_eq!(previous["data"][0]["id"],alias(f,ResourceKind::Cover,"PageCov101").await);
    for (offset,limit) in [(0,0),(205,100),(10100,100),(4294967296,100),(9007199254740991,100)] {
        let result=page(f,manga,account,offset,limit).await;assert_eq!(result["total"],205);assert!(result["data"].as_array().unwrap().is_empty());assert_eq!(result["offset"],offset);
    }
    let base=format!("/api/v1/nostr/manga/{manga}/covers");
    for query in ["limit=101","offset=-1","offset=1.5","offset=9007199254740992","limit=100&limit=1","offset=0&offset=100","manga[]=11111111-1111-8111-8111-111111111111","unknown=1"] {
        f.call(Some(1),"GET",&format!("{base}?{query}"),None,400).await;
    }
    let path=format!("{base}?limit=100&offset=200&expectedAccountId={account}");
    f.call(None,"GET",&path,None,401).await;
    let token=proof(1,11,&format!("{}{base}?limit=100&offset=100&expectedAccountId={account}",f.origin),"GET",&[]);
    f.raw("GET",&path,Vec::new(),"application/json",Some(token),401).await;
    f.raw("GET",&path,Vec::new(),"application/json",Some(proof(1,12,&format!("{}{path}",f.origin),"GET",&[])),401).await;
    let db=f.state.db.client().await.unwrap();
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser2' WHERE publisher=$1",&[&public(1)]).await.unwrap();
    f.call(Some(1),"GET",&path,None,409).await;
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser1' WHERE publisher=$1",&[&public(1)]).await.unwrap();
    let foreign=alias(f,ResourceKind::Manga,"PageOther1").await;
    f.call(Some(1),"GET",&format!("/api/v1/nostr/manga/{foreign}/covers?limit=100&offset=0&expectedAccountId={account}"),None,404).await;
    for (change,restore) in [("protocol_source_visible=FALSE","protocol_source_visible=TRUE"),("state='draft'","state='published'")] {
        db.batch_execute(&format!("UPDATE platform.catalog_manga SET {change} WHERE id='UploadMng1'")).await.unwrap();
        if change.starts_with("state") {f.call(Some(2),"GET",&base,None,404).await;}else{f.call(Some(1),"GET",&path,None,404).await;}
        db.batch_execute(&format!("UPDATE platform.catalog_manga SET {restore} WHERE id='UploadMng1'")).await.unwrap();
    }
    f.repository.set_account_admission(id(1),false).await.unwrap();f.call(Some(1),"GET",&path,None,403).await;f.repository.set_account_admission(id(1),true).await.unwrap();
}
async fn edits_and_removal(f:&Fixture,manga:&str,account:&str,cover:&str,original:&Value) {
    let target=format!("/api/v1/cover/{cover}");
    let updated=f.call(Some(1),"PUT",&target,Some(json!({"version":1,"description":"Edited older main"})),200).await;
    assert_eq!(updated["data"]["attributes"]["version"],2);assert_eq!(updated["data"]["attributes"]["nostr"],original["data"]["attributes"]["nostr"]);
    f.call(Some(1),"PUT",&target,Some(json!({"version":1,"description":"Stale page"})),409).await;
    f.call(Some(2),"PUT",&target,Some(json!({"version":2,"description":"No real cover grant"})),403).await;
    let db=f.state.db.client().await.unwrap();let native=f.native(ResourceKind::Cover,cover).await;
    let key:String=db.query_one("SELECT o.storage_key FROM platform.cover_media c JOIN platform.media_objects o ON o.id=c.object_id WHERE c.cover_id=$1 AND c.variant='original'",&[&native]).await.unwrap().get(0);
    assert_eq!(tokio::fs::read(domains::uploads::storage::object_path(&f.state.config.uploads_dir,&key).unwrap()).await.unwrap(),png());
    let preview=format!("/api/v1/nostr/covers/{cover}/original");let response=f.http.get(format!("{}{preview}",f.origin)).header("authorization",proof(1,11,&format!("{}{preview}",f.origin),"GET",&[])).send().await.unwrap();assert_eq!(response.status(),200);assert_eq!(response.bytes().await.unwrap().to_vec(),png());
    let after=page(f,manga,account,200,100).await;assert_eq!(after["data"][4]["attributes"]["version"],2);
    f.call(Some(1),"DELETE",&target,Some(json!({"version":2})),403).await;
    f.call(Some(8),"DELETE",&target,Some(json!({"version":1})),409).await;
    let moderator=alias(f,ResourceKind::User,"PublicUploadUser8").await;let body=serde_json::to_vec(&json!({"version":2,"expectedAccountId":moderator})).unwrap();
    let response=f.http.delete(format!("{}{target}",f.origin)).header("content-type","application/json").header("authorization",proof(8,11,&format!("{}{target}",f.origin),"DELETE",&body)).body(body).send().await.unwrap();assert_eq!(response.status(),204);
    let after=page(f,manga,account,200,100).await;assert_eq!(after["total"],204);assert_eq!(after["data"].as_array().unwrap().len(),4);assert!(after["data"].as_array().unwrap().iter().all(|r|r["id"]!=cover));
    f.call(Some(1),"PUT",&target,Some(json!({"version":2,"description":"Removed selection"})),404).await;
}
async fn genuine_source_visibility(f:&Fixture) {
    use std::collections::BTreeMap;
    use weebdex_platform::protocol::{bridge,publication::PublicationInput};
    let runtime=f.state.protocol.publication_runtime().await.unwrap();let bytes=original_fixture::png();
    let event=runtime.publish_operator(PublicationInput{kind:1002,canonical_manifest:original_fixture::cbor(json!({"type":"series","version":1,"titles":{"en":"Paged source"},"contentRating":"safe","required":["series-cover-v1"],"cover":original_fixture::page(&bytes)})),originals:BTreeMap::from([(auth::hash(&bytes),bytes.clone())]),created_at:None}).await.unwrap().event;
    let event_id:[u8;32]=hex::decode(&event.id).unwrap().try_into().unwrap();tokio::time::timeout(Duration::from_secs(20),async{loop{if f.state.protocol.entry(event_id).await.unwrap().is_some(){break;}tokio::time::sleep(Duration::from_millis(30)).await;}}).await.unwrap();
    for _ in 0..8 {let result=bridge::import_page(&f.state).await.unwrap();assert_eq!(result.rejected,0);if result.completed_sweep&&result.deferred==0&&result.scanned>0{break;}}
    let db=f.state.db.client().await.unwrap();
    let source=db.query_one("SELECT id,manga_id FROM platform.catalog_covers WHERE protocol_event_id=$1",&[&event.id]).await.unwrap();
    let cover:String=source.get(0);let native_manga:String=source.get(1);
    // The real schema allows one cover per source event. Keep that genuine
    // identity and parent unchanged, adding ordinary owned fixture covers to
    // its title instead of fabricating duplicate source-event references.
    db.execute("UPDATE platform.catalog_covers SET created_at='2025-01-01' WHERE id=$1",&[&cover]).await.unwrap();
    let ids:Vec<String>=(1..=101).map(|index|format!("SrcCovr{index:03}")).collect();
    assert!(ids.iter().all(|id|domains::uploads::storage::valid_id(id)),"source page fixture IDs must preserve the native filename contract");
    db.execute("INSERT INTO platform.catalog_covers(id,manga_id,uploader_id,uploader_name,ext,dimensions,language,created_at) SELECT id,$1,'PublicUploadUser1','Fixture','png',ARRAY[13,17],'en','2026-01-01'::timestamptz FROM unnest($2::text[]) AS id",&[&native_manga,&ids]).await.unwrap();
    let manga=alias(f,ResourceKind::Manga,&native_manga).await;let account=alias(f,ResourceKind::User,"PublicUploadUser1").await;
    let tail=page(f,&manga,&account,100,100).await;assert_eq!(tail["total"],102);assert_eq!(tail["data"].as_array().unwrap().len(),2);
    let source_cover=alias(f,ResourceKind::Cover,&cover).await;
    assert_eq!(tail["data"][1]["id"],source_cover);
    let edited=f.call(Some(1),"PUT",&format!("/api/v1/cover/{source_cover}"),Some(json!({"version":1,"description":"Annotation on the genuine source cover"})),200).await;
    assert_eq!(edited["data"]["attributes"]["nostr"]["sourceEventId"],event.id);assert_eq!(edited["data"]["attributes"]["nostr"]["originalEventRewritten"],false);
    let hash:[u8;32]=hex::decode(auth::hash(&bytes)).unwrap().try_into().unwrap();let (original,mime)=runtime.download_original(hash).await.unwrap().unwrap();let actual=axum::body::to_bytes(original.response(None,&mime,false).into_body(),bytes.len()+1).await.unwrap();assert_eq!(actual.to_vec(),bytes);
    db.execute("UPDATE events SET state='withdrawing' WHERE id=$1",&[&&event_id[..]]).await.unwrap();
    f.call(Some(1),"GET",&format!("/api/v1/nostr/manga/{manga}/covers?limit=100&offset=100&expectedAccountId={account}"),None,404).await;
    f.call(Some(1),"PUT",&format!("/api/v1/cover/{source_cover}"),Some(json!({"version":2,"description":"Hidden page stale selection"})),404).await;
    let other=alias(f,ResourceKind::Manga,FIXTURE_MANGA_ID).await;
    let untouched=page(f,&other,&account,200,100).await;assert_eq!(untouched["total"],204);assert_eq!(untouched["data"].as_array().unwrap().len(),4);
    assert_eq!(db.query_one("SELECT raw_event FROM events WHERE id=$1",&[&&event_id[..]]).await.unwrap().get::<_,Value>(0),serde_json::to_value(&event).unwrap());event.verify().unwrap();
    assert!(db.query_opt("SELECT cover_id FROM platform.protocol_cover_sources WHERE cover_id=$1",&[&cover]).await.unwrap().is_some());
}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires owned isolated PostgreSQL and pinned strfry"]
async fn signed_cover_pagination_full_history_and_fences() {
    let mut f=fixture().await;let (manga,account,cover,original)=seed(&f).await;
    covers_and_intent(&f,&manga,&account,&cover,&original).await;
    edits_and_removal(&f,&manga,&account,&cover,&original).await;
    genuine_source_visibility(&f).await;
    let mut native=f.native.take().unwrap();if let Some(shutdown)=native.shutdown.take(){let _=shutdown.send(());}tokio::time::timeout(Duration::from_secs(30),native.task).await.unwrap().unwrap().unwrap();
    println!("SIGNED_COVER_PAGINATION_VERIFIED=205 covers, older main original, exact pages and totals, deterministic ordering, no 10000 or 32-bit cap, account/service/signature and grant fences, stale versions, foreign/hidden/deleted exclusion, immutable bytes and signed source");
}
