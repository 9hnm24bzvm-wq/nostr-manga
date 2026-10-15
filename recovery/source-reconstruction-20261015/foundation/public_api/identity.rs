//! RECONSTRUCTED, UNVERIFIED: additive public identity boundary.
use std::{collections::BTreeMap, fmt, future::Future, str::FromStr};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use uuid::Uuid;
use super::errors::{PublicApiError, PublicApiResult};

pub const MAX_PUBLIC_ID_BATCH: usize = 1024;

/// Public resource UUID. Never reinterpret this as a Nostr hash/pubkey or native PK.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PublicUuid(Uuid);
impl PublicUuid {
    pub fn parse(value: &str) -> PublicApiResult<Self> {
        if value.len() != 36 || value.bytes().any(|b| b.is_ascii_uppercase()) {
            return Err(PublicApiError::bad_request("Expected a canonical public UUID"));
        }
        let parsed = Uuid::parse_str(value)
            .map_err(|_| PublicApiError::bad_request("Expected a canonical public UUID"))?;
        if parsed.hyphenated().to_string() != value {
            return Err(PublicApiError::bad_request("Expected a canonical public UUID"));
        }
        Ok(Self(parsed))
    }
    pub fn from_uuid(value: Uuid) -> Self { Self(value) }
    pub fn as_uuid(&self) -> &Uuid { &self.0 }
}
impl fmt::Display for PublicUuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.hyphenated().fmt(f) }
}
impl FromStr for PublicUuid {
    type Err = PublicApiError;
    fn from_str(value: &str) -> Result<Self, Self::Err> { Self::parse(value) }
}
impl Serialize for PublicUuid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok,S::Error> {
        serializer.collect_str(self)
    }
}
impl<'de> Deserialize<'de> for PublicUuid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self,D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum ResourceKind {
    Manga, Chapter, Cover, Author, Group, Tag, User, List, Report, UploadSession,
    UploadFile, Client, MangaRelation, MangaRecommendation,
}
impl ResourceKind {
    pub const ALL: [Self;14] = [Self::Manga,Self::Chapter,Self::Cover,Self::Author,
        Self::Group,Self::Tag,Self::User,Self::List,Self::Report,Self::UploadSession,
        Self::UploadFile,Self::Client,Self::MangaRelation,Self::MangaRecommendation];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manga=>"manga", Self::Chapter=>"chapter", Self::Cover=>"cover",
            Self::Author=>"author", Self::Group=>"group", Self::Tag=>"tag",
            Self::User=>"user", Self::List=>"list", Self::Report=>"report",
            Self::UploadSession=>"upload_session", Self::UploadFile=>"upload_file",
            Self::Client=>"client", Self::MangaRelation=>"manga_relation",
            Self::MangaRecommendation=>"manga_recommendation",
        }
    }
}
impl fmt::Display for ResourceKind {
    fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result { f.write_str(self.as_str()) }
}
impl FromStr for ResourceKind {
    type Err=PublicApiError;
    fn from_str(value:&str)->Result<Self,Self::Err> {
        Self::ALL.into_iter().find(|kind|kind.as_str()==value)
            .ok_or_else(||PublicApiError::bad_request("Unknown resource kind"))
    }
}

/// This is presentation provenance. It never confers authentication/authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicIdTarget {
    NativeResource,
    ProtocolPublisher { pubkey: String },
    ProtocolList { owner: String, list_id: String },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPublicId {
    pub native_id: String,
    pub preferred_id: PublicUuid,
    pub target: PublicIdTarget,
}

/// Async-compatible without async-trait or a new SQL client dependency.
/// Implementations must be read-only: resolving an unknown UUID never registers it.
/// Visibility and permissions are separately enforced by each operation.
pub trait PublicIdResolver: Send + Sync {
    fn resolve(&self,kind:ResourceKind,id:PublicUuid)
        -> impl Future<Output=PublicApiResult<Option<ResolvedPublicId>>> + Send;
    fn preferred(&self,kind:ResourceKind,native_id:&str)
        -> impl Future<Output=PublicApiResult<Option<PublicUuid>>> + Send;
    fn resolve_many(&self,kind:ResourceKind,ids:&[PublicUuid])
        -> impl Future<Output=PublicApiResult<BTreeMap<PublicUuid,ResolvedPublicId>>> + Send;
    fn preferred_many(&self,kind:ResourceKind,native_ids:&[String])
        -> impl Future<Output=PublicApiResult<BTreeMap<String,PublicUuid>>> + Send;
    fn describe_many(&self,kind:ResourceKind,native_ids:&[String])
        -> impl Future<Output=PublicApiResult<BTreeMap<String,ResolvedPublicId>>> + Send;
}

pub fn validate_batch_size(length:usize)->PublicApiResult<()> {
    if length>MAX_PUBLIC_ID_BATCH {
        return Err(PublicApiError::bad_request("Too many public resource identifiers"));
    }
    Ok(())
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn canonical_ids_and_kinds_are_strict() {
        assert!(PublicUuid::parse("11b8d076-e9a2-8f00-b100-c9981c3df1aa").is_ok());
        assert!(PublicUuid::parse("11B8D076-E9A2-8F00-B100-C9981C3DF1AA").is_err());
        assert!(PublicUuid::parse("Manga00001").is_err());
        assert_eq!(ResourceKind::from_str("manga_relation").unwrap(),ResourceKind::MangaRelation);
        assert!(ResourceKind::from_str("artist").is_err()); // same author identity, different role
    }
}
