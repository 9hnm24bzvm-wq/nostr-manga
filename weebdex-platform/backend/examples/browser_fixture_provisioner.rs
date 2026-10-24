//! Guarded disposable browser-fixture provisioning, never a deployment tool.
//! Secrets remain private fixture files; stdout contains only public receipts.
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use manga_node::{
    auth,
    db::{DbConfig, Id, Repository},
    manifest::{Manifest, Page},
    series_metadata::{Credit, SeriesMetadata},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};
use weebdex_platform::protocol::publication::{
    OperatorRuntimeConfig, PublicationInput, PublicationRuntime,
};
use zeroize::Zeroizing;

#[path = "browser_fixture/ui_regressions.rs"]
mod ui_regressions;
#[path = "browser_fixture/signed_features.rs"]
mod signed_features;

// These must satisfy the original UploadStore volume grammar (alnum or '-').
const SOURCE_VOLUME: &str = "browser-fixture";
const RECIPIENT_VOLUME: &str = "browser-recipient-fixture";
const UPLOAD_SCENARIO: &str = "public-upload-v1";
const UPLOAD_USER: &str = "UpUser0001";
const UPLOAD_GROUP: &str = "UpGroup001";

#[derive(Debug)]
enum SeedStage {
    Input,
    Runtime,
    Cover,
    Series,
    SeriesSnapshot,
    ChapterImage,
    ChapterEncoding,
    ChapterPublication,
    ChapterSnapshot,
}
impl SeedStage {
    fn label(&self) -> &'static str {
        match self {
            Self::Input => "seed-input",
            Self::Runtime => "seed-runtime-open",
            Self::Cover => "seed-cover-validation",
            Self::Series => "seed-series-publication",
            Self::SeriesSnapshot => "seed-series-snapshot",
            Self::ChapterImage => "seed-chapter-image-validation",
            Self::ChapterEncoding => "seed-chapter-encoding",
            Self::ChapterPublication => "seed-chapter-publication",
            Self::ChapterSnapshot => "seed-chapter-snapshot",
        }
    }
}
impl std::fmt::Display for SeedStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
fn safe_failure(error: &anyhow::Error) -> (&'static str, &'static str) {
    let stage = error
        .downcast_ref::<SeedStage>()
        .map(SeedStage::label)
        .unwrap_or("fixture-action");
    // Only fixed labels leave the fixture. Never interpolate the error chain,
    // request/config, filesystem paths, signed bodies or credential contents.
    let reason = if error
        .chain()
        .any(|cause| cause.to_string() == "invalid volume name")
    {
        "invalid-volume-name"
    } else if error.downcast_ref::<tokio_postgres::Error>().is_some() {
        "database-operation-failed"
    } else if error.downcast_ref::<serde_json::Error>().is_some() {
        "invalid-json-input"
    } else if let Some(io) = error.downcast_ref::<std::io::Error>() {
        match io.kind() {
            std::io::ErrorKind::NotFound => "filesystem-not-found",
            std::io::ErrorKind::PermissionDenied => "filesystem-permission-denied",
            _ => "filesystem-operation-failed",
        }
    } else {
        "operation-failed"
    };
    (stage, reason)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    root: PathBuf,
    #[serde(default)]
    ui_origin: Option<String>,
    #[serde(default)]
    protocol_origin: Option<String>,
    #[serde(default)]
    backend_binary: Option<PathBuf>,
    #[serde(default)]
    seed_input: Option<PathBuf>,
    #[serde(default)]
    pubkey: Option<String>,
    #[serde(default)]
    allowed: Option<bool>,
    #[serde(default)]
    event_id: Option<String>,
    #[serde(default)]
    recipient_origin: Option<String>,
    #[serde(default)]
    scenario: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChapterInput {
    chapter: u16,
    title: String,
    pages: Vec<PathBuf>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedInput {
    title: String,
    description: String,
    attribution: String,
    language: String,
    cover: PathBuf,
    chapters: Vec<ChapterInput>,
}
fn id(value: &str) -> Result<Id> {
    ensure!(auth::valid_hash(value), "invalid synthetic public identity");
    hex::decode(value)?
        .try_into()
        .ok()
        .context("synthetic identity size")
}
fn bounded_json(path: &Path) -> Result<Value> {
    let meta = std::fs::symlink_metadata(path)?;
    ensure!(
        meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= 65536,
        "fixture request file bounds"
    );
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}
fn private_path(root: &Path, path: &Path) -> Result<PathBuf> {
    let path = path.canonicalize()?;
    ensure!(path.starts_with(root), "fixture path escaped private root");
    Ok(path)
}
fn fixture_root(value: &Path) -> Result<PathBuf> {
    let parent = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("fixture project root")?
        .join("test-runtime")
        .canonicalize()?;
    let root = value.canonicalize()?;
    ensure!(
        root.parent() == Some(parent.as_path()),
        "fixture root must be a direct disposable test-runtime child"
    );
    let name = root
        .file_name()
        .and_then(|v| v.to_str())
        .context("fixture root name")?;
    ensure!(
        name.starts_with("browser-final-")
            && name.len() <= 80
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "reserved fixture root required"
    );
    let metadata = std::fs::symlink_metadata(value)?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "fixture root cannot be a link"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            metadata.permissions().mode() & 0o077 == 0,
            "private fixture root required"
        );
    }
    let marker = bounded_json(&root.join("fixture-owned.json"))?;
    ensure!(
        marker["fixture_only"] == true && marker["database"] == "weebdex_browser_test",
        "owned disposable fixture marker required"
    );
    Ok(root)
}
fn loopback_origin(value: Option<&str>) -> Result<String> {
    let value = value.context("fixture loopback origin required")?;
    let url = url::Url::parse(value)?;
    ensure!(
        url.scheme() == "http"
            && url.host_str() == Some("127.0.0.1")
            && url.port().is_some_and(|p| p > 0)
            && url.path() == "/"
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "exact reserved loopback HTTP origin required"
    );
    Ok(url.origin().ascii_serialization())
}
async fn repository(recipient: bool, root: &Path) -> Result<Repository> {
    let dsn = std::env::var("WEEBDEX_BROWSER_FIXTURE_DSN")
        .context("private disposable DSN environment required")?;
    let pg: tokio_postgres::Config = dsn.parse().context("invalid disposable DSN")?;
    let marker = bounded_json(&root.join("fixture-owned.json"))?;
    let expected = if recipient {
        "weebdex_browser_recipient_test"
    } else {
        "weebdex_browser_test"
    };
    ensure!(
        pg.get_dbname() == Some(expected)
            && pg.get_hosts() == [tokio_postgres::config::Host::Tcp("127.0.0.1".into())]
            && pg.get_ports().len() == 1
            && pg.get_ports()[0] > 1024
            && pg.get_ports()[0] != 5432
            && Some(u64::from(pg.get_ports()[0])) == marker["pg_port"].as_u64()
            && pg.get_password().is_none()
            && pg.get_options().is_none(),
        "provisioner accepts only reserved password-free disposable loopback database"
    );
    Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_millis(500),
        cache_weight: 4 * 1024 * 1024,
        plaintext_local: true,
    })
    .await
}
fn write_json(path: &Path, value: &Value) -> Result<()> {
    ensure!(
        !path.exists(),
        "fixture output already exists; refusing replacement"
    );
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}
fn synthetic_key(path: &Path) -> Result<String> {
    let (secret, public) = loop {
        let bytes = Zeroizing::new(rand::random::<[u8; 32]>());
        let secret = Zeroizing::new(hex::encode(&*bytes));
        if let Ok(event) = auth::sign(&secret, 1, vec![], String::new()) {
            break (secret, event.pubkey);
        }
    };
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(secret.as_bytes())?;
    file.sync_all()?;
    Ok(public)
}
async fn setup(repo: &Repository, root: &Path, request: &Request) -> Result<Value> {
    let ui = loopback_origin(request.ui_origin.as_deref())?;
    let origin = loopback_origin(request.protocol_origin.as_deref())?;
    ensure!(origin != ui, "original and UI fixture sockets must differ");
    let backend = request
        .backend_binary
        .as_ref()
        .context("real installed backend binary required")?
        .canonicalize()?;
    let expected = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("project root")?
        .parent()
        .context("workspace root")?
        .join("manga-protocol-lab/vendor/strfry/strfry")
        .canonicalize()?;
    ensure!(
        backend == expected && backend.is_file(),
        "only the reviewed installed original strfry binary is allowed"
    );
    repo.migrate().await?;
    repo.verify_schema().await?;
    for path in [
        "fixture-keys",
        "original-storage",
        "original-backend",
        "original-group-keys",
    ] {
        let path = root.join(path);
        std::fs::create_dir(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    let operator = synthetic_key(&root.join("fixture-keys/operator.key"))?;
    let service = synthetic_key(&root.join("fixture-keys/service.key"))?;
    ensure!(
        operator != service,
        "fixture operator and service identities must differ"
    );
    repo.set_publisher_admission(id(&operator)?, true).await?;
    let storage = json!({"path":root.join("original-storage"),"volume":SOURCE_VOLUME,"physical_bytes":512*1024*1024,"staging_bytes":128*1024*1024,"max_blob_bytes":8*1024*1024,"upload_slots":2,"reclamation_slots":2,"credit_lanes":4,"formats":["image/png","image/jpeg","image/webp"]});
    let parsed = url::Url::parse(&origin)?;
    let native = json!({"bind":format!("127.0.0.1:{}",parsed.port().context("native port")?),"base_url":origin,"service_key_file":root.join("fixture-keys/service.key"),"admin_pubkey":operator,"database_connections":8,"database_wait_ms":500,"cache_bytes":4*1024*1024,"database_plaintext_local":true,"storage":storage,"group_keys_dir":root.join("original-group-keys"),"backend":{"binary":backend,"data_dir":root.join("original-backend"),"timeout_seconds":10,"map_bytes":64*1024*1024},"read_policy":"public","cors_origins":[ui],"auto_approve_publication_deletions":false,"library_reclamation":false,"publication_reclamation":false,"staging_reclamation":false,"storage_audit_interval_seconds":0,"sync_worker_enabled":false,"peer_discovery_enabled":false});
    let runtime = json!({"operator_key_file":root.join("fixture-keys/operator.key"),"expected_publisher":operator,"storage_owner":operator,"storage_holder":service,"storage":storage,"group_keys_dir":root.join("original-group-keys"),"backend":null,"native_node":native});
    let _: OperatorRuntimeConfig = serde_json::from_value(runtime.clone())?;
    write_json(&root.join("protocol-runtime.json"), &runtime)?;
    let receipt = json!({"fixture_only":true,"original_migrations_verified":true,"operator_pubkey":operator,"service_key":service,"protocol_origin":origin,"relay_origin":origin.replacen("http://","ws://",1),"backend_binary_sha256":auth::hash(&std::fs::read(backend)?),"browser_account_admitted":false,"server_keys_private":true});
    write_json(&root.join("protocol-setup.json"), &receipt)?;
    Ok(receipt)
}
fn image(root: &Path, path: &Path, budget: &mut u64) -> Result<(Page, Vec<u8>)> {
    let path = private_path(root, path)?;
    let meta = std::fs::symlink_metadata(&path)?;
    ensure!(
        meta.is_file() && meta.len() > 0 && meta.len() <= 4 * 1024 * 1024,
        "seed image file bounds"
    );
    *budget = budget
        .checked_add(meta.len())
        .context("seed size overflow")?;
    ensure!(*budget <= 32 * 1024 * 1024, "seed total32MiB budget");
    let bytes = std::fs::read(&path)?;
    let format = image::guess_format(&bytes)?;
    ensure!(
        format == image::ImageFormat::Png,
        "fixture seed originals must be PNG"
    );
    let (width, height) =
        image::ImageReader::with_format(std::io::Cursor::new(&bytes), format).into_dimensions()?;
    ensure!(
        u64::from(width) * u64::from(height) <= 16 * 1024 * 1024,
        "seed pixel bound"
    );
    let page = Page {
        hash: auth::hash(&bytes),
        size: bytes.len() as u64,
        mime: "image/png".into(),
        width,
        height,
    };
    manga_node::manifest::validate_image(&bytes, &page).map_err(anyhow::Error::msg)?;
    Ok((page, bytes))
}
async fn seed(repo: &Repository, root: &Path, request: &Request) -> Result<Value> {
    let path = private_path(
        root,
        request.seed_input.as_ref().context("seed input required")?,
    )?;
    let input: SeedInput = serde_json::from_value(bounded_json(&path).context(SeedStage::Input)?)
        .context(SeedStage::Input)?;
    ensure!(
        input.title.len() <= 256
            && input.description.len() <= 4096
            && input.attribution.len() <= 2048
            && input.chapters.len() == 2,
        "bounded two-chapter seed required"
    );
    let runtime: OperatorRuntimeConfig =
        serde_json::from_value(bounded_json(&root.join("protocol-runtime.json"))?)?;
    let runtime = PublicationRuntime::open(repo.clone(), runtime)
        .await
        .context(SeedStage::Runtime)?;
    let snapshots = root.join("canonical-seed");
    std::fs::create_dir(&snapshots)?;
    let mut budget = 0;
    let (cover, bytes) = image(root, &input.cover, &mut budget).context(SeedStage::Cover)?;
    let metadata = SeriesMetadata {
        titles: BTreeMap::from([("en".into(), input.title)]),
        alt_titles: None,
        description: Some(BTreeMap::from([(
            "en".into(),
            format!("{}\n{}", input.description, input.attribution),
        )])),
        categories: Some(vec!["fixture".into()]),
        credits: Some(vec![Credit {
            role: "author".into(),
            name: if input.language == "ja" {
                "Hiroshi Seo (Seotch)".into()
            } else {
                "Synthetic fixture creator".into()
            },
        }]),
        original_language: Some(input.language.clone()),
        status: Some("completed".into()),
        content_rating: Some("safe".into()),
        publication_demographic: None,
        year: Some(2009),
        cover: Some(cover.clone()),
    };
    let series_manifest = manga_node::platform_bridge::encode_series(&metadata)?;
    let series = runtime
        .publish_operator(PublicationInput {
            kind: 1002,
            canonical_manifest: series_manifest.clone(),
            originals: BTreeMap::from([(cover.hash.clone(), bytes)]),
            created_at: None,
        })
        .await
        .context(SeedStage::Series)?;
    save_snapshot(
        root,
        &series.event,
        &series_manifest,
        &BTreeMap::from([(cover.hash, input.cover.clone())]),
    )
    .context(SeedStage::SeriesSnapshot)?;
    let mut chapters = Vec::new();
    for item in input.chapters {
        ensure!(
            (1..=2).contains(&item.chapter)
                && item.pages.len() > 0
                && item.pages.len() <= 12
                && item.title.len() <= 256,
            "chapter seed bounds"
        );
        let mut pages = Vec::new();
        let mut originals = BTreeMap::new();
        let mut paths = BTreeMap::new();
        for path in &item.pages {
            let (page, bytes) = image(root, path, &mut budget).context(SeedStage::ChapterImage)?;
            originals.insert(page.hash.clone(), bytes);
            paths.insert(page.hash.clone(), path.clone());
            pages.push(page);
        }
        let manifest = Manifest {
            series: series.event.id.clone(),
            language: input.language.clone(),
            chapter: Some(item.chapter.to_string()),
            volume: Some("1".into()),
            title: Some(item.title),
            pages,
        };
        let canonical = manga_node::platform_bridge::encode_chapter(&manifest)
            .context(SeedStage::ChapterEncoding)?;
        let receipt = runtime
            .publish_operator(PublicationInput {
                kind: 1001,
                canonical_manifest: canonical.clone(),
                originals,
                created_at: None,
            })
            .await
            .context(SeedStage::ChapterPublication)?;
        save_snapshot(root, &receipt.event, &canonical, &paths)
            .context(SeedStage::ChapterSnapshot)?;
        chapters.push(json!({"event_id":receipt.event.id,"chapter":item.chapter,"canonical_kind":1001,"accepted_hosted":true,"public_visibility_observed":receipt.locally_visible}));
    }
    // No listener/backend worker is started here; the volume owner is released
    // before the real platform opens the single embedded original runtime.
    drop(runtime);
    let receipt = json!({"fixture_only":true,"series":{"event_id":series.event.id,"canonical_kind":1002,"accepted_hosted":true,"public_visibility_observed":series.locally_visible},"chapters":chapters,"input_bytes":budget,"projection_pending_is_not_success":true,"store_owner_released":true});
    write_json(&root.join("protocol-seed.json"), &receipt)?;
    Ok(receipt)
}
fn save_snapshot(
    root: &Path,
    event: &auth::Event,
    manifest: &[u8],
    paths: &BTreeMap<String, PathBuf>,
) -> Result<()> {
    let name = root
        .join("canonical-seed")
        .join(format!("{}.cbor", event.id));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&name)?;
    file.write_all(manifest)?;
    file.sync_all()?;
    write_json(
        &root
            .join("canonical-seed")
            .join(format!("{}.json", event.id)),
        &json!({"event":event,"manifest":name,"originals":paths}),
    )
}
fn recorded_source(root: &Path, value: Option<&str>) -> Result<(auth::Event, Value)> {
    let value = value.context("explicit recorded fixture event required")?;
    id(value)?;
    let seed = bounded_json(&root.join("protocol-seed.json"))?;
    ensure!(
        std::iter::once(&seed["series"])
            .chain(seed["chapters"].as_array().context("seed receipt")?.iter())
            .any(|row| row["event_id"].as_str() == Some(value)),
        "only recorded canonical fixture seeds may be moderated"
    );
    let snapshot = bounded_json(&root.join("canonical-seed").join(format!("{value}.json")))?;
    let event: auth::Event = serde_json::from_value(snapshot["event"].clone())?;
    event.verify().map_err(anyhow::Error::msg)?;
    let setup = bounded_json(&root.join("protocol-setup.json"))?;
    ensure!(
        event.id == value
            && event.pubkey
                == setup["operator_pubkey"]
                    .as_str()
                    .context("fixture operator")?
            && matches!(event.kind, 1001 | 1002),
        "recorded source ownership mismatch"
    );
    Ok((event, snapshot))
}
fn operator_secret(root: &Path, expected: &str) -> Result<Zeroizing<String>> {
    let path = private_path(root, &root.join("fixture-keys/operator.key"))?;
    let metadata = std::fs::symlink_metadata(&path)?;
    ensure!(
        metadata.is_file() && metadata.len() <= 128,
        "fixture key bounds"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            metadata.permissions().mode() & 0o077 == 0,
            "private fixture operator required"
        );
    }
    let secret = Zeroizing::new(std::fs::read_to_string(path)?.trim().to_owned());
    ensure!(
        auth::sign(&secret, 1, vec![], String::new())
            .map_err(anyhow::Error::msg)?
            .pubkey
            == expected,
        "fixture operator identity mismatch"
    );
    Ok(secret)
}
fn listeners_offline(root: &Path) -> Result<()> {
    let stopped = bounded_json(&root.join("listeners-stopped.json"))?;
    ensure!(
        stopped["fixture_only"] == true
            && stopped["verified_stopped"] == true
            && stopped["ports_closed"] == true,
        "joined fixture listeners required"
    );
    // Exclusive original store ownership still independently fences restoration.
    Ok(())
}
/// One real durable original outbox item, not a native upload/publication intent.
/// Only the explicit restore driver may stage it, after joined listener shutdown.
fn restore_qualification_enabled(root: &Path) -> Result<()> {
    let marker = bounded_json(&root.join("fixture-owned.json"))?;
    ensure!(
        marker["fixture_only"] == true && marker["full_restore_qualification"] == true,
        "explicit synthetic restore qualification required"
    );
    listeners_offline(root)
}
async fn restore_pending_note(repo: &Repository, root: &Path) -> Result<Value> {
    restore_qualification_enabled(root)?;
    repo.verify_schema().await?;
    let setup = bounded_json(&root.join("protocol-setup.json"))?;
    let public = setup["operator_pubkey"].as_str().context("fixture operator identity")?;
    let secret = operator_secret(root, public)?;
    repo.set_account_admission(id(public)?, true).await?;
    let permit = repo.account_permit(id(public)?).await?;
    let event = auth::sign(
        &secret, 1, vec![], "Synthetic isolated restore outbox probe".into(),
    ).map_err(anyhow::Error::msg)?;
    ensure!(repo.stage_personal_note(&permit, &event).await?, "restore probe already staged");
    let pending: i64 = repo.pool.get().await?.query_one(
        "SELECT count(*) FROM backend_outbox WHERE event_id=$1 AND action='import' AND completed_at IS NULL",
        &[&id(&event.id)?.as_slice()],
    ).await?.get(0);
    ensure!(pending == 1, "one durable pending original outbox item required");
    let receipt = json!({"fixture_only":true,"event_id":event.id,"kind":1,
        "pending_original_outbox_items":pending,"native_upload_intent_covered":false});
    write_json(&root.join("restore-pending-note.json"), &receipt)?;
    Ok(receipt)
}

async fn source_revoke(repo: &Repository, root: &Path, request: &Request) -> Result<Value> {
    let (event, _) = recorded_source(root, request.event_id.as_deref())?;
    let secret = operator_secret(root, &event.pubkey)?;
    let deletion = auth::sign(
        &secret,
        5,
        vec![vec!["e".into(), event.id.clone()]],
        "Synthetic fixture operator source revocation".into(),
    )
    .map_err(anyhow::Error::msg)?;
    repo.record_deletion_request(&deletion).await?;
    let changed = repo
        .approve_deletion(id(&deletion.id)?, Some(vec![id(&event.id)?]))
        .await?;
    Ok(
        json!({"fixture_only":true,"event_id":event.id,"deletion_request":deletion.id,"operator_approval":true,"changed":changed,"projection_pending_is_not_success":true}),
    )
}
async fn source_restore(repo: &Repository, root: &Path, request: &Request) -> Result<Value> {
    listeners_offline(root)?;
    let (event, snapshot) = recorded_source(root, request.event_id.as_deref())?;
    let path = private_path(
        root,
        Path::new(
            snapshot["manifest"]
                .as_str()
                .context("canonical fixture manifest")?,
        ),
    )?;
    let metadata = std::fs::metadata(&path)?;
    ensure!(
        metadata.len() <= 1_000_000,
        "canonical fixture manifest bound"
    );
    let canonical = std::fs::read(path)?;
    let expected: Value = serde_json::from_str(&event.content)?;
    ensure!(
        expected["manifest"].as_str() == Some(auth::hash(&canonical).as_str()),
        "canonical fixture manifest changed"
    );
    let mut originals = BTreeMap::new();
    let mut budget = 0;
    for (hash, path) in snapshot["originals"]
        .as_object()
        .context("fixture originals")?
    {
        let (page, bytes) = image(
            root,
            Path::new(path.as_str().context("fixture original path")?),
            &mut budget,
        )?;
        ensure!(&page.hash == hash, "canonical fixture original changed");
        originals.insert(hash.clone(), bytes);
    }
    let config: OperatorRuntimeConfig =
        serde_json::from_value(bounded_json(&root.join("protocol-runtime.json"))?)?;
    // Open the exclusive original owner before changing lifecycle. A falsely
    // claimed stopped marker cannot mutate a still-running store's publication.
    let runtime = PublicationRuntime::open(repo.clone(), config).await?;
    let state: String = repo
        .pool
        .get()
        .await?
        .query_one(
            "SELECT state FROM events WHERE id=$1",
            &[&id(&event.id)?.as_slice()],
        )
        .await?
        .get(0);
    if state == "removed" {
        repo.restore_publication(id(&event.id)?).await?;
    } else {
        ensure!(
            state == "candidate",
            "only removed or exact pending restore may be retried"
        );
    }
    let receipt = runtime
        .publish_operator(PublicationInput {
            kind: event.kind,
            canonical_manifest: canonical,
            originals,
            created_at: Some(event.created_at),
        })
        .await?;
    ensure!(
        receipt.event.id == event.id,
        "restoration changed signed source identity"
    );
    drop(runtime);
    Ok(
        json!({"fixture_only":true,"event_id":event.id,"same_signed_identity":true,"accepted_hosted":true,"store_owner_released":true,"projection_pending_is_not_success":true}),
    )
}
async fn source_status(repo: &Repository, root: &Path, request: &Request) -> Result<Value> {
    let (event, _) = recorded_source(root, request.event_id.as_deref())?;
    let state: String = repo
        .pool
        .get()
        .await?
        .query_one(
            "SELECT state FROM events WHERE id=$1",
            &[&id(&event.id)?.as_slice()],
        )
        .await?
        .get(0);
    let visible = repo.catalogue_entry(id(&event.id)?).await?.is_some();
    let pending:bool=repo.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM backend_outbox WHERE event_id=$1 AND completed_at IS NULL)",&[&id(&event.id)?.as_slice()]).await?.get(0);
    Ok(
        json!({"fixture_only":true,"event_id":event.id,"state":state,"visible":visible,"projection_pending":pending,"removed_terminal":state=="removed"&&!pending}),
    )
}
async fn catalogue_rebuild(repo: &Repository, root: &Path) -> Result<Value> {
    listeners_offline(root)?;
    repo.verify_schema().await?;
    let before: String = repo
        .pool
        .get()
        .await?
        .query_one(
            "SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton",
            &[],
        )
        .await?
        .get(0);
    repo.pool
        .get()
        .await?
        .batch_execute("DROP SCHEMA platform CASCADE")
        .await?;
    weebdex_platform::db::PgStore::new(repo.pool.clone())
        .migrate(weebdex_platform::domains::MIGRATIONS)
        .await?;
    let after: String = repo
        .pool
        .get()
        .await?
        .query_one(
            "SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton",
            &[],
        )
        .await?
        .get(0);
    ensure!(
        before != after,
        "real platform migration must establish a new catalogue namespace"
    );
    Ok(
        json!({"fixture_only":true,"old_catalog_namespace":before,"catalog_namespace":after,"original_schema_preserved":true,"migration_generated_namespace":true,"namespace_uuid_updated_by_fixture":false}),
    )
}
async fn recipient_setup(repo: &Repository, root: &Path, request: &Request) -> Result<Value> {
    let origin = loopback_origin(request.recipient_origin.as_deref())?;
    let primary = bounded_json(&root.join("protocol-setup.json"))?;
    ensure!(
        Some(origin.as_str()) != primary["protocol_origin"].as_str(),
        "distinct fixture recipient required"
    );
    repo.migrate().await?;
    repo.verify_schema().await?;
    for name in [
        "recipient-keys",
        "recipient-storage",
        "recipient-backend",
        "recipient-group-keys",
    ] {
        let path = root.join(name);
        std::fs::create_dir(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    let operator = synthetic_key(&root.join("recipient-keys/operator.key"))?;
    let service = synthetic_key(&root.join("recipient-keys/service.key"))?;
    ensure!(
        operator != service && Some(service.as_str()) != primary["service_key"].as_str(),
        "distinct recipient synthetic identities required"
    );
    repo.set_publisher_admission(id(&operator)?, true).await?;
    let runtime = bounded_json(&root.join("protocol-runtime.json"))?;
    let mut node = runtime["native_node"].clone();
    node["base_url"] = json!(origin);
    node["bind"] = json!(format!(
        "127.0.0.1:{}",
        url::Url::parse(&origin)?.port().context("recipient port")?
    ));
    node["service_key_file"] = json!(root.join("recipient-keys/service.key"));
    node["admin_pubkey"] = json!(operator);
    node["group_keys_dir"] = json!(root.join("recipient-group-keys"));
    node["storage"]["path"] = json!(root.join("recipient-storage"));
    node["storage"]["volume"] = json!(RECIPIENT_VOLUME);
    node["backend"]["data_dir"] = json!(root.join("recipient-backend"));
    let _: manga_node::node::NodeConfig = serde_json::from_value(node.clone())?;
    write_json(&root.join("recipient-node.json"), &node)?;
    let value = json!({"fixture_only":true,"origin":origin,"relay_origin":origin.replacen("http://","ws://",1),"service_key":service,"operator_pubkey":operator,"original_migrations_verified":true});
    write_json(&root.join("recipient-setup.json"), &value)?;
    Ok(value)
}
async fn recipient_serve(repo: &Repository, root: &Path) -> Result<Value> {
    let node: manga_node::node::NodeConfig =
        serde_json::from_value(bounded_json(&root.join("recipient-node.json"))?)?;
    let setup = bounded_json(&root.join("recipient-setup.json"))?;
    ensure!(
        node.bind.ip().is_loopback() && Some(node.base_url.as_str()) == setup["origin"].as_str(),
        "recipient fixture listener mismatch"
    );
    let storage: manga_node::upload::StorageConfig = serde_json::from_value(
        bounded_json(&root.join("recipient-node.json"))?["storage"].clone(),
    )?;
    let store = manga_node::upload::UploadStore::open(
        repo.clone(),
        storage,
        id(&node.admin_pubkey)?,
        id(setup["service_key"].as_str().context("recipient holder")?)?,
    )
    .await?;
    let serving = manga_node::serving_role::ServingRole::prepare(&node).await?;
    let (ready_send, ready_recv) = tokio::sync::oneshot::channel();
    let (shutdown_send, shutdown_recv) = tokio::sync::oneshot::channel();
    let repository = repo.clone();
    let mut task = tokio::spawn(async move {
        manga_node::node::serve_prepared_with_store(
            node,
            repository,
            serving,
            Some(store),
            Some(ready_send),
            Some(shutdown_recv),
        )
        .await
    });
    if !matches!(
        tokio::time::timeout(Duration::from_secs(30), ready_recv).await,
        Ok(Ok(()))
    ) {
        task.abort();
        let _ = task.await;
        anyhow::bail!("recipient startup deadline");
    }
    println!(
        "{}",
        json!({"fixture_only":true,"recipient_listener_bound":true})
    );
    let unexpected = tokio::select! {_=&mut task=>true,_=shutdown_signal()=>false};
    if !unexpected {
        let _ = shutdown_send.send(());
        match tokio::time::timeout(Duration::from_secs(210), &mut task).await {
            Ok(result) => result??,
            Err(_) => {
                task.abort();
                let _ = task.await;
                anyhow::bail!("recipient drain incomplete");
            }
        }
    }
    ensure!(!unexpected, "recipient listener exited unexpectedly");
    Ok(json!({"fixture_only":true,"recipient_stopped_after_drain":true}))
}
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        if let Ok(mut term) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! {_ =tokio::signal::ctrl_c()=>{},_ =term.recv()=>{}};
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
async fn peer_policy(repo: &Repository, root: &Path, request: &Request) -> Result<Value> {
    let allowed = request
        .allowed
        .context("explicit fixture peer decision required")?;
    let primary = bounded_json(&root.join("protocol-setup.json"))?;
    let recipient = bounded_json(&root.join("recipient-setup.json"))?;
    let remote = recipient["origin"].as_str().context("recorded recipient")?;
    loopback_origin(Some(remote))?;
    repo.set_peer(
        id(recipient["service_key"]
            .as_str()
            .context("recorded recipient key")?)?,
        remote,
        allowed,
    )
    .await?;
    let port = bounded_json(&root.join("fixture-owned.json"))?["pg_port"]
        .as_u64()
        .context("owned PG port")?;
    let secondary = Repository::connect(DbConfig {
        dsn: format!("host=127.0.0.1 port={port} dbname=weebdex_browser_recipient_test"),
        connections: 4,
        acquisition_timeout: Duration::from_millis(500),
        cache_weight: 1024 * 1024,
        plaintext_local: true,
    })
    .await?;
    secondary
        .set_peer(
            id(primary["service_key"]
                .as_str()
                .context("recorded primary key")?)?,
            primary["protocol_origin"]
                .as_str()
                .context("recorded primary origin")?,
            allowed,
        )
        .await?;
    secondary.pool.close();
    Ok(
        json!({"fixture_only":true,"mutual_recorded_peer_approval":allowed,"arbitrary_endpoint_received":false}),
    )
}
async fn status(repo: &Repository, root: &Path) -> Result<Value> {
    let seed = bounded_json(&root.join("protocol-seed.json"))?;
    let client = repo.pool.get().await?;
    let namespace: String = client
        .query_one(
            "SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton",
            &[],
        )
        .await?
        .get(0);
    let mut resources = Vec::new();
    let mut all = true;
    for item in std::iter::once(&seed["series"]).chain(
        seed["chapters"]
            .as_array()
            .context("seed chapter receipt")?
            .iter(),
    ) {
        let text = item["event_id"]
            .as_str()
            .context("seed public event identity")?;
        let event = id(text)?;
        let projected = repo.projection_complete(event).await?;
        let visible = repo.catalogue_entry(event).await?.is_some();
        let (table, column) = if item["canonical_kind"] == 1002 {
            ("catalog_manga", "protocol_series_id")
        } else {
            ("catalog_chapters", "protocol_event_id")
        };
        let row = client
            .query_opt(
                &format!("SELECT id FROM platform.{table} WHERE {column}=$1"),
                &[&text],
            )
            .await?;
        let native: Option<String> = row.map(|r| r.get(0));
        let ready = projected && visible && native.is_some();
        all &= ready;
        resources.push(json!({"event_id":text,"native_id":native,"canonical_kind":item["canonical_kind"],"chapter":item.get("chapter"),"visible":visible,"projected":projected,"mapped":ready}));
    }
    let series = resources.remove(0);
    Ok(
        json!({"fixture_only":true,"catalog_namespace":namespace,"series":series,"chapters":resources,"fully_ready":all}),
    )
}

fn upload_fixture_enabled(root: &Path) -> Result<()> {
    ensure!(
        bounded_json(&root.join("fixture-owned.json"))?["public_upload_controls"] == true,
        "explicit public upload fixture opt-in required"
    );
    Ok(())
}

/// These are existing original operator/group APIs, not another storage owner.
/// The operator secret and its fresh proof never leave this fixture process.
async fn upload_operator_post(root: &Path, path: &str, payload: Value) -> Result<Value> {
    let setup = bounded_json(&root.join("protocol-setup.json"))?;
    let origin = loopback_origin(setup["protocol_origin"].as_str())?;
    let operator = setup["operator_pubkey"].as_str().context("fixture operator")?;
    let secret = operator_secret(root, operator)?;
    let url = format!("{origin}{path}");
    let body = serde_json::to_vec(&payload)?;
    let proof = auth::sign(&secret, 27235, vec![
        vec!["u".into(), url.clone()], vec!["method".into(), "POST".into()],
        vec!["payload".into(), auth::hash(&body)],
        vec!["nonce".into(), hex::encode(rand::random::<[u8; 16]>())],
    ], String::new()).map_err(anyhow::Error::msg)?;
    let authorization = Zeroizing::new(format!("Nostr {}", STANDARD.encode(serde_json::to_vec(&proof)?)));
    let client = reqwest::Client::builder().no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(2)).timeout(Duration::from_secs(5)).build()?;
    let mut response = client.post(url).header("Authorization", authorization.as_str())
        .header("Content-Type", "application/json").body(body).send().await?;
    ensure!(response.status() == reqwest::StatusCode::OK, "original group provision rejected");
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(bytes.len() + chunk.len() <= 4096, "original group receipt bounds");
        bytes.extend_from_slice(&chunk);
    }
    Ok(serde_json::from_slice(&bytes)?)
}

async fn upload_account(repo: &Repository, root: &Path, request: &Request) -> Result<Value> {
    upload_fixture_enabled(root)?;
    repo.verify_schema().await?;
    let public = request.pubkey.as_deref().context("synthetic browser public key required")?;
    id(public)?;
    let setup = bounded_json(&root.join("protocol-setup.json"))?;
    ensure!(Some(public) != setup["operator_pubkey"].as_str()
        && Some(public) != setup["service_key"].as_str(), "browser must use its own synthetic identity");
    let descriptor_path = root.join("public-upload.json");
    if descriptor_path.exists() {
        let receipt = bounded_json(&descriptor_path)?;
        ensure!(receipt["scenario"] == UPLOAD_SCENARIO && receipt["pubkey"] == public,
            "recorded upload actor cannot be rebound");
        let group = id(receipt["group"]["pubkey"].as_str().context("recorded upload group")?)?;
        let client = repo.pool.get().await?;
        let valid: bool = client.query_one("SELECT EXISTS(SELECT 1 FROM platform.users u JOIN platform.protocol_identity_bindings b ON b.user_id=u.id JOIN platform.catalog_group_members m ON m.user_id=u.id JOIN platform.protocol_identity_bindings g ON g.group_id=m.group_id WHERE u.id=$1 AND b.publisher=$2 AND u.verified AND NOT u.disabled AND u.profile->>'fixture_scenario'=$3 AND u.permissions='[\"chapter.create\",\"self.chapter.update\"]'::jsonb AND m.group_id=$4 AND g.publisher=$5)", &[&UPLOAD_USER,&public,&UPLOAD_SCENARIO,&UPLOAD_GROUP,&hex::encode(group)]).await?.get(0);
        ensure!(valid, "recorded upload native authority changed");
        let valid: bool = client.query_one("SELECT EXISTS(SELECT 1 FROM managed_groups g JOIN group_members m ON m.group_key=g.group_key WHERE g.group_key=$1 AND g.enabled AND m.member=$2 AND m.role='publisher')", &[&group.as_slice(),&id(public)?.as_slice()]).await?.get(0);
        ensure!(valid, "recorded original upload authority changed");
        repo.account_permit(id(public)?).await?;
        return Ok(receipt);
    }
    {
        let client = repo.pool.get().await?;
        let conflicts: bool = client.query_one("SELECT EXISTS(SELECT 1 FROM platform.users WHERE id=$1) OR EXISTS(SELECT 1 FROM platform.catalog_groups WHERE id=$2) OR EXISTS(SELECT 1 FROM platform.protocol_identity_bindings WHERE publisher=$3 OR user_id=$1 OR group_id=$2)", &[&UPLOAD_USER,&UPLOAD_GROUP,&public]).await?.get(0);
        ensure!(!conflicts, "fresh fixed upload fixture identities required");
    }
    // Persist only the public group identity so an interrupted provision does
    // not create another group on a same-fixture retry. Never read its key file.
    let group_path = root.join("public-upload-group.json");
    let group = if group_path.exists() {
        let value = bounded_json(&group_path)?;
        ensure!(value["scenario"] == UPLOAD_SCENARIO && value["member"] == public,
            "recorded upload group and browser actor cannot be rebound");
        value["pubkey"].as_str().context("recorded upload group")?.to_owned()
    } else {
        let value = upload_operator_post(root, "/admin/groups", json!({"administrator":setup["operator_pubkey"]})).await?;
        let group = value["group"].as_str().context("original managed group receipt")?.to_owned();
        id(&group)?;
        ensure!(value["private_key_returned"] == false && value["custody"] == "instance",
            "original managed group custody required");
        write_json(&group_path, &json!({"fixture_only":true,"scenario":UPLOAD_SCENARIO,"pubkey":group,"member":public}))?;
        group
    };
    id(&group)?;
    let member = upload_operator_post(root, &format!("/groups/{group}/members"),
        json!({"member":public,"role":"publisher"})).await?;
    ensure!(member["updated"] == true && member["group"] == group, "original group member receipt required");
    // Admission alone never confers native grants or original publisher status.
    repo.set_account_admission(id(public)?, true).await?;
    let mut client = repo.pool.get().await?;
    let tx = client.transaction().await?;
    let seeded = bounded_json(&root.join("protocol-seed.json"))?;
    let event = seeded["series"]["event_id"].as_str().context("recorded canonical series")?;
    let manga: String = tx.query_one("SELECT id FROM platform.catalog_manga WHERE protocol_series_id=$1 AND state='published' AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id)", &[&event]).await?.get(0);
    tx.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,roles,permissions,profile) VALUES($1,'SyntheticUploadPublisher','synthetic-upload@example.invalid','not-a-login-password-hash',TRUE,'[\"user\"]','[\"chapter.create\",\"self.chapter.update\"]',jsonb_build_object('fixture_scenario',$2::text))", &[&UPLOAD_USER,&UPLOAD_SCENARIO]).await?;
    tx.execute("INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name,metadata) VALUES($1,'SeedStaff1','FixtureSeeder','Synthetic signed upload group',jsonb_build_object('fixture_scenario',$2::text))", &[&UPLOAD_GROUP,&UPLOAD_SCENARIO]).await?;
    tx.execute("INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader,is_officer) VALUES($1,$2,'SyntheticUploadPublisher',FALSE,FALSE)", &[&UPLOAD_GROUP,&UPLOAD_USER]).await?;
    tx.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)", &[&public,&UPLOAD_USER]).await?;
    tx.execute("INSERT INTO platform.protocol_identity_bindings(publisher,group_id) VALUES($1,$2)", &[&group,&UPLOAD_GROUP]).await?;
    let mut aliases = BTreeMap::new();
    for (kind, native) in [("user", UPLOAD_USER), ("group", UPLOAD_GROUP), ("manga", manga.as_str())] {
        let alias: String = tx.query_one("SELECT public_id::text FROM platform.public_resource_ids WHERE kind=$1 AND native_id=$2 AND preferred AND principal_type='native'", &[&kind,&native]).await?.get(0);
        aliases.insert(kind, alias);
    }
    let namespace: String = tx.query_one("SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton", &[]).await?.get(0);
    tx.commit().await?;
    let receipt = json!({"fixture_only":true,"scenario":UPLOAD_SCENARIO,"pubkey":public,
        "user":{"native_id":UPLOAD_USER,"id":aliases["user"]},
        "group":{"native_id":UPLOAD_GROUP,"id":aliases["group"],"pubkey":group},
        "manga":{"native_id":manga,"id":aliases["manga"],"event_id":event},
        "catalog_namespace":namespace,"service_key":setup["service_key"],
        "allowed":true,"original_group_role":"publisher","native_permissions":["chapter.create","self.chapter.update"],
        "browser_private_key_received":false,"browser_operator_binding":false,"browser_publisher_admission_granted":false});
    write_json(&descriptor_path, &receipt)?;
    Ok(receipt)
}

fn upload_original_observation(root: &Path, key: &str, expected_bytes: i64) -> Result<Value> {
    use std::io::Read;
    ensure!(auth::valid_hash(key) && (1..=512).contains(&expected_bytes), "small synthetic PNG observation only");
    let path = root.join("uploads/objects").join(&key[..2]).join(key);
    ensure!(path.canonicalize()? == path, "upload original cannot traverse a link");
    let metadata = std::fs::symlink_metadata(&path)?;
    ensure!(metadata.is_file() && !metadata.file_type().is_symlink()
        && metadata.len() == expected_bytes as u64, "synthetic original length changed");
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.take(513).read_to_end(&mut bytes)?;
    ensure!(bytes.len() == expected_bytes as usize && bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "bounded actual synthetic PNG required");
    Ok(json!({"storage_sha256":auth::hash(&bytes),"storage_bytes":bytes.len(),"fresh_original_read":true}))
}

async fn upload_snapshot(repo: &Repository, root: &Path, request: &Request) -> Result<Value> {
    upload_fixture_enabled(root)?;
    let fixture = bounded_json(&root.join("public-upload.json"))?;
    ensure!(fixture["scenario"] == UPLOAD_SCENARIO, "recorded upload fixture required");
    let session = weebdex_platform::public_api::PublicUuid::parse(request.session_id.as_deref().context("exact public session UUID required")?)?;
    let mut client = repo.pool.get().await?;
    let tx = client.transaction().await?;
    let count: i64 = tx.query_one("SELECT count(*) FROM platform.upload_sessions WHERE owner_id=$1", &[&UPLOAD_USER]).await?.get(0);
    ensure!(count <= 2, "two-session synthetic upload observation budget");
    let row = tx.query_one("SELECT s.id,s.state,s.version,s.resume_draft,s.resume_page_order,s.committed_chapter_id,s.commit_hash,s.committed_result FROM platform.upload_sessions s JOIN platform.public_resource_ids a ON a.kind='upload_session' AND a.native_id=s.id AND a.principal_type='native' WHERE a.public_id::text=$1 AND s.owner_id=$2 AND s.manga_id=$3 AND s.groups=$4 FOR SHARE OF s", &[&session.as_str(),&UPLOAD_USER,&fixture["manga"]["native_id"].as_str().context("fixture manga")?,&vec![UPLOAD_GROUP.to_owned()]]).await?;
    let native: String = row.get(0);
    let stored: Vec<String> = row.get(4);
    let current = tx.query("SELECT a.public_id::text,f.id,f.position,f.original_name,o.digest,o.bytes,o.storage_key,o.mime FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id JOIN platform.public_resource_ids a ON a.kind='upload_file' AND a.native_id=f.id AND a.preferred AND a.principal_type='native' WHERE f.session_id=$1 ORDER BY f.position,f.id LIMIT 4 FOR SHARE OF f,o", &[&native]).await?;
    ensure!(current.len() <= 3, "three-file synthetic upload observation budget");
    let mut files = Vec::new();
    for file in &current {
        ensure!(file.get::<_,String>(7) == "image/png", "synthetic PNG observation only");
        let observation = upload_original_observation(root, &file.get::<_,String>(6), file.get(5))?;
        files.push(json!({"id":file.get::<_,String>(0),"position":file.get::<_,i32>(2),
            "original_file_name":file.get::<_,String>(3),"file_hash":file.get::<_,String>(4),"file_size":file.get::<_,i64>(5),
            "storage_sha256":observation["storage_sha256"],"storage_bytes":observation["storage_bytes"],"fresh_original_read":true}));
    }
    let mut native_order = stored.clone();
    if row.get::<_,String>(1) == "open" {
        native_order.retain(|id| current.iter().any(|file|file.get::<_,String>(1) == *id));
        for file in &current { let id: String = file.get(1); if !native_order.contains(&id) { native_order.push(id); } }
    }
    ensure!(native_order.len() <= 3 && stored.len() <= 3, "bounded stored page order required");
    let mut page_order = Vec::new();
    for file in &native_order {
        page_order.push(tx.query_one("SELECT public_id::text FROM platform.public_resource_ids WHERE kind='upload_file' AND native_id=$1 AND preferred AND principal_type='native'", &[&file]).await?.get::<_,String>(0));
    }
    // Preserve the actual autosave evidence independently of the API-like
    // effective order. Normalization must never hide a stale/missing save.
    let mut stored_page_order = Vec::new();
    for file in &stored {
        stored_page_order.push(tx.query_one("SELECT public_id::text FROM platform.public_resource_ids WHERE kind='upload_file' AND native_id=$1 AND preferred AND principal_type='native'", &[&file]).await?.get::<_,String>(0));
    }
    let mut chapter = Value::Null;
    let chapter_native: Option<String> = row.get(5);
    if let Some(chapter_id) = &chapter_native {
        let c = tx.query_one("SELECT a.public_id::text,c.title,c.protocol_event_id,c.is_unavailable FROM platform.catalog_chapters c JOIN platform.public_resource_ids a ON a.kind='chapter' AND a.native_id=c.id AND a.preferred AND a.principal_type='native' WHERE c.id=$1 AND c.uploader_id=$2 AND c.manga_id=$3", &[&chapter_id,&UPLOAD_USER,&fixture["manga"]["native_id"].as_str().context("fixture manga")?]).await?;
        let pages = tx.query("SELECT p.position,o.digest,o.bytes,o.storage_key,o.mime FROM platform.chapter_pages p JOIN platform.media_objects o ON o.id=p.object_id WHERE p.chapter_id=$1 ORDER BY p.position LIMIT 4 FOR SHARE OF p,o", &[&chapter_id]).await?;
        ensure!(pages.len() <= 3, "three-page synthetic chapter observation budget");
        let mut observed = Vec::new();
        for page in pages {
            ensure!(page.get::<_,String>(4) == "image/png", "synthetic chapter PNG required");
            let observation = upload_original_observation(root, &page.get::<_,String>(3), page.get(2))?;
            observed.push(json!({"position":page.get::<_,i32>(0),"file_hash":page.get::<_,String>(1),
                "storage_sha256":observation["storage_sha256"],"storage_bytes":observation["storage_bytes"],"fresh_original_read":true}));
        }
        chapter = json!({"id":c.get::<_,String>(0),"title":c.get::<_,Option<String>>(1),"event_id":c.get::<_,Option<String>>(2),"is_unavailable":c.get::<_,bool>(3),"pages":observed});
    }
    let commands = tx.query("SELECT state,event_id FROM platform.protocol_publication_commands WHERE upload_session_id=$1 AND user_id=$2 AND group_id=$3 AND resource_kind='chapter' LIMIT 2", &[&native,&UPLOAD_USER,&UPLOAD_GROUP]).await?;
    ensure!(commands.len() <= 1, "exact upload command required");
    let mut publication = Value::Null;
    if let Some(command) = commands.first() {
        let event_id: Option<String> = command.get(1);
        let mut original = Value::Null;
        if let Some(event_id) = &event_id {
            if let Some(event) = tx.query_opt("SELECT raw_event,state FROM events WHERE id=$1", &[&id(event_id)?.as_slice()]).await? {
                let signed: auth::Event = serde_json::from_value(event.get(0))?;
                signed.verify().map_err(anyhow::Error::msg)?;
                ensure!(signed.id == *event_id && signed.kind == 1001 && Some(signed.pubkey.as_str()) == fixture["group"]["pubkey"].as_str(), "exact genuine group chapter event required");
                original = json!({"id":signed.id,"kind":signed.kind,"pubkey":signed.pubkey,"verified":true,"state":event.get::<_,String>(1)});
            }
        }
        publication = json!({"state":command.get::<_,String>(0),"event_id":event_id,"original_event":original});
    }
    let committed: Option<Value> = row.get(7);
    let result = json!({"fixture_only":true,"scenario":UPLOAD_SCENARIO,"pubkey":fixture["pubkey"],
        "user":fixture["user"],"group":fixture["group"],"manga":fixture["manga"],
        "session":{"id":session,"state":row.get::<_,String>(1),"version":row.get::<_,i32>(2),
            "draft":row.get::<_,Option<Value>>(3),"page_order":page_order,
            "page_order_source":"effective_stored_plus_current_files","stored_page_order":stored_page_order,"files":files,
            "committed_chapter_id":chapter.get("id"),"commit_hash":row.get::<_,Option<String>>(6),
            "publication_status":committed.as_ref().and_then(|value|value.get("publication_status"))},"publication":publication,"chapter":chapter});
    tx.commit().await?;
    Ok(result)
}
async fn run() -> Result<Value> {
    let arguments: Vec<_> = std::env::args().collect();
    ensure!(
        arguments.len() == 3,
        "usage: browser_fixture_provisioner ACTION REQUEST.json"
    );
    let request: Request = serde_json::from_value(bounded_json(Path::new(&arguments[2]))?)?;
    let root = fixture_root(&request.root)?;
    ensure!(
        private_path(&root, Path::new(&arguments[2]))?.starts_with(&root),
        "request must belong to owned fixture"
    );
    let recipient = arguments[1].starts_with("recipient-");
    let repo = repository(recipient, &root).await?;
    let result = match arguments[1].as_str() {
        "setup" => setup(&repo, &root, &request).await,
        "seed" => seed(&repo, &root, &request).await,
        "account" => {
            repo.verify_schema().await?;
            let public = request
                .pubkey
                .as_deref()
                .context("explicit synthetic browser public key required")?;
            let allowed = request
                .allowed
                .context("explicit fixture admission decision required")?;
            repo.set_account_admission(id(public)?, allowed).await?;
            Ok(
                json!({"fixture_only":true,"pubkey":public,"allowed":allowed,"publisher_permission_granted":false,"private_key_received":false}),
            )
        }
        "status" => status(&repo, &root).await,
        "signed-feature-account" => signed_features::account(&repo, &root, &request).await,
        "signed-feature-snapshot" => signed_features::snapshot(&repo, &root, &request).await,
        "upload-account" => upload_account(&repo, &root, &request).await,
        "upload-snapshot" => upload_snapshot(&repo, &root, &request).await,
        "restore-pending-note" => restore_pending_note(&repo, &root).await,
        "source-revoke" => source_revoke(&repo, &root, &request).await,
        "source-restore" => source_restore(&repo, &root, &request).await,
        "source-status" => source_status(&repo, &root, &request).await,
        "catalogue-rebuild" => catalogue_rebuild(&repo, &root).await,
        "ui-regressions-seed" => ui_regressions::seed(&repo, &root, &request).await,
        "ui-regressions-readers" => ui_regressions::readers(&repo, &root, &request).await,
        "ui-regressions-withdraw" => ui_regressions::withdraw(&repo, &root, &request).await,
        "ui-regressions-status" => ui_regressions::status(&repo, &root).await,
        "recipient-setup" => recipient_setup(&repo, &root, &request).await,
        "recipient-serve" => recipient_serve(&repo, &root).await,
        "peer-policy" => peer_policy(&repo, &root, &request).await,
        "recipient-account" => {
            repo.verify_schema().await?;
            let public = request
                .pubkey
                .as_deref()
                .context("explicit synthetic recipient account public key required")?;
            let allowed = request
                .allowed
                .context("explicit fixture account decision required")?;
            repo.set_account_admission(id(public)?, allowed).await?;
            Ok(
                json!({"fixture_only":true,"pubkey":public,"allowed":allowed,"recipient":true,"publisher_permission_granted":false,"private_key_received":false}),
            )
        }
        _ => anyhow::bail!("unknown disposable fixture action"),
    };
    repo.pool.close();
    result
}
#[tokio::main]
async fn main() {
    match run().await {
        Ok(receipt) => println!("{}", receipt),
        // Do not print arbitrary errors which may contain DB/secret-bearing paths.
        Err(error) => {
            let (stage, reason) = safe_failure(&error);
            eprintln!(
                "Disposable protocol fixture action failed; stage={stage}; reason={reason}; no authority success asserted"
            );
            std::process::exit(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_volumes_match_original_upload_store_grammar() {
        for volume in [SOURCE_VOLUME, RECIPIENT_VOLUME] {
            assert!(!volume.is_empty() && volume.len() <= 64);
            assert!(
                volume
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            );
        }
    }

    #[test]
    fn safe_seed_diagnostics_emit_only_static_stage_and_reason() {
        let invalid = anyhow::anyhow!("invalid volume name").context(SeedStage::Runtime);
        assert_eq!(
            safe_failure(&invalid),
            ("seed-runtime-open", "invalid-volume-name")
        );
        let private = anyhow::anyhow!("private fixture path or token must not be emitted")
            .context(SeedStage::Series);
        assert_eq!(
            safe_failure(&private),
            ("seed-series-publication", "operation-failed")
        );
        let io = anyhow::Error::new(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "private signer path",
        ))
        .context(SeedStage::Runtime);
        assert_eq!(
            safe_failure(&io),
            ("seed-runtime-open", "filesystem-permission-denied")
        );
    }

    #[test]
    fn restore_probe_requires_opt_in_and_joined_listener_marker() {
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("fixture-owned.json");
        std::fs::write(&marker, br#"{"fixture_only":true}"#).unwrap();
        assert!(restore_qualification_enabled(root.path()).is_err());
        std::fs::write(&marker, br#"{"fixture_only":true,"full_restore_qualification":true}"#).unwrap();
        assert!(restore_qualification_enabled(root.path()).is_err());
        let stopped = root.path().join("listeners-stopped.json");
        std::fs::write(&stopped, br#"{"fixture_only":true,"verified_stopped":true,"ports_closed":false}"#).unwrap();
        assert!(restore_qualification_enabled(root.path()).is_err());
        std::fs::write(&stopped, br#"{"fixture_only":true,"verified_stopped":true,"ports_closed":true}"#).unwrap();
        assert!(restore_qualification_enabled(root.path()).is_ok());
        std::fs::write(&marker, br#"{"fixture_only":true,"full_restore_qualification":1}"#).unwrap();
        assert!(restore_qualification_enabled(root.path()).is_err());
    }

    #[test]
    fn upload_controls_require_explicit_fixture_opt_in() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("fixture-owned.json"), b"{\"fixture_only\":true}").unwrap();
        assert!(upload_fixture_enabled(root.path()).is_err());
        std::fs::write(root.path().join("fixture-owned.json"), b"{\"public_upload_controls\":true}").unwrap();
        assert!(upload_fixture_enabled(root.path()).is_ok());
    }

    #[test]
    fn upload_observation_hashes_fresh_bounded_original_bytes() {
        let root = tempfile::tempdir().unwrap();
        let key = "11".repeat(32);
        let path = root.path().join("uploads/objects/11").join(&key);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let first = b"\x89PNG\r\n\x1a\nsynthetic-one";
        let second = b"\x89PNG\r\n\x1a\nsynthetic-two";
        std::fs::write(&path, first).unwrap();
        let a = upload_original_observation(root.path(), &key, first.len() as i64).unwrap();
        assert_eq!(a["storage_sha256"], auth::hash(first));
        std::fs::write(&path, second).unwrap();
        let b = upload_original_observation(root.path(), &key, second.len() as i64).unwrap();
        assert_eq!(b["storage_sha256"], auth::hash(second));
        assert_ne!(a["storage_sha256"], b["storage_sha256"]);
        assert!(upload_original_observation(root.path(), &key, 513).is_err());
        assert!(upload_original_observation(root.path(), "../invalid", first.len() as i64).is_err());
        assert!(upload_original_observation(root.path(), &key, 1).is_err());
    }
}
