//! Real original signed lists, actual HTTP and isolated PostgreSQL membership.
#[path="support/nostr_library.rs"]mod fixture;
use fixture::*;
use anyhow::{Result,Context,ensure};
use manga_node::{auth,library_manifest::Record};
use serde_json::{Value,json};
use weebdex_platform::{protocol::list_aliases,public_api::{PostgresPublicIdResolver,PublicIdResolver,ResourceKind}};

fn list(n:u8,name:&str)->Record {Record::List{id:[n;16],name:name.into(),updated_at:1}}
fn request_proof(key:u8,url:&str,method:&str,body:&[u8])->String {
    token(&sign_at(key,27235,vec![vec!["u".into(),url.into()],vec!["method".into(),method.into()],vec!["service".into(),public(11)],vec!["payload".into(),auth::hash(body)],vec!["nonce".into(),hex::encode(rand::random::<[u8;16]>())]],String::new(),auth::now()))
}
async fn call(f:&Fixture,key:Option<u8>,method:&str,path:&str,payload:Option<Value>,status:u16)->Result<Value> {
    let url=format!("{}/api/v1{path}",f.origin);let body=payload.map_or_else(Vec::new,|p|serde_json::to_vec(&p).unwrap());
    let mut request=f.c.request(method.parse()?,&url).header("content-type","application/json");
    if let Some(key)=key {request=request.header("authorization",request_proof(key,&url,method,&body));}
    response(request.body(body),status).await
}
async fn project(f:&Fixture)->Result<()> {
    for _ in 0..32 {if list_aliases::import_page(&f.state).await.map_err(|e|anyhow::anyhow!("{e:?}"))?.completed_sweep{return Ok(());}}
    anyhow::bail!("bounded list projection did not complete")
}
async fn graph(f:&Fixture,key:u8,time:u64,records:Vec<Record>)->Result<manga_node::library_client_bundle::SaveBundle> {
    let b=bundle(key,time,records)?;save(&f.c,&f.native_origin,key,&b).await?;drain_backend(&f.repo).await?;Ok(b)
}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires owned PG runner and verified actual strfry"]
async fn signed_public_list_follows_scope_accounts_and_track_real_visibility()->Result<()> {
    let f=Fixture::start().await?;let result=exercise(&f).await;f.shutdown().await?;result
}
async fn exercise(f:&Fixture)->Result<()> {
    let now=auth::now()-30;
    let a=graph(f,2,now,vec![list(1,"First public"),list(2,"Second public")]).await?;
    let b=graph(f,3,now,vec![list(3,"Other publisher")]).await?;
    project(f).await?;
    let db=f.state.db.client().await.map_err(|e|anyhow::anyhow!("{e:?}"))?;
    for key in [2,3] {
        let user=format!("FollowUser{key}");
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,permissions) VALUES($1,$1,$2,'synthetic-no-password',true,'[\"self.preferences.update\"]')",&[&user,&format!("follow-{key}@fixture.invalid")]).await?;
        db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await?;
    }
    db.execute("INSERT INTO platform.library_lists(id,user_id,name,visibility) VALUES('PrivateList1','FollowUser2','Private native list','private')",&[]).await?;
    let resolver=PostgresPublicIdResolver::new(f.state.db.clone());
    let key1=format!("protocol-list:{}:{}",public(2),hex::encode([1;16]));
    let key2=format!("protocol-list:{}:{}",public(2),hex::encode([2;16]));
    let key3=format!("protocol-list:{}:{}",public(3),hex::encode([3;16]));
    let mut ids=Vec::new();
    for key in [&key1,&key2,&key3] {ids.push(resolver.preferred(ResourceKind::List,key).await.map_err(|e|anyhow::anyhow!("{e:?}"))?.context("real list alias")?);}
    let owner=resolver.preferred(ResourceKind::User,"FollowUser2").await.map_err(|e|anyhow::anyhow!("{e:?}"))?.unwrap();
    let other=resolver.preferred(ResourceKind::User,"FollowUser3").await.map_err(|e|anyhow::anyhow!("{e:?}"))?.unwrap();
    let private=resolver.preferred(ResourceKind::List,"PrivateList1").await.map_err(|e|anyhow::anyhow!("{e:?}"))?.unwrap();
    let alias="aaaaaaaa-bbbb-8ccc-8ddd-eeeeeeeeeeee";
    db.query_one("SELECT platform.public_register_id('list',$1,$2::text::uuid,'legacy','protocol_list')",&[&key1,&alias]).await?;
    let aliases_before:i64=db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await?.get(0);drop(db);
    let follows="/user/follows/list";let first=format!("/list/{}/follow",ids[0]);let first_status=format!("{follows}/{}",ids[0]);
    call(f,None,"GET",follows,None,401).await?;call(f,Some(4),"GET",follows,None,403).await?;
    call(f,Some(2),"GET",&first_status,None,404).await?;
    assert_eq!(call(f,Some(2),"GET",follows,None,200).await?["total"],0);
    call(f,Some(2),"POST",&format!("/list/{private}/follow"),None,404).await?;
    call(f,Some(2),"GET",&format!("{follows}/{private}"),None,404).await?;
    call(f,Some(2),"POST",&format!("/list/{owner}/follow"),None,404).await?;
    call(f,Some(2),"POST",&first,Some(json!({"version":0,"expectedAccountId":other})),409).await?;
    let created=call(f,Some(2),"POST",&format!("/list/{alias}/follow"),Some(json!({"version":0,"expectedAccountId":owner,"expectedHead":a.event().id})),200).await?;
    assert_eq!(created["nostr"]["version"],1);assert_eq!(created["nostr"]["listId"],ids[0].as_str());assert_eq!(created["nostr"]["accountId"],owner.as_str());
    let status=call(f,Some(2),"GET",&first_status,None,200).await?;assert_eq!(status["nostr"]["version"],1);
    let duplicate=call(f,Some(2),"POST",&first,None,200).await?;assert_eq!(duplicate["nostr"]["changed"],false);assert_eq!(duplicate["nostr"]["version"],1);
    assert_eq!(call(f,Some(2),"GET",&first_status,None,200).await?,status,"idempotent follow retains real clocks");
    call(f,Some(2),"POST",&first,Some(json!({"version":0})),409).await?;
    call(f,Some(3),"GET",&first_status,None,404).await?;assert_eq!(call(f,Some(3),"GET",follows,None,200).await?["total"],0);
    call(f,Some(2),"POST",&format!("/list/{}/follow",ids[1]),None,200).await?;
    call(f,Some(2),"POST",&format!("/list/{}/follow",ids[2]),None,200).await?;
    let all=call(f,Some(2),"GET",follows,None,200).await?;assert_eq!(all["total"],3);assert_eq!(all["nostr"]["accountId"],owner.as_str());
    assert_eq!(all["data"][0]["id"],ids[2].as_str());assert_ne!(all["data"][2]["relationships"][0]["id"],owner.as_str(),"native follower UUID is not a protocol publisher UUID");
    let page=call(f,Some(2),"GET",&format!("{follows}?limit=1&offset=1"),None,200).await?;assert_eq!(page["total"],3);assert_eq!(page["data"][0]["id"],ids[1].as_str());
    for raw in ["limit=0","limit=101","includes[]=user","head=bad"] {call(f,Some(2),"GET",&format!("{follows}?{raw}"),None,400).await?;}
    call(f,Some(2),"POST",&first,Some(json!({"private":true})),400).await?;
    call(f,Some(2),"DELETE",&first,Some(json!({"expectedHead":a.event().id})),400).await?;
    let url=format!("{}/api/v1{first}",f.origin);let authorization=request_proof(2,&url,"POST",b"");
    response(f.c.post(&url).header("authorization",&authorization),200).await?;response(f.c.post(&url).header("authorization",&authorization),401).await?;
    // Names change through a new genuine signed graph, never local follow state.
    let renamed=graph(f,2,now+1,vec![list(1,"Renamed public"),list(2,"Second public")]).await?;
    call(f,Some(2),"GET",follows,None,503).await?;project(f).await?;
    let current=call(f,Some(2),"GET",follows,None,200).await?;
    assert_eq!(current["data"].as_array().unwrap().iter().find(|v|v["id"]==ids[0].as_str()).unwrap()["attributes"]["name"],"Renamed public");
    call(f,Some(2),"POST",&first,Some(json!({"expectedHead":a.event().id})),409).await?;
    call(f,Some(2),"POST",&first,Some(json!({"expectedHead":renamed.event().id})),200).await?;
    graph(f,2,now+2,vec![list(2,"Second public")]).await?;project(f).await?;
    call(f,Some(2),"GET",&first_status,None,404).await?;
    let withdrawn=call(f,Some(2),"GET",follows,None,200).await?;assert_eq!(withdrawn["total"],2);assert_eq!(withdrawn["nostr"]["unavailableFollowCount"],1);
    // Source withdrawal hides metadata but retains private membership, and an
    // explicit unfollow can still remove it without requiring a visible source.
    let removed=call(f,Some(2),"DELETE",&first,Some(json!({"version":1,"expectedAccountId":owner})),200).await?;assert_eq!(removed["nostr"]["version"],2);
    assert_eq!(call(f,Some(2),"DELETE",&first,None,200).await?["nostr"]["changed"],false);
    graph(f,2,now+3,vec![list(1,"Returned public"),list(2,"Second public")]).await?;project(f).await?;
    call(f,Some(2),"POST",&first,Some(json!({"version":0})),409).await?;
    call(f,Some(2),"POST",&first,Some(json!({"version":2})),200).await?;
    // Original signed deletion is authoritative even while alias rows remain.
    let deletion=auth::sign(&format!("{:064x}",3),5,vec![vec!["e".into(),b.event().id.clone()]],"fixture list deletion".into()).unwrap();
    let permit=f.repo.account_permit(id(&public(3))).await?;assert_eq!(f.repo.delete_ordinary(&permit,&deletion).await?,1);
    call(f,Some(2),"GET",&format!("{follows}/{}",ids[2]),None,404).await?;
    let deleted=call(f,Some(2),"GET",follows,None,200).await?;assert_eq!(deleted["total"],2);assert_eq!(deleted["nostr"]["unavailableFollowCount"],1);
    ensure!(!deleted.to_string().contains("Other publisher"),"deleted source metadata leaked");
    call(f,Some(2),"DELETE",&format!("/list/{}/follow",ids[2]),None,200).await?;
    let count:i64=f.state.db.client().await.map_err(|e|anyhow::anyhow!("{e:?}"))?.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await?.get(0);assert_eq!(count,aliases_before,"follow/read operations never manufacture source aliases");
    f.repo.set_account_admission(id(&public(2)),false).await?;
    call(f,Some(2),"GET",follows,None,403).await?;call(f,Some(2),"DELETE",&first,None,403).await?;
    f.repo.set_account_admission(id(&public(2)),true).await?;
    f.state.db.client().await.map_err(|e|anyhow::anyhow!("{e:?}"))?.execute("UPDATE platform.users SET permissions='[]' WHERE id='FollowUser2'",&[]).await?;
    call(f,Some(2),"DELETE",&first,None,403).await?;
    Ok(())
}
