# Micro-OS — an educational RISC-V kernel in Rust (CA-6)

A small operating-system kernel that makes OS mechanisms *visible*. It boots
on an emulated RISC-V machine, runs real tasks with their own stacks, switches
between them on a timer interrupt, synchronises them, allocates memory, lets
programs run in unprivileged **user mode** behind real system calls, and
reports what every mechanism did through structured traces that Python turns
into tables and plots.

On screen it is **NEXUS OS**: a desktop operating system in the browser, lit
by the running kernel. Its wallpaper, widgets, menu bar and windows show the
live machine, and every number on them comes from the kernel's trace.

![NEXUS OS desktop: the wallpaper lit by the running tasks, live CPU, scheduler, memory and system-call widgets, and a notification that the kernel killed a U-mode program](results/screenshots/03_desktop.png)

*Above: the wallpaper's coral and cyan lights are the two busiest tasks
(`spin` and `primes`), sized by their real CPU share. The widgets show CPU use
for the last 60 s, who got the CPU in each quarter-second, the 16 MiB heap in
128 slices, and the latest system calls. The notification is the kernel
killing `user priv` for a privileged instruction.*

## At a glance

| | |
|---|---|
| Kernel | Rust `no_std`, RISC-V 64 (S-mode), QEMU `virt` + OpenSBI, 1 hart, 128 MiB |
| Tasks | real context switch (`switch.S`), 16 KiB stacks with overflow canaries, up to 512 tasks |
| Scheduling | 1 ms timer interrupt; 8 policies: RR, Priority, Priority+aging, MLFQ, FCFS, SJF, SRTF, HRRN |
| User mode | U-mode programs, `ecall` system calls (`write`, `exit`, `yield`, `getpid`, `sleep_ms`, `uptime_ms`), faulting programs killed |
| Sync | IRQ-safe spinlock, spinlock, FIFO mutex, semaphore, deadlock detection |
| Memory | own 16 MiB first-fit free-list heap; bump and best-fit for comparison |
| Tests | 34 host unit tests + **68 in-kernel checks** (TC1–TC6, mechanisms, system calls), byte-identical across boots |
| Research | the R3 study *Fairness vs Throughput in CPU Scheduling* re-run on the kernel: 440 runs, H1–H3 supported |
| Linux | same workloads on Linux 6.6 (EEVDF, SCHED_RR) for comparison |
| Micro-OS Linux | a Debian 13 distribution built in 7 measured stages: LAN, Wi-Fi, laptop hardware, APT/aptitude, Snap Store |
| Desktop | NEXUS OS: living wallpaper, widgets, menus, Control Center, Mission Control, Spotlight, 10 apps, phone layout |

## The NEXUS OS desktop

NEXUS is built like a real desktop operating system, but nothing on it is
decoration: it is the kernel's own state, drawn.

- **Living wallpaper.** Each running task is a soft field of light. The three
  busiest tasks get coral, cyan and mint, and keep their colour while they
  stay in the top three; every other task is a dim haze. A dispatch makes a
  task's light flare, a killed program flashes rose, and the heap in use glows
  along the horizon. With the kernel off, the sky is dark.
- **Boot and lock screen.** The boot screen replays this boot's real trace with
  timestamps. The lock screen shows the machine's identity and how many
  milliseconds the kernel took to reach its shell.
- **Menu bar.** The NEXUS menu, the focused app's own menu (Shell, File, Go,
  Run or Tests), and Window, Kernel and Help menus, all keyboard-navigable.
  In the middle, a capsule with the policy, quantum, tick count and number
  of tasks; on the right, live CPU and heap meters.
- **Widgets.** CPU over the last 60 s, a scheduler ribbon, a heap map and the
  latest system calls, each opening its full app.
- **Control Center.** Pick any of the 8 scheduling policies and a quantum, then
  reboot the kernel with them; turn the living wallpaper or notifications
  off; power the machine off or on.
- **Mission Control, Spotlight and the dock.** See all windows at once (Ctrl+F3),
  search apps and run shell commands (Ctrl+K), launch from a magnifying dock.
  Windows move, resize, minimise and zoom, and the layout is remembered.
- **Notifications.** Raised by real events: a program killed, a deadlock,
  a kernel build, a finished test run, the machine powering off.
- **Phone.** The same desktop works at phone width.

What it does not pretend: the kernel has no file system, compiler or IPC
messages. *Files* browses the project on the host, *Build & run* compiles on
the host and reboots the kernel, and the Kernel Map's arcs are real semaphore
wake-ups. Each of these is labelled as such on screen.

## Tour

| | |
|---|---|
| ![Boot](results/screenshots/01_boot.png) | **Boot.** The boot screen replays this boot's real trace: hart entry, firmware, trap vector, heap, timer, scheduler, shell, each with its timestamp. |
| ![Lock screen](results/screenshots/02_lock.png) | **Lock screen.** The clock where the desktop's sits, the machine's identity and its measured boot time. |
| ![Terminal](results/screenshots/04_terminal.png) | **Terminal.** The kernel's own shell over the serial port: `user hello` reaches the kernel only through `ecall`, `user priv` is killed for reading a supervisor register, `user badptr` is refused with `-EFAULT`. |
| ![Kernel Map](results/screenshots/05_topo.png) | **Kernel Map.** Each task is a peak whose height is its CPU share; the path is the scheduler's real dispatch order and the arcs are real semaphore wake-ups. |
| ![System calls](results/screenshots/06_syscalls.png) | **System calls.** Live `ecall`s from U-mode programs, what each returned, and the kernel stopping a program that breaks the rules. |
| ![Processes](results/screenshots/07_processes.png) | **Processes.** Task table (state, priority, MLFQ level, CPU ticks, dispatches), CPU share over the last 60 s, and why tasks left the CPU. |
| ![Memory](results/screenshots/08_memory.png) | **Memory.** A map of the 16 MiB heap in 128 slices from the kernel's `HEAPMAP` trace, with fragmentation and allocation counters. |
| ![Journal](results/screenshots/09_journal.png) | **Journal.** Every spawn, exit, wake-up and policy event, as the kernel reported it. |
| ![Code](results/screenshots/10_code.png) | **Code.** Edit a program in `kernel/src/programs/`; *Build & run* rebuilds the kernel on the host (labelled as such) and runs it. |
| ![Files](results/screenshots/11_files.png) | **Files.** The project's files on the host, opened in Code with one click; the kernel itself has no file system. |
| ![Research](results/screenshots/12_research.png) | **Research.** The R3 fairness-vs-throughput results measured on this kernel, and the Micro-OS Linux boot times. |
| ![System](results/screenshots/13_system.png) | **System.** Machine facts from the kernel, the 68-check test suite, and reboot under any policy and quantum. |
| ![Control Center](results/screenshots/14_control_center.png) | **Control Center.** Scheduler, quantum and reboot, living wallpaper, do not disturb, Mission Control and power. |
| ![Mission Control](results/screenshots/15_mission_control.png) | **Mission Control.** Every open window at once; click or press Enter to pick one. |
| ![Phone](results/screenshots/16_phone.png) | **Phone.** The same live desktop at 375 px: widgets in one column, a two-row dock. |

Regenerate these pictures from the live system: `scripts/screenshots.py` (see its header).

## Core requirements → code

| Core requirement | Where |
|---|---|
| Rust kernel boot | `kernel/src/boot/` (`entry.S` → `kmain`) |
| Console + diagnostics | `kernel/src/kernel/console.rs`, `panic.rs` (backtraces) |
| Task representation | `kernel/src/task/` (TCB, context, stacks) |
| Ready queue + Round Robin | `core/src/sched/rr.rs`, `kernel/src/scheduler/` |
| Timer-driven preemption | `kernel/src/timer/`, `kernel/src/trap/` |
| Synchronization | `kernel/src/sync/` (IRQ lock, spinlock, mutex, semaphore) |
| Memory allocation | `core/src/mem/` (bump, first-fit/best-fit free list) |
| Integrated demo + tests | `kernel/src/scenarios/` (TC1–TC6 + mechanism demos) |
| **Specialization: Track A + G** | Scheduler engineering: RR, Priority, Priority+aging, MLFQ, **FCFS, SJF, SRTF, HRRN**; research-to-kernel: the R3 study re-run on the kernel ([docs/research_sync.md](docs/research_sync.md)) |
| System interface (Track D) | `kernel/src/user/` (U-mode, `ecall` system calls), `kernel/src/sys.rs`, `programs/`, `shell/` |
| Linux vs Micro-OS (§20) | `linux/compare.py`, [docs/linux_comparison.md](docs/linux_comparison.md) |

## User mode and system calls

`kernel/src/user/` runs programs in RISC-V U-mode on their own user stack.
They cannot execute privileged instructions; the only way into the kernel is
`ecall`:

```text
 U-mode program           trap.S (S-mode)              user::syscall
 a7 = number, a0.. args
 ecall  ────────────────▶ swap to the task's kernel
                          stack (sscratch), save  ───▶ check arguments,
                          the TrapFrame                do the work, a0 = result
 continues after ecall ◀─ restore, sret to U-mode ◀──
```

In the shell: `user hello`, `user priv`, `user badptr`, `user unknown`,
`user spin`. The `syscall` test checks the round trip, that a privileged
instruction kills only the offending program, `-EFAULT` for a bad pointer,
`-ENOSYS` for an unknown call, that the timer preempts user code, and that
the other programs survive. Paging is off, so this is privilege separation,
not memory isolation (stated in the code and docs).

## Micro-OS Linux (`distro/`)

A separate artifact for the extra requirements (network, laptop hardware,
package manager, app store, boot time per component): a Debian 13 system
built one component at a time, each stage booted under UEFI Secure Boot and
timed with `systemd-analyze` (5 boots each).

| Stage | Adds | Boot (s) |
|---|---|---:|
| S0 | minimal base: Linux 6.12, systemd | 4.08 ± 0.58 |
| S1 | LAN: NetworkManager, Realtek Ethernet | 4.18 ± 0.14 |
| S2 | Wi-Fi: wpa_supplicant, RTL8852BE driver + firmware | 4.44 ± 0.16 |
| S3 | laptop hardware: microcode, Intel graphics/audio, Bluetooth | 4.13 ± 0.19 |
| S4 | APT + aptitude | 4.20 ± 0.55 |
| S5 | snapd | 4.69 ± 0.62 |
| S6 | Xfce desktop + Snap Store | 6.57 ± 0.38 |

122 component checks pass in the VM ([results/distro_verify.md](results/distro_verify.md));
full table in [results/distro_boottime.md](results/distro_boottime.md), USB guide in
[distro/README.md](distro/README.md).

## Where everything runs

Everything lives inside **WSL2 Ubuntu** (a virtual machine), in `~/micro-os`.
The kernel itself only ever runs inside **QEMU**, an emulated RISC-V
computer, so it never touches real hardware.

```bash
cd ~/micro-os
scripts/frontend.sh               # NEXUS OS desktop at http://localhost:8080
```

`frontend/server.py` (Python standard library only) runs QEMU and bridges it to the page:
serial output streams to the browser (Server-Sent Events), keystrokes go back, and `#T` trace
lines become the live views. Stop it with Ctrl-C.

## Quick start (command line)

```bash
scripts/shell.sh                  # boot into the interactive shell
scripts/run.sh tc2                # run one test scenario, log to traces/tc2.log
scripts/test_all.sh               # unit tests + full suite + repeatability + plots
scripts/linux_compare.sh          # same workloads on Linux, compared (docs/linux_comparison.md)
scripts/research.sh               # R3 research experiment on the kernel (~15 min; docs/research_sync.md)
```

In the shell try: `help`, `progs`, `hello world`, `spin 10 &`, `spin 10 yield &`,
`pingpong`, `user hello`, `user priv`, `top`, `ps`, `sched`, `mem`, `exit`.
Leave QEMU at any time with **Ctrl-A, then X**.

Run the shell under a different scheduler: `scripts/shell.sh policy=mlfq` (or `prio`, `aging`, `rr`, `fcfs`, `sjf`, `srtf`, `hrrn`; add `quantum=N`).

## Writing your own program

See [docs/writing-programs.md](docs/writing-programs.md). In short: copy
`kernel/src/programs/template.rs`, register it in `programs/mod.rs`, and run
it from the shell (or edit and *Build & run* it in the desktop's Code app).

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
| `syscall` | U-mode programs: system-call round trip, privileged instruction killed, -EFAULT, -ENOSYS, user code preempted |
| `r3_quick` | short R3 run: checks FCFS/SJF/SRTF/HRRN and the research measurements |
| `research` | the full R3 study on the kernel (not in `all`; use `scripts/research.sh`) |
| `fault` | deliberate access fault → diagnostic panic with backtrace |
| `all` | everything except `fault` (default) |

Options go after the name, e.g. `scripts/run.sh tc2 policy=mlfq quantum=2 log=debug`.

## Layout

```
core/       hardware-independent mechanisms, unit-tested on the host (cargo test)
kernel/     the RISC-V kernel (boot, traps, user mode, scheduler, sync, memory, shell)
scripts/    build/run/test/debug/screenshot helpers
analysis/   Python trace parser and plots
frontend/   NEXUS OS desktop (server.py + static page)
linux/      Linux comparison experiment (compare.py)
distro/     Micro-OS Linux: Debian-based stages S0–S6
traces/     serial logs from runs
results/    CSV tables, plots, screenshots, summary.md
docs/       architecture, design, testing, debugging log, report
```

## Requirements (already installed in WSL)

Rust stable with target `riscv64gc-unknown-none-elf`, `qemu-system-riscv64`
(QEMU ≥ 7, with OpenSBI), Python 3 with matplotlib. Screenshots: Playwright
with Chromium. Distro: `mmdebstrap`, `qemu-system-x86`, `ovmf`, `mtools`, `snapd`.
