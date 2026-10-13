use crate::{config::Config, db::PgStore, error::ApiResult, protocol::ProtocolFacade};
use manga_node::{
    db::{DbConfig, Repository},
    read_admission::Admission,
};
use moka::future::Cache;
use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct AppState {
    pub db: PgStore,
    pub config: Arc<Config>,
    pub protocol: Arc<ProtocolFacade>,
    pub reads: Arc<Admission>,
    pub requests: Arc<Semaphore>,
    pub runtime: Arc<crate::runtime::RuntimeState>,
    /// Only immutable derived public values belong here, never sessions/authority.
    pub public_cache: Cache<String, Arc<serde_json::Value>>,
}

impl AppState {
    pub async fn connect(config: Config, dsn: String) -> anyhow::Result<Self> {
        config.validate()?;
        let repository = Repository::connect(DbConfig {
            dsn,
            connections: config.database_connections,
            acquisition_timeout: config.acquisition_timeout(),
            cache_weight: config.cache_bytes,
            plaintext_local: config.database_plaintext_local,
        })
        .await?;
        let reads = Arc::new(Admission::new(
            config.admission_config(),
            config.database_connections,
        )?);
        let requests = Arc::new(Semaphore::new(config.maximum_requests));
        let public_cache = Cache::builder()
            .max_capacity(config.cache_bytes)
            .weigher(|key: &String, value: &Arc<serde_json::Value>| {
                key.len()
                    .saturating_add(value.to_string().len())
                    .saturating_add(128)
                    .min(u32::MAX as usize) as u32
            })
            .build();
        Ok(Self {
            db: PgStore::new(repository.pool.clone()),
            config: Arc::new(config),
            protocol: Arc::new(ProtocolFacade::new(repository)),
            reads,
            requests,
            runtime: Arc::new(crate::runtime::RuntimeState::default()),
            public_cache,
        })
    }

    pub async fn health(&self) -> ApiResult<()> {
        self.db.health().await
    }
}
