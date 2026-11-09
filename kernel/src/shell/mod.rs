//! Interactive command shell on the serial console (`test=shell`).
//!
//! The shell is itself a task (priority 0). It polls the UART once per
//! timer tick and sleeps in between, so programs it starts get the CPU.
//! Foreground programs are waited for (Ctrl-C asks them to stop);
//! programs started with a trailing `&` run in the background.
//!
//! With `monitor=1` a second task emits a `#T SNAP` line every
//! `monitor_ms` (default 250) describing all tasks, the scheduler and the
//! heap; the web front end (`frontend/server.py`) draws its live views
//! from these.

use alloc::{string::String, vec::Vec};
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering::Relaxed};

use microos_core::args::BootArgs;
use microos_core::sched::PolicyKind;

use crate::kernel::console::{self, Level};
use crate::scheduler::{self, BurstInfo, Config, SpawnOpts, MAX_TASKS};
use crate::{drivers, memory, programs, sys, timer};

const CTRL_C: u8 = 0x03;
const BACKSPACE: u8 = 0x08;
const DELETE: u8 = 0x7f;

/// Set when the shell exits, so the monitor task stops too.
static SHELL_DONE: AtomicBool = AtomicBool::new(false);
/// Running under the web panel (`monitor=1`): start the console on a clean screen.
static PANEL_MODE: AtomicBool = AtomicBool::new(false);

pub fn run(args: &BootArgs) -> u32 {
    let policy = args.get("policy").and_then(PolicyKind::parse).unwrap_or(PolicyKind::RoundRobin);
    let quantum = args.get_u64("quantum").unwrap_or(4).clamp(1, 1000) as u32;
    // Trace events would flood the terminal; `trace on` re-enables them.
    console::set_trace(args.flag("trace", false));
    SHELL_DONE.store(false, Relaxed);
    // Burst-ordered policies cannot be told a program's CPU demand here,
    // so the kernel predicts it from each task's history.
    let bursts = if policy.needs_bursts() { BurstInfo::Predicted } else { BurstInfo::Oracle };
    scheduler::begin(Config { label: "shell".into(), policy, quantum, preemptive: true, bursts, ..Config::default() });
    scheduler::spawn("shell", SpawnOpts { prio: 0, ..SpawnOpts::default() }, shell_main).expect("could not start the shell");
    PANEL_MODE.store(args.flag("monitor", false), Relaxed);
    if args.flag("monitor", false) {
        let interval_ms = args.get_u64("monitor_ms").unwrap_or(250).max(20);
        scheduler::spawn("monitor", SpawnOpts { prio: 0, ..SpawnOpts::default() }, move || monitor(interval_ms))
            .expect("could not start the monitor");
    }
    let r = scheduler::run();
    kinfo!("shell session over: {} tasks ran, {} dispatches, {} preemptions", r.tasks.len(), r.stats.dispatches, r.stats.preemptions);
    0
}

// ANSI colours (understood by xterm.js and real terminals alike).
const PURPLE: &str = "\x1b[38;5;141m";
const GREEN: &str = "\x1b[38;5;114m";
const DIM: &str = "\x1b[90m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";
const PINK: &str = "\x1b[38;5;219m";
const LINE: &str = "\x1b[38;5;97m";

/// A horizontal rule between sections of output.
fn rule() {
    println!("{LINE}{}{RESET}", "\u{2500}".repeat(76));
}

/// Checklist of what the kernel actually brought up, read from live state.
fn banner() {
    let ok = |text: core::fmt::Arguments| println!("{GREEN}[\u{2713}]{RESET}    {}", text);
    let h = memory::stats();
    if PANEL_MODE.load(Relaxed) {
        // Firmware and kernel boot logs scroll into the terminal history.
        print!("\x1b[2J\x1b[H");
    } else {
        println!();
    }
    println!("{PURPLE}[BOOT]{RESET} Micro-OS v{} (riscv64, S-mode, {})", env!("CARGO_PKG_VERSION"), drivers::sbi::impl_name());
    println!("{PURPLE}[BOOT]{RESET} Kernel subsystems:");
    ok(format_args!("Trap vector installed (timer interrupts on)"));
    ok(format_args!("Kernel heap ready ({} MiB, first-fit free list)", h.capacity >> 20));
    ok(format_args!("Timer running ({} us per tick)", timer::tick_us()));
    if let Some((policy, quantum, preemptive, _, _)) = scheduler::run_info() {
        let mode = if preemptive { "preemptive" } else { "cooperative" };
        ok(format_args!("Scheduler initialized ({}, quantum {} ticks, {})", policy, quantum, mode));
    }
    ok(format_args!("Starting tasks..."));
    for t in scheduler::snapshot() {
        println!("         {DIM}->{RESET} {} (pid: {})", t.name, t.tid);
    }
    println!("{GREEN}[\u{2713}]{RESET}    {BOLD}Micro-OS is ready!{RESET} {PINK}\u{2726}{RESET}  {DIM}type 'help' for commands, 'progs' for programs{RESET}");
    rule();
}

fn prompt() {
    print!("{PURPLE}{BOLD}micro@os{RESET}:{GREEN}~{RESET}$ ");
}

fn shell_main() {
    banner();
    let mut line = String::new();
    loop {
        prompt();
        read_line(&mut line);
        let words: Vec<&str> = line.split_whitespace().collect();
        let Some((&cmd, rest)) = words.split_first() else { continue };
        match cmd {
            "help" | "?" => help(),
            "progs" | "ls" => list_programs(),
            "run" => run_program(rest),
            "ps" => ps(),
            "top" => top(),
            "sched" => sched(),
            "mem" => mem(),
            "sysinfo" => sysinfo(),
            "uptime" => println!("up {} ms, {} timer ticks of {} us", sys::uptime_ms(), timer::ticks(), timer::tick_us()),
            "kill" => kill(rest),
            "trace" => set_trace(rest),
            "log" => set_log(rest),
            "echo" => println!("{}", rest.join(" ")),
            "clear" => print!("\x1b[2J\x1b[H"),
            "exit" | "shutdown" | "poweroff" => break,
            name if programs::find(name).is_some() => run_program(&words),
            other => println!("unknown command '{}' (try 'help')", other),
        }
    }
    SHELL_DONE.store(true, Relaxed);
    let others = scheduler::snapshot().iter().filter(|t| t.name != "shell" && t.name != "monitor").count();
    if others > 0 {
        println!("waiting for {} background task(s) to finish...", others);
    }
    println!("bye");
}

/// Live monitor task: one machine-readable snapshot per interval.
fn monitor(interval_ms: u64) {
    while !SHELL_DONE.load(Relaxed) {
        emit_snapshot();
        sys::sleep_ms(interval_ms);
    }
    emit_snapshot();
}

/// `#T ... SNAP key=value ... tasks=tid:name:state:prio:level:cpu:dispatches;...`
fn emit_snapshot() {
    let Some((policy, quantum, _, st, ready)) = scheduler::run_info() else { return };
    let h = memory::stats();
    let mut tasks = String::new();
    for t in scheduler::snapshot() {
        let sep = if tasks.is_empty() { "" } else { ";" };
        let _ = write!(
            tasks, "{}{}:{}:{}:{}:{}:{}:{}",
            sep, t.tid, t.name, t.state.as_str(), t.prio, t.level, t.cpu_ticks, t.dispatches
        );
    }
    console::trace_always(
        "SNAP",
        format_args!(
            "ticks={} idle={} disp={} pre={} yld={} blk={} wake={} ready={} policy={} q={} tick_us={} \
             heap_used={} heap_peak={} heap_cap={} free_blocks={} largest={} frag_bp={} allocs={} frees={} tasks={}",
            st.ticks, st.idle_ticks, st.dispatches, st.preemptions, st.yields, st.blocks, st.wakeups, ready,
            policy, quantum, timer::tick_us(), h.bytes_in_use, h.peak_bytes, h.capacity, h.free_blocks,
            h.largest_free, h.fragmentation_bp(), h.allocs, h.frees, tasks
        ),
    );
}

/// Read one line with echo and backspace. Ctrl-C discards the line.
fn read_line(line: &mut String) {
    line.clear();
    loop {
        match console::read_byte() {
            None => scheduler::sleep_ticks(1),
            Some(b'\r' | b'\n') => {
                println!();
                return;
            }
            Some(BACKSPACE | DELETE) => {
                if line.pop().is_some() {
                    print!("\x08 \x08");
                }
            }
            Some(CTRL_C) => {
                println!("^C");
                line.clear();
                return;
            }
            Some(b) if (0x20..0x7f).contains(&b) => {
                line.push(b as char);
                print!("{}", b as char);
            }
            Some(_) => {} // ignore other control bytes / escape sequences
        }
    }
}

fn help() {
    const LEFT: [(&str, &str); 8] = [
        ("help", "show this help"),
        ("progs", "list programs"),
        ("run [-p N] PROG [&]", "run a program"),
        ("PROG ARGS [&]", "same as run PROG"),
        ("ps", "list tasks"),
        ("top", "live task view"),
        ("kill PID", "ask a task to stop"),
        ("exit", "power off"),
    ];
    const RIGHT: [(&str, &str); 8] = [
        ("sched", "scheduler statistics"),
        ("mem", "kernel heap usage"),
        ("sysinfo", "system information"),
        ("uptime", "time since boot"),
        ("trace on|off", "kernel trace events"),
        ("log LEVEL", "error|warn|info|debug"),
        ("echo TEXT", "print text"),
        ("clear", "clear the screen"),
    ];
    println!("Available commands:");
    rule();
    for ((lc, ld), (rc, rd)) in LEFT.iter().zip(RIGHT.iter()) {
        println!(
            "  {PURPLE}{:<20}{RESET}{DIM}-{RESET} {:<20} {DIM}|{RESET}  {PURPLE}{:<13}{RESET}{DIM}-{RESET} {}",
            lc, ld, rc, rd
        );
    }
    rule();
    println!("{DIM}N = priority 0 (high) to 7 (low); '&' runs in the background; Ctrl-C stops the foreground program.{RESET}");
}

fn sysinfo() {
    let (major, minor) = drivers::sbi::spec_version();
    let h = memory::stats();
    let (ks, ke) = memory::kernel_image();
    let row = |k: &str, v: core::fmt::Arguments| println!("  {PURPLE}{:<12}{RESET} {}", k, v);
    row("os", format_args!("Micro-OS v{}", env!("CARGO_PKG_VERSION")));
    row("machine", format_args!("QEMU virt, 1 RISC-V hart (riscv64gc), supervisor mode"));
    row("firmware", format_args!("{} (SBI v{}.{})", drivers::sbi::impl_name(), major, minor));
    row("kernel", format_args!("{:#x}..{:#x} ({} KiB)", ks, ke, (ke - ks) / 1024));
    row("heap", format_args!("{} MiB first-fit free list, {} B in use", h.capacity >> 20, h.bytes_in_use));
    row("timer", format_args!("{} us per tick, {} ticks so far", timer::tick_us(), timer::ticks()));
    if let Some((policy, quantum, _, _, _)) = scheduler::run_info() {
        row("scheduler", format_args!("{} with a {}-tick quantum, max {} tasks", policy, quantum, MAX_TASKS));
    }
    row("uptime", format_args!("{} ms", sys::uptime_ms()));
}

fn list_programs() {
    for p in programs::PROGRAMS {
        println!("  {:<26} {}", p.usage, p.about);
    }
}

fn run_program(words: &[&str]) {
    let mut words: Vec<&str> = words.to_vec();
    let mut background = false;
    if let Some(last) = words.last_mut() {
        if *last == "&" {
            words.pop();
            background = true;
        } else if let Some(stripped) = last.strip_suffix('&') {
            *last = stripped;
            background = true;
        }
    }
    let mut prio = 4u8;
    if words.first() == Some(&"-p") {
        match words.get(1).and_then(|p| p.parse::<u8>().ok()) {
            Some(p) if p <= 7 => prio = p,
            _ => {
                println!("run: -p needs a priority 0-7");
                return;
            }
        }
        words.drain(..2);
    }
    let Some((&name, args)) = words.split_first() else {
        println!("usage: run [-p N] PROG [ARGS...] [&]");
        return;
    };
    let Some(prog) = programs::find(name) else {
        println!("run: no program '{}' (try 'progs')", name);
        return;
    };

    let owned: Vec<String> = args.iter().map(|a| String::from(*a)).collect();
    let (pname, main) = (prog.name, prog.main);
    let spawned = scheduler::spawn(pname, SpawnOpts { prio, ..SpawnOpts::default() }, move || {
        let args: Vec<&str> = owned.iter().map(String::as_str).collect();
        let code = main(&args);
        println!("[{} pid {} exited with code {}]", pname, sys::getpid(), code);
    });
    let tid = match spawned {
        Ok(tid) => tid,
        Err(e) => {
            println!("run: could not start '{}': {:?} (limit {} tasks)", name, e, MAX_TASKS);
            return;
        }
    };
    if background {
        println!("[started {} as pid {} in the background]", name, tid);
        return;
    }
    while scheduler::is_alive(tid) {
        if console::read_byte() == Some(CTRL_C) {
            println!("^C");
            scheduler::request_cancel(tid);
        }
        scheduler::sleep_ticks(1);
    }
}

fn ps() {
    println!("  PID NAME       STATE       PRIO LEVEL  CPU_TICKS  DISPATCHES");
    for t in scheduler::snapshot() {
        println!(
            "  {:>3} {:<10} {:<11} {:>4} {:>5} {:>10} {:>11}",
            t.tid, t.name, t.state.as_str(), t.prio, t.level, t.cpu_ticks, t.dispatches
        );
    }
}

fn sched() {
    let Some((policy, quantum, preemptive, st, ready)) = scheduler::run_info() else { return };
    let busy = st.ticks.saturating_sub(st.idle_ticks);
    println!("  policy        {} (quantum {} ticks, {})", policy, quantum, if preemptive { "preemptive" } else { "cooperative" });
    println!("  timer ticks   {} ({} idle, CPU busy {}%)", st.ticks, st.idle_ticks, busy * 100 / st.ticks.max(1));
    println!("  dispatches    {}  preemptions {}  yields {}", st.dispatches, st.preemptions, st.yields);
    println!("  blocks        {}  wakeups {}  ready queue {}", st.blocks, st.wakeups, ready);
    println!("  restart with another policy: scripts/shell.sh policy=rr|prio|aging|mlfq|fcfs|sjf|srtf|hrrn quantum=N");
}

fn mem() {
    let h = memory::stats();
    let (ks, ke) = memory::kernel_image();
    println!("  kernel image  {:#x}..{:#x} ({} KiB)", ks, ke, (ke - ks) / 1024);
    println!("  heap          {} KiB first-fit free list", h.capacity / 1024);
    println!("  in use        {} B (peak {} B)", h.bytes_in_use, h.peak_bytes);
    println!("  allocs/frees  {} / {}  (failed {}, rejected frees {})", h.allocs, h.frees, h.failed, h.rejected_frees);
    println!(
        "  free blocks   {}  largest {} B  fragmentation {}.{:02}%",
        h.free_blocks, h.largest_free, h.fragmentation_bp() / 100, h.fragmentation_bp() % 100
    );
}

fn top() {
    loop {
        print!("\x1b[2J\x1b[H");
        println!("Micro-OS top - up {} ms - press any key to quit", sys::uptime_ms());
        sched();
        println!();
        ps();
        if !sys::sleep_ms(1000) || console::read_byte().is_some() {
            return;
        }
    }
}

fn kill(args: &[&str]) {
    match args.first().and_then(|a| a.parse::<usize>().ok()) {
        Some(tid) if tid == sys::getpid() => println!("kill: use 'exit' to stop the shell"),
        Some(tid) if scheduler::request_cancel(tid) => println!("asked pid {} to stop", tid),
        Some(tid) => println!("kill: no task {}", tid),
        None => println!("usage: kill PID"),
    }
}

fn set_trace(args: &[&str]) {
    match args.first() {
        Some(&"on") => {
            console::set_trace(true);
        }
        Some(&"off") => {
            console::set_trace(false);
        }
        _ => println!("usage: trace on|off (currently {})", if console::trace_enabled() { "on" } else { "off" }),
    }
}

fn set_log(args: &[&str]) {
    match args.first().and_then(|a| Level::parse(a)) {
        Some(level) => console::set_level(level),
        None => println!("usage: log error|warn|info|debug (currently {:?})", console::level()),
    }
}
