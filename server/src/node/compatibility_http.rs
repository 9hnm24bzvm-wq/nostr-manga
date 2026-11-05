//! Explicit MangaDex-shaped compatibility seam; all authority stays in core services.
use super::*;
use crate::{
    catalogue::Entry, compatibility_query::Query as CompatQuery, mangadex_compat as compat,
};
pub(super) fn missing() -> ApiError {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"result":"error","errors":[{"status":404,"title":"Content unavailable"}]})),
    )
}
pub(super) fn failure(e: anyhow::Error) -> ApiError {
    if e.is::<tokio_postgres::Error>() || e.is::<deadpool_postgres::PoolError>() {
        unavailable()
    } else {
        bad("unsupported or invalid compatibility request")
    }
}
pub(super) fn bounded(value: Value) -> Result<Json<Value>, ApiError> {
    if serde_json::to_vec(&value).map_err(|_| unavailable())?.len() > 2 * 1048576 {
        return Err(bad("compatibility response budget"));
    }
    Ok(Json(value))
}
async fn entry(a: &App, kind: &str, key: &str) -> Result<Entry, ApiError> {
    if !compat::valid_uuid(key) {
        return Err(bad("UUID required"));
    }
    a.repo
        .compatibility_entry(kind, key)
        .await
        .map_err(failure)?
        .ok_or_else(missing)
}
async fn render(
    a: &App,
    entries: &[Entry],
    includes: &std::collections::BTreeSet<String>,
) -> Result<Vec<Value>, ApiError> {
    let mut parents = vec![];
    if includes.contains("manga") {
        let ids: Vec<Vec<u8>> = entries
            .iter()
            .filter_map(|e| e.metadata["series"].as_str())
            .filter_map(|s| hex::decode(s).ok())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        if !ids.is_empty() {
            for row in a.repo.pool.get().await.map_err(|_|unavailable())?.query(&format!("SELECT e.raw_event,c.metadata FROM events e JOIN catalogue_metadata c ON c.event_id=e.id WHERE e.id=ANY($1) AND e.kind=1002 AND {}",crate::catalogue::VISIBLE),&[&ids]).await.map_err(|_|unavailable())? {parents.push(Entry{event:serde_json::from_value(row.get(0)).map_err(|_|unavailable())?,metadata:row.get(1)});}
        }
    }
    let refs: Vec<_> = entries.iter().chain(parents.iter()).collect();
    let ids = a
        .repo
        .compatibility_ids_for_refs(&refs)
        .await
        .map_err(failure)?;
    let mut data = vec![];
    for e in entries {
        let mut value = compat::entity(e, &ids, None, includes).map_err(failure)?;
        if let Some(parent) = parents.iter().find(|p| e.metadata["series"] == p.event.id) {
            let rendered = compat::entity(parent, &ids, None, &std::collections::BTreeSet::new())
                .map_err(failure)?;
            if let Some(rels) = value["relationships"].as_array_mut() {
                for r in rels {
                    if r["type"] == "manga" {
                        r["attributes"] = rendered["attributes"].clone();
                    }
                }
            }
        }
        data.push(value);
    }
    Ok(data)
}
async fn list(
    a: &Shared,
    kind: &str,
    raw: Option<&str>,
    series: Option<&str>,
) -> Result<Json<Value>, ApiError> {
    let q = CompatQuery::parse(raw, kind).map_err(failure)?;
    let (entries, total) = a
        .repo
        .compatibility_query(kind, &q, series)
        .await
        .map_err(failure)?;
    let data = render(a, &entries, &q.includes).await?;
    bounded(
        json!({"result":"ok","response":"collection","data":data,"limit":q.limit,"offset":q.offset,"total":total}),
    )
}
pub(super) async fn manga(
    State(a): State<Shared>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    list(&a, "manga", raw.as_deref(), None).await
}
pub(super) async fn chapters(
    State(a): State<Shared>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    list(&a, "chapter", raw.as_deref(), None).await
}
async fn single(
    a: &Shared,
    kind: &str,
    key: &str,
    raw: Option<&str>,
) -> Result<Json<Value>, ApiError> {
    for (k, _) in url::form_urlencoded::parse(raw.unwrap_or("").as_bytes()) {
        if k != "includes[]" {
            return Err(bad("detail supports only includes[]"));
        }
    }
    let q = CompatQuery::parse(raw, kind).map_err(failure)?;
    let e = entry(a, kind, key).await?;
    let mut data = render(a, std::slice::from_ref(&e), &q.includes)
        .await?
        .pop()
        .ok_or_else(unavailable)?;
    data["id"] = json!(key);
    bounded(json!({"result":"ok","response":"entity","data":data}))
}
pub(super) async fn manga_one(
    State(a): State<Shared>,
    Path(key): Path<String>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    single(&a, "manga", &key, raw.as_deref()).await
}
pub(super) async fn chapter_one(
    State(a): State<Shared>,
    Path(key): Path<String>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    single(&a, "chapter", &key, raw.as_deref()).await
}
pub(super) async fn feed(
    State(a): State<Shared>,
    Path(key): Path<String>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    let e = entry(&a, "manga", &key).await?;
    list(&a, "chapter", raw.as_deref(), Some(&e.event.id)).await
}
pub(super) async fn at_home(
    State(a): State<Shared>,
    Path(key): Path<String>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    for (k, v) in url::form_urlencoded::parse(raw.as_deref().unwrap_or("").as_bytes()) {
        if k != "forcePort443" || !["true", "false"].contains(&v.as_ref()) {
            return Err(bad("unsupported at-home option"));
        }
        if v == "true"
            && url::Url::parse(&a.base_url)
                .map_err(|_| unavailable())?
                .port_or_known_default()
                != Some(443)
        {
            return Err(bad("instance does not serve port443"));
        }
    }
    let e = entry(&a, "chapter", &key).await?;
    let pages = e.metadata["pages"].as_array().ok_or_else(unavailable)?;
    let names = pages
        .iter()
        .map(|p| {
            let hash = p["sha256"].as_str().ok_or_else(unavailable)?;
            let mime = p["mime"].as_str().ok_or_else(unavailable)?;
            Ok(format!("{hash}.{}", crate::manifest::extension(mime)))
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    bounded(
        json!({"result":"ok","baseUrl":format!("{}{}",a.base_url,compat::PREFIX),"chapter":{"hash":e.event.id,"data":names,"dataSaver":names},"nostr":{"originalsOnly":true}}),
    )
}
pub(super) async fn image(
    State(a): State<Shared>,
    Path((event, file)): Path<(String, String)>,
) -> Result<axum::response::Response, ApiError> {
    let e = a
        .repo
        .catalogue_entry(id(&event).map_err(|_| bad("chapter hash"))?)
        .await
        .map_err(failure)?
        .filter(|e| e.event.kind == 1001)
        .ok_or_else(missing)?;
    let pages = e.metadata["pages"].as_array().ok_or_else(unavailable)?;
    let hash = pages
        .iter()
        .find_map(|p| {
            let hash = p["sha256"].as_str()?;
            let mime = p["mime"].as_str()?;
            (file == format!("{hash}.{}", crate::manifest::extension(mime)))
                .then(|| hash.to_owned())
        })
        .ok_or_else(missing)?;
    public_original(State(a), Path(hash)).await
}
pub(super) async fn cover(
    State(a): State<Shared>,
    Path(key): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let e = entry(&a, "cover", &key).await?;
    let ids = a
        .repo
        .compatibility_ids_for(std::slice::from_ref(&e))
        .await
        .map_err(failure)?;
    bounded(
        json!({"result":"ok","response":"entity","data":{"id":key,"type":"cover_art","attributes":compat::cover_attributes(&e).map_err(failure)?,"relationships":[{"id":ids.get("manga",&e.event.id),"type":"manga"}]}}),
    )
}
pub(super) async fn cover_image(
    State(a): State<Shared>,
    Path((key, file)): Path<(String, String)>,
) -> Result<axum::response::Response, ApiError> {
    let e = entry(&a, "manga", &key).await?;
    let attrs = compat::cover_attributes(&e).map_err(|_| missing())?;
    if attrs["fileName"] != file {
        return Err(missing());
    }
    let hash = e.metadata["cover"]["sha256"]
        .as_str()
        .ok_or_else(missing)?
        .to_owned();
    public_original(State(a), Path(hash)).await
}
pub(super) async fn aliases(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Alias {
        kind: String,
        id: String,
        target: String,
        #[serde(default)]
        preferred: bool,
    }
    let v = operator_json(&a, &uri, &headers, request).await?;
    let r: Alias = serde_json::from_value(v).map_err(|_| bad("alias fields"))?;
    a.repo
        .register_compatibility_alias(&r.kind, &r.id, &r.target, r.preferred)
        .await
        .map_err(failure)?;
    Ok(Json(
        json!({"result":"ok","id":r.id,"signed_identity_changed":false}),
    ))
}
pub(super) async fn capability() -> Json<Value> {
    Json(
        json!({"result":"ok","experimental":true,"prefix":compat::PREFIX,"drop_in_full_api":false,"writes":"NIP98 plus original signed publication or library root; managed group signing uses private role checks","reader_accounts":true,"public_lists":true,"reading_progress":true,"managed_group_profiles":true,"group_publication_retries":"persist_created_at_and_payload","library_snapshot_parameter":"head","library_snapshot_conflict_status":409,"in_place_edits":false,"mangadex_passwords_accepted":false,"dataSaver":"exact originals; no transformed variants","ids":"local UUID aliases; Nostr remains authoritative"}),
    )
}
pub(super) async fn tags(State(a): State<Shared>) -> Result<Json<Value>, ApiError> {
    let rows=a.repo.pool.get().await.map_err(|_|unavailable())?.query(&format!("SELECT DISTINCT t.id,t.target COLLATE \"C\" AS target FROM compatibility_ids t WHERE t.kind='tag' AND t.preferred AND EXISTS(SELECT 1 FROM events e JOIN catalogue_metadata c ON c.event_id=e.id WHERE e.kind=1002 AND {} AND c.metadata->'categories' @> jsonb_build_array(t.target)) ORDER BY target LIMIT 1001",crate::catalogue::VISIBLE),&[]).await.map_err(|_|unavailable())?;
    if rows.len() > 1000 {
        return Err(bad("tag list budget exceeded"));
    }
    bounded(
        json!({"result":"ok","response":"collection","data":rows.into_iter().map(|r|json!({"id":r.get::<_,String>(0),"type":"tag","attributes":{"name":{"en":r.get::<_,String>(1)},"description":{},"version":1},"relationships":[]})).collect::<Vec<_>>()}),
    )
}
pub(super) async fn unsupported() -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(
            json!({"result":"error","errors":[{"status":501,"title":"Operation not supported by this compatibility profile"}]}),
        ),
    )
}
async fn group_key(a: &App, alias: &str) -> Result<String, ApiError> {
    if !compat::valid_uuid(alias) {
        return Err(bad("group UUID required"));
    }
    a.repo
        .compatibility_target("group", alias)
        .await
        .map_err(failure)?
        .map(|v| v.0)
        .ok_or_else(missing)
}
pub(super) async fn group_create(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let Json(v) = create_group(State(a.clone()), OriginalUri(uri), headers, request).await?;
    let key = v["group"].as_str().ok_or_else(unavailable)?;
    Ok(Json(
        json!({"result":"ok","response":"entity","data":{"id":a.repo.compatibility_id("group",key).await.map_err(failure)?,"type":"scanlation_group","attributes":{"name":key,"version":1},"relationships":[],"nostr":v}}),
    ))
}
pub(super) async fn group_members_get(
    State(a): State<Shared>,
    Path(alias): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let key = group_key(&a, &alias).await?;
    list_group_members(State(a), Path(key), OriginalUri(uri), headers).await
}
pub(super) async fn group_members_set(
    State(a): State<Shared>,
    Path(alias): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let key = group_key(&a, &alias).await?;
    set_group_member(State(a), Path(key), OriginalUri(uri), headers, request).await
}
pub(super) async fn group_publication(
    State(a): State<Shared>,
    Path(alias): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let key = group_key(&a, &alias).await?;
    let (status, Json(mut value)) = group_publish(
        State(a.clone()),
        Path(key),
        OriginalUri(uri),
        headers,
        request,
    )
    .await?;
    let kind = if value["event"]["kind"] == 1002 {
        "manga"
    } else {
        "chapter"
    };
    value["compatibilityId"] = json!(
        a.repo
            .compatibility_id(kind, value["event"]["id"].as_str().ok_or_else(unavailable)?)
            .await
            .map_err(failure)?
    );
    Ok((status, Json(value)))
}
pub(super) async fn group_blob(
    State(a): State<Shared>,
    Path((alias, hash)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let key = group_key(&a, &alias).await?;
    group_upload(
        State(a),
        Path((key, hash)),
        OriginalUri(uri),
        headers,
        request,
    )
    .await
}
pub(super) async fn group_deletion(
    State(a): State<Shared>,
    Path(alias): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let key = group_key(&a, &alias).await?;
    group_deletion_request(State(a), Path(key), OriginalUri(uri), headers, request).await
}
pub(super) async fn publication(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let (status, Json(mut value)) =
        publish_event(State(a.clone()), OriginalUri(uri), headers, request).await?;
    let eid = value["id"].as_str().ok_or_else(unavailable)?;
    let kind = a
        .repo
        .pool
        .get()
        .await
        .map_err(|_| unavailable())?
        .query_one(
            "SELECT kind FROM events WHERE id=$1",
            &[&id(eid).map_err(|_| unavailable())?.as_slice()],
        )
        .await
        .map_err(|_| unavailable())?
        .get::<_, i32>(0);
    value["compatibilityId"] = json!(
        a.repo
            .compatibility_id(if kind == 1002 { "manga" } else { "chapter" }, eid)
            .await
            .map_err(failure)?
    );
    Ok((status, Json(value)))
}
#[derive(Default)]
struct ResourceQuery {
    limit: i64,
    offset: i64,
    ids: Vec<String>,
    manga: Vec<String>,
    name: Option<String>,
}
impl ResourceQuery {
    fn parse(raw: Option<&str>) -> Result<Self, ApiError> {
        let raw = raw.unwrap_or("");
        if raw.len() > 32768 {
            return Err(bad("query budget"));
        }
        let mut q = Self {
            limit: 10,
            ..Self::default()
        };
        let mut seen = std::collections::BTreeSet::new();
        for (k, v) in url::form_urlencoded::parse(raw.as_bytes()) {
            if v.len() > 4096 || v.contains('\0') {
                return Err(bad("query value budget"));
            }
            if !k.ends_with("[]") && !seen.insert(k.to_string()) {
                return Err(bad("duplicate query field"));
            }
            match k.as_ref() {
                "limit" => q.limit = v.parse().map_err(|_| bad("limit"))?,
                "offset" => q.offset = v.parse().map_err(|_| bad("offset"))?,
                "ids[]" | "manga[]" => {
                    if !compat::valid_uuid(&v) {
                        return Err(bad("UUID required"));
                    }
                    let a = if k == "ids[]" {
                        &mut q.ids
                    } else {
                        &mut q.manga
                    };
                    if a.len() >= 100 {
                        return Err(bad("query ID budget"));
                    }
                    a.push(v.to_string());
                }
                "name" => q.name = Some(v.to_string()),
                "includes[]" => {
                    if !["manga"].contains(&v.as_ref()) {
                        return Err(bad("unsupported expansion"));
                    }
                }
                _ => return Err(bad("unsupported resource query")),
            }
        }
        if !(0..=100).contains(&q.limit) || q.offset < 0 {
            return Err(bad("paging bounds"));
        }
        Ok(q)
    }
}
async fn resource_list(a: &Shared, kind: &str, raw: Option<&str>) -> Result<Json<Value>, ApiError> {
    let q = ResourceQuery::parse(raw)?;
    if kind == "cover" && q.name.is_some() || kind != "cover" && !q.manga.is_empty() {
        return Err(bad("unsupported resource filter"));
    }
    let manga = a
        .repo
        .compatibility_targets("manga", &q.manga)
        .await
        .map_err(failure)?;
    let selection = if kind == "group" {
        "JOIN managed_groups g ON encode(g.group_key,'hex')=t.target WHERE t.kind='group' AND g.enabled".to_owned()
    } else {
        let target = if kind == "cover" {
            "t.target"
        } else {
            "t.payload->>'series'"
        };
        format!(
            "JOIN events e ON encode(e.id,'hex')={target} JOIN catalogue_metadata c ON c.event_id=e.id WHERE t.kind='{}' AND {}{}",
            if kind == "cover" { "cover" } else { "author" },
            crate::catalogue::VISIBLE,
            if kind == "cover" {
                " AND c.metadata->'cover' IS NOT NULL AND c.metadata->'cover'<>'null'::jsonb"
            } else {
                ""
            }
        )
    };
    let name = if kind == "group" {
        "COALESCE((SELECT CASE WHEN jsonb_typeof((e.raw_event->>'content')::jsonb->'display_name')='string' AND octet_length((e.raw_event->>'content')::jsonb->>'display_name') BETWEEN 1 AND 512 THEN (e.raw_event->>'content')::jsonb->>'display_name' WHEN jsonb_typeof((e.raw_event->>'content')::jsonb->'name')='string' AND octet_length((e.raw_event->>'content')::jsonb->>'name') BETWEEN 1 AND 512 THEN (e.raw_event->>'content')::jsonb->>'name' ELSE NULL END FROM account_heads h JOIN events e ON e.id=h.event_id WHERE h.publisher=decode(t.target,'hex') AND h.kind=0 AND e.state='hosted'),t.target)"
    } else {
        "t.payload->'credit'->>'name'"
    };
    let pred = format!(
        "{selection} AND t.preferred AND (NOT $1 OR t.id=ANY($2)) AND (NOT $3 OR t.target=ANY($4)) AND ($5::text IS NULL OR strpos({name},$5)>0)"
    );
    let mut conn = a.repo.pool.get().await.map_err(|_| unavailable())?;
    let tx = conn
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .map_err(|_| unavailable())?;
    tx.batch_execute("SET LOCAL statement_timeout='2000ms'; SET LOCAL lock_timeout='500ms'")
        .await
        .map_err(|_| unavailable())?;
    let total: i64 = tx
        .query_one(
            &format!("SELECT count(*) FROM compatibility_ids t {pred}"),
            &[
                &!q.ids.is_empty(),
                &q.ids,
                &!q.manga.is_empty(),
                &manga,
                &q.name,
            ],
        )
        .await
        .map_err(|_| unavailable())?
        .get(0);
    let rows=tx.query(&format!("SELECT t.id,t.target,t.payload FROM compatibility_ids t {pred} ORDER BY t.id LIMIT $6 OFFSET $7"),&[&!q.ids.is_empty(),&q.ids,&!q.manga.is_empty(),&manga,&q.name,&q.limit,&q.offset]).await.map_err(|_|unavailable())?;
    tx.commit().await.map_err(|_| unavailable())?;
    drop(conn);
    let mut data = vec![];
    for row in rows {
        let key: String = row.get(0);
        let target: String = row.get(1);
        let payload: Value = row.get(2);
        if kind == "cover" {
            let Json(v) = cover(State(a.clone()), Path(key)).await?;
            data.push(v["data"].clone());
        } else if kind == "group" {
            let profile =
                super::reader_http::profile(a, id(&target).map_err(|_| unavailable())?, true)
                    .await?;
            let metadata = &profile["nostr"]["metadata"];
            data.push(json!({"id":key,"type":"scanlation_group","attributes":{"name":profile["attributes"]["username"],"description":metadata.get("about").and_then(Value::as_str),"website":metadata.get("website").and_then(Value::as_str)},"relationships":[],"nostr":{"pubkey":target,"profile":profile["nostr"]["profile"],"nameIsPublicKeyFallback":profile["attributes"]["username"]==target}}));
        } else {
            let series = payload["series"].as_str().ok_or_else(unavailable)?;
            data.push(json!({"id":key,"type":"author","attributes":{"name":payload["credit"]["name"],"biography":{},"version":1},"relationships":[{"id":a.repo.compatibility_id("manga",series).await.map_err(failure)?,"type":"manga"}],"nostr":{"descriptiveCreditOnly":true}}));
        }
    }
    bounded(
        json!({"result":"ok","response":"collection","data":data,"limit":q.limit,"offset":q.offset,"total":total}),
    )
}
pub(super) async fn covers(
    State(a): State<Shared>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    resource_list(&a, "cover", raw.as_deref()).await
}
pub(super) async fn authors(
    State(a): State<Shared>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    resource_list(&a, "author", raw.as_deref()).await
}
pub(super) async fn groups(
    State(a): State<Shared>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    resource_list(&a, "group", raw.as_deref()).await
}
async fn resource_one(a: &Shared, kind: &str, key: &str) -> Result<Json<Value>, ApiError> {
    if !compat::valid_uuid(key) {
        return Err(bad("UUID required"));
    }
    let (target, _) = a
        .repo
        .compatibility_target(kind, key)
        .await
        .map_err(failure)?
        .ok_or_else(missing)?;
    let preferred = a
        .repo
        .compatibility_id(kind, &target)
        .await
        .map_err(failure)?;
    let query = format!("ids[]={preferred}");
    let Json(v) = resource_list(a, kind, Some(&query)).await?;
    let mut data = v["data"]
        .as_array()
        .and_then(|a| a.first())
        .cloned()
        .ok_or_else(missing)?;
    data["id"] = json!(key);
    Ok(Json(json!({"result":"ok","response":"entity","data":data})))
}
pub(super) async fn group_profile(
    State(a): State<Shared>,
    Path(alias): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let key = a
        .repo
        .compatibility_target("group", &alias)
        .await
        .map_err(failure)?
        .ok_or_else(missing)?
        .0;
    super::set_group_profile(State(a), Path(key), OriginalUri(uri), headers, request).await
}
pub(super) async fn group_one(
    State(a): State<Shared>,
    Path(key): Path<String>,
) -> Result<Json<Value>, ApiError> {
    resource_one(&a, "group", &key).await
}
pub(super) async fn author_one(
    State(a): State<Shared>,
    Path(key): Path<String>,
) -> Result<Json<Value>, ApiError> {
    resource_one(&a, "author", &key).await
}
pub(super) async fn aggregate(
    State(a): State<Shared>,
    Path(key): Path<String>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    let raw = raw.unwrap_or_default();
    if raw.len() > 32768 {
        return Err(bad("query budget"));
    }
    let mut languages = vec![];
    let mut groups = vec![];
    for (k, v) in url::form_urlencoded::parse(raw.as_bytes()) {
        match k.as_ref() {
            "translatedLanguage[]" => {
                if languages.len() >= 100
                    || v.len() > 64
                    || (v != "*" && language_tags::LanguageTag::parse(&v).is_err())
                {
                    return Err(bad("language range"));
                }
                languages.push(v.to_string());
            }
            "groups[]" => {
                if groups.len() >= 100 || !compat::valid_uuid(&v) {
                    return Err(bad("group UUID"));
                }
                groups.push(v.to_string());
            }
            "includeUnavailable" if v == "0" => {}
            _ => return Err(bad("unsupported aggregate option")),
        }
    }
    let series = entry(&a, "manga", &key).await?;
    let publishers = a
        .repo
        .compatibility_targets("group", &groups)
        .await
        .map_err(failure)?
        .into_iter()
        .map(hex::decode)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| unavailable())?;
    let mut c = a.repo.pool.get().await.map_err(|_| unavailable())?;
    let tx = c
        .build_transaction()
        .read_only(true)
        .start()
        .await
        .map_err(|_| unavailable())?;
    tx.batch_execute("SET LOCAL statement_timeout='2000ms'")
        .await
        .map_err(|_| unavailable())?;
    let rows=tx.query(&format!("SELECT e.id,left(e.volume,257),left(e.chapter,257),t.id FROM events e LEFT JOIN compatibility_ids t ON t.kind='chapter' AND t.target=encode(e.id,'hex') AND t.preferred WHERE e.kind=1001 AND e.series=$1 AND {} AND (NOT $2 OR EXISTS(SELECT 1 FROM unnest($3::text[]) l WHERE l='*' OR lower(e.language)=lower(l) OR lower(e.language) LIKE lower(l)||'-%')) AND (NOT $4 OR e.publisher=ANY($5::bytea[])) ORDER BY e.created_at DESC,e.id LIMIT 5001",crate::catalogue::VISIBLE),&[&id(&series.event.id).map_err(|_|unavailable())?.as_slice(),&!languages.is_empty(),&languages,&!groups.is_empty(),&publishers]).await.map_err(|_|unavailable())?;
    tx.commit().await.map_err(|_| unavailable())?;
    if rows.len() > 5000 {
        return Err(bad("aggregate chapter budget; use paginated feed"));
    }
    let mut grouped: std::collections::BTreeMap<(String, String), Vec<String>> =
        std::collections::BTreeMap::new();
    for r in rows {
        let volume = r
            .get::<_, Option<String>>(1)
            .unwrap_or_else(|| "none".into());
        let chapter = r
            .get::<_, Option<String>>(2)
            .unwrap_or_else(|| "none".into());
        if volume.chars().count() > 256 || chapter.chars().count() > 256 {
            return Err(bad("aggregate label budget; use paginated feed"));
        }
        let eid = hex::encode(r.get::<_, Vec<u8>>(0));
        let key = r
            .get::<_, Option<String>>(3)
            .unwrap_or_else(|| compat::uuid("chapter", &eid));
        grouped.entry((volume, chapter)).or_default().push(key);
    }
    let mut volumes = serde_json::Map::new();
    for ((volume, chapter), ids) in grouped {
        let v = volumes
            .entry(volume.clone())
            .or_insert_with(|| json!({"volume":volume,"count":0,"chapters":{}}));
        v["count"] = json!(v["count"].as_u64().unwrap() + 1);
        v["chapters"][&chapter] = json!({"chapter":chapter,"id":ids[0],"others":&ids[1..],"count":ids.len(),"isUnavailable":false});
    }
    bounded(
        json!({"result":"ok","volumes":volumes,"nostr":{"groupingIsInstancePresentationOnly":true,"completionNeverInheritsBetweenIds":true}}),
    )
}
