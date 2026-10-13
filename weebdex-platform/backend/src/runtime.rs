//! Bounded request-body reads and observable process lifecycle.
//! Deadlines end body intake, never cancel an already-running SQL transaction.
use axum::body::{Body, Bytes, HttpBody};
use http_body::{Frame, SizeHint};
use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll},
    time::Duration,
};
use tokio::time::Sleep;

/// One cooperating writer per native upload root, also used by offline backup.
/// Never truncate/unlink this named lock; its inode must remain stable.
pub fn acquire_storage_owner(root: &std::path::Path) -> anyhow::Result<std::fs::File> {
    use anyhow::{Context, ensure};
    std::fs::create_dir_all(root).context("cannot initialize native upload root")?;
    let metadata = std::fs::symlink_metadata(root)?;
    ensure!(metadata.is_dir(), "native upload root must be a directory");
    let path = root.join(".platform-owner");
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let owner = options.open(&path).context("cannot open native upload owner")?;
    owner.try_lock().context("native upload root has a live platform owner")?;
    crate::domains::uploads::storage::verify_named_lease(&owner, &path)
        .map_err(|_| anyhow::anyhow!("native upload owner path changed or is not regular"))?;
    owner.sync_all()?;
    std::fs::File::open(root)?.sync_all()?;
    Ok(owner)
}

#[cfg(test)]
mod owner_tests {
    use super::acquire_storage_owner;

    #[test]
    fn native_owner_excludes_another_writer_and_releases_on_drop() {
        let root = tempfile::tempdir().unwrap();
        let first = acquire_storage_owner(root.path()).unwrap();
        assert!(acquire_storage_owner(root.path()).is_err());
        drop(first);
        assert!(acquire_storage_owner(root.path()).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn native_owner_rejects_symlink_name() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("regular");
        std::fs::write(&target, b"unchanged").unwrap();
        std::os::unix::fs::symlink(&target, root.path().join(".platform-owner")).unwrap();
        assert!(acquire_storage_owner(root.path()).is_err());
        assert_eq!(std::fs::read(target).unwrap(), b"unchanged");
    }
}

#[derive(Default)]
pub struct RuntimeState {
    ready: AtomicBool,
    draining: AtomicBool,
}
impl RuntimeState {
    pub fn mark_ready(&self) {
        self.ready.store(true, Ordering::Release);
    }
    pub fn begin_draining(&self) {
        self.draining.store(true, Ordering::Release);
        self.ready.store(false, Ordering::Release);
    }
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
    pub fn is_draining(&self) -> bool {
        self.draining.load(Ordering::Acquire)
    }
}

/// Streaming hard byte/total-read budget. No JSON or multipart body is buffered
/// here; backpressure and trailers remain owned by the original HTTP body.
pub struct BoundedBody {
    inner: Body,
    deadline: Pin<Box<Sleep>>,
    maximum: u64,
    observed: u64,
    ended: bool,
}
impl BoundedBody {
    pub fn wrap(inner: Body, maximum: usize, timeout: Duration) -> Body {
        Body::new(Self {
            inner,
            deadline: Box::pin(tokio::time::sleep(timeout)),
            maximum: maximum as u64,
            observed: 0,
            ended: false,
        })
    }
}
impl HttpBody for BoundedBody {
    type Data = Bytes;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        if self.ended {
            return Poll::Ready(None);
        }
        if self.inner.is_end_stream() {
            self.ended = true;
            return Poll::Ready(None);
        }
        if self.deadline.as_mut().poll(cx).is_ready() {
            self.ended = true;
            return Poll::Ready(Some(Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Request body deadline exceeded",
            )
            .into())));
        }
        match Pin::new(&mut self.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    self.observed = self.observed.saturating_add(data.len() as u64);
                    if self.observed > self.maximum {
                        self.ended = true;
                        return Poll::Ready(Some(Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Request body byte limit exceeded",
                        )
                        .into())));
                    }
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(error))) => {
                self.ended = true;
                Poll::Ready(Some(Err(error.into())))
            }
            Poll::Ready(None) => {
                self.ended = true;
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
    fn is_end_stream(&self) -> bool {
        self.ended || self.inner.is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

/// One owned, bounded cleanup worker. Existing durable ledger reconciliation
/// retries in small batches; discarded temporary bytes can never become media.
pub fn spawn_storage_maintenance(state: crate::state::AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            if state.runtime.is_draining() {
                return;
            }
            crate::domains::uploads::recover_storage(&state).await;
        }
    })
}
/// Expired, non-referenced intake/proxy files are conservatively age-limited.
/// A held OS file lock protects active streams/decoders/bodies even after async
/// suspension; process death releases it for a later bounded maintenance pass.
pub(crate) async fn cleanup_expired_temporary(root: &std::path::Path) {
    for name in ["temporary", "proxy-temporary"] {
        let temporary = root.join(name);
        let Ok(metadata) = tokio::fs::symlink_metadata(&temporary).await else {
            continue;
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            continue;
        }
        let Ok(mut directory) = tokio::fs::read_dir(&temporary).await else {
            continue;
        };
        let mut inspected = 0usize;
        let mut removed = 0usize;
        while let Ok(Some(entry)) = directory.next_entry().await {
            inspected += 1;
            if inspected > 1000 || removed >= 100 {
                break;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !crate::domains::uploads::storage::valid_key(&name) {
                continue;
            }
            let Ok(kind) = entry.file_type().await else {
                continue;
            };
            if !kind.is_file() || kind.is_symlink() {
                continue;
            }
            let Ok(metadata) = entry.metadata().await else {
                continue;
            };
            let old = metadata
                .modified()
                .ok()
                .and_then(|time| time.elapsed().ok())
                .is_some_and(|age| age >= Duration::from_secs(24 * 60 * 60));
            if !old {
                continue;
            }
            let Ok(file) = tokio::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(entry.path())
                .await
            else {
                continue;
            };
            let lease = file.into_std().await;
            if lease.try_lock().is_err() {
                continue;
            }
            if tokio::fs::remove_file(entry.path()).await.is_ok() {
                removed += 1;
            }
        }
        if removed > 0 {
            let _ = crate::domains::uploads::storage::sync_directory(&temporary).await;
            tracing::info!(removed, "expired unreferenced intake/proxy files removed");
        }
    }
}
