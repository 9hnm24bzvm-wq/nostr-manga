//! Public integration seam for instance-local catalogue projections.
//!
//! This module owns the protocol-side visibility query. Consumers never turn a
//! candidate, pending projection, quarantined original, or removed event into a
//! platform publication. A repeating keyset sweep is deliberately used instead
//! of an outbox allocation-ID watermark, which can skip late commits.
use crate::{
    auth::Event,
    catalogue::VISIBLE,
    db::{Id, Repository},
};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
pub struct SourcePublication {
    pub event: Event,
    pub source_version: i64,
    pub visible: bool,
    pub metadata: Option<serde_json::Value>,
    pub compatibility_id: String,
    /// Preferred legacy aliases, keyed as kind:target.
    pub related_ids: BTreeMap<String, String>,
}
#[derive(Debug)]
pub struct SourcePage {
    pub entries: Vec<SourcePublication>,
    /// Individually invalid/over-budget rows are explicit, durable downstream
    /// rejections, not an unreadable page that permanently wedges its cursor.
    pub rejected: Vec<SourceRejection>,
    pub next_after: Option<Id>,
    pub exhausted: bool,
}

#[derive(Debug)]
pub struct SourceRejection {
    pub event_id: String,
    pub source_version: i64,
    pub reason: String,
}
fn byte_bounded_prefix(sizes: &[i64], budget: i64) -> (usize, Vec<usize>, Vec<usize>) {
    let mut consumed = 0;
    let mut bytes = 0;
    let mut selected = Vec::new();
    let mut rejected = Vec::new();
    for (index, &size) in sizes.iter().enumerate() {
        if size <= 0 || size > budget {
            rejected.push(index);
            consumed = index + 1;
            continue;
        }
        if bytes + size > budget {
            break;
        }
        bytes += size;
        selected.push(index);
        consumed = index + 1;
    }
    (consumed, selected, rejected)
}

/// Safe SQL predicate for a derived text event-ID column. The only accepted
/// inputs are fixed, unquoted SQL identifiers with an optional table alias.
/// Protocol visibility rules remain defined by the original catalogue module.
pub fn visibility_sql(column: &str) -> Result<String> {
    ensure!(
        !column.is_empty()
            && column.len() <= 128
            && column.split('.').count() <= 2
            && column.split('.').all(|part| !part.is_empty()
                && part
                    .bytes()
                    .enumerate()
                    .all(|(i, b)| b.is_ascii_alphabetic()
                        || b == b'_'
                        || i > 0 && b.is_ascii_digit())),
        "invalid source column"
    );
    Ok(format!(
        "({column} IS NULL OR EXISTS(SELECT 1 FROM events e WHERE encode(e.id,'hex')={column} AND e.kind IN(1001,1002) AND {VISIBLE}))"
    ))
}

impl Repository {
    pub async fn platform_catalogue_page(
        &self,
        after: Option<Id>,
        limit: i64,
    ) -> Result<SourcePage> {
        ensure!((1..=64).contains(&limit), "platform catalogue sweep bounds");
        let after = after.map(|id| id.to_vec());
        let mut client = self.pool.get().await?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await?;
        tx.batch_execute("SET LOCAL statement_timeout='2000ms'; SET LOCAL lock_timeout='500ms'")
            .await?;
        // Preflight only identities and lengths before loading any JSON bodies.
        // The bounded prefix adapts to bytes, so valid large siblings cannot wedge
        // a fixed count page. A single over-budget row is rejected explicitly.
        let candidates=tx.query("SELECT e.id,e.version,octet_length(e.raw_event::text)::bigint+COALESCE(octet_length(c.metadata::text),0)::bigint FROM events e LEFT JOIN catalogue_metadata c ON c.event_id=e.id WHERE e.kind IN(1001,1002) AND ($1::bytea IS NULL OR e.id>$1) ORDER BY e.id LIMIT $2", &[&after,&limit]).await?;
        let sizes: Vec<i64> = candidates.iter().map(|r| r.get(2)).collect();
        let (consumed, selected_positions, oversized_positions) =
            byte_bounded_prefix(&sizes, 2 * 1048576);
        let selected: Vec<Vec<u8>> = selected_positions
            .iter()
            .map(|&i| candidates[i].get(0))
            .collect();
        let mut rejected = Vec::new();
        for index in oversized_positions {
            rejected.push(SourceRejection {
                event_id: hex::encode(candidates[index].get::<_, Vec<u8>>(0)),
                source_version: candidates[index].get(1),
                reason: "Original row exceeds the catalogue projection byte budget".into(),
            });
        }
        let rows=tx.query(&format!("SELECT e.id,e.raw_event,e.version,({VISIBLE}) AND c.event_id IS NOT NULL,c.metadata,COALESCE((SELECT id FROM compatibility_ids i WHERE i.kind=CASE WHEN e.kind=1002 THEN 'manga' ELSE 'chapter' END AND i.target=encode(e.id,'hex') AND i.preferred LIMIT 1),'') FROM events e LEFT JOIN catalogue_metadata c ON c.event_id=e.id WHERE e.id=ANY($1) ORDER BY e.id"),&[&selected]).await?;
        let mut entries = Vec::with_capacity(rows.len());
        let next_after = if consumed > 0 {
            Some(
                candidates[consumed - 1]
                    .get::<_, Vec<u8>>(0)
                    .try_into()
                    .ok()
                    .context("stored source identity")?,
            )
        } else {
            None
        };
        for row in &rows {
            let source_id = hex::encode(row.get::<_, Vec<u8>>(0));
            let event: Event = match serde_json::from_value::<Event>(row.get(1)) {
                Ok(event)
                    if event.id == source_id
                        && matches!(event.kind, 1001 | 1002)
                        && event.verify().is_ok() =>
                {
                    event
                }
                _ => {
                    rejected.push(SourceRejection {
                        event_id: source_id,
                        source_version: row.get(2),
                        reason: "Invalid stored original publication signature or identity".into(),
                    });
                    continue;
                }
            };
            let visible: bool = row.get(3);
            let alias: String = row.get(5);
            let kind = if event.kind == 1002 {
                "manga"
            } else {
                "chapter"
            };
            let metadata: Option<serde_json::Value> = if visible { row.get(4) } else { None };
            let mut targets = Vec::new();
            if let Some(metadata) = &metadata {
                if metadata["cover"].is_object() {
                    targets.push(format!("cover:{}", event.id));
                }
                if let Some(categories) = metadata["categories"].as_array() {
                    for category in categories.iter().filter_map(serde_json::Value::as_str) {
                        targets.push(format!("tag:{category}"));
                    }
                }
                if let Some(credits) = metadata["credits"].as_array() {
                    for index in 0..credits.len() {
                        targets.push(format!("author:{}:{index}", event.id));
                    }
                }
            }
            if targets.len() > 385 {
                rejected.push(SourceRejection {
                    event_id: source_id,
                    source_version: row.get(2),
                    reason: "Original related compatibility identity budget exceeded".into(),
                });
                continue;
            }
            let mut related_ids = BTreeMap::new();
            if !targets.is_empty() {
                for alias in tx.query("SELECT kind||':'||target,id FROM compatibility_ids WHERE preferred AND kind||':'||target=ANY($1)",&[&targets]).await?{related_ids.insert(alias.get(0),alias.get(1));}
            }
            entries.push(SourcePublication {
                related_ids,
                compatibility_id: if alias.is_empty() {
                    crate::mangadex_compat::uuid(kind, &event.id)
                } else {
                    alias
                },
                event,
                source_version: row.get(2),
                visible,
                metadata,
            });
        }
        tx.commit().await?;
        Ok(SourcePage {
            entries,
            rejected,
            next_after,
            exhausted: consumed == candidates.len() && candidates.len() < limit as usize,
        })
    }
}

#[derive(Debug)]
pub struct VisibleResource {
    pub event: Event,
    pub compatibility_id: String,
}

/// Original generation and signed body are captured under the caller's source
/// row lock. Keep that transaction open until its current native request has
/// been persisted/submitted; restoration cannot silently retarget the intent.
pub async fn lock_deletion_source<C: tokio_postgres::GenericClient + Sync>(
    client: &C,
    target: Id,
) -> Result<Option<(Event, i64)>> {
    let Some(row)=client.query_opt(&format!("SELECT e.raw_event,e.version FROM events e WHERE e.id=$1 AND e.kind=1001 AND {VISIBLE} FOR SHARE OF e"),&[&target.as_slice()]).await? else {return Ok(None)};
    let event: Event = serde_json::from_value(row.get(0))?;
    event.verify().map_err(anyhow::Error::msg)?;
    let generation: i64 = row.get(1);
    ensure!(
        event.id == hex::encode(target) && event.kind == 1001 && generation > 0,
        "invalid current original deletion generation"
    );
    Ok(Some((event, generation)))
}
/// Batch signed-resource mapping uses current original catalogue policy for the
/// resource and the immutable parent, with a bounded signed-body budget.
pub async fn visible_resource_ids<C: tokio_postgres::GenericClient + Sync>(
    client: &C,
    ids: &[Id],
) -> Result<Vec<VisibleResource>> {
    ensure!(ids.len() <= 256, "resource mapping batch budget");
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let ids: Vec<Vec<u8>> = ids.iter().map(|id| id.to_vec()).collect();
    let parent_visible = VISIBLE.replace("e.", "parent.");
    let predicate = format!(
        "e.id=ANY($1) AND e.kind IN(1001,1002) AND {VISIBLE} AND (e.kind=1002 OR EXISTS(SELECT 1 FROM events parent WHERE parent.id=e.series AND parent.kind=1002 AND {parent_visible}))"
    );
    let bytes:i64=client.query_one(&format!("SELECT COALESCE(sum(octet_length(e.raw_event::text)),0)::bigint FROM events e WHERE {predicate}"),&[&ids]).await?.get(0);
    ensure!(bytes <= 2 * 1048576, "resource mapping signed-body budget");
    let rows=client.query(&format!("SELECT e.id,e.raw_event,COALESCE((SELECT id FROM compatibility_ids i WHERE i.kind=CASE WHEN e.kind=1002 THEN 'manga' ELSE 'chapter' END AND i.target=encode(e.id,'hex') AND i.preferred LIMIT 1),'') FROM events e WHERE {predicate} ORDER BY e.id"),&[&ids]).await?;
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let event: Event = serde_json::from_value(row.get(1))?;
        event.verify().map_err(anyhow::Error::msg)?;
        ensure!(
            event.id == hex::encode(row.get::<_, Vec<u8>>(0)) && matches!(event.kind, 1001 | 1002),
            "stored resource mapping identity mismatch"
        );
        let alias: String = row.get(2);
        let kind = if event.kind == 1002 {
            "manga"
        } else {
            "chapter"
        };
        let compatibility_id = if alias.is_empty() {
            crate::mangadex_compat::uuid(kind, &event.id)
        } else {
            alias
        };
        ensure!(
            crate::mangadex_compat::valid_uuid(&compatibility_id),
            "invalid original compatibility alias"
        );
        result.push(VisibleResource {
            event,
            compatibility_id,
        });
    }
    Ok(result)
}

/// Current healthy signed series acceptance with genuinely incomplete projection.
/// This is a private owner receipt seam, never a public visibility grant.
pub async fn pending_series_receipt<C: tokio_postgres::GenericClient + Sync>(
    client: &C,
    series: Id,
) -> Result<Option<Event>> {
    let Some(row)=client.query_opt("SELECT e.raw_event FROM events e WHERE e.id=$1 AND e.kind=1002 AND e.state='hosted' AND NOT EXISTS(SELECT 1 FROM publication_blobs p JOIN quarantined_blobs q ON q.hash=p.hash AND q.generation=p.generation WHERE p.event_id=e.id) AND EXISTS(SELECT 1 FROM backend_outbox o WHERE o.event_id=e.id AND o.completed_at IS NULL)",&[&series.as_slice()]).await? else{return Ok(None)};
    let event: Event = serde_json::from_value(row.get(0))?;
    event.verify().map_err(anyhow::Error::msg)?;
    ensure!(
        event.kind == 1002 && event.id == hex::encode(series),
        "stored series receipt identity mismatch"
    );
    Ok(Some(event))
}

/// Owner receipt eligibility is narrower than historical acceptance: both real
/// immutable resources must still be hosted with healthy original generations,
/// the signed chapter must refer to this exact parent, and only incomplete
/// original projection can explain a pending response. This never authorizes a
/// public read or treats a removed publication as a pending receipt.
pub async fn pending_chapter_receipt<C: tokio_postgres::GenericClient + Sync>(
    client: &C,
    chapter: Id,
    parent: Id,
) -> Result<Option<Event>> {
    let row=client.query_opt("SELECT c.raw_event,s.raw_event FROM events c JOIN events s ON s.id=c.series WHERE c.id=$1 AND s.id=$2 AND c.kind=1001 AND s.kind=1002 AND c.state='hosted' AND s.state='hosted' AND NOT EXISTS(SELECT 1 FROM publication_blobs p JOIN quarantined_blobs q ON q.hash=p.hash AND q.generation=p.generation WHERE p.event_id IN(c.id,s.id)) AND EXISTS(SELECT 1 FROM backend_outbox o WHERE o.event_id IN(c.id,s.id) AND o.completed_at IS NULL)", &[&chapter.as_slice(),&parent.as_slice()]).await?;
    let Some(row) = row else { return Ok(None) };
    let chapter_event: Event = serde_json::from_value(row.get(0))?;
    let parent_event: Event = serde_json::from_value(row.get(1))?;
    chapter_event.verify().map_err(anyhow::Error::msg)?;
    parent_event.verify().map_err(anyhow::Error::msg)?;
    ensure!(
        chapter_event.kind == 1001
            && parent_event.kind == 1002
            && chapter_event.id == hex::encode(chapter)
            && parent_event.id == hex::encode(parent),
        "stored pending receipt identity mismatch"
    );
    Ok(Some(chapter_event))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn byte_pages_advance_large_valid_siblings_and_explicitly_reject_single_invalid_rows() {
        assert_eq!(
            byte_bounded_prefix(&[1_100_000, 1_100_000, 10], 2_097_152),
            (1, vec![0], vec![])
        );
        assert_eq!(
            byte_bounded_prefix(&[1_100_000, 10], 2_097_152),
            (2, vec![0, 1], vec![])
        );
        assert_eq!(
            byte_bounded_prefix(&[3_000_000, 10, 0, 20], 2_097_152),
            (4, vec![1, 3], vec![0, 2])
        );
        assert_eq!(
            byte_bounded_prefix(&[2_097_152], 2_097_152),
            (1, vec![0], vec![])
        );
    }
    #[test]
    fn visibility_is_owned_by_protocol_and_cannot_inject_sql() {
        let sql = visibility_sql("c.protocol_event_id").unwrap();
        assert!(sql.contains("quarantined_blobs"));
        assert!(sql.contains("completed_at IS NULL"));
        assert!(sql.contains("e.state='hosted'"));
        for column in [
            "",
            "c.id;DELETE FROM events",
            "c.id OR TRUE",
            "c..id",
            "1id",
            "a.b.c",
            "c.\"id\"",
        ] {
            assert!(visibility_sql(column).is_err());
        }
    }
}

/// Platform read functions use this generated canonical policy without copying
/// protocol authority rules into downstream migrations. The function is lazy-
/// planned, so a standalone native platform can safely lack protocol tables.
pub fn visibility_function_sql() -> String {
    format!(
        "CREATE OR REPLACE FUNCTION platform.protocol_source_is_visible(source_event TEXT) RETURNS BOOLEAN LANGUAGE plpgsql STABLE AS $bridge$ BEGIN IF source_event IS NULL THEN RETURN TRUE; END IF; RETURN EXISTS(SELECT 1 FROM events e WHERE encode(e.id,'hex')=source_event AND e.kind IN(1001,1002) AND {VISIBLE}); EXCEPTION WHEN undefined_table OR undefined_column THEN RETURN FALSE; END; $bridge$;"
    )
}

fn deterministic_json(value: &serde_json::Value) -> Result<ciborium::Value> {
    use ciborium::Value as C;
    Ok(match value {
        serde_json::Value::Null => C::Null,
        serde_json::Value::Bool(v) => C::Bool(*v),
        serde_json::Value::String(v) => C::Text(v.clone()),
        serde_json::Value::Number(v) => C::Integer(
            v.as_u64()
                .context("manifest integers must be unsigned")?
                .into(),
        ),
        serde_json::Value::Array(values) => C::Array(
            values
                .iter()
                .map(deterministic_json)
                .collect::<Result<_>>()?,
        ),
        serde_json::Value::Object(values) => {
            let mut entries = Vec::new();
            for (key, value) in values {
                let key = C::Text(key.clone());
                let mut encoded = Vec::new();
                ciborium::into_writer(&key, &mut encoded)?;
                entries.push((encoded, key, deterministic_json(value)?));
            }
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            C::Map(entries.into_iter().map(|(_, k, v)| (k, v)).collect())
        }
    })
}
fn encode_value(value: serde_json::Value) -> Result<Vec<u8>> {
    let value = deterministic_json(&value)?;
    let mut bytes = Vec::new();
    ciborium::into_writer(&value, &mut bytes)?;
    crate::manifest::decode(&bytes).map_err(anyhow::Error::msg)?;
    Ok(bytes)
}
/// Typed construction still runs the original canonical parser before staging.
pub fn encode_chapter(manifest: &crate::manifest::Manifest) -> Result<Vec<u8>> {
    let mut value = serde_json::json!({"version":1,"series":manifest.series,"language":manifest.language,"pages":manifest.pages});
    for (key, text) in [
        ("chapter", &manifest.chapter),
        ("volume", &manifest.volume),
        ("title", &manifest.title),
    ] {
        if let Some(text) = text {
            value[key] = serde_json::Value::String(text.clone());
        }
    }
    let bytes = encode_value(value)?;
    crate::manifest::chapter(&bytes).map_err(anyhow::Error::msg)?;
    Ok(bytes)
}
pub fn encode_series(metadata: &crate::series_metadata::SeriesMetadata) -> Result<Vec<u8>> {
    let mut value = serde_json::to_value(metadata)?;
    value["version"] = serde_json::json!(1);
    if metadata.cover.is_some() {
        value["required"] = serde_json::json!([crate::series_metadata::COVER_FEATURE]);
    }
    let bytes = encode_value(value)?;
    crate::series_metadata::parse(&bytes).map_err(anyhow::Error::msg)?;
    Ok(bytes)
}
/// Restrict downstream durable signing to the existing publication contract.
/// Authority and storage grants are still checked by Publications at commit.
pub fn sign_publication_at(
    secret: &str,
    kind: u32,
    manifest: &str,
    created_at: u64,
) -> Result<Event> {
    ensure!(
        matches!(kind, 1001 | 1002),
        "unsupported publication signing kind"
    );
    let event = crate::auth::sign_at(
        secret,
        kind,
        vec![],
        serde_json::json!({"manifest":manifest,"version":1}).to_string(),
        created_at,
    )
    .map_err(anyhow::Error::msg)?;
    crate::publication::pointer(&event)?;
    Ok(event)
}
