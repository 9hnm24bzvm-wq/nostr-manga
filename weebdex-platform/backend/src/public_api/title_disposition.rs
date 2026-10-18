//! Signed instance title locks and review-first removal. A local disposition
//! never signs an original title deletion or changes the original event/bytes.
use super::{
    PublicApiError, PublicApiResult, PublicUuid, ResourceKind,
    auth::{BoundAccountWrite, bound_account_write}, catalogue_dto::AliasIndex,
    dto::{EntityResponse, Resource}, uploads::{authorize, preferred, resolve},
};
use crate::{AppState, domains::catalog::{mutations as core, repository, validation as v}};
use axum::{Json, Router, extract::{OriginalUri, Path, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode}, middleware,
    response::{IntoResponse, Response}, routing::{delete, post}};
use serde_json::{Value, json};
use tokio_postgres::Transaction;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/manga/{p0}", delete(remove))
        // Retained-platform extensions, not MangaDex lock operations.
        .route("/manga/{p0}/lock", post(lock))
        .route("/manga/{p0}/unlock", post(unlock))
        .layer(middleware::map_response(super::upload_extensions::private_response))
}
#[derive(Clone,Copy,PartialEq,Eq)]
enum RemovalMode { RequestReview, ApproveHide }
fn input(body: &[u8],removal:bool) -> PublicApiResult<(i32, PublicUuid,Option<RemovalMode>)> {
    let body:Value=serde_json::from_slice(body).map_err(|_|PublicApiError::bad_request("Expected signed title version/account intent"))?;
    v::allowed(&body,if removal {&["version","expectedAccountId","removalMode"]}else{&["version","expectedAccountId"]})?;
    let version=v::version(&body)?;
    let account=PublicUuid::parse(body["expectedAccountId"].as_str().ok_or_else(||PublicApiError::bad_request("Signed expectedAccountId is required"))?)?;
    let mode=if removal {Some(match body["removalMode"].as_str(){Some("request-review")=>RemovalMode::RequestReview,Some("approve-hide")=>RemovalMode::ApproveHide,_=>return Err(PublicApiError::bad_request("Signed removalMode must be request-review or approve-hide"))})}else{None};
    Ok((version,account,mode))
}
async fn account_intent(tx:&Transaction<'_>,actor:&BoundAccountWrite,expected:&PublicUuid)->PublicApiResult<()> {
    if resolve(tx,ResourceKind::User,expected).await?!=actor.user_id(){return Err(PublicApiError::conflict("Native account binding changed; reopen the intended title account"));}
    Ok(())
}
async fn resource(tx:&Transaction<'_>,native:Value)->PublicApiResult<Json<EntityResponse<Resource>>> {
    let aliases=AliasIndex::load_in(tx,&[(ResourceKind::Manga,native.clone())]).await?;
    let includes=["author","artist","tag","creator","cover_art"].into_iter().map(str::to_owned).collect();
    let mut data=aliases.resource(ResourceKind::Manga,&native,&includes)?;
    if !data.attributes["nostr"].is_object(){data.attributes["nostr"]=json!({});}
    data.attributes["nostr"]["metadataScope"]=json!("instance-local");
    data.attributes["nostr"]["originalEventRewritten"]=json!(false);
    Ok(Json(EntityResponse::new(data)))
}
async fn lock(state:State<AppState>,path:Path<String>,uri:OriginalUri,headers:HeaderMap,request:Request)->PublicApiResult<Json<EntityResponse<Resource>>>{set_lock(state,path,uri,headers,request,true).await}
async fn unlock(state:State<AppState>,path:Path<String>,uri:OriginalUri,headers:HeaderMap,request:Request)->PublicApiResult<Json<EntityResponse<Resource>>>{set_lock(state,path,uri,headers,request,false).await}
async fn set_lock(State(state):State<AppState>,Path(manga):Path<String>,OriginalUri(uri):OriginalUri,headers:HeaderMap,request:Request,locked:bool)->PublicApiResult<Json<EntityResponse<Resource>>>{
    let manga=PublicUuid::parse(&manga)?;
    let (cap,body)=authorize(&state,&uri,&headers,request,1024,false).await?;
    let (version,expected_account,_)=input(&body,false)?;
    let mut db=state.db.client().await?;let tx=db.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;account_intent(&tx,&actor,&expected_account).await?;
    let manga=resolve(&tx,ResourceKind::Manga,&manga).await?;
    let native=core::manga_lock_in(&tx,&actor,&manga,locked,Some(version)).await?;
    let result=resource(&tx,native).await?;tx.commit().await?;Ok(result)
}
async fn remove(State(state):State<AppState>,Path(manga):Path<String>,OriginalUri(uri):OriginalUri,headers:HeaderMap,request:Request)->PublicApiResult<Response>{
    let public=PublicUuid::parse(&manga)?;
    let (cap,body)=authorize(&state,&uri,&headers,request,1024,false).await?;
    let (version,expected_account,mode)=input(&body,true)?;
    let mut db=state.db.client().await?;let tx=db.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;account_intent(&tx,&actor,&expected_account).await?;
    let manga=resolve(&tx,ResourceKind::Manga,&public).await?;
    // Target-first locks match moderator report acceptance. A saved UUID never
    // bypasses current hidden/withdrawn source state or an immutable command.
    let row=tx.query_opt("SELECT owner_id,version,protocol_series_id FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id) FOR UPDATE",&[&manga]).await?.ok_or_else(PublicApiError::not_found)?;
    if row.get::<_,i32>(1)!=version{return Err(PublicApiError::conflict("Title changed; reload before requesting removal"));}
    repository::require_mutable_manga(&tx,&manga).await?;
    let source:Option<String>=row.get(2);
    if mode==Some(RemovalMode::ApproveHide) {
        if !actor.is_moderator(){return Err(PublicApiError::forbidden("Instance moderator removal approval required"));}
        actor.require_permission("manga.delete")?;
        // The explicit signed moderator action approves this instance hide only.
        tx.query("SELECT id FROM platform.reports WHERE entity_type='manga' AND entity_id=$1 AND reason_id='manga_removal_request' AND status='waiting' AND deleted_at IS NULL ORDER BY id FOR UPDATE",&[&manga]).await?;
        let next=core::manga_delete_in(&tx,&actor,&manga,Some(version)).await?;
        let reports=tx.query("UPDATE platform.reports SET status='accepted',reviewed_by=$2,notes='Explicit moderator instance title removal',version=version+1,updated_at=clock_timestamp() WHERE entity_type='manga' AND entity_id=$1 AND reason_id='manga_removal_request' AND status='waiting' AND deleted_at IS NULL RETURNING id,version",&[&manga,&actor.user_id()]).await?;
        for report in reports {
            let id:String=report.get(0);let current:i32=report.get(1);
            tx.execute("INSERT INTO platform.report_audit(report_id,actor_id,operation,old_status,new_status,old_version,new_version,effect) VALUES($1,$2,'accept','waiting','accepted',$3,$4,$5)",&[&id,&actor.user_id(),&(current-1),&current,&json!({"action":"instance_soft_hide","old_version":version,"new_version":next,"source_deletion":"not_requested","original_event_rewritten":false})]).await?;
        }
        tx.commit().await?;
        let mut response=StatusCode::NO_CONTENT.into_response();
        response.headers_mut().insert("x-weebdex-instance-removal",HeaderValue::from_static("soft-hidden"));
        response.headers_mut().insert("x-weebdex-source-deletion-status",HeaderValue::from_static("not-requested"));
        return Ok(response);
    }
    actor.require_permission("manga.create")?;
    if row.get::<_,String>(0)!=actor.user_id(){return Err(PublicApiError::forbidden("Only the current title owner can request instance removal"));}
    let pending=tx.query_opt("SELECT id,target_version FROM platform.reports WHERE submitter_id=$1 AND entity_type='manga' AND entity_id=$2 AND reason_id='manga_removal_request' AND status='waiting' AND deleted_at IS NULL FOR UPDATE",&[&actor.user_id(),&manga]).await?;
    let report=if let Some(row)=pending {
        if row.get::<_,Option<i32>>(1)!=Some(version){return Err(PublicApiError::conflict("Title changed since this pending removal request; ask a moderator to review it"));}
        row.get::<_,String>(0)
    }else{
        let count:i64=tx.query_one("SELECT count(*) FROM platform.reports WHERE submitter_id=$1 AND created_at>clock_timestamp()-interval '1 hour'",&[&actor.user_id()]).await?.get(0);
        if count>=20{return Err(crate::error::ApiError::busy().into());}
        let id=v::new_id("report");
        tx.execute("INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,body,target_version) VALUES($1,$2,'manga',$3,'manga_removal_request','Authorized current owner requests instance title removal; no original deletion requested',$4)",&[&id,&actor.user_id(),&manga,&version]).await?;
        tx.execute("INSERT INTO platform.report_audit(report_id,actor_id,operation,new_status,new_version,effect) VALUES($1,$2,'create','waiting',1,$3)",&[&id,&actor.user_id(),&json!({"action":"request_instance_removal","target_version":version,"source_deletion":"not_requested"})]).await?;
        id
    };
    let report=preferred(&tx,ResourceKind::Report,&report).await?;
    tx.commit().await?;
    let mut response=(StatusCode::ACCEPTED,Json(json!({"result":"ok","status":"pending-moderation","requestId":report,"mangaId":public,"targetVersion":version,"instanceRemoval":"pending-moderation","sourceDeletionStatus":"not-requested","sourceEventId":source,"originalEventRewritten":false}))).into_response();
    response.headers_mut().insert("x-weebdex-instance-removal",HeaderValue::from_static("pending-moderation"));
    response.headers_mut().insert("x-weebdex-source-deletion-status",HeaderValue::from_static("not-requested"));
    Ok(response)
}
#[cfg(test)]mod tests{use super::*;#[test]fn disposition_requires_exact_version_account_and_removal_effect(){let id="11111111-1111-8111-8111-111111111111";let parse=|value:Value,remove|input(serde_json::to_string(&value).unwrap().as_bytes(),remove);assert_eq!(parse(json!({"version":3,"expectedAccountId":id}),false).unwrap().0,3);assert!(parse(json!({"version":3,"expectedAccountId":id,"removalMode":"request-review"}),true).unwrap().2==Some(RemovalMode::RequestReview));assert!(parse(json!({"version":3,"expectedAccountId":id,"removalMode":"approve-hide"}),true).unwrap().2==Some(RemovalMode::ApproveHide));for value in[json!({"version":3}),json!({"version":0,"expectedAccountId":id}),json!({"version":3,"expectedAccountId":"NativeUser"}),json!({"version":3,"expectedAccountId":id,"originalDelete":true})]{assert!(parse(value,false).is_err());}assert!(parse(json!({"version":3,"expectedAccountId":id}),true).is_err());assert!(parse(json!({"version":3,"expectedAccountId":id,"removalMode":"automatic"}),true).is_err());}}
