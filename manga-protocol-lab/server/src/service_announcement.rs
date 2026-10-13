//! Provisional kind11998, explicitly approved for this experimental protocol.
use crate::{
    auth::{self, Event},
    db::{Id, Repository},
    endpoint_policy,
};
use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeSet;
pub const KIND: u32 = 11998;
pub const PROTOCOL: &str = "manga-service-endpoints-v1";
#[derive(Clone)]
pub struct Announcement {
    event: Event,
    endpoints: Vec<String>,
}
#[derive(Deserialize)]
struct Payload {
    version: u64,
    endpoints: Vec<String>,
    #[serde(default)]
    required: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheOutcome {
    Inserted,
    Current,
    Superseded,
}
impl Announcement {
    pub fn parse(event: Event) -> Result<Self> {
        ensure!(
            serde_json::to_vec(&event)?.len() <= 16384,
            "announcement byte budget"
        );
        event.verify().map_err(anyhow::Error::msg)?;
        ensure!(
            event.kind == KIND && event.one("protocol").map_err(anyhow::Error::msg)? == PROTOCOL,
            "announcement kind/protocol"
        );
        ensure!(
            event.created_at <= auth::now().saturating_add(60),
            "announcement future timestamp"
        );
        ensure!(
            !event
                .tags
                .iter()
                .any(|t| t.first().is_some_and(|s| s == "expiration")),
            "announcement expiry excluded"
        );
        let payload: Payload = serde_json::from_str(&event.content)?;
        ensure!(
            payload.version == 1 && payload.required.is_empty(),
            "unsupported announcement schema/features"
        );
        let endpoints = canonical_endpoints(payload.endpoints)?;
        Ok(Self { event, endpoints })
    }
    pub fn event(&self) -> &Event {
        &self.event
    }
    pub fn endpoints(&self) -> &[String] {
        &self.endpoints
    }
    pub fn service_key(&self) -> Id {
        hex::decode(&self.event.pubkey).unwrap().try_into().unwrap()
    }
    pub fn signed(secret: &str, endpoints: Vec<String>, created_at: u64) -> Result<Self> {
        let endpoints = canonical_endpoints(endpoints)?;
        Self::parse(
            auth::sign_at(
                secret,
                KIND,
                vec![vec!["protocol".into(), PROTOCOL.into()]],
                json!({"version":1,"endpoints":endpoints}).to_string(),
                created_at,
            )
            .map_err(anyhow::Error::msg)?,
        )
    }
}
fn canonical_endpoints(values: Vec<String>) -> Result<Vec<String>> {
    ensure!(values.len() <= 8, "announcement endpoint budget");
    let mut seen = BTreeSet::new();
    let mut endpoints = Vec::new();
    for value in values {
        let normalized = endpoint_policy::canonical_origin(&value)?;
        ensure!(seen.insert(normalized.clone()), "duplicate endpoint origin");
        endpoints.push(normalized);
    }
    Ok(endpoints)
}
impl Repository {
    pub async fn service_announcement(&self, key: Id) -> Result<Option<Announcement>> {
        let row=self.pool.get().await?.query_opt("SELECT raw_event,event_id,created_at FROM service_announcements WHERE service_key=$1",&[&key.as_slice()]).await?;
        row.map(|row| -> Result<_> {
            let event: Event = serde_json::from_value(row.get(0))?;
            ensure!(
                event.pubkey == hex::encode(key)
                    && hex::decode(&event.id)? == row.get::<_, Vec<u8>>(1)
                    && event.created_at == u64::try_from(row.get::<_, i64>(2))?,
                "stored announcement identity mismatch"
            );
            Announcement::parse(event)
        })
        .transpose()
    }
    pub async fn cache_service_announcement(
        &self,
        announcement: &Announcement,
    ) -> Result<CacheOutcome> {
        let event = announcement.event();
        let key = announcement.service_key();
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended('manga:service-announcements:v1',0))",
            &[],
        )
        .await?;
        let old=tx.query_opt("SELECT event_id,created_at FROM service_announcements WHERE service_key=$1 FOR UPDATE",&[&key.as_slice()]).await?;
        let eid = hex::decode(&event.id)?;
        let created = i64::try_from(event.created_at)?;
        if let Some(old) = old {
            let oldid: Vec<u8> = old.get(0);
            let oldtime: i64 = old.get(1);
            if oldid == eid {
                return Ok(CacheOutcome::Current);
            }
            if created < oldtime || (created == oldtime && eid > oldid) {
                return Ok(CacheOutcome::Superseded);
            }
        } else {
            let count: i64 = tx
                .query_one("SELECT COUNT(*) FROM service_announcements", &[])
                .await?
                .get(0);
            ensure!(count < 4096, "service announcement cache budget");
        }
        tx.execute("INSERT INTO service_announcements(service_key,event_id,created_at,raw_event) VALUES($1,$2,$3,$4) ON CONFLICT(service_key) DO UPDATE SET event_id=EXCLUDED.event_id,created_at=EXCLUDED.created_at,raw_event=EXCLUDED.raw_event,cached_at=clock_timestamp()",&[&key.as_slice(),&eid,&created,&serde_json::to_value(event)?]).await?;
        tx.commit().await?;
        Ok(CacheOutcome::Inserted)
    }
    pub async fn publish_service_endpoints(
        &self,
        secret: &str,
        endpoints: Vec<String>,
    ) -> Result<Announcement> {
        let endpoints = canonical_endpoints(endpoints)?;
        let identity = auth::sign(secret, 1, vec![], String::new()).map_err(anyhow::Error::msg)?;
        let key: Id = hex::decode(identity.pubkey)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("service key"))?;
        for _ in 0..8 {
            let current = self.service_announcement(key).await?;
            if let Some(current) = &current
                && current.endpoints() == endpoints
            {
                return Ok(current.clone());
            }
            let created = current.as_ref().map_or(auth::now(), |old| {
                auth::now().max(old.event().created_at.saturating_add(1))
            });
            let next = Announcement::signed(secret, endpoints.clone(), created)?;
            if self.cache_service_announcement(&next).await? != CacheOutcome::Superseded {
                return Ok(next);
            }
        }
        Err(anyhow::anyhow!("concurrent endpoint updates; retry"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_scope_bounds_and_empty_withdrawal() {
        let secret = format!("{:064x}", 11);
        let record =
            Announcement::signed(&secret, vec!["https://a.example/".into()], auth::now()).unwrap();
        assert_eq!(record.endpoints(), ["https://a.example"]);
        assert_eq!(record.event().kind, 11998);
        assert_eq!(
            record.event().one("protocol").unwrap(),
            "manga-service-endpoints-v1"
        );
        assert_eq!(
            hex::encode(record.service_key()),
            "774ae7f858a9411e5ef4246b70c65aac5649980be5c17891bbec17895da008cb"
        );
        assert!(
            Announcement::signed(&secret, vec![], auth::now())
                .unwrap()
                .endpoints()
                .is_empty()
        );
        assert!(
            Announcement::signed(&secret, vec!["https://a.example".into(); 2], auth::now())
                .is_err()
        );
        assert!(
            Announcement::signed(&secret, vec!["https://a.example/path".into()], auth::now())
                .is_err()
        );
        assert!(Announcement::signed(&secret, vec![], auth::now() + 120).is_err());
        let old = Announcement::signed(&secret, vec![], 1).unwrap();
        assert_eq!(old.event().created_at, 1);
        for body in [
            json!({"version":2,"endpoints":[]}),
            json!({"version":1,"endpoints":[],"required":["unknown"]}),
        ] {
            let event = auth::sign(
                &secret,
                KIND,
                vec![vec!["protocol".into(), PROTOCOL.into()]],
                body.to_string(),
            )
            .unwrap();
            assert!(Announcement::parse(event).is_err());
        }
        let event = auth::sign(
            &secret,
            KIND,
            vec![
                vec!["protocol".into(), PROTOCOL.into()],
                vec!["expiration".into(), "9999999999".into()],
            ],
            json!({"version":1,"endpoints":[]}).to_string(),
        )
        .unwrap();
        assert!(Announcement::parse(event).is_err());
    }
}
