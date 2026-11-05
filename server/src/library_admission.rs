//! Bounded network receipt is independent of expensive library processing.
//! Slow admitted uploaders retain byte/count credits, not database/codec slots.
use anyhow::{Result, ensure};
use std::{fmt, sync::Arc, time::Duration};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
#[derive(Debug)]
pub struct Busy {
    pub stage: &'static str,
}
impl fmt::Display for Busy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "library admission busy: {}", self.stage)
    }
}
impl std::error::Error for Busy {}
pub struct Admission {
    receipts: Arc<Semaphore>,
    bytes: Arc<Semaphore>,
    processing: Arc<Semaphore>,
}
pub struct Receipt {
    pub limit: usize,
    _count: OwnedSemaphorePermit,
    _bytes: OwnedSemaphorePermit,
}
pub struct Processing {
    _receipt: Receipt,
    _work: OwnedSemaphorePermit,
}
impl Default for Admission {
    fn default() -> Self {
        Self::with_limits(128, 8)
    }
}
impl Admission {
    pub(crate) fn with_limits(requests: usize, processing: usize) -> Self {
        Self {
            receipts: Arc::new(Semaphore::new(requests)),
            bytes: Arc::new(Semaphore::new(64 * 1024 * 1024)),
            processing: Arc::new(Semaphore::new(processing)),
        }
    }
}
impl Admission {
    pub fn receive(&self, declared: Option<usize>) -> Result<Receipt> {
        let limit = declared.unwrap_or(crate::library_wire::MAX_WIRE);
        ensure!(
            limit <= crate::library_wire::MAX_WIRE,
            "library wire budget"
        );
        let count = self
            .receipts
            .clone()
            .try_acquire_owned()
            .map_err(|_| Busy {
                stage: "request_count",
            })?;
        // Charge body aggregation/copy headroom plus a per-request allowance.
        // Decoded objects have a separate bound from the eight processing slots.
        let weight = u32::try_from(limit * 2 + 65536)?;
        let bytes = self
            .bytes
            .clone()
            .try_acquire_many_owned(weight)
            .map_err(|_| Busy {
                stage: "wire_bytes",
            })?;
        Ok(Receipt {
            limit,
            _count: count,
            _bytes: bytes,
        })
    }
    pub async fn process(&self, receipt: Receipt) -> Result<Processing> {
        let work = wait(self.processing.clone()).await?;
        Ok(Processing {
            _receipt: receipt,
            _work: work,
        })
    }
}
pub(crate) async fn wait(slots: Arc<Semaphore>) -> Result<OwnedSemaphorePermit> {
    tokio::time::timeout(Duration::from_secs(2), slots.acquire_owned())
        .await
        .map_err(|_| Busy {
            stage: "work_queue_timeout",
        })?
        .map_err(|_| {
            Busy {
                stage: "work_queue_closed",
            }
            .into()
        })
}
