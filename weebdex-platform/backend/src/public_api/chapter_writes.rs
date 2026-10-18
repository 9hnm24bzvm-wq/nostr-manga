//! Signed chapter annotations and instance removal. Original metadata/images are
//! immutable; claims grant local catalogue ownership only. Source deletion is a
//! durable request for original moderator review, never automatic approval.
use super::{PublicApiError, PublicApiResult, PublicUuid, ResourceKind,
    auth::bound_account_write, editorial::{expected_account,take_expected_account}, catalogue_dto::AliasIndex, dto::{EntityResponse, Resource},
    uploads::{authorize, authorize_account_query, resolve}};
use crate::{AppState, domains::catalog::{CatalogRepository, validation}, protocol::deletions};
use axum::{Json, Router, extract::{OriginalUri, Path, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode}, response::{IntoResponse, Response}, routing::{post, put}};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use tokio_postgres::Transaction;

pub fn router() -> Router<AppState> {
    Router::new().route("/chapter/{p0}", put(update).delete(remove))
        .route("/chapter/{p0}/claim", post(claim))
}
fn number(text: &str) -> bool {
    if text.len()>32 { return false; }
    if text.is_empty() { return true; }
    let text=if text.as_bytes().last().is_some_and(u8::is_ascii_lowercase) { &text[..text.len()-1] } else { text };
    let mut parts=text.split('.'); let whole=parts.next().unwrap();
    !whole.is_empty() && whole.bytes().all(|b|b.is_ascii_digit()) && (whole=="0" || !whole.starts_with('0'))
        && parts.next().is_none_or(|fraction|!fraction.is_empty() && fraction.bytes().all(|b|b.is_ascii_digit())) && parts.next().is_none()
}
fn normalize(mut patch: Value) -> PublicApiResult<Value> {
    validation::allowed(&patch,&["title","volume","chapter","translatedLanguage","groups","version"])?;
    validation::version(&patch)?;
    validation::string(&patch,"title",255,false)?;
    for field in ["volume","chapter"] {
        if let Some(text)=validation::string(&patch,field,32,false)? {
            if !number(&text) { return Err(PublicApiError::bad_request("Invalid chapter or volume number")); }
        }
    }
    if let Some(language)=validation::string(&patch,"translatedLanguage",12,false)? { validation::language(&language)?; }
    if let Some(groups)=patch.get("groups") {
        let groups=groups.as_array().filter(|groups|groups.len()<=10)
            .ok_or_else(||PublicApiError::bad_request("Select at most ten distinct groups"))?;
        let mut seen=BTreeSet::new();
        for group in groups {
            let group=PublicUuid::parse(group.as_str().ok_or_else(||PublicApiError::bad_request("Expected a group UUID"))?)?;
            if !seen.insert(group) { return Err(PublicApiError::bad_request("Duplicate group UUID")); }
        }
    }
    if let Some(language)=patch.as_object_mut().unwrap().remove("translatedLanguage") { patch["language"]=language; }
    Ok(patch)
}
async fn resource(tx:&Transaction<'_>, native:Value) -> PublicApiResult<Json<EntityResponse<Resource>>> {
    let aliases=AliasIndex::load_in(tx,&[(ResourceKind::Chapter,native.clone())]).await?;
    let includes=["manga","scanlation_group","user"].into_iter().map(str::to_owned).collect();
    let data=aliases.resource(ResourceKind::Chapter,&native,&includes)?;
    Ok(Json(EntityResponse::new(data)))
}
async fn update(State(state):State<AppState>, Path(chapter):Path<String>, OriginalUri(uri):OriginalUri,
    headers:HeaderMap, request:Request) -> PublicApiResult<Json<EntityResponse<Resource>>> {
    let chapter=PublicUuid::parse(&chapter)?;
    let (cap,body)=authorize(&state,&uri,&headers,request,16*1024,false).await?;
    let mut input=serde_json::from_slice(&body).map_err(|_|PublicApiError::bad_request("Invalid chapter request body"))?;
    let expected=take_expected_account(&mut input)?;
    let mut patch=normalize(input)?;
    let mut client=state.db.client().await?; let tx=client.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;
    expected_account(&tx,&actor,expected.as_ref()).await?;
    let chapter=resolve(&tx,ResourceKind::Chapter,&chapter).await?;
    if let Some(groups)=patch.get_mut("groups") {
        let mut native=Vec::new();
        for group in groups.as_array().unwrap() {
            native.push(resolve(&tx,ResourceKind::Group,&PublicUuid::parse(group.as_str().unwrap())?).await?);
        }
        *groups=json!(native);
    }
    let native=CatalogRepository::update_chapter(&tx,&chapter,&actor,&patch).await?;
    let result=resource(&tx,native).await?; tx.commit().await?; Ok(result)
}
async fn claim(State(state):State<AppState>, Path(chapter):Path<String>, OriginalUri(uri):OriginalUri,
    headers:HeaderMap, request:Request) -> PublicApiResult<Json<EntityResponse<Resource>>> {
    let chapter=PublicUuid::parse(&chapter)?;
    let (cap,expected)=authorize_account_query(&state,&uri,&headers,request).await?;
    let mut client=state.db.client().await?; let tx=client.transaction().await?;
    let actor=bound_account_write(&tx,&cap).await?;
    expected_account(&tx,&actor,expected.as_ref()).await?;
    let chapter=resolve(&tx,ResourceKind::Chapter,&chapter).await?;
    let native=CatalogRepository::claim_chapter(&tx,&chapter,&actor).await?;
    let result=resource(&tx,native).await?; tx.commit().await?; Ok(result)
}
async fn remove(State(state):State<AppState>, Path(chapter):Path<String>, OriginalUri(uri):OriginalUri,
    headers:HeaderMap, request:Request) -> PublicApiResult<Response> {
    let chapter=PublicUuid::parse(&chapter)?;
    let (expected,mode)=removal_query(&uri)?;
    let (cap,_)=super::uploads::authorize_body(&state,&uri,&headers,request,1,true).await?;
    let mut client=state.db.client().await?; let tx=client.transaction().await?;
    // Authorize and lock the local review transition in this transaction.
    // Independent source submission uses a fresh guard after local commit;
    // no chapter row lock crosses the original source-generation operation.
    let actor=bound_account_write(&tx,&cap).await?;
    expected_account(&tx,&actor,expected.as_ref()).await?;
    let chapter=resolve(&tx,ResourceKind::Chapter,&chapter).await?;
    let source=CatalogRepository::check_chapter_delete_authority(&tx,&chapter,&actor).await?;
    let approves_hide=removal_approval(mode,actor.is_moderator(),actor.has_perm("chapter.remove"))?;
    if approves_hide {
        // This explicit moderator action approves this instance's local hide.
        // Moderator status alone does not authorize signing the original owner.
        tx.query("SELECT id FROM platform.reports WHERE entity_type='chapter' AND entity_id=$1 AND reason_id='chapter_removal_request' AND status='waiting' AND deleted_at IS NULL ORDER BY id FOR UPDATE",&[&chapter]).await?;
        CatalogRepository::delete_chapter(&tx,&chapter,&actor).await?;
        let rows=tx.query("UPDATE platform.reports SET status='accepted',reviewed_by=$2,notes='Explicit moderator chapter removal',version=version+1,updated_at=clock_timestamp() WHERE entity_type='chapter' AND entity_id=$1 AND reason_id='chapter_removal_request' AND status='waiting' AND deleted_at IS NULL RETURNING id,version",&[&chapter,&actor.user_id()]).await?;
        for row in rows {
            let id:String=row.get(0); let version:i32=row.get(1);
            tx.execute("INSERT INTO platform.report_audit(report_id,actor_id,operation,old_status,new_status,old_version,new_version) VALUES($1,$2,'accept','waiting','accepted',$3,$4)",&[&id,&actor.user_id(),&(version-1),&version]).await?;
        }
        let mut response=StatusCode::NO_CONTENT.into_response();
        response.headers_mut().insert("x-weebdex-instance-removal",HeaderValue::from_static("soft-hidden"));
        response.headers_mut().insert("x-weebdex-source-deletion-status",HeaderValue::from_static(if source.is_some(){"independent-original-moderation"}else{"instance-local-only"}));
        tx.commit().await?; return Ok(response);
    }
    // Commit the authorized local review request BEFORE any independent source
    // signing. Stale/rate-limited requests have no original side effects, and a
    // missing original key cannot prevent a legitimate local claimant's queue.
    let existing=tx.query_opt("SELECT id,target_version FROM platform.reports WHERE submitter_id=$1 AND entity_type='chapter' AND entity_id=$2 AND reason_id='chapter_removal_request' AND status='waiting' AND deleted_at IS NULL FOR UPDATE",&[&actor.user_id(),&chapter]).await?;
    let version:i32=tx.query_opt("SELECT version FROM platform.catalog_chapters WHERE id=$1 AND deleted_at IS NULL FOR UPDATE",&[&chapter]).await?.ok_or_else(PublicApiError::not_found)?.get(0);
    // The unlocked preflight can precede a concurrent ownership claim. Recheck
    // current owner/delegation and source/parent visibility after acquiring the
    // chapter lock, before attaching a durable request to this version.
    let source=CatalogRepository::check_chapter_delete_authority(&tx,&chapter,&actor).await?;
    let report=if let Some(row)=existing {
        if row.get::<_,Option<i32>>(1)!=Some(version) { return Err(PublicApiError::conflict("Chapter changed since this pending removal request; ask a moderator to review it")); }
        row.get::<_,String>(0)
    } else {
        let count:i64=tx.query_one("SELECT count(*) FROM platform.reports WHERE submitter_id=$1 AND created_at>clock_timestamp()-interval '1 hour'",&[&actor.user_id()]).await?.get(0);
        if count>=20 { return Err(crate::error::ApiError::busy().into()); }
        let report=validation::new_id("report");
        tx.execute("INSERT INTO platform.reports(id,submitter_id,entity_type,entity_id,reason_id,body,target_version) VALUES($1,$2,'chapter',$3,'chapter_removal_request','Authorized instance chapter removal request',$4)",&[&report,&actor.user_id(),&chapter,&version]).await?;
        tx.execute("INSERT INTO platform.report_audit(report_id,actor_id,operation,new_status,new_version) VALUES($1,$2,'create','waiting',1)",&[&report,&actor.user_id()]).await?;
        report
    };
    let report=super::uploads::preferred(&tx,ResourceKind::Report,&report).await?;
    tx.commit().await?;
    let (receipt,source_status)=if source.is_none() { (None,"instance-local-only") } else {
        let result:crate::error::ApiResult<_>=async {
            let mut client=state.db.client().await?;let guard=client.transaction().await?;
            // This is a fresh transaction, so recheck the exact admitted proof,
            // current binding/account grants and native group authority again.
            let actor=bound_account_write(&guard,&cap).await.map_err(|e|crate::error::ApiError::new(e.status,"source_recheck_failed",e.detail))?;
            expected_account(&guard,&actor,expected.as_ref()).await.map_err(|e|crate::error::ApiError::new(e.status,"source_account_intent_changed",e.detail))?;
            CatalogRepository::check_chapter_delete_authority(&guard,&chapter,&actor).await?;
            let receipt=deletions::request_native_chapter_deletion(&state,&actor,&chapter).await?;
            guard.commit().await?;Ok(receipt)
        }.await;
        match result {
            Ok(Some(receipt))=>(Some(receipt),"moderator-review-requested"),
            Ok(None)=>(None,"instance-local-only"),
            Err(error) if error.code=="source_deletion_not_authorized"=>(None,"not-authorized"),
            // Local review is already durable. A source error cannot turn it
            // into a fabricated rejection, approval or automatic source retry.
            Err(_)=>(None,"unconfirmed"),
        }
    };
    let mut response=(StatusCode::ACCEPTED,Json(json!({"result":"ok","status":"pending-moderation","requestId":report,
        "instanceRemoval":"pending-moderation","sourceDeletionStatus":source_status,"sourceDeletionRequestId":receipt.as_ref().map(|r|&r.request_id)}))).into_response();
    response.headers_mut().insert("x-weebdex-instance-removal",HeaderValue::from_static("pending-moderation"));
    response.headers_mut().insert("x-weebdex-source-deletion-status",HeaderValue::from_static(source_status));
    if let Some(receipt)=receipt {response.headers_mut().insert("x-weebdex-source-deletion-request",HeaderValue::from_str(&receipt.request_id).map_err(|_|PublicApiError::unavailable())?);}
    Ok(response)
}

#[derive(Clone,Copy,Debug,PartialEq)]
enum RemovalMode { RequestReview, ApproveHide }
fn removal_query(uri:&axum::http::Uri)->PublicApiResult<(Option<Value>,Option<RemovalMode>)> {
    let Some(query)=uri.query() else{return Ok((None,None));};
    let mut account=None;let mut mode=None;let mut seen=std::collections::BTreeSet::new();
    for (key,value) in url::form_urlencoded::parse(query.as_bytes()) {
        if !seen.insert(key.to_string()){return Err(PublicApiError::bad_request("Duplicate chapter removal query"));}
        match key.as_ref() {
            "expectedAccountId"=>account=Some(json!(PublicUuid::parse(&value)?)),
            "removalMode"=>mode=Some(match value.as_ref(){"request-review"=>RemovalMode::RequestReview,"approve-hide"=>RemovalMode::ApproveHide,_=>return Err(PublicApiError::bad_request("Invalid chapter removalMode"))}),
            _=>return Err(PublicApiError::bad_request("Unsupported chapter removal query")),
        }
    }
    if seen.is_empty(){return Err(PublicApiError::bad_request("Empty chapter removal query"));}
    Ok((account,mode))
}
fn removal_approval(mode:Option<RemovalMode>,moderator:bool,grant:bool)->PublicApiResult<bool> {
    match mode {
        Some(RemovalMode::RequestReview)=>Ok(false),
        Some(RemovalMode::ApproveHide) if !moderator || !grant=>Err(PublicApiError::forbidden("Explicit instance hide requires the current moderator chapter removal grant")),
        Some(RemovalMode::ApproveHide)=>Ok(true),
        None=>Ok(moderator && grant), // Existing external API compatibility only.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn captured_chapter_removal_effect_is_strict_and_never_upgraded() {
        let id="11111111-1111-8111-8111-111111111111";
        assert_eq!(removal_query(&"/chapter/id".parse().unwrap()).unwrap(),(None,None));
        let (_,mode)=removal_query(&format!("/chapter/id?expectedAccountId={id}&removalMode=request-review").parse().unwrap()).unwrap();
        assert_eq!(mode,Some(RemovalMode::RequestReview));
        for (moderator,grant) in [(false,false),(true,false),(false,true),(true,true)] {
            assert!(!removal_approval(Some(RemovalMode::RequestReview),moderator,grant).unwrap());
            assert_eq!(removal_approval(Some(RemovalMode::ApproveHide),moderator,grant).is_ok(),moderator && grant);
            assert_eq!(removal_approval(None,moderator,grant).unwrap(),moderator && grant);
        }
        for query in ["", "removalMode=", "removalMode=remove", "removalMode=request-review&removalMode=approve-hide", "expectedAccountId=native", "unknown=1"] {
            assert!(removal_query(&format!("/chapter/id?{query}").parse().unwrap()).is_err());
        }
    }
    #[test]
    fn chapter_edit_has_only_versioned_metadata_and_public_uuid_groups() {
        let id="00000000-0000-4000-8000-000000000001";
        assert_eq!(normalize(json!({"version":1,"title":"","volume":"2.5a","chapter":"0","translatedLanguage":"en","groups":[id]})).unwrap()["language"],"en");
        for bad in [json!({"version":0}),json!({"version":1,"data":[]}),json!({"version":1,"pageOrder":[]}),json!({"version":1,"language":"en"}),json!({"version":1,"translatedLanguage":"??"}),json!({"version":1,"groups":[id,id]}),json!({"version":1,"groups":["NativeGrp1"]}),json!({"version":1,"volume":"01"}),json!({"version":1,"chapter":"1..2"}),json!({"version":1,"chapter":"a"})] { assert!(normalize(bad).is_err()); }
    }
}
