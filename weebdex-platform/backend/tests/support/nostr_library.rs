//! RECONSTRUCTED UNVERIFIED genuine original-library HTTP fixture.
#![allow(dead_code, unused_imports)]
#[path = "../../../../manga-protocol-lab/server/tests/support/mod.rs"]
mod support;
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Id, Repository},
    library_client_bundle::SaveBundle,
    library_manifest::{Chunk, Descriptor, Family, FamilyRoot, Limits, Record, Root},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Duration};
pub use support::*;
use weebdex_platform::{
    AppState, Config, domains,
    protocol::{
        bridge::import_page,
        publication::{OperatorRuntimeConfig, PublicationInput, PublicationRuntime},
    },
    public_api::{PostgresPublicIdResolver, PublicIdResolver, ResourceKind},
};
pub fn id(s: &str) -> Id {
    hex::decode(s).unwrap().try_into().unwrap()
}
pub fn request_event(key: u8, url: &str, created: u64, service: u8) -> auth::Event {
    sign_at(
        key,
        27235,
        vec![
            vec!["u".into(), url.into()],
            vec!["method".into(), "GET".into()],
            vec!["service".into(), public(service)],
            vec!["payload".into(), auth::hash(b"")],
            vec!["nonce".into(), hex::encode(rand::random::<[u8; 16]>())],
        ],
        String::new(),
        created,
    )
}
pub fn proof(key: u8, url: &str) -> String {
    token(&request_event(key, url, auth::now(), 11))
}
pub fn bundle(key: u8, time: u64, records: Vec<Record>) -> Result<SaveBundle> {
    let limits = Limits::default();
    let mut chunks = BTreeMap::new();
    let mut families = vec![];
    for family in Family::ALL {
        let records = records
            .iter()
            .filter(|r| r.family() == family)
            .cloned()
            .collect::<Vec<_>>();
        let count = if records.len() > limits.max_chunk_records {
            16
        } else {
            1
        };
        let mut buckets = vec![vec![]; count];
        for r in records {
            buckets[manga_node::library_manifest::bucket_for(&r.key(), count)?].push(r);
        }
        let mut descriptors = vec![];
        for records in buckets {
            let descriptor = if records.is_empty() {
                Descriptor::empty()
            } else {
                let bytes = Chunk::new(family, records, &limits)?.encode(&limits)?;
                let summary = Chunk::decode(&bytes, &limits)?.1;
                chunks.insert(summary.hash(), bytes);
                Descriptor::from(&summary)
            };
            descriptors.push(descriptor);
        }
        families.push(FamilyRoot {
            family,
            chunks: descriptors,
        });
    }
    let root = Root { families }.encode(&limits)?;
    let event = sign_at(
        key,
        11999,
        vec![],
        json!({"manifest":auth::hash(&root),"version":1}).to_string(),
        time,
    );
    SaveBundle::validate(event, root, chunks)
}
pub fn chapter(event: &auth::Event, completed: bool, page: Option<u32>) -> Record {
    Record::Chapter {
        id: id(&event.id),
        completed,
        page,
        updated_at: 1,
    }
}
pub async fn save(c: &reqwest::Client, origin: &str, key: u8, bundle: &SaveBundle) -> Result<()> {
    let mut cursor = 0;
    while let Some(batch) = bundle.batch(cursor)? {
        let url = format!("{origin}/compat/mangadex/nostr/library/batches");
        let r = c
            .post(&url)
            .header("authorization", http(key, &url, &batch.body))
            .body(batch.body)
            .send()
            .await?;
        ensure!(
            r.status().is_success(),
            "genuine original batch: {}",
            r.text().await?
        );
        cursor = batch.next_object;
    }
    let url = format!("{origin}/compat/mangadex/nostr/libraries");
    let body = bundle.commit_body()?;
    let r = c
        .post(&url)
        .header("authorization", http(key, &url, &body))
        .body(body)
        .send()
        .await?;
    ensure!(
        r.status().is_success(),
        "genuine original commit: {}",
        r.text().await?
    );
    Ok(())
}
pub async fn get(c: &reqwest::Client, url: &str, key: Option<u8>, status: u16) -> Result<Value> {
    let mut req = c.get(url);
    if let Some(key) = key {
        req = req.header("authorization", proof(key, url));
    }
    response(req, status).await
}
pub async fn response(req: reqwest::RequestBuilder, status: u16) -> Result<Value> {
    let r = req.send().await?;
    let actual = r.status().as_u16();
    let headers = r.headers().clone();
    let value: Value = r.json().await?;
    ensure!(actual == status, "expected {status}, got {actual}: {value}");
    ensure!(
        headers
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.contains("no-store")),
        "private response must not be cached"
    );
    if status >= 400 {
        assert_eq!(value["result"], "error");
        assert_eq!(value["errors"][0]["status"], status);
    }
    Ok(value)
}
pub async fn publish(
    runtime: &PublicationRuntime,
    kind: u32,
    manifest: Vec<u8>,
    originals: BTreeMap<String, Vec<u8>>,
) -> Result<auth::Event> {
    Ok(runtime
        .publish_operator(PublicationInput {
            kind,
            canonical_manifest: manifest,
            originals,
            created_at: None,
        })
        .await?
        .event)
}
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}
pub struct Fixture {
    pub repo: Repository,
    pub state: AppState,
    pub runtime: std::sync::Arc<PublicationRuntime>,
    pub c: reqwest::Client,
    pub origin: String,
    pub native_origin: String,
    native: weebdex_platform::protocol::publication::NativeListener,
    _server: Server,
    _dir: tempfile::TempDir,
}
impl Fixture {
    pub async fn start() -> Result<Self> {
        Self::start_with_backend(true).await
    }
    /// Real original auth/admission needs no catalogue backend. Graph fixtures
    /// still require the actual pinned strfry path and use start().
    pub async fn start_auth_only() -> Result<Self> {
        Self::start_with_backend(false).await
    }
    async fn start_with_backend(backend: bool) -> Result<Self> {
        let dsn =
            std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").context("Use isolated runner")?;
        let pg: tokio_postgres::Config = dsn.parse()?;
        ensure!(
            pg.get_dbname() == Some("weebdex_platform_test"),
            "fixture database only"
        );
        ensure!(
            !pg.get_hosts().is_empty()
                && pg
                    .get_hosts()
                    .iter()
                    .all(|h| matches!(h,tokio_postgres::config::Host::Tcp(h)if h=="127.0.0.1")),
            "owned loopback fixture only"
        );
        ensure!(
            std::env::var("PLATFORM_TEST_RUNTIME_DIR")?.contains("/weebdex-platform/test-runtime/"),
            "owned runtime required"
        );
        let repo = Repository::connect(DbConfig {
            dsn: dsn.clone(),
            connections: 8,
            acquisition_timeout: Duration::from_secs(2),
            cache_weight: 1048576,
            plaintext_local: true,
        })
        .await?;
        repo.migrate().await?;
        repo.bind_instance(id(&public(11)), id(&public(1))).await?;
        for key in [2, 3, 4] {
            repo.set_account_admission(id(&public(key)), true).await?;
        }
        let dir = tempfile::tempdir()?;
        let service = dir.path().join("service.key");
        write_key(&service, 11)?;
        let publisher = dir.path().join("publisher.key");
        write_key(&publisher, 1)?;
        let reserve = std::net::TcpListener::bind("127.0.0.1:0")?;
        let native_bind = reserve.local_addr()?;
        drop(reserve);
        let native_origin = format!("http://{native_bind}");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let origin = format!("http://{}", listener.local_addr()?);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let mut publication = OperatorRuntimeConfig {
            operator_key_file: publisher,
            expected_publisher: public(1),
            storage_owner: public(1),
            storage_holder: public(11),
            storage: json!({"path":dir.path().join("objects"),"volume":"publicmarkers","physical_bytes":16777216,"staging_bytes":16777216,"max_blob_bytes":1000000}),
            group_keys_dir: None,
            backend: None,
            native_node: Some(
                json!({"bind":native_bind.to_string(),"base_url":native_origin,"service_key_file":service,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"peer_discovery_enabled":false,"staging_reclamation":false,"backend":{"binary":std::env::var_os("MANGA_TEST_STRFRY_BINARY").map(std::path::PathBuf::from).unwrap_or_else(||root.join("manga-protocol-lab/vendor/strfry/strfry")),"data_dir":dir.path().join("backend"),"map_bytes":67108864}}),
            ),
        };
        if !backend {
            publication
                .native_node
                .as_mut()
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove("backend");
        }
        let state = AppState::connect(
            Config {
                public_origin: origin.clone(),
                database_plaintext_local: true,
                database_connections: 8,
                protocol_publication: Some(publication.clone()),
                ..Config::default()
            },
            dsn,
        )
        .await?;
        state.db.migrate(domains::MIGRATIONS).await?;
        state.protocol.verify_schema().await?;
        state.protocol.install_visibility_function().await?;
        let runtime = PublicationRuntime::open(repo.clone(), publication).await?;
        state
            .protocol
            .attach_publication_runtime(runtime.clone())
            .await;
        let native = runtime
            .start_native_listener()
            .await?
            .context("configured native listener")?;
        let app = weebdex_platform::router::router(state.clone());
        let _server = Server(tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
            .unwrap();
        }));
        let c = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(20))
            .build()?;
        Ok(Self {
            repo,
            state,
            runtime,
            c,
            origin,
            native_origin,
            native,
            _server,
            _dir: dir,
        })
    }
    pub async fn shutdown(mut self) -> Result<()> {
        if let Some(shutdown) = self.native.shutdown.take() {
            let _ = shutdown.send(());
        }
        tokio::time::timeout(Duration::from_secs(30), self.native.task).await???;
        Ok(())
    }
}
