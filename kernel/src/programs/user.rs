//! `user PROG [arg]`: start a program in U-mode (unprivileged) and wait for it.
//!
//! The U-mode programs live in `crate::user`; they talk to the kernel only
//! through `ecall` system calls. This launcher is an ordinary kernel task
//! that starts one as a separate task and reports how it ended.

use crate::{scheduler, sys, user};

pub fn main(args: &[&str]) -> i32 {
    let name = args.first().copied().unwrap_or("hello");
    let Some(prog) = user::program(name) else {
        println!("user: no user program '{}'", name);
        println!("  hello [n]   well-behaved: write, getpid, uptime_ms, sleep_ms, yield, exit");
        println!("  priv        runs a privileged instruction: the kernel kills it");
        println!("  badptr      passes a bad pointer to write: refused with -EFAULT");
        println!("  unknown     calls a system call that does not exist: -ENOSYS");
        println!("  spin [m]    m million loop steps in U-mode (preempted by the timer)");
        return 2;
    };
    let default_arg = match name {
        "hello" => 3,
        "spin" => 20,
        _ => 0,
    };
    let arg = super::arg_u64(args, 1, default_arg) as usize;
    let calls_before = user::SYSCALLS.load(core::sync::atomic::Ordering::Relaxed);
    let tid = match user::spawn("user", prog, arg) {
        Ok(t) => t,
        Err(e) => {
            println!("user: could not start: {:?}", e);
            return 1;
        }
    };
    println!("[started user program '{}' as pid {} in U-mode]", name, tid);
    while scheduler::is_alive(tid) {
        if !sys::sleep_ms(20) {
            scheduler::request_cancel(tid);
        }
    }
    let calls = user::SYSCALLS.load(core::sync::atomic::Ordering::Relaxed) - calls_before;
    match user::exit_code(tid) {
        Some(code) => {
            println!("[user pid {} exited with code {} after {} system calls]", tid, code, calls);
            code as i32
        }
        None => {
            println!("[user pid {} ended without an exit code]", tid);
            1
        }
    }
}
