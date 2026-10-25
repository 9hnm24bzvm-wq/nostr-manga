//! Isolated real HTTP/SQL controls for the closure + scheduling semantic merge.
//! Reuses owned public_uploads fixtures. No production hooks, real accounts, or
//! test execution are authorized by merely compiling this support module.
use super::*;
const OWNER: &str = "PublicUploadUser1";
const COOKIE: &str = "weebdex_session=synthetic-public-upload-cookie";

async fn closure_http(f: &Fixture, method: &str, path: &str, body: Option<Value>, status: u16) -> Value {
    let mut request = f.http.request(method.parse().unwrap(), format!("{}{path}", f.origin))
        .header("origin", &f.origin).header("cookie", COOKIE).timeout(Duration::from_secs(8));
    if let Some(body) = body { request = request.json(&body); }
    let response = request.send().await.unwrap();
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    assert_eq!(response.headers()["referrer-policy"], "no-referrer");
    let actual = response.status().as_u16();
    let text = response.text().await.unwrap();
    assert_eq!(actual, status, "{method} {path}: {text}");
    serde_json::from_str(&text).unwrap()
}
async fn prepare_closure(f: &Fixture) -> Value {
    let view = closure_http(f, "GET", &format!("/api/account/closure/{OWNER}"), None, 200).await;
    assert_eq!(view["blockers"], json!([]));
    closure_http(f, "POST", "/api/account/closure/prepare", Some(json!({
        "expectedAccountId": OWNER, "expectedUserId": OWNER,
        "expectedLifecycleId": view["lifecycleId"], "version": view["version"]
    })), 200).await
}
fn confirmation(proof: &Value) -> Value {
    assert_eq!(proof["result"], "prepared");
    json!({"capture":{"expectedAccountId":OWNER,"expectedUserId":OWNER,
        "expectedLifecycleId":proof["lifecycleId"],"version":proof["version"]},
        "confirmationId":proof["confirmationId"],"confirmationToken":proof["confirmationToken"],
        "acknowledge":"DISABLE THIS ACCOUNT"})
}
async fn close(f: &Fixture, proof: &Value, status: u16) -> Value {
    closure_http(f, "POST", "/api/account/closure/confirm", Some(confirmation(proof)), status).await
}
async fn private_snapshot(f: &Fixture, session: &str) -> Value {
    let db = f.state.db.client().await.unwrap();
    let state: Value = db.query_one("SELECT jsonb_build_object('intent',(SELECT to_jsonb(i) FROM platform.scheduled_uploads i WHERE session_id=$1),'commands',(SELECT jsonb_agg(to_jsonb(c) ORDER BY command_id) FROM platform.protocol_publication_commands c WHERE command_id='upload:'||$1 OR command_id='manga:'||$2 OR scheduled_session_id=$1),'events',(SELECT jsonb_agg(encode(id,'hex') ORDER BY id) FROM public.events))", &[&session,&f.native_manga]).await.unwrap().get(0);
    drop(db);
    json!({"private":state,"media":retained_snapshot(f,session).await})
}
async fn unchanged_authority(f: &Fixture, proof: &Value) {
    let row = f.state.db.client().await.unwrap().query_one("SELECT disabled,version,(SELECT count(*) FROM platform.account_lifecycle_audit WHERE user_id=$1),(SELECT used_at IS NULL FROM platform.account_disable_confirmations WHERE id=$2),(SELECT count(*) FROM platform.sessions WHERE user_id=$1 AND revoked_at IS NULL),(SELECT count(*) FROM platform.protocol_identity_bindings WHERE user_id=$1 AND revoked_at IS NULL) FROM platform.users WHERE id=$1", &[&OWNER,&proof["confirmationId"].as_str().unwrap()]).await.unwrap();
    assert!(!row.get::<_,bool>(0));assert_eq!(i64::from(row.get::<_,i32>(1)),proof["version"].as_i64().unwrap());
    assert_eq!(row.get::<_,i64>(2),0);assert!(row.get::<_,bool>(3));assert_eq!(row.get::<_,i64>(4),1);assert_eq!(row.get::<_,i64>(5),1);
}
async fn blocked_query(f: &Fixture, blocker: i32, query_pattern: &str) -> i32 {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let rows = f.state.db.client().await.unwrap().query("SELECT pid FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND $1=ANY(pg_blocking_pids(pid)) AND query LIKE $2", &[&blocker,&query_pattern]).await.unwrap();
            if !rows.is_empty() { assert_eq!(rows.len(),1,"ambiguous fixture waiter");break rows[0].get(0); }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap_or_else(|error| panic!("the exact production transaction never reached its intended lock: pattern={:?}: {error}", query_pattern.chars().take(160).collect::<String>()))
}
async fn assert_no_source_or_alias(f: &Fixture, chapter: &str) {
    let row = f.state.db.client().await.unwrap().query_one("SELECT (SELECT count(*) FROM platform.protocol_publication_commands),(SELECT count(*) FROM public.events WHERE kind IN (1001,1002)),(SELECT count(*) FROM platform.public_resource_ids WHERE kind='chapter' AND native_id=$1),(SELECT count(*) FROM platform.catalog_chapters WHERE id=$1)", &[&chapter]).await.unwrap();
    for column in 0..4 {assert_eq!(row.get::<_,i64>(column),0);}
}

#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Owned isolated fixture and original loopback node; exact test only, coordinated heavy slot"]
async fn scheduled_execution_account_fence_refuses_closure_without_losing_source_provenance() {
    let f = fixture_options(Config::default(),true).await;
    let proof = prepare_closure(&f).await;
    let (_,session,_,_) = scheduled_upload(&f,&png(),2).await;
    let chapter: String = f.state.db.client().await.unwrap().query_one("SELECT chapter_id FROM platform.scheduled_uploads WHERE session_id=$1", &[&session]).await.unwrap().get(0);
    await_schedule_due(&f,&session).await;
    let mut alias_db=f.state.db.client().await.unwrap();let alias=alias_db.transaction().await.unwrap();
    let alias_pid:i32=alias.query_one("SELECT pg_backend_pid()",&[]).await.unwrap().get(0);
    alias.query_one("SELECT pg_advisory_xact_lock(hashtextextended($1,0))",&[&format!("platform-public-id:chapter:{chapter}")]).await.unwrap();
    let state=f.state.clone();let worker=tokio::spawn(async move{weebdex_platform::public_api::scheduled_uploads::run_due(&state).await});
    blocked_query(&f,alias_pid,"%public_register_id%").await;
    assert!(f.state.db.client().await.unwrap().query_one("SELECT state='starting' AND started_at IS NOT NULL FROM platform.scheduled_uploads WHERE session_id=$1",&[&session]).await.unwrap().get::<_,bool>(0));
    // Durable start is already committed. Block the final execution gate on its
    // parent row only after releasing the independent alias registration barrier.
    let mut parent_db=f.state.db.client().await.unwrap();let parent=parent_db.transaction().await.unwrap();
    let parent_pid:i32=parent.query_one("SELECT pg_backend_pid()",&[]).await.unwrap().get(0);
    parent.query_one("SELECT id FROM platform.catalog_manga WHERE id=$1 FOR UPDATE",&[&f.native_manga]).await.unwrap();
    alias.commit().await.unwrap();drop(alias_db);
    blocked_query(&f,parent_pid,"SELECT protocol_series_id,version FROM platform.catalog_manga%").await;
    let before=private_snapshot(&f,&session).await;
    close(&f,&proof,409).await;unchanged_authority(&f,&proof).await;
    assert_eq!(private_snapshot(&f,&session).await,before);
    // Also pause inside real original chapter admission, after the platform
    // command has independently committed. This catches dropping account_tx
    // before command_publish, rather than merely exercising the earlier gate.
    f.state.db.client().await.unwrap().batch_execute("CREATE FUNCTION public.closure_scheduled_pause() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.kind=1001 THEN PERFORM pg_advisory_xact_lock(760532); END IF; RETURN NEW; END $$; CREATE TRIGGER closure_scheduled_pause BEFORE INSERT ON public.events FOR EACH ROW EXECUTE FUNCTION public.closure_scheduled_pause()").await.unwrap();
    let mut source_db=f.state.db.client().await.unwrap();let source=source_db.transaction().await.unwrap();
    let source_pid:i32=source.query_one("SELECT pg_backend_pid()",&[]).await.unwrap().get(0);
    source.query_one("SELECT pg_advisory_xact_lock(760532)",&[]).await.unwrap();
    parent.rollback().await.unwrap();drop(parent_db);
    let original_pid=blocked_query(&f,source_pid,"INSERT INTO events(id,publisher,kind,created_at,raw_event,state,manifest)%").await;
    assert!(f.state.db.client().await.unwrap().query_one("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE pid=$1 AND locktype='advisory' AND objid=760532 AND NOT granted)",&[&original_pid]).await.unwrap().get::<_,bool>(0));
    let during_source=private_snapshot(&f,&session).await;
    assert!(during_source["private"]["commands"].as_array().unwrap().iter().any(|c|c["command_id"]==format!("upload:{session}")&&c["state"]=="prepared"));
    let mut probe_db=f.state.db.client().await.unwrap();let probe=probe_db.transaction().await.unwrap();
    let conflict=probe.query_one("SELECT id FROM platform.users WHERE id=$1 FOR UPDATE NOWAIT",&[&OWNER]).await.unwrap_err();
    assert_eq!(conflict.code().unwrap().code(),"55P03");probe.rollback().await.unwrap();drop(probe_db);
    close(&f,&proof,409).await;unchanged_authority(&f,&proof).await;
    assert_eq!(private_snapshot(&f,&session).await,during_source);
    source.rollback().await.unwrap();drop(source_db);
    assert!(tokio::time::timeout(Duration::from_secs(20),worker).await.unwrap().unwrap().unwrap());
    f.state.db.client().await.unwrap().batch_execute("DROP TRIGGER closure_scheduled_pause ON public.events; DROP FUNCTION public.closure_scheduled_pause()").await.unwrap();
    let db=f.state.db.client().await.unwrap();
    let row=db.query_one("SELECT i.state,i.started_at IS NOT NULL,s.state,i.signer,i.payload_hash=s.commit_hash FROM platform.scheduled_uploads i JOIN platform.upload_sessions s ON s.id=i.session_id WHERE i.session_id=$1",&[&session]).await.unwrap();
    assert_eq!(row.get::<_,String>(0),"complete");assert!(row.get::<_,bool>(1));assert_eq!(row.get::<_,String>(2),"committed");assert_eq!(row.get::<_,String>(3),public(1));assert!(row.get::<_,bool>(4));
    let commands=db.query("SELECT c.user_id,c.scheduled_session_id,c.resource_kind,c.source_manifest,i.parent_manifest,e.raw_event FROM platform.protocol_publication_commands c JOIN platform.scheduled_uploads i ON i.session_id=c.scheduled_session_id JOIN public.events e ON e.id=decode(c.event_id,'hex') ORDER BY c.command_id",&[]).await.unwrap();
    assert_eq!(commands.len(),2);
    for command in commands {assert_eq!(command.get::<_,String>(0),OWNER);assert_eq!(command.get::<_,String>(1),session);if command.get::<_,String>(2)=="manga" {assert_eq!(command.get::<_,Vec<u8>>(3),command.get::<_,Vec<u8>>(4));}let event:auth::Event=serde_json::from_value(command.get(5)).unwrap();event.verify().unwrap();assert_eq!(event.pubkey,public(1));}
    drop(db);unchanged_authority(&f,&proof).await;
}

#[tokio::test(flavor="multi_thread",worker_threads=4)]
#[ignore="Owned isolated fixture; uses a fixture-only global admission-table barrier, exact test alone"]
async fn cancelled_claim_then_real_closure_fences_late_scheduled_authority() {
    let f=fixture().await;let proof=prepare_closure(&f).await;
    let (public_session,session,_,_)=scheduled_upload(&f,&png(),2).await;
    let chapter:String=f.state.db.client().await.unwrap().query_one("SELECT chapter_id FROM platform.scheduled_uploads WHERE session_id=$1",&[&session]).await.unwrap().get(0);
    await_schedule_due(&f,&session).await;
    let mut session_db=f.state.db.client().await.unwrap();let held=session_db.transaction().await.unwrap();
    let session_pid:i32=held.query_one("SELECT pg_backend_pid()",&[]).await.unwrap().get(0);
    held.query_one("SELECT id FROM platform.upload_sessions WHERE id=$1 FOR UPDATE",&[&session]).await.unwrap();
    let url=format!("{}/api/v1/upload/{public_session}",f.origin);let token=proof_for_cancel(&url);let http=f.http.clone();
    let cancel=tokio::spawn(async move{http.delete(url).header("authorization",token).timeout(Duration::from_secs(10)).send().await.unwrap().status().as_u16()});
    let cancel_pid=blocked_query(&f,session_pid,"%platform.upload_sessions%").await;
    let state=f.state.clone();let(pid_tx,pid_rx)=tokio::sync::oneshot::channel();let(acquired_tx,acquired_rx)=tokio::sync::oneshot::channel();let(release_tx,release_rx)=tokio::sync::oneshot::channel();
    let barrier=tokio::spawn(async move{let mut db=state.db.client().await.unwrap();let tx=db.transaction().await.unwrap();let pid:i32=tx.query_one("SELECT pg_backend_pid()",&[]).await.unwrap().get(0);pid_tx.send(pid).unwrap();tx.batch_execute("LOCK TABLE account_permissions IN ACCESS EXCLUSIVE MODE").await.unwrap();acquired_tx.send(()).unwrap();let _=release_rx.await;tx.rollback().await.unwrap();});
    let barrier_pid=tokio::time::timeout(Duration::from_secs(5),pid_rx).await.unwrap().unwrap();assert_eq!(blocked_query(&f,cancel_pid,"LOCK TABLE account_permissions%").await,barrier_pid);
    let state=f.state.clone();let worker=tokio::spawn(async move{weebdex_platform::public_api::scheduled_uploads::run_due(&state).await});
    wait_claim(&f,&session,None).await;
    let worker_pid=blocked_query(&f,barrier_pid,"SELECT allowed,version FROM account_permissions WHERE publisher=$1 FOR SHARE").await;
    assert!(f.state.db.client().await.unwrap().query_one("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE pid=$1 AND relation='account_permissions'::regclass AND mode='RowShareLock' AND NOT granted)",&[&worker_pid]).await.unwrap().get::<_,bool>(0));
    held.commit().await.unwrap();drop(session_db);
    assert_eq!(tokio::time::timeout(Duration::from_secs(8),cancel).await.unwrap().unwrap(),200);
    tokio::time::timeout(Duration::from_secs(5),acquired_rx).await.unwrap().unwrap();
    let before=private_snapshot(&f,&session).await;
    assert_eq!(before["private"]["intent"]["state"],"cancelled");assert!(before["private"]["intent"]["started_at"].is_null());
    // The native close never reads original admission. It succeeds while the
    // stale worker is deterministically still before its fresh binding/user gate.
    assert_eq!(close(&f,&proof,200).await["disabled"],true);
    release_tx.send(()).unwrap();tokio::time::timeout(Duration::from_secs(5),barrier).await.unwrap().unwrap();
    let failure=tokio::time::timeout(Duration::from_secs(8),worker).await.unwrap().unwrap().unwrap_err();assert_eq!(failure.status.as_u16(),403);assert!(failure.detail.contains("binding")||failure.detail.contains("active"));
    assert_eq!(private_snapshot(&f,&session).await,before);assert_no_source_or_alias(&f,&chapter).await;
    let db=f.state.db.client().await.unwrap();let row=db.query_one("SELECT disabled,(SELECT count(*) FROM platform.sessions WHERE user_id=$1 AND revoked_at IS NULL),(SELECT count(*) FROM platform.protocol_identity_bindings WHERE user_id=$1 AND revoked_at IS NULL),(SELECT count(*) FROM platform.account_lifecycle_audit WHERE user_id=$1 AND operation='disable') FROM platform.users WHERE id=$1",&[&OWNER]).await.unwrap();assert!(row.get::<_,bool>(0));assert_eq!(row.get::<_,i64>(1),0);assert_eq!(row.get::<_,i64>(2),0);assert_eq!(row.get::<_,i64>(3),1);drop(db);
    f.repository.publisher_permit(id(1)).await.unwrap();
    assert!(!weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap());
}
fn proof_for_cancel(url:&str)->String {proof(1,11,url,"DELETE",b"")}

#[tokio::test]
#[ignore="Owned isolated fixture and original loopback node; exact test only, coordinated heavy slot"]
async fn unknown_scheduled_command_blocks_closure_without_replay_or_media_loss() {
    let f=fixture_options(Config::default(),true).await;let proof=prepare_closure(&f).await;
    let (public_session,session,_,_)=scheduled_upload(&f,&replacement_png(),2).await;
    f.state.db.client().await.unwrap().batch_execute("CREATE FUNCTION public.closure_scheduled_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.kind=1001 THEN RAISE EXCEPTION 'synthetic original admission failure' USING ERRCODE='58000'; END IF; RETURN NEW; END $$; CREATE TRIGGER closure_scheduled_fault BEFORE INSERT ON public.events FOR EACH ROW EXECUTE FUNCTION public.closure_scheduled_fault()").await.unwrap();
    await_schedule_due(&f,&session).await;assert_eq!(weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap_err().status.as_u16(),503);
    f.state.db.client().await.unwrap().batch_execute("DROP TRIGGER closure_scheduled_fault ON public.events; DROP FUNCTION public.closure_scheduled_fault()").await.unwrap();
    let before=private_snapshot(&f,&session).await;assert_eq!(before["private"]["intent"]["state"],"blocked");assert!(!before["private"]["intent"]["started_at"].is_null());assert!(!before["media"]["pins"].as_array().unwrap().is_empty());
    let view=closure_http(&f,"GET",&format!("/api/account/closure/{OWNER}"),None,200).await;for kind in ["scheduled-work","upload-active","publication-unresolved"] {assert!(view["blockers"].as_array().unwrap().iter().any(|b|b["kind"]==kind));}
    close(&f,&proof,409).await;unchanged_authority(&f,&proof).await;
    assert_eq!(private_snapshot(&f,&session).await,before);
    assert!(!weebdex_platform::public_api::scheduled_uploads::run_due(&f.state).await.unwrap());
    f.call(Some(1),"DELETE",&format!("/api/v1/upload/{public_session}"),None,409).await;
    domains::uploads::recover_storage(&f.state).await;assert_eq!(private_snapshot(&f,&session).await,before);
    let db=f.state.db.client().await.unwrap();let row=db.query_one("SELECT c.state,c.scheduled_session_id,c.native_commit_hash=i.payload_hash,c.native_commit_payload=i.payload,EXISTS(SELECT 1 FROM public.events e WHERE e.id=decode(c.event_id,'hex')) FROM platform.protocol_publication_commands c JOIN platform.scheduled_uploads i ON i.session_id=c.scheduled_session_id WHERE c.upload_session_id=$1",&[&session]).await.unwrap();assert_eq!(row.get::<_,String>(0),"prepared");assert_eq!(row.get::<_,String>(1),session);assert!(row.get::<_,bool>(2)&&row.get::<_,bool>(3));assert!(!row.get::<_,bool>(4));drop(db);
    original_get(&f,&hex::encode(Sha256::digest(replacement_png())),reqwest::Method::GET,true,404).await;
}
