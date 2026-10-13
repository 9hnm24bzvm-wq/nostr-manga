use crate::auth::Event;
use crate::read_diagnostics::{Phase, during, phase};
use anyhow::{Context, Result, anyhow, ensure};
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod, Runtime, Timeouts};
use moka::future::Cache;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{net::IpAddr, sync::Arc, time::Duration};
use tokio_postgres::{
    NoTls,
    config::{Host, SslMode},
};
use tokio_postgres_rustls::MakeRustlsConnect;

pub type Id = [u8; 32];
/// Database failures are operational errors, never evidence of a denied role.
pub fn is_database_error(error: &anyhow::Error) -> bool {
    error.is::<deadpool_postgres::PoolError>() || error.is::<tokio_postgres::Error>()
}
const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../migrations/0001_foundation.sql")),
    (2, include_str!("../migrations/0002_capacity.sql")),
    (3, include_str!("../migrations/0003_node_identity.sql")),
    (4, include_str!("../migrations/0004_blob_intents.sql")),
    (
        5,
        include_str!("../migrations/0005_projection_generation.sql"),
    ),
    (
        6,
        include_str!("../migrations/0006_publication_references.sql"),
    ),
    (
        7,
        include_str!("../migrations/0007_protected_moderation.sql"),
    ),
    (8, include_str!("../migrations/0008_approved_peers.sql")),
    (
        9,
        include_str!("../migrations/0009_inventory_snapshots.sql"),
    ),
    (10, include_str!("../migrations/0010_durable_sync.sql")),
    (11, include_str!("../migrations/0011_group_authority.sql")),
    (12, include_str!("../migrations/0012_account_heads.sql")),
    (13, include_str!("../migrations/0013_library_staging.sql")),
    (14, include_str!("../migrations/0014_blob_quarantine.sql")),
    (15, include_str!("../migrations/0015_retention_credits.sql")),
    (16, include_str!("../migrations/0016_library_holds.sql")),
    (17, include_str!("../migrations/0017_credit_lanes.sql")),
    (
        18,
        include_str!("../migrations/0018_unreleased_credits.sql"),
    ),
    (
        19,
        include_str!("../migrations/0019_library_reclamation.sql"),
    ),
    (20, include_str!("../migrations/0020_library_history.sql")),
    (21, include_str!("../migrations/0021_social_events.sql")),
    (22, include_str!("../migrations/0022_ordinary_deletion.sql")),
    (23, include_str!("../migrations/0023_storage_audit.sql")),
    (
        24,
        include_str!("../migrations/0024_publication_reclamation.sql"),
    ),
    (25, include_str!("../migrations/0025_restore_fence.sql")),
    (26, include_str!("../migrations/0026_guarded_relay.sql")),
    (27, include_str!("../migrations/0027_inventory_scope.sql")),
    (
        28,
        include_str!("../migrations/0028_account_inventories.sql"),
    ),
    (29, include_str!("../migrations/0029_library_inventory.sql")),
    (30, include_str!("../migrations/0030_social_inventory.sql")),
    (31, include_str!("../migrations/0031_sync_schedules.sql")),
    (
        32,
        include_str!("../migrations/0032_public_export_holds.sql"),
    ),
    (
        33,
        include_str!("../migrations/0033_public_export_volume_binding.sql"),
    ),
    (
        34,
        include_str!("../migrations/0034_public_export_queue.sql"),
    ),
    (
        35,
        include_str!("../migrations/0035_public_export_health_work.sql"),
    ),
    (
        36,
        include_str!("../migrations/0036_publisher_staging_grants.sql"),
    ),
    (
        37,
        include_str!("../migrations/0037_staging_expiry_queue.sql"),
    ),
    (
        38,
        include_str!("../migrations/0038_group_staging_leases.sql"),
    ),
    (
        39,
        include_str!("../migrations/0039_staging_operation_holds.sql"),
    ),
    (
        40,
        include_str!("../migrations/0040_owner_upload_interests.sql"),
    ),
    (
        41,
        include_str!("../migrations/0041_sync_item_rejections.sql"),
    ),
    (
        42,
        include_str!("../migrations/0042_remembered_service_identities.sql"),
    ),
    (
        43,
        include_str!("../migrations/0043_service_announcements.sql"),
    ),
    (
        44,
        include_str!("../migrations/0044_group_action_audit.sql"),
    ),
    (45, include_str!("../migrations/0045_submission_review.sql")),
    (
        46,
        include_str!("../migrations/0046_discovered_peer_endpoints.sql"),
    ),
    (
        47,
        include_str!("../migrations/0047_catalogue_metadata.sql"),
    ),
    (48, include_str!("../migrations/0048_compatibility_ids.sql")),
    (
        49,
        include_str!("../migrations/0049_library_reference_lookup.sql"),
    ),
    (
        50,
        include_str!("../migrations/0050_public_list_aliases.sql"),
    ),
    (
        51,
        include_str!("../migrations/0051_group_profile_audit.sql"),
    ),
    (52, include_str!("../migrations/0052_reader_enrollment.sql")),
];

// Do not derive Debug: a DSN may contain a password.
pub struct DbConfig {
    pub dsn: String,
    pub connections: usize,
    pub acquisition_timeout: Duration,
    pub cache_weight: u64,
    /// Explicit opt-in for a same-machine development database. Never remote.
    pub plaintext_local: bool,
}
#[derive(Clone)]
pub struct Repository {
    pub pool: Pool,
    bodies: Cache<Id, Arc<Event>>,
    pub(crate) connection_config: Arc<tokio_postgres::Config>,
    pub(crate) plaintext_local: bool,
}
/// A signed public library head and its projection status from one primary
/// database statement. The event may be superseded after this snapshot; callers
/// needing a pinned multi-read view must still verify its identity afterwards.
pub struct LibraryHead {
    pub event: Arc<Event>,
    pub projected: bool,
}
#[derive(Clone, Debug)]
pub struct Lease {
    pub id: Id,
    pub generation: i64,
    pub token: [u8; 16],
    pub payload: Value,
}
fn identifier(value: &str) -> Result<Id> {
    let bytes = hex::decode(value).context("invalid event identifier")?;
    bytes
        .try_into()
        .map_err(|_| anyhow!("identifier must be 32 bytes"))
}
fn duration_ms(duration: Duration) -> Result<i64> {
    let millis = i64::try_from(duration.as_millis())?;
    ensure!(
        (10..=300_000).contains(&millis),
        "lease duration outside 10ms..300s"
    );
    Ok(millis)
}
impl Repository {
    pub async fn connect(cfg: DbConfig) -> Result<Self> {
        ensure!(
            (1..=128).contains(&cfg.connections),
            "pool size outside 1..128"
        );
        ensure!(
            cfg.acquisition_timeout > Duration::ZERO
                && cfg.acquisition_timeout <= Duration::from_secs(30),
            "pool acquisition deadline required"
        );
        ensure!(cfg.cache_weight > 0, "cache capacity required");
        let mut pg: tokio_postgres::Config = cfg
            .dsn
            .parse()
            .context("invalid database connection configuration")?;
        pg.connect_timeout(Duration::from_secs(5));
        pg.application_name("manga-node");
        pg.options("-c statement_timeout=5000 -c lock_timeout=1000 -c idle_in_transaction_session_timeout=10000");
        let manager_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };
        let manager = if cfg.plaintext_local {
            ensure!(
                !pg.get_hosts().is_empty(),
                "plaintext requires an explicit local host"
            );
            for host in pg.get_hosts() {
                match host {
                    Host::Tcp(name) => ensure!(
                        name.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()),
                        "plaintext database must use a loopback IP"
                    ),
                    #[cfg(unix)]
                    Host::Unix(_) => {}
                }
            }
            ensure!(
                pg.get_hostaddrs().iter().all(IpAddr::is_loopback),
                "plaintext database hostaddr must be loopback"
            );
            pg.ssl_mode(SslMode::Disable);
            Manager::from_config(pg.clone(), NoTls, manager_config)
        } else {
            pg.ssl_mode(SslMode::Require);
            let _ = rustls::crypto::ring::default_provider().install_default();
            let (tls, _root_warnings) = MakeRustlsConnect::with_native_certs()
                .map_err(|_| anyhow!("no usable native TLS trust roots"))?;
            Manager::from_config(pg.clone(), tls, manager_config)
        };
        let pool = Pool::builder(manager)
            .max_size(cfg.connections)
            .runtime(Runtime::Tokio1)
            .timeouts(Timeouts {
                wait: Some(cfg.acquisition_timeout),
                create: Some(Duration::from_secs(5)),
                recycle: Some(Duration::from_secs(2)),
            })
            .build()?;
        pool.get()
            .await?
            .simple_query("SELECT 1")
            .await
            .context("database health check")?;
        let bodies = Cache::builder()
            .max_capacity(cfg.cache_weight)
            .weigher(|_: &Id, event: &Arc<Event>| {
                let tags: usize = event
                    .tags
                    .iter()
                    .map(|t| {
                        std::mem::size_of::<Vec<String>>()
                            + t.iter()
                                .map(|s| std::mem::size_of::<String>() + s.len())
                                .sum::<usize>()
                    })
                    .sum();
                (std::mem::size_of::<Event>()
                    + event.content.len()
                    + event.id.len()
                    + event.pubkey.len()
                    + event.sig.len()
                    + tags
                    + 128)
                    .min(u32::MAX as usize) as u32
            })
            .build();
        Ok(Self {
            pool,
            bodies,
            connection_config: Arc::new(pg),
            plaintext_local: cfg.plaintext_local,
        })
    }
    /// Explicit deployment operation; not run by every API process at startup.
    pub async fn migrate(&self) -> Result<()> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        tx.execute("SELECT pg_advisory_xact_lock(697241113)", &[])
            .await?;
        tx.batch_execute("CREATE TABLE IF NOT EXISTS schema_migrations(version BIGINT PRIMARY KEY,checksum BYTEA NOT NULL,applied_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp())").await?;
        let applied = tx
            .query(
                "SELECT version,checksum FROM schema_migrations ORDER BY version",
                &[],
            )
            .await?;
        ensure!(
            applied.len() <= MIGRATIONS.len(),
            "database schema is newer than this binary"
        );
        for (index, row) in applied.iter().enumerate() {
            let (version, sql) = MIGRATIONS[index];
            ensure!(
                row.get::<_, i64>(0) == version,
                "migration history is not a known prefix"
            );
            ensure!(
                row.get::<_, Vec<u8>>(1) == Sha256::digest(sql.as_bytes()).to_vec(),
                "migration checksum mismatch"
            );
        }
        for (version, sql) in &MIGRATIONS[applied.len()..] {
            let checksum = Sha256::digest(sql.as_bytes()).to_vec();
            tx.batch_execute(sql).await?;
            tx.execute(
                "INSERT INTO schema_migrations(version,checksum) VALUES($1,$2)",
                &[version, &checksum],
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    pub async fn verify_schema(&self) -> Result<()> {
        let rows = self
            .pool
            .get()
            .await?
            .query(
                "SELECT version,checksum FROM schema_migrations ORDER BY version",
                &[],
            )
            .await
            .context("database migrations are required")?;
        ensure!(
            rows.len() == MIGRATIONS.len(),
            "database schema version does not match this binary"
        );
        for (row, (version, sql)) in rows.iter().zip(MIGRATIONS) {
            ensure!(
                row.get::<_, i64>(0) == *version
                    && row.get::<_, Vec<u8>>(1) == Sha256::digest(sql.as_bytes()).to_vec(),
                "database migration checksum/version mismatch"
            );
        }
        Ok(())
    }
    pub async fn bind_instance(&self, service: Id, admin: Id) -> Result<()> {
        self.verify_schema().await?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        tx.execute("INSERT INTO node_identity(singleton,service_key,admin_key) VALUES(TRUE,$1,$2) ON CONFLICT(singleton) DO NOTHING",&[&service.as_slice(),&admin.as_slice()]).await?;
        let row = tx
            .query_one(
                "SELECT service_key,admin_key FROM node_identity WHERE singleton=TRUE",
                &[],
            )
            .await?;
        ensure!(
            row.get::<_, Vec<u8>>(0) == service && row.get::<_, Vec<u8>>(1) == admin,
            "database belongs to a different service/admin identity"
        );
        tx.execute("INSERT INTO publisher_permissions(publisher,allowed) VALUES($1,TRUE) ON CONFLICT(publisher) DO NOTHING", &[&admin.as_slice()]).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn authorization_clock_allowed(&self, created: u64) -> Result<bool> {
        let created = i64::try_from(created)?;
        Ok(self.pool.get().await?.query_one("SELECT $1::bigint>=COALESCE((SELECT auth_not_before FROM node_identity WHERE singleton),0)",&[&created]).await?.get(0))
    }
    pub async fn consume_http_auth(&self, id: Id, created_at: u64) -> Result<bool> {
        let created = i64::try_from(created_at)?;
        let client = during(Phase::PoolCheckout, self.pool.get()).await?;
        let statement=during(Phase::DbRoundtrip, client.prepare_cached("INSERT INTO auth_used(id,expires_at) SELECT $1,$2::BIGINT+120 WHERE $2::BIGINT BETWEEN floor(extract(epoch FROM clock_timestamp()))::BIGINT-60 AND floor(extract(epoch FROM clock_timestamp()))::BIGINT+60 AND $2::BIGINT>=COALESCE((SELECT auth_not_before FROM node_identity WHERE singleton),0) ON CONFLICT(id) DO NOTHING")).await?;
        Ok(during(
            Phase::DbRoundtrip,
            client.execute(&statement, &[&id.as_slice(), &created]),
        )
        .await?
            == 1)
    }
    pub async fn purge_expired_auth(&self) -> Result<u64> {
        Ok(self.pool.get().await?.execute("DELETE FROM auth_used WHERE id IN (SELECT id FROM auth_used WHERE expires_at<floor(extract(epoch FROM clock_timestamp()))::BIGINT ORDER BY expires_at,id LIMIT 1000)",&[]).await?)
    }
    /// Durable regular-event staging with atomic projection intent. HTTP callers
    /// must separately honor their promised backend-projection completion boundary.
    /// Publications, replaceable events and kind-5 requests have separate workflows.
    pub async fn stage_regular_event(&self, event: &Event) -> Result<bool> {
        self.stage_regular_authorized(event, None).await
    }
    pub async fn stage_personal_note(
        &self,
        permit: &crate::accounts::AccountPermit,
        event: &Event,
    ) -> Result<bool> {
        ensure!(
            event.kind == 1 && event.pubkey == hex::encode(permit.publisher),
            "personal note authority mismatch"
        );
        self.stage_regular_authorized(event, Some(permit)).await
    }
    async fn stage_regular_authorized(
        &self,
        event: &Event,
        permit: Option<&crate::accounts::AccountPermit>,
    ) -> Result<bool> {
        event.verify().map_err(|e| anyhow!(e))?;
        ensure!(
            [1, 7, 1111].contains(&event.kind),
            "event kind requires its dedicated workflow"
        );
        ensure!(
            event.values("expiration").is_empty(),
            "expiring events require a dedicated lifecycle"
        );
        ensure!(
            event.tags.iter().flatten().all(|tag| tag.len() <= 1024),
            "tag value exceeds backend policy"
        );
        let raw = serde_json::to_value(event)?;
        ensure!(
            serde_json::to_vec(&raw)?.len() <= 1_048_576,
            "event size budget"
        );
        let id = identifier(&event.id)?;
        let author = identifier(&event.pubkey)?;
        let kind = i32::try_from(event.kind)?;
        let created = i64::try_from(event.created_at)?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        if let Some(permit) = permit {
            crate::accounts::check_account_authority_tx(&tx, permit).await?;
        }
        crate::ordinary_deletion::admit_event_tx(&tx, event).await?;
        let inserted=tx.execute("INSERT INTO events(id,publisher,kind,created_at,raw_event,state) VALUES($1,$2,$3,$4,$5,'hosted') ON CONFLICT(id) DO NOTHING", &[&id.as_slice(),&author.as_slice(),&kind,&created,&raw]).await? == 1;
        if inserted {
            tx.execute(
                "INSERT INTO backend_outbox(event_id,action,payload) VALUES($1,'import',$2)",
                &[&id.as_slice(), &raw],
            )
            .await?;
        }
        if let Some(permit) = permit {
            crate::accounts::check_account_lease_tx(&tx, permit).await?;
        }
        tx.commit().await?;
        Ok(inserted)
    }
    /// Completion of this specific currently hosted event, including its health.
    /// This is not a queue-drain or predecessor-retirement fence: verified imports
    /// may acknowledge before unrelated author-bound retirement transactions.
    pub async fn projection_complete(&self, id: Id) -> Result<bool> {
        let row=self.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM events e WHERE id=$1 AND state='hosted' AND NOT EXISTS(SELECT 1 FROM publication_blobs p JOIN quarantined_blobs q ON q.hash=p.hash AND q.generation=p.generation WHERE p.event_id=e.id) AND NOT EXISTS(SELECT 1 FROM library_object_holds h JOIN quarantined_blobs q ON q.hash=h.hash AND q.generation=h.generation WHERE e.kind=11999 AND h.publisher=e.publisher AND h.state='current')) AND NOT EXISTS(SELECT 1 FROM backend_outbox WHERE event_id=$1 AND completed_at IS NULL)", &[&id.as_slice()]).await?;
        Ok(row.get(0))
    }
    /// Resolve head identity, current health, signed body and projection state
    /// together. A replacement between separate ID/body reads must not become a
    /// false "no head" result. An immutable cache cannot establish this snapshot.
    pub async fn current_library_head(&self, publisher: Id) -> Result<Option<LibraryHead>> {
        let client = during(Phase::PoolCheckout, self.pool.get()).await?;
        let statement = during(Phase::DbRoundtrip, client.prepare_cached(
            "SELECT e.id,e.raw_event,NOT EXISTS(SELECT 1 FROM backend_outbox o WHERE o.event_id=e.id AND o.completed_at IS NULL) FROM account_heads h JOIN events e ON e.id=h.event_id AND e.publisher=h.publisher AND e.kind=h.kind WHERE h.publisher=$1 AND h.kind=11999 AND e.state='hosted' AND NOT EXISTS(SELECT 1 FROM publication_blobs p JOIN quarantined_blobs q ON q.hash=p.hash AND q.generation=p.generation WHERE p.event_id=e.id) AND NOT EXISTS(SELECT 1 FROM library_object_holds l JOIN quarantined_blobs q ON q.hash=l.hash AND q.generation=l.generation WHERE l.publisher=e.publisher AND l.state='current')"
        )).await?;
        let Some(row) = during(
            Phase::DbRoundtrip,
            client.query_opt(&statement, &[&publisher.as_slice()]),
        )
        .await?
        else {
            return Ok(None);
        };
        let _decode = phase(Phase::DecodeResponse);
        let event: Event = serde_json::from_value(row.get(1))?;
        event.verify().map_err(|error| anyhow!(error))?;
        ensure!(
            identifier(&event.id)?.as_slice() == row.get::<_, Vec<u8>>(0)
                && identifier(&event.pubkey)? == publisher
                && event.kind == 11999,
            "stored library head identity mismatch"
        );
        Ok(Some(LibraryHead {
            event: Arc::new(event),
            projected: row.get(2),
        }))
    }
    /// A cached immutable body never bypasses current primary-database visibility.
    pub async fn visible_event(&self, id: Id) -> Result<Option<Arc<Event>>> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_cached("SELECT state FROM events e WHERE id=$1 AND NOT EXISTS(SELECT 1 FROM publication_blobs p JOIN quarantined_blobs q ON q.hash=p.hash AND q.generation=p.generation WHERE p.event_id=e.id) AND NOT EXISTS(SELECT 1 FROM library_object_holds h JOIN quarantined_blobs q ON q.hash=h.hash AND q.generation=h.generation WHERE e.kind=11999 AND h.publisher=e.publisher AND h.state='current')")
            .await?;
        let visible = client
            .query_opt(&statement, &[&id.as_slice()])
            .await?
            .is_some_and(|r| r.get::<_, String>(0) == "hosted");
        drop(client);
        if !visible {
            return Ok(None);
        }
        let repo = self.clone();
        let value = self
            .bodies
            .try_get_with(id, async move {
                let client = repo.pool.get().await?;
                let statement = client
                    .prepare_cached("SELECT raw_event FROM events WHERE id=$1")
                    .await?;
                let row = client.query_one(&statement, &[&id.as_slice()]).await?;
                let event: Event = serde_json::from_value(row.get(0))?;
                event.verify().map_err(|e| anyhow!(e))?;
                ensure!(
                    identifier(&event.id)? == id,
                    "stored event identity mismatch"
                );
                Ok::<_, anyhow::Error>(Arc::new(event))
            })
            .await
            .map_err(|e| anyhow!("event cache load: {e}"))?;
        Ok(Some(value))
    }
    pub async fn lease_current(&self, lease: &Lease) -> Result<bool> {
        Ok(self.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM jobs WHERE id=$1 AND generation=$2 AND claim_token=$3 AND state='leased' AND lease_until>clock_timestamp())", &[&lease.id.as_slice(),&lease.generation,&lease.token.as_slice()]).await?.get(0))
    }
    pub async fn renew(&self, lease: &Lease, duration: Duration) -> Result<bool> {
        let millis = duration_ms(duration)?;
        let client = self.pool.get().await?;
        Ok(client.execute("UPDATE jobs SET lease_until=clock_timestamp()+($4::BIGINT*interval '1 millisecond'),updated_at=clock_timestamp() WHERE id=$1 AND generation=$2 AND claim_token=$3 AND state='leased' AND lease_until>clock_timestamp()", &[&lease.id.as_slice(),&lease.generation,&lease.token.as_slice(),&millis]).await?==1)
    }
}

pub(crate) async fn check_lease_tx(
    tx: &tokio_postgres::Transaction<'_>,
    lease: &Lease,
) -> Result<()> {
    let row=tx.query_opt("SELECT lease_until>clock_timestamp() FROM jobs WHERE id=$1 AND generation=$2 AND claim_token=$3 AND state='leased' FOR SHARE", &[&lease.id.as_slice(),&lease.generation,&lease.token.as_slice()]).await?;
    ensure!(
        row.is_some_and(|r| r.get::<_, bool>(0)),
        "job lease lost, expired or cancelled"
    );
    Ok(())
}
