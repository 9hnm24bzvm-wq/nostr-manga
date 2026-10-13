//! Bounded live-offset discovery. Each request reads a consistent DB snapshot;
//! the next request is live and can repeat/skip rows after concurrent mutations.
use crate::read_diagnostics::{Phase, during, phase};
use crate::{
    auth::Event,
    db::Repository,
    filter::{Filter, Value},
};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::HashSet;
#[derive(Clone, Debug)]
pub struct QueryOptions {
    pub filter: String,
    pub offset: i64,
    pub limit: i64,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub ascending: bool,
    pub include_total: bool,
}
impl QueryOptions {
    pub fn parse(raw: Option<&str>) -> Result<Self> {
        let raw = raw.unwrap_or("");
        ensure!(
            raw.len() <= crate::protocol_limits::QUERY_BYTES,
            "query URL budget"
        );
        let mut q = Self {
            filter: String::new(),
            offset: 0,
            limit: crate::protocol_limits::QUERY_LIMIT,
            since: None,
            until: None,
            ascending: false,
            include_total: false,
        };
        let mut seen = HashSet::new();
        for (key, value) in url::form_urlencoded::parse(raw.as_bytes()) {
            ensure!(seen.insert(key.to_string()), "duplicate query parameter");
            match key.as_ref() {
                "filter" => q.filter = value.into_owned(),
                "include_total" => q.include_total = value.parse()?,
                "offset" => q.offset = value.parse()?,
                "limit" => q.limit = value.parse()?,
                "since" => q.since = Some(value.parse()?),
                "until" => q.until = Some(value.parse()?),
                "sort" => ensure!(value == "created_at", "unsupported sort"),
                "order" => {
                    ensure!(value == "asc" || value == "desc", "unsupported order");
                    q.ascending = value == "asc";
                }
                _ => anyhow::bail!("unknown query parameter"),
            }
        }
        ensure!(
            seen.contains("order"),
            "explicit order=asc or order=desc required"
        );
        ensure!(
            q.offset >= 0 && (1..=crate::protocol_limits::QUERY_LIMIT).contains(&q.limit),
            "paging bounds"
        );
        ensure!(
            q.since.is_none_or(|v| v >= 0) && q.until.is_none_or(|v| v >= 0),
            "timestamp bounds"
        );
        ensure!(
            !matches!((q.since,q.until),(Some(a),Some(b)) if a>b),
            "reversed timestamp bounds"
        );
        Filter::new(&q.filter)
            .and_then(Filter::compile)
            .map_err(anyhow::Error::msg)?;
        Ok(q)
    }
}
#[derive(Serialize)]
pub struct Page {
    pub events: Vec<Event>,
    pub offset: i64,
    pub limit: i64,
    pub has_more: bool,
    pub next_offset: Option<i64>,
    pub sort: &'static str,
    pub order: &'static str,
    pub snapshot: bool,
    pub defaults_provisional: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
}
impl Repository {
    pub async fn discover(&self, q: QueryOptions) -> Result<Page> {
        let (filter, mut args) = Filter::new(&q.filter)
            .and_then(Filter::compile)
            .map_err(anyhow::Error::msg)?;
        let mut where_sql = format!(
            "e.kind=1001 AND e.state='hosted' AND NOT EXISTS(SELECT 1 FROM publication_blobs p JOIN quarantined_blobs q ON q.hash=p.hash AND q.generation=p.generation WHERE p.event_id=e.id) AND NOT EXISTS(SELECT 1 FROM backend_outbox o WHERE o.event_id=e.id AND o.completed_at IS NULL) AND ({filter})"
        );
        for (bound, op) in [(q.since, ">="), (q.until, "<=")] {
            if let Some(value) = bound {
                args.push(Value::Integer(value));
                where_sql.push_str(&format!(" AND e.created_at{op}${}", args.len()));
            }
        }
        let count_parameters = args.len();
        let order = if q.ascending { "ASC" } else { "DESC" };
        args.push(Value::Integer(q.limit + 1));
        let limit_parameter = args.len();
        args.push(Value::Integer(q.offset));
        let offset_parameter = args.len();
        let sql = format!(
            "SELECT e.id,octet_length(e.raw_event::text)::BIGINT FROM events e WHERE {where_sql} ORDER BY e.created_at {order},e.id ASC LIMIT ${limit_parameter} OFFSET ${offset_parameter}"
        );
        let params: Vec<_> = args.iter().map(Value::parameter).collect();
        let mut client = during(Phase::PoolCheckout, self.pool.get()).await?;
        let tx = during(
            Phase::DbRoundtrip,
            client
                .build_transaction()
                .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
                .read_only(true)
                .start(),
        )
        .await?;
        during(
            Phase::DbRoundtrip,
            tx.batch_execute("SET LOCAL statement_timeout='250ms'; SET LOCAL work_mem='4MB'"),
        )
        .await?;
        let total = if q.include_total {
            Some(
                during(
                    Phase::DbRoundtrip,
                    tx.query_one(
                        &format!("SELECT count(*) FROM events e WHERE {where_sql}"),
                        &params[..count_parameters],
                    ),
                )
                .await?
                .get::<_, i64>(0),
            )
        } else {
            None
        };
        // Fetch IDs and sizes first: no path can buffer 1000 one-MiB event bodies.
        let rows = during(Phase::DbRoundtrip, tx.query(&sql, &params)).await?;
        let decode = phase(Phase::DecodeResponse);
        let has_more = rows.len() > q.limit as usize;
        let rows = &rows[..rows.len().min(q.limit as usize)];
        let mut estimated = 1024i64;
        for row in rows {
            estimated = estimated
                .checked_add(row.get::<_, i64>(1) + 2)
                .context("response size overflow")?;
        }
        ensure!(
            estimated <= 2 * 1048576,
            "query response byte budget; narrow the query"
        );
        let ids: Vec<Vec<u8>> = rows.iter().map(|r| r.get(0)).collect();
        drop(decode);
        let rows=during(Phase::DbRoundtrip, tx.query(&format!("SELECT raw_event FROM events WHERE id=ANY($1) ORDER BY created_at {order},id ASC"), &[&ids])).await?;
        let decode = phase(Phase::DecodeResponse);
        let events = rows
            .into_iter()
            .map(|r| serde_json::from_value(r.get(0)))
            .collect::<Result<Vec<Event>, _>>()?;
        drop(decode);
        during(Phase::DbRoundtrip, tx.commit()).await?;
        let _response = phase(Phase::DecodeResponse);
        let next_offset = if has_more {
            Some(q.offset.checked_add(q.limit).context("offset overflow")?)
        } else {
            None
        };
        let page = Page {
            events,
            offset: q.offset,
            limit: q.limit,
            has_more,
            next_offset,
            sort: "created_at",
            order: if q.ascending { "asc" } else { "desc" },
            snapshot: false,
            defaults_provisional: false,
            total,
        };
        ensure!(
            serde_json::to_vec(&page)?.len() <= 2 * 1048576,
            "query response byte budget"
        );
        Ok(page)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parameters_are_strict() {
        for raw in [
            "",
            "limit=10",
            "order=asc&include_total=yes",
            "limit=0",
            "limit=1001",
            "offset=-1",
            "since=3&until=2",
            "since=-1",
            "limit=2&limit=3",
            "total=true",
            "order=sideways",
            "sort=publisher",
            "filter=pages%3D%22two%22",
        ] {
            assert!(QueryOptions::parse(Some(raw)).is_err(), "{raw}");
        }
        let q = QueryOptions::parse(Some("offset=12&limit=4&since=20&until=20&order=asc")).unwrap();
        assert!(
            QueryOptions::parse(Some("order=desc&include_total=true"))
                .unwrap()
                .include_total
        );
        assert_eq!(q.offset, 12);
        assert!(q.ascending);
        assert_eq!(q.until, Some(20));
    }
}
