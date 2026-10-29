//! Reasoned appeals and explicit, revision-fenced local inverses. No source writes.
use super::reports::{actor,digest,key,prior,receipt};
use crate::{AppState,domains::{auth::SessionPrincipal,catalog::validation as v},public_api::{PublicApiError as Error,PublicApiResult as Result,moderator_reports}};
use axum::{Json,extract::{State,Path},http::HeaderMap};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use tokio_postgres::Transaction;

fn authorize(a:&SessionPrincipal)->Result<()> {
    if !a.is_staff(){return Err(Error::forbidden("Staff correction authority required"));}
    a.require_permission("report.correct")?;Ok(())
}
fn reason(text:&str)->Result<&str>{let text=text.trim();if text.is_empty()||text.len()>4000||text.contains('\0'){return Err(Error::bad_request("Explain this action in 1–4000 bytes"));}Ok(text)}
fn conflict(error:tokio_postgres::Error)->Error{if error.code().is_some_and(|c|matches!(c.code(),"55P03"|"40P01"|"40001")){Error::conflict("This report or target is being changed. Reopen the review.")}else{error.into()}}
struct Inverse {context:Value,table:&'static str,target:String,version:i32,decision:i64,effect:Value,report_version:i32}
async fn inverse(tx:&Transaction<'_>,a:&SessionPrincipal,id:&str,write:bool)->Result<Inverse> {
    let lock=if write{"FOR UPDATE NOWAIT"}else{"FOR SHARE NOWAIT"};
    // Preserve title-before-report order used by all original removal decisions.
    let pre=tx.query_opt("SELECT entity_type,entity_id FROM platform.reports WHERE id=$1 AND deleted_at IS NULL",&[&id]).await?.ok_or_else(Error::not_found)?;
    let category:String=pre.get(0);let target:String=pre.get(1);v::id(&target)?;
    if category=="manga"{tx.query_opt(&format!("SELECT id FROM platform.catalog_manga WHERE id=$1 {lock}"),&[&target]).await.map_err(conflict)?.ok_or_else(Error::not_found)?;}
    let report=tx.query_opt(&format!("SELECT version,status,entity_type,entity_id FROM platform.reports WHERE id=$1 AND deleted_at IS NULL {lock}"),&[&id]).await.map_err(conflict)?.ok_or_else(Error::not_found)?;
    if report.get::<_,String>(1)!="accepted"||report.get::<_,String>(2)!=category||report.get::<_,String>(3)!=target{return Err(Error::conflict("Only the current accepted decision can have its local effect corrected"));}
    let report_version:i32=report.get(0);
    let decision=tx.query_opt("SELECT id,effect FROM platform.report_audit WHERE report_id=$1 AND operation='accept' ORDER BY id DESC LIMIT 1",&[&id]).await?.ok_or_else(||Error::conflict("No recorded accepted decision is available"))?;
    let decision_id:i64=decision.get(0);let effect:Value=decision.get(1);
    if tx.query_opt("SELECT decision_id FROM platform.report_corrections WHERE decision_id=$1",&[&decision_id]).await?.is_some(){return Err(Error::conflict("This decision was already corrected"));}
    let (table,grant,version_key,label)=match (category.as_str(),effect["action"].as_str()) {
        ("manga",Some("instance_soft_hide")) if effect["manga_id"]==target => ("platform.catalog_manga","manga.restore","new_version","title"),
        ("chapter",Some("instance_soft_hide")) if effect["chapter_id"]==target => ("platform.catalog_chapters","chapter.restore","new_version","COALESCE(title,'Chapter')"),
        ("scanlation_group",Some("grant_group_leadership")) if effect["group_id"]==target => ("platform.catalog_groups","group.leadership.revoke","new_group_version","name"),
        _=>return Err(Error::conflict("This decision has no supported automatic local inverse. Reopen it for a reasoned review; original-service effects need their own operator."))
    };
    a.require_permission(grant)?;
    let expected=effect[version_key].as_i64().and_then(|n|i32::try_from(n).ok()).filter(|n|*n>0&&*n<i32::MAX).ok_or_else(||Error::conflict("This older decision lacks a complete before/after record. A separately authorized current-state review is required; do not guess an inverse."))?;
    if category=="scanlation_group" && (!effect.as_object().is_some_and(|m|m.contains_key("previous_membership")) || !effect["new_member_name"].is_string()) || category!="scanlation_group" && effect["previous_deleted"]!=false {
        return Err(Error::conflict("This older decision lacks a safe restoration record. Ask an administrator for a separately authorized current-state review."));
    }
    let row=tx.query_opt(&format!("SELECT version,deleted_at IS NOT NULL,{label} FROM {table} WHERE id=$1 {lock}"),&[&target]).await.map_err(conflict)?.ok_or_else(Error::not_found)?;
    let current:i32=row.get(0);let deleted:bool=row.get(1);let name:String=row.get(2);
    if current!=expected || (category=="scanlation_group" && deleted) || (category!="scanlation_group" && !deleted){return Err(Error::conflict("The target changed after the decision. Its current state must be reviewed separately; the old decision will not be blindly undone."));}
    if tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.protocol_publication_commands WHERE catalog_id=$1 AND state IN ('prepared','source_accepted'))",&[&target]).await?.get::<_,bool>(0){return Err(Error::conflict("This target has an unresolved publication. Reconcile its original outcome before a correction."));}
    let inverse=if category=="scanlation_group" {
        if !tx.query_one("SELECT platform.protocol_group_is_visible($1)",&[&target]).await?.get::<_,bool>(0){return Err(Error::conflict("The original group source is unavailable. A local correction cannot override it."));}
        moderator_reports::correction_group_source_fence(tx,&target).await.map_err(|_|Error::conflict("The original group source is changing or unavailable; a local correction cannot override it."))?;
        let claimant=effect["claimant_id"].as_str().ok_or_else(Error::unavailable)?;v::id(claimant)?;
        let member=tx.query_opt(&format!("SELECT name,is_leader,is_officer FROM platform.catalog_group_members WHERE group_id=$1 AND user_id=$2 {lock}"),&[&target,&claimant]).await.map_err(conflict)?.ok_or_else(||Error::conflict("The recorded leadership membership changed"))?;
        if member.get::<_,String>(0)!=effect["new_member_name"].as_str().unwrap()||!member.get::<_,bool>(1)||member.get::<_,bool>(2){return Err(Error::conflict("The recorded leadership membership changed"));}
        if tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.catalog_group_members WHERE group_id=$1 AND is_leader AND user_id<>$2)",&[&target,&claimant]).await?.get::<_,bool>(0){return Err(Error::conflict("Group leadership changed after this decision"));}
        let previous=&effect["previous_membership"];
        if !previous.is_null() && (!previous["name"].is_string()||!previous["is_leader"].is_boolean()||!previous["is_officer"].is_boolean()||previous["is_leader"]!=false){return Err(Error::conflict("The recorded membership cannot be safely restored"));}
        json!({"action":"restore_previous_group_membership","groupId":target,"claimantId":claimant,"previousMembership":previous,"scope":"instance-local","originalSourceChanged":false})
    } else {
        let sources=if category=="manga" {
            let r=tx.query_one("SELECT protocol_series_id,protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id) FROM platform.catalog_manga WHERE id=$1",&[&target]).await?;
            if !r.get::<_,bool>(1){return Err(Error::conflict("The original source is withdrawn or unavailable. Restoring a local hide cannot make it available."));}vec![r.get::<_,Option<String>>(0)]
        }else {
            let r=tx.query_one("SELECT manga_id,protocol_event_id,platform.protocol_source_is_visible(protocol_event_id) FROM platform.catalog_chapters WHERE id=$1",&[&target]).await?;
            let parent:String=r.get(0);let p=tx.query_opt("SELECT protocol_series_id FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND state='published' AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id) FOR SHARE NOWAIT",&[&parent]).await.map_err(conflict)?.ok_or_else(||Error::conflict("The title is unavailable. Restore its local state separately before this chapter."))?;
            if !r.get::<_,bool>(2){return Err(Error::conflict("The chapter source is withdrawn or unavailable; a local correction cannot override it."));}vec![r.get::<_,Option<String>>(1),p.get::<_,Option<String>>(0)]
        };
        moderator_reports::correction_source_fences(tx,sources).await.map_err(|_|Error::conflict("The original source is changing or unavailable. A local correction cannot override source withdrawal."))?;
        json!({"action":"restore_local_visibility","category":category,"targetId":target,"scope":"instance-local","originalSourceChanged":false})
    };
    let mut context=json!({"reportId":id,"reportVersion":report_version,"decisionId":decision_id.to_string(),"targetId":target,"targetName":name,"targetVersion":current,"requiredGrant":grant,"effect":inverse,"previousEffect":effect});context["contextHash"]=json!(digest(&context)?);
    Ok(Inverse{context,table,target,version:current,decision:decision_id,effect,report_version})
}
pub(super) async fn preview(State(s):State<AppState>,Path(id):Path<String>,h:HeaderMap)->Result<Json<Value>> {
    v::id(&id)?;let mut db=s.db.client().await?;let tx=db.transaction().await?;let a=actor(&s,&h,&tx,false).await?;authorize(&a)?;
    let selected=inverse(&tx,&a,&id,false).await?;let result=json!({"scope":"native-correction-review","actorId":a.user_id,"context":selected.context});tx.commit().await?;Ok(Json(result))
}
#[derive(Deserialize,Serialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub(super) struct Correction {request_id:String,context_hash:String,reason:String}
pub(super) async fn correct(State(s):State<AppState>,Path(id):Path<String>,h:HeaderMap,Json(c):Json<Correction>)->Result<Json<Value>> {
    v::id(&id)?;key(&c.request_id)?;reason(&c.reason)?;let mut db=s.db.client().await?;let tx=db.transaction().await?;let a=actor(&s,&h,&tx,true).await?;authorize(&a)?;
    let hash=digest(&c)?;if let Some(result)=prior(&tx,&a.user_id,&id,&c.request_id,&hash).await?{tx.commit().await?;return Ok(Json(result));}
    let selected=inverse(&tx,&a,&id,true).await?;
    if selected.context["contextHash"]!=c.context_hash{return Err(Error::conflict("The correction review changed. Open a fresh review."));}
    if selected.effect["action"]=="grant_group_leadership" {
        let claimant=selected.effect["claimant_id"].as_str().unwrap();let previous=&selected.effect["previous_membership"];
        if previous.is_null(){tx.execute("DELETE FROM platform.catalog_group_members WHERE group_id=$1 AND user_id=$2",&[&selected.target,&claimant]).await?;}
        else {tx.execute("UPDATE platform.catalog_group_members SET name=$3,is_leader=$4,is_officer=$5 WHERE group_id=$1 AND user_id=$2",&[&selected.target,&claimant,&previous["name"].as_str().unwrap(),&previous["is_leader"].as_bool().unwrap(),&previous["is_officer"].as_bool().unwrap()]).await?;}
        tx.execute("UPDATE platform.catalog_groups SET version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&selected.target]).await?;
    } else {tx.execute(&format!("UPDATE {} SET deleted_at=NULL,version=version+1,updated_at=clock_timestamp() WHERE id=$1",selected.table),&[&selected.target]).await?;}
    let mut effect=selected.context["effect"].clone();effect["decisionId"]=json!(selected.decision.to_string());effect["oldTargetVersion"]=json!(selected.version);effect["targetVersion"]=json!(selected.version+1);
    let next=selected.report_version.checked_add(1).ok_or_else(||Error::conflict("Report revision exhausted"))?;
    tx.execute("UPDATE platform.reports SET status='refused',notes=$2,reviewed_by=$3,appeal_state=CASE WHEN appeal_state='pending' THEN 'reviewed' ELSE appeal_state END,version=$4,updated_at=clock_timestamp() WHERE id=$1",&[&id,&c.reason.trim(),&a.user_id,&next]).await?;
    let audit:i64=tx.query_one("INSERT INTO platform.report_audit(report_id,actor_id,operation,old_status,new_status,old_version,new_version,notes,effect) VALUES($1,$2,'correct','accepted','refused',$3,$4,$5,$6) RETURNING id",&[&id,&a.user_id,&selected.report_version,&next,&c.reason.trim(),&effect]).await?.get(0);
    tx.execute("INSERT INTO platform.report_corrections(decision_id,correction_id,actor_id,reason) VALUES($1,$2,$3,$4)",&[&selected.decision,&audit,&a.user_id,&c.reason.trim()]).await?;
    let kind=if selected.table.ends_with("groups"){"group"}else if selected.table.ends_with("chapters"){"chapter"}else{"manga"};
    crate::domains::catalog::repository::audit(&tx,&a.user_id,kind,&selected.target,&format!("report-correction:{id}"),Some(selected.version),Some(selected.version+1)).await?;
    let result=receipt(&tx,&a.user_id,&id,&c.request_id,&hash,json!({"id":id,"status":"refused","version":next,"effect":effect,"reason":c.reason.trim()})).await?;tx.commit().await?;Ok(Json(result))
}
#[derive(Deserialize,Serialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub(super) struct Reasoned {request_id:String,version:i32,reason:String}
pub(super) async fn reopen(State(s):State<AppState>,Path(id):Path<String>,h:HeaderMap,Json(c):Json<Reasoned>)->Result<Json<Value>> {
    change(s,id,h,c,false).await
}
pub(super) async fn appeal(State(s):State<AppState>,Path(id):Path<String>,h:HeaderMap,Json(c):Json<Reasoned>)->Result<Json<Value>> {
    change(s,id,h,c,true).await
}
async fn change(s:AppState,id:String,h:HeaderMap,c:Reasoned,appeal:bool)->Result<Json<Value>> {
    v::id(&id)?;key(&c.request_id)?;reason(&c.reason)?;if c.version<1||c.version==i32::MAX{return Err(Error::bad_request("Invalid report revision"));}
    let mut db=s.db.client().await?;let tx=db.transaction().await?;let a=actor(&s,&h,&tx,true).await?;if !appeal{authorize(&a)?;}else{a.require_permission("report.create")?;}
    let hash=digest(&(appeal,&c))?;if let Some(result)=prior(&tx,&a.user_id,&id,&c.request_id,&hash).await?{tx.commit().await?;return Ok(Json(result));}
    let row=tx.query_opt("SELECT version,status,submitter_id,appeal_state,reason_id,entity_id,target_version FROM platform.reports WHERE id=$1 AND deleted_at IS NULL FOR UPDATE NOWAIT",&[&id]).await.map_err(conflict)?.ok_or_else(Error::not_found)?;
    if appeal&&row.get::<_,String>(2)!=a.user_id{return Err(Error::not_found());}
    let status:String=row.get(1);if row.get::<_,i32>(0)!=c.version||status=="waiting"{return Err(Error::conflict("The report changed or is already awaiting review"));}
    if appeal&&row.get::<_,String>(3)=="pending"{return Err(Error::conflict("Your appeal is already awaiting review"));}
    if !appeal {
        let reason_id:String=row.get(4);let target:String=row.get(5);let submitted:Option<i32>=row.get(6);
        let table=match reason_id.as_str(){"scanlation_group_claim"=>Some("platform.catalog_groups"),"manga_removal_request"=>Some("platform.catalog_manga"),"chapter_removal_request"=>Some("platform.catalog_chapters"),_=>None};
        if let Some(table)=table {
            let current=tx.query_opt(&format!("SELECT version FROM {table} WHERE id=$1 AND deleted_at IS NULL"),&[&target]).await?;
            if current.is_none_or(|r|Some(r.get::<_,i32>(0))!=submitted){return Err(Error::conflict("This request is tied to an earlier target version. Preserve this history and ask the reporter for a new request; reopening cannot silently renew their old approval."));}
        }
    }
    let operation=if appeal{"appeal"}else{"reopen"};let next=if appeal{status.as_str()}else{"waiting"};
    if !appeal&&tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.reports other JOIN platform.reports current ON current.id=$1 WHERE other.id<>current.id AND other.submitter_id=current.submitter_id AND other.entity_type=current.entity_type AND other.entity_id=current.entity_id AND other.reason_id=current.reason_id AND other.status='waiting' AND other.deleted_at IS NULL)",&[&id]).await?.get::<_,bool>(0){return Err(Error::conflict("Another matching report is already waiting. Review it instead of reopening a duplicate."));}
    let effect=json!({"action":operation,"scope":"report-workflow-only","targetChanged":false,"originalSourceChanged":false});
    tx.execute("UPDATE platform.reports SET status=$2,appeal_state=CASE WHEN $3 THEN 'pending' ELSE 'reviewed' END,version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id,&next,&appeal]).await?;
    tx.execute("INSERT INTO platform.report_audit(report_id,actor_id,operation,old_status,new_status,old_version,new_version,notes,effect) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",&[&id,&a.user_id,&operation,&status,&next,&c.version,&(c.version+1),&c.reason.trim(),&effect]).await?;
    let result=receipt(&tx,&a.user_id,&id,&c.request_id,&hash,json!({"id":id,"status":next,"version":c.version+1,"effect":effect,"reason":c.reason.trim()})).await?;tx.commit().await?;Ok(Json(result))
}
