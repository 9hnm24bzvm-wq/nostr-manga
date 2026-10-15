//! Instance-local MangaDex editorial writes over the shared catalogue core.
//! Nostr request authority, current native binding, mutation and audit share one
//! transaction. These routes never sign or change protocol group-key authority.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdTarget, PublicUuid,
    ResourceKind,
    auth::{BoundAccountWrite, admitted_request, bound_account_write},
    catalogue_dto::AliasIndex,
    dto::{EntityResponse, Resource},
};
use crate::{AppState, domains::catalog::entity_mutations as core};
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{OriginalUri, Path, Request, State},
    http::{HeaderMap, header},
    routing::{post, put},
};
use serde_json::{Map, Value, json};
use std::{collections::BTreeSet, time::Duration};
use tokio_postgres::Transaction;

const BODY_LIMIT: usize = 64 * 1024;
const AUTHOR_LINKS: &[(&str, &str, bool)] = &[
    ("twitter", "twitter.com", false),
    ("pixiv", "pixiv.net", true),
    ("melonBook", "melonbooks.co.jp", true),
    ("fanBox", "fanbox.cc", true),
    ("booth", "booth.pm", true),
    ("nicoVideo", "nicovideo.jp", true),
    ("skeb", "skeb.jp", true),
    ("fantia", "fantia.jp", true),
    ("tumblr", "tumblr.com", true),
    ("youtube", "youtube.com", true),
    ("weibo", "weibo.com", true),
    ("naver", "naver.com", true),
    ("website", "", false),
];
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/author", post(author_create))
        .route("/author/{p0}", put(author_update).delete(author_delete))
        .route("/group", post(group_create))
        .route("/group/{p0}", put(group_update).delete(group_delete))
}
impl core::sealed::Sealed for BoundAccountWrite {}
impl core::CatalogMutationActor for BoundAccountWrite {
    fn user_id(&self) -> &str {
        BoundAccountWrite::user_id(self)
    }
    fn username(&self) -> &str {
        BoundAccountWrite::username(self)
    }
    fn is_moderator(&self) -> bool {
        BoundAccountWrite::is_moderator(self)
    }
    fn has_perm(&self, permission: &str) -> bool {
        BoundAccountWrite::has_perm(self, permission)
    }
    fn require_permission(&self, permission: &str) -> crate::error::ApiResult<()> {
        BoundAccountWrite::require_permission(self, permission)
    }
}
fn bad(field: &str) -> PublicApiError {
    PublicApiError::bad_request(format!("Invalid {field}"))
}
fn object(value: &Value) -> PublicApiResult<&Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| PublicApiError::bad_request("Expected a JSON object"))
}
fn allowed(value: &Value, fields: &[&str]) -> PublicApiResult<()> {
    if object(value)?
        .keys()
        .any(|key| !fields.contains(&key.as_str()))
    {
        return Err(PublicApiError::bad_request("Unknown request field"));
    }
    Ok(())
}
fn string(value: &Value, field: &str, nullable: bool, max: usize) -> PublicApiResult<()> {
    let Some(value) = value.get(field) else {
        return Ok(());
    };
    if nullable && value.is_null() {
        return Ok(());
    }
    let value = value.as_str().ok_or_else(|| bad(field))?;
    if value.chars().count() > max || value.contains('\0') {
        return Err(bad(field));
    }
    Ok(())
}
fn name(value: &Value, required: bool) -> PublicApiResult<()> {
    string(value, "name", false, 200)?;
    if required && !value.get("name").is_some_and(Value::is_string)
        || value
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|v| v.trim().is_empty())
    {
        return Err(bad("name"));
    }
    Ok(())
}
fn version(value: &Value) -> PublicApiResult<()> {
    if !value["version"]
        .as_i64()
        .is_some_and(|v| (1..=i64::from(i32::MAX)).contains(&v))
    {
        return Err(bad("version"));
    }
    Ok(())
}
fn http_url(value: &str) -> bool {
    (value.starts_with("http://") || value.starts_with("https://"))
        && url::Url::parse(value).is_ok_and(|u| u.has_host())
}
fn author_url(value: &str, domain: &str, subdomain: bool) -> bool {
    if !http_url(value) {
        return false;
    }
    if domain.is_empty() {
        return true;
    }
    let rest = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
        .unwrap();
    let host = rest.split('/').next().unwrap();
    let matches = |domain: &str| {
        host == domain
            || subdomain
                && host
                    .strip_suffix(&format!(".{domain}"))
                    .is_some_and(|prefix| {
                        !prefix.is_empty()
                            && prefix
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                    })
    };
    matches(domain) || domain == "weibo.com" && matches("weibo.cn")
}
fn normalize_author(value: Value, edit: bool) -> PublicApiResult<Value> {
    let mut fields = vec!["name", "biography"];
    fields.extend(AUTHOR_LINKS.iter().map(|(field, _, _)| *field));
    if edit {
        fields.push("version");
    }
    allowed(&value, &fields)?;
    name(&value, !edit)?;
    if edit {
        version(&value)?;
    }
    if let Some(biography) = value.get("biography") {
        let biography = biography
            .as_object()
            .filter(|v| v.len() <= 100)
            .ok_or_else(|| bad("biography"))?;
        let mut total = 0;
        for (language, text) in biography {
            let text = text.as_str().ok_or_else(|| bad("biography"))?;
            total += text.chars().count();
            if language.is_empty()
                || language.len() > 100
                || language.contains('\0')
                || text.contains('\0')
                || total > 50_000
            {
                return Err(bad("biography"));
            }
        }
    }
    for (field, domain, subdomain) in AUTHOR_LINKS {
        string(&value, field, true, 2048)?;
        if value
            .get(*field)
            .and_then(Value::as_str)
            .is_some_and(|url| !author_url(url, domain, *subdomain))
        {
            return Err(bad(field));
        }
    }
    let mut out = value.as_object().unwrap().clone();
    if let Some(fanbox) = out.remove("fanBox") {
        out.insert("fanbox".into(), fanbox);
    }
    Ok(Value::Object(out))
}
fn slug(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}
fn manga_updates(value: &str) -> bool {
    if value.chars().count() > 128 {
        return false;
    }
    let Some(path) = value.strip_prefix("https://www.mangaupdates.com/") else {
        return false;
    };
    for prefix in ["group", "publisher"] {
        if let Some(id) = path.strip_prefix(&format!("{prefix}s.html?id=")) {
            if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) {
                return true;
            }
        }
        if let Some(path) = path.strip_prefix(&format!("{prefix}/")) {
            let parts = path
                .strip_suffix('/')
                .unwrap_or(path)
                .split('/')
                .collect::<Vec<_>>();
            if (1..=2).contains(&parts.len()) && parts.iter().all(|part| slug(part)) {
                return true;
            }
        }
    }
    false
}
// Exactly the pinned create pattern: D, W, then optional T/H/M/S; no zero or
// padded values, and the upstream pattern also accepts P and PT.
fn publish_delay(value: &str) -> bool {
    let Some(mut value) = value.strip_prefix('P') else {
        return false;
    };
    for (suffix, maximum) in [('D', 99), ('W', 9)] {
        let digits = value.bytes().take_while(u8::is_ascii_digit).count();
        if value.get(digits..).is_some_and(|v| v.starts_with(suffix)) {
            let number = &value[..digits];
            if number.starts_with('0')
                || !number
                    .parse::<u8>()
                    .is_ok_and(|v| (1..=maximum).contains(&v))
            {
                return false;
            }
            value = &value[digits + 1..];
        }
    }
    if value.is_empty() {
        return true;
    }
    let Some(mut value) = value.strip_prefix('T') else {
        return false;
    };
    for (suffix, maximum) in [('H', 24), ('M', 60), ('S', 60)] {
        let digits = value.bytes().take_while(u8::is_ascii_digit).count();
        if value.get(digits..).is_some_and(|v| v.starts_with(suffix)) {
            let number = &value[..digits];
            if number.starts_with('0')
                || !number
                    .parse::<u8>()
                    .is_ok_and(|v| (1..=maximum).contains(&v))
            {
                return false;
            }
            value = &value[digits + 1..];
        }
    }
    value.is_empty()
}
fn normalize_group(value: Value, edit: bool) -> PublicApiResult<Value> {
    let mut fields = vec![
        "name",
        "website",
        "ircServer",
        "ircChannel",
        "discord",
        "contactEmail",
        "description",
        "twitter",
        "mangaUpdates",
        "inactive",
        "publishDelay",
    ];
    if edit {
        fields.extend(["leader", "members", "focusedLanguages", "locked", "version"]);
    }
    allowed(&value, &fields)?;
    name(&value, !edit)?;
    if edit {
        version(&value)?;
    }
    for field in [
        "website",
        "ircServer",
        "ircChannel",
        "discord",
        "contactEmail",
        "description",
        "twitter",
        "mangaUpdates",
        "publishDelay",
    ] {
        string(
            &value,
            field,
            field != "publishDelay" || !edit,
            if field == "description" { 50_000 } else { 2048 },
        )?;
    }
    for field in ["inactive", "locked"] {
        if value.get(field).is_some_and(|v| !v.is_boolean()) {
            return Err(bad(field));
        }
    }
    if let Some(twitter) = value.get("twitter").and_then(Value::as_str) {
        if !http_url(twitter)
            || !edit
                && !(twitter.starts_with("http://twitter.com")
                    || twitter.starts_with("https://twitter.com"))
        {
            return Err(bad("twitter"));
        }
    }
    if value
        .get("mangaUpdates")
        .and_then(Value::as_str)
        .is_some_and(|v| !manga_updates(v))
    {
        return Err(bad("mangaUpdates"));
    }
    if !edit
        && value
            .get("publishDelay")
            .and_then(Value::as_str)
            .is_some_and(|v| !publish_delay(v))
    {
        return Err(bad("publishDelay"));
    }
    if let Some(languages) = value.get("focusedLanguages").filter(|v| !v.is_null()) {
        let languages = languages
            .as_array()
            .filter(|v| v.len() <= 100)
            .ok_or_else(|| bad("focusedLanguages"))?;
        for language in languages {
            let language = language.as_str().ok_or_else(|| bad("focusedLanguages"))?;
            let valid =
                |part: &str| part.len() == 2 && part.bytes().all(|b| b.is_ascii_lowercase());
            let parts = language.split('-').collect::<Vec<_>>();
            if !(1..=2).contains(&parts.len()) || !parts.iter().all(|part| valid(part)) {
                return Err(bad("focusedLanguages"));
            }
        }
    }
    if let Some(leader) = value.get("leader") {
        PublicUuid::parse(leader.as_str().ok_or_else(|| bad("leader"))?)?;
    }
    if let Some(members) = value.get("members") {
        let members = members
            .as_array()
            .filter(|v| v.len() <= 100)
            .ok_or_else(|| bad("members"))?;
        let mut seen = BTreeSet::new();
        for member in members {
            let member = PublicUuid::parse(member.as_str().ok_or_else(|| bad("members"))?)?;
            if !seen.insert(member) {
                return Err(bad("members"));
            }
        }
    }
    let mut out = value.as_object().unwrap().clone();
    for (wire, native) in [
        ("contactEmail", "contact_email"),
        ("mangaUpdates", "mangaupdates"),
        ("focusedLanguages", "focused_languages"),
    ] {
        if let Some(value) = out.remove(wire) {
            out.insert(native.into(), value);
        }
    }
    Ok(Value::Object(out))
}
async fn read_body(
    request: Request,
    headers: &HeaderMap,
    delete: bool,
) -> PublicApiResult<Vec<u8>> {
    if !delete
        && headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .is_none_or(|v| !v.trim().eq_ignore_ascii_case("application/json"))
    {
        return Err(PublicApiError::bad_request(
            "Content-Type must be application/json",
        ));
    }
    let body = tokio::time::timeout(
        Duration::from_secs(5),
        to_bytes(request.into_body(), BODY_LIMIT),
    )
    .await
    .map_err(|_| PublicApiError::bad_request("Request body timeout"))?
    .map_err(|_| PublicApiError::bad_request("Request body exceeds 64KB"))?;
    if delete && !body.is_empty() {
        return Err(PublicApiError::bad_request(
            "DELETE accepts no request body",
        ));
    }
    Ok(body.to_vec())
}
async fn native_id(tx: &Transaction<'_>, kind: ResourceKind, id: &str) -> PublicApiResult<String> {
    let id = PublicUuid::parse(id)?;
    PostgresPublicIdResolver::resolve_many_in(tx, kind, std::slice::from_ref(&id))
        .await?
        .remove(&id)
        .filter(|v| v.target == PublicIdTarget::NativeResource)
        .map(|v| v.native_id)
        .ok_or_else(PublicApiError::not_found)
}
async fn member_ids(tx: &Transaction<'_>, value: &mut Value) -> PublicApiResult<()> {
    let mut ids = BTreeSet::new();
    if let Some(leader) = value.get("leader").and_then(Value::as_str) {
        ids.insert(PublicUuid::parse(leader)?);
    }
    if let Some(members) = value.get("members").and_then(Value::as_array) {
        for member in members {
            ids.insert(PublicUuid::parse(
                member.as_str().ok_or_else(|| bad("members"))?,
            )?);
        }
    }
    let resolved = PostgresPublicIdResolver::resolve_many_in(
        tx,
        ResourceKind::User,
        &ids.iter().cloned().collect::<Vec<_>>(),
    )
    .await?;
    let mut native = Vec::new();
    for id in &ids {
        let target = resolved
            .get(id)
            .filter(|v| v.target == PublicIdTarget::NativeResource)
            .ok_or_else(|| PublicApiError::bad_request("Unknown native group member"))?;
        native.push(target.native_id.clone());
    }
    native.sort();
    native.dedup();
    if !native.is_empty() {
        let rows = tx.query("SELECT id FROM platform.users WHERE id=ANY($1::text[]) AND NOT disabled ORDER BY id FOR SHARE", &[&native]).await?;
        if rows.len() != native.len() {
            return Err(PublicApiError::bad_request("Unknown native group member"));
        }
    }
    if let Some(leader) = value.get_mut("leader") {
        let id = PublicUuid::parse(leader.as_str().ok_or_else(|| bad("leader"))?)?;
        *leader = json!(resolved[&id].native_id);
    }
    if let Some(members) = value.get_mut("members").and_then(Value::as_array_mut) {
        for member in members {
            let id = PublicUuid::parse(member.as_str().ok_or_else(|| bad("members"))?)?;
            *member = json!(resolved[&id].native_id);
        }
    }
    Ok(())
}
async fn mutate(
    state: AppState,
    uri: axum::http::Uri,
    headers: HeaderMap,
    request: Request,
    kind: ResourceKind,
    id: Option<String>,
) -> PublicApiResult<Json<Value>> {
    if uri.query().is_some() {
        return Err(PublicApiError::bad_request(
            "Editorial writes accept no query parameters",
        ));
    }
    let method = request.method().as_str().to_owned();
    let delete = method == "DELETE";
    let body = read_body(request, &headers, delete).await?;
    let cap = admitted_request(&state, &uri, &headers, &method, &body).await?;
    let payload = if delete {
        Value::Null
    } else {
        let value = serde_json::from_slice(&body)
            .map_err(|_| PublicApiError::bad_request("Invalid JSON body"))?;
        if kind == ResourceKind::Author {
            normalize_author(value, id.is_some())?
        } else {
            normalize_group(value, id.is_some())?
        }
    };
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let actor = bound_account_write(&tx, &cap).await?;
    let native = if let Some(id) = id {
        Some(native_id(&tx, kind, &id).await?)
    } else {
        None
    };
    let data = match (kind, native.as_deref(), delete) {
        (ResourceKind::Author, None, false) => Some(
            core::author_create(
                &tx,
                &actor,
                core::AuthorCreate::parse_public_normalized(&payload)?,
            )
            .await?,
        ),
        (ResourceKind::Author, Some(id), false) => Some(
            core::author_update(
                &tx,
                &actor,
                id,
                core::AuthorUpdate::parse_public_normalized(&actor, &payload)?,
            )
            .await?,
        ),
        (ResourceKind::Group, None, false) => Some(
            core::group_create(
                &tx,
                &actor,
                core::GroupCreate::parse_public_normalized(&payload)?,
            )
            .await?,
        ),
        (ResourceKind::Group, Some(id), false) => {
            let mut payload = payload;
            member_ids(&tx, &mut payload).await?;
            Some(
                core::group_update(
                    &tx,
                    &actor,
                    id,
                    core::GroupUpdate::parse_public_normalized(&payload)?,
                )
                .await?,
            )
        }
        (_, Some(id), true) => {
            // Shared legacy deletion keeps its native behavior. The public
            // boundary additionally refuses hidden protocol projections.
            let sql = if kind == ResourceKind::Author {
                "SELECT id FROM platform.catalog_authors WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_source_is_visible(protocol_event_id) FOR UPDATE"
            } else {
                "SELECT id FROM platform.catalog_groups WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_group_is_visible(id) FOR UPDATE"
            };
            tx.query_opt(sql, &[&id])
                .await?
                .ok_or_else(PublicApiError::not_found)?;
            if kind == ResourceKind::Author {
                core::author_delete(&tx, &actor, id).await?;
            } else {
                core::group_delete(&tx, &actor, id).await?;
            }
            None
        }
        _ => {
            return Err(PublicApiError::bad_request(
                "Unsupported editorial operation",
            ));
        }
    };
    let response = if let Some(data) = data {
        let index = AliasIndex::load_in(&tx, &[(kind, data.clone())]).await?;
        serde_json::to_value(EntityResponse::<Resource>::new(index.resource(
            kind,
            &data,
            &BTreeSet::new(),
        )?))
        .map_err(|_| PublicApiError::unavailable())?
    } else {
        json!({"result":"ok"})
    };
    tx.commit().await?;
    Ok(Json(response))
}
macro_rules! create_handler {
    ($handler:ident,$kind:ident) => {
        async fn $handler(
            State(state): State<AppState>,
            OriginalUri(uri): OriginalUri,
            headers: HeaderMap,
            request: Request,
        ) -> PublicApiResult<Json<Value>> {
            mutate(state, uri, headers, request, ResourceKind::$kind, None).await
        }
    };
}
macro_rules! id_handler {
    ($handler:ident,$kind:ident) => {
        async fn $handler(
            State(state): State<AppState>,
            Path(id): Path<String>,
            OriginalUri(uri): OriginalUri,
            headers: HeaderMap,
            request: Request,
        ) -> PublicApiResult<Json<Value>> {
            mutate(state, uri, headers, request, ResourceKind::$kind, Some(id)).await
        }
    };
}
create_handler!(author_create, Author);
create_handler!(group_create, Group);
id_handler!(author_update, Author);
id_handler!(author_delete, Author);
id_handler!(group_update, Group);
id_handler!(group_delete, Group);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn author_contract_validates_nullable_links_and_closed_fields() {
        assert_eq!(normalize_author(json!({"name":"Author","biography":{"en":"A biography"},"fanBox":null,"weibo":"https://author.weibo.cn/a"}),false).unwrap()["fanbox"],Value::Null);
        for value in [
            json!({"name":"Author","fanbox":null}),
            json!({"name":"Author","biography":null}),
            json!({"name":"Author","twitter":"https://twitter.com.evil/a"}),
            json!({"name":"Author","pixiv":"https://a.b.pixiv.net/a"}),
            json!({"name":"Author","locked":true}),
            json!({"name":null}),
            json!({"name":"Author","twitter":42}),
        ] {
            assert!(normalize_author(value, false).is_err());
        }
        assert!(normalize_author(json!({"version":1,"website":null}), true).is_ok());
        for version in [
            json!(null),
            json!(0),
            json!(-1),
            json!(1.0),
            json!(2147483648_u64),
        ] {
            assert!(normalize_author(json!({"version":version}), true).is_err());
        }
    }
    #[test]
    fn group_contract_distinguishes_create_and_edit() {
        assert!(
            normalize_group(
                json!({"name":"Group","publishDelay":null,"inactive":true}),
                false
            )
            .is_ok()
        );
        assert!(normalize_group(json!({"version":1,"publishDelay":null}), true).is_err());
        assert!(
            normalize_group(
                json!({"version":1,"publishDelay":"custom","focusedLanguages":null}),
                true
            )
            .is_ok()
        );
        for delay in ["P", "PT", "P1D", "P99D9WT24H60M60S", "PT1S"] {
            assert!(publish_delay(delay), "{delay}");
        }
        for delay in [
            "", "P0D", "P01D", "P100D", "P10W", "PT25H", "PT61M", "PT1M1H", "P1DT0S", "P1M",
        ] {
            assert!(!publish_delay(delay), "{delay}");
        }
        for value in [
            json!({"name":"Group","focusedLanguages":["en"]}),
            json!({"name":"Group","inactive":null}),
            json!({"name":"Group","mangaUpdates":"https://mangaupdates.com/group/a"}),
            json!({"version":1,"focusedLanguages":["EN"]}),
            json!({"version":1,"members":["native-id"]}),
        ] {
            let edit = value.get("version").is_some();
            assert!(normalize_group(value, edit).is_err());
        }
    }
}
