//! Explicit recurring policies. Merely approving a peer never starts replication.
use crate::{
    auth,
    db::{Id, Repository},
    sync_jobs::{SyncSpec, enqueue_sync_tx},
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleSpec {
    pub spec: SyncSpec,
    pub interval_seconds: i64,
    pub enabled: bool,
}
impl Repository {
    pub async fn save_sync_schedule(&self, request: ScheduleSpec) -> Result<Id> {
        ensure!(
            (30..=86400).contains(&request.interval_seconds),
            "schedule interval must be30..86400seconds"
        );
        self.validate_sync_spec(&request.spec).await?;
        let spec = serde_json::to_value(&request.spec)?;
        let id: Id = hex::decode(auth::hash(&serde_json::to_vec(&spec)?))?
            .try_into()
            .unwrap();
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        tx.execute("SELECT pg_advisory_xact_lock(697241115)", &[])
            .await?;
        let existing = tx
            .query_opt(
                "SELECT enabled FROM sync_schedules WHERE id=$1 FOR UPDATE",
                &[&id.as_slice()],
            )
            .await?;
        if existing.is_none() {
            ensure!(
                tx.query_one("SELECT count(*) FROM sync_schedules", &[])
                    .await?
                    .get::<_, i64>(0)
                    < 256,
                "schedule record budget"
            );
        }
        if request.enabled && !existing.is_some_and(|r| r.get::<_, bool>(0)) {
            ensure!(
                tx.query_one("SELECT count(*) FROM sync_schedules WHERE enabled", &[])
                    .await?
                    .get::<_, i64>(0)
                    < 32,
                "enabled schedule budget"
            );
        }
        tx.execute("INSERT INTO sync_schedules(id,spec,interval_seconds,enabled) VALUES($1,$2,$3,$4) ON CONFLICT(id) DO UPDATE SET interval_seconds=EXCLUDED.interval_seconds,enabled=EXCLUDED.enabled,revision=sync_schedules.revision+1,next_run=clock_timestamp(),updated_at=clock_timestamp()",&[&id.as_slice(),&spec,&request.interval_seconds,&request.enabled]).await?;
        tx.commit().await?;
        Ok(id)
    }
    pub async fn enable_sync_schedule(&self, id: Id, enabled: bool) -> Result<()> {
        if enabled {
            let spec: Value = self
                .pool
                .get()
                .await?
                .query_opt(
                    "SELECT spec FROM sync_schedules WHERE id=$1",
                    &[&id.as_slice()],
                )
                .await?
                .context("schedule missing")?
                .get(0);
            self.validate_sync_spec(&serde_json::from_value(spec)?)
                .await?;
        }
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        tx.execute("SELECT pg_advisory_xact_lock(697241115)", &[])
            .await?;
        let old: bool = tx
            .query_opt(
                "SELECT enabled FROM sync_schedules WHERE id=$1 FOR UPDATE",
                &[&id.as_slice()],
            )
            .await?
            .context("schedule missing")?
            .get(0);
        if enabled && !old {
            ensure!(
                tx.query_one("SELECT count(*) FROM sync_schedules WHERE enabled", &[])
                    .await?
                    .get::<_, i64>(0)
                    < 32,
                "enabled schedule budget"
            );
        }
        tx.execute("UPDATE sync_schedules SET enabled=$2,revision=revision+1,next_run=clock_timestamp(),updated_at=clock_timestamp() WHERE id=$1",&[&id.as_slice(),&enabled]).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn sync_schedules(&self) -> Result<Vec<Value>> {
        Ok(self.pool.get().await?.query("SELECT s.id,s.spec,s.interval_seconds,s.enabled,s.revision,extract(epoch FROM s.next_run)::bigint,s.last_batch,s.last_error,s.dispatches,b.state,b.last_error FROM sync_schedules s LEFT JOIN sync_batches b ON b.id=s.last_batch ORDER BY s.id LIMIT 256",&[]).await?.into_iter().map(|r|json!({"id":hex::encode(r.get::<_,Vec<u8>>(0)),"spec":r.get::<_,Value>(1),"interval_seconds":r.get::<_,i64>(2),"enabled":r.get::<_,bool>(3),"revision":r.get::<_,i64>(4),"next_run":r.get::<_,i64>(5),"last_batch":r.get::<_,Option<Vec<u8>>>(6).map(hex::encode),"dispatch_error":r.get::<_,Option<String>>(7),"dispatches":r.get::<_,i64>(8),"last_batch_state":r.get::<_,Option<String>>(9),"last_batch_error":r.get::<_,Option<String>>(10)})).collect())
    }
    pub async fn dispatch_sync_schedule(&self) -> Result<Option<Id>> {
        let Some(candidate)=self.pool.get().await?.query_opt("SELECT id,spec,revision FROM sync_schedules WHERE enabled AND next_run<=clock_timestamp() ORDER BY next_run,id LIMIT 1",&[]).await? else{return Ok(None)};
        let id: Vec<u8> = candidate.get(0);
        let revision: i64 = candidate.get(2);
        let spec: SyncSpec = serde_json::from_value(candidate.get(1))?;
        let valid = self.validate_sync_spec(&spec).await;
        let mut c = self.pool.get().await?;
        let mut tx = c.transaction().await?;
        if !tx
            .query_one("SELECT pg_try_advisory_xact_lock(697241115)", &[])
            .await?
            .get::<_, bool>(0)
        {
            return Ok(None);
        }
        let Some(row)=tx.query_opt("SELECT interval_seconds FROM sync_schedules WHERE id=$1 AND revision=$2 AND enabled AND next_run<=clock_timestamp() FOR UPDATE",&[&id,&revision]).await? else{return Ok(None)};
        let interval: i64 = row.get(0);
        let result = if let Err(error) = valid {
            Err(error)
        } else {
            let sub = tx.transaction().await?;
            match enqueue_sync_tx(&sub, &spec, true).await {
                Ok(batch) => {
                    sub.commit().await?;
                    Ok(batch)
                }
                Err(error) => {
                    sub.rollback().await?;
                    Err(error)
                }
            }
        };
        let (batch, error) = match result {
            Ok(id) => (Some(id.to_vec()), None),
            Err(error) => (
                None,
                Some(error.to_string().chars().take(240).collect::<String>()),
            ),
        };
        tx.execute("UPDATE sync_schedules SET next_run=clock_timestamp()+($2::bigint*interval '1 second'),last_batch=COALESCE($3,last_batch),last_error=$4,dispatches=dispatches+1,updated_at=clock_timestamp() WHERE id=$1",&[&id,&interval,&batch,&error]).await?;
        tx.commit().await?;
        Ok(Some(id.try_into().unwrap()))
    }
}
pub async fn run(repo: Repository) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tick.tick().await;
        for _ in 0..4 {
            match repo.dispatch_sync_schedule().await {
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(error) => {
                    tracing::warn!(error=%error,"sync schedule dispatch unavailable");
                    break;
                }
            }
        }
    }
}
