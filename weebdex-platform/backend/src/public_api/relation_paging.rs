//! Bounded relation-page presentation. This module grants no account authority.
//! The caller supplies an authorized repeatable snapshot and complete resources.
use super::{PublicApiError, PublicApiResult, PublicUuid, dto::{CollectionResponse, Resource}, query::PublicQuery};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

pub(crate) const MAX_RELATIONS: usize = 4096;
pub(crate) const MAX_PAGE: usize = 32;
pub(crate) const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
/// Ordered native edge key, relation label, target native key and actual target
/// version. These keys are hashed, never returned as public resource IDs.
pub(crate) type SnapshotEdge = (String, String, String, i32);

pub(crate) struct RelationPageRequest {
    pub limit: u32,
    pub offset: u32,
    pub owner_version: Option<i32>,
    pub snapshot: Option<String>,
}
impl RelationPageRequest {
    pub fn parse(raw: Option<&str>, require_owner_version: bool, extra_allowed: &[&str]) -> PublicApiResult<(PublicQuery, Self)> {
        let q = PublicQuery::parse_unchecked(raw)?;
        let mut allowed = vec!["limit", "offset", "ownerVersion", "snapshot"];
        allowed.extend_from_slice(extra_allowed);
        q.require_allowed(&allowed)?;
        let limit = q.bounded_usize("limit", MAX_PAGE, 1, MAX_PAGE)? as u32;
        let offset = q.bounded_usize("offset", 0, 0, MAX_RELATIONS)? as u32;
        let owner_version = q.first("ownerVersion").map(|_| q.bounded_usize("ownerVersion", 1, 1, i32::MAX as usize).map(|v| v as i32)).transpose()?;
        let snapshot = q.first("snapshot").map(|v| {
            if v.len() != 64 || !v.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
                return Err(PublicApiError::bad_request("Invalid relation snapshot token"));
            }
            Ok(v.to_owned())
        }).transpose()?;
        if (require_owner_version || offset > 0) && owner_version.is_none() {
            return Err(PublicApiError::bad_request("Relation pages require the captured ownerVersion"));
        }
        if offset > 0 && snapshot.is_none() {
            return Err(PublicApiError::bad_request("Relation continuation requires the captured snapshot"));
        }
        Ok((q, Self { limit, offset, owner_version, snapshot }))
    }
    pub fn check(&self, owner_version: i32, snapshot: &str) -> PublicApiResult<()> {
        if self.owner_version.is_some_and(|v| v != owner_version)
            || self.snapshot.as_ref().is_some_and(|v| v != snapshot) {
            return Err(PublicApiError::conflict("Relation snapshot changed; refresh from the first page"));
        }
        Ok(())
    }
}

pub(crate) fn snapshot(owner_native: &str, owner_version: i32, persisted_total: u64, edges: &[SnapshotEdge]) -> PublicApiResult<String> {
    if owner_version <= 0 || edges.len() > MAX_RELATIONS || persisted_total > MAX_RELATIONS as u64 || persisted_total < edges.len() as u64 || edges.iter().any(|edge| edge.3 <= 0) {
        return Err(PublicApiError::unavailable());
    }
    let bytes = serde_json::to_vec(&json!(["weebdex.relation-snapshot.v1", owner_native, owner_version, persisted_total, edges]))
        .map_err(|_| PublicApiError::unavailable())?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

pub(crate) struct RelationPageMeta {
    pub owner_id: PublicUuid,
    pub owner_version: i32,
    pub snapshot: String,
    pub total: u64,
    /// Actual hidden+visible persisted count is only exposed to authorized
    /// editors. Public readers receive only the visible eligible `total`.
    pub persisted_total: Option<u64>,
    pub version_alias: bool,
    pub extensions: Map<String, Value>,
}

/// Fit whole resources into the existing transport budget. A shortened prefix
/// always carries its exact nextOffset; one oversized resource fails closed.
pub(crate) fn response(data: Vec<Resource>, request: &RelationPageRequest, meta: RelationPageMeta) -> PublicApiResult<CollectionResponse<Resource>> {
    if data.len() > request.limit as usize || meta.total > MAX_RELATIONS as u64 || meta.owner_version <= 0
        || (!data.is_empty() && u64::from(request.offset) + data.len() as u64 > meta.total) {
        return Err(PublicApiError::unavailable());
    }
    let mut response = CollectionResponse::new(data, request.limit, request.offset, meta.total);
    response.extensions = meta.extensions;
    response.extensions.insert("ownerMangaId".into(), json!(meta.owner_id));
    response.extensions.insert("ownerVersion".into(), json!(meta.owner_version));
    response.extensions.insert("snapshot".into(), json!(meta.snapshot));
    if meta.version_alias { response.extensions.insert("version".into(), json!(meta.owner_version)); }
    if let Some(total) = meta.persisted_total { response.extensions.insert("persistedTotal".into(), json!(total)); }
    loop {
        let next = u64::from(request.offset) + response.data.len() as u64;
        response.extensions.insert("nextOffset".into(), if next < meta.total { json!(next) } else { Value::Null });
        if serde_json::to_vec(&response).map_err(|_| PublicApiError::unavailable())?.len() <= MAX_RESPONSE_BYTES {
            if response.data.is_empty() && u64::from(request.offset) < meta.total {
                return Err(PublicApiError::unavailable());
            }
            return Ok(response);
        }
        if response.data.len() <= 1 {
            return Err(PublicApiError::new(axum::http::StatusCode::SERVICE_UNAVAILABLE, "Relation resource exceeds response budget", "A complete relation resource cannot fit the existing 1 MiB transport budget"));
        }
        response.data.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::public_api::dto::ResourceType;
    fn id() -> PublicUuid { PublicUuid::parse("11111111-1111-8111-8111-111111111111").unwrap() }
    fn meta(total: u64) -> RelationPageMeta { RelationPageMeta { owner_id:id(),owner_version:2,snapshot:"a".repeat(64),total,persisted_total:None,version_alias:false,extensions:Map::new() } }
    fn resource(bytes: usize) -> Resource { Resource { id:id(),resource_type:ResourceType::MangaRelation,attributes:json!({"complete":"x".repeat(bytes)}),relationships:vec![] } }
    #[test]
    fn strict_relation_page_guards_and_continuity() {
        let (_, q) = RelationPageRequest::parse(None,false,&[]).unwrap(); assert_eq!(q.limit,32);
        for raw in ["limit=0","limit=33","offset=4097","offset=1","ownerVersion=0","ownerVersion=-1","ownerVersion=2147483648","snapshot=A","limit=1&limit=2","unknown=1"] {
            assert!(RelationPageRequest::parse(Some(raw),false,&[]).is_err(),"{raw}");
        }
        assert!(RelationPageRequest::parse(None,true,&[]).is_err());
        let raw=format!("limit=1&offset=1&ownerVersion=2&snapshot={}","a".repeat(64));
        let (_, q)=RelationPageRequest::parse(Some(&raw),false,&[]).unwrap();
        assert!(q.check(2,&"a".repeat(64)).is_ok());assert!(q.check(3,&"a".repeat(64)).is_err());assert!(q.check(2,&"b".repeat(64)).is_err());
    }
    #[test]
    fn relation_snapshot_binds_order_kind_target_and_versions() {
        let a=vec![("edge".into(),"sequel".into(),"target".into(),1)];
        let digest=snapshot("owner",2,1,&a).unwrap();assert_eq!(digest.len(),64);
        assert_ne!(digest,snapshot("other",2,1,&a).unwrap());assert_ne!(digest,snapshot("owner",3,1,&a).unwrap());assert_ne!(digest,snapshot("owner",2,2,&a).unwrap());
        for tuple in [("other","sequel","target",1),("edge","prequel","target",1),("edge","sequel","other",1),("edge","sequel","target",2)] {
            assert_ne!(digest,snapshot("owner",2,1,&vec![(tuple.0.into(),tuple.1.into(),tuple.2.into(),tuple.3)]).unwrap());
        }
        let mut b=a.clone();b.push(("edge2".into(),"prequel".into(),"target2".into(),1));let first=snapshot("owner",2,2,&b).unwrap();b.reverse();assert_ne!(first,snapshot("owner",2,2,&b).unwrap());
    }
    #[test]
    fn whole_resource_budget_uses_truthful_continuation_and_no_hidden_count() {
        let (_, q)=RelationPageRequest::parse(None,false,&[]).unwrap();
        let response=response(vec![resource(600_000),resource(600_000)],&q,meta(2)).unwrap();
        assert_eq!(response.data.len(),1);assert_eq!(response.extensions["nextOffset"],1);assert!(!response.extensions.contains_key("persistedTotal"));assert_eq!(response.data[0].attributes["complete"].as_str().unwrap().len(),600_000);
        assert!(serde_json::to_vec(&response).unwrap().len()<=MAX_RESPONSE_BYTES);
        assert!(super::response(vec![resource(MAX_RESPONSE_BYTES)],&q,meta(1)).is_err());
        assert!(super::response(vec![],&q,meta(1)).is_err());
        let response=super::response(vec![],&q,meta(0)).unwrap();assert!(response.extensions["nextOffset"].is_null());
    }
}
