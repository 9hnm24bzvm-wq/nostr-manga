//! Stable per-item rejection codes. Operational/storage failures are not skips.
use serde::Serialize;
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RejectionClass {
    Unsupported,
    PolicyRejected,
    Invalid,
}
impl RejectionClass {
    pub fn code(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::PolicyRejected => "policy_rejected",
            Self::Invalid => "invalid",
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReason {
    PublicationSchema,
    ManifestSchema,
    RequiredFeatures,
    Lifecycle,
    LocalFormat,
    LocalSize,
    LocalRemoval,
    ReviewRejected,
    PublicationPointer,
    Manifest,
    Original,
    InventoryKind,
    LibraryPointer,
    LibraryObject,
}
impl RejectionReason {
    pub fn code(self) -> &'static str {
        match self {
            Self::PublicationSchema => "publication_schema",
            Self::ManifestSchema => "manifest_schema",
            Self::RequiredFeatures => "required_features",
            Self::Lifecycle => "lifecycle",
            Self::LocalFormat => "local_format",
            Self::LocalSize => "local_size",
            Self::LocalRemoval => "local_removal",
            Self::ReviewRejected => "review_rejected",
            Self::PublicationPointer => "publication_pointer",
            Self::Manifest => "manifest",
            Self::Original => "original",
            Self::InventoryKind => "inventory_kind",
            Self::LibraryPointer => "library_pointer",
            Self::LibraryObject => "library_object",
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, thiserror::Error)]
#[error("{class:?}: {reason:?}")]
pub struct TransferRejection {
    pub class: RejectionClass,
    pub reason: RejectionReason,
}
impl TransferRejection {
    pub fn unsupported(reason: RejectionReason) -> Self {
        Self {
            class: RejectionClass::Unsupported,
            reason,
        }
    }
    pub fn invalid(reason: RejectionReason) -> Self {
        Self {
            class: RejectionClass::Invalid,
            reason,
        }
    }
    pub fn policy(reason: RejectionReason) -> Self {
        Self {
            class: RejectionClass::PolicyRejected,
            reason,
        }
    }
}
/// Inspect signed envelope versions without guessing from error-message text.
pub fn pointer_version(content: &str, invalid: RejectionReason) -> Result<(), TransferRejection> {
    let value: serde_json::Value =
        serde_json::from_str(content).map_err(|_| TransferRejection::invalid(invalid))?;
    let version = value
        .get("version")
        .and_then(|v| v.as_u64())
        .ok_or(TransferRejection::invalid(invalid))?;
    if version != 1 {
        return Err(TransferRejection::unsupported(
            RejectionReason::PublicationSchema,
        ));
    }
    Ok(())
}
/// Canonical CBOR is already decoded under size/depth budgets. Unknown optional
/// fields stay optional; only explicit version/required contracts cause skips.
pub fn manifest_support(
    value: &ciborium::Value,
    invalid: RejectionReason,
) -> Result<(), TransferRejection> {
    manifest_support_features(value, invalid, &[])
}
pub fn manifest_support_features(
    value: &ciborium::Value,
    invalid: RejectionReason,
    supported: &[&str],
) -> Result<(), TransferRejection> {
    let map = value.as_map().ok_or(TransferRejection::invalid(invalid))?;
    let get = |name: &str| {
        map.iter()
            .find(|(k, _)| k.as_text() == Some(name))
            .map(|(_, v)| v)
    };
    let version = get("version")
        .and_then(|v| v.as_integer())
        .and_then(|v| u64::try_from(v).ok())
        .ok_or(TransferRejection::invalid(invalid))?;
    if version != 1 {
        return Err(TransferRejection::unsupported(
            RejectionReason::ManifestSchema,
        ));
    }
    if let Some(required) = get("required") {
        let values = required
            .as_array()
            .ok_or(TransferRejection::invalid(invalid))?;
        if values.iter().any(|v| v.as_text().is_none_or(str::is_empty)) {
            return Err(TransferRejection::invalid(invalid));
        }
        if values
            .iter()
            .any(|v| !supported.contains(&v.as_text().unwrap()))
        {
            return Err(TransferRejection::unsupported(
                RejectionReason::RequiredFeatures,
            ));
        }
    }
    Ok(())
}
