//! Pinned MangaRequest.primaryCover through real PostgreSQL transactions and
//! signed HTTP. Keys, originals and accounts below are disposable owned fixtures.
use super::*;
use weebdex_platform::domains::{auth::optional_principal,catalog::CatalogRepository};
#[path="../../../../manga-protocol-lab/server/tests/support/mod.rs"]
mod original_fixture;

async fn alias(f:&Fixture,kind:ResourceKind,native:&str)->String {
    PostgresPublicIdResolver::new(f.state.db.clone()).preferred(kind,native).await.unwrap().unwrap().to_string()
}
async fn snapshot(f:&Fixture)->Value {
    f.state.db.client().await.unwrap().query_one("SELECT jsonb_build_object('manga',(SELECT jsonb_agg(to_jsonb(m) ORDER BY id) FROM platform.catalog_manga m),'covers',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM platform.catalog_covers c),'audit',(SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM platform.catalog_audit a),'commands',(SELECT jsonb_agg(to_jsonb(p) ORDER BY command_id) FROM platform.protocol_publication_commands p))",&[]).await.unwrap().get(0)
}
async fn refused(f:&Fixture,key:Option<u8>,method:&str,path:&str,payload:Value,status:u16) {
    let before=snapshot(f).await;
    f.call(key,method,path,Some(payload),status).await;
    assert_eq!(snapshot(f).await,before,"refusal mutated catalogue, audit or publication intent");
}
fn projected_cover(response:&Value)->&str {
    response["data"]["relationships"].as_array().unwrap().iter().find(|r|r["type"]=="cover_art").unwrap()["id"].as_str().unwrap()
}
async fn fixture_with_covers()->Fixture {
    let f=fixture().await;
    f.state.db.client().await.unwrap().batch_execute(r#"
        UPDATE platform.users SET permissions='["manga.create","manga.update","manga.cover.edit","manga.tag.edit"]' WHERE id='PublicUploadUser1';
        UPDATE platform.users SET permissions='["manga.update"]' WHERE id='PublicUploadUser2';
        UPDATE platform.users SET permissions='["manga.cover.edit"]' WHERE id='PublicUploadUser4';
        INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES('OtherMng01','PublicUploadUser1','Fixture','Other parent','en','ongoing','safe','published');
        INSERT INTO platform.catalog_covers(id,manga_id,uploader_id,uploader_name,ext,dimensions,volume,language,description,is_main,created_at) VALUES
          ('PrimCover1','UploadMng1','PublicUploadUser1','Fixture','png',ARRAY[13,17],'1','en','Older explicit',TRUE,'2026-01-01'),
          ('PrimCover2','UploadMng1','PublicUploadUser1','Fixture','png',ARRAY[13,17],'2','en','Newest fallback',FALSE,'2026-02-01'),
          ('PrimOther1','OtherMng01','PublicUploadUser1','Fixture','png',ARRAY[13,17],'1','en','Foreign',TRUE,'2026-01-01'),
          ('PrimDelete','UploadMng1','PublicUploadUser1','Fixture','png',ARRAY[13,17],'1','en','Deleted',FALSE,'2026-01-01'),
          ('PrimHidden','UploadMng1','PublicUploadUser1','Fixture','png',ARRAY[13,17],'1','en','Hidden',FALSE,'2026-01-01');
        UPDATE platform.catalog_covers SET deleted_at=clock_timestamp() WHERE id='PrimDelete';
        UPDATE platform.catalog_covers SET protocol_event_id=repeat('f',64) WHERE id='PrimHidden';
    "#).await.unwrap();
    f
}
async fn transactions_keep_guards_and_rollback(f:&Fixture) {
    let mut headers=axum::http::HeaderMap::new();headers.insert("cookie","weebdex_session=synthetic-public-upload-cookie".parse().unwrap());
    let actor=optional_principal(&f.state,&headers).await.unwrap().unwrap();
    let before=snapshot(f).await;
    let mut client=f.state.db.client().await.unwrap();let tx=client.transaction().await.unwrap();
    CatalogRepository::select_primary_cover(&tx,FIXTURE_MANGA_ID,&actor,1,Some("PrimCover2")).await.unwrap();
    let rows=tx.query("SELECT id,is_main,version FROM platform.catalog_covers WHERE id IN('PrimCover1','PrimCover2') ORDER BY id",&[]).await.unwrap();
    assert!(!rows[0].get::<_,bool>(1));assert!(rows[1].get::<_,bool>(1));for row in rows {assert_eq!(row.get::<_,i32>(2),2);}
    CatalogRepository::select_primary_cover(&tx,FIXTURE_MANGA_ID,&actor,1,None).await.unwrap();
    assert_eq!(tx.query_one("SELECT count(*) FROM platform.catalog_covers WHERE manga_id=$1 AND is_main AND deleted_at IS NULL",&[&FIXTURE_MANGA_ID]).await.unwrap().get::<_,i64>(0),0);
    tx.rollback().await.unwrap();assert_eq!(snapshot(f).await,before);
    for (version,cover) in [(2,Some("PrimCover2")),(1,Some("PrimOther1")),(1,Some("PrimDelete")),(1,Some("PrimHidden"))] {
        let tx=client.transaction().await.unwrap();assert!(CatalogRepository::select_primary_cover(&tx,FIXTURE_MANGA_ID,&actor,version,cover).await.is_err());tx.rollback().await.unwrap();assert_eq!(snapshot(f).await,before);
    }
}
async fn refusals_and_creation(f:&Fixture) {
    let account=alias(f,ResourceKind::User,"PublicUploadUser1").await;
    let other=alias(f,ResourceKind::User,"PublicUploadUser2").await;
    let path=format!("/api/v1/manga/{}",alias(f,ResourceKind::Manga,FIXTURE_MANGA_ID).await);
    let cover=alias(f,ResourceKind::Cover,"PrimCover2").await;
    let patch=json!({"version":1,"expectedAccountId":account,"primaryCover":cover});
    refused(f,None,"PUT",&path,patch.clone(),401).await;
    for key in [3,5,6,7] {refused(f,Some(key),"PUT",&path,patch.clone(),403).await;}
    for key in [2,4] {
        let user=alias(f,ResourceKind::User,&format!("PublicUploadUser{key}")).await;
        refused(f,Some(key),"PUT",&path,json!({"version":1,"expectedAccountId":user,"primaryCover":cover}),403).await;
        refused(f,Some(key),"PUT",&path,json!({"version":1,"expectedAccountId":user,"primaryCover":null}),403).await;
    }
    // Force a downstream native title validation error after the guarded cover
    // operation; the router must roll back cover flags, versions and audits too.
    let before=snapshot(f).await;
    let response=f.http.put(format!("{}/api/manga/{FIXTURE_MANGA_ID}",f.origin)).header("origin",&f.origin).header("cookie","weebdex_session=synthetic-public-upload-cookie").json(&json!({"version":1,"primary_cover":"PrimCover2","tags":["MissingTag"]})).send().await.unwrap();
    assert_eq!(response.status().as_u16(),400);assert_eq!(snapshot(f).await,before);
    for value in [json!(cover),Value::Null] {refused(f,Some(1),"PUT",&path,json!({"version":1,"primaryCover":value}),400).await;}
    refused(f,Some(1),"PUT",&path,json!({"version":1,"expectedAccountId":other,"primaryCover":cover}),409).await;
    refused(f,Some(1),"PUT",&path,json!({"version":2,"expectedAccountId":account,"primaryCover":cover}),409).await;
    for value in [json!(42),json!("bad"),json!({}),json!(true)] {refused(f,Some(1),"PUT",&path,json!({"version":1,"expectedAccountId":account,"primaryCover":value}),400).await;}
    for native in ["PrimOther1","PrimDelete","PrimHidden"] {
        let id=alias(f,ResourceKind::Cover,native).await;
        refused(f,Some(1),"PUT",&path,json!({"version":1,"expectedAccountId":account,"primaryCover":id}),404).await;
    }
    refused(f,Some(1),"PUT",&path,json!({"version":1,"expectedAccountId":account,"primaryCover":account}),404).await;
    let db=f.state.db.client().await.unwrap();
    for (set,restore,status) in [("locked=TRUE","locked=FALSE",403),("protocol_source_visible=FALSE","protocol_source_visible=TRUE",404),("deleted_at=clock_timestamp()","deleted_at=NULL",404),("protocol_series_id=repeat('e',64)","protocol_series_id=NULL",404)] {
        db.batch_execute(&format!("UPDATE platform.catalog_manga SET {set} WHERE id='UploadMng1'")).await.unwrap();
        refused(f,Some(1),"PUT",&path,patch.clone(),status).await;
        refused(f,Some(1),"PUT",&path,json!({"version":1,"expectedAccountId":account,"primaryCover":null}),status).await;
        db.batch_execute(&format!("UPDATE platform.catalog_manga SET {restore} WHERE id='UploadMng1'")).await.unwrap();
    }
    db.batch_execute("INSERT INTO platform.protocol_publication_commands(command_id,user_id,input_hash,resource_kind,catalog_id,created_at) VALUES('manga:UploadMng1','PublicUploadUser1',repeat('a',64),'manga','UploadMng1',1)").await.unwrap();
    refused(f,Some(1),"PUT",&path,patch.clone(),409).await;
    db.batch_execute("DELETE FROM platform.protocol_publication_commands WHERE command_id='manga:UploadMng1'").await.unwrap();
    let bytes=serde_json::to_vec(&patch).unwrap();let token=proof(1,11,&format!("{}{path}",f.origin),"PUT",&bytes);
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser2' WHERE publisher=$1",&[&public(1)]).await.unwrap();
    let before=snapshot(f).await;f.raw("PUT",&path,bytes,"application/json",Some(token),409).await;assert_eq!(snapshot(f).await,before);
    db.execute("UPDATE platform.protocol_identity_bindings SET user_id='PublicUploadUser1' WHERE publisher=$1",&[&public(1)]).await.unwrap();
    let mut create=json!({"title":{"en":"No initial main"},"originalLanguage":"en","status":"ongoing","contentRating":"safe","primaryCover":cover,"expectedAccountId":account});
    refused(f,Some(1),"POST","/api/v1/manga",create.clone(),400).await;
    create["primaryCover"]=Value::Null;
    let created=f.call(Some(1),"POST","/api/v1/manga",Some(create.clone()),200).await;
    assert!(!created["data"]["relationships"].as_array().unwrap().iter().any(|r|r["type"]=="cover_art"));
    let native=f.native(ResourceKind::Manga,created["data"]["id"].as_str().unwrap()).await;
    assert_eq!(db.query_one("SELECT count(*) FROM platform.catalog_covers WHERE manga_id=$1",&[&native]).await.unwrap().get::<_,i64>(0),0);
    create.as_object_mut().unwrap().remove("primaryCover");create.as_object_mut().unwrap().remove("expectedAccountId");
    f.call(Some(1),"POST","/api/v1/manga",Some(create),200).await;
    // Native draft authority is retained: owner + manga.create, never a foreign draft.
    db.batch_execute("UPDATE platform.users SET permissions='[\"manga.create\"]' WHERE id IN('PublicUploadUser1','PublicUploadUser2')").await.unwrap();
    let draft=format!("/api/v1/manga/{}",created["data"]["id"].as_str().unwrap());
    refused(f,Some(2),"PUT",&draft,json!({"version":1,"expectedAccountId":other,"primaryCover":null}),403).await;
    f.call(Some(1),"PUT",&draft,Some(json!({"version":1,"expectedAccountId":account,"primaryCover":null})),200).await;
    db.batch_execute("UPDATE platform.users SET permissions='[\"manga.create\",\"manga.update\",\"manga.cover.edit\"]' WHERE id='PublicUploadUser1'").await.unwrap();
}
async fn signed_selection_clear_omission_and_race(f:&Fixture) {
    let account=alias(f,ResourceKind::User,"PublicUploadUser1").await;
    let path=format!("/api/v1/manga/{}",alias(f,ResourceKind::Manga,FIXTURE_MANGA_ID).await);
    let a=alias(f,ResourceKind::Cover,"PrimCover1").await;let b=alias(f,ResourceKind::Cover,"PrimCover2").await;
    let db=f.state.db.client().await.unwrap();
    let omitted=f.call(Some(1),"PUT",&path,Some(json!({"version":1})),200).await;assert_eq!(projected_cover(&omitted),a);
    for (version,value,projected) in [(2,json!(b),&b),(3,json!(a),&a),(4,Value::Null,&b),(5,Value::Null,&b)] {
        let result=f.call(Some(1),"PUT",&path,Some(json!({"version":version,"expectedAccountId":account,"primaryCover":value})),200).await;
        assert_eq!(result["data"]["attributes"]["version"],version+1);assert_eq!(projected_cover(&result),projected);
        let read=f.call(None,"GET",&path,None,200).await;assert_eq!(projected_cover(&read),projected);
    }
    assert_eq!(db.query_one("SELECT count(*) FROM platform.catalog_covers WHERE manga_id=$1 AND is_main AND deleted_at IS NULL",&[&FIXTURE_MANGA_ID]).await.unwrap().get::<_,i64>(0),0);
    let omitted=f.call(Some(1),"PUT",&path,Some(json!({"version":6})),200).await;assert_eq!(projected_cover(&omitted),b);
    let rows=db.query("SELECT id,version FROM platform.catalog_covers WHERE id IN('PrimCover1','PrimCover2') ORDER BY id",&[]).await.unwrap();assert_eq!(rows[0].get::<_,i32>(1),4);assert_eq!(rows[1].get::<_,i32>(1),3);
    let audit=db.query("SELECT operation,old_version,new_version FROM platform.catalog_audit WHERE entity_type='cover' ORDER BY id",&[]).await.unwrap();assert_eq!(audit.len(),5);for row in audit {let old:i32=row.get(1);assert_eq!(row.get::<_,i32>(2),old+1);assert!(["main-select","main-clear"].contains(&row.get::<_,String>(0).as_str()));}
    refused(f,Some(1),"PUT",&path,json!({"version":6,"expectedAccountId":account,"primaryCover":a}),409).await;
    // A cover editor opened before the title selection cannot overwrite the new cover version.
    refused(f,Some(1),"PUT",&format!("/api/v1/cover/{a}"),json!({"version":1,"expectedAccountId":account,"main":true}),409).await;
    f.call(Some(1),"PUT",&path,Some(json!({"version":7,"expectedAccountId":account,"primaryCover":a})),200).await;
    let before_audit:i64=db.query_one("SELECT count(*) FROM platform.catalog_audit WHERE entity_id=$1",&[&FIXTURE_MANGA_ID]).await.unwrap().get(0);
    let send=|value:Value| {
        let body=serde_json::to_vec(&json!({"version":8,"expectedAccountId":account,"primaryCover":value})).unwrap();
        let token=proof(1,11,&format!("{}{path}",f.origin),"PUT",&body);
        f.http.put(format!("{}{path}",f.origin)).header("origin",&f.origin).header("content-type","application/json").header("authorization",token).body(body).send()
    };
    let (one,two)=tokio::join!(send(json!(b)),send(Value::Null));let mut statuses=[one.unwrap().status().as_u16(),two.unwrap().status().as_u16()];statuses.sort();assert_eq!(statuses,[200,409]);
    let row=db.query_one("SELECT version,(SELECT count(*) FROM platform.catalog_audit WHERE entity_id=$1),(SELECT count(*) FROM platform.catalog_covers WHERE manga_id=$1 AND is_main AND deleted_at IS NULL) FROM platform.catalog_manga WHERE id=$1",&[&FIXTURE_MANGA_ID]).await.unwrap();assert_eq!(row.get::<_,i32>(0),9);assert_eq!(row.get::<_,i64>(1),before_audit+1);assert!(row.get::<_,i64>(2)<=1);
}
async fn signed_originals_remain_immutable(f:&Fixture) {
    use std::collections::BTreeMap;
    use weebdex_platform::protocol::{bridge,publication::PublicationInput};
    let runtime=f.state.protocol.publication_runtime().await.unwrap();let bytes=original_fixture::png();
    let event=runtime.publish_operator(PublicationInput{kind:1002,canonical_manifest:original_fixture::cbor(json!({"type":"series","version":1,"titles":{"en":"Primary cover source"},"contentRating":"safe","required":["series-cover-v1"],"cover":original_fixture::page(&bytes)})),originals:BTreeMap::from([(auth::hash(&bytes),bytes.clone())]),created_at:None}).await.unwrap().event;
    let id:[u8;32]=hex::decode(&event.id).unwrap().try_into().unwrap();tokio::time::timeout(Duration::from_secs(20),async {loop {if f.state.protocol.entry(id).await.unwrap().is_some(){break;}tokio::time::sleep(Duration::from_millis(30)).await;}}).await.unwrap();
    for _ in 0..8 {let result=bridge::import_page(&f.state).await.unwrap();assert_eq!(result.rejected,0);if result.completed_sweep && result.deferred==0 && result.scanned>0 {break;}}
    let db=f.state.db.client().await.unwrap();
    let manga:String=db.query_one("SELECT id FROM platform.catalog_manga WHERE protocol_series_id=$1",&[&event.id]).await.unwrap().get(0);
    let cover:String=db.query_one("SELECT id FROM platform.catalog_covers WHERE protocol_event_id=$1",&[&event.id]).await.unwrap().get(0);
    let before:Value=db.query_one("SELECT jsonb_build_object('events',(SELECT jsonb_agg(raw_event ORDER BY id) FROM events),'source',(SELECT metadata - 'instance_overlay' FROM platform.catalog_manga WHERE id=$1),'cover',(SELECT to_jsonb(c)-'is_main'-'version'-'updated_at' FROM platform.catalog_covers c WHERE id=$2),'blob',(SELECT to_jsonb(s) FROM platform.protocol_cover_sources s WHERE cover_id=$2))",&[&manga,&cover]).await.unwrap().get(0);
    let account=alias(f,ResourceKind::User,"PublicUploadUser1").await;let cover_id=alias(f,ResourceKind::Cover,&cover).await;let path=format!("/api/v1/manga/{}",alias(f,ResourceKind::Manga,&manga).await);
    f.call(Some(1),"PUT",&path,Some(json!({"version":1,"expectedAccountId":account,"primaryCover":null})),200).await;
    assert!(!db.query_one("SELECT is_main FROM platform.catalog_covers WHERE id=$1",&[&cover]).await.unwrap().get::<_,bool>(0));
    for _ in 0..8 {let result=bridge::import_page(&f.state).await.unwrap();assert_eq!(result.rejected,0);if result.completed_sweep {break;}}
    assert!(!db.query_one("SELECT is_main FROM platform.catalog_covers WHERE id=$1",&[&cover]).await.unwrap().get::<_,bool>(0));
    f.call(Some(1),"PUT",&path,Some(json!({"version":2,"expectedAccountId":account,"primaryCover":cover_id})),200).await;
    let unchanged:Value=db.query_one("SELECT jsonb_build_object('cover',to_jsonb(c),'audit',(SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM platform.catalog_audit a WHERE a.entity_type='cover' AND a.entity_id=c.id)) FROM platform.catalog_covers c WHERE id=$1",&[&cover]).await.unwrap().get(0);
    f.call(Some(1),"PUT",&path,Some(json!({"version":3,"expectedAccountId":account,"primaryCover":cover_id})),200).await;
    let still:Value=db.query_one("SELECT jsonb_build_object('cover',to_jsonb(c),'audit',(SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM platform.catalog_audit a WHERE a.entity_type='cover' AND a.entity_id=c.id)) FROM platform.catalog_covers c WHERE id=$1",&[&cover]).await.unwrap().get(0);assert_eq!(still,unchanged);
    let after:Value=db.query_one("SELECT jsonb_build_object('events',(SELECT jsonb_agg(raw_event ORDER BY id) FROM events),'source',(SELECT metadata - 'instance_overlay' FROM platform.catalog_manga WHERE id=$1),'cover',(SELECT to_jsonb(c)-'is_main'-'version'-'updated_at' FROM platform.catalog_covers c WHERE id=$2),'blob',(SELECT to_jsonb(s) FROM platform.protocol_cover_sources s WHERE cover_id=$2))",&[&manga,&cover]).await.unwrap().get(0);assert_eq!(after,before);
    let hash:[u8;32]=hex::decode(auth::hash(&bytes)).unwrap().try_into().unwrap();
    let (original,mime)=runtime.download_original(hash).await.unwrap().unwrap();
    let original=axum::body::to_bytes(original.response(None,&mime,false).into_body(),bytes.len()+1).await.unwrap();assert_eq!(original.to_vec(),bytes);
    assert_eq!(serde_json::to_value(f.state.protocol.entry(id).await.unwrap().unwrap().event).unwrap(),serde_json::to_value(&event).unwrap());
    event.verify().unwrap();
}
async fn stop_native(f:&mut Fixture) {
    let mut native=f.native.take().unwrap();if let Some(shutdown)=native.shutdown.take(){let _=shutdown.send(());}tokio::time::timeout(Duration::from_secs(30),native.task).await.unwrap().unwrap().unwrap();
}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires owned isolated PostgreSQL and pinned strfry"]
async fn primary_cover_transaction_http_authority_and_originals() {
    let mut f=fixture_with_covers().await;
    transactions_keep_guards_and_rollback(&f).await;
    refusals_and_creation(&f).await;
    signed_selection_clear_omission_and_race(&f).await;
    signed_originals_remain_immutable(&f).await;
    stop_native(&mut f).await;
    println!("PRIMARY_COVER_VERIFIED=selection, clear, omission, fallback, atomic title CAS race, cover versions and audit, account and permission fences, parent and cover visibility, creation safety, transaction rollback, immutable signed originals");
}
// This unchanged HTTP probe also fails on the original source specifically at
// the rejected primaryCover field, not at setup or a new core symbol.
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires owned isolated PostgreSQL and pinned strfry"]
async fn primary_cover_http_selection() {
    let mut f=fixture_with_covers().await;let account=alias(&f,ResourceKind::User,"PublicUploadUser1").await;let cover=alias(&f,ResourceKind::Cover,"PrimCover2").await;
    let path=format!("/api/v1/manga/{}",alias(&f,ResourceKind::Manga,FIXTURE_MANGA_ID).await);
    let response=f.call(Some(1),"PUT",&path,Some(json!({"version":1,"expectedAccountId":account,"primaryCover":cover})),200).await;assert_eq!(projected_cover(&response),cover);
    stop_native(&mut f).await;
}

#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires owned isolated PostgreSQL and pinned strfry"]
async fn primary_cover_http_clear() {
    let mut f=fixture_with_covers().await;let account=alias(&f,ResourceKind::User,"PublicUploadUser1").await;let fallback=alias(&f,ResourceKind::Cover,"PrimCover2").await;
    let path=format!("/api/v1/manga/{}",alias(&f,ResourceKind::Manga,FIXTURE_MANGA_ID).await);
    let response=f.call(Some(1),"PUT",&path,Some(json!({"version":1,"expectedAccountId":account,"primaryCover":null})),200).await;assert_eq!(projected_cover(&response),fallback);
    assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.catalog_covers WHERE manga_id=$1 AND is_main AND deleted_at IS NULL",&[&FIXTURE_MANGA_ID]).await.unwrap().get::<_,i64>(0),0);
    stop_native(&mut f).await;
}
