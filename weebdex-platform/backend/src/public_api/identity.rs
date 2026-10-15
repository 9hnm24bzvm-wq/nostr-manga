//! RECONSTRUCTED-UNVERIFIED core seam. Reconcile with later foundation additions.
use super::PublicUuid;
use crate::error::ApiResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Manga,
    Chapter,
    Cover,
    Author,
    Group,
    Tag,
    User,
    List,
    Report,
    ReportReason,
    UploadSession,
    UploadFile,
    Client,
    MangaRelation,
    MangaRecommendation,
}
impl ResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manga => "manga",
            Self::Chapter => "chapter",
            Self::Cover => "cover",
            Self::Author => "author",
            Self::Group => "group",
            Self::Tag => "tag",
            Self::User => "user",
            Self::List => "list",
            Self::Report => "report",
            Self::ReportReason => "report_reason",
            Self::UploadSession => "upload_session",
            Self::UploadFile => "upload_file",
            Self::Client => "client",
            Self::MangaRelation => "manga_relation",
            Self::MangaRecommendation => "manga_recommendation",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicIdTarget {
    NativeResource,
    ProtocolList { owner: String, list_id: String },
    ProtocolPublisher { pubkey: String },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPublicId {
    pub native_id: String,
    pub preferred_id: PublicUuid,
    pub target: PublicIdTarget,
}
#[async_trait]
pub trait PublicIdResolver: Send + Sync {
    async fn resolve(
        &self,
        kind: ResourceKind,
        id: &PublicUuid,
    ) -> ApiResult<Option<ResolvedPublicId>>;
    async fn preferred(&self, kind: ResourceKind, native_id: &str)
    -> ApiResult<Option<PublicUuid>>;
    async fn resolve_many(
        &self,
        kind: ResourceKind,
        ids: &[PublicUuid],
    ) -> ApiResult<BTreeMap<PublicUuid, ResolvedPublicId>>;
    async fn describe_many(
        &self,
        kind: ResourceKind,
        native_ids: &[String],
    ) -> ApiResult<BTreeMap<String, ResolvedPublicId>>;
    async fn preferred_many(
        &self,
        kind: ResourceKind,
        native_ids: &[String],
    ) -> ApiResult<BTreeMap<String, PublicUuid>>;
}
