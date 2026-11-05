//! One cancellation-safe budget for actual volume work. FIFO contenders may
//! bypass only a background class whose allowance is already occupied.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use tokio::sync::Notify;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    Foreground,
    Background,
}

pub(crate) struct Admission {
    total: usize,
    background_normal: usize,
    background_pressure: usize,
    state: Mutex<State>,
    changed: Notify,
}
struct State {
    active: usize,
    background: usize,
    next: u64,
    pressure: bool,
    waiting: VecDeque<(u64, Class)>,
}
pub(crate) struct Permit {
    admission: Arc<Admission>,
    class: Class,
}
struct Waiting {
    admission: Arc<Admission>,
    id: u64,
    registered: bool,
}
impl Admission {
    pub(crate) fn new(total: usize, background_max: usize) -> Arc<Self> {
        Arc::new(Self {
            total,
            // A one-slot volume keeps its historical capacity and interleaves
            // FIFO; simultaneous foreground reservation is impossible there.
            background_normal: background_max
                .min(total.saturating_sub(1).max(1))
                .min((total / 4).max(1)),
            background_pressure: background_max.min(total.saturating_sub(1).max(1)),
            state: Mutex::new(State {
                active: 0,
                background: 0,
                next: 0,
                pressure: false,
                waiting: VecDeque::new(),
            }),
            changed: Notify::new(),
        })
    }
    /// Quota free bytes count unused and unreserved credits, not merely
    /// unallocated grants. Filesystem space is measured separately. A failed
    /// filesystem sample cannot prematurely clear an existing pressure state.
    pub(crate) fn observe_headroom(&self, quota: (u64, u64), filesystem: Option<(u64, u64)>) {
        fn below((free, total): (u64, u64), percent: u128) -> bool {
            u128::from(free) * 100 <= u128::from(total) * percent
        }
        let mut state = self.state.lock().unwrap();
        if below(quota, 10) || filesystem.is_some_and(|space| below(space, 10)) {
            state.pressure = true;
        } else if !below(quota, 20) && filesystem.is_some_and(|space| !below(space, 20)) {
            state.pressure = false;
        }
        drop(state);
        self.changed.notify_waiters();
    }
    pub(crate) async fn acquire(self: &Arc<Self>, class: Class) -> Arc<Permit> {
        let mut waiting = {
            let mut state = self.state.lock().unwrap();
            let id = state.next;
            state.next = state.next.wrapping_add(1);
            state.waiting.push_back((id, class));
            Waiting {
                admission: self.clone(),
                id,
                registered: true,
            }
        };
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let granted = {
                let mut state = self.state.lock().unwrap();
                let background_limit = if state.pressure {
                    self.background_pressure
                } else {
                    self.background_normal
                };
                let eligible = state.waiting.iter().position(|(_, kind)| {
                    *kind == Class::Foreground || state.background < background_limit
                });
                if state.active < self.total
                    && eligible.is_some_and(|n| state.waiting[n].0 == waiting.id)
                {
                    state.waiting.remove(eligible.unwrap());
                    state.active += 1;
                    if class == Class::Background {
                        state.background += 1;
                    }
                    waiting.registered = false;
                    true
                } else {
                    false
                }
            };
            if granted {
                self.changed.notify_waiters();
                return Arc::new(Permit {
                    admission: self.clone(),
                    class,
                });
            }
            changed.await;
        }
    }
}
impl Drop for Waiting {
    fn drop(&mut self) {
        if self.registered {
            self.admission
                .state
                .lock()
                .unwrap()
                .waiting
                .retain(|(id, _)| *id != self.id);
            self.admission.changed.notify_waiters();
        }
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        let mut state = self.admission.state.lock().unwrap();
        state.active -= 1;
        if self.class == Class::Background {
            state.background -= 1;
        }
        drop(state);
        self.admission.changed.notify_waiters();
    }
}
