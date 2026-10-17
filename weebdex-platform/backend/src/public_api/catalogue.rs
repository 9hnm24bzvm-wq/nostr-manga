//! Reconstructed public catalogue adapter. SQL values are always parameters;
//! ordering identifiers come only from operation-specific fixed vocabularies.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdResolver, PublicIdTarget,
    PublicUuid, ResourceKind,
    catalogue_dto::AliasIndex,
    dto::{CollectionResponse, EntityResponse, Resource},
    query::{PublicQuery, QuerySchema},
};
use crate::{
    AppState,
    domains::catalog::query::{QueryParams, Sql, chapter_filters, manga_filters},
};
use axum::{
    Json, Router,
    extract::{Path, RawQuery, State},
    routing::get,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
pub(super) const MANGA_VISIBLE: &str = "m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)";
const CHAPTER_VISIBLE: &str = "c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)";
pub const MANGA_INCLUDES: &[&str] = &["author", "artist", "cover_art", "manga", "tag", "creator"];
const CHAPTER_INCLUDES: &[&str] = &["manga", "scanlation_group", "user"];
const MANGA_ORDER: &[&str] = &[
    "title",
    "year",
    "createdAt",
    "updatedAt",
    "latestUploadedChapter",
    "followedCount",
    "relevance",
    "rating",
];
const CHAPTER_ORDER: &[&str] = &[
    "createdAt",
    "updatedAt",
    "publishAt",
    "readableAt",
    "volume",
    "chapter",
];
const MANGA_SCALAR: &[&str] = &[
    "title",
    "authorOrArtist",
    "year",
    "includedTagsMode",
    "excludedTagsMode",
    "createdAtSince",
    "updatedAtSince",
    "hasAvailableChapters",
    "hasUnavailableChapters",
    "group",
];
const MANGA_ARRAY: &[&str] = &[
    "authors[]",
    "artists[]",
    "includedTags[]",
    "excludedTags[]",
    "status[]",
    "originalLanguage[]",
    "excludedOriginalLanguage[]",
    "availableTranslatedLanguage[]",
    "publicationDemographic[]",
    "ids[]",
    "contentRating[]",
];
const CHAPTER_SCALAR: &[&str] = &[
    "title",
    "manga",
    "includeFutureUpdates",
    "includeEmptyPages",
    "includeFuturePublishAt",
    "includeExternalUrl",
    "externalUrl",
    "excludeExternalUrl",
    "includeUnavailable",
    "createdAtSince",
    "updatedAtSince",
    "publishAtSince",
];
const CHAPTER_ARRAY: &[&str] = &[
    "ids[]",
    "groups[]",
    "uploader",
    "includes",
    "volume[]",
    "chapter",
    "chapter[]",
    "translatedLanguage[]",
    "originalLanguage[]",
    "excludedOriginalLanguage[]",
    "contentRating[]",
    "excludedGroups[]",
    "excludedUploaders[]",
];
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ping", get(|| async { "pong" }))
        .route("/manga", get(manga_many))
        .route("/manga/random", get(random))
        .route("/manga/tag", get(tags))
        .route("/manga/{p0}", get(manga_one))
        .route("/manga/{p0}/feed", get(feed))
        .route("/manga/{p0}/aggregate", get(aggregate))
        .route("/chapter", get(chapter_many))
        .route("/chapter/{p0}", get(chapter_one))
        .route("/author", get(author_many))
        .route("/author/{p0}", get(author_one))
        .route("/group", get(group_many))
        .route("/group/{p0}", get(group_one))
        .route("/cover", get(cover_many))
        .route("/cover/{p0}", get(cover_one))
}
pub(super) fn schema(kind: ResourceKind) -> QuerySchema<'static> {
    match kind {
        ResourceKind::Manga => QuerySchema {
            default_limit: 10,
            max_limit: 100,
            max_offset: 10000,
            scalar: MANGA_SCALAR,
            arrays: MANGA_ARRAY,
            includes: MANGA_INCLUDES,
            order: MANGA_ORDER,
        },
        ResourceKind::Chapter => QuerySchema {
            default_limit: 10,
            max_limit: 100,
            max_offset: 10000,
            scalar: CHAPTER_SCALAR,
            arrays: CHAPTER_ARRAY,
            includes: CHAPTER_INCLUDES,
            order: CHAPTER_ORDER,
        },
        ResourceKind::Author => QuerySchema {
            default_limit: 10,
            max_limit: 100,
            max_offset: 10000,
            scalar: &["name"],
            arrays: &["ids[]"],
            includes: &["manga"],
            order: &["name", "createdAt", "updatedAt"],
        },
        ResourceKind::Group => QuerySchema {
            default_limit: 10,
            max_limit: 100,
            max_offset: 10000,
            scalar: &["name", "focusedLanguage"],
            arrays: &["ids[]"],
            includes: &["leader", "member"],
            order: &[
                "name",
                "createdAt",
                "updatedAt",
                "followedCount",
                "relevance",
            ],
        },
        ResourceKind::Cover => QuerySchema {
            default_limit: 10,
            max_limit: 100,
            max_offset: 10000,
            scalar: &[],
            arrays: &["ids[]", "manga[]", "uploaders[]", "locales[]"],
            includes: &["manga", "user"],
            order: &["createdAt", "updatedAt", "volume"],
        },
        _ => QuerySchema {
            default_limit: 100,
            max_limit: 100,
            max_offset: 0,
            scalar: &[],
            arrays: &[],
            includes: &[],
            order: &[],
        },
    }
}
pub(super) fn detail_query(raw: Option<&str>, kind: ResourceKind) -> PublicApiResult<PublicQuery> {
    for (key, _) in url::form_urlencoded::parse(raw.unwrap_or("").as_bytes()) {
        if key != "includes[]" {
            return Err(PublicApiError::bad_request(
                "This operation only accepts includes[]",
            ));
        }
    }
    let mut s = schema(kind);
    s.default_limit = 0;
    s.max_limit = 0;
    s.max_offset = 0;
    s.scalar = &[];
    s.arrays = &[];
    s.order = &[];
    PublicQuery::parse(raw, &s)
}
macro_rules! handlers {
    ($many:ident,$one:ident,$kind:ident) => {
        async fn $many(
            State(state): State<AppState>,
            RawQuery(raw): RawQuery,
        ) -> PublicApiResult<Json<CollectionResponse<Resource>>> {
            collection(state, ResourceKind::$kind, raw, None).await
        }
        async fn $one(
            State(state): State<AppState>,
            Path(id): Path<String>,
            RawQuery(raw): RawQuery,
        ) -> PublicApiResult<Json<EntityResponse<Resource>>> {
            detail(state, ResourceKind::$kind, &id, raw).await
        }
    };
}
handlers!(manga_many, manga_one, Manga);
handlers!(chapter_many, chapter_one, Chapter);
handlers!(author_many, author_one, Author);
handlers!(group_many, group_one, Group);
handlers!(cover_many, cover_one, Cover);
pub(super) async fn resolve(state: &AppState, kind: ResourceKind, public: &str) -> PublicApiResult<String> {
    let id = PublicUuid::parse(public)?;
    let target = PostgresPublicIdResolver::new(state.db.clone())
        .resolve(kind, &id)
        .await?
        .ok_or_else(PublicApiError::not_found)?;
    if target.target != PublicIdTarget::NativeResource {
        return Err(PublicApiError::not_found());
    }
    Ok(target.native_id)
}
async fn detail(
    state: AppState,
    kind: ResourceKind,
    public: &str,
    raw: Option<String>,
) -> PublicApiResult<Json<EntityResponse<Resource>>> {
    let q = detail_query(raw.as_deref(), kind)?;
    let native = resolve(&state, kind, public).await?;
    let mut sql = Sql::default();
    let (alias, table, projection, visibility) = parts(kind)?;
    sql.conditions.push(visibility.into());
    let id = sql.bind(native);
    sql.conditions.push(format!("{alias}.id={id}"));
    let value: Option<Value> = state
        .db
        .client()
        .await?
        .query_opt(
            &format!("SELECT {projection} FROM {table} WHERE {}", sql.predicate()),
            &sql.params(),
        )
        .await?
        .and_then(|r| r.get(0));
    let value = value.ok_or_else(PublicApiError::not_found)?;
    let resolver = PostgresPublicIdResolver::new(state.db.clone());
    let index = AliasIndex::load(&state, &resolver, &[(kind, value.clone())]).await?;
    Ok(Json(EntityResponse::new(index.resource(
        kind,
        &value,
        &q.includes,
    )?)))
}
pub(super) fn parts(
    kind: ResourceKind,
) -> PublicApiResult<(&'static str, &'static str, &'static str, &'static str)> {
    Ok(match kind {
        ResourceKind::Manga => (
            "m",
            "platform.catalog_manga m LEFT JOIN platform.catalog_manga_counters stats ON stats.manga_id=m.id",
            "platform.catalog_manga_json(m.id) || jsonb_build_object('_latest_chapter_id',(SELECT l.id FROM platform.catalog_chapters l WHERE l.manga_id=m.id AND l.deleted_at IS NULL AND platform.protocol_source_is_visible(l.protocol_event_id) ORDER BY l.published_at DESC,l.id LIMIT 1))",
            MANGA_VISIBLE,
        ),
        ResourceKind::Chapter => (
            "c",
            "platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id",
            "platform.catalog_chapter_json(c.id)",
            CHAPTER_VISIBLE,
        ),
        ResourceKind::Cover => (
            "c",
            "platform.catalog_covers c JOIN platform.catalog_manga m ON m.id=c.manga_id",
            "platform.catalog_cover_json(c.id) || jsonb_build_object('_manga_id',c.manga_id,'_manga',platform.catalog_manga_json(c.manga_id))",
            "c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)",
        ),
        ResourceKind::Author => (
            "a",
            "platform.catalog_authors a",
            "platform.catalog_author_json(a.id) || jsonb_build_object('relationships',jsonb_build_object('manga',(SELECT COALESCE(jsonb_agg(platform.catalog_manga_json(am.id) ORDER BY am.id),'[]'::jsonb) FROM platform.catalog_manga am WHERE am.state='published' AND am.deleted_at IS NULL AND am.protocol_source_visible AND platform.protocol_source_is_visible(am.protocol_series_id) AND EXISTS(SELECT 1 FROM platform.catalog_manga_creators ac WHERE ac.author_id=a.id AND ac.manga_id=am.id))))",
            "a.deleted_at IS NULL AND platform.protocol_source_is_visible(a.protocol_event_id)",
        ),
        ResourceKind::Group => (
            "g",
            "platform.catalog_groups g",
            "platform.catalog_group_json(g.id)",
            "g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id)",
        ),
        ResourceKind::User => (
            "u", "platform.users u",
            "jsonb_build_object('id',u.id,'username',u.username,'roles',u.roles,'version',u.version)",
            "NOT u.disabled",
        ),
        ResourceKind::Tag => (
            "t",
            "platform.catalog_tags t",
            "jsonb_build_object('id',t.id,'name',t.name,'group',t.tag_group)",
            "TRUE",
        ),
        _ => return Err(PublicApiError::unsupported()),
    })
}
async fn mapped(
    state: &AppState,
    tx: Option<&tokio_postgres::Transaction<'_>>,
    q: &PublicQuery,
    key: &str,
    kind: ResourceKind,
) -> PublicApiResult<(Vec<String>, usize)> {
    let values = q.list(key);
    if values.is_empty() {
        return Ok((vec![], 0));
    }
    let ids = values
        .iter()
        .map(|v| PublicUuid::parse(v).map_err(Into::into))
        .collect::<PublicApiResult<Vec<_>>>()?;
    let rows = if let Some(tx) = tx {
        PostgresPublicIdResolver::resolve_many_in(tx, kind, &ids).await?
    } else {
        PostgresPublicIdResolver::new(state.db.clone()).resolve_many(kind, &ids).await?
    };
    let missing = ids.iter().filter(|id| !rows.contains_key(*id)).count();
    let native = rows
        .into_values()
        .map(|r| r.native_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok((native, missing))
}
async fn id_filter(
    state: &AppState,
    tx: Option<&tokio_postgres::Transaction<'_>>,
    q: &PublicQuery,
    sql: &mut Sql,
    key: &str,
    kind: ResourceKind,
    column: &str,
    exclude: bool,
) -> PublicApiResult<Vec<String>> {
    let (ids, _) = mapped(state, tx, q, key, kind).await?;
    if !q.list(key).is_empty() && ids.is_empty() && !exclude {
        sql.conditions.push("FALSE".into());
    }
    sql.array(column, ids.clone(), exclude);
    Ok(ids)
}
fn native_query(q: &PublicQuery) -> QueryParams {
    QueryParams {
        values: BTreeMap::new(),
        limit: i64::from(q.limit),
        page: 1,
    }
}
fn values(q: &PublicQuery, from: &str, to: &str, native: &mut QueryParams) {
    let v = q.list(from);
    if !v.is_empty() {
        native.values.insert(to.into(), v);
    }
}
fn validate_choices(values: &[String], choices: &[&str]) -> PublicApiResult<()> {
    if values.iter().any(|v| !choices.contains(&v.as_str())) {
        return Err(PublicApiError::bad_request("Invalid enum filter"));
    }
    Ok(())
}
fn validate_languages(values: &[String]) -> PublicApiResult<()> {
    for value in values {
        crate::domains::catalog::validation::language(value)?;
    }
    Ok(())
}
fn date_filter(q: &PublicQuery, sql: &mut Sql, key: &str, column: &str) -> PublicApiResult<()> {
    if let Some(value) = q.first(key) {
        if !valid_utc_datetime(value) {
            return Err(PublicApiError::bad_request(
                "Expected UTC YYYY-MM-DDTHH:MM:SS",
            ));
        }
        let p = sql.bind(value.to_owned() + "Z");
        sql.conditions
            .push(format!("{column}>={p}::text::timestamptz"));
    }
    Ok(())
}
fn valid_utc_datetime(value: &str) -> bool {
    let b = value.as_bytes();
    if b.len() != 19
        || [4, 7].iter().any(|i| b[*i] != b'-')
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b.iter()
            .enumerate()
            .any(|(i, b)| !matches!(i, 4 | 7 | 10 | 13 | 16) && !b.is_ascii_digit())
    {
        return false;
    }
    let num = |a, b| value[a..b].parse::<u32>().unwrap_or(u32::MAX);
    let y = num(0, 4);
    let m = num(5, 7);
    let d = num(8, 10);
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    y > 0 && d > 0 && d <= days && num(11, 13) < 24 && num(14, 16) < 60 && num(17, 19) < 60
}
pub(super) async fn filters(
    state: &AppState,
    tx: Option<&tokio_postgres::Transaction<'_>>,
    kind: ResourceKind,
    q: &PublicQuery,
    sql: &mut Sql,
    scope: Option<&[String]>,
    account_scope: Option<&str>,
) -> PublicApiResult<()> {
    let (alias, _, _, visible) = parts(kind)?;
    sql.conditions.push(visible.into());
    id_filter(state, tx, q, sql, "ids[]", kind, &format!("{alias}.id"), false).await?;
    match kind {
        ResourceKind::Manga => {
            let mut n = native_query(q);
            for (from, to) in [
                ("title", "title"),
                ("originalLanguage[]", "lang"),
                ("excludedOriginalLanguage[]", "langx"),
                ("availableTranslatedLanguage[]", "availableTranslatedLang"),
                ("status[]", "status"),
                ("publicationDemographic[]", "demographic"),
                ("contentRating[]", "contentRating"),
                ("includedTagsMode", "tmod"),
                ("excludedTagsMode", "txmod"),
            ] {
                values(q, from, to, &mut n);
            }
            for key in ["lang", "langx", "availableTranslatedLang"] {
                validate_languages(&n.list(key))?;
            }
            validate_choices(
                &n.list("status"),
                &["ongoing", "completed", "hiatus", "cancelled"],
            )?;
            validate_choices(
                &n.list("demographic"),
                &["shounen", "shoujo", "josei", "seinen", "none"],
            )?;
            if n.list("demographic").iter().any(|value| value == "none") {
                let demographics = n.values.remove("demographic").unwrap_or_default();
                let p = sql.bind(demographics);
                sql.conditions
                    .push(format!("(m.demographic IS NULL OR m.demographic=ANY({p}))"));
            }
            if n.list("contentRating").is_empty() {
                n.values.insert(
                    "contentRating".into(),
                    vec!["safe".into(), "suggestive".into(), "erotica".into()],
                );
            }
            validate_choices(
                &n.list("contentRating"),
                &["safe", "suggestive", "erotica", "pornographic"],
            )?;
            for (key, to, kind) in [
                ("authors[]", "author", ResourceKind::Author),
                ("artists[]", "artist", ResourceKind::Author),
                ("authorOrArtist", "authorOrArtist", ResourceKind::Author),
            ] {
                let (ids, _) = mapped(state, tx, q, key, kind).await?;
                if !q.list(key).is_empty() && ids.is_empty() {
                    sql.conditions.push("FALSE".into());
                }
                if !ids.is_empty() {
                    n.values.insert(to.into(), ids);
                }
            }
            for (key, to, mode, exclude) in [
                ("includedTags[]", "tag", "includedTagsMode", false),
                ("excludedTags[]", "tagx", "excludedTagsMode", true),
            ] {
                let (ids, missing) = mapped(state, tx, q, key, ResourceKind::Tag).await?;
                let mode = q.first(mode).unwrap_or(if exclude { "OR" } else { "AND" });
                if !matches!(mode, "AND" | "OR") {
                    return Err(PublicApiError::bad_request("Invalid tag mode"));
                }
                if !exclude && missing > 0 && mode == "AND" {
                    sql.conditions.push("FALSE".into());
                } else if !exclude && !q.list(key).is_empty() && ids.is_empty() {
                    sql.conditions.push("FALSE".into());
                }
                if !(exclude && missing > 0 && mode == "AND") && !ids.is_empty() {
                    n.values.insert(to.into(), ids);
                }
            }
            if let Some(year) = q.first("year") {
                if year == "none" {
                    sql.conditions.push("m.year=0".into());
                } else {
                    let year = year
                        .parse::<i32>()
                        .ok()
                        .filter(|y| (1..=9999).contains(y))
                        .ok_or_else(|| PublicApiError::bad_request("Invalid year"))?;
                    let p = sql.bind(year);
                    sql.conditions.push(format!("m.year={p}"));
                }
            }
            for (key, unavailable) in [
                ("hasAvailableChapters", false),
                ("hasUnavailableChapters", true),
            ] {
                if let Some(wanted) = q.boolean(key)? {
                    let test = format!(
                        "EXISTS(SELECT 1 FROM platform.catalog_chapters hc WHERE hc.manga_id=m.id AND hc.deleted_at IS NULL AND platform.protocol_source_is_visible(hc.protocol_event_id) AND hc.is_unavailable={unavailable})"
                    );
                    sql.conditions
                        .push(if wanted { test } else { format!("NOT({test})") });
                }
            }
            if q.first("group").is_some() {
                let (ids, _) = mapped(state, tx, q, "group", ResourceKind::Group).await?;
                if ids.is_empty() {
                    sql.conditions.push("FALSE".into());
                } else {
                    let p = sql.bind(ids);
                    sql.conditions.push(format!("EXISTS(SELECT 1 FROM platform.catalog_chapters gc JOIN platform.catalog_chapter_groups gcm ON gcm.chapter_id=gc.id WHERE gc.manga_id=m.id AND gc.deleted_at IS NULL AND platform.protocol_source_is_visible(gc.protocol_event_id) AND gcm.group_id=ANY({p}))"));
                }
            }
            manga_filters(&n, sql, "m", None)?;
            date_filter(q, sql, "createdAtSince", "m.created_at")?;
            date_filter(q, sql, "updatedAtSince", "m.updated_at")?;
        }
        ResourceKind::Chapter => {
            let mut n = native_query(q);
            values(q, "translatedLanguage[]", "tlang", &mut n);
            validate_languages(&n.list("tlang"))?;
            for (key, to, kind) in [
                ("groups[]", "group", ResourceKind::Group),
                ("excludedGroups[]", "groupx", ResourceKind::Group),
                ("uploader", "uploader", ResourceKind::User),
                ("excludedUploaders[]", "uploaderx", ResourceKind::User),
                ("manga", "manga", ResourceKind::Manga),
            ] {
                let (ids, _) = mapped(state, tx, q, key, kind).await?;
                if !q.list(key).is_empty() && ids.is_empty() && !key.starts_with("excluded") {
                    sql.conditions.push("FALSE".into());
                }
                if !ids.is_empty() {
                    n.values.insert(to.into(), ids);
                }
            }
            chapter_filters(&n, sql, "c")?;
            if let Some(owner) = account_scope {
                let owner = sql.bind(owner.to_owned());
                sql.conditions.push(format!("EXISTS(SELECT 1 FROM platform.library_manga_follows af WHERE af.user_id={owner} AND af.manga_id=c.manga_id)"));
            }
            if let Some(scope) = scope {
                if scope.is_empty() {
                    sql.conditions.push("FALSE".into());
                } else {
                    sql.array("c.manga_id", scope.to_vec(), false);
                }
            }
            for (key, column, exclude) in [
                ("originalLanguage[]", "m.language", false),
                ("excludedOriginalLanguage[]", "m.language", true),
                ("contentRating[]", "m.content_rating", false),
                ("volume[]", "c.volume", false),
            ] {
                let mut list = q.list(key);
                if key == "contentRating[]" {
                    if list.is_empty() {
                        list = vec!["safe".into(), "suggestive".into(), "erotica".into()];
                    }
                    validate_choices(&list, &["safe", "suggestive", "erotica", "pornographic"])?;
                } else if key.contains("Language") {
                    validate_languages(&list)?;
                }
                sql.array(column, list, exclude);
            }
            let mut chapters = q.list("chapter[]");
            chapters.extend(q.list("chapter"));
            sql.array("c.chapter", chapters, false);
            if let Some(title) = q.first("title") {
                let p = sql.bind(title.to_owned());
                sql.conditions.push(format!(
                    "position(lower({p}) in lower(COALESCE(c.title,'')))>0"
                ));
            }
            let flag = |key, default| -> PublicApiResult<bool> {
                match q.first(key) {
                    None => Ok(default),
                    Some("0") => Ok(false),
                    Some("1") => Ok(true),
                    _ => Err(PublicApiError::bad_request("Expected toggle0 or1")),
                }
            };
            if !flag("includeUnavailable", false)? {
                sql.conditions.push("NOT c.is_unavailable".into());
            }
            if !flag("includeEmptyPages", false)? {
                sql.conditions
                    .push("jsonb_array_length(COALESCE(c.metadata->'data','[]'::jsonb))>0".into());
            }
            if !flag("includeFuturePublishAt", true)? {
                sql.conditions
                    .push("c.published_at<=clock_timestamp()".into());
            }
            // This module has no distinct readable-at embargo. Future update scope is the
            // same actual publish clock; preserve the parameter's filtering effect.
            if !flag("includeFutureUpdates", true)? {
                sql.conditions
                    .push("c.published_at<=clock_timestamp()".into());
            }
            if !flag("includeExternalUrl", true)? {
                sql.conditions.push(
                    "COALESCE(c.metadata->>'externalUrl',c.metadata->>'external_url','')=''".into(),
                );
            }
            for (key, exclude) in [("externalUrl", false), ("excludeExternalUrl", true)] {
                if let Some(host) = q.first(key) {
                    if scope.is_none() && account_scope.is_none() && q.first("manga").is_none() {
                        return Err(PublicApiError::bad_request(
                            "External host filter requires manga scope",
                        ));
                    }
                    if !(3..=64).contains(&host.len())
                        || !host
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
                    {
                        return Err(PublicApiError::bad_request("Invalid external hostname"));
                    }
                    let p = sql.bind(host.to_ascii_lowercase());
                    let cmp = format!(
                        "lower(substring(COALESCE(c.metadata->>'externalUrl',c.metadata->>'external_url','') FROM '^https?://([^/:?#]+)'))={p}"
                    );
                    sql.conditions.push(if exclude {
                        format!("NOT COALESCE(({cmp}),FALSE)")
                    } else {
                        cmp
                    });
                }
            }
            for (key, column) in [
                ("createdAtSince", "c.created_at"),
                ("updatedAtSince", "c.updated_at"),
                ("publishAtSince", "c.published_at"),
            ] {
                date_filter(q, sql, key, column)?;
            }
        }
        ResourceKind::Author | ResourceKind::Group => {
            if let Some(name) = q.first("name") {
                let p = sql.bind(name.to_owned());
                sql.conditions
                    .push(format!("position(lower({p}) in lower({alias}.name))>0"));
            }
            if kind == ResourceKind::Group {
                if let Some(language) = q.first("focusedLanguage") {
                    validate_languages(&[language.to_owned()])?;
                    let p = sql.bind(language.to_owned());
                    sql.conditions.push(format!("COALESCE(g.metadata->'focused_languages',g.metadata->'focusedLanguage','[]'::jsonb) ? {p}"));
                }
            }
        }
        ResourceKind::Cover => {
            id_filter(
                state,
                tx,
                q,
                sql,
                "manga[]",
                ResourceKind::Manga,
                "c.manga_id",
                false,
            )
            .await?;
            id_filter(
                state,
                tx,
                q,
                sql,
                "uploaders[]",
                ResourceKind::User,
                "c.uploader_id",
                false,
            )
            .await?;
            validate_languages(&q.list("locales[]"))?;
            sql.array("c.language", q.list("locales[]"), false);
        }
        _ => {}
    }
    Ok(())
}
pub(super) fn order_clause(
    kind: ResourceKind,
    q: &PublicQuery,
    sql: &mut Sql,
) -> PublicApiResult<String> {
    let (alias, _, _, _) = parts(kind)?;
    let mut order = vec![];
    for (field, direction) in &q.order {
        let column=match(kind,field.as_str()){
  (ResourceKind::Manga,"title")=>"lower(m.title)".to_owned(),(ResourceKind::Manga,"year")=>"NULLIF(m.year,0)".into(),
  (ResourceKind::Manga,"latestUploadedChapter")=>"(SELECT max(l.published_at) FROM platform.catalog_chapters l WHERE l.manga_id=m.id AND l.deleted_at IS NULL AND platform.protocol_source_is_visible(l.protocol_event_id))".into(),
  (ResourceKind::Manga,"followedCount")=>"COALESCE(stats.follows,0)".into(),(ResourceKind::Manga,"rating")=>"COALESCE(stats.rating,0)".into(),
  (ResourceKind::Manga,"relevance")=>{let p=sql.bind(q.first("title").unwrap_or("").to_owned());format!("CASE WHEN lower(m.title)=lower({p}) THEN 3 WHEN starts_with(lower(m.title),lower({p})) THEN 2 ELSE 1 END")},
  (_,"createdAt")=>format!("{alias}.created_at"),(_,"updatedAt")=>format!("{alias}.updated_at"),(_,"name")=>format!("lower({alias}.name)"),
  (ResourceKind::User,"username")=>"lower(u.username)".into(),
  (ResourceKind::Chapter,"publishAt"|"readableAt")=>"c.published_at".into(),
  (ResourceKind::Chapter,"volume"|"chapter")|(ResourceKind::Cover,"volume")=>format!("CASE WHEN {alias}.{field} ~ '^[0-9]+([.][0-9]+)?$' THEN {alias}.{field}::numeric END"),
  (ResourceKind::Group,"followedCount")=>"(SELECT count(*) FROM platform.library_group_follows gf JOIN platform.users gu ON gu.id=gf.user_id AND NOT gu.disabled WHERE gf.group_id=g.id)".into(),
  (ResourceKind::Group,"relevance")=>{let p=sql.bind(q.first("name").unwrap_or("").to_owned());format!("CASE WHEN lower(g.name)=lower({p}) THEN 3 WHEN starts_with(lower(g.name),lower({p})) THEN 2 ELSE 1 END")},
  _=>return Err(PublicApiError::bad_request("Unsupported order field")),
 };
        order.push(format!("{column} {} NULLS LAST", direction.sql()));
    }
    if order.is_empty() {
        order.push(match kind{ResourceKind::Manga=>"(SELECT max(l.published_at) FROM platform.catalog_chapters l WHERE l.manga_id=m.id AND l.deleted_at IS NULL AND platform.protocol_source_is_visible(l.protocol_event_id)) DESC NULLS LAST".into(),ResourceKind::Chapter=>"c.published_at DESC".into(),ResourceKind::Tag=>"t.name ASC".into(),_=>format!("{alias}.created_at DESC")});
    }
    order.push(format!("{alias}.id ASC"));
    Ok(order.join(","))
}
async fn collection(
    state: AppState,
    kind: ResourceKind,
    raw: Option<String>,
    scope: Option<Vec<String>>,
) -> PublicApiResult<Json<CollectionResponse<Resource>>> {
    let operation = collection_schema(kind, scope.is_some());
    let q = PublicQuery::parse(raw.as_deref(), &operation)?;
    if scope.is_some() && q.limit == 0 {
        return Err(PublicApiError::bad_request("Feed limit must be 1..500"));
    }
    let mut client = state.db.client().await?;
    let tx = client.build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true).start().await?;
    let mut sql = Sql::default();
    filters(&state, Some(&tx), kind, &q, &mut sql, scope.as_deref(), None).await?;
    let response = collection_in(&tx, kind, &q, &mut sql).await?;
    tx.commit().await?;
    Ok(Json(response))
}
pub(super) fn collection_schema(kind: ResourceKind, scoped: bool) -> QuerySchema<'static> {
    let mut operation = schema(kind);
    if kind == ResourceKind::Chapter && scoped {
        operation.default_limit = 100;
        operation.max_limit = 500;
        operation.scalar = &[
            "includeFutureUpdates",
            "externalUrl",
            "excludeExternalUrl",
            "createdAtSince",
            "updatedAtSince",
            "publishAtSince",
            "includeEmptyPages",
            "includeFuturePublishAt",
            "includeExternalUrl",
            "includeUnavailable",
        ];
        operation.arrays = &[
            "translatedLanguage[]",
            "originalLanguage[]",
            "excludedOriginalLanguage[]",
            "contentRating[]",
            "excludedGroups[]",
            "excludedUploaders[]",
        ];
    }
    operation
}
pub(super) async fn collection_in(
    tx: &tokio_postgres::Transaction<'_>,
    kind: ResourceKind,
    q: &PublicQuery,
    sql: &mut Sql,
) -> PublicApiResult<CollectionResponse<Resource>> {
    let (alias, table, projection, _) = parts(kind)?;
    let order = order_clause(kind, q, sql)?;
    let limit = sql.bind(i64::from(q.limit));
    let offset = sql.bind(i64::from(q.offset));
    let predicate = sql.predicate();
    let projection_query = format!(
        "WITH matched AS MATERIALIZED(SELECT {alias}.id,row_number() OVER(ORDER BY {order}) position FROM {table} WHERE {predicate}),selected AS(SELECT * FROM matched ORDER BY position LIMIT {limit} OFFSET {offset}) SELECT (SELECT count(*) FROM matched),COALESCE((SELECT jsonb_agg((SELECT {projection} FROM {table} WHERE {alias}.id=selected.id) ORDER BY selected.position) FROM selected),'[]'::jsonb)"
    );
    let row = tx.query_one(&projection_query, &sql.params())
        .await?;
    let total: i64 = row.get(0);
    let native: Vec<Value> =
        serde_json::from_value(row.get(1)).map_err(|_| PublicApiError::unavailable())?;
    let resources = native.into_iter().map(|v| (kind, v)).collect::<Vec<_>>();
    let index = AliasIndex::load_in(tx, &resources).await?;
    let data = resources
        .iter()
        .map(|(kind, v)| index.resource(*kind, v, &q.includes))
        .collect::<PublicApiResult<Vec<_>>>()?;
    Ok(CollectionResponse::new(
        data,
        q.limit,
        q.offset,
        total as u64,
    ))
}
async fn feed(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> PublicApiResult<Json<CollectionResponse<Resource>>> {
    let native = resolve(&state, ResourceKind::Manga, &id).await?;
    let exists = state
        .db
        .client()
        .await?
        .query_opt(
            &format!("SELECT id FROM platform.catalog_manga m WHERE m.id=$1 AND {MANGA_VISIBLE}"),
            &[&native],
        )
        .await?;
    if exists.is_none() {
        return Err(PublicApiError::not_found());
    }
    feed_for_manga_ids(state, raw, vec![native]).await
}
pub(super) async fn feed_for_manga_ids(
    state: AppState,
    raw: Option<String>,
    native_ids: Vec<String>,
) -> PublicApiResult<Json<CollectionResponse<Resource>>> {
    if native_ids.len() > 10000 {
        return Err(PublicApiError::bad_request(
            "Manga scope exceeds feed budget",
        ));
    }
    collection(state, ResourceKind::Chapter, raw, Some(native_ids)).await
}
async fn tags(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
) -> PublicApiResult<Json<CollectionResponse<Resource>>> {
    if raw.as_deref().is_some_and(|r| !r.is_empty()) {
        return Err(PublicApiError::bad_request("Tags accepts no query"));
    }
    let rows=state.db.client().await?.query("SELECT jsonb_build_object('id',t.id,'name',t.name,'group',t.tag_group) FROM platform.catalog_tags t ORDER BY t.name,t.id LIMIT 4097",&[]).await?;
    if rows.len() > 4096 {
        return Err(PublicApiError::unavailable());
    }
    let resources = rows
        .into_iter()
        .map(|row| (ResourceKind::Tag, row.get::<_, Value>(0)))
        .collect::<Vec<_>>();
    let resolver = PostgresPublicIdResolver::new(state.db.clone());
    let index = AliasIndex::load(&state, &resolver, &resources).await?;
    let data = resources
        .iter()
        .map(|(kind, v)| index.resource(*kind, v, &BTreeSet::new()))
        .collect::<PublicApiResult<Vec<_>>>()?;
    let total = data.len() as u32;
    Ok(Json(CollectionResponse::new(
        data,
        total,
        0,
        u64::from(total),
    )))
}
async fn random(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
) -> PublicApiResult<Json<EntityResponse<Resource>>> {
    let allowed = [
        "contentRating[]",
        "includedTags[]",
        "includedTagsMode",
        "excludedTags[]",
        "excludedTagsMode",
        "includes[]",
    ];
    let raw_query = PublicQuery::parse_unchecked(raw.as_deref())?;
    raw_query.require_allowed(&allowed)?;
    let mut operation = schema(ResourceKind::Manga);
    operation.default_limit = 0;
    operation.max_limit = 0;
    operation.max_offset = 0;
    operation.scalar = &["includedTagsMode", "excludedTagsMode"];
    operation.arrays = &["contentRating[]", "includedTags[]", "excludedTags[]"];
    operation.order = &[];
    let q = PublicQuery::parse(raw.as_deref(), &operation)?;
    let mut sql = Sql::default();
    filters(&state, None, ResourceKind::Manga, &q, &mut sql, None, None).await?;
    let value:Option<Value>=state.db.client().await?.query_opt(&format!("SELECT platform.catalog_manga_json(m.id) FROM platform.catalog_manga m WHERE {} ORDER BY random() LIMIT 1",sql.predicate()),&sql.params()).await?.and_then(|r|r.get(0));
    let value = value.ok_or_else(PublicApiError::not_found)?;
    let resolver = PostgresPublicIdResolver::new(state.db.clone());
    let index =
        AliasIndex::load(&state, &resolver, &[(ResourceKind::Manga, value.clone())]).await?;
    Ok(Json(EntityResponse::new(index.resource(
        ResourceKind::Manga,
        &value,
        &q.includes,
    )?)))
}
async fn aggregate(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> PublicApiResult<Json<Value>> {
    let raw_query = PublicQuery::parse_unchecked(raw.as_deref())?;
    raw_query.require_allowed(&["includeUnavailable", "translatedLanguage[]", "groups[]"])?;
    let q = PublicQuery::parse(
        raw.as_deref(),
        &QuerySchema {
            default_limit: 0,
            max_limit: 0,
            max_offset: 0,
            scalar: &["includeUnavailable"],
            arrays: &["translatedLanguage[]", "groups[]"],
            includes: &[],
            order: &[],
        },
    )?;
    let native = resolve(&state, ResourceKind::Manga, &id).await?;
    let client = state.db.client().await?;
    if client
        .query_opt(
            &format!("SELECT id FROM platform.catalog_manga m WHERE m.id=$1 AND {MANGA_VISIBLE}"),
            &[&native],
        )
        .await?
        .is_none()
    {
        return Err(PublicApiError::not_found());
    }
    drop(client);
    let mut sql = Sql::default();
    sql.conditions.push(CHAPTER_VISIBLE.into());
    let p = sql.bind(native);
    sql.conditions.push(format!("c.manga_id={p}"));
    validate_languages(&q.list("translatedLanguage[]"))?;
    sql.array("c.language", q.list("translatedLanguage[]"), false);
    if q.bounded_usize("includeUnavailable", 0, 0, 1)? == 0 {
        sql.conditions.push("NOT c.is_unavailable".into());
    }
    if !q.list("groups[]").is_empty() {
        let (ids, _) = mapped(&state, None, &q, "groups[]", ResourceKind::Group).await?;
        if ids.is_empty() {
            sql.conditions.push("FALSE".into());
        } else {
            let p = sql.bind(ids);
            sql.conditions.push(format!("EXISTS(SELECT 1 FROM platform.catalog_chapter_groups cg WHERE cg.chapter_id=c.id AND cg.group_id=ANY({p}))"));
        }
    }
    let rows=state.db.client().await?.query(&format!("SELECT c.id,c.volume,c.chapter,c.is_unavailable FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE {} ORDER BY c.volume NULLS LAST,c.chapter NULLS LAST,c.is_unavailable,c.id LIMIT 10001",sql.predicate()),&sql.params()).await?;
    if rows.len() > 10000 {
        return Err(PublicApiError::unavailable());
    }
    let natives = rows
        .iter()
        .map(|r| r.get::<_, String>(0))
        .collect::<Vec<_>>();
    let resolver = PostgresPublicIdResolver::new(state.db.clone());
    let mut aliases = BTreeMap::new();
    for ids in natives.chunks(super::postgres_identity::MAX_PUBLIC_ID_BATCH) {
        aliases.extend(resolver.preferred_many(ResourceKind::Chapter, ids).await?);
    }
    let mut grouped = BTreeMap::<String, BTreeMap<String, Vec<(PublicUuid, bool)>>>::new();
    for row in rows {
        let native: String = row.get(0);
        let volume: Option<String> = row.get(1);
        let chapter: Option<String> = row.get(2);
        grouped
            .entry(volume.unwrap_or_else(|| "none".into()))
            .or_default()
            .entry(chapter.unwrap_or_else(|| "none".into()))
            .or_default()
            .push((
                aliases
                    .get(&native)
                    .cloned()
                    .ok_or_else(PublicApiError::unavailable)?,
                row.get(3),
            ));
    }
    let volumes = grouped
        .into_iter()
        .map(|(volume, chapters)| {
            let count = chapters.values().map(Vec::len).sum::<usize>();
            let chapters = chapters
                .into_iter()
                .map(|(chapter, mut ids)| {
                    let (id, unavailable) = ids.remove(0);
                    let count = ids.len() + 1;
                    let others = ids.into_iter().map(|(id, _)| id).collect::<Vec<_>>();
                    (
                        chapter.clone(),
                        json!({"chapter":chapter,"id":id,"isUnavailable":unavailable,"others":others,"count":count}),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            (
                volume.clone(),
                json!({"volume":volume,"count":count,"chapters":chapters}),
            )
        })
        .collect::<BTreeMap<_, _>>();
    Ok(Json(json!({"result":"ok","volumes":volumes})))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn read_schemas_cover_pinned_filters_sorts_and_reference_expansions() {
        let inventory: Value = serde_json::from_str(include_str!("../../../contracts/mangadex-5.13.1-inventory.json")).unwrap();
        for (path, kind) in [
            ("/manga", ResourceKind::Manga),
            ("/chapter", ResourceKind::Chapter),
            ("/author", ResourceKind::Author),
            ("/group", ResourceKind::Group),
            ("/cover", ResourceKind::Cover),
        ] {
            let operation = inventory["operations"].as_array().unwrap().iter()
                .find(|operation| operation["method"] == "GET" && operation["path"] == path).unwrap();
            let s = schema(kind);
            for parameter in operation["parameters"].as_array().unwrap() {
                let name = parameter["name"].as_str().unwrap();
                match name {
                    "limit" | "offset" => {},
                    "includes[]" | "includes" => {
                        for include in parameter["schema"]["items"]["enum"].as_array().unwrap() {
                            assert!(s.includes.contains(&include.as_str().unwrap()), "{path} missing include {include}");
                        }
                    },
                    "order" => {
                        for field in parameter["schema"]["properties"].as_array().unwrap() {
                            assert!(s.order.contains(&field.as_str().unwrap()), "{path} missing sort {field}");
                        }
                    },
                    _ => assert!(s.scalar.contains(&name) || s.arrays.contains(&name), "{path} missing filter {name}"),
                }
            }
        }
    }
    #[test]
    fn actual_utc_date_validation_rejects_invalid_calendar_and_offsets() {
        for good in ["2024-02-29T12:00:00", "2025-01-01T00:00:00"] {
            assert!(valid_utc_datetime(good));
        }
        for bad in [
            "2025-02-29T00:00:00",
            "2025-00-01T00:00:00",
            "2025-01-01T24:00:00",
            "2025-01-01T00:00:00Z",
            "bad",
        ] {
            assert!(!valid_utc_datetime(bad));
        }
    }
}
