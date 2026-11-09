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
scripts/research.sh r3_seeds=10     # the results below: 440 runs, ~50 min
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

## 3. Results (10 seeds, 5000 ms window: 440 kernel runs)

All three hypotheses are supported on the kernel, as in the simulator.

| | Simulator | Kernel |
|---|---|---|
| **H1** SJF/SRTF highest throughput, Jain < 0.5; RR ≈ 1 (ρ = 1.1) | supported | **supported** |
| — bimodal throughput SJF / SRTF / RR q=2 | 6.94 / 6.96 / 6.29 | 7.02 ± 0.21 / 7.04 ± 0.21 / 6.48 ± 0.28 |
| — bimodal Jain SJF / SRTF / RR q=2 | 0.28 / 0.23 / 0.99 | 0.39 ± 0.13 / 0.46 ± 0.16 / 0.99 |
| — uniform throughput SJF / SRTF / RR q=2 | 6.42 / 6.41 / 4.95 | 6.19 / 6.19 / 4.74 |
| — uniform Jain SJF / SRTF / RR q=2 | 0.12 / 0.13 / 0.96 | 0.19 ± 0.09 / 0.18 ± 0.07 / 0.96 |
| — correlation throughput vs Jain, uniform / bimodal | −0.94 / −0.51 | −0.91 / −0.31 |
| **H2** at ρ = 0.7 all policies complete the same work | supported (4.49–4.53) | **supported** (bimodal 4.49–4.52; uniform 4.19–4.23) |
| **H3** RR: small q loses throughput, large q loses fairness | supported, peak at q = 8 | **supported, peak at q = 8** |
| — throughput at q = 1 / 8 / 128 | 6.03 / 6.48 / 6.29 | 6.10 / 6.65 / 6.44 |
| — context-switch share at q = 1 / 2 / 8 | 16.5 / 9.2 / 3.0 % | 19.6 / 9.9 / 3.0 % |
| — Jain at q = 1 / 8 / 128 | 1.00 / 0.93 / 0.12 | 1.00 / 0.96 / 0.19 |
| Slowdown-based Jain, SRTF vs RR q=2 (bimodal) | 0.89 vs 0.31 | 0.89 vs 0.41 |
| HRRN limits the worst case among shortest-first policies | yes | yes (bimodal: max wait 1,039 ms vs 3,337 ms for SRTF; variance 45,014 vs 158,358 ms²) |
| HRRN has the lowest waiting variance of **all** policies | yes | **no**: FCFS was lower on the kernel (37,960 ms²) |

So the paper's central finding, **maximizing throughput does not produce fair
CPU allocation**, holds on a real scheduler, and so does the "two
definitions of fairness" result.

## 4. Where the kernel differs, and why

1. **Fairness at low load rests on very few intervals.** Jain's index is
   only defined for 250 ms intervals in which some task was present the
   whole time. At ρ = 0.7 that is rare: on the bimodal workload SJF averaged
   0.6 such intervals per run (6 of 10 runs had none), FCFS 0.4; on the
   uniform workload FCFS and HRRN had none in any run. The kernel averages
   only over defined intervals. With 10 seeds the ρ = 0.7 values come out
   close to the simulator for SJF, SRTF and RR (0.88 / 0.91 / 1.00 vs
   0.91 / 0.87 / 1.00) but not for FCFS (0.57 vs 0.89), and all of them are
   based on a handful of intervals. The simulator's high low-load values
   likely also reflect how near-empty intervals were treated (the R3 report:
   "two processes rarely compete for a full 250 ms interval"). This is the
   R3 report's own lesson that the conclusion depends on how fairness is
   measured. Compare the ρ = 1.1 column (4–23 intervals per run; fewest for SJF, whose starved jobs often receive nothing in an interval) with
   confidence; treat ρ = 0.7 and 0.9 as indicative only.
2. **Some intervals are undefined under SJF/SRTF.** When every task present
   for the whole interval was starved (all CPU went to jobs that arrived
   during it), all shares are 0 and Jain's index is 0/0. The kernel skips
   such intervals. They occur mostly under SJF/SRTF, so the kernel's Jain
   for those policies is, if anything, an *over*-estimate of their fairness.
3. **Fairness under overload is higher than the simulator's for the
   shortest-first policies on the bimodal workload** (SRTF 0.46 ± 0.16 vs
   0.23), so H1's "Jain < 0.5" holds on the mean but its confidence interval
   reaches above 0.5 for SRTF. On the uniform workload the gap is clear
   (0.18–0.19 vs 0.96 for RR). Item 2 is one likely cause. As a result the
   bimodal throughput–fairness correlation is weaker than the simulator's
   (r = −0.31 vs −0.51). With 3 seeds it happened to come out at −0.51, a
   reminder that a single matching number is not evidence on its own.
4. **Absolute throughput differs by a few percent.** On the uniform workload
   every policy completes about 3–4 % fewer jobs than in the simulator, and
   utilisation is lower (≈ 92 % vs ≈ 97 %). The kernel has real overheads
   beyond the modelled 0.2 ms (a timer interrupt every millisecond, the
   dispatcher, the in-kernel measurement) that the simulator does not. On
   the bimodal workload the kernel's values are slightly *higher* but
   within their 95 % confidence intervals of the simulator's. The ranking of
   the policies is the same in both.
5. **The 0.5 ms quantum cannot be run**: the kernel's scheduling resolution
   is its 1 ms timer tick.

## 5. Engineering lessons from the port (debugging log material)

Getting the kernel to reproduce the simulator exposed four measurement
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
