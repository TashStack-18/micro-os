//! CPU-bound: counts primes by trial division. Never sleeps, so the timer
//! interrupt is what forces it to share the CPU.

use super::arg_u64;
use crate::sys;

fn is_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    let mut d = 2;
    while d * d <= n {
        if n % d == 0 {
            return false;
        }
        d += 1;
    }
    true
}

pub fn main(args: &[&str]) -> i32 {
    let limit = arg_u64(args, 0, 200_000);
    let start = sys::uptime_ms();
    let mut count = 0u64;
    let mut largest = 0;
    for n in 0..=limit {
        if n % 1000 == 0 && sys::interrupted() {
            println!("primes: interrupted at {} ({} primes so far)", n, count);
            return 130;
        }
        if is_prime(n) {
            count += 1;
            largest = n;
        }
    }
    println!(
        "primes: {} primes <= {} (largest {}) in {} ms wall time, {} CPU ticks",
        count, limit, largest, sys::uptime_ms() - start, sys::cpu_ticks()
    );
    0
}
