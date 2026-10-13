//! Versioned immutable imports and explicit local deletion barriers. Raw kind5
//! never enters strfry. A subprocess-held flock survives death of the API parent.
use crate::{auth::Event, db::Repository};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::HashSet, path::PathBuf, process::Stdio, sync::Arc, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendConfig {
    pub binary: PathBuf,
    pub data_dir: PathBuf,
    #[serde(default = "deadline")]
    pub timeout_seconds: u64,
    #[serde(default = "map_bytes")]
    pub map_bytes: u64,
}
fn deadline() -> u64 {
    10
}
fn map_bytes() -> u64 {
    1024 * 1024 * 1024
}
pub struct Backend {
    repo: Repository,
    binary: PathBuf,
    config: PathBuf,
    timeout: u64,
    command_lock: PathBuf,
    _owner: std::fs::File,
}
// Observe the leader without reaping, retaining its process-group identity
// until actual descendants terminate. The command task also joins every pipe.
struct OwnedCommand {
    child: tokio::process::Child,
    embedded: bool,
}
impl OwnedCommand {
    async fn wait_terminal(&mut self) -> Result<std::process::ExitStatus> {
        if !self.embedded {
            return Ok(self.child.wait().await?);
        }
        let pid = self
            .child
            .id()
            .context("backend command process identity")? as i32;
        let terminal = async {
            loop {
                // siginfo_t contains platform pointer fields and must remain
                // entirely in this synchronous observation scope.
                let exited = {
                    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
                    let result = unsafe {
                        libc::waitid(
                            libc::P_PID,
                            pid as u32,
                            &mut info,
                            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                        )
                    };
                    if result == -1 {
                        let error = std::io::Error::last_os_error();
                        if error.kind() == std::io::ErrorKind::Interrupted {
                            continue;
                        }
                        return Err(error.into());
                    }
                    (unsafe { info.si_pid() }) != 0
                };
                if exited {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            // Failed commands can leave descendants retaining pipes or writable
            // projection handles. The zombie leader still reserves this group ID.
            if unsafe { libc::kill(-pid, libc::SIGKILL) } == -1 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error.into());
                }
            }
            loop {
                if !process_group_running(pid)? {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Ok::<(), anyhow::Error>(())
        }
        .await;
        if terminal.is_err() {
            crate::node::owned_terminal_unconfirmed();
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
        // A nonterminal kernel task retains this actual lease until completion;
        // the supervisor's bounded drain then reports uncertainty, not success.
        let status = self.child.wait().await?;
        terminal?;
        Ok(status)
    }
}
impl Drop for OwnedCommand {
    fn drop(&mut self) {
        if self.embedded
            && let Some(pid) = self.child.id()
        {
            // Forced runtime cancellation is not a successful join observation.
            crate::node::owned_terminal_unconfirmed();
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
    }
}
fn process_group_running(group: i32) -> Result<bool> {
    for entry in std::fs::read_dir("/proc")? {
        let entry = entry?;
        if !entry
            .file_name()
            .as_encoded_bytes()
            .iter()
            .all(u8::is_ascii_digit)
        {
            continue;
        }
        let stat = match std::fs::read_to_string(entry.path().join("stat")) {
            Ok(stat) => stat,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        let fields = stat
            .rsplit_once(')')
            .context("process stat identity")?
            .1
            .split_whitespace()
            .collect::<Vec<_>>();
        ensure!(fields.len() >= 3, "process stat fields");
        if fields[2].parse::<i32>()? == group && !matches!(fields[0], "Z" | "X" | "x") {
            return Ok(true);
        }
    }
    Ok(false)
}
struct Item {
    id: i64,
    event: String,
    version: i64,
    payload: Vec<u8>,
    deleting: bool,
}
impl Backend {
    pub async fn open(repo: Repository, cfg: BackendConfig) -> Result<Arc<Self>> {
        ensure!(
            (1..=60).contains(&cfg.timeout_seconds),
            "backend timeout outside 1..60 seconds"
        );
        ensure!(cfg.map_bytes >= 16 * 1024 * 1024, "backend map too small");
        ensure!(
            cfg.binary.is_absolute() && cfg.binary.is_file(),
            "explicit installed strfry binary required"
        );
        std::fs::create_dir_all(&cfg.data_dir)?;
        let root = cfg.data_dir.canonicalize()?;
        let owner = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join(".projection-owner"))?;
        owner
            .try_lock()
            .context("backend directory already has a projection owner")?;
        let identity_path = root.join(".projection-id");
        let target: [u8; 32] = if identity_path.exists() {
            hex::decode(std::fs::read_to_string(&identity_path)?.trim())?
                .try_into()
                .ok()
                .context("invalid backend target ID")?
        } else {
            use std::io::Write;
            let target: [u8; 32] = rand::random();
            let mut file = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&identity_path)?;
            file.write_all(hex::encode(target).as_bytes())?;
            file.sync_all()?;
            std::fs::File::open(&root)?.sync_all()?;
            target
        };
        let mut client = repo.pool.get().await?;
        let tx = client.transaction().await?;
        tx.execute(&tx.prepare_cached("INSERT INTO backend_identity(singleton,target_id) VALUES(TRUE,$1) ON CONFLICT(singleton) DO NOTHING").await?, &[&target.as_slice()]).await?;
        let stored: Vec<u8> = tx
            .query_one(
                &tx.prepare_cached("SELECT target_id FROM backend_identity WHERE singleton=TRUE")
                    .await?,
                &[],
            )
            .await?
            .get(0);
        ensure!(
            stored == target,
            "database is bound to another backend target; explicit rebuild/restore required"
        );
        tx.commit().await?;
        drop(client);
        let db = root.join("strfry");
        std::fs::create_dir_all(&db)?;
        let path = db.to_str().context("backend path is not UTF8")?;
        ensure!(
            !path.contains(['"', '\\', '\n', '\r']),
            "unsupported backend path quoting"
        );
        let config = root.join("strfry.conf");
        std::fs::write(
            &config,
            format!(
                "db = \"{path}\"\ndbParams {{ mapsize = {} }}\nevents {{ maxEventSize = 1048576 rejectEventsOlderThanSeconds = 4294967295 }}\nrelay {{ nofiles = 0 bind = \"127.0.0.1\" }}\n",
                cfg.map_bytes
            ),
        )?;
        Ok(Arc::new(Self {
            repo,
            binary: cfg.binary,
            config,
            timeout: cfg.timeout_seconds,
            command_lock: root.join(".command-lock"),
            _owner: owner,
        }))
    }
    async fn command(
        &self,
        args: &[String],
        input: Vec<u8>,
        output_limit: usize,
    ) -> Result<Vec<u8>> {
        let embedded = crate::node::owned_context_active();
        let timeout = self.timeout;
        let command_lock = self.command_lock.clone();
        let binary = self.binary.clone();
        let config = self.config.clone();
        let args = args.to_vec();
        let operation = async move {
            // GNU timeout supervises a process group including descendants. Stdout/stderr
            // capture is bounded independently of input; no DB transaction spans this I/O.
            let mut command = tokio::process::Command::new("timeout");
            command
                .args(["--signal=TERM", "--kill-after=1s", &format!("{}s", timeout)])
                .arg("flock")
                .arg("--exclusive")
                .arg(&command_lock)
                .arg(&binary)
                .arg("--config")
                .arg(&config)
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if embedded {
                command.process_group(0);
            }
            let mut child = OwnedCommand {
                child: command.spawn()?,
                embedded,
            };
            let mut stdin = child.child.stdin.take().context("backend input")?;
            let stdout = child.child.stdout.take().context("backend output")?;
            let stderr = child.child.stderr.take().context("backend error output")?;
            let write = async move {
                stdin.write_all(&input).await?;
                stdin.shutdown().await?;
                Ok::<_, std::io::Error>(())
            };
            let read = async move {
                let mut bytes = Vec::new();
                stdout
                    .take(output_limit as u64 + 1)
                    .read_to_end(&mut bytes)
                    .await?;
                Ok::<_, std::io::Error>(bytes)
            };
            let errors = async move {
                let mut bytes = Vec::new();
                stderr.take(65537).read_to_end(&mut bytes).await?;
                Ok::<_, std::io::Error>(bytes)
            };
            let (written, out, err, status) =
                tokio::join!(write, read, errors, child.wait_terminal());
            let out = out?;
            let err = err?;
            let status = status?;
            ensure!(
                status.success(),
                "strfry exited unsuccessfully or timed out: {status}"
            );
            written?;
            ensure!(
                out.len() <= output_limit && err.len() <= 65536,
                "backend output budget exceeded"
            );
            Ok(out)
        };
        if embedded {
            crate::node::spawn_owned_scope(operation)
                .await
                .context("owned backend command task failed")?
        } else {
            operation.await
        }
    }

    async fn batch(&self, client: &mut deadpool_postgres::ClientWrapper) -> Result<usize> {
        let rows=client.query("SELECT o.id,o.event_id,o.payload,o.action,o.event_version,e.version,e.state FROM backend_outbox o JOIN events e ON e.id=o.event_id WHERE o.completed_at IS NULL ORDER BY o.id LIMIT 64",&[]).await?;
        let mut items = Vec::new();
        let mut size = 0;
        let mut addresses = HashSet::new();
        for row in rows {
            let job: i64 = row.get(0);
            let version: i64 = row.get(4);
            let current: i64 = row.get(5);
            let state: String = row.get(6);
            let action: String = row.get(3);
            if version < current {
                client.execute("UPDATE backend_outbox SET completed_at=clock_timestamp() WHERE id=$1 AND completed_at IS NULL", &[&job]).await?;
                continue;
            }
            ensure!(version == current, "outbox generation is ahead of event");
            let id: Vec<u8> = row.get(1);
            if action == "delete" {
                ensure!(state == "withdrawing", "deletion projection state mismatch");
                items.push(Item {
                    id: job,
                    event: hex::encode(id),
                    version,
                    payload: vec![],
                    deleting: true,
                });
                continue;
            }
            ensure!(
                action == "import" && state == "hosted",
                "projection lifecycle barrier"
            );
            let event: Event = serde_json::from_value(row.get::<_, Value>(2))?;
            event.verify().map_err(anyhow::Error::msg)?;
            ensure!(
                event.id == hex::encode(&id)
                    && matches!(
                        event.kind,
                        0 | 3 | 10002 | 11999 | 1 | 7 | 1111 | 1001 | 1002
                    ),
                "event requires a guarded projection workflow"
            );
            // A replacement may remove another ID from strfry. Verify each
            // address at its own boundary rather than expecting both to survive.
            if matches!(event.kind, 0 | 3 | 10002 | 11999)
                && !addresses.insert((event.pubkey.clone(), event.kind))
            {
                break;
            }
            let payload = serde_json::to_vec(&event)?;
            ensure!(payload.len() <= 1048576, "event exceeds backend bound");
            if !items.is_empty() && size + payload.len() + 1 > 1048577 {
                break;
            }
            size += payload.len() + 1;
            items.push(Item {
                id: job,
                event: event.id,
                version,
                payload,
                deleting: false,
            });
        }
        if items.is_empty() {
            return Ok(0);
        }
        // One verified import per replaceable address. Imports precede explicit
        // ID deletions so a replacement is present before its predecessor retires.
        // Version-stale jobs were excluded above; a current ID cannot be both
        // imported and deleted in this plan. A later concurrent change is fenced
        // again by the acknowledgement transaction.
        let ids: Vec<_> = items.iter().map(|i| i.event.as_str()).collect();
        let deleted: Vec<_> = items
            .iter()
            .filter(|i| i.deleting)
            .map(|i| i.event.as_str())
            .collect();
        let mut input = Vec::with_capacity(size);
        for item in items.iter().filter(|i| !i.deleting) {
            input.extend_from_slice(&item.payload);
            input.push(b'\n');
        }
        if !input.is_empty() {
            self.command(
                &["import".into(), "--debounce-millis=0".into()],
                input,
                65536,
            )
            .await?;
        }
        if !deleted.is_empty() {
            self.command(
                &[
                    "delete".into(),
                    "--filter".into(),
                    json!({"ids":deleted}).to_string(),
                ],
                vec![],
                65536,
            )
            .await?;
        }
        let output = self
            .command(
                &["scan".into(), json!({"ids":ids}).to_string()],
                vec![],
                2 * 1048576,
            )
            .await?;
        let mut found = HashSet::new();
        for line in output
            .split(|b| *b == b'\n')
            .filter(|line| !line.is_empty())
        {
            let event: Event = serde_json::from_slice(line)?;
            event.verify().map_err(anyhow::Error::msg)?;
            ensure!(
                ids.contains(&event.id.as_str()),
                "backend returned unrelated event"
            );
            found.insert(event.id);
        }
        // This detached session owns the advisory projection lock. A lost
        // session cannot acquire a replacement connection to acknowledge work.
        // Imports need no author retirement lock: acknowledge the verified set
        // atomically, fenced against concurrent withdrawal/version changes.
        let imports: Vec<_> = items
            .iter()
            .filter(|item| !item.deleting && found.contains(&item.event))
            .collect();
        let mut complete = 0;
        if !imports.is_empty() {
            let jobs: Vec<i64> = imports.iter().map(|item| item.id).collect();
            let versions: Vec<i64> = imports.iter().map(|item| item.version).collect();
            complete += client.execute(
                "UPDATE backend_outbox o SET completed_at=clock_timestamp() FROM events e,unnest($1::bigint[],$2::bigint[]) verified(id,version) WHERE o.id=verified.id AND o.event_version=verified.version AND o.action='import' AND o.completed_at IS NULL AND e.id=o.event_id AND e.version=verified.version AND e.state='hosted'",
                &[&jobs, &versions],
            ).await? as usize;
        }
        let retirements: Vec<_> = items
            .iter()
            .filter(|item| item.deleting && !found.contains(&item.event))
            .collect();
        // Bound actual retirement work, not arbitrary import items or only the
        // distinct-author count. Four items imply at most four author locks even
        // when many old generations belong to one author.
        for batch in retirements.chunks(4) {
            let tx = client.transaction().await?;
            let ids: Vec<Vec<u8>> = batch
                .iter()
                .map(|item| hex::decode(&item.event))
                .collect::<std::result::Result<_, _>>()?;
            let authors = tx.query(&tx.prepare_cached(
                "SELECT DISTINCT publisher FROM events WHERE id=ANY($1) AND kind=11999 ORDER BY publisher"
            ).await?, &[&ids]).await?;
            for author in authors {
                let publisher: Vec<u8> = author.get(0);
                tx.query_one(
                    &tx.prepare_cached(
                        "SELECT publisher FROM account_permissions WHERE publisher=$1 FOR UPDATE",
                    )
                    .await?,
                    &[&publisher],
                )
                .await?;
            }
            for item in batch {
                let id = hex::decode(&item.event)?;
                // Withdraw the last public pathname before acknowledgement.
                // Existing reader holds continue to prevent physical reclaim.
                let export_pending:bool=tx.query_one(&tx.prepare_cached("SELECT EXISTS(SELECT 1 FROM publication_blobs p JOIN public_export_holds h ON h.hash=p.hash AND h.generation=p.generation WHERE p.event_id=$1 AND h.state IN ('creating','active') AND NOT EXISTS(SELECT 1 FROM publication_blobs other JOIN events e ON e.id=other.event_id WHERE other.hash=p.hash AND other.generation=p.generation AND e.id<>p.event_id AND e.state='hosted' AND NOT EXISTS(SELECT 1 FROM publication_blobs bad JOIN quarantined_blobs q ON q.hash=bad.hash AND q.generation=bad.generation WHERE bad.event_id=e.id) AND NOT EXISTS(SELECT 1 FROM backend_outbox pending WHERE pending.event_id=e.id AND pending.completed_at IS NULL)))").await?,&[&id]).await?.get(0);
                if export_pending {
                    continue;
                }
                let changed = tx.execute(&tx.prepare_cached(
                    "UPDATE events SET state='removed',retired_at=CASE WHEN kind=11999 THEN clock_timestamp() ELSE retired_at END WHERE id=$1 AND version=$2 AND state='withdrawing'"
                ).await?, &[&id, &item.version]).await?;
                if changed == 1 {
                    crate::library::retire(&tx, &id).await?;
                    crate::reclamation::retire_publication(&tx, &id).await?;
                    tx.execute(
                        &tx.prepare_cached("DELETE FROM publication_blobs WHERE event_id=$1")
                            .await?,
                        &[&id],
                    )
                    .await?;
                    complete += tx.execute(&tx.prepare_cached(
                        "UPDATE backend_outbox SET completed_at=clock_timestamp() WHERE id=$1 AND event_version=$2 AND action='delete' AND completed_at IS NULL"
                    ).await?, &[&item.id, &item.version]).await? as usize;
                }
            }
            tx.commit().await?;
        }
        ensure!(
            complete == items.len(),
            "backend failed to persist every expected current event"
        );
        Ok(complete)
    }
    pub async fn run(self: Arc<Self>) {
        loop {
            if let Err(error) = self.session().await {
                tracing::warn!(error=%error,"strfry projection pending; retrying");
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    async fn session(&self) -> Result<()> {
        // Detach one connection so dropping this scope closes it and releases its
        // session lock; a locked connection is never returned to the shared pool.
        let mut client = deadpool_postgres::Object::take(self.repo.pool.get().await?);
        let acquired: bool = client
            .query_one("SELECT pg_try_advisory_lock(697241114)", &[])
            .await?
            .get(0);
        ensure!(acquired, "another projector owns the database outbox");
        loop {
            if self.batch(&mut client).await? == 0 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

#[cfg(test)]
mod owned_command_tests {
    use super::*;
    #[tokio::test]
    async fn failed_leader_cannot_leave_writable_process_group_descendant_running() {
        use tokio::io::AsyncBufReadExt;
        let mut command = tokio::process::Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("sleep 30 & printf '%s\n' $!; wait")
            .process_group(0)
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut owned = OwnedCommand {
            child: command.spawn().unwrap(),
            embedded: true,
        };
        let pid = owned.child.id().unwrap() as i32;
        let mut line = String::new();
        tokio::time::timeout(
            Duration::from_secs(1),
            tokio::io::BufReader::new(owned.child.stdout.take().unwrap()).read_line(&mut line),
        )
        .await
        .unwrap()
        .unwrap();
        let descendant = line.trim().parse::<i32>().unwrap();
        assert_eq!(unsafe { libc::getpgid(descendant) }, pid);
        assert!(process_group_running(pid).unwrap());
        // Synthetic timeout/error: only the supervisor leader exits first.
        assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
        let status = tokio::time::timeout(Duration::from_secs(2), owned.wait_terminal())
            .await
            .unwrap()
            .unwrap();
        assert!(!status.success());
        assert!(!process_group_running(pid).unwrap());
        assert!(owned.child.id().is_none());
    }
}
