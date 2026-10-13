//! Bounded MangaDex-shaped query translation; unsupported options are explicit.
use crate::{
    catalogue::{Entry, VISIBLE},
    db::Repository,
    mangadex_compat::{parse_timestamp, valid_uuid},
};
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone)]
pub struct Query {
    pub limit: i64,
    pub offset: i64,
    pub includes: BTreeSet<String>,
    pub fields: BTreeMap<String, Vec<String>>,
    pub order: Vec<(String, bool)>,
}
impl Query {
    pub fn parse(raw: Option<&str>, kind: &str) -> Result<Self> {
        let raw = raw.unwrap_or("");
        ensure!(raw.len() <= 32768, "query byte budget");
        let mut fields: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut order = vec![];
        for (k, v) in url::form_urlencoded::parse(raw.as_bytes()) {
            ensure!(
                k.len() <= 64 && v.len() <= 4096 && !v.contains('\0'),
                "query value budget"
            );
            let k = k.to_string();
            let v = v.to_string();
            if let Some(name) = k.strip_prefix("order[").and_then(|s| s.strip_suffix(']')) {
                ensure!(
                    [
                        "createdAt",
                        "updatedAt",
                        "title",
                        "year",
                        "volume",
                        "chapter",
                        "publishAt",
                        "readableAt",
                        "latestUploadedChapter"
                    ]
                    .contains(&name)
                        && ["asc", "desc"].contains(&v.as_str())
                        && !order.iter().any(|(n, _)| n == name)
                        && order.len() < 4,
                    "unsupported order"
                );
                order.push((name.to_owned(), v == "asc"));
                continue;
            }
            let allowed = [
                "limit",
                "offset",
                "includes[]",
                "ids[]",
                "createdAtSince",
                "updatedAtSince",
                "title",
                "year",
                "status[]",
                "originalLanguage[]",
                "excludedOriginalLanguage[]",
                "translatedLanguage[]",
                "contentRating[]",
                "publicationDemographic[]",
                "includedTags[]",
                "excludedTags[]",
                "includedTagsMode",
                "excludedTagsMode",
                "manga",
                "groups[]",
                "uploader[]",
                "authors[]",
                "artists[]",
                "authorOrArtist",
                "hasAvailableChapters",
                "hasUnavailableChapters",
                "availableTranslatedLanguage[]",
            ];
            ensure!(
                allowed.contains(&k.as_str()),
                "unsupported query parameter: {k}"
            );
            let values = fields.entry(k.clone()).or_default();
            ensure!(
                (k.ends_with("[]") || values.is_empty()) && values.len() < 100,
                "duplicate or overbudget query parameter"
            );
            values.push(v);
        }
        ensure!(fields.len() <= 32, "query field budget");
        let limit = fields
            .remove("limit")
            .map(|v| v[0].parse::<i64>())
            .transpose()?
            .unwrap_or(10);
        let offset = fields
            .remove("offset")
            .map(|v| v[0].parse::<i64>())
            .transpose()?
            .unwrap_or(0);
        ensure!((0..=100).contains(&limit) && offset >= 0, "paging bounds");
        let includes: BTreeSet<_> = fields
            .remove("includes[]")
            .unwrap_or_default()
            .into_iter()
            .collect();
        ensure!(
            includes.iter().all(|v| [
                "cover_art",
                "author",
                "artist",
                "manga",
                "scanlation_group",
                "user"
            ]
            .contains(&v.as_str())),
            "unsupported expansion"
        );
        let manga_only = [
            "title",
            "year",
            "status[]",
            "originalLanguage[]",
            "excludedOriginalLanguage[]",
            "publicationDemographic[]",
            "includedTags[]",
            "excludedTags[]",
            "includedTagsMode",
            "excludedTagsMode",
            "authors[]",
            "artists[]",
            "authorOrArtist",
            "hasAvailableChapters",
            "hasUnavailableChapters",
            "availableTranslatedLanguage[]",
        ];
        if kind == "chapter" {
            ensure!(
                !fields.keys().any(|k| manga_only.contains(&k.as_str())),
                "manga-only filter on chapter list"
            );
        }
        if kind == "manga" {
            ensure!(
                !fields.contains_key("manga")
                    && !fields.contains_key("translatedLanguage[]")
                    && !fields.contains_key("uploader[]"),
                "chapter-only filter on manga list"
            );
        }
        for (k, vs) in &fields {
            if k == "ids[]"
                || [
                    "manga",
                    "groups[]",
                    "uploader[]",
                    "authors[]",
                    "artists[]",
                    "authorOrArtist",
                    "includedTags[]",
                    "excludedTags[]",
                ]
                .contains(&k.as_str())
            {
                ensure!(vs.iter().all(|v| valid_uuid(v)), "UUID query value");
            }
        }
        Ok(Self {
            limit,
            offset,
            includes,
            fields,
            order,
        })
    }
}
enum Arg {
    Number(i64),
    Text(String),
    Texts(Vec<String>),
    Bytes(Vec<u8>),
    ByteArrays(Vec<Vec<u8>>),
}
impl Arg {
    fn parameter(&self) -> &(dyn tokio_postgres::types::ToSql + Sync) {
        match self {
            Self::Number(v) => v,
            Self::Text(v) => v,
            Self::Texts(v) => v,
            Self::Bytes(v) => v,
            Self::ByteArrays(v) => v,
        }
    }
}
fn push(args: &mut Vec<Arg>, v: Arg) -> String {
    args.push(v);
    format!("${}", args.len())
}
fn language(column: &str, values: &[String], args: &mut Vec<Arg>) -> Result<String> {
    let mut terms = vec![];
    for v in values {
        ensure!(crate::filter::basic_language_range(v), "language range");
        if v == "*" {
            terms.push(format!(
                "CASE WHEN {column} IS NULL THEN NULL ELSE TRUE END"
            ));
        } else {
            let a = push(args, Arg::Text(v.to_ascii_lowercase()));
            let b = push(args, Arg::Text(format!("{}-%", v.to_ascii_lowercase())));
            terms.push(format!(
                "(lower({column} COLLATE \"C\")={a} OR lower({column} COLLATE \"C\") LIKE {b})"
            ));
        }
    }
    Ok(format!("({})", terms.join(" OR ")))
}
impl Repository {
    pub(crate) async fn compatibility_targets(
        &self,
        kind: &str,
        ids: &[String],
    ) -> Result<Vec<String>> {
        Ok(self
            .pool
            .get()
            .await?
            .query(
                "SELECT DISTINCT target FROM compatibility_ids WHERE kind=$1 AND id=ANY($2)",
                &[&kind, &ids],
            )
            .await?
            .into_iter()
            .map(|r| r.get(0))
            .collect())
    }
    pub async fn compatibility_query(
        &self,
        kind: &str,
        q: &Query,
        series: Option<&str>,
    ) -> Result<(Vec<Entry>, i64)> {
        ensure!(["manga", "chapter"].contains(&kind), "content kind");
        let mut args = vec![];
        let mut terms = vec![
            format!("e.kind={}", if kind == "manga" { 1002 } else { 1001 }),
            VISIBLE.to_owned(),
            "c.metadata_version>=2".into(),
        ];
        if let Some(s) = series {
            let id: Vec<u8> = hex::decode(s)?;
            terms.push(format!("e.series={}", push(&mut args, Arg::Bytes(id))));
        }
        let mut tag_modes = BTreeMap::new();
        for (name, default) in [("includedTagsMode", "AND"), ("excludedTagsMode", "OR")] {
            let mode = q.fields.get(name).map(|v| v[0].as_str()).unwrap_or(default);
            ensure!(["AND", "OR"].contains(&mode), "tag mode");
            tag_modes.insert(name, mode);
        }
        for (key, values) in &q.fields {
            match key.as_str() {
                "ids[]" => {
                    let ids = self
                        .compatibility_targets(kind, values)
                        .await?
                        .into_iter()
                        .map(hex::decode)
                        .collect::<Result<Vec<_>, _>>()?;
                    terms.push(format!(
                        "e.id=ANY({}::bytea[])",
                        push(&mut args, Arg::ByteArrays(ids))
                    ));
                }
                "manga" => {
                    let ids = self
                        .compatibility_targets("manga", values)
                        .await?
                        .into_iter()
                        .map(hex::decode)
                        .collect::<Result<Vec<_>, _>>()?;
                    terms.push(format!(
                        "e.series=ANY({}::bytea[])",
                        push(&mut args, Arg::ByteArrays(ids))
                    ));
                }
                "groups[]" | "uploader[]" => {
                    let ids = self
                        .compatibility_targets(
                            if key == "groups[]" { "group" } else { "user" },
                            values,
                        )
                        .await?
                        .into_iter()
                        .map(hex::decode)
                        .collect::<Result<Vec<_>, _>>()?;
                    let arg = push(&mut args, Arg::ByteArrays(ids));
                    if kind == "chapter" {
                        terms.push(format!("e.publisher=ANY({arg}::bytea[])"));
                    } else {
                        terms.push(format!("EXISTS(SELECT 1 FROM events child_event WHERE child_event.series=e.id AND child_event.publisher=ANY({arg}::bytea[]) AND {})",VISIBLE.replace("e.","child_event.")));
                    }
                }
                "createdAtSince" | "updatedAtSince" => terms.push(format!(
                    "e.created_at>={}",
                    push(&mut args, Arg::Number(parse_timestamp(&values[0])?))
                )),
                "title" => {
                    let a = push(&mut args, Arg::Text(values[0].clone()));
                    terms.push(format!("EXISTS(SELECT 1 FROM unnest(c.title_values) t(v) WHERE strpos(t.v COLLATE \"C\",{a})>0)"));
                }
                "year" => {
                    if values[0] == "none" {
                        terms.push("c.metadata->>'year' IS NULL".into())
                    } else {
                        let y = values[0].parse::<i64>()?;
                        ensure!((0..=9999).contains(&y), "year bounds");
                        terms.push(format!(
                            "(c.metadata->>'year')::bigint={}",
                            push(&mut args, Arg::Number(y))
                        ));
                    }
                }
                "status[]" | "publicationDemographic[]" | "contentRating[]" => {
                    let field = match key.as_str() {
                        "status[]" => "status",
                        "contentRating[]" => "contentRating",
                        _ => "publicationDemographic",
                    };
                    let table = if kind == "chapter" {
                        "parent_meta"
                    } else {
                        "c"
                    };
                    let a = push(&mut args, Arg::Texts(values.clone()));
                    terms.push(format!(
                        "({table}.metadata->>'{field}') COLLATE \"C\"=ANY({a}::text[])"
                    ));
                }
                "originalLanguage[]" | "excludedOriginalLanguage[]" | "translatedLanguage[]" => {
                    let col = if key == "translatedLanguage[]" {
                        "e.language"
                    } else {
                        "(c.metadata->>'originalLanguage')"
                    };
                    let pred = language(col, values, &mut args)?;
                    terms.push(if key == "excludedOriginalLanguage[]" {
                        format!("NOT {pred}")
                    } else {
                        pred
                    });
                }
                "availableTranslatedLanguage[]" => {
                    let pred = language("child_event.language", values, &mut args)?;
                    terms.push(format!("EXISTS(SELECT 1 FROM events child_event WHERE child_event.series=e.id AND child_event.kind=1001 AND {} AND {pred})",VISIBLE.replace("e.","child_event.")));
                }
                "hasAvailableChapters" => {
                    ensure!(
                        ["0", "1", "true", "false"].contains(&values[0].as_str()),
                        "boolean value"
                    );
                    let exists = format!(
                        "EXISTS(SELECT 1 FROM events child_event WHERE child_event.series=e.id AND child_event.kind=1001 AND {})",
                        VISIBLE.replace("e.", "child_event.")
                    );
                    terms.push(if ["1", "true"].contains(&values[0].as_str()) {
                        exists
                    } else {
                        format!("NOT {exists}")
                    });
                }
                "hasUnavailableChapters" => {
                    ensure!(
                        ["0", "false"].contains(&values[0].as_str()),
                        "unavailable content is not advertised"
                    );
                }
                "includedTags[]" | "excludedTags[]" => {
                    let tags = self.compatibility_targets("tag", values).await?;
                    let mut predicates = vec![];
                    for tag in tags {
                        let a = push(&mut args, Arg::Text(tag));
                        predicates.push(format!("c.metadata @> jsonb_build_object('categories',jsonb_build_array({a}::text))"));
                    }
                    let mode = tag_modes[if key == "includedTags[]" {
                        "includedTagsMode"
                    } else {
                        "excludedTagsMode"
                    }];
                    if predicates.is_empty() {
                        predicates.push("FALSE".into());
                    }
                    let pred = format!("({})", predicates.join(&format!(" {mode} ")));
                    terms.push(if key=="excludedTags[]"{format!("CASE WHEN c.metadata->>'categories' IS NULL THEN NULL ELSE NOT {pred} END")}else{pred});
                }
                "authors[]" | "artists[]" | "authorOrArtist" => {
                    let a = push(&mut args, Arg::Texts(values.clone()));
                    let role = match key.as_str() {
                        "authors[]" => " AND x.payload->'credit'->>'role'='author'",
                        "artists[]" => " AND x.payload->'credit'->>'role'='artist'",
                        _ => "",
                    };
                    terms.push(format!("EXISTS(SELECT 1 FROM compatibility_ids x WHERE x.kind='author' AND x.id=ANY({a}::text[]) AND x.payload->>'series'=encode(e.id,'hex'){role})"));
                }
                "includedTagsMode" | "excludedTagsMode" => {}
                _ => anyhow::bail!("unsupported filter"),
            }
        }
        let mut order = vec![];
        for (name, asc) in &q.order {
            let dir = if *asc { "ASC" } else { "DESC" };
            let col = match name.as_str() {
                "createdAt" | "updatedAt" | "publishAt" | "readableAt" => "e.created_at".to_owned(),
                "title" if kind == "manga" => "c.title_values[1] COLLATE \"C\"".into(),
                "year" if kind == "manga" => "(c.metadata->>'year')::bigint".into(),
                "chapter" | "volume" if kind == "chapter" => {
                    let f = if name == "chapter" {
                        "e.chapter"
                    } else {
                        "e.volume"
                    };
                    order.push(format!("CASE WHEN length({f})<=64 AND {f} ~ '^[0-9]+(\\.[0-9]+)?$' THEN {f}::numeric END {dir} NULLS LAST"));
                    format!("{f} COLLATE \"C\"")
                }
                "latestUploadedChapter" if kind == "manga" => format!(
                    "COALESCE((SELECT MAX(child_event.created_at) FROM events child_event WHERE child_event.series=e.id AND child_event.kind=1001 AND {}),e.created_at)",
                    VISIBLE.replace("e.", "child_event.")
                ),
                _ => anyhow::bail!("unsupported order for content type"),
            };
            order.push(format!("{col} {dir} NULLS LAST"));
        }
        if order.is_empty() {
            order.push("e.created_at DESC".into());
        }
        order.push("e.id ASC".into());
        let from = if kind == "chapter" {
            format!(
                "events e JOIN catalogue_metadata c ON c.event_id=e.id LEFT JOIN (SELECT parent_c.* FROM catalogue_metadata parent_c JOIN events parent_e ON parent_e.id=parent_c.event_id WHERE {}) parent_meta ON parent_meta.event_id=e.series",
                VISIBLE.replace("e.", "parent_e.")
            )
        } else {
            "events e JOIN catalogue_metadata c ON c.event_id=e.id".into()
        };
        let predicate = terms.join(" AND ");
        let count = args.len();
        let limit = push(&mut args, Arg::Number(q.limit));
        let offset = push(&mut args, Arg::Number(q.offset));
        let params: Vec<_> = args.iter().map(Arg::parameter).collect();
        let mut client = self.pool.get().await?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await?;
        tx.batch_execute("SET LOCAL statement_timeout='2000ms'; SET LOCAL lock_timeout='500ms'")
            .await?;
        let total: i64 = tx
            .query_one(
                &format!("SELECT COUNT(*) FROM {from} WHERE {predicate}"),
                &params[..count],
            )
            .await?
            .get(0);
        let bytes:i64=tx.query_one(&format!("SELECT COALESCE(sum(n),0)::bigint FROM (SELECT octet_length(e.raw_event::text)+octet_length(c.metadata::text) AS n FROM {from} WHERE {predicate} ORDER BY {} LIMIT {limit} OFFSET {offset}) page",order.join(",")),&params).await?.get(0);
        ensure!(
            bytes <= 2 * 1048576,
            "compatibility page byte budget; reduce limit"
        );
        let rows=tx.query(&format!("SELECT e.raw_event,c.metadata FROM {from} WHERE {predicate} ORDER BY {} LIMIT {limit} OFFSET {offset}",order.join(",")),&params).await?;
        let entries = rows
            .into_iter()
            .map(|r| {
                Ok(Entry {
                    event: serde_json::from_value(r.get(0))?,
                    metadata: r.get(1),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        tx.commit().await?;
        Ok((entries, total))
    }
}
