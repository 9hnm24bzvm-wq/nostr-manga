//! Sole catalogue writer for verified original-protocol projections.
//! Native editorial ownership is never inferred from a Nostr public key.
use super::{
    repository::{CatalogRepository, audit, create_thread},
    validation as v,
};
use crate::error::{ApiError, ApiResult};
use manga_node::{auth, platform_bridge::SourcePublication};
use serde_json::{Value, json};
use tokio_postgres::Transaction;

#[derive(Clone, Debug)]
pub(crate) struct SeriesProjection {
    pub title: String,
    pub language: String,
    pub status: String,
    pub rating: String,
    pub demographic: Option<String>,
    pub year: i32,
    pub metadata: Value,
}
fn optional_text(value: &Value, key: &str, max: usize) -> ApiResult<Option<String>> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text.len() <= max && !text.contains('\0') => {
            Ok(Some(text.clone()))
        }
        _ => Err(ApiError::bad_request(format!(
            "Protocol {key} cannot be represented by this platform"
        ))),
    }
}
fn language(text: &str) -> ApiResult<String> {
    // BCP47 identities stay intact. Unknown languages are retained and displayed
    // as unknown; they are never relabelled as a different language.
    if !(2..=12).contains(&text.len())
        || !text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(ApiError::bad_request(
            "Protocol language exceeds the platform language contract",
        ));
    }
    Ok(text.to_ascii_lowercase())
}
pub(crate) fn project_series(metadata: &Value) -> ApiResult<SeriesProjection> {
    let titles = metadata["titles"]
        .as_object()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| ApiError::bad_request("Protocol series has no titles"))?;
    let title = titles
        .get("en")
        .or_else(|| titles.values().next())
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty() && s.len() <= 500)
        .ok_or_else(|| ApiError::bad_request("Protocol title exceeds the platform title contract"))?
        .to_owned();
    let mut defaults = Vec::<&str>::new();
    let lang = match optional_text(metadata, "originalLanguage", 64)? {
        Some(l) => language(&l)?,
        None => {
            defaults.push("language");
            "und".into()
        }
    };
    let status = optional_text(metadata, "status", 64)?.unwrap_or_default();
    let status = if ["ongoing", "completed", "hiatus", "cancelled"].contains(&status.as_str()) {
        status
    } else {
        defaults.push("status");
        "ongoing".into()
    };
    // Missing or unknown source classification is conservatively adult-rated.
    let rating = optional_text(metadata, "contentRating", 64)?.unwrap_or_default();
    let rating = if ["safe", "suggestive", "erotica", "pornographic"].contains(&rating.as_str()) {
        rating
    } else {
        defaults.push("content_rating");
        "pornographic".into()
    };
    let demographic = optional_text(metadata, "publicationDemographic", 64)?
        .filter(|s| ["shounen", "shoujo", "josei", "seinen", "none"].contains(&s.as_str()));
    let year = metadata["year"].as_u64().unwrap_or(0);
    if year > 9999 {
        return Err(ApiError::bad_request("Protocol year cannot be represented"));
    }
    let mut alt = serde_json::Map::new();
    for (lang, title) in titles {
        if let Some(title) = title.as_str() {
            alt.entry(lang.clone())
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .unwrap()
                .push(json!(title));
        }
    }
    if let Some(alts) = metadata["altTitles"].as_array() {
        for localized in alts {
            if let Some(localized) = localized.as_object() {
                for (lang, title) in localized {
                    if let Some(title) = title.as_str() {
                        alt.entry(lang.clone())
                            .or_insert_with(|| json!([]))
                            .as_array_mut()
                            .unwrap()
                            .push(json!(title));
                    }
                }
            }
        }
    }
    let description = metadata["description"]
        .as_object()
        .and_then(|m| m.get("en").or_else(|| m.values().next()))
        .and_then(Value::as_str)
        .unwrap_or("");
    Ok(SeriesProjection {
        title,
        language: lang,
        status,
        rating,
        demographic,
        year: year as i32,
        metadata: json!({"alt_titles":alt,"description":description,"protocol_source_metadata":metadata,"protocol_projection_defaults":defaults}),
    })
}
fn alias(source: &SourcePublication, kind: &str, target: &str) -> String {
    source
        .related_ids
        .get(&format!("{kind}:{target}"))
        .cloned()
        .unwrap_or_else(|| manga_node::mangadex_compat::uuid(kind, target))
}
async fn resource_id(
    tx: &tokio_postgres::Transaction<'_>,
    kind: &str,
    target: &str,
    compatibility: &str,
) -> crate::error::ApiResult<String> {
    if !manga_node::mangadex_compat::valid_uuid(compatibility) {
        return Err(crate::error::ApiError::bad_request("Invalid protocol compatibility alias"));
    }
    let id = v::new_id(kind);
    tx.execute("INSERT INTO platform.protocol_resource_ids(kind,source_target,compatibility_id,catalog_id) VALUES($1,$2,$3,$4) ON CONFLICT(kind,source_target) DO NOTHING",&[&kind,&target,&compatibility,&id]).await?;
    let row=tx.query_one("SELECT catalog_id,compatibility_id FROM platform.protocol_resource_ids WHERE kind=$1 AND source_target=$2",&[&kind,&target]).await?;
    let native:String=row.get(0);
    if row.get::<_,String>(1)!=compatibility {
        let verified=tx.query_opt(
            "SELECT a.native_id FROM public.compatibility_ids i LEFT JOIN platform.public_resource_ids a ON a.kind=i.kind AND a.public_id=i.id::uuid WHERE i.kind=$1 AND i.id=$2 AND i.target=$3 FOR SHARE OF i",
            &[&kind,&compatibility,&target],
        ).await?;
        let verified=verified.ok_or_else(|| crate::error::ApiError::conflict("Protocol compatibility alias does not identify this source"))?;
        if let Some(assigned)=verified.get::<_,Option<String>>(0) {
            if assigned!=native { return Err(crate::error::ApiError::conflict("Protocol compatibility alias is already assigned")); }
            return Ok(native);
        }
        tx.query_one("SELECT platform.public_register_resource_alias($1,$2,$3::text::uuid)",&[&kind,&native,&compatibility]).await.map_err(|error| {
            if error.code()==Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION) {
                crate::error::ApiError::conflict("Protocol compatibility alias is already assigned")
            } else { error.into() }
        })?;
    }
    Ok(native)
}

async fn owner(tx: &Transaction<'_>, publisher: &str) -> ApiResult<(String, String)> {
    if let Some(row)=tx.query_opt("SELECT u.id,u.username FROM platform.protocol_identity_bindings b JOIN platform.users u ON u.id=b.user_id WHERE b.publisher=$1 AND NOT u.disabled",&[&publisher]).await? {return Ok((row.get(0),row.get(1)));}
    Ok((
        format!("protocol:{publisher}"),
        format!("protocol:{}", &publisher[..12]),
    ))
}
impl CatalogRepository {
    pub(crate) async fn reject_protocol_source(tx: &Transaction<'_>, event: &str) -> ApiResult<()> {
        if !auth::valid_hash(event) {
            return Err(ApiError::unavailable());
        }
        tx.execute(
            "UPDATE platform.catalog_chapters SET is_unavailable=TRUE WHERE protocol_event_id=$1",
            &[&event],
        )
        .await?;
        tx.execute("UPDATE platform.catalog_manga SET protocol_source_visible=FALSE WHERE protocol_series_id=$1", &[&event]).await?;
        Ok(())
    }
    pub(crate) async fn materialize_protocol(
        tx: &Transaction<'_>,
        source: &SourcePublication,
        media_origin: &str,
    ) -> ApiResult<bool> {
        source
            .event
            .verify()
            .map_err(|_| ApiError::bad_request("Invalid original protocol signature"))?;
        let event = &source.event;
        let kind = match event.kind {
            1002 => "manga",
            1001 => "chapter",
            _ => return Err(ApiError::bad_request("Unsupported protocol publication")),
        };
        // Per-event lock also serializes two instance replicas creating aliases.
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended('platform-protocol:'||$1::text,0))",
            &[&event.id],
        )
        .await?;
        if !source.visible {
            tx.execute("UPDATE platform.catalog_chapters SET is_unavailable=TRUE WHERE protocol_event_id=$1",&[&event.id]).await?;
            // A local moderator tombstone is never cleared by later source sweeps.
            tx.execute("UPDATE platform.catalog_manga SET protocol_source_visible=FALSE WHERE protocol_series_id=$1",&[&event.id]).await?;
            return Ok(false);
        }
        let command = tx.query_opt("SELECT command_id,catalog_id,upload_session_id,native_commit_hash,user_id,native_commit_payload FROM platform.protocol_publication_commands WHERE event_id=$1 AND resource_kind=$2 FOR UPDATE NOWAIT", &[&event.id,&kind]).await.map_err(recovery_error)?;
        if let Some(command) = &command {
            if let Some(session) = command.get::<_, Option<String>>(2) {
                tx.query_opt(
                    "SELECT id FROM platform.upload_sessions WHERE id=$1 FOR UPDATE NOWAIT",
                    &[&session],
                )
                .await
                .map_err(recovery_error)?;
            }
        }
        let metadata = source.metadata.as_ref().ok_or_else(|| {
            ApiError::bad_request("Visible protocol publication lacks canonical metadata")
        })?;
        let id = resource_id(tx, kind, &event.id, &source.compatibility_id).await?;
        let (owner_id, owner_name) = if kind == "chapter" {
            if let Some(command) = &command {
                let user = command.get::<_, String>(4);
                let row = tx
                    .query_one(
                        "SELECT id,username FROM platform.users WHERE id=$1",
                        &[&user],
                    )
                    .await?;
                (row.get(0), row.get(1))
            } else {
                owner(tx, &event.pubkey).await?
            }
        } else {
            owner(tx, &event.pubkey).await?
        };
        let created = i64::try_from(event.created_at)
            .map_err(|_| ApiError::bad_request("Protocol timestamp cannot be represented"))?;
        if kind == "manga" {
            let data = project_series(metadata)?;
            let existing = tx.query_opt("SELECT protocol_series_id,state,version,deleted_at IS NULL FROM platform.catalog_manga WHERE id=$1 FOR UPDATE NOWAIT", &[&id]).await.map_err(recovery_error)?;
            let recovered = if let Some(existing) = existing {
                let associated: Option<String> = existing.get(0);
                if associated.as_deref() == Some(&event.id) {
                    true
                } else if associated.is_none()
                    && command
                        .as_ref()
                        .is_some_and(|c| c.get::<_, String>(1) == id)
                {
                    if let Some(command) = &command {
                        let payload: Option<Value> = command.get(5);
                        if let Some(payload) = payload.filter(|p| p["action"] == "publish_manga") {
                            let expected = payload["submitted_version"]
                                .as_i64()
                                .and_then(|v| i32::try_from(v).ok())
                                .ok_or_else(|| {
                                    ApiError::conflict("Invalid durable title review version")
                                })?;
                            if payload["manga_id"] != id
                                || existing.get::<_, String>(1) != "submitted"
                                || existing.get::<_, i32>(2) != expected
                                || !existing.get::<_, bool>(3)
                            {
                                return Err(ApiError::conflict(
                                    "Native title review changed after source acceptance; explicit reconciliation is required",
                                ));
                            }
                            let next = expected.checked_add(1).ok_or_else(ApiError::unavailable)?;
                            tx.execute("UPDATE platform.catalog_manga SET state='published',published_at=clock_timestamp(),version=$2,updated_at=clock_timestamp() WHERE id=$1",&[&id,&next]).await?;
                            audit(
                                tx,
                                &command.get::<_, String>(4),
                                "manga",
                                &id,
                                "published-source-recovery",
                                Some(expected),
                                Some(next),
                            )
                            .await?;
                        }
                    }
                    Self::associate_protocol_manga(tx, &id, event).await?;
                    tx.execute("UPDATE platform.catalog_manga SET metadata=metadata||$2 WHERE id=$1", &[&id,&json!({"protocol_source_metadata":metadata,"protocol_publisher":event.pubkey,"publication_scope":"original-protocol","publication_status":"locally_hosted","protocol_event_id":event.id})]).await?;
                    true
                } else {
                    return Err(ApiError::conflict(
                        "Native alias is already associated with a different publication",
                    ));
                }
            } else {
                false
            };
            let changed = if recovered {
                0
            } else {
                tx.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,year,language,demographic,publication_status,content_rating,state,metadata,protocol_series_id,created_at,updated_at,published_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,'published',$10,$11,to_timestamp($12::bigint::double precision),to_timestamp($12::bigint::double precision),to_timestamp($12::bigint::double precision)) ON CONFLICT(protocol_series_id) DO NOTHING",&[&id,&owner_id,&owner_name,&data.title,&data.year,&data.language,&data.demographic,&data.status,&data.rating,&data.metadata,&event.id,&created]).await?
            };
            // Source records are immutable. Subsequent sweeps preserve local overlays.
            if changed > 0 {
                create_thread(tx, "manga", &id, &owner_id, &owner_name, &data.title).await?;
                tx.execute("INSERT INTO platform.catalog_manga_counters(manga_id) VALUES($1) ON CONFLICT DO NOTHING",&[&id]).await?;
                if let Some(categories) = metadata["categories"].as_array() {
                    for category in categories.iter().filter_map(Value::as_str) {
                        if category.is_empty() || category.len() > 100 {
                            continue;
                        }
                        let alias = alias(source, "tag", category);
                        let tag = resource_id(tx, "tag", category, &alias).await?;
                        tx.execute("INSERT INTO platform.catalog_tags(id,name,tag_group) VALUES($1,$2,'tag') ON CONFLICT(id) DO NOTHING",&[&tag,&category]).await?;
                        tx.execute("INSERT INTO platform.catalog_manga_tags(manga_id,tag_id) VALUES($1,$2) ON CONFLICT DO NOTHING",&[&id,&tag]).await?;
                    }
                }
                if let Some(credits) = metadata["credits"].as_array() {
                    for (index, credit) in credits.iter().enumerate() {
                        let role = credit["role"].as_str().unwrap_or("");
                        let name = credit["name"].as_str().unwrap_or("");
                        if !["author", "artist"].contains(&role)
                            || name.is_empty()
                            || name.len() > 200
                        {
                            continue;
                        }
                        let target = format!("{}:{index}", event.id);
                        let alias = alias(source, "author", &target);
                        let author = resource_id(tx, "author", &target, &alias).await?;
                        tx.execute("INSERT INTO platform.catalog_authors(id,owner_id,owner_name,name,protocol_event_id) VALUES($1,$2,$3,$4,$5) ON CONFLICT(id) DO NOTHING",&[&author,&owner_id,&owner_name,&name,&event.id]).await?;
                        tx.execute("INSERT INTO platform.catalog_manga_creators(manga_id,author_id,role) VALUES($1,$2,$3) ON CONFLICT DO NOTHING",&[&id,&author,&role]).await?;
                    }
                }
                if metadata["cover"].is_object() {
                    Self::materialize_protocol_cover(
                        tx,
                        source,
                        &id,
                        &owner_id,
                        &owner_name,
                        &data.language,
                    )
                    .await?;
                }
            }
            // Restore source-visibility state only. User removal is represented by
            // deleted_at and intentionally survives every sweep.
            tx.execute("UPDATE platform.catalog_manga SET protocol_source_visible=TRUE,metadata=metadata||jsonb_build_object('publication_scope','original-protocol','publication_status','locally_hosted','protocol_event_id',$1::text) WHERE protocol_series_id=$1",&[&event.id]).await?;
            Self::link_protocol(tx, kind, &id, source).await?;
            if let Some(command) = &command {
                tx.execute("UPDATE platform.protocol_publication_commands SET state='materialized',updated_at=clock_timestamp() WHERE command_id=$1", &[&command.get::<_,String>(0)]).await?;
                tx.execute(
                    "DELETE FROM platform.protocol_publication_originals WHERE command_id=$1",
                    &[&command.get::<_, String>(0)],
                )
                .await?;
            }
            Ok(changed > 0 || recovered)
        } else {
            let series = metadata["series"]
                .as_str()
                .filter(|s| auth::valid_hash(s))
                .ok_or_else(|| ApiError::bad_request("Invalid original series identity"))?;
            let Some(row)=tx.query_opt("SELECT id FROM platform.catalog_manga WHERE protocol_series_id=$1 AND deleted_at IS NULL AND state='published'",&[&series]).await? else{return Err(ApiError::conflict("Original series is not imported yet"));};
            let manga: String = row.get(0);
            let lang = language(metadata["language"].as_str().unwrap_or(""))?;
            let pages = metadata["pages"]
                .as_array()
                .filter(|p| !p.is_empty() && p.len() <= 10000)
                .ok_or_else(|| ApiError::bad_request("Invalid protocol page count"))?;
            let mut data = Vec::with_capacity(pages.len());
            for page in pages {
                let (hash, _, width, height, _) = descriptor(page)?;
                data.push(json!({"name":hash,"dimensions":[width,height]}));
            }
            let title = optional_text(metadata, "title", 500)?;
            let chapter = optional_text(metadata, "chapter", 32)?;
            let volume = optional_text(metadata, "volume", 32)?;
            let payload = json!({"node":media_origin,"data":data,"protocol_source_metadata":metadata,"protocol_event_id":event.id,"protocol_publisher":event.pubkey,"publication_scope":"original-protocol","publication_status":"locally_hosted"});
            let changed=tx.execute("INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,title,volume,chapter,language,metadata,source_id,protocol_event_id,created_at,updated_at,published_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,to_timestamp($12::bigint::double precision),to_timestamp($12::bigint::double precision),to_timestamp($12::bigint::double precision)) ON CONFLICT(protocol_event_id) DO NOTHING",&[&id,&manga,&owner_id,&owner_name,&title,&volume,&chapter,&lang,&payload,&source.compatibility_id,&event.id,&created]).await?;
            if changed > 0 {
                create_thread(
                    tx,
                    "chapter",
                    &id,
                    &owner_id,
                    &owner_name,
                    title.as_deref().unwrap_or("Chapter discussion"),
                )
                .await?;
                tx.execute("INSERT INTO platform.catalog_chapter_counters(chapter_id) VALUES($1) ON CONFLICT DO NOTHING",&[&id]).await?;
                if let Some(row)=tx.query_opt("SELECT group_id FROM platform.protocol_identity_bindings WHERE publisher=$1 AND group_id IS NOT NULL",&[&event.pubkey]).await? {let group:String=row.get(0);tx.execute("INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES($1,$2) ON CONFLICT DO NOTHING",&[&id,&group]).await?;}
            }
            tx.execute("UPDATE platform.catalog_chapters SET is_unavailable=FALSE,metadata=jsonb_set(metadata,'{publication_status}','\"locally_hosted\"') WHERE protocol_event_id=$1 AND deleted_at IS NULL",&[&event.id]).await?;
            Self::link_protocol(tx, kind, &id, source).await?;
            if let Some(command) = &command {
                if command.get::<_, String>(1) != id {
                    return Err(ApiError::conflict("Prepared native alias changed"));
                }
                if let (Some(session), Some(hash)) = (
                    command.get::<_, Option<String>>(2),
                    command.get::<_, Option<String>>(3),
                ) {
                    let session_row=tx.query_opt("SELECT groups FROM platform.upload_sessions WHERE id=$1 AND owner_id=$2 AND state='open' AND chapter_id IS NULL", &[&session,&command.get::<_,String>(4)]).await?;
                    if let Some(session_row) = session_row {
                        let groups: Vec<String> = session_row.get(0);
                        for group in groups {
                            tx.execute("INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) SELECT $1,id FROM platform.catalog_groups WHERE id=$2 AND deleted_at IS NULL ON CONFLICT DO NOTHING", &[&id,&group]).await?;
                        }
                        Self::recover_native_media(tx, &id, &session, pages).await?;
                    }
                    let value: Option<Value> = tx
                        .query_one("SELECT platform.catalog_chapter_json($1)", &[&id])
                        .await?
                        .get(0);
                    let value=value.unwrap_or_else(||json!({"id":id,"manga_id":manga,"node":media_origin,"data":data,"protocol_event_id":event.id,"publication_scope":"original-protocol","publication_status":"accepted_pending_projection","is_unavailable":true,"version":1}));
                    tx.execute("UPDATE platform.upload_sessions SET state='committed',committed_chapter_id=$2,commit_hash=$3,committed_result=$4,version=version+1,updated_at=clock_timestamp() WHERE id=$1 AND owner_id=$5 AND state='open' AND chapter_id IS NULL", &[&session,&id,&hash,&value,&command.get::<_,String>(4)]).await?;
                    Self::release_recovered_session_files(tx, &session).await?;
                }
                tx.execute("UPDATE platform.protocol_publication_commands SET state='materialized',updated_at=clock_timestamp() WHERE command_id=$1", &[&command.get::<_,String>(0)]).await?;
                tx.execute(
                    "DELETE FROM platform.protocol_publication_originals WHERE command_id=$1",
                    &[&command.get::<_, String>(0)],
                )
                .await?;
            }
            Ok(changed > 0)
        }
    }
    /// Reconcile after every source in the page, because a child may sort before
    /// its parent or finish on an earlier page. Both projections must be visible
    /// before replacing an owner-only pending receipt. Work and locks are bounded;
    /// busy native sessions are retried by the next ordinary importer page.
    pub(crate) async fn reconcile_protocol_upload_receipts(tx: &Transaction<'_>) -> ApiResult<usize> {
        let rows = tx.query("SELECT s.id,s.owner_id,s.committed_chapter_id,s.commit_hash FROM platform.upload_sessions s JOIN platform.protocol_publication_commands p ON p.upload_session_id=s.id AND p.user_id=s.owner_id AND p.catalog_id=s.committed_chapter_id AND p.native_commit_hash=s.commit_hash JOIN platform.catalog_chapters c ON c.id=s.committed_chapter_id AND c.protocol_event_id=p.event_id JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE s.state='committed' AND s.committed_result->>'publication_status' IN ('accepted_pending_projection','pending_projection') AND p.state='materialized' AND c.deleted_at IS NULL AND NOT c.is_unavailable AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(c.protocol_event_id) AND platform.protocol_source_is_visible(m.protocol_series_id) ORDER BY s.updated_at,s.id LIMIT 16 FOR UPDATE OF s SKIP LOCKED", &[]).await?;
        let mut updated = 0;
        for row in rows {
            let session: String = row.get(0);
            let owner: String = row.get(1);
            let chapter: String = row.get(2);
            let hash: String = row.get(3);
            let value: Option<Value> = tx.query_one("SELECT platform.catalog_chapter_json($1)", &[&chapter]).await?.get(0);
            let Some(value) = value.filter(|value| value["publication_status"] == "locally_hosted" && value["is_unavailable"] == false) else { continue; };
            updated += tx.execute("UPDATE platform.upload_sessions SET committed_result=$4,version=version+1,updated_at=clock_timestamp() WHERE id=$1 AND owner_id=$5 AND state='committed' AND committed_chapter_id=$2 AND commit_hash=$3 AND committed_result->>'publication_status' IN ('accepted_pending_projection','pending_projection')", &[&session,&chapter,&hash,&value,&owner]).await? as usize;
        }
        Ok(updated)
    }

    async fn recover_native_media(
        tx: &Transaction<'_>,
        chapter: &str,
        session: &str,
        pages: &[Value],
    ) -> ApiResult<()> {
        let mut originals = Vec::with_capacity(pages.len());
        let mut derivatives = Vec::new();
        let mut used = Vec::<String>::new();
        for (position, page) in pages.iter().enumerate() {
            let (hash, mime, width, height, bytes) = descriptor(page)?;
            let row=tx.query_opt("SELECT f.id,o.id,d.name,optimized.id,optimized.width,optimized.height FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id LEFT JOIN platform.upload_file_derivatives d ON d.file_id=f.id LEFT JOIN platform.media_objects optimized ON optimized.id=d.object_id WHERE f.session_id=$1 AND o.digest=$2 AND o.mime=$3 AND o.width=$4 AND o.height=$5 AND o.bytes=$6 AND NOT(f.id=ANY($7)) ORDER BY f.position LIMIT 1", &[&session,&hash,&mime,&width,&height,&bytes,&used]).await?;
            let Some(row) = row else {
                return Ok(());
            };
            used.push(row.get(0));
            originals.push((hash, row.get::<_, String>(1), position as i32));
            if let (Some(name), Some(object), Some(w), Some(h)) = (
                row.get::<_, Option<String>>(2),
                row.get::<_, Option<String>>(3),
                row.get::<_, Option<i32>>(4),
                row.get::<_, Option<i32>>(5),
            ) {
                derivatives.push((
                    super::CatalogPage {
                        name,
                        dimensions: [w, h],
                    },
                    object,
                    position as i32,
                ));
            }
        }
        for (name, object, position) in originals {
            tx.execute("INSERT INTO platform.chapter_pages(chapter_id,name,object_id,position) VALUES($1,$2,$3,$4) ON CONFLICT(chapter_id,position) DO NOTHING", &[&chapter,&name,&object,&position]).await?;
        }
        if derivatives.len() == pages.len() {
            for (page, object, position) in &derivatives {
                tx.execute("INSERT INTO platform.chapter_page_derivatives(chapter_id,name,object_id,position) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING", &[&chapter,&page.name,object,position]).await?;
            }
            Self::install_optimized_pages(
                tx,
                chapter,
                &derivatives
                    .into_iter()
                    .map(|(page, _, _)| page)
                    .collect::<Vec<_>>(),
            )
            .await?;
        }
        Ok(())
    }
    async fn release_recovered_session_files(tx: &Transaction<'_>, session: &str) -> ApiResult<()> {
        // Only discard consumed temporary associations after a real source event
        // was imported and this exact session was atomically committed.
        let rows=tx.query("SELECT f.object_id FROM platform.upload_files f JOIN platform.upload_sessions s ON s.id=f.session_id WHERE s.id=$1 AND s.state='committed' UNION SELECT d.object_id FROM platform.upload_file_derivatives d JOIN platform.upload_files f ON f.id=d.file_id JOIN platform.upload_sessions s ON s.id=f.session_id WHERE s.id=$1 AND s.state='committed'", &[&session]).await?;
        let objects: Vec<String> = rows.into_iter().map(|r| r.get(0)).collect();
        tx.execute("DELETE FROM platform.upload_files f USING platform.upload_sessions s WHERE f.session_id=s.id AND s.id=$1 AND s.state='committed'", &[&session]).await?;
        let unused=tx.query("DELETE FROM platform.media_objects o WHERE o.id=ANY($1) AND NOT EXISTS(SELECT 1 FROM platform.upload_files f WHERE f.object_id=o.id) AND NOT EXISTS(SELECT 1 FROM platform.chapter_pages p WHERE p.object_id=o.id) AND NOT EXISTS(SELECT 1 FROM platform.cover_media c WHERE c.object_id=o.id) AND NOT EXISTS(SELECT 1 FROM platform.upload_file_derivatives d WHERE d.object_id=o.id) AND NOT EXISTS(SELECT 1 FROM platform.chapter_page_derivatives d WHERE d.object_id=o.id) AND NOT EXISTS(SELECT 1 FROM platform.protocol_publication_originals pin WHERE pin.object_id=o.id) RETURNING storage_key", &[&objects]).await?;
        for row in unused {
            tx.execute(
                "INSERT INTO platform.media_cleanup(storage_key) VALUES($1) ON CONFLICT DO NOTHING",
                &[&row.get::<_, String>(0)],
            )
            .await?;
        }
        Ok(())
    }
    async fn link_protocol(
        tx: &Transaction<'_>,
        kind: &str,
        id: &str,
        source: &SourcePublication,
    ) -> ApiResult<()> {
        let version = source.source_version.to_string();
        tx.execute("INSERT INTO platform.catalog_protocol_links(kind,catalog_id,protocol_event_id,source_version) VALUES($1,$2,$3,$4) ON CONFLICT(kind,catalog_id) DO UPDATE SET source_version=EXCLUDED.source_version WHERE platform.catalog_protocol_links.protocol_event_id=EXCLUDED.protocol_event_id",&[&kind,&id,&source.event.id,&version]).await?;
        Ok(())
    }
    async fn materialize_protocol_cover(
        tx: &Transaction<'_>,
        source: &SourcePublication,
        manga: &str,
        owner: &str,
        name: &str,
        language: &str,
    ) -> ApiResult<()> {
        let (hash, mime, width, height, size) =
            descriptor(&source.metadata.as_ref().unwrap()["cover"])?;
        let alias = alias(source, "cover", &source.event.id);
        let id = resource_id(tx, "cover", &source.event.id, &alias).await?;
        let ext = manga_node::manifest::extension(&mime);
        if !["jpg", "png", "webp"].contains(&ext) {
            return Err(ApiError::bad_request(
                "Protocol cover MIME cannot be represented",
            ));
        }
        let dimensions = vec![width, height];
        tx.execute("INSERT INTO platform.catalog_covers(id,manga_id,uploader_id,uploader_name,ext,dimensions,is_main,language,protocol_event_id) VALUES($1,$2,$3,$4,$5,$6,TRUE,$7,$8) ON CONFLICT(id) DO NOTHING",&[&id,&manga,&owner,&name,&ext,&dimensions,&language,&source.event.id]).await?;
        tx.execute("INSERT INTO platform.protocol_cover_sources(cover_id,event_id,blob_hash,mime,size) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING",&[&id,&source.event.id,&hash,&mime,&size]).await?;
        Ok(())
    }
}
pub(crate) fn descriptor(page: &Value) -> ApiResult<(String, String, i32, i32, i64)> {
    let hash = page["sha256"]
        .as_str()
        .filter(|s| auth::valid_hash(s))
        .ok_or_else(|| ApiError::bad_request("Invalid protocol original identity"))?;
    let mime = page["mime"]
        .as_str()
        .filter(|s| ["image/png", "image/jpeg", "image/webp"].contains(s))
        .ok_or_else(|| ApiError::bad_request("Unsupported protocol original MIME"))?;
    let dimension = |key: &str| {
        page[key]
            .as_u64()
            .and_then(|n| i32::try_from(n).ok())
            .filter(|n| *n > 0 && *n <= 100_000)
            .ok_or_else(|| ApiError::bad_request("Invalid protocol original dimension"))
    };
    let size = page["size"]
        .as_u64()
        .and_then(|n| i64::try_from(n).ok())
        .filter(|n| *n > 0)
        .ok_or_else(|| ApiError::bad_request("Invalid protocol original size"))?;
    Ok((
        hash.to_owned(),
        mime.to_owned(),
        dimension("width")?,
        dimension("height")?,
        size,
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_metadata_is_preserved_and_missing_rating_is_conservative() {
        let input = json!({"titles":{"ja":"合成漫画","en":"Synthetic source"},"altTitles":[{"en":"Other title"}],"description":{"ja":"説明"},"credits":[{"role":"author","name":"Fixture author"}],"categories":["Adventure"],"originalLanguage":"ja","status":"completed","year":2025});
        let projected = project_series(&input).unwrap();
        assert_eq!(projected.title, "Synthetic source");
        assert_eq!(projected.language, "ja");
        assert_eq!(projected.status, "completed");
        assert_eq!(projected.rating, "pornographic");
        assert_eq!(projected.metadata["protocol_source_metadata"], input);
        assert_eq!(
            projected.metadata["alt_titles"]["en"],
            json!(["Synthetic source", "Other title"])
        );
        assert_eq!(
            projected.metadata["protocol_projection_defaults"],
            json!(["content_rating"])
        );
    }
    #[test]
    fn unknown_language_is_never_changed_to_english() {
        let p = project_series(&json!({"titles":{"ja":"Fixture"}})).unwrap();
        assert_eq!(p.language, "und");
        assert!(project_series(
            &json!({"titles":{"en":"Fixture"},"originalLanguage":"this-tag-exceeds-the-platform"})
        )
        .is_err());
    }
    #[test]
    fn original_descriptors_cannot_become_paths_or_fake_hashes() {
        let p = json!({"sha256":"11".repeat(32),"mime":"image/png","width":1,"height":2,"size":3});
        assert_eq!(descriptor(&p).unwrap().2, 1);
        let mut bad = p.clone();
        bad["sha256"] = json!("../../file");
        assert!(descriptor(&bad).is_err());
        bad = p.clone();
        bad["width"] = json!(0);
        assert!(descriptor(&bad).is_err());
        bad = p;
        bad["mime"] = json!("text/html");
        assert!(descriptor(&bad).is_err());
    }
}

impl CatalogRepository {
    /// Used only after the original module accepted a real signed publication.
    /// Existing native aliases survive, including CSV and personal-library links.
    pub(crate) async fn associate_protocol_manga(
        tx: &Transaction<'_>,
        id: &str,
        event: &manga_node::auth::Event,
    ) -> ApiResult<()> {
        event
            .verify()
            .map_err(|_| ApiError::bad_request("Invalid original signature"))?;
        if event.kind != 1002 {
            return Err(ApiError::bad_request(
                "Original series publication required",
            ));
        }
        let row = tx
            .query_one(
                "SELECT protocol_series_id FROM platform.catalog_manga WHERE id=$1 FOR UPDATE",
                &[&id],
            )
            .await?;
        let old: Option<String> = row.get(0);
        if old.as_ref().is_some_and(|old| old != &event.id) {
            return Err(ApiError::conflict(
                "Original series association is immutable",
            ));
        }
        let alias = manga_node::mangadex_compat::uuid("manga", &event.id);
        tx.execute("INSERT INTO platform.protocol_resource_ids(kind,source_target,compatibility_id,catalog_id) VALUES('manga',$1,$2,$3) ON CONFLICT(kind,source_target) DO NOTHING",&[&event.id,&alias,&id]).await?;
        let actual:String=tx.query_one("SELECT catalog_id FROM platform.protocol_resource_ids WHERE kind='manga' AND source_target=$1",&[&event.id]).await?.get(0);
        if actual != id {
            return Err(ApiError::conflict(
                "Original series already has a different native alias",
            ));
        }
        tx.execute(
            "UPDATE platform.catalog_manga SET protocol_series_id=$2 WHERE id=$1",
            &[&id, &event.id],
        )
        .await?;
        tx.execute("INSERT INTO platform.catalog_protocol_links(kind,catalog_id,protocol_event_id,source_version) VALUES('manga',$1,$2,'1') ON CONFLICT(kind,catalog_id) DO NOTHING",&[&id,&event.id]).await?;
        Ok(())
    }
    pub(crate) async fn publish_verified_protocol_chapter(
        tx: &Transaction<'_>,
        record: &super::repository::ChapterPublication,
        receipt: &crate::protocol::publication::PublicationReceipt,
    ) -> ApiResult<Value> {
        receipt
            .event
            .verify()
            .map_err(|_| ApiError::bad_request("Invalid original signature"))?;
        if receipt.event.kind != 1001
            || record.protocol_event_id.as_deref() != Some(&receipt.event.id)
        {
            return Err(ApiError::bad_request(
                "Original chapter publication identity mismatch",
            ));
        }
        let alias = manga_node::mangadex_compat::uuid("chapter", &receipt.event.id);
        let mut local = record.clone();
        local.source_id = Some(alias.clone());
        Self::publish_chapter_after_verified_acceptance(tx, &local, receipt).await?;
        tx.execute("INSERT INTO platform.protocol_resource_ids(kind,source_target,compatibility_id,catalog_id) VALUES('chapter',$1,$2,$3) ON CONFLICT(kind,source_target) DO NOTHING",&[&receipt.event.id,&alias,&record.id]).await?;
        let actual:String=tx.query_one("SELECT catalog_id FROM platform.protocol_resource_ids WHERE kind='chapter' AND source_target=$1",&[&receipt.event.id]).await?.get(0);
        if actual != record.id {
            return Err(ApiError::conflict(
                "Original chapter already has a different native alias",
            ));
        }
        let metadata = json!({"protocol_event_id":receipt.event.id,"protocol_publisher":receipt.event.pubkey,"publication_scope":"original-protocol","publication_status":receipt.state});
        tx.execute("UPDATE platform.catalog_chapters SET protocol_event_id=$2,is_unavailable=$3,metadata=metadata||$4 WHERE id=$1",&[&record.id,&receipt.event.id,&!receipt.locally_visible,&metadata]).await?;
        tx.execute("INSERT INTO platform.catalog_protocol_links(kind,catalog_id,protocol_event_id,source_version) VALUES('chapter',$1,$2,'1') ON CONFLICT(kind,catalog_id) DO NOTHING",&[&record.id,&receipt.event.id]).await?;
        // Pending projection is returned explicitly to this authenticated uploader;
        // public read functions still fail closed until original visibility changes.
        let mut value=tx.query_one("SELECT platform.catalog_chapter_json($1)",&[&record.id]).await?.get::<_,Option<Value>>(0).unwrap_or_else(||json!({"id":record.id,"manga_id":record.manga_id,"is_unavailable":true,"version":1,"node":record.node,"data":record.pages,"title":record.title,"language":record.language,"volume":record.volume,"chapter":record.chapter}));
        value["publication_scope"] = json!("original-protocol");
        value["publication_status"] = json!(receipt.state);
        value["protocol_event_id"] = json!(receipt.event.id);
        Ok(value)
    }
}

fn recovery_error(error: tokio_postgres::Error) -> ApiError {
    if error.code() == Some(&tokio_postgres::error::SqlState::LOCK_NOT_AVAILABLE) {
        ApiError::conflict("Native publication transaction is still active")
    } else {
        error.into()
    }
}
