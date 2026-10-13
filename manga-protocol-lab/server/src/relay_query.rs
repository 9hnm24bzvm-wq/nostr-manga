use crate::{
    auth::{self, Event},
    db::{Id, Repository},
};
use anyhow::{Result, ensure};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug)]
pub struct RelayFilter {
    pub ids: Option<Vec<Id>>,
    pub authors: Option<Vec<Id>>,
    pub kinds: Option<Vec<i32>>,
    pub tags: BTreeMap<String, Vec<String>>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub limit: i64,
}
impl RelayFilter {
    pub fn parse(value: &Value) -> Result<Self> {
        let o = value
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("filter object required"))?;
        ensure!(o.len() <= 20, "filter field budget");
        let mut f = Self {
            ids: None,
            authors: None,
            kinds: None,
            tags: BTreeMap::new(),
            since: None,
            until: None,
            limit: crate::protocol_limits::RELAY_LIMIT,
        };
        for (key, value) in o {
            match key.as_str() {
                "ids" | "authors" => {
                    let values = value
                        .as_array()
                        .ok_or_else(|| anyhow::anyhow!("filter array required"))?;
                    ensure!(
                        !values.is_empty() && values.len() <= 256,
                        "filter array budget"
                    );
                    let mut ids = Vec::new();
                    for v in values {
                        let text = v
                            .as_str()
                            .ok_or_else(|| anyhow::anyhow!("hex ID required"))?;
                        ensure!(auth::valid_hash(text), "exact lowercase ID required");
                        ids.push(hex::decode(text)?.try_into().unwrap());
                    }
                    if key == "ids" {
                        f.ids = Some(ids)
                    } else {
                        f.authors = Some(ids)
                    }
                }
                "kinds" => {
                    let values = value
                        .as_array()
                        .ok_or_else(|| anyhow::anyhow!("kind array required"))?;
                    ensure!(
                        !values.is_empty() && values.len() <= 64,
                        "kind array budget"
                    );
                    let mut kinds = Vec::new();
                    for v in values {
                        let k = v
                            .as_u64()
                            .ok_or_else(|| anyhow::anyhow!("unsigned kind required"))?;
                        ensure!(k <= 65535, "kind bound");
                        kinds.push(k as i32);
                    }
                    f.kinds = Some(kinds);
                }
                "since" | "until" | "limit" => {
                    let n = value
                        .as_i64()
                        .ok_or_else(|| anyhow::anyhow!("integer bound required"))?;
                    ensure!(n >= 0, "negative bound");
                    match key.as_str() {
                        "since" => f.since = Some(n),
                        "until" => f.until = Some(n),
                        _ => f.limit = n.min(crate::protocol_limits::RELAY_LIMIT),
                    }
                }
                _ => {
                    ensure!(
                        key.len() == 2
                            && key.starts_with('#')
                            && key.as_bytes()[1].is_ascii_alphabetic(),
                        "unsupported filter field"
                    );
                    let values = value
                        .as_array()
                        .ok_or_else(|| anyhow::anyhow!("tag array required"))?;
                    ensure!(
                        !values.is_empty() && values.len() <= 128,
                        "tag filter budget"
                    );
                    let mut list = Vec::new();
                    for v in values {
                        let text = v
                            .as_str()
                            .ok_or_else(|| anyhow::anyhow!("tag text required"))?;
                        ensure!(text.len() <= 1024, "tag value budget");
                        if matches!(key.as_str(), "#e" | "#p") {
                            ensure!(auth::valid_hash(text), "exact tag identifier required");
                        }
                        list.push(text.into());
                    }
                    f.tags.insert(key[1..].into(), list);
                }
            }
        }
        ensure!(
            !matches!((f.since,f.until),(Some(a),Some(b)) if a>b),
            "reversed timestamp bounds"
        );
        Ok(f)
    }
    pub fn matches(&self, e: &Event) -> bool {
        let id = hex::decode(&e.id).unwrap_or_default();
        let author = hex::decode(&e.pubkey).unwrap_or_default();
        self.ids
            .as_ref()
            .is_none_or(|v| v.iter().any(|x| x.as_slice() == id))
            && self
                .authors
                .as_ref()
                .is_none_or(|v| v.iter().any(|x| x.as_slice() == author))
            && self
                .kinds
                .as_ref()
                .is_none_or(|v| v.contains(&(e.kind as i32)))
            && self.since.is_none_or(|v| e.created_at >= v as u64)
            && self.until.is_none_or(|v| e.created_at <= v as u64)
            && self
                .tags
                .iter()
                .all(|(tag, values)| e.values(tag).iter().any(|v| values.iter().any(|x| x == v)))
    }
}
impl Repository {
    pub async fn relay_event(&self, id: Id) -> Result<Option<Event>> {
        self.pool
            .get()
            .await?
            .query_opt(
                "SELECT raw_event FROM relay_visible_events WHERE id=$1",
                &[&id.as_slice()],
            )
            .await?
            .map(|r| serde_json::from_value(r.get(0)).map_err(Into::into))
            .transpose()
    }
    pub async fn relay_history(&self, filters: &[RelayFilter]) -> Result<Vec<Event>> {
        ensure!(
            !filters.is_empty() && filters.len() <= crate::protocol_limits::RELAY_FILTERS,
            "filter count budget"
        );
        let mut c = self.pool.get().await?;
        let tx = c
            .build_transaction()
            .read_only(true)
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await?;
        tx.batch_execute("SET LOCAL statement_timeout='500ms';SET LOCAL work_mem='4MB'")
            .await?;
        let mut ids = BTreeSet::new();
        let mut bytes = 0i64;
        for f in filters {
            if f.limit == 0 {
                continue;
            }
            let id_values = f
                .ids
                .as_ref()
                .map(|v| v.iter().map(|x| x.to_vec()).collect::<Vec<_>>());
            let authors = f
                .authors
                .as_ref()
                .map(|v| v.iter().map(|x| x.to_vec()).collect::<Vec<_>>());
            let mut args: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = vec![
                Box::new(id_values),
                Box::new(authors),
                Box::new(f.kinds.clone()),
                Box::new(f.since),
                Box::new(f.until),
                Box::new(f.limit),
            ];
            let mut sql = String::from(
                "SELECT e.id,octet_length(e.raw_event::text)::bigint FROM relay_visible_events e WHERE ($1::bytea[] IS NULL OR e.id=ANY($1)) AND ($2::bytea[] IS NULL OR e.publisher=ANY($2)) AND ($3::integer[] IS NULL OR e.kind=ANY($3)) AND ($4::bigint IS NULL OR e.created_at>=$4) AND ($5::bigint IS NULL OR e.created_at<=$5)",
            );
            for (tag, values) in &f.tags {
                args.push(Box::new(tag.clone()));
                let k = args.len();
                args.push(Box::new(values.clone()));
                let v = args.len();
                sql.push_str(&format!(" AND EXISTS(SELECT 1 FROM relay_tags t WHERE t.event_id=e.id AND t.name=${k} AND t.value_hash IN (SELECT md5(x) FROM unnest(${v}::text[]) x)) AND EXISTS(SELECT 1 FROM jsonb_array_elements(e.raw_event->'tags') t WHERE t->>0=${k} AND t->>1=ANY(${v}))"));
            }
            sql.push_str(" ORDER BY e.created_at DESC,e.id LIMIT $6");
            let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
                args.iter().map(|v| v.as_ref() as _).collect();
            for row in tx.query(&sql, &params).await? {
                let id: Vec<u8> = row.get(0);
                if ids.insert(id) {
                    bytes += row.get::<_, i64>(1) + 128;
                    ensure!(bytes <= 4 * 1024 * 1024, "historical response byte budget");
                }
            }
        }
        let ids: Vec<Vec<u8>> = ids.into_iter().collect();
        let rows=tx.query("SELECT raw_event FROM relay_visible_events WHERE id=ANY($1) ORDER BY created_at DESC,id",&[&ids]).await?;
        let events = rows
            .into_iter()
            .map(|r| serde_json::from_value(r.get(0)))
            .collect::<std::result::Result<Vec<Event>, _>>()?;
        tx.commit().await?;
        Ok(events)
    }
}
