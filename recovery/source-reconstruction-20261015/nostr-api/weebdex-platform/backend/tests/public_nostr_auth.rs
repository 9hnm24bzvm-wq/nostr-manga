//! RECONSTRUCTED UNVERIFIED original-admission HTTP principal tests. Native
//! explicit-bound-account/draft/write tests must be restored by foundation too.
#[path="support/nostr_library.rs"]mod fixture;
use anyhow::Result;use fixture::*;use manga_node::auth;use serde_json::json;
#[tokio::test(flavor="multi_thread",worker_threads=4)]#[ignore="Requires owned PostgreSQL runner and actual strfry"]async fn public_http_proofs_bind_exact_service_origin_method_body_time_replay_and_principal()->Result<()>{let f=Fixture::start().await?;let result=exercise(&f).await;f.shutdown().await?;result}
async fn exercise(f:&Fixture)->Result<()>{
 let url=format!("{}/api/v1/auth/check",f.origin);get(&f.c,&url,None,401).await?;
 let first=get(&f.c,&url,Some(2),200).await?;assert_eq!(first["isAuthenticated"],true);assert_eq!(first["roles"],json!([]));assert_eq!(first["permissions"],json!([]));assert_eq!(first["nostr"]["pubkey"],public(2));assert_eq!(first["nostr"]["serviceKey"],public(11));assert_eq!(first["nostr"]["nativeSessionAuthority"],false);
 assert_eq!(get(&f.c,&url,Some(3),200).await?["nostr"]["pubkey"],public(3));get(&f.c,&url,Some(5),403).await?;
 let replay=proof(2,&url);response(f.c.get(&url).header("authorization",replay.clone()),200).await?;response(f.c.get(&url).header("authorization",replay),401).await?;
 for event in[request_event(2,&url,auth::now()-120,11),request_event(2,&url,auth::now()+120,11),request_event(2,&url,auth::now(),12),request_event(2,&format!("{url}?extra=1"),auth::now(),11),request_event(2,"https://other-instance.invalid/api/v1/auth/check",auth::now(),11)]{response(f.c.get(&url).header("authorization",token(&event)),401).await?;}
 let template=request_event(2,&url,auth::now(),11);let tags=template.tags.into_iter().filter(|t|t.first().map(String::as_str)!=Some("service")).collect();let missing=sign_at(2,27235,tags,String::new(),auth::now());response(f.c.get(&url).header("authorization",token(&missing)),401).await?;
 let mut tags=request_event(2,&url,auth::now(),11).tags;tags.push(vec!["service".into(),public(11)]);let duplicate=sign_at(2,27235,tags,String::new(),auth::now());response(f.c.get(&url).header("authorization",token(&duplicate)),401).await?;
 let mut tags=request_event(2,&url,auth::now(),11).tags;tags.iter_mut().find(|t|t[0]=="method").unwrap()[1]="POST".into();let wrong_method=sign_at(2,27235,tags,String::new(),auth::now());response(f.c.get(&url).header("authorization",token(&wrong_method)),401).await?;
 response(f.c.get(&url).header("authorization",proof(2,&url)).body("changed body"),401).await?;
 let mut bad=request_event(2,&url,auth::now(),11);bad.sig="00".repeat(64);response(f.c.get(&url).header("authorization",token(&bad)),401).await?;
 response(f.c.get(&url).header("authorization","Nostr invalid-base64"),401).await?;
 response(f.c.get(&url).header("cookie","native_session=synthetic-not-a-nostr-proof"),401).await?;
 let duplicate_token=proof(2,&url);response(f.c.get(&url).header("authorization",duplicate_token.clone()).header("authorization",duplicate_token),401).await?;
 let attacker=token(&request_event(2,"https://attacker.invalid/api/v1/auth/check",auth::now(),11));response(f.c.get(&url).header("authorization",attacker).header("host","attacker.invalid").header("x-forwarded-host","attacker.invalid").header("x-forwarded-proto","https"),401).await?;
 let db=f.repo.pool.get().await?;db.execute("UPDATE node_identity SET service_key=$1 WHERE singleton",&[&id(&public(12)).as_slice()]).await?;drop(db);get(&f.c,&url,Some(2),503).await?;f.repo.pool.get().await?.execute("UPDATE node_identity SET service_key=$1 WHERE singleton",&[&id(&public(11)).as_slice()]).await?;get(&f.c,&url,Some(2),200).await?;
 get(&f.c,&format!("{url}?extra=1"),Some(2),400).await?;Ok(())
}
