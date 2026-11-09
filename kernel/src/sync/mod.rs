//! Layer 7 — synchronization.
//!
//! | primitive      | waits by            | used for                                  |
//! |----------------|---------------------|-------------------------------------------|
//! | `IrqSpinLock`  | masking interrupts  | kernel-internal data (scheduler, heap, console) |
//! | `SpinLock`     | busy-wait or yield  | short task-level critical sections        |
//! | `Mutex`        | blocking (sleeping) | task-level critical sections of any length|
//! | `Semaphore`    | blocking (sleeping) | counting resources, producer/consumer     |

mod irq;
mod mutex;
mod semaphore;

pub use irq::{IrqGuard, IrqSpinLock};
pub use microos_core::spin::SpinLock;
pub use mutex::Mutex;
pub use semaphore::Semaphore;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncError {
    /// Unlock of a mutex that is not locked.
    NotLocked,
    /// Unlock by a task that does not own the mutex.
    NotOwner,
}
