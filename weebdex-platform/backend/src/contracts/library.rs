//! Exact existing WeebDex payloads, plus explicitly additive instance-only list/progress contracts.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReadingStatus {
    Reading,
    PlanToRead,
    Completed,
    OnHold,
    ReReading,
    Dropped,
}
impl ReadingStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reading => "reading",
            Self::PlanToRead => "plan_to_read",
            Self::Completed => "completed",
            Self::OnHold => "on_hold",
            Self::ReReading => "re_reading",
            Self::Dropped => "dropped",
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateFollow {
    pub status: ReadingStatus,
    pub notification: Option<bool>,
    pub private: Option<bool>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportFollows {
    pub weebdex: Option<BTreeMap<String, ReadingStatus>>,
    pub myanimelist: Option<BTreeMap<String, ReadingStatus>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryBatch {
    pub read_ids: Option<Vec<String>>,
    pub unread_ids: Option<Vec<String>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilePatch {
    pub version: i32,
    pub description: Option<String>,
    pub discord: Option<String>,
    pub twitter: Option<String>,
    pub website: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreferencesPatch {
    pub version: i32,
    pub items_per_page: Option<String>,
    pub languages: Option<Vec<String>>,
    pub show_safe: Option<bool>,
    pub show_suggestive: Option<bool>,
    pub show_erotic: Option<bool>,
    pub show_hentai: Option<bool>,
    pub tags: Option<Vec<String>>,
    pub translated_languages: Option<Vec<String>>,
    pub uploaders: Option<Vec<String>>,
    pub groups: Option<Vec<String>>,
    pub data_saver: Option<bool>,
}
#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ListVisibility {
    Private,
    Unlisted,
    Public,
}
impl ListVisibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Unlisted => "unlisted",
            Self::Public => "public",
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListCreate {
    pub name: String,
    pub description: Option<String>,
    pub visibility: Option<ListVisibility>,
    pub manga_ids: Option<Vec<String>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListPatch {
    pub version: i32,
    pub name: Option<String>,
    pub description: Option<String>,
    pub visibility: Option<ListVisibility>,
    pub manga_ids: Option<Vec<String>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionRequest {
    pub version: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressPatch {
    pub version: i32,
    pub page: i32,
    pub completed: Option<bool>,
}
