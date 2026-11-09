//! Supervisor Binary Interface calls into the M-mode firmware (OpenSBI).
//! The kernel runs in S-mode, so programming the timer and powering off
//! go through `ecall`.

use core::arch::asm;

const EXT_BASE: usize = 0x10;
const EXT_TIME: usize = 0x5449_4D45; // "TIME"
const EXT_SRST: usize = 0x5352_5354; // "SRST"
const LEGACY_SET_TIMER: usize = 0x00;
const LEGACY_SHUTDOWN: usize = 0x08;

#[derive(Clone, Copy, Debug)]
pub struct SbiRet {
    pub error: isize,
    pub value: usize,
}

#[inline(always)]
fn ecall(eid: usize, fid: usize, arg0: usize, arg1: usize) -> SbiRet {
    let (error, value): (usize, usize);
    // SAFETY: SBI calling convention: arguments in a0-a1, function id in a6,
    // extension id in a7; the firmware returns error/value in a0/a1 and
    // preserves every other register.
    unsafe {
        asm!("ecall",
             inlateout("a0") arg0 => error,
             inlateout("a1") arg1 => value,
             in("a6") fid,
             in("a7") eid,
             options(nostack));
    }
    SbiRet { error: error as isize, value }
}

/// SBI specification version as (major, minor).
pub fn spec_version() -> (usize, usize) {
    let v = ecall(EXT_BASE, 0, 0, 0).value;
    ((v >> 24) & 0x7f, v & 0xff_ffff)
}

pub fn impl_name() -> &'static str {
    match ecall(EXT_BASE, 1, 0, 0).value {
        0 => "BBL",
        1 => "OpenSBI",
        2 => "Xvisor",
        3 => "KVM",
        4 => "RustSBI",
        5 => "Diosix",
        _ => "unknown",
    }
}

/// Request a supervisor timer interrupt when `time >= stime`. This also
/// clears any pending timer interrupt.
pub fn set_timer(stime: u64) {
    if ecall(EXT_TIME, 0, stime as usize, 0).error != 0 {
        // Firmware without the TIME extension: use the legacy call.
        ecall(LEGACY_SET_TIMER, 0, stime as usize, 0);
    }
}

/// Power off through SBI (used only if the QEMU test device is unavailable).
pub fn system_reset(failure: bool) {
    ecall(EXT_SRST, 0, 0, failure as usize);
    ecall(LEGACY_SHUTDOWN, 0, 0, 0);
}
