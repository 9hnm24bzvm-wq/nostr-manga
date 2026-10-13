//! Generation-bound filesystem receipts. Every blocking mutation owns both its
//! hash guard and volume file lock until I/O ends, even if its async caller drops.
use crate::{
    db::Id,
    storage_admission::{Admission, Class, Permit},
};
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
    admission: Arc<Admission>,
}
pub(crate) struct Observation {
    inner: Arc<VolumeInner>,
    permit: Arc<Permit>,
    hash: Id,
    guard: Arc<OwnedRwLockReadGuard<()>>,
}
impl Observation {
    pub(crate) async fn withdraw_public(&self, key: &BlobKey) -> Result<PublicWithdrawal> {
        ensure!(key.hash == self.hash, "observation hash mismatch");
        let pin = self.guard.clone();
        let key = key.clone();
        let permit = self.permit.clone();
        let inner = self.inner.clone();
        crate::node::spawn_blocking_owned_scope(move || {
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
        let permit = self.permit.clone();
        let inner = self.inner.clone();
        crate::node::spawn_blocking_owned_scope(move || {
            let _pin = pin;
            let _permit = permit;
            inner.read_verified(&key, expected, limit)
        })
        .await?
    }
}
pub struct Mutation {
    inner: Arc<VolumeInner>,
    permit: Arc<Permit>,
    hash: Id,
    guard: Arc<OwnedRwLockWriteGuard<()>>,
}
impl FilesystemVolume {
    pub fn open(root: PathBuf) -> Result<Self> {
        Self::open_with_limit(root, 4)
    }
    pub fn open_with_limit(root: PathBuf, io_slots: usize) -> Result<Self> {
        Self::open_with_policy(root, io_slots, io_slots)
    }
    pub(crate) fn open_with_policy(
        root: PathBuf,
        io_slots: usize,
        cleanup_slots: usize,
    ) -> Result<Self> {
        ensure!((1..=16).contains(&io_slots), "filesystem I/O slot bounds");
        ensure!(
            (1..=16).contains(&cleanup_slots),
            "filesystem cleanup slot bounds"
        );
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
            admission: Admission::new(io_slots, cleanup_slots),
        })
    }
    pub(crate) async fn update_cleanup_headroom(&self, quota: (u64, u64)) -> Result<()> {
        let inner = self.inner.clone();
        let filesystem = crate::node::spawn_blocking_owned_scope(move || {
            use std::os::fd::AsRawFd;
            let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
            // The live owner descriptor identifies this exact opened volume;
            // fstatvfs initializes the output only when it returns success.
            if unsafe { libc::fstatvfs(inner._owner.as_raw_fd(), stats.as_mut_ptr()) } != 0 {
                tracing::warn!(error=%std::io::Error::last_os_error(), "filesystem headroom sample unavailable");
                return None;
            }
            let stats = unsafe { stats.assume_init() };
            (stats.f_blocks > 0).then_some((stats.f_bavail, stats.f_blocks))
        }).await?;
        self.admission.observe_headroom(quota, filesystem);
        Ok(())
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
    pub(crate) async fn observation(&self, hash: Id) -> Observation {
        self.observation_for(hash, Class::Foreground).await
    }
    pub(crate) async fn background_observation(&self, hash: Id) -> Observation {
        self.observation_for(hash, Class::Background).await
    }
    async fn observation_for(&self, hash: Id, class: Class) -> Observation {
        let guard = Arc::new(self.hash_lock(hash).read_owned().await);
        let permit = self.admission.acquire(class).await;
        Observation {
            inner: self.inner.clone(),
            permit,
            hash,
            guard,
        }
    }
    pub fn identity(&self) -> Id {
        self.inner.identity
    }
    pub fn public_path(&self) -> PathBuf {
        self.inner.root.join("public")
    }
    pub async fn mutation(&self, hash: Id) -> Mutation {
        self.mutation_for(hash, Class::Foreground).await
    }
    pub(crate) async fn background_mutation(&self, hash: Id) -> Mutation {
        self.mutation_for(hash, Class::Background).await
    }
    async fn mutation_for(&self, hash: Id, class: Class) -> Mutation {
        let guard = Arc::new(self.hash_lock(hash).write_owned().await);
        let permit = self.admission.acquire(class).await;
        Mutation {
            inner: self.inner.clone(),
            permit,
            hash,
            guard,
        }
    }
    pub async fn read_verified(&self, key: &BlobKey, expected: u64, limit: u64) -> Result<Vec<u8>> {
        self.observation(key.hash)
            .await
            .read_verified(key, expected, limit)
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
            if count == 0 {
                break;
            }
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
impl Mutation {
    async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce(Arc<VolumeInner>, Id) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let permit = self.permit.clone();
        let inner = self.inner.clone();
        let guard = self.guard.clone();
        let hash = self.hash;
        crate::node::spawn_blocking_owned_scope(move || {
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
