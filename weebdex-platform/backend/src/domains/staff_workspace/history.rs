use super::reports::{actor,authorize};
use crate::{AppState,domains::catalog::validation as v,public_api::{PublicApiError as Error,PublicApiResult as Result}};
use axum::{Json,extract::{State,Path,Query},http::HeaderMap};
use serde::Deserialize;
use serde_json::{Value,json};
const EXPORT_ROWS:i64=250;
const EXPORT_BYTES:usize=262144;
const OPERATIONS:[&str;12]=["create","accept","reject","delete","claim","unclaim","assign","progress","priority","appeal","reopen","correct"];
#[derive(Default,Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub(super) struct Filter {report_id:Option<String>,object_id:Option<String>,actor_id:Option<String>,search:Option<String>,page:Option<i64>,from:Option<String>,to:Option<String>,operation:Option<String>,export:Option<String>}
fn valid_date(text:&str)->bool {
    let bytes=text.as_bytes();if bytes.len()!=10||bytes[4]!=b'-'||bytes[7]!=b'-'||!bytes.iter().enumerate().all(|(i,c)|i==4||i==7||c.is_ascii_digit()){return false;}
    let year=text[..4].parse::<u32>().unwrap_or(0);let month=text[5..7].parse::<usize>().unwrap_or(0);let day=text[8..].parse::<u32>().unwrap_or(0);
    if !(1970..=9999).contains(&year)||!(1..=12).contains(&month){return false;}
    let leap=year%4==0&&(year%100!=0||year%400==0);let days=[31,if leap{29}else{28},31,30,31,30,31,31,30,31,30,31];(1..=days[month-1]).contains(&day)
}
// Every cell is quoted; dangerous spreadsheet prefixes are made literal AFTER
// controls/line separators/invisible formatting are normalized. No raw effect JSON.
fn csv_cell(text:&str)->String {
    let mut safe:String=text.chars().map(|c|if c.is_control()||matches!(c,'\u{2028}'|'\u{2029}'|'\u{feff}'|'\u{200b}'..='\u{200f}'|'\u{202a}'..='\u{202e}'|'\u{2060}'..='\u{206f}'){ ' ' }else{c}).collect();
    if safe.trim_start().starts_with(['=','+','-','@','＝','＋','－','＠']){safe.insert(0,'\'');}
    format!("\"{}\"",safe.replace('\"',"\"\""))
}
fn outcome(row:&Value)->&'static str {match row["effect"]["action"].as_str(){
    Some("instance_soft_hide")=>"Hidden on this instance only",Some("grant_group_leadership")=>"Local group leadership granted",Some("restore_local_visibility")=>"Local visibility restored",Some("restore_previous_group_membership")=>"Previous local membership restored",Some("reopen")=>"Reopened; previous effects unchanged",_=>"Report workflow only"
}}
fn csv_row(row:&Value)->String {
    let keys=["createdAt","reportId","category","objectId","actor","operation","oldStatus","status","version","reason"];
    let mut cells:Vec<String>=keys.iter().map(|key|{let value=&row[*key];csv_cell(&if let Some(text)=value.as_str(){text.to_owned()}else if let Some(number)=value.as_i64(){number.to_string()}else{String::new()})}).collect();cells.push(csv_cell(outcome(row)));cells.join(",")+"\r\n"
}
pub(super) async fn list(State(s):State<AppState>,h:HeaderMap,Query(q):Query<Filter>)->Result<Json<Value>> {
    let page=q.page.unwrap_or(1);if !(1..=2000).contains(&page){return Err(Error::bad_request("Invalid history page"));}
    for id in [&q.report_id,&q.object_id,&q.actor_id].into_iter().flatten(){v::id(id)?;}
    for date in [&q.from,&q.to].into_iter().flatten(){if !valid_date(date){return Err(Error::bad_request("Use a valid UTC date in YYYY-MM-DD format (1970–9999)"));}}
    if q.from.as_ref().zip(q.to.as_ref()).is_some_and(|(from,to)|from>to){return Err(Error::bad_request("The from date must not be after the through date"));}
    if q.operation.as_deref().is_some_and(|op|!OPERATIONS.contains(&op)){return Err(Error::bad_request("Choose a supported history operation"));}
    let export=q.export.is_some();if q.export.as_deref().is_some_and(|mode|mode!="csv")||export&&page!=1{return Err(Error::bad_request("CSV export starts at the newest matching result; use export=csv and page=1"));}
    let search=q.search.unwrap_or_default();if search.len()>200{return Err(Error::bad_request("Search is too long"));}
    let mut db=s.db.client().await?;let tx=db.transaction().await?;let a=actor(&s,&h,&tx,false).await?;authorize(&a,"report.history")?;
    let predicate="($1::text IS NULL OR r.id=$1) AND ($2::text IS NULL OR r.entity_id=$2) AND ($3::text IS NULL OR a.actor_id=$3) AND ($4='' OR position(lower($4) in lower(r.id||' '||r.entity_id||' '||u.username||' '||COALESCE(a.notes,'')))>0) AND ($5::text IS NULL OR a.created_at>=($5::text::date::timestamp AT TIME ZONE 'UTC')) AND ($6::text IS NULL OR a.created_at<(($6::text::date+1)::timestamp AT TIME ZONE 'UTC')) AND ($7::text IS NULL OR a.operation=$7)";
    let total:i64=tx.query_one(&format!("SELECT count(*) FROM platform.report_audit a JOIN platform.reports r ON r.id=a.report_id JOIN platform.users u ON u.id=a.actor_id WHERE {predicate}"),&[&q.report_id,&q.object_id,&q.actor_id,&search,&q.from,&q.to,&q.operation]).await?.get(0);
    let limit=if export{EXPORT_ROWS}else{25};
    let rows=tx.query(&format!("SELECT jsonb_build_object('id',a.id::text,'reportId',r.id,'objectId',r.entity_id,'category',r.entity_type,'actorId',a.actor_id,'actor',u.username,'operation',a.operation,'oldStatus',a.old_status,'status',a.new_status,'oldVersion',a.old_version,'version',a.new_version,'reason',a.notes,'effect',a.effect,'createdAt',platform.catalog_timestamp(a.created_at)),count(*) OVER() FROM platform.report_audit a JOIN platform.reports r ON r.id=a.report_id JOIN platform.users u ON u.id=a.actor_id WHERE {predicate} ORDER BY a.created_at DESC,a.id DESC LIMIT $8 OFFSET $9"),&[&q.report_id,&q.object_id,&q.actor_id,&search,&q.from,&q.to,&q.operation,&limit,&((page-1)*25)]).await?;
    let result=if export {
        // Window total and rows come from one statement snapshot. The fixed first
        // page makes an empty result unambiguously zero, even under concurrent work.
        let matching:i64=rows.first().map(|row|row.get(1)).unwrap_or(0);
        let mut csv=String::from("\u{feff}\"Date (UTC)\",\"Report\",\"Subject type\",\"Subject reference\",\"Reviewer\",\"Operation\",\"Previous status\",\"Status\",\"Revision\",\"Reason\",\"Outcome\"\r\n");let mut exported=0;
        for row in rows {let line=csv_row(&row.get::<_,Value>(0));if csv.len()+line.len()>EXPORT_BYTES{break;}csv.push_str(&line);exported+=1;}
        json!({"scope":"staff-decision-history-export","actorId":a.user_id,"bytes":csv.len(),"csv":csv,"exported":exported,"total":matching,"truncated":exported<matching,"rowLimit":EXPORT_ROWS,"byteLimit":EXPORT_BYTES})
    }else {json!({"scope":"staff-decision-history","actorId":a.user_id,"data":rows.into_iter().map(|r|r.get::<_,Value>(0)).collect::<Vec<_>>(),"page":page,"limit":25,"total":total})};
    tx.commit().await?;Ok(Json(result))
}
pub(super) async fn personal(State(s):State<AppState>,Path(id):Path<String>,h:HeaderMap)->Result<Json<Value>> {
    v::id(&id)?;let mut db=s.db.client().await?;let tx=db.transaction().await?;let a=actor(&s,&h,&tx,false).await?;
    let row=tx.query_opt("SELECT version,status,appeal_state FROM platform.reports WHERE id=$1 AND submitter_id=$2 AND deleted_at IS NULL",&[&id,&a.user_id]).await?.ok_or_else(Error::not_found)?;
    // Do not leak assignment conversations, reviewer IDs, pre-membership state,
    // private profile data or another person's report through a personal view.
    let rows=tx.query("SELECT jsonb_build_object('operation',operation,'status',new_status,'reason',notes,'effect',jsonb_build_object('action',effect->'action','scope','instance-local'),'createdAt',platform.catalog_timestamp(created_at)) FROM platform.report_audit WHERE report_id=$1 AND operation IN ('create','accept','reject','appeal','reopen','correct') ORDER BY id LIMIT 101",&[&id]).await?;
    if rows.len()>100{return Err(Error::conflict("This report has a long history. Contact the instance moderator for a complete review."));}
    let result=json!({"scope":"personal-report-history","actorId":a.user_id,"id":id,"version":row.get::<_,i32>(0),"status":row.get::<_,String>(1),"appealState":row.get::<_,String>(2),"data":rows.into_iter().map(|r|r.get::<_,Value>(0)).collect::<Vec<_>>()});tx.commit().await?;Ok(Json(result))
}
