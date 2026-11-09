//! Layer 10 — integrated test scenarios.
//!
//! Each scenario boots tasks through the real scheduler, then checks what the
//! mechanisms actually did (dispatch order, counters, heap state) instead of
//! only "it ran". Every check prints `[PASS]`/`[FAIL]` and a `#T CHECK` trace
//! event; the kernel's exit code is the number of failed checks.
//!
//! Select with `test=<name>` on the command line (default `all`).

/// `check!(suite, "ID", condition, "detail {}", x)`
macro_rules! check {
    ($suite:expr, $id:expr, $cond:expr, $($arg:tt)*) => {
        $suite.check($id, $cond, format_args!($($arg)*))
    };
}

mod failure;
mod mem_tests;
mod research;
mod sched_tests;
mod stress;
mod sync_tests;

use alloc::string::String;
use core::fmt;

use microos_core::args::BootArgs;
use microos_core::sched::{PolicyKind, DEFAULT_PRIO};

use crate::scheduler::{self, workload::Work, Config, RunReport, SpawnOpts};

#[derive(Default)]
pub struct Suite {
    passed: u32,
    failed: u32,
}

impl Suite {
    pub fn check(&mut self, id: &str, ok: bool, detail: fmt::Arguments) {
        if ok {
            self.passed += 1;
            kinfo!("[PASS] {}: {}", id, detail);
        } else {
            self.failed += 1;
            kerror!("[FAIL] {}: {}", id, detail);
        }
        ktrace!("CHECK", "id={} ok={}", id, ok as u8);
    }
}

/// Parameters a user can override on the command line for TC1/TC2/TC6.
pub struct Options {
    pub policy: PolicyKind,
    pub quantum: u32,
}

impl Options {
    fn from_args(args: &BootArgs) -> Self {
        let policy = match args.get("policy") {
            None => PolicyKind::RoundRobin,
            Some(name) => PolicyKind::parse(name).unwrap_or_else(|| {
                kwarn!("unknown policy '{}', using rr (valid: rr prio aging mlfq fcfs sjf srtf hrrn)", name);
                PolicyKind::RoundRobin
            }),
        };
        let quantum = args.get_u64("quantum").unwrap_or(4).clamp(1, 1000) as u32;
        Self { policy, quantum }
    }
}

type Scenario = fn(&mut Suite, &Options);

const ALL: &[(&str, Scenario)] = &[
    ("tc1", sched_tests::tc1_two_tasks),
    ("tc2", sched_tests::tc2_multiple_tasks),
    ("tc3", sched_tests::tc3_boundaries),
    ("tc4", stress::tc4_stress),
    ("tc5", failure::tc5_failures),
    ("tc6", sched_tests::tc6_repeatability),
    ("coop", sched_tests::coop_vs_preemptive),
    ("race", sync_tests::race),
    ("prodcons", sync_tests::producer_consumer),
    ("mem", mem_tests::allocator_comparison),
    ("sched_cmp", sched_tests::policy_comparison),
    ("quantum_sweep", sched_tests::quantum_sweep),
    ("r3_quick", r3_quick),
];

/// Small R3 run inside the regular suite: checks the new policies and the
/// research measurements work. The full experiment is `test=research`.
fn r3_quick(s: &mut Suite, _o: &Options) {
    research::run(s, research::Params::quick());
}

/// Run the named scenario (or `all`); returns the number of failed checks.
pub fn run(test: &str, args: &BootArgs) -> u32 {
    let opts = Options::from_args(args);
    let mut suite = Suite::default();
    match test {
        "all" => {
            for (name, f) in ALL {
                banner(name);
                f(&mut suite, &opts);
            }
        }
        "fault" => {
            banner("fault");
            failure::deliberate_fault();
        }
        "shell" => return crate::shell::run(args),
        "research" => {
            banner("research");
            research::run(&mut suite, research::Params::full(args));
        }
        name => match ALL.iter().find(|(n, _)| *n == name) {
            Some((n, f)) => {
                banner(n);
                f(&mut suite, &opts);
            }
            None => {
                kerror!("unknown test '{}'", name);
                for (n, _) in ALL {
                    kerror!("  available: {}", n);
                }
                return 1;
            }
        },
    }
    kinfo!("==== checks: {} passed, {} failed ====", suite.passed, suite.failed);
    ktrace!("SUMMARY", "passed={} failed={}", suite.passed, suite.failed);
    suite.failed
}

fn banner(name: &str) {
    println!();
    kinfo!("==================== scenario: {} ====================", name);
    ktrace!("SCENARIO", "name={}", name);
}

/// Declarative description of one synthetic task.
#[derive(Clone, Copy)]
pub struct Spec {
    pub name: &'static str,
    pub prio: u8,
    pub delay: u64,
    pub work: Work,
}

impl Spec {
    pub const fn cpu(name: &'static str, ticks: u64) -> Self {
        Self { name, prio: DEFAULT_PRIO, delay: 0, work: Work::Cpu(ticks) }
    }

    pub const fn new(name: &'static str, prio: u8, work: Work) -> Self {
        Self { name, prio, delay: 0, work }
    }
}

/// Run one workload under one scheduler configuration and print its report.
pub fn run_specs(label: String, policy: PolicyKind, quantum: u32, preemptive: bool, specs: &[Spec]) -> RunReport {
    scheduler::begin(Config { label, policy, quantum, preemptive, ..Config::default() });
    for s in specs {
        let work = s.work;
        scheduler::spawn(s.name, SpawnOpts { prio: s.prio, delay_ticks: s.delay, burst_ticks: work.cpu_demand() as u32 }, move || work.run())
            .expect("spawn failed");
    }
    let r = scheduler::run();
    print_report(&r);
    r
}

pub fn print_report(r: &RunReport) {
    kinfo!(
        "run '{}': policy={} q={} preemptive={} elapsed={}us dispatches={} switches={} preemptions={} yields={} blocks={}{}",
        r.label, r.policy, r.quantum, r.preemptive, r.elapsed_us, r.stats.dispatches, r.switches, r.stats.preemptions,
        r.stats.yields, r.stats.blocks, if r.deadlock { " DEADLOCK" } else { "" }
    );
    if r.tasks.len() > 12 {
        kinfo!("  ({} tasks; per-task table omitted, see EXIT trace events)", r.tasks.len());
    } else {
        kinfo!(
            "  {:>3} {:<6} {:>4} {:>9} {:>9} {:>9} {:>8} {:>4} {:>4} {:>6}",
            "tid", "name", "prio", "turn_us", "resp_us", "wait_us", "lat_us", "cpu", "disp", "stack"
        );
        for t in &r.tasks {
            kinfo!(
                "  {:>3} {:<6} {:>4} {:>9} {:>9} {:>9} {:>8} {:>4} {:>4} {:>6}",
                t.tid, t.name, t.prio, t.turnaround_us(), t.response_us(), t.wait_us, t.latency_us(),
                t.cpu_ticks, t.dispatches, t.stack_used
            );
        }
    }
    kinfo!(
        "  avg: turnaround={}us response={}us wait={}us",
        r.avg_turnaround_us(), r.avg_response_us(), r.avg_wait_us()
    );
}
