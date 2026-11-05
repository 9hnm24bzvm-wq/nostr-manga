//! Bounded signed endpoint refresh for already-approved peers. Never grants access.
use crate::{
    db::{Id, Repository},
    endpoint_policy::{EndpointPolicy, VerifiedEndpoint},
    peers::Peer,
    service_announcement::Announcement,
};
use anyhow::{Context, Result, ensure};
use futures_util::{StreamExt, stream};
use serde::Serialize;
use std::time::Duration;
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Unchanged,
    Relocated,
    Withdrawn,
    Unavailable,
    ApprovalChanged,
}
impl Repository {
    pub async fn discovery_peers(&self, after: Option<Id>, limit: i64) -> Result<Vec<Peer>> {
        ensure!((1..=64).contains(&limit), "discovery page limit");
        let after = after.map(|id| id.to_vec());
        self.pool.get().await?.query("SELECT service_key,endpoint,version,discovered_endpoint FROM approved_peers WHERE allowed AND ($1::bytea IS NULL OR service_key>$1) ORDER BY service_key LIMIT $2",&[&after,&limit]).await?.into_iter().map(|r| Ok(Peer{key:r.get::<_,Vec<u8>>(0).try_into().map_err(|_|anyhow::anyhow!("peer key"))?,endpoint:r.get(1),version:r.get(2),discovered_endpoint:r.get(3)})).collect()
    }
    async fn apply_discovered_peer(
        &self,
        old: &Peer,
        announcement: &Announcement,
        endpoint: &VerifiedEndpoint,
    ) -> Result<Outcome> {
        ensure!(
            endpoint.service_key() == old.key
                && announcement.service_key() == old.key
                && announcement
                    .endpoints()
                    .iter()
                    .any(|e| e == endpoint.origin()),
            "relocation identity/announcement"
        );
        self.remember_service_identity(endpoint).await?;
        let mut connection = self.pool.get().await?;
        let tx = connection.transaction().await?;
        // Same lock as announcement replacement: a withdrawal/newer record
        // cannot race past the final endpoint selection check.
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended('manga:service-announcements:v1',0))",
            &[],
        )
        .await?;
        let latest = tx
            .query_opt(
                "SELECT event_id FROM service_announcements WHERE service_key=$1",
                &[&old.key.as_slice()],
            )
            .await?
            .context("announcement disappeared")?;
        if latest.get::<_, Vec<u8>>(0) != hex::decode(&announcement.event().id)? {
            return Ok(Outcome::Unavailable);
        }
        let row=tx.query_opt("SELECT endpoint,version,allowed FROM approved_peers WHERE service_key=$1 FOR UPDATE",&[&old.key.as_slice()]).await?.context("peer disappeared")?;
        if !row.get::<_, bool>(2)
            || row.get::<_, i64>(1) != old.version
            || row.get::<_, String>(0) != old.endpoint
        {
            return Ok(Outcome::ApprovalChanged);
        }
        if endpoint.origin() == old.endpoint {
            return Ok(Outcome::Unchanged);
        }
        tx.execute(
            "UPDATE approved_peers SET endpoint=$2,discovered_endpoint=true WHERE service_key=$1",
            &[&old.key.as_slice(), &endpoint.origin()],
        )
        .await?;
        tx.commit().await?;
        Ok(Outcome::Relocated)
    }
}
/// At most nine independently approved seed identities (target plus eight
/// others). Their signed target records may update cache; only target identity
/// proof and unchanged local approval can move the configured endpoint.
pub async fn refresh(repo: &Repository, policy: &EndpointPolicy, key: Id) -> Result<Outcome> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let peer = repo.approved_peer(key).await?;
        let mut seeds = vec![peer.clone()];
        seeds.extend(
            repo.discovery_peers(None, 8)
                .await?
                .into_iter()
                .filter(|p| p.key != key),
        );
        let tasks = stream::iter(seeds)
            .map(|seed| {
                let repo = repo.clone();
                let policy = policy.clone();
                async move {
                    tokio::time::timeout(Duration::from_secs(8), async {
                        let endpoint = policy.connect(&seed.endpoint, Some(seed.key)).await?;
                        repo.remember_service_identity(&endpoint).await?;
                        let bytes = endpoint
                            .get_metadata(&format!("/service/announcements/{}", hex::encode(key)))
                            .await?;
                        let record = Announcement::parse(serde_json::from_slice(&bytes)?)?;
                        ensure!(record.service_key() == key, "wrong target announcement");
                        repo.cache_service_announcement(&record).await?;
                        Ok::<_, anyhow::Error>(())
                    })
                    .await
                }
            })
            .buffer_unordered(4);
        tokio::pin!(tasks);
        while tasks.next().await.is_some() {}
        let Some(record) = repo.service_announcement(key).await? else {
            return Ok(Outcome::Unavailable);
        };
        if record.endpoints().is_empty() {
            return Ok(Outcome::Withdrawn);
        }
        for endpoint in record.endpoints() {
            let Ok(verified) = policy.connect(endpoint, Some(key)).await else {
                continue;
            };
            match repo.apply_discovered_peer(&peer, &record, &verified).await {
                Err(error) if error.is::<crate::service_identity::IdentityChanged>() => continue,
                result => return result,
            }
        }
        Ok(Outcome::Unavailable)
    })
    .await
    .context("peer discovery deadline")?
}
/// Four-peer keyset quanta; one operation at a time, bounded by refresh deadline.
/// No immediate startup migration; paused sync configuration pauses this worker.
pub async fn run(repo: Repository, policy: EndpointPolicy) {
    let mut after = None;
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        let peers = match repo.discovery_peers(after, 4).await {
            Ok(p) => p,
            Err(error) => {
                tracing::warn!(error=%error,"peer discovery page unavailable");
                continue;
            }
        };
        if peers.is_empty() {
            after = None;
            continue;
        }
        for peer in peers {
            after = Some(peer.key);
            match refresh(&repo, &policy, peer.key).await {
                Ok(Outcome::Relocated) => {
                    tracing::info!(service=%hex::encode(peer.key),"approved peer endpoint relocated")
                }
                Ok(_) => {}
                Err(error) => tracing::warn!(error=%error,"peer discovery deferred"),
            }
        }
    }
}
