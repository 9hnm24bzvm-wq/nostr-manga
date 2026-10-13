//! Optional original-reader role. Prepared ownership precedes database startup.
use crate::{
    db::{Id, Repository},
    node::NodeConfig,
    relay::ReadPolicy,
    serving::{ManagedServing, ServingConfig},
    upload::UploadStore,
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{path::PathBuf, sync::Arc, time::Duration};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicServingConfig {
    pub enabled: bool,
    #[serde(default = "python")]
    pub python: PathBuf,
    pub helper: PathBuf,
    pub nginx: PathBuf,
    pub bind: String,
    #[serde(default = "workers")]
    pub workers: u8,
    #[serde(default = "shutdown_seconds")]
    pub shutdown_seconds: u8,
}
fn python() -> PathBuf {
    PathBuf::from("python3")
}
fn workers() -> u8 {
    1
}
fn shutdown_seconds() -> u8 {
    30
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum ServingPhase {
    Prepared = 0,
    Active = 1,
    Stopping = 2,
    Stopped = 3,
    FailedClosed = 4,
    ShutdownUnconfirmed = 5,
    Activating = 6,
}
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[repr(u8)]
enum ServingFault {
    #[error("storage verification requires recovery")]
    StorageVerification = 1,
    #[error("public namespace reconciliation incomplete")]
    NamespaceReconciliation = 2,
    #[error("reader drain not verified")]
    ReaderDrain = 3,
    #[error("reader activation failed")]
    Activation = 4,
    #[error("active reader health check failed")]
    ReaderHealth = 5,
    #[error("serving task ended unexpectedly")]
    TaskExited = 6,
}
pub struct ServingStatus {
    enabled: bool,
    state: std::sync::atomic::AtomicU8,
    fault: std::sync::atomic::AtomicU8,
}
impl ServingStatus {
    fn set(&self, state: ServingPhase) {
        self.state
            .store(state as u8, std::sync::atomic::Ordering::Relaxed);
    }
    fn failed(&self, fault: ServingFault) {
        self.fault
            .store(fault as u8, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn snapshot(&self) -> serde_json::Value {
        let state = match self.state.load(std::sync::atomic::Ordering::Relaxed) {
            0 => "prepared",
            1 => "active",
            2 => "stopping",
            3 => "stopped",
            4 => "failed_closed",
            6 => "activating",
            _ => "shutdown_unconfirmed",
        };
        let fault = match self.fault.load(std::sync::atomic::Ordering::Relaxed) {
            0 => None,
            1 => Some("storage_verification"),
            2 => Some("namespace_reconciliation"),
            3 => Some("reader_drain"),
            4 => Some("activation"),
            5 => Some("reader_health"),
            _ => Some("task_exited"),
        };
        serde_json::json!({"enabled":self.enabled,"state":state,"fault":fault,"automatic_reactivation":false,"recovery_requires_verified_reader_shutdown":true})
    }
}
pub struct ServingRole {
    status: Arc<ServingStatus>,
    family: Arc<ManagedServing>,
    enabled: bool,
    task: Option<tokio::task::JoinHandle<()>>,
    stop: tokio::sync::watch::Sender<bool>,
}
impl ServingRole {
    /// No DB operations: stale readers must be recovered even when PG is down.
    pub async fn prepare(config: &NodeConfig) -> Result<Option<Self>> {
        let Some(cfg) = &config.public_serving else {
            if let Some(storage) = &config.storage {
                ensure!(
                    !storage.path.join("public/.serving").try_exists()?,
                    "managed serving state exists; retain public_serving configuration with enabled=false to recover safely"
                );
            }
            return Ok(None);
        };
        ensure!(
            !cfg.enabled || config.read_policy == ReadPolicy::Public,
            "direct originals require public read policy"
        );
        let storage = config
            .storage
            .as_ref()
            .context("managed serving requires local storage")?;
        ensure!(
            !cfg.enabled || config.backend.is_some(),
            "direct originals require the publication backend"
        );
        let owner: Id = hex::decode(&config.admin_pubkey)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid administrator key"))?;
        let public = if storage.path.is_absolute() {
            storage.path.join("public")
        } else {
            std::env::current_dir()?.join(&storage.path).join("public")
        };
        let family = Arc::new(
            ManagedServing::prepare(ServingConfig {
                python: cfg.python.clone(),
                helper: cfg.helper.clone(),
                nginx: cfg.nginx.clone(),
                runtime: public.join(".serving"),
                public,
                bind: cfg.bind.clone(),
                owner,
                workers: cfg.workers,
                shutdown_seconds: cfg.shutdown_seconds,
            })
            .await?,
        );
        let (stop, _) = tokio::sync::watch::channel(false);
        Ok(Some(Self {
            status: Arc::new(ServingStatus {
                enabled: cfg.enabled,
                state: std::sync::atomic::AtomicU8::new(ServingPhase::Prepared as u8),
                fault: std::sync::atomic::AtomicU8::new(0),
            }),
            family,
            enabled: cfg.enabled,
            task: None,
            stop,
        }))
    }
    pub fn status(&self) -> Arc<ServingStatus> {
        self.status.clone()
    }
    pub async fn run(&mut self, store: Arc<UploadStore>, repo: Repository) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        store.configure_public_export_role().await?;
        let family = self.family.clone();
        let status = self.status.clone();
        let mut stopping = self.stop.subscribe();
        let fault = store.public_faults();
        let mut fault_changes = fault.clone();
        self.task = Some(crate::node::spawn_infrastructure_owned_scope(async move {
            let operation = async {
                let mut active = false;
                let mut heartbeat = tokio::time::Instant::now();
                loop {
                    ensure!(!*fault.borrow(), ServingFault::StorageVerification);
                    let step = tokio::time::timeout(
                        Duration::from_secs(10),
                        store.reconcile_public_exports(),
                    )
                    .await
                    .context(ServingFault::NamespaceReconciliation)?
                    .context(ServingFault::NamespaceReconciliation)?;
                    ensure!(step.retried == 0, ServingFault::NamespaceReconciliation);
                    tokio::time::timeout(
                        family.operation_deadline(),
                        store.drain_public_originals(family.clone()),
                    )
                    .await
                    .context(ServingFault::ReaderDrain)?
                    .context(ServingFault::ReaderDrain)?;
                    if !active
                        && repo
                            .public_export_idle(
                                store.owner(),
                                store.volume_name(),
                                store.public_volume_identity(),
                            )
                            .await
                            .context(ServingFault::NamespaceReconciliation)?
                    {
                        ensure!(!*fault.borrow(), ServingFault::StorageVerification);
                        status.set(ServingPhase::Activating);
                        family.activate().await.context(ServingFault::Activation)?;
                        active = true;
                        status.set(ServingPhase::Active);
                        tracing::info!("managed original reader activated after reconciliation");
                    }
                    if active && heartbeat.elapsed() >= Duration::from_secs(1) {
                        family
                            .check_active()
                            .await
                            .context(ServingFault::ReaderHealth)?;
                        heartbeat = tokio::time::Instant::now();
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                #[allow(unreachable_code)]
                Ok::<(), anyhow::Error>(())
            };
            let failed = tokio::select! {
                biased;
                _=stopping.changed()=>false,
                _=fault_changes.changed()=>{status.failed(ServingFault::StorageVerification);tracing::error!("storage verification failure; stopping direct original serving");true},
                result=operation=>{status.failed(result.as_ref().err().and_then(|e|e.downcast_ref::<ServingFault>()).copied().unwrap_or(ServingFault::TaskExited));if let Err(error)=result{tracing::error!(error=%error,"direct serving failure; withdrawing reader family");}true}
            };
            status.set(ServingPhase::Stopping);
            if let Err(error) = family.shutdown_verified().await {
                status.set(ServingPhase::ShutdownUnconfirmed);
                tracing::error!(error=%error,"reader shutdown unconfirmed; holds retained");
            } else {
                status.set(if failed {
                    ServingPhase::FailedClosed
                } else {
                    ServingPhase::Stopped
                });
            }
        }));
        Ok(())
    }
    pub async fn shutdown(&mut self) -> Result<()> {
        self.stop.send_replace(true);
        if let Some(task) = self.task.take() {
            if let Err(error) = task.await {
                self.status.failed(ServingFault::TaskExited);
                self.status.set(ServingPhase::ShutdownUnconfirmed);
                return Err(error.into());
            }
        } else {
            self.status.set(ServingPhase::Stopping);
            if let Err(error) = self.family.shutdown_verified().await {
                self.status.set(ServingPhase::ShutdownUnconfirmed);
                return Err(error);
            }
            self.status.set(ServingPhase::Stopped);
        }
        Ok(())
    }
}
impl Drop for ServingRole {
    fn drop(&mut self) {
        self.stop.send_replace(true);
        // The worker owns orderly shutdown. An already poisoned channel closes
        // stdin and the supervisor observes EOF independently of an active drain.
    }
}
