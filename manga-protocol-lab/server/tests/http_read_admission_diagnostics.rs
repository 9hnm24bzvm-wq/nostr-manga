//! Operator diagnostics through real HTTP, process logs and an external DB fault.
mod support;

use anyhow::{Context, Result, ensure};
use manga_node::db::{DbConfig, Repository};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::Write,
    os::fd::{AsRawFd, FromRawFd},
    path::{Path, PathBuf},
    process::Child,
    process::Command,
    time::Duration,
};
use support::*;

const PHASE_NAMES: [&str; 8] = [
    "dispatch_pending",
    "handler",
    "authentication",
    "pool_checkout",
    "db_roundtrip",
    "decode_response",
    "storage_wait",
    "uninstrumented",
];
const POOL_CHECKOUT: usize = 3;
const DB_ROUNDTRIP: usize = 4;

fn numeric_array(value: &Value, length: usize) -> Result<()> {
    let values = value.as_array().context("fixed numeric array missing")?;
    ensure!(values.len() == length, "diagnostic array shape changed");
    ensure!(
        values.iter().all(|value| value.as_u64().is_some()),
        "diagnostic arrays must contain only unsigned counters"
    );
    Ok(())
}

fn numeric_matrix(value: &Value, columns: usize) -> Result<()> {
    let rows = value.as_array().context("fixed numeric matrix missing")?;
    ensure!(rows.len() == 3, "diagnostic class order must stay fixed");
    for row in rows {
        numeric_array(row, columns)?;
    }
    Ok(())
}

fn holder_diagnostics(status: &Value) -> Result<&Value> {
    let holders = status
        .get("read_holder_diagnostics")
        .context("opt-in operator holder diagnostics missing")?;
    ensure!(holders["phase_names"] == json!(PHASE_NAMES));
    ensure!(
        holders["lifetime_upper_us"] == json!([1000, 10000, 100000, 500000, 1000000, null]),
        "holder lifetime histogram bounds changed"
    );
    for field in [
        "started",
        "finished",
        "lifetime_us",
        "lifetime_max_us",
        "active_draining",
    ] {
        numeric_array(&holders[field], 3)?;
    }
    for field in ["active_phase_counts", "phase_us"] {
        numeric_matrix(&holders[field], PHASE_NAMES.len())?;
    }
    numeric_matrix(&holders["lifetime_buckets"], 6)?;
    ensure!(
        holders
            .as_object()
            .context("holder diagnostic object missing")?
            .keys()
            .all(|key| matches!(
                key.as_str(),
                "phase_names"
                    | "lifetime_upper_us"
                    | "started"
                    | "finished"
                    | "active_phase_counts"
                    | "phase_us"
                    | "lifetime_us"
                    | "lifetime_max_us"
                    | "lifetime_buckets"
                    | "active_draining"
            )),
        "holder status must not expose request identities, SQL, or unbounded labels"
    );
    Ok(holders)
}

fn rejection_phase(record: &Value, phase: usize) -> Result<()> {
    numeric_matrix(&record["holder_phases"], PHASE_NAMES.len())?;
    numeric_array(&record["holder_draining"], 3)?;
    ensure!(
        record["holder_phases"][0][phase] == 1,
        "externally stalled catalogue holder must identify {}: {record}",
        PHASE_NAMES[phase]
    );
    let other = if phase == DB_ROUNDTRIP {
        POOL_CHECKOUT
    } else {
        DB_ROUNDTRIP
    };
    ensure!(
        record["holder_phases"][0][other] == 0,
        "pool acquisition and database round-trip wait must remain distinct"
    );
    ensure!(record["holder_draining"][0] == 0);
    Ok(())
}

struct Node(Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Fixture {
    _node: Node,
    repo: Repository,
    client: reqwest::Client,
    origin: String,
    log: PathBuf,
}
impl Fixture {
    async fn start(repo: Repository, dir: &Path, enabled: bool, label: &str) -> Result<Self> {
        Self::start_with_sink(repo, dir, enabled, None, label).await
    }
    async fn start_with_sink(
        repo: Repository,
        dir: &Path,
        enabled: bool,
        sink: Option<&File>,
        label: &str,
    ) -> Result<Self> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let bind = listener.local_addr()?;
        drop(listener);
        let origin = format!("http://{bind}");
        let key = dir.join("service.key");
        if !key.exists() {
            write_key(&key, 11)?;
        }
        let config = dir.join(format!("{label}.json"));
        std::fs::write(
            &config,
            serde_json::to_vec(&json!({
                "bind": bind.to_string(), "base_url": origin,
                "service_key_file": key, "admin_pubkey": public(1),
                "read_policy": "admitted", "database_plaintext_local": true,
                "database_connections": 8, "sync_worker_enabled": false,
                "read_admission": {
                    "overall": 2, "wait_ms": 300,
                    "catalogue": {"active": 1, "waiting": 1},
                    "library_head": {"active": 1, "waiting": 1},
                    "live": {"active": 1, "waiting": 1}
                },
                "group_keys_dir": dir.join("groups"),
                "storage": {
                    "path": dir.join("objects"), "volume": "readdiag",
                    "physical_bytes": 1048576, "staging_bytes": 1048576,
                    "max_blob_bytes": 262144
                },
                "backend": {"binary": root.join("vendor/strfry/strfry"),
                    "data_dir": dir.join("backend"), "map_bytes": 67108864}
            }))?,
        )?;
        let log = dir.join(format!("{label}.log"));
        let file = std::fs::File::create(&log)?;
        let mut command = Command::new(
            std::env::var_os("MANGA_TEST_NODE_BIN")
                .unwrap_or_else(|| env!("CARGO_BIN_EXE_manga-node").into()),
        );
        command
            .args(["serve", config.to_str().unwrap()])
            .env(
                "MANGA_DATABASE_URL",
                std::env::var("MANGA_TEST_DATABASE_URL")?,
            )
            .env_remove("MANGA_READ_ADMISSION_DIAGNOSTICS")
            .env_remove("MANGA_BENCH_PROFILE")
            .stdout(file.try_clone()?)
            .stderr(sink.map(File::try_clone).transpose()?.unwrap_or(file));
        if enabled {
            command.env("MANGA_READ_ADMISSION_DIAGNOSTICS", "1");
        }
        let node = Node(command.spawn()?);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;
        let mut ready = false;
        for _ in 0..100 {
            if client
                .get(&origin)
                .send()
                .await
                .is_ok_and(|response| response.status().is_success())
            {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        ensure!(
            ready,
            "node startup failed: {}",
            std::fs::read_to_string(&log)?
        );
        ensure!(
            post_json(
                &client,
                1,
                &format!("{origin}/admin/accounts/{}", public(2)),
                &json!({"allowed": true}),
            )
            .await?
            .status()
                == 200,
            "reader admission setup failed"
        );
        Ok(Self {
            _node: node,
            repo,
            client,
            origin,
            log,
        })
    }

    async fn status(&self) -> Result<Value> {
        let url = format!("{}/admin/status", self.origin);
        let response = self
            .client
            .get(&url)
            .header("authorization", http_get(1, &url))
            .send()
            .await?;
        ensure!(response.status() == 200, "operator status must recover");
        Ok(response.json().await?)
    }

    async fn denied_status_hides_diagnostics(&self) -> Result<()> {
        let url = format!("{}/admin/status", self.origin);
        for authorization in [None, Some(http_get(2, &url))] {
            let mut request = self.client.get(&url);
            if let Some(authorization) = authorization {
                request = request.header("authorization", authorization);
            }
            let response = request.send().await?;
            ensure!(
                response.status() == 403,
                "unauthenticated and admitted non-admin readers must remain forbidden"
            );
            let body = response.text().await?;
            ensure!(
                !body.contains("read_admission_diagnostics")
                    && !body.contains("read_holder_diagnostics"),
                "forbidden operator responses must not expose diagnostic fields"
            );
        }
        Ok(())
    }

    fn records(&self) -> Result<Vec<Value>> {
        std::fs::read_to_string(&self.log)?
            .lines()
            .filter_map(|line| line.strip_prefix("READ_ADMISSION_DIAGNOSTIC "))
            .map(|line| {
                ensure!(
                    line.len() <= 2048,
                    "diagnostic record exceeded its fixed budget"
                );
                Ok(serde_json::from_str(line)?)
            })
            .collect()
    }

    async fn wait_records(&self, count: usize) -> Result<Vec<Value>> {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let records = self.records()?;
                if records.len() >= count {
                    break Ok::<_, anyhow::Error>(records);
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .context("bounded diagnostic delivery did not complete")?
    }

    /// SQL is only the external policy-stall arrangement and its lock barrier.
    /// Public HTTP responses and operator diagnostics are the outcome oracles.
    async fn reject_behind_policy_lock(&self, requests: usize) -> Result<()> {
        let mut blocker = self.repo.pool.get().await?;
        let pid: i32 = blocker
            .query_one("SELECT pg_backend_pid()", &[])
            .await?
            .get(0);
        let transaction = blocker.transaction().await?;
        transaction
            .batch_execute("LOCK TABLE account_permissions IN ACCESS EXCLUSIVE MODE")
            .await?;
        let url = format!("{}/query?order=asc", self.origin);
        let active = {
            let client = self.client.clone();
            let url = url.clone();
            tokio::spawn(async move {
                client
                    .get(&url)
                    .header("authorization", http_get(2, &url))
                    .send()
                    .await
            })
        };
        tokio::time::timeout(Duration::from_millis(400), async {
            loop {
                let blocked: bool = self.repo.pool.get().await?.query_one(
                    "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
                    &[&pid],
                ).await?.get(0);
                if blocked {
                    break Ok::<_, anyhow::Error>(());
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }).await.context("external policy lock did not engage")??;
        let mut burst = tokio::task::JoinSet::new();
        for _ in 0..requests {
            let client = self.client.clone();
            let url = url.clone();
            burst.spawn(async move {
                client
                    .get(&url)
                    .header("authorization", http_get(2, &url))
                    .send()
                    .await
            });
        }
        let responses = tokio::time::timeout(Duration::from_millis(700), async {
            let mut responses = Vec::new();
            while let Some(response) = burst.join_next().await {
                responses.push(response??);
            }
            Ok::<_, anyhow::Error>(responses)
        })
        .await;
        transaction.rollback().await?;
        let admitted = active.await??;
        ensure!(
            admitted.status() == 200,
            "admitted work must finish after unlock"
        );
        let responses = responses.context("bounded rejection burst did not finish")??;
        ensure!(
            responses.len() == requests,
            "each rejected request needs a response"
        );
        for response in responses {
            ensure!(
                response.status() == 429,
                "existing busy HTTP status changed"
            );
            ensure!(
                response.headers()["retry-after"] == "1",
                "retry contract changed"
            );
            ensure!(
                response.json::<Value>().await? == json!({"error":"read_slots_busy"}),
                "public busy response must not expose diagnostic details"
            );
        }
        Ok(())
    }

    /// Fill the application's pool with real public writes. The test's own
    /// connection only supplies an external fault and its blocking barrier.
    async fn reject_behind_exhausted_pool(&self) -> Result<()> {
        let mut blocker = self.repo.pool.get().await?;
        let pid: i32 = blocker
            .query_one("SELECT pg_backend_pid()", &[])
            .await?
            .get(0);
        let transaction = blocker.transaction().await?;
        transaction
            .batch_execute("LOCK TABLE account_permissions IN ACCESS EXCLUSIVE MODE")
            .await?;
        let mut writes = tokio::task::JoinSet::new();
        for key in 20..28 {
            let client = self.client.clone();
            let url = format!("{}/admin/accounts/{}", self.origin, public(key));
            writes
                .spawn(async move { post_json(&client, 1, &url, &json!({"allowed": true})).await });
        }
        tokio::time::timeout(Duration::from_millis(400), async {
            loop {
                let blocked: i64 = self
                    .repo
                    .pool
                    .get()
                    .await?
                    .query_one(
                        "SELECT count(*) FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid))",
                        &[&pid],
                    )
                    .await?
                    .get(0);
                if blocked == 8 {
                    break Ok::<_, anyhow::Error>(());
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .context("public writes did not occupy the node pool before the lock deadline")??;

        // Two same-class reads leave one holder waiting for a connection and
        // one admission waiter. Which HTTP request wins admission is irrelevant.
        let mut reads = tokio::task::JoinSet::new();
        for _ in 0..2 {
            let client = self.client.clone();
            let url = format!("{}/query?order=asc", self.origin);
            reads.spawn(async move {
                client
                    .get(&url)
                    .header("authorization", http_get(2, &url))
                    .send()
                    .await
            });
        }
        let first = tokio::time::timeout(Duration::from_millis(450), reads.join_next()).await;
        transaction.rollback().await?;
        let first = first
            .context("admission expiry was blocked behind pool checkout")?
            .context("missing admission waiter response")???;
        ensure!(
            first.status() == 429,
            "pool pressure changed the busy status"
        );
        ensure!(first.headers()["retry-after"] == "1");
        ensure!(
            first.json::<Value>().await? == json!({"error":"read_slots_busy"}),
            "pool pressure leaked diagnostics into the public response"
        );
        let admitted = reads
            .join_next()
            .await
            .context("missing admitted read response")???;
        ensure!(
            admitted.status() == 200,
            "read must recover after pool release"
        );
        while let Some(write) = writes.join_next().await {
            ensure!(
                write??.status() == 200,
                "public pool-filling write did not recover"
            );
        }
        Ok(())
    }
}

// A real OS pipe is an external process-output fault, not an application hook.
fn pipe() -> Result<(File, File)> {
    let mut fds = [0; 2];
    ensure!(
        unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } == 0,
        "pipe creation failed: {}",
        std::io::Error::last_os_error()
    );
    Ok(unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) })
}
fn fill_pipe(writer: &mut File) -> Result<()> {
    let fd = writer.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    ensure!(flags >= 0 && unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == 0);
    let mut total = 0;
    loop {
        match writer.write(&[0; 4096]) {
            Ok(count) => {
                total += count;
                ensure!(total <= 1_048_576, "pipe fault exceeded bounded setup");
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(error) => return Err(error.into()),
        }
    }
    ensure!(unsafe { libc::fcntl(fd, libc::F_SETFL, flags) } == 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "real PostgreSQL, node process and externally blocked HTTP policy work"]
async fn bounded_operator_diagnostics_classify_rejections_without_changing_http() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    ensure!(
        dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test"),
        "fixture only"
    );
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let dir = tempfile::Builder::new()
        .prefix("manga-read-admission-diagnostic-")
        .tempdir()?
        .keep();
    println!("Diagnostic fixture retained at {}", dir.display());

    let disabled = Fixture::start(repo.clone(), &dir, false, "disabled").await?;
    disabled.reject_behind_policy_lock(4).await?;
    ensure!(
        disabled.records()?.is_empty(),
        "default-off mode must emit no diagnostics"
    );
    let status = disabled.status().await?;
    ensure!(
        status.get("read_admission_diagnostics").is_none()
            && status.get("read_holder_diagnostics").is_none(),
        "default-off operator response must remain unchanged"
    );
    drop(disabled);

    let enabled = Fixture::start(repo.clone(), &dir, true, "enabled").await?;
    enabled.denied_status_hides_diagnostics().await?;
    enabled.reject_behind_policy_lock(16).await?;
    let status = enabled.status().await?;
    let holders = holder_diagnostics(&status)?;
    ensure!(holders["finished"][0].as_u64().unwrap() >= 1);
    ensure!(
        holders["phase_us"][0][DB_ROUNDTRIP].as_u64().unwrap() >= 300_000,
        "the completed externally blocked read must account for its DB wait"
    );
    ensure!(holders["lifetime_max_us"][0].as_u64().unwrap() >= 300_000);
    ensure!(
        holders["lifetime_buckets"][0]
            .as_array()
            .unwrap()
            .iter()
            .skip(3)
            .any(|count| count.as_u64().unwrap() >= 1),
        "completed blocked work must reach its bounded lifetime histogram"
    );
    ensure!(holders["active_draining"] == json!([0, 0, 0]));
    // Status is itself an admitted catalogue read. Its independently sampled
    // counters are deliberately not asserted to sum or conserve while active.
    let diagnostic = status
        .get("read_admission_diagnostics")
        .context("opt-in operator rejection diagnostics missing")?;
    ensure!(
        diagnostic["queue_full"] == json!([15, 0, 0]),
        "overflow totals must include suppressed records"
    );
    ensure!(
        diagnostic["wait_expired"] == json!([1, 0, 0]),
        "expiry must be distinct from queue overflow"
    );
    ensure!(diagnostic["ticket_exhausted"] == json!([0, 0, 0]));
    ensure!(
        diagnostic["records_selected"] == 1 && diagnostic["records_suppressed"] == 15,
        "a rejection burst must emit at most one bounded record"
    );
    let first = enabled.wait_records(1).await?;
    ensure!(first.len() == 1 && first[0]["reason"] == "queue_full");
    ensure!(first[0]["class"] == "catalogue");
    ensure!(first[0]["timeout_overshoot_us"].is_null());
    ensure!(first[0]["active"] == json!([1, 0, 0]) && first[0]["waiting"] == json!([1, 0, 0]));
    rejection_phase(&first[0], DB_ROUNDTRIP)?;

    // Wait beyond the documented log interval, then provoke only wait expiry.
    tokio::time::sleep(Duration::from_millis(1100)).await;
    enabled.reject_behind_policy_lock(1).await?;
    let status = enabled.status().await?;
    let diagnostic = &status["read_admission_diagnostics"];
    ensure!(diagnostic["queue_full"] == json!([15, 0, 0]));
    ensure!(diagnostic["wait_expired"] == json!([2, 0, 0]));
    ensure!(diagnostic["records_selected"] == 2 && diagnostic["records_suppressed"] == 15);
    let records = enabled.wait_records(2).await?;
    ensure!(records.len() == 2 && records[1]["reason"] == "wait_expired");
    let expired = &records[1];
    rejection_phase(expired, DB_ROUNDTRIP)?;
    let waited = expired["queued_us"]
        .as_u64()
        .context("measured queue wait missing")?;
    ensure!(
        waited >= 300_000,
        "expiry must report actual elapsed queue time"
    );
    ensure!(
        expired["elapsed_us"]
            .as_u64()
            .context("elapsed admission time missing")?
            >= waited
    );
    ensure!(expired["timeout_overshoot_us"].as_u64() == Some(waited - 300_000));
    ensure!(
        expired["waiting"] == json!([0, 0, 0]),
        "expired ticket must be released before snapshot"
    );
    ensure!(
        expired["since_start_us"].as_u64().unwrap()
            - records[0]["since_start_us"].as_u64().unwrap()
            >= 1_000_000,
        "diagnostic emission must respect its monotonic one-second interval"
    );
    for record in records {
        let fields = record.as_object().context("diagnostic object required")?;
        ensure!(
            fields.keys().all(|key| matches!(
                key.as_str(),
                "version"
                    | "at_unix_ms"
                    | "since_start_us"
                    | "class"
                    | "reason"
                    | "elapsed_us"
                    | "queued_us"
                    | "timeout_overshoot_us"
                    | "wait_ms"
                    | "active"
                    | "waiting"
                    | "overall_active"
                    | "overall_limit"
                    | "active_limits"
                    | "waiting_limits"
                    | "holder_phases"
                    | "holder_draining"
                    | "totals"
            )),
            "diagnostics may contain only fixed numeric fields and class/reason enums"
        );
    }
    drop(enabled);

    // Keep the database's existing backend/storage identity; only the stage
    // config and log names differ across these sequential node restarts.
    let pool_wait = Fixture::start(repo.clone(), &dir, true, "pool-checkout").await?;
    pool_wait.reject_behind_exhausted_pool().await?;
    let records = pool_wait.wait_records(1).await?;
    ensure!(records.len() == 1 && records[0]["reason"] == "wait_expired");
    rejection_phase(&records[0], POOL_CHECKOUT)?;
    let status = pool_wait.status().await?;
    let holders = holder_diagnostics(&status)?;
    ensure!(
        holders["phase_us"][0][POOL_CHECKOUT].as_u64().unwrap() >= 250_000,
        "holder pool wait must be measured separately from DB round trips"
    );
    ensure!(holders["finished"][0].as_u64().unwrap() >= 1);
    drop(pool_wait);

    let (reader, mut writer) = pipe()?;
    let mut stalled =
        Fixture::start_with_sink(repo, &dir, true, Some(&writer), "blocked-sink").await?;
    fill_pipe(&mut writer)?;
    // With an undrained output sink, each selected rejection must still return
    // the same bounded HTTP response. Three intervals fill writer + queue + drop.
    for round in 0..3 {
        if round != 0 {
            tokio::time::sleep(Duration::from_millis(1100)).await;
        }
        stalled.reject_behind_policy_lock(1).await?;
    }
    let status = stalled.status().await?;
    holder_diagnostics(&status)?;
    let totals = &status["read_admission_diagnostics"];
    ensure!(totals["wait_expired"] == json!([3, 0, 0]));
    ensure!(
        totals["records_selected"] == 3 && totals["records_dropped"] == 1,
        "full diagnostic sink must drop instead of accumulating work"
    );
    ensure!(totals["sink_write_attempts"] == 1 && totals["sink_write_failures"] == 0);
    // Losing the pipe reader turns the bounded blocked writer into an ordinary
    // output failure; neither failure nor process termination may hang HTTP.
    drop(reader);
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if stalled.status().await?["read_admission_diagnostics"]["sink_write_failures"]
                .as_u64()
                .unwrap_or(0)
                >= 1
            {
                break Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("faulted sink did not report its failed write")??;
    stalled._node.0.kill()?;
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if stalled._node.0.try_wait()?.is_some() {
                break Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("diagnostic node did not terminate promptly")??;
    Ok(())
}
