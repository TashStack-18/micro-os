//! Programs you can start from the shell: `run <name> [args...]`.
//!
//! ## Adding your own program
//!
//! 1. Copy `template.rs` to e.g. `myprog.rs` and edit `main`.
//! 2. Add `mod myprog;` below and one line to `PROGRAMS`.
//! 3. `scripts/shell.sh`, then type `run myprog` (or just `myprog`).
//!
//! Each program runs as its own kernel task, scheduled alongside the shell
//! and any other program, so `ps`, `sched` and `top` show it working.
//! Use only `crate::sys::*`, `println!`/`print!`, and `alloc` types.

mod counter;
mod fib;
mod hello;
mod memtest;
mod pingpong;
mod primes;
mod spin;
mod template;
mod user;

pub struct Program {
    pub name: &'static str,
    pub usage: &'static str,
    pub about: &'static str,
    /// Receives the words after the program name; returns an exit code.
    pub main: fn(&[&str]) -> i32,
}

pub const PROGRAMS: &[Program] = &[
    Program { name: "hello", usage: "hello [args...]", about: "print a greeting, pid and arguments", main: hello::main },
    Program { name: "counter", usage: "counter [n=10] [ms=500]", about: "count with sleeps (BLOCKED between numbers)", main: counter::main },
    Program { name: "primes", usage: "primes [limit=200000]", about: "CPU-bound prime counting", main: primes::main },
    Program { name: "fib", usage: "fib [n=40]", about: "Fibonacci numbers with u128", main: fib::main },
    Program { name: "spin", usage: "spin [seconds=5] [yield]", about: "CPU hog (or polite with 'yield'): shows its CPU share", main: spin::main },
    Program { name: "memtest", usage: "memtest [kib=256]", about: "allocate, fill, verify and free heap memory", main: memtest::main },
    Program { name: "pingpong", usage: "pingpong [rounds=20] [ms=80]", about: "two tasks hand two semaphores back and forth (real wake-ups)", main: pingpong::main },
    Program { name: "user", usage: "user [hello|priv|badptr|unknown|spin] [arg]", about: "run a program in U-mode; it reaches the kernel only via ecall", main: user::main },
    Program { name: "template", usage: "template", about: "starting point for your own program", main: template::main },
];

pub fn find(name: &str) -> Option<&'static Program> {
    PROGRAMS.iter().find(|p| p.name == name)
}

/// Parse argument `i` as a number, or use `default` if it is missing/invalid.
pub fn arg_u64(args: &[&str], i: usize, default: u64) -> u64 {
    args.get(i).and_then(|a| a.parse().ok()).unwrap_or(default)
}
