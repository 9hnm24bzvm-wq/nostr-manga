//! Narrow integration boundary: adapters never write protocol authority tables.
use crate::error::{ApiError, ApiResult};
use manga_node::{
    auth::Event,
    catalogue::{Entry, Page, SeriesQuery},
    db::{Id, Repository},
    discovery::QueryOptions,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub struct ProtocolFacade {
    repository: Repository,
    schema_ready: Arc<AtomicBool>,
    runtime: Arc<tokio::sync::RwLock<Option<Arc<publication::PublicationRuntime>>>>,
}

impl ProtocolFacade {
    pub fn new(repository: Repository) -> Self {
        Self {
            repository,
            schema_ready: Arc::new(AtomicBool::new(false)),
            runtime: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    pub async fn verify_schema(&self) -> anyhow::Result<()> {
        self.repository.verify_schema().await?;
        self.schema_ready.store(true, Ordering::Release);
        Ok(())
    }

    pub async fn install_visibility_function(&self) -> anyhow::Result<()> {
        let sql = if self.is_ready() {
            manga_node::platform_bridge::visibility_function_sql()
        } else {
            "CREATE OR REPLACE FUNCTION platform.protocol_source_is_visible(source_event TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$ SELECT source_event IS NULL $$;".to_owned()
        };
        self.repository
            .pool
            .get()
            .await?
            .batch_execute(&sql)
            .await?;
        Ok(())
    }

    pub fn is_ready(&self) -> bool {
        self.schema_ready.load(Ordering::Acquire)
    }

    fn require_schema(&self) -> ApiResult<()> {
        if self.is_ready() {
            Ok(())
        } else {
            Err(ApiError::unavailable())
        }
    }

    pub async fn series(&self, query: SeriesQuery) -> ApiResult<Page> {
        self.require_schema()?;
        self.repository
            .catalogue_series(query)
            .await
            .map_err(|_| ApiError::unavailable())
    }

    pub async fn entry(&self, event: Id) -> ApiResult<Option<Entry>> {
        self.require_schema()?;
        self.repository
            .catalogue_entry(event)
            .await
            .map_err(|_| ApiError::unavailable())
    }

    pub async fn chapters(&self, query: QueryOptions) -> ApiResult<manga_node::discovery::Page> {
        self.require_schema()?;
        self.repository
            .discover(query)
            .await
            .map_err(|_| ApiError::unavailable())
    }

    pub async fn event(&self, event: Id) -> ApiResult<Option<Arc<Event>>> {
        self.require_schema()?;
        self.repository
            .visible_event(event)
            .await
            .map_err(|_| ApiError::unavailable())
    }

    pub async fn compatibility_id(&self, kind: &str, target: &str) -> ApiResult<String> {
        self.require_schema()?;
        self.repository
            .compatibility_id(kind, target)
            .await
            .map_err(|_| ApiError::unavailable())
    }

    pub async fn compatibility_target(
        &self,
        kind: &str,
        id: &str,
    ) -> ApiResult<Option<(String, serde_json::Value)>> {
        self.require_schema()?;
        if !manga_node::mangadex_compat::valid_uuid(id) {
            return Err(ApiError::bad_request("Invalid resource identifier"));
        }
        self.repository
            .compatibility_target(kind, id)
            .await
            .map_err(|_| ApiError::unavailable())
    }
}

pub mod bridge;
pub mod media;
pub mod publication;

pub mod covers;
pub mod deletions;

pub mod adapters;

pub mod http_auth;

pub mod library;
pub mod list_aliases;
pub mod profiles;
