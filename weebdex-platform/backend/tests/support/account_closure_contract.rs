//! Additional synthetic boundary controls inside the existing ignored owned suite.
//! Source only until the integrator admits its exact isolated fixture run.
use super::*;

async fn unchanged(state:&AppState,user:&str,version:i32,challenge:Option<&str>) {
    let db=state.db.client().await.unwrap();
    let row=db.query_one("SELECT disabled,version,(SELECT count(*) FROM platform.account_lifecycle_audit WHERE user_id=$1),(SELECT count(*) FROM platform.sessions WHERE user_id=$1 AND revoked_at IS NULL),(SELECT count(*) FROM platform.protocol_identity_bindings WHERE user_id=$1 AND revoked_at IS NULL),(SELECT count(*) FROM platform.account_disable_confirmations WHERE user_id=$1) FROM platform.users WHERE id=$1",&[&user]).await.unwrap();
    assert!(!row.get::<_,bool>(0));assert_eq!(row.get::<_,i32>(1),version);
    assert_eq!(row.get::<_,i64>(2),0);assert_eq!(row.get::<_,i64>(3),1);assert_eq!(row.get::<_,i64>(4),1);
    assert_eq!(row.get::<_,i64>(5),if challenge.is_some(){1}else{0});
    if let Some(challenge)=challenge {assert!(db.query_one("SELECT used_at IS NULL FROM platform.account_disable_confirmations WHERE id=$1 AND user_id=$2",&[&challenge,&user]).await.unwrap().get::<_,bool>(0));}
}

pub(super) async fn revision_headroom(state:&AppState,original:&Repository,client:&reqwest::Client,origin:&str) {
    use weebdex_platform::public_api::{PostgresPublicIdResolver,PublicIdResolver,ResourceKind};
    for (key,signed_mode) in [(5_u8,false),(6_u8,true)] {
        let user=format!("ClosureBound{key}");let token=format!("closure-bound-token-{key}");let session=format!("closure-bound-session-{key}");
        original.set_account_admission(id(key),true).await.unwrap();
        let db=state.db.client().await.unwrap();
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,version) VALUES($1,$1,$2,'synthetic-unused',TRUE,$3)",&[&user,&format!("closure-bound-{key}@fixture.invalid"),&(i32::MAX-1)]).await.unwrap();
        db.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",&[&public(key),&user]).await.unwrap();
        db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES($1,$2,$3,clock_timestamp()+interval '1 hour')",&[&session,&user,&Sha256::digest(token.as_bytes()).to_vec()]).await.unwrap();
        let lifecycle:Value=json!(db.query_one("SELECT account_lifecycle_id::text FROM platform.users WHERE id=$1",&[&user]).await.unwrap().get::<_,String>(0));drop(db);
        let alias=PostgresPublicIdResolver::new(state.db.clone()).preferred(ResourceKind::User,&user).await.unwrap().unwrap();
        let native_preview=format!("/api/account/closure/{user}");let signed_preview=format!("/api/v1/nostr/user/{alias}/closure?expectedAccountId={alias}");let signed_prepare=format!("/api/v1/user/{alias}");
        for version in [i32::MAX-1,i32::MAX] {
            state.db.client().await.unwrap().execute("UPDATE platform.users SET version=$2 WHERE id=$1",&[&user,&version]).await.unwrap();
            let n=native(client,origin,Some(&token),"GET",&native_preview,None,200).await;
            let s=call(client,origin,"GET",&signed_preview,None,Some(key),200).await;
            assert_eq!(n["version"],version);assert_eq!(s["version"],version);assert_eq!(n["lifecycleId"],s["lifecycleId"]);
            let refused=native(client,origin,Some(&token),"POST","/api/account/closure/prepare",Some(capture(&user,version,&lifecycle)),400).await;
            assert!(refused["error"].as_str().unwrap().contains("reactivation"));
            call(client,origin,"DELETE",&signed_prepare,Some(capture(alias.as_str(),version,&lifecycle)),Some(key),400).await;
            unchanged(state,&user,version,None).await;
        }
        // Only synthetic fixture setup lowers the test revision. Production never does.
        state.db.client().await.unwrap().execute("UPDATE platform.users SET version=$2 WHERE id=$1",&[&user,&(i32::MAX-2)]).await.unwrap();
        let account=if signed_mode {alias.as_str()} else {user.as_str()};
        let proof=if signed_mode {call(client,origin,"DELETE",&signed_prepare,Some(capture(account,i32::MAX-2,&lifecycle)),Some(key),200).await}
            else {native(client,origin,Some(&token),"POST","/api/account/closure/prepare",Some(capture(account,i32::MAX-2,&lifecycle)),200).await};
        assert_eq!(proof["version"],i32::MAX-2);assert_eq!(proof["result"],"prepared");
        let challenge=proof["confirmationId"].as_str().unwrap();let signed_confirm=format!("/api/v1/user/delete/{challenge}");
        for version in [i32::MAX-1,i32::MAX] {
            let mut command=confirm(account,&proof);command["capture"]["version"]=json!(version);
            if signed_mode {call(client,origin,"POST",&signed_confirm,Some(command),Some(key),400).await;}
            else {native(client,origin,Some(&token),"POST","/api/account/closure/confirm",Some(command),400).await;}
            unchanged(state,&user,i32::MAX-2,Some(challenge)).await;
        }
        let receipt=if signed_mode {call(client,origin,"POST",&signed_confirm,Some(confirm(account,&proof)),Some(key),200).await}
            else {native(client,origin,Some(&token),"POST","/api/account/closure/confirm",Some(confirm(account,&proof)),200).await};
        assert_eq!(receipt["version"],i32::MAX-1);assert_eq!(receipt["disabled"],true);assert_eq!(receipt["lifecycleId"],lifecycle);
        native(client,origin,Some(&token),"GET",&native_preview,None,401).await;
        call(client,origin,"GET","/api/v1/user/me",None,Some(key),403).await;
        original.publisher_permit(id(key)).await.unwrap();
        // Exact existing-administrator recovery consumes the reserved final revision.
        let mut admin=state.db.client().await.unwrap();let tx=admin.transaction().await.unwrap();
        assert_eq!(tx.query_one("SELECT version FROM platform.users WHERE id=$1 AND disabled AND closed_at IS NOT NULL FOR UPDATE NOWAIT",&[&user]).await.unwrap().get::<_,i32>(0),i32::MAX-1);
        assert_eq!(tx.execute("UPDATE platform.users SET disabled=FALSE,closed_at=NULL,version=version+1,updated_at=clock_timestamp() WHERE id=$1 AND version=$2 AND disabled AND closed_at IS NOT NULL",&[&user,&(i32::MAX-1)]).await.unwrap(),1);
        tx.execute("INSERT INTO platform.account_lifecycle_audit(user_id,operation,authority,old_version,new_version) VALUES($1,'reactivate','fixture-existing-administrator:version-boundary',$2,$3)",&[&user,&(i32::MAX-1),&i32::MAX]).await.unwrap();tx.commit().await.unwrap();drop(admin);
        let db=state.db.client().await.unwrap();
        let audits=db.query("SELECT operation,old_version,new_version FROM platform.account_lifecycle_audit WHERE user_id=$1 ORDER BY id",&[&user]).await.unwrap();assert_eq!(audits.len(),2);
        assert_eq!(audits[0].get::<_,String>(0),"disable");assert_eq!(audits[0].get::<_,i32>(1),i32::MAX-2);assert_eq!(audits[0].get::<_,i32>(2),i32::MAX-1);
        assert_eq!(audits[1].get::<_,String>(0),"reactivate");assert_eq!(audits[1].get::<_,i32>(1),i32::MAX-1);assert_eq!(audits[1].get::<_,i32>(2),i32::MAX);
        assert_eq!(db.query_one("SELECT count(*) FROM platform.protocol_identity_bindings WHERE user_id=$1 AND revoked_at IS NULL",&[&user]).await.unwrap().get::<_,i64>(0),0);
        assert_eq!(db.query_one("SELECT count(*) FROM platform.sessions WHERE user_id=$1 AND revoked_at IS NULL",&[&user]).await.unwrap().get::<_,i64>(0),0);
        assert!(db.query_one("SELECT used_at IS NOT NULL FROM platform.account_disable_confirmations WHERE id=$1",&[&challenge]).await.unwrap().get::<_,bool>(0));
        let fresh_token=format!("closure-bound-fresh-token-{key}");db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES($1,$2,$3,clock_timestamp()+interval '1 hour')",&[&format!("closure-bound-fresh-session-{key}"),&user,&Sha256::digest(fresh_token.as_bytes()).to_vec()]).await.unwrap();drop(db);
        let final_view=native(client,origin,Some(&fresh_token),"GET",&native_preview,None,200).await;assert_eq!(final_view["version"],i32::MAX);
        native(client,origin,Some(&fresh_token),"POST","/api/account/closure/prepare",Some(capture(&user,i32::MAX,&lifecycle)),400).await;
        let db=state.db.client().await.unwrap();let row=db.query_one("SELECT version,disabled,(SELECT count(*) FROM platform.account_lifecycle_audit WHERE user_id=$1),(SELECT count(*) FROM platform.account_disable_confirmations WHERE user_id=$1) FROM platform.users WHERE id=$1",&[&user]).await.unwrap();
        assert_eq!(row.get::<_,i32>(0),i32::MAX);assert!(!row.get::<_,bool>(1));assert_eq!(row.get::<_,i64>(2),2);assert_eq!(row.get::<_,i64>(3),1);
    }
}
