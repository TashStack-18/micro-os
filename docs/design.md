# Design decisions

| Decision | Chosen | Alternatives | Reason |
|---|---|---|---|
| Target | RISC-V 64 (`riscv64gc-unknown-none-elf`) on QEMU `virt` | x86_64 + bootloader crate | Works on **stable** Rust with no bootloader dependency; small, regular ISA; the timer comes through one SBI call |
| Privilege | Kernel and tasks in S-mode | U-mode tasks with syscalls | Keeps CA-6 focused on scheduling/sync/memory; U-mode is listed as future work (Track D/F) |
| Task bodies | `Box<dyn FnOnce() + Send>` closures | `fn(usize)` pointers | Safe sharing via `Arc`, no global mutable state per scenario |
| Context switch | Callee-saved registers only (`switch.S`) | Save all registers | Switches happen at function-call boundaries; the trap path saves the rest |
| Preemption | Trap frame on the task's stack; switch from inside the handler | Separate interrupt stack | Simple, and correct because traps are not nested (SIE cleared on entry) |
| Scheduler structure | Dedicated scheduler loop (xv6) | Direct task-to-task switch | Stacks of exited tasks are freed safely by the loop |
| Policy interface | `trait Policy` in `core`, `Box<dyn Policy>` | `enum` dispatch | Open for new policies; host-testable |
| Kernel locking | Disable interrupts + flag (`IrqSpinLock`) | Plain spinlock | One hart: interrupts are the only concurrency; flag detects re-entrancy and panics with the lock name |
| Task-level locking | Blocking FIFO mutex, counting semaphore, spinlock with pluggable relax | Only spinlocks | Shows the uniprocessor cost of spinning (see results) |
| Heap | First-fit free list with headers, splitting, coalescing | Bump, buddy, slab | Real reuse plus detectable invalid frees; bump/best-fit kept for comparison |
| Time | `time` CSR at 10 MHz, 1 ms tick | — | Tick = scheduling quantum unit |
| Reproducibility | QEMU `-icount shift=3,align=off,sleep=off` | Real time | Virtual time follows instruction count, so every run is identical |
| Exit status | QEMU test device carries the failed-check count; `run.sh` also reads the kernel's `SHUTDOWN failed=N` / `PANIC` line | QEMU exit code only | The trace line distinguishes panic (101), timeout (124) and silent stop (125) |

## Scheduling policies (`core/src/sched`)

* **RR**: FIFO queue, fixed quantum.
* **PRIO**: lowest number wins and is preemptive (a higher-priority READY
  task preempts at the next tick); FIFO within a level. Can starve.
* **PRIO+AGING**: every 10 ticks spent waiting raises effective priority by
  one level. The aged level holds for the slice it is dispatched with.
* **MLFQ** (OSTEP rules): 3 levels, quanta q/2q/4q, new tasks at the top, an
  allotment summed across yields so tasks cannot game it, a boost every
  50 ticks via an epoch counter.

## `unsafe` inventory

Every `unsafe` block carries a `// SAFETY:` comment with its invariant.

| Area | Why `unsafe` is needed | Invariant relied on |
|---|---|---|
| `arch.rs` (10) | inline `asm!` for CSRs, `wfi` | each instruction only touches the named CSR; on/off helpers are compiler barriers |
| `boot/mod.rs` (1) | read DTB from a raw address | OpenSBI passes a valid, never-written DTB; header read before sizing the slice |
| `drivers/*` (5) | MMIO and `ecall` | fixed QEMU `virt` addresses; SBI preserves all registers except a0/a1 |
| `task/stack.rs` (6) | raw allocation, paint, canary, `Send` | the stack is exclusively owned; read for high-water only while the task is not running |
| `scheduler` (3) | `__switch_context` | the boxed `Task` is not freed or moved while current; interrupts off; lock not held across the switch |
| `sync/*` (9) | `UnsafeCell` access, `Sync` impls | data reachable only through a guard that exists only while owned |
| `memory/mod.rs` (5) | `GlobalAlloc`, heap init | heap range is RAM above the kernel image, used by nothing else |
| `core/mem/*` (25) | raw free-list nodes and headers | nodes are written only by `init`/`insert_free` inside the owned region; frees are validated (range, alignment, magic) |
| `core/spin.rs` (4) | `UnsafeCell`, `Sync` | Acquire/Release on the flag orders successive holders |
| tests (`scenarios`) (10) | deliberately invalid frees/unlocks/loads | each test only passes pointers the allocator validates, or faults on purpose |

Unsafe code is isolated behind safe APIs: tasks and programs never need
`unsafe` (the `programs/` directory contains none).

## Known limitations

* No memory protection: programs run in S-mode with the kernel.
* Floating-point registers are not saved on context switch; kernel code and
  programs must not use `f32`/`f64` (everything is integer arithmetic).
* Single hart. `IrqSpinLock` would need real spinning for SMP.
* Mutex is not priority-inheriting: priority inversion is possible.
* Abandoning deadlocked tasks leaks what lived on their stacks.
* Cancellation (Ctrl-C/`kill`) is cooperative.
