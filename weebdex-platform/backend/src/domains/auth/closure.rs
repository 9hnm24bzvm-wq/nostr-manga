//! Reversible instance disable. No purge, source signing, credential generation,
//! automatic reactivation, or independently committed publication is allowed here.
use super::{crypto, principal, SessionPrincipal};
use crate::{error::{ApiError,ApiResult},state::AppState};
use axum::{Json,Router,extract::{Path,State},http::{HeaderMap,HeaderValue,header},response::{IntoResponse,Response},routing::{get,post}};
use serde::Deserialize;
use serde_json::{Value,json};
use tokio_postgres::Transaction;

// Six independently bounded families, with no omission/truncation on overflow.
const MAX_BLOCKERS_PER_KIND:usize=100;
const MAX_BLOCKERS:usize=6*MAX_BLOCKERS_PER_KIND;
const MAX_RESPONSE_BYTES:usize=512*1024;
// Disable consumes one revision; existing administrator recovery consumes another.
const MAX_DISABLE_VERSION:i32=i32::MAX-2;
fn response_limit()->ApiError {
    ApiError::conflict("Account-disable response exceeds its bounded contract; contact the instance administrator")
}
fn blocker_kind(kind:&str,id:&str,action:&str,href:&str)->ApiResult<usize> {
    let (index,characters,ascii,expected_href)=match kind {
        "sole-group-leader"=>(0,128,false,"/following/group"),
        "publication-unresolved"=>(1,200,false,"/upload"),
        "source-request-unresolved"=>(2,10,true,"/following/manga"),
        "upload-active"=>(3,10,true,"/upload"),
        "draft-in-review"=>(4,128,false,"/drafts/title"),
        "scheduled-work"=>(5,10,true,"/upload"),
        _=>return Err(response_limit()),
    };
    // PostgreSQL length(text) counts Unicode scalar characters, not UTF-8 bytes.
    if id.is_empty() || id.contains('\0') || id.len()>characters*4 || id.chars().take(characters+1).count()>characters
        || (ascii&&(id.len()!=10||!id.bytes().all(|b|b.is_ascii_alphanumeric())))
        || kind.len()>25 || action.is_empty() || action.len()>116 || href.len()>16 || href!=expected_href {
        return Err(response_limit());
    }
    Ok(index)
}
fn blocker(kind:&str,id:String,action:&str,href:&str)->ApiResult<Value> {
    blocker_kind(kind,&id,action,href)?;
    Ok(json!({"kind":kind,"id":id,"action":action,"href":href}))
}
/// Count the WHOLE serialized JSON without allocating a second unbounded buffer.
/// Every caller checks this before committing, including prepare/confirm writes.
fn bounded_response(body:Value)->ApiResult<Value> {
    if let Some(blockers)=body.get("blockers") {
        let rows=blockers.as_array().ok_or_else(response_limit)?;
        if rows.len()>MAX_BLOCKERS {return Err(response_limit());}
        let mut counts=[0;6];
        for row in rows {
            let field=|name:&str|row.get(name).and_then(Value::as_str).ok_or_else(response_limit);
            let index=blocker_kind(field("kind")?,field("id")?,field("action")?,field("href")?)?;
            counts[index]+=1;
            if counts[index]>MAX_BLOCKERS_PER_KIND {return Err(response_limit());}
        }
    }
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self,bytes:&[u8])->std::io::Result<usize> {
            if bytes.len()>MAX_RESPONSE_BYTES-self.0 {return Err(std::io::Error::other("Account-disable response limit"));}
            self.0+=bytes.len();Ok(bytes.len())
        }
        fn flush(&mut self)->std::io::Result<()> {Ok(())}
    }
    serde_json::to_writer(Counter(0),&body).map_err(|_|response_limit())?;
    Ok(body)
}

#[derive(Clone,Debug,Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub(crate) struct Capture {
    pub expected_account_id:String,
    pub expected_user_id:String,
    pub expected_lifecycle_id:String,
    pub version:i32,
}
impl Capture {
    pub(crate) fn validate(&self)->ApiResult<()> {
        if self.version>MAX_DISABLE_VERSION {
            return Err(ApiError::bad_request("Account revision has no room for reversible disable and administrator reactivation; contact the instance administrator"));
        }
        if self.version<1 || self.expected_account_id.is_empty()
            || crate::public_api::id::PublicUuid::parse(&self.expected_lifecycle_id).is_err()
            || self.expected_account_id.len()>128 || self.expected_account_id!=self.expected_user_id {
            return Err(ApiError::bad_request("Capture the exact current account, user and revision before disabling"));
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub(crate) struct Confirm {
    pub capture:Capture,
    pub confirmation_id:String,
    pub confirmation_token:String,
    pub acknowledge:String,
}
pub(crate) const ACK:&str="DISABLE THIS ACCOUNT";
pub(crate) fn private(body:Value)->Response {
    let mut response=Json(body).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL,HeaderValue::from_static("private, no-store"));
    response.headers_mut().insert(header::REFERRER_POLICY,HeaderValue::from_static("no-referrer"));
    response.headers_mut().append(header::VARY,HeaderValue::from_static("Authorization, Cookie"));
    response
}
pub fn router()->Router<AppState> {
    Router::new().route("/account/closure/{id}",get(native_snapshot))
        .route("/account/closure/prepare",post(native_prepare))
        .route("/account/closure/confirm",post(native_confirm))
}
/// Compatible with an outer signed writer's NO KEY UPDATE; closure's exceptional
/// UPDATE conflicts. Call before external effects, or within the same rollback
/// transaction as every mutation. A cached SessionPrincipal is not this fence.
pub(crate) async fn active_in(tx:&Transaction<'_>,user:&str)->ApiResult<()> {
    tx.query_one("SELECT platform.require_active_account($1)",&[&user]).await.map_err(lock_error)?;
    Ok(())
}
fn lock_error(error:tokio_postgres::Error)->ApiError {
    if error.code().is_some_and(|code|matches!(code.code(),"55P03"|"40P01"|"40001")) {
        ApiError::conflict("Account activity changed or is still running. Nothing was disabled; refresh and retry deliberately")
    } else if error.code().is_some_and(|code|code.code()=="42501") {
        ApiError::forbidden("This account is no longer active")
    } else {error.into()}
}
/// Binding -> account. NOWAIT makes upgrades and an existing reverse holder
/// refuse, not deadlock. Closure must never invoke a second source transaction.
pub(crate) async fn lock_account(tx:&Transaction<'_>,user:&str)->ApiResult<(String,i32)> {
    let bindings=tx.query("SELECT publisher FROM platform.protocol_identity_bindings WHERE user_id=$1 ORDER BY publisher LIMIT 1001 FOR UPDATE NOWAIT",&[&user]).await.map_err(lock_error)?;
    if bindings.len()>1000 {return Err(ApiError::conflict("Too many account bindings; contact the instance administrator"));}
    let row=tx.query_opt("SELECT username,version FROM platform.users WHERE id=$1 AND NOT disabled FOR UPDATE NOWAIT",&[&user]).await.map_err(lock_error)?.ok_or_else(ApiError::unauthorized)?;
    Ok((row.get(0),row.get(1)))
}
async fn native_authority(tx:&Transaction<'_>,actor:&SessionPrincipal,recent:bool)->ApiResult<()> {
    actor.require_browser_session()?;
    if recent {actor.require_recent_browser_auth()?;}
    let row=tx.query_opt("SELECT s.purpose='normal',s.authenticated_at>clock_timestamp()-interval '10 minutes' AND s.authenticated_at<=clock_timestamp() FROM platform.sessions s WHERE s.id=$1 AND s.user_id=$2 AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp() FOR SHARE NOWAIT",&[&actor.session_id,&actor.user_id]).await.map_err(lock_error)?.ok_or_else(ApiError::unauthorized)?;
    if !row.get::<_,bool>(0) || (recent&&!row.get::<_,bool>(1)) {
        return Err(ApiError::forbidden("Sign in again with your normal account before preparing or confirming account disable"));
    }
    Ok(())
}
fn native_capture(actor:&SessionPrincipal,capture:&Capture)->ApiResult<()> {
    capture.validate()?;
    if capture.expected_account_id!=actor.user_id {return Err(ApiError::conflict("The selected native account changed"));}
    Ok(())
}
async fn native_snapshot(State(state):State<AppState>,Path(id):Path<String>,headers:HeaderMap)->ApiResult<Response> {
    let actor=principal::require_principal(&state,&headers).await?;
    actor.require_browser_session()?;
    if actor.user_id!=id {return Err(ApiError::forbidden("Select your own native account"));}
    let mut db=state.db.client().await?;let tx=db.transaction().await?;
    let (name,version)=lock_account(&tx,&actor.user_id).await?;
    native_authority(&tx,&actor,false).await?;
    let result=snapshot_in(&tx,&actor.user_id,&id,&name,version).await?;
    tx.commit().await?;Ok(private(result))
}
async fn native_prepare(State(state):State<AppState>,headers:HeaderMap,Json(capture):Json<Capture>)->ApiResult<Response> {
    principal::validate_mutation(&state,&headers)?;
    let actor=principal::require_principal(&state,&headers).await?;native_capture(&actor,&capture)?;
    let mut db=state.db.client().await?;let tx=db.transaction().await?;
    let (_,version)=lock_account(&tx,&actor.user_id).await?;native_authority(&tx,&actor,true).await?;
    let result=prepare_in(&tx,&actor.user_id,&format!("session:{}",actor.session_id),&capture,version).await?;
    tx.commit().await?;Ok(private(result))
}
async fn native_confirm(State(state):State<AppState>,headers:HeaderMap,Json(command):Json<Confirm>)->ApiResult<Response> {
    principal::validate_mutation(&state,&headers)?;
    let actor=principal::require_principal(&state,&headers).await?;native_capture(&actor,&command.capture)?;
    let mut db=state.db.client().await?;let tx=db.transaction().await?;
    let (_,version)=lock_account(&tx,&actor.user_id).await?;native_authority(&tx,&actor,true).await?;
    let result=confirm_in(&tx,&actor.user_id,&format!("session:{}",actor.session_id),&command,version).await?;
    tx.commit().await?;Ok(private(result))
}
async fn lifecycle_in(tx:&Transaction<'_>,user:&str)->ApiResult<String> {
    Ok(tx.query_one("SELECT account_lifecycle_id::text FROM platform.users WHERE id=$1",&[&user]).await?.get(0))
}
pub(crate) async fn snapshot_in(tx:&Transaction<'_>,user:&str,account:&str,name:&str,version:i32)->ApiResult<Value> {
    let lifecycle_id=lifecycle_in(tx,user).await?;
    bounded_response(json!({"result":"ok","scope":"instance-account-disable","accountId":account,"userId":account,"username":name,"version":version,"lifecycleId":lifecycle_id,"effect":"reversible-disable","reactivation":"instance-administrator","automaticPurge":false,"blockers":blockers_in(tx,user).await?}))
}
/// Account UPDATE prevents new grants/commands. Group rows prevent concurrent
/// removal of an alternate leader; alternate user locks prevent mutual closure.
async fn blockers_in(tx:&Transaction<'_>,user:&str)->ApiResult<Vec<Value>> {
    let mut out=Vec::new();
    let groups=tx.query("SELECT g.id FROM platform.catalog_groups g JOIN platform.catalog_group_members gm ON gm.group_id=g.id WHERE gm.user_id=$1 AND gm.is_leader AND g.deleted_at IS NULL ORDER BY g.id LIMIT 101 FOR UPDATE OF g NOWAIT",&[&user]).await.map_err(lock_error)?;
    if groups.len()>MAX_BLOCKERS_PER_KIND {return Err(ApiError::conflict("More than 100 led groups require administrator-assisted handoff"));}
    for group in groups {
        let id:String=group.get(0);
        let alternatives=tx.query("SELECT u.id FROM platform.catalog_group_members gm JOIN platform.users u ON u.id=gm.user_id WHERE gm.group_id=$1 AND gm.is_leader AND u.id<>$2 AND NOT u.disabled AND u.verified AND NOT (u.roles ?| ARRAY['banned','restricted']) ORDER BY u.id LIMIT 101 FOR SHARE OF u NOWAIT",&[&id,&user]).await.map_err(lock_error)?;
        if alternatives.is_empty() {out.push(blocker("sole-group-leader",id,"Hand leadership to an active verified member before disabling","/following/group")?);}
    }
    for (sql,kind,action,href) in [
        ("SELECT command_id FROM platform.protocol_publication_commands WHERE user_id=$1 AND state<>'materialized' ORDER BY command_id LIMIT 101","publication-unresolved","Reconcile the exact existing publication with the instance administrator; do not resend or discard it","/upload"),
        ("SELECT chapter_id FROM platform.protocol_deletion_intents WHERE actor_user_id=$1 AND state='prepared' ORDER BY chapter_id LIMIT 101","source-request-unresolved","Ask the instance administrator to reconcile this prepared source request; do not replay it","/following/manga"),
        ("SELECT id FROM platform.upload_sessions WHERE owner_id=$1 AND state NOT IN ('committed','deleted') ORDER BY id LIMIT 101","upload-active","Finish or explicitly cancel an unstarted upload; started or uncertain publication needs administrator reconciliation","/upload"),
        ("SELECT id FROM platform.catalog_manga WHERE owner_id=$1 AND state='submitted' AND deleted_at IS NULL ORDER BY id LIMIT 101","draft-in-review","Resolve or withdraw the submitted draft before disabling; its source intent must remain intact","/drafts/title"),
    ] {
        let rows=tx.query(sql,&[&user]).await?;
        if rows.len()>MAX_BLOCKERS_PER_KIND {return Err(ApiError::conflict("Account has more than 100 unresolved work items; contact the instance administrator"));}
        for row in rows {out.push(blocker(kind,row.get(0),action,href)?);}
    }
    // Scheduling migration may be integrated independently. No prepared-source
    // intent is ever treated as cancelled because a session looks terminal.
    if tx.query_one("SELECT to_regclass('platform.scheduled_uploads') IS NOT NULL",&[]).await?.get::<_,bool>(0) {
        let rows=tx.query("SELECT session_id FROM platform.scheduled_uploads WHERE owner_id=$1 AND state NOT IN ('complete','cancelled') ORDER BY session_id LIMIT 101",&[&user]).await?;
        if rows.len()>MAX_BLOCKERS_PER_KIND {return Err(ApiError::conflict("Too many unresolved schedules; contact the instance administrator"));}
        for row in rows {out.push(blocker("scheduled-work",row.get(0),"Cancel only an unstarted schedule, or ask the administrator to reconcile started or uncertain work","/upload")?);}
    }
    Ok(out)
}
pub(crate) async fn prepare_in(tx:&Transaction<'_>,user:&str,authority:&str,capture:&Capture,version:i32)->ApiResult<Value> {
    capture.validate()?;
    let lifecycle_id=lifecycle_in(tx,user).await?;
    if capture.version!=version || capture.expected_lifecycle_id!=lifecycle_id {return Err(ApiError::conflict("Account changed; load its current revision again"));}
    let blockers=blockers_in(tx,user).await?;
    if !blockers.is_empty() {return bounded_response(json!({"result":"blocked","scope":"instance-account-disable","accountId":capture.expected_account_id,"userId":capture.expected_user_id,"version":version,"blockers":blockers}));}
    // Bound challenge churn and outstanding rows. Never persist the raw token.
    let recent:i64=tx.query_one("SELECT count(*) FROM platform.account_disable_confirmations WHERE user_id=$1 AND created_at>clock_timestamp()-interval '1 hour'",&[&user]).await?.get(0);
    if recent>=10 {return Err(ApiError::busy());}
    tx.execute("UPDATE platform.account_disable_confirmations SET used_at=clock_timestamp() WHERE user_id=$1 AND used_at IS NULL",&[&user]).await?;
    let id=crypto::id();let token=crypto::token();let hash=crypto::digest(&token);
    tx.execute("INSERT INTO platform.account_disable_confirmations(id,token_hash,user_id,authority,account_id,public_user_id,user_version) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&id,&hash,&user,&authority,&capture.expected_account_id,&capture.expected_user_id,&version]).await?;
    bounded_response(json!({"result":"prepared","scope":"instance-account-disable","effect":"reversible-disable","accountId":capture.expected_account_id,"userId":capture.expected_user_id,"version":version,"lifecycleId":lifecycle_id,"confirmationId":id,"confirmationToken":token,"expiresInSeconds":600,"acknowledge":ACK,"reactivation":"instance-administrator","automaticPurge":false}))
}
pub(crate) async fn confirm_in(tx:&Transaction<'_>,user:&str,authority:&str,command:&Confirm,version:i32)->ApiResult<Value> {
    command.capture.validate()?;
    let lifecycle_id=lifecycle_in(tx,user).await?;
    if command.acknowledge!=ACK || command.confirmation_id.len()!=36 || command.confirmation_token.len()!=43 || version!=command.capture.version || command.capture.expected_lifecycle_id!=lifecycle_id {
        return Err(ApiError::conflict("The exact fresh account-disable confirmation is required"));
    }
    let hash=crypto::digest(&command.confirmation_token);
    let row=tx.query_opt("SELECT token_hash FROM platform.account_disable_confirmations WHERE id=$1 AND user_id=$2 AND authority=$3 AND account_id=$4 AND public_user_id=$5 AND user_version=$6 AND used_at IS NULL AND expires_at>clock_timestamp() FOR UPDATE NOWAIT",&[&command.confirmation_id,&user,&authority,&command.capture.expected_account_id,&command.capture.expected_user_id,&version]).await.map_err(lock_error)?.ok_or_else(||ApiError::conflict("Account-disable confirmation expired, changed or was already used"))?;
    if !crypto::constant_time_equal(&hash,&row.get::<_,Vec<u8>>(0)) {return Err(ApiError::forbidden("Invalid account-disable confirmation"));}
    if !blockers_in(tx,user).await?.is_empty() {return Err(ApiError::conflict("New shared ownership or unresolved work blocks disabling. Reload the account blockers"));}
    // Auth flows elsewhere may acquire flow -> user. Never wait on their rows
    // while holding closure's user lock. New credential issuance requires user.
    for sql in [
        "SELECT f.id FROM platform.auth_flows f WHERE f.session_id IN (SELECT id FROM platform.sessions WHERE user_id=$1) OR f.id IN (SELECT flow_id FROM platform.auth_email_tokens WHERE user_id=$1) ORDER BY f.id LIMIT 10001 FOR UPDATE OF f NOWAIT",
        "SELECT id FROM platform.sessions WHERE user_id=$1 ORDER BY id LIMIT 10001 FOR UPDATE NOWAIT",
        "SELECT id FROM platform.api_clients WHERE user_id=$1 ORDER BY id LIMIT 10001 FOR UPDATE NOWAIT",
        "SELECT id FROM platform.auth_email_tokens WHERE user_id=$1 ORDER BY id LIMIT 10001 FOR UPDATE NOWAIT",
        "SELECT user_id FROM platform.native_identity_links WHERE user_id=$1 FOR UPDATE NOWAIT",
        "SELECT id FROM platform.native_identity_link_challenges WHERE user_id=$1 ORDER BY id LIMIT 10001 FOR UPDATE NOWAIT",
    ] {
        if tx.query(sql,&[&user]).await.map_err(lock_error)?.len()>10000 {return Err(ApiError::conflict("Account credential history requires administrator-assisted disable"));}
    }
    crate::domains::publication_access::revoke_for_account_closure_in(tx,user,&command.confirmation_id).await?;
    tx.execute("UPDATE platform.auth_flows SET used_at=COALESCE(used_at,clock_timestamp()) WHERE session_id IN (SELECT id FROM platform.sessions WHERE user_id=$1) OR id IN (SELECT flow_id FROM platform.auth_email_tokens WHERE user_id=$1)",&[&user]).await?;
    tx.execute("UPDATE platform.sessions SET revoked_at=COALESCE(revoked_at,clock_timestamp()) WHERE user_id=$1",&[&user]).await?;
    tx.execute("UPDATE platform.api_clients SET revoked_at=clock_timestamp(),updated_at=clock_timestamp(),version=version+1 WHERE user_id=$1 AND revoked_at IS NULL",&[&user]).await?;
    tx.execute("UPDATE platform.auth_email_tokens SET used_at=COALESCE(used_at,clock_timestamp()) WHERE user_id=$1",&[&user]).await?;
    tx.execute("UPDATE platform.protocol_identity_bindings SET revoked_at=COALESCE(revoked_at,clock_timestamp()) WHERE user_id=$1",&[&user]).await?;
    // Private portable association is separate from publisher authority. Closure
    // clears it in the same rollback transaction; reactivation cannot revive it.
    tx.execute("UPDATE platform.native_identity_links SET public_key=NULL,revision=revision+1,updated_at=clock_timestamp() WHERE user_id=$1",&[&user]).await?;
    tx.execute("UPDATE platform.native_identity_link_challenges SET used_at=clock_timestamp(),outcome='account-closed' WHERE user_id=$1 AND used_at IS NULL",&[&user]).await?;
    tx.execute("UPDATE platform.account_disable_confirmations SET used_at=COALESCE(used_at,clock_timestamp()) WHERE user_id=$1",&[&user]).await?;
    tx.execute("INSERT INTO platform.account_lifecycle_audit(user_id,confirmation_id,operation,authority,old_version,new_version) VALUES($1,$2,'disable',$3,$4,$5)",&[&user,&command.confirmation_id,&authority,&version,&(version+1)]).await?;
    tx.execute("UPDATE platform.users SET disabled=true,closed_at=clock_timestamp(),version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&user]).await?;
    bounded_response(json!({"result":"ok","scope":"instance-account-disable","effect":"reversible-disable","accountId":command.capture.expected_account_id,"userId":command.capture.expected_user_id,"version":version+1,"lifecycleId":lifecycle_id,"confirmationId":command.confirmation_id,"disabled":true,"reactivation":"instance-administrator","automaticPurge":false}))
}
#[cfg(test)] mod tests {
    use super::*;
    fn full_blockers(character:&str)->Vec<Value> {
        [("sole-group-leader",128,"/following/group"),("publication-unresolved",200,"/upload"),
         ("source-request-unresolved",10,"/following/manga"),("upload-active",10,"/upload"),
         ("draft-in-review",128,"/drafts/title"),("scheduled-work",10,"/upload")]
        .into_iter().flat_map(|(kind,width,href)| {
            let id=if width==10 {"A".repeat(10)} else {character.repeat(width)};
            (0..100).map(move |_|blocker(kind,id.clone(),&"A".repeat(116),href).unwrap())
        }).collect()
    }
    #[test] fn closure_response_retains_all_six_families_and_exact_character_widths() {
        for character in ["😀","\u{1}","\"","\\"] {
            let rows=full_blockers(character);
            let body=json!({"result":"ok","blockers":rows});
            assert_eq!(bounded_response(body.clone()).unwrap(),body);
            assert_eq!(body["blockers"].as_array().unwrap().len(),600);
        }
        for (kind,width,href) in [("sole-group-leader",128,"/following/group"),("draft-in-review",128,"/drafts/title"),("publication-unresolved",200,"/upload")] {
            assert!(blocker(kind,"😀".repeat(width),"Resolve",href).is_ok());
            assert!(blocker(kind,"😀".repeat(width+1),"Resolve",href).is_err());
        }
        for (kind,href) in [("source-request-unresolved","/following/manga"),("upload-active","/upload"),("scheduled-work","/upload")] {
            assert!(blocker(kind,"A1".repeat(5),"Resolve",href).is_ok());
            for id in ["A".repeat(9),"A".repeat(11),"é".repeat(10),"_".repeat(10)] {
                assert!(blocker(kind,id,"Resolve",href).is_err());
            }
        }
    }
    #[test] fn closure_response_refuses_count_field_and_whole_envelope_overflow() {
        let mut rows=full_blockers("x");rows.push(rows[0].clone());
        assert!(bounded_response(json!({"blockers":rows})).is_err());
        assert!(bounded_response(json!({"blockers":vec![full_blockers("x")[0].clone();101]})).is_err());
        for (kind,action,href) in [("unknown","Resolve","/upload"),("upload-active","Resolve","/following/group"),("upload-active","","/upload")] {
            assert!(blocker(kind,"A".repeat(10),action,href).is_err());
        }
        assert!(blocker("upload-active","A".repeat(10),&"A".repeat(117),"/upload").is_err());
        assert!(blocker("upload-active","A".repeat(10),&"é".repeat(59),"/upload").is_err());
        // Unknown envelope fields count too; exact boundary accepted, +1 refused.
        let overhead=serde_json::to_vec(&json!({"padding":""})).unwrap().len();
        let exact=json!({"padding":"x".repeat(MAX_RESPONSE_BYTES-overhead)});
        assert_eq!(serde_json::to_vec(&exact).unwrap().len(),MAX_RESPONSE_BYTES);
        assert!(bounded_response(exact).is_ok());
        assert!(bounded_response(json!({"padding":"x".repeat(MAX_RESPONSE_BYTES-overhead+1)})).is_err());
        for result in ["ok","prepared","blocked"] {
            assert!(bounded_response(json!({"result":result,"padding":"x".repeat(MAX_RESPONSE_BYTES)})).is_err());
        }
    }
    #[test] fn closure_capture_is_exact_and_acknowledgement_is_explicit() {
        let mut c=Capture{expected_account_id:"owner".into(),expected_user_id:"owner".into(),expected_lifecycle_id:"11111111-1111-4111-8111-111111111111".into(),version:1};assert!(c.validate().is_ok());
        c.expected_user_id="other".into();assert!(c.validate().is_err());c.expected_user_id="owner".into();
        for version in [0,-1,i32::MAX-1,i32::MAX] {c.version=version;assert!(c.validate().is_err());}
        assert!(serde_json::from_str::<Capture>(r#"{"expectedAccountId":"a","expectedUserId":"a","version":1,"version":2}"#).is_err());
        assert!(serde_json::from_str::<Capture>(r#"{"expectedAccountId":"a","expectedUserId":"a","version":1,"purge":true}"#).is_err());
        c.version=MAX_DISABLE_VERSION;assert!(c.validate().is_ok());
        assert_eq!(MAX_DISABLE_VERSION.checked_add(2),Some(i32::MAX));
        assert_eq!(ACK,"DISABLE THIS ACCOUNT");
    }
}

#[cfg(test)]
#[path="closure_native_tests.rs"]
mod native_fence_tests;
