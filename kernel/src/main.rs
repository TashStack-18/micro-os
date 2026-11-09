//! Micro-OS: an educational RISC-V kernel in Rust (CA-6).
//!
//! Boot path: OpenSBI (M-mode) → `_start` (boot/entry.S) → `kmain` (S-mode).
//! `kmain` initialises each layer in order, then runs the scenarios selected
//! on the kernel command line and exits QEMU with the number of failed checks.

#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
mod kernel;

mod arch;
mod boot;
mod drivers;
mod memory;
mod programs;
mod scenarios;
mod scheduler;
mod shell;
mod sync;
mod sys;
mod task;
mod timer;
mod trap;

use drivers::{qemu_exit, sbi};
use kernel::console::{self, Level};

#[no_mangle]
pub extern "C" fn kmain(hartid: usize, dtb: usize) -> ! {
    println!();
    kinfo!("Micro-OS v{} booting on hart {} (riscv64, S-mode)", env!("CARGO_PKG_VERSION"), hartid);
    ktrace!("BOOT", "stage=entry hart={} dtb={:#x}", hartid, dtb);
    let (major, minor) = sbi::spec_version();
    kinfo!("firmware: {} implementing SBI v{}.{}", sbi::impl_name(), major, minor);

    let args = boot::boot_args(dtb);
    if let Some(level) = args.get("log").and_then(Level::parse) {
        console::set_level(level);
    }
    console::set_trace(args.flag("trace", true));
    kinfo!("command line: '{}'", args.raw());
    ktrace!("BOOT", "stage=args");

    trap::init();
    kinfo!("trap vector installed");
    ktrace!("BOOT", "stage=trap");

    let (heap_start, heap_size) = memory::init();
    let (kstart, kend) = memory::kernel_image();
    kinfo!("kernel image {:#x}..{:#x} ({} KiB)", kstart, kend, (kend - kstart) / 1024);
    kinfo!("heap {:#x}..{:#x} ({} KiB, first-fit free list)", heap_start, heap_start + heap_size, heap_size / 1024);
    ktrace!("BOOT", "stage=heap start={:#x} size={}", heap_start, heap_size);

    let tick_us = args.get_u64("tick_us").unwrap_or(1000);
    timer::init(tick_us);
    arch::intr_on();
    kinfo!("timer running: one tick every {} us", tick_us);
    ktrace!("BOOT", "stage=timer tick_us={}", tick_us);

    let test = args.get("test").unwrap_or("all");
    let failed = scenarios::run(test, &args);

    let h = memory::stats();
    kinfo!(
        "heap at shutdown: in_use={} B peak={} B allocs={} frees={} failed={} rejected_frees={} free_blocks={}",
        h.bytes_in_use, h.peak_bytes, h.allocs, h.frees, h.failed, h.rejected_frees, h.free_blocks
    );
    ktrace!(
        "HEAP",
        "in_use={} peak={} allocs={} frees={} failed={} rejected={} free_blocks={} largest={}",
        h.bytes_in_use, h.peak_bytes, h.allocs, h.frees, h.failed, h.rejected_frees, h.free_blocks, h.largest_free
    );
    kinfo!("ticks={} - shutting down with exit code {}", timer::ticks(), failed);
    ktrace!("SHUTDOWN", "failed={}", failed);
    qemu_exit::exit(failed)
}
