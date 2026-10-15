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
            if ids.len() > 4096 {
                return Err(PublicApiError::unavailable());
            }
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
        Ok(Self {
            rows,
            user_attributes,
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
                common(value, &mut attributes);
                if let Some(links) = value["relationships"]["links"].as_object() {
                    attributes["links"] = json!(
                        links
                            .iter()
                            .filter(|(_, v)| v.is_string())
                            .collect::<BTreeMap<_, _>>()
                    );
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
                let mut a = json!({"name":value["name"],"biography":localized(value.get("biography").unwrap_or(&value["description"])),"imageUrl":nullable(value,&["imageUrl","image_url"])});
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
                let mut a = json!({"name":value["name"],"altNames":alt_titles(&value["alt_names"]),"locked":value["locked"],"inactive":value["inactive"]});
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
                    if let Some(r) = self.relationship(
                        ResourceKind::User,
                        if role == "leader" {
                            ResourceType::Leader
                        } else {
                            ResourceType::Member
                        },
                        &member,
                        includes.contains(role),
                    )? {
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
            _ => return Err(PublicApiError::unsupported()),
        };
        let wire = match kind {
            ResourceKind::Manga => ResourceType::Manga,
            ResourceKind::Chapter => ResourceType::Chapter,
            ResourceKind::Cover => ResourceType::CoverArt,
            ResourceKind::Author => ResourceType::Author,
            ResourceKind::Group => ResourceType::ScanlationGroup,
            ResourceKind::Tag => ResourceType::Tag,
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
}
