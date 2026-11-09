//! Blocking mutex with FIFO hand-off.
//!
//! A task that finds the mutex held is put on the mutex's wait queue and
//! BLOCKED, so it uses no CPU while waiting. `unlock` hands ownership
//! directly to the longest waiter before waking it, so there is no
//! "thundering herd" and no barging: waiters acquire in arrival order.

use alloc::collections::VecDeque;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};

use microos_core::sched::Tid;

use super::{IrqGuard, IrqSpinLock, SyncError};
use crate::{scheduler, timer};

#[derive(Clone, Copy, Debug, Default)]
pub struct LockStats {
    pub acquisitions: u64,
    pub contended: u64,
    pub total_wait_us: u64,
    pub max_wait_us: u64,
    pub max_waiters: usize,
}

impl LockStats {
    const fn new() -> Self {
        Self { acquisitions: 0, contended: 0, total_wait_us: 0, max_wait_us: 0, max_waiters: 0 }
    }
}

struct State {
    owner: Option<Tid>,
    waiters: VecDeque<(Tid, u64)>,
    stats: LockStats,
}

pub struct Mutex<T> {
    name: &'static str,
    state: IrqSpinLock<State>,
    data: UnsafeCell<T>,
}

// SAFETY: `data` is reachable only through a MutexGuard, which exists only
// for the single task recorded as `owner` (updated under `state`).
unsafe impl<T: Send> Sync for Mutex<T> {}
unsafe impl<T: Send> Send for Mutex<T> {}

impl<T> Mutex<T> {
    pub const fn new(name: &'static str, value: T) -> Self {
        Self {
            name,
            state: IrqSpinLock::new("mutex", State { owner: None, waiters: VecDeque::new(), stats: LockStats::new() }),
            data: UnsafeCell::new(value),
        }
    }

    /// Acquire, blocking the calling task while another task owns the mutex.
    /// Must be called from a task.
    pub fn lock(&self) -> MutexGuard<'_, T> {
        let me = scheduler::current_tid().expect("Mutex::lock called outside a task");
        // Interrupts stay off from the ownership check until we have switched
        // away, so the owner cannot unlock (and fail to find us) in between.
        let _irq = IrqGuard::new();
        {
            let mut st = self.state.lock();
            st.stats.acquisitions += 1;
            match st.owner {
                None => {
                    st.owner = Some(me);
                    return MutexGuard { mutex: self };
                }
                Some(owner) if owner == me => {
                    panic!("mutex '{}': task {} locked it twice (self-deadlock)", self.name, me)
                }
                Some(_) => {
                    st.stats.contended += 1;
                    kdebug!("mutex '{}': tid {} blocks behind tid {:?}", self.name, me, st.owner);
                    st.waiters.push_back((me, timer::now_us()));
                    st.stats.max_waiters = st.stats.max_waiters.max(st.waiters.len());
                    scheduler::mark_current_blocked();
                }
            }
        }
        scheduler::switch_to_scheduler();
        // We only run again after unlock() made us the owner.
        debug_assert_eq!(self.state.lock().owner, Some(me));
        MutexGuard { mutex: self }
    }

    /// Release the mutex as the current task.
    ///
    /// # Safety
    /// The caller must not hold a `MutexGuard` for this mutex (the guard
    /// would then outlive the ownership it represents). This exists so tests
    /// can demonstrate that invalid unlocks are rejected.
    pub unsafe fn force_unlock(&self) -> Result<(), SyncError> {
        self.unlock_as(scheduler::current_tid())
    }

    fn unlock_as(&self, me: Option<Tid>) -> Result<(), SyncError> {
        let next = {
            let mut st = self.state.lock();
            match st.owner {
                None => return Err(SyncError::NotLocked),
                Some(owner) if Some(owner) != me => return Err(SyncError::NotOwner),
                Some(_) => {}
            }
            match st.waiters.pop_front() {
                Some((tid, since)) => {
                    let waited = timer::now_us().saturating_sub(since);
                    st.stats.total_wait_us += waited;
                    st.stats.max_wait_us = st.stats.max_wait_us.max(waited);
                    st.owner = Some(tid);
                    Some(tid)
                }
                None => {
                    st.owner = None;
                    None
                }
            }
        };
        if let Some(tid) = next {
            scheduler::wake(tid);
        }
        Ok(())
    }

    pub fn stats(&self) -> LockStats {
        self.state.lock().stats
    }

    /// Consume the mutex; ownership of `self` proves nobody else can hold it.
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: the guard proves the current task owns the mutex.
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: the guard proves exclusive ownership.
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        if let Err(e) = self.mutex.unlock_as(scheduler::current_tid()) {
            panic!("mutex '{}': guard release failed: {:?}", self.mutex.name, e);
        }
    }
}
