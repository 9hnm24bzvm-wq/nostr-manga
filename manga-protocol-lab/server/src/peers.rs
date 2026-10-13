//! Explicit peer identities and bounded original transfer. No URL is learned from
//! a manifest/event, redirects are disabled, and fallback keys need separate approval.
use crate::{
    auth::{self, Event},
    db::{Id, Repository},
    filter::{Filter, Value},
    manifest,
    publication::{self, Publications},
    transfer,
    upload::UploadStore,
};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use zeroize::Zeroizing;
/// Operational source failure, separate from verified permanent content rejection.
/// A caller may explicitly retry; this type never authorizes write replay.
#[derive(Debug, thiserror::Error)]
#[error("approved peer temporarily unavailable")]
pub(crate) struct PeerUnavailable;
impl PeerUnavailable {
    fn transient_status(status: reqwest::StatusCode) -> bool {
        matches!(
            status,
            reqwest::StatusCode::REQUEST_TIMEOUT | reqwest::StatusCode::TOO_MANY_REQUESTS
        ) || status.is_server_error()
    }
    fn check_status(status: reqwest::StatusCode) -> Result<()> {
        ensure!(!Self::transient_status(status), Self);
        Ok(())
    }
    fn transport(error: anyhow::Error) -> anyhow::Error {
        let retryable = error.downcast_ref::<reqwest::Error>().is_some_and(|e| {
            e.is_connect() || e.is_timeout() || e.status().is_some_and(Self::transient_status)
        }) || error.is::<tokio::time::error::Elapsed>()
            || matches!(
                error.downcast_ref::<transfer::ReadError>(),
                Some(transfer::ReadError::Transport(_))
            );
        if retryable { Self.into() } else { error }
    }
}
#[derive(Clone, Debug)]
pub struct Peer {
    pub key: Id,
    pub endpoint: String,
    pub version: i64,
    pub discovered_endpoint: bool,
}
impl Repository {
    pub async fn set_peer(&self, key: Id, endpoint: &str, allowed: bool) -> Result<i64> {
        let endpoint = crate::node::normalize_origin(endpoint)?;
        Ok(self.pool.get().await?.query_one("INSERT INTO approved_peers(service_key,endpoint,allowed) VALUES($1,$2,$3) ON CONFLICT(service_key) DO UPDATE SET endpoint=EXCLUDED.endpoint,allowed=EXCLUDED.allowed,discovered_endpoint=false RETURNING version", &[&key.as_slice(),&endpoint,&allowed]).await?.get(0))
    }
    pub async fn approved_peer(&self, key: Id) -> Result<Peer> {
        let row = self
            .pool
            .get()
            .await?
            .query_opt(
                "SELECT endpoint,version,discovered_endpoint FROM approved_peers WHERE service_key=$1 AND allowed",
                &[&key.as_slice()],
            )
            .await?
            .context("peer is not approved")?;
        Ok(Peer {
            key,
            endpoint: row.get(0),
            version: row.get(1),
            discovered_endpoint: row.get(2),
        })
    }
    pub async fn series_matches(&self, event: &Event, expression: &str) -> Result<bool> {
        let (sql, mut args) = Filter::new(expression)
            .and_then(Filter::compile)
            .map_err(anyhow::Error::msg)?;
        args.push(Value::Bytes(hex::decode(&event.pubkey)?));
        let publisher = args.len();
        args.push(Value::Integer(event.created_at as i64));
        let created = args.len();
        let params: Vec<_> = args.iter().map(Value::parameter).collect();
        Ok(self.pool.get().await?.query_one(&format!("SELECT ({sql}) IS TRUE FROM (SELECT ${publisher}::bytea AS publisher,${created}::bigint AS created_at,NULL::bytea AS series,NULL::text AS language,NULL::text AS chapter,NULL::text AS volume,NULL::text AS title,NULL::bigint AS pages,NULL::bigint AS size) metadata"),&params).await?.get(0))
    }
    pub async fn manifest_matches(
        &self,
        event: &Event,
        m: &manifest::Manifest,
        expression: &str,
    ) -> Result<bool> {
        let (sql, mut args) = Filter::new(expression)
            .and_then(Filter::compile)
            .map_err(anyhow::Error::msg)?;
        let total = m
            .pages
            .iter()
            .try_fold(0i64, |sum, p| sum.checked_add(i64::try_from(p.size).ok()?))
            .context("manifest size overflow")?;
        let mut columns = Vec::new();
        for (name, kind, value) in [
            ("series", "bytea", Value::Bytes(hex::decode(&m.series)?)),
            (
                "publisher",
                "bytea",
                Value::Bytes(hex::decode(&event.pubkey)?),
            ),
            ("language", "text", Value::Text(m.language.clone())),
            ("pages", "bigint", Value::Integer(m.pages.len() as i64)),
            ("size", "bigint", Value::Integer(total)),
            (
                "created_at",
                "bigint",
                Value::Integer(event.created_at as i64),
            ),
        ] {
            args.push(value);
            columns.push(format!("${}::{kind} AS {name}", args.len()));
        }
        if let Some(chapter) = &m.chapter {
            args.push(Value::Text(chapter.clone()));
            columns.push(format!("${}::text AS chapter", args.len()));
        } else {
            columns.push("NULL::text AS chapter".into());
        }
        for (name, value) in [("volume", &m.volume), ("title", &m.title)] {
            if let Some(v) = value {
                args.push(Value::Text(v.clone()));
                columns.push(format!("${}::text AS {name}", args.len()));
            } else {
                columns.push(format!("NULL::text AS {name}"));
            }
        }
        let params: Vec<_> = args.iter().map(Value::parameter).collect();
        let row = self
            .pool
            .get()
            .await?
            .query_one(
                &format!(
                    "SELECT ({sql}) IS TRUE FROM (SELECT {}) metadata",
                    columns.join(",")
                ),
                &params,
            )
            .await?;
        Ok(row.get(0))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncRequest {
    pub peer: String,
    pub event: String,
    #[serde(default)]
    pub filter: String,
    #[serde(default)]
    pub fallbacks: Vec<String>,
}
#[derive(Serialize)]
pub struct SyncResult {
    pub event: String,
    pub matched: bool,
    pub hosted: bool,
    pub projected: bool,
    pub originals_fetched: usize,
    pub used_peers: Vec<String>,
}
pub struct PeerSync {
    repo: Repository,
    storage: Arc<UploadStore>,
    publications: Arc<Publications>,
    secret: Zeroizing<String>,
    client: reqwest::Client,
    endpoint_policy: crate::endpoint_policy::EndpointPolicy,
    pub slots: Arc<tokio::sync::Semaphore>,
}
fn id(text: &str) -> Result<Id> {
    ensure!(auth::valid_hash(text), "invalid identifier");
    Ok(hex::decode(text)?.try_into().unwrap())
}
impl PeerSync {
    pub fn new(
        repo: Repository,
        storage: Arc<UploadStore>,
        publications: Arc<Publications>,
        secret: Zeroizing<String>,
    ) -> Result<Arc<Self>> {
        Self::with_endpoint_policy(
            repo,
            storage,
            publications,
            secret,
            crate::endpoint_policy::EndpointPolicy::default(),
        )
    }
    pub fn with_endpoint_policy(
        repo: Repository,
        storage: Arc<UploadStore>,
        publications: Arc<Publications>,
        secret: Zeroizing<String>,
        endpoint_policy: crate::endpoint_policy::EndpointPolicy,
    ) -> Result<Arc<Self>> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(15))
            .pool_max_idle_per_host(2)
            .build()?;
        Ok(Arc::new(Self {
            repo,
            storage,
            publications,
            secret,
            client,
            endpoint_policy,
            slots: Arc::new(tokio::sync::Semaphore::new(2)),
        }))
    }
    async fn transport(&self, peer: &Peer) -> Result<reqwest::Client> {
        self.current(peer).await?;
        if peer.discovered_endpoint {
            let endpoint = self
                .endpoint_policy
                .connect(&peer.endpoint, Some(peer.key))
                .await
                .map_err(PeerUnavailable::transport)?;
            self.current(peer).await?;
            Ok(endpoint.pinned_client())
        } else {
            Ok(self.client.clone())
        }
    }
    fn authorization_for(&self, url: &str, method: &str, body: &[u8]) -> Result<String> {
        let mut tags = vec![
            vec!["u".into(), url.into()],
            vec!["method".into(), method.into()],
            vec!["nonce".into(), hex::encode(rand::random::<[u8; 16]>())],
        ];
        if method == "POST" {
            tags.push(vec!["payload".into(), auth::hash(body)]);
        }
        let event =
            auth::sign(&self.secret, 27235, tags, String::new()).map_err(anyhow::Error::msg)?;
        Ok(format!(
            "Nostr {}",
            STANDARD.encode(serde_json::to_vec(&event)?)
        ))
    }
    fn authorization(&self, url: &str) -> Result<String> {
        self.authorization_for(url, "GET", &[])
    }
    async fn current(&self, peer: &Peer) -> Result<()> {
        let current = self.repo.approved_peer(peer.key).await?;
        ensure!(
            current.version == peer.version
                && current.endpoint == peer.endpoint
                && current.discovered_endpoint == peer.discovered_endpoint,
            "peer approval changed"
        );
        Ok(())
    }
    async fn verify(&self, peer: &Peer) -> Result<()> {
        self.current(peer).await?;
        let nonce = hex::encode(rand::random::<[u8; 32]>());
        let url = format!("{}/identity?nonce={nonce}", peer.endpoint);
        let response = self
            .transport(peer)
            .await?
            .get(&url)
            .send()
            .await
            .map_err(|error| PeerUnavailable::transport(error.into()))?;
        PeerUnavailable::check_status(response.status())?;
        ensure!(response.status() == 200, "identity endpoint unavailable");
        let bytes = transfer::read_bounded(response, 16384, None)
            .await
            .map_err(|error| PeerUnavailable::transport(error.into()))?;
        let proof: Event = serde_json::from_slice(&bytes)?;
        proof.verify().map_err(anyhow::Error::msg)?;
        ensure!(
            proof.kind == 20005
                && proof.pubkey == hex::encode(peer.key)
                && proof.created_at.abs_diff(auth::now()) <= 60
                && proof.one("nonce").map_err(anyhow::Error::msg)? == nonce
                && proof.one("endpoint").map_err(anyhow::Error::msg)? == peer.endpoint,
            "service identity mismatch"
        );
        Ok(())
    }
    async fn get(
        &self,
        peer: &Peer,
        path: &str,
        limit: usize,
        expected: Option<u64>,
    ) -> Result<Vec<u8>> {
        self.current(peer).await?;
        let url = format!("{}{path}", peer.endpoint);
        let authorization = self.authorization(&url)?;
        let request_id = auth::token(&authorization).map_err(anyhow::Error::msg)?.id;
        let response = self
            .transport(peer)
            .await?
            .get(&url)
            .header("authorization", authorization)
            .send()
            .await
            .map_err(|error| PeerUnavailable::transport(error.into()))?;
        let status = response.status();
        // A busy source need not supply a success proof or the requested length.
        PeerUnavailable::check_status(status)?;
        // A definitive non-transient rejection stays permanent even when its
        // error body is interrupted; only accepted content reaches verification.
        ensure!(status == 200, "peer response was not complete");
        let proof = response
            .headers()
            .get(crate::service_proof::HEADER)
            .filter(|v| v.as_bytes().len() <= 16384)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let bytes = transfer::read_bounded(response, limit, expected)
            .await
            .map_err(|error| PeerUnavailable::transport(error.into()))?;
        if path.contains("/blobs/") {
            let hash = path.rsplit('/').next().context("blob identity missing")?;
            ensure!(
                auth::valid_hash(hash) && auth::hash(&bytes) == hash,
                "peer content-address binding"
            );
        } else {
            crate::service_proof::verify(
                &proof,
                peer.key,
                &crate::service_proof::Scope {
                    url: &url,
                    method: "GET",
                    request: &request_id,
                    status: status.as_u16(),
                },
                &bytes,
            )?;
        }
        Ok(bytes)
    }
    async fn fetch_blob(
        &self,
        peers: &[Peer],
        hash: Id,
        limit: usize,
        expected: Option<u64>,
        used: &mut BTreeMap<Id, i64>,
    ) -> Result<Vec<u8>> {
        let mut unavailable = false;
        for peer in peers {
            if let Err(error) = self.verify(peer).await {
                unavailable |= error.is::<PeerUnavailable>();
                continue;
            }
            match self
                .get(
                    peer,
                    &format!("/federation/blobs/{}", hex::encode(hash)),
                    limit,
                    expected,
                )
                .await
            {
                Ok(bytes) if auth::hash(&bytes) == hex::encode(hash) => {
                    used.insert(peer.key, peer.version);
                    return Ok(bytes);
                }
                Err(error) => unavailable |= error.is::<PeerUnavailable>(),
                Ok(_) => {}
            }
        }
        ensure!(!unavailable, PeerUnavailable);
        anyhow::bail!("no independently approved peer supplied the required verified blob")
    }
    pub(crate) async fn reconcile_partition(
        &self,
        key: &str,
        filter: &str,
        prefix: &str,
        scope: &str,
    ) -> Result<Vec<Id>> {
        use crate::inventory::{self, FRAME_BYTES, MAX_ITEMS, PartitionRequired, ReconcileReply};
        use negentropy::Negentropy;
        let peer = self.repo.approved_peer(id(key)?).await?;
        self.verify(&peer).await?;
        let local = self
            .repo
            .inventory_rows_scoped(scope, filter, prefix)
            .await?;
        let mut neg = Negentropy::owned(inventory::storage(&local)?, FRAME_BYTES as u64)?;
        let mut message = neg.initiate()?;
        let mut session = None::<String>;
        let mut need = Vec::new();
        let mut have = Vec::new();
        let mut bytes = 0usize;
        let mut completed = false;
        let endpoint = format!("{}/reconcile", peer.endpoint);
        let outcome=async {
            for _ in 0..256 {
                self.current(&peer).await?;
                let body=serde_json::to_vec(&serde_json::json!({"session":session,"filter":filter,"prefix":prefix,"inventory":scope,"message":hex::encode(&message)}))?;
                let authorization=self.authorization_for(&endpoint,"POST",&body)?;
                let request_id=auth::token(&authorization).map_err(anyhow::Error::msg)?.id;
                let response=self.transport(&peer).await?.post(&endpoint).header("authorization",authorization).body(body).send().await?;
                let status=response.status();let proof=response.headers().get(crate::service_proof::HEADER).filter(|v|v.as_bytes().len()<=16384).and_then(|v|v.to_str().ok()).unwrap_or("").to_owned();let raw=transfer::read_bounded(response,262144,None).await.map_err(|error|PeerUnavailable::transport(error.into()))?;
                crate::service_proof::verify(&proof,peer.key,&crate::service_proof::Scope{url:&endpoint,method:"POST",request:&request_id,status:status.as_u16()},&raw)?;
                if status==reqwest::StatusCode::PAYLOAD_TOO_LARGE && serde_json::from_slice::<serde_json::Value>(&raw)?.get("error").and_then(|v|v.as_str())==Some("partition_required") {return Err(PartitionRequired.into());}
                ensure!(status==reqwest::StatusCode::OK,"reconciliation peer rejected request");
                let reply:ReconcileReply=serde_json::from_slice(&raw)?;
                ensure!(reply.prefix==prefix && reply.snapshot && !reply.closed && reply.message.len()<=FRAME_BYTES*2 && auth::valid_hash(&reply.session),"invalid snapshot response scope");
                if let Some(previous)=&session {ensure!(*previous==reply.session,"peer changed snapshot mid-session");}
                session=Some(reply.session);
                let response=hex::decode(reply.message)?;bytes+=message.len()+response.len();ensure!(bytes<=16*1024*1024,"reconciliation byte budget");
                let next=neg.reconcile_with_ids(&response,&mut have,&mut need)?;
                ensure!(need.len()<=MAX_ITEMS && have.len()<=MAX_ITEMS,"reconciliation ID budget");
                ensure!(need.iter().chain(&have).all(|id|hex::encode(id.as_bytes()).starts_with(prefix)),"peer IDs outside partition");
                match next {Some(next)=>message=next,None=>{completed=true;break;}}
            }
            ensure!(completed,"reconciliation round budget");Ok::<_,anyhow::Error>(())
        }.await;
        if let Some(session) = session {
            let body = serde_json::to_vec(
                &serde_json::json!({"session":session,"inventory":scope,"close":true}),
            )?;
            if let Ok(client) = self.transport(&peer).await {
                let _ = client
                    .post(&endpoint)
                    .header(
                        "authorization",
                        self.authorization_for(&endpoint, "POST", &body)?,
                    )
                    .body(body)
                    .send()
                    .await;
            }
        }
        outcome?;
        Ok(need.into_iter().map(|id| *id.as_bytes()).collect())
    }
    async fn check_job(&self, lease: Option<&crate::db::Lease>) -> Result<()> {
        if let Some(lease) = lease {
            ensure!(
                self.repo.lease_current(lease).await?,
                "job lease is no longer current"
            );
        }
        Ok(())
    }
    pub async fn synchronize(&self, request: SyncRequest) -> Result<SyncResult> {
        self.synchronize_fenced(request, None, None).await
    }
    pub(crate) async fn synchronize_fenced(
        &self,
        request: SyncRequest,
        lease: Option<crate::db::Lease>,
        expected_kind: Option<i32>,
    ) -> Result<SyncResult> {
        let operation = self
            .repo
            .begin_staging_operation(self.storage.owner(), self.storage.volume_name(), 660)
            .await?;
        let result = tokio::time::timeout(
            Duration::from_secs(600),
            self.synchronize_held(request, lease, expected_kind, &operation),
        )
        .await
        .context("publication transfer deadline")?;
        self.repo.end_staging_operation(&operation).await?;
        result
    }
    async fn synchronize_held(
        &self,
        request: SyncRequest,
        lease: Option<crate::db::Lease>,
        expected_kind: Option<i32>,
        operation: &crate::staging_operation::StagingOperation,
    ) -> Result<SyncResult> {
        use crate::transfer_rejection::{RejectionReason as Why, TransferRejection as Reject};
        self.check_job(lease.as_ref()).await?;
        ensure!(request.fallbacks.len() <= 7, "fallback peer budget");
        let event_id = id(&request.event)?;
        Filter::new(&request.filter)
            .and_then(Filter::compile)
            .map_err(anyhow::Error::msg)?;
        let mut keys = vec![id(&request.peer)?];
        for key in &request.fallbacks {
            let key = id(key)?;
            ensure!(!keys.contains(&key), "duplicate fallback peer");
            keys.push(key);
        }
        let mut peers = Vec::new();
        for key in keys {
            peers.push(self.repo.approved_peer(key).await?);
        }
        if self.repo.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM events WHERE id=$1 AND state IN ('removed','withdrawing'))", &[&event_id.as_slice()]).await?.get::<_,bool>(0) {
            return Err(Reject::policy(Why::LocalRemoval).into());
        }
        let mut used = BTreeMap::new();
        let mut selected = None;
        let mut unavailable = false;
        for peer in &peers {
            self.check_job(lease.as_ref()).await?;
            if let Err(error) = self.verify(peer).await {
                unavailable |= error.is::<PeerUnavailable>();
                continue;
            }
            let bytes = match self
                .get(
                    peer,
                    &format!("/federation/events/{}", request.event),
                    1048576,
                    None,
                )
                .await
            {
                Ok(bytes) => bytes,
                Err(error) => {
                    unavailable |= error.is::<PeerUnavailable>();
                    continue;
                }
            };
            let Ok(event) = serde_json::from_slice::<Event>(&bytes) else {
                continue;
            };
            if event.verify().is_ok() && event.id == request.event {
                if !matches!(event.kind, 1001 | 1002)
                    || expected_kind.is_some_and(|kind| event.kind != kind as u32)
                {
                    return Err(Reject::invalid(Why::InventoryKind).into());
                }
                publication::pointer_for_sync(&event)?;
                used.insert(peer.key, peer.version);
                selected = Some(event);
                break;
            }
        }
        ensure!(selected.is_some() || !unavailable, PeerUnavailable);
        let event =
            selected.context("no approved peer supplied the requested signed publication")?;
        ensure!(id(&event.id)? == event_id, "event identity mismatch");
        let mh = publication::pointer_for_sync(&event)?;
        let bytes = self
            .fetch_blob(&peers, mh, transfer::MANIFEST_LIMIT, None, &mut used)
            .await?;
        let (pages, matched) = if event.kind == 1001 {
            let m = manifest::chapter_for_sync(&bytes)?;
            (
                manifest::unique_pages(&m.pages)
                    .map_err(|_| Reject::invalid(Why::Manifest))?
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>(),
                self.repo
                    .manifest_matches(&event, &m, &request.filter)
                    .await?,
            )
        } else {
            manifest::series_for_sync(&bytes)?;
            let series = crate::series_metadata::parse(&bytes)
                .map_err(|_| Reject::invalid(Why::Manifest))?;
            (
                series.cover.into_iter().collect(),
                self.repo.series_matches(&event, &request.filter).await?,
            )
        };
        if !matched {
            return Ok(SyncResult {
                event: event.id,
                matched: false,
                hosted: false,
                projected: false,
                originals_fetched: 0,
                used_peers: used.keys().map(hex::encode).collect(),
            });
        }
        if pages
            .iter()
            .any(|p| !self.storage.formats.contains(&p.mime))
        {
            return Err(Reject::policy(Why::LocalFormat).into());
        }
        if pages.iter().any(|p| {
            p.size
                > self
                    .storage
                    .max_blob_bytes
                    .min(transfer::ORIGINAL_LIMIT as u64)
        }) {
            return Err(Reject::policy(Why::LocalSize).into());
        }
        {
            let _permit = self.storage.slots.acquire().await?;
            let intent = self.storage.put(mh, &bytes).await?;
            self.repo.hold_staging_original(operation, &intent).await?;
        }
        let mut fetched = 0;
        for page in pages {
            self.check_job(lease.as_ref()).await?;
            let hash = id(&page.hash)?;
            if let Ok((intent, existing)) = self.storage.snapshot(hash, self.storage.owner()).await
                && existing.len() as u64 == page.size
            {
                crate::node::spawn_blocking_owned_scope(move || manifest::validate_image(&existing, &page))
                    .await?
                    .map_err(|_| Reject::invalid(Why::Original))?;
                self.repo.hold_staging_original(operation, &intent).await?;
                continue;
            }
            let _permit = self.storage.slots.acquire().await?;
            let bytes = self
                .fetch_blob(
                    &peers,
                    hash,
                    transfer::ORIGINAL_LIMIT,
                    Some(page.size),
                    &mut used,
                )
                .await?;
            let bytes = crate::node::spawn_blocking_owned_scope(move || -> Result<Vec<u8>> {
                manifest::validate_image(&bytes, &page)
                    .map_err(|_| Reject::invalid(Why::Original))?;
                Ok(bytes)
            })
            .await??;
            let intent = self.storage.put(hash, &bytes).await?;
            self.repo.hold_staging_original(operation, &intent).await?;
            fetched += 1;
        }
        let _permit = self.publications.slots.acquire().await?;
        // Approval epochs used for event/manifest/original transfer are checked again
        // under the short publication commit; peer receipt never grants publisher login.
        let prepared = self
            .publications
            .prepare_from_peers(&event, used.iter().map(|(k, v)| (*k, *v)).collect())
            .await?;
        self.publications.commit(prepared.with_lease(lease)).await?;
        Ok(SyncResult {
            event: event.id,
            matched: true,
            hosted: true,
            projected: self.repo.projection_complete(event_id).await?,
            originals_fetched: fetched,
            used_peers: used.keys().map(hex::encode).collect(),
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibrarySyncRequest {
    pub peer: String,
    pub publisher: String,
}
#[derive(Serialize)]
pub struct LibrarySyncResult {
    pub matched: bool,
    pub event: String,
    pub publisher: String,
    pub hosted: bool,
    pub superseded: bool,
    pub projected: bool,
    pub objects_fetched: usize,
}
impl PeerSync {
    pub async fn synchronize_library(
        &self,
        library: Arc<crate::library_storage::LibraryObjects>,
        request: LibrarySyncRequest,
    ) -> Result<LibrarySyncResult> {
        self.synchronize_library_fenced(library, request, None, "")
            .await
    }
    pub(crate) async fn synchronize_library_inventory(
        &self,
        library: Arc<crate::library_storage::LibraryObjects>,
        peer_key: &str,
        event_id: Id,
        lease: crate::db::Lease,
        filter: &str,
    ) -> Result<LibrarySyncResult> {
        self.check_job(Some(&lease)).await?;
        let peer = self.repo.approved_peer(id(peer_key)?).await?;
        self.verify(&peer).await?;
        let bytes = self
            .get(
                &peer,
                &format!("/federation/library-routing/{}", hex::encode(event_id)),
                16384,
                None,
            )
            .await?;
        let pointer: Event = serde_json::from_slice(&bytes)?;
        crate::library_manifest::pointer_for_sync(&pointer)?;
        ensure!(
            id(&pointer.id)? == event_id,
            "library routing identity mismatch"
        );
        self.synchronize_library_fenced(
            library,
            LibrarySyncRequest {
                peer: peer_key.into(),
                publisher: pointer.pubkey,
            },
            Some(lease),
            filter,
        )
        .await
    }
    pub(crate) async fn synchronize_library_fenced(
        &self,
        library: Arc<crate::library_storage::LibraryObjects>,
        request: LibrarySyncRequest,
        lease: Option<crate::db::Lease>,
        filter: &str,
    ) -> Result<LibrarySyncResult> {
        let operation = self
            .repo
            .begin_staging_operation(self.storage.owner(), self.storage.volume_name(), 660)
            .await?;
        let result = tokio::time::timeout(
            Duration::from_secs(600),
            self.synchronize_library_held(library, request, lease, filter, &operation),
        )
        .await
        .context("library transfer deadline")?;
        self.repo.end_staging_operation(&operation).await?;
        result
    }
    async fn synchronize_library_held(
        &self,
        library: Arc<crate::library_storage::LibraryObjects>,
        request: LibrarySyncRequest,
        lease: Option<crate::db::Lease>,
        filter: &str,
        operation: &crate::staging_operation::StagingOperation,
    ) -> Result<LibrarySyncResult> {
        self.check_job(lease.as_ref()).await?;
        use crate::{
            accounts::StageOutcome,
            library::ExpectedHead,
            library_manifest::{self, Limits, Root},
            library_storage::ObjectType,
        };
        let peer = self.repo.approved_peer(id(&request.peer)?).await?;
        let publisher = id(&request.publisher)?;
        self.verify(&peer).await?;
        let bytes = self
            .get(
                &peer,
                &format!("/federation/libraries/{}", hex::encode(publisher)),
                32768,
                None,
            )
            .await?;
        let head: serde_json::Value = serde_json::from_slice(&bytes)?;
        ensure!(head["projected"] == true, "remote library not projected");
        let event: Event = serde_json::from_value(head["event"].clone())?;
        ensure!(
            event.pubkey == hex::encode(publisher),
            "peer returned another author"
        );
        let hash = library_manifest::pointer_for_sync(&event)?;
        if !self.repo.series_matches(&event, filter).await? {
            return Ok(LibrarySyncResult {
                matched: false,
                event: event.id,
                publisher: request.publisher,
                hosted: false,
                superseded: false,
                projected: false,
                objects_fetched: 0,
            });
        }
        let mut permit = self
            .repo
            .peer_account_permit(publisher, peer.clone())
            .await?;
        permit.sync_lease = lease.clone();
        let limits = Limits::default();
        let root_bytes = self
            .get(
                &peer,
                &format!("/federation/library/blobs/{}", hex::encode(hash)),
                limits.max_root_bytes,
                None,
            )
            .await?;
        ensure!(
            auth::hash(&root_bytes) == hex::encode(hash),
            "root hash mismatch"
        );
        library_manifest::check_object_support(&root_bytes)?;
        let root = Root::decode(&root_bytes, &limits).map_err(|_| {
            crate::transfer_rejection::TransferRejection::invalid(
                crate::transfer_rejection::RejectionReason::LibraryObject,
            )
        })?;
        let mut descriptors = BTreeMap::new();
        for family in &root.families {
            for d in &family.chunks {
                if !d.is_empty() {
                    ensure!(
                        d.hash != hash,
                        crate::transfer_rejection::TransferRejection::invalid(
                            crate::transfer_rejection::RejectionReason::LibraryObject
                        )
                    );
                    if let Some(size) = descriptors.insert(d.hash, d.size) {
                        ensure!(
                            size == d.size,
                            crate::transfer_rejection::TransferRejection::invalid(
                                crate::transfer_rejection::RejectionReason::LibraryObject
                            )
                        );
                    }
                }
            }
        }
        let mut fetched = 1;
        for (hash, size) in descriptors {
            self.check_job(lease.as_ref()).await?;
            let bytes = self
                .get(
                    &peer,
                    &format!("/federation/library/blobs/{}", hex::encode(hash)),
                    limits.max_chunk_bytes,
                    Some(size),
                )
                .await?;
            ensure!(
                auth::hash(&bytes) == hex::encode(hash),
                "chunk hash mismatch"
            );
            library_manifest::check_object_support(&bytes)?;
            crate::library_manifest::Chunk::decode(&bytes, &limits).map_err(|_| {
                crate::transfer_rejection::TransferRejection::invalid(
                    crate::transfer_rejection::RejectionReason::LibraryObject,
                )
            })?;
            let intent = library
                .stage(permit.clone(), ObjectType::Chunk, bytes)
                .await?;
            self.repo.hold_staging_original(operation, &intent).await?;
            fetched += 1;
        }
        let intent = library
            .stage(permit.clone(), ObjectType::Root, root_bytes)
            .await?;
        self.repo.hold_staging_original(operation, &intent).await?;
        self.current(&peer).await?;
        let outcome = library
            .commit(permit, event.clone(), ExpectedHead::Any)
            .await?;
        let superseded = outcome == StageOutcome::Superseded;
        Ok(LibrarySyncResult {
            matched: true,
            event: event.id.clone(),
            publisher: request.publisher,
            hosted: !superseded,
            superseded,
            projected: self.repo.projection_complete(id(&event.id)?).await?,
            objects_fetched: fetched,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairRequest {
    pub peer: String,
    pub hash: String,
    #[serde(default)]
    pub library: bool,
}
impl PeerSync {
    pub async fn repair_original(&self, request: RepairRequest) -> Result<bool> {
        let peer = self.repo.approved_peer(id(&request.peer)?).await?;
        self.verify(&peer).await?;
        let intent = self
            .repo
            .blob_intent(id(&request.hash)?)
            .await?
            .context("no local receipt to repair")?;
        ensure!(
            intent.owner == self.storage.owner(),
            "repair target has another owner"
        );
        if !self.repo.blob_quarantined(&intent.key).await? {
            if self
                .storage
                .get(intent.key.hash, intent.owner)
                .await
                .is_ok()
            {
                return Ok(false);
            }
            ensure!(
                self.repo.blob_quarantined(&intent.key).await?,
                "cannot establish durable quarantine"
            );
        }
        let bytes = self
            .get(
                &peer,
                &format!(
                    "{}/{}",
                    if request.library {
                        "/federation/library/blobs"
                    } else {
                        "/federation/blobs"
                    },
                    request.hash
                ),
                self.storage.max_blob_bytes as usize,
                Some(intent.size),
            )
            .await?;
        self.storage.repair_verified(intent, bytes, peer).await
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountSyncRequest {
    pub peer: String,
    pub event: String,
}
impl PeerSync {
    pub async fn synchronize_account_event(
        &self,
        request: AccountSyncRequest,
    ) -> Result<serde_json::Value> {
        self.synchronize_account_fenced(request, None, None, "")
            .await
    }
    pub(crate) async fn synchronize_account_fenced(
        &self,
        request: AccountSyncRequest,
        lease: Option<crate::db::Lease>,
        scope: Option<&str>,
        filter: &str,
    ) -> Result<serde_json::Value> {
        self.check_job(lease.as_ref()).await?;
        let peer = self.repo.approved_peer(id(&request.peer)?).await?;
        self.verify(&peer).await?;
        let event_id = id(&request.event)?;
        let bytes = self
            .get(
                &peer,
                &format!("/federation/account-events/{}", hex::encode(event_id)),
                1048576,
                None,
            )
            .await?;
        let event: Event = serde_json::from_slice(&bytes)?;
        event.verify().map_err(anyhow::Error::msg)?;
        ensure!(
            event.id == request.event && matches!(event.kind, 0 | 1 | 3 | 5 | 7 | 1111 | 10002),
            "unsupported or mismatched account event"
        );
        if let Some(scope) = scope {
            ensure!(
                crate::inventory::inventory_kinds(scope)?.contains(&(event.kind as i32)),
                "event outside inventory scope"
            );
        }
        if !self.repo.series_matches(&event, filter).await? {
            return Ok(serde_json::json!({"event":event.id,"matched":false}));
        }
        if scope == Some("social") {
            return self
                .synchronize_social_dependencies(&peer, event, lease, filter)
                .await;
        }
        self.apply_account_event(&peer, &event, lease).await
    }
    async fn apply_account_event(
        &self,
        peer: &Peer,
        event: &Event,
        lease: Option<crate::db::Lease>,
    ) -> Result<serde_json::Value> {
        let event_id = id(&event.id)?;
        let mut permit = self
            .repo
            .peer_account_permit(id(&event.pubkey)?, peer.clone())
            .await?;
        permit.sync_lease = lease;
        if event.kind == 5 {
            let changed = self.repo.delete_ordinary(&permit, event).await?;
            return Ok(
                serde_json::json!({"event":event.id,"request_recorded":true,"ordinary_withdrawals":changed,"protected_publications_require_local_policy":true}),
            );
        }
        let inserted = if event.kind == 1 {
            self.repo.stage_personal_note(&permit, event).await?
        } else if matches!(event.kind, 7 | 1111) {
            self.repo.stage_social(&permit, event).await?
        } else {
            let outcome = self.repo.stage_account_metadata(&permit, event).await?;
            if outcome == crate::accounts::StageOutcome::Superseded {
                return Ok(serde_json::json!({"event":event.id,"superseded":true}));
            }
            outcome == crate::accounts::StageOutcome::Accepted
        };
        Ok(
            serde_json::json!({"event":event.id,"inserted":inserted,"projected":self.repo.projection_complete(event_id).await?}),
        )
    }
    async fn synchronize_social_dependencies(
        &self,
        peer: &Peer,
        event: Event,
        lease: Option<crate::db::Lease>,
        filter: &str,
    ) -> Result<serde_json::Value> {
        let target = event.id.clone();
        let mut stack = vec![(event, false)];
        let mut visited = BTreeMap::<Id, bool>::new();
        let mut ordered = Vec::new();
        let mut total = 0usize;
        while let Some((event, expanded)) = stack.pop() {
            self.check_job(lease.as_ref()).await?;
            let event_id = id(&event.id)?;
            if expanded {
                visited.insert(event_id, true);
                ordered.push(event);
                continue;
            }
            if let Some(done) = visited.get(&event_id) {
                ensure!(*done, "cyclic social dependency");
                continue;
            }
            ensure!(visited.len() < 64, "social dependency count budget");
            total = total
                .checked_add(serde_json::to_vec(&event)?.len())
                .context("dependency byte overflow")?;
            ensure!(total <= 2 * 1024 * 1024, "social dependency byte budget");
            ensure!(
                matches!(event.kind, 1 | 7 | 1111),
                "dependency outside social inventory"
            );
            ensure!(
                self.repo.series_matches(&event, filter).await?,
                "missing dependency excluded by selected filter"
            );
            let (root, dependencies) = crate::social::replication_dependencies(&event)?;
            if let Some(root) = root {
                ensure!(
                    self.repo.visible_event(root).await?.is_some(),
                    "manga root must already be hosted locally"
                );
            }
            visited.insert(event_id, false);
            stack.push((event, true));
            for dependency in dependencies {
                if self.repo.visible_event(dependency).await?.is_some() {
                    continue;
                }
                let bytes = self
                    .get(
                        peer,
                        &format!("/federation/account-events/{}", hex::encode(dependency)),
                        1048576,
                        None,
                    )
                    .await?;
                let parent: Event = serde_json::from_slice(&bytes)?;
                parent.verify().map_err(anyhow::Error::msg)?;
                ensure!(
                    id(&parent.id)? == dependency,
                    "social dependency identity mismatch"
                );
                stack.push((parent, false));
            }
        }
        let processed = ordered.len().saturating_sub(1);
        let mut result = None;
        for event in ordered {
            let applied = self
                .apply_account_event(peer, &event, lease.clone())
                .await?;
            if event.id == target {
                result = Some(applied);
            }
        }
        let mut result = result.context("social target not processed")?;
        result["dependencies_processed"] = serde_json::json!(processed);
        Ok(result)
    }
}
