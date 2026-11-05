//! A verified snapshot owns independent download count and memory credits.
//! Credits move into the I/O task before reads and into the response producer
//! afterwards; cancelling an HTTP handler cannot free memory still in use.
use anyhow::{Result, ensure};
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Debug, thiserror::Error)]
#[error("download budget exhausted")]
pub struct Busy;

pub struct Budget {
    count: Arc<Semaphore>,
    bytes: Arc<Semaphore>,
}
pub struct Lease {
    _count: OwnedSemaphorePermit,
    _bytes: OwnedSemaphorePermit,
}
pub struct Snapshot {
    bytes: Vec<u8>,
    lease: Lease,
}
impl Budget {
    pub fn new(count: usize, bytes: usize) -> Result<Self> {
        ensure!(
            count > 0 && bytes > 0 && bytes <= u32::MAX as usize,
            "download budget bounds"
        );
        Ok(Self {
            count: Arc::new(Semaphore::new(count)),
            bytes: Arc::new(Semaphore::new(bytes)),
        })
    }
    pub fn acquire(&self, expected: u64) -> Result<Lease> {
        // Include the excess-byte probe, allocator growth and copied frame headroom.
        let weight = expected
            .checked_add(1)
            .and_then(|n| n.checked_mul(2))
            .and_then(|n| n.checked_add(16384))
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(Busy)?;
        let count = self.count.clone().try_acquire_owned().map_err(|_| Busy)?;
        let bytes = self
            .bytes
            .clone()
            .try_acquire_many_owned(weight)
            .map_err(|_| Busy)?;
        Ok(Lease {
            _count: count,
            _bytes: bytes,
        })
    }
}
impl Snapshot {
    pub(crate) fn verified(bytes: Vec<u8>, lease: Lease) -> Self {
        Self { bytes, lease }
    }
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
    pub fn mime(&self) -> &str {
        crate::manifest::detected_mime(&self.bytes).unwrap_or("application/octet-stream")
    }
    pub fn response(
        mut self,
        range: Option<(usize, usize)>,
        mime: &str,
        private: bool,
    ) -> axum::response::Response {
        if let Some((start, end)) = range {
            self.bytes.truncate(end + 1);
            self.bytes.drain(..start);
        }
        crate::bounded_response::response(self.bytes, self.lease, mime, private, false)
    }
}
