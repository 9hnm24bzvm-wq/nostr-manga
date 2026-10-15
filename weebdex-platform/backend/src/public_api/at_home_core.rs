//! RECONSTRUCTED, UNVERIFIED real-metadata at-home translator.
//! Native storage/original-module visibility remains authoritative.
use serde::Serialize;
use super::{errors::{PublicApiError,PublicApiResult},PublicUuid};
#[derive(Clone,Debug)] pub struct StoredPage {pub file_name:String,pub dimensions:Option<[u32;2]>}
#[derive(Clone,Debug)] pub struct StoredChapterMedia {
    pub original:Vec<StoredPage>,
    /// None means no actual optimized derivative set. Never synthesize one.
    pub optimized:Option<Vec<StoredPage>>,
}
#[allow(async_fn_in_trait)]pub trait ChapterMediaAuthority {
    /// Resolve aliases without writes; enforce native chapter AND parent deletion,
    /// publication, cached source visibility and current original live visibility.
    /// Return canonical public UUID and actual persisted storage metadata only.
    async fn visible_media(&self,id:PublicUuid)->PublicApiResult<(PublicUuid,StoredChapterMedia)>;
    /// Exact externally configured origin used by the existing data routes.
    fn public_origin(&self)->&str;
}
#[derive(Debug,Serialize)]#[serde(rename_all="camelCase")]
pub struct AtHomeChapter {pub hash:PublicUuid,pub data:Vec<String>,pub data_saver:Vec<String>}
#[derive(Debug,Serialize)]#[serde(rename_all="camelCase")]
pub struct AtHomeProvenance {
    pub delivery_token_type:&'static str,pub originals_only:bool,
    #[serde(skip_serializing_if="Option::is_none")]pub page_dimensions:Option<Vec<[u32;2]>>,
}
#[derive(Debug,Serialize)]#[serde(rename_all="camelCase")]
pub struct AtHomeResponse {
    pub result:&'static str,pub base_url:String,pub chapter:AtHomeChapter,pub nostr:AtHomeProvenance,
}
pub async fn at_home(authority:&impl ChapterMediaAuthority,id:PublicUuid)->PublicApiResult<AtHomeResponse>{
    let (canonical,media)=authority.visible_media(id).await?;
    if media.original.is_empty(){return Err(PublicApiError::not_found("Chapter media is unavailable"));}
    for page in media.original.iter().chain(media.optimized.iter().flatten()){
        validate_filename(&page.file_name)?;
        if page.dimensions.is_some_and(|[w,h]|w==0||h==0){return Err(PublicApiError::internal());}
    }
    let data:Vec<_>=media.original.iter().map(|p|p.file_name.clone()).collect();
    let (data_saver,originals_only)=match media.optimized{
        Some(ref pages) if !pages.is_empty()=>{
            if pages.len()!=data.len(){return Err(PublicApiError::unavailable());}
            (pages.iter().map(|p|p.file_name.clone()).collect(),false)
        },
        _=>(data.clone(),true),
    };
    let dimensions:Option<Vec<_>>=media.original.iter().map(|p|p.dimensions).collect();
    Ok(AtHomeResponse{result:"ok",base_url:authority.public_origin().trim_end_matches('/').to_owned(),
        chapter:AtHomeChapter{hash:canonical,data,data_saver},nostr:AtHomeProvenance{
            delivery_token_type:"public_uuid",originals_only,page_dimensions:dimensions,
        }})
}
fn validate_filename(value:&str)->PublicApiResult<()> {
    if value.is_empty()||value.len()>4096||value=="."||value==".."||value.chars().any(|c|c=='/'||c=='\\'||c.is_control()){
        return Err(PublicApiError::internal());
    }
    Ok(())
}
/* Required routing and tests from the original implementation:
 * Both /data/{canonicalChapterUuid}/{opaqueFilename} and
 * /data-saver/{canonicalChapterUuid}/{actualOptimizedFilename} delegate to the
 * SAME existing byte authority/range/HEAD handler. Never serve arbitrary paths.
 * Preserve original hash/native-ID compatibility routes and storage filenames.
 * Reserve /api, /data, /data-saver, /covers from successful SPA fallback,
 * including malformed/short paths with Accept:text/html.
 * Native and protocol media preflight both require parent cached source-visible
 * flag in addition to the existing original live authority.
 * Verify distinct actual optimized bytes, original bytes, HEAD/ranges,
 * cached withdrawal, deletion and actual original/256/512 cover dimensions.
 */
