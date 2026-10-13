//! MangaDex-shaped local facade. UUID aliases never rewrite Nostr identities.
use crate::{
    auth,
    catalogue::{Entry, VISIBLE},
    db::{Id, Repository},
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
pub const PREFIX: &str = "/compat/mangadex";
pub fn uuid(kind: &str, target: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"manga-compat-v1:");
    h.update(kind.as_bytes());
    h.update(b":");
    h.update(target.as_bytes());
    let mut b = h.finalize()[..16].to_vec();
    b[6] = (b[6] & 15) | 128;
    b[8] = (b[8] & 63) | 128;
    let s = hex::encode(b);
    format!(
        "{}-{}-{}-{}-{}",
        &s[..8],
        &s[8..12],
        &s[12..16],
        &s[16..20],
        &s[20..]
    )
}
pub fn valid_uuid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
}
pub fn timestamp(t: u64) -> Result<String> {
    ensure!(
        t <= 253402300799,
        "timestamp outside compatibility date range"
    );
    let z = (t / 86400) as i64 + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += i64::from(m <= 2);
    let rem = t % 86400;
    Ok(format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    ))
}
pub fn parse_timestamp(s: &str) -> Result<i64> {
    let s = s.strip_suffix('Z').unwrap_or(s);
    ensure!(s.len() == 19, "timestamp format");
    let b = s.as_bytes();
    for (i, ch) in b.iter().enumerate() {
        ensure!(
            match i {
                4 | 7 => *ch == b'-',
                10 => *ch == b'T',
                13 | 16 => *ch == b':',
                _ => ch.is_ascii_digit(),
            },
            "timestamp syntax"
        );
    }
    let y: i64 = s[..4].parse()?;
    let m: i64 = s[5..7].parse()?;
    let d: i64 = s[8..10].parse()?;
    let hour: i64 = s[11..13].parse()?;
    let min: i64 = s[14..16].parse()?;
    let sec: i64 = s[17..].parse()?;
    ensure!(
        (1970..=9999).contains(&y)
            && (1..=12).contains(&m)
            && (1..=31).contains(&d)
            && hour < 24
            && min < 60
            && sec < 60,
        "date bounds"
    );
    let yy = y - i64::from(m <= 2);
    let era = yy / 400;
    let yoe = yy - era * 400;
    let mp = m + if m > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let days = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468;
    let t = days * 86400 + hour * 3600 + min * 60 + sec;
    ensure!(t >= 0 && timestamp(t as u64)? == s, "invalid calendar date");
    Ok(t)
}
pub(crate) async fn register(
    tx: &deadpool_postgres::Transaction<'_>,
    kind: &str,
    target: &str,
    payload: Value,
) -> Result<String> {
    tx.query_one("SELECT pg_advisory_xact_lock(hashtextextended('compat-target:'||$1::text||':'||$2::text,0))",&[&kind,&target]).await?;
    let id = uuid(kind, target);
    // First canonical mapping is preferred unless an explicit legacy alias exists.
    tx.execute("INSERT INTO compatibility_ids(kind,id,target,payload,preferred) VALUES($1,$2,$3,$4,NOT EXISTS(SELECT 1 FROM compatibility_ids WHERE kind=$1 AND target=$3 AND preferred)) ON CONFLICT(kind,id) DO NOTHING",&[&kind,&id,&target,&payload]).await?;
    ensure!(
        tx.query_one(
            "SELECT target=$3 FROM compatibility_ids WHERE kind=$1 AND id=$2",
            &[&kind, &id, &target]
        )
        .await?
        .get::<_, bool>(0),
        "compatibility UUID collision"
    );
    Ok(id)
}
pub(crate) async fn index_tx(
    tx: &deadpool_postgres::Transaction<'_>,
    e: &auth::Event,
    m: &Value,
) -> Result<()> {
    let key = hex::decode(&e.id)?;
    // Serialize publication/backfill/client access for this exact immutable ID.
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended('compat:'||$1::text,0))",
        &[&e.id],
    )
    .await?;
    if tx
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM compatibility_indexed WHERE event_id=$1)",
            &[&key],
        )
        .await?
        .get::<_, bool>(0)
    {
        return Ok(());
    }
    let kind = if e.kind == 1002 { "manga" } else { "chapter" };
    register(tx, kind, &e.id, json!({})).await?;
    register(tx, "user", &e.pubkey, json!({"nostrPubkey":e.pubkey})).await?;
    if e.kind == 1002 {
        if !m["cover"].is_null() {
            register(tx, "cover", &e.id, json!({})).await?;
        }
        if let Some(categories) = m["categories"].as_array() {
            for c in categories
                .iter()
                .filter_map(|c| c.as_str())
                .collect::<std::collections::BTreeSet<_>>()
            {
                register(tx, "tag", c, json!({"name":c})).await?;
            }
        }
        if let Some(credits) = m["credits"].as_array() {
            for (i, c) in credits.iter().enumerate() {
                register(
                    tx,
                    "author",
                    &format!("{}:{i}", e.id),
                    json!({"series":e.id,"credit":c}),
                )
                .await?;
            }
        }
    } else {
        // Relationships may refer to an unhosted series; registering an alias does
        // not invent its metadata or make that series publicly readable.
        if let Some(series) = m["series"].as_str() {
            register(tx, "manga", series, json!({})).await?;
        }
    }
    tx.execute(
        "INSERT INTO compatibility_indexed(event_id) VALUES($1) ON CONFLICT DO NOTHING",
        &[&key],
    )
    .await?;
    Ok(())
}
impl Repository {
    pub async fn compatibility_target(
        &self,
        kind: &str,
        id: &str,
    ) -> Result<Option<(String, Value)>> {
        ensure!(valid_uuid(id), "UUID required");
        Ok(self
            .pool
            .get()
            .await?
            .query_opt(
                "SELECT target,payload FROM compatibility_ids WHERE kind=$1 AND id=$2",
                &[&kind, &id],
            )
            .await?
            .map(|r| (r.get(0), r.get(1))))
    }
    pub async fn compatibility_id(&self, kind: &str, target: &str) -> Result<String> {
        Ok(self
            .pool
            .get()
            .await?
            .query_opt(
                "SELECT id FROM compatibility_ids WHERE kind=$1 AND target=$2 AND preferred",
                &[&kind, &target],
            )
            .await?
            .map(|r| r.get(0))
            .unwrap_or_else(|| uuid(kind, target)))
    }
    pub(crate) async fn compatibility_identifiers(
        &self,
        kind: &str,
        targets: &[String],
    ) -> Result<Ids> {
        ensure!(
            targets.len() <= 10000 && targets.iter().map(String::len).sum::<usize>() <= 2 * 1048576,
            "identifier query budget"
        );
        let mut ids = Ids::default();
        if !targets.is_empty() {
            for row in self.pool.get().await?.query("SELECT target,id FROM compatibility_ids WHERE kind=$1 AND target=ANY($2) AND preferred",&[&kind,&targets]).await? {
                ids.0.insert((kind.to_owned(),row.get(0)),row.get(1));
            }
        }
        Ok(ids)
    }
    pub async fn compatibility_entry(&self, kind: &str, id: &str) -> Result<Option<Entry>> {
        let Some((target, _)) = self.compatibility_target(kind, id).await? else {
            return Ok(None);
        };
        let hash: Id = hex::decode(target)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("publication ID"))?;
        let e = self.catalogue_entry(hash).await?;
        Ok(e.filter(|e| {
            if kind == "chapter" {
                e.event.kind == 1001
            } else {
                e.event.kind == 1002
            }
        }))
    }
    pub async fn compatibility_backfill(&self) -> Result<usize> {
        let accounts=self.pool.get().await?.query("SELECT DISTINCT h.publisher FROM account_heads h JOIN events e ON e.id=h.event_id WHERE e.state='hosted' AND NOT EXISTS(SELECT 1 FROM compatibility_ids i WHERE i.kind='user' AND i.target=encode(h.publisher,'hex')) ORDER BY h.publisher LIMIT 64",&[]).await?;
        let account_count = accounts.len();
        for row in accounts {
            let mut c = self.pool.get().await?;
            let tx = c.transaction().await?;
            let key = hex::encode(row.get::<_, Vec<u8>>(0));
            register(&tx, "user", &key, json!({"nostrPubkey":key})).await?;
            tx.commit().await?;
        }
        let groups=self.pool.get().await?.query("SELECT group_key FROM managed_groups g WHERE NOT EXISTS(SELECT 1 FROM compatibility_ids t WHERE t.kind='group' AND t.target=encode(g.group_key,'hex')) ORDER BY group_key LIMIT 64",&[]).await?;
        let group_count = groups.len();
        for row in groups {
            let mut c = self.pool.get().await?;
            let tx = c.transaction().await?;
            let key = hex::encode(row.get::<_, Vec<u8>>(0));
            register(&tx, "group", &key, json!({"nostrPubkey":key})).await?;
            tx.commit().await?;
        }
        let rows=self.pool.get().await?.query(&format!("SELECT e.raw_event,c.metadata FROM events e JOIN catalogue_metadata c ON c.event_id=e.id WHERE e.kind IN(1001,1002) AND c.metadata_version>=2 AND {VISIBLE} AND NOT EXISTS(SELECT 1 FROM compatibility_indexed x WHERE x.event_id=e.id) ORDER BY e.id LIMIT 64"),&[]).await?;
        let count = rows.len();
        for r in rows {
            let event: auth::Event = serde_json::from_value(r.get(0))?;
            let metadata: Value = r.get(1);
            let mut c = self.pool.get().await?;
            let tx = c.transaction().await?;
            let visible = tx
                .query_opt(
                    "SELECT state FROM events WHERE id=$1 FOR SHARE",
                    &[&hex::decode(&event.id)?],
                )
                .await?;
            if visible.is_none_or(|r| r.get::<_, String>(0) != "hosted") {
                continue;
            }
            index_tx(&tx, &event, &metadata).await?;
            tx.commit().await?;
        }
        Ok(count.max(group_count).max(account_count))
    }
    pub async fn register_compatibility_alias(
        &self,
        kind: &str,
        id: &str,
        target: &str,
        preferred: bool,
    ) -> Result<()> {
        ensure!(
            [
                "manga", "chapter", "cover", "user", "group", "author", "tag", "list"
            ]
            .contains(&kind)
                && valid_uuid(id)
                && target.len() <= 1024,
            "alias bounds"
        );
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        tx.query_one("SELECT pg_advisory_xact_lock(hashtextextended('compat-target:'||$1::text||':'||$2::text,0))",&[&kind,&target]).await?;
        let row = tx
            .query_opt(
                "SELECT payload FROM compatibility_ids WHERE kind=$1 AND target=$2 LIMIT 1",
                &[&kind, &target],
            )
            .await?
            .context("target must already be indexed")?;
        let payload: Value = row.get(0);
        tx.execute("INSERT INTO compatibility_ids(kind,id,target,payload) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING",&[&kind,&id,&target,&payload]).await?;
        ensure!(
            tx.query_one(
                "SELECT target=$3 FROM compatibility_ids WHERE kind=$1 AND id=$2",
                &[&kind, &id, &target]
            )
            .await?
            .get::<_, bool>(0),
            "alias already belongs to another target"
        );
        if preferred {
            tx.execute("UPDATE compatibility_ids SET preferred=false WHERE kind=$1 AND target=$2 AND preferred",&[&kind,&target]).await?;
            tx.execute(
                "UPDATE compatibility_ids SET preferred=true WHERE kind=$1 AND id=$2",
                &[&kind, &id],
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
#[derive(Default)]
pub struct Ids(std::collections::BTreeMap<(String, String), String>);
impl Ids {
    pub fn has(&self, kind: &str, target: &str) -> bool {
        self.0.contains_key(&(kind.into(), target.into()))
    }
    pub fn get(&self, kind: &str, target: &str) -> String {
        self.0
            .get(&(kind.into(), target.into()))
            .cloned()
            .unwrap_or_else(|| uuid(kind, target))
    }
}
impl Repository {
    pub async fn compatibility_ids_for(&self, entries: &[Entry]) -> Result<Ids> {
        self.compatibility_ids_for_refs(&entries.iter().collect::<Vec<_>>())
            .await
    }
    pub async fn compatibility_ids_for_refs(&self, entries: &[&Entry]) -> Result<Ids> {
        let mut targets = std::collections::BTreeSet::new();
        for e in entries {
            targets.insert(e.event.id.clone());
            targets.insert(e.event.pubkey.clone());
            if let Some(s) = e.metadata["series"].as_str() {
                targets.insert(s.to_owned());
            }
            if let Some(c) = e.metadata["categories"].as_array() {
                for c in c {
                    if let Some(c) = c.as_str() {
                        targets.insert(c.to_owned());
                    }
                }
            }
            if let Some(c) = e.metadata["credits"].as_array() {
                for i in 0..c.len() {
                    targets.insert(format!("{}:{i}", e.event.id));
                }
            }
        }
        ensure!(targets.len() <= 20000, "compatibility relationship budget");
        let mut ids = Ids::default();
        let targets: Vec<_> = targets.into_iter().collect();
        for batch in targets.chunks(1000) {
            for row in self.pool.get().await?.query("SELECT kind,target,id FROM compatibility_ids WHERE target=ANY($1) AND preferred",&[&batch]).await?{ids.0.insert((row.get(0),row.get(1)),row.get(2));}
        }
        Ok(ids)
    }
}
pub fn entity(
    entry: &Entry,
    ids: &Ids,
    requested: Option<&str>,
    expanded: &std::collections::BTreeSet<String>,
) -> Result<Value> {
    let e = &entry.event;
    let m = &entry.metadata;
    let date = timestamp(e.created_at)?;
    let kind = if e.kind == 1002 { "manga" } else { "chapter" };
    let id = requested
        .map(str::to_owned)
        .unwrap_or_else(|| ids.get(kind, &e.id));
    let mut relationships = vec![];
    let attributes = if e.kind == 1002 {
        if !m["cover"].is_null() {
            let mut cover = json!({"id":ids.get("cover",&e.id),"type":"cover_art"});
            if expanded.contains("cover_art") {
                cover["attributes"] = cover_attributes(entry)?;
            }
            relationships.push(cover);
        }
        if let Some(credits) = m["credits"].as_array() {
            for (i, c) in credits.iter().enumerate() {
                let role = c["role"].as_str().unwrap_or("");
                if !["author", "artist"].contains(&role) {
                    continue;
                }
                let mut rel = json!({"id":ids.get("author",&format!("{}:{i}",e.id)),"type":role});
                if expanded.contains(role) {
                    rel["attributes"] = json!({"name":c["name"],"biography":{},"version":1,"createdAt":date,"updatedAt":date});
                }
                relationships.push(rel);
            }
        }
        let tags:Vec<_>=m["categories"].as_array().into_iter().flatten().filter_map(|v|v.as_str()).map(|c|json!({"id":ids.get("tag",c),"type":"tag","attributes":{"name":{"en":c},"description":{},"version":1},"relationships":[]})).collect();
        let mut attributes = json!({"title":m["titles"],"altTitles":m["altTitles"].as_array().cloned().unwrap_or_default(),"description":m["description"].as_object().cloned().unwrap_or_default(),"isLocked":true,"links":{},"lastVolume":null,"lastChapter":null,"publicationDemographic":m["publicationDemographic"],"year":m["year"],"tags":tags,"state":"published","version":1,"createdAt":date,"updatedAt":date,"nostrCredits":m["credits"]});
        for field in ["originalLanguage", "status", "contentRating"] {
            if !m[field].is_null() {
                attributes[field] = m[field].clone();
            }
        }
        attributes
    } else {
        if let Some(series) = m["series"].as_str() {
            relationships.push(json!({"id":ids.get("manga",series),"type":"manga"}));
        }
        let mut user = json!({"id":ids.get("user",&e.pubkey),"type":"user"});
        if expanded.contains("user") {
            user["attributes"] = json!({"username":e.pubkey,"roles":[],"version":1,"nostrNameIsPublicKeyFallback":true});
        }
        relationships.push(user);
        if ids.has("group", &e.pubkey) {
            let mut group = json!({"id":ids.get("group",&e.pubkey),"type":"scanlation_group"});
            if expanded.contains("scanlation_group") {
                group["attributes"] =
                    json!({"name":e.pubkey,"version":1,"nostrNameIsPublicKeyFallback":true});
            }
            relationships.push(group);
        }

        json!({"title":m["title"],"volume":m["volume"],"chapter":m["chapter"],"pages":m["pages"].as_array().map_or(0,Vec::len),"translatedLanguage":m["language"],"uploader":ids.get("user",&e.pubkey),"externalUrl":null,"version":1,"createdAt":date,"updatedAt":date,"publishAt":date,"readableAt":date,"isUnavailable":false})
    };
    Ok(
        json!({"id":id,"type":kind,"attributes":attributes,"relationships":relationships,"nostr":{"event":e,"eventId":e.id,"pubkey":e.pubkey,"manifest":serde_json::from_str::<Value>(&e.content)?["manifest"]}}),
    )
}
pub fn cover_attributes(entry: &Entry) -> Result<Value> {
    let c = &entry.metadata["cover"];
    ensure!(c.is_object(), "series has no cover");
    let mime = c["mime"].as_str().context("cover MIME")?;
    let hash = c["sha256"].as_str().context("cover hash")?;
    let date = timestamp(entry.event.created_at)?;
    Ok(
        json!({"volume":null,"fileName":format!("{hash}.{}",crate::manifest::extension(mime)),"description":null,"locale":null,"version":1,"createdAt":date,"updatedAt":date}),
    )
}

#[cfg(test)]
mod independent_vectors {
    use crate::mangadex_compat::{parse_timestamp, timestamp, uuid, valid_uuid};

    #[test]
    fn compatibility_identifiers_and_dates_match_independent_vectors() {
        // Fixed Python hashlib/uuid and datetime results, not paired Rust round trips.
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/protocol-vectors/compatibility.json"
        ))
        .unwrap();
        for case in cases["uuids"].as_array().unwrap() {
            let expected = case["uuid"].as_str().unwrap();
            assert_eq!(
                uuid(
                    case["kind"].as_str().unwrap(),
                    case["target"].as_str().unwrap()
                ),
                expected
            );
            assert!(valid_uuid(expected));
            assert!(!valid_uuid(&expected.to_uppercase()));
        }
        for case in cases["dates"].as_array().unwrap() {
            let text = case["text"].as_str().unwrap();
            let seconds = case["seconds"].as_u64().unwrap();
            assert_eq!(timestamp(seconds).unwrap(), text);
            assert_eq!(parse_timestamp(text).unwrap(), seconds as i64);
            assert_eq!(
                parse_timestamp(&format!("{text}Z")).unwrap(),
                seconds as i64
            );
        }
        for invalid in [
            "2026-02-30T00:00:00",
            "2100-02-29T12:34:56",
            "2026-01-01T24:00:00",
            "1969-12-31T23:59:59",
            "2026-01-01T00:00:60",
            "2026-01-01T00:00:00+00:00",
        ] {
            assert!(parse_timestamp(invalid).is_err(), "{invalid}");
        }
        assert!(timestamp(253402300800).is_err());
    }
}
