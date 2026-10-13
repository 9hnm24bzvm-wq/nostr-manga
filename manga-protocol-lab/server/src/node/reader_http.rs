//! Public reader convenience interfaces; signed native workflows remain authoritative.
use super::compatibility_http::{bounded, missing};
use super::*;
use crate::{
    library_manifest::{Family, Record},
    library_view::{SnapshotChanged, View, ViewBudget},
    mangadex_compat as compat,
};
use axum::Extension;
use std::collections::{BTreeMap, BTreeSet};

fn error(e: anyhow::Error) -> ApiError {
    if e.is::<SnapshotChanged>() {
        (
            StatusCode::CONFLICT,
            Json(json!({"error":"library_snapshot_changed"})),
        )
    } else if let Some(b) = e.downcast_ref::<ViewBudget>() {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"library_view_budget","resource":b.0})),
        )
    } else {
        library_error(e, StatusCode::NOT_FOUND)
    }
}
async fn deadline<T>(
    future: impl std::future::Future<Output = anyhow::Result<T>>,
) -> Result<T, ApiError> {
    read_admission::deadline(
        Duration::from_secs(15),
        future,
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"library_view_deadline"})),
        ),
    )
    .await?
    .map_err(error)
}
fn fields(raw: Option<&str>, allowed: &[&str]) -> Result<BTreeMap<String, String>, ApiError> {
    let raw = raw.unwrap_or("");
    if raw.len() > 32768 {
        return Err(bad("query byte budget"));
    }
    let mut result = BTreeMap::new();
    for (k, v) in url::form_urlencoded::parse(raw.as_bytes()).into_owned() {
        if !allowed.contains(&k.as_str()) || result.contains_key(&k) || v.len() > 4096 {
            return Err(bad("unsupported or duplicate query parameter"));
        }
        result.insert(k, v);
    }
    if let Some(head) = result.get("head") {
        id(head).map_err(|_| bad("invalid library head"))?;
    }
    Ok(result)
}
fn paging(f: &BTreeMap<String, String>) -> Result<(u64, usize), ApiError> {
    let offset = f
        .get("offset")
        .map(|s| s.parse::<u64>())
        .transpose()
        .map_err(|_| bad("offset"))?
        .unwrap_or(0);
    let limit = f
        .get("limit")
        .map(|s| s.parse::<usize>())
        .transpose()
        .map_err(|_| bad("limit"))?
        .unwrap_or(10);
    if limit > 100 {
        return Err(bad("limit outside0..100"));
    }
    Ok((offset, limit))
}
fn identity(identity: Option<Extension<ReadIdentity>>) -> Result<Id, ApiError> {
    identity
        .map(|Extension(i)| i.0)
        .ok_or_else(|| forbidden("NIP98 reader authentication required"))
}
async fn target(a: &App, kind: &str, key: &str) -> Result<Id, ApiError> {
    if key.len() == 64 {
        return id(key).map_err(|_| bad("invalid public identifier"));
    }
    let (key, _) = a
        .repo
        .compatibility_target(kind, key)
        .await
        .map_err(error)?
        .ok_or_else(missing)?;
    id(&key).map_err(|_| unavailable())
}
async fn register(a: &App, kind: &str, key: &str, payload: Value) -> Result<String, ApiError> {
    let mut c = a.repo.pool.get().await.map_err(|_| unavailable())?;
    let tx = c.transaction().await.map_err(|_| unavailable())?;
    let key = compat::register(&tx, kind, key, payload)
        .await
        .map_err(error)?;
    tx.commit().await.map_err(|_| unavailable())?;
    Ok(key)
}
pub(super) async fn profile(a: &App, publisher: Id, allow_empty: bool) -> Result<Value, ApiError> {
    let c = a.repo.pool.get().await.map_err(|_| unavailable())?;
    let known: bool = c
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM events WHERE publisher=$1 AND state='hosted')",
            &[&publisher.as_slice()],
        )
        .await
        .map_err(|_| unavailable())?
        .get(0);
    let row=c.query_opt("SELECT event_id FROM account_heads WHERE publisher=$1 AND kind=0 AND event_id IS NOT NULL",&[&publisher.as_slice()]).await.map_err(|_|unavailable())?;
    drop(c);
    if !known && !allow_empty {
        return Err(missing());
    }
    let mut event = None;
    if let Some(row) = row {
        let key: Id = row
            .get::<_, Vec<u8>>(0)
            .try_into()
            .map_err(|_| unavailable())?;
        event = a.repo.visible_event(key).await.map_err(|_| unavailable())?;
    }
    let metadata = event
        .as_ref()
        .map(|e| serde_json::from_str::<Value>(&e.content))
        .transpose()
        .map_err(|_| unavailable())?
        .unwrap_or(json!({}));
    let public = hex::encode(publisher);
    let username = metadata
        .get("display_name")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 512)
        .or_else(|| {
            metadata
                .get("name")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty() && s.len() <= 512)
        })
        .unwrap_or(&public);
    let alias = if known {
        register(a, "user", &public, json!({"nostrPubkey":public})).await?
    } else {
        compat::uuid("user", &public)
    };
    Ok(
        json!({"id":alias,"type":"user","attributes":{"username":username,"roles":[]},"relationships":[],"nostr":{"pubkey":public,"known":known,"profile":event.as_deref(),"metadata":metadata}}),
    )
}
pub(super) async fn me(
    State(a): State<Shared>,
    identity_arg: Option<Extension<ReadIdentity>>,
) -> Result<Json<Value>, ApiError> {
    let data = profile(&a, identity(identity_arg)?, true).await?;
    bounded(json!({"result":"ok","response":"entity","data":data}))
}
pub(super) async fn user(
    State(a): State<Shared>,
    Path(key): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let data = profile(&a, target(&a, "user", &key).await?, false).await?;
    bounded(json!({"result":"ok","response":"entity","data":data}))
}
pub(super) async fn entries(
    State(a): State<Shared>,
    Path(key): Path<String>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    let f = fields(raw.as_deref(), &["family", "limit", "offset", "head"])?;
    let (offset, limit) = paging(&f)?;
    let key = target(&a, "user", &key).await?;
    read_library_entries(
        State(a),
        Path(hex::encode(key)),
        Query(EntryQuery {
            family: f
                .get("family")
                .cloned()
                .ok_or_else(|| bad("family required"))?,
            offset: Some(offset),
            limit: Some(limit),
            head: f.get("head").cloned(),
        }),
    )
    .await
}
async fn list_entities(a: &App, view: &View, records: &[Record]) -> Result<Vec<Value>, ApiError> {
    let objects = a.library_objects.clone().ok_or_else(unavailable)?;
    let wanted: BTreeSet<[u8; 16]> = records
        .iter()
        .filter_map(|r| {
            if let Record::List { id, .. } = r {
                Some(*id)
            } else {
                None
            }
        })
        .collect();
    let members = deadline(objects.view_filtered(
        view,
        Family::Members,
        move |r| matches!(r,Record::Member{list,..} if wanted.contains(list)),
        0,
        5001,
    ))
    .await?;
    if members.total > 5000 {
        return Err(error(ViewBudget("list_relationships").into()));
    }
    let targets = members
        .records
        .iter()
        .filter_map(|r| {
            if let Record::Member { series, .. } = r {
                Some(hex::encode(series))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let aliases = a
        .repo
        .compatibility_identifiers("manga", &targets)
        .await
        .map_err(error)?;
    let owner = a
        .repo
        .compatibility_id("user", &hex::encode(view.publisher))
        .await
        .map_err(error)?;
    let mut data = vec![];
    for record in records {
        let Record::List {
            id: list_id,
            name,
            updated_at,
        } = record
        else {
            continue;
        };
        let list_key = format!("{}:{}", hex::encode(view.publisher), hex::encode(list_id));
        let alias = register(
            a,
            "list",
            &list_key,
            json!({"pubkey":hex::encode(view.publisher),"list":hex::encode(list_id)}),
        )
        .await?;
        let mut related = members
            .records
            .iter()
            .filter_map(|r| match r {
                Record::Member {
                    list,
                    series,
                    position,
                    ..
                } if list == list_id => Some((*position, *series)),
                _ => None,
            })
            .collect::<Vec<_>>();
        related.sort();
        let mut relationships = vec![json!({"id":owner,"type":"user"})];
        for (position, series) in related {
            let key = hex::encode(series);
            let alias = aliases.get("manga", &key);
            relationships.push(
                json!({"id":alias,"type":"manga","nostr":{"eventId":key,"position":position}}),
            );
        }
        data.push(json!({"id":alias,"type":"custom_list","attributes":{"name":name,"visibility":"public"},"relationships":relationships,"nostr":{"pubkey":hex::encode(view.publisher),"listId":hex::encode(list_id),"head":view.event.id,"root":hex::encode(view.root_hash),"updatedAt":updated_at,"memberOrder":"position_then_exact_id"}}));
    }
    objects.verify_view(view).await.map_err(error)?;
    Ok(data)
}
async fn lists(a: &Shared, publisher: Id, raw: Option<&str>) -> Result<Json<Value>, ApiError> {
    let f = fields(raw, &["limit", "offset", "head"])?;
    let (offset, limit) = paging(&f)?;
    let objects = a.library_objects.clone().ok_or_else(unavailable)?;
    let Some(view) =
        deadline(objects.public_view(publisher, f.get("head").map(String::as_str))).await?
    else {
        return bounded(
            json!({"result":"ok","response":"collection","data":[],"limit":limit,"offset":offset,"total":0,"nostr":{"head":null}}),
        );
    };
    let page = deadline(objects.view_page(&view, Family::Lists, offset, limit)).await?;
    let data = list_entities(a, &view, &page.records).await?;
    bounded(
        json!({"result":"ok","response":"collection","data":data,"limit":limit,"offset":offset,"total":page.total,"nostr":{"head":view.event.id,"root":hex::encode(view.root_hash)}}),
    )
}
pub(super) async fn my_lists(
    State(a): State<Shared>,
    identity_arg: Option<Extension<ReadIdentity>>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    lists(&a, identity(identity_arg)?, raw.as_deref()).await
}
pub(super) async fn user_lists(
    State(a): State<Shared>,
    Path(key): Path<String>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    lists(&a, target(&a, "user", &key).await?, raw.as_deref()).await
}
async fn list_target(a: &App, key: &str) -> Result<(Id, [u8; 16]), ApiError> {
    let (target, _) = a
        .repo
        .compatibility_target("list", key)
        .await
        .map_err(error)?
        .ok_or_else(missing)?;
    let (owner, list) = target.split_once(':').ok_or_else(unavailable)?;
    Ok((
        id(owner).map_err(|_| unavailable())?,
        hex::decode(list)
            .map_err(|_| unavailable())?
            .try_into()
            .map_err(|_| unavailable())?,
    ))
}
pub(super) async fn list(
    State(a): State<Shared>,
    Path(key): Path<String>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    let f = fields(raw.as_deref(), &["head"])?;
    let (owner, list_id) = list_target(&a, &key).await?;
    let objects = a.library_objects.clone().ok_or_else(unavailable)?;
    let view = deadline(objects.public_view(owner, f.get("head").map(String::as_str)))
        .await?
        .ok_or_else(missing)?;
    let records = deadline(objects.view_lookup(&view, Family::Lists, &[list_id.to_vec()])).await?;
    if records.is_empty() {
        return Err(missing());
    }
    let data = list_entities(&a, &view, &records).await?.remove(0);
    bounded(json!({"result":"ok","response":"entity","data":data}))
}
pub(super) async fn members(
    State(a): State<Shared>,
    Path(key): Path<String>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    let f = fields(raw.as_deref(), &["limit", "offset", "head"])?;
    let (offset, limit) = paging(&f)?;
    let (owner, list_id) = list_target(&a, &key).await?;
    let objects = a.library_objects.clone().ok_or_else(unavailable)?;
    let view = deadline(objects.public_view(owner, f.get("head").map(String::as_str)))
        .await?
        .ok_or_else(missing)?;
    if deadline(objects.view_lookup(&view, Family::Lists, &[list_id.to_vec()]))
        .await?
        .is_empty()
    {
        return Err(missing());
    }
    let page = deadline(objects.view_filtered(
        &view,
        Family::Members,
        move |r| matches!(r,Record::Member{list,..} if *list==list_id),
        offset,
        limit,
    ))
    .await?;
    bounded(
        json!({"result":"ok","response":"collection","data":page.records.iter().map(crate::library_view::record_json).collect::<Vec<_>>(),"limit":limit,"offset":offset,"total":page.total,"nostr":{"head":view.event.id,"root":hex::encode(view.root_hash),"order":"bucket_then_record_key","positionsPreserved":true}}),
    )
}
pub(super) async fn statuses(
    State(a): State<Shared>,
    identity_arg: Option<Extension<ReadIdentity>>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    let publisher = identity(identity_arg)?;
    let f = fields(raw.as_deref(), &["status", "head"])?;
    let status = f.get("status").cloned();
    if status.as_ref().is_some_and(|s| {
        ![
            "reading",
            "on_hold",
            "plan_to_read",
            "dropped",
            "re_reading",
            "completed",
        ]
        .contains(&s.as_str())
    }) {
        return Err(bad("invalid status"));
    }
    let objects = a.library_objects.clone().ok_or_else(unavailable)?;
    let Some(view) =
        deadline(objects.public_view(publisher, f.get("head").map(String::as_str))).await?
    else {
        return bounded(json!({"result":"ok","statuses":{},"nostr":{"head":null}}));
    };
    let page=deadline(objects.view_filtered(&view,Family::Series,move|r|matches!(r,Record::Series{status:s,..} if status.as_ref().is_none_or(|wanted|wanted==s)),0,10001)).await?;
    if page.total > 10000 {
        return Err(error(ViewBudget("statuses_count").into()));
    }
    let targets = page
        .records
        .iter()
        .filter_map(|r| {
            if let Record::Series { id, .. } = r {
                Some(hex::encode(id))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let aliases = a
        .repo
        .compatibility_identifiers("manga", &targets)
        .await
        .map_err(error)?;
    let mut statuses = serde_json::Map::new();
    let mut ids = serde_json::Map::new();
    for r in page.records {
        if let Record::Series { id, status, .. } = r {
            let key = hex::encode(id);
            let alias = aliases.get("manga", &key);
            statuses.insert(alias.clone(), json!(status));
            ids.insert(alias, json!(key));
        }
    }
    objects.verify_view(&view).await.map_err(error)?;
    bounded(
        json!({"result":"ok","statuses":statuses,"nostr":{"head":view.event.id,"root":hex::encode(view.root_hash),"eventIds":ids}}),
    )
}
pub(super) async fn status(
    State(a): State<Shared>,
    identity_arg: Option<Extension<ReadIdentity>>,
    Path(key): Path<String>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    let publisher = identity(identity_arg)?;
    let f = fields(raw.as_deref(), &["head"])?;
    let manga = target(&a, "manga", &key).await?;
    let objects = a.library_objects.clone().ok_or_else(unavailable)?;
    let mut status = None;
    let mut head = None;
    if let Some(view) =
        deadline(objects.public_view(publisher, f.get("head").map(String::as_str))).await?
    {
        let records =
            deadline(objects.view_lookup(&view, Family::Series, &[manga.to_vec()])).await?;
        for r in records {
            if let Record::Series { status: s, .. } = r {
                status = Some(s)
            }
        }
        head = Some(view.event.id);
    }
    bounded(
        json!({"result":"ok","status":status,"nostr":{"head":head,"eventId":hex::encode(manga)}}),
    )
}
async fn read_for(
    a: &Shared,
    publisher: Id,
    requested: Vec<(String, Id)>,
    head: Option<&str>,
    grouped: bool,
) -> Result<Json<Value>, ApiError> {
    let objects = a.library_objects.clone().ok_or_else(unavailable)?;
    let mut groups = serde_json::Map::new();
    for (key, _) in &requested {
        groups.insert(key.clone(), json!([]));
    }
    let Some(view) = deadline(objects.public_view(publisher, head)).await? else {
        return bounded(
            json!({"result":"ok","data":if grouped{Value::Object(groups)}else{json!([])},"nostr":{"head":null}}),
        );
    };
    let series = requested
        .iter()
        .map(|(_, key)| key.to_vec())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut c = a.repo.pool.get().await.map_err(|_| unavailable())?;
    let tx = c.transaction().await.map_err(|_| unavailable())?;
    tx.batch_execute("SET LOCAL statement_timeout='2s';SET LOCAL lock_timeout='500ms'")
        .await
        .map_err(|_| unavailable())?;
    let rows=tx.query(&format!("SELECT e.id,e.series FROM events e WHERE e.kind=1001 AND e.series=ANY($1) AND {} ORDER BY e.id LIMIT5001",crate::catalogue::VISIBLE).replace("LIMIT5001","LIMIT 5001"),&[&series]).await.map_err(|_|unavailable())?;
    tx.commit().await.map_err(|_| unavailable())?;
    drop(c);
    if rows.len() > 5000 {
        return Err(error(ViewBudget("manga_chapters").into()));
    }
    let keys = rows
        .iter()
        .map(|r| r.get::<_, Vec<u8>>(0))
        .collect::<Vec<_>>();
    let records = deadline(objects.view_lookup(&view, Family::Chapters, &keys)).await?;
    let completed = records
        .iter()
        .filter_map(|r| {
            if let Record::Chapter {
                id,
                completed: true,
                ..
            } = r
            {
                Some(*id)
            } else {
                None
            }
        })
        .collect::<BTreeSet<_>>();
    let targets = completed.iter().map(hex::encode).collect::<Vec<_>>();
    let aliases = a
        .repo
        .compatibility_identifiers("chapter", &targets)
        .await
        .map_err(error)?;
    let mut all = vec![];
    for row in rows {
        let chapter: Id = row
            .get::<_, Vec<u8>>(0)
            .try_into()
            .map_err(|_| unavailable())?;
        if !completed.contains(&chapter) {
            continue;
        }
        let series: Vec<u8> = row.get(1);
        let alias = aliases.get("chapter", &hex::encode(chapter));
        all.push(json!(alias));
        for (key, wanted) in &requested {
            if wanted.as_slice() == series.as_slice() {
                groups
                    .get_mut(key)
                    .unwrap()
                    .as_array_mut()
                    .unwrap()
                    .push(json!(alias));
            }
        }
    }
    objects.verify_view(&view).await.map_err(error)?;
    bounded(
        json!({"result":"ok","data":if grouped{Value::Object(groups)}else{Value::Array(all)},"nostr":{"head":view.event.id,"root":hex::encode(view.root_hash),"scope":"locally_hosted_chapters"}}),
    )
}
pub(super) async fn read_markers(
    State(a): State<Shared>,
    identity_arg: Option<Extension<ReadIdentity>>,
    Path(key): Path<String>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    let publisher = identity(identity_arg)?;
    let f = fields(raw.as_deref(), &["head"])?;
    let manga = target(&a, "manga", &key).await?;
    read_for(
        &a,
        publisher,
        vec![(key, manga)],
        f.get("head").map(String::as_str),
        false,
    )
    .await
}
pub(super) async fn read_markers_many(
    State(a): State<Shared>,
    identity_arg: Option<Extension<ReadIdentity>>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Value>, ApiError> {
    let publisher = identity(identity_arg)?;
    let raw = raw.unwrap_or_default();
    if raw.len() > 32768 {
        return Err(bad("query byte budget"));
    }
    let mut ids = BTreeSet::new();
    let mut grouped = None;
    let mut head = None;
    for (k, v) in url::form_urlencoded::parse(raw.as_bytes()).into_owned() {
        match k.as_str() {
            "ids[]" => {
                if ids.len() >= 100 {
                    return Err(bad("manga ID budget"));
                }
                ids.insert(v);
            }
            "grouped" if grouped.is_none() => {
                grouped = Some(v.parse::<bool>().map_err(|_| bad("grouped boolean"))?)
            }
            "head" if head.is_none() => {
                id(&v).map_err(|_| bad("head identity"))?;
                head = Some(v);
            }
            _ => return Err(bad("unsupported or duplicate query parameter")),
        }
    }
    if ids.is_empty() {
        return Err(bad("manga IDs required"));
    }
    let mut requested = vec![];
    for key in ids {
        let manga = target(&a, "manga", &key).await?;
        requested.push((key, manga));
    }
    read_for(
        &a,
        publisher,
        requested,
        head.as_deref(),
        grouped.unwrap_or(false),
    )
    .await
}
