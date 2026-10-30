//! Direct cached-native-principal controls. Real database, no HTTP auth shortcut.
use crate::{AppState,Config,domains::{self,auth::SessionPrincipal,catalog::{entity_mutations as entities,mutations as titles},community::mutations as discussion,uploads::sessions as upload,library::{profile_mutations,preference_mutations},moderation}};
use serde_json::json;
use std::time::Duration;
#[tokio::test]
#[ignore="Owned isolated fixture and coordinated heavy slot required"]
async fn stale_principal_core_families_refuse_after_disable() {
 let dsn=std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use owned isolated fixture runner");let pg:tokio_postgres::Config=dsn.parse().unwrap();assert_eq!(pg.get_dbname(),Some("weebdex_platform_test"));assert!(pg.get_hosts().iter().all(|h|matches!(h,tokio_postgres::config::Host::Tcp(h) if h=="127.0.0.1")));
 let runtime=std::path::PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap());assert!(runtime.to_str().unwrap().contains("/weebdex-platform/test-runtime/auth-"));let receipt:serde_json::Value=serde_json::from_slice(&std::fs::read(runtime.join("fixture.json")).unwrap()).unwrap();assert_eq!(receipt["fixture_only"],true);assert_eq!(receipt["database"],"weebdex_platform_test");
 let original=manga_node::db::Repository::connect(manga_node::db::DbConfig{dsn:dsn.clone(),connections:1,acquisition_timeout:Duration::from_secs(2),cache_weight:1048576,plaintext_local:true}).await.unwrap();original.migrate().await.unwrap();
 let state=AppState::connect(Config{database_plaintext_local:true,database_connections:2,uploads_dir:runtime.join("closure-core-media"),..Config::default()},dsn).await.unwrap();state.db.migrate(domains::MIGRATIONS).await.unwrap();let mut db=state.db.client().await.unwrap();
 let permissions=vec!["author.create","scanlation_group.create","manga.create","self.chapter.update","chapter.create","reply.create","report.create","self.user.update","self.preferences.update"].into_iter().map(str::to_owned).collect::<Vec<_>>();
 db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,disabled,closed_at,permissions) VALUES('ClosedCore1','Closed core fixture','closed-core@fixture.invalid','synthetic-unused',TRUE,TRUE,clock_timestamp(),$1)",&[&json!(permissions)]).await.unwrap();
 let actor=SessionPrincipal{user_id:"ClosedCore1".into(),username:"Closed core fixture".into(),email:"closed-core@fixture.invalid".into(),verified:true,roles:vec!["user".into()],permissions,session_id:"cached-fixture-session".into(),is_api_client:false,authenticated_age_seconds:0,recovery_session:false};
 macro_rules! refused {($tx:ident,$operation:expr)=>{{let $tx=db.transaction().await.unwrap();let failure=$operation.await.unwrap_err();assert_eq!(failure.status.as_u16(),403,"{}",stringify!($operation));assert!(["This account is no longer active","This account cannot edit that profile","This account cannot change preferences"].contains(&failure.message.as_str()),"wrong refusal seam: {}",failure.message);$tx.rollback().await.unwrap();}};}
 refused!(tx,entities::author_create(&tx,&actor,entities::AuthorCreate::parse(&json!({"name":"No late author"})).unwrap()));
 refused!(tx,entities::group_create(&tx,&actor,entities::GroupCreate::parse(&json!({"name":"No late group"})).unwrap()));
 refused!(tx,titles::manga_create_in(&tx,&actor,&json!({}),&json!({})));
 refused!(tx,titles::manga_update_in(&tx,&actor,"UnusedManga",&json!({"version":1}),&json!({})));
 refused!(tx,titles::manga_transition_in(&state,&tx,&actor,"UnusedManga",&json!({"version":1}),"published",true));
 refused!(tx,discussion::create_in(&tx,&actor,"UnusedThread","No late reply",None));
 refused!(tx,discussion::update_in(&tx,&actor,"UnusedThread",1,"No late edit",1));
 refused!(tx,discussion::remove_in(&tx,&actor,"UnusedThread",1,Some(1)));
 refused!(tx,discussion::delete_in(&tx,&actor,"UnusedThread",1,Some(1)));
 refused!(tx,discussion::thread_vote_in(&tx,&actor,"UnusedThread",1,None));
 refused!(tx,discussion::reply_vote_in(&tx,&actor,"UnusedThread",1,1,None,Some(1)));
 refused!(tx,upload::begin_in(&tx,&actor,&upload::Begin{manga:"UnusedManga".into(),groups:vec![]}));
 refused!(tx,upload::begin_edit_in(&tx,&actor,"UnusedChapter",&upload::Edit{replace:true,version:1}));
 refused!(tx,upload::cancel_in(&tx,&actor,"UnusedUpload"));
 refused!(tx,upload::remove_files_in(&tx,&actor,"UnusedUpload",&["UnusedFile".into()]));
 refused!(tx,moderation::create_report(&tx,&actor,"manga","UnusedManga","UnusedReason","No late report"));
 let patch=serde_json::from_value::<crate::contracts::library::ProfilePatch>(json!({"version":1,"description":"No late profile"})).unwrap();
 refused!(tx,profile_mutations::update_in(&tx,&actor,"ClosedCore1",&patch));
 refused!(tx,preference_mutations::update_in(&tx,&actor,1,json!({}),preference_mutations::Mode::PublicPreserving,None));
 refused!(tx,crate::protocol::publication::publish_native_manga(&state,&tx,&actor,"UnusedManga",1));
 let mut record=crate::domains::catalog::ChapterPublication{id:"UnusedChapter".into(),manga_id:"UnusedManga".into(),uploader_id:actor.user_id.clone(),uploader_name:actor.username.clone(),title:None,volume:None,chapter:None,language:"en".into(),node:"fixture".into(),pages:vec![],optimized_pages:vec![],groups:vec![],source_id:None,protocol_event_id:None};
 refused!(tx,crate::protocol::publication::publish_native_upload(&state,&tx,&actor,"UnusedUpload","unused-hash",&mut record,&[],&json!({})));
 let input=crate::protocol::publication::PublicationInput{kind:0,canonical_manifest:vec![],originals:std::collections::BTreeMap::new(),created_at:None};
 refused!(tx,state.protocol.publish_for_native_user(&state,&tx,&actor,None,None,input));
 let failure=crate::protocol::deletions::request_native_chapter_deletion(&state,&actor,"UnusedChapter").await.unwrap_err();assert_eq!(failure.status.as_u16(),403);assert_eq!(failure.message,"This account is no longer active");
 assert_eq!(db.query_one("SELECT count(*) FROM platform.catalog_audit WHERE actor_id='ClosedCore1'",&[]).await.unwrap().get::<_,i64>(0),0);
 assert_eq!(db.query_one("SELECT count(*) FROM platform.upload_sessions WHERE owner_id='ClosedCore1'",&[]).await.unwrap().get::<_,i64>(0),0);
 assert_eq!(db.query_one("SELECT count(*) FROM platform.community_audit WHERE actor_id='ClosedCore1'",&[]).await.unwrap().get::<_,i64>(0),0);
}
