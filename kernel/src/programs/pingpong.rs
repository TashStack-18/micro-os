//! pingpong: two tasks take turns through two semaphores.
//!
//! `ping` (this program's task) signals `to_pong` and blocks on `to_ping`;
//! a second task, `pong`, does the reverse. Every signal wakes the other task,
//! so the kernel traces a `WAKE tid=X by=Y` for each hand-off: a real
//! task-to-task wake-up (the kernel has no message-passing IPC; this is how
//! its tasks hand work to each other).
//!
//! Usage: pingpong [rounds=20] [ms=80]   (ms: pause between rounds)

use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};

use super::arg_u64;
use crate::scheduler::{self, SpawnOpts};
use crate::sync::Semaphore;
use crate::sys;

struct Table {
    to_ping: Semaphore,
    to_pong: Semaphore,
    stop: AtomicBool,
}

pub fn main(args: &[&str]) -> i32 {
    let rounds = arg_u64(args, 0, 20).clamp(1, 10_000);
    let pause = arg_u64(args, 1, 80);
    let t = Arc::new(Table {
        to_ping: Semaphore::new("to_ping", 0),
        to_pong: Semaphore::new("to_pong", 0),
        stop: AtomicBool::new(false),
    });

    let peer = Arc::clone(&t);
    let spawned = scheduler::spawn("pong", SpawnOpts { prio: 4, ..SpawnOpts::default() }, move || {
        for i in 1..=rounds {
            peer.to_pong.wait();
            if peer.stop.load(Ordering::Acquire) {
                return;
            }
            println!("  pong (pid {}) <- {}", sys::getpid(), i);
            peer.to_ping.signal();
        }
    });
    let pong = match spawned {
        Ok(tid) => tid,
        Err(e) => {
            println!("pingpong: could not start pong: {:?}", e);
            return 1;
        }
    };
    println!("pingpong: ping is pid {}, pong is pid {}, {} rounds", sys::getpid(), pong, rounds);

    for i in 1..=rounds {
        println!("ping (pid {}) -> {}", sys::getpid(), i);
        t.to_pong.signal(); // wakes pong
        t.to_ping.wait(); // blocks until pong wakes us
        if !sys::sleep_ms(pause) {
            // Ctrl-C: release pong so it can finish too.
            t.stop.store(true, Ordering::Release);
            t.to_pong.signal();
            return 130;
        }
    }
    0
}
