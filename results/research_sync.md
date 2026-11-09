# R3 on the Micro-OS kernel: measured results

Kernel runs: 3 seed(s) per configuration, 5000 ms arrival window, 200 µs modelled context-switch cost, Jain's index over 250 ms intervals. Mean ± 95% CI (1.96·s/√n) where n > 1. Simulator values are from the R3 report (30 seeds, 5000 ms window).

## Hypotheses

| hypothesis | simulator (R3 report) | Micro-OS kernel |
|---|---|---|
| H1: overload: SJF/SRTF highest throughput, Jain < 0.5; RR fair (≈1) | supported | supported |
| H2: underload: all policies complete about the same number of jobs | supported | supported |
| H3: RR: tiny q loses throughput to switching, large q loses fairness | supported | supported |

## Table 1: bimodal workload, ρ = 1.1

| policy | throughput (jobs/100 ms) | CPU util. (%) | context switch (%) | mean wait (ms) | wait variance (ms²) | max wait (ms) | Jain's index | simulator: throughput / Jain |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| FCFS | 6.54 ± 0.40 | 90.5 | 1.30 | 349.5 | 35,067 | 676 | 0.26 ± 0.31 | 6.29 / 0.12 |
| SJF | 6.95 ± 0.22 | 90.1 | 1.39 | 89.1 | 151,901 | 3,136 | 0.32 ± 0.18 | 6.94 / 0.28 |
| SRTF | 6.96 ± 0.22 | 90.0 | 2.23 | 72.6 | 159,468 | 3,244 | 0.45 ± 0.35 | 6.96 / 0.23 |
| HRRN | 6.87 ± 0.18 | 90.2 | 1.37 | 128.5 | 42,847 | 1,035 | 0.38 ± 0.30 | 6.85 / 0.26 |
| RR (q=2) | 6.50 ± 0.36 | 85.2 | 9.90 | 336.9 | 319,982 | 2,406 | 0.99 ± 0.01 | 6.29 / 0.99 |
| RR (q=8) | 6.66 ± 0.28 | 89.8 | 2.93 | 247.0 | 132,954 | 1,702 | 0.96 ± 0.03 | 6.48 / 0.93 |

## Table 2: uniform workload, ρ = 1.1

| policy | throughput (jobs/100 ms) | CPU util. (%) | context switch (%) | mean wait (ms) | wait variance (ms²) | max wait (ms) | Jain's index | simulator: throughput / Jain |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| FCFS | 5.77 ± 0.26 | 92.6 | 1.15 | 460.1 | 68,602 | 916 | 0.15 ± 0.14 | 6.05 / 0.12 |
| SJF | 6.17 ± 0.24 | 92.2 | 1.23 | 274.1 | 624,907 | 4,920 | 0.27 ± 0.27 | 6.42 / 0.12 |
| SRTF | 6.17 ± 0.24 | 92.2 | 1.55 | 272.8 | 632,421 | 4,936 | 0.25 ± 0.20 | 6.41 / 0.13 |
| HRRN | 5.93 ± 0.21 | 92.5 | 1.18 | 369.8 | 89,871 | 1,220 | 0.18 ± 0.10 | 6.23 / 0.15 |
| RR (q=2) | 4.70 ± 0.29 | 86.1 | 9.97 | 1,069.3 | 555,880 | 2,784 | 0.95 ± 0.01 | 4.95 / 0.96 |
| RR (q=8) | 5.41 ± 0.33 | 92.6 | 2.88 | 657.1 | 206,960 | 1,692 | 0.74 ± 0.10 | 5.63 / 0.78 |

Correlation between throughput and Jain's index across the six policies at ρ = 1.1: uniform r = -0.90 (simulator −0.94), bimodal r = -0.51 (simulator −0.51).

## Table 3: effect of load (bimodal workload)

| policy | thr ρ=0.7 | thr ρ=0.9 | thr ρ=1.1 | J ρ=0.7 | J ρ=0.9 | J ρ=1.1 | simulator J ρ=0.7/0.9/1.1 |
|---|---:|---:|---:|---:|---:|---:|---:|
| FCFS | 4.47 | 5.72 | 6.54 | 0.75 | 0.25 | 0.26 | 0.89 / 0.42 / 0.12 |
| SJF | 4.47 | 5.72 | 6.95 | 0.50 | 0.68 | 0.32 | 0.91 / 0.54 / 0.28 |
| SRTF | 4.47 | 5.72 | 6.96 | 0.44 | 0.82 | 0.45 | 0.87 / 0.51 / 0.23 |
| HRRN | 4.47 | 5.72 | 6.87 | 0.75 | 0.62 | 0.38 | 0.92 / 0.57 / 0.26 |
| RR (q=2) | 4.47 | 5.69 | 6.50 | 1.00 | 1.00 | 0.99 | 1.00 / 1.00 / 0.99 |
| RR (q=8) | 4.47 | 5.72 | 6.66 | 0.99 | 1.00 | 0.96 | 1.00 / 0.99 / 0.93 |

## Table 4: Round Robin quantum sweep (bimodal, ρ = 1.1)

| quantum q (ms) | 1 | 2 | 4 | 8 | 16 | 32 | 64 | 128 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| throughput | 6.11 | 6.49 | 6.63 | 6.66 | 6.60 | 6.53 | 6.53 | 6.54 |
| CPU util. (%) | 76.2 | 85.2 | 88.6 | 89.8 | 90.3 | 90.5 | 90.5 | 90.5 |
| context sw. (%) | 19.4 | 9.9 | 5.2 | 2.9 | 2.1 | 1.6 | 1.4 | 1.3 |
| Jain's index | 1.00 | 0.99 | 0.98 | 0.96 | 0.83 | 0.62 | 0.42 | 0.26 |
| simulator throughput | 6.03 | 6.29 | 6.42 | 6.48 | 6.41 | 6.31 | 6.27 | 6.29 |
| simulator Jain | 1.00 | 0.99 | 0.97 | 0.93 | 0.80 | 0.53 | 0.21 | 0.12 |

Slowdown-based fairness (bimodal, ρ = 1.1): SRTF 0.89, RR (q=2) 0.29 (simulator: 0.89 and 0.31).

## Figures

![r3_fig1_throughput_vs_fairness.png](r3_fig1_throughput_vs_fairness.png)
![r3_fig2_fairness_vs_load.png](r3_fig2_fairness_vs_load.png)
![r3_fig3_wait_variance.png](r3_fig3_wait_variance.png)
![r3_fig4_quantum_sweep.png](r3_fig4_quantum_sweep.png)
![r3_kernel_vs_simulator.png](r3_kernel_vs_simulator.png)
