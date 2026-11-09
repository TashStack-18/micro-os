//! Test-and-test-and-set spinlock with contention counters.
//!
//! What a waiter does while the lock is held is chosen by the caller through
//! `lock_with(relax)`: pure busy-waiting (`spin_loop`) or giving the CPU away
//! (the kernel passes `yield_now`). On a uniprocessor that choice decides
//! whether a waiter burns its whole time slice while the holder is preempted.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub struct SpinLock<T> {
    locked: AtomicBool,
    acquisitions: AtomicU64,
    contended: AtomicU64,
    spins: AtomicU64,
    data: UnsafeCell<T>,
}

// SAFETY: `data` is only reachable through a `SpinGuard`, and a guard exists
// only while `locked` is true. Acquire/Release on `locked` orders the
// accesses of successive holders.
unsafe impl<T: Send> Sync for SpinLock<T> {}
unsafe impl<T: Send> Send for SpinLock<T> {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpinStats {
    pub acquisitions: u64,
    /// Acquisitions that found the lock already held.
    pub contended: u64,
    /// Total `relax()` calls made while waiting.
    pub spins: u64,
}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            acquisitions: AtomicU64::new(0),
            contended: AtomicU64::new(0),
            spins: AtomicU64::new(0),
            data: UnsafeCell::new(value),
        }
    }

    pub fn try_lock(&self) -> Option<SpinGuard<'_, T>> {
        self.locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| {
                self.acquisitions.fetch_add(1, Ordering::Relaxed);
                SpinGuard { lock: self }
            })
    }

    /// Busy-wait until the lock is free.
    pub fn lock(&self) -> SpinGuard<'_, T> {
        self.lock_with(core::hint::spin_loop)
    }

    /// Wait for the lock, calling `relax` on every failed poll.
    pub fn lock_with(&self, mut relax: impl FnMut()) -> SpinGuard<'_, T> {
        let mut spins = 0u64;
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Spin on a plain load so waiters do not hammer the line with writes.
            while self.locked.load(Ordering::Relaxed) {
                relax();
                spins += 1;
            }
        }
        self.acquisitions.fetch_add(1, Ordering::Relaxed);
        if spins > 0 {
            self.contended.fetch_add(1, Ordering::Relaxed);
            self.spins.fetch_add(spins, Ordering::Relaxed);
        }
        SpinGuard { lock: self }
    }

    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Relaxed)
    }

    pub fn stats(&self) -> SpinStats {
        SpinStats {
            acquisitions: self.acquisitions.load(Ordering::Relaxed),
            contended: self.contended.load(Ordering::Relaxed),
            spins: self.spins.load(Ordering::Relaxed),
        }
    }
}

pub struct SpinGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T> Deref for SpinGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: holding the guard means we own the lock.
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for SpinGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: holding the guard means we own the lock exclusively.
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for SpinGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn mutual_exclusion_across_threads() {
        let lock = Arc::new(SpinLock::new(0u64));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let lock = lock.clone();
                std::thread::spawn(move || {
                    for _ in 0..20_000 {
                        *lock.lock() += 1;
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(*lock.lock(), 80_000);
        assert_eq!(lock.stats().acquisitions, 80_001);
    }

    #[test]
    fn try_lock_fails_while_held_and_relax_is_called() {
        let lock = SpinLock::new(());
        let g = lock.try_lock().unwrap();
        assert!(lock.try_lock().is_none());
        drop(g);
        assert!(!lock.is_locked());

        // A relax callback that releases the lock proves waiters call it.
        let lock = SpinLock::new(1);
        let g = lock.lock();
        let mut held = Some(g);
        let g2 = lock.lock_with(|| {
            held.take();
        });
        assert_eq!(*g2, 1);
        assert_eq!(lock.stats().contended, 1);
    }
}
