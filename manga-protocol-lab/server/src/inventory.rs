//! Stable bounded snapshots are distinct from live-offset discovery. PostgreSQL
//! owns session scope/expiry/budgets; Moka caches only immutable snapshot content.
use crate::{
    db::{Id, Repository},
    filter::{Filter, Value},
};
use anyhow::{Context, Result, ensure};
use moka::future::Cache;
use negentropy::{Negentropy, NegentropyStorageBase, NegentropyStorageVector};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
pub const MAX_ITEMS: usize = 100_000;
pub const FRAME_BYTES: usize = 65536;
#[derive(Debug, thiserror::Error)]
#[error("inventory exceeds partition budget; split event-ID prefix")]
pub struct PartitionRequired;
pub fn prefix_bounds(prefix: &str) -> Result<(Id, Id)> {
    ensure!(
        prefix.len() <= 64
            && prefix
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid inventory prefix"
    );
    let low = hex::decode(format!("{prefix}{}", "0".repeat(64 - prefix.len())))?
        .try_into()
        .unwrap();
    let high = hex::decode(format!("{prefix}{}", "f".repeat(64 - prefix.len())))?
        .try_into()
        .unwrap();
    Ok((low, high))
}
pub fn storage(rows: &[(u64, Id)]) -> Result<NegentropyStorageVector> {
    ensure!(rows.len() <= MAX_ITEMS, "snapshot item budget");
    let mut storage = NegentropyStorageVector::with_capacity(rows.len());
    for (time, id) in rows {
        storage.insert(*time, negentropy::Id::from_slice(id)?)?;
    }
    storage.seal()?;
    Ok(storage)
}
fn pack(rows: &[(u64, Id)]) -> Vec<u8> {
    let mut data = Vec::with_capacity(rows.len() * 40);
    for (time, id) in rows {
        data.extend_from_slice(&time.to_be_bytes());
        data.extend_from_slice(id);
    }
    data
}
fn unpack(data: &[u8]) -> Result<NegentropyStorageVector> {
    ensure!(
        data.len().is_multiple_of(40) && data.len() <= MAX_ITEMS * 40,
        "invalid snapshot storage"
    );
    let rows = data
        .chunks_exact(40)
        .map(|b| {
            (
                u64::from_be_bytes(b[..8].try_into().unwrap()),
                b[8..].try_into().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    storage(&rows)
}
impl Repository {
    pub async fn inventory_rows_scoped(
        &self,
        scope: &str,
        filter: &str,
        prefix: &str,
    ) -> Result<Vec<(u64, Id)>> {
        let kinds = inventory_kinds(scope)?
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let (low, high) = prefix_bounds(prefix)?;
        let (compiled, mut args) = Filter::new(filter)
            .and_then(Filter::compile)
            .map_err(anyhow::Error::msg)?;
        args.push(Value::Bytes(low.to_vec()));
        let lo = args.len();
        args.push(Value::Bytes(high.to_vec()));
        let hi = args.len();
        let params: Vec<_> = args.iter().map(Value::parameter).collect();
        let mut client = self.pool.get().await?;
        let tx = client.build_transaction().read_only(true).start().await?;
        tx.batch_execute("SET LOCAL statement_timeout='2s'; SET LOCAL work_mem='4MB'")
            .await?;
        let rows=tx.query(&format!("SELECT created_at,id FROM (SELECT v.*,m.series,m.language,m.chapter,m.pages,m.size FROM relay_visible_events v LEFT JOIN events m ON m.id=v.id) e WHERE kind IN ({kinds}) AND id>=${lo} AND id<=${hi} AND ({compiled}) AND NOT EXISTS(SELECT 1 FROM backend_outbox o WHERE o.event_id=e.id AND o.completed_at IS NULL) ORDER BY id LIMIT 100001"),&params).await?;
        if rows.len() > MAX_ITEMS {
            return Err(PartitionRequired.into());
        }
        let rows = rows
            .into_iter()
            .map(|r| {
                Ok((
                    u64::try_from(r.get::<_, i64>(0))?,
                    r.get::<_, Vec<u8>>(1)
                        .try_into()
                        .ok()
                        .context("invalid stored event ID")?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        tx.commit().await?;
        Ok(rows)
    }
}
pub fn inventory_kinds(scope: &str) -> Result<&'static [i32]> {
    match scope {
        "chapters" => Ok(&[1001]),
        "series" => Ok(&[1002]),
        "profiles" => Ok(&[0, 3, 10002]),
        "social" => Ok(&[1, 7, 1111]),
        "libraries" => Ok(&[11999]),
        "deletions" => Ok(&[5]),
        _ => anyhow::bail!("unsupported inventory"),
    }
}
pub fn inventory_kind(scope: &str) -> Result<i32> {
    match scope {
        "chapters" => Ok(1001),
        "series" => Ok(1002),
        _ => anyhow::bail!("unsupported inventory"),
    }
}
pub fn inventory_name() -> String {
    "chapters".into()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconcileRequest {
    pub session: Option<String>,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub filter: String,
    #[serde(default = "inventory_name")]
    pub inventory: String,
    pub message: Option<String>,
    #[serde(default)]
    pub close: bool,
}
#[derive(Serialize, Deserialize)]
pub struct ReconcileReply {
    pub session: String,
    pub prefix: String,
    pub message: String,
    pub snapshot: bool,
    pub closed: bool,
}
pub struct Snapshots {
    repo: Repository,
    cache: Cache<Id, Arc<NegentropyStorageVector>>,
    pub slots: Arc<tokio::sync::Semaphore>,
}
impl Snapshots {
    pub fn new(repo: Repository) -> Arc<Self> {
        Arc::new(Self {
            repo,
            cache: Cache::builder()
                .max_capacity(64 * 1024 * 1024)
                .weigher(|_: &Id, s: &Arc<NegentropyStorageVector>| {
                    (s.size().unwrap_or(MAX_ITEMS) * 48 + 128) as u32
                })
                .time_to_live(Duration::from_secs(60))
                .build(),
            slots: Arc::new(tokio::sync::Semaphore::new(4)),
        })
    }
    async fn start(
        &self,
        peer: Id,
        epoch: i64,
        filter: &str,
        prefix: &str,
        scope: &str,
    ) -> Result<Id> {
        let health_epoch = self.repo.storage_health_epoch().await?;
        let rows = self
            .repo
            .inventory_rows_scoped(scope, filter, prefix)
            .await?;
        let bytes = pack(&rows);
        let session: Id = rand::random();
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        let permission = tx
            .query_one(
                "SELECT allowed,version FROM approved_peers WHERE service_key=$1 FOR UPDATE",
                &[&peer.as_slice()],
            )
            .await?;
        ensure!(
            permission.get::<_, bool>(0) && permission.get::<_, i64>(1) == epoch,
            "peer approval changed"
        );
        let current: i64 = tx
            .query_one(
                "SELECT epoch FROM storage_health_epoch WHERE singleton FOR SHARE",
                &[],
            )
            .await?
            .get(0);
        ensure!(
            current == health_epoch,
            "storage health changed; restart snapshot"
        );
        let active:i64=tx.query_one("SELECT COUNT(*) FROM inventory_snapshot_slots WHERE peer=$1 AND expires_at>clock_timestamp()", &[&peer.as_slice()]).await?.get(0);
        ensure!(active < 2, "per-peer snapshot session budget exhausted");
        let slot=tx.query_opt("SELECT slot FROM inventory_snapshot_slots WHERE expires_at<=clock_timestamp() ORDER BY slot FOR UPDATE SKIP LOCKED LIMIT 1",&[]).await?.context("global snapshot session budget exhausted")?.get::<_,i16>(0);
        tx.execute("UPDATE inventory_snapshot_slots SET session_id=$2,peer=$3,peer_epoch=$4,prefix=$5,filter=$6,snapshot=$7,health_epoch=$8,inventory=$9,expires_at=clock_timestamp()+interval '30 seconds',rounds=0,protocol_bytes=0 WHERE slot=$1", &[&slot,&session.as_slice(),&peer.as_slice(),&epoch,&prefix,&filter,&bytes,&health_epoch,&scope]).await?;
        tx.commit().await?;
        self.cache.insert(session, Arc::new(storage(&rows)?)).await;
        Ok(session)
    }
    pub async fn reconcile(
        &self,
        peer: Id,
        epoch: i64,
        request: ReconcileRequest,
    ) -> Result<ReconcileReply> {
        inventory_kinds(&request.inventory)?;
        prefix_bounds(&request.prefix)?;
        if request.close {
            let session = request.session.context("session required")?;
            let id: Id = hex::decode(&session)?
                .try_into()
                .ok()
                .context("session ID")?;
            let changed=self.repo.pool.get().await?.execute("UPDATE inventory_snapshot_slots SET expires_at='-infinity',snapshot=''::bytea WHERE session_id=$1 AND peer=$2 AND inventory=$3", &[&id.as_slice(),&peer.as_slice(),&request.inventory]).await?;
            if changed == 1 {
                self.cache.invalidate(&id).await;
            }
            return Ok(ReconcileReply {
                session,
                prefix: request.prefix,
                message: String::new(),
                snapshot: true,
                closed: true,
            });
        }
        let message = request.message.context("message required")?;
        ensure!(message.len() <= FRAME_BYTES * 2, "frame budget");
        let input = hex::decode(message)?;
        let id: Id = if let Some(session) = request.session {
            hex::decode(session)?
                .try_into()
                .ok()
                .context("session ID")?
        } else {
            self.start(
                peer,
                epoch,
                &request.filter,
                &request.prefix,
                &request.inventory,
            )
            .await?
        };
        let client = self.repo.pool.get().await?;
        let scope=client.query_opt("SELECT peer,peer_epoch,prefix,filter,health_epoch,inventory FROM inventory_snapshot_slots WHERE session_id=$1 AND expires_at>clock_timestamp()", &[&id.as_slice()]).await?.context("session expired; restart reconciliation")?;
        ensure!(
            scope.get::<_, Vec<u8>>(0) == peer
                && scope.get::<_, i64>(1) == epoch
                && scope.get::<_, String>(2) == request.prefix
                && scope.get::<_, String>(3) == request.filter
                && scope.get::<_, String>(5) == request.inventory,
            "snapshot scope mismatch"
        );
        let snapshot_health: i64 = scope.get(4);
        ensure!(
            snapshot_health
                == client
                    .query_one(
                        "SELECT epoch FROM storage_health_epoch WHERE singleton",
                        &[]
                    )
                    .await?
                    .get::<_, i64>(0),
            "storage health changed; restart reconciliation"
        );
        let snapshot = if let Some(value) = self.cache.get(&id).await {
            value
        } else {
            let bytes: Vec<u8> = client
                .query_one(
                    "SELECT snapshot FROM inventory_snapshot_slots WHERE session_id=$1",
                    &[&id.as_slice()],
                )
                .await?
                .get(0);
            let value = Arc::new(unpack(&bytes)?);
            self.cache.insert(id, value.clone()).await;
            value
        };
        drop(client);
        let input_size = input.len();
        // The responder resets its timestamp decoder on each frame; immutable storage
        // is sufficient to resume a session on a different API process after restart.
        let output = crate::node::spawn_blocking_owned_scope(move || -> Result<Vec<u8>> {
            let mut neg = Negentropy::borrowed(&*snapshot, FRAME_BYTES as u64)?;
            Ok(neg.reconcile(&input)?)
        })
        .await??;
        ensure!(output.len() <= FRAME_BYTES, "response frame budget");
        let bytes = (input_size + output.len()) as i64;
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        let permission = tx
            .query_one(
                "SELECT allowed,version FROM approved_peers WHERE service_key=$1 FOR SHARE",
                &[&peer.as_slice()],
            )
            .await?;
        ensure!(
            permission.get::<_, bool>(0) && permission.get::<_, i64>(1) == epoch,
            "peer approval changed"
        );
        let current_health: i64 = tx
            .query_one(
                "SELECT epoch FROM storage_health_epoch WHERE singleton FOR SHARE",
                &[],
            )
            .await?
            .get(0);
        ensure!(
            current_health == snapshot_health,
            "storage health changed during frame; restart reconciliation"
        );
        let changed=tx.execute("UPDATE inventory_snapshot_slots SET rounds=rounds+1,protocol_bytes=protocol_bytes+$4 WHERE session_id=$1 AND peer=$2 AND peer_epoch=$3 AND expires_at>clock_timestamp() AND rounds<256 AND protocol_bytes+$4<=16777216", &[&id.as_slice(),&peer.as_slice(),&epoch,&bytes]).await?;
        ensure!(
            changed == 1,
            "session expired or round/byte budget exhausted"
        );
        tx.commit().await?;
        Ok(ReconcileReply {
            session: hex::encode(id),
            prefix: request.prefix,
            message: hex::encode(output),
            snapshot: true,
            closed: false,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nibble_prefix_bounds() {
        let (low, high) = prefix_bounds("a1f").unwrap();
        assert_eq!(&hex::encode(low)[..4], "a1f0");
        assert_eq!(&hex::encode(high)[..4], "a1ff");
        for bad in ["A", "g", "0x"] {
            assert!(prefix_bounds(bad).is_err());
        }
        assert!(prefix_bounds(&"0".repeat(65)).is_err());
    }
}
