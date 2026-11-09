# Micro-OS — an educational RISC-V kernel in Rust (CA-6)

A small operating-system kernel that makes OS mechanisms *visible*: it boots
on an emulated RISC-V machine, schedules tasks with a timer interrupt,
synchronises them, allocates memory, and reports what every mechanism did
through structured traces that a Python script turns into tables and plots.
It also has an interactive shell for running your own small Rust programs.

| Core requirement | Where |
|---|---|
| Rust kernel boot | `kernel/src/boot/` (`entry.S` → `kmain`) |
| Console + diagnostics | `kernel/src/kernel/console.rs`, `panic.rs` |
| Task representation | `kernel/src/task/` (TCB, context, stacks) |
| Ready queue + Round Robin | `core/src/sched/rr.rs`, `kernel/src/scheduler/` |
| Timer-driven preemption | `kernel/src/timer/`, `kernel/src/trap/` |
| Synchronization | `kernel/src/sync/` (IRQ lock, spinlock, mutex, semaphore) |
| Memory allocation | `core/src/mem/` (bump, first-fit/best-fit free list) |
| Integrated demo + tests | `kernel/src/scenarios/` (TC1–TC6 + mechanism demos) |
| **Specialization: Track A + G** | Scheduler engineering: RR, Priority, Priority+aging, MLFQ, **FCFS, SJF, SRTF, HRRN**; research-to-kernel: the R3 study *Fairness vs Throughput in CPU Scheduling* re-run on the kernel ([docs/research_sync.md](docs/research_sync.md)) |
| Bonus: system interface | `kernel/src/sys.rs`, `programs/`, interactive `shell/` |
| Linux vs Micro-OS (§20) | `linux/compare.py`, [docs/linux_comparison.md](docs/linux_comparison.md) |

## Where everything runs

Everything lives inside **WSL2 Ubuntu** (a virtual machine), in `~/micro-os`.
The kernel itself only ever runs inside **QEMU**, an emulated RISC-V
computer, so it never touches real hardware.

Open a WSL terminal (Windows Terminal → Ubuntu, or `wsl -d Ubuntu`), then:

```bash
cd ~/micro-os
```

To edit in VS Code with the WSL extension: `code ~/micro-os`.

## Web control panel

```bash
scripts/frontend.sh               # then open http://localhost:8080 in your Windows browser
```

A browser front end for the running kernel, served from inside the VM:

* **Console**: the Micro-OS shell in a real terminal, plus one-click buttons for the programs.
* **Live views**, updated 4 times a second from the kernel's monitor task: CPU share per task
  (last 60 s), task table with states and priorities, context switches per second, kernel heap
  usage and fragmentation, scheduler counters, and an activity feed.
* **Scheduler switch**: pick Round Robin / Priority / Priority+aging / MLFQ and a quantum,
  then Reboot.
* **Tests & results**: run the whole test suite and browse every check and plot.

`frontend/server.py` (Python standard library only) runs QEMU and bridges it to the page:
serial output streams to the browser (Server-Sent Events), keystrokes go back, and `#T SNAP`
trace lines become the live dashboards. Stop it with Ctrl-C.

## Quick start (command line)

```bash
scripts/shell.sh                  # boot into the interactive shell
scripts/run.sh tc2                # run one test scenario, log to traces/tc2.log
scripts/test_all.sh               # unit tests + full suite + repeatability + plots
scripts/linux_compare.sh          # same workloads on Linux, compared (docs/linux_comparison.md)
scripts/research.sh               # R3 research experiment on the kernel (~15 min; docs/research_sync.md)
```

In the shell try: `help`, `progs`, `hello world`, `spin 10 &`, `spin 10 yield &`,
`top`, `ps`, `sched`, `mem`, `exit`. Leave QEMU at any time with **Ctrl-A, then X**.

Run the shell under a different scheduler: `scripts/shell.sh policy=mlfq` (or `prio`, `aging`, `rr`; add `quantum=N`).

## Writing your own program

See [docs/writing-programs.md](docs/writing-programs.md). In short: copy
`kernel/src/programs/template.rs`, register it in `programs/mod.rs`, and run
it from the shell.

## Test scenarios (`scripts/run.sh <name>`)

| name | what it shows |
|---|---|
| `tc1` | two tasks alternate under RR (minimum configuration) |
| `tc2` | five tasks with different bursts finish in burst order |
| `tc3` | boundaries: quantum 1, quantum ≥ any burst (= FCFS), one task, empty task |
| `tc4` | stress: 48 tasks with mutex, heap traffic, sleeps; heap-leak check |
| `tc5` | failure cases are rejected: task limit, huge alloc, bad/double free, foreign unlock, deadlock detection |
| `tc6` | repeatability: identical dispatch sequence across runs |
| `coop` | cooperative vs timer-driven scheduling with a CPU hog |
| `race` | lost updates without sync; fixed by spinlock, spin-yield, mutex |
| `prodcons` | bounded buffer with semaphores + mutex |
| `mem` | bump vs first-fit vs best-fit: success, speed, fragmentation |
| `sched_cmp` | specialization: RR vs PRIO vs PRIO+AGING vs MLFQ |
| `quantum_sweep` | RR quantum 1–32: switches vs response time (CA-4 link) |
| `r3_quick` | short R3 run: checks FCFS/SJF/SRTF/HRRN and the research measurements |
| `research` | the full R3 study on the kernel (not in `all`; use `scripts/research.sh`) |
| `fault` | deliberate access fault → diagnostic panic with backtrace |
| `all` | everything except `fault` (default) |

Options go after the name, e.g. `scripts/run.sh tc2 policy=mlfq quantum=2 log=debug`.

## Layout

```
core/       hardware-independent mechanisms, unit-tested on the host (cargo test)
kernel/     the RISC-V kernel (boot, traps, scheduler, sync, memory, shell)
scripts/    build/run/test/debug helpers (bash)
analysis/   Python trace parser and plots
frontend/   web control panel (server.py + static page)
linux/      Linux comparison experiment (compare.py)
traces/     serial logs from runs
results/    CSV tables, plots, summary.md
docs/       architecture, design, testing, debugging log, report
```

## Requirements (already installed in WSL)

Rust stable with target `riscv64gc-unknown-none-elf`, `qemu-system-riscv64`
(QEMU ≥ 7, with OpenSBI), Python 3 with matplotlib.
