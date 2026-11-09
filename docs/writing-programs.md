# Writing your own programs

Programs are small Rust functions the shell starts as kernel tasks. Each one
gets its own stack and task id, and the scheduler preempts it like any other
task, so you can watch it with `ps` and `top`.

## 1. Create the file

Copy the template:

```bash
cd ~/micro-os/kernel/src/programs
cp template.rs countdown.rs
```

Edit `countdown.rs`:

```rust
use super::arg_u64;
use crate::sys;

pub fn main(args: &[&str]) -> i32 {
    let from = arg_u64(args, 0, 5);            // first argument, default 5
    for i in (1..=from).rev() {
        println!("{}...", i);
        if !sys::sleep_ms(1000) {               // false if Ctrl-C was pressed
            println!("cancelled");
            return 130;
        }
    }
    println!("liftoff! (pid {})", sys::getpid());
    0                                          // exit code
}
```

## 2. Register it

In `kernel/src/programs/mod.rs`, add the module and a table entry:

```rust
mod countdown;
// ...
Program { name: "countdown", usage: "countdown [n=5]", about: "counts down", main: countdown::main },
```

## 3. Run it

```bash
cd ~/micro-os
scripts/shell.sh
```

```
microos> countdown 3
3...
2...
1...
liftoff! (pid 2)
[countdown pid 2 exited with code 0]
microos> countdown 10 &
[started countdown as pid 3 in the background]
microos> ps
```

`scripts/shell.sh` rebuilds the kernel first, so compile errors appear there.

## What a program may use

| API | meaning |
|---|---|
| `println!` / `print!` | write to the console |
| `sys::getpid()` | your task id |
| `sys::sleep_ms(ms) -> bool` | sleep (BLOCKED); `false` if interrupted |
| `sys::yield_now()` | let another ready task run |
| `sys::interrupted() -> bool` | check for Ctrl-C/`kill` in long loops |
| `sys::uptime_ms()`, `sys::cpu_ticks()`, `sys::tick_us()` | time and CPU used |
| `sys::heap_used()` | kernel heap bytes in use |
| `super::arg_u64(args, i, default)` | numeric argument |
| `alloc::vec::Vec`, `alloc::string::String`, `alloc::collections::*` | heap collections |

There is no `std`: no files, threads or `std::time`. Programs run in
supervisor mode, so a bug in one (an infinite loop that never checks
`interrupted()`, or a panic) affects the whole kernel. A panic stops the
kernel with a backtrace. That is the main limitation a user mode with
system calls would remove (Track D/F future work).

## Experiments to try

* `spin 10 &` twice, then `top`: the two hogs split the CPU about evenly.
* `spin 10 &` and `spin 10 yield &`: the polite one gets far less CPU.
* `run -p 1 spin 5 &` and `run -p 6 spin 5 &` under `scripts/shell.sh policy=prio`:
  the low-priority one starves until the other finishes; then try `policy=aging`.
* `counter 20 100 &` while `spin` runs: the sleeper stays responsive.
