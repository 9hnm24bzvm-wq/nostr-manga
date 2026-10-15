//! RECONSTRUCTED-UNVERIFIED original String-backed public UUID codec.
use serde::{Deserialize, Deserializer, Serialize};
use std::{fmt, str::FromStr};
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct PublicUuid(String);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidPublicUuid;
impl PublicUuid {
    pub fn parse(value: &str) -> Result<Self, InvalidPublicUuid> {
        if value.len() != 36
            || !value.bytes().enumerate().all(|(i, b)| {
                if matches!(i, 8 | 13 | 18 | 23) {
                    b == b'-'
                } else {
                    b.is_ascii_hexdigit()
                }
            })
        {
            return Err(InvalidPublicUuid);
        }
        Ok(Self(value.to_ascii_lowercase()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl FromStr for PublicUuid {
    type Err = InvalidPublicUuid;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}
impl fmt::Display for PublicUuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl fmt::Display for InvalidPublicUuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("A hyphenated UUID is required")
    }
}
impl std::error::Error for InvalidPublicUuid {}
impl<'de> Deserialize<'de> for PublicUuid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_custom_version_eight_and_normalizes_case() {
        let alias = manga_node::mangadex_compat::uuid("manga", &"a".repeat(64));
        assert_eq!(alias.as_bytes()[14], b'8');
        assert_eq!(PublicUuid::parse(&alias).unwrap().as_str(), alias);
        assert_eq!(
            PublicUuid::parse(&alias.to_uppercase()).unwrap().as_str(),
            alias
        );
    }
    #[test]
    fn rejects_native_ids_hashes_and_malformed_ids() {
        for value in [
            "abc123DEF0".into(),
            "a".repeat(64),
            "list_0123456789abcdef0123456789abcdef".into(),
            "00000000-0000-0000-0000-00000000000g".into(),
            "000000000000-0000-0000-000000000000".into(),
        ] {
            assert!(PublicUuid::parse(&value).is_err());
        }
    }
}
