//! RECONSTRUCTED, UNVERIFIED generic MDex-shaped envelopes.
use super::PublicUuid;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum ResourceType {
    #[serde(rename = "manga")]
    Manga,
    #[serde(rename = "chapter")]
    Chapter,
    #[serde(rename = "cover_art")]
    CoverArt,
    #[serde(rename = "author")]
    Author,
    #[serde(rename = "artist")]
    Artist,
    #[serde(rename = "scanlation_group")]
    ScanlationGroup,
    #[serde(rename = "tag")]
    Tag,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "creator")]
    Creator,
    #[serde(rename = "leader")]
    Leader,
    #[serde(rename = "member")]
    Member,
    #[serde(rename = "custom_list")]
    CustomList,
    #[serde(rename = "report")]
    Report,
    #[serde(rename = "report_reason")]
    ReportReason,
    #[serde(rename = "upload_session")]
    UploadSession,
    #[serde(rename = "upload_session_file")]
    UploadSessionFile,
    #[serde(rename = "client")]
    Client,
    #[serde(rename = "manga_relation")]
    MangaRelation,
    #[serde(rename = "manga_recommendation")]
    MangaRecommendation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Relationship {
    pub id: PublicUuid,
    #[serde(rename = "type")]
    pub resource_type: ResourceType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resource {
    pub id: PublicUuid,
    #[serde(rename = "type")]
    pub resource_type: ResourceType,
    pub attributes: Value,
    #[serde(default)]
    pub relationships: Vec<Relationship>,
}
#[derive(Debug, Serialize)]
pub struct EntityResponse<T> {
    pub result: &'static str,
    pub response: &'static str,
    pub data: T,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}
impl<T> EntityResponse<T> {
    pub fn new(data: T) -> Self {
        Self {
            result: "ok",
            response: "entity",
            data,
            extensions: Map::new(),
        }
    }
}
#[derive(Debug, Serialize)]
pub struct CollectionResponse<T> {
    pub result: &'static str,
    pub response: &'static str,
    pub data: Vec<T>,
    pub limit: u32,
    pub offset: u32,
    pub total: u64,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}
impl<T> CollectionResponse<T> {
    pub fn new(data: Vec<T>, limit: u32, offset: u32, total: u64) -> Self {
        Self {
            result: "ok",
            response: "collection",
            data,
            limit,
            offset,
            total,
            extensions: Map::new(),
        }
    }
}
