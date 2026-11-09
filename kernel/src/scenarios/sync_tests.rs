//! Synchronization scenarios: a reproducible race and its fixes, and a
//! bounded-buffer producer/consumer built from semaphores and a mutex.

use alloc::{collections::VecDeque, format, sync::Arc};
use core::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering::Relaxed};

use microos_core::sched::PolicyKind;

use super::{print_report, Options, Suite};
use crate::scheduler::{self, workload::spin, Config, SpawnOpts};
use crate::sync::{Mutex, Semaphore, SpinLock};

const WORKERS: u64 = 4;
const INCREMENTS: u64 = 1000;
/// Work done between reading and writing the counter: the race window.
const WINDOW: u32 = 300;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    Unsync,
    Spin,
    SpinYield,
    Mutex,
}

impl Variant {
    fn name(self) -> &'static str {
        match self {
            Variant::Unsync => "unsync",
            Variant::Spin => "spin",
            Variant::SpinYield => "spin_yield",
            Variant::Mutex => "mutex",
        }
    }
}

struct Counter {
    racy: AtomicU64,
    spin: SpinLock<u64>,
    mutex: Mutex<u64>,
}

/// Shared counter incremented by 4 preemptible tasks, four ways:
/// unsynchronized (atomic load + store, but not an atomic read-modify-write),
/// busy-wait spinlock, spin-then-yield, and the blocking mutex.
pub fn race(s: &mut Suite, _o: &Options) {
    let expected = WORKERS * INCREMENTS;
    for v in [Variant::Unsync, Variant::Spin, Variant::SpinYield, Variant::Mutex] {
        let c = Arc::new(Counter { racy: AtomicU64::new(0), spin: SpinLock::new(0), mutex: Mutex::new("race", 0) });
        scheduler::begin(Config { label: format!("race_{}", v.name()), policy: PolicyKind::RoundRobin, quantum: 2, preemptive: true });
        for _ in 0..WORKERS {
            let c = c.clone();
            scheduler::spawn("INC", SpawnOpts::default(), move || {
                for _ in 0..INCREMENTS {
                    match v {
                        Variant::Unsync => {
                            let x = c.racy.load(Relaxed);
                            spin(WINDOW);
                            c.racy.store(x + 1, Relaxed);
                        }
                        Variant::Spin => {
                            let mut g = c.spin.lock();
                            let x = *g;
                            spin(WINDOW);
                            *g = x + 1;
                        }
                        Variant::SpinYield => {
                            let mut g = c.spin.lock_with(scheduler::yield_now);
                            let x = *g;
                            spin(WINDOW);
                            *g = x + 1;
                        }
                        Variant::Mutex => {
                            let mut g = c.mutex.lock();
                            let x = *g;
                            spin(WINDOW);
                            *g = x + 1;
                        }
                    }
                }
            })
            .expect("spawn failed");
        }
        let r = scheduler::run();
        print_report(&r);

        let c = Arc::try_unwrap(c).ok().expect("workers still hold the counter");
        let spin_stats = c.spin.stats();
        let mstats = c.mutex.stats();
        let result = match v {
            Variant::Unsync => c.racy.load(Relaxed),
            Variant::Spin | Variant::SpinYield => *c.spin.lock(),
            Variant::Mutex => c.mutex.into_inner(),
        };
        let (contended, spins, wait_us) = match v {
            Variant::Mutex => (mstats.contended, 0, mstats.total_wait_us),
            _ => (spin_stats.contended, spin_stats.spins, 0),
        };
        ktrace!(
            "LOCK",
            "variant={} result={} expected={} lost={} elapsed_us={} contended={} spins={} wait_us={} preemptions={} yields={} blocks={}",
            v.name(), result, expected, expected - result, r.elapsed_us, contended, spins, wait_us,
            r.stats.preemptions, r.stats.yields, r.stats.blocks
        );
        kinfo!(
            "race/{}: result={} of {} elapsed={}us contended={} spins={} mutex_wait={}us",
            v.name(), result, expected, r.elapsed_us, contended, spins, wait_us
        );
        if v == Variant::Unsync {
            check!(s, "RACE.unsync_loses_updates", result < expected, "lost {} of {} increments to preemption", expected - result, expected);
        } else {
            check!(s, "RACE.protected_counter_exact", result == expected, "{}: {} of {}", v.name(), result, expected);
        }
    }
}

const CAPACITY: usize = 4;
const ITEMS_PER_PRODUCER: u64 = 25;

struct BoundedBuffer {
    buf: Mutex<VecDeque<u64>>,
    empty_slots: Semaphore,
    full_slots: Semaphore,
    consumed: AtomicU32,
    sum: AtomicU64,
    max_len: AtomicUsize,
}

pub fn producer_consumer(s: &mut Suite, _o: &Options) {
    let bb = Arc::new(BoundedBuffer {
        buf: Mutex::new("buffer", VecDeque::new()),
        empty_slots: Semaphore::new("empty", CAPACITY),
        full_slots: Semaphore::new("full", 0),
        consumed: AtomicU32::new(0),
        sum: AtomicU64::new(0),
        max_len: AtomicUsize::new(0),
    });
    scheduler::begin(Config { label: "prodcons".into(), policy: PolicyKind::RoundRobin, quantum: 2, preemptive: true });
    for p in 1..=2u64 {
        let bb = bb.clone();
        scheduler::spawn("PROD", SpawnOpts::default(), move || {
            for i in 0..ITEMS_PER_PRODUCER {
                bb.empty_slots.wait();
                {
                    let mut b = bb.buf.lock();
                    b.push_back(p * 1000 + i);
                    bb.max_len.fetch_max(b.len(), Relaxed);
                }
                bb.full_slots.signal();
                if i % 5 == 4 {
                    scheduler::sleep_ticks(1);
                }
            }
        })
        .unwrap();
    }
    for _ in 0..2 {
        let bb = bb.clone();
        scheduler::spawn("CONS", SpawnOpts::default(), move || {
            for _ in 0..ITEMS_PER_PRODUCER {
                bb.full_slots.wait();
                let item = bb.buf.lock().pop_front().expect("full_slots promised an item");
                bb.empty_slots.signal();
                bb.sum.fetch_add(item, Relaxed);
                bb.consumed.fetch_add(1, Relaxed);
                spin(3000);
            }
        })
        .unwrap();
    }
    let r = scheduler::run();
    print_report(&r);
    if r.deadlock {
        // Abandoned tasks still own Arc clones, so the buffer cannot be inspected.
        check!(s, "PC.no_deadlock", false, "producer/consumer deadlocked");
        return;
    }

    let expected_sum: u64 = (1..=2u64).flat_map(|p| (0..ITEMS_PER_PRODUCER).map(move |i| p * 1000 + i)).sum();
    let bb = Arc::try_unwrap(bb).ok().expect("tasks still hold the buffer");
    let (empty, full) = (bb.empty_slots.stats(), bb.full_slots.stats());
    let max_len = bb.max_len.load(Relaxed);
    let leftover = bb.buf.into_inner().len();
    ktrace!(
        "PRODCONS",
        "consumed={} sum={} expected_sum={} max_len={} capacity={} producer_blocked={} consumer_blocked={}",
        bb.consumed.load(Relaxed), bb.sum.load(Relaxed), expected_sum, max_len, CAPACITY, empty.blocked, full.blocked
    );
    check!(s, "PC.all_items_consumed", bb.consumed.load(Relaxed) == 50 && leftover == 0, "consumed {} of 50, {} left in buffer", bb.consumed.load(Relaxed), leftover);
    check!(s, "PC.no_lost_or_duplicated_items", bb.sum.load(Relaxed) == expected_sum, "sum={} expected={}", bb.sum.load(Relaxed), expected_sum);
    check!(s, "PC.capacity_respected", max_len <= CAPACITY, "max occupancy {} <= {}", max_len, CAPACITY);
    check!(
        s, "PC.both_sides_blocked", empty.blocked > 0 && full.blocked > 0,
        "producers blocked {} times on a full buffer, consumers {} times on an empty one", empty.blocked, full.blocked
    );
}
