//! Interrupt-safe critical sections for kernel-internal data.
//!
//! On this single-hart kernel the only source of concurrency inside the
//! kernel is the timer interrupt, so "disable interrupts, then take the
//! lock" is both necessary and sufficient. The atomic flag is kept to detect
//! re-entrancy bugs: if the flag is already set while interrupts are off on
//! the only hart, the holder can never run again, so we panic with the lock's
//! name instead of hanging silently.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

use crate::arch;

/// Disables interrupts on creation and restores the previous state on drop,
/// so guards nest correctly.
pub struct IrqGuard {
    was_enabled: bool,
}

impl IrqGuard {
    pub fn new() -> Self {
        let was_enabled = arch::intr_enabled();
        arch::intr_off();
        Self { was_enabled }
    }
}

impl Drop for IrqGuard {
    fn drop(&mut self) {
        if self.was_enabled {
            arch::intr_on();
        }
    }
}

pub struct IrqSpinLock<T> {
    name: &'static str,
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

// SAFETY: `data` is reachable only through a guard; a guard exists only while
// `locked` is set and interrupts are disabled, so neither an interrupt
// handler nor (on a multi-hart system) another hart can access it concurrently.
unsafe impl<T: Send> Sync for IrqSpinLock<T> {}
unsafe impl<T: Send> Send for IrqSpinLock<T> {}

impl<T> IrqSpinLock<T> {
    pub const fn new(name: &'static str, value: T) -> Self {
        Self { name, locked: AtomicBool::new(false), data: UnsafeCell::new(value) }
    }

    pub fn lock(&self) -> IrqSpinGuard<'_, T> {
        let irq = IrqGuard::new();
        if self.locked.swap(true, Ordering::Acquire) {
            panic!("deadlock: IrqSpinLock '{}' re-acquired while already held", self.name);
        }
        IrqSpinGuard { lock: self, _irq: irq }
    }

    /// Like `lock`, but returns `None` instead of panicking if held (used by
    /// the panic handler).
    pub fn try_lock(&self) -> Option<IrqSpinGuard<'_, T>> {
        let irq = IrqGuard::new();
        if self.locked.swap(true, Ordering::Acquire) {
            return None;
        }
        Some(IrqSpinGuard { lock: self, _irq: irq })
    }
}

/// Field order matters: `Drop::drop` releases the flag first, then `_irq` is
/// dropped and re-enables interrupts.
pub struct IrqSpinGuard<'a, T> {
    lock: &'a IrqSpinLock<T>,
    _irq: IrqGuard,
}

impl<T> Deref for IrqSpinGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: we hold the lock.
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for IrqSpinGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: we hold the lock exclusively.
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for IrqSpinGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}
