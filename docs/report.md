# CA-6 Technical Report — Micro-OS in Rust

> Draft structure following CA-6 §23. Sections marked **TODO** need your
> team's own words, especially your reasoning, CA-4/CA-5 links and
> lessons. The examiner will ask you to defend them.

## 1. Title
Micro-OS: a timer-driven, multi-policy educational kernel for RISC-V in Rust.

## 2. Abstract
**TODO** (≈150 words): what was built, the specialization (Track A), headline
results (59/59 checks; aging cuts the low-priority task's response from
140.3 ms to 40.2 ms; spin-yield 2.3x faster than pure spinning on one CPU), and the main
limitation (no user mode).

## 3. Problem statement
**TODO**: which OS mechanisms you set out to make observable, and why an
emulated RISC-V machine.

## 4. Objectives
Boot a Rust kernel; represent and schedule tasks with a timer; synchronise
them; allocate memory; compare scheduling policies quantitatively; make
every run reproducible.

## 5. OS concepts
Trap handling, context switching, preemption vs cooperation, ready queues,
RR / priority / aging / MLFQ, mutual exclusion, blocking vs spinning,
semaphores, deadlock, heap allocation and fragmentation.

## 6. Architecture
See `docs/architecture.md` (diagram with data/control flow).

## 7. Design decisions
See `docs/design.md`. **TODO**: explain the two you find most important in your own words.

## 8. Rust implementation
`core` crate (host-tested) + `kernel` crate; 5.4k lines including tests;
`unsafe` isolated with SAFETY comments (inventory in design.md); programs
and tasks need no `unsafe`.

## 9. Scheduling
RR + timer preemption. Evidence: `results/gantt_tc1.png`, `gantt_tc2.png`,
TC1–TC3 checks.

## 10. Synchronization
`IrqSpinLock` (kernel), `SpinLock`, `Mutex` (FIFO hand-off), `Semaphore`.
Evidence: `results/locks.png`, the race table, prodcons.

## 11. Memory management
16 MiB first-fit free-list heap with headers, splitting and coalescing;
bump and best-fit for comparison. Evidence: `results/allocators.png`.

## 12. Specialization — Track A: scheduler engineering
PRIO, PRIO+AGING and MLFQ on one mixed workload (3 CPU-bound, 2 interactive,
1 low-priority batch task); quantum sweep. Evidence: `policy_compare.png`,
`gantt_cmp_*.png`, `quantum_sweep.png`.

## 13. Testing
See `docs/testing.md`: 25 unit tests, 59 in-kernel checks, TC1–TC6, shell smoke test.

## 14. Results
Copy the tables from `results/summary.md` (regenerate with `scripts/test_all.sh`).

## 15. Performance analysis
**TODO**: discuss at least:
* Quantum sweep: dispatches fall from 112 (q=1) to 14 (q=32), while average
  response rises from 2.5 ms to 42.9 ms. That is the classic RR trade-off
  from your CA-4 work.
* Spinning on a uniprocessor: pure spin wasted 1.76 M iterations because a
  waiter cannot make progress while the preempted holder is off the CPU.
* Mutex convoy: FIFO hand-off made almost every increment a context switch
  (about 3900 contended hand-offs, 259 ms). Why barging would be faster here, and what it costs in fairness.
* Allocator trade-off: best-fit is marginally more successful but ~37% slower.

## 16. Limitations
See design.md "Known limitations".

## 17. Lessons learned
**TODO**

## 18. Future work
User mode with system calls (Track D), page tables (Track B), priority
inheritance for the mutex, SMP, UART interrupts through the PLIC instead of polling.

## 19. References
* R. Arpaci-Dusseau, A. Arpaci-Dusseau, *Operating Systems: Three Easy Pieces* (MLFQ, locks).
* MIT xv6-riscv source and book (scheduler-loop structure).
* RISC-V Privileged Specification; RISC-V SBI Specification.
* The Rust Reference — `unsafe`, inline assembly; The Rustonomicon.

## 20. AI usage disclosure
See `docs/ai_usage.md`.
