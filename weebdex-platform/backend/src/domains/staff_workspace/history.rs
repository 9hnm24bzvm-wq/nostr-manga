use super::reports::{actor,authorize};
use crate::{AppState,domains::catalog::validation as v,public_api::{PublicApiError as Error,PublicApiResult as Result}};
use axum::{Json,extract::{State,Path,Query},http::HeaderMap};
use serde::Deserialize;
use serde_json::{Value,json};
#[derive(Default,Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub(super) struct Filter {report_id:Option<String>,object_id:Option<String>,actor_id:Option<String>,search:Option<String>,page:Option<i64>}
pub(super) async fn list(State(s):State<AppState>,h:HeaderMap,Query(q):Query<Filter>)->Result<Json<Value>> {
    let page=q.page.unwrap_or(1);if !(1..=2000).contains(&page){return Err(Error::bad_request("Invalid history page"));}
    for id in [&q.report_id,&q.object_id,&q.actor_id].into_iter().flatten(){v::id(id)?;}
    let search=q.search.unwrap_or_default();if search.len()>200{return Err(Error::bad_request("Search is too long"));}
    let mut db=s.db.client().await?;let tx=db.transaction().await?;let a=actor(&s,&h,&tx,false).await?;authorize(&a,"report.history")?;
    let predicate="($1::text IS NULL OR r.id=$1) AND ($2::text IS NULL OR r.entity_id=$2) AND ($3::text IS NULL OR a.actor_id=$3) AND ($4='' OR position(lower($4) in lower(r.id||' '||r.entity_id||' '||u.username||' '||COALESCE(a.notes,'')))>0)";
    let total:i64=tx.query_one(&format!("SELECT count(*) FROM platform.report_audit a JOIN platform.reports r ON r.id=a.report_id JOIN platform.users u ON u.id=a.actor_id WHERE {predicate}"),&[&q.report_id,&q.object_id,&q.actor_id,&search]).await?.get(0);
    let rows=tx.query(&format!("SELECT jsonb_build_object('id',a.id::text,'reportId',r.id,'objectId',r.entity_id,'category',r.entity_type,'actorId',a.actor_id,'actor',u.username,'operation',a.operation,'oldStatus',a.old_status,'status',a.new_status,'oldVersion',a.old_version,'version',a.new_version,'reason',a.notes,'effect',a.effect,'createdAt',platform.catalog_timestamp(a.created_at)) FROM platform.report_audit a JOIN platform.reports r ON r.id=a.report_id JOIN platform.users u ON u.id=a.actor_id WHERE {predicate} ORDER BY a.created_at DESC,a.id DESC LIMIT 25 OFFSET $5"),&[&q.report_id,&q.object_id,&q.actor_id,&search,&((page-1)*25)]).await?;
    let result=json!({"scope":"staff-decision-history","actorId":a.user_id,"data":rows.into_iter().map(|r|r.get::<_,Value>(0)).collect::<Vec<_>>(),"page":page,"limit":25,"total":total});tx.commit().await?;Ok(Json(result))
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
