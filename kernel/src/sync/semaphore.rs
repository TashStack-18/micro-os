//! Counting semaphore with a FIFO wait queue. `signal` passes the permit
//! straight to the oldest waiter if there is one, otherwise banks it.

use alloc::collections::VecDeque;

use microos_core::sched::Tid;

use super::{IrqGuard, IrqSpinLock};
use crate::scheduler;

#[derive(Clone, Copy, Debug, Default)]
pub struct SemStats {
    pub waits: u64,
    /// `wait` calls that had to block.
    pub blocked: u64,
    pub signals: u64,
}

struct State {
    permits: usize,
    waiters: VecDeque<Tid>,
    stats: SemStats,
}

pub struct Semaphore {
    name: &'static str,
    state: IrqSpinLock<State>,
}

impl Semaphore {
    pub const fn new(name: &'static str, permits: usize) -> Self {
        Self {
            name,
            state: IrqSpinLock::new(
                "semaphore",
                State { permits, waiters: VecDeque::new(), stats: SemStats { waits: 0, blocked: 0, signals: 0 } },
            ),
        }
    }

    /// P / down: take a permit, blocking until one is available.
    pub fn wait(&self) {
        let me = scheduler::current_tid().expect("Semaphore::wait called outside a task");
        let _irq = IrqGuard::new();
        {
            let mut st = self.state.lock();
            st.stats.waits += 1;
            if st.permits > 0 {
                st.permits -= 1;
                return;
            }
            st.stats.blocked += 1;
            kdebug!("semaphore '{}': tid {} blocks (no permits)", self.name, me);
            st.waiters.push_back(me);
            scheduler::mark_current_blocked();
        }
        scheduler::switch_to_scheduler();
    }

    /// V / up: release a permit, waking the oldest waiter if any.
    pub fn signal(&self) {
        let waiter = {
            let mut st = self.state.lock();
            st.stats.signals += 1;
            let w = st.waiters.pop_front();
            if w.is_none() {
                st.permits += 1;
            }
            w
        };
        if let Some(tid) = waiter {
            scheduler::wake(tid);
        }
    }

    pub fn stats(&self) -> SemStats {
        self.state.lock().stats
    }
}
