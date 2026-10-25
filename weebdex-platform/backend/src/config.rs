use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    time::Duration,
};

/// DSNs are read separately so this configuration can be safely inspected.
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub bind: SocketAddr,
    /// Exact immediate/intermediate proxy IPs; empty preserves transport-only identity.
    pub trusted_proxy_peers: Vec<IpAddr>,
    /// Exact proxy count: XFF has this many addresses, including client first.
    pub trusted_proxy_hops: usize,
    /// Explicit production gate; local fixtures must leave this disabled.
    pub production: bool,
    /// Closed feed failure headers for disposable fixtures only; rejected in production.
    pub fixture_feed_diagnostics: bool,
    /// An explicitly limited protocol reader does not need a publisher signer.
    pub protocol_read_only: bool,
    pub json_body_timeout_seconds: u64,
    pub upload_body_timeout_seconds: u64,
    pub shutdown_grace_seconds: u64,
    pub public_origin: String,
    pub ui_dir: PathBuf,
    pub ui_fallback: String,
    pub database_connections: usize,
    pub database_wait_ms: u64,
    pub database_plaintext_local: bool,
    pub cache_bytes: u64,
    pub maximum_requests: usize,
    pub maximum_json_bytes: usize,
    pub read_admission: Option<manga_node::read_admission::Config>,
    /// Native routes can be served by the retained node on this explicit origin.
    pub protocol_origin: Option<String>,
    pub protocol_origin_self_hosted: bool,
    /// Exact operator-reviewed browser signer/relay origins, never a wildcard.
    pub protocol_connect_origins: Vec<String>,
    pub protocol_publication: Option<crate::protocol::publication::OperatorRuntimeConfig>,
    pub session_cookie_name: String,
    pub session_lifetime_seconds: u64,
    pub registration_enabled: bool,
    pub uploads_dir: PathBuf,
    /// Protective instance scheduling limits, not upstream MangaDex limits.
    pub scheduled_upload_maximum: u32,
    pub scheduled_upload_bytes: i64,
    pub scheduled_upload_horizon_seconds: u32,
    pub scheduled_upload_retry_seconds: u32,
    pub auth_mail: Option<crate::domains::auth::MailConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8080".parse().expect("literal bind address"),
            trusted_proxy_peers: Vec::new(),
            trusted_proxy_hops: 1,
            production: false,
            fixture_feed_diagnostics: false,
            protocol_read_only: false,
            json_body_timeout_seconds: 30,
            upload_body_timeout_seconds: 180,
            shutdown_grace_seconds: 210,
            public_origin: "http://localhost:8080".into(),
            ui_dir: "artifacts/frontend".into(),
            ui_fallback: "index.html".into(),
            database_connections: 8,
            database_wait_ms: 500,
            database_plaintext_local: false,
            cache_bytes: 16 * 1024 * 1024,
            maximum_requests: 32,
            maximum_json_bytes: 1024 * 1024,
            read_admission: None,
            protocol_origin: None,
            protocol_origin_self_hosted: false,
            protocol_connect_origins: Vec::new(),
            protocol_publication: None,
            session_cookie_name: "weebdex_session".into(),
            session_lifetime_seconds: 7 * 24 * 60 * 60,
            registration_enabled: false,
            uploads_dir: "runtime/uploads".into(),
            scheduled_upload_maximum: 32,
            scheduled_upload_bytes: 500 * 1024 * 1024,
            scheduled_upload_horizon_seconds: 30 * 24 * 60 * 60,
            scheduled_upload_retry_seconds: 24 * 60 * 60,
            auth_mail: None,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let mut value: Self = match std::env::var("PLATFORM_CONFIG") {
            Ok(path) => serde_json::from_slice(
                &std::fs::read(path).context("cannot read platform configuration")?,
            )?,
            Err(_) => Self::default(),
        };
        if let Ok(bind) = std::env::var("PLATFORM_BIND") {
            value.bind = bind.parse().context("invalid platform bind")?;
        }
        if let Ok(origin) = std::env::var("PLATFORM_PUBLIC_ORIGIN") {
            value.public_origin = origin;
        }
        if let Ok(path) = std::env::var("PLATFORM_UI_DIR") {
            value.ui_dir = path.into();
        }
        if let Ok(name) = std::env::var("PLATFORM_UI_FALLBACK") {
            value.ui_fallback = name;
        }
        if let Ok(origin) = std::env::var("PLATFORM_PROTOCOL_ORIGIN") {
            value.protocol_origin = Some(origin);
        }
        if let Ok(production) = std::env::var("PLATFORM_PRODUCTION") {
            value.production = production
                .parse()
                .context("PLATFORM_PRODUCTION must be true or false")?;
        }
        if let Ok(path) = std::env::var("PLATFORM_UPLOADS_DIR") {
            value.uploads_dir = path.into();
        }
        value.validate()?;
        Ok(value)
    }

    /// Read-only deployment preflight. Does not connect, migrate, create keys or
    /// expose upload/signer directories through the static file service.
    pub fn validate_runtime_paths(&self) -> Result<()> {
        ensure_no_parent_components(&self.ui_dir)?;
        let ui = self
            .ui_dir
            .canonicalize()
            .context("UI artifact directory is unavailable")?;
        ensure!(ui.is_dir(), "UI artifact path must be a directory");
        // The static service follows filesystem entries. Only a dedicated,
        // immutable artifact tree is safe; reject links before serving it.
        let mut directories = vec![ui.clone()];
        let mut files = 0usize;
        while let Some(directory) = directories.pop() {
            for entry in
                std::fs::read_dir(directory).context("cannot inspect public UI artifacts")?
            {
                let entry = entry?;
                let kind = entry.file_type()?;
                ensure!(
                    !kind.is_symlink() && (kind.is_file() || kind.is_dir()),
                    "public UI artifacts cannot contain links or special files"
                );
                if kind.is_dir() {
                    directories.push(entry.path());
                }
                files += 1;
                ensure!(files <= 100_000, "public UI artifact entry bound");
            }
        }
        let fallback = ui.join(&self.ui_fallback);
        let metadata = std::fs::symlink_metadata(&fallback)
            .context("UI fallback is unavailable; build the integrated frontend")?;
        ensure!(
            metadata.is_file() && metadata.len() <= 2 * 1024 * 1024,
            "UI fallback must be a bounded regular file"
        );
        let html = std::fs::read_to_string(&fallback).context("UI fallback must be UTF-8")?;
        ensure!(
            !crate::privacy::bootstrap_hashes(&html).is_empty(),
            "UI fallback has no recognized SvelteKit bootstrap; integrated artifact required"
        );
        let uploads = resolved_future_path(&self.uploads_dir)?;
        ensure_disjoint(
            &ui,
            &uploads,
            "UI and private upload roots must be disjoint",
        )?;
        if self.uploads_dir.exists() {
            ensure!(uploads.is_dir(), "upload root must be a directory");
        } else {
            ensure!(
                !self.production,
                "production upload directory must be provisioned before startup"
            );
        }
        if let Some(publication) = &self.protocol_publication {
            ensure!(
                publication.operator_key_file.is_absolute() || !self.production,
                "production signer path must be absolute"
            );
            ensure_disjoint(
                &ui,
                &resolved_future_path(&publication.operator_key_file)?,
                "signer and public UI roots must be disjoint",
            )?;
            if let Some(groups) = &publication.group_keys_dir {
                ensure!(
                    groups.is_absolute() || !self.production,
                    "production group-key path must be absolute"
                );
                ensure_disjoint(
                    &ui,
                    &resolved_future_path(groups)?,
                    "publication group keys and public UI roots must be disjoint",
                )?;
            }
            if let Some(storage) = publication
                .storage
                .get("path")
                .and_then(serde_json::Value::as_str)
            {
                let storage = std::path::Path::new(storage);
                ensure!(
                    storage.is_absolute() || !self.production,
                    "production original storage path must be absolute"
                );
                let storage = resolved_future_path(storage)?;
                ensure_disjoint(
                    &ui,
                    &storage,
                    "original storage and public UI roots must be disjoint",
                )?;
                ensure_disjoint(
                    &uploads,
                    &storage,
                    "native and protocol upload roots must be disjoint",
                )?;
            }
            if let Some(value) = &publication.backend {
                let backend: manga_node::backend::BackendConfig =
                    serde_json::from_value(value.clone())?;
                ensure_disjoint(
                    &ui,
                    &resolved_future_path(&backend.data_dir)?,
                    "publication backend data and public UI roots must be disjoint",
                )?;
            }
            if let Some(value) = &publication.native_node {
                let node: manga_node::node::NodeConfig = serde_json::from_value(value.clone())?;
                ensure_disjoint(
                    &ui,
                    &resolved_future_path(&node.service_key_file)?,
                    "original service key and public UI roots must be disjoint",
                )?;
                if let Some(groups) = &node.group_keys_dir {
                    ensure!(
                        groups.is_absolute() || !self.production,
                        "production original group-key path must be absolute"
                    );
                    ensure_disjoint(
                        &ui,
                        &resolved_future_path(groups)?,
                        "original group keys and public UI roots must be disjoint",
                    )?;
                }
                if let Some(storage) = &node.storage {
                    let storage = resolved_future_path(&storage.path)?;
                    ensure_disjoint(
                        &ui,
                        &storage,
                        "original listener storage and public UI roots must be disjoint",
                    )?;
                    ensure_disjoint(
                        &uploads,
                        &storage,
                        "native and original listener upload roots must be disjoint",
                    )?;
                }
                if let Some(backend) = &node.backend {
                    ensure_disjoint(
                        &ui,
                        &resolved_future_path(&backend.data_dir)?,
                        "original backend data and public UI roots must be disjoint",
                    )?;
                }
            }
        }
        Ok(())
    }

    pub fn database_dsn() -> Result<String> {
        std::env::var("PLATFORM_DATABASE_URL")
            .or_else(|_| std::env::var("MANGA_DATABASE_URL"))
            .context("PLATFORM_DATABASE_URL is required")
    }

    pub fn acquisition_timeout(&self) -> Duration {
        Duration::from_millis(self.database_wait_ms)
    }

    pub fn admission_config(&self) -> manga_node::read_admission::Config {
        self.read_admission.clone().unwrap_or_else(|| {
            manga_node::read_admission::Config::for_database(self.database_connections)
        })
    }

    pub fn validate(&self) -> Result<()> {
        ensure!((1..=1000).contains(&self.scheduled_upload_maximum)
            && (1..=536870912000_i64).contains(&self.scheduled_upload_bytes)
            && (1..=31536000).contains(&self.scheduled_upload_horizon_seconds)
            && (1..=86400).contains(&self.scheduled_upload_retry_seconds),
            "Invalid scheduled upload limits");
        ensure!(
            !self.production || !self.fixture_feed_diagnostics,
            "fixture feed diagnostics are forbidden in production"
        );
        ensure!(
            self.trusted_proxy_peers.len() <= 32 && (1..=8).contains(&self.trusted_proxy_hops),
            "trusted proxy address/hop bounds"
        );
        ensure!(
            !self.trusted_proxy_peers.is_empty() || self.trusted_proxy_hops == 1,
            "proxy hop policy requires explicit trusted addresses"
        );
        let mut proxies = std::collections::BTreeSet::new();
        for peer in &self.trusted_proxy_peers {
            let peer = crate::client_peer::normalized(*peer);
            ensure!(
                crate::client_peer::valid_unicast(peer) && proxies.insert(peer),
                "trusted proxy addresses must be unique exact unicast IPs"
            );
        }
        ensure!(
            self.bind.ip().is_loopback(),
            "platform binds loopback; configure a reverse proxy for public access"
        );
        validate_origin(&self.public_origin)?;
        ensure!(
            self.protocol_connect_origins.len() <= 16,
            "at most sixteen reviewed protocol connect origins"
        );
        for origin in &self.protocol_connect_origins {
            ensure!(origin.len() <= 300, "protocol connect origin length");
            let url = url::Url::parse(origin).context("invalid protocol connect origin")?;
            let local = match url.host() {
                Some(url::Host::Domain(host)) => host == "localhost",
                Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
                Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
                None => false,
            };
            ensure!(
                url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none()
                    && url.path() == "/"
                    && (matches!(url.scheme(), "https" | "wss")
                        || !self.production && local && matches!(url.scheme(), "http" | "ws")),
                "protocol connect origin requires exact HTTPS/WSS, or development-only loopback HTTP/WS"
            );
            ensure!(
                !crate::privacy::is_tracking_origin(&url),
                "upstream tracking endpoints cannot be protocol connect origins"
            );
        }
        if let Some(origin) = &self.protocol_origin {
            validate_origin(origin)?;
        }
        ensure!(
            !self.protocol_origin_self_hosted || self.protocol_origin.is_some(),
            "self-hosted protocol redirect requires an explicit origin"
        );
        ensure!(
            (1..=128).contains(&self.database_connections),
            "database pool bounds"
        );
        ensure!(
            self.protocol_publication.is_none() || self.database_connections >= 4,
            "protocol publication requires at least four shared database connections"
        );
        ensure!(
            (1..=30_000).contains(&self.database_wait_ms),
            "database acquisition deadline bounds"
        );
        ensure!(
            (1..=256).contains(&self.maximum_requests),
            "request concurrency bounds"
        );
        ensure!(
            (1024..=16 * 1024 * 1024).contains(&self.maximum_json_bytes),
            "JSON body bounds"
        );
        ensure!(
            (1024..=1024 * 1024 * 1024).contains(&self.cache_bytes),
            "cache bounds"
        );
        ensure!(
            !self.ui_fallback.is_empty()
                && self.ui_fallback.len() <= 128
                && self
                    .ui_fallback
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
                && !self.ui_fallback.starts_with('.'),
            "UI fallback must be a filename"
        );
        ensure!(
            !self.session_cookie_name.is_empty()
                && self.session_cookie_name.len() <= 64
                && self
                    .session_cookie_name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')),
            "invalid session cookie name"
        );
        ensure!(
            (60..=31_536_000).contains(&self.session_lifetime_seconds),
            "session lifetime bounds"
        );
        ensure!(
            (1..=120).contains(&self.json_body_timeout_seconds),
            "JSON body deadline bounds"
        );
        ensure!(
            (10..=600).contains(&self.upload_body_timeout_seconds),
            "upload body deadline bounds"
        );
        ensure!(
            (10..=900).contains(&self.shutdown_grace_seconds),
            "shutdown grace bounds"
        );
        ensure!(
            !self.protocol_read_only || self.protocol_publication.is_none(),
            "read-only protocol mode cannot configure publication ownership"
        );
        if self.production {
            ensure!(
                url::Url::parse(&self.public_origin)?.scheme() == "https",
                "production public origin requires HTTPS"
            );
            ensure!(
                !self.database_plaintext_local,
                "production database requires certificate-verified TLS"
            );
            ensure!(
                self.ui_dir.is_absolute() && self.uploads_dir.is_absolute(),
                "production UI and upload directories must be absolute"
            );
            ensure!(
                self.auth_mail
                    .as_ref()
                    .is_some_and(|mail| !mail.allow_loopback_test_sink),
                "production account verification/recovery requires a real configured mail provider"
            );
            if self.protocol_read_only {
                ensure!(
                    self.protocol_origin_self_hosted && self.protocol_origin.is_some(),
                    "read-only production requires a self-hosted original-serving origin"
                );
                ensure!(
                    self.protocol_origin
                        .as_ref()
                        .is_some_and(|origin| url::Url::parse(origin).is_ok_and(|origin| origin
                            .origin()
                            == url::Url::parse(&self.public_origin)
                                .expect("validated public origin")
                                .origin())),
                    "read-only originals must be reverse-proxied on the public UI origin for its same-origin media policy"
                );
            } else {
                ensure!(
                    self.protocol_publication.is_some(),
                    "full production requires an operator-provisioned protocol publication runtime; select protocol_read_only for an explicitly limited reader"
                );
            }
            ensure!(
                self.shutdown_grace_seconds >= self.upload_body_timeout_seconds + 10,
                "production drain must cover the upload body deadline plus transaction grace"
            );
        }
        self.admission_config()
            .validate(self.database_connections)?;
        if let Some(mail) = &self.auth_mail {
            mail.validate()?;
        }
        if let Some(publication) = &self.protocol_publication {
            ensure!(
                publication.backend.is_none(),
                "publication.backend is retired; configure the supervised native_node.backend instead"
            );
            if let Some(value) = &publication.native_node {
                let node: manga_node::node::NodeConfig = serde_json::from_value(value.clone())
                    .context("invalid embedded original node configuration")?;
                ensure!(
                    node.bind.ip().is_loopback() && node.bind != self.bind,
                    "embedded original listener must use a distinct loopback socket"
                );
                validate_origin(&node.base_url)?;
                ensure!(
                    node.admin_pubkey == publication.storage_owner,
                    "embedded original administrator must match storage owner"
                );
                if self.production {
                    ensure!(
                        matches!(node.read_policy, manga_node::relay::ReadPolicy::Public),
                        "native browser integration requires public counterpart reads; signed private-read transport is not configured"
                    );
                    ensure!(
                        url::Url::parse(&node.base_url)?.origin()
                            == url::Url::parse(&self.public_origin)?.origin()
                            || node
                                .cors_origins
                                .iter()
                                .any(|origin| url::Url::parse(origin).ok().is_some_and(|origin| {
                                    origin.origin()
                                        == url::Url::parse(&self.public_origin)
                                            .expect("validated public origin")
                                            .origin()
                                })),
                        "cross-origin native browser transport must explicitly allow the platform public origin in original CORS"
                    );
                    ensure!(
                        url::Url::parse(&node.base_url)?.scheme() == "https",
                        "production original protocol origin requires HTTPS"
                    );
                    ensure!(
                        node.service_key_file.is_absolute(),
                        "production original service-key path must be absolute"
                    );
                    ensure!(
                        node.backend.is_some(),
                        "full production requires the original projection backend"
                    );
                    ensure!(
                        !node.cors_origins.iter().any(|origin| origin == "*"),
                        "production original node CORS must be explicitly bounded"
                    );
                    ensure!(
                        !node.database_plaintext_local,
                        "production original node configuration must retain TLS policy"
                    );
                    if let Some(backend) = &node.backend {
                        ensure!(
                            backend.binary.is_absolute() && backend.data_dir.is_absolute(),
                            "production original backend paths must be absolute"
                        );
                    }
                }
            } else {
                ensure!(
                    !self.production,
                    "full production requires the supervised original protocol listener"
                );
            }
        }
        if let Some(publication) = &self.protocol_publication {
            ensure!(
                [
                    &publication.expected_publisher,
                    &publication.storage_owner,
                    &publication.storage_holder
                ]
                .iter()
                .all(|value| manga_node::auth::valid_hash(value)),
                "protocol publication requires explicit valid public identities"
            );
            ensure!(
                !publication.operator_key_file.as_os_str().is_empty(),
                "protocol signer file path is required"
            );
            let _: manga_node::upload::StorageConfig =
                serde_json::from_value(publication.storage.clone())
                    .context("invalid original storage configuration")?;
            if let Some(backend) = &publication.backend {
                let _: manga_node::backend::BackendConfig = serde_json::from_value(backend.clone())
                    .context("invalid original backend configuration")?;
            }
        }
        Ok(())
    }
}

fn ensure_no_parent_components(path: &std::path::Path) -> Result<()> {
    ensure!(
        !path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir)),
        "configured runtime/UI roots must not contain parent-directory traversal; use an explicit normalized path"
    );
    Ok(())
}

/// Resolve existing symlink ancestry even when the final directory has not yet
/// been provisioned. A broken/inaccessible ancestor is an error, never a reason
/// to skip the public/private boundary check.
fn resolved_future_path(path: &std::path::Path) -> Result<PathBuf> {
    ensure_no_parent_components(path)?;
    ensure!(
        !path.as_os_str().is_empty(),
        "private runtime path must not be empty"
    );
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut ancestor = absolute.clone();
    let mut suffix = Vec::new();
    loop {
        match std::fs::symlink_metadata(&ancestor) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let component = ancestor
                    .components()
                    .next_back()
                    .context("runtime path has no existing ancestor")?;
                suffix.push(component.as_os_str().to_owned());
                ensure!(ancestor.pop(), "runtime path has no existing ancestor");
            }
            Err(error) => return Err(error).context("runtime path ancestry is unavailable"),
        }
    }
    let mut resolved = ancestor
        .canonicalize()
        .context("runtime path ancestor cannot be resolved")?;
    for component in suffix.into_iter().rev() {
        if component == std::ffi::OsStr::new("..") {
            ensure!(resolved.pop(), "runtime path escapes filesystem root");
        } else if component != std::ffi::OsStr::new(".") {
            resolved.push(component);
        }
    }
    Ok(resolved)
}
fn ensure_disjoint(
    public: &std::path::Path,
    private: &std::path::Path,
    message: &str,
) -> Result<()> {
    ensure!(
        !private.starts_with(public) && !public.starts_with(private),
        "{message}"
    );
    Ok(())
}

fn validate_origin(text: &str) -> Result<()> {
    let url = url::Url::parse(text).context("invalid public origin")?;
    let local = match url.host() {
        Some(url::Host::Domain(host)) => host == "localhost",
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    };
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url.path() == "/"
            && (url.scheme() == "https" || url.scheme() == "http" && local),
        "public origin requires HTTPS or explicit loopback HTTP and no suffix"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scheduling_retry_limit_is_protective_and_defaults_to_one_day() {
        assert_eq!(Config::default().scheduled_upload_retry_seconds,86400);
        for seconds in [1,2,86400] {assert!(Config{scheduled_upload_retry_seconds:seconds,..Config::default()}.validate().is_ok());}
        for seconds in [0,86401,u32::MAX] {assert!(Config{scheduled_upload_retry_seconds:seconds,..Config::default()}.validate().is_err());}
    }


    fn runtime_fixture() -> (tempfile::TempDir, Config) {
        let root = tempfile::tempdir().unwrap();
        let ui = root.path().join("ui");
        std::fs::create_dir(&ui).unwrap();
        std::fs::write(
            ui.join("index.html"),
            "<script>const __sveltekit_test={}; import('./start.js').then(x=>x.start());</script>",
        )
        .unwrap();
        let mut config = Config::default();
        config.ui_dir = ui;
        config.uploads_dir = root.path().join("private-uploads");
        config.protocol_publication = Some(crate::protocol::publication::OperatorRuntimeConfig {
            operator_key_file: root.path().join("private/operator.key"),
            expected_publisher: "11".repeat(32),
            storage_owner: "22".repeat(32),
            storage_holder: "33".repeat(32),
            storage: serde_json::json!({"path":root.path().join("originals")}),
            group_keys_dir: Some(root.path().join("groups")),
            backend: None,
            native_node: None,
        });
        (root, config)
    }
    #[test]
    fn fixture_feed_diagnostics_are_opt_in_and_forbidden_in_production() {
        let mut config: Config = serde_json::from_str("{}").unwrap();
        assert!(!config.fixture_feed_diagnostics);
        config.fixture_feed_diagnostics = true;
        assert!(config.validate().is_ok());
        config.production = true;
        assert!(
            config
                .validate()
                .unwrap_err()
                .to_string()
                .contains("fixture feed diagnostics are forbidden in production")
        );
    }

    #[test]
    fn standalone_publication_backend_is_rejected_even_in_development() {
        let (_root, mut config) = runtime_fixture();
        config.protocol_publication.as_mut().unwrap().backend = Some(serde_json::json!({}));
        assert!(!config.production);
        assert!(
            config
                .validate()
                .unwrap_err()
                .to_string()
                .contains("publication.backend is retired")
        );
    }
    #[test]
    fn runtime_preflight_rejects_both_uncreated_group_key_roots_inside_public_ui() {
        let (root, mut config) = runtime_fixture();
        assert!(config.validate_runtime_paths().is_ok());
        config.protocol_publication.as_mut().unwrap().group_keys_dir =
            Some(config.ui_dir.join("not-created/groups"));
        assert!(config.validate_runtime_paths().is_err());
        config.protocol_publication.as_mut().unwrap().group_keys_dir =
            Some(root.path().join("private-groups"));
        config.protocol_publication.as_mut().unwrap().native_node = Some(serde_json::json!({
            "bind":"127.0.0.1:29999","base_url":"http://127.0.0.1:29999",
            "service_key_file":root.path().join("native-service.key"),"admin_pubkey":"22".repeat(32),
            "group_keys_dir":config.ui_dir.join("future-native-groups")
        }));
        assert!(config.validate_runtime_paths().is_err());
    }
    #[test]
    fn runtime_preflight_rejects_equal_ancestor_and_lexical_future_private_roots() {
        let (root, mut config) = runtime_fixture();
        for uploads in [
            config.ui_dir.clone(),
            root.path().to_owned(),
            config.ui_dir.join("future/../uploads"),
            root.path().join("missing/../private-looking-link/future"),
        ] {
            config.uploads_dir = uploads;
            assert!(config.validate_runtime_paths().is_err());
        }
        config.uploads_dir = root.path().join("uploads");
        config
            .protocol_publication
            .as_mut()
            .unwrap()
            .operator_key_file = config.ui_dir.join("future.key");
        assert!(config.validate_runtime_paths().is_err());
    }
    #[cfg(unix)]
    #[test]
    fn runtime_preflight_resolves_symlink_ancestry_before_uncreated_private_suffixes() {
        let (root, mut config) = runtime_fixture();
        let link = root.path().join("private-looking-link");
        std::os::unix::fs::symlink(&config.ui_dir, &link).unwrap();
        config.protocol_publication.as_mut().unwrap().group_keys_dir =
            Some(link.join("uncreated/groups"));
        assert!(config.validate_runtime_paths().is_err());
        config.protocol_publication.as_mut().unwrap().group_keys_dir =
            Some(root.path().join("missing/../private-looking-link/future"));
        assert!(config.validate_runtime_paths().is_err());
    }
    #[test]
    fn trusted_proxy_policy_rejects_ambiguous_unbounded_and_nonunicast_addresses() {
        let mut value = Config::default();
        assert!(value.trusted_proxy_peers.is_empty() && value.trusted_proxy_hops == 1);
        value.trusted_proxy_hops = 2;
        assert!(value.validate().is_err());
        value.trusted_proxy_peers = vec!["127.0.0.1".parse().unwrap()];
        assert!(value.validate().is_ok());
        value.trusted_proxy_hops = 0;
        assert!(value.validate().is_err());
        value.trusted_proxy_hops = 9;
        assert!(value.validate().is_err());
        value.trusted_proxy_hops = 1;
        for peers in [
            vec!["0.0.0.0"],
            vec!["224.0.0.1"],
            vec!["255.255.255.255"],
            vec!["::"],
            vec!["ff02::1"],
            vec!["127.0.0.1", "::ffff:127.0.0.1"],
        ] {
            value.trusted_proxy_peers = peers.iter().map(|peer| peer.parse().unwrap()).collect();
            assert!(value.validate().is_err());
        }
        value.trusted_proxy_peers = (1..=33)
            .map(|last| std::net::IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, last)))
            .collect();
        assert!(value.validate().is_err());
    }

    #[test]
    fn public_origins_exclude_credentials_queries_and_insecure_remote_hosts() {
        for text in [
            "https://user:password@example.com",
            "https://example.com?token=value",
            "https://example.com/path",
            "http://example.com",
            "file:///tmp/file",
        ] {
            assert!(validate_origin(text).is_err());
        }
        for text in [
            "https://example.com",
            "http://localhost:8080",
            "http://127.0.0.1:8080",
            "http://[::1]:8080",
        ] {
            assert!(validate_origin(text).is_ok());
        }
    }

    #[test]
    fn configuration_cannot_escape_static_fallback_or_disable_resource_limits() {
        let mut value = Config::default();
        assert!(value.validate().is_ok());
        value.ui_fallback = "../secret".into();
        assert!(value.validate().is_err());
        value.ui_fallback = "index.html".into();
        value.maximum_requests = 0;
        assert!(value.validate().is_err());
        value.maximum_requests = 32;
        value.database_connections = 0;
        assert!(value.validate().is_err());
    }
    #[test]
    fn production_cannot_silently_use_fixture_or_unconfigured_publication() {
        let mut value = Config::default();
        value.production = true;
        assert!(value.validate().is_err());
        value.public_origin = "https://instance.example.invalid".into();
        value.ui_dir = "/opt/weebdex/ui".into();
        value.uploads_dir = "/var/lib/weebdex/uploads".into();
        value.auth_mail = Some(crate::domains::auth::MailConfig {
            endpoint: "https://mail.example.invalid/send".into(),
            from: "noreply@example.invalid".into(),
            bearer_token_env: None,
            allow_loopback_test_sink: false,
        });
        assert!(value.validate().is_err());
        value.protocol_read_only = true;
        value.protocol_origin = Some(value.public_origin.clone());
        value.protocol_origin_self_hosted = true;
        assert!(value.validate().is_ok());
        value.protocol_origin = Some("https://other.example.invalid".into());
        assert!(value.validate().is_err());
        value.protocol_origin = Some(value.public_origin.clone());
        value.database_plaintext_local = true;
        assert!(value.validate().is_err());
    }
}
