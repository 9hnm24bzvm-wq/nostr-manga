//! RECONSTRUCTED, UNVERIFIED exact signed-library status adapter core.
//! The authority implementation must use the one existing LibraryObjects Arc,
//! genuine admitted capability, original public manifest and final head recheck.
use std::collections::BTreeMap;
use serde::{Serialize,Deserialize};
use super::{errors::{PublicApiError,PublicApiResult},identity::PublicUuid};
#[derive(Clone,Copy,Debug,Eq,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum ReadingStatus{Reading,OnHold,PlanToRead,Dropped,ReReading,Completed}
impl ReadingStatus{
    pub fn parse(value:&str)->PublicApiResult<Self>{match value{
        "reading"=>Ok(Self::Reading),"on_hold"=>Ok(Self::OnHold),"plan_to_read"=>Ok(Self::PlanToRead),
        "dropped"=>Ok(Self::Dropped),"re_reading"=>Ok(Self::ReReading),"completed"=>Ok(Self::Completed),
        _=>Err(PublicApiError::bad_request("Invalid reading status")),
    }}
}
/// Original signed series ID, actual status, actual source update timestamp.
/// This is not a follow flag and must never be interpreted as one.
#[derive(Clone,Debug)]pub struct StatusEntry{pub series:String,pub status:ReadingStatus,pub updated_at:i64}
#[derive(Clone,Debug)]pub struct StatusSnapshot{pub publisher:String,pub head:String,pub root:String,pub statuses:Vec<StatusEntry>}
#[derive(Clone,Debug,Serialize)]#[serde(rename_all="camelCase")]
pub struct StatusProvenance{pub scope:&'static str,pub head:String,pub root:String,pub follow_flag_inferred:bool}
#[derive(Debug,Serialize)]pub struct StatusesResponse{pub result:&'static str,pub statuses:BTreeMap<PublicUuid,ReadingStatus>,pub nostr:StatusProvenance}
#[derive(Debug,Serialize)]pub struct StatusResponse{pub result:&'static str,pub status:ReadingStatus,pub nostr:StatusProvenance}

#[allow(async_fn_in_trait)]pub trait ReadingStatusAuthority{
    /// Must be an unforgeable original admitted-request capability, never a UUID/account.
    type Capability;
    async fn snapshot(&self,cap:&Self::Capability,series:Option<&str>,status:Option<ReadingStatus>,expected_head:Option<&str>)
        ->PublicApiResult<StatusSnapshot>;
    /// Map only locally projected, published and original-live-visible series;
    /// include both cached projection flags and current original visibility.
    async fn visible_public_ids(&self,original_series:&[String])->PublicApiResult<BTreeMap<String,PublicUuid>>;
    /// Resolve a public manga UUID read-only, enforce the same publication/live gates,
    /// and return the actual original signed series ID. Unknown and hidden are 404.
    async fn visible_original_series(&self,id:PublicUuid)->PublicApiResult<String>;
    /// Recheck exact same publisher, admitted permit version and current public head.
    /// Must happen after alias/DTO work, using the original capability transaction.
    async fn verify_snapshot(&self,cap:&Self::Capability,snapshot:&StatusSnapshot)->PublicApiResult<()>;
}
fn provenance(snapshot:&StatusSnapshot)->StatusProvenance{StatusProvenance{
    scope:"signed_public_library",head:snapshot.head.clone(),root:snapshot.root.clone(),follow_flag_inferred:false,
}}
pub async fn all<A:ReadingStatusAuthority>(authority:&A,cap:&A::Capability,status:Option<ReadingStatus>,expected_head:Option<&str>)
    ->PublicApiResult<StatusesResponse>{
    let snapshot=authority.snapshot(cap,None,status,expected_head).await?;
    if snapshot.statuses.len()>10_000{return Err(PublicApiError::unavailable())}
    let ids:Vec<_>=snapshot.statuses.iter().map(|s|s.series.clone()).collect();
    let public=authority.visible_public_ids(&ids).await?;
    let mut statuses=BTreeMap::new();
    for entry in &snapshot.statuses{
        if status.is_some_and(|s|s!=entry.status){return Err(PublicApiError::internal())}
        if let Some(id)=public.get(&entry.series){statuses.insert(*id,entry.status);}
    }
    authority.verify_snapshot(cap,&snapshot).await?;
    Ok(StatusesResponse{result:"ok",statuses,nostr:provenance(&snapshot)})
}
pub async fn single<A:ReadingStatusAuthority>(authority:&A,cap:&A::Capability,id:PublicUuid,expected_head:Option<&str>)
    ->PublicApiResult<StatusResponse>{
    let series=authority.visible_original_series(id).await?;
    let snapshot=authority.snapshot(cap,Some(&series),None,expected_head).await?;
    let status=snapshot.statuses.iter().find(|s|s.series==series).map(|s|s.status)
        .ok_or_else(||PublicApiError::not_found("Reading status not found"))?;
    authority.verify_snapshot(cap,&snapshot).await?;
    Ok(StatusResponse{result:"ok",status,nostr:provenance(&snapshot)})
}
