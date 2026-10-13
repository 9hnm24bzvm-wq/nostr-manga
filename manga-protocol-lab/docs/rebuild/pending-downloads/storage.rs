//! Generation-bound filesystem receipts. Every blocking mutation owns both its
//! hash guard and volume file lock until I/O ends, even if its async caller drops.
use crate::db::Id;
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock as AsyncRwLock};
pub type Generation = [u8; 16];
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobKey {
    pub hash: Id,
    pub generation: Generation,
}
#[derive(Clone, Debug)]
pub struct DurableReceipt {
    key: BlobKey,
    size: u64,
}
impl DurableReceipt {
    pub fn key(&self) -> &BlobKey {
        &self.key
    }
    pub fn size(&self) -> u64 {
        self.size
    }
}
#[derive(Debug)]
pub struct CleanupReceipt {
    key: BlobKey,
}
impl CleanupReceipt {
    pub fn key(&self) -> &BlobKey {
        &self.key
    }
}
/// Minted only after every supported public alias is unlinked and its directory
/// fsynced. This does not prove that external readers have released the inode.
#[derive(Debug)]
pub(crate) struct PublicWithdrawal {
    key: BlobKey,
    root_identity: Id,
}
impl PublicWithdrawal {
    pub(crate) fn root_identity(&self) -> Id {
        self.root_identity
    }
    pub(crate) fn key(&self) -> &BlobKey {
        &self.key
    }
}
struct VolumeInner {
    root: PathBuf,
    _owner: fs::File,
    identity: Id,
}
pub struct FilesystemVolume {
    inner: Arc<VolumeInner>,
    locks: Mutex<HashMap<Id, Weak<AsyncRwLock<()>>>>,
    io_slots: Arc<tokio::sync::Semaphore>,
}
pub(crate) struct Observation<'a> {
    volume: &'a FilesystemVolume,
    hash: Id,
    guard: Arc<OwnedRwLockReadGuard<()>>,
}
impl Observation<'_> {
    pub(crate) async fn withdraw_public(&self, key: &BlobKey) -> Result<PublicWithdrawal> {
        ensure!(key.hash == self.hash, "observation hash mismatch");
        let pin = self.guard.clone();
        let key = key.clone();
        let permit = self.volume.io_slots.clone().acquire_owned().await?;
        let inner = self.volume.inner.clone();
        tokio::task::spawn_blocking(move || {
            let _pin = pin;
            let _permit = permit;
            inner.withdraw_public(key)
        })
        .await?
    }

    pub(crate) async fn read_verified(
        &self,
        key: &BlobKey,
        expected: u64,
        limit: u64,
    ) -> Result<Vec<u8>> {
        ensure!(key.hash == self.hash, "observation hash mismatch");
        let pin = self.guard.clone();
        let key = key.clone();
        let permit = self.volume.io_slots.clone().acquire_owned().await?;
        let inner = self.volume.inner.clone();
        tokio::task::spawn_blocking(move || {
            let _pin = pin;
            let _permit = permit;
            inner.read_verified(&key, expected, limit)
        })
        .await?
    }
}
pub struct Mutation<'a> {
    volume: &'a FilesystemVolume,
    hash: Id,
    guard: Arc<OwnedRwLockWriteGuard<()>>,
}
impl FilesystemVolume {
    pub fn open(root: PathBuf) -> Result<Self> {
        Self::open_with_limit(root, 4)
    }
    pub fn open_with_limit(root: PathBuf, io_slots: usize) -> Result<Self> {
        ensure!((1..=16).contains(&io_slots), "filesystem I/O slot bounds");
        fs::create_dir_all(root.join("objects"))?;
        let root = root.canonicalize()?;
        let owner = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join(".volume-owner"))?;
        owner
            .try_lock()
            .context("storage volume already has a mutation owner")?;
        use std::os::unix::fs::MetadataExt;
        let metadata = owner.metadata()?;
        let mut identity = Sha256::new();
        identity.update(b"manga-public-volume-v1");
        identity.update(fs::read("/proc/sys/kernel/random/boot_id")?);
        identity.update(metadata.dev().to_be_bytes());
        identity.update(metadata.ino().to_be_bytes());
        identity.update(root.as_os_str().as_encoded_bytes());
        let identity: Id = identity.finalize().into();
        Ok(Self {
            inner: Arc::new(VolumeInner {
                root,
                _owner: owner,
                identity,
            }),
            locks: Mutex::new(HashMap::new()),
            io_slots: Arc::new(tokio::sync::Semaphore::new(io_slots)),
        })
    }
    fn hash_lock(&self, hash: Id) -> Arc<AsyncRwLock<()>> {
        let mut locks = self.locks.lock().unwrap();
        if locks.len() > 256 {
            locks.retain(|_, v| v.strong_count() > 0);
        }
        if let Some(lock) = locks.get(&hash).and_then(Weak::upgrade) {
            lock
        } else {
            let lock = Arc::new(AsyncRwLock::new(()));
            locks.insert(hash, Arc::downgrade(&lock));
            lock
        }
    }
    pub(crate) async fn observation(&self, hash: Id) -> Observation<'_> {
        Observation {
            volume: self,
            hash,
            guard: Arc::new(self.hash_lock(hash).read_owned().await),
        }
    }
    pub fn identity(&self) -> Id {
        self.inner.identity
    }
    pub fn public_path(&self) -> PathBuf {
        self.inner.root.join("public")
    }
    pub async fn mutation(&self, hash: Id) -> Mutation<'_> {
        Mutation {
            volume: self,
            hash,
            guard: Arc::new(self.hash_lock(hash).write_owned().await),
        }
    }
    async fn read_blocking<T: Send + 'static>(
        &self,
        hash: Id,
        operation: impl FnOnce(Arc<VolumeInner>) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let pin = self.hash_lock(hash).read_owned().await;
        let permit = self.io_slots.clone().acquire_owned().await?;
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let _pin = pin;
            let _permit = permit;
            operation(inner)
        })
        .await?
    }
    #[cfg(test)]
    fn object_path(&self, key: &BlobKey) -> PathBuf {
        self.inner.object_path(key)
    }
    pub async fn read_verified(&self, key: &BlobKey, expected: u64, limit: u64) -> Result<Vec<u8>> {
        let key = key.clone();
        self.read_blocking(key.hash, move |inner| {
            inner.read_verified(&key, expected, limit)
        })
        .await
    }
}
impl VolumeInner {
    fn object_path(&self, key: &BlobKey) -> PathBuf {
        let hash = hex::encode(key.hash);
        self.root
            .join("objects")
            .join(&hash[..2])
            .join(&hash[2..4])
            .join(format!("{}.{}", hash, hex::encode(key.generation)))
    }
    fn partial_path(&self, key: &BlobKey) -> PathBuf {
        self.object_path(key)
            .with_extension(format!("{}.partial", hex::encode(key.generation)))
    }
    fn sync_ancestors(&self, parent: &Path) -> Result<()> {
        let mut current = parent;
        loop {
            fs::File::open(current)?.sync_all()?;
            if current == self.root {
                break;
            }
            current = current.parent().context("storage path escaped root")?;
            ensure!(current.starts_with(&self.root), "storage path escaped root");
        }
        Ok(())
    }
    fn read_verified(&self, key: &BlobKey, expected: u64, limit: u64) -> Result<Vec<u8>> {
        ensure!(expected <= limit, "blob size budget");
        let path = self.object_path(key);
        ensure!(
            fs::symlink_metadata(&path)?.file_type().is_file(),
            "blob is not a regular file"
        );
        // Allocate exactly the trusted size plus a one-byte excess probe. Never
        // scan up to the configured blob maximum for a small expected object.
        let capacity = usize::try_from(expected.checked_add(1).context("blob size overflow")?)?;
        let mut bytes = vec![0; capacity];
        let mut file = fs::File::open(path)?;
        let mut used = 0;
        while used < capacity {
            let count = file.read(&mut bytes[used..])?;
            if count == 0 { break; }
            used += count;
        }
        bytes.truncate(used);
        ensure!(bytes.len() as u64 == expected, "blob size mismatch");
        ensure!(
            <[u8; 32]>::from(Sha256::digest(&bytes)) == key.hash,
            "blob hash mismatch"
        );
        Ok(bytes)
    }
    fn recover(&self, key: BlobKey, expected: u64, limit: u64) -> Result<DurableReceipt> {
        self.read_verified(&key, expected, limit)?;
        let path = self.object_path(&key);
        fs::File::open(&path)?.sync_all()?;
        self.sync_ancestors(path.parent().unwrap())?;
        Ok(DurableReceipt {
            key,
            size: expected,
        })
    }
    fn put(&self, key: BlobKey, bytes: &[u8], limit: u64) -> Result<DurableReceipt> {
        ensure!(bytes.len() as u64 <= limit, "blob size budget");
        ensure!(
            <[u8; 32]>::from(Sha256::digest(bytes)) == key.hash,
            "blob hash mismatch"
        );
        let path = self.object_path(&key);
        let parent = path.parent().unwrap();
        fs::create_dir_all(parent)?;
        if path.try_exists()? {
            return self.recover(key, bytes.len() as u64, limit);
        }
        let partial = self.partial_path(&key);
        match fs::remove_file(&partial) {
            Ok(()) => self.sync_ancestors(parent)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&partial)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&partial, &path)?;
        self.sync_ancestors(parent)?;
        Ok(DurableReceipt {
            key,
            size: bytes.len() as u64,
        })
    }
    fn link_public(&self, receipt: &DurableReceipt, mime: &str) -> Result<()> {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt};
        let extension = match mime {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/webp" => "webp",
            _ => anyhow::bail!("unsupported public image MIME"),
        };
        let hash = hex::encode(receipt.key.hash);
        let source = self.object_path(&receipt.key);
        let source_meta = fs::symlink_metadata(&source)?;
        ensure!(
            source_meta.file_type().is_file() && source_meta.len() == receipt.size,
            "verified original changed before export"
        );
        let mut parent = self.root.clone();
        for part in ["public", &hash[..2], &hash[2..4]] {
            parent = parent.join(part);
            match fs::DirBuilder::new().mode(0o700).create(&parent) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
            ensure!(
                fs::symlink_metadata(&parent)?.file_type().is_dir(),
                "public shard is not a directory"
            );
        }
        let target = parent.join(format!("{hash}.{extension}"));
        match fs::hard_link(&source, &target) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let meta = fs::symlink_metadata(&target)?;
                ensure!(
                    meta.file_type().is_file()
                        && meta.dev() == source_meta.dev()
                        && meta.ino() == source_meta.ino(),
                    "public alias belongs to a different inode"
                );
            }
            Err(error) => return Err(error.into()),
        }
        self.sync_ancestors(&parent)?;
        Ok(())
    }
    fn withdraw_public(&self, key: BlobKey) -> Result<PublicWithdrawal> {
        let hash = hex::encode(key.hash);
        let mut parent = self.root.clone();
        for part in ["public", &hash[..2], &hash[2..4]] {
            let next = parent.join(part);
            match fs::symlink_metadata(&next) {
                Ok(meta) => ensure!(meta.file_type().is_dir(), "public shard is not a directory"),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    self.sync_ancestors(&parent)?;
                    return Ok(PublicWithdrawal {
                        key,
                        root_identity: self.identity,
                    });
                }
                Err(error) => return Err(error.into()),
            }
            parent = next;
        }
        for extension in ["png", "jpg", "webp"] {
            match fs::remove_file(parent.join(format!("{hash}.{extension}"))) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        self.sync_ancestors(&parent)?;
        Ok(PublicWithdrawal {
            key,
            root_identity: self.identity,
        })
    }
    fn cleanup(&self, key: BlobKey) -> Result<CleanupReceipt> {
        let path = self.object_path(&key);
        for file in [&path, &self.partial_path(&key)] {
            match fs::remove_file(file) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        if path.parent().unwrap().try_exists()? {
            self.sync_ancestors(path.parent().unwrap())?;
        }
        Ok(CleanupReceipt { key })
    }
}
impl Mutation<'_> {
    async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce(Arc<VolumeInner>, Id) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let permit = self.volume.io_slots.clone().acquire_owned().await?;
        let inner = self.volume.inner.clone();
        let guard = self.guard.clone();
        let hash = self.hash;
        tokio::task::spawn_blocking(move || {
            let _guard = guard;
            let _permit = permit;
            operation(inner, hash)
        })
        .await?
    }
    pub async fn put_verified(
        &self,
        generation: Generation,
        bytes: &[u8],
        limit: u64,
    ) -> Result<DurableReceipt> {
        ensure!(bytes.len() as u64 <= limit, "blob size budget");
        let bytes = bytes.to_vec();
        self.blocking(move |inner, hash| inner.put(BlobKey { hash, generation }, &bytes, limit))
            .await
    }
    pub async fn recover(
        &self,
        generation: Generation,
        expected: u64,
        limit: u64,
    ) -> Result<DurableReceipt> {
        self.blocking(move |inner, hash| {
            inner.recover(BlobKey { hash, generation }, expected, limit)
        })
        .await
    }
    pub(crate) async fn link_public(&self, receipt: &DurableReceipt, mime: &str) -> Result<()> {
        ensure!(
            receipt.key.hash == self.hash,
            "export mutation hash mismatch"
        );
        let receipt = receipt.clone();
        let mime = mime.to_owned();
        self.blocking(move |inner, _| inner.link_public(&receipt, &mime))
            .await
    }
    pub(crate) async fn withdraw_public(&self, generation: Generation) -> Result<PublicWithdrawal> {
        self.blocking(move |inner, hash| inner.withdraw_public(BlobKey { hash, generation }))
            .await
    }
    pub async fn cleanup(&self, generation: Generation) -> Result<CleanupReceipt> {
        self.blocking(move |inner, hash| inner.cleanup(BlobKey { hash, generation }))
            .await
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn completed_read_observation_pins_until_health_decision() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let volume = FilesystemVolume::open(dir.path().into())?;
        let bytes = b"observation";
        let hash: [u8; 32] = Sha256::digest(bytes).into();
        let mutation = volume.mutation(hash).await;
        let receipt = mutation.put_verified([19; 16], bytes, 100).await?;
        drop(mutation);
        let observation = volume.observation(hash).await;
        assert_eq!(
            observation
                .read_verified(receipt.key(), bytes.len() as u64, 100)
                .await?,
            bytes
        );
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), volume.mutation(hash))
                .await
                .is_err(),
            "repair overtook a still-uncommitted health observation"
        );
        drop(observation);
        let mutation =
            tokio::time::timeout(std::time::Duration::from_secs(1), volume.mutation(hash)).await?;
        drop(mutation);
        Ok(())
    }

    fn hash(bytes: &[u8]) -> Id {
        Sha256::digest(bytes).into()
    }
    #[tokio::test]
    async fn receipt_requires_verified_exact_bytes_and_recovers_idempotently() {
        let dir = tempfile::tempdir().unwrap();
        let volume = FilesystemVolume::open(dir.path().into()).unwrap();
        let data = b"original bytes";
        let h = hash(data);
        let m = volume.mutation(h).await;
        assert!(m.put_verified([1; 16], b"different", 100).await.is_err());
        let receipt = m.put_verified([1; 16], data, 100).await.unwrap();
        assert_eq!(receipt.size(), data.len() as u64);
        let again = m.put_verified([1; 16], data, 100).await.unwrap();
        assert_eq!(again.key(), receipt.key());
        drop(m);
        assert_eq!(
            volume
                .read_verified(receipt.key(), receipt.size(), 100)
                .await
                .unwrap(),
            data
        );
    }
    #[tokio::test]
    async fn stale_generation_cleanup_preserves_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let volume = FilesystemVolume::open(dir.path().into()).unwrap();
        let data = b"same hash, distinct storage generations";
        let m = volume.mutation(hash(data)).await;
        let old = m.put_verified([1; 16], data, 100).await.unwrap();
        let new = m.put_verified([2; 16], data, 100).await.unwrap();
        assert_eq!(m.cleanup([1; 16]).await.unwrap().key(), old.key());
        drop(m);
        assert!(
            volume
                .read_verified(old.key(), old.size(), 100)
                .await
                .is_err()
        );
        assert_eq!(
            volume
                .read_verified(new.key(), new.size(), 100)
                .await
                .unwrap(),
            data
        );
        volume
            .mutation(hash(data))
            .await
            .cleanup([1; 16])
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn corruption_and_oversize_do_not_mint_receipts() {
        let dir = tempfile::tempdir().unwrap();
        let volume = FilesystemVolume::open(dir.path().into()).unwrap();
        let data = b"correct";
        let m = volume.mutation(hash(data)).await;
        assert!(m.put_verified([1; 16], data, 2).await.is_err());
        let receipt = m.put_verified([1; 16], data, 100).await.unwrap();
        tokio::fs::write(volume.object_path(receipt.key()), b"corrupt")
            .await
            .unwrap();
        assert!(m.recover([1; 16], data.len() as u64, 100).await.is_err());
        assert!(m.put_verified([1; 16], data, 100).await.is_err());
        m.cleanup([1; 16]).await.unwrap();
    }
    #[tokio::test]
    async fn hash_mutations_serialize_without_blocking_other_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let volume = FilesystemVolume::open(dir.path().into()).unwrap();
        let guard = volume.mutation([1; 32]).await;
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(20),
                volume.mutation([1; 32])
            )
            .await
            .is_err()
        );
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(20),
                volume.mutation([2; 32])
            )
            .await
            .is_ok()
        );
        drop(guard);
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(20),
                volume.mutation([1; 32])
            )
            .await
            .is_ok()
        );
    }
    #[test]
    fn volume_has_one_mutation_owner() {
        let dir = tempfile::tempdir().unwrap();
        let first = FilesystemVolume::open(dir.path().into()).unwrap();
        assert!(FilesystemVolume::open(dir.path().into()).is_err());
        drop(first);
        assert!(FilesystemVolume::open(dir.path().into()).is_ok());
    }
    #[tokio::test]
    async fn cancelled_caller_cannot_release_inflight_mutation_ownership() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let volume = Arc::new(FilesystemVolume::open(root.clone()).unwrap());
        let data = b"blocking write after async caller cancellation";
        let h = hash(data);
        let worker_volume = volume.clone();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let task = tokio::spawn(async move {
            let mutation = worker_volume.mutation(h).await;
            mutation
                .blocking(move |inner, hash| {
                    started_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    inner.put(
                        BlobKey {
                            hash,
                            generation: [7; 16],
                        },
                        data,
                        100,
                    )
                })
                .await
        });
        started_rx.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), volume.mutation(h))
                .await
                .is_err()
        );
        drop(volume);
        assert!(
            FilesystemVolume::open(root.clone()).is_err(),
            "pending I/O must keep cross-process volume lock"
        );
        release_tx.send(()).unwrap();
        let mut reopened = None;
        for _ in 0..100 {
            if let Ok(v) = FilesystemVolume::open(root.clone()) {
                reopened = Some(v);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let reopened = reopened.expect("finished I/O releases ownership");
        assert_eq!(
            reopened
                .read_verified(
                    &BlobKey {
                        hash: h,
                        generation: [7; 16]
                    },
                    data.len() as u64,
                    100
                )
                .await
                .unwrap(),
            data
        );
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn cancelled_open_reader_pins_bytes_until_io_finishes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let volume = Arc::new(FilesystemVolume::open(root.clone()).unwrap());
        let data = b"an open reader must outlive caller cancellation";
        let h = hash(data);
        let receipt = volume
            .mutation(h)
            .await
            .put_verified([9; 16], data, 100)
            .await
            .unwrap();
        let key = receipt.key().clone();
        let worker = volume.clone();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let task = tokio::spawn(async move {
            worker
                .read_blocking(h, move |inner| {
                    let mut file = fs::File::open(inner.object_path(&key))?;
                    started_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    let mut bytes = Vec::new();
                    file.read_to_end(&mut bytes)?;
                    Ok(bytes)
                })
                .await
        });
        started_rx.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), volume.mutation(h))
                .await
                .is_err()
        );
        // Other readers can proceed; only a mutation/cleanup must wait.
        assert_eq!(
            volume
                .read_verified(receipt.key(), receipt.size(), 100)
                .await
                .unwrap(),
            data
        );
        drop(volume);
        assert!(FilesystemVolume::open(root.clone()).is_err());
        release_tx.send(()).unwrap();
        let mut reopened = None;
        for _ in 0..100 {
            if let Ok(v) = FilesystemVolume::open(root.clone()) {
                reopened = Some(v);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let reopened = reopened.expect("read completion releases owner and pin");
        reopened.mutation(h).await.cleanup([9; 16]).await.unwrap();
        assert!(!reopened.object_path(receipt.key()).exists());
    }
}
