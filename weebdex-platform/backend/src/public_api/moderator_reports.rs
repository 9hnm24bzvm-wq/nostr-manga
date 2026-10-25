//! Explicit, private native moderation. Separate from personal inspection and
//! source-event reports. All decisions share the original native transaction core.
use super::{PublicApiError, PublicApiResult, PublicUuid, auth::{admitted_request, bound_account_read, bound_account_write, BoundAccountWrite}, query::PublicQuery};
use crate::{AppState, domains::{catalog::validation as v, library::account_public_id_in, moderation}};
use axum::{Json, Router, extract::{OriginalUri, Path, Request, State}, http::{HeaderMap, Uri, header}, response::{IntoResponse, Response}, routing::get};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio_postgres::{Row, Transaction};

impl moderation::ReportReviewActor for BoundAccountWrite {}
const CATEGORIES: &[&str] = &["user","manga","author","chapter","scanlation_group","thread","reply"];
const STATUSES: &[&str] = &["waiting","accepted","refused","autoresolved"];
const PAGE_SIZE: usize = 5;

const REPORT_COLUMNS: &str = "CASE WHEN octet_length(r.id) BETWEEN 1 AND 128 THEN r.id END, r.entity_type, CASE WHEN octet_length(r.entity_id) BETWEEN 1 AND 128 THEN r.entity_id END, r.status,r.version,to_char(r.created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),CASE WHEN octet_length(r.submitter_id) BETWEEN 1 AND 128 THEN r.submitter_id END,CASE WHEN octet_length(r.reason_id) BETWEEN 1 AND 128 THEN r.reason_id END,r.target_version,CASE WHEN octet_length(r.body)<=20000 THEN r.body END,CASE WHEN r.notes IS NULL THEN '' WHEN octet_length(r.notes)<=20000 THEN r.notes END";
pub fn router() -> Router<AppState> {
    Router::new().route("/nostr/moderation/reports",get(list))
        .route("/nostr/moderation/reports/{id}",get(detail).post(decide))
        .layer(axum::middleware::from_fn(super::native_user_discussion::private_headers))
}
fn lock_error(error: tokio_postgres::Error) -> PublicApiError {
    if error.code().is_some_and(|c| matches!(c.code(),"55P03"|"40P01"|"40001"|"57014")) {
        PublicApiError::conflict("Moderation state is changing; reload before another review")
    } else {error.into()}
}
async fn bounds(tx: &Transaction<'_>) -> PublicApiResult<()> {
    tx.batch_execute("SET LOCAL lock_timeout='250ms'; SET LOCAL statement_timeout='5s'").await?;
    Ok(())
}
fn id(row: &Row, index: usize) -> PublicApiResult<String> {
    let value = row.get::<_,Option<String>>(index).ok_or_else(PublicApiError::unavailable)?;
    v::id(&value).map_err(|_|PublicApiError::unavailable())?;
    Ok(value)
}
fn positive(value: i32) -> PublicApiResult<i32> { if value>0 {Ok(value)} else {Err(PublicApiError::unavailable())} }
fn report(row: &Row, full: bool) -> PublicApiResult<Value> {
    let category:String=row.get(1);let status:String=row.get(3);
    if !CATEGORIES.contains(&category.as_str())||!STATUSES.contains(&status.as_str()){return Err(PublicApiError::unavailable());}
    let mut value=json!({"id":id(row,0)?,"category":category,"objectId":id(row,2)?,"status":status,"version":positive(row.get(4))?,"createdAt":row.get::<_,String>(5),"submitterId":id(row,6)?,"reasonId":id(row,7)?});
    if full {
        value["submittedTargetVersion"]=json!(row.get::<_,Option<i32>>(8));
        value["details"]=json!(row.get::<_,Option<String>>(9).ok_or_else(PublicApiError::unavailable)?);
        value["notes"]=json!(row.get::<_,Option<String>>(10).ok_or_else(PublicApiError::unavailable)?);
    }
    Ok(value)
}
fn digest(value:&Value)->PublicApiResult<String>{Ok(hex::encode(Sha256::digest(serde_json::to_vec(value).map_err(|_|PublicApiError::unavailable())?)))}
fn meta(account:&str)->Value{json!({"scope":"instance-moderator","source":"persisted-native-report-review","accountId":account,"nativeSessionAuthority":false})}
fn authorize(moderator:bool,permission:bool)->PublicApiResult<()> {
    if moderator&&permission {Ok(())} else {Err(PublicApiError::forbidden("Current instance moderator and action permission required"))}
}
#[derive(Deserialize,Serialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
struct Cursor {at:String,id:String,account_id:String,category:Option<String>,status:Option<String>,object:Option<String>,reason:Option<String>,order:String}
struct Selection {account:PublicUuid,cursor:Option<Cursor>,category:Option<String>,status:Option<String>,object:Option<String>,reason:Option<String>,order:String,delete:bool}
fn timestamp(value:&str)->bool {
    if value.len()!=27||!value.is_ascii(){return false;}
    let b=value.as_bytes();for i in 0..27 {let expected=match i{4|7=>Some(b'-'),10=>Some(b'T'),13|16=>Some(b':'),19=>Some(b'.'),26=>Some(b'Z'),_=>None};if expected.map_or(!b[i].is_ascii_digit(),|v|b[i]!=v){return false;}}
    let n=|a:usize,z:usize|value[a..z].parse::<u32>().unwrap_or(0);let year=n(0,4);let month=n(5,7);let day=n(8,10);let leap=year%4==0&&(year%100!=0||year%400==0);let days=match month{1|3|5|7|8|10|12=>31,4|6|9|11=>30,2=>if leap{29}else{28},_=>0};year>0&&day>0&&day<=days&&n(11,13)<24&&n(14,16)<60&&n(17,19)<60
}
fn query(uri:&Uri,detail:bool)->PublicApiResult<Selection>{
    let q=PublicQuery::parse_unchecked(uri.query())?;
    q.require_allowed(if detail {&["expectedAccountId","intent"]} else {&["expectedAccountId","cursor","category","status","object","reason","order"]})?;
    let account=PublicUuid::parse(q.first("expectedAccountId").ok_or_else(||PublicApiError::bad_request("expectedAccountId is required"))?)?;
    if q.first("category").is_some_and(|v|!CATEGORIES.contains(&v))||q.first("status").is_some_and(|v|!STATUSES.contains(&v))||q.first("order").is_some_and(|v|!matches!(v,"asc"|"desc"))||q.first("intent").is_some_and(|v|v!="delete"){return Err(PublicApiError::bad_request("Invalid moderation filter"));}
    let category=q.first("category").map(str::to_owned);let status=q.first("status").map(str::to_owned);
    let object=q.first("object").map(str::to_owned);let reason=q.first("reason").map(str::to_owned);let order=q.first("order").unwrap_or("desc").to_owned();
    for value in [&object,&reason].into_iter().flatten(){v::id(value)?;}
    let cursor=if let Some(raw)=q.first("cursor") {
        if raw.len()>1024{return Err(PublicApiError::bad_request("Invalid moderation cursor"));}
        let c:Cursor=serde_json::from_str(raw).map_err(|_|PublicApiError::bad_request("Invalid moderation cursor"))?;
        v::id(&c.id)?;for value in [&c.object,&c.reason].into_iter().flatten(){v::id(value)?;}
        if !timestamp(&c.at)||c.account_id!=account.as_str()||c.category!=category||c.status!=status||c.object!=object||c.reason!=reason||c.order!=order||serde_json::to_string(&c).map_err(|_|PublicApiError::unavailable())?!=raw{return Err(PublicApiError::bad_request("Moderation cursor scope changed"));}Some(c)
    }else{None};
    Ok(Selection{account,cursor,category,status,object,reason,order,delete:q.first("intent")==Some("delete")})
}
async fn list(State(state):State<AppState>,OriginalUri(uri):OriginalUri,headers:HeaderMap,request:Request)->PublicApiResult<Response>{
    let Selection{account:expected,cursor,category,status,object,reason,order,..}=query(&uri,false)?;
    let method=request.method().as_str().to_owned();let bytes=super::native_user_reports::body(request).await?;
    if !bytes.is_empty(){return Err(PublicApiError::bad_request("Moderation reads accept no body"));}
    let cap=admitted_request(&state,&uri,&headers,&method,&bytes).await?;
    let mut db=state.db.client().await?;let tx=db.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).start().await?;bounds(&tx).await?;
    let actor=bound_account_read(&tx,&cap).await?;let account=account_public_id_in(&tx,actor.user_id()).await?;
    if expected.as_str()!=account{return Err(PublicApiError::conflict("Account binding changed; reopen moderation"));}
    authorize(actor.is_moderator(),actor.has_perm("report.update"))?;
    crate::domains::auth::closure::active_in(&tx,actor.user_id()).await?;
    let predicate="r.deleted_at IS NULL AND ($1::text IS NULL OR r.entity_type=$1) AND ($2::text IS NULL OR r.status=$2) AND ($3::text IS NULL OR r.entity_id=$3) AND ($4::text IS NULL OR r.reason_id=$4)";
    let total:i64=tx.query_one(&format!("SELECT count(*) FROM platform.reports r WHERE {predicate}"),&[&category,&status,&object,&reason]).await.map_err(lock_error)?.get(0);
    let after_at=cursor.as_ref().map(|c|c.at.as_str());let after_id=cursor.as_ref().map(|c|c.id.as_str());
    // One look-ahead row establishes continuation, without an offset ceiling.
    // Timestamp microseconds and C-collated native ID form a stable total order.
    let columns=REPORT_COLUMNS.split(",r.target_version,").next().ok_or_else(PublicApiError::unavailable)?;
    let comparison=if order=="asc" {">"} else {"<"};
    let rows=tx.query(&format!("SELECT {columns} FROM platform.reports r WHERE {predicate} AND ($5::text IS NULL OR (r.created_at,r.id COLLATE \"C\") {comparison} ($5::text::timestamptz,$6::text COLLATE \"C\")) ORDER BY r.created_at {order},r.id COLLATE \"C\" {order} LIMIT 6"),&[&category,&status,&object,&reason,&after_at,&after_id]).await.map_err(lock_error)?;
    let data=rows.iter().take(PAGE_SIZE).map(|r|report(r,false)).collect::<PublicApiResult<Vec<_>>>()?;
    let next_cursor=if rows.len()>PAGE_SIZE {let last=data.last().ok_or_else(PublicApiError::unavailable)?;Some(serde_json::to_string(&Cursor{at:last["createdAt"].as_str().unwrap().to_owned(),id:last["id"].as_str().unwrap().to_owned(),account_id:account.clone(),category:category.clone(),status:status.clone(),object:object.clone(),reason:reason.clone(),order:order.clone()}).map_err(|_|PublicApiError::unavailable())?)}else{None};
    let result=json!({"result":"ok","response":"collection","nostr":meta(&account),"data":data,"total":total,"limit":PAGE_SIZE,"nextCursor":next_cursor});
    super::native_user_reports::require_context_budget(&result)?;tx.commit().await?;Ok(Json(result).into_response())
}
// Project bounded native target evidence without report_json expansion, profile
// contacts, hidden source-event content or fabricated public UUIDs. Every actual
// row is locked NOWAIT, including the chapter parent and group claimant below.
async fn source_fence(tx:&Transaction<'_>,category:&str,id:&str)->PublicApiResult<()> {
    use super::native_discussion::{source_fences,manga,chapter};
    let sources=match category {
        "manga"=>vec![manga(tx,id).await?],
        "chapter"=>chapter(tx,id).await?,
        "author"=>vec![tx.query_one("SELECT protocol_event_id FROM platform.catalog_authors WHERE id=$1 FOR SHARE NOWAIT",&[&id]).await.map_err(lock_error)?.get(0)],
        "scanlation_group"=>{
            let imported=tx.query_opt("SELECT catalog_id FROM platform.protocol_resource_ids WHERE kind='group' AND catalog_id=$1 FOR SHARE NOWAIT",&[&id]).await.map_err(lock_error)?.is_some();
            if imported {
                let row=tx.query_opt("SELECT CASE WHEN octet_length(cg.chapter_id) BETWEEN 1 AND 128 THEN cg.chapter_id END FROM platform.catalog_chapter_groups cg JOIN platform.catalog_chapters c ON c.id=cg.chapter_id JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE cg.group_id=$1 AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(c.protocol_event_id) AND platform.protocol_source_is_visible(m.protocol_series_id) ORDER BY c.id LIMIT 1 FOR SHARE OF cg NOWAIT",&[&id]).await.map_err(lock_error)?.ok_or_else(PublicApiError::not_found)?;
                let witness:String=row.get::<_,Option<String>>(0).ok_or_else(PublicApiError::unavailable)?;v::id(&witness).map_err(|_|PublicApiError::unavailable())?;chapter(tx,&witness).await?
            }else{vec![]}
        }
        _=>vec![]
    };
    source_fences(tx,sources).await
}
async fn target_base(tx:&Transaction<'_>,category:&str,id:&str,write:bool)->PublicApiResult<Value>{
    let lock=if write {"FOR UPDATE NOWAIT"} else {"FOR SHARE NOWAIT"};
    let (table,label,body,version,extra)=match category {
        "user"=>("platform.users","username","''","version","jsonb_build_object('disabled',disabled)"),
        "manga"=>("platform.catalog_manga","title","''","version","jsonb_build_object('locked',locked,'state',state,'deleted',deleted_at IS NOT NULL)"),
        "author"=>("platform.catalog_authors","name","''","version","jsonb_build_object('locked',locked,'deleted',deleted_at IS NOT NULL)"),
        "scanlation_group"=>("platform.catalog_groups","name","''","version","jsonb_build_object('locked',locked,'deleted',deleted_at IS NOT NULL)"),
        "chapter"=>("platform.catalog_chapters","COALESCE(title,'')","''","version","jsonb_build_object('parentId',CASE WHEN octet_length(manga_id) BETWEEN 1 AND 128 THEN manga_id END,'deleted',deleted_at IS NOT NULL)"),
        "thread"=>("platform.catalog_threads","subject","body","NULL::integer","jsonb_build_object('locked',locked,'parentKind',entity_type,'parentId',CASE WHEN octet_length(entity_id) BETWEEN 1 AND 128 THEN entity_id END)"),
        "reply"=>("platform.community_replies","''","body","version","jsonb_build_object('state',state,'threadId',CASE WHEN octet_length(thread_id) BETWEEN 1 AND 128 THEN thread_id END)"),
        _=>return Err(PublicApiError::unavailable())
    };
    let selector=if category=="reply" {"id::text"} else {"id"};
    let row=tx.query_opt(&format!("SELECT {version},updated_at::text,CASE WHEN octet_length({label})<=8000 THEN {label} END,CASE WHEN octet_length({body})<=50000 THEN {body} END,{extra},platform.report_entity_exists($2,$1) FROM {table} WHERE {selector}=$1 {lock}"),&[&id,&category]).await.map_err(lock_error)?;
    let Some(row)=row else {return Ok(json!({"category":category,"id":id,"available":false}));};
    let available:bool=row.get(5);
    if !available{return Ok(json!({"category":category,"id":id,"available":false,"version":row.get::<_,Option<i32>>(0),"updatedAt":row.get::<_,String>(1)}));}
    source_fence(tx,category,id).await?;
    let label:String=row.get::<_,Option<String>>(2).ok_or_else(PublicApiError::unavailable)?;
    let body:String=row.get::<_,Option<String>>(3).ok_or_else(PublicApiError::unavailable)?;
    let extra:Value=row.get(4);
    // Parent IDs are bounded before any nested query or response serialization.
    for key in ["parentId","threadId"] {if let Some(value)=extra.get(key){v::id(value.as_str().ok_or_else(PublicApiError::unavailable)?).map_err(|_|PublicApiError::unavailable())?;}}
    Ok(json!({"category":category,"id":id,"available":true,"version":row.get::<_,Option<i32>>(0),"updatedAt":row.get::<_,String>(1),"label":label,"body":body,"state":extra}))
}
async fn context(tx:&Transaction<'_>,id:&str,write:bool,has:impl Fn(&str)->bool)->PublicApiResult<Value>{
    let lock=if write {"FOR UPDATE NOWAIT"} else {"FOR SHARE NOWAIT"};
    // Preserve the native title-before-report lock graph. Re-read the report
    // identity under its lock; an untrusted pre-read can never redirect a target.
    let pre=tx.query_opt("SELECT entity_id FROM platform.reports WHERE id=$1 AND entity_type='manga' AND reason_id='manga_removal_request' AND deleted_at IS NULL",&[&id]).await.map_err(lock_error)?;
    let title=if let Some(row)=pre{let target:String=row.get(0);v::id(&target).map_err(|_|PublicApiError::unavailable())?;Some(target_base(tx,"manga",&target,write).await?)}else{None};
    let row=tx.query_opt(&format!("SELECT {REPORT_COLUMNS} FROM platform.reports r WHERE r.id=$1 AND r.deleted_at IS NULL {lock}"),&[&id]).await.map_err(lock_error)?.ok_or_else(PublicApiError::not_found)?;
    let report=report(&row,true)?;let category=report["category"].as_str().unwrap();let target_id=report["objectId"].as_str().unwrap();let reason_id=report["reasonId"].as_str().unwrap();
    if reason_id=="manga_removal_request" && title.as_ref().and_then(|t|t["id"].as_str())!=Some(target_id){return Err(PublicApiError::conflict("Report target changed; reopen review"));}
    let reason=tx.query_opt("SELECT version,CASE WHEN octet_length(reason) BETWEEN 1 AND 8000 THEN reason END,details_required,enabled,category FROM platform.report_reasons WHERE id=$1 FOR SHARE NOWAIT",&[&reason_id]).await.map_err(lock_error)?.ok_or_else(PublicApiError::unavailable)?;
    if reason.get::<_,String>(4)!=category{return Err(PublicApiError::unavailable());}
    let reason=json!({"id":reason_id,"version":positive(reason.get(0))?,"label":reason.get::<_,Option<String>>(1).ok_or_else(PublicApiError::unavailable)?,"detailsRequired":reason.get::<_,bool>(2),"enabled":reason.get::<_,bool>(3)});
    // The original claim order is report -> claimant -> group. Never hold the
    // group and then wait for another actor row; NOWAIT makes reciprocity bounded.
    let claimant=if reason_id=="scanlation_group_claim" {
        let claimant=report["submitterId"].as_str().unwrap();
        let row=tx.query_opt(&format!("SELECT version,CASE WHEN octet_length(username) BETWEEN 1 AND 128 THEN username END,verified AND NOT disabled AND NOT (roles ?| ARRAY['banned','restricted']) FROM platform.users WHERE id=$1 {lock}"),&[&claimant]).await.map_err(lock_error)?;
        if let Some(r)=row {json!({"id":claimant,"version":positive(r.get(0))?,"name":r.get::<_,Option<String>>(1).ok_or_else(PublicApiError::unavailable)?,"eligible":r.get::<_,bool>(2)})} else {Value::Null}
    }else{Value::Null};
    let mut target=if let Some(title)=title{title}else{target_base(tx,category,target_id,write).await?};
    if target["available"]==true {
        if category=="chapter" {target["parent"]=target_base(tx,"manga",target["state"]["parentId"].as_str().unwrap(),write).await?;}
        if category=="reply" {target["thread"]=target_base(tx,"thread",target["state"]["threadId"].as_str().unwrap(),write).await?;}
        if category=="thread" || category=="reply" {
            let thread=if category=="thread" {&target} else {&target["thread"]};
            if thread["available"]==true {
                let kind=match thread["state"]["parentKind"].as_str(){Some("group")=>"scanlation_group",Some("manga")=>"manga",Some("chapter")=>"chapter",Some("user")=>"user",_=>return Err(PublicApiError::unavailable())};
                let mut parent=target_base(tx,kind,thread["state"]["parentId"].as_str().unwrap(),write).await?;
                if kind=="chapter" && parent["available"]==true {parent["parent"]=target_base(tx,"manga",parent["state"]["parentId"].as_str().unwrap(),write).await?;}
                target["parent"]=parent;
            }
        }
        if category=="scanlation_group" {target["hasLeader"]=json!(tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.catalog_group_members WHERE group_id=$1 AND is_leader)",&[&target_id]).await.map_err(lock_error)?.get::<_,bool>(0));}
    }
    let version=target["version"].as_i64();let submitted=report["submittedTargetVersion"].as_i64();
    let mut allowed=report["status"]=="waiting";let mut effect=json!({});
    match reason_id {
        "scanlation_group_claim"=>{allowed &= category=="scanlation_group"&&has("scanlation_group.update")&&target["available"]==true&&target["state"]["locked"]==false&&target["hasLeader"]==false&&claimant["eligible"]==true&&version.is_some()&&submitted==version;effect=json!({"action":"grant_group_leadership","group_id":target_id,"claimant_id":report["submitterId"],"old_group_version":version,"new_group_version":version.map(|v|v+1)});}
        "chapter_removal_request"=>{allowed &= category=="chapter"&&has("chapter.remove")&&target["available"]==true&&target["parent"]["available"]==true&&version.is_some()&&submitted==version;effect=json!({"action":"instance_soft_hide","chapter_id":target_id,"source_deletion":"independent_original_moderation"});}
        "manga_removal_request"=>{allowed &= category=="manga"&&has("manga.delete")&&target["available"]==true&&version.is_some()&&submitted==version;effect=json!({"action":"instance_soft_hide","manga_id":target_id,"old_version":version,"new_version":version.map(|v|v+1),"source_deletion":"not_requested","original_event_rewritten":false});}
        _=>{}
    }
    let target_revision=digest(&json!({"target":target,"claimant":claimant}))?;
    let mut context=json!({"report":report,"reason":reason,"target":target,"claimant":claimant,"targetRevision":target_revision,"acceptAllowed":allowed,"acceptEffect":effect});
    context["contextHash"]=json!(digest(&context)?);super::native_user_reports::require_context_budget(&context)?;Ok(context)
}
// Deletion intentionally does not use the rich review context: unavailable,
// orphaned and already reviewed reports remain deletable with report.delete.
// Authority is read from the actor row already locked by bound_account_*.
async fn deletion_authority(tx:&Transaction<'_>,user:&str,account:&str)->PublicApiResult<Value>{
    let row=tx.query_one("SELECT version,CASE WHEN octet_length(roles::text)<=8192 THEN roles END,CASE WHEN octet_length(permissions::text)<=8192 THEN permissions END FROM platform.users WHERE id=$1",&[&user]).await.map_err(lock_error)?;
    let roles:Value=row.get::<_,Option<Value>>(1).ok_or_else(PublicApiError::unavailable)?;
    let permissions:Value=row.get::<_,Option<Value>>(2).ok_or_else(PublicApiError::unavailable)?;
    let normalize=|value:Value|->PublicApiResult<Vec<String>>{let mut values:Vec<String>=serde_json::from_value(value).map_err(|_|PublicApiError::unavailable())?;if values.len()>256||values.iter().any(|v|v.len()>128||v.contains('\0')){return Err(PublicApiError::unavailable());}values.sort();values.dedup();Ok(values)};
    let authority=json!({"accountId":account,"version":positive(row.get(0))?,"roles":normalize(roles)?,"permissions":normalize(permissions)?});
    Ok(json!({"accountId":account,"version":authority["version"],"revision":digest(&authority)?}))
}
async fn deletion_context(tx:&Transaction<'_>,id:&str,write:bool,authority:Value)->PublicApiResult<Value>{
    let lock=if write {"FOR UPDATE NOWAIT"} else {"FOR SHARE NOWAIT"};
    let row=tx.query_opt(&format!("SELECT CASE WHEN octet_length(id) BETWEEN 1 AND 128 THEN id END,entity_type,CASE WHEN octet_length(entity_id) BETWEEN 1 AND 128 THEN entity_id END,status,version,CASE WHEN octet_length(reason_id) BETWEEN 1 AND 128 THEN reason_id END FROM platform.reports WHERE id=$1 AND deleted_at IS NULL {lock}"),&[&id]).await.map_err(lock_error)?.ok_or_else(PublicApiError::not_found)?;
    let category:String=row.get(1);let status:String=row.get(3);let version=positive(row.get(4))?;
    if !CATEGORIES.contains(&category.as_str())||!STATUSES.contains(&status.as_str())||version==i32::MAX{return Err(PublicApiError::unavailable());}
    let report=json!({"id":self::id(&row,0)?,"category":category,"objectId":self::id(&row,2)?,"reasonId":self::id(&row,5)?,"status":status,"version":version,"deleted":false});
    let effect=json!({"action":"report_soft_delete","report_id":id,"old_version":version,"new_version":version+1,"content_changed":false,"source_deletion":"not_requested"});
    let mut value=json!({"intent":"delete","report":report,"authority":authority,"effect":effect});value["contextHash"]=json!(digest(&value)?);Ok(value)
}
#[derive(Deserialize)]
#[serde(rename_all="lowercase")]
enum DeleteAction{Delete}
#[derive(Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
struct DeleteCommand{#[serde(rename="action")] _action:DeleteAction,expected_account_id:PublicUuid,expected_authority_revision:String,expected_report_version:i32,expected_context_hash:String,expected_effect:Value}
impl DeleteCommand{
    fn parse(bytes:&[u8])->PublicApiResult<Self>{let c:Self=serde_json::from_slice(bytes).map_err(|_|PublicApiError::bad_request("Invalid report deletion"))?;
        for hash in [&c.expected_authority_revision,&c.expected_context_hash]{if hash.len()!=64||!hash.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)){return Err(PublicApiError::bad_request("Invalid deletion revision"));}}
        if c.expected_report_version<1||c.expected_report_version==i32::MAX||!c.expected_effect.is_object(){return Err(PublicApiError::bad_request("Invalid deletion capture"));}Ok(c)}
}
async fn delete_decision_in(state:&AppState,uri:&Uri,method:&str,bytes:&[u8],cap:&manga_node::http_auth::AdmittedHttpCapability,c:DeleteCommand,id:&str)->PublicApiResult<Response>{
    let mut db=state.db.client().await?;let tx=db.transaction().await?;bounds(&tx).await?;
    let actor=bound_account_write(&tx,cap).await?;let account=account_public_id_in(&tx,actor.user_id()).await?;
    if c.expected_account_id.as_str()!=account{return Err(PublicApiError::conflict("Account binding changed; reopen deletion"));}
    authorize(actor.is_moderator(),actor.has_perm("report.delete"))?;crate::domains::auth::closure::active_in(&tx,actor.user_id()).await?;
    let authority=deletion_authority(&tx,actor.user_id(),&account).await?;
    let context=deletion_context(&tx,id,true,authority).await?;
    if context["authority"]["revision"]!=c.expected_authority_revision||context["report"]["version"]!=c.expected_report_version||context["contextHash"]!=c.expected_context_hash||context["effect"]!=c.expected_effect{return Err(PublicApiError::conflict("Deletion authority or report changed; reopen review"));}
    let result=moderation::delete_report(&tx,&actor,id,Some(c.expected_report_version)).await?;
    if result.old_version!=c.expected_report_version||result.new_version!=c.expected_report_version+1||context["report"]["status"]!=result.status{return Err(PublicApiError::unavailable());}
    let mut metadata=meta(&account);metadata["requestHash"]=json!(hex::encode(Sha256::digest(bytes)));metadata["requestMethod"]=json!(method);metadata["requestPath"]=json!(uri.to_string());
    let response=json!({"result":"ok","response":"entity","nostr":metadata,"data":{"id":id,"oldStatus":result.status,"status":result.status,"oldVersion":result.old_version,"version":result.new_version,"deleted":true,"effect":context["effect"],"contextHash":c.expected_context_hash,"authorityRevision":c.expected_authority_revision}});
    super::native_user_reports::require_context_budget(&response)?;tx.commit().await?;Ok(Json(response).into_response())
}
async fn detail(State(state):State<AppState>,OriginalUri(uri):OriginalUri,Path(id):Path<String>,headers:HeaderMap,request:Request)->PublicApiResult<Response>{
    v::id(&id)?;let selected=query(&uri,true)?;let expected=&selected.account;let method=request.method().as_str().to_owned();let bytes=super::native_user_reports::body(request).await?;
    if !bytes.is_empty(){return Err(PublicApiError::bad_request("Moderation reads accept no body"));}
    let cap=admitted_request(&state,&uri,&headers,&method,&bytes).await?;let mut db=state.db.client().await?;let tx=db.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).start().await?;bounds(&tx).await?;
    let actor=bound_account_read(&tx,&cap).await?;let account=account_public_id_in(&tx,actor.user_id()).await?;
    if expected.as_str()!=account{return Err(PublicApiError::conflict("Account binding changed; reopen moderation"));}authorize(actor.is_moderator(),actor.has_perm(if selected.delete {"report.delete"} else {"report.update"}))?;crate::domains::auth::closure::active_in(&tx,actor.user_id()).await?;
    let data=if selected.delete {let authority=deletion_authority(&tx,actor.user_id(),&account).await?;deletion_context(&tx,&id,false,authority).await?}else{context(&tx,&id,false,|p|actor.has_perm(p)).await?};let result=json!({"result":"ok","response":"entity","nostr":meta(&account),"data":data});super::native_user_reports::require_context_budget(&result)?;tx.commit().await?;Ok(Json(result).into_response())
}
#[derive(Deserialize,PartialEq)]
#[serde(rename_all="lowercase")]
enum Action{Accept,Refuse}
#[derive(Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
struct Command{expected_account_id:PublicUuid,expected_report_version:i32,expected_reason_version:i32,expected_target_revision:String,expected_context_hash:String,expected_effect:Value,action:Action,notes:String}
impl Command {
    fn parse(bytes:&[u8])->PublicApiResult<Self>{
        let mut c:Self=serde_json::from_slice(bytes).map_err(|_|PublicApiError::bad_request("Invalid moderation decision"))?;
        for hash in [&c.expected_target_revision,&c.expected_context_hash]{if hash.len()!=64||!hash.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)){return Err(PublicApiError::bad_request("Invalid review revision"));}}
        if c.expected_report_version<=0||c.expected_reason_version<=0||!c.expected_effect.is_object(){return Err(PublicApiError::bad_request("Invalid review revision or effect"));}
        c.notes=c.notes.trim().to_owned();if c.notes.len()>20000||c.notes.contains('\0'){return Err(PublicApiError::bad_request("Moderation notes exceed native limits"));}Ok(c)
    }
}
async fn decide(State(state):State<AppState>,OriginalUri(uri):OriginalUri,Path(id):Path<String>,headers:HeaderMap,request:Request)->PublicApiResult<Response>{
    v::id(&id)?;if uri.query().is_some(){return Err(PublicApiError::bad_request("Moderation decisions accept no query"));}
    let mut types=headers.get_all(header::CONTENT_TYPE).iter();if !types.next().and_then(|h|h.to_str().ok()).is_some_and(|h|h.split(';').next().is_some_and(|h|h.trim().eq_ignore_ascii_case("application/json")))||types.next().is_some(){return Err(PublicApiError::bad_request("JSON content type required"));}
    let method=request.method().as_str().to_owned();let bytes=super::native_user_reports::body(request).await?;let cap=admitted_request(&state,&uri,&headers,&method,&bytes).await?;if let Ok(c)=DeleteCommand::parse(&bytes){return delete_decision_in(&state,&uri,&method,&bytes,&cap,c,&id).await;}let c=Command::parse(&bytes)?;
    let mut db=state.db.client().await?;let tx=db.transaction().await?;bounds(&tx).await?;
    let actor=bound_account_write(&tx,&cap).await?;let account=account_public_id_in(&tx,actor.user_id()).await?;
    if c.expected_account_id.as_str()!=account{return Err(PublicApiError::conflict("Account binding changed; reopen moderation"));}authorize(actor.is_moderator(),actor.has_perm("report.update"))?;crate::domains::auth::closure::active_in(&tx,actor.user_id()).await?;
    let context=context(&tx,&id,true,|p|actor.has_perm(p)).await?;
    if context["report"]["status"]!="waiting"||context["report"]["version"]!=c.expected_report_version||context["reason"]["version"]!=c.expected_reason_version||context["targetRevision"]!=c.expected_target_revision||context["contextHash"]!=c.expected_context_hash{return Err(PublicApiError::conflict("Report, reason, or target changed; reopen review"));}
    let accept=c.action==Action::Accept;let expected_effect=if accept {context["acceptEffect"].clone()} else {json!({})};
    if accept&&context["acceptAllowed"]!=true{return Err(PublicApiError::forbidden("This report effect is not currently permitted"));}
    if c.expected_effect!=expected_effect{return Err(PublicApiError::conflict("Reviewed effect changed; reopen review"));}
    let (status,operation)=if accept {("accepted","accept")}else{("refused","reject")};
    let decision=moderation::review_report(&tx,&actor,&id,Some(&c.notes),Some(c.expected_report_version),status,operation).await?;
    if decision.effect!=expected_effect{return Err(PublicApiError::unavailable());}
    let mut metadata=meta(&account);metadata["requestHash"]=json!(hex::encode(Sha256::digest(&bytes)));metadata["requestMethod"]=json!(method);metadata["requestPath"]=json!(uri.to_string());
    let result=json!({"result":"ok","response":"entity","nostr":metadata,"data":{"id":id,"oldStatus":"waiting","status":status,"oldVersion":decision.old_version,"version":decision.new_version,"effect":decision.effect,"contextHash":c.expected_context_hash}});
    super::native_user_reports::require_context_budget(&result)?;tx.commit().await?;Ok(Json(result).into_response())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn command()->Value{json!({"expectedAccountId":"11111111-1111-8111-8111-111111111111","expectedReportVersion":1,"expectedReasonVersion":1,"expectedTargetRevision":"a".repeat(64),"expectedContextHash":"b".repeat(64),"expectedEffect":{},"action":"refuse","notes":"review"})}
    #[test]fn strict_capture_and_body(){let c=command();assert!(Command::parse(&serde_json::to_vec(&c).unwrap()).is_ok());for key in c.as_object().unwrap().keys(){let mut bad=c.clone();bad.as_object_mut().unwrap().remove(key);assert!(Command::parse(&serde_json::to_vec(&bad).unwrap()).is_err());}for(key,value)in[("action",json!("delete")),("expectedReportVersion",json!(0)),("expectedReasonVersion",json!(0)),("expectedEffect",json!(null)),("expectedTargetRevision",json!("x".repeat(64))),("notes",json!("🌌".repeat(5001))),("notes",json!("\0")),("extra",json!(true))]{let mut bad=c.clone();bad[key]=value;assert!(Command::parse(&serde_json::to_vec(&bad).unwrap()).is_err());}let duplicate=serde_json::to_string(&c).unwrap().replacen('{',"{\"action\":\"accept\",",1);assert!(Command::parse(duplicate.as_bytes()).is_err());}
    #[test]fn moderator_requires_both_role_and_grant(){for(role,grant)in[(false,false),(false,true),(true,false),(true,true)]{assert_eq!(authorize(role,grant).is_ok(),role&&grant);}}
    #[test]fn deletion_capture_is_separate_and_strict(){
        let value=json!({"action":"delete","expectedAccountId":"11111111-1111-8111-8111-111111111111","expectedAuthorityRevision":"a".repeat(64),"expectedReportVersion":1,"expectedContextHash":"b".repeat(64),"expectedEffect":{}});
        assert!(DeleteCommand::parse(&serde_json::to_vec(&value).unwrap()).is_ok());assert!(Command::parse(&serde_json::to_vec(&value).unwrap()).is_err());
        for key in value.as_object().unwrap().keys(){let mut bad=value.clone();bad.as_object_mut().unwrap().remove(key);assert!(DeleteCommand::parse(&serde_json::to_vec(&bad).unwrap()).is_err());}
        for (key,changed) in [("action",json!("accept")),("expectedReportVersion",json!(i32::MAX)),("expectedReportVersion",json!(0)),("expectedAuthorityRevision",json!("invalid")),("notes",json!("not a deletion field"))]{let mut bad=value.clone();bad[key]=changed;assert!(DeleteCommand::parse(&serde_json::to_vec(&bad).unwrap()).is_err());}
        let duplicate=serde_json::to_string(&value).unwrap().replacen('{',"{\"action\":\"delete\",",1);assert!(DeleteCommand::parse(duplicate.as_bytes()).is_err());
    }

}
