//! Bounded named observation of long-lived background tasks.
//! A running task is not proof of progress; unexpected exits stay visible.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, AtomicU64, Ordering},
    },
};
#[derive(Default)]
pub struct BackgroundWorkers {
    workers: Mutex<BTreeMap<&'static str, Arc<Worker>>>,
}
struct Worker {
    state: AtomicU8,
    started_at: u64,
    finished_at: AtomicU64,
}
impl BackgroundWorkers {
    pub fn spawn(
        &self,
        name: &'static str,
        future: impl Future<Output = ()> + Send + 'static,
    ) -> tokio::task::AbortHandle {
        let worker = Arc::new(Worker {
            state: AtomicU8::new(0),
            started_at: crate::auth::now(),
            finished_at: AtomicU64::new(0),
        });
        let mut workers = self.workers.lock().unwrap_or_else(|e| e.into_inner());
        assert!(
            workers.len() < 16 && !workers.contains_key(name),
            "background worker registry invariant"
        );
        workers.insert(name, worker.clone());
        drop(workers);
        let task = tokio::spawn(future);
        let abort = task.abort_handle();
        tokio::spawn(async move {
            let state = match task.await {
                Ok(()) => 1,
                Err(error) if error.is_cancelled() => 2,
                Err(_) => 3,
            };
            worker
                .finished_at
                .store(crate::auth::now(), Ordering::Relaxed);
            worker.state.store(state, Ordering::Release);
            tracing::error!(
                worker = name,
                state,
                "long-lived background worker exited; inspect durable state before restart"
            );
        });
        abort
    }
    pub fn snapshot(&self) -> Value {
        let workers = self.workers.lock().unwrap_or_else(|e| e.into_inner());
        let mut failed = false;
        let values:Vec<Value>=workers.iter().map(|(name,w)|{
            let state=w.state.load(Ordering::Acquire);failed|=state!=0;
            json!({"name":name,"state":match state{0=>"running",1=>"returned",2=>"cancelled",_=>"panicked"},"started_at":w.started_at,"finished_at":if state==0{None}else{Some(w.finished_at.load(Ordering::Relaxed))}})
        }).collect();
        json!({"unexpected_exit":failed,"semantics":"task_lifecycle_not_progress_or_health_proof","workers":values})
    }
}
