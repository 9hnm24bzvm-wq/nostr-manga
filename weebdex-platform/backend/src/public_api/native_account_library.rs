//! Cookie-only adapters over the existing native library. No second identity,
//! library silo, signing key, public grant, or protocol publication is involved.
use super::{
    PostgresPublicIdResolver, PublicApiError as Error, PublicApiResult as Result, PublicIdTarget,
    PublicUuid, ResourceKind, catalogue_dto::AliasIndex,
};
use crate::{
    AppState,
    contracts::library::ReadingStatus,
    domains::{
        auth,
        catalog::query::{QueryParams, Sql, chapter_filters, manga_filters},
        library::{account_public_id_in, repository as repo, validation},
    },
};
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{OriginalUri, Path, Request, State},
    http::{HeaderMap, Uri, header},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use tokio_postgres::Transaction;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/account/library/follows", get(follows))
        .route("/account/library/follows/state", get(follow_states))
        .route(
            "/account/library/follows/{id}",
            post(follow_set).delete(follow_delete),
        )
        .route("/account/library/updates", get(updates))
        .route("/account/library/continue/{id}", get(continue_read))
        .route("/account/library/history/feed", get(history_feed))
        .route("/account/library/history/state", get(history_state))
        .route("/account/library/history/{id}", post(history_write))
        .route(
            "/account/library/progress/{id}",
            get(progress_read).post(progress_write),
        )
        .route("/account/library/lists", get(lists).post(list_create))
        .route("/account/library/lists/{id}/titles", get(list_titles))
        .route(
            "/account/library/lists/{id}",
            get(list_read).post(list_write).delete(list_delete),
        )
}
const CHAPTER_VISIBLE: &str = "c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)";
const MAX_REVISION: i64 = 9_007_199_254_740_991;
async fn body(request: Request, headers: &HeaderMap, write: bool) -> Result<Vec<u8>> {
    if write {
        let mut types = headers.get_all(header::CONTENT_TYPE).iter();
        if !types.next().and_then(|v| v.to_str().ok()).is_some_and(|v| {
            v.split(';')
                .next()
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
        }) || types.next().is_some()
        {
            return Err(Error::bad_request("JSON content type required"));
        }
    }
    let bytes = tokio::time::timeout(
        Duration::from_secs(5),
        to_bytes(request.into_body(), if write { 8192 } else { 1024 }),
    )
    .await
    .map_err(|_| Error::bad_request("Request body timeout"))?
    .map_err(|_| Error::bad_request("Request body exceeds operation budget"))?;
    if !write && !bytes.is_empty() {
        return Err(Error::bad_request("GET accepts no body"));
    }
    Ok(bytes.to_vec())
}
fn no_query(uri: &Uri) -> Result<()> {
    if uri.query().is_some() {
        Err(Error::bad_request(
            "This operation accepts no query parameters",
        ))
    } else {
        Ok(())
    }
}
async fn actor(
    state: &AppState,
    headers: &HeaderMap,
    tx: &Transaction<'_>,
    write: bool,
) -> Result<auth::SessionPrincipal> {
    crate::domains::catalog::reader_selector::deadline(tx).await?;
    let actor = if write {
        auth::require_native_account_principal_in(state, headers, tx, true).await?
    } else {
        auth::require_native_library_principal_in(state, headers, tx).await?
    };
    if write {
        auth::validate_mutation(state, headers)?;
        actor.require_permission("self.preferences.update")?;
    } else {
        actor.require_library_read()?;
    }
    Ok(actor)
}
async fn revision(tx: &Transaction<'_>, user: &str) -> Result<i64> {
    Ok(tx
        .query_opt(
            "SELECT revision FROM platform.library_account_revisions WHERE user_id=$1",
            &[&user],
        )
        .await?
        .map_or(0, |r| r.get(0)))
}
async fn envelope(tx: &Transaction<'_>, user: &str, extra: Value) -> Result<Value> {
    let mut out = json!({"result":"ok","authentication":"native-session","scope":"instance-account","accountId":account_public_id_in(tx,user).await?,"revision":revision(tx,user).await?});
    out.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    Ok(out)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Command {
    expected_account_id: PublicUuid,
    revision: i64,
    status: Option<ReadingStatus>,
    notification: Option<bool>,
    read_ids: Option<Vec<PublicUuid>>,
    unread_ids: Option<Vec<PublicUuid>>,
    page: Option<i32>,
    completed: Option<bool>,
    progress_version: Option<i32>,
    name: Option<String>,
    manga_ids: Option<Vec<PublicUuid>>,
    list_version: Option<i32>,
}
impl Command {
    fn parse(bytes: &[u8], allowed: &[&str]) -> Result<Self> {
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| Error::bad_request("Invalid library command JSON"))?;
        let object = value
            .as_object()
            .ok_or_else(|| Error::bad_request("Expected command object"))?;
        if object.keys().any(|k| {
            !matches!(k.as_str(), "expectedAccountId" | "revision")
                && !allowed.contains(&k.as_str())
        }) {
            return Err(Error::bad_request("Unsupported command field"));
        }
        let out: Self = serde_json::from_slice(bytes)
            .map_err(|_| Error::bad_request("Invalid library command fields"))?;
        if !(0..MAX_REVISION).contains(&out.revision) {
            return Err(Error::bad_request("Invalid library revision"));
        }
        Ok(out)
    }
    async fn check(&self, tx: &Transaction<'_>, user: &str) -> Result<()> {
        if self.expected_account_id.as_str() != account_public_id_in(tx, user).await? {
            return Err(Error::conflict("Account changed; reload before saving"));
        }
        if self.revision != revision(tx, user).await? {
            return Err(Error::conflict("Library changed; reload before saving"));
        }
        Ok(())
    }
}
async fn resolve(
    tx: &Transaction<'_>,
    kind: ResourceKind,
    ids: &[PublicUuid],
) -> Result<Vec<super::ResolvedPublicId>> {
    if ids.len() > 100 {
        return Err(Error::bad_request("At most 100 identifiers are allowed"));
    }
    let map = PostgresPublicIdResolver::resolve_many_in(tx, kind, ids).await?;
    let mut seen = BTreeSet::new();
    let mut out = vec![];
    for id in ids {
        let target = map
            .get(id)
            .filter(|r| r.target == PublicIdTarget::NativeResource)
            .ok_or_else(Error::not_found)?;
        if seen.insert(target.native_id.clone()) {
            out.push(target.clone());
        }
    }
    Ok(out)
}
async fn target(
    tx: &Transaction<'_>,
    kind: ResourceKind,
    id: &PublicUuid,
) -> Result<super::ResolvedPublicId> {
    resolve(tx, kind, std::slice::from_ref(id))
        .await?
        .pop()
        .ok_or_else(Error::not_found)
}
async fn visible_manga(tx: &Transaction<'_>, ids: &[String]) -> Result<()> {
    let count: i64 = tx
        .query_one(
            &format!(
                "SELECT count(*) FROM platform.catalog_manga m WHERE m.id=ANY($1::text[]) AND {}",
                super::catalogue::MANGA_VISIBLE
            ),
            &[&ids],
        )
        .await?
        .get(0);
    if count as usize != ids.len() {
        return Err(Error::not_found());
    }
    Ok(())
}
async fn visible_chapter(tx: &Transaction<'_>, id: &str) -> Result<(String, i32)> {
    tx.query_opt(&format!("SELECT c.manga_id,jsonb_array_length(COALESCE(c.metadata->'data','[]'::jsonb)) FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=$1 AND {CHAPTER_VISIBLE}"),&[&id]).await?.map(|r|(r.get(0),r.get(1))).ok_or_else(Error::not_found)
}
async fn public_id(tx: &Transaction<'_>, kind: ResourceKind, id: &str) -> Result<String> {
    tx.query_opt("SELECT public_id::text FROM platform.public_resource_ids WHERE kind=$1 AND native_id=$2 AND preferred AND principal_type='native'",&[&kind.as_str(),&id]).await?.map(|r|r.get(0)).ok_or_else(Error::unavailable)
}
fn raw_query(raw: Option<&str>, allowed: &[&str]) -> Result<BTreeMap<String, Vec<String>>> {
    let raw = raw.unwrap_or("");
    if raw.len() > 16384 {
        return Err(Error::bad_request("Query is too long"));
    }
    let mut map = BTreeMap::<String, Vec<String>>::new();
    for (k, v) in url::form_urlencoded::parse(raw.as_bytes()) {
        let k = k.strip_suffix("[]").unwrap_or(&k).to_owned();
        if !allowed.contains(&k.as_str()) || v.len() > 1000 {
            return Err(Error::bad_request("Unsupported query parameter"));
        }
        let values = map.entry(k).or_default();
        if values.len() >= 100 {
            return Err(Error::bad_request("Too many query values"));
        }
        values.push(v.into_owned());
    }
    for k in [
        "page",
        "limit",
        "sort",
        "order",
        "title",
        "yearFrom",
        "yearTo",
        "tmod",
        "txmod",
        "hasChapters",
        "list",
        "latest",
    ] {
        if map.get(k).is_some_and(|v| v.len() > 1) {
            return Err(Error::bad_request(format!("{k} must occur once")));
        }
    }
    Ok(map)
}
fn paging(map: &BTreeMap<String, Vec<String>>) -> Result<(i64, i64)> {
    let int = |k: &str, default: i64| {
        map.get(k).map_or(Ok(default), |v| {
            v[0].parse::<i64>()
                .map_err(|_| Error::bad_request("Invalid pagination"))
        })
    };
    let limit = int("limit", 20)?;
    let page = int("page", 1)?;
    if !(1..=100).contains(&limit) || !(1..=1_000_000).contains(&page) || (page - 1) * limit > 10000
    {
        return Err(Error::bad_request(
            "Limit must be 1..100 and offset at most 10000",
        ));
    }
    Ok((limit, page))
}
const MANGA_QUERY: &[&str] = &[
    "page",
    "limit",
    "sort",
    "order",
    "title",
    "reading_status",
    "contentRating",
    "lang",
    "langx",
    "availableTranslatedLang",
    "status",
    "demographic",
    "yearFrom",
    "yearTo",
    "tag",
    "tagx",
    "tmod",
    "txmod",
    "id",
    "author",
    "artist",
    "artists",
    "authorOrArtist",
    "hasChapters",
    "list",
];
async fn query(
    tx: &Transaction<'_>,
    raw: Option<&str>,
    chapters: bool,
    latest: bool,
) -> Result<QueryParams> {
    let mut allowed = MANGA_QUERY.to_vec();
    if chapters {
        allowed.extend([
            "tlang",
            "tlangx",
            "uploader",
            "uploaderx",
            "group",
            "groupx",
            "manga",
        ]);
    }
    if latest {
        allowed.push("latest");
    }
    let mut values = raw_query(raw, &allowed)?;
    paging(&values)?;
    for (key, kind) in [
        (
            "id",
            if chapters {
                ResourceKind::Chapter
            } else {
                ResourceKind::Manga
            },
        ),
        ("tag", ResourceKind::Tag),
        ("tagx", ResourceKind::Tag),
        ("author", ResourceKind::Author),
        ("artist", ResourceKind::Author),
        ("artists", ResourceKind::Author),
        ("authorOrArtist", ResourceKind::Author),
        ("manga", ResourceKind::Manga),
        ("uploader", ResourceKind::User),
        ("uploaderx", ResourceKind::User),
        ("group", ResourceKind::Group),
        ("groupx", ResourceKind::Group),
        ("list", ResourceKind::List),
    ] {
        if let Some(ids) = values.get(key) {
            let ids = ids
                .iter()
                .map(|s| PublicUuid::parse(s))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let targets = resolve(tx, kind, &ids).await?;
            values.insert(
                key.into(),
                targets.into_iter().map(|t| t.native_id).collect(),
            );
        }
    }
    let raw = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(
            values
                .iter()
                .flat_map(|(k, vs)| vs.iter().map(move |v| (k, v))),
        )
        .finish();
    Ok(QueryParams::parse(Some(raw))?)
}
async fn list_filter(
    tx: &Transaction<'_>,
    actor: &auth::SessionPrincipal,
    q: &QueryParams,
    sql: &mut Sql,
) -> Result<()> {
    if let Some(id) = q.first("list") {
        owned_list(tx, &actor.user_id, id, false).await?;
        let p = sql.bind(id.to_owned());
        sql.conditions.push(format!("EXISTS(SELECT 1 FROM platform.library_list_members lm WHERE lm.list_id={p} AND lm.manga_id=m.id)"));
    }
    Ok(())
}
async fn follows(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>> {
    body(request, &headers, false).await?;
    let mut db = state.db.client().await?;
    let tx = db
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = actor(&state, &headers, &tx, false).await?;
    let q = query(&tx, uri.query(), false, false).await?;
    let mut sql = Sql::default();
    let user = sql.bind(actor.user_id.clone());
    sql.conditions.push(format!(
        "f.user_id={user} AND {}",
        super::catalogue::MANGA_VISIBLE
    ));
    manga_filters(&q, &mut sql, "m", Some(&actor))?;
    list_filter(&tx, &actor, &q, &mut sql).await?;
    let sort = match q.first("sort").unwrap_or("followedAt") {
        "followedAt" => "f.followed_at",
        "createdAt" => "m.created_at",
        "updatedAt" => "m.updated_at",
        "title" => "lower(m.title)",
        "year" => "m.year",
        "rating" => "COALESCE(s.rating,0)",
        "views" => "COALESCE(s.views,0)",
        "follows" => "COALESCE(s.follows,0)",
        "lastUploadedChapterAt" => {
            "(SELECT max(c.published_at) FROM platform.catalog_chapters c WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id))"
        }
        "chapters" => {
            "(SELECT count(*) FROM platform.catalog_chapters c WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id))"
        }
        _ => return Err(Error::bad_request("Invalid library sort")),
    };
    let order = q.order("desc")?;
    let from = "platform.library_manga_follows f JOIN platform.catalog_manga m ON m.id=f.manga_id LEFT JOIN platform.catalog_manga_counters s ON s.manga_id=m.id";
    let pred = sql.predicate();
    let total: i64 = tx
        .query_one(
            &format!("SELECT count(*) FROM {from} WHERE {pred}"),
            &sql.params(),
        )
        .await?
        .get(0);
    let limit = sql.bind(q.limit);
    let offset = sql.bind((q.page - 1) * q.limit);
    let rows=tx.query(&format!("SELECT platform.catalog_public_manga_json(m.id),f.reading_status,f.notification,platform.catalog_timestamp(f.followed_at) FROM {from} WHERE {pred} ORDER BY {sort} {order} NULLS LAST,m.id ASC LIMIT {limit} OFFSET {offset}"),&sql.params()).await?;
    let resources = rows
        .iter()
        .map(|r| (ResourceKind::Manga, r.get::<_, Value>(0)))
        .collect::<Vec<_>>();
    let index = AliasIndex::load_in(&tx, &resources).await?;
    let includes = ["cover_art", "author", "artist", "tag"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let data=rows.iter().zip(resources.iter()).map(|(r,(_,v))|Ok(json!({"manga":index.resource(ResourceKind::Manga,v,&includes)?,"status":r.get::<_,String>(1),"notification":r.get::<_,bool>(2),"followedAt":r.get::<_,String>(3)}))).collect::<Result<Vec<_>>>()?;
    let out = envelope(
        &tx,
        &actor.user_id,
        json!({"total":total,"limit":q.limit,"page":q.page,"data":data}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(out))
}
async fn follow_state(
    tx: &Transaction<'_>,
    user: &str,
    target: &super::ResolvedPublicId,
) -> Result<Value> {
    let row=tx.query_opt("SELECT reading_status,notification,platform.catalog_timestamp(followed_at) FROM platform.library_manga_follows WHERE user_id=$1 AND manga_id=$2",&[&user,&target.native_id]).await?;
    Ok(match row {
        Some(r) => {
            json!({"mangaId":target.preferred_id,"saved":true,"status":r.get::<_,String>(0),"notification":r.get::<_,bool>(1),"followedAt":r.get::<_,String>(2)})
        }
        None => json!({"mangaId":target.preferred_id,"saved":false}),
    })
}
async fn follow_states(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>> {
    let q = raw_query(uri.query(), &["ids"])?;
    let ids = q
        .get("ids")
        .filter(|v| !v.is_empty())
        .ok_or_else(|| Error::bad_request("Select 1..100 manga IDs"))?
        .iter()
        .map(|s| PublicUuid::parse(s))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    body(request, &headers, false).await?;
    let mut db = state.db.client().await?;
    let tx = db
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = actor(&state, &headers, &tx, false).await?;
    let targets = resolve(&tx, ResourceKind::Manga, &ids).await?;
    visible_manga(
        &tx,
        &targets
            .iter()
            .map(|t| t.native_id.clone())
            .collect::<Vec<_>>(),
    )
    .await?;
    let mut data = vec![];
    for t in targets {
        data.push(follow_state(&tx, &actor.user_id, &t).await?);
    }
    let out = envelope(&tx, &actor.user_id, json!({"data":data})).await?;
    tx.commit().await?;
    Ok(Json(out))
}
async fn follow_set(
    State(s): State<AppState>,
    OriginalUri(u): OriginalUri,
    Path(id): Path<String>,
    h: HeaderMap,
    r: Request,
) -> Result<Json<Value>> {
    follow_write(s, u, id, h, r, false).await
}
async fn follow_delete(
    State(s): State<AppState>,
    OriginalUri(u): OriginalUri,
    Path(id): Path<String>,
    h: HeaderMap,
    r: Request,
) -> Result<Json<Value>> {
    follow_write(s, u, id, h, r, true).await
}
async fn follow_write(
    state: AppState,
    uri: Uri,
    id: String,
    headers: HeaderMap,
    request: Request,
    delete: bool,
) -> Result<Json<Value>> {
    no_query(&uri)?;
    let id = PublicUuid::parse(&id)?;
    let p = Command::parse(
        &body(request, &headers, true).await?,
        if delete {
            &[]
        } else {
            &["status", "notification"]
        },
    )?;
    if !delete && (p.status.is_none() || p.notification.is_none()) {
        return Err(Error::bad_request("Status and notification are required"));
    }
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, true).await?;
    p.check(&tx, &actor.user_id).await?;
    let target = target(&tx, ResourceKind::Manga, &id).await?;
    repo::published_manga(&tx, &target.native_id).await?;
    if delete {
        let n = tx
            .execute(
                "DELETE FROM platform.library_manga_follows WHERE user_id=$1 AND manga_id=$2",
                &[&actor.user_id, &target.native_id],
            )
            .await?;
        if n > 0 {
            tx.execute("UPDATE platform.catalog_manga_counters SET follows=GREATEST(0,follows-1) WHERE manga_id=$1",&[&target.native_id]).await?;
        }
    } else {
        repo::upsert_follow(
            &tx,
            &actor.user_id,
            &target.native_id,
            p.status.unwrap().as_str(),
            p.notification,
            None,
        )
        .await?;
    }
    let out = envelope(
        &tx,
        &actor.user_id,
        json!({"data":follow_state(&tx,&actor.user_id,&target).await?}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(out))
}
async fn updates(
    State(s): State<AppState>,
    OriginalUri(u): OriginalUri,
    h: HeaderMap,
    r: Request,
) -> Result<Json<Value>> {
    feed(s, u, h, r, false).await
}
async fn history_feed(
    State(s): State<AppState>,
    OriginalUri(u): OriginalUri,
    h: HeaderMap,
    r: Request,
) -> Result<Json<Value>> {
    feed(s, u, h, r, true).await
}
async fn feed(
    state: AppState,
    uri: Uri,
    headers: HeaderMap,
    request: Request,
    history: bool,
) -> Result<Json<Value>> {
    body(request, &headers, false).await?;
    let mut db = state.db.client().await?;
    let tx = db
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = actor(&state, &headers, &tx, false).await?;
    let q = query(&tx, uri.query(), true, !history).await?;
    let latest = q.bool("latest")?.unwrap_or(false);
    let mut sql = Sql::default();
    let user = sql.bind(actor.user_id.clone());
    let join = if history {
        format!(
            "JOIN platform.library_chapter_history h ON h.chapter_id=c.id AND h.manga_id=m.id AND h.user_id={user}"
        )
    } else {
        format!(
            "JOIN platform.library_manga_follows f ON f.manga_id=m.id AND f.user_id={user} AND f.notification LEFT JOIN platform.library_chapter_history h ON h.chapter_id=c.id AND h.manga_id=m.id AND h.user_id={user}"
        )
    };
    sql.conditions.push(CHAPTER_VISIBLE.into());
    let mut mq = q.clone();
    mq.values.remove("id");
    manga_filters(&mq, &mut sql, "m", Some(&actor))?;
    chapter_filters(&q, &mut sql, "c")?;
    list_filter(&tx, &actor, &q, &mut sql).await?;
    let sort = match q
        .first("sort")
        .unwrap_or(if history { "readAt" } else { "publishedAt" })
    {
        "readAt" if history => "h.read_at",
        "publishedAt" => "c.published_at",
        "createdAt" => "c.created_at",
        "updatedAt" => "c.updated_at",
        "chapter" => "c.chapter",
        "volume" => "c.volume",
        _ => return Err(Error::bad_request("Invalid chapter feed sort")),
    };
    let order = q.order("desc")?;
    let from = format!(
        "platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id {join}"
    );
    let pred = sql.predicate();
    let count = if latest {
        "count(DISTINCT c.manga_id)"
    } else {
        "count(*)"
    };
    let total: i64 = tx
        .query_one(
            &format!("SELECT {count} FROM {from} WHERE {pred}"),
            &sql.params(),
        )
        .await?
        .get(0);
    let latest_filter = if latest {
        format!(
            "AND c.id IN (SELECT DISTINCT ON (c.manga_id) c.id FROM {from} WHERE {pred} ORDER BY c.manga_id,c.published_at DESC,c.id ASC)"
        )
    } else {
        String::new()
    };
    let limit = sql.bind(q.limit);
    let offset = sql.bind((q.page - 1) * q.limit);
    let rows=tx.query(&format!("SELECT platform.catalog_public_chapter_json(c.id) || jsonb_build_object('_local_page_source_mutable',c.protocol_event_id IS NULL),platform.catalog_public_manga_json(m.id),h.chapter_id IS NOT NULL,platform.catalog_timestamp(h.read_at) FROM {from} WHERE {pred} {latest_filter} ORDER BY {sort} {order} NULLS LAST,c.id ASC LIMIT {limit} OFFSET {offset}"),&sql.params()).await?;
    let resources = rows
        .iter()
        .flat_map(|r| {
            [
                (ResourceKind::Chapter, r.get::<_, Value>(0)),
                (ResourceKind::Manga, r.get::<_, Value>(1)),
            ]
        })
        .collect::<Vec<_>>();
    let index = AliasIndex::load_in(&tx, &resources).await?;
    let ci = ["scanlation_group", "user"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let mi = ["cover_art", "author", "artist", "tag"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let data=rows.iter().map(|r| {
        let mut value=json!({"chapter":index.resource(ResourceKind::Chapter,&r.get::<_,Value>(0),&ci)?,"manga":index.resource(ResourceKind::Manga,&r.get::<_,Value>(1),&mi)?,"completed":r.get::<_,bool>(2)});
        if let Some(read_at)=r.get::<_,Option<String>>(3){value["readAt"]=json!(read_at);}
        Ok(value)
    }).collect::<Result<Vec<_>>>()?;
    let out = envelope(
        &tx,
        &actor.user_id,
        json!({"total":total,"limit":q.limit,"page":q.page,"data":data}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(out))
}
async fn history_state(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>> {
    let q = raw_query(uri.query(), &["manga", "ids", "limit", "page"])?;
    let (limit, page) = paging(&q)?;
    if q.contains_key("manga") == q.contains_key("ids") {
        return Err(Error::bad_request("Select manga IDs or exact chapter IDs"));
    }
    let kind = if q.contains_key("ids") {
        ResourceKind::Chapter
    } else {
        ResourceKind::Manga
    };
    let key = if kind == ResourceKind::Chapter {
        "ids"
    } else {
        "manga"
    };
    let ids = q[key]
        .iter()
        .map(|s| PublicUuid::parse(s))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    body(request, &headers, false).await?;
    let mut db = state.db.client().await?;
    let tx = db
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = actor(&state, &headers, &tx, false).await?;
    let targets = resolve(&tx, kind, &ids).await?;
    let ids = targets
        .iter()
        .map(|t| t.native_id.clone())
        .collect::<Vec<_>>();
    if kind == ResourceKind::Manga {
        visible_manga(&tx, &ids).await?;
    } else {
        for id in &ids {
            visible_chapter(&tx, id).await?;
        }
    }
    let column = if kind == ResourceKind::Chapter {
        "h.chapter_id"
    } else {
        "h.manga_id"
    };
    let from = format!(
        "platform.library_chapter_history h JOIN platform.catalog_chapters c ON c.id=h.chapter_id AND c.manga_id=h.manga_id JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE h.user_id=$1 AND {column}=ANY($2::text[]) AND {CHAPTER_VISIBLE}"
    );
    let total: i64 = tx
        .query_one(
            &format!("SELECT count(*) FROM {from}"),
            &[&actor.user_id, &ids],
        )
        .await?
        .get(0);
    let offset = (page - 1) * limit;
    let rows=tx.query(&format!("SELECT h.manga_id,h.chapter_id,platform.catalog_timestamp(h.read_at) FROM {from} ORDER BY h.read_at DESC,h.chapter_id ASC LIMIT $3 OFFSET $4"),&[&actor.user_id,&ids,&limit,&offset]).await?;
    let mut data = vec![];
    for r in rows {
        data.push(json!({"mangaId":public_id(&tx,ResourceKind::Manga,r.get(0)).await?,"chapterId":public_id(&tx,ResourceKind::Chapter,r.get(1)).await?,"readAt":r.get::<_,String>(2)}));
    }
    let mut extra = json!({"total":total,"limit":limit,"page":page,"data":data});
    if kind == ResourceKind::Chapter {
        extra["selection"] = json!(
            targets
                .iter()
                .map(|target| target.preferred_id.clone())
                .collect::<Vec<_>>()
        );
        extra["catalogIdentity"] =
            json!(crate::domains::catalog::reader_selector::catalog_identity(&tx, &state).await?);
    }
    let out = envelope(&tx, &actor.user_id, extra).await?;
    tx.commit().await?;
    Ok(Json(out))
}
async fn history_write(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>> {
    no_query(&uri)?;
    let id = PublicUuid::parse(&id)?;
    let p = Command::parse(
        &body(request, &headers, true).await?,
        &["readIds", "unreadIds"],
    )?;
    let reads = p.read_ids.as_deref().unwrap_or(&[]);
    let unreads = p.unread_ids.as_deref().unwrap_or(&[]);
    if reads.len() + unreads.len() > 100 || reads.is_empty() && unreads.is_empty() {
        return Err(Error::bad_request("Select 1..100 chapters"));
    }
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, true).await?;
    p.check(&tx, &actor.user_id).await?;
    let manga = target(&tx, ResourceKind::Manga, &id).await?;
    repo::published_manga(&tx, &manga.native_id).await?;
    let reads = resolve(&tx, ResourceKind::Chapter, reads)
        .await?
        .into_iter()
        .map(|t| t.native_id)
        .collect::<Vec<_>>();
    let unreads = resolve(&tx, ResourceKind::Chapter, unreads)
        .await?
        .into_iter()
        .map(|t| t.native_id)
        .collect::<Vec<_>>();
    if unreads.iter().any(|id| reads.contains(id)) {
        return Err(Error::bad_request(
            "Read and unread selections must be disjoint",
        ));
    }
    for id in reads.iter().chain(unreads.iter()) {
        if visible_chapter(&tx, id).await?.0 != manga.native_id {
            return Err(Error::not_found());
        }
    }
    let read_updated=tx.execute("INSERT INTO platform.library_chapter_history(user_id,manga_id,chapter_id) SELECT $1,$2,unnest($3::text[]) ON CONFLICT(user_id,chapter_id) DO NOTHING",&[&actor.user_id,&manga.native_id,&reads]).await?;
    let unread_updated=tx.execute("DELETE FROM platform.library_chapter_history WHERE user_id=$1 AND manga_id=$2 AND chapter_id=ANY($3::text[])",&[&actor.user_id,&manga.native_id,&unreads]).await?;
    tx.execute("UPDATE platform.library_chapter_progress SET completed=true,version=version+1,updated_at=clock_timestamp() WHERE user_id=$1 AND chapter_id=ANY($2::text[]) AND NOT completed",&[&actor.user_id,&reads]).await?;
    tx.execute("UPDATE platform.library_chapter_progress SET completed=false,version=version+1,updated_at=clock_timestamp() WHERE user_id=$1 AND chapter_id=ANY($2::text[]) AND completed",&[&actor.user_id,&unreads]).await?;
    let out = envelope(
        &tx,
        &actor.user_id,
        json!({"readUpdated":read_updated,"unreadUpdated":unread_updated}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(out))
}
async fn progress_data(
    tx: &Transaction<'_>,
    user: &str,
    chapter: &super::ResolvedPublicId,
    manga: &str,
) -> Result<Value> {
    let row=tx.query_opt("SELECT page,version FROM platform.library_chapter_progress WHERE user_id=$1 AND chapter_id=$2",&[&user,&chapter.native_id]).await?;
    let completed:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.library_chapter_history WHERE user_id=$1 AND chapter_id=$2)",&[&user,&chapter.native_id]).await?.get(0);
    Ok(
        json!({"chapterId":chapter.preferred_id,"mangaId":public_id(tx,ResourceKind::Manga,manga).await?,"page":row.as_ref().map(|r|r.get::<_,i32>(0)),"version":row.as_ref().map_or(0,|r|r.get::<_,i32>(1)),"completed":completed}),
    )
}
async fn progress_read(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>> {
    no_query(&uri)?;
    let id = PublicUuid::parse(&id)?;
    body(request, &headers, false).await?;
    let mut db = state.db.client().await?;
    let tx = db
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = actor(&state, &headers, &tx, false).await?;
    let chapter = target(&tx, ResourceKind::Chapter, &id).await?;
    let (manga, page_count) = visible_chapter(&tx, &chapter.native_id).await?;
    let mut data = progress_data(&tx, &actor.user_id, &chapter, &manga).await?;
    data["pageCount"] = json!(page_count);
    let identity = crate::domains::catalog::reader_selector::catalog_identity(&tx, &state).await?;
    let out = envelope(
        &tx,
        &actor.user_id,
        json!({"data":data,"catalogIdentity":identity}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(out))
}
async fn progress_write(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>> {
    no_query(&uri)?;
    let id = PublicUuid::parse(&id)?;
    let p = Command::parse(
        &body(request, &headers, true).await?,
        &["page", "completed", "progressVersion"],
    )?;
    let (page, completed, version) = match (p.page, p.completed, p.progress_version) {
        (Some(page), Some(completed), Some(version)) if page >= 0 && version >= 0 => {
            (page, completed, version)
        }
        _ => {
            return Err(Error::bad_request(
                "Page, completed and nonnegative progressVersion are required",
            ));
        }
    };
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, true).await?;
    p.check(&tx, &actor.user_id).await?;
    let chapter = target(&tx, ResourceKind::Chapter, &id).await?;
    let (manga, count) = visible_chapter(&tx, &chapter.native_id).await?;
    if count <= 0 || page >= count {
        return Err(Error::bad_request("Progress page is outside the chapter"));
    }
    let current=tx.query_opt("SELECT version,page,completed FROM platform.library_chapter_progress WHERE user_id=$1 AND chapter_id=$2 FOR UPDATE",&[&actor.user_id,&chapter.native_id]).await?;
    let actual = current.as_ref().map_or(0, |r| r.get::<_, i32>(0));
    if actual != version {
        return Err(Error::conflict("Progress changed; reload before saving"));
    }
    let next = version
        .checked_add(1)
        .ok_or_else(|| Error::conflict("Progress version exhausted"))?;
    tx.execute("INSERT INTO platform.library_chapter_progress(user_id,chapter_id,page,completed,version) VALUES($1,$2,$3,$4,$5) ON CONFLICT(user_id,chapter_id) DO UPDATE SET page=EXCLUDED.page,completed=EXCLUDED.completed,version=EXCLUDED.version,updated_at=clock_timestamp()",&[&actor.user_id,&chapter.native_id,&page,&completed,&next]).await?;
    if completed {
        tx.execute("INSERT INTO platform.library_chapter_history(user_id,manga_id,chapter_id) VALUES($1,$2,$3) ON CONFLICT(user_id,chapter_id) DO NOTHING",&[&actor.user_id,&manga,&chapter.native_id]).await?;
    } else {
        tx.execute(
            "DELETE FROM platform.library_chapter_history WHERE user_id=$1 AND chapter_id=$2",
            &[&actor.user_id, &chapter.native_id],
        )
        .await?;
    }
    let out = envelope(
        &tx,
        &actor.user_id,
        json!({"data":progress_data(&tx,&actor.user_id,&chapter,&manga).await?}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(out))
}
async fn owned_list(tx: &Transaction<'_>, user: &str, id: &str, lock: bool) -> Result<i32> {
    let suffix = if lock { " FOR UPDATE" } else { "" };
    tx.query_opt(&format!("SELECT version FROM platform.library_lists WHERE id=$1 AND user_id=$2 AND visibility='private'{suffix}"),&[&id,&user]).await?.map(|r|r.get(0)).ok_or_else(Error::not_found)
}
async fn list_data(tx: &Transaction<'_>, user: &str, id: &str) -> Result<Value> {
    let row=tx.query_opt("SELECT name,version,platform.catalog_timestamp(updated_at) FROM platform.library_lists WHERE id=$1 AND user_id=$2 AND visibility='private'",&[&id,&user]).await?.ok_or_else(Error::not_found)?;
    // Hidden/deleted members are omitted, never projected via an owner-only list.
    // Updating members replaces only this visible projection while retaining
    // hidden memberships, so a rename/reorder cannot silently destroy them.
    let rows=tx.query(&format!("SELECT m.id FROM platform.library_list_members lm JOIN platform.catalog_manga m ON m.id=lm.manga_id WHERE lm.list_id=$1 AND {} ORDER BY lm.position,m.id LIMIT 101",super::catalogue::MANGA_VISIBLE),&[&id]).await?;
    if rows.len() > 100 {
        return Err(Error::unavailable());
    }
    let mut ids = vec![];
    for r in rows {
        ids.push(public_id(tx, ResourceKind::Manga, r.get(0)).await?);
    }
    Ok(
        json!({"id":public_id(tx,ResourceKind::List,id).await?,"name":row.get::<_,String>(0),"version":row.get::<_,i32>(1),"updatedAt":row.get::<_,String>(2),"memberCount":ids.len(),"mangaIds":ids}),
    )
}
async fn lists(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>> {
    let q = raw_query(uri.query(), &["page", "limit"])?;
    let (limit, page) = paging(&q)?;
    body(request, &headers, false).await?;
    let mut db = state.db.client().await?;
    let tx = db
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = actor(&state, &headers, &tx, false).await?;
    let total: i64 = tx
        .query_one(
            "SELECT count(*) FROM platform.library_lists WHERE user_id=$1 AND visibility='private'",
            &[&actor.user_id],
        )
        .await?
        .get(0);
    let offset = (page - 1) * limit;
    let rows=tx.query(&format!("SELECT l.id,l.name,l.version,platform.catalog_timestamp(l.updated_at),(SELECT count(*) FROM platform.library_list_members lm JOIN platform.catalog_manga m ON m.id=lm.manga_id WHERE lm.list_id=l.id AND {}) FROM platform.library_lists l WHERE l.user_id=$1 AND l.visibility='private' ORDER BY l.created_at DESC,l.id ASC LIMIT $2 OFFSET $3",super::catalogue::MANGA_VISIBLE),&[&actor.user_id,&limit,&offset]).await?;
    let mut data = vec![];
    for r in rows {
        data.push(json!({"id":public_id(&tx,ResourceKind::List,r.get(0)).await?,"name":r.get::<_,String>(1),"version":r.get::<_,i32>(2),"updatedAt":r.get::<_,String>(3),"memberCount":r.get::<_,i64>(4)}));
    }
    let out = envelope(
        &tx,
        &actor.user_id,
        json!({"total":total,"limit":limit,"page":page,"data":data}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(out))
}
async fn list_read(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>> {
    no_query(&uri)?;
    let id = PublicUuid::parse(&id)?;
    body(request, &headers, false).await?;
    let mut db = state.db.client().await?;
    let tx = db
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = actor(&state, &headers, &tx, false).await?;
    let target = target(&tx, ResourceKind::List, &id).await?;
    let out = envelope(
        &tx,
        &actor.user_id,
        json!({"data":list_data(&tx,&actor.user_id,&target.native_id).await?}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(out))
}
async fn list_create(
    State(s): State<AppState>,
    OriginalUri(u): OriginalUri,
    h: HeaderMap,
    r: Request,
) -> Result<Json<Value>> {
    list_change(s, u, None, h, r, false).await
}
async fn list_write(
    State(s): State<AppState>,
    OriginalUri(u): OriginalUri,
    Path(id): Path<String>,
    h: HeaderMap,
    r: Request,
) -> Result<Json<Value>> {
    list_change(s, u, Some(id), h, r, false).await
}
async fn list_delete(
    State(s): State<AppState>,
    OriginalUri(u): OriginalUri,
    Path(id): Path<String>,
    h: HeaderMap,
    r: Request,
) -> Result<Json<Value>> {
    list_change(s, u, Some(id), h, r, true).await
}
async fn list_change(
    state: AppState,
    uri: Uri,
    id: Option<String>,
    headers: HeaderMap,
    request: Request,
    delete: bool,
) -> Result<Json<Value>> {
    no_query(&uri)?;
    let id = id.as_deref().map(PublicUuid::parse).transpose()?;
    let p = Command::parse(
        &body(request, &headers, true).await?,
        if delete {
            &["listVersion"]
        } else if id.is_some() {
            &["listVersion", "name", "mangaIds"]
        } else {
            &["name", "mangaIds"]
        },
    )?;
    if id.is_some() && p.list_version.is_none_or(|v| v < 1) {
        return Err(Error::bad_request("Positive listVersion is required"));
    }
    if !delete && p.name.is_none() && p.manga_ids.is_none() {
        return Err(Error::bad_request("No list changes supplied"));
    }
    let name = p
        .name
        .as_deref()
        .map(|s| validation::text(s, 200, true))
        .transpose()?;
    if id.is_none() && name.is_none() {
        return Err(Error::bad_request("List name is required"));
    }
    if p.manga_ids.as_ref().is_some_and(|ids| ids.len() > 100) {
        return Err(Error::bad_request("At most 100 list members are allowed"));
    }
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = actor(&state, &headers, &tx, true).await?;
    p.check(&tx, &actor.user_id).await?;
    let native = if let Some(id) = id {
        let target = target(&tx, ResourceKind::List, &id).await?;
        let version = owned_list(&tx, &actor.user_id, &target.native_id, true).await?;
        if Some(version) != p.list_version {
            return Err(Error::conflict("List changed; reload before editing"));
        }
        target.native_id
    } else {
        let count: i64 = tx
            .query_one(
                "SELECT count(*) FROM platform.library_lists WHERE user_id=$1",
                &[&actor.user_id],
            )
            .await?
            .get(0);
        if count >= 100 {
            return Err(Error::bad_request("An account may have at most 100 lists"));
        }
        let id = validation::new_id();
        tx.execute("INSERT INTO platform.library_lists(id,user_id,name,visibility) VALUES($1,$2,$3,'private')",&[&id,&actor.user_id,&name]).await?;
        id
    };
    let out = if delete {
        let id = public_id(&tx, ResourceKind::List, &native).await?;
        tx.execute(
            "DELETE FROM platform.library_lists WHERE id=$1 AND user_id=$2",
            &[&native, &actor.user_id],
        )
        .await?;
        envelope(&tx, &actor.user_id, json!({"deleted":true,"id":id})).await?
    } else {
        if let Some(ids) = &p.manga_ids {
            let targets = resolve(&tx, ResourceKind::Manga, ids).await?;
            let mut natives = targets.into_iter().map(|t| t.native_id).collect::<Vec<_>>();
            visible_manga(&tx, &natives).await?;
            // A hidden member cannot be returned or supplied by clients; retain
            // it on replacement and never let its existence leak in the DTO.
            let hidden=tx.query(&format!("SELECT lm.manga_id FROM platform.library_list_members lm JOIN platform.catalog_manga m ON m.id=lm.manga_id WHERE lm.list_id=$1 AND NOT ({}) ORDER BY lm.position LIMIT 101",super::catalogue::MANGA_VISIBLE),&[&native]).await?;
            for r in hidden {
                natives.push(r.get(0));
            }
            if natives.len() > 100 {
                return Err(Error::conflict("List cannot accept that many members"));
            }
            tx.execute(
                "DELETE FROM platform.library_list_members WHERE list_id=$1",
                &[&native],
            )
            .await?;
            for (position, manga) in natives.iter().enumerate() {
                tx.execute("INSERT INTO platform.library_list_members(list_id,manga_id,position) VALUES($1,$2,$3)",&[&native,&manga,&(position as i32)]).await?;
            }
        }
        if let Some(version) = p.list_version {
            let next = version
                .checked_add(1)
                .ok_or_else(|| Error::conflict("List version exhausted"))?;
            tx.execute("UPDATE platform.library_lists SET name=COALESCE($2,name),version=$3,updated_at=clock_timestamp() WHERE id=$1 AND user_id=$4",&[&native,&name,&next,&actor.user_id]).await?;
        }
        envelope(
            &tx,
            &actor.user_id,
            json!({"data":list_data(&tx,&actor.user_id,&native).await?}),
        )
        .await?
    };
    tx.commit().await?;
    Ok(Json(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_queries_and_commands_fail_closed() {
        assert!(raw_query(Some("wat=1"), &["limit"]).is_err());
        assert!(raw_query(Some("page=1&page=2"), &["page"]).is_err());
        for raw in ["limit=0", "limit=101", "limit=100&page=102", "page=-1"] {
            assert!(paging(&raw_query(Some(raw), &["page", "limit"]).unwrap()).is_err());
        }
        assert_eq!(
            paging(&raw_query(Some("limit=100&page=101"), &["page", "limit"]).unwrap()).unwrap(),
            (100, 101)
        );
        let good=br#"{"expectedAccountId":"11111111-1111-8111-8111-111111111111","revision":0,"status":"reading","notification":true}"#;
        assert!(Command::parse(good, &["status", "notification"]).is_ok());
        assert!(Command::parse(good, &[]).is_err());
        assert!(Command::parse(br#"{"expectedAccountId":"native","revision":0}"#, &[]).is_err());
        assert!(Command::parse(br#"{"expectedAccountId":"11111111-1111-8111-8111-111111111111","revision":0,"revision":1}"#,&[]).is_err());
    }
}

/// Membership is independent of following. Selecting a private list must not
/// hide its unsaved titles or implicitly save them into the account library.
async fn list_titles(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>> {
    let id = PublicUuid::parse(&id)?;
    body(request, &headers, false).await?;
    let mut db = state.db.client().await?;
    let tx = db
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await?;
    let actor = actor(&state, &headers, &tx, false).await?;
    let target = target(&tx, ResourceKind::List, &id).await?;
    let list_version = owned_list(&tx, &actor.user_id, &target.native_id, false).await?;
    let q = query(&tx, uri.query(), false, false).await?;
    if q.first("list").is_some() {
        return Err(Error::bad_request("The path selects this list"));
    }
    let mut sql = Sql::default();
    let user = sql.bind(actor.user_id.clone());
    let list = sql.bind(target.native_id);
    sql.conditions.push(format!(
        "lm.list_id={list} AND {}",
        super::catalogue::MANGA_VISIBLE
    ));
    manga_filters(&q, &mut sql, "m", Some(&actor))?;
    let sort = match q.first("sort").unwrap_or("position") {
        "position" => "lm.position",
        "followedAt" => "f.followed_at",
        "createdAt" => "m.created_at",
        "updatedAt" => "m.updated_at",
        "title" => "lower(m.title)",
        "year" => "m.year",
        "rating" => "COALESCE(s.rating,0)",
        "views" => "COALESCE(s.views,0)",
        "follows" => "COALESCE(s.follows,0)",
        "lastUploadedChapterAt" => {
            "(SELECT max(c.published_at) FROM platform.catalog_chapters c WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id))"
        }
        "chapters" => {
            "(SELECT count(*) FROM platform.catalog_chapters c WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id))"
        }
        _ => return Err(Error::bad_request("Invalid library sort")),
    };
    let order = q.order(if q.first("sort").is_none_or(|v| v == "position") {
        "asc"
    } else {
        "desc"
    })?;
    let from = format!(
        "platform.library_list_members lm JOIN platform.catalog_manga m ON m.id=lm.manga_id LEFT JOIN platform.library_manga_follows f ON f.manga_id=m.id AND f.user_id={user} LEFT JOIN platform.catalog_manga_counters s ON s.manga_id=m.id"
    );
    let pred = sql.predicate();
    let total: i64 = tx
        .query_one(
            &format!("SELECT count(*) FROM {from} WHERE {pred}"),
            &sql.params(),
        )
        .await?
        .get(0);
    let limit = sql.bind(q.limit);
    let offset = sql.bind((q.page - 1) * q.limit);
    let rows=tx.query(&format!("SELECT platform.catalog_public_manga_json(m.id),lm.position,f.user_id IS NOT NULL,f.reading_status,f.notification,platform.catalog_timestamp(f.followed_at) FROM {from} WHERE {pred} ORDER BY {sort} {order} NULLS LAST,m.id ASC LIMIT {limit} OFFSET {offset}"),&sql.params()).await?;
    let resources = rows
        .iter()
        .map(|r| (ResourceKind::Manga, r.get::<_, Value>(0)))
        .collect::<Vec<_>>();
    let index = AliasIndex::load_in(&tx, &resources).await?;
    let includes = ["cover_art", "author", "artist", "tag"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let data=rows.iter().zip(resources.iter()).map(|(r,(_,v))|Ok(json!({"manga":index.resource(ResourceKind::Manga,v,&includes)?,"position":r.get::<_,i32>(1),"saved":r.get::<_,bool>(2),"status":r.get::<_,Option<String>>(3),"notification":r.get::<_,Option<bool>>(4),"followedAt":r.get::<_,Option<String>>(5)}))).collect::<Result<Vec<_>>>()?;
    let out=envelope(&tx,&actor.user_id,json!({"listId":target.preferred_id,"listVersion":list_version,"total":total,"limit":q.limit,"page":q.page,"data":data})).await?;
    tx.commit().await?;
    Ok(Json(out))
}

async fn continue_read(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>> {
    let id = PublicUuid::parse(&id)?;
    body(request, &headers, false).await?;
    tokio::time::timeout(Duration::from_secs(8), async move {
        use crate::domains::catalog::reader_selector::{
            catalog_identity, deadline, public_account_continue,
        };
        let (q, filters) = super::reader_selector::reader_filters(uri.query().unwrap_or(""))?;
        let mut db = state.db.client().await?;
        let tx = db
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await?;
        deadline(&tx).await?;
        let actor = actor(&state, &headers, &tx, false).await?;
        let identity = catalog_identity(&tx, &state).await?;
        let manga = target(&tx, ResourceKind::Manga, &id).await?;
        visible_manga(&tx, std::slice::from_ref(&manga.native_id)).await?;
        let mut sql = Sql::default();
        super::catalogue::filters(
            &state,
            Some(&tx),
            ResourceKind::Chapter,
            &q,
            &mut sql,
            Some(std::slice::from_ref(&manga.native_id)),
            None,
        )
        .await?;
        let mut data = public_account_continue(&tx, &actor.user_id, &mut sql).await?;
        data["catalogIdentity"] = json!(identity);
        data["mangaId"] = json!(manga.preferred_id);
        data["effectiveFilters"] = json!(filters);
        let value = envelope(&tx, &actor.user_id, json!({"data":data})).await?;
        tx.commit().await?;
        Ok(Json(value))
    })
    .await
    .map_err(|_| Error::unavailable())?
}
