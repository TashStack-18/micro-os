//! Layer 6 — periodic timer. Each tick re-arms the SBI timer and advances a
//! global tick counter; the scheduler decides what a tick means.

use core::sync::atomic::{AtomicU64, Ordering::Relaxed};

use crate::{arch, drivers::sbi};

/// Frequency of the `time` CSR on QEMU virt.
pub const TIMEBASE_HZ: u64 = 10_000_000;
const UNITS_PER_US: u64 = TIMEBASE_HZ / 1_000_000;

static TICKS: AtomicU64 = AtomicU64::new(0);
static INTERVAL: AtomicU64 = AtomicU64::new(0);

pub fn init(tick_us: u64) {
    INTERVAL.store(tick_us.max(10) * UNITS_PER_US, Relaxed);
    arch::enable_timer_interrupt();
    arm();
}

fn arm() {
    sbi::set_timer(arch::read_time() + INTERVAL.load(Relaxed));
}

/// Acknowledge a timer interrupt: schedule the next one and count the tick.
pub fn handle_tick() -> u64 {
    arm();
    TICKS.fetch_add(1, Relaxed) + 1
}

pub fn ticks() -> u64 {
    TICKS.load(Relaxed)
}

pub fn now_us() -> u64 {
    arch::read_time() / UNITS_PER_US
}

/// Raw timebase units (100 ns each) for fine-grained measurements.
pub fn now_raw() -> u64 {
    arch::read_time()
}

/// Length of one timer tick in microseconds.
pub fn tick_us() -> u64 {
    (INTERVAL.load(Relaxed) / UNITS_PER_US).max(1)
}

pub fn raw_to_ns(raw: u64) -> u64 {
    raw * (1_000_000_000 / TIMEBASE_HZ)
}
