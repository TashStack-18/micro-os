# Architecture

## Layers and control flow

```
                         ┌──────────────────────────────────────────────┐
  serial keyboard ─────► │ shell task (prio 0)        programs/*  (tasks)│ ◄── sys:: API (Layer 9)
                         │  run / ps / top / kill      hello, spin, ...   │
                         └──────┬───────────────────────────┬────────────┘
                     spawn()    │                           │ sleep / yield / lock / exit
                                ▼                           ▼
 ┌────────────────── scheduler (Layers 4-6) ─────────────────────────────────────┐
 │  task table BTreeMap<Tid, Box<Task>>     sleepers: Vec<Tid>                    │
 │                                                                                │
 │  scheduler loop (boot stack) ──pick_next()──► __switch_context ──► task runs   │
 │        ▲          Policy (core/sched)                                │         │
 │        │  RR | PRIO | PRIO+AGING | MLFQ                              │         │
 │        └── retire_current() ◄── __switch_context ◄── preempt/yield/block/exit  │
 └────────────▲──────────────────────────────────────────────▲────────────────────┘
              │ on_timer_interrupt(): tick, wake sleepers,   │ wake(tid)
              │ policy.on_tick, charge CPU, preempt?         │
 ┌────────────┴──────────────┐                  ┌────────────┴──────────────────────┐
 │ trap (trap.S + trap_handler)│                │ sync (Layer 7)                     │
 │ save TrapFrame on task stack│                │ IrqSpinLock  SpinLock  Mutex  Sem  │
 └────────────▲──────────────┘                  └───────────────────────────────────┘
              │ supervisor timer interrupt
 ┌────────────┴──────────────┐   ┌───────────────────────┐   ┌──────────────────────┐
 │ timer: SBI set_timer       │   │ memory (Layer 8)       │   │ console (Layer 2)     │
 │ every tick_us (1 ms)       │   │ #[global_allocator]    │   │ log lines + #T traces │
 └────────────▲──────────────┘   │ first-fit free list     │   └──────────┬───────────┘
              │ ecall             └───────────────────────┘              │ NS16550A UART
 ┌────────────┴──────────────────────────────────────────────────────────▼──────────┐
 │ OpenSBI firmware (M-mode)          QEMU `virt`: RV64GC hart, CLINT, UART, test dev │
 └──────────────────────────────────────────────────────────────────────────────────┘
```

## Boot path (Layer 1)

1. QEMU starts OpenSBI in M-mode at `0x8000_0000`.
2. OpenSBI jumps to `_start` at `0x8020_0000` in S-mode with `a0 = hartid`, `a1 = DTB`.
3. `entry.S` sets `sp` to a 64 KiB boot stack, zeroes `.bss`, calls `kmain`.
4. `kmain`: console → SBI version → command line (from `/chosen/bootargs`
   in the DTB) → trap vector → heap → timer → enable interrupts → scenarios or shell.

## One timer tick

1. The CLINT timer fires; the hart traps to `__trap_entry` and pushes a
   `TrapFrame` (all registers plus `sstatus`/`sepc`) onto the current task's stack.
2. `trap_handler` → `scheduler::on_timer_interrupt`: re-arm the timer,
   count the tick, wake sleepers whose time has come, let the policy age or
   boost, and charge the tick to the running task.
3. If the slice is used up (or the policy wants a higher-priority task to
   run), the task is marked READY/Preempted and `switch_to_scheduler` saves
   its callee-saved registers.
4. The scheduler loop re-queues it and dispatches the next task.
5. Later, when this task is picked again, `__switch_context` returns into
   step 3. The trap frame is restored and `sret` resumes the interrupted code.

## Why this shape

* **Scheduler loop on its own context** (xv6 style). A task never switches
  directly to another task, so "who frees a dead task's stack" has a simple
  answer: the loop, after the task has left its stack.
* **Policies in a separate `core` crate** keep the decision logic
  hardware-free, unit-testable on the host, and switchable at boot.
* **Single hart + interrupt-masking locks.** The only concurrency inside
  the kernel is the timer interrupt, so masking interrupts is both necessary
  and sufficient for kernel data. Task-level locks (spin/mutex/semaphore)
  are separate and do let tasks be preempted while holding them.
* **Everything observable.** Every dispatch, deschedule, wake, policy
  decision, check and exit emits a `#T` trace line.

## Memory map

| range | contents |
|---|---|
| `0x0010_0000` | QEMU test device (power-off with status) |
| `0x1000_0000` | UART0 (NS16550A) |
| `0x8000_0000` | OpenSBI (PMP-protected from S-mode) |
| `0x8020_0000` | kernel `.text .rodata .data`, 64 KiB boot stack, `.bss` |
| `__heap_start` | 16 MiB kernel heap (task stacks, task table, collections) |
