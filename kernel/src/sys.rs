//! Layer 9 — system interface for programs.
//!
//! Programs in `src/programs/` should use only this module (plus `println!`
//! and `alloc` collections). It is the boundary between "application" code
//! and kernel internals, like a system-call table. These programs run in
//! S-mode as kernel tasks, so the boundary is by convention only. Programs
//! that need a hardware-enforced boundary run in U-mode instead and reach
//! the kernel through real `ecall` system calls: see `crate::user`.

use crate::{memory, scheduler, timer};

/// Task id of the calling program.
pub fn getpid() -> usize {
    scheduler::current_tid().unwrap_or(0)
}

/// Milliseconds since boot.
pub fn uptime_ms() -> u64 {
    timer::now_us() / 1000
}

/// Timer ticks this program has been charged for (its CPU time).
pub fn cpu_ticks() -> u64 {
    scheduler::current_cpu_ticks()
}

/// Length of one timer tick in microseconds.
pub fn tick_us() -> u64 {
    timer::tick_us()
}

/// Sleep (BLOCKED, using no CPU) for at least `ms` milliseconds.
/// Returns `false` if the user pressed Ctrl-C, so callers can stop early.
pub fn sleep_ms(ms: u64) -> bool {
    let ticks = (ms * 1000).div_ceil(timer::tick_us()).max(1);
    scheduler::sleep_ticks(ticks);
    !interrupted()
}

/// Give the CPU to another ready task.
pub fn yield_now() {
    scheduler::yield_now();
}

/// Has the user asked this program to stop (Ctrl-C or `kill`)?
/// Long-running loops should check this regularly.
pub fn interrupted() -> bool {
    scheduler::cancel_requested()
}

/// Bytes currently allocated on the kernel heap (all tasks together).
pub fn heap_used() -> usize {
    memory::stats().bytes_in_use
}
