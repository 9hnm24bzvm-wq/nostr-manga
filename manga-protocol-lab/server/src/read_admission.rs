//! Bounded read admission. Waiting owns no active class or database capacity.
//! Class caps provide isolation from a single class, not strict reservations.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    io::Write,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Notify;

#[derive(Clone, Copy, Debug)]
pub enum Class {
    Catalogue,
    LibraryHead,
    Live,
}
impl Class {
    fn index(self) -> usize {
        self as usize
    }
    fn name(self) -> &'static str {
        match self {
            Self::Catalogue => "catalogue",
            Self::LibraryHead => "library_head",
            Self::Live => "live",
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClassConfig {
    pub active: usize,
    pub waiting: usize,
}
impl Default for ClassConfig {
    fn default() -> Self {
        Self {
            active: 2,
            waiting: 8,
        }
    }
}
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub overall: usize,
    pub wait_ms: u64,
    pub catalogue: ClassConfig,
    pub library_head: ClassConfig,
    pub live: ClassConfig,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            overall: 4,
            wait_ms: 200,
            catalogue: ClassConfig::default(),
            library_head: ClassConfig::default(),
            live: ClassConfig::default(),
        }
    }
}
impl Config {
    /// Omitted node configuration preserves smaller existing DB pools.
    pub fn for_database(database_connections: usize) -> Self {
        let overall = 4.min(database_connections);
        let class = ClassConfig {
            active: 2.min(overall),
            waiting: 8,
        };
        Self {
            overall,
            wait_ms: 200,
            catalogue: class.clone(),
            library_head: class.clone(),
            live: class,
        }
    }

    pub fn validate(&self, database_connections: usize) -> Result<()> {
        ensure!(
            (1..=1024).contains(&self.overall) && self.overall <= database_connections,
            "read overall limit must fit database connections"
        );
        ensure!(
            (1..=5000).contains(&self.wait_ms),
            "read wait must be 1..=5000 milliseconds"
        );
        for class in [&self.catalogue, &self.library_head, &self.live] {
            ensure!(
                (1..=self.overall).contains(&class.active),
                "read class active limit must fit overall limit"
            );
            ensure!(
                class.waiting <= 1024,
                "read class waiting limit exceeds 1024"
            );
        }
        Ok(())
    }
}
#[derive(Debug, thiserror::Error)]
#[error("read admission busy; retry later")]
pub struct Busy;

/// Cumulative operator counters. Arrays follow Catalogue, LibraryHead, Live.
/// Selection, sink attempts and failures are distinct; delivery is not guaranteed.
#[derive(Clone, Default, Serialize)]
pub(crate) struct DiagnosticTotals {
    queue_full: [u64; 3],
    wait_expired: [u64; 3],
    ticket_exhausted: [u64; 3],
    records_selected: u64,
    records_suppressed: u64,
    records_dropped: u64,
    sink_write_attempts: u64,
    sink_write_failures: u64,
}
struct Diagnostics {
    since: Instant,
    last_record: Option<Instant>,
    totals: DiagnosticTotals,
}
enum Rejection {
    QueueFull,
    WaitExpired,
    TicketExhausted,
}
#[derive(Serialize)]
struct DiagnosticRecord {
    version: u8,
    at_unix_ms: u64,
    since_start_us: u64,
    class: &'static str,
    reason: &'static str,
    elapsed_us: u64,
    queued_us: Option<u64>,
    timeout_overshoot_us: Option<u64>,
    wait_ms: u64,
    active: [usize; 3],
    waiting: [usize; 3],
    overall_active: usize,
    overall_limit: usize,
    active_limits: [usize; 3],
    waiting_limits: [usize; 3],
    totals: DiagnosticTotals,
    holder_phases: [[u64; 8]; 3],
    holder_draining: [u64; 3],
}
fn micros(duration: Duration) -> u64 {
    duration.as_micros().try_into().unwrap_or(u64::MAX)
}
#[derive(Default)]
struct SinkCounters {
    dropped: AtomicU64,
    attempts: AtomicU64,
    failures: AtomicU64,
}
fn increment(counter: &AtomicU64) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        Some(value.saturating_add(1))
    });
}
struct Sink {
    sender: SyncSender<DiagnosticRecord>,
    counters: Arc<SinkCounters>,
}
impl Sink {
    fn new() -> Self {
        let (sender, receiver) = sync_channel::<DiagnosticRecord>(1);
        let counters = Arc::new(SinkCounters::default());
        let worker_counters = counters.clone();
        // One enabled-controller worker, never one per request. A blocked OS
        // sink holds at most this record plus one queued record. There is no
        // join on shutdown: the node owns this worker for its process lifetime.
        // If creation fails, the disconnected sender records dropped handoffs.
        let _ = std::thread::Builder::new()
            .name("read-diagnostic-sink".into())
            .spawn(move || {
                while let Ok(record) = receiver.recv() {
                    increment(&worker_counters.attempts);
                    let written = serde_json::to_string(&record)
                        .ok()
                        .filter(|json| json.len() <= 2048)
                        .is_some_and(|json| {
                            writeln!(std::io::stderr().lock(), "READ_ADMISSION_DIAGNOSTIC {json}")
                                .is_ok()
                        });
                    if !written {
                        increment(&worker_counters.failures);
                    }
                }
            });
        Self { sender, counters }
    }
    fn emit(&self, record: Option<DiagnosticRecord>) {
        if let Some(record) = record
            && self.sender.try_send(record).is_err()
        {
            increment(&self.counters.dropped);
        }
    }
    fn totals(&self, mut totals: DiagnosticTotals) -> DiagnosticTotals {
        totals.records_dropped = self.counters.dropped.load(Ordering::Relaxed);
        totals.sink_write_attempts = self.counters.attempts.load(Ordering::Relaxed);
        totals.sink_write_failures = self.counters.failures.load(Ordering::Relaxed);
        totals
    }
}
struct State {
    active: [usize; 3],
    total: usize,
    next: u64,
    waiting: VecDeque<(u64, Class)>,
    diagnostics: Option<Diagnostics>,
}
struct Inner {
    config: Config,
    sink: Option<Sink>,
    holders: Option<Arc<crate::read_diagnostics::Observer>>,
    state: Mutex<State>,
    changed: Notify,
}
#[derive(Clone)]
pub struct Admission {
    inner: Arc<Inner>,
}
pub struct Permit {
    inner: Arc<Inner>,
    class: Class,
    holder: Option<crate::read_diagnostics::Holder>,
}
struct Waiting {
    inner: Arc<Inner>,
    ticket: Option<u64>,
}
impl Inner {
    fn rejected(
        &self,
        state: &mut State,
        class: Class,
        reason: Rejection,
        started: Option<Instant>,
        queued: Option<Instant>,
    ) -> Option<DiagnosticRecord> {
        let diagnostics = state.diagnostics.as_mut()?;
        let now = Instant::now();
        let (name, count) = match reason {
            Rejection::QueueFull => ("queue_full", &mut diagnostics.totals.queue_full),
            Rejection::WaitExpired => ("wait_expired", &mut diagnostics.totals.wait_expired),
            Rejection::TicketExhausted => {
                ("ticket_exhausted", &mut diagnostics.totals.ticket_exhausted)
            }
        };
        count[class.index()] = count[class.index()].saturating_add(1);
        if diagnostics
            .last_record
            .is_some_and(|last| now.duration_since(last) < Duration::from_secs(1))
        {
            diagnostics.totals.records_suppressed =
                diagnostics.totals.records_suppressed.saturating_add(1);
            return None;
        }
        diagnostics.last_record = Some(now);
        diagnostics.totals.records_selected = diagnostics.totals.records_selected.saturating_add(1);
        let mut waiting = [0; 3];
        for (_, class) in &state.waiting {
            waiting[class.index()] += 1;
        }
        let queued_us = queued.map(|at| micros(now.duration_since(at)));
        Some(DiagnosticRecord {
            version: 1,
            at_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            since_start_us: micros(now.duration_since(diagnostics.since)),
            class: class.name(),
            reason: name,
            elapsed_us: micros(now.duration_since(started?)),
            queued_us,
            timeout_overshoot_us: queued_us
                .map(|value| value.saturating_sub(self.config.wait_ms * 1000)),
            wait_ms: self.config.wait_ms,
            active: state.active,
            waiting,
            overall_active: state.total,
            overall_limit: self.config.overall,
            active_limits: [
                self.config.catalogue.active,
                self.config.library_head.active,
                self.config.live.active,
            ],
            waiting_limits: [
                self.config.catalogue.waiting,
                self.config.library_head.waiting,
                self.config.live.waiting,
            ],
            totals: self.sink.as_ref()?.totals(diagnostics.totals.clone()),
            holder_phases: self.holders.as_ref()?.active(),
            holder_draining: self.holders.as_ref()?.draining(),
        })
    }
    fn emit(&self, record: Option<DiagnosticRecord>) {
        if let Some(sink) = &self.sink {
            sink.emit(record);
        }
    }
    fn class(&self, class: Class) -> &ClassConfig {
        match class {
            Class::Catalogue => &self.config.catalogue,
            Class::LibraryHead => &self.config.library_head,
            Class::Live => &self.config.live,
        }
    }
    fn room(&self, state: &State, class: Class) -> bool {
        state.total < self.config.overall && state.active[class.index()] < self.class(class).active
    }
    fn take(self: &Arc<Self>, state: &mut State, class: Class) -> Permit {
        state.total += 1;
        state.active[class.index()] += 1;
        Permit {
            inner: self.clone(),
            class,
            holder: self
                .holders
                .as_ref()
                .map(|observer| observer.grant(class.index())),
        }
    }
}
impl Admission {
    pub fn new(config: Config, database_connections: usize) -> Result<Self> {
        config.validate(database_connections)?;
        let diagnostics_enabled =
            std::env::var("MANGA_READ_ADMISSION_DIAGNOSTICS").is_ok_and(|value| value == "1");
        Ok(Self {
            inner: Arc::new(Inner {
                config,
                sink: diagnostics_enabled.then(Sink::new),
                holders: diagnostics_enabled.then(crate::read_diagnostics::Observer::new),
                state: Mutex::new(State {
                    active: [0; 3],
                    total: 0,
                    next: 0,
                    waiting: VecDeque::new(),
                    diagnostics: diagnostics_enabled.then(|| Diagnostics {
                        since: Instant::now(),
                        last_record: None,
                        totals: DiagnosticTotals::default(),
                    }),
                }),
                changed: Notify::new(),
            }),
        })
    }
    /// Application-used authenticated status surface; absent in default-off mode.
    pub(crate) fn diagnostic_totals(&self) -> Option<DiagnosticTotals> {
        let sink = self.inner.sink.as_ref()?;
        self.inner
            .state
            .lock()
            .unwrap()
            .diagnostics
            .as_ref()
            .map(|d| sink.totals(d.totals.clone()))
    }
    pub(crate) fn diagnostic_holders(&self) -> Option<crate::read_diagnostics::Snapshot> {
        self.inner
            .holders
            .as_ref()
            .map(|observer| observer.snapshot())
    }
    pub async fn acquire(&self, class: Class) -> std::result::Result<Permit, Busy> {
        let inner = &self.inner;
        let started = inner.sink.as_ref().map(|_| Instant::now());
        let mut waiting = {
            let mut state = inner.state.lock().unwrap();
            let queued = state
                .waiting
                .iter()
                .filter(|(_, c)| c.index() == class.index())
                .count();
            if queued == 0 && inner.room(&state, class) {
                return Ok(inner.take(&mut state, class));
            }
            if queued >= inner.class(class).waiting {
                let record = inner.rejected(&mut state, class, Rejection::QueueFull, started, None);
                drop(state);
                inner.emit(record);
                return Err(Busy);
            }
            let ticket = state.next;
            let Some(next) = state.next.checked_add(1) else {
                let record =
                    inner.rejected(&mut state, class, Rejection::TicketExhausted, started, None);
                drop(state);
                inner.emit(record);
                return Err(Busy);
            };
            state.next = next;
            state.waiting.push_back((ticket, class));
            Waiting {
                inner: inner.clone(),
                ticket: Some(ticket),
            }
        };
        let queued = started.map(|_| Instant::now());
        let result = tokio::time::timeout(Duration::from_millis(inner.config.wait_ms), async {
            loop {
                let changed = inner.changed.notified();
                tokio::pin!(changed);
                changed.as_mut().enable();
                {
                    let mut state = inner.state.lock().unwrap();
                    let first = state
                        .waiting
                        .iter()
                        .find(|(_, c)| c.index() == class.index())
                        .map(|(id, _)| *id);
                    if first == waiting.ticket && inner.room(&state, class) {
                        let ticket = waiting.ticket.take().unwrap();
                        state.waiting.retain(|(id, _)| *id != ticket);
                        let permit = inner.take(&mut state, class);
                        inner.changed.notify_waiters();
                        return permit;
                    }
                }
                changed.await;
            }
        })
        .await;
        match result {
            Ok(permit) => Ok(permit),
            Err(_) => {
                // Ticket cleanup remains cancellation-safe and occurs before the
                // rejection snapshot and any diagnostic output can delay return.
                drop(waiting);
                let record = started.and_then(|started| {
                    inner.rejected(
                        &mut inner.state.lock().unwrap(),
                        class,
                        Rejection::WaitExpired,
                        Some(started),
                        queued,
                    )
                });
                inner.emit(record);
                Err(Busy)
            }
        }
    }
}
impl Permit {
    pub(crate) fn observation(&self) -> Option<Arc<crate::read_diagnostics::Context>> {
        self.holder
            .as_ref()
            .map(crate::read_diagnostics::Holder::context)
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        let mut state = self.inner.state.lock().unwrap();
        state.total -= 1;
        state.active[self.class.index()] -= 1;
        self.inner.changed.notify_waiters();
        drop(state);
        if let Some(holder) = &self.holder {
            holder.finish();
        }
    }
}
impl Drop for Waiting {
    fn drop(&mut self) {
        if let Some(ticket) = self.ticket {
            self.inner
                .state
                .lock()
                .unwrap()
                .waiting
                .retain(|(id, _)| *id != ticket);
            self.inner.changed.notify_waiters();
        }
    }
}
