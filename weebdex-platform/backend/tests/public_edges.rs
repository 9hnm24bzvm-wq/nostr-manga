//! Fresh real TCP/PostgreSQL edge identity, ordering and visibility regression.
use serde_json::{Value,json};
use weebdex_platform::{AppState,Config,domains,public_api::{PostgresPublicIdResolver,PublicIdResolver,ResourceKind}};
async fn get(c:&reqwest::Client,base:&str,path:&str,status:u16)->Value {
 let r=c.get(format!("{base}/api/v1{path}")).send().await.unwrap();let actual=r.status().as_u16();let value=r.json::<Value>().await.unwrap();assert_eq!(actual,status,"{path}: {value}");value
}
#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Requires explicitly owned isolated PostgreSQL"]
async fn immutable_edge_resources_have_real_order_includes_and_both_endpoint_visibility(){
 let dsn=std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use isolated runner");
 let pg:tokio_postgres::Config=dsn.parse().unwrap();assert_eq!(pg.get_dbname(),Some("weebdex_platform_test"));
 assert!(!pg.get_hosts().is_empty()&&pg.get_hosts().iter().all(|h|matches!(h,tokio_postgres::config::Host::Tcp(h)if h=="127.0.0.1")));
 assert!(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap().contains("/weebdex-platform/test-runtime/"));
 let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let base=format!("http://{}",listener.local_addr().unwrap());
 let state=AppState::connect(Config{database_plaintext_local:true,database_connections:4,public_origin:base.clone(),..Config::default()},dsn).await.unwrap();state.db.migrate(domains::MIGRATIONS).await.unwrap();
 let db=state.db.client().await.unwrap();db.batch_execute("INSERT INTO platform.users(id,username,email,password_hash) VALUES('EdgeUser01','edge-fixture','edge@fixture.invalid','synthetic-no-password'); INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) SELECT 'EdgeM'||lpad(i::text,5,'0'),'EdgeUser01','Fixture','Edge target '||i,'en','ongoing',CASE WHEN i%4=0 AND i>0 THEN 'pornographic' ELSE 'safe' END,'published' FROM generate_series(0,24)i; INSERT INTO platform.catalog_relations(manga_id,related_id,relation_type,created_by) VALUES('EdgeM00000','EdgeM00001','sequel','EdgeUser01'),('EdgeM00000','EdgeM00002','prequel','EdgeUser01'); INSERT INTO platform.catalog_recommendations(manga_id,recommended_id,score) SELECT 'EdgeM00000','EdgeM'||lpad(i::text,5,'0'),i FROM generate_series(1,24)i;").await.unwrap();
 let resolver=PostgresPublicIdResolver::new(state.db.clone());let root=resolver.preferred(ResourceKind::Manga,"EdgeM00000").await.unwrap().unwrap();let one=resolver.preferred(ResourceKind::Manga,"EdgeM00001").await.unwrap().unwrap();let two=resolver.preferred(ResourceKind::Manga,"EdgeM00002").await.unwrap().unwrap();
 let app=weebdex_platform::router::router(state.clone());let server=tokio::spawn(async move{axum::serve(listener,app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await.unwrap()});
 let c=reqwest::Client::builder().no_proxy().timeout(std::time::Duration::from_secs(10)).build().unwrap();
 let before:i64=db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);
 let relations=get(&c,&base,&format!("/manga/{root}/relation?includes[]=manga"),200).await;
 assert_eq!(relations["total"],2);assert_eq!(relations["limit"],2);assert_eq!(relations["data"][0]["type"],"manga_relation");assert_eq!(relations["data"][0]["attributes"]["relation"],"prequel");assert!(relations["data"][0]["attributes"].get("version").is_none());
 assert_eq!(relations["data"][0]["relationships"][0]["id"],two.as_str());assert_eq!(relations["data"][0]["relationships"][0]["attributes"]["title"]["en"],"Edge target 2");assert!(relations["data"][0]["relationships"][0].get("related").is_none());
 let relation_id=relations["data"][1]["id"].clone();assert_ne!(relation_id,one.as_str());
 let unexpanded=get(&c,&base,&format!("/manga/{root}/relation"),200).await;assert!(unexpanded["data"][0]["relationships"][0].get("attributes").is_none());
 let rec=get(&c,&base,&format!("/manga/{root}/recommendation?includes[]=manga"),200).await;assert_eq!(rec["total"],18);assert_eq!(rec["limit"],20);assert_eq!(rec["data"][0]["attributes"]["score"],23);assert_eq!(rec["data"][0]["type"],"manga_recommendation");
 let all=get(&c,&base,&format!("/manga/{root}/recommendation?contentRating[]=safe&contentRating[]=pornographic"),200).await;assert_eq!(all["total"],24);assert_eq!(all["data"].as_array().unwrap().len(),20);assert_eq!(all["data"][0]["attributes"]["score"],24);
 let asc=get(&c,&base,&format!("/manga/{root}/recommendation?order[score]=asc"),200).await;assert_eq!(asc["data"][0]["attributes"]["score"],1);assert_eq!(asc["data"][0]["relationships"][0]["id"],one.as_str());assert_ne!(asc["data"][0]["id"],relation_id);
 assert_eq!(before,db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get::<_,i64>(0));
 for q in ["limit=1","order[title]=asc","includes[]=author","contentRating[]=invalid"]{get(&c,&base,&format!("/manga/{root}/recommendation?{q}"),400).await;}
 get(&c,&base,&format!("/manga/{one}/relation"),200).await;
 db.execute("UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='EdgeM00001'",&[]).await.unwrap();let hidden=get(&c,&base,&format!("/manga/{root}/relation"),200).await;assert_eq!(hidden["total"],1);
 let rec_hidden=get(&c,&base,&format!("/manga/{root}/recommendation?order[score]=asc"),200).await;assert_eq!(rec_hidden["total"],17);assert_eq!(rec_hidden["data"][0]["attributes"]["score"],2);
 db.execute("UPDATE platform.catalog_manga SET protocol_source_visible=true WHERE id='EdgeM00001'",&[]).await.unwrap();
 db.batch_execute("DELETE FROM platform.catalog_relations WHERE manga_id='EdgeM00000' AND related_id='EdgeM00001'; INSERT INTO platform.catalog_relations(manga_id,related_id,relation_type,created_by) VALUES('EdgeM00000','EdgeM00001','sequel','EdgeUser01');").await.unwrap();assert_eq!(get(&c,&base,&format!("/manga/{root}/relation"),200).await["data"][1]["id"],relation_id);
 db.execute("UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='EdgeM00000'",&[]).await.unwrap();for suffix in ["relation","recommendation"]{get(&c,&base,&format!("/manga/{root}/{suffix}"),404).await;}
 db.execute("UPDATE platform.catalog_manga SET protocol_source_visible=true WHERE id='EdgeM00000'",&[]).await.unwrap();get(&c,&base,&format!("/manga/{root}/relation"),200).await;
 server.abort();let _=server.await;
}
