//! A CPU hog. Run two in the background (`spin 10 &` twice), then use `ps`
//! or `top`: the scheduler splits the CPU between them, and each reports
//! what share of the wall-clock time it actually received.
//!
//! `spin 5 yield` is the polite version: it calls `sys::yield_now()` after
//! every chunk of work, handing the CPU to anyone else who is ready.

use super::arg_u64;
use crate::sys;

pub fn main(args: &[&str]) -> i32 {
    let seconds = arg_u64(args, 0, 5);
    let polite = args.get(1) == Some(&"yield");
    let start = sys::uptime_ms();
    let mut loops = 0u64;
    while sys::uptime_ms() - start < seconds * 1000 {
        if sys::interrupted() {
            break;
        }
        for i in 0..1000u32 {
            core::hint::black_box(i);
        }
        loops += 1;
        if polite {
            sys::yield_now();
        }
    }
    let wall_ms = (sys::uptime_ms() - start).max(1);
    let cpu_ms = sys::cpu_ticks() * sys::tick_us() / 1000;
    println!(
        "[spin pid {}] {} ms wall, ~{} ms CPU ({}% share), {} loops",
        sys::getpid(), wall_ms, cpu_ms, cpu_ms * 100 / wall_ms, loops
    );
    0
}
