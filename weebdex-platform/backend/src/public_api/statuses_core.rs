//! Exact signed-library reading status values; no follow state is inferred.
use super::{PublicApiError, PublicApiResult};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadingStatus {
    Reading,
    OnHold,
    PlanToRead,
    Dropped,
    ReReading,
    Completed,
}
impl ReadingStatus {
    pub fn parse(value: &str) -> PublicApiResult<Self> {
        match value {
            "reading" => Ok(Self::Reading),
            "on_hold" => Ok(Self::OnHold),
            "plan_to_read" => Ok(Self::PlanToRead),
            "dropped" => Ok(Self::Dropped),
            "re_reading" => Ok(Self::ReReading),
            "completed" => Ok(Self::Completed),
            _ => Err(PublicApiError::bad_request("Invalid reading status")),
        }
    }
}
