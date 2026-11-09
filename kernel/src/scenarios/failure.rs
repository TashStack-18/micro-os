//! TC5 — controlled invalid operations must be rejected, not corrupt state.
//! Also the deliberate-fault demo (`test=fault`).

use alloc::alloc::{alloc, dealloc};
use alloc::sync::Arc;
use core::alloc::Layout;
use core::sync::atomic::{AtomicU8, Ordering::Relaxed};

use microos_core::mem::FreeError;
use microos_core::sched::PolicyKind;

use super::{print_report, Options, Suite};
use crate::memory;
use crate::scheduler::{self, Config, SpawnError, SpawnOpts, MAX_TASKS};
use crate::sync::{Mutex, SyncError};

fn config(label: &str) -> Config {
    Config { label: label.into(), policy: PolicyKind::RoundRobin, quantum: 4, preemptive: true }
}

fn encode(r: Result<(), SyncError>) -> u8 {
    match r {
        Ok(()) => 1,
        Err(SyncError::NotLocked) => 2,
        Err(SyncError::NotOwner) => 3,
    }
}

pub fn tc5_failures(s: &mut Suite, _o: &Options) {
    // (a) spawning with no scheduler run active
    let r = scheduler::spawn("orphan", SpawnOpts::default(), || {});
    check!(s, "TC5.spawn_without_run", r == Err(SpawnError::NotInitialised), "{:?}", r);

    // (b) task-table limit
    scheduler::begin(config("tc5_limit"));
    for _ in 0..MAX_TASKS {
        scheduler::spawn("tiny", SpawnOpts::default(), || {}).expect("spawn below the limit failed");
    }
    let over = scheduler::spawn("extra", SpawnOpts::default(), || {});
    let rep = scheduler::run();
    check!(
        s, "TC5.task_limit", over == Err(SpawnError::TooManyTasks) && rep.tasks.len() == MAX_TASKS,
        "task {} rejected with {:?}; the {} admitted tasks all finished", MAX_TASKS + 1, over, rep.tasks.len()
    );

    // (c) allocation larger than the whole heap
    let failed_before = memory::stats().failed;
    let huge = Layout::from_size_align(64 << 20, 16).unwrap();
    // SAFETY: non-zero size; a null result is the expected outcome.
    let p = unsafe { alloc(huge) };
    check!(
        s, "TC5.oversized_alloc", p.is_null() && memory::stats().failed == failed_before + 1,
        "64 MiB request returned {:p}; failed counter {} -> {}", p, failed_before, memory::stats().failed
    );

    // (d) freeing memory the heap never handed out
    let mut local = 0u64;
    let l8 = Layout::new::<u64>();
    // SAFETY: the allocator validates the pointer and refuses it.
    let r = unsafe { memory::checked_free(&mut local as *mut u64 as *mut u8, l8) };
    check!(s, "TC5.foreign_free", r == Err(FreeError::OutOfRange), "free(&stack_var) -> {:?}", r);

    // (e) double free and (f) misaligned free
    let l64 = Layout::from_size_align(64, 16).unwrap();
    // SAFETY: valid layout; the pointers are only passed back to the heap.
    unsafe {
        let p = alloc(l64);
        let first = memory::checked_free(p, l64);
        let second = memory::checked_free(p, l64);
        check!(s, "TC5.double_free", first.is_ok() && second == Err(FreeError::NotAllocated), "first={:?} second={:?}", first, second);
        let q = alloc(l64);
        let mis = memory::checked_free(q.add(8), l64);
        check!(s, "TC5.misaligned_free", mis == Err(FreeError::Misaligned), "free(p+8) -> {:?}", mis);
        dealloc(q, l64);
    }

    // (g) unlocking a mutex you do not own / that is not locked
    let m = Arc::new(Mutex::new("tc5", ()));
    let res = Arc::new([AtomicU8::new(0), AtomicU8::new(0)]);
    scheduler::begin(config("tc5_mutex"));
    let m1 = m.clone();
    scheduler::spawn("owner", SpawnOpts::default(), move || {
        let _g = m1.lock();
        scheduler::sleep_ticks(4);
    })
    .unwrap();
    for (i, delay) in [(0usize, 1u64), (1, 8)] {
        let (m2, res2) = (m.clone(), res.clone());
        scheduler::spawn("intruder", SpawnOpts { delay_ticks: delay, ..SpawnOpts::default() }, move || {
            // SAFETY: this task never holds a guard for `m2`.
            res2[i].store(encode(unsafe { m2.force_unlock() }), Relaxed);
        })
        .unwrap();
    }
    print_report(&scheduler::run());
    let (while_held, after) = (res[0].load(Relaxed), res[1].load(Relaxed));
    check!(s, "TC5.unlock_not_owner", while_held == 3, "unlock by non-owner -> code {} (3 = NotOwner)", while_held);
    check!(s, "TC5.unlock_not_locked", after == 2, "unlock of free mutex -> code {} (2 = NotLocked)", after);

    // (h) a lock-order deadlock is detected instead of hanging the kernel
    let (a, b) = (Arc::new(Mutex::new("A", ())), Arc::new(Mutex::new("B", ())));
    scheduler::begin(config("tc5_deadlock"));
    for (name, first, second) in [("D1", a.clone(), b.clone()), ("D2", b.clone(), a.clone())] {
        scheduler::spawn(name, SpawnOpts::default(), move || {
            let _x = first.lock();
            scheduler::sleep_ticks(2);
            let _y = second.lock();
        })
        .unwrap();
    }
    let rep = scheduler::run();
    check!(
        s, "TC5.deadlock_detected", rep.deadlock && rep.tasks.is_empty(),
        "deadlock={} (both tasks blocked forever; run abandoned cleanly)", rep.deadlock
    );

    // (i) invalid configuration input
    check!(s, "TC5.bad_policy_name", PolicyKind::parse("fifo").is_none(), "policy=fifo rejected");
}

/// Read firmware memory, which OpenSBI's PMP forbids for S-mode. The trap
/// handler turns the resulting load access fault into a diagnostic panic.
pub fn deliberate_fault() {
    scheduler::begin(config("fault"));
    scheduler::spawn("faulty", SpawnOpts::default(), || {
        kinfo!("reading 0x8000_0000 (OpenSBI, PMP-protected): expect a load access fault");
        // SAFETY: intentionally invalid; the access traps before any value is used.
        let v = unsafe { core::ptr::read_volatile(0x8000_0000 as *const u64) };
        kerror!("read unexpectedly succeeded: {:#x}", v);
    })
    .unwrap();
    scheduler::run();
}
