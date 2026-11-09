//! TC4 — stress: many tasks mixing locking, heap traffic, sleeping and
//! yielding, followed by a heap-leak check.

use alloc::{sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicU64, Ordering::Relaxed};

use microos_core::rng::XorShift64;

use super::{print_report, Options, Suite};
use crate::memory;
use crate::scheduler::{self, workload::cpu_burst, Config, SpawnOpts};
use crate::sync::Mutex;

const WORKERS: usize = 48;
const ROUNDS: u64 = 10;

struct Shared {
    counter: Mutex<u64>,
    checksum: AtomicU64,
}

/// Size and fill byte of worker `i`'s allocation in `round` (deterministic).
fn allocation(rng: &mut XorShift64, i: usize, round: u64) -> (usize, u8) {
    (rng.range(16, 4096) as usize, (round as u8) ^ (i as u8))
}

pub fn tc4_stress(s: &mut Suite, o: &Options) {
    let heap_before = memory::stats().bytes_in_use;
    let shared = Arc::new(Shared { counter: Mutex::new("stress", 0), checksum: AtomicU64::new(0) });

    let mut expected_checksum = 0u64;
    for i in 0..WORKERS {
        let mut rng = XorShift64::new(i as u64 + 1);
        for round in 0..ROUNDS {
            let (size, fill) = allocation(&mut rng, i, round);
            expected_checksum += size as u64 * fill as u64;
        }
    }

    scheduler::begin(Config { label: "tc4".into(), policy: o.policy, quantum: o.quantum, preemptive: true, ..Config::default() });
    for i in 0..WORKERS {
        let sh = shared.clone();
        scheduler::spawn("W", SpawnOpts { prio: (i % 8) as u8, delay_ticks: (i % 4) as u64, ..SpawnOpts::default() }, move || {
            let mut rng = XorShift64::new(i as u64 + 1);
            for round in 0..ROUNDS {
                *sh.counter.lock() += 1;
                let (size, fill) = allocation(&mut rng, i, round);
                let mut v: Vec<u8> = Vec::with_capacity(size);
                v.resize(size, fill);
                sh.checksum.fetch_add(v.iter().map(|&b| b as u64).sum(), Relaxed);
                drop(v);
                match round % 3 {
                    0 => scheduler::sleep_ticks(1),
                    1 => cpu_burst(1),
                    _ => scheduler::yield_now(),
                }
            }
        })
        .expect("spawn failed");
    }
    let r = scheduler::run();
    print_report(&r);

    let lock = shared.counter.stats();
    let max_stack = r.tasks.iter().map(|t| t.stack_used).max().unwrap_or(0);
    check!(s, "TC4.complete", r.tasks.len() == WORKERS && !r.deadlock, "{} of {} workers finished", r.tasks.len(), WORKERS);
    let shared = Arc::try_unwrap(shared).ok().expect("workers still hold the shared state");
    let counter = shared.counter.into_inner();
    check!(s, "TC4.mutex_counter", counter == WORKERS as u64 * ROUNDS, "counter={} expected={}", counter, WORKERS as u64 * ROUNDS);
    let checksum = shared.checksum.load(Relaxed);
    check!(s, "TC4.heap_data_integrity", checksum == expected_checksum, "checksum={} expected={}", checksum, expected_checksum);
    kinfo!(
        "TC4: mutex acquisitions={} contended={} max_waiters={} max_wait={}us; peak task stack {} of {} bytes",
        lock.acquisitions, lock.contended, lock.max_waiters, lock.max_wait_us, max_stack, crate::task::STACK_SIZE
    );
    drop(r);
    let heap_after = memory::stats().bytes_in_use;
    check!(s, "TC4.no_heap_leak", heap_after == heap_before, "heap in use before={} after={} bytes", heap_before, heap_after);
}
