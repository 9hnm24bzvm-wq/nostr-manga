//! Actual loopback HTTP / real router / disposable PostgreSQL, synthetic principals only.
use manga_node::db::{DbConfig,Repository};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use std::time::Duration;
use weebdex_platform::{AppState,Config,domains,router};

struct Server(tokio::task::JoinHandle<()>);impl Drop for Server{fn drop(&mut self){self.0.abort();}}
struct Fixture{state:AppState,http:reqwest::Client,origin:String,_server:Server}
impl Fixture{
 fn request(&self,user:&str,method:&str,path:&str)->reqwest::RequestBuilder{self.http.request(method.parse().unwrap(),format!("{}{path}",self.origin)).header("origin",&self.origin).header("content-type","application/json").header("cookie",format!("weebdex_session=synthetic-staff-{user}")).header("x-weebdex-expected-account",user)}
 async fn send(&self,user:&str,method:&str,path:&str,body:Option<Value>)->(u16,Value){let request=self.request(user,method,path);let response=if let Some(body)=body{request.json(&body)}else{request}.send().await.unwrap();let status=response.status().as_u16();assert!(response.headers()["cache-control"].to_str().unwrap().contains("no-store"));let bytes=response.bytes().await.unwrap();let data:Value=serde_json::from_slice(&bytes).unwrap_or_else(|error|panic!("{method} {path}: HTTP {status} expected JSON, {error}; body: {}",String::from_utf8_lossy(&bytes).chars().take(500).collect::<String>()));let text=data.to_string();for private in ["synthetic-staff-","@example.invalid","password_hash","token_hash","operator_key_file","source_manifest"]{assert!(!text.contains(private),"private value leaked: {private}");}(status,data)}
 async fn call(&self,user:&str,method:&str,path:&str,body:Option<Value>,expected:u16)->Value{let(status,data)=self.send(user,method,path,body).await;assert_eq!(status,expected,"{method} {path}: {data}");data}
 async fn review(&self,user:&str,id:&str)->Value{self.call(user,"GET",&format!("/api/staff/reports/{id}/review"),None,200).await["context"].clone()}
 fn decision(&self,context:&Value,action:&str,key:&str)->Value{json!({"requestId":key,"action":action,"expectedReportVersion":context["report"]["version"],"expectedContextHash":context["contextHash"],"expectedEffect":if action=="accept"{context["acceptEffect"].clone()}else if action=="delete"{context["effect"].clone()}else{json!({})},"notes":"Synthetic reviewed decision"})}
}
async fn fixture()->Fixture{
 let dsn=std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Only isolated run-auth-tests.sh is accepted");let pg:tokio_postgres::Config=dsn.parse().unwrap();assert_eq!(pg.get_dbname(),Some("weebdex_platform_test"));assert!(pg.get_hosts().iter().all(|h|matches!(h,tokio_postgres::config::Host::Tcp(h) if h=="127.0.0.1")));assert!(pg.get_ports().iter().all(|p|*p>1024&&*p!=5432));
 let runtime=std::path::PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap());assert!(runtime.to_string_lossy().contains("/weebdex-platform/test-runtime/"));assert!(runtime.join("fixture.json").is_file());
 let repository=Repository::connect(DbConfig{dsn:dsn.clone(),connections:2,acquisition_timeout:Duration::from_secs(2),cache_weight:1048576,plaintext_local:true}).await.unwrap();repository.migrate().await.unwrap();
 let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let origin=format!("http://{}",listener.local_addr().unwrap());
 let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:10,ui_dir:runtime.join("no-ui"),uploads_dir:runtime.join("media"),instance:domains::staff_workspace::InstanceInformation{operator_name:"Synthetic operator".into(),contact_url:Some("mailto:support@example.invalid".into()),terms:"Synthetic terms".into(),privacy:"Synthetic privacy".into(),guidelines:"Synthetic guidelines".into(),..Default::default()},..Default::default()},dsn).await.unwrap();state.db.migrate(domains::MIGRATIONS).await.unwrap();state.protocol.verify_schema().await.unwrap();state.protocol.install_visibility_function().await.unwrap();
 let db=state.db.client().await.unwrap();
 let normal=json!(["report.create","self.user.update"]);
 let moderator=json!(["report.create","report.update","report.delete","report.history","report.assign","scanlation_group.update","manga.delete","chapter.remove"]);
 let staff=json!(["report.create","report.update","report.delete","report.history","report.correct","manga.restore","chapter.restore","group.leadership.revoke","user.update","operations.read"]);
 for(user,role,permissions)in[("Reporter01","user",normal.clone()),("Foreign001","user",normal.clone()),("ModOne0001","moderator",moderator.clone()),("ModTwo0001","moderator",moderator.clone()),("StaffOne01","staff",staff.clone()),("StaffTwo01","staff",staff),("RoleOnly01","moderator",json!([])),("GrantOnly1","user",moderator),("ForeignMod","moderator",json!(["user.update"]))]{
  db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,roles,permissions,profile) VALUES($1,$1,$2,'synthetic-not-a-password',TRUE,$3,$4,$5)",&[&user,&format!("{user}@example.invalid"),&json!([role]),&permissions,&json!({"description":"Original biography","website":"https://example.invalid","discord":"unchanged"})]).await.unwrap();
  let hash=Sha256::digest(format!("synthetic-staff-{user}").as_bytes()).to_vec();db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES($1,$1,$2,clock_timestamp()+interval '1 hour')",&[&user,&hash]).await.unwrap();
 }
 for(id,name)in[("StaffGrp01","First synthetic group"),("StaffGrp02","Second synthetic group")]{db.execute("INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES($1,'Reporter01','Reporter01',$2)",&[&id,&name]).await.unwrap();}
 db.execute("INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader,is_officer) VALUES('StaffGrp01','Reporter01','Previous officer name',FALSE,TRUE)",&[]).await.unwrap();
 for(id,title)in[("StaffTitle","Synthetic title"),("StaffOther","Other synthetic title")]{db.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES($1,'Reporter01','Reporter01',$2,'ja','ongoing','safe','published')",&[&id,&title]).await.unwrap();}
 db.execute("INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,language) VALUES('StaffChap1','StaffTitle','Reporter01','Reporter01','en')",&[]).await.unwrap();
 for(id,owner,category,target,reason)in[("ReportOne1","Reporter01","user","Foreign001","user_spam"),("ReportTwo1","Foreign001","user","Reporter01","user_spam"),("ClaimOne01","Reporter01","scanlation_group","StaffGrp01","scanlation_group_claim"),("ClaimTwo01","Reporter01","scanlation_group","StaffGrp02","scanlation_group_claim"),("HideTitle1","Reporter01","manga","StaffTitle","manga_removal_request"),("HideChap01","Reporter01","chapter","StaffChap1","chapter_removal_request")]{db.execute("INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,body,target_version) VALUES($1,$2,$3,$4,$5,'Synthetic report evidence',1)",&[&id,&owner,&category,&target,&reason]).await.unwrap();db.execute("INSERT INTO platform.report_audit(report_id,actor_id,operation,new_status,new_version) VALUES($1,$2,'create','waiting',1)",&[&id,&owner]).await.unwrap();}
 drop(db);state.runtime.mark_ready();let app=router::router(state.clone());let server=Server(tokio::spawn(async move{axum::serve(listener,app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await.unwrap();}));
 Fixture{state,http:reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(15)).build().unwrap(),origin,_server:server}
}
#[tokio::test]
#[ignore="owned disposable database only"]
async fn native_staff_reports_profiles_and_corrections_are_scoped_fenced_and_audited(){
 let f=fixture().await;
 for user in ["Reporter01","RoleOnly01","GrantOnly1"]{f.call(user,"GET","/api/staff/reports",None,403).await;}
 let own=f.call("ModOne0001","GET","/api/report",None,200).await;assert_eq!(own["total"],0);
 let all=f.call("ModOne0001","GET","/api/staff/reports",None,200).await;assert_eq!(all["total"],6);assert_eq!(all["scope"],"site-moderator-queue");
 let personal=f.call("Reporter01","GET","/api/report",None,200).await;assert_eq!(personal["total"],5);
 f.call("Reporter01","GET","/api/my/reports/ReportTwo1/history",None,404).await;
 f.call("Reporter01","GET","/api/staff/history",None,403).await;
 let(before_version,before_audit):(i32,i64)={let row=f.state.db.client().await.unwrap().query_one("SELECT version,(SELECT count(*) FROM platform.report_audit WHERE report_id='ClaimOne01') FROM platform.reports WHERE id='ClaimOne01'",&[]).await.unwrap();(row.get(0),row.get(1))};
 let preview=f.review("ModOne0001","ClaimOne01").await;assert_eq!(preview["acceptEffect"]["action"],"grant_group_leadership");assert_eq!(preview["claimant"]["name"],"Reporter01");assert_eq!(preview["acceptEffect"]["previous_membership"]["is_officer"],true);
 let after=f.state.db.client().await.unwrap().query_one("SELECT version,(SELECT count(*) FROM platform.report_audit WHERE report_id='ClaimOne01') FROM platform.reports WHERE id='ClaimOne01'",&[]).await.unwrap();assert_eq!(after.get::<_,i32>(0),before_version);assert_eq!(after.get::<_,i64>(1),before_audit); // Cancel = read only.
 let (one,two)=tokio::join!(f.send("ModOne0001","POST","/api/staff/reports/ReportOne1/work",Some(json!({"requestId":"claim-first-00000001","version":1,"action":"claim"}))),f.send("ModTwo0001","POST","/api/staff/reports/ReportOne1/work",Some(json!({"requestId":"claim-second-0000001","version":1,"action":"claim"}))));
 let mut statuses=vec![one.0,two.0];statuses.sort();assert_eq!(statuses,vec![200,409],"claim race: {one:?} {two:?}");let winner=if one.0==200{"ModOne0001"}else{"ModTwo0001"};let loser=if winner=="ModOne0001"{"ModTwo0001"}else{"ModOne0001"};
 let started=f.call(winner,"POST","/api/staff/reports/ReportOne1/work",Some(json!({"requestId":"progress-0000000001","version":2,"action":"progress"})),200).await;assert_eq!(started["effect"]["workState"],"in_progress");
 f.call(loser,"POST","/api/staff/reports/ReportOne1/work",Some(json!({"requestId":"stale-progress-00001","version":2,"action":"progress"})),409).await;
 let command=f.decision(&preview,"accept","claim-accept-00000001");let saved=f.call("ModOne0001","POST","/api/staff/reports/ClaimOne01/decision",Some(command.clone()),200).await;let repeated=f.call("ModOne0001","POST","/api/staff/reports/ClaimOne01/decision",Some(command.clone()),200).await;assert_eq!(saved,repeated);
 let audit_count:i64=f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.report_audit WHERE report_id='ClaimOne01' AND operation='accept'",&[]).await.unwrap().get(0);assert_eq!(audit_count,1);
 let correction=f.call("StaffOne01","GET","/api/staff/reports/ClaimOne01/correction",None,200).await;assert_eq!(correction["context"]["effect"]["action"],"restore_previous_group_membership");
 f.call("ModOne0001","POST","/api/staff/reports/ClaimOne01/correction",Some(json!({"requestId":"correction-denied-01","contextHash":correction["context"]["contextHash"],"reason":"Synthetic reason"})),403).await;
 f.state.db.client().await.unwrap().execute("UPDATE platform.users SET permissions=permissions-'group.leadership.revoke' WHERE id='StaffOne01'",&[]).await.unwrap();
 f.call("StaffOne01","POST","/api/staff/reports/ClaimOne01/correction",Some(json!({"requestId":"lost-effect-grant-001","contextHash":correction["context"]["contextHash"],"reason":"Missing exact inverse authority"})),403).await;
 f.state.db.client().await.unwrap().execute("UPDATE platform.users SET permissions=permissions||'[\"group.leadership.revoke\"]'::jsonb WHERE id='StaffOne01'",&[]).await.unwrap();
 let corrected=f.call("StaffOne01","POST","/api/staff/reports/ClaimOne01/correction",Some(json!({"requestId":"correction-group-001","contextHash":correction["context"]["contextHash"],"reason":"The original claim was mistaken"})),200).await;assert_eq!(corrected["status"],"refused");
 let membership=f.state.db.client().await.unwrap().query_one("SELECT name,is_leader,is_officer FROM platform.catalog_group_members WHERE group_id='StaffGrp01' AND user_id='Reporter01'",&[]).await.unwrap();assert_eq!(membership.get::<_,String>(0),"Previous officer name");assert!(!membership.get::<_,bool>(1));assert!(membership.get::<_,bool>(2));
 let history=f.call("StaffOne01","GET","/api/staff/history?search=ClaimOne01",None,200).await;assert!(history["data"].as_array().unwrap().iter().any(|r|r["operation"]=="accept"));assert!(history["data"].as_array().unwrap().iter().any(|r|r["operation"]=="correct"));
 assert!(f.state.db.client().await.unwrap().execute("UPDATE platform.report_audit SET notes='rewrite' WHERE report_id='ClaimOne01'",&[]).await.is_err());
 let mine=f.call("Reporter01","GET","/api/my/reports/ClaimOne01/history",None,200).await;assert!(!mine.to_string().contains("previousMembership"));assert!(!mine.to_string().contains("actorId\":\"StaffOne01"));
 let appeal=f.call("Reporter01","POST","/api/my/reports/ClaimOne01/appeal",Some(json!({"requestId":"own-appeal-000000001","version":mine["version"],"reason":"Please review the new evidence"})),200).await;assert_eq!(appeal["effect"]["targetChanged"],false);
 f.call("Reporter01","GET","/api/staff/receipts/own-appeal-000000001",None,200).await;
 f.call("Foreign001","GET","/api/staff/receipts/own-appeal-000000001",None,404).await;
 let claim=f.review("ModOne0001","ClaimTwo01").await;
 f.call("ModOne0001","POST","/api/staff/reports/ClaimTwo01/decision",Some(f.decision(&claim,"accept","second-claim-accept-01")),200).await;
 let ca=f.call("StaffOne01","GET","/api/staff/reports/ClaimTwo01/correction",None,200).await;
 let cb=f.call("StaffTwo01","GET","/api/staff/reports/ClaimTwo01/correction",None,200).await;
 let(a,b)=tokio::join!(f.send("StaffOne01","POST","/api/staff/reports/ClaimTwo01/correction",Some(json!({"requestId":"correction-race-one01","contextHash":ca["context"]["contextHash"],"reason":"One exact inverse"}))),f.send("StaffTwo01","POST","/api/staff/reports/ClaimTwo01/correction",Some(json!({"requestId":"correction-race-two01","contextHash":cb["context"]["contextHash"],"reason":"One exact inverse"}))));
 let mut codes=vec![a.0,b.0];codes.sort();assert_eq!(codes,vec![200,409],"correction race {a:?} {b:?}");
 assert_eq!(f.state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.catalog_group_members WHERE group_id='StaffGrp02'",&[]).await.unwrap().get::<_,i64>(0),0);
 let ca=f.review("ModOne0001","ReportTwo1").await;let cb=f.review("ModTwo0001","ReportTwo1").await;
 let(a,b)=tokio::join!(f.send("ModOne0001","POST","/api/staff/reports/ReportTwo1/decision",Some(f.decision(&ca,"refuse","decision-race-one001"))),f.send("ModTwo0001","POST","/api/staff/reports/ReportTwo1/decision",Some(f.decision(&cb,"refuse","decision-race-two001"))));let mut codes=vec![a.0,b.0];codes.sort();assert_eq!(codes,vec![200,409],"decision race {a:?} {b:?}");
 let deletion=f.call("ModOne0001","GET","/api/staff/reports/ReportTwo1/review?intent=delete",None,200).await;
 assert_eq!(deletion["context"]["effect"]["content_changed"],false);
 assert!(!f.state.db.client().await.unwrap().query_one("SELECT deleted_at IS NOT NULL FROM platform.reports WHERE id='ReportTwo1'",&[]).await.unwrap().get::<_,bool>(0));
 let command=f.decision(&deletion["context"],"delete","delete-report-only001");let once=f.call("ModOne0001","POST","/api/staff/reports/ReportTwo1/decision",Some(command.clone()),200).await;let twice=f.call("ModOne0001","POST","/api/staff/reports/ReportTwo1/decision",Some(command),200).await;assert_eq!(once,twice);
 let note:String=f.state.db.client().await.unwrap().query_one("SELECT notes FROM platform.report_audit WHERE report_id='ReportTwo1' AND operation='delete'",&[]).await.unwrap().get(0);assert_eq!(note,"Synthetic reviewed decision");
 assert!(!f.state.db.client().await.unwrap().query_one("SELECT disabled FROM platform.users WHERE id='Reporter01'",&[]).await.unwrap().get::<_,bool>(0));
 let review=f.review("ModOne0001","HideChap01").await;assert_eq!(review["acceptEffect"]["action"],"instance_soft_hide");f.call("ModOne0001","POST","/api/staff/reports/HideChap01/decision",Some(f.decision(&review,"accept","hide-chapter-00000001")),200).await;
 let restore=f.call("StaffOne01","GET","/api/staff/reports/HideChap01/correction",None,200).await;
 f.state.db.client().await.unwrap().execute("UPDATE platform.catalog_manga SET protocol_source_visible=FALSE WHERE id='StaffTitle'",&[]).await.unwrap();
 f.call("StaffOne01","POST","/api/staff/reports/HideChap01/correction",Some(json!({"requestId":"source-withdrawn-001","contextHash":restore["context"]["contextHash"],"reason":"Must not override source"})),409).await;
 assert!(f.state.db.client().await.unwrap().query_one("SELECT deleted_at IS NOT NULL FROM platform.catalog_chapters WHERE id='StaffChap1'",&[]).await.unwrap().get::<_,bool>(0));
 f.state.db.client().await.unwrap().execute("UPDATE platform.catalog_manga SET protocol_source_visible=TRUE WHERE id='StaffTitle'",&[]).await.unwrap();
 let restore=f.call("StaffOne01","GET","/api/staff/reports/HideChap01/correction",None,200).await;
 f.call("StaffOne01","POST","/api/staff/reports/HideChap01/correction",Some(json!({"requestId":"restore-chapter-00001","contextHash":restore["context"]["contextHash"],"reason":"Restore this local hide"})),200).await;
 let review=f.review("ModOne0001","HideTitle1").await;f.call("ModOne0001","POST","/api/staff/reports/HideTitle1/decision",Some(f.decision(&review,"accept","hide-title-000000001")),200).await;
 let restore=f.call("StaffOne01","GET","/api/staff/reports/HideTitle1/correction",None,200).await;
 f.state.db.client().await.unwrap().execute("UPDATE platform.catalog_manga SET version=version+1 WHERE id='StaffTitle'",&[]).await.unwrap();
 f.call("StaffOne01","POST","/api/staff/reports/HideTitle1/correction",Some(json!({"requestId":"stale-inverse-000001","contextHash":restore["context"]["contextHash"],"reason":"A newer edit must win"})),409).await;
 let target:String=f.state.db.client().await.unwrap().query_one("SELECT public_id::text FROM platform.public_resource_ids WHERE kind='user' AND native_id='Foreign001' AND preferred",&[]).await.unwrap().get(0);let path=format!("/api/staff/users/{target}/profile");
 for user in ["Reporter01","ForeignMod","GrantOnly1"]{f.call(user,"GET",&path,None,403).await;}
 let profile=f.call("StaffOne01","GET",&path,None,200).await;let changed=f.call("StaffOne01","PUT",&path,Some(json!({"expectedUserId":target,"patch":{"version":profile["attributes"]["version"],"description":"Reviewed staff edit"}})),200).await;assert_eq!(changed["attributes"]["description"],"Reviewed staff edit");assert_eq!(changed["attributes"]["discord"],"unchanged");assert_eq!(changed["attributes"]["website"],"https://example.invalid");
 f.call("StaffOne01","PUT",&path,Some(json!({"expectedUserId":target,"patch":{"version":profile["attributes"]["version"],"description":"Stale write"}})),409).await;
 f.state.db.client().await.unwrap().execute("UPDATE platform.users SET roles='[\"user\"]' WHERE id='StaffOne01'",&[]).await.unwrap();
 f.call("StaffOne01","PUT",&path,Some(json!({"expectedUserId":target,"patch":{"version":changed["attributes"]["version"],"description":"Lost role must fail"}})),403).await;
 f.call("StaffOne01","GET","/api/staff/operations",None,403).await;
 f.state.db.client().await.unwrap().execute("UPDATE platform.users SET roles='[\"staff\"]' WHERE id='StaffOne01'",&[]).await.unwrap();
 let reverted=f.call("StaffOne01","PUT",&path,Some(json!({"expectedUserId":target,"patch":{"version":changed["attributes"]["version"],"description":"Original biography"}})),200).await;assert_eq!(reverted["attributes"]["description"],"Original biography");
 let public=f.call("Reporter01","GET","/api/user/Foreign001",None,200).await;assert_eq!(public["description"],"Original biography");
 f.state.db.client().await.unwrap().execute("INSERT INTO platform.protocol_publication_commands(command_id,user_id,input_hash,resource_kind,catalog_id,created_at,state) VALUES('manga:StaffOther','Reporter01',$1,'manga','StaffOther',1,'prepared')",&[&"a".repeat(64)]).await.unwrap();
 f.call("Reporter01","GET","/api/staff/operations",None,403).await;
 let operations=f.call("StaffOne01","GET","/api/staff/operations",None,200).await;assert_eq!(operations["scope"],"native-operations-read-only");assert_eq!(operations["publicationCount"],1);assert!(operations["pendingPublications"][0]["nextStep"].as_str().unwrap().contains("do not repeat"));assert!(operations["health"]["instancePoliciesConfigured"].as_bool().unwrap());
 let policies=f.http.get(format!("{}/api/instance-information",f.origin)).send().await.unwrap().json::<Value>().await.unwrap();assert_eq!(policies["information"]["operatorName"],"Synthetic operator");assert!(!policies.to_string().contains("weebdex.org"));
 let invalid=f.request("ModOne0001","POST","/api/staff/reports/ReportOne1/work").header("origin","https://foreign.example.invalid").json(&json!({"requestId":"cross-origin-00000001","version":3,"action":"unclaim"})).send().await.unwrap();assert_eq!(invalid.status().as_u16(),403);
 history_filter_export_acceptance(&f).await;
 f.state.db.client().await.unwrap().execute("UPDATE platform.sessions SET revoked_at=clock_timestamp() WHERE user_id='ModOne0001'",&[]).await.unwrap();f.call("ModOne0001","GET","/api/staff/reports",None,401).await;
 println!("Verified actual HTTP native queue/privacy, claim race, durable idempotent decisions, local inverse/officer preservation, append-only audit, own appeals, source/stale fences, role/grant denial, foreign profile CAS/downgrade, operations and configured policies.");
}

// F16-A01 successor: same authorized history scope, real HTTP and PostgreSQL.
async fn history_filter_export_acceptance(f:&Fixture){
 let db=f.state.db.client().await.unwrap();
 let samples=[("2024-02-28T23:59:59.999999Z","OUTSIDE_BEFORE"),("2024-02-29T00:00:00Z","\u{feff}\u{200b}=HYPERLINK(\"x\")\r\nline\tend"),("2024-02-29T12:00:00Z","\t+SUM(1,2)"),("2024-02-29T12:00:01Z"," \r\n-10+20"),("2024-02-29T12:00:02Z","＝fullwidth 東京"),("2024-02-29T23:59:59.999999Z","@formula"),("2024-03-01T00:00:00Z","OUTSIDE_AFTER")];
 for(date,note)in samples{db.execute("INSERT INTO platform.report_audit(report_id,actor_id,operation,old_status,new_status,old_version,new_version,notes,effect,created_at) VALUES('ReportOne1','ModOne0001','appeal','refused','refused',3,4,$1,$2,$3::text::timestamptz)",&[&note,&json!({"action":"appeal","private":"PRESTATE_SECRET_NOT_FOR_EXPORT","previousMembership":{"name":"PRIVATE_MEMBER_NAME"}}),&date]).await.unwrap();}
 db.execute("INSERT INTO platform.report_audit(report_id,actor_id,operation,new_status,new_version,notes,created_at) SELECT 'ReportOne1','ModOne0001','priority','waiting',4,'CSV_ROW_LIMIT','2026-01-03T12:00:00Z'::timestamptz FROM generate_series(1,260)",&[]).await.unwrap();
 let long=format!("CSV_BYTE_LIMIT {}","語".repeat(1200));
 db.execute("INSERT INTO platform.report_audit(report_id,actor_id,operation,new_status,new_version,notes,created_at) SELECT 'ReportOne1','ModOne0001','reject','refused',4,$1,'2026-01-04T12:00:00Z'::timestamptz FROM generate_series(1,80)",&[&long]).await.unwrap();drop(db);
 for invalid in ["from=2023-02-29","from=2024-02-30","to=2024-13-01","from=2024-2-01","from=2024-03-01&to=2024-02-29","from=1969-12-31","operation=role_grant","export=json","export=csv&page=2"]{f.call("ModOne0001","GET",&format!("/api/staff/history?{invalid}"),None,400).await;}
 // Unknown fields are rejected by Axum's Query extractor before the JSON handler.
 // Assert that narrow contract directly; all handler refusals remain strict JSON.
 let unknown=f.request("ModOne0001","GET","/api/staff/history?export=csv&limit=10000").send().await.unwrap();assert_eq!(unknown.status().as_u16(),400);assert!(unknown.headers()["cache-control"].to_str().unwrap().contains("no-store"));assert!(unknown.headers()["content-type"].to_str().unwrap().starts_with("text/plain"));let rejected=unknown.text().await.unwrap();assert!(rejected.starts_with("Failed to deserialize query string:"),"{rejected}");assert!(rejected.contains("unknown field `limit`"),"{rejected}");assert!(!rejected.contains("staff-decision-history-export")&&!rejected.contains("PRESTATE_SECRET")&&!rejected.contains("CSV_ROW_LIMIT"));
 let query="from=2024-02-29&to=2024-02-29&operation=appeal&reportId=ReportOne1";
 let filtered=f.call("ModOne0001","GET",&format!("/api/staff/history?{query}"),None,200).await;assert_eq!(filtered["total"],5);assert_eq!(filtered["data"].as_array().unwrap().len(),5);assert!(!filtered.to_string().contains("OUTSIDE_"));
 let export_path=format!("/api/staff/history?{query}&export=csv&page=1");
 for user in ["Reporter01","Foreign001","RoleOnly01","GrantOnly1"]{let denied=f.call(user,"GET",&export_path,None,403).await;assert!(denied.get("csv").is_none());}
 f.call("Reporter01","GET","/api/my/reports/ReportTwo1/history",None,404).await; // Export adds no owner-scope bypass.
 let saved=f.call("ModOne0001","GET",&export_path,None,200).await;assert_eq!(saved["scope"],"staff-decision-history-export");assert_eq!(saved["exported"],5);assert_eq!(saved["total"],5);assert_eq!(saved["truncated"],false);
 let csv=saved["csv"].as_str().unwrap();assert_eq!(saved["bytes"].as_u64().unwrap() as usize,csv.len());assert!(csv.starts_with('\u{feff}'));assert!(csv.contains("東京"));assert!(csv.contains("\"'  =HYPERLINK(\"\"x\"\")  line end\""));assert!(!csv.contains("PRESTATE_SECRET")&&!csv.contains("PRIVATE_MEMBER_NAME")&&!csv.contains("previousMembership")&&!csv.contains("actorId"));assert!(!csv.contains('\u{200b}')&&!csv[3..].contains('\u{feff}'));
 let records=csv.split("\r\n").collect::<Vec<_>>();assert_eq!(records.len(),7);assert_eq!(records.last(),Some(&""));for record in &records[1..6]{assert!(!record.contains('\n')&&!record.contains('\r')&&!record.contains('\t'));}
 let page=f.call("ModOne0001","GET","/api/staff/history?from=2026-01-03&to=2026-01-03&operation=priority&page=2",None,200).await;assert_eq!(page["total"],260);assert_eq!(page["data"].as_array().unwrap().len(),25);assert_eq!(page["page"],2);
 let limited=f.call("ModOne0001","GET","/api/staff/history?from=2026-01-03&to=2026-01-03&operation=priority&export=csv",None,200).await;assert_eq!(limited["exported"],250);assert_eq!(limited["total"],260);assert_eq!(limited["truncated"],true);assert_eq!(limited["rowLimit"],250);assert!(limited["bytes"].as_u64().unwrap()<=262144);assert_eq!(limited["csv"].as_str().unwrap().split("\r\n").count(),252);
 let bounded=f.call("ModOne0001","GET","/api/staff/history?from=2026-01-04&to=2026-01-04&operation=reject&export=csv",None,200).await;let count=bounded["exported"].as_u64().unwrap();assert!(count>0&&count<80);assert_eq!(bounded["total"],80);assert_eq!(bounded["truncated"],true);assert_eq!(bounded["byteLimit"],262144);assert!(bounded["bytes"].as_u64().unwrap()<=262144);let csv=bounded["csv"].as_str().unwrap();assert_eq!(csv.split("\r\n").count() as u64,count+2);assert_eq!(csv.matches(long.as_str()).count() as u64,count); // No partial row or partial UTF-8 cell.
 let none=f.call("ModOne0001","GET","/api/staff/history?from=2025-01-01&to=2025-01-01&operation=appeal&export=csv",None,200).await;assert_eq!(none["exported"],0);assert_eq!(none["total"],0);assert_eq!(none["truncated"],false);
 f.state.db.client().await.unwrap().execute("UPDATE platform.users SET permissions=permissions-'report.history' WHERE id='ModOne0001'",&[]).await.unwrap();
 let revoked=f.call("ModOne0001","GET",&export_path,None,403).await;assert!(revoked.get("csv").is_none());
 f.state.db.client().await.unwrap().execute("UPDATE platform.users SET permissions=permissions||'[\"report.history\"]'::jsonb WHERE id='ModOne0001'",&[]).await.unwrap();
 println!("Verified F16-A01 actual HTTP date/operation filters, UTC inclusive leap-day endpoints, denied lower roles, private projection, formula/control/quote/UTF-8 safety, 250-row and256KiB whole-row bounds.");
}
