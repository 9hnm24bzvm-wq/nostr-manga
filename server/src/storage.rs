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
        #[cfg(windows)]
        {
            create_private_tree(&root)?;
            create_private_tree(&root.join("objects"))?;
        }
        #[cfg(not(windows))]
        fs::create_dir_all(root.join("objects"))?;
        let root = root.canonicalize()?;
        let owner_path = root.join(".volume-owner");
        let owner = if owner_path.try_exists()? {
            #[cfg(windows)]
            {
                crate::private_files::open_private(&owner_path)?
            }
            #[cfg(not(windows))]
            {
                fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&owner_path)?
            }
        } else {
            crate::private_files::create_private(&owner_path)?
        };
        crate::platform_fs::sync_directory(&root)?;
        owner
            .try_lock()
            .context("storage volume already has a mutation owner")?;
        let mut identity = Sha256::new();
        identity.update(b"manga-public-volume-v1");
        identity.update(crate::platform_fs::storage_epoch()?);
        identity.update(crate::platform_fs::file_identity(&owner)?);
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
            match crate::platform_fs::available_space(&inner._owner, &inner.root) {
                Ok(value) => value,
                Err(error) => {
                    tracing::warn!(%error, "filesystem headroom sample unavailable");
                    None
                }
            }
        })
        .await?;
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
            crate::platform_fs::sync_directory(current)?;
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
        #[cfg(windows)]
        let mut file = crate::private_files::open_private(&path)?;
        #[cfg(not(windows))]
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
        let verified = crate::private_files::open_private(&path)?;
        let flushing = fs::OpenOptions::new().read(true).write(true).open(&path)?;
        ensure!(
            crate::platform_fs::same_file(&verified, &flushing)?,
            "object changed before flush"
        );
        flushing.sync_all()?;
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
        #[cfg(windows)]
        create_private_tree(parent)?;
        #[cfg(not(windows))]
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
        let mut file = crate::private_files::create_private(&partial)?;
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
        crate::platform_fs::require_external_serving()?;
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
            let create = {
                let mut builder = fs::DirBuilder::new();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    builder.mode(0o700);
                }
                builder.create(&parent)
            };
            match create {
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
                        && crate::platform_fs::same_file(
                            &fs::File::open(&source)?,
                            &fs::File::open(&target)?
                        )?,
                    "public alias belongs to a different inode"
                );
            }
            Err(error) => return Err(error.into()),
        }
        self.sync_ancestors(&parent)?;
        Ok(())
    }
    fn withdraw_public(&self, key: BlobKey) -> Result<PublicWithdrawal> {
        // Native HTTP creates no exported namespace. Permit its corruption
        // quarantine receipt only when no legacy public tree exists; never
        // unlink or attest to recovery of legacy external readers here.
        #[cfg(not(target_os = "linux"))]
        {
            ensure!(
                !self.root.join("public").try_exists()?,
                "legacy public namespace requires Linux recovery"
            );
            crate::platform_fs::sync_directory(&self.root)?;
            return Ok(PublicWithdrawal {
                key,
                root_identity: self.identity,
            });
        }
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

#[cfg(windows)]
fn create_private_tree(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => crate::private_files::verify_private_directory(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path.parent().context("private directory has no parent")?;
            if !parent.as_os_str().is_empty() && !parent.exists() {
                create_private_tree(parent)?;
            }
            crate::private_files::create_private_directory(path)?;
            crate::private_files::verify_private_directory(path)
        }
        Err(error) => Err(error.into()),
    }
}
