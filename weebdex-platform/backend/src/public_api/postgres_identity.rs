//! RECONSTRUCTED-UNVERIFIED after executor reset. Recompile and rerun tests.
//! Durable presentation aliases, never resource visibility or account authority.
//! Alias resolution only selects a native key; authorize through the domain repo.
use super::{PublicIdResolver, PublicIdTarget, PublicUuid, ResolvedPublicId, ResourceKind};
use crate::{
    db::PgStore,
    error::{ApiError, ApiResult},
};
use async_trait::async_trait;
use std::collections::BTreeMap;

pub const MAX_PUBLIC_ID_BATCH: usize = 1024;
#[derive(Clone)]
pub struct PostgresPublicIdResolver {
    db: PgStore,
}
impl PostgresPublicIdResolver {
    pub fn new(db: PgStore) -> Self {
        Self { db }
    }

    /// Resolve against the caller's protected transaction so binding, authority,
    /// UUID inputs and mutation share one snapshot/lock lifetime. Read-only.
    pub async fn resolve_many_in<C: tokio_postgres::GenericClient + Sync>(
        client: &C,
        kind: ResourceKind,
        ids: &[PublicUuid],
    ) -> ApiResult<BTreeMap<PublicUuid, ResolvedPublicId>> {
        validate_batch(ids.len())?;
        if ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let ids = ids.iter().map(PublicUuid::as_str).collect::<Vec<_>>();
        let rows=client.query("SELECT requested.id,a.native_id,p.public_id::text,a.principal_type FROM unnest($2::text[]) requested(id) JOIN platform.public_resource_ids a ON a.kind=$1 AND a.public_id=requested.id::uuid JOIN platform.public_resource_ids p ON p.kind=a.kind AND p.native_id=a.native_id AND p.preferred",&[&kind.as_str(),&ids]).await?;
        rows.into_iter()
            .map(|row| {
                Ok((
                    persisted_uuid(row.get::<_, &str>(0))?,
                    ResolvedPublicId {
                        native_id: row.get(1),
                        preferred_id: persisted_uuid(row.get::<_, &str>(2))?,
                        target: persisted_target(
                            kind,
                            row.get::<_, &str>(1),
                            row.get::<_, &str>(3),
                        )?,
                    },
                ))
            })
            .collect()
    }

    /// Preferred IDs and typed targets for one bounded relationship batch.
    pub async fn describe_many(
        &self,
        kind: ResourceKind,
        native_ids: &[String],
    ) -> ApiResult<BTreeMap<String, ResolvedPublicId>> {
        validate_batch(native_ids.len())?;
        for native_id in native_ids {
            validate_native_id(native_id)?;
        }
        if native_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let rows = self.db.client().await?.query(
            "SELECT native_id,public_id::text,principal_type FROM platform.public_resource_ids WHERE kind=$1 AND native_id=ANY($2::text[]) AND preferred",
            &[&kind.as_str(), &native_ids],
        ).await?;
        rows.into_iter()
            .map(|row| {
                let native_id: String = row.get(0);
                let resolved = ResolvedPublicId {
                    preferred_id: persisted_uuid(row.get::<_, &str>(1))?,
                    target: persisted_target(kind, &native_id, row.get::<_, &str>(2))?,
                    native_id: native_id.clone(),
                };
                Ok((native_id, resolved))
            })
            .collect()
    }

    /// Explicit writer; ordinary reads never create resource IDs.
    pub async fn ensure(&self, kind: ResourceKind, native_id: &str) -> ApiResult<PublicUuid> {
        validate_native_id(native_id)?;
        let row = self
            .db
            .client()
            .await?
            .query_one(
                "SELECT platform.public_ensure_resource_id($1,$2)::text",
                &[&kind.as_str(), &native_id],
            )
            .await
            .map_err(write_error)?;
        persisted_uuid(row.get::<_, &str>(0))
    }

    /// Preserve imported aliases without replacing the preferred public ID.
    pub async fn register_alias(
        &self,
        kind: ResourceKind,
        native_id: &str,
        alias: &PublicUuid,
    ) -> ApiResult<PublicUuid> {
        validate_native_id(native_id)?;
        let row = self
            .db
            .client()
            .await?
            .query_one(
                "SELECT platform.public_register_resource_alias($1,$2,$3::text::uuid)::text",
                &[&kind.as_str(), &native_id, &alias.as_str()],
            )
            .await
            .map_err(write_error)?;
        persisted_uuid(row.get::<_, &str>(0))
    }

    /// Reconcile source aliases for exactly one platform projection.
    pub async fn sync_protocol_aliases(
        &self,
        kind: ResourceKind,
        native_id: &str,
    ) -> ApiResult<()> {
        validate_native_id(native_id)?;
        self.db
            .client()
            .await?
            .query_one(
                "SELECT platform.public_sync_protocol_aliases($1,$2)",
                &[&kind.as_str(), &native_id],
            )
            .await
            .map_err(write_error)?;
        Ok(())
    }
}

#[async_trait]
impl PublicIdResolver for PostgresPublicIdResolver {
    async fn resolve(
        &self,
        kind: ResourceKind,
        id: &PublicUuid,
    ) -> ApiResult<Option<ResolvedPublicId>> {
        Ok(self
            .resolve_many(kind, std::slice::from_ref(id))
            .await?
            .remove(id))
    }
    async fn preferred(
        &self,
        kind: ResourceKind,
        native_id: &str,
    ) -> ApiResult<Option<PublicUuid>> {
        let native_id = native_id.to_owned();
        Ok(self
            .preferred_many(kind, std::slice::from_ref(&native_id))
            .await?
            .remove(&native_id))
    }
    async fn resolve_many(
        &self,
        kind: ResourceKind,
        ids: &[PublicUuid],
    ) -> ApiResult<BTreeMap<PublicUuid, ResolvedPublicId>> {
        validate_batch(ids.len())?;
        if ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let ids: Vec<&str> = ids.iter().map(PublicUuid::as_str).collect();
        // Cast bounded input rather than indexed UUID columns.
        let rows = self.db.client().await?.query(
            "SELECT requested.id,a.native_id,p.public_id::text,a.principal_type FROM unnest($2::text[]) requested(id) JOIN platform.public_resource_ids a ON a.kind=$1 AND a.public_id=requested.id::uuid JOIN platform.public_resource_ids p ON p.kind=a.kind AND p.native_id=a.native_id AND p.preferred",
            &[&kind.as_str(), &ids],
        ).await?;
        rows.into_iter()
            .map(|row| {
                Ok((
                    persisted_uuid(row.get::<_, &str>(0))?,
                    ResolvedPublicId {
                        native_id: row.get(1),
                        preferred_id: persisted_uuid(row.get::<_, &str>(2))?,
                        target: persisted_target(
                            kind,
                            row.get::<_, &str>(1),
                            row.get::<_, &str>(3),
                        )?,
                    },
                ))
            })
            .collect()
    }
    async fn describe_many(
        &self,
        kind: ResourceKind,
        native_ids: &[String],
    ) -> ApiResult<BTreeMap<String, ResolvedPublicId>> {
        PostgresPublicIdResolver::describe_many(self, kind, native_ids).await
    }
    async fn preferred_many(
        &self,
        kind: ResourceKind,
        native_ids: &[String],
    ) -> ApiResult<BTreeMap<String, PublicUuid>> {
        validate_batch(native_ids.len())?;
        for native_id in native_ids {
            validate_native_id(native_id)?;
        }
        if native_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let rows = self.db.client().await?.query(
            "SELECT native_id,public_id::text FROM platform.public_resource_ids WHERE kind=$1 AND native_id=ANY($2::text[]) AND preferred",
            &[&kind.as_str(), &native_ids],
        ).await?;
        rows.into_iter()
            .map(|row| Ok((row.get(0), persisted_uuid(row.get::<_, &str>(1))?)))
            .collect()
    }
}

fn validate_batch(len: usize) -> ApiResult<()> {
    if len > MAX_PUBLIC_ID_BATCH {
        return Err(ApiError::bad_request("Too many public resource IDs"));
    }
    Ok(())
}
fn validate_native_id(id: &str) -> ApiResult<()> {
    if id.is_empty() || id.len() > 384 || id.contains('\0') {
        return Err(ApiError::bad_request("Invalid native resource ID"));
    }
    Ok(())
}
fn persisted_uuid(id: &str) -> ApiResult<PublicUuid> {
    PublicUuid::parse(id).map_err(|_| ApiError::unavailable())
}
pub(super) fn persisted_target(
    kind: ResourceKind,
    native_id: &str,
    principal_type: &str,
) -> ApiResult<PublicIdTarget> {
    match principal_type {
        "protocol_list" => persisted_protocol_list_target(kind, native_id),
        "native"
            if (kind != ResourceKind::User || !native_id.starts_with("protocol:"))
                && (kind != ResourceKind::List || !native_id.starts_with("protocol-list:")) =>
        {
            Ok(PublicIdTarget::NativeResource)
        }
        "protocol_publisher" if kind == ResourceKind::User => {
            let pubkey = native_id
                .strip_prefix("protocol:")
                .filter(|key| {
                    key.len() == 64
                        && key
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
                .ok_or_else(ApiError::unavailable)?;
            Ok(PublicIdTarget::ProtocolPublisher {
                pubkey: pubkey.to_owned(),
            })
        }
        _ => Err(ApiError::unavailable()),
    }
}
fn write_error(error: tokio_postgres::Error) -> ApiError {
    match error.code() {
        Some(code) if *code == tokio_postgres::error::SqlState::UNIQUE_VIOLATION => {
            ApiError::conflict("Public resource alias is already assigned")
        }
        Some(code) if *code == tokio_postgres::error::SqlState::FOREIGN_KEY_VIOLATION => {
            ApiError::not_found("Resource not found")
        }
        _ => ApiError::from(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batches_and_native_keys_are_bounded_before_database_work() {
        assert!(validate_batch(MAX_PUBLIC_ID_BATCH).is_ok());
        assert!(validate_batch(MAX_PUBLIC_ID_BATCH + 1).is_err());
        for id in ["", "a\0b", &"a".repeat(385)] {
            assert!(validate_native_id(id).is_err());
        }
        for id in [
            "Abcdef1234",
            "list_0123456789abcdef0123456789abcdef",
            "550e8400-e29b-41d4-a716-446655440000",
        ] {
            assert!(validate_native_id(id).is_ok());
        }
    }
    #[test]
    fn principal_metadata_fails_closed_without_type_or_valid_publisher() {
        assert_eq!(
            persisted_target(
                ResourceKind::User,
                "550e8400-e29b-41d4-a716-446655440000",
                "native"
            )
            .unwrap(),
            PublicIdTarget::NativeResource
        );
        let key = "a".repeat(64);
        assert_eq!(
            persisted_target(
                ResourceKind::User,
                &format!("protocol:{key}"),
                "protocol_publisher"
            )
            .unwrap(),
            PublicIdTarget::ProtocolPublisher { pubkey: key }
        );
        assert!(
            persisted_target(ResourceKind::User, "protocol:bad", "protocol_publisher").is_err()
        );
        assert!(
            persisted_target(
                ResourceKind::Manga,
                &format!("protocol:{}", "a".repeat(64)),
                "protocol_publisher"
            )
            .is_err()
        );
        assert!(
            persisted_target(
                ResourceKind::User,
                &format!("protocol:{}", "a".repeat(64)),
                "native"
            )
            .is_err()
        );
    }
}

// RECONSTRUCTED UNVERIFIED. Add ProtocolList{owner:String,list_id:String} to the
// PublicIdTarget enum. In persisted_target, handle principal_type protocol_list
// by calling this helper. Native List keys with protocol-list: prefix must fail.
fn persisted_protocol_list_target(
    kind: ResourceKind,
    native_id: &str,
) -> ApiResult<PublicIdTarget> {
    if kind != ResourceKind::List {
        return Err(ApiError::unavailable());
    }
    let (owner, list_id) = native_id
        .strip_prefix("protocol-list:")
        .and_then(|value| value.split_once(':'))
        .ok_or_else(ApiError::unavailable)?;
    if !manga_node::auth::valid_hash(owner)
        || list_id.len() != 32
        || !list_id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(ApiError::unavailable());
    }
    Ok(PublicIdTarget::ProtocolList {
        owner: owner.to_owned(),
        list_id: list_id.to_owned(),
    })
}
#[cfg(test)]
mod protocol_list_recovery_tests {
    use super::*;
    #[test]
    fn protocol_lists_require_exact_owner_and_list_type() {
        let owner = "a".repeat(64);
        let list = "b".repeat(32);
        let key = format!("protocol-list:{owner}:{list}");
        assert_eq!(
            persisted_protocol_list_target(ResourceKind::List, &key).unwrap(),
            PublicIdTarget::ProtocolList {
                owner: owner.clone(),
                list_id: list.clone()
            }
        );
        for malformed in [
            format!("protocol-list:bad:{list}"),
            format!("protocol-list:{}:{list}", "A".repeat(64)),
            format!("protocol-list:{owner}:bad"),
            format!("protocol-list:{owner}:{}", "B".repeat(32)),
            format!("{key}:extra"),
            format!("{owner}:{list}"),
        ] {
            assert!(persisted_protocol_list_target(ResourceKind::List, &malformed).is_err());
        }
        assert!(persisted_protocol_list_target(ResourceKind::User, &key).is_err());
        assert!(persisted_target(ResourceKind::List, &key, "native").is_err());
        assert!(persisted_target(ResourceKind::List, &key, "protocol_publisher").is_err());
    }
}
