//! Explicitly configured original-module publication, never unsigned federation.
//! This runtime must be the sole mutation owner of its original storage volume.
use super::ProtocolFacade;
use crate::error::{ApiError, ApiResult};
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth::{self, Event},
    db::{Id, Repository},
    groups::{GroupPublication, Groups},
    publication::Publications,
    upload::UploadStore,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use zeroize::Zeroizing;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorRuntimeConfig {
    pub operator_key_file: PathBuf,
    pub expected_publisher: String,
    pub storage_owner: String,
    pub storage_holder: String,
    /// Original manga-node StorageConfig, parsed and validated by that module.
    pub storage: serde_json::Value,
    pub group_keys_dir: Option<PathBuf>,
    /// Retired platform-only alternative. Non-null configuration is rejected;
    /// projection belongs to the supervised original native_node backend.
    pub backend: Option<serde_json::Value>,
    /// Original NodeConfig. Its listener shares this exact pool/storage owner.
    pub native_node: Option<serde_json::Value>,
}
fn identity(text: &str) -> Result<Id> {
    ensure!(
        auth::valid_hash(text),
        "invalid configured protocol identity"
    );
    hex::decode(text)?
        .try_into()
        .ok()
        .context("invalid configured protocol identity")
}
fn load_signer(path: &Path, expected: &str) -> Result<Zeroizing<String>> {
    let metadata =
        std::fs::symlink_metadata(path).context("configured publication signer is unavailable")?;
    ensure!(
        metadata.file_type().is_file() && metadata.len() <= 128,
        "invalid publication signer file"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            metadata.permissions().mode() & 0o077 == 0,
            "publication signer must not be group/world accessible"
        );
    }
    let raw = Zeroizing::new(
        std::fs::read_to_string(path).context("configured publication signer is unavailable")?,
    );
    let secret = Zeroizing::new(raw.trim().to_owned());
    let event = auth::sign(&secret, 1, vec![], String::new())
        .map_err(|_| anyhow::anyhow!("invalid publication signer"))?;
    ensure!(
        event.pubkey == expected,
        "configured publication signer identity mismatch"
    );
    Ok(secret)
}
pub struct PublicationInput {
    pub kind: u32,
    pub canonical_manifest: Vec<u8>,
    pub originals: BTreeMap<String, Vec<u8>>,
    pub created_at: Option<u64>,
}
#[derive(Debug, Serialize)]
pub struct PublicationReceipt {
    pub event: Event,
    pub newly_hosted: bool,
    pub locally_visible: bool,
    pub state: &'static str,
}
pub struct PublicationRuntime {
    repository: Repository,
    storage: Arc<UploadStore>,
    pub(super) library_objects: Arc<manga_node::library_storage::LibraryObjects>,
    publications: Arc<Publications>,
    groups: Option<Arc<Groups>>,
    peer_sync: Arc<manga_node::peers::PeerSync>,
    secret: Zeroizing<String>,
    publisher: String,
    native_inputs: Arc<tokio::sync::Semaphore>,
    native_node: Option<serde_json::Value>,
    storage_holder: String,
}
pub struct NativeListener {
    pub task: tokio::task::JoinHandle<Result<()>>,
    pub shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}
impl PublicationRuntime {
    /// Never generates credentials, admits a publisher, approves a peer, or binds
    /// a platform login to a signer. Those are authenticated operator provisions.
    pub async fn open(repository: Repository, config: OperatorRuntimeConfig) -> Result<Arc<Self>> {
        ensure!(
            config.backend.is_none(),
            "publication.backend is retired; configure the supervised native_node.backend instead"
        );
        if let Some(value) = config.native_node.as_ref() {
            let native: manga_node::node::NodeConfig = serde_json::from_value(value.clone())
                .context("invalid embedded original node configuration")?;
            match (&config.group_keys_dir, &native.group_keys_dir) {
                (None, None) => {}
                (Some(runtime), Some(node)) => {
                    ensure!(
                        runtime
                            .canonicalize()
                            .context("configured group key root is unavailable")?
                            == node
                                .canonicalize()
                                .context("embedded node group key root is unavailable")?,
                        "platform and original listener must use the same configured group key root"
                    );
                }
                _ => anyhow::bail!(
                    "platform and original listener group capabilities must have matching key roots"
                ),
            }
        }
        repository.verify_schema().await?;
        let publisher = identity(&config.expected_publisher)?;
        repository
            .publisher_permit(publisher)
            .await
            .context("configured publisher must already be admitted by the original protocol")?;
        let expected = config.expected_publisher.clone();
        let path = config.operator_key_file.clone();
        let secret = tokio::task::spawn_blocking(move || load_signer(&path, &expected)).await??;
        let storage_config = serde_json::from_value(config.storage)
            .context("invalid original storage configuration")?;
        let storage = UploadStore::open(
            repository.clone(),
            storage_config,
            identity(&config.storage_owner)?,
            identity(&config.storage_holder)?,
        )
        .await?;
        let library_limits = config
            .native_node
            .as_ref()
            .map(|value| {
                serde_json::from_value::<manga_node::node::NodeConfig>(value.clone())
                    .map(|node| node.library_concurrency)
            })
            .transpose()?
            .unwrap_or_default();
        let library_objects = manga_node::library_storage::LibraryObjects::configured(
            repository.clone(),
            storage.clone(),
            library_limits,
        )?;
        let publications = Publications::new(repository.clone(), storage.clone());
        let groups = config
            .group_keys_dir
            .map(|path| {
                Groups::open(
                    repository.clone(),
                    path,
                    storage.clone(),
                    publications.clone(),
                )
            })
            .transpose()?;
        let peer_secret = if let Some(value) = config.native_node.as_ref() {
            let node: manga_node::node::NodeConfig = serde_json::from_value(value.clone())
                .context("invalid embedded original node configuration")?;
            let path = node.service_key_file;
            let holder = config.storage_holder.clone();
            // Outgoing federation proofs must use the explicitly configured
            // original service identity, not the distinct publication signer.
            tokio::task::spawn_blocking(move || load_signer(&path, &holder)).await??
        } else {
            Zeroizing::new(secret.to_string())
        };
        let peer_sync = manga_node::peers::PeerSync::new(
            repository.clone(),
            storage.clone(),
            publications.clone(),
            peer_secret,
        )?;
        Ok(Arc::new(Self {
            repository,
            storage,
            library_objects,
            publications,
            groups,
            peer_sync,
            secret,
            publisher: config.expected_publisher,
            native_inputs: Arc::new(tokio::sync::Semaphore::new(1)),
            native_node: config.native_node,
            storage_holder: config.storage_holder,
        }))
    }
    /// Injection from an already-owned protocol runtime avoids opening a second
    /// writer on a physical volume. The signer must match an operator identity.
    pub fn from_owned_store(
        repository: Repository,
        storage: Arc<UploadStore>,
        key_file: &Path,
        expected_publisher: &str,
        groups: Option<Arc<Groups>>,
    ) -> Result<Arc<Self>> {
        identity(expected_publisher)?;
        let secret = load_signer(key_file, expected_publisher)?;
        let library_objects =
            manga_node::library_storage::LibraryObjects::new(repository.clone(), storage.clone());
        let publications = Publications::new(repository.clone(), storage.clone());
        let peer_sync = manga_node::peers::PeerSync::new(
            repository.clone(),
            storage.clone(),
            publications.clone(),
            Zeroizing::new(secret.to_string()),
        )?;
        Ok(Arc::new(Self {
            repository,
            storage,
            library_objects,
            publications,
            groups,
            peer_sync,
            secret,
            publisher: expected_publisher.to_owned(),
            native_inputs: Arc::new(tokio::sync::Semaphore::new(1)),
            native_node: None,
            storage_holder: String::new(),
        }))
    }
    /// Startup returns only after the original listener reports its real bind.
    pub async fn start_native_listener(self: &Arc<Self>) -> Result<Option<NativeListener>> {
        let Some(value) = self.native_node.clone() else {
            return Ok(None);
        };
        let mut config: manga_node::node::NodeConfig = serde_json::from_value(value)
            .context("invalid embedded original node configuration")?;
        ensure!(
            config.bind.ip().is_loopback(),
            "embedded original node must bind loopback"
        );
        config.database_connections = self.repository.pool.status().max_size;
        // Its HTTP/federation proof identity must be the configured holder, and never
        // an opportunistically generated key or native-user login identity.
        let holder = self.storage_holder.clone();
        let path = config.service_key_file.clone();
        tokio::task::spawn_blocking(move || load_signer(&path, &holder)).await??;
        let serving = manga_node::serving_role::ServingRole::prepare(&config).await?;
        let (ready_sender, ready_receiver) = tokio::sync::oneshot::channel();
        let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
        let repo = self.repository.clone();
        let storage = self.storage.clone();
        let library_objects = self.library_objects.clone();
        let task = tokio::spawn(async move {
            manga_node::node::serve_prepared_with_shared_library(
                config,
                repo,
                serving,
                Some(storage),
                Some(library_objects),
                Some(ready_sender),
                Some(shutdown_receiver),
            )
            .await
        });
        match tokio::time::timeout(std::time::Duration::from_secs(30), ready_receiver).await {
            Ok(Ok(())) => Ok(Some(NativeListener {
                task,
                shutdown: Some(shutdown_sender),
            })),
            _ => {
                task.abort();
                let _ = task.await;
                Err(anyhow::anyhow!(
                    "embedded original listener failed to become ready"
                ))
            }
        }
    }
    pub async fn download_original(
        &self,
        hash: Id,
    ) -> Result<Option<(manga_node::download_budget::Snapshot, String)>> {
        self.storage.download_public_optional(hash).await
    }
    pub async fn synchronize_approved_peer(
        &self,
        request: manga_node::peers::SyncRequest,
    ) -> Result<manga_node::peers::SyncResult> {
        self.peer_sync.synchronize(request).await
    }
    async fn descriptors(input: &PublicationInput) -> Result<Vec<manga_node::manifest::Page>> {
        ensure!(
            matches!(input.kind, 1001 | 1002) && input.canonical_manifest.len() <= 1_000_000,
            "unsupported protocol publication"
        );
        let bytes = input.canonical_manifest.clone();
        let kind = input.kind;
        let pages =
            tokio::task::spawn_blocking(move || -> Result<Vec<manga_node::manifest::Page>> {
                if kind == 1001 {
                    Ok(manga_node::manifest::chapter(&bytes)
                        .map_err(anyhow::Error::msg)?
                        .pages)
                } else {
                    Ok(manga_node::series_metadata::parse(&bytes)
                        .map_err(anyhow::Error::msg)?
                        .cover
                        .into_iter()
                        .collect())
                }
            })
            .await??;
        let unique = manga_node::manifest::unique_pages(&pages).map_err(anyhow::Error::msg)?;
        ensure!(
            input.originals.len() == unique.len(),
            "publication must contain exactly its canonical originals"
        );
        let total = input
            .originals
            .values()
            .try_fold(0usize, |sum, bytes| sum.checked_add(bytes.len()))
            .context("publication size overflow")?;
        ensure!(
            total <= 256 * 1024 * 1024,
            "publication staging byte budget"
        );
        for page in unique {
            let bytes = input
                .originals
                .get(&page.hash)
                .context("publication original missing")?;
            manga_node::manifest::validate_image(bytes, page).map_err(anyhow::Error::msg)?;
        }
        Ok(pages)
    }
    pub async fn publish_operator(&self, input: PublicationInput) -> Result<PublicationReceipt> {
        let _slot = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            self.publications.slots.clone().acquire_owned(),
        )
        .await??;
        let pages = Self::descriptors(&input).await?;
        let permit = self
            .repository
            .publisher_permit(identity(&self.publisher)?)
            .await?;
        let manifest = auth::hash(&input.canonical_manifest);
        let event = manga_node::platform_bridge::sign_publication_at(
            &self.secret,
            input.kind,
            &manifest,
            input.created_at.unwrap_or_else(auth::now),
        )?;
        if self
            .repository
            .visible_event(identity(&event.id)?)
            .await?
            .is_some()
        {
            let newly_hosted = self
                .publications
                .publish_with_permit(&event, permit)
                .await?;
            return self.receipt(event, newly_hosted).await;
        }
        for page in manga_node::manifest::unique_pages(&pages).map_err(anyhow::Error::msg)? {
            self.storage
                .put_for_publisher(&permit, identity(&page.hash)?, &input.originals[&page.hash])
                .await?;
        }
        self.storage
            .put_for_publisher(&permit, identity(&manifest)?, &input.canonical_manifest)
            .await?;
        let newly_hosted = self
            .publications
            .publish_with_permit(&event, permit)
            .await?;
        self.receipt(event, newly_hosted).await
    }
    pub async fn publish_group(
        &self,
        group: Id,
        member: Id,
        input: PublicationInput,
    ) -> Result<PublicationReceipt> {
        let groups = self
            .groups
            .as_ref()
            .context("protocol group publication runtime is not configured")?;
        let _slot = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            self.publications.slots.clone().acquire_owned(),
        )
        .await??;
        let version = groups.authorize(group, member, "publish").await?;
        let pages = Self::descriptors(&input).await?;
        // Replay returns the original stored signed body after current group
        // authorization, without requiring expired staging grants again.
        if let Some(created_at) = input.created_at {
            let manifest = auth::hash(&input.canonical_manifest);
            let candidate = Event {
                id: String::new(),
                pubkey: hex::encode(group),
                created_at,
                kind: input.kind,
                tags: vec![],
                content: serde_json::json!({"manifest":manifest,"version":1}).to_string(),
                sig: String::new(),
            };
            if let Some(event) =
                self.repository
                    .visible_event(
                        candidate.digest().try_into().map_err(|_| {
                            anyhow::anyhow!("invalid publication event digest length")
                        })?,
                    )
                    .await?
            {
                event.verify().map_err(anyhow::Error::msg)?;
                return self.receipt((*event).clone(), false).await;
            }
        }
        for page in manga_node::manifest::unique_pages(&pages).map_err(anyhow::Error::msg)? {
            groups
                .upload(
                    group,
                    member,
                    version,
                    identity(&page.hash)?,
                    &input.originals[&page.hash],
                )
                .await?;
        }
        let manifest = auth::hash(&input.canonical_manifest);
        groups
            .upload(
                group,
                member,
                version,
                identity(&manifest)?,
                &input.canonical_manifest,
            )
            .await?;
        let event = groups
            .publish(
                group,
                member,
                version,
                GroupPublication {
                    kind: input.kind,
                    manifest,
                    version: 1,
                    created_at: input.created_at,
                },
            )
            .await?;
        self.receipt(event, true).await
    }
    async fn receipt(&self, event: Event, newly_hosted: bool) -> Result<PublicationReceipt> {
        let locally_visible = self
            .repository
            .catalogue_entry(identity(&event.id)?)
            .await?
            .is_some();
        Ok(PublicationReceipt {
            event,
            newly_hosted,
            locally_visible,
            state: if locally_visible {
                "locally_hosted"
            } else {
                "accepted_pending_projection"
            },
        })
    }
    /// A deletion request is recorded for original moderator review. No approval,
    /// raw kind-5 relay insertion, or physical deletion is performed here.
    pub async fn request_operator_deletion(
        &self,
        targets: Vec<String>,
        reason: String,
    ) -> Result<Event> {
        ensure!(
            !targets.is_empty() && targets.len() <= 100 && reason.len() <= 65536,
            "deletion request bounds"
        );
        for target in &targets {
            identity(target)?;
        }
        self.repository
            .publisher_permit(identity(&self.publisher)?)
            .await?;
        let tags = targets
            .into_iter()
            .map(|target| vec!["e".into(), target])
            .collect();
        let event = auth::sign(&self.secret, 5, tags, reason).map_err(anyhow::Error::msg)?;
        self.repository.record_deletion_request(&event).await?;
        Ok(event)
    }
}
impl ProtocolFacade {
    pub async fn configure_publication(&self, config: OperatorRuntimeConfig) -> Result<()> {
        self.require_schema()
            .map_err(|_| anyhow::anyhow!("original protocol schema is unavailable"))?;
        let runtime = PublicationRuntime::open(self.repository.clone(), config).await?;
        *self.runtime.write().await = Some(runtime);
        Ok(())
    }
    pub async fn attach_publication_runtime(&self, runtime: Arc<PublicationRuntime>) {
        *self.runtime.write().await = Some(runtime);
    }
    pub async fn publication_runtime(&self) -> ApiResult<Arc<PublicationRuntime>> {
        self.require_schema()?;
        self.runtime.read().await.clone().ok_or_else(|| {
            ApiError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "protocol_publication_not_configured",
                "Protocol publication runtime is not configured",
            )
        })
    }
    pub async fn publish_for_native_user(
        &self,
        user_id: &str,
        group_id: Option<&str>,
        input: PublicationInput,
    ) -> ApiResult<PublicationReceipt> {
        let runtime = self.publication_runtime().await?;
        let member = self
            .repository
            .pool
            .get()
            .await
            .map_err(|_| ApiError::unavailable())?
            .query_opt(
                "SELECT publisher FROM platform.protocol_identity_bindings WHERE user_id=$1",
                &[&user_id],
            )
            .await?
            .map(|row| row.get::<_, String>(0))
            .ok_or_else(|| {
                ApiError::forbidden(
                    "No operator-provisioned protocol signer identity is bound to this account",
                )
            })?;
        if let Some(group_id) = group_id {
            let group = self
                .repository
                .pool
                .get()
                .await
                .map_err(|_| ApiError::unavailable())?
                .query_opt(
                    "SELECT publisher FROM platform.protocol_identity_bindings WHERE group_id=$1",
                    &[&group_id],
                )
                .await?
                .map(|row| row.get::<_, String>(0))
                .ok_or_else(|| {
                    ApiError::forbidden(
                        "No original protocol group identity is bound to this group",
                    )
                })?;
            runtime.publish_group(identity(&group).map_err(|_|ApiError::unavailable())?,identity(&member).map_err(|_|ApiError::unavailable())?,input).await.map_err(|_|ApiError::forbidden("Original protocol group publication authority or staging validation failed"))
        } else {
            if member != runtime.publisher {
                return Err(ApiError::forbidden(
                    "This account is not bound to the configured operator signer",
                ));
            }
            runtime
                .publish_operator(input)
                .await
                .map_err(|_| ApiError::unavailable())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signer_files_are_never_created_and_public_identity_is_explicit() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.key");
        assert!(load_signer(&missing, &"00".repeat(32)).is_err());
        assert!(!missing.exists());
    }
    #[cfg(unix)]
    #[test]
    fn unsafe_permissions_and_wrong_identity_are_rejected() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("public-test-only.key");
        let secret = "11".repeat(32);
        std::fs::write(&path, &secret).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let public = auth::sign(&secret, 1, vec![], String::new())
            .unwrap()
            .pubkey;
        assert!(load_signer(&path, &public).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(load_signer(&path, &"00".repeat(32)).is_err());
        assert!(load_signer(&path, &public).is_ok());
    }
}

#[derive(Clone, Debug)]
pub struct NativeOriginal {
    pub hash: String,
    pub mime: String,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
    pub storage_key: String,
}
async fn read_native_original(
    state: &crate::state::AppState,
    file: &NativeOriginal,
) -> ApiResult<Vec<u8>> {
    let path = crate::domains::uploads::storage::object_path(
        &state.config.uploads_dir,
        &file.storage_key,
    )?;
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|_| ApiError::unavailable())?;
    if !metadata.is_file() || metadata.len() != file.bytes || file.bytes > 64 * 1024 * 1024 {
        return Err(ApiError::bad_request(
            "Original exceeds protocol publication limits",
        ));
    }
    tokio::fs::read(path)
        .await
        .map_err(|_| ApiError::unavailable())
}
async fn prepared_input(
    state: &crate::state::AppState,
    actor: &impl crate::domains::catalog::entity_mutations::CatalogMutationActor,
    group: Option<&str>,
    command: &str,
    kind: &str,
    session: Option<&str>,
    commit_hash: Option<&str>,
    native_payload: Option<&serde_json::Value>,
) -> ApiResult<Option<PublicationInput>> {
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let Some(row)=tx.query_opt("SELECT user_id,group_id,resource_kind,upload_session_id,native_commit_hash,native_commit_payload,source_manifest FROM platform.protocol_publication_commands WHERE command_id=$1",&[&command]).await? else{return Ok(None)};
    if row.get::<_, String>(0) != actor.user_id()
        || row.get::<_, Option<String>>(1).as_deref() != group
        || row.get::<_, String>(2) != kind
        || row.get::<_, Option<String>>(3).as_deref() != session
        || row.get::<_, Option<String>>(4).as_deref() != commit_hash
        || row.get::<_, Option<serde_json::Value>>(5).as_ref() != native_payload
    {
        return Err(ApiError::conflict(
            "Prepared publication input or authority changed",
        ));
    }
    let canonical_manifest:Vec<u8>=row.get::<_,Option<Vec<u8>>>(6).ok_or_else(||ApiError::conflict("This older publication command lacks an immutable source input snapshot; explicit reconciliation is required"))?;
    if canonical_manifest.len() > 1_000_000 {
        return Err(ApiError::unavailable());
    }
    let files=tx.query("SELECT p.hash,o.digest,o.mime,o.bytes,o.width,o.height,o.storage_key FROM platform.protocol_publication_originals p JOIN platform.media_objects o ON o.id=p.object_id WHERE p.command_id=$1 ORDER BY p.hash",&[&command]).await?;
    if files.len() > 500 {
        return Err(ApiError::unavailable());
    }
    let mut descriptors = Vec::with_capacity(files.len());
    let mut total = 0u64;
    for file in files {
        let hash: String = file.get(0);
        let digest: String = file.get(1);
        let bytes: i64 = file.get(3);
        let width: i32 = file.get(4);
        let height: i32 = file.get(5);
        if hash != digest || bytes <= 0 || bytes > 64 * 1024 * 1024 || width <= 0 || height <= 0 {
            return Err(ApiError::unavailable());
        }
        total = total
            .checked_add(bytes as u64)
            .filter(|n| *n <= 256 * 1024 * 1024)
            .ok_or_else(ApiError::unavailable)?;
        descriptors.push(NativeOriginal {
            hash,
            mime: file.get(2),
            bytes: bytes as u64,
            width: width as u32,
            height: height as u32,
            storage_key: file.get(6),
        });
    }
    tx.commit().await?;
    drop(client);
    let mut originals = BTreeMap::new();
    for file in &descriptors {
        originals.insert(file.hash.clone(), read_native_original(state, file).await?);
    }
    Ok(Some(PublicationInput {
        kind: if kind == "manga" { 1002 } else { 1001 },
        canonical_manifest,
        originals,
        created_at: None,
    }))
}

async fn command_publish(
    state: &crate::state::AppState,
    actor: &impl crate::domains::catalog::entity_mutations::CatalogMutationActor,
    group: Option<&str>,
    command: &str,
    kind: &str,
    catalog_id: &mut String,
    session: Option<&str>,
    commit_hash: Option<&str>,
    native_payload: Option<&serde_json::Value>,
    mut input: PublicationInput,
) -> ApiResult<PublicationReceipt> {
    let runtime = state.protocol.publication_runtime().await?;
    let publisher = if let Some(group) = group {
        state
            .db
            .client()
            .await?
            .query_opt(
                "SELECT publisher FROM platform.protocol_identity_bindings WHERE group_id=$1",
                &[&group],
            )
            .await?
            .map(|r| r.get::<_, String>(0))
            .ok_or_else(|| ApiError::forbidden("Protocol group identity is not configured"))?
    } else {
        runtime.publisher.clone()
    };
    // Reject absent/revoked signer authority before allocating a durable command.
    // Otherwise an unsigned native account could reserve another title's stable
    // source command and block the genuinely configured publisher forever.
    let member = state
        .db
        .client()
        .await?
        .query_opt(
            "SELECT publisher FROM platform.protocol_identity_bindings WHERE user_id=$1",
            &[&actor.user_id()],
        )
        .await?
        .map(|row| row.get::<_, String>(0))
        .ok_or_else(|| {
            ApiError::forbidden(
                "No operator-provisioned protocol signer identity is bound to this account",
            )
        })?;
    if group.is_some() {
        let groups = runtime.groups.as_ref().ok_or_else(|| {
            ApiError::forbidden("Original managed group publication runtime is not configured")
        })?;
        groups
            .authorize(
                identity(&publisher).map_err(|_| ApiError::unavailable())?,
                identity(&member).map_err(|_| ApiError::unavailable())?,
                "publish",
            )
            .await
            .map_err(|_| {
                ApiError::forbidden("Current original group publication authority is required")
            })?;
    } else {
        if member != runtime.publisher {
            return Err(ApiError::forbidden(
                "This account is not bound to the configured operator signer",
            ));
        }
        runtime
            .repository
            .publisher_permit(identity(&member).map_err(|_| ApiError::unavailable())?)
            .await
            .map_err(|_| ApiError::forbidden("Current original publisher admission is required"))?;
    }
    let manifest = auth::hash(&input.canonical_manifest);
    let input_hash=auth::hash(serde_json::json!({"kind":input.kind,"manifest":manifest,"user":actor.user_id(),"group":group}).to_string().as_bytes());
    let timestamp = auth::now();
    let mut unsigned = Event {
        id: String::new(),
        pubkey: publisher,
        created_at: timestamp,
        kind: input.kind,
        tags: vec![],
        content: serde_json::json!({"manifest":manifest,"version":1}).to_string(),
        sig: String::new(),
    };
    // Independent durable intent survives rollback of the native upload transaction.
    let alias;
    {
        let mut client = state.db.client().await?;
        let tx = client.transaction().await?;
        tx.execute("INSERT INTO platform.protocol_publication_commands(command_id,user_id,group_id,input_hash,created_at,resource_kind,catalog_id,upload_session_id,native_commit_hash,native_commit_payload,source_manifest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT(command_id) DO NOTHING",&[&command,&actor.user_id(),&group,&input_hash,&(timestamp as i64),&kind,&*catalog_id,&session,&commit_hash,&native_payload,&input.canonical_manifest]).await?;
        let row=tx.query_one("SELECT user_id,group_id,input_hash,created_at,catalog_id,event_id,native_commit_hash,upload_session_id,native_commit_payload,source_manifest FROM platform.protocol_publication_commands WHERE command_id=$1 FOR UPDATE",&[&command]).await?;
        if row.get::<_, String>(0) != actor.user_id()
            || row.get::<_, Option<String>>(1).as_deref() != group
            || row.get::<_, String>(2) != input_hash
            || row.get::<_, Option<String>>(6).as_deref() != commit_hash
            || row.get::<_, Option<String>>(7).as_deref() != session
            || row.get::<_, Option<serde_json::Value>>(8).as_ref() != native_payload
            || row.get::<_, Option<Vec<u8>>>(9).as_deref()
                != Some(input.canonical_manifest.as_slice())
        {
            return Err(ApiError::conflict(
                "Protocol publication command was prepared with different content or authority",
            ));
        }
        for (hash, bytes) in &input.originals {
            let object=tx.query_opt("SELECT o.id FROM platform.media_objects o WHERE o.digest=$1 AND o.bytes=$2 AND (($3='manga' AND EXISTS(SELECT 1 FROM platform.cover_media cm JOIN platform.catalog_covers c ON c.id=cm.cover_id WHERE cm.object_id=o.id AND cm.variant='original' AND c.manga_id=$4 AND c.deleted_at IS NULL)) OR ($3='chapter' AND EXISTS(SELECT 1 FROM platform.upload_files f WHERE f.object_id=o.id AND f.session_id=$5))) ORDER BY o.id LIMIT 1 FOR KEY SHARE OF o",&[&hash,&(bytes.len() as i64),&kind,&*catalog_id,&session]).await?;
            if let Some(object) = object {
                tx.execute("INSERT INTO platform.protocol_publication_originals(command_id,hash,object_id) VALUES($1,$2,$3) ON CONFLICT(command_id,hash) DO NOTHING",&[&command,&hash,&object.get::<_,String>(0)]).await?;
            } else {
                let pinned:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.protocol_publication_originals WHERE command_id=$1 AND hash=$2)",&[&command,&hash]).await?.get(0);
                if !pinned {
                    return Err(ApiError::conflict(
                        "Immutable original input is no longer owned by this native resource",
                    ));
                }
            }
        }
        unsigned.created_at = row.get::<_, i64>(3) as u64;
        *catalog_id = row.get(4);
        let event_id = hex::encode(unsigned.digest());
        if row
            .get::<_, Option<String>>(5)
            .is_some_and(|id| id != event_id)
        {
            return Err(ApiError::conflict(
                "Protocol signer identity changed for a prepared command",
            ));
        }
        // Original facade preserves existing preferred legacy aliases.
        alias = state.protocol.compatibility_id(kind, &event_id).await?;
        tx.execute(
            "UPDATE platform.protocol_publication_commands SET event_id=$2 WHERE command_id=$1",
            &[&command, &event_id],
        )
        .await?;
        tx.execute("INSERT INTO platform.protocol_resource_ids(kind,source_target,compatibility_id,catalog_id) VALUES($1,$2,$3,$4) ON CONFLICT(kind,source_target) DO NOTHING",&[&kind,&event_id,&alias,&*catalog_id]).await?;
        tx.commit().await?;
    }
    input.created_at = Some(unsigned.created_at);
    let expected_id = hex::encode(unsigned.digest());
    let receipt = state
        .protocol
        .publish_for_native_user(&actor.user_id(), group, input)
        .await?;
    if receipt.event.id != expected_id {
        return Err(ApiError::unavailable());
    }
    state.db.client().await?.execute("UPDATE platform.protocol_publication_commands SET state='source_accepted',updated_at=clock_timestamp() WHERE command_id=$1 AND state='prepared'",&[&command]).await?;
    Ok(receipt)
}
async fn native_series_input(
    state: &crate::state::AppState,
    tx: &tokio_postgres::Transaction<'_>,
    manga: &str,
    require_original_cover: bool,
) -> ApiResult<PublicationInput> {
    let row=tx.query_one("SELECT title,metadata,language,publication_status,content_rating,demographic,year FROM platform.catalog_manga WHERE id=$1",&[&manga]).await?;
    let metadata: serde_json::Value = row.get(1);
    let lang: String = row.get(2);
    let title: String = row.get(0);
    let mut titles = BTreeMap::new();
    titles.insert(lang.clone(), title);
    if let Some(localized) = metadata["localized_titles"].as_object() {
        titles = localized.iter().filter_map(|(lang,title)|title.as_str().map(|title|(lang.clone(),title.to_owned()))).collect();
    }
    let mut alternatives = Vec::new();
    if let Some(localized) = metadata["alt_titles"].as_object() {
        for (lang, names) in localized {
            if let Some(names) = names.as_array() {
                for name in names.iter().filter_map(serde_json::Value::as_str) {
                    alternatives.push(BTreeMap::from([(lang.clone(), name.to_owned())]));
                }
            }
        }
    }
    let mut credits = Vec::new();
    for credit in tx.query("SELECT mc.role,a.name FROM platform.catalog_manga_creators mc JOIN platform.catalog_authors a ON a.id=mc.author_id WHERE mc.manga_id=$1 AND a.deleted_at IS NULL ORDER BY mc.role,a.id",&[&manga]).await?{credits.push(manga_node::series_metadata::Credit{role:credit.get(0),name:credit.get(1)});}
    let categories=tx.query("SELECT t.name FROM platform.catalog_manga_tags mt JOIN platform.catalog_tags t ON t.id=mt.tag_id WHERE mt.manga_id=$1 ORDER BY t.id",&[&manga]).await?.into_iter().map(|r|r.get::<_,String>(0)).collect();
    let mut originals = BTreeMap::new();
    let cover=if let Some(cover)=tx.query_opt("SELECT o.digest,o.mime,o.bytes,o.width,o.height,o.storage_key FROM platform.catalog_covers c JOIN platform.cover_media cm ON cm.cover_id=c.id AND cm.variant='original' JOIN platform.media_objects o ON o.id=cm.object_id WHERE c.manga_id=$1 AND c.deleted_at IS NULL AND (NOT $2::bool OR (c.language=$3 AND platform.protocol_source_is_visible(c.protocol_event_id) AND o.bytes>0)) ORDER BY c.is_main DESC,c.created_at DESC,c.id LIMIT 1",&[&manga,&require_original_cover,&lang]).await? {
  let file=NativeOriginal{hash:cover.get(0),mime:cover.get(1),bytes:cover.get::<_,i64>(2) as u64,width:cover.get::<_,i32>(3) as u32,height:cover.get::<_,i32>(4) as u32,storage_key:cover.get(5)};
  originals.insert(file.hash.clone(),read_native_original(state,&file).await?);Some(manga_node::manifest::Page{hash:file.hash,size:file.bytes,mime:file.mime,width:file.width,height:file.height})
 }else{
     if require_original_cover {return Err(ApiError::bad_request("Original publication requires an original-language cover"));}
     None
 };
    if let Some(alts) = metadata["localized_alt_titles"].as_array() {
        alternatives = alts.iter().filter_map(|alt|alt.as_object().map(|m|m.iter().filter_map(|(lang,title)|title.as_str().map(|title|(lang.clone(),title.to_owned()))).collect())).collect();
    }
    let description = metadata["localized_description"].as_object()
        .map(|m|m.iter().filter_map(|(lang,text)|text.as_str().map(|text|(lang.clone(),text.to_owned()))).collect())
        .or_else(|| metadata["description"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| BTreeMap::from([(lang.clone(), s.to_owned())])));
    let source = manga_node::series_metadata::SeriesMetadata {
        titles,
        alt_titles: Some(alternatives),
        description,
        categories: Some(categories),
        credits: Some(credits),
        original_language: Some(lang),
        status: Some(row.get(3)),
        content_rating: Some(row.get(4)),
        publication_demographic: row.get(5),
        year: Some(row.get::<_, i32>(6) as u16),
        cover,
    };
    let canonical_manifest = manga_node::platform_bridge::encode_series(&source).map_err(|_| {
        ApiError::bad_request("Manga metadata cannot be represented by the original protocol")
    })?;
    Ok(PublicationInput {
        kind: 1002,
        canonical_manifest,
        originals,
        created_at: None,
    })
}
/// Called only after the normal submitted-version/staff draft review checks.
/// A native review is not permission to sign: the actor still needs the exact
/// explicitly configured original operator identity binding and admission.
pub async fn publish_native_manga(
    state: &crate::state::AppState,
    tx: &tokio_postgres::Transaction<'_>,
    actor: &impl crate::domains::catalog::entity_mutations::CatalogMutationActor,
    manga: &str,
    submitted_version: i32,
) -> ApiResult<Option<PublicationReceipt>> {
    publish_native_manga_with_cover_requirement(state, tx, actor, manga, submitted_version, false).await
}
/// Public title review requires a qualifying original only for fresh input.
/// Exact durable retries keep the original input captured by prepared_input.
pub(crate) async fn publish_native_manga_with_cover_requirement(
    state: &crate::state::AppState,
    tx: &tokio_postgres::Transaction<'_>,
    actor: &impl crate::domains::catalog::entity_mutations::CatalogMutationActor,
    manga: &str,
    submitted_version: i32,
    require_original_cover: bool,
) -> ApiResult<Option<PublicationReceipt>> {
    let command = format!("manga:{manga}");
    if state.config.protocol_publication.is_none() {
        let prepared:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.protocol_publication_commands WHERE command_id=$1)",&[&command]).await?.get(0);
        if prepared {
            return Err(ApiError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "protocol_publication_not_configured",
                "This title already has a durable protocol publication command; restore its configured authority before retrying",
            ));
        }
        return Ok(None);
    }
    if state.config.database_connections < 4 {
        return Err(ApiError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "protocol_pool_too_small",
            "Configured protocol publication requires at least four shared database connections",
        ));
    }
    let runtime = state.protocol.publication_runtime().await?;
    let native_payload = serde_json::json!({"action":"publish_manga","manga_id":manga,"submitted_version":submitted_version});
    if let Some(command_row)=tx.query_opt("SELECT p.event_id FROM platform.protocol_publication_commands p JOIN platform.catalog_manga m ON m.id=p.catalog_id AND m.protocol_series_id=p.event_id WHERE p.command_id=$1 AND p.user_id=$2 AND p.resource_kind='manga' AND p.state='materialized' AND p.native_commit_payload=$3 AND m.id=$4 AND m.deleted_at IS NULL AND m.state='published'",&[&command,&actor.user_id(),&native_payload,&manga]).await? {
        let source:String=command_row.get(0);let source_id=identity(&source).map_err(|_|ApiError::unavailable())?;
        let (event,locally_visible)=if let Some(entry)=state.protocol.entry(source_id).await? {(entry.event,true)} else {
            let event=manga_node::platform_bridge::pending_series_receipt(tx,source_id).await.map_err(|_|ApiError::unavailable())?.ok_or_else(||ApiError::not_found("Original title is no longer available"))?;
            (event,false)
        };
        event.verify().map_err(|_|ApiError::unavailable())?;
        if event.kind!=1002 || event.id!=source || !current_operator_receipt_authority(state,tx,actor,&event.pubkey).await? {return Err(ApiError::forbidden("Current original title publication authority is required"));}
        return Ok(Some(PublicationReceipt{event,newly_hosted:false,locally_visible,state:if locally_visible{"locally_hosted"}else{"accepted_pending_projection"}}));
    }
    let _input = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        runtime.native_inputs.clone().acquire_owned(),
    )
    .await
    .map_err(|_| ApiError::busy())?
    .map_err(|_| ApiError::unavailable())?;

    let input = match prepared_input(
        state,
        actor,
        None,
        &command,
        "manga",
        None,
        None,
        Some(&native_payload),
    )
    .await?
    {
        Some(input) => input,
        None => native_series_input(state, tx, manga, require_original_cover).await?,
    };
    let mut alias = manga.to_owned();
    let receipt = command_publish(
        state,
        actor,
        None,
        &command,
        "manga",
        &mut alias,
        None,
        None,
        Some(&native_payload),
        input,
    )
    .await?;
    if alias != manga {
        return Err(ApiError::conflict(
            "Original title already has a different native alias",
        ));
    }
    crate::domains::catalog::CatalogRepository::associate_protocol_manga(tx, manga, &receipt.event)
        .await?;
    let metadata = serde_json::json!({"publication_scope":"original-protocol","publication_status":receipt.state,"protocol_event_id":receipt.event.id,"protocol_publisher":receipt.event.pubkey});
    tx.execute(
        "UPDATE platform.catalog_manga SET metadata=metadata||$2 WHERE id=$1",
        &[&manga, &metadata],
    )
    .await?;
    tx.execute("UPDATE platform.protocol_publication_commands SET state='materialized',updated_at=clock_timestamp() WHERE command_id=$1 AND state='source_accepted'",&[&command]).await?;
    tx.execute(
        "DELETE FROM platform.protocol_publication_originals WHERE command_id=$1",
        &[&command],
    )
    .await?;
    Ok(Some(receipt))
}

/// Called only after normal upload CSRF/session/page-order/target checks.
pub async fn publish_native_upload(
    state: &crate::state::AppState,
    tx: &tokio_postgres::Transaction<'_>,
    actor: &impl crate::domains::catalog::entity_mutations::CatalogMutationActor,
    session: &str,
    commit_hash: &str,
    record: &mut crate::domains::catalog::ChapterPublication,
    files: &[NativeOriginal],
    native_commit_payload: &serde_json::Value,
) -> ApiResult<Option<PublicationReceipt>> {
    if state.config.protocol_publication.is_none() {
        let prepared:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.protocol_publication_commands WHERE upload_session_id=$1)", &[&session]).await?.get(0);
        if prepared {
            return Err(ApiError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "protocol_publication_not_configured",
                "This upload already has a durable protocol publication command; restore its configured authority before retrying",
            ));
        }
        return Ok(None);
    }
    if state.config.database_connections < 4 {
        return Err(ApiError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "protocol_pool_too_small",
            "Configured protocol publication requires at least four shared database connections",
        ));
    }
    let runtime = state.protocol.publication_runtime().await?;
    let _input = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        runtime.native_inputs.clone().acquire_owned(),
    )
    .await
    .map_err(|_| ApiError::busy())?
    .map_err(|_| ApiError::unavailable())?;
    // Reject unsupported source capabilities before buffers or any signed series.
    let mut unique = BTreeMap::new();
    for file in files {
        if !auth::valid_hash(&file.hash)
            || !["image/jpeg", "image/png", "image/webp"].contains(&file.mime.as_str())
            || file.bytes == 0
            || file.bytes > runtime.storage.max_blob_bytes
            || file.width == 0
            || file.height == 0
            || u64::from(file.width) * u64::from(file.height) > 40_000_000
        {
            return Err(ApiError::bad_request(
                "This original cannot be published by the configured protocol; native-only GIF support is unchanged",
            ));
        }
        if let Some(previous) = unique.insert(file.hash.clone(), file) {
            let previous: &NativeOriginal = previous;
            if previous.bytes != file.bytes
                || previous.mime != file.mime
                || previous.width != file.width
                || previous.height != file.height
            {
                return Err(ApiError::bad_request(
                    "Conflicting descriptors for a repeated original",
                ));
            }
        }
    }
    let total = unique
        .values()
        .try_fold(0u64, |sum, file| sum.checked_add(file.bytes))
        .ok_or_else(|| ApiError::bad_request("Original byte total overflow"))?;
    if total > 256 * 1024 * 1024 {
        return Err(ApiError::bad_request(
            "Configured protocol publication supports at most 256 MiB of unique original bytes",
        ));
    }

    let groups = tx
        .query(
            "SELECT group_id FROM platform.protocol_identity_bindings WHERE group_id=ANY($1)",
            &[&record.groups],
        )
        .await?;
    if groups.len() > 1 {
        return Err(ApiError::bad_request(
            "Select at most one configured protocol signing group",
        ));
    }
    let group = groups.first().map(|r| r.get::<_, String>(0));
    let source: Option<String> = tx
        .query_one(
            "SELECT protocol_series_id FROM platform.catalog_manga WHERE id=$1 FOR UPDATE",
            &[&record.manga_id],
        )
        .await?
        .get(0);
    let series = if let Some(source) = source {
        source
    } else {
        let series_command = format!("manga:{}", record.manga_id);
        let input = match prepared_input(
            state,
            actor,
            group.as_deref(),
            &series_command,
            "manga",
            None,
            None,
            None,
        )
        .await?
        {
            Some(input) => input,
            None => native_series_input(state, tx, &record.manga_id, false).await?,
        };
        let mut alias = record.manga_id.clone();
        let receipt = command_publish(
            state,
            actor,
            group.as_deref(),
            &format!("manga:{}", record.manga_id),
            "manga",
            &mut alias,
            None,
            None,
            None,
            input,
        )
        .await?;
        crate::domains::catalog::CatalogRepository::associate_protocol_manga(
            tx,
            &record.manga_id,
            &receipt.event,
        )
        .await?;
        receipt.event.id
    };
    let mut pages = Vec::new();
    let mut originals = BTreeMap::new();
    for file in files {
        if !originals.contains_key(&file.hash) {
            originals.insert(file.hash.clone(), read_native_original(state, file).await?);
        }
        pages.push(manga_node::manifest::Page {
            hash: file.hash.clone(),
            size: file.bytes,
            mime: file.mime.clone(),
            width: file.width,
            height: file.height,
        });
    }
    let manifest = manga_node::manifest::Manifest {
        series,
        language: record.language.clone(),
        chapter: record.chapter.clone(),
        volume: record.volume.clone(),
        title: record.title.clone(),
        pages,
    };
    let canonical_manifest =
        manga_node::platform_bridge::encode_chapter(&manifest).map_err(|_| {
            ApiError::bad_request("Chapter cannot be represented by the original protocol")
        })?;
    let input = PublicationInput {
        kind: 1001,
        canonical_manifest,
        originals,
        created_at: None,
    };
    let receipt = command_publish(
        state,
        actor,
        group.as_deref(),
        &format!("upload:{session}"),
        "chapter",
        &mut record.id,
        Some(session),
        Some(commit_hash),
        Some(native_commit_payload),
        input,
    )
    .await?;
    record.protocol_event_id = Some(receipt.event.id.clone());
    record.source_id = Some(
        state
            .protocol
            .compatibility_id("chapter", &receipt.event.id)
            .await?,
    );
    record.pages = files
        .iter()
        .map(|file| crate::domains::catalog::CatalogPage {
            name: file.hash.clone(),
            dimensions: [file.width as i32, file.height as i32],
        })
        .collect();
    Ok(Some(receipt))
}
pub async fn finalize_native_upload(
    tx: &tokio_postgres::Transaction<'_>,
    session: &str,
    manga: &str,
) -> ApiResult<()> {
    let commands = vec![format!("upload:{session}"), format!("manga:{manga}")];
    tx.execute("UPDATE platform.protocol_publication_commands SET state='materialized',updated_at=clock_timestamp() WHERE command_id=ANY($1) AND state='source_accepted'",&[&commands]).await?;
    tx.execute(
        "DELETE FROM platform.protocol_publication_originals WHERE command_id=ANY($1)",
        &[&commands],
    )
    .await?;
    Ok(())
}

impl PublicationRuntime {
    pub(crate) async fn authorize_bound_deletion(
        &self,
        group: Option<Id>,
        member: Id,
    ) -> Result<()> {
        if let Some(group) = group {
            self.groups
                .as_ref()
                .context("original managed group runtime is not configured")?
                .authorize(group, member, "remove")
                .await?;
        } else {
            ensure!(
                identity(&self.publisher)? == member,
                "configured deletion signer mismatch"
            );
            self.repository.publisher_permit(member).await?;
        }
        Ok(())
    }
    pub(crate) fn prepare_bound_operator_deletion(
        &self,
        target: &str,
        reason: &str,
    ) -> Result<Event> {
        identity(target)?;
        ensure!(reason.len() <= 4096, "native deletion intent reason budget");
        auth::sign(
            &self.secret,
            5,
            vec![vec!["e".into(), target.into()]],
            reason.into(),
        )
        .map_err(anyhow::Error::msg)
    }
    pub(crate) async fn submit_bound_operator_deletion(&self, event: &Event) -> Result<()> {
        event.verify().map_err(anyhow::Error::msg)?;
        ensure!(
            event.kind == 5 && event.pubkey == self.publisher,
            "deletion signer does not match configured operator"
        );
        self.repository
            .publisher_permit(identity(&self.publisher)?)
            .await?;
        self.repository.record_deletion_request(event).await?;
        Ok(())
    }
    pub(crate) async fn request_bound_group_deletion(
        &self,
        group: Id,
        member: Id,
        target: &str,
        intent_reason: &str,
    ) -> Result<Event> {
        identity(target)?;
        ensure!(
            intent_reason.len() <= 4096,
            "native deletion intent reason budget"
        );
        let groups = self
            .groups
            .as_ref()
            .context("original managed group runtime is not configured")?;
        let version = groups.authorize(group, member, "remove").await?;
        // Original group requests are already durably recorded in the original
        // authority transaction. Recover that actual body after a platform failure.
        let target_id = identity(target)?;
        let row=self.repository.pool.get().await?.query_opt("SELECT d.raw_event FROM deletion_requests d JOIN deletion_targets t ON t.request_id=d.id WHERE d.author=$1 AND t.event_id=$2 AND d.raw_event->>'content'=$3 AND NOT EXISTS(SELECT 1 FROM deletion_targets other WHERE other.request_id=d.id AND other.event_id<>$2) ORDER BY d.id LIMIT 1", &[&group.as_slice(),&target_id.as_slice(),&intent_reason]).await?;
        if let Some(row) = row {
            let event: Event = serde_json::from_value(row.get(0))?;
            event.verify().map_err(anyhow::Error::msg)?;
            ensure!(
                event.kind == 5
                    && identity(&event.pubkey)? == group
                    && event.values("e") == vec![target]
                    && event.content == intent_reason,
                "invalid exact native group deletion intent evidence"
            );
            return Ok(event);
        }
        groups
            .deletion_request(
                group,
                member,
                version,
                vec![target.into()],
                intent_reason.into(),
            )
            .await
    }
    pub(crate) fn publisher_identity(&self) -> &str {
        &self.publisher
    }
}

async fn current_operator_receipt_authority(
    state: &crate::state::AppState,
    tx: &tokio_postgres::Transaction<'_>,
    actor: &impl crate::domains::catalog::entity_mutations::CatalogMutationActor,
    publisher: &str,
) -> ApiResult<bool> {
    let Ok(runtime) = state.protocol.publication_runtime().await else {
        return Ok(false);
    };
    let Some(binding) = tx
        .query_opt(
            "SELECT publisher FROM platform.protocol_identity_bindings WHERE user_id=$1",
            &[&actor.user_id()],
        )
        .await?
    else {
        return Ok(false);
    };
    let member: String = binding.get(0);
    Ok(member == publisher
        && member == runtime.publisher
        && runtime
            .repository
            .publisher_permit(identity(&member).map_err(|_| ApiError::unavailable())?)
            .await
            .is_ok())
}
/// Native source publisher may reload only a small pending draft receipt. A
/// staff role alone cannot turn retired/hidden source data into a cached DTO.
/// This is a retry capability for the exact owned native intent, never evidence
/// that a prepared digest has been accepted or become publicly visible.
pub async fn native_manga_publication_retry_allowed(
    state: &crate::state::AppState,
    tx: &tokio_postgres::Transaction<'_>,
    manga: &str,
    actor: &impl crate::domains::catalog::entity_mutations::CatalogMutationActor,
) -> ApiResult<bool> {
    if !state.protocol.is_ready() || state.config.protocol_publication.is_none() {
        return Ok(false);
    }
    let Some(row) = tx.query_opt(
        "SELECT m.version,p.native_commit_payload FROM platform.catalog_manga m JOIN platform.protocol_publication_commands p ON p.catalog_id=m.id AND p.command_id='manga:'||m.id AND p.resource_kind='manga' WHERE m.id=$1 AND m.deleted_at IS NULL AND m.state='submitted' AND p.user_id=$2 AND p.group_id IS NULL AND p.upload_session_id IS NULL AND p.state IN('prepared','source_accepted') AND p.source_manifest IS NOT NULL",
        &[&manga, &actor.user_id()],
    ).await? else {
        return Ok(false);
    };
    let version: i32 = row.get(0);
    let expected =
        serde_json::json!({"action":"publish_manga","manga_id":manga,"submitted_version":version});
    if row.get::<_, Option<serde_json::Value>>(1).as_ref() != Some(&expected) {
        return Ok(false);
    }
    let Ok(runtime) = state.protocol.publication_runtime().await else {
        return Ok(false);
    };
    current_operator_receipt_authority(state, tx, actor, runtime.publisher_identity()).await
}

pub async fn pending_native_manga_receipt_allowed(
    state: &crate::state::AppState,
    tx: &tokio_postgres::Transaction<'_>,
    manga: &str,
    actor: &impl crate::domains::catalog::entity_mutations::CatalogMutationActor,
) -> ApiResult<bool> {
    if !state.protocol.is_ready() || state.config.protocol_publication.is_none() {
        return Ok(false);
    }
    let Some(row)=tx.query_opt("SELECT m.protocol_series_id FROM platform.catalog_manga m JOIN platform.protocol_publication_commands p ON p.catalog_id=m.id AND p.resource_kind='manga' AND p.event_id=m.protocol_series_id WHERE m.id=$1 AND m.deleted_at IS NULL AND m.state='published' AND p.user_id=$2 AND p.group_id IS NULL AND p.state IN('source_accepted','materialized')",&[&manga,&actor.user_id()]).await? else{return Ok(false)};
    let Some(source) = row.get::<_, Option<String>>(0) else {
        return Ok(false);
    };
    let Some(event) = manga_node::platform_bridge::pending_series_receipt(
        tx,
        identity(&source).map_err(|_| ApiError::unavailable())?,
    )
    .await
    .map_err(|_| ApiError::unavailable())?
    else {
        return Ok(false);
    };
    current_operator_receipt_authority(state, tx, actor, &event.pubkey).await
}

/// Authenticated upload retry may expose a minimal pending receipt only while
/// original acceptance is current. Historical DTO snapshots never grant reads.
pub async fn pending_native_receipt_allowed(
    state: &crate::state::AppState,
    tx: &tokio_postgres::Transaction<'_>,
    chapter: &str,
    session: &str,
    actor: &impl crate::domains::catalog::entity_mutations::CatalogMutationActor,
) -> ApiResult<bool> {
    pending_native_receipt_allowed_for_owner(state, tx, chapter, session, actor.user_id()).await
}
/// Read-only receipt policy. The caller authenticates and locks the account and
/// exact session owner in this transaction; no catalogue mutation actor is needed.
pub(crate) async fn pending_native_receipt_allowed_for_owner(
    state: &crate::state::AppState,
    tx: &tokio_postgres::Transaction<'_>,
    chapter: &str,
    session: &str,
    owner: &str,
) -> ApiResult<bool> {
    if !state.protocol.is_ready() || state.config.protocol_publication.is_none() {
        return Ok(false);
    }
    let Some(row)=tx.query_opt("SELECT c.protocol_event_id,m.protocol_series_id,p.group_id,b.publisher FROM platform.upload_sessions s JOIN platform.catalog_chapters c ON c.id=s.committed_chapter_id JOIN platform.catalog_manga m ON m.id=c.manga_id JOIN platform.protocol_publication_commands p ON p.upload_session_id=s.id AND p.catalog_id=c.id AND p.event_id=c.protocol_event_id AND p.user_id=s.owner_id JOIN platform.protocol_identity_bindings b ON b.user_id=s.owner_id WHERE s.id=$1 AND s.owner_id=$2 AND c.id=$3 AND s.state='committed' AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.state='published' AND p.state IN('source_accepted','materialized')", &[&session,&owner,&chapter]).await? else{return Ok(false)};
    let Some(source) = row.get::<_, Option<String>>(0) else {
        return Ok(false);
    };
    let Some(parent) = row.get::<_, Option<String>>(1) else {
        return Ok(false);
    };
    let Some(event) = manga_node::platform_bridge::pending_chapter_receipt(
        tx,
        identity(&source).map_err(|_| ApiError::unavailable())?,
        identity(&parent).map_err(|_| ApiError::unavailable())?,
    )
    .await
    .map_err(|_| ApiError::unavailable())?
    else {
        return Ok(false);
    };
    let Ok(runtime) = state.protocol.publication_runtime().await else {
        return Ok(false);
    };
    let member: String = row.get(3);
    if let Some(group) = row.get::<_, Option<String>>(2) {
        let Some(binding) = tx
            .query_opt(
                "SELECT publisher FROM platform.protocol_identity_bindings WHERE group_id=$1",
                &[&group],
            )
            .await?
        else {
            return Ok(false);
        };
        let publisher: String = binding.get(0);
        if event.pubkey != publisher {
            return Ok(false);
        };
        let Some(groups) = runtime.groups.as_ref() else {
            return Ok(false);
        };
        Ok(groups
            .authorize(
                identity(&publisher).map_err(|_| ApiError::unavailable())?,
                identity(&member).map_err(|_| ApiError::unavailable())?,
                "publish",
            )
            .await
            .is_ok())
    } else {
        Ok(member == event.pubkey
            && member == runtime.publisher
            && runtime
                .repository
                .publisher_permit(identity(&member).map_err(|_| ApiError::unavailable())?)
                .await
                .is_ok())
    }
}
