//! RECONSTRUCTED, UNVERIFIED complete pure companion core.
//! Connect to the real existing Postgres/community/publication authority; no fake data.
use std::collections::BTreeMap;
use serde::{Serialize,Deserialize};
use serde_json::Value;
use super::{errors::{PublicApiError,PublicApiResult},identity::{PublicUuid,ResourceKind,ResolvedPublicId,PublicIdTarget}};

pub const MAX_FORUM_CONTEXTS:usize=100;
#[derive(Clone,Copy,Debug,Eq,PartialEq,Ord,PartialOrd,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum ForumKind {Manga,Chapter,Group}
impl ForumKind {pub fn resource_kind(self)->ResourceKind{match self{Self::Manga=>ResourceKind::Manga,Self::Chapter=>ResourceKind::Chapter,Self::Group=>ResourceKind::Group}}}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForumReference {pub kind:ForumKind,pub id:PublicUuid}
#[derive(Debug,Deserialize)] #[serde(deny_unknown_fields)]
pub struct ContextRequest {pub resources:Vec<ForumReference>}
#[derive(Debug,Serialize)] pub struct ForumContext {pub kind:ForumKind,pub id:PublicUuid,pub thread:Option<Value>}
#[derive(Debug,Serialize)] #[serde(rename_all="camelCase")]
pub struct ForumProvenance {
    pub module:&'static str,pub api_base:&'static str,pub thread_identity:&'static str,
    pub catalogue_identity:&'static str,pub read_only:bool,
}
#[derive(Debug,Serialize)]pub struct ContextResponse{
    pub result:&'static str,pub data:Vec<ForumContext>,pub unavailable:Vec<ForumReference>,pub nostr:ForumProvenance,
}
/// A missing map key means resource hidden/missing; present None means real visible
/// resource without a persisted forum thread. Thread values are the native DTO.
#[allow(async_fn_in_trait)]
pub trait NativeForumRepository {
    async fn resolve_many(&self,kind:ResourceKind,ids:&[PublicUuid])
        ->PublicApiResult<BTreeMap<PublicUuid,ResolvedPublicId>>;
    async fn visible_contexts(&self,kind:ForumKind,native_ids:&[String])
        ->PublicApiResult<BTreeMap<String,Option<Value>>>;
}
pub async fn contexts(repository:&impl NativeForumRepository,request:ContextRequest)
    ->PublicApiResult<ContextResponse>{
    if request.resources.len()>MAX_FORUM_CONTEXTS{return Err(PublicApiError::bad_request("At most 100 resources are allowed"));}
    let mut resolved=BTreeMap::new();let mut visible=BTreeMap::new();
    for kind in [ForumKind::Manga,ForumKind::Chapter,ForumKind::Group]{
        let ids:Vec<_>=request.resources.iter().filter(|r|r.kind==kind).map(|r|r.id).collect();
        if ids.is_empty(){continue}
        let rows=repository.resolve_many(kind.resource_kind(),&ids).await?;
        let native:Vec<_>=rows.values().filter(|r|r.target==PublicIdTarget::NativeResource).map(|r|r.native_id.clone()).collect();
        let contexts=repository.visible_contexts(kind,&native).await?;
        for (id,row) in rows{resolved.insert((kind,id),row);}
        for (id,thread) in contexts{visible.insert((kind,id),thread);}
    }
    let mut data=Vec::new();let mut unavailable=Vec::new();
    for reference in request.resources{
        let Some(resource)=resolved.get(&(reference.kind,reference.id)) else{unavailable.push(reference);continue};
        if resource.target!=PublicIdTarget::NativeResource{unavailable.push(reference);continue}
        let Some(thread)=visible.get(&(reference.kind,resource.native_id.clone())) else{unavailable.push(reference);continue};
        data.push(ForumContext{kind:reference.kind,id:resource.preferred_id,thread:thread.clone()});
    }
    Ok(ContextResponse{result:"ok",data,unavailable,nostr:ForumProvenance{
        module:"native_forum",api_base:"/api",thread_identity:"native",catalogue_identity:"public_uuid",read_only:true,
    }})
}
/* Required real repository visibility, from prior implementation:
 * Manga: deleted_at IS NULL, state='published', protocol_source_visible,
 *        platform.protocol_manga_is_visible(id), then actual thread projection.
 * Chapter: chapter and parent publication/deletion/cached flags and original
 *          platform.protocol_chapter_is_visible(id); actual chapter thread.
 * Group: existing platform.protocol_group_is_visible(id), not deleted.
 * Use platform.catalog_thread_json for actual existing native thread DTO;
 * existing community visibility and reply counts remain authoritative.
 * No GET/companion writes; do not invent thread IDs or MDex numeric forum IDs.
 */
