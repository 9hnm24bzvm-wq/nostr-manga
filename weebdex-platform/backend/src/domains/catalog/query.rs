use crate::{
    domains::auth::SessionPrincipal,
    error::{ApiError, ApiResult},
};
use std::collections::BTreeMap;
use tokio_postgres::types::ToSql;
#[derive(Clone, Debug)]
pub struct QueryParams {
    pub values: BTreeMap<String, Vec<String>>,
    pub limit: i64,
    pub page: i64,
}
impl QueryParams {
    pub fn parse(raw: Option<String>) -> ApiResult<Self> {
        Self::parse_with_statuses(raw, &["ongoing", "completed", "hiatus", "cancelled"])
    }
    /// The bounded query transport is shared, but status vocabularies belong
    /// to the endpoint. Report states must not broaden manga discovery states.
    pub fn parse_with_statuses(raw: Option<String>, statuses: &[&str]) -> ApiResult<Self> {
        let raw = raw.unwrap_or_default();
        if raw.len() > 16_384 {
            return Err(ApiError::bad_request("Query is too long"));
        }
        let mut values: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (k, v) in url::form_urlencoded::parse(raw.as_bytes()) {
            let key = k.strip_suffix("[]").unwrap_or(&k).to_owned();
            if key.len() > 64 || v.len() > 1000 {
                return Err(ApiError::bad_request("Invalid query value"));
            }
            let list = values.entry(key).or_default();
            if list.len() >= 100 {
                return Err(ApiError::bad_request("Too many filter values"));
            }
            list.push(v.into_owned());
        }
        for key in [
            "limit",
            "page",
            "sort",
            "order",
            "title",
            "name",
            "yearFrom",
            "yearTo",
            "tmod",
            "txmod",
            "hasChapters",
            "rank",
            "time",
            "draft",
        ] {
            if values.get(key).is_some_and(|v| v.len() > 1) {
                return Err(ApiError::bad_request(format!("{key} must occur once")));
            }
        }
        for key in [
            "lang",
            "langx",
            "tlang",
            "tlangx",
            "availableTranslatedLang",
        ] {
            if let Some(languages) = values.get(key) {
                for language in languages {
                    super::validation::language(language)?;
                }
            }
        }
        for (key, choices) in [
            ("status", statuses),
            (
                "contentRating",
                &["safe", "suggestive", "erotica", "pornographic", "__none__"][..],
            ),
            (
                "demographic",
                &["shounen", "shoujo", "josei", "seinen", "none"][..],
            ),
            (
                "reading_status",
                &[
                    "reading",
                    "plan_to_read",
                    "completed",
                    "on_hold",
                    "re_reading",
                    "dropped",
                ][..],
            ),
        ] {
            if values
                .get(key)
                .is_some_and(|v| v.iter().any(|s| !choices.contains(&s.as_str())))
            {
                return Err(ApiError::bad_request(format!("Invalid {key}")));
            }
        }
        if values
            .get("contentRating")
            .is_some_and(|v| v.iter().any(|s| s == "__none__") && v.len() != 1)
        {
            return Err(ApiError::bad_request(
                "The empty rating sentinel must be used alone",
            ));
        }
        for key in [
            "id",
            "tag",
            "tagx",
            "author",
            "artist",
            "artists",
            "authorOrArtist",
            "manga",
            "uploader",
            "uploaderx",
            "group",
            "groupx",
        ] {
            if let Some(ids) = values.get(key) {
                for id in ids {
                    super::validation::id(id)?;
                }
            }
        }
        let mut out = Self {
            values,
            limit: 20,
            page: 1,
        };
        out.limit = out.integer("limit", 20, 0, 100)?;
        out.page = out.integer("page", 1, 1, 1_000_000)?;
        Ok(out)
    }
    pub fn first(&self, key: &str) -> Option<&str> {
        self.values
            .get(key)
            .and_then(|v| v.first())
            .map(String::as_str)
    }
    pub fn list(&self, key: &str) -> Vec<String> {
        self.values.get(key).cloned().unwrap_or_default()
    }
    pub fn integer(&self, key: &str, default: i64, min: i64, max: i64) -> ApiResult<i64> {
        match self.first(key) {
            None => Ok(default),
            Some(v) => v
                .parse::<i64>()
                .ok()
                .filter(|v| *v >= min && *v <= max)
                .ok_or_else(|| ApiError::bad_request(format!("Invalid {key}"))),
        }
    }
    pub fn order(&self, default: &str) -> ApiResult<&'static str> {
        match self.first("order").unwrap_or(default) {
            "asc" => Ok("ASC"),
            "desc" => Ok("DESC"),
            _ => Err(ApiError::bad_request("Invalid order")),
        }
    }
    pub fn bool(&self, key: &str) -> ApiResult<Option<bool>> {
        match self.first(key) {
            None => Ok(None),
            Some("true" | "1") => Ok(Some(true)),
            Some("false" | "0") => Ok(Some(false)),
            _ => Err(ApiError::bad_request(format!("Invalid {key}"))),
        }
    }
}
pub struct Sql {
    pub conditions: Vec<String>,
    pub parameters: Vec<Box<dyn ToSql + Sync + Send>>,
}
impl Default for Sql {
    fn default() -> Self {
        Self {
            conditions: Vec::new(),
            parameters: Vec::new(),
        }
    }
}
impl Sql {
    pub fn bind<T: ToSql + Sync + Send + 'static>(&mut self, value: T) -> String {
        self.parameters.push(Box::new(value));
        format!("${}", self.parameters.len())
    }
    pub fn params(&self) -> Vec<&(dyn ToSql + Sync)> {
        self.parameters
            .iter()
            .map(|v| &**v as &(dyn ToSql + Sync))
            .collect()
    }
    pub fn predicate(&self) -> String {
        if self.conditions.is_empty() {
            "TRUE".to_owned()
        } else {
            self.conditions.join(" AND ")
        }
    }
    pub fn array(&mut self, column: &str, values: Vec<String>, exclude: bool) {
        if !values.is_empty() {
            let arg = self.bind(values);
            self.conditions.push(format!(
                "{}({column}=ANY({arg}))",
                if exclude { "NOT " } else { "" }
            ));
        }
    }
}
pub fn manga_filters(
    q: &QueryParams,
    sql: &mut Sql,
    alias: &str,
    principal: Option<&SessionPrincipal>,
) -> ApiResult<()> {
    sql.conditions.push(format!("{alias}.protocol_source_visible AND platform.protocol_source_is_visible({alias}.protocol_series_id)"));
    sql.array(&format!("{alias}.id"), q.list("id"), false);
    if q.first("contentRating") == Some("__none__") {
        sql.conditions.push("FALSE".to_owned());
    }
    if let Some(title) = q.first("title").filter(|s| !s.trim().is_empty()) {
        let p = sql.bind(title.trim().to_owned());
        sql.conditions.push(format!("(position(lower({p}) in lower({alias}.title))>0 OR EXISTS(SELECT 1 FROM jsonb_each(COALESCE({alias}.metadata->'alt_titles','{{}}'::jsonb)) alt, jsonb_array_elements_text(alt.value) alt_title WHERE position(lower({p}) in lower(alt_title))>0))"));
    }
    for (key, column) in [("yearFrom", "year"), ("yearTo", "year")] {
        if q.first(key).is_some() {
            let p = sql.bind(q.integer(key, 0, 0, 9999)? as i32);
            sql.conditions.push(format!(
                "{alias}.{column} {} {p}",
                if key == "yearFrom" { ">=" } else { "<=" }
            ));
        }
    }
    if q.first("yearFrom").is_some()
        && q.first("yearTo").is_some()
        && q.integer("yearFrom", 0, 0, 9999)? > q.integer("yearTo", 0, 0, 9999)?
    {
        return Err(ApiError::bad_request("yearFrom exceeds yearTo"));
    }
    for (key, column, exclude) in [
        ("lang", "language", false),
        ("langx", "language", true),
        ("demographic", "demographic", false),
        ("status", "publication_status", false),
        ("contentRating", "content_rating", false),
    ] {
        sql.array(&format!("{alias}.{column}"), q.list(key), exclude);
    }
    if let Some(has) = q.bool("hasChapters")? {
        sql.conditions.push(format!("{}EXISTS(SELECT 1 FROM platform.catalog_chapters hc WHERE hc.manga_id={alias}.id AND hc.deleted_at IS NULL AND NOT hc.is_unavailable AND platform.protocol_source_is_visible(hc.protocol_event_id))",if has{""}else{"NOT "}));
    }
    let langs = q.list("availableTranslatedLang");
    if !langs.is_empty() {
        let p = sql.bind(langs);
        sql.conditions.push(format!("EXISTS(SELECT 1 FROM platform.catalog_chapters lc WHERE lc.manga_id={alias}.id AND lc.deleted_at IS NULL AND NOT lc.is_unavailable AND platform.protocol_source_is_visible(lc.protocol_event_id) AND lc.language=ANY({p}))"));
    }
    for (key, mode, exclude) in [("tag", "tmod", false), ("tagx", "txmod", true)] {
        let tags = q.list(key);
        if !tags.is_empty() {
            let choices = tags.iter().collect::<std::collections::BTreeSet<_>>().len() as i64;
            let p = sql.bind(tags);
            let all = match q.first(mode).unwrap_or(if exclude { "OR" } else { "AND" }) {
                "AND" | "and" => true,
                "OR" | "or" => false,
                _ => return Err(ApiError::bad_request(format!("Invalid {mode}"))),
            };
            let predicate = if all {
                let n = sql.bind(choices);
                format!(
                    "(SELECT count(*) FROM platform.catalog_manga_tags mt WHERE mt.manga_id={alias}.id AND mt.tag_id=ANY({p}))={n}"
                )
            } else {
                format!(
                    "EXISTS(SELECT 1 FROM platform.catalog_manga_tags mt WHERE mt.manga_id={alias}.id AND mt.tag_id=ANY({p}))"
                )
            };
            sql.conditions.push(format!(
                "{}({predicate})",
                if exclude { "NOT " } else { "" }
            ));
        }
    }
    for (key, role) in [
        ("author", "author"),
        ("artist", "artist"),
        ("artists", "artist"),
    ] {
        let ids = q.list(key);
        if !ids.is_empty() {
            let p = sql.bind(ids);
            sql.conditions.push(format!("EXISTS(SELECT 1 FROM platform.catalog_manga_creators mc WHERE mc.manga_id={alias}.id AND mc.role='{role}' AND mc.author_id=ANY({p}))"));
        }
    }
    let both = q.list("authorOrArtist");
    if !both.is_empty() {
        let p = sql.bind(both);
        sql.conditions.push(format!("EXISTS(SELECT 1 FROM platform.catalog_manga_creators mc WHERE mc.manga_id={alias}.id AND mc.author_id=ANY({p}))"));
    }
    let statuses = q.list("reading_status");
    if !statuses.is_empty() {
        let actor = principal.ok_or_else(ApiError::unauthorized)?;
        actor.require_library_read()?;
        let u = sql.bind(actor.user_id.clone());
        let s = sql.bind(statuses);
        sql.conditions.push(format!("EXISTS(SELECT 1 FROM platform.library_manga_follows mf WHERE mf.manga_id={alias}.id AND mf.user_id={u} AND mf.reading_status=ANY({s}))"));
    }
    Ok(())
}
pub fn chapter_filters(q: &QueryParams, sql: &mut Sql, alias: &str) -> ApiResult<()> {
    sql.conditions.push(format!(
        "platform.protocol_source_is_visible({alias}.protocol_event_id)"
    ));
    for (key, column, exclude) in [
        ("tlang", "language", false),
        ("tlangx", "language", true),
        ("uploader", "uploader_id", false),
        ("uploaderx", "uploader_id", true),
    ] {
        sql.array(&format!("{alias}.{column}"), q.list(key), exclude);
    }
    for (key, exclude) in [("group", false), ("groupx", true)] {
        let ids = q.list(key);
        if !ids.is_empty() {
            let p = sql.bind(ids);
            sql.conditions.push(format!("{}EXISTS(SELECT 1 FROM platform.catalog_chapter_groups cg WHERE cg.chapter_id={alias}.id AND cg.group_id=ANY({p}))",if exclude{"NOT "}else{""}));
        }
    }
    let ids = q.list("id");
    if !ids.is_empty() {
        sql.array(&format!("{alias}.id"), ids, false);
    }
    let manga = q.list("manga");
    if !manga.is_empty() {
        sql.array(&format!("{alias}.manga_id"), manga, false);
    }
    Ok(())
}
pub fn window_seconds(q: &QueryParams) -> ApiResult<Option<i64>> {
    match q.first("time").unwrap_or("all") {
        "24h" | "1d" | "daily" => Ok(Some(86400)),
        "7d" | "weekly" => Ok(Some(604800)),
        "30d" | "monthly" => Ok(Some(2592000)),
        "all" | "all-time" => Ok(None),
        _ => Err(ApiError::bad_request("Invalid ranking time window")),
    }
}
