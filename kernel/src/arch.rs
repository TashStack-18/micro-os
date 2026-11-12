//! RISC-V supervisor-mode CSR access. Every inline-asm block in the kernel
//! outside the three `.S` files lives here, behind small safe functions.
//!
//! The interrupt on/off helpers deliberately omit `nomem` so they also act
//! as compiler barriers: memory accesses cannot be moved across them.

use core::arch::asm;

pub const SSTATUS_SIE: usize = 1 << 1;
pub const SIE_STIE: usize = 1 << 5;
pub const SCAUSE_INTERRUPT: usize = 1 << 63;
pub const IRQ_S_TIMER: usize = 5;

/// Value of the `time` CSR (10 MHz on QEMU virt).
#[inline]
pub fn read_time() -> u64 {
    let t: u64;
    // SAFETY: reading `time` has no side effects.
    unsafe { asm!("rdtime {}", out(reg) t, options(nomem, nostack)) };
    t
}

#[inline]
pub fn intr_on() {
    // SAFETY: setting sstatus.SIE only allows pending, enabled interrupts in.
    unsafe { asm!("csrsi sstatus, 2", options(nostack)) };
}

#[inline]
pub fn intr_off() {
    // SAFETY: clearing sstatus.SIE only masks interrupts.
    unsafe { asm!("csrci sstatus, 2", options(nostack)) };
}

#[inline]
pub fn intr_enabled() -> bool {
    read_sstatus() & SSTATUS_SIE != 0
}

#[inline]
pub fn read_sstatus() -> usize {
    let v: usize;
    // SAFETY: plain CSR read.
    unsafe { asm!("csrr {}, sstatus", out(reg) v, options(nomem, nostack)) };
    v
}

pub fn enable_timer_interrupt() {
    // SAFETY: unmasks the supervisor timer interrupt source only.
    unsafe { asm!("csrs sie, {}", in(reg) SIE_STIE, options(nostack)) };
}

/// Install the trap vector (direct mode: all traps enter at `addr`).
///
/// # Safety
/// `addr` must be the 4-byte-aligned address of a correct trap entry routine.
pub unsafe fn write_stvec(addr: usize) {
    asm!("csrw stvec, {}", in(reg) addr, options(nostack));
}

/// Mark "currently in the kernel" for trap.S (see the sscratch protocol there).
pub fn clear_sscratch() {
    // SAFETY: sscratch is only read by trap.S; zero means "trap from S-mode",
    // which is true whenever this runs.
    unsafe { asm!("csrw sscratch, zero", options(nomem, nostack)) };
}

pub fn read_scause() -> usize {
    let v: usize;
    // SAFETY: plain CSR read.
    unsafe { asm!("csrr {}, scause", out(reg) v, options(nomem, nostack)) };
    v
}

pub fn read_stval() -> usize {
    let v: usize;
    // SAFETY: plain CSR read.
    unsafe { asm!("csrr {}, stval", out(reg) v, options(nomem, nostack)) };
    v
}

/// Current frame pointer (s0), for backtraces.
#[inline(always)]
pub fn read_fp() -> usize {
    let v: usize;
    // SAFETY: copies a register.
    unsafe { asm!("mv {}, s0", out(reg) v, options(nomem, nostack)) };
    v
}

/// Wait for an interrupt. Returns when one is pending *and enabled in `sie`*,
/// even if sstatus.SIE is clear, which is what the idle loop relies on.
#[inline]
pub fn wfi() {
    // SAFETY: wfi is a hint; the worst case is that it returns immediately.
    unsafe { asm!("wfi", options(nomem, nostack)) };
}
