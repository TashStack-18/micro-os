# Linux vs Micro-OS (CA-6 §20, §38)

**Question:** do the scheduling behaviours measured in the Micro-OS also
appear in a production kernel, and where do the two differ?

Run it yourself (inside the WSL VM, as root):

```bash
scripts/linux_compare.sh          # 5 repetitions; ~1 minute
```

Raw data: `results/linux_tasks.csv`, `results/linux_summary.csv`.
Generated tables: `results/linux_comparison.md`.
Plots: `results/linux_vs_microos_tc2.png`, `results/linux_vs_microos_cmp.png`,
`results/linux_repeatability.png`.

## Method

| | Micro-OS | Linux |
|---|---|---|
| Kernel | this project, RISC-V, QEMU | Linux 6.6 (WSL2), x86_64 |
| CPUs | 1 hart | 16 CPUs; every task pinned to **one** CPU with `sched_setaffinity` |
| Tasks | kernel tasks running `Work::Cpu` / `Work::Interactive` | one Python process per task, same CPU bursts (1 tick = 1 ms) |
| Release | all tasks READY at the same instant | all processes block on one event, released together |
| Time | QEMU instruction counting (deterministic) | real time, 5 repetitions, mean ± standard deviation |
| Run-queue wait | kernel metric `wait_us` | `/proc/self/schedstat` (time runnable but not running) |
| Context switches | dispatches | voluntary + involuntary switches (`/proc/self/status`) |

Linux scheduling configurations (`linux/compare.py`):

| Linux | What it is | Closest Micro-OS policy |
|---|---|---|
| `eevdf` | Default `SCHED_OTHER`. Since 6.6 this is EEVDF, the successor of CFS: weighted fair share with a ~3 ms base slice. | RR (4 ms quantum) |
| `eevdf_nice` | `SCHED_OTHER` with nice = 3 × (priority − 4) | PRIO + aging |
| `rr` | `SCHED_RR`, one real-time priority, 100 ms slice | RR |
| `rr_prio` | `SCHED_RR`, real-time priority 20 − priority (strict) | PRIO |

Workloads are the kernel's own: **tc2** (5 CPU-bound tasks: 12, 24, 8, 30,
16 ms) and **cmp** (3 CPU-bound tasks at priority 3, 2 interactive tasks at
priority 1 doing 10 × [1 ms CPU, 3 ms sleep], and a 20 ms batch task at
priority 7).

## Findings (run of 2026-11-09; see `results/linux_comparison.md` for the latest)

1. **Linux's fair scheduler reproduces Micro-OS Round Robin.** On tc2,
   EEVDF finished the tasks in exactly the Micro-OS RR order
   (T3 T1 T5 T2 T4), in every one of the 5 repetitions, with almost the same
   average response time (8.4 ± 0.4 ms vs 7.6 ms). Its ~3 ms slice is close
   to our 4 ms quantum, so short jobs finish first in both.
2. **The quantum is the parameter that matters.** Linux `SCHED_RR` uses a
   100 ms slice, longer than every burst here, so it ran the tasks in arrival
   order (T1 … T5). That is first-come-first-served, exactly what the
   Micro-OS does with `quantum=1000` (TC3). Average response time jumped to
   42.5 ms, while turnaround dropped (60.6 ms vs 83.3 ms for EEVDF) because
   there were only ~10 context switches instead of ~38. This is the same
   response-vs-switching trade-off as the Micro-OS quantum sweep.
3. **Strict priority starves on both systems.** Under `rr_prio`, Linux
   completed the cmp tasks in the same order as Micro-OS PRIO
   (INT1 INT2 CPU1 CPU2 CPU3 BATCH). The batch task waited 175 ms (Micro-OS:
   140 ms), and the interactive tasks waited about 0.1 ms on both.
4. **Linux avoids starvation by weighting, not by aging.** EEVDF with nice
   values gave the batch task its first run after 52 ms on average, with
   large run-to-run variation (± 47 ms; one earlier 5-run batch averaged
   40 ms). Micro-OS PRIO + aging gives 40.2 ms every time. Both are far
   below strict priority (140–175 ms), by different mechanisms: Linux gives
   a low-priority task a smaller *share* of the CPU instead of none, while
   aging raises its priority the longer it waits.
5. **EEVDF favours tasks that sleep.** Plain EEVDF gave the interactive tasks
   4.7 ms average run-queue latency vs 10.3 ms for Micro-OS RR and 7.2 ms for
   MLFQ, because a task that slept has unused "lag" and is eligible to run
   first. MLFQ achieves something similar with explicit queue levels.
6. **Repeatability differs by design.** The Micro-OS produces byte-identical
   traces every run. Linux does not: the cmp workload produced up to 4
   different completion orders in 5 runs under EEVDF and EEVDF+nice, and the
   turnaround standard deviation is 1–7 ms (`linux_repeatability.png`).

## Caveats (state these in the report)

* **WSL2 is itself a virtual machine.** Its virtual CPUs share the physical
  CPUs with Windows, so timings include hypervisor noise. Linux charged a
  typical task about 12% more CPU time than its nominal burst (median; 90%
  of tasks within +34%, the worst case about 1.8×; `cpu_us` in
  `linux_tasks.csv`).
* **Python overhead.** Each Linux task is a Python process; interpreter
  overhead adds a little CPU to every burst. Micro-OS tasks are native code.
* **Different architectures.** x86_64 at full host speed vs emulated RISC-V
  under QEMU. Compare *shapes* (orders, ratios, who starves) rather than
  absolute milliseconds.
* **Context-switch counts are not identical metrics.** Linux counts every
  switch of the measured processes, including blocking to sleep; Micro-OS
  counts dispatches.

## Concept comparison (§20)

| Concept | Linux (measured here) | Micro-OS |
|---|---|---|
| Task | process with its own address space, credentials and files | kernel task: stack + registers + scheduling info |
| Default scheduler | EEVDF: weighted fair share with virtual deadlines | Round Robin, fixed quantum |
| Priorities | nice (weights) for normal tasks; strict real-time priorities 1–99 | 0–7 strict, optional aging, MLFQ |
| Starvation control | weights: everyone keeps a share | aging or MLFQ boost |
| Time slice | ~3 ms (EEVDF base slice), 100 ms (`SCHED_RR`) | 4 ms by default, configurable |
| Determinism | no: interrupts, other CPUs, the hypervisor | yes (instruction-counted QEMU) |
| Observability | `/proc/<pid>/schedstat`, `status`, perf, ftrace | built-in `#T` trace of every decision |

## Viva Q16, "How is your Micro-OS different from Linux?" — talking points

* Same core idea: a run queue, time slices, preemption on a timer tick.
  The measured completion orders match when the slices match.
* Linux's default is a *fair-share* scheduler (EEVDF), not Round Robin, but
  with similar slices it behaves almost identically on CPU-bound jobs.
* Linux handles priority with weights, which prevents starvation by design;
  our strict PRIO starves, and we needed aging to fix it.
* We trade realism for determinism and visibility: every scheduling
  decision in the Micro-OS is traced and repeatable.
