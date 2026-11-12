//! System calls from U-mode: the user/kernel boundary (Track D / F).
//!
//! Five programs run in U-mode alongside each other under Round Robin:
//! a well-behaved one, one that executes a privileged instruction, one that
//! passes a bad pointer, one that calls an unknown system call, and a
//! CPU-bound one. The checks confirm that each request crossed the boundary
//! the way it should and that a misbehaving program cannot stop the others.

use core::sync::atomic::Ordering::Relaxed;

use microos_core::sched::PolicyKind;

use super::{print_report, Options, Suite};
use crate::scheduler::{self, Config};
use crate::user;

pub fn syscalls(s: &mut Suite, _o: &Options) {
    let calls0 = user::SYSCALLS.load(Relaxed);
    let kills0 = user::KILLS.load(Relaxed);
    let bytes0 = user::BYTES_WRITTEN.load(Relaxed);

    scheduler::begin(Config {
        label: "syscall".into(),
        policy: PolicyKind::RoundRobin,
        quantum: 4,
        preemptive: true,
        ..Config::default()
    });
    let hello = user::spawn("u_hello", user::prog_hello, 2).unwrap();
    let bad = user::spawn("u_priv", user::prog_privileged, 0).unwrap();
    let ptr = user::spawn("u_badptr", user::prog_bad_pointer, 0).unwrap();
    let unk = user::spawn("u_unknown", user::prog_unknown, 0).unwrap();
    // Long enough to keep the CPU busy while u_hello sleeps: a fully idle
    // machine lets QEMU's icount clock jitter by a microsecond across boots.
    let spin = user::spawn("u_spin", user::prog_spin, 6).unwrap();
    let rep = scheduler::run();
    print_report(&rep);

    let calls = user::SYSCALLS.load(Relaxed) - calls0;
    let kills = user::KILLS.load(Relaxed) - kills0;
    let bytes = user::BYTES_WRITTEN.load(Relaxed) - bytes0;
    let task = |tid| rep.tasks.iter().find(|t| t.tid == tid);

    check!(
        s, "SYS.roundtrip", user::exit_code(hello) == Some(0) && calls >= 10 && bytes > 0,
        "u_hello made its calls and exited 0 ({} ecalls in total, {} bytes written via write)", calls, bytes
    );
    check!(
        s, "SYS.privileged_killed", user::exit_code(bad) == Some(user::KILLED) && kills == 1,
        "csrr sstatus in U-mode raised an illegal-instruction trap; the kernel killed only that task (exit {:?})",
        user::exit_code(bad)
    );
    check!(
        s, "SYS.bad_pointer", user::exit_code(ptr) == Some(user::EFAULT),
        "write(0x10, 5) returned {:?} (expected -EFAULT = {})", user::exit_code(ptr), user::EFAULT
    );
    check!(
        s, "SYS.unknown_call", user::exit_code(unk) == Some(user::ENOSYS),
        "syscall 4242 returned {:?} (expected -ENOSYS = {})", user::exit_code(unk), user::ENOSYS
    );
    let spin_pre = task(spin).map_or(0, |t| t.preemptions);
    check!(
        s, "SYS.user_preempted", user::exit_code(spin) == Some(0) && spin_pre > 0,
        "the CPU-bound U-mode task was preempted {} times by the timer and still finished", spin_pre
    );
    check!(
        s, "SYS.others_survive", rep.tasks.len() == 5 && !rep.deadlock,
        "all 5 user tasks finished although one was killed ({} reported, deadlock={})", rep.tasks.len(), rep.deadlock
    );
}
