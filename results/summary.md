# Micro-OS results

**Checks:** 62 passed, 0 failed

## Scheduling runs

| run | policy | q | tasks | dispatches | avg turnaround (ms) | avg response (ms) | avg wait (ms) | throughput (tasks/s) | Jain fairness |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| tc1 | RR | 4 | 2 | 12 | 40.3 | 1.9 | 20.4 | 48.0 | 1.0 |
| tc2 | RR | 4 | 5 | 27 | 70.3 | 7.6 | 53.1 | 55.2 | 0.944 |
| tc3_q1 | RR | 1 | 5 | 95 | 67.9 | 1.6 | 50.7 | 55.2 | 0.959 |
| tc3_qmax | RR | 1000 | 5 | 5 | 50.8 | 33.5 | 33.5 | 55.2 | 0.702 |
| tc3_single | RR | 4 | 1 | 3 | 9.7 | 0.0 | 0.0 | 94.2 | 1.0 |
| tc3_empty | RR | 4 | 1 | 1 | 0.0 | 0.0 | 0.0 | 1057.1 | nan |
| tc4 | RR | 4 | 48 | 384 | 197.1 | 5.7 | 191.4 | 216.5 | 0.959 |
| tc5_limit | RR | 4 | 512 | 512 | 241.1 | 241.0 | 241.0 | 1061.6 | 0.633 |
| tc5_mutex | RR | 4 | 3 | 4 | 1.3 | 0.1 | 0.1 | 341.2 | nan |
| tc5_deadlock | RR | 4 | 0 | 4 | 0.0 | 0.0 | 0.0 | 0.0 |  |
| tc6_r1 | RR | 4 | 5 | 27 | 70.3 | 7.6 | 53.1 | 55.2 | 0.944 |
| tc6_r2 | RR | 4 | 5 | 27 | 70.4 | 7.7 | 53.2 | 55.1 | 0.943 |
| tc6_r3 | RR | 4 | 5 | 27 | 70.4 | 7.7 | 53.2 | 55.1 | 0.943 |
| coop_off | RR | 4 | 4 | 19 | 51.2 | 30.5 | 37.8 | 70.1 | 0.402 |
| coop_on | RR | 4 | 4 | 29 | 43.8 | 3.6 | 30.3 | 69.6 | 0.557 |
| race_unsync | RR | 2 | 4 | 28 | 50.6 | 2.7 | 38.1 | 74.7 | 0.999 |
| race_spin | RR | 2 | 4 | 64 | 92.8 | 2.6 | 62.2 | 31.8 | 0.978 |
| race_spin_yield | RR | 2 | 4 | 64 | 33.9 | 1.1 | 21.0 | 72.5 | 0.76 |
| race_mutex | RR | 2 | 4 | 3907 | 253.6 | 1.1 | 21.0 | 15.3 | 0.999 |
| prodcons | RR | 2 | 4 | 50 | 9.3 | 0.3 | 4.7 | 326.7 | 0.781 |
| cmp_rr | RR | 4 | 6 | 60 | 140.9 | 8.7 | 104.8 | 37.0 | 0.799 |
| cmp_prio | PRIO | 4 | 6 | 66 | 103.0 | 26.3 | 67.1 | 37.3 | 0.931 |
| cmp_aging | PRIO+AGING | 4 | 6 | 66 | 105.6 | 9.6 | 69.8 | 37.4 | 0.937 |
| cmp_mlfq | MLFQ | 4 | 6 | 52 | 124.6 | 8.2 | 88.8 | 37.3 | 0.829 |
| sweep_q1 | RR | 1 | 6 | 112 | 76.5 | 2.5 | 58.5 | 60.6 | 0.907 |
| sweep_q2 | RR | 2 | 6 | 59 | 76.6 | 4.6 | 58.3 | 60.4 | 0.908 |
| sweep_q4 | RR | 4 | 6 | 36 | 78.8 | 9.5 | 60.6 | 59.7 | 0.885 |
| sweep_q8 | RR | 8 | 6 | 24 | 78.2 | 19.6 | 60.0 | 55.2 | 0.864 |
| sweep_q16 | RR | 16 | 6 | 17 | 69.7 | 32.5 | 51.3 | 52.5 | 0.57 |
| sweep_q32 | RR | 32 | 6 | 14 | 61.1 | 42.9 | 42.9 | 52.4 | 0.616 |
| r3_uniform_110_FCFS_s1 | FCFS | 1 | 35 | 35 | 72.8 | 57.8 | 57.8 | 58.2 | 0.56 |
| r3_uniform_110_SJF_s1 | SJF | 1 | 35 | 35 | 53.9 | 38.8 | 38.8 | 58.2 | 0.755 |
| r3_uniform_110_SRTF_s1 | SRTF | 1 | 35 | 43 | 52.5 | 35.6 | 37.5 | 58.0 | 0.798 |
| r3_uniform_110_HRRN_s1 | HRRN | 1 | 35 | 35 | 62.0 | 46.9 | 46.9 | 58.2 | 0.612 |
| r3_uniform_110_RR2_s1 | RR | 2 | 35 | 311 | 131.3 | 11.2 | 116.2 | 53.5 | 0.415 |
| r3_uniform_110_RR8_s1 | RR | 8 | 35 | 84 | 96.7 | 30.7 | 81.6 | 57.3 | 0.478 |
| r3_bimodal_110_FCFS_s1 | FCFS | 1 | 36 | 36 | 60.9 | 48.7 | 48.7 | 62.4 | 0.525 |
| r3_bimodal_110_SJF_s1 | SJF | 1 | 36 | 36 | 38.2 | 25.9 | 25.9 | 62.4 | 0.672 |
| r3_bimodal_110_SRTF_s1 | SRTF | 1 | 36 | 48 | 25.2 | 11.2 | 13.0 | 62.2 | 0.924 |
| r3_bimodal_110_HRRN_s1 | HRRN | 1 | 36 | 36 | 40.7 | 28.4 | 28.4 | 62.4 | 0.616 |
| r3_bimodal_110_RR2_s1 | RR | 2 | 36 | 261 | 46.6 | 3.4 | 34.4 | 58.9 | 0.678 |
| r3_bimodal_110_RR8_s1 | RR | 8 | 36 | 75 | 43.8 | 12.0 | 31.6 | 61.7 | 0.653 |
| r3_sweep_q1_s1 | RR | 1 | 36 | 550 | 57.5 | 2.1 | 45.3 | 54.8 | 0.68 |
| r3_sweep_q8_s1 | RR | 8 | 36 | 75 | 43.8 | 12.0 | 31.6 | 61.7 | 0.653 |
| r3_sweep_q128_s1 | RR | 128 | 36 | 36 | 60.9 | 48.7 | 48.7 | 62.4 | 0.525 |

## Synchronization (4 tasks x 1000 increments)

| variant | result | lost | time (ms) | contended | spins | mutex wait (ms) |
|---|---:|---:|---:|---:|---:|---:|
| unsync | 1432 | 2568 | 53.6 | 0 | 0 | 0.0 |
| spin | 4000 | 0 | 125.8 | 3 | 1758217 | 0.0 |
| spin_yield | 4000 | 0 | 55.2 | 3 | 36 | 0.0 |
| mutex | 4000 | 0 | 260.6 | 3902 | 0 | 691.7 |

## Allocators (4000 random operations, 256 KiB region)

| allocator | profile | ok | failed | ns/op | peak (B) | avg frag | max frag |
|---|---|---:|---:|---:|---:|---:|---:|
| bump | mixed | 1608 | 828 | 308 | 262144 | 0.00% | 0.00% |
| first-fit | mixed | 2097 | 3 | 3814 | 218976 | 52.77% | 86.23% |
| best-fit | mixed | 2099 | 2 | 5240 | 227424 | 52.63% | 85.45% |
| bump | uniform64 | 2099 | 0 | 322 | 134080 | 0.00% | 0.00% |
| first-fit | uniform64 | 2099 | 0 | 886 | 16000 | 0.06% | 0.23% |
| best-fit | uniform64 | 2099 | 0 | 1157 | 16000 | 0.06% | 0.23% |

## Plots

![gantt_tc1.png](gantt_tc1.png)
![gantt_tc2.png](gantt_tc2.png)
![gantt_coop_off.png](gantt_coop_off.png)
![gantt_coop_on.png](gantt_coop_on.png)
![gantt_prodcons.png](gantt_prodcons.png)
![gantt_cmp_rr.png](gantt_cmp_rr.png)
![gantt_cmp_prio.png](gantt_cmp_prio.png)
![gantt_cmp_aging.png](gantt_cmp_aging.png)
![gantt_cmp_mlfq.png](gantt_cmp_mlfq.png)
![quantum_sweep.png](quantum_sweep.png)
![policy_compare.png](policy_compare.png)
![locks.png](locks.png)
![allocators.png](allocators.png)
