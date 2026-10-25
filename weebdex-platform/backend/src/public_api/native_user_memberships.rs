//! Public, target-only native membership presentation. Never account authority.
//! Pages are live keyset reads, not an atomic cross-request membership snapshot.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdTarget,
    PublicUuid, ResourceKind, query::PublicQuery,
};
use crate::AppState;
use axum::{Json, Router, extract::{Path, RawQuery, State}, routing::get};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

const MAX_PAGE: usize = 32;
const MAX_RESPONSE_BYTES: usize = 65536;

pub fn router() -> Router<AppState> {
    Router::new().route("/nostr/user/{p0}/memberships", get(page))
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all="camelCase")]
struct Cursor { version: u8, user_id: PublicUuid, group_id: PublicUuid, name: String }
impl Cursor {
    fn decode(value: &str) -> PublicApiResult<Self> {
        if value.len() > 2048 || value.is_empty() { return Err(PublicApiError::bad_request("Invalid membership cursor")); }
        let bytes = URL_SAFE_NO_PAD.decode(value).map_err(|_| PublicApiError::bad_request("Invalid membership cursor"))?;
        let cursor: Self = serde_json::from_slice(&bytes).map_err(|_| PublicApiError::bad_request("Invalid membership cursor"))?;
        if cursor.version != 1 || cursor.name.is_empty() || cursor.name.chars().count() > 200 || cursor.name.len() > 800 || cursor.encode()? != value {
            return Err(PublicApiError::bad_request("Invalid membership cursor"));
        }
        Ok(cursor)
    }
    fn encode(&self) -> PublicApiResult<String> {
        let encoded = URL_SAFE_NO_PAD.encode(serde_json::to_vec(self).map_err(|_| PublicApiError::unavailable())?);
        if encoded.len() > 2048 { return Err(PublicApiError::unavailable()); }
        Ok(encoded)
    }
}
struct PageRequest { limit: usize, after: Option<String>, cursor: Option<Cursor> }
impl PageRequest {
    fn parse(raw: Option<&str>) -> PublicApiResult<Self> {
        let query = PublicQuery::parse_unchecked(raw)?;
        query.require_allowed(&["limit", "after"])?;
        let after = query.first("after").map(str::to_owned);
        let cursor = after.as_deref().map(Cursor::decode).transpose()?;
        Ok(Self { limit: query.bounded_usize("limit", MAX_PAGE, 1, MAX_PAGE)?, after, cursor })
    }
}

fn group(id: Option<&str>, principal: Option<&str>, name: &str, version: i32, leader: bool, officer: bool) -> PublicApiResult<Value> {
    if principal != Some("native") || name.is_empty() || name.chars().count() > 200 || name.len() > 800 || version < 1 {
        return Err(PublicApiError::unavailable());
    }
    let id = PublicUuid::parse(id.ok_or_else(PublicApiError::unavailable)?)
        .map_err(|_| PublicApiError::unavailable())?;
    // Deliberately no group metadata, expanded members, user roles or permissions.
    Ok(json!({"id":id,"type":"scanlation_group","attributes":{"name":name,"version":version},"relationships":[],"membership":{"isLeader":leader,"isOfficer":officer}}))
}

async fn page(State(state): State<AppState>, Path(id): Path<String>, RawQuery(raw): RawQuery) -> PublicApiResult<Json<Value>> {
    let request = PageRequest::parse(raw.as_deref())?;
    let requested = PublicUuid::parse(&id)?;
    let mut db = state.db.client().await?;
    let tx = db.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).read_only(true).start().await?;
    let target = PostgresPublicIdResolver::resolve_many_in(&*tx, ResourceKind::User, std::slice::from_ref(&requested)).await?
        .remove(&requested).filter(|row| row.target == PublicIdTarget::NativeResource).ok_or_else(PublicApiError::not_found)?;
    if tx.query_opt("SELECT id FROM platform.users WHERE id=$1 AND NOT disabled", &[&target.native_id]).await?.is_none() {
        return Err(PublicApiError::not_found());
    }
    // Native source order is name,id under the database collation. The opaque
    // cursor contains only already-public fields. Resolve its alias locally;
    // never disclose a native key, even base64-encoded, or trust it as authority.
    let mut anchor_name: Option<String> = None;
    let mut anchor_native: Option<String> = None;
    if let Some(cursor) = &request.cursor {
        if cursor.user_id != target.preferred_id { return Err(PublicApiError::conflict("Membership cursor belongs to another user; refresh")); }
        let anchor = tx.query_opt(
            "SELECT g.id,g.name FROM platform.public_resource_ids a JOIN platform.catalog_groups g ON g.id=a.native_id \
             JOIN platform.catalog_group_members gm ON gm.group_id=g.id \
             WHERE a.kind='group' AND a.public_id=$1::text::uuid AND a.preferred AND a.principal_type='native' \
             AND gm.user_id=$2 AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id)",
            &[&cursor.group_id.as_str(), &target.native_id],
        ).await?.ok_or_else(|| PublicApiError::conflict("Membership continuation changed; refresh from the first page"))?;
        let name: String = anchor.get(1);
        if name != cursor.name { return Err(PublicApiError::conflict("Membership continuation changed; refresh from the first page")); }
        anchor_name = Some(name); anchor_native = Some(anchor.get(0));
    }
    let limit = (request.limit + 1) as i64;
    // Only the target's public membership edge, and <=32+1 rows. A missing alias
    // sorts first on EVERY page, including continuation, and fails closed below.
    // Do not INNER JOIN aliases or call catalog_group_json: either would hide
    // missing identity or unnecessarily expand the group's complete member list.
    let rows = tx.query(
        "SELECT a.public_id::text,a.principal_type,g.name,g.version,gm.is_leader,gm.is_officer \
         FROM platform.catalog_group_members gm JOIN platform.catalog_groups g ON g.id=gm.group_id \
         LEFT JOIN platform.public_resource_ids a ON a.kind='group' AND a.native_id=g.id AND a.preferred \
         WHERE gm.user_id=$1 AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id) \
         AND ($2::text IS NULL OR (g.name,g.id) > ($2::text,$3::text) OR a.public_id IS NULL) \
         ORDER BY (a.public_id IS NULL) DESC,g.name ASC,g.id ASC LIMIT $4",
        &[&target.native_id, &anchor_name, &anchor_native, &limit],
    ).await?;
    let mut data = rows.iter().map(|row| group(row.get(0), row.get(1), row.get(2), row.get(3), row.get(4), row.get(5)))
        .collect::<PublicApiResult<Vec<_>>>()?;
    let more = data.len() > request.limit;
    data.truncate(request.limit);
    let next = if more {
        let last = data.last().ok_or_else(PublicApiError::unavailable)?;
        json!(Cursor { version:1,user_id:target.preferred_id.clone(),group_id:PublicUuid::parse(last["id"].as_str().ok_or_else(PublicApiError::unavailable)?)?,name:last["attributes"]["name"].as_str().ok_or_else(PublicApiError::unavailable)?.to_owned() }.encode()?)
    } else { Value::Null };
    let response = json!({
        "result":"ok","response":"membership_page","data":data,"limit":request.limit,"after":request.after,"nextCursor":next,
        "nostr":{"scope":"public-native-user","source":"persisted-visible-native-memberships","identity":"native_public_user",
            "requestedId":requested,"userId":target.preferred_id,"consistency":"live-keyset-v1"}
    });
    if serde_json::to_vec(&response).map_err(|_| PublicApiError::unavailable())?.len() > MAX_RESPONSE_BYTES {
        return Err(PublicApiError::unavailable());
    }
    tx.commit().await?;
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "11111111-1111-8111-8111-111111111111";
    #[test]
    fn bounded_membership_query_is_explicit() {
        assert_eq!(PageRequest::parse(None).unwrap().limit, 32);
        let cursor = Cursor {version:1,user_id:PublicUuid::parse(ID).unwrap(),group_id:PublicUuid::parse(ID).unwrap(),name:"Visible group".into()}.encode().unwrap();
        assert!(PageRequest::parse(Some(&format!("limit=1&after={cursor}"))).is_ok());
        assert!(Cursor::decode(&(cursor + "=")).is_err());
        for raw in ["limit=0", "limit=33", "limit=1&limit=2", "offset=1", "snapshot=x", "includes[]=member", "after=native", "after="] {
            assert!(PageRequest::parse(Some(raw)).is_err(), "{raw}");
        }
    }
    #[test]
    fn valid_worst_case_names_keep_usable_continuation() {
        for name in ["\u{1}".repeat(200), "🦀".repeat(200)] {
            let cursor = Cursor { version:1,user_id:PublicUuid::parse(ID).unwrap(),group_id:PublicUuid::parse(ID).unwrap(),name:name.clone() };
            let encoded = cursor.encode().unwrap();
            assert!(encoded.len() <= 2048);
            assert_eq!(Cursor::decode(&encoded).unwrap().name, name);
        }
    }
    #[test]
    fn membership_projection_is_target_only_and_bounded() {
        let value = group(Some(ID), Some("native"), "Public group", 2, true, false).unwrap();
        assert_eq!(value["membership"], json!({"isLeader":true,"isOfficer":false}));
        assert_eq!(value["relationships"], json!([]));
        assert_eq!(value["attributes"], json!({"name":"Public group","version":2}));
        for (id, principal, name, version) in [(None, Some("native"), "name", 1), (Some(ID), Some("protocol_publisher"), "name", 1), (Some(ID), Some("native"), "", 1), (Some(ID), Some("native"), "name", 0)] {
            assert!(group(id, principal, name, version, false, false).is_err());
        }
        assert!(group(Some(ID), Some("native"), &"界".repeat(201), 1, false, false).is_err());
    }
}
