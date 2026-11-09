# R3 research ↔ Micro-OS kernel (Track G: Research-to-Micro-OS)

**Research (CA-4):** *R3. Fairness vs Throughput in CPU Scheduling*, Tashvi
Adhlakha (25BTRCL271): a discrete-event simulator in Python comparing FCFS,
SJF, SRTF, HRRN and Round Robin on throughput and CPU-allocation fairness.

**CA-6 question (§11, Track G):** can the mechanisms studied in the
simulator be implemented in a real kernel, and do the findings survive
contact with real scheduling: real dispatching, a real timer, real tasks
running on a CPU?

Reproduce:

```bash
scripts/research.sh                 # 3 seeds, 5000 ms window: 132 kernel runs, ~15 min
scripts/research.sh r3_seeds=10     # tighter confidence intervals (~50 min)
```

Measured tables: [`results/research_sync.md`](../results/research_sync.md) ·
raw data: `results/r3_runs.csv`, `r3_exp1.csv`, `r3_exp2.csv` ·
figures: `results/r3_fig1..4_*.png`, `results/r3_kernel_vs_simulator.png`.

## 1. What from the paper is now in the kernel

| R3 paper | Micro-OS | Where |
|---|---|---|
| Policies FCFS, SJF (non-preemptive), SRTF, HRRN | Real kernel schedulers behind the `Policy` trait | `core/src/sched/classic.rs` (+8 unit tests) |
| RR with q = 2 ms and q = 8 ms; quantum sweep | RR with any quantum (1 tick = 1 ms) | `core/src/sched/rr.rs` |
| Uniform U(2,30) ms and bimodal 75 % U(1,6) + 25 % U(20,80) bursts | Generated in the kernel, seeded so every policy sees identical jobs | `core/src/research.rs` |
| Poisson arrivals, λ = ρ / mean burst, ρ ∈ {0.7, 0.9, 1.1} | Same rate, generated per 1 ms tick (Bernoulli: the discrete form of Poisson) | `core/src/research.rs` |
| Fixed context-switch cost 0.2 ms, charged when the next process differs | The dispatcher spends 200 µs doing no useful work on every switch to a different task; ticks inside it are counted as switch time | `scheduler::switch_delay` |
| Jain's index over 250 ms intervals, over processes present for the whole interval | Computed **inside the kernel** in the timer interrupt from exact CPU time | `scheduler::account_window_and_fairness` |
| Throughput = jobs completed in the arrival window per 100 ms; utilisation; switch %; waiting = finish − arrival − burst (mean, variance, max); slowdown-based Jain | All measured per run | `kernel/src/scenarios/research.rs` |
| Experiment 1 (6 policies × 2 workloads × 3 loads) and Experiment 2 (quantum sweep, bimodal ρ = 1.1) | `test=research` runs both; `r3_quick` (in the normal suite) checks the mechanisms | `kernel/src/scenarios/research.rs` |
| H1, H2, H3 | Scored automatically on the kernel's numbers (`VERDICT` trace events) | same |
| Limitation: "burst times are known in advance, whereas a real OS has to estimate them (exponential averaging)" | **Implemented:** burst prediction by exponential averaging (α = ½); the live shell runs SJF/SRTF/HRRN this way | `SchedInfo::end_burst_predicted`, `BurstInfo::Predicted` |
| Future work: "include MLFQ" | MLFQ is a kernel policy | `core/src/sched/mlfq.rs` |
| Future work: "repeat … on real Linux" | Linux comparison experiment | `docs/linux_comparison.md` |

## 2. Method differences (state these in the report)

| | R3 simulator | Micro-OS kernel |
|---|---|---|
| Time | simulated, continuous | real (QEMU instruction-counted), 1 ms timer ticks |
| Jobs | numbers in a list | real kernel tasks that spin until they have received their burst of CPU, measured exactly from the time CSR |
| Arrivals | Poisson, continuous | per-tick Bernoulli (< 0.3 % chance of a missed double arrival) |
| Bursts | real-valued | whole milliseconds |
| Smallest quantum | 0.5 ms | 1 ms (the tick) |
| Overheads | only the 0.2 ms switch cost | the 0.2 ms modelled cost **plus** real ones: timer interrupts, the dispatcher, the measurement code |
| Repetitions | 30 seeds | 3 seeds by default (time budget); `r3_seeds=` raises it |
| Arrival window | 5000 ms | 5000 ms |

## 3. Results (3 seeds, 5000 ms window)

All three hypotheses are supported on the kernel, as in the simulator.

| | Simulator | Kernel |
|---|---|---|
| **H1** SJF/SRTF highest throughput, Jain < 0.5; RR ≈ 1 (ρ = 1.1) | supported | **supported** |
| — bimodal throughput SJF / SRTF / RR q=2 | 6.94 / 6.96 / 6.29 | 6.95 / 6.96 / 6.50 |
| — bimodal Jain SJF / SRTF / RR q=2 | 0.28 / 0.23 / 0.99 | 0.32 / 0.45 / 0.99 |
| — uniform Jain SJF / SRTF / RR q=2 | 0.12 / 0.13 / 0.96 | 0.27 / 0.25 / 0.95 |
| — correlation throughput vs Jain, uniform / bimodal | −0.94 / −0.51 | −0.90 / −0.51 |
| **H2** at ρ = 0.7 all policies complete the same work | supported (4.49–4.53) | **supported** (bimodal: 4.47 for every policy; uniform: 4.17–4.18) |
| **H3** RR: small q loses throughput, large q loses fairness | supported, peak at q = 8 | **supported, peak at q = 8** (6.66) |
| — throughput at q = 1 / 8 / 128 | 6.03 / 6.48 / 6.29 | 6.11 / 6.66 / 6.54 |
| — context-switch share at q = 1 / 2 / 8 | 16.5 / 9.2 / 3.0 % | 19.4 / 9.9 / 2.9 % |
| — Jain at q = 1 / 8 / 128 | 1.00 / 0.93 / 0.12 | 1.00 / 0.96 / 0.26 |
| Slowdown-based Jain, SRTF vs RR q=2 (bimodal) | 0.89 vs 0.31 | 0.89 vs 0.29 |
| HRRN limits the worst case among shortest-first policies | yes | yes (bimodal: max wait 1,035 ms vs 3,244 ms for SRTF; variance 42,847 vs 159,468 ms²) |
| HRRN has the lowest waiting variance of **all** policies | yes | **no**: FCFS was lower on the kernel (35,067 ms²) |

So the paper's central finding, **maximizing throughput does not produce fair
CPU allocation**, holds on a real scheduler, and so does the "two
definitions of fairness" result.

## 4. Where the kernel differs, and why

1. **Fairness at low load is not measurable from these runs.** At ρ = 0.7
   only 0–3 of the ~20 intervals per run had any task present for the whole
   250 ms (SJF: one interval in all three runs). The kernel averages only
   over intervals where Jain's index is defined. The simulator's ~0.9 values
   at ρ = 0.7 likely come from counting near-empty intervals as fair (the
   R3 report: "two processes rarely compete for a full 250 ms interval").
   Table 3's ρ = 0.7 and 0.9 fairness columns should therefore not be
   compared directly. This is the R3 report's own lesson that the
   conclusion depends on how fairness is measured.
2. **Some intervals are undefined under SJF/SRTF.** When every task present
   for the whole interval was starved (all CPU went to jobs that arrived
   during it), all shares are 0 and Jain's index is 0/0. The kernel skips
   such intervals. They occur mostly under SJF/SRTF, so the kernel's Jain
   for those policies (0.25–0.45) is, if anything, an *over*-estimate of
   their fairness; they are still far below RR.
3. **Wide confidence intervals.** With 3 seeds, Jain's CIs are large (for
   example SRTF bimodal 0.45 ± 0.35), so H1's "Jain < 0.5" is met on the
   mean but not with confidence for every policy. Run `r3_seeds=10` or more
   before quoting fairness values as precise.
4. **Utilisation is lower (≈ 90–93 % vs ≈ 95–97 % under overload)** and throughput
   at large quanta is slightly higher (6.54 vs 6.29 at q = 128, inside the
   ±0.4 CI). The kernel has real overheads beyond the modelled 0.2 ms
   (timer interrupts every millisecond, the dispatcher, the in-kernel
   measurement) that the simulator does not.
5. **The 0.5 ms quantum cannot be run**: the kernel's scheduling resolution
   is its 1 ms timer tick.

## 5. Engineering lessons from the port (debugging log material)

Getting the kernel to reproduce the simulator exposed three measurement
bugs, each a real systems lesson:

* **Tick-sampled CPU accounting hides switch costs.** At first the modelled
  0.2 ms switch cost did not reduce throughput at all: CPU time was charged
  per timer tick, so the next task was credited a full tick even though
  part of it was spent switching, and q = 1 came out *fastest*. Fix: charge
  CPU time exactly from the `time` CSR, and run the switch delay before the
  task starts with interrupts enabled. After the fix q = 1 delivered ≈ 8 %
  less throughput than the q = 8 peak, and H3 appeared as in the paper.
* **Tick-sampled utilisation misses short jobs.** A 1 ms job that starts
  just after a tick can finish before the next one, so bimodal utilisation
  read 51 % instead of ~69 %. Fix: accumulate exact busy time.
* **Runs that finish early never close their window.** Low-load runs
  finished before the 5000 ms window ended and reported zero throughput.
  Fix: a run that ends early has completed everything inside the window.
* **A default of "1.0" is not a measurement.** With too few qualifying
  intervals the kernel first reported perfect fairness for FCFS. Now it
  reports "n/a".

## 6. How this answers the CA-6 questions

* *§38 Simulation → Linux → Micro-OS:* the same scheduling questions are now
  answered three ways: the R3 simulator, real Linux (`docs/linux_comparison.md`)
  and this kernel.
* *Viva Q18 "What CA-4 research question could be extended into your
  Micro-OS?":* this one, already done. The kernel runs the R3 experiment
  and reproduces H1–H3.
* *Viva Q19 "What would you implement next?":* lottery/stride scheduling
  (R3 future work), I/O-bound jobs in the R3 workload, and more seeds.
