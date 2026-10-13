//! Bounded live catalogue queries over locally hosted, projected originals.
use crate::read_diagnostics::{Phase, during, phase};
use crate::{
    auth::Event,
    db::{Id, Repository},
    discovery::QueryOptions,
    filter::{Filter, Value},
    upload::UploadStore,
};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use serde_json::json;
use std::{collections::HashSet, sync::Arc, time::Duration};
pub(crate) fn title_values(metadata: &serde_json::Value) -> Vec<String> {
    let mut values = std::collections::BTreeSet::new();
    if let Some(titles) = metadata["titles"].as_object() {
        for v in titles.values() {
            if let Some(v) = v.as_str() {
                values.insert(v.to_owned());
            }
        }
    }
    if let Some(alternatives) = metadata["altTitles"].as_array() {
        for titles in alternatives {
            if let Some(titles) = titles.as_object() {
                for v in titles.values() {
                    if let Some(v) = v.as_str() {
                        values.insert(v.to_owned());
                    }
                }
            }
        }
    }
    values.into_iter().collect()
}
pub struct SeriesQuery {
    pub page: QueryOptions,
    pub title: Option<String>,
    pub search: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub year: Option<i64>,
}
impl SeriesQuery {
    pub fn parse(raw: Option<&str>) -> Result<Self> {
        let raw = raw.unwrap_or("");
        ensure!(
            raw.len() <= crate::protocol_limits::QUERY_BYTES,
            "query budget"
        );
        let mut extra = std::collections::BTreeMap::new();
        let mut base = url::form_urlencoded::Serializer::new(String::new());
        let mut seen = HashSet::new();
        for (k, v) in url::form_urlencoded::parse(raw.as_bytes()) {
            ensure!(seen.insert(k.to_string()), "duplicate query parameter");
            if ["title", "search", "category", "status", "year"].contains(&k.as_ref()) {
                ensure!(
                    !v.is_empty() && v.len() <= 4096 && !v.contains('\0'),
                    "metadata query bounds"
                );
                extra.insert(k.to_string(), v.to_string());
            } else {
                base.append_pair(&k, &v);
            }
        }
        let page = QueryOptions::parse(Some(&base.finish()))?;
        let year = extra.remove("year").map(|s| s.parse::<i64>()).transpose()?;
        ensure!(year.is_none_or(|y| (0..=9999).contains(&y)), "year bounds");
        Ok(Self {
            page,
            title: extra.remove("title"),
            search: extra.remove("search"),
            category: extra.remove("category"),
            status: extra.remove("status"),
            year,
        })
    }
}
#[derive(Serialize)]
pub struct Entry {
    pub event: Event,
    pub metadata: serde_json::Value,
}
#[derive(Serialize)]
pub struct Page {
    pub entries: Vec<Entry>,
    pub offset: i64,
    pub limit: i64,
    pub has_more: bool,
    pub next_offset: Option<i64>,
    pub order: &'static str,
    pub snapshot: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
}
pub(crate) const VISIBLE: &str = "e.state='hosted' AND NOT EXISTS(SELECT 1 FROM publication_blobs p JOIN quarantined_blobs q ON q.hash=p.hash AND q.generation=p.generation WHERE p.event_id=e.id) AND NOT EXISTS(SELECT 1 FROM backend_outbox o WHERE o.event_id=e.id AND o.completed_at IS NULL)";
impl Repository {
    pub async fn catalogue_series(&self, q: SeriesQuery) -> Result<Page> {
        let (filter, mut args) = Filter::new(&q.page.filter)
            .and_then(Filter::compile)
            .map_err(anyhow::Error::msg)?;
        let mut clauses = vec![
            "e.kind=1002".to_owned(),
            VISIBLE.to_owned(),
            format!("({filter})"),
        ];
        for (bound, op) in [(q.page.since, ">="), (q.page.until, "<=")] {
            if let Some(v) = bound {
                args.push(Value::Integer(v));
                clauses.push(format!("e.created_at{op}${}", args.len()));
            }
        }
        for (value, substring) in [(q.title, false), (q.search, true)] {
            if let Some(v) = value {
                args.push(Value::Text(v));
                let n = args.len();
                let pred = if substring {
                    format!("strpos(t.value COLLATE \"C\",${n})>0")
                } else {
                    format!("t.value COLLATE \"C\"=${n}")
                };
                if substring {
                    clauses.push(format!(
                        "EXISTS(SELECT 1 FROM unnest(c.title_values) t(value) WHERE {pred})"
                    ));
                } else {
                    clauses.push(format!("c.title_values @> ARRAY[${n}::text]"));
                }
            }
        }
        if let Some(v) = q.category {
            args.push(Value::Text(v));
            clauses.push(format!(
                "c.metadata @> jsonb_build_object('categories',jsonb_build_array(${}::text))",
                args.len()
            ));
        }
        if let Some(v) = q.status {
            args.push(Value::Text(v));
            clauses.push(format!(
                "c.metadata->>'status' COLLATE \"C\"=${}",
                args.len()
            ));
        }
        if let Some(v) = q.year {
            args.push(Value::Integer(v));
            clauses.push(format!("(c.metadata->>'year')::bigint=${}", args.len()));
        }
        let predicate = clauses.join(" AND ");
        let count_len = args.len();
        args.push(Value::Integer(q.page.limit + 1));
        let lp = args.len();
        args.push(Value::Integer(q.page.offset));
        let op = args.len();
        let order = if q.page.ascending { "ASC" } else { "DESC" };
        let mut conn = during(Phase::PoolCheckout, self.pool.get()).await?;
        let tx = during(
            Phase::DbRoundtrip,
            conn.build_transaction()
                .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
                .read_only(true)
                .start(),
        )
        .await?;
        during(
            Phase::DbRoundtrip,
            tx.batch_execute(
                "SET LOCAL statement_timeout='2000ms'; SET LOCAL lock_timeout='500ms'",
            ),
        )
        .await?;
        let params: Vec<_> = args.iter().map(Value::parameter).collect();
        let total = if q.page.include_total {
            Some(during(Phase::DbRoundtrip, tx.query_one(&format!("SELECT COUNT(*) FROM events e JOIN catalogue_metadata c ON c.event_id=e.id WHERE {predicate}"),&params[..count_len])).await?.get(0))
        } else {
            None
        };
        let bytes:i64=during(Phase::DbRoundtrip, tx.query_one(&format!("SELECT COALESCE(sum(n),0)::bigint FROM (SELECT octet_length(e.raw_event::text)+octet_length(c.metadata::text) AS n FROM events e JOIN catalogue_metadata c ON c.event_id=e.id WHERE {predicate} ORDER BY e.created_at {order},e.id ASC LIMIT ${lp} OFFSET ${op}) page"),&params)).await?.get(0);
        ensure!(
            bytes <= 2 * 1048576,
            "catalogue page byte budget; reduce limit"
        );
        let mut rows=during(Phase::DbRoundtrip, tx.query(&format!("SELECT e.raw_event,c.metadata FROM events e JOIN catalogue_metadata c ON c.event_id=e.id WHERE {predicate} ORDER BY e.created_at {order},e.id ASC LIMIT ${lp} OFFSET ${op}"),&params)).await?;
        let decode = phase(Phase::DecodeResponse);
        let more = rows.len() > q.page.limit as usize;
        rows.truncate(q.page.limit as usize);
        let entries = rows
            .into_iter()
            .map(|r| {
                Ok(Entry {
                    event: serde_json::from_value(r.get(0))?,
                    metadata: r.get(1),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        drop(decode);
        during(Phase::DbRoundtrip, tx.commit()).await?;
        let _response = phase(Phase::DecodeResponse);
        let result = Page {
            entries,
            offset: q.page.offset,
            limit: q.page.limit,
            has_more: more,
            next_offset: if more {
                Some(
                    q.page
                        .offset
                        .checked_add(q.page.limit)
                        .context("offset overflow")?,
                )
            } else {
                None
            },
            order: if q.page.ascending { "asc" } else { "desc" },
            snapshot: false,
            total,
        };
        ensure!(
            serde_json::to_vec(&result)?.len() <= 2 * 1048576,
            "catalogue response budget"
        );
        Ok(result)
    }
    pub async fn catalogue_entry(&self, id: Id) -> Result<Option<Entry>> {
        // The original tail-expression checkout also lived through decoding.
        let client = during(Phase::PoolCheckout, self.pool.get()).await?;
        let row = during(Phase::DbRoundtrip, client.query_opt(&format!("SELECT e.raw_event,c.metadata FROM events e JOIN catalogue_metadata c ON c.event_id=e.id WHERE e.id=$1 AND e.kind IN(1001,1002) AND {VISIBLE}"),&[&id.as_slice()])).await?;
        let _decode = phase(Phase::DecodeResponse);
        row.map(|r| {
            Ok(Entry {
                event: serde_json::from_value(r.get(0))?,
                metadata: r.get(1),
            })
        })
        .transpose()
    }
}
/// Legacy supported manifests are derived lazily in64-row quanta. Invalid old
/// optional metadata is recorded for operator review, never rewritten or unhosted.
pub async fn backfill_page(repo: &Repository, store: &UploadStore) -> Result<usize> {
    let rows=repo.pool.get().await?.query("SELECT e.id,e.kind,e.manifest,e.version FROM events e WHERE e.kind IN(1001,1002) AND e.state='hosted' AND NOT EXISTS(SELECT 1 FROM catalogue_metadata c WHERE c.event_id=e.id AND c.metadata_version>=2) AND NOT EXISTS(SELECT 1 FROM catalogue_index_errors x WHERE x.event_id=e.id) ORDER BY e.id LIMIT 64",&[]).await?;
    let count = rows.len();
    for row in rows {
        let event: Vec<u8> = row.get(0);
        let kind: i32 = row.get(1);
        let manifest: Vec<u8> = row.get(2);
        let version: i64 = row.get(3);
        let hash: Id = manifest
            .clone()
            .try_into()
            .map_err(|_| anyhow::anyhow!("manifest ID"))?;
        let bytes = match store.snapshot(hash, store.owner()).await {
            Ok((_, bytes)) => bytes,
            Err(error)
                if error.is::<tokio_postgres::Error>()
                    || error.is::<deadpool_postgres::PoolError>() =>
            {
                return Err(error);
            }
            Err(_) => {
                repo.pool.get().await?.execute("INSERT INTO catalogue_index_errors(event_id,reason) VALUES($1,'manifest_unavailable') ON CONFLICT DO NOTHING",&[&event]).await?;
                continue;
            }
        };
        let parsed=crate::node::spawn_blocking_owned_scope(move||->Result<serde_json::Value>{if kind==1002{Ok(serde_json::to_value(crate::series_metadata::parse(&bytes).map_err(anyhow::Error::msg)?)?)}else{let m=crate::manifest::chapter(&bytes).map_err(anyhow::Error::msg)?;Ok(json!({"series":m.series,"language":m.language,"chapter":m.chapter,"volume":m.volume,"title":m.title,"pages":m.pages}))}}).await?;
        match parsed {
            Ok(value) => {
                if kind == 1001 {
                    repo.pool.get().await?.execute("UPDATE events SET volume=$2,title=$3 WHERE id=$1 AND version=$4 AND state='hosted'",&[&event,&value["volume"].as_str(),&value["title"].as_str(),&version]).await?;
                }

                repo.pool.get().await?.execute("INSERT INTO catalogue_metadata(event_id,metadata,title_values,metadata_version) SELECT id,$2,$4,2 FROM events WHERE id=$1 AND version=$3 AND state='hosted' ON CONFLICT(event_id) DO UPDATE SET metadata=EXCLUDED.metadata,title_values=EXCLUDED.title_values,metadata_version=2 WHERE catalogue_metadata.metadata_version<2",&[&event,&value,&version,&title_values(&value)]).await?;
            }
            Err(_) => {
                repo.pool.get().await?.execute("INSERT INTO catalogue_index_errors(event_id,reason) VALUES($1,'legacy_metadata_requires_review') ON CONFLICT DO NOTHING",&[&event]).await?;
            }
        }
    }
    Ok(count)
}
pub async fn run(repo: Repository, store: Arc<UploadStore>) {
    loop {
        match backfill_page(&repo, &store).await {
            Ok(64) => tokio::time::sleep(Duration::from_millis(100)).await,
            Ok(_) => tokio::time::sleep(Duration::from_secs(30)).await,
            Err(error) => {
                tracing::warn!(error=%error,"catalogue indexing deferred");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}
