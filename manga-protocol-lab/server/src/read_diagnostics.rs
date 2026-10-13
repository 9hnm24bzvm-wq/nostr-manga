//! Optional, finite admission-holder observations. No I/O or diagnostic locks.
//!
//! Durations are wall time observed by the client task, not SQL CPU time.
//! A task-local context has one owner; spawned tasks do not inherit it. Keep
//! phase guards nested within that owner and label child-operation awaits as
//! uninstrumented unless their actual boundary is known.
use serde::Serialize;
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    time::Instant,
};

const CLASSES: usize = 3;
const PHASES: usize = 8;
const UPPER_US: [u64; 5] = [1_000, 10_000, 100_000, 500_000, 1_000_000];
#[derive(Clone, Copy)]
pub(crate) enum Phase {
    DispatchPending,
    Handler,
    Authentication,
    PoolCheckout,
    DbRoundtrip,
    DecodeResponse,
    StorageWait,
    Uninstrumented,
}
impl Phase {
    fn from_index(value: u8) -> Self {
        match value {
            0 => Self::DispatchPending,
            1 => Self::Handler,
            2 => Self::Authentication,
            3 => Self::PoolCheckout,
            4 => Self::DbRoundtrip,
            5 => Self::DecodeResponse,
            6 => Self::StorageWait,
            _ => Self::Uninstrumented,
        }
    }
}
type Counts = [AtomicU64; CLASSES];
type Phases = [[AtomicU64; PHASES]; CLASSES];
pub(crate) struct Observer {
    since: Instant,
    started: Counts,
    finished: Counts,
    active: Phases,
    elapsed: Phases,
    draining: Counts,
    lifetime: Counts,
    maximum: Counts,
    buckets: [[AtomicU64; 6]; CLASSES],
}
/// Independent atomic observations, not a transactional conservation snapshot.
#[derive(Serialize)]
pub(crate) struct Snapshot {
    phase_names: [&'static str; PHASES],
    lifetime_upper_us: [Option<u64>; 6],
    started: [u64; CLASSES],
    finished: [u64; CLASSES],
    active_phase_counts: [[u64; PHASES]; CLASSES],
    active_draining: [u64; CLASSES],
    phase_us: [[u64; PHASES]; CLASSES],
    lifetime_us: [u64; CLASSES],
    lifetime_max_us: [u64; CLASSES],
    lifetime_buckets: [[u64; 6]; CLASSES],
}
fn add(counter: &AtomicU64, value: u64) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
        Some(old.saturating_add(value))
    });
}
fn counts(values: &Counts) -> [u64; CLASSES] {
    std::array::from_fn(|class| values[class].load(Ordering::Relaxed))
}
impl Observer {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            since: Instant::now(),
            started: Default::default(),
            finished: Default::default(),
            active: Default::default(),
            elapsed: Default::default(),
            draining: Default::default(),
            lifetime: Default::default(),
            maximum: Default::default(),
            buckets: Default::default(),
        })
    }
    fn now(&self) -> u64 {
        self.since
            .elapsed()
            .as_micros()
            .try_into()
            .unwrap_or(u64::MAX)
    }
    pub(crate) fn grant(self: &Arc<Self>, class: usize) -> Holder {
        let now = self.now();
        add(&self.started[class], 1);
        self.active[class][Phase::Uninstrumented as usize].fetch_add(1, Ordering::Relaxed);
        Holder(Arc::new(Context {
            observer: self.clone(),
            class,
            granted: now,
            phase: AtomicU8::new(Phase::Uninstrumented as u8),
            changed: AtomicU64::new(now),
            bound: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            draining: AtomicBool::new(false),
        }))
    }
    pub(crate) fn active(&self) -> [[u64; PHASES]; CLASSES] {
        std::array::from_fn(|class| {
            std::array::from_fn(|phase| self.active[class][phase].load(Ordering::Relaxed))
        })
    }
    pub(crate) fn draining(&self) -> [u64; CLASSES] {
        counts(&self.draining)
    }
    pub(crate) fn snapshot(&self) -> Snapshot {
        Snapshot {
            phase_names: [
                "dispatch_pending",
                "handler",
                "authentication",
                "pool_checkout",
                "db_roundtrip",
                "decode_response",
                "storage_wait",
                "uninstrumented",
            ],
            lifetime_upper_us: [
                Some(1_000),
                Some(10_000),
                Some(100_000),
                Some(500_000),
                Some(1_000_000),
                None,
            ],
            started: counts(&self.started),
            finished: counts(&self.finished),
            active_phase_counts: self.active(),
            active_draining: self.draining(),
            phase_us: std::array::from_fn(|class| {
                std::array::from_fn(|phase| self.elapsed[class][phase].load(Ordering::Relaxed))
            }),
            lifetime_us: counts(&self.lifetime),
            lifetime_max_us: counts(&self.maximum),
            lifetime_buckets: std::array::from_fn(|class| {
                std::array::from_fn(|bucket| self.buckets[class][bucket].load(Ordering::Relaxed))
            }),
        }
    }
}
pub(crate) struct Context {
    observer: Arc<Observer>,
    class: usize,
    granted: u64,
    phase: AtomicU8,
    changed: AtomicU64,
    bound: AtomicBool,
    finished: AtomicBool,
    draining: AtomicBool,
}
impl Context {
    // Only the scoped owner changes phases. Atomic fields let the context travel
    // with its Send future; no context is propagated into concurrent children.
    fn enter(&self, phase: Phase) -> Option<Phase> {
        if self.finished.load(Ordering::Relaxed) {
            return None;
        }
        let now = self.observer.now();
        let previous = Phase::from_index(self.phase.swap(phase as u8, Ordering::Relaxed));
        let at = self.changed.swap(now, Ordering::Relaxed);
        add(
            &self.observer.elapsed[self.class][previous as usize],
            now.saturating_sub(at),
        );
        if previous as u8 != phase as u8 {
            self.observer.active[self.class][previous as usize].fetch_sub(1, Ordering::Relaxed);
            self.observer.active[self.class][phase as usize].fetch_add(1, Ordering::Relaxed);
        }
        Some(previous)
    }
}
pub(crate) struct Holder(Arc<Context>);
impl Holder {
    pub(crate) fn context(&self) -> Arc<Context> {
        if !self.0.bound.swap(true, Ordering::Relaxed) {
            self.0.enter(Phase::DispatchPending);
        }
        self.0.clone()
    }
    pub(crate) fn finish(&self) {
        let context = &self.0;
        if context.finished.swap(true, Ordering::Relaxed) {
            return;
        }
        let observer = &context.observer;
        let now = observer.now();
        let phase = context.phase.load(Ordering::Relaxed) as usize;
        add(
            &observer.elapsed[context.class][phase],
            now.saturating_sub(context.changed.load(Ordering::Relaxed)),
        );
        observer.active[context.class][phase].fetch_sub(1, Ordering::Relaxed);
        if context.draining.load(Ordering::Relaxed) {
            observer.draining[context.class].fetch_sub(1, Ordering::Relaxed);
        }
        let duration = now.saturating_sub(context.granted);
        add(&observer.finished[context.class], 1);
        add(&observer.lifetime[context.class], duration);
        observer.maximum[context.class].fetch_max(duration, Ordering::Relaxed);
        let bucket = UPPER_US
            .iter()
            .position(|upper| duration <= *upper)
            .unwrap_or(5);
        add(&observer.buckets[context.class][bucket], 1);
    }
}
tokio::task_local! {
    static CURRENT: Arc<Context>;
}
pub(crate) async fn scope<F: Future>(context: Option<Arc<Context>>, future: F) -> F::Output {
    if let Some(context) = context {
        CURRENT
            .scope(context, async {
                let _phase = phase(Phase::Handler);
                future.await
            })
            .await
    } else {
        future.await
    }
}
pub(crate) struct PhaseGuard(Option<(Arc<Context>, Phase)>);
pub(crate) fn phase(phase: Phase) -> PhaseGuard {
    PhaseGuard(
        CURRENT
            .try_with(|context| {
                context
                    .enter(phase)
                    .map(|previous| (context.clone(), previous))
            })
            .ok()
            .flatten(),
    )
}
impl Drop for PhaseGuard {
    fn drop(&mut self) {
        if let Some((context, previous)) = &self.0 {
            context.enter(*previous);
        }
    }
}
pub(crate) async fn during<F: Future>(kind: Phase, future: F) -> F::Output {
    let _phase = phase(kind);
    future.await
}
pub(crate) fn draining() {
    let _ = CURRENT.try_with(|context| {
        if !context.finished.load(Ordering::Relaxed)
            && !context.draining.swap(true, Ordering::Relaxed)
        {
            context.observer.draining[context.class].fetch_add(1, Ordering::Relaxed);
        }
    });
}
