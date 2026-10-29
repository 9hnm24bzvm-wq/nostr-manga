//! Browser-session moderation workspace. No signed proof, publisher or relay authority.
use crate::{AppState, domains::{auth, catalog::validation as v, moderation}, public_api::{PublicApiError as Error, PublicApiResult as Result, moderator_reports as review}};
use axum::{Json, extract::{Path, Query, State}, http::HeaderMap};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio_postgres::Transaction;

pub(super) fn authorize(actor: &auth::SessionPrincipal, grant: &str) -> Result<()> {
    if !actor.is_moderator() { return Err(Error::forbidden("Moderator permission required")); }
    actor.require_permission(grant)?; Ok(())
}
pub(super) async fn actor(state: &AppState, h: &HeaderMap, tx: &Transaction<'_>, write: bool) -> Result<auth::SessionPrincipal> {
    tx.batch_execute("SET LOCAL lock_timeout='250ms'; SET LOCAL statement_timeout='5s'").await?;
    if write { auth::validate_mutation(state, h)?; }
    Ok(auth::require_native_account_principal_in(state, h, tx, write).await?)
}
pub(super) fn digest(value: &impl Serialize) -> Result<String> {
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(value).map_err(|_| Error::unavailable())?)))
}
fn lock_error(error:tokio_postgres::Error)->Error {
    if error.code().is_some_and(|c|matches!(c.code(),"55P03"|"40P01"|"40001")) {Error::conflict("Another moderator is changing this report. Refresh before continuing.")} else {error.into()}
}
pub(super) fn key(key: &str) -> Result<()> {
    if !(16..=80).contains(&key.len()) || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)) {
        return Err(Error::bad_request("Invalid action receipt reference"));
    } Ok(())
}
async fn target_link(tx:&Transaction<'_>,category:&str,id:&str)->Result<Option<String>> {
    let (kind,route)=match category {"user"=>("user","user"),"manga"=>("manga","title"),"chapter"=>("chapter","chapter"),"author"=>("author","author"),"scanlation_group"=>("group","group"),_=>return Ok(None)};
    let row=tx.query_opt("SELECT public_id::text FROM platform.public_resource_ids WHERE kind=$1 AND native_id=$2 AND preferred",&[&kind,&id]).await?;
    Ok(row.map(|r|format!("/{route}/{}",r.get::<_,String>(0))))
}
async fn target_name(tx:&Transaction<'_>,category:&str,id:&str)->Result<String> {
    review::native_target_label(tx,category,id).await
}
pub(super) async fn prior(tx: &Transaction<'_>, who: &str, id: &str, request_id: &str, hash: &str) -> Result<Option<Value>> {
    key(request_id)?;
    if let Some(row) = tx.query_opt("SELECT request_hash,report_id,receipt FROM platform.staff_report_receipts WHERE actor_id=$1 AND request_id=$2", &[&who,&request_id]).await? {
        if row.get::<_,String>(0)!=hash || row.get::<_,String>(1)!=id { return Err(Error::conflict("This receipt reference belongs to a different action")); }
        return Ok(Some(row.get(2)));
    } Ok(None)
}
pub(super) async fn receipt(tx: &Transaction<'_>, who: &str, id: &str, request_id: &str, hash: &str, mut result: Value) -> Result<Value> {
    result["scope"] = json!("native-moderation"); result["actorId"] = json!(who); result["requestId"] = json!(request_id);
    tx.execute("INSERT INTO platform.staff_report_receipts(actor_id,request_id,request_hash,report_id,receipt) VALUES($1,$2,$3,$4,$5)", &[&who,&request_id,&hash,&id,&result]).await?;
    Ok(result)
}
pub(super) async fn receipt_read(State(s): State<AppState>, Path(request_id): Path<String>, h: HeaderMap) -> Result<Json<Value>> {
    key(&request_id)?;
    let mut db=s.db.client().await?;let tx=db.transaction().await?;
    let a=actor(&s,&h,&tx,false).await?;
    // Only the original actor can reconcile a command. No role can select a
    // different actor's request ID, and losing review authority hides receipts.
    let row=tx.query_opt("SELECT receipt,report_id FROM platform.staff_report_receipts WHERE actor_id=$1 AND request_id=$2",&[&a.user_id,&request_id]).await?.ok_or_else(Error::not_found)?;
    let result:Value=row.get(0);
    if result["effect"]["action"]=="appeal" {
        let report:String=row.get(1);
        if tx.query_opt("SELECT id FROM platform.reports WHERE id=$1 AND submitter_id=$2",&[&report,&a.user_id]).await?.is_none(){return Err(Error::not_found());}
    } else if result["deleted"]==true {authorize(&a,"report.delete")?;}
    else if matches!(result["effect"]["action"].as_str(),Some("restore_local_visibility"|"restore_previous_group_membership"|"reopen")) {
        if !a.is_staff(){return Err(Error::forbidden("Staff correction authority required"));}a.require_permission("report.correct")?;
    } else {authorize(&a,"report.update")?;}
    tx.commit().await?;Ok(Json(result))
}
#[derive(Default,Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub(super) struct Filter { status: Option<String>, assigned: Option<String>, priority: Option<String>, search: Option<String>, page: Option<i64>, limit:Option<i64>, reason:Option<String>, category:Option<String>, order:Option<String> }
pub(super) async fn list(State(s): State<AppState>, h: HeaderMap, Query(q): Query<Filter>) -> Result<Json<Value>> {
    let page=q.page.unwrap_or(1);let limit=q.limit.unwrap_or(25);if !(1..=2000).contains(&page)||!(1..=25).contains(&limit){return Err(Error::bad_request("Invalid queue page or size"));}
    if let Some(reason)=&q.reason{v::id(reason)?;}
    if q.category.as_deref().is_some_and(|s|!["user","manga","author","chapter","scanlation_group","thread","reply"].contains(&s)){return Err(Error::bad_request("Invalid report category"));}
    let order=q.order.as_deref().unwrap_or("asc");if !["asc","desc"].contains(&order){return Err(Error::bad_request("Invalid queue order"));}
    let status=q.status.unwrap_or_else(||"waiting".into());
    if !["waiting","accepted","refused","autoresolved","all"].contains(&status.as_str()){return Err(Error::bad_request("Invalid report status"));}
    let assigned=q.assigned.unwrap_or_else(||"all".into());if !["all","mine","unassigned"].contains(&assigned.as_str()){return Err(Error::bad_request("Invalid assignment filter"));}
    let priority=q.priority.unwrap_or_default();if !["","low","normal","high","urgent"].contains(&priority.as_str()){return Err(Error::bad_request("Invalid priority"));}
    let search=q.search.unwrap_or_default();if search.len()>200{return Err(Error::bad_request("Search is too long"));}
    let mut db=s.db.client().await?;let tx=db.transaction().await?;let a=actor(&s,&h,&tx,false).await?;authorize(&a,"report.update")?;
    let where_sql="r.deleted_at IS NULL AND ($1='all' OR r.status=$1) AND ($2='all' OR ($2='mine' AND r.assigned_to=$3) OR ($2='unassigned' AND r.assigned_to IS NULL)) AND ($4='' OR r.priority=$4) AND ($5='' OR position(lower($5) in lower(r.id||' '||r.entity_id||' '||rr.reason||' '||u.username))>0) AND ($6::text IS NULL OR r.reason_id=$6) AND ($7::text IS NULL OR r.entity_type=$7)";
    let total:i64=tx.query_one(&format!("SELECT count(*) FROM platform.reports r JOIN platform.report_reasons rr ON rr.id=r.reason_id JOIN platform.users u ON u.id=r.submitter_id WHERE {where_sql}"),&[&status,&assigned,&a.user_id,&priority,&search,&q.reason,&q.category]).await?.get(0);
    let rows=tx.query(&format!("SELECT jsonb_build_object('id',r.id,'category',r.entity_type,'objectId',r.entity_id,'reason',rr.reason,'status',r.status,'version',r.version,'submitter',u.username,'assignedTo',r.assigned_to,'assignee',au.username,'workState',r.work_state,'priority',r.priority,'appealState',r.appeal_state,'createdAt',platform.catalog_timestamp(r.created_at)) FROM platform.reports r JOIN platform.report_reasons rr ON rr.id=r.reason_id JOIN platform.users u ON u.id=r.submitter_id LEFT JOIN platform.users au ON au.id=r.assigned_to WHERE {where_sql} ORDER BY CASE r.priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'normal' THEN 2 ELSE 3 END,r.created_at {order},r.id {order} LIMIT $8 OFFSET $9"),&[&status,&assigned,&a.user_id,&priority,&search,&q.reason,&q.category,&limit,&((page-1)*limit)]).await?;
    let mut data:Vec<Value>=rows.into_iter().map(|r|r.get(0)).collect();
    for item in &mut data {let category=item["category"].as_str().ok_or_else(Error::unavailable)?.to_owned();let id=item["objectId"].as_str().ok_or_else(Error::unavailable)?.to_owned();item["targetName"]=json!(target_name(&tx,&category,&id).await?);item["targetHref"]=json!(target_link(&tx,&category,&id).await?);}
    let result=json!({"scope":"site-moderator-queue","actorId":a.user_id,"data":data,"total":total,"page":page,"limit":limit});tx.commit().await?;Ok(Json(result))
}
pub(super) async fn reviewers(State(s):State<AppState>,h:HeaderMap)->Result<Json<Value>> {
    let mut db=s.db.client().await?;let tx=db.transaction().await?;let a=actor(&s,&h,&tx,false).await?;authorize(&a,"report.assign")?;
    let rows=tx.query("SELECT id,username FROM platform.users WHERE verified AND NOT disabled AND roles ?| ARRAY['moderator','staff','sysop'] AND NOT (roles ?| ARRAY['banned','restricted']) AND permissions ? 'report.update' ORDER BY lower(username),id LIMIT 101",&[]).await?;
    if rows.len()>100{return Err(Error::conflict("The reviewer directory exceeds this workspace's bound. Ask an administrator to narrow the reviewer roster."));}
    let result=json!({"scope":"native-reviewer-directory","actorId":a.user_id,"data":rows.into_iter().map(|r|json!({"id":r.get::<_,String>(0),"name":r.get::<_,String>(1)})).collect::<Vec<_>>()});tx.commit().await?;Ok(Json(result))
}
#[derive(Default,Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreviewQuery { intent: Option<String> }
pub(super) async fn preview(State(s): State<AppState>, Path(id): Path<String>, h: HeaderMap, Query(q): Query<PreviewQuery>) -> Result<Json<Value>> {
    v::id(&id)?;let delete=q.intent.as_deref()==Some("delete");if q.intent.as_deref().is_some_and(|x|x!="delete"){return Err(Error::bad_request("Invalid review intent"));}
    let mut db=s.db.client().await?;let tx=db.transaction().await?;let a=actor(&s,&h,&tx,false).await?;authorize(&a,if delete {"report.delete"}else{"report.update"})?;
    let context=if delete {let authority=review::deletion_authority(&tx,&a.user_id,&a.user_id).await?;review::deletion_context(&tx,&id,false,authority).await?} else {review::context(&tx,&id,false,|p|a.has_perm(p)).await?};
    let link=target_link(&tx,context["report"]["category"].as_str().ok_or_else(Error::unavailable)?,context["report"]["objectId"].as_str().ok_or_else(Error::unavailable)?).await?;
    let result=json!({"scope":"native-moderation-review","actorId":a.user_id,"context":context,"targetHref":link});tx.commit().await?;Ok(Json(result))
}
#[derive(Deserialize,Serialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub(super) struct Decision { request_id:String, action:String, expected_report_version:i32, expected_context_hash:String, expected_effect:Value, notes:String }
pub(super) async fn decide(State(s): State<AppState>, Path(id): Path<String>, h: HeaderMap, Json(c): Json<Decision>) -> Result<Json<Value>> {
    v::id(&id)?;key(&c.request_id)?;if !["accept","refuse","delete"].contains(&c.action.as_str()) || c.expected_report_version<1 || c.notes.trim().is_empty() || c.notes.len()>4000 || c.notes.contains('\0') || !c.expected_effect.is_object() {return Err(Error::bad_request("Review the decision and provide a reason of 1–4000 bytes"));}
    let mut db=s.db.client().await?;let tx=db.transaction().await?;let a=actor(&s,&h,&tx,true).await?;authorize(&a,if c.action=="delete"{"report.delete"}else{"report.update"})?;
    let hash=digest(&c)?;if let Some(result)=prior(&tx,&a.user_id,&id,&c.request_id,&hash).await?{tx.commit().await?;return Ok(Json(result));}
    let context=if c.action=="delete" {let authority=review::deletion_authority(&tx,&a.user_id,&a.user_id).await?;review::deletion_context(&tx,&id,true,authority).await?} else {review::context(&tx,&id,true,|p|a.has_perm(p)).await?};
    if context["report"]["version"]!=c.expected_report_version || context["contextHash"]!=c.expected_context_hash {return Err(Error::conflict("The report, your authority or the target changed. Open a fresh review."));}
    let effect=if c.action=="delete"{context["effect"].clone()}else if c.action=="accept"{context["acceptEffect"].clone()}else{json!({})};
    if effect!=c.expected_effect{return Err(Error::conflict("The reviewed consequence changed. Open a fresh review."));}
    let result=if c.action=="delete" {
        let d=moderation::delete_report_reasoned(&tx,&a,&id,Some(c.expected_report_version),Some(c.notes.trim())).await?;
        json!({"id":id,"status":d.status,"version":d.new_version,"deleted":true,"effect":effect})
    } else {
        if c.action=="accept" && context["acceptAllowed"]!=true{return Err(Error::forbidden("This consequence is unavailable or needs a separate permission"));}
        let (status,operation)=if c.action=="accept"{("accepted","accept")}else{("refused","reject")};
        let d=moderation::review_report(&tx,&a,&id,Some(c.notes.trim()),Some(c.expected_report_version),status,operation).await?;
        if d.effect!=effect{return Err(Error::unavailable());}
        tx.execute("UPDATE platform.reports SET appeal_state=CASE WHEN appeal_state='pending' THEN 'reviewed' ELSE appeal_state END WHERE id=$1",&[&id]).await?;
        json!({"id":id,"status":status,"version":d.new_version,"deleted":false,"effect":effect})
    };
    let result=receipt(&tx,&a.user_id,&id,&c.request_id,&hash,result).await?;tx.commit().await?;Ok(Json(result))
}
#[derive(Deserialize,Serialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub(super) struct Work {request_id:String,version:i32,action:String,value:Option<String>}
pub(super) async fn work(State(s):State<AppState>,Path(id):Path<String>,h:HeaderMap,Json(c):Json<Work>)->Result<Json<Value>> {
    v::id(&id)?;key(&c.request_id)?;if c.version<1{return Err(Error::bad_request("Invalid report revision"));}
    let mut db=s.db.client().await?;let tx=db.transaction().await?;let a=actor(&s,&h,&tx,true).await?;authorize(&a,"report.update")?;
    let hash=digest(&c)?;if let Some(result)=prior(&tx,&a.user_id,&id,&c.request_id,&hash).await?{tx.commit().await?;return Ok(Json(result));}
    let row=tx.query_opt("SELECT version,status,assigned_to,work_state,priority FROM platform.reports WHERE id=$1 AND deleted_at IS NULL FOR UPDATE NOWAIT",&[&id]).await.map_err(lock_error)?.ok_or_else(Error::not_found)?;
    let version:i32=row.get(0);let status:String=row.get(1);let mut assigned:Option<String>=row.get(2);let mut state:String=row.get(3);let mut priority:String=row.get(4);
    if version!=c.version||status!="waiting"||version==i32::MAX{return Err(Error::conflict("Another moderator changed this report. Refresh the queue."));}
    match c.action.as_str() {
        "claim"=>{if assigned.is_some(){return Err(Error::conflict("This report is already assigned"));}assigned=Some(a.user_id.clone());state="assigned".into();},
        "unclaim"=>{if assigned.as_deref()!=Some(&a.user_id){a.require_permission("report.assign")?;}assigned=None;state="unassigned".into();},
        "assign"=>{a.require_permission("report.assign")?;let target=c.value.as_deref().ok_or_else(||Error::bad_request("Choose an eligible reviewer"))?;v::id(target)?;
            if tx.query_opt("SELECT id FROM platform.users WHERE id=$1 AND verified AND NOT disabled AND roles ?| ARRAY['moderator','staff','sysop'] AND NOT (roles ?| ARRAY['banned','restricted']) AND permissions ? 'report.update' FOR SHARE NOWAIT",&[&target]).await?.is_none(){return Err(Error::conflict("That account is not an eligible reviewer"));}assigned=Some(target.into());state="assigned".into();},
        "progress"=>{if assigned.as_deref()!=Some(&a.user_id){return Err(Error::forbidden("Claim this report before starting work"));}state="in_progress".into();},
        "priority"=>{let value=c.value.as_deref().unwrap_or("");if !["low","normal","high","urgent"].contains(&value){return Err(Error::bad_request("Choose a supported priority"));}priority=value.into();},
        _=>return Err(Error::bad_request("Unsupported queue action"))
    }
    tx.execute("UPDATE platform.reports SET assigned_to=$2,work_state=$3,priority=$4,version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id,&assigned,&state,&priority]).await?;
    let effect=json!({"assignedTo":assigned,"workState":state,"priority":priority});
    tx.execute("INSERT INTO platform.report_audit(report_id,actor_id,operation,old_status,new_status,old_version,new_version,effect) VALUES($1,$2,$3,$4,$4,$5,$6,$7)",&[&id,&a.user_id,&c.action,&status,&version,&(version+1),&effect]).await?;
    let result=receipt(&tx,&a.user_id,&id,&c.request_id,&hash,json!({"id":id,"status":status,"version":version+1,"effect":effect})).await?;tx.commit().await?;Ok(Json(result))
}
