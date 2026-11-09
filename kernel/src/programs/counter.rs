//! Counts slowly. Between numbers the task is BLOCKED (sleeping), so it uses
//! almost no CPU: compare its CPU ticks in `ps` with `spin`'s.

use super::arg_u64;
use crate::sys;

pub fn main(args: &[&str]) -> i32 {
    let n = arg_u64(args, 0, 10);
    let ms = arg_u64(args, 1, 500);
    let pid = sys::getpid();
    for i in 1..=n {
        println!("[counter pid {}] {} / {}", pid, i, n);
        if !sys::sleep_ms(ms) {
            println!("[counter pid {}] interrupted", pid);
            return 130;
        }
    }
    println!("[counter pid {}] done, used {} CPU ticks", pid, sys::cpu_ticks());
    0
}
