//! Public catalogue DTOs over authorized native projections and persisted UUID aliases.
//! Never exports native catalogue IDs as public resource identifiers.
use super::{
    PublicApiError, PublicApiResult, PublicIdResolver, PublicIdTarget, PublicUuid,
    ResolvedPublicId, ResourceKind,
    cover_filename::cover_filename,
    dto::{Relationship, Resource, ResourceType},
};
use crate::AppState;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub struct AliasIndex {
    rows: BTreeMap<(ResourceKind, String), ResolvedPublicId>,
    user_attributes: BTreeMap<String, Value>,
    // Trusted SQL comparison keys, deliberately separate from caller/native JSON.
    manga_sort_keys: BTreeMap<String, Value>,
}
fn alias_budget(kind: ResourceKind, count: usize) -> PublicApiResult<()> {
    // A full4096-relation graph needs its owner Manga UUID as well. This is
    // exactly one base-identity allowance, not an unbounded nested alias load.
    let limit = 4096 + usize::from(kind == ResourceKind::Manga);
    if count > limit { Err(PublicApiError::unavailable()) } else { Ok(()) }
}
impl AliasIndex {
    pub async fn load(
        state: &AppState,
        resolver: &dyn PublicIdResolver,
        resources: &[(ResourceKind, Value)],
    ) -> PublicApiResult<Self> {
        let mut keys = BTreeMap::<ResourceKind, BTreeSet<String>>::new();
        for (kind, value) in resources {
            collect(*kind, value, &mut keys, 0)?;
        }
        let mut rows = BTreeMap::new();
        for (kind, ids) in keys {
            let ids = ids.into_iter().collect::<Vec<_>>();
            alias_budget(kind, ids.len())?;
            for batch in ids.chunks(super::postgres_identity::MAX_PUBLIC_ID_BATCH) {
                for (native, row) in resolver.describe_many(kind, batch).await? {
                    rows.insert((kind, native), row);
                }
            }
        }
        let native_users = rows
            .iter()
            .filter_map(|((kind, id), row)| {
                (*kind == ResourceKind::User && row.target == PublicIdTarget::NativeResource)
                    .then(|| id.clone())
            })
            .collect::<Vec<_>>();
        let mut user_attributes = BTreeMap::new();
        if !native_users.is_empty() {
            for row in state.db.client().await?.query("SELECT id,username,roles,version FROM platform.users WHERE id=ANY($1::text[]) AND NOT disabled",&[&native_users]).await?{
                user_attributes.insert(row.get::<_,String>(0),json!({"username":row.get::<_,String>(1),"roles":row.get::<_,Value>(2),"version":row.get::<_,i32>(3)}));
            }
        }
        for ((kind, native), row) in &rows {
            if *kind == ResourceKind::User {
                if let PublicIdTarget::ProtocolPublisher { pubkey } = &row.target {
                    // An explicit principal identifier fallback, not a claimed signed display name.
                    user_attributes.insert(native.clone(),json!({"username":pubkey,"roles":[],"nostr":{"principalType":"protocol_publisher","pubkey":pubkey,"displayNameSource":"public_key"}}));
                }
            }
        }
        let manga_sort_keys = if rows.keys().any(|(kind, _)| *kind == ResourceKind::Manga) {
            let client = state.db.client().await?;
            manga_sort_keys_in(&**client, &rows).await?
        } else {
            BTreeMap::new()
        };
        Ok(Self {
            rows,
            user_attributes,
            manga_sort_keys,
        })
    }
    /// Build protected/private or newly-created responses on the caller's exact
    /// transaction. No second connection can miss an uncommitted alias or let a
    /// DTO failure happen after an editorial write has already committed.
    pub(crate) async fn load_in(
        tx: &tokio_postgres::Transaction<'_>,
        resources: &[(ResourceKind, Value)],
    ) -> PublicApiResult<Self> {
        let mut keys = BTreeMap::<ResourceKind, BTreeSet<String>>::new();
        for (kind, value) in resources {
            collect(*kind, value, &mut keys, 0)?;
        }
        let mut rows = BTreeMap::new();
        for (kind, ids) in keys {
            alias_budget(kind, ids.len())?;
            let ids = ids.into_iter().collect::<Vec<_>>();
            for batch in ids.chunks(super::postgres_identity::MAX_PUBLIC_ID_BATCH) {
                for row in tx.query("SELECT native_id,public_id::text,principal_type FROM platform.public_resource_ids WHERE kind=$1 AND native_id=ANY($2::text[]) AND preferred", &[&kind.as_str(), &batch]).await? {
                    let native: String = row.get(0);
                    let target = super::postgres_identity::persisted_target(kind, &native, row.get(2))?;
                    let preferred_id = PublicUuid::parse(row.get::<_, &str>(1)).map_err(|_| PublicApiError::unavailable())?;
                    rows.insert((kind, native.clone()), ResolvedPublicId { native_id: native, preferred_id, target });
                }
            }
        }
        let native_users = rows
            .iter()
            .filter_map(|((kind, id), row)| {
                (*kind == ResourceKind::User && row.target == PublicIdTarget::NativeResource)
                    .then(|| id.clone())
            })
            .collect::<Vec<_>>();
        let mut user_attributes = BTreeMap::new();
        if !native_users.is_empty() {
            for row in tx.query("SELECT id,username,roles,version FROM platform.users WHERE id=ANY($1::text[]) AND NOT disabled", &[&native_users]).await? {
                user_attributes.insert(row.get::<_, String>(0), json!({"username":row.get::<_, String>(1),"roles":row.get::<_, Value>(2),"version":row.get::<_, i32>(3)}));
            }
        }
        for ((kind, native), row) in &rows {
            if *kind == ResourceKind::User {
                if let PublicIdTarget::ProtocolPublisher { pubkey } = &row.target {
                    user_attributes.insert(native.clone(), json!({"username":pubkey,"roles":[],"nostr":{"principalType":"protocol_publisher","pubkey":pubkey,"displayNameSource":"public_key"}}));
                }
            }
        }
        let manga_sort_keys = manga_sort_keys_in(tx, &rows).await?;
        Ok(Self {
            rows,
            user_attributes,
            manga_sort_keys,
        })
    }
    pub fn id(&self, kind: ResourceKind, native: &str) -> PublicApiResult<PublicUuid> {
        self.rows
            .get(&(kind, native.to_owned()))
            .map(|r| r.preferred_id.clone())
            .ok_or_else(PublicApiError::unavailable)
    }
    fn relationship(
        &self,
        kind: ResourceKind,
        role: ResourceType,
        value: &Value,
        include: bool,
    ) -> PublicApiResult<Option<Relationship>> {
        let Some(native) = value["id"].as_str() else {
            return Ok(None);
        };
        let Some(row) = self.rows.get(&(kind, native.to_owned())) else {
            // Historical attribution with no actual native account is not an invented user.
            if kind == ResourceKind::User {
                return Ok(None);
            }
            return Err(PublicApiError::unavailable());
        };
        let attributes = if !include {
            None
        } else if kind == ResourceKind::User {
            self.user_attributes.get(native).cloned()
        } else {
            Some(self.resource(kind, value, &BTreeSet::new())?.attributes)
        };
        Ok(Some(Relationship {
            id: row.preferred_id.clone(),
            resource_type: role,
            related: None,
            attributes,
        }))
    }
    pub fn resource(
        &self,
        kind: ResourceKind,
        value: &Value,
        includes: &BTreeSet<String>,
    ) -> PublicApiResult<Resource> {
        let native = value["id"]
            .as_str()
            .ok_or_else(PublicApiError::unavailable)?;
        let id = self.id(kind, native)?;
        let mut relationships = Vec::new();
        let attributes = match kind {
            ResourceKind::Manga => {
                let language = value["language"].as_str().unwrap_or("en");
                let mut attributes = json!({"title":{language:value["title"]},"altTitles":alt_titles(&value["alt_titles"]),"description":localized(&value["description"]),
                    "isLocked":value["locked"].as_bool().unwrap_or(false),"originalLanguage":language,
                    "lastVolume":nullable(value,&["lastVolume","last_volume"]),"lastChapter":nullable(value,&["lastChapter","last_chapter"]),
                    "publicationDemographic":demographic(value),"status":value["status"],"year":if value["year"].as_i64().is_some_and(|y|y>0){value["year"].clone()}else{Value::Null},
                    "contentRating":value["content_rating"],"state":value["state"],"availableTranslatedLanguages":value["relationships"]["available_languages"].as_array().cloned().unwrap_or_default(),
                    "tags":array(&value["relationships"]["tags"]).iter().map(|tag|self.resource(ResourceKind::Tag,tag,&BTreeSet::new())).collect::<PublicApiResult<Vec<_>>>()?});
                for (native, wire) in [("localized_titles", "title"), ("localized_description", "description"), ("localized_alt_titles", "altTitles")] {
                    if let Some(localized) = value.get(native) { attributes[wire] = localized.clone(); }
                }
                if value.get("protocol_event_id").is_some() || value.get("publication_scope").is_some() {
                    attributes["nostr"] = json!({"eventId":value["protocol_event_id"],"publisher":value["protocol_publisher"],"publicationScope":value["publication_scope"],"publicationStatus":value["publication_status"]});
                    if value.get("instance_overlay").is_some() { attributes["nostr"]["metadataScope"] = json!("instance-local"); }
                }
                if value["_public_title_groups"] == json!(true) {
                    let nostr = attributes.as_object_mut().ok_or_else(PublicApiError::unavailable)?
                        .entry("nostr").or_insert_with(|| json!({}))
                        .as_object_mut().ok_or_else(PublicApiError::unavailable)?;
                    nostr.insert("availableGroupSemantics".into(), json!("visible-title-groups-v1"));
                }
                common(value, &mut attributes);
                if let Some(keys) = self.manga_sort_keys.get(native) {
                    let mut keys = keys.clone();
                    keys["tieBreak"] = json!(id);
                    // Sort presentation is additive to independently generated
                    // source/publication provenance, never its replacement.
                    let nostr = attributes.as_object_mut().ok_or_else(PublicApiError::unavailable)?
                        .entry("nostr").or_insert_with(|| json!({}))
                        .as_object_mut().ok_or_else(PublicApiError::unavailable)?;
                    nostr.insert("sortKeySemantics".into(), json!("postgresql-exact-v1"));
                    nostr.insert("sortKeys".into(), keys);
                }
                if let Some(links) = value["relationships"]["links"].as_object() {
                    attributes["links"] = json!(
                        links
                            .iter()
                            .filter(|(_, v)| v.is_string())
                            .collect::<BTreeMap<_, _>>()
                    );
                }
                if let Some(links) = value.get("official_links") {
                    attributes["officialLinks"] = super::title_links::dictionary(links).map_err(|_| PublicApiError::unavailable())?;
                }
                if let Some(flag) = value["chapterNumbersResetOnNewVolume"]
                    .as_bool()
                    .or_else(|| value["chapter_numbers_reset_on_new_volume"].as_bool())
                {
                    attributes["chapterNumbersResetOnNewVolume"] = json!(flag);
                }
                if let Some(chapter) = value["_latest_chapter_id"].as_str() {
                    attributes["latestUploadedChapter"] =
                        json!(self.id(ResourceKind::Chapter, chapter)?);
                }
                for (field, kind, role, include) in [
                    (
                        "authors",
                        ResourceKind::Author,
                        ResourceType::Author,
                        "author",
                    ),
                    (
                        "artists",
                        ResourceKind::Author,
                        ResourceType::Artist,
                        "artist",
                    ),
                ] {
                    for related in array(&value["relationships"][field]) {
                        if let Some(r) =
                            self.relationship(kind, role, &related, includes.contains(include))?
                        {
                            relationships.push(r);
                        }
                    }
                }
                if let Some(r) = self.relationship(
                    ResourceKind::Cover,
                    ResourceType::CoverArt,
                    &value["relationships"]["cover"],
                    includes.contains("cover_art"),
                )? {
                    relationships.push(r);
                }
                if let Some(r) = self.relationship(
                    ResourceKind::User,
                    ResourceType::Creator,
                    &value["relationships"]["submitter"],
                    includes.contains("creator"),
                )? {
                    relationships.push(r);
                }
                // Tags remain fully projected in attributes.tags. Reference expansion
                // also exposes their canonical IDs through the relationship seam.
                for tag in array(&value["relationships"]["tags"]) {
                    if let Some(r) = self.relationship(
                        ResourceKind::Tag,
                        ResourceType::Tag,
                        &tag,
                        includes.contains("tag"),
                    )? {
                        relationships.push(r);
                    }
                }
                for related in array(&value["relationships"]["relations"]) {
                    if let Some(mut r) = self.relationship(
                        ResourceKind::Manga,
                        ResourceType::Manga,
                        &related,
                        includes.contains("manga"),
                    )? {
                        r.related = related["type"].as_str().map(str::to_owned);
                        relationships.push(r);
                    }
                }
                attributes
            }
            ResourceKind::Chapter => {
                let pages = array(&value["data"]);
                let mut a = json!({"title":nullable(value,&["title"]),"volume":nullable(value,&["volume"]),"chapter":nullable(value,&["chapter"]),
                    "pages":pages.len(),"translatedLanguage":value["language"],"externalUrl":nullable(value,&["externalUrl","external_url"]),"isUnavailable":value["is_unavailable"].as_bool().unwrap_or(false)});
                common(value, &mut a);
                // Read-only stored provenance, overwritten after the metadata merge by
                // catalogue SELECT. This is not actor permission or write authority.
                if let Some(local) = value["_local_page_source_mutable"].as_bool() {
                    a["localPageSource"] = json!(if local { "local" } else { "protocol" });
                }
                // Native compatibility fact, not account-dependent permission. The
                // native DTO already exposes this stored attribution name; current
                // users.username must never stand in for the claim rule's value.
                if let Some(name) = value["relationships"]["uploader"]["name"].as_str() {
                    a["localUploaderIsSystem"] = json!(name == "system");
                }

                if let Some(published) = value.get("published_at") {
                    a["publishAt"] = published.clone();
                    a["readableAt"] = published.clone();
                }
                let dims: Option<Vec<Value>> = pages
                    .iter()
                    .map(|p| {
                        p.get("dimensions")
                            .filter(|v| {
                                v.as_array().is_some_and(|a| {
                                    a.len() == 2
                                        && a.iter().all(|n| n.as_u64().is_some_and(|n| n > 0))
                                })
                            })
                            .cloned()
                    })
                    .collect();
                if let Some(dims) = dims {
                    if !dims.is_empty() {
                        a["pageDimensions"] = json!(dims);
                    }
                }
                if let Some(r) = self.relationship(
                    ResourceKind::Manga,
                    ResourceType::Manga,
                    &value["relationships"]["manga"],
                    includes.contains("manga"),
                )? {
                    relationships.push(r);
                }
                for group in array(&value["relationships"]["groups"]) {
                    if let Some(r) = self.relationship(
                        ResourceKind::Group,
                        ResourceType::ScanlationGroup,
                        &group,
                        includes.contains("scanlation_group"),
                    )? {
                        relationships.push(r);
                    }
                }
                if let Some(r) = self.relationship(
                    ResourceKind::User,
                    ResourceType::User,
                    &value["relationships"]["uploader"],
                    includes.contains("user"),
                )? {
                    a["uploader"] = json!(r.id);
                    relationships.push(r);
                }
                a
            }
            ResourceKind::Cover => {
                let mut a = json!({"volume":nullable_nonempty(value,&["volume"]),"fileName":cover_filename(native,value["ext"].as_str().ok_or_else(PublicApiError::unavailable)?)?,
                    "description":nullable(value,&["description"]),"locale":nullable_nonempty(value,&["language"])});
                common(value, &mut a);
                if let Some(dimensions) = value.get("dimensions") {
                    a["dimensions"] = dimensions.clone();
                }
                if let Some(manga) = value["_manga_id"].as_str() {
                    relationships.push(Relationship {
                        id: self.id(ResourceKind::Manga, manga)?,
                        resource_type: ResourceType::Manga,
                        related: None,
                        attributes: if includes.contains("manga") {
                            value
                                .get("_manga")
                                .filter(|v| v.is_object())
                                .map(|v| {
                                    self.resource(ResourceKind::Manga, v, &BTreeSet::new())
                                        .map(|r| r.attributes)
                                })
                                .transpose()?
                        } else {
                            None
                        },
                    });
                }
                if let Some(r) = self.relationship(
                    ResourceKind::User,
                    ResourceType::User,
                    &value["relationships"]["uploader"],
                    includes.contains("user"),
                )? {
                    relationships.push(r);
                }
                a
            }
            ResourceKind::Author => {
                let mut a = json!({"name":value["name"],"locked":value["locked"].as_bool().unwrap_or(false),"biography":localized(value.get("biography").unwrap_or(&value["description"])),"imageUrl":nullable(value,&["imageUrl","image_url"])});
                common(value, &mut a);
                for (wire, native) in [
                    ("twitter", "twitter"),
                    ("pixiv", "pixiv"),
                    ("melonBook", "melonBook"),
                    ("fanBox", "fanbox"),
                    ("booth", "booth"),
                    ("nicoVideo", "nicoVideo"),
                    ("skeb", "skeb"),
                    ("fantia", "fantia"),
                    ("tumblr", "tumblr"),
                    ("youtube", "youtube"),
                    ("weibo", "weibo"),
                    ("naver", "naver"),
                    ("namicomi", "namicomi"),
                    ("website", "website"),
                ] {
                    a[wire] = nullable(value, &[wire, native]);
                }
                for related in array(&value["relationships"]["manga"]) {
                    if let Some(r) = self.relationship(
                        ResourceKind::Manga,
                        ResourceType::Manga,
                        &related,
                        includes.contains("manga"),
                    )? {
                        relationships.push(r);
                    }
                }
                a
            }
            ResourceKind::Group => {
                let mut a = json!({"name":value["name"],"altNames":alt_titles(&value["alt_names"]),"locked":value["locked"],"inactive":value["inactive"],"localMembershipRoles":"leader-officer-member-v1"});
                common(value, &mut a);
                for (wire, native) in [
                    ("website", "website"),
                    ("ircServer", "ircServer"),
                    ("ircChannel", "ircChannel"),
                    ("discord", "discord"),
                    ("contactEmail", "contact_email"),
                    ("description", "description"),
                    ("twitter", "twitter"),
                    ("mangaUpdates", "mangaupdates"),
                    ("focusedLanguage", "focused_languages"),
                    ("publishDelay", "publishDelay"),
                ] {
                    a[wire] = nullable(value, &[wire, native]);
                }
                for flag in ["official", "verified", "exLicensed"] {
                    if let Some(value) = value.get(flag).filter(|v| v.is_boolean()) {
                        a[flag] = value.clone();
                    }
                }
                for member in array(&value["relationships"]["members"]) {
                    let role = if member["is_leader"].as_bool().unwrap_or(false) {
                        "leader"
                    } else {
                        "member"
                    };
                    if let Some(mut r) = self.relationship(
                        ResourceKind::User,
                        if role == "leader" {
                            ResourceType::Leader
                        } else {
                            ResourceType::Member
                        },
                        &member,
                        includes.contains(role),
                    )? {
                        // This is a local membership edge flag, not a native
                        // account role or original protocol signing authority.
                        r.attributes.get_or_insert_with(|| json!({}))["isOfficer"] = json!(member["is_officer"].as_bool().unwrap_or(false));
                        relationships.push(r);
                    }
                }
                a
            }
            ResourceKind::Tag => {
                let mut a = json!({"name":localized_name(&value["name"]),"description":localized(&value["description"])});
                if value["group"]
                    .as_str()
                    .is_some_and(|g| ["content", "format", "genre", "theme"].contains(&g))
                {
                    a["group"] = value["group"].clone();
                } else if value["group"].is_string() {
                    a["nostr"] = json!({"nativeTagGroup":value["group"]});
                }
                if let Some(version) = value.get("version") {
                    a["version"] = version.clone();
                }
                a
            }
            ResourceKind::User => {
                json!({"username":value["username"],"roles":value["roles"],"version":value["version"]})
            }
            _ => return Err(PublicApiError::unsupported()),
        };
        let wire = match kind {
            ResourceKind::Manga => ResourceType::Manga,
            ResourceKind::Chapter => ResourceType::Chapter,
            ResourceKind::Cover => ResourceType::CoverArt,
            ResourceKind::Author => ResourceType::Author,
            ResourceKind::Group => ResourceType::ScanlationGroup,
            ResourceKind::Tag => ResourceType::Tag,
            ResourceKind::User => ResourceType::User,
            _ => return Err(PublicApiError::unsupported()),
        };
        Ok(Resource {
            id,
            resource_type: wire,
            attributes,
            relationships,
        })
    }
}
fn collect(
    kind: ResourceKind,
    value: &Value,
    keys: &mut BTreeMap<ResourceKind, BTreeSet<String>>,
    depth: usize,
) -> PublicApiResult<()> {
    if value.is_null() {
        return Ok(());
    }
    if depth > 4 {
        return Err(PublicApiError::unavailable());
    }
    if let Some(id) = value["id"].as_str() {
        keys.entry(kind).or_default().insert(id.to_owned());
    } else {
        return Ok(());
    }
    let rel = &value["relationships"];
    let entries: &[(&str, ResourceKind)] = match kind {
        ResourceKind::Manga => &[
            ("authors", ResourceKind::Author),
            ("artists", ResourceKind::Author),
            ("cover", ResourceKind::Cover),
            ("tags", ResourceKind::Tag),
            ("relations", ResourceKind::Manga),
            ("submitter", ResourceKind::User),
        ],
        ResourceKind::Chapter => &[
            ("manga", ResourceKind::Manga),
            ("groups", ResourceKind::Group),
            ("uploader", ResourceKind::User),
        ],
        ResourceKind::Cover => &[("uploader", ResourceKind::User)],
        ResourceKind::Author => &[("manga", ResourceKind::Manga)],
        ResourceKind::Group => &[("members", ResourceKind::User)],
        _ => &[],
    };
    for (field, kind) in entries {
        if let Some(a) = rel[*field].as_array() {
            for entry in a {
                collect(*kind, entry, keys, depth + 1)?;
            }
        } else {
            collect(*kind, &rel[*field], keys, depth + 1)?;
        }
    }
    if kind == ResourceKind::Cover && value["_manga"].is_object() {
        collect(ResourceKind::Manga, &value["_manga"], keys, depth + 1)?;
    }
    if let Some(manga) = value["_manga_id"].as_str() {
        keys.entry(ResourceKind::Manga)
            .or_default()
            .insert(manga.to_owned());
    }
    if let Some(chapter) = value["_latest_chapter_id"].as_str() {
        keys.entry(ResourceKind::Chapter)
            .or_default()
            .insert(chapter.to_owned());
    }
    Ok(())
}
fn array(value: &Value) -> Vec<Value> {
    value.as_array().cloned().unwrap_or_default()
}
fn nullable(value: &Value, keys: &[&str]) -> Value {
    keys.iter()
        .find_map(|k| value.get(*k))
        .cloned()
        .unwrap_or(Value::Null)
}
fn demographic(value: &Value) -> Value {
    let result = nullable_nonempty(value, &["demographic"]);
    if result.as_str() == Some("none") {
        Value::Null
    } else {
        result
    }
}
fn nullable_nonempty(value: &Value, keys: &[&str]) -> Value {
    let v = nullable(value, keys);
    if v.as_str() == Some("") {
        Value::Null
    } else {
        v
    }
}
fn localized(value: &Value) -> Value {
    if value.is_object() {
        value.clone()
    } else if let Some(s) = value.as_str() {
        if s.is_empty() {
            json!({})
        } else {
            json!({"en":s})
        }
    } else {
        json!({})
    }
}
fn localized_name(value: &Value) -> Value {
    if value.is_object() {
        value.clone()
    } else {
        json!({"en":value})
    }
}
fn alt_titles(value: &Value) -> Vec<Value> {
    if let Some(a) = value.as_array() {
        return a.iter().filter(|v| v.is_object()).cloned().collect();
    }
    let mut out = Vec::new();
    if let Some(map) = value.as_object() {
        for (language, values) in map {
            if let Some(names) = values.as_array() {
                for name in names {
                    if name.is_string() {
                        out.push(json!({language:name}));
                    }
                }
            } else if values.is_string() {
                out.push(json!({language:values}));
            }
        }
    }
    out
}
/// One bounded query per alias batch, using the same transaction as collection
/// selection. These are the actual SQL ordering expressions, not guessed stats
/// or metadata supplied by a publisher. Decimal text avoids JSON/JS rounding.
async fn manga_sort_keys_in<C: tokio_postgres::GenericClient + Sync>(
    client: &C,
    aliases: &BTreeMap<(ResourceKind, String), ResolvedPublicId>,
) -> PublicApiResult<BTreeMap<String, Value>> {
    let ids = aliases.keys().filter_map(|(kind, id)|
        (*kind == ResourceKind::Manga).then(|| id.clone())
    ).collect::<Vec<_>>();
    let mut keys = BTreeMap::new();
    let query = format!(r#"
        SELECT m.id,jsonb_build_object(
            'title',encode(convert_to(lower(m.title),'UTF8'),'hex'),
            'year',NULLIF(m.year,0)::text,
            'createdAt',to_char(m.created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
            'updatedAt',to_char(m.updated_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
            'latestUploadedChapter',to_char((SELECT max(l.published_at) FROM platform.catalog_chapters l WHERE l.manga_id=m.id AND l.deleted_at IS NULL AND platform.protocol_source_is_visible(l.protocol_event_id)) AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
            'followedCount',COALESCE(stats.follows,0)::text,
            'rating',COALESCE(stats.rating,0)::text
        ) FROM platform.catalog_manga m
        LEFT JOIN platform.catalog_manga_counters stats ON stats.manga_id=m.id
        WHERE m.id=ANY($1::text[]) AND {}
    "#, super::catalogue::MANGA_VISIBLE);
    for batch in ids.chunks(super::postgres_identity::MAX_PUBLIC_ID_BATCH) {
        for row in client.query(&query, &[&batch]).await? {
            let value: Value = row.get(1);
            if !valid_sort_keys(&value) {
                return Err(PublicApiError::unavailable());
            }
            keys.insert(row.get::<_, String>(0), value);
        }
    }
    Ok(keys)
}

fn exact_decimal(value: &str) -> bool {
    if value.is_empty() || value.len() > 256 { return false; }
    let value = value.strip_prefix('-').unwrap_or(value);
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or("");
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    digits(whole) && parts.next().is_none_or(digits) && parts.next().is_none()
}

fn exact_timestamp(value: &str) -> bool {
    value.len() == 27 && value.bytes().enumerate().all(|(i, b)| match i {
        4 | 7 => b == b'-', 10 => b == b'T', 13 | 16 => b == b':',
        19 => b == b'.', 26 => b == b'Z', _ => b.is_ascii_digit(),
    })
}

fn valid_sort_keys(keys: &Value) -> bool {
    let decimal = |field: &str| keys[field].as_str().is_some_and(exact_decimal);
    let timestamp = |field: &str| keys[field].as_str().is_some_and(exact_timestamp);
    decimal("rating") && decimal("followedCount")
        && (keys["year"].is_null() || decimal("year"))
        && timestamp("createdAt") && timestamp("updatedAt")
        && (keys["latestUploadedChapter"].is_null() || timestamp("latestUploadedChapter"))
        && keys["title"].as_str().is_some_and(|v| !v.is_empty() && v.len() <= 8000
            && v.len() % 2 == 0 && v.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
}

fn common(native: &Value, attributes: &mut Value) {
    for (wire, field) in [
        ("version", "version"),
        ("createdAt", "created_at"),
        ("updatedAt", "updated_at"),
    ] {
        if let Some(value) = native.get(field) {
            attributes[wire] = value.clone();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_backed_manga_provenance_survives_exact_sort_key_projection() {
        let native = "SourceBackedManga";
        let id = PublicUuid::parse("11111111-1111-8111-8111-111111111111").unwrap();
        let event = "a".repeat(64); let publisher = "b".repeat(64);
        let keys = json!({"title":"736f75726365","year":"2026","createdAt":"2026-10-17T00:00:00.000001Z","updatedAt":"2026-10-17T00:00:00.000002Z","latestUploadedChapter":null,"followedCount":"9007199254740993","rating":"7.50000000000000000001"});
        let index = AliasIndex {
            rows:BTreeMap::from([((ResourceKind::Manga,native.into()),ResolvedPublicId{native_id:native.into(),preferred_id:id.clone(),target:PublicIdTarget::NativeResource})]),
            user_attributes:BTreeMap::new(),manga_sort_keys:BTreeMap::from([(native.into(),keys.clone())]),
        };
        let mut source = json!({"id":native,"title":"Source","language":"en","state":"published","version":4,"protocol_event_id":event,"protocol_publisher":publisher,"publication_scope":"original-protocol","publication_status":"published","instance_overlay":true,"nostr":{"eventId":"untrusted-metadata","sortKeys":{"rating":"999"}}});
        let resource = index.resource(ResourceKind::Manga,&source,&BTreeSet::new()).unwrap();
        let nostr = &resource.attributes["nostr"];
        assert_eq!(nostr["eventId"],event);assert_eq!(nostr["publisher"],publisher);
        assert_eq!(nostr["publicationScope"],"original-protocol");assert_eq!(nostr["publicationStatus"],"published");assert_eq!(nostr["metadataScope"],"instance-local");
        assert_eq!(nostr["sortKeySemantics"],"postgresql-exact-v1");assert_eq!(nostr["sortKeys"]["rating"],keys["rating"]);assert_eq!(nostr["sortKeys"]["tieBreak"],json!(id));
        for field in ["protocol_event_id","protocol_publisher","publication_scope","publication_status","instance_overlay"] { source.as_object_mut().unwrap().remove(field); }
        let local = index.resource(ResourceKind::Manga,&source,&BTreeSet::new()).unwrap();
        assert!(local.attributes["nostr"].get("eventId").is_none(),"native metadata cannot invent source provenance");
        assert_eq!(local.attributes["nostr"]["sortKeys"]["rating"],keys["rating"]);
    }
    #[test]
    fn manga_alias_budget_includes_one_owner_and_still_rejects_overflow() {
        assert!(alias_budget(ResourceKind::Manga,4097).is_ok());
        assert!(alias_budget(ResourceKind::Manga,4098).is_err());
        for kind in [ResourceKind::Chapter,ResourceKind::Cover,ResourceKind::Author,ResourceKind::Group,ResourceKind::Tag,ResourceKind::User] {
            assert!(alias_budget(kind,4096).is_ok());assert!(alias_budget(kind,4097).is_err());
        }
    }
    #[test]
    fn exact_sort_key_scalars_are_bounded_and_do_not_lose_precision() {
        for value in ["0", "-0.00", "9007199254740993", "7.50000000000000000000001", "-12.34"] {
            assert!(exact_decimal(value));
        }
        for value in ["", "-", ".1", "1.", "1.2.3", "+1", "1e2", "NaN", "Infinity", " 1"] {
            assert!(!exact_decimal(value), "{value}");
        }
        assert!(!exact_decimal(&"1".repeat(257)));
        assert!(exact_timestamp("2026-10-17T12:34:56.000001Z"));
        for value in ["2026-10-17T12:34:56Z", "2026-10-17T12:34:56.001Z", "2026-10-17T12:34:56.000001+00:00", "infinity"] {
            assert!(!exact_timestamp(value));
        }
    }
    #[test]
    fn native_metadata_cannot_supply_trusted_sort_keys() {
        let id = PublicUuid::parse("11111111-1111-8111-8111-111111111111").unwrap();
        let index = AliasIndex {
            rows: BTreeMap::from([((ResourceKind::Manga, "Manga00001".into()), ResolvedPublicId {
                native_id: "Manga00001".into(), preferred_id: id.clone(), target: PublicIdTarget::NativeResource,
            })]),
            user_attributes: BTreeMap::new(), manga_sort_keys: BTreeMap::new(),
        };
        let value = index.resource(ResourceKind::Manga, &json!({
            "id":"Manga00001", "title":"Real", "nostr":{"sortKeys":{"rating":"999"}},
            "_sort_keys":{"rating":"999"}, "relationships":{}
        }), &BTreeSet::new()).unwrap();
        assert!(value.attributes.get("nostr").is_none());
    }
    #[test]
    fn chapter_stored_system_uploader_fact_is_exact_and_not_current_username() {
        let mut rows=BTreeMap::new();
        for (kind,native,id) in [(ResourceKind::Chapter,"Chapter","11111111-1111-8111-8111-111111111111"),
            (ResourceKind::User,"Uploader","22222222-2222-8222-8222-222222222222")] {
            rows.insert((kind,native.into()),ResolvedPublicId{native_id:native.into(),preferred_id:PublicUuid::parse(id).unwrap(),target:PublicIdTarget::NativeResource});
        }
        let mut index=AliasIndex{rows,user_attributes:BTreeMap::new(),manga_sort_keys:BTreeMap::new()};
        for username in ["system","Renamed current user"] {
            index.user_attributes.insert("Uploader".into(),json!({"username":username,"version":4}));
            for (stored,expected) in [(json!("system"),Some(true)),(json!("System"),Some(false)),(json!(" system"),Some(false)),(json!("Actual uploader"),Some(false)),(json!(""),Some(false)),(Value::Null,None),(json!(42),None),(json!({"name":"system"}),None)] {
                let native=json!({"id":"Chapter","version":7,"localUploaderIsSystem":false,"relationships":{"uploader":{"id":"Uploader","name":stored}}});
                for includes in [BTreeSet::new(),BTreeSet::from(["user".to_owned()])] {
                    let value=index.resource(ResourceKind::Chapter,&native,&includes).unwrap();
                    assert_eq!(value.attributes.get("localUploaderIsSystem").and_then(Value::as_bool),expected);
                }
            }
        }
        let value=index.resource(ResourceKind::Chapter,&json!({"id":"Chapter","version":7,"localUploaderIsSystem":true}),&BTreeSet::new()).unwrap();
        assert!(value.attributes.get("localUploaderIsSystem").is_none());
    }
    #[test]
    fn local_group_officer_edges_and_author_lock_are_explicit_not_source_authority() {
        let gid=PublicUuid::parse("11111111-1111-8111-8111-111111111111").unwrap();
        let uid=PublicUuid::parse("22222222-2222-8222-8222-222222222222").unwrap();
        let aid=PublicUuid::parse("33333333-3333-8333-3333-333333333333").unwrap();
        let mut rows=BTreeMap::new();
        for (kind,native,id) in [(ResourceKind::Group,"Group",gid),(ResourceKind::User,"Member",uid),(ResourceKind::Author,"Author",aid)] {
            rows.insert((kind,native.into()),ResolvedPublicId{native_id:native.into(),preferred_id:id,target:PublicIdTarget::NativeResource});
        }
        let index=AliasIndex{rows,user_attributes:BTreeMap::from([("Member".into(),json!({"username":"Actual user","version":3}))]),manga_sort_keys:BTreeMap::new()};
        let group=json!({"id":"Group","name":"Group","relationships":{"members":[{"id":"Member","is_leader":false,"is_officer":true}]}});
        for includes in [BTreeSet::new(),BTreeSet::from(["member".to_owned()])] {
            let result=index.resource(ResourceKind::Group,&group,&includes).unwrap();
            assert_eq!(result.attributes["localMembershipRoles"],"leader-officer-member-v1");
            let member=&result.relationships[0];assert_eq!(member.resource_type,ResourceType::Member);
            assert_eq!(member.attributes.as_ref().unwrap()["isOfficer"],true);
            assert!(member.attributes.as_ref().unwrap().get("nostr").is_none());
        }
        let result=index.resource(ResourceKind::Author,&json!({"id":"Author","name":"Author","locked":true}),&BTreeSet::new()).unwrap();
        assert_eq!(result.attributes["locked"],true);
    }
    #[test]
    fn native_nullable_demographic_biography_and_group_links_are_preserved() {
        let mut rows = BTreeMap::new();
        for (kind, native) in [
            (ResourceKind::Author, "Author0001"),
            (ResourceKind::Group, "Group00001"),
        ] {
            rows.insert(
                (kind, native.into()),
                ResolvedPublicId {
                    native_id: native.into(),
                    preferred_id: PublicUuid::parse("11111111-1111-8111-8111-111111111111")
                        .unwrap(),
                    target: PublicIdTarget::NativeResource,
                },
            );
        }
        let index = AliasIndex {
            rows,
            user_attributes: BTreeMap::new(),
            manga_sort_keys: BTreeMap::new(),
        };
        let author = index
            .resource(
                ResourceKind::Author,
                &json!({"id":"Author0001","name":"Actual","description":"Native biography"}),
                &BTreeSet::new(),
            )
            .unwrap();
        assert_eq!(author.attributes["biography"]["en"], "Native biography");
        let group=index.resource(ResourceKind::Group,&json!({"id":"Group00001","name":"Actual","mangaupdates":"https://www.mangaupdates.com/group/actual"}),&BTreeSet::new()).unwrap();
        assert_eq!(
            group.attributes["mangaUpdates"],
            "https://www.mangaupdates.com/group/actual"
        );
        assert!(demographic(&json!({"demographic":"none"})).is_null());
        assert!(demographic(&json!({"demographic":null})).is_null());
        assert_eq!(demographic(&json!({"demographic":"seinen"})), "seinen");
    }
    #[test]
    fn nullable_numbering_and_native_cover_filename_are_preserved() {
        let mut rows = BTreeMap::new();
        for (kind, native, uuid) in [
            (
                ResourceKind::Chapter,
                "Chapter001",
                "11111111-1111-8111-8111-111111111111",
            ),
            (
                ResourceKind::Cover,
                "Cover00001",
                "22222222-2222-8222-8222-222222222222",
            ),
        ] {
            rows.insert(
                (kind, native.into()),
                ResolvedPublicId {
                    native_id: native.into(),
                    preferred_id: PublicUuid::parse(uuid).unwrap(),
                    target: PublicIdTarget::NativeResource,
                },
            );
        }
        let index = AliasIndex {
            rows,
            user_attributes: BTreeMap::new(),
            manga_sort_keys: BTreeMap::new(),
        };
        let chapter = index
            .resource(
                ResourceKind::Chapter,
                &json!({"id":"Chapter001","language":"en","data":[],"version":2}),
                &BTreeSet::new(),
            )
            .unwrap();
        assert!(chapter.attributes["volume"].is_null());
        assert!(chapter.attributes["chapter"].is_null());
        let cover = index
            .resource(
                ResourceKind::Cover,
                &json!({"id":"Cover00001","ext":".webp","version":3}),
                &BTreeSet::new(),
            )
            .unwrap();
        assert_eq!(cover.attributes["fileName"], "Cover00001.webp");
        assert_eq!(cover.id.as_str(), "22222222-2222-8222-8222-222222222222");
    }
    #[test]
    fn separate_official_link_dictionary_is_projected_without_provider_invention() {
        let index=AliasIndex{rows:BTreeMap::from([((ResourceKind::Manga,"Manga00001".into()),ResolvedPublicId{native_id:"Manga00001".into(),preferred_id:PublicUuid::parse("11111111-1111-8111-8111-111111111111").unwrap(),target:PublicIdTarget::NativeResource})]),user_attributes:BTreeMap::new(),manga_sort_keys:BTreeMap::new()};
        let mut native=json!({"id":"Manga00001","title":"Title","relationships":{"links":{"raw":"https://legacy.example"}},"official_links":{"provider":"opaque-id","":"","__proto__":"<b>text</b>"}});
        let result=index.resource(ResourceKind::Manga,&native,&BTreeSet::new()).unwrap();
        assert_eq!(result.attributes["links"],native["relationships"]["links"]);
        assert_eq!(result.attributes["officialLinks"],native["official_links"]);
        assert!(result.relationships.is_empty());
        native["official_links"]=json!({"provider":{"language":"en","region":"US"}});
        assert!(index.resource(ResourceKind::Manga,&native,&BTreeSet::new()).is_err());
    }

}
