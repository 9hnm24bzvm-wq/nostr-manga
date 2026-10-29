//! Private account-directory and persisted-follow reads. Legacy signed routes
//! require admitted proof and binding. Separate native follow routes require
//! current cookie account authority; route selection never falls back between them.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdTarget, PublicUuid,
    ResourceKind,
    auth::{admitted_request, bound_account_read},
    catalogue::{self, MANGA_INCLUDES},
    catalogue_dto::AliasIndex,
    dto::EntityResponse,
    query::{PublicQuery, QuerySchema, SortDirection},
};
use crate::{AppState, domains::catalog::query::Sql};
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{OriginalUri, Path, Request, State},
    http::{HeaderMap, HeaderValue, header},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, time::Duration};

#[derive(Clone, Copy)]
enum Operation {
    Directory,
    SelfProfile,
    History,
    Settings,
    SettingsTemplate,
    FollowList(ResourceKind),
    FollowStatus(ResourceKind),
    FollowFeed,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/user", get(directory))
        .route("/user/me", get(me))
        .route("/user/history", get(history))
        .route(
            "/settings",
            get(settings).post(super::settings_writes::write),
        )
        .route("/settings/template", get(template))
        .route("/settings/template/{p0}", get(template_version))
        .route("/user/follows/manga", get(manga_list))
        .route("/user/follows/manga/feed", get(feed))
        .route("/user/follows/manga/{p0}", get(manga_status))
        .route("/user/follows/group", get(group_list))
        .route("/user/follows/group/{p0}", get(group_status))
        .route("/user/follows/user", get(user_list))
        .route("/user/follows/user/{p0}", get(user_status))
        .layer(axum::middleware::from_fn(private_headers))
}

pub fn native_router() -> Router<AppState> {
    Router::new()
        .route("/account/user/follows/manga", get(native_manga_list))
        .route("/account/user/follows/group", get(native_group_list))
        .route("/account/user/follows/user", get(native_user_list))
        .route("/account/user/follows/manga/feed", get(native_feed))
}
macro_rules! native_list_handler {
    ($name:ident, $operation:expr) => {
        async fn $name(
            State(state): State<AppState>,
            OriginalUri(uri): OriginalUri,
            headers: HeaderMap,
            request: Request,
        ) -> PublicApiResult<Response> {
            read(state, uri, headers, request, None, $operation, true).await
        }
    };
}
native_list_handler!(
    native_manga_list,
    Operation::FollowList(ResourceKind::Manga)
);
native_list_handler!(
    native_group_list,
    Operation::FollowList(ResourceKind::Group)
);
native_list_handler!(native_user_list, Operation::FollowList(ResourceKind::User));
native_list_handler!(native_feed, Operation::FollowFeed);

async fn private_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response.headers_mut().insert(
        header::VARY,
        HeaderValue::from_static("Authorization, Cookie"),
    );
    response
}

macro_rules! list_handler {
    ($name:ident, $operation:expr) => {
        async fn $name(
            State(state): State<AppState>,
            OriginalUri(uri): OriginalUri,
            headers: HeaderMap,
            request: Request,
        ) -> PublicApiResult<Response> {
            read(state, uri, headers, request, None, $operation, false).await
        }
    };
}
macro_rules! status_handler {
    ($name:ident, $kind:ident) => {
        async fn $name(
            State(state): State<AppState>,
            OriginalUri(uri): OriginalUri,
            Path(id): Path<String>,
            headers: HeaderMap,
            request: Request,
        ) -> PublicApiResult<Response> {
            read(
                state,
                uri,
                headers,
                request,
                Some(id),
                Operation::FollowStatus(ResourceKind::$kind),
                false,
            )
            .await
        }
    };
}
list_handler!(directory, Operation::Directory);
list_handler!(me, Operation::SelfProfile);
list_handler!(history, Operation::History);
list_handler!(settings, Operation::Settings);
list_handler!(template, Operation::SettingsTemplate);
async fn template_version(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Response> {
    read(
        state,
        uri,
        headers,
        request,
        Some(id),
        Operation::SettingsTemplate,
        false,
    )
    .await
}
list_handler!(feed, Operation::FollowFeed);
list_handler!(manga_list, Operation::FollowList(ResourceKind::Manga));
list_handler!(group_list, Operation::FollowList(ResourceKind::Group));
list_handler!(user_list, Operation::FollowList(ResourceKind::User));
status_handler!(manga_status, Manga);
status_handler!(group_status, Group);
status_handler!(user_status, User);

fn query(raw: Option<&str>, operation: Operation) -> PublicApiResult<PublicQuery> {
    let includes: &[&str] = match operation {
        Operation::FollowList(ResourceKind::Manga) => MANGA_INCLUDES,
        Operation::FollowList(ResourceKind::Group) => &["leader", "member"],
        _ => &[],
    };
    let mut q = match operation {
        Operation::SelfProfile
        | Operation::History
        | Operation::Settings
        | Operation::SettingsTemplate
        | Operation::FollowStatus(_) => {
            let q = PublicQuery::parse_unchecked(raw)?;
            q.require_allowed(&[])?;
            q
        }
        Operation::FollowFeed => {
            let q = PublicQuery::parse(
                raw,
                &catalogue::collection_schema(ResourceKind::Chapter, true),
            )?;
            if q.limit == 0 {
                return Err(PublicApiError::bad_request("Feed limit must be 1..500"));
            }
            q
        }
        Operation::Directory | Operation::FollowList(_) => {
            let directory = matches!(operation, Operation::Directory);
            let q = PublicQuery::parse(
                raw,
                &QuerySchema {
                    default_limit: 10,
                    max_limit: 100,
                    max_offset: 10000,
                    scalar: if directory { &["username"] } else { &[] },
                    arrays: if directory { &["ids[]"] } else { &[] },
                    includes,
                    order: if directory { &["username"] } else { &[] },
                },
            )?;
            if !directory && q.limit == 0 {
                return Err(PublicApiError::bad_request(
                    "Follow list limit must be 1..100",
                ));
            }
            q
        }
    };
    if matches!(operation, Operation::Directory) && q.order.is_empty() {
        q.order.push(("username".into(), SortDirection::Asc));
    }
    Ok(q)
}

fn follow_table(kind: ResourceKind) -> PublicApiResult<(&'static str, &'static str)> {
    Ok(match kind {
        ResourceKind::Manga => ("platform.library_manga_follows", "manga_id"),
        ResourceKind::Group => ("platform.library_group_follows", "group_id"),
        ResourceKind::User => ("platform.library_user_follows", "target_id"),
        _ => return Err(PublicApiError::bad_request("Unsupported follow resource")),
    })
}

async fn read(
    state: AppState,
    uri: axum::http::Uri,
    headers: HeaderMap,
    request: Request,
    id: Option<String>,
    operation: Operation,
    native_session: bool,
) -> PublicApiResult<Response> {
    let q = query(uri.query(), operation)?;
    let id = id.as_deref().map(PublicUuid::parse).transpose()?;
    let method = request.method().as_str().to_owned();
    let body = tokio::time::timeout(Duration::from_secs(5), to_bytes(request.into_body(), 1024))
        .await
        .map_err(|_| PublicApiError::bad_request("Request body timeout"))?
        .map_err(|_| PublicApiError::bad_request("GET accepts no body"))?;
    if !body.is_empty() {
        return Err(PublicApiError::bad_request("GET accepts no body"));
    }
    let cap = if native_session {
        None
    } else {
        Some(admitted_request(&state, &uri, &headers, &method, &body).await?)
    };
    let mut client = state.db.client().await?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let user = if let Some(cap) = &cap {
        bound_account_read(&tx, cap).await?.user_id().to_owned()
    } else {
        // Existing own-library reads permit unverified/restricted normal
        // accounts; a private collection read is not a social write grant.
        crate::domains::auth::require_native_library_principal_in(&state, &headers, &tx)
            .await?
            .user_id
    };
    let response = match operation {
        Operation::Settings => {
            Json(super::account_settings::read_in(&tx, user.as_str()).await?).into_response()
        }
        Operation::SettingsTemplate => {
            if id.is_some_and(|id| id.as_str() != super::account_settings::TEMPLATE_ID) {
                return Err(PublicApiError::not_found());
            }
            Json(super::account_settings::template()?).into_response()
        }
        Operation::History => {
            // The original signed graph has no per-read date. Only real private
            // native history records supply dates through this explicit scope.
            let rows = tx.query("SELECT c.id,platform.catalog_timestamp(h.read_at),p.public_id::text FROM platform.library_chapter_history h JOIN platform.catalog_chapters c ON c.id=h.chapter_id AND c.manga_id=h.manga_id JOIN platform.catalog_manga m ON m.id=c.manga_id LEFT JOIN platform.public_resource_ids p ON p.kind='chapter' AND p.native_id=c.id AND p.preferred WHERE h.user_id=$1 AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id) ORDER BY h.read_at DESC,c.id ASC LIMIT 10001", &[&user.as_str()]).await?;
            if rows.len() > 10000 {
                return Err(PublicApiError::unavailable());
            }
            let records = rows
                .into_iter()
                .map(|row| {
                    let id: Option<&str> = row.get(2);
                    let id = PublicUuid::parse(id.ok_or_else(PublicApiError::unavailable)?)
                        .map_err(|_| PublicApiError::unavailable())?;
                    Ok(json!({"chapterId":id,"readDate":row.get::<_,String>(1)}))
                })
                .collect::<PublicApiResult<Vec<_>>>()?;
            // `ratings` is the history-array field in the official5.13.1 schema.
            Json(json!({"result":"ok","ratings":records,"nostr":{"scope":"instance-account","source":"persisted-native-account-history","readDateSemantics":"stored-native-read-time"}})).into_response()
        }
        Operation::SelfProfile => {
            let native: Value = tx.query_one("SELECT jsonb_build_object('id',id,'username',username,'roles',roles,'version',version) FROM platform.users WHERE id=$1 AND NOT disabled", &[&user.as_str()]).await?.get(0);
            let index = AliasIndex::load_in(&tx, &[(ResourceKind::User, native.clone())]).await?;
            let mut response = EntityResponse::new(index.resource(
                ResourceKind::User,
                &native,
                &BTreeSet::new(),
            )?);
            response.extensions.insert(
                "nostr".into(),
                json!({"scope":"instance-account","identity":"explicitly-bound-native-account"}),
            );
            Json(response).into_response()
        }
        Operation::FollowStatus(kind) => {
            let id = id.ok_or_else(PublicApiError::not_found)?;
            let target = PostgresPublicIdResolver::resolve_many_in(&*tx, kind, &[id])
                .await?
                .into_values()
                .find(|target| target.target == PublicIdTarget::NativeResource)
                .ok_or_else(PublicApiError::not_found)?;
            let (alias, table, _, visible) = catalogue::parts(kind)?;
            let exists: bool = tx
                .query_one(
                    &format!(
                        "SELECT EXISTS(SELECT 1 FROM {table} WHERE {alias}.id=$1 AND {visible})"
                    ),
                    &[&target.native_id],
                )
                .await?
                .get(0);
            if !exists {
                return Err(PublicApiError::not_found());
            }
            let follow_target = match kind {
                ResourceKind::Manga => crate::domains::library::follow_mutations::Target::Manga,
                ResourceKind::Group => crate::domains::library::follow_mutations::Target::Group,
                _ => crate::domains::library::follow_mutations::Target::User,
            };
            let mut metadata = crate::domains::library::follow_mutations::state_in(
                &tx,
                user.as_str(),
                follow_target,
                &target.native_id,
            )
            .await?;
            if metadata["followed"] != true {
                return Err(PublicApiError::not_found());
            }
            metadata["accountId"] =
                json!(crate::domains::library::account_public_id_in(&tx, user.as_str()).await?);
            metadata["targetId"] = json!(target.preferred_id);
            metadata["kind"] = json!(kind.as_str());
            metadata["scope"] = json!("instance-account");
            metadata["source"] = json!("persisted-native-account-follows");
            metadata["versionSemantics"] = json!("membership-change-revision");
            Json(json!({"result":"ok","nostr":metadata})).into_response()
        }
        Operation::Directory | Operation::FollowList(_) | Operation::FollowFeed => {
            let kind = match operation {
                Operation::Directory => ResourceKind::User,
                Operation::FollowList(kind) => kind,
                _ => ResourceKind::Chapter,
            };
            let mut sql = Sql::default();
            if matches!(operation, Operation::FollowList(_)) {
                sql.conditions.push(catalogue::parts(kind)?.3.into());
            } else {
                catalogue::filters(
                    &state,
                    Some(&tx),
                    kind,
                    &q,
                    &mut sql,
                    None,
                    matches!(operation, Operation::FollowFeed).then_some(user.as_str()),
                )
                .await?;
            }
            if matches!(operation, Operation::Directory) {
                if let Some(username) = q.first("username") {
                    let username = sql.bind(username.to_owned());
                    sql.conditions.push(format!(
                        "position(lower({username}) in lower(u.username))>0"
                    ));
                }
            }
            if let Operation::FollowList(kind) = operation {
                let (alias, _, _, _) = catalogue::parts(kind)?;
                let (follows, column) = follow_table(kind)?;
                let owner = sql.bind(user.as_str().to_owned());
                sql.conditions.push(format!("EXISTS(SELECT 1 FROM {follows} f WHERE f.user_id={owner} AND f.{column}={alias}.id)"));
            }
            let mut response = catalogue::collection_in(&tx, kind, &q, &mut sql).await?;
            response.extensions.insert("nostr".into(), json!({"scope":"instance-account","source":"persisted-native-account-library","accountId":crate::domains::library::account_public_id_in(&tx,user.as_str()).await?,"requires":if native_session {"native-session-account"} else {"request-bound-nostr-proof-and-explicit-native-account-binding"}}));
            if native_session {
                response.extensions.get_mut("nostr").unwrap()["authentication"] =
                    json!("native-session");
            }
            Json(response).into_response()
        }
    };
    tx.commit().await?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_read_queries_are_operation_specific() {
        assert_eq!(query(None, Operation::Directory).unwrap().limit, 10);
        assert!(
            query(
                Some("limit=0&username=user&order[username]=desc"),
                Operation::Directory
            )
            .is_ok()
        );
        assert!(
            query(
                Some("includes[]=creator&includes[]=tag"),
                Operation::FollowList(ResourceKind::Manga)
            )
            .is_ok()
        );
        assert!(query(Some("limit=500&translatedLanguage[]=en&excludedGroups[]=11111111-1111-8111-8111-111111111111"), Operation::FollowFeed).is_ok());
        for operation in [
            Operation::SelfProfile,
            Operation::History,
            Operation::FollowStatus(ResourceKind::Manga),
        ] {
            assert!(query(Some("limit=0"), operation).is_err());
        }
        for raw in [
            "limit=0",
            "username=other",
            "ids[]=11111111-1111-8111-8111-111111111111",
            "order[username]=asc",
        ] {
            assert!(query(Some(raw), Operation::FollowList(ResourceKind::User)).is_err());
        }
    }
}
