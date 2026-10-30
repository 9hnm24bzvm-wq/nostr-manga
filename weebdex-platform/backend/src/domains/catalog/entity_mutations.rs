//! Transaction-owned catalogue mutations shared by authenticated native and
//! explicitly account-bound public adapters. This module never authenticates,
//! opens a transaction, commits one, or turns a read capability into write authority.
use super::{repository::{audit, create_thread}, validation as v};
use crate::{domains::auth::SessionPrincipal, error::{ApiError, ApiResult}};
use serde_json::Value;
use tokio_postgres::Transaction;

// Only genuine authenticated principals and the public adapter's privately
// constructed, transaction-rechecked write capability implement this marker.
pub(crate) mod sealed { pub trait Sealed {} }
pub(crate) trait CatalogMutationActor: sealed::Sealed + Sync {
    // Contract selection follows the sealed authenticated transport, never JSON
    // input or a UUID. Native principals keep their historical validation.
    fn language_contract(&self) -> v::LanguageContract { v::LanguageContract::Native }
    // Sealed transport provenance, never inferred from an account ID or JSON.
    fn native_publication_session(&self) -> Option<&SessionPrincipal> { None }
    fn user_id(&self) -> &str;
    fn username(&self) -> &str;
    fn is_moderator(&self) -> bool;
    fn has_perm(&self, permission: &str) -> bool;
    fn require_permission(&self, permission: &str) -> ApiResult<()> {
        if self.has_perm(permission) { Ok(()) }
        else { Err(ApiError::forbidden("This account cannot perform that action")) }
    }
}
impl sealed::Sealed for SessionPrincipal {}
impl CatalogMutationActor for SessionPrincipal {
    fn native_publication_session(&self) -> Option<&SessionPrincipal> { (!self.is_api_client && !self.recovery_session).then_some(self) }
    fn user_id(&self) -> &str { &self.user_id }
    fn username(&self) -> &str { &self.username }
    fn is_moderator(&self) -> bool { SessionPrincipal::is_moderator(self) }
    fn has_perm(&self, permission: &str) -> bool { SessionPrincipal::has_perm(self, permission) }
    fn require_permission(&self, permission: &str) -> ApiResult<()> {
        SessionPrincipal::require_permission(self, permission)
    }
}
pub(crate) const AUTHOR_FIELDS: &[&str] = &[
    "name",
    "description",
    "twitter",
    "pixiv",
    "fanbox",
    "fantia",
    "skeb",
    "youtube",
    "website",
    "locked",
    "version",
];
pub(crate) const AUTHOR_META: &[&str] = &[
    "description",
    "twitter",
    "pixiv",
    "fanbox",
    "fantia",
    "skeb",
    "youtube",
    "website",
];
pub(crate) const GROUP_FIELDS: &[&str] = &[
    "name",
    "description",
    "contact_email",
    "discord",
    "mangaupdates",
    "twitter",
    "website",
    "locked",
    "inactive",
    "focused_languages",
    "leader",
    "officers",
    "members",
    "version",
];
pub(crate) const GROUP_META: &[&str] = &[
    "description",
    "contact_email",
    "discord",
    "mangaupdates",
    "twitter",
    "website",
    "focused_languages",
];

pub(crate) const AUTHOR_CREATE_FIELDS: &[&str] = &[
    "name", "description", "twitter", "pixiv", "fanbox", "fantia", "skeb", "youtube", "website",
];
// Additional normalized MDex metadata is accepted only by the explicitly
// public constructors. Native controllers retain their original field contract.
pub(crate) const AUTHOR_PUBLIC_METADATA: &[&str] = &[
    "biography", "melonBook", "booth", "nicoVideo", "tumblr", "weibo", "naver",
];
pub(crate) const GROUP_PUBLIC_METADATA: &[&str] = &["ircServer", "ircChannel", "publishDelay"];
fn public_allowed(payload: &Value, native: &[&str], additional: &[&str]) -> ApiResult<()> {
    for key in v::object(payload)?.keys() {
        if !native.contains(&key.as_str()) && !additional.contains(&key.as_str()) {
            return Err(ApiError::bad_request(format!("Unknown field: {key}")));
        }
    }
    Ok(())
}
/// Nullable metadata is preserved as JSON null. URL/host/duration semantics are
/// checked by the MDex adapter; this boundary validates every stored value and
/// bounds its size independently, without altering the legacy native validator.
fn public_metadata(payload: &Value, native: &[&str], additional: &[&str]) -> ApiResult<Value> {
    let mut out = serde_json::Map::new();
    for key in native.iter().chain(additional) {
        let Some(value) = payload.get(*key) else { continue; };
        if value.is_null() { out.insert((*key).into(), Value::Null); continue; }
        if *key == "biography" {
            let localized = value.as_object().filter(|m| m.len() <= 100)
                .ok_or_else(|| ApiError::bad_request("Invalid biography"))?;
            let mut total = 0;
            for (language, text) in localized {
                let text = text.as_str().ok_or_else(|| ApiError::bad_request("Invalid biography"))?;
                total += text.chars().count();
                if language.is_empty() || language.len() > 100 || language.contains('\0')
                    || text.contains('\0') || total > 50_000 {
                    return Err(ApiError::bad_request("Invalid biography"));
                }
            }
        } else if *key == "focused_languages" {
            let languages = value.as_array().filter(|a| a.len() <= 100)
                .ok_or_else(|| ApiError::bad_request("Invalid focused languages"))?;
            for language in languages {
                v::public_language(language.as_str().ok_or_else(|| ApiError::bad_request("Invalid language"))?)?;
            }
        } else {
            v::string(payload, key, if *key == "description" { 50_000 } else { 2048 }, false)?;
        }
        out.insert((*key).into(), value.clone());
    }
    Ok(Value::Object(out))
}
/// Fields are private so adapters cannot bypass shared validation.
pub(crate) struct AuthorCreate { name: String, metadata: Value }
impl AuthorCreate {
    pub(crate) fn parse(payload: &Value) -> ApiResult<Self> { Self::parse_impl(payload, false) }
    pub(crate) fn parse_public_normalized(payload: &Value) -> ApiResult<Self> { Self::parse_impl(payload, true) }
    fn parse_impl(payload: &Value, public: bool) -> ApiResult<Self> {
        if public { public_allowed(payload, AUTHOR_CREATE_FIELDS, AUTHOR_PUBLIC_METADATA)?; }
        else { v::allowed(payload, AUTHOR_CREATE_FIELDS)?; }
        Ok(Self {
            name: v::string(payload, "name", 200, true)?.unwrap(),
            metadata: if public { public_metadata(payload, AUTHOR_META, AUTHOR_PUBLIC_METADATA)? }
                else { v::metadata(payload, AUTHOR_META)? },
        })
    }
}
pub(crate) struct AuthorUpdate { version: i32, name: Option<String>, locked: Option<bool>, metadata: Value }
impl AuthorUpdate {
    pub(crate) fn parse(actor: &impl CatalogMutationActor, payload: &Value) -> ApiResult<Self> {
        Self::parse_impl(actor, payload, false)
    }
    pub(crate) fn parse_public_normalized(actor: &impl CatalogMutationActor, payload: &Value) -> ApiResult<Self> {
        Self::parse_impl(actor, payload, true)
    }
    fn parse_impl(actor: &impl CatalogMutationActor, payload: &Value, public: bool) -> ApiResult<Self> {
        if public { public_allowed(payload, AUTHOR_FIELDS, AUTHOR_PUBLIC_METADATA)?; }
        else { v::allowed(payload, AUTHOR_FIELDS)?; }
        let version = v::version(payload)?;
        let name = v::string(payload, "name", 200, false)?;
        if name.as_deref() == Some("") { return Err(ApiError::bad_request("Name must not be empty")); }
        let locked = v::boolean(payload, "locked")?;
        if locked.is_some() { staff_permission(actor, "author.lock.update")?; }
        let metadata = if public { public_metadata(payload, AUTHOR_META, AUTHOR_PUBLIC_METADATA)? }
            else { v::metadata(payload, AUTHOR_META)? };
        Ok(Self { version, name, locked, metadata })
    }
}
pub(crate) const GROUP_CREATE_FIELDS: &[&str] = &[
    "name", "description", "contact_email", "discord", "mangaupdates", "twitter", "website", "focused_languages",
];
pub(crate) struct GroupCreate { name: String, metadata: Value, inactive: bool, assign_creator_leader: bool }
impl GroupCreate {
    pub(crate) fn parse(payload: &Value) -> ApiResult<Self> { Self::parse_impl(payload, false) }
    pub(crate) fn parse_public_normalized(payload: &Value) -> ApiResult<Self> { Self::parse_impl(payload, true) }
    fn parse_impl(payload: &Value, public: bool) -> ApiResult<Self> {
        if public { public_allowed(payload, GROUP_CREATE_FIELDS, &["inactive", "ircServer", "ircChannel", "publishDelay"])?; }
        else { v::allowed(payload, GROUP_CREATE_FIELDS)?; }
        Ok(Self {
            name: v::string(payload, "name", 200, true)?.unwrap(),
            metadata: if public { public_metadata(payload, GROUP_META, GROUP_PUBLIC_METADATA)? }
                else { v::metadata(payload, GROUP_META)? },
            inactive: if public { v::boolean(payload, "inactive")?.unwrap_or(false) } else { false },
            // This is local editorial account membership, never a protocol
            // group key or a capability inferred from a Nostr publisher alias.
            assign_creator_leader: public,
        })
    }
}
pub(crate) struct GroupUpdate {
    version: i32, name: Option<String>, locked: Option<bool>, inactive: Option<bool>, metadata: Value,
    leader: Option<String>, officers: Option<Vec<String>>, members: Option<Vec<String>>,
}
impl GroupUpdate {
    pub(crate) fn parse(payload: &Value) -> ApiResult<Self> { Self::parse_impl(payload, false) }
    pub(crate) fn parse_public_normalized(payload: &Value) -> ApiResult<Self> { Self::parse_impl(payload, true) }
    fn parse_impl(payload: &Value, public: bool) -> ApiResult<Self> {
        if public { public_allowed(payload, GROUP_FIELDS, GROUP_PUBLIC_METADATA)?; }
        else { v::allowed(payload, GROUP_FIELDS)?; }
        let version = v::version(payload)?;
        let name = v::string(payload, "name", 200, false)?;
        if name.as_deref() == Some("") { return Err(ApiError::bad_request("Name must not be empty")); }
        let locked = v::boolean(payload, "locked")?;
        let inactive = v::boolean(payload, "inactive")?;
        let metadata = if public { public_metadata(payload, GROUP_META, GROUP_PUBLIC_METADATA)? }
            else { v::metadata(payload, GROUP_META)? };
        let leader = v::string(payload, "leader", 128, false)?;
        if let Some(l) = &leader { v::id(l)?; }
        let officers = v::ids(payload, "officers", false)?;
        let members = v::ids(payload, "members", false)?;
        Ok(Self { version, name, locked, inactive, metadata, leader, officers, members })
    }
}
pub(crate) async fn author_create(
    tx: &Transaction<'_>,
    actor: &impl CatalogMutationActor,
    input: AuthorCreate,
) -> ApiResult<Value> {
    crate::domains::auth::closure::active_in(tx,actor.user_id()).await?;
    actor.require_permission("author.create")?;
    let AuthorCreate { name, metadata } = input;
    let id = v::new_id("author");
    tx.execute(
        "INSERT INTO platform.catalog_authors(id,owner_id,owner_name,name,metadata) VALUES($1,$2,$3,$4,$5)",
        &[&id, &actor.user_id(), &actor.username(), &name, &metadata],
    )
    .await?;
    audit(tx, &actor.user_id(), "author", &id, "create", None, Some(1)).await?;
    let data = tx
        .query_one("SELECT platform.catalog_author_json($1)", &[&id])
        .await?
        .get(0);
    Ok(data)
}

pub(crate) async fn author_update(
    tx: &Transaction<'_>,
    actor: &impl CatalogMutationActor,
    id: &str,
    input: AuthorUpdate,
) -> ApiResult<Value> {
    crate::domains::auth::closure::active_in(tx,actor.user_id()).await?;
    actor.require_permission("author.update")?;
    v::id(id)?;
    let AuthorUpdate { version, name, locked, metadata } = input;
    if locked.is_some() { staff_permission(actor, "author.lock.update")?; }
    let row=tx.query_opt("SELECT locked,version FROM platform.catalog_authors WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_source_is_visible(protocol_event_id) FOR UPDATE",&[&id]).await?.ok_or_else(||ApiError::not_found("Author not found"))?;
    if row.get::<_, bool>(0) && !actor.is_moderator() {
        return Err(ApiError::forbidden("Author is locked"));
    }
    check_version(row.get(1), version)?;
    tx.execute("UPDATE platform.catalog_authors SET name=COALESCE($2,name),metadata=metadata || $3,locked=COALESCE($4,locked),version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id,&name,&metadata,&locked]).await?;
    audit(
        tx,
        &actor.user_id(),
        "author",
        &id,
        "update",
        Some(version),
        Some(version + 1),
    )
    .await?;
    let data = tx
        .query_one("SELECT platform.catalog_author_json($1)", &[&id])
        .await?
        .get(0);
    Ok(data)
}

pub(crate) async fn author_delete(
    tx: &Transaction<'_>,
    actor: &impl CatalogMutationActor,
    id: &str,
) -> ApiResult<()> {
    crate::domains::auth::closure::active_in(tx,actor.user_id()).await?;
    staff_permission(actor, "author.delete")?;
    v::id(id)?;
    // Serialize soft deletion with title creator assignment. The reference
    // check must run after this row lock, so a reference committed by an earlier
    // holder cannot be missed while this deletion waits to update the author.
    tx.query_opt("SELECT id FROM platform.catalog_authors WHERE id=$1 AND deleted_at IS NULL FOR UPDATE",&[&id]).await?.ok_or_else(||ApiError::not_found("Author not found"))?;
    let used:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.catalog_manga_creators mc JOIN platform.catalog_manga m ON m.id=mc.manga_id WHERE mc.author_id=$1 AND m.deleted_at IS NULL)",&[&id]).await?.get(0);
    if used {
        return Err(ApiError::conflict("Author is still referenced by a manga"));
    }
    let n=tx.execute("UPDATE platform.catalog_authors SET deleted_at=clock_timestamp(),updated_at=clock_timestamp(),version=version+1 WHERE id=$1 AND deleted_at IS NULL",&[&id]).await?;
    if n == 0 {
        return Err(ApiError::not_found("Author not found"));
    }
    audit(tx, &actor.user_id(), "author", &id, "remove", None, None).await?;
    Ok(())
}

pub(crate) async fn group_create(
    tx: &Transaction<'_>,
    actor: &impl CatalogMutationActor,
    input: GroupCreate,
) -> ApiResult<Value> {
    crate::domains::auth::closure::active_in(tx,actor.user_id()).await?;
    actor.require_permission("scanlation_group.create")?;
    let GroupCreate { name, metadata, inactive, assign_creator_leader } = input;
    let id = v::new_id("group");
    tx.execute("INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name,metadata,inactive) VALUES($1,$2,$3,$4,$5,$6)",&[&id,&actor.user_id(),&actor.username(),&name,&metadata,&inactive]).await?;
    if assign_creator_leader {
        tx.execute(
            "INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader,is_officer) VALUES($1,$2,$3,TRUE,FALSE)",
            &[&id, &actor.user_id(), &actor.username()],
        ).await?;
    }
    create_thread(tx, "group", &id, &actor.user_id(), &actor.username(), &name).await?;
    audit(tx, &actor.user_id(), "group", &id, "create", None, Some(1)).await?;
    let data = tx
        .query_one("SELECT platform.catalog_group_json($1)", &[&id])
        .await?
        .get(0);
    Ok(data)
}

pub(crate) async fn group_update(
    tx: &Transaction<'_>,
    actor: &impl CatalogMutationActor,
    id: &str,
    input: GroupUpdate,
) -> ApiResult<Value> {
    crate::domains::auth::closure::active_in(tx,actor.user_id()).await?;
    v::id(id)?;
    let GroupUpdate { version, name, locked, inactive, metadata, leader, officers, members } = input;
    // The legacy native route loads its principal before opening a transaction.
    // Recheck and hold actual account grants before the group lock, just as the
    // account adapters do, so a revoked grant cannot authorize a late role edit.
    let actual = tx.query_opt("SELECT verified,roles,permissions FROM platform.users WHERE id=$1 AND NOT disabled FOR SHARE NOWAIT", &[&actor.user_id()]).await?.ok_or_else(|| ApiError::forbidden("Group account authority ended"))?;
    let actual_roles: Value = actual.get(1);
    let actual_permissions: Value = actual.get(2);
    let eligible = actual.get::<_, bool>(0) && actual_roles.as_array().is_some_and(|roles| !roles.iter().any(|role| role == "banned" || role == "restricted"));
    let current_permission = |permission: &str| eligible && actual_permissions.as_array().is_some_and(|permissions| permissions.iter().any(|value| value == permission));
    let current_staff = actual_roles.as_array().is_some_and(|roles| roles.iter().any(|role| matches!(role.as_str(), Some("moderator" | "staff" | "sysop"))));
    let row=tx.query_opt("SELECT locked,version FROM platform.catalog_groups WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_group_is_visible(id) FOR UPDATE",&[&id]).await?.ok_or_else(||ApiError::not_found("Group not found"))?;
    let staff = actor.is_moderator() && actor.has_perm("scanlation_group.update") && current_staff && current_permission("scanlation_group.update");
    if !staff {
        actor.require_permission("self.scanlation_group.update")?;
        if !current_permission("self.scanlation_group.update") { return Err(ApiError::forbidden("Group account authority ended")); }
        let owns:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.catalog_group_members WHERE group_id=$1 AND user_id=$2 AND is_leader)",&[&id,&actor.user_id()]).await?.get(0);
        if !owns || row.get::<_, bool>(0) {
            return Err(ApiError::forbidden("Group leader permission required"));
        }
        if name.is_some() || locked.is_some() {
            return Err(ApiError::forbidden("Only staff may rename or lock groups"));
        }
    }
    check_version(row.get(1), version)?;
    if leader.is_some() || officers.is_some() || members.is_some() {
        let current=tx.query("SELECT user_id,is_leader,is_officer FROM platform.catalog_group_members WHERE group_id=$1",&[&id]).await?;
        let leader = leader
            .or_else(|| {
                current
                    .iter()
                    .find(|r| r.get::<_, bool>(1))
                    .map(|r| r.get::<_, String>(0))
            })
            .ok_or_else(|| ApiError::bad_request("Group must have a leader"))?;
        let officers = officers.unwrap_or_else(|| {
            current
                .iter()
                .filter(|r| r.get::<_, bool>(2))
                .map(|r| r.get::<_, String>(0))
                .collect()
        });
        let members =
            members.unwrap_or_else(|| current.iter().map(|r| r.get::<_, String>(0)).collect());
        let mut all: std::collections::BTreeSet<String> = members.into_iter().collect();
        all.insert(leader.clone());
        all.extend(officers.iter().cloned());
        let ids: Vec<String> = all.into_iter().collect();
        if !staff && ids.iter().any(|id| !current.iter().any(|row| row.get::<_, String>(0) == *id)) {
            return Err(ApiError::new(axum::http::StatusCode::CONFLICT, "invitation_required", "Invite new members and wait for acceptance before assigning local roles"));
        }
        if !current.iter().any(|row| row.get::<_, String>(0) == leader && row.get::<_, bool>(1)) {
            // A leadership transfer must leave an account that can actually manage
            // this team. Hold the destination against disable/grant changes until
            // the membership replacement commits; NOWAIT prevents lock cycles.
            let target = tx.query_opt("SELECT verified,roles,permissions FROM platform.users WHERE id=$1 AND NOT disabled FOR SHARE NOWAIT", &[&leader]).await?;
            let eligible = target.as_ref().is_some_and(|row| {
                let roles: Value = row.get(1);
                let permissions: Value = row.get(2);
                row.get::<_, bool>(0)
                    && roles.as_array().is_some_and(|values| !values.iter().any(|value| value == "banned" || value == "restricted"))
                    && permissions.as_array().is_some_and(|values| values.iter().any(|value| value == "self.scanlation_group.update"))
            });
            if !eligible {
                return Err(ApiError::conflict("Choose a verified, active group member with permission to manage groups as the new leader"));
            }
        }
        let users = tx
            .query(
                "SELECT id,username FROM platform.users WHERE id=ANY($1) AND NOT disabled",
                &[&ids],
            )
            .await?;
        if users.len() != ids.len() {
            return Err(ApiError::bad_request("Unknown group member"));
        }
        tx.execute(
            "DELETE FROM platform.catalog_group_members WHERE group_id=$1",
            &[&id],
        )
        .await?;
        for user in users {
            let uid: String = user.get(0);
            let uname: String = user.get(1);
            tx.execute("INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader,is_officer) VALUES($1,$2,$3,$4,$5)",&[&id,&uid,&uname,&(uid==leader),&officers.contains(&uid)]).await?;
        }
    }
    tx.execute("UPDATE platform.catalog_groups SET name=COALESCE($2,name),metadata=metadata || $3,locked=COALESCE($4,locked),inactive=COALESCE($5,inactive),version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id,&name,&metadata,&locked,&inactive]).await?;
    audit(
        tx,
        &actor.user_id(),
        "group",
        &id,
        "update",
        Some(version),
        Some(version + 1),
    )
    .await?;
    let data = tx
        .query_one("SELECT platform.catalog_group_json($1)", &[&id])
        .await?
        .get(0);
    Ok(data)
}

pub(crate) async fn group_delete(
    tx: &Transaction<'_>,
    actor: &impl CatalogMutationActor,
    id: &str,
) -> ApiResult<()> {
    crate::domains::auth::closure::active_in(tx,actor.user_id()).await?;
    staff_permission(actor, "scanlation_group.delete")?;
    v::id(id)?;
    let n=tx.execute("UPDATE platform.catalog_groups SET deleted_at=clock_timestamp(),updated_at=clock_timestamp(),version=version+1 WHERE id=$1 AND deleted_at IS NULL",&[&id]).await?;
    if n == 0 {
        return Err(ApiError::not_found("Group not found"));
    }
    audit(tx, &actor.user_id(), "group", &id, "remove", None, None).await?;
    Ok(())
}

fn staff_permission(actor: &impl CatalogMutationActor, permission: &str) -> ApiResult<()> {
    if !actor.is_moderator() { return Err(ApiError::forbidden("Staff permission required")); }
    actor.require_permission(permission)
}
fn check_version(actual: i32, expected: i32) -> ApiResult<()> {
    if actual == expected { Ok(()) } else { Err(ApiError::conflict("Entity changed; reload before editing")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn principal(roles: &[&str], permissions: &[&str]) -> SessionPrincipal {
        SessionPrincipal {
            user_id: "CatalogActor".into(), username: "Catalog actor".into(),
            email: "catalog-actor@fixture.invalid".into(), verified: true,
            roles: roles.iter().map(|s| (*s).into()).collect(),
            permissions: permissions.iter().map(|s| (*s).into()).collect(),
            session_id: "synthetic-unit-session".into(), is_api_client: false,
            authenticated_age_seconds: 0, recovery_session: false,
        }
    }
    #[test]
    fn create_inputs_validate_every_supported_field() {
        let author = AuthorCreate::parse(&json!({"name":"  漫画作者  ","description":"bio","fanbox":"https://fixture.invalid/author"})).unwrap();
        assert_eq!(author.name,"漫画作者");
        assert_eq!(author.metadata["description"],"bio");
        for bad in [json!({"name":" "}),json!({"name":"valid","ignored":true}),json!({"name":"valid","twitter":42}),json!({"name":"valid","version":1})] {
            assert!(AuthorCreate::parse(&bad).is_err());
        }
        let group = GroupCreate::parse(&json!({"name":"Group","focused_languages":["en","zh-hk"]})).unwrap();
        assert_eq!(group.metadata["focused_languages"],json!(["en","zh-hk"]));
        assert!(GroupCreate::parse(&json!({"name":"Group","focused_languages":["unsupported"]})).is_err());
    }
    #[test]
    fn public_normalized_metadata_is_typed_nullable_and_native_contract_stays_closed() {
        let author_payload = json!({"name":"Author","biography":{"en":"A biography","ja":"漫画家"},"fanbox":null,"melonBook":"https://fixture.invalid/author","naver":null});
        let author = AuthorCreate::parse_public_normalized(&author_payload).unwrap();
        assert_eq!(author.metadata["biography"]["ja"],"漫画家");
        assert!(author.metadata["fanbox"].is_null());
        assert!(AuthorCreate::parse(&author_payload).is_err());
        assert!(AuthorCreate::parse_public_normalized(&json!({"name":"Author","biography":{"en":false}})).is_err());
        assert!(AuthorCreate::parse_public_normalized(&json!({"name":"Author","biography":{"en":"a".repeat(50_001)}})).is_err());
        assert!(AuthorCreate::parse_public_normalized(&json!({"name":"Author","ignored":null})).is_err());
        let actor = principal(&["user"],&["author.update"]);
        let update = json!({"version":1,"biography":null,"twitter":null});
        assert!(AuthorUpdate::parse_public_normalized(&actor,&update).is_ok());
        assert!(AuthorUpdate::parse(&actor,&update).is_err());
        let group_payload = json!({"name":"Group","inactive":true,"ircServer":"irc.fixture.invalid","publishDelay":null});
        let group = GroupCreate::parse_public_normalized(&group_payload).unwrap();
        assert!(group.inactive);
        assert!(group.assign_creator_leader);
        assert!(!GroupCreate::parse(&json!({"name":"Native group"})).unwrap().assign_creator_leader);
        assert_eq!(group.metadata["ircServer"],"irc.fixture.invalid");
        assert!(group.metadata["publishDelay"].is_null());
        assert!(GroupCreate::parse(&group_payload).is_err());
        assert!(GroupCreate::parse_public_normalized(&json!({"name":"Group","inactive":null})).is_err());
        let update = json!({"version":1,"focused_languages":null,"publishDelay":"P1D","ircChannel":null});
        let group = GroupUpdate::parse_public_normalized(&update).unwrap();
        assert!(group.metadata["focused_languages"].is_null());
        assert_eq!(group.metadata["publishDelay"],"P1D");
        assert!(GroupUpdate::parse(&update).is_err());
        assert!(GroupUpdate::parse_public_normalized(&json!({"version":1,"focused_languages":["unsupported"]})).is_err());
        assert!(GroupUpdate::parse_public_normalized(&json!({"version":1,"focused_languages":["ta"]})).is_ok());
        assert!(GroupUpdate::parse_public_normalized(&json!({"version":1,"focused_languages":["tam"]})).is_err());
        assert!(GroupUpdate::parse(&json!({"version":1,"focused_languages":["tam"]})).is_ok());
        assert!(GroupUpdate::parse(&json!({"version":1,"focused_languages":["ta"]})).is_err());
    }
    #[test]
    fn author_lock_requires_both_real_staff_role_and_permission() {
        let input = json!({"version":1,"locked":false});
        let user = principal(&["user"],&["author.update","author.lock.update"]);
        assert_eq!(AuthorUpdate::parse(&user,&input).err().unwrap().message,"Staff permission required");
        let mut staff = principal(&["moderator"],&["author.update","author.lock.update"]);
        assert!(AuthorUpdate::parse(&staff,&input).is_ok());
        staff.permissions.retain(|p| p != "author.lock.update");
        assert!(AuthorUpdate::parse(&staff,&input).is_err());
        staff.permissions.push("author.lock.update".into()); staff.is_api_client = true;
        assert!(AuthorUpdate::parse(&staff,&input).is_err());
        staff.is_api_client = false; staff.recovery_session = true;
        assert!(AuthorUpdate::parse(&staff,&input).is_err());
    }
    #[test]
    fn update_inputs_keep_version_metadata_and_membership_validation() {
        let user = principal(&["user"],&["author.update"]);
        for bad in [json!({"name":"next"}),json!({"version":0}),json!({"version":1,"name":" "}),json!({"version":1,"description":false})] {
            assert!(AuthorUpdate::parse(&user,&bad).is_err());
        }
        let input = GroupUpdate::parse(&json!({"version":7,"leader":"MemberA","officers":["MemberB","MemberB"],"members":["MemberA","MemberB"],"inactive":true})).unwrap();
        assert_eq!(input.version,7);
        assert_eq!(input.officers,Some(vec!["MemberB".into()]));
        assert_eq!(input.inactive,Some(true));
        for bad in [json!({"version":1,"leader":"bad/id"}),json!({"version":1,"members":[null]}),json!({"version":1,"locked":1}),json!({"version":1,"ignored":true})] {
            assert!(GroupUpdate::parse(&bad).is_err());
        }
        assert!(check_version(7,7).is_ok());
        assert_eq!(check_version(7,6).unwrap_err().message,"Entity changed; reload before editing");
    }
    #[tokio::test]
    #[ignore = "Requires the explicitly isolated PostgreSQL runner"]
    async fn public_group_creator_is_real_account_and_core_never_commits() {
        use crate::{Config,domains,domains::auth::require_principal,state::AppState};
        use axum::http::HeaderMap;
        use sha2::{Digest,Sha256};
        let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL")
            .expect("Use scripts/run-auth-tests.sh with the owned disposable fixture");
        let config: tokio_postgres::Config = dsn.parse().unwrap();
        assert_eq!(config.get_dbname(),Some("weebdex_platform_test"));
        assert!(!config.get_hosts().is_empty());
        for host in config.get_hosts() {
            assert!(matches!(host,tokio_postgres::config::Host::Tcp(host) if host == "127.0.0.1"));
        }
        assert!(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap().contains("weebdex-platform/test-runtime/"));
        let state = AppState::connect(Config {
            public_origin:"https://mutation-core.fixture.invalid".into(),database_plaintext_local:true,
            registration_enabled:false,..Config::default()
        },dsn).await.unwrap();
        state.db.migrate(domains::MIGRATIONS).await.unwrap();
        let db = state.db.client().await.unwrap();
        db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,roles,permissions) VALUES('CoreGroupActor','Actual account name','core-group-actor@fixture.invalid','synthetic-no-password',TRUE,'[\"user\"]','[\"scanlation_group.create\",\"self.scanlation_group.update\"]')",&[]).await.unwrap();
        let digest = Sha256::digest(b"core-group-fixture-session").to_vec();
        db.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES('CoreGroupSession','CoreGroupActor',$1,clock_timestamp()+interval '1 hour')",&[&digest]).await.unwrap();
        drop(db);
        let mut headers = HeaderMap::new();
        headers.insert("cookie","weebdex_session=core-group-fixture-session".parse().unwrap());
        let actor = require_principal(&state,&headers).await.unwrap();
        assert_eq!(actor.user_id,"CoreGroupActor");
        assert_eq!(actor.username,"Actual account name");
        let mut db = state.db.client().await.unwrap();
        let tx = db.transaction().await.unwrap();
        let input = GroupCreate::parse_public_normalized(&json!({"name":"Public editorial group","inactive":true,"ircServer":"irc.fixture.invalid","publishDelay":"P1D"})).unwrap();
        let group = group_create(&tx,&actor,input).await.unwrap();
        let id = group["id"].as_str().unwrap();
        assert_eq!(group["inactive"],true);
        assert_eq!(group["ircServer"],"irc.fixture.invalid");
        assert_eq!(group["publishDelay"],"P1D");
        assert_eq!(group["relationships"]["members"].as_array().unwrap().len(),1);
        assert_eq!(group["relationships"]["members"][0]["id"],"CoreGroupActor");
        assert_eq!(group["relationships"]["members"][0]["name"],"Actual account name");
        assert_eq!(group["relationships"]["members"][0]["is_leader"],true);
        let observer = state.db.client().await.unwrap();
        assert_eq!(observer.query_one("SELECT count(*) FROM platform.catalog_groups WHERE id=$1",&[&id]).await.unwrap().get::<_,i64>(0),0);
        tx.rollback().await.unwrap();
        for table in ["catalog_groups","catalog_group_members"] {
            let column = if table == "catalog_groups" {"id"} else {"group_id"};
            let sql = format!("SELECT count(*) FROM platform.{table} WHERE {column}=$1");
            assert_eq!(observer.query_one(&sql,&[&id]).await.unwrap().get::<_,i64>(0),0);
        }
        assert_eq!(observer.query_one("SELECT count(*) FROM platform.catalog_audit WHERE entity_id=$1",&[&id]).await.unwrap().get::<_,i64>(0),0);
        assert_eq!(observer.query_one("SELECT count(*) FROM platform.catalog_threads WHERE entity_id=$1",&[&id]).await.unwrap().get::<_,i64>(0),0);
        drop(observer);
        let tx = db.transaction().await.unwrap();
        let native = group_create(&tx,&actor,GroupCreate::parse(&json!({"name":"Native editorial group"})).unwrap()).await.unwrap();
        assert_eq!(native["relationships"]["members"],json!([]));
        assert_eq!(native["inactive"],false);
        tx.rollback().await.unwrap();
    }
}
