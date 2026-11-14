# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

- **Tashvi (the student who built it)**, driving NEXUS OS live during the CA-7 viva and demonstration, on the HP Victus laptop (1536×864 CSS pixels at 125% scaling) or a projector, while explaining the kernel to an examiner.
- **The same student during development**: booting the kernel, writing and running Rust programs, watching scheduling, memory and test results.
- **The examiner**, as the audience of the demo: needs to see, at a glance and from across a room, what the OS is doing and that it is real.

## Product Purpose

NEXUS OS is the on-screen operating system of the student's educational kernel (the CA-6 "Micro-OS" project). It is a browser desktop that boots the real Rust kernel in QEMU and makes its mechanisms visible: a live terminal to the kernel shell, a kernel topography that draws the scheduler, wake-ups and heap as they happen, process and memory monitors, an event journal, a file explorer and a Rust code editor that rebuilds the kernel and runs the program. Success: in a viva, every claim about the kernel can be shown live, and the desktop never misrepresents what the kernel does.

## Positioning

Every number, state and event on screen comes from the running kernel's own trace (`#T SNAP`, `HEAPMAP`, `DISPATCH`, `OFF`, `WAKE`, `SPAWN`, `EXIT`, policy events, the boot trace) or from real host actions (cargo builds, files on disk, test logs). Nothing is mocked or simulated.

## Operating Context

- Served by `frontend/server.py` (Python standard library) inside WSL Ubuntu at `http://localhost:8080`; viewed in a desktop browser on the laptop, or projected.
- The kernel runs in QEMU without a display (`test=shell monitor=1 trace=1`). Snapshots arrive 4 times a second and the heap map once a second over Server-Sent Events; the scheduler's trace lines are summarised by the server per snapshot; keystrokes go back to the serial console.
- The idle shell polls the keyboard every 10 ms, so tracing costs about 4% of one host core.

## Capabilities and Constraints

- Kernel facts that must stay true on screen: RISC-V 64 (`riscv64`), QEMU `virt`, single hart, 128 MiB RAM VM, 16 MiB kernel heap, 1 ms timer tick, up to 512 tasks; policies RR, PRIO, PRIO+aging, MLFQ, FCFS, SJF, SRTF, HRRN. It is a monolithic educational kernel in S-mode, **not a microkernel**.
- **No message-passing IPC.** Tasks interact through semaphores and mutexes; the UI shows the kernel's real wake-ups ("task X woke task Y") and labels them as wake-ups, never as IPC.
- **No file system and no compiler in the kernel.** The file explorer shows the project's real files on the host (read-only, except programs). The code editor edits `kernel/src/programs/*.rs` on the host; Build runs `cargo build --release` in WSL, labelled as a host build, then reboots the VM and runs the program.
- The terminal is xterm.js connected to the kernel's serial shell (`nexus@os:~$`); the kernel prints basic ANSI colours, which the desktop maps to its palette.
- Everything is served locally (fonts, xterm.js), so it works without internet.
- The separate Debian-based "Micro-OS Linux" distribution (`distro/`) is a different artifact: it does not run in this desktop. Its measured boot times and component checks are shown in the Research app, labelled as a separate system.

## Brand Commitments

- Name on screen: **NEXUS OS**. Repository, report and code keep "Micro-OS" as the project name.
- Direction chosen by the student: a **spatial premium desktop OS** (soft depth, translucent layered windows, a live generative wallpaper, desktop clock and widgets, a dock, Control Center, Mission Control, Spotlight-style search, a lock screen). It must read instantly as a real, polished operating system, at least the level of a classmate's macOS-like web desktop, without copying it.
- Everything the desktop shows stays live from the kernel; widgets are real instruments, not decoration.
- Anti-references: copying the classmate's "Zenkai OS" (its photo wallpaper, its centred clock, its music player, its assets); the earlier ASTRA and NEXUS-topography looks, which the student rejected; a generic dashboard.

## Evidence on Hand

- Real kernel output: boot trace, live snapshots and scheduler traces, heap map, test results (`results/*.md`, `results/*.png`), R3 research tables and figures, the program sources.
- No users, testimonials or benchmarks beyond the measured results; none may be invented.

## Product Principles

1. Show the mechanism, not a picture of it: everything visible is driven by the live kernel.
2. Truth over decoration: labels name real hardware, real versions, real states and real limits.
3. Legible from across a room: a projector demo is a primary scene.
4. The terminal and the topography are the heart; the other apps explain and act on what they show.
