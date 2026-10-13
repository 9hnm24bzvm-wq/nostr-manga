//! Local client outbox: public payloads and outcomes only. One process owns a
//! directory lock; atomic replacement + directory fsync makes retries recoverable.
use crate::{
    auth,
    library_client::{Outcome, Recipient},
    library_client_bundle::SaveBundle,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};
const STATE_LIMIT: usize = 128 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipientState {
    pub recipient: Recipient,
    pub attempts: u32,
    pub outcome: Option<Outcome>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub version: u32,
    pub event: auth::Event,
    pub recipients: Vec<RecipientState>,
    pub unresolved_relays: Vec<String>,
}
#[derive(Clone, Copy)]
pub struct OutboxLimits {
    pub jobs: usize,
    pub bytes: u64,
    pub attempts: u32,
}
impl Default for OutboxLimits {
    fn default() -> Self {
        Self {
            jobs: 16,
            bytes: 512 * 1024 * 1024,
            attempts: 8,
        }
    }
}
pub struct Outbox {
    root: PathBuf,
    _lock: File,
    limits: OutboxLimits,
    active: std::collections::BTreeSet<String>,
}
fn read_regular(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path)?;
    ensure!(
        meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= limit as u64,
        "outbox regular file/size required"
    );
    let file = File::open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let opened = file.metadata()?;
        ensure!(
            opened.dev() == meta.dev() && opened.ino() == meta.ino(),
            "outbox file changed"
        );
    }
    let mut bytes = vec![];
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "outbox read budget");
    Ok(bytes)
}
fn new_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = File::options();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
fn synced_dir(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}
impl Outbox {
    /// Use an owner-controlled local directory. Network filesystems and hostile
    /// concurrent directory replacement are outside this filesystem contract.
    pub fn open(root: &Path, limits: OutboxLimits) -> Result<Self> {
        ensure!(
            (1..=64).contains(&limits.jobs)
                && limits.bytes > STATE_LIMIT as u64
                && limits.bytes <= 16 * 1024 * 1024 * 1024
                && (1..=32).contains(&limits.attempts),
            "outbox limits"
        );
        fs::create_dir_all(root)?;
        let meta = fs::symlink_metadata(root)?;
        ensure!(
            meta.is_dir() && !meta.file_type().is_symlink(),
            "outbox directory required"
        );
        let root = root.canonicalize()?;
        let lock_path = root.join(".lock");
        if let Ok(meta) = fs::symlink_metadata(&lock_path) {
            ensure!(
                meta.is_file() && !meta.file_type().is_symlink(),
                "outbox lock regular file required"
            );
        }
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        // A concurrently spawning process can briefly inherit an open lock FD
        // until exec closes it. Bound this local retry; never steal a live lock.
        let started = std::time::Instant::now();
        loop {
            match lock.try_lock() {
                Ok(()) => break,
                Err(error) if started.elapsed() >= std::time::Duration::from_millis(100) => {
                    return Err(anyhow::Error::new(error)
                        .context("outbox already owned or locking unsupported"));
                }
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(5)),
            }
        }
        synced_dir(&root)?;
        Ok(Self {
            root,
            _lock: lock,
            limits,
            active: Default::default(),
        })
    }
    fn path(&self, id: &str) -> Result<PathBuf> {
        ensure!(auth::valid_hash(id), "outbox event ID");
        let path = self.root.join(id);
        if path.exists() {
            let m = fs::symlink_metadata(&path)?;
            ensure!(
                m.is_dir() && !m.file_type().is_symlink(),
                "outbox job directory required"
            );
        }
        Ok(path)
    }
    fn used(&self) -> Result<(usize, u64)> {
        let mut jobs = 0;
        let mut bytes = 0u64;
        let mut entries = 0;
        let mut directories = vec![self.root.clone()];
        while let Some(dir) = directories.pop() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                entries += 1;
                ensure!(entries <= 65536, "outbox entry budget");
                let meta = fs::symlink_metadata(entry.path())?;
                ensure!(!meta.file_type().is_symlink(), "outbox symlink rejected");
                if meta.is_dir() {
                    if entry.path().parent() == Some(self.root.as_path()) {
                        jobs += 1;
                    }
                    directories.push(entry.path());
                } else {
                    ensure!(meta.is_file(), "outbox special file rejected");
                    bytes = bytes
                        .checked_add(meta.len())
                        .context("outbox byte overflow")?;
                }
            }
        }
        Ok((jobs, bytes))
    }
    pub fn enqueue(
        &mut self,
        bundle: &SaveBundle,
        recipients: Vec<Recipient>,
        unresolved_relays: Vec<String>,
    ) -> Result<String> {
        ensure!(
            !recipients.is_empty() && recipients.len() <= 32 && unresolved_relays.len() <= 32,
            "outbox recipient budget"
        );
        let mut destinations = std::collections::BTreeSet::new();
        for recipient in &recipients {
            ensure!(
                crate::endpoint_policy::canonical_origin(&recipient.endpoint)?
                    == recipient.endpoint,
                "canonical recipient endpoint required"
            );
            ensure!(
                destinations.insert((recipient.endpoint.clone(), recipient.service_key)),
                "duplicate recipient binding"
            );
        }
        for route in recipients
            .iter()
            .map(|r| &r.relay_url)
            .chain(unresolved_relays.iter())
        {
            ensure!(route.len() <= 2048, "relay route budget");
            let url = url::Url::parse(route)?;
            ensure!(
                matches!(url.scheme(), "ws" | "wss")
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.fragment().is_none(),
                "invalid relay route"
            );
        }
        let id = &bundle.event().id;
        let destination = self.path(id)?;
        if destination.exists() {
            let old = self.load(id)?;
            ensure!(
                serde_json::to_vec(
                    &old.recipients
                        .iter()
                        .map(|r| &r.recipient)
                        .collect::<Vec<_>>()
                )? == serde_json::to_vec(&recipients)?
                    && old.unresolved_relays == unresolved_relays,
                "existing job destinations differ"
            );
            self.bundle(id)?;
            return Ok(id.clone());
        }
        let job = Job {
            version: 1,
            event: bundle.event().clone(),
            recipients: recipients
                .into_iter()
                .map(|recipient| RecipientState {
                    recipient,
                    attempts: 0,
                    outcome: None,
                })
                .collect(),
            unresolved_relays,
        };
        let state = serde_json::to_vec(&job)?;
        ensure!(state.len() <= STATE_LIMIT, "outbox state budget");
        let incoming = bundle
            .objects()
            .map(|(_, _, bytes)| bytes.len() as u64)
            .sum::<u64>()
            + state.len() as u64;
        self.prune_completed(1)?;
        let (jobs, used) = self.used()?;
        ensure!(
            jobs < self.limits.jobs
                && used
                    .checked_add(incoming + STATE_LIMIT as u64)
                    .is_some_and(|total| total <= self.limits.bytes),
            "outbox capacity exhausted"
        );
        let temporary = self.root.join(format!(
            ".pending-{}",
            hex::encode(rand::random::<[u8; 16]>())
        ));
        fs::create_dir(&temporary)?;
        for (_, hash, bytes) in bundle.objects() {
            new_file(&temporary.join(hex::encode(hash)), bytes)?;
        }
        new_file(&temporary.join("state.json"), &state)?;
        synced_dir(&temporary)?;
        fs::rename(&temporary, &destination)?;
        synced_dir(&self.root)?;
        Ok(id.clone())
    }
    /// Retain the newest completed saves, never discard pending/rejected work.
    /// Retirement is explicit on disk so interrupted cleanup can be retried.
    pub fn prune_completed(&mut self, keep: usize) -> Result<usize> {
        ensure!(keep <= 64, "completed retention budget");
        let mut completed = vec![];
        let mut retired = vec![];
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(id) = name.strip_prefix(".completed-") {
                ensure!(auth::valid_hash(id), "invalid retirement marker");
                retired.push(entry.path());
                continue;
            }
            if !auth::valid_hash(&name) || self.active.contains(&name) {
                continue;
            }
            let job = self.load(&name)?;
            if job
                .recipients
                .iter()
                .all(|state| matches!(state.outcome, Some(Outcome::Accepted { .. })))
            {
                completed.push((job.event.created_at, name));
            }
        }
        completed.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        for (_, id) in completed.into_iter().skip(keep) {
            let old = self.path(&id)?;
            let target = self.root.join(format!(".completed-{id}"));
            ensure!(!target.exists(), "retirement marker collision");
            fs::rename(old, &target)?;
            synced_dir(&self.root)?;
            retired.push(target);
        }
        let count = retired.len();
        for path in retired {
            let meta = fs::symlink_metadata(&path)?;
            ensure!(
                meta.is_dir() && !meta.file_type().is_symlink(),
                "retired directory required"
            );
            for entry in fs::read_dir(&path)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                let meta = fs::symlink_metadata(entry.path())?;
                ensure!(
                    meta.is_file()
                        && !meta.file_type().is_symlink()
                        && (auth::valid_hash(&name)
                            || name == "state.json"
                            || name.starts_with(".state-")),
                    "unexpected retired file"
                );
            }
            fs::remove_dir_all(path)?;
            synced_dir(&self.root)?;
        }
        Ok(count)
    }

    pub fn load(&self, id: &str) -> Result<Job> {
        let job: Job = serde_json::from_slice(&read_regular(
            &self.path(id)?.join("state.json"),
            STATE_LIMIT,
        )?)?;
        ensure!(
            job.version == 1
                && job.event.id == id
                && !job.recipients.is_empty()
                && job.recipients.len() <= 32
                && job.unresolved_relays.len() <= 32,
            "outbox schema/budget"
        );
        crate::library_manifest::pointer(&job.event)?;
        for state in &job.recipients {
            if let Some(Outcome::Accepted { event_id, .. } | Outcome::Superseded { event_id }) =
                &state.outcome
            {
                ensure!(event_id == id, "outbox receipt event mismatch");
            }
        }
        Ok(job)
    }
    pub fn bundle(&self, id: &str) -> Result<SaveBundle> {
        let job = self.load(id)?;
        let path = self.path(id)?;
        let root_hash = crate::library_manifest::pointer(&job.event)?;
        let root_bytes = read_regular(&path.join(hex::encode(root_hash)), 256 * 1024)?;
        let root = crate::library_manifest::Root::decode(
            &root_bytes,
            &crate::library_manifest::Limits::default(),
        )?;
        let hashes: std::collections::BTreeSet<_> = root
            .families
            .iter()
            .flat_map(|f| f.chunks.iter())
            .filter(|d| !d.is_empty())
            .map(|d| d.hash)
            .collect();
        let mut chunks = BTreeMap::new();
        let mut total = root_bytes.len();
        for hash in hashes {
            let bytes = read_regular(&path.join(hex::encode(hash)), 1_000_000)?;
            total = total
                .checked_add(bytes.len())
                .context("outbox payload overflow")?;
            ensure!(
                total <= 256 * 1024 * 1024 + 256 * 1024,
                "outbox graph budget"
            );
            chunks.insert(hash, bytes);
        }
        SaveBundle::validate(job.event, root_bytes, chunks)
    }
    fn save(&self, job: &Job) -> Result<()> {
        let path = self.path(&job.event.id)?;
        let bytes = serde_json::to_vec(job)?;
        ensure!(bytes.len() <= STATE_LIMIT, "outbox state budget");
        let (_, used) = self.used()?;
        ensure!(
            used.checked_add(bytes.len() as u64)
                .is_some_and(|n| n <= self.limits.bytes),
            "outbox update capacity exhausted"
        );
        let temporary = path.join(format!(
            ".state-{}",
            hex::encode(rand::random::<[u8; 16]>())
        ));
        new_file(&temporary, &bytes)?;
        fs::rename(&temporary, path.join("state.json"))?;
        synced_dir(&path)
    }
    /// Persist attempt numbers before network work; a resumed process may retry
    /// only unfinished recipients. Late results from an older attempt are fenced.
    pub fn begin(&mut self, id: &str) -> Result<Vec<(usize, u32, Recipient)>> {
        let mut job = self.load(id)?;
        let mut pending = vec![];
        for (index, state) in job.recipients.iter_mut().enumerate() {
            if matches!(state.outcome, None | Some(Outcome::Retryable { .. })) {
                state.attempts = state
                    .attempts
                    .checked_add(1)
                    .context("attempt counter exhausted")?;
                pending.push((index, state.attempts, state.recipient.clone()));
            }
        }
        if !pending.is_empty() {
            self.save(&job)?;
        }
        Ok(pending)
    }
    pub fn record(
        &mut self,
        id: &str,
        index: usize,
        attempt: u32,
        outcome: Outcome,
    ) -> Result<bool> {
        ensure!(attempt > 0, "attempt must have begun");
        let mut job = self.load(id)?;
        let state = job
            .recipients
            .get_mut(index)
            .context("outbox recipient index")?;
        if state.attempts != attempt
            || matches!(
                state.outcome,
                Some(
                    Outcome::Accepted { .. }
                        | Outcome::Superseded { .. }
                        | Outcome::Rejected { .. }
                )
            )
        {
            return Ok(false);
        }
        if let Outcome::Accepted { event_id, .. } | Outcome::Superseded { event_id } = &outcome {
            ensure!(event_id == id, "receipt targets another save");
        }
        state.outcome = Some(outcome);
        self.save(&job)?;
        Ok(true)
    }
}

pub struct PersistentDelivery {
    pub first_acceptance: tokio::sync::oneshot::Receiver<crate::library_client::RecipientResult>,
    pub updates: tokio::sync::mpsc::Receiver<crate::library_client::RecipientResult>,
    pub completion: tokio::task::JoinHandle<Result<Job>>,
}
struct ActiveJob {
    outbox: std::sync::Arc<std::sync::Mutex<Outbox>>,
    id: String,
}
impl Drop for ActiveJob {
    fn drop(&mut self) {
        if let Ok(mut outbox) = self.outbox.lock() {
            outbox.active.remove(&self.id);
        }
    }
}
struct AbortNetwork(tokio::task::AbortHandle);
impl Drop for AbortNetwork {
    fn drop(&mut self) {
        self.0.abort();
    }
}
/// Resume pending recipients with bounded retries. A success is emitted only
/// after its outcome is fsynced. Signers and authorization tokens stay in memory.
pub async fn run_job(
    outbox: std::sync::Arc<std::sync::Mutex<Outbox>>,
    id: String,
    signer: std::sync::Arc<dyn crate::library_client::Signer>,
    policy: crate::endpoint_policy::EndpointPolicy,
    limits: crate::library_client::DeliveryLimits,
) -> Result<PersistentDelivery> {
    use std::sync::Arc;
    let storage = outbox.clone();
    let job_id = id.clone();
    ensure!(
        (1..=8).contains(&limits.concurrency)
            && limits.recipient_timeout >= std::time::Duration::from_secs(1)
            && limits.recipient_timeout <= std::time::Duration::from_secs(600),
        "delivery limits"
    );
    let (bundle, existing, guard, max_rounds) =
        tokio::task::spawn_blocking(move || -> Result<_> {
            let mut store = storage
                .lock()
                .map_err(|_| anyhow::anyhow!("outbox lock poisoned"))?;
            ensure!(!store.active.contains(&job_id), "job already running");
            let bundle = store.bundle(&job_id)?;
            let job = store.load(&job_id)?;
            store.active.insert(job_id.clone());
            let guard = ActiveJob {
                outbox: storage.clone(),
                id: job_id,
            };
            Ok((Arc::new(bundle), job, guard, store.limits.attempts))
        })
        .await??;
    let (first_tx, first_acceptance) = tokio::sync::oneshot::channel();
    let (updates_tx, updates) = tokio::sync::mpsc::channel(1024);
    let completion = tokio::spawn(async move {
        let _guard = guard;
        let mut first = Some(first_tx);
        for (index, state) in existing.recipients.iter().enumerate() {
            if let Some(outcome @ Outcome::Accepted { .. }) = &state.outcome
                && let Some(sender) = first.take()
            {
                let _ = sender.send(crate::library_client::RecipientResult {
                    recipient: index,
                    outcome: outcome.clone(),
                });
            }
        }
        let mut round = 0u32;
        loop {
            let storage = outbox.clone();
            let job_id = id.clone();
            let pending = tokio::task::spawn_blocking(move || {
                storage
                    .lock()
                    .map_err(|_| anyhow::anyhow!("outbox lock poisoned"))?
                    .begin(&job_id)
            })
            .await??;
            if pending.is_empty() {
                break;
            }
            let recipients = pending
                .iter()
                .map(|(_, _, recipient)| recipient.clone())
                .collect();
            let mut delivery = crate::library_client::deliver(
                bundle.clone(),
                recipients,
                signer.clone(),
                policy.clone(),
                limits,
            )?;
            let _abort = AbortNetwork(delivery.completion.abort_handle());
            while let Some(mut result) = delivery.updates.recv().await {
                let (index, attempt, _) = pending
                    .get(result.recipient)
                    .context("delivery recipient index")?;
                result.recipient = *index;
                let storage = outbox.clone();
                let job_id = id.clone();
                let outcome = result.outcome.clone();
                let index = *index;
                let attempt = *attempt;
                let recorded = tokio::task::spawn_blocking(move || {
                    storage
                        .lock()
                        .map_err(|_| anyhow::anyhow!("outbox lock poisoned"))?
                        .record(&job_id, index, attempt, outcome)
                })
                .await??;
                ensure!(recorded, "delivery attempt superseded locally");
                if matches!(result.outcome, Outcome::Accepted { .. })
                    && let Some(sender) = first.take()
                {
                    let _ = sender.send(result.clone());
                }
                let _ = updates_tx.try_send(result);
            }
            delivery.completion.await?;
            round += 1;
            let storage = outbox.clone();
            let job_id = id.clone();
            let retry =
                tokio::task::spawn_blocking(move || -> Result<bool> {
                    let store = storage
                        .lock()
                        .map_err(|_| anyhow::anyhow!("outbox lock poisoned"))?;
                    Ok(store.load(&job_id)?.recipients.iter().any(|state| {
                        matches!(state.outcome, None | Some(Outcome::Retryable { .. }))
                    }))
                })
                .await??;
            if !retry || round >= max_rounds {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(
                (1u64 << round.min(5)).min(30),
            ))
            .await;
        }
        let storage = outbox.clone();
        tokio::task::spawn_blocking(move || {
            storage
                .lock()
                .map_err(|_| anyhow::anyhow!("outbox lock poisoned"))?
                .load(&id)
        })
        .await?
    });
    Ok(PersistentDelivery {
        first_acceptance,
        updates,
        completion,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_manifest::{Descriptor, Family, FamilyRoot, Limits, Root};
    fn bundle() -> SaveBundle {
        let root = Root {
            families: Family::ALL
                .into_iter()
                .map(|family| FamilyRoot {
                    family,
                    chunks: vec![Descriptor::empty()],
                })
                .collect(),
        }
        .encode(&Limits::default())
        .unwrap();
        let event = auth::sign(
            &format!("{:064x}", 2),
            11999,
            vec![],
            serde_json::json!({"manifest":auth::hash(&root),"version":1}).to_string(),
        )
        .unwrap();
        SaveBundle::validate(event, root, BTreeMap::new()).unwrap()
    }
    fn recipient() -> Recipient {
        Recipient {
            relay_url: "wss://relay.example/".into(),
            endpoint: "https://relay.example".into(),
            service_key: [7; 32],
        }
    }
    #[test]
    fn durable_reopen_attempt_fences_and_terminal_outcomes() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let bundle = bundle();
        let mut outbox = Outbox::open(dir.path(), OutboxLimits::default())?;
        assert!(Outbox::open(dir.path(), OutboxLimits::default()).is_err());
        let id = outbox.enqueue(&bundle, vec![recipient()], vec![])?;
        assert_eq!(outbox.enqueue(&bundle, vec![recipient()], vec![])?, id);
        assert!(outbox.load("../state.json").is_err());
        let first = outbox.begin(&id)?;
        assert_eq!(first[0].1, 1);
        drop(outbox);
        let mut outbox = Outbox::open(dir.path(), OutboxLimits::default())?;
        assert_eq!(outbox.bundle(&id)?.event().id, id);
        let next = outbox.begin(&id)?;
        assert_eq!(next[0].1, 2);
        let accepted = Outcome::Accepted {
            event_id: id.clone(),
            projected: false,
        };
        assert!(!outbox.record(&id, 0, 1, accepted.clone())?);
        assert!(outbox.record(&id, 0, 2, accepted)?);
        assert!(!outbox.record(
            &id,
            0,
            2,
            Outcome::Retryable {
                reason: "late failure".into()
            }
        )?);
        assert!(outbox.begin(&id)?.is_empty());
        drop(outbox);
        let outbox = Outbox::open(dir.path(), OutboxLimits::default())?;
        assert!(matches!(
            outbox.load(&id)?.recipients[0].outcome,
            Some(Outcome::Accepted { .. })
        ));
        let root_hash = crate::library_manifest::pointer(bundle.event())?;
        fs::write(
            dir.path().join(&id).join(hex::encode(root_hash)),
            b"corrupt",
        )?;
        assert!(outbox.bundle(&id).is_err());
        Ok(())
    }
    #[test]
    #[ignore = "child-process harness; invoked by the crash parent"]
    fn outbox_child_process() -> Result<()> {
        let path = std::env::var("MANGA_OUTBOX_TEST_PATH")?;
        let id = std::env::var("MANGA_OUTBOX_TEST_ID")?;
        let mut outbox = Outbox::open(Path::new(&path), OutboxLimits::default())?;
        outbox.begin(&id)?;
        fs::write(Path::new(&path).join("child-ready"), b"durable")?;
        std::thread::sleep(std::time::Duration::from_secs(60));
        Ok(())
    }
    #[test]
    fn killed_process_releases_lock_and_preserves_pending_attempt() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let bundle = bundle();
        let mut outbox = Outbox::open(dir.path(), OutboxLimits::default())?;
        let id = outbox.enqueue(&bundle, vec![recipient()], vec![])?;
        drop(outbox);
        let mut child = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "library_client_outbox::tests::outbox_child_process",
                "--nocapture",
                "--ignored",
            ])
            .env("MANGA_OUTBOX_TEST_PATH", dir.path())
            .env("MANGA_OUTBOX_TEST_ID", &id)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        let started = std::time::Instant::now();
        while !dir.path().join("child-ready").exists()
            && started.elapsed() < std::time::Duration::from_secs(5)
        {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let ready = dir.path().join("child-ready").exists();
        let _ = child.kill();
        let _ = child.wait();
        assert!(ready, "child must persist before crash");
        let mut outbox = Outbox::open(dir.path(), OutboxLimits::default())?;
        assert_eq!(outbox.load(&id)?.recipients[0].attempts, 1);
        assert!(outbox.load(&id)?.recipients[0].outcome.is_none());
        assert_eq!(outbox.begin(&id)?[0].1, 2);
        Ok(())
    }
    #[test]
    fn completed_retention_never_prunes_unfinished_jobs() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let first = bundle();
        let root = first
            .objects()
            .find(|(kind, _, _)| *kind == "root")
            .unwrap()
            .2
            .to_vec();
        let event = auth::sign(
            &format!("{:064x}", 3),
            11999,
            vec![],
            first.event().content.clone(),
        )
        .unwrap();
        let second = SaveBundle::validate(event, root, BTreeMap::new())?;
        let mut outbox = Outbox::open(dir.path(), OutboxLimits::default())?;
        let old = outbox.enqueue(&first, vec![recipient()], vec![])?;
        let attempt = outbox.begin(&old)?[0].1;
        outbox.record(
            &old,
            0,
            attempt,
            Outcome::Accepted {
                event_id: old.clone(),
                projected: true,
            },
        )?;
        let pending = outbox.enqueue(&second, vec![recipient()], vec![])?;
        assert_eq!(outbox.prune_completed(0)?, 1);
        assert!(outbox.load(&old).is_err());
        assert_eq!(outbox.bundle(&pending)?.event().id, pending);
        // A crash after retirement rename is completed by the next cleanup.
        let attempt = outbox.begin(&pending)?[0].1;
        outbox.record(
            &pending,
            0,
            attempt,
            Outcome::Accepted {
                event_id: pending.clone(),
                projected: true,
            },
        )?;
        fs::rename(
            dir.path().join(&pending),
            dir.path().join(format!(".completed-{pending}")),
        )?;
        assert_eq!(outbox.prune_completed(0)?, 1);
        assert!(outbox.load(&pending).is_err());
        assert_eq!(outbox.prune_completed(0)?, 0);
        Ok(())
    }
    #[test]
    fn storage_attempt_and_binding_budgets_are_enforced() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let bundle = bundle();
        let mut box1 = Outbox::open(
            dir.path(),
            OutboxLimits {
                jobs: 1,
                bytes: 131_073,
                attempts: 1,
            },
        )?;
        assert!(box1.enqueue(&bundle, vec![recipient()], vec![]).is_err());
        drop(box1);
        let mut box1 = Outbox::open(
            dir.path(),
            OutboxLimits {
                attempts: 1,
                ..OutboxLimits::default()
            },
        )?;
        assert!(
            box1.enqueue(&bundle, vec![recipient(), recipient()], vec![])
                .is_err()
        );
        let id = box1.enqueue(&bundle, vec![recipient()], vec![])?;
        let attempts = box1.begin(&id)?;
        box1.record(
            &id,
            0,
            attempts[0].1,
            Outcome::Retryable {
                reason: "offline".into(),
            },
        )?;
        assert_eq!(
            box1.begin(&id)?[0].1,
            2,
            "a later run may retry unfinished work with a new fenced attempt"
        );
        assert!(
            matches!(
                box1.load(&id)?.recipients[0].outcome,
                Some(Outcome::Retryable { .. })
            ),
            "budget exhaustion does not fabricate acceptance"
        );
        Ok(())
    }
}
