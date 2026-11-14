# Linux vs Micro-OS: measured results

Linux 6.6.87.2-microsoft-standard-WSL2 (WSL2), all tasks pinned to CPU 15; 5 repetitions per Linux configuration (mean ± standard deviation). SCHED_RR time slice 100 ms; EEVDF base slice 3 ms. Micro-OS: one deterministic run (QEMU instruction counting), 4 ms quantum.

## Workload `tc2`

| system | policy | avg turnaround (ms) | avg response (ms) | avg wait (ms) | context switches | interactive latency (ms) | BATCH response (ms) | Jain | completion order | orders seen |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|---:|
| linux | eevdf | 83.3 ± 2.6 | 8.4 ± 0.4 | 63.0 ± 3.2 | 38.2 | – | – | 1.0 | T3 T1 T5 T2 T4 | 1 |
| linux | eevdf_nice | 86.3 ± 7.1 | 7.9 ± 1.0 | 65.7 ± 7.2 | 39.4 | – | – | 1.0 | T3 T1 T5 T2 T4 | 1 |
| linux | rr | 60.6 ± 4.2 | 42.5 ± 4.2 | 40.0 ± 4.4 | 10.2 | – | – | 0.7 | T1 T2 T3 T4 T5 | 3 |
| linux | rr_prio | 60.2 ± 1.4 | 42.1 ± 1.4 | 40.0 ± 2.3 | 10 | – | – | 0.7 | T1 T2 T3 T4 T5 | 2 |
| micro-os | tc2 | 70.3 | 7.6 | 53.1 | 27 | – | – | 0.9 | T3 T1 T5 T2 T4 | 1 |

## Workload `cmp`

| system | policy | avg turnaround (ms) | avg response (ms) | avg wait (ms) | context switches | interactive latency (ms) | BATCH response (ms) | Jain | completion order | orders seen |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|---:|
| linux | eevdf | 146.1 ± 3.6 | 8.8 ± 1.7 | 116.8 ± 3.4 | 81.2 | 4.7 | 10.0 | 0.9 | INT1 INT2 BATCH CPU1 CPU2 CPU3 | 4 |
| linux | eevdf_nice | 135.0 ± 5.0 | 21.7 ± 15.7 | 105.0 ± 5.4 | 80.8 | 1.2 | 52.3 | 0.9 | INT2 INT1 CPU1 CPU2 CPU3 BATCH | 4 |
| linux | rr | 139.6 ± 5.8 | 89.0 ± 6.1 | 110.3 ± 6.5 | 32 | 5.7 | 144.0 | 0.5 | CPU1 CPU2 CPU3 BATCH INT1 INT2 | 3 |
| linux | rr_prio | 104.8 ± 3.6 | 61.6 ± 3.3 | 75.9 ± 3.9 | 48 | 0.1 | 175.4 | 0.9 | INT1 INT2 CPU1 CPU2 CPU3 BATCH | 2 |
| micro-os | cmp_rr | 140.0 | 8.0 | 104.1 | 60 | 10.3 | 13.2 | 0.8 | BATCH CPU1 CPU2 INT1 INT2 CPU3 | 1 |
| micro-os | cmp_prio | 103.0 | 26.4 | 67.2 | 66 | 0.1 | 140.3 | 0.9 | INT1 INT2 CPU1 CPU2 CPU3 BATCH | 1 |
| micro-os | cmp_aging | 105.6 | 9.6 | 69.8 | 66 | 0.0 | 40.2 | 0.9 | INT1 INT2 CPU1 CPU2 CPU3 BATCH | 1 |
| micro-os | cmp_mlfq | 124.7 | 8.2 | 88.8 | 52 | 7.2 | 13.4 | 0.8 | BATCH CPU1 INT1 INT2 CPU2 CPU3 | 1 |

Context switches: Linux counts voluntary + involuntary switches of the measured processes; Micro-OS counts dispatches.

