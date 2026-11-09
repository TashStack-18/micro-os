//! Panic handler: report what happened, where, and in which task, then exit
//! QEMU with a failure code so test scripts notice.

use core::fmt::Write;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::{arch, drivers::qemu_exit, kernel::console, scheduler, timer};

pub const PANIC_EXIT_CODE: u32 = 101;
const RAM_START: usize = 0x8000_0000;
const RAM_END: usize = 0x8800_0000;

static PANICKING: AtomicBool = AtomicBool::new(false);

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    arch::intr_off();
    let mut w = console::emergency_writer();
    if PANICKING.swap(true, Ordering::SeqCst) {
        let _ = writeln!(w, "\n!!! nested panic: {}", info);
        qemu_exit::exit(PANIC_EXIT_CODE);
    }
    let _ = writeln!(w, "\n!!! KERNEL PANIC: {}", info);
    match scheduler::try_current() {
        Some(Some((tid, name))) => {
            let _ = writeln!(w, "!!! running task: tid={} name={}", tid, name);
        }
        Some(None) => {
            let _ = writeln!(w, "!!! running in: scheduler/boot context");
        }
        None => {
            let _ = writeln!(w, "!!! running task: unknown (scheduler lock held)");
        }
    }
    let _ = writeln!(w, "!!! backtrace (symbolize with scripts/symbolize.sh):");
    backtrace(&mut w);
    let _ = writeln!(w, "#T {} {} PANIC", timer::now_us(), timer::ticks());
    qemu_exit::exit(PANIC_EXIT_CODE)
}

/// Walk the frame-pointer chain. With `-C force-frame-pointers`, each frame
/// stores the return address at `fp - 8` and the caller's fp at `fp - 16`.
fn backtrace(w: &mut impl Write) {
    let mut fp = arch::read_fp();
    for depth in 0..24 {
        if !(RAM_START + 16..RAM_END).contains(&fp) || fp % 8 != 0 {
            break;
        }
        // SAFETY: fp was range- and alignment-checked against RAM above, and
        // reads have no side effects; a bogus chain just ends the walk.
        let (ra, prev) = unsafe { (*((fp - 8) as *const usize), *((fp - 16) as *const usize)) };
        if ra == 0 {
            break;
        }
        let _ = writeln!(w, "  #{:<2} ra={:#x}", depth, ra);
        if prev <= fp {
            break; // stacks grow down, so callers' frames are at higher addresses
        }
        fp = prev;
    }
}
