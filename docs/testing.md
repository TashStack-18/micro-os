# Test plan and results

Run everything: `scripts/test_all.sh` (results go to `traces/` and `results/`).

## Level 1 — unit tests on the host (`cd core && cargo test`): 25 tests

| Module | Tests |
|---|---|
| `sched::rr` | FIFO rotation, quantum clamped to ≥ 1 |
| `sched::priority` | highest first + FIFO ties, preempt only for higher, aging lifts 7→2 after 50 ticks, no aging when disabled |
| `sched::mlfq` | CPU-bound task sinks level by level, early yields cannot game the allotment, higher queue preempts, boost resets |
| `mem::bump` | alignment, reset only when empty, exhaustion, rejected frees |
| `mem::freelist` | full coalescing, large alignments, double/foreign/misaligned free rejected, exhaustion and recovery, best-fit picks the tightest hole, 20 000-step randomized test against a shadow model (no overlap, no payload corruption) |
| `spin` | mutual exclusion over 4 real threads × 20 000, `try_lock`, relax callback |
| `fdt`, `args`, `rng` | command-line parsing, DTB walking, malformed blobs, determinism |

## Level 2/3 — mechanism and integration tests in the kernel (`scripts/run.sh all`): 59 checks

| Required case | Scenario | Checks |
|---|---|---|
| TC1 minimum (2 tasks) | `tc1` | completion, strict A/B alternation, slices per task, CPU received |
| TC2 multiple (5 tasks) | `tc2` | completion, full bursts, RR completion order T3,T1,T5,T2,T4 |
| TC3 boundary | `tc3` | q=1 → one dispatch per tick; q=1000 → FCFS (5 dispatches, 0 preemptions); single task; zero-work task |
| TC4 stress | `tc4` | 48 tasks: mutex counter exact (480), heap data checksum, no heap leak |
| TC5 failure | `tc5` | spawn without run, task limit (65th rejected), 64 MiB alloc fails, foreign/double/misaligned free rejected, unlock by non-owner / of free mutex rejected, lock-order deadlock detected, bad policy name |
| TC6 repeatability | `tc6` + `test_all.sh` | 3 runs in one boot have identical dispatch hashes; two separate boots produce **byte-identical** logs |
| Timer vs cooperative | `coop` | without preemption the hog runs in 1 dispatch; preemption improves response |
| Synchronization | `race`, `prodcons` | race loses updates; spin, spin-yield and mutex are exact; bounded buffer conserves items and respects capacity |
| Memory | `mem` | every allocator reclaims everything; free list beats bump on mixed sizes |
| Specialization | `sched_cmp`, `quantum_sweep` | PRIO starves BATCH, aging fixes it, MLFQ latency ≤ RR; switch count falls as q grows |
| Shell | `scripts/shell_smoke.sh` | scripted session: programs, background tasks, ps/sched/mem, clean exit |

## Latest results

From `results/summary.md` (deterministic mode):

* Checks: **59 passed, 0 failed**. Unit tests: **25 passed**.
* Two boots: logs byte-identical.
* Race: unsynchronized lost **2568 of 4000** increments; every lock gave 4000.
* Lock cost: spin-yield 55 ms, pure spin 126 ms (1.76 M wasted spins),
  mutex 261 ms (3907 context switches from the FIFO hand-off convoy).
* Allocators (mixed sizes): bump 1608 successes / 828 failures; first-fit
  2097 / 3; best-fit 2099 / 2, but best-fit is ~37% slower per operation.
* Policies (same workload): BATCH response time is 26.3 ms under PRIO and
  9.6 ms with aging. MLFQ interactive latency is at or below RR.
