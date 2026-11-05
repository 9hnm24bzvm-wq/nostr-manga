//! One authoritative lookup per notification, then bounded filtered fanout.
//! A session registers before history; overflow/discontinuity requires resubscribe.
use crate::{
    auth::Event,
    db::{Id, Repository},
    read_admission::{Admission, Class},
    relay_query::RelayFilter,
};
use anyhow::{Result, anyhow, ensure};
use std::{
    collections::BTreeMap,
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Semaphore, broadcast, mpsc, watch};

pub trait EventSource: Send + Sync + 'static {
    fn fetch(&self, id: Id) -> impl Future<Output = Result<Option<Event>>> + Send;
}
pub struct RepositorySource {
    repo: Repository,
    reads: Arc<Admission>,
    pending: Arc<Semaphore>,
}
impl RepositorySource {
    pub fn new(repo: Repository, reads: Arc<Admission>) -> Self {
        Self {
            repo,
            reads,
            pending: Arc::new(Semaphore::new(1)),
        }
    }
}
impl EventSource for RepositorySource {
    async fn fetch(&self, id: Id) -> Result<Option<Event>> {
        // At most one owned lookup per dispatcher, including one whose caller
        // has timed out. A dropped PostgreSQL future need not stop server work.
        let pending = self.pending.clone().acquire_owned().await?;
        let repo = self.repo.clone();
        let reads = self.reads.clone();
        crate::node::spawn_owned_scope(async move {
            let _pending = pending;
            let _permit = tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    match reads.acquire(Class::Live).await {
                        Ok(permit) => break permit,
                        Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
                    }
                }
            })
            .await?;
            // Existing server-side lock/statement and pool deadlines terminate
            // the operation; its client and read credit stay owned until then.
            repo.relay_event(id).await
        })
        .await?
    }
}
struct Client {
    filters: Vec<RelayFilter>,
    sender: mpsc::Sender<Arc<Event>>,
    lost: watch::Sender<bool>,
}
#[derive(Default)]
struct Registry {
    next: u64,
    clients: BTreeMap<u64, Client>,
}
#[derive(Default)]
pub struct Dispatcher {
    registry: Mutex<Registry>,
    ready: AtomicBool,
}

pub struct Session {
    dispatcher: Arc<Dispatcher>,
    id: u64,
    receiver: mpsc::Receiver<Arc<Event>>,
    lost: watch::Receiver<bool>,
}
struct Reset(Arc<Dispatcher>);
impl Drop for Reset {
    fn drop(&mut self) {
        self.0.ready.store(false, Ordering::Release);
        self.0.interrupt();
    }
}
impl Dispatcher {
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
    pub fn subscribe(self: &Arc<Self>) -> Result<Session> {
        let mut registry = self.registry.lock().unwrap();
        ensure!(registry.clients.len() < 40, "live session budget");
        let id = registry.next;
        registry.next = registry
            .next
            .checked_add(1)
            .ok_or_else(|| anyhow!("session sequence exhausted"))?;
        let (sender, receiver) = mpsc::channel(64);
        let (lost, loss) = watch::channel(false);
        registry.clients.insert(
            id,
            Client {
                filters: Vec::new(),
                sender,
                lost,
            },
        );
        Ok(Session {
            dispatcher: self.clone(),
            id,
            receiver,
            lost: loss,
        })
    }
    fn interrupt(&self) {
        let clients = std::mem::take(&mut self.registry.lock().unwrap().clients);
        for (_, client) in clients {
            let _ = client.lost.send(true);
        }
    }
    fn deliver(&self, event: Event) {
        let event = Arc::new(event);
        self.registry.lock().unwrap().clients.retain(|_, client| {
            if !client.filters.iter().any(|filter| filter.matches(&event)) {
                return true;
            }
            if client.sender.try_send(event.clone()).is_err() {
                let _ = client.lost.send(true);
                return false;
            }
            true
        });
    }
    pub async fn run(
        self: Arc<Self>,
        mut hints: broadcast::Receiver<Option<Id>>,
        source: impl EventSource,
    ) {
        let _reset = Reset(self.clone());
        self.ready.store(true, Ordering::Release);
        loop {
            let id = match hints.recv().await {
                Ok(Some(id)) => id,
                Err(broadcast::error::RecvError::Closed) => return,
                _ => {
                    self.interrupt();
                    continue;
                }
            };
            if !self
                .registry
                .lock()
                .unwrap()
                .clients
                .values()
                .any(|client| !client.filters.is_empty())
            {
                continue;
            }
            match tokio::time::timeout(Duration::from_secs(2), source.fetch(id)).await {
                Ok(Ok(Some(event))) => self.deliver(event),
                Ok(Ok(None)) => {}
                _ => self.interrupt(),
            }
        }
    }
}
impl Session {
    pub fn replace(&self, filters: Vec<RelayFilter>) -> Result<()> {
        ensure!(
            filters.len()
                <= crate::protocol_limits::RELAY_FILTERS
                    * crate::protocol_limits::RELAY_SUBSCRIPTIONS,
            "live filter budget"
        );
        let mut registry = self.dispatcher.registry.lock().unwrap();
        let client = registry
            .clients
            .get_mut(&self.id)
            .ok_or_else(|| anyhow!("live feed interrupted"))?;
        client.filters = filters;
        Ok(())
    }
    pub async fn recv(&mut self) -> Result<Arc<Event>> {
        ensure!(!*self.lost.borrow(), "live feed interrupted");
        tokio::select! {
            biased;
            _ = self.lost.changed() => Err(anyhow!("live feed interrupted")),
            event = self.receiver.recv() => {
                ensure!(!*self.lost.borrow(), "live feed interrupted");
                event.ok_or_else(|| anyhow!("live feed closed"))
            }
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.dispatcher
            .registry
            .lock()
            .unwrap()
            .clients
            .remove(&self.id);
    }
}
