//! Research-to-Micro-OS (Track G): the R3 study "Fairness vs Throughput in
//! CPU Scheduling" re-run on this kernel's real scheduler instead of a
//! discrete-event simulator.
//!
//! Same design as the study:
//! * policies FCFS, SJF, SRTF, HRRN, RR (q = 2 ms) and RR (q = 8 ms);
//! * uniform U(2,30) ms and bimodal (75 % U(1,6), 25 % U(20,80)) bursts;
//! * Poisson-like arrivals at offered load rho = 0.7 / 0.9 / 1.1;
//! * a modelled context-switch cost (default 0.2 ms) whenever the CPU moves
//!   to a different task;
//! * throughput = jobs completed inside the arrival window, per 100 ms;
//!   CPU utilisation; context-switch share; waiting time = finish - arrival
//!   - burst (mean, variance, max); Jain's index of CPU allocation over
//!   250 ms intervals, plus the slowdown-based index;
//! * Experiment 2: RR quantum sweep on bimodal rho = 1.1.
//!
//! Differences (stated in docs/research_sync.md): 1 ms clock resolution
//! (no 0.5 ms quantum), a shorter window and fewer seeds so the run fits in
//! minutes, and jobs that really execute on the CPU.
//!
//! Options: `r3_seeds=N` (3), `r3_window=MS` (5000, as in the study), `r3_cs_us=US` (200).

use alloc::{format, string::String, vec::Vec};

use microos_core::args::BootArgs;
use microos_core::research::{generate, jain_bp, Dist};
use microos_core::sched::{PolicyKind, DEFAULT_PRIO};

use super::Suite;
use crate::scheduler::{self, workload::cpu_burst_us, BurstInfo, Config, SpawnOpts};

const POLICIES: [(&str, PolicyKind, u32); 6] = [
    ("FCFS", PolicyKind::Fcfs, 1),
    ("SJF", PolicyKind::Sjf, 1),
    ("SRTF", PolicyKind::Srtf, 1),
    ("HRRN", PolicyKind::Hrrn, 1),
    ("RR2", PolicyKind::RoundRobin, 2),
    ("RR8", PolicyKind::RoundRobin, 8),
];
const FAIR_INTERVAL_MS: u64 = 250;

pub struct Params {
    seeds: u64,
    window: u64,
    cs_us: u64,
    loads: &'static [u64],
    quanta: &'static [u32],
    verdicts: bool,
}

impl Params {
    pub fn full(args: &BootArgs) -> Self {
        Self {
            seeds: args.get_u64("r3_seeds").unwrap_or(3).clamp(1, 30),
            window: args.get_u64("r3_window").unwrap_or(5000).clamp(250, 10_000),
            cs_us: args.get_u64("r3_cs_us").unwrap_or(200).min(5000),
            loads: &[70, 90, 110],
            quanta: &[1, 2, 4, 8, 16, 32, 64, 128],
            verdicts: true,
        }
    }

    /// A small version for the regular test suite: checks the mechanisms,
    /// not the statistics.
    pub fn quick() -> Self {
        Self { seeds: 1, window: 500, cs_us: 200, loads: &[110], quanta: &[1, 8, 128], verdicts: false }
    }
}

/// Measurements of one run (integers; *_bp = basis points, x100 = hundredths).
#[derive(Clone, Copy, Default)]
struct Outcome {
    jobs: u64,
    throughput_x100: u64,
    util_bp: u64,
    cs_bp: u64,
    /// None when no task stayed for a whole fairness interval.
    jain_bp: Option<u64>,
    slowdown_jain_bp: u64,
    mean_wait_us: u64,
    var_wait_ms2: u64,
    max_wait_us: u64,
    preemptions: u64,
    complete: bool,
}

fn one_run(label: &str, policy: PolicyKind, quantum: u32, dist: Dist, load: u64, seed: u64, p: &Params) -> Outcome {
    let jobs = generate(dist, load, p.window, seed);
    scheduler::begin(Config {
        label: label.into(),
        policy,
        quantum,
        preemptive: true,
        switch_cost_us: p.cs_us,
        bursts: BurstInfo::Oracle,
        trace_slices: false,
        window_ticks: p.window,
        fair_interval: FAIR_INTERVAL_MS,
    });
    for j in &jobs {
        let burst = j.burst;
        let opts = SpawnOpts { prio: DEFAULT_PRIO, delay_ticks: j.arrival, burst_ticks: burst };
        scheduler::spawn("J", opts, move || cpu_burst_us(burst as u64 * 1000)).expect("spawn failed (raise MAX_TASKS?)");
    }
    let r = scheduler::run();

    let window_us = p.window * 1000;
    let w = r.window.unwrap_or_default();
    let switch_us = w.switches * p.cs_us;
    // Paper definition: waiting = finish - arrival - burst (switch cost included).
    let waits: Vec<u64> = r.tasks.iter().map(|t| t.turnaround_us().saturating_sub(t.cpu_us)).collect();
    let n = waits.len().max(1) as u64;
    let mean = waits.iter().sum::<u64>() / n;
    let var_us2 = waits.iter().map(|&x| (x.abs_diff(mean) as u128).pow(2)).sum::<u128>() / n as u128;
    // 1 / slowdown = burst / turnaround, in parts per million.
    let inv_sd: Vec<u64> = r.tasks.iter().map(|t| t.cpu_us * 1_000_000 / t.turnaround_us().max(1)).collect();
    let (s1, s2) = inv_sd.iter().fold((0u64, 0u128), |(a, b), &x| (a + x, b + x as u128 * x as u128));

    let o = Outcome {
        jobs: jobs.len() as u64,
        throughput_x100: w.completed * 100 * 100 / p.window,
        // Exact running time; modelled switch time is reported separately.
        util_bp: (w.busy_us * 10_000 / window_us).min(10_000),
        cs_bp: switch_us * 10_000 / window_us,
        jain_bp: r.fairness_bp,
        slowdown_jain_bp: jain_bp(inv_sd.len() as u64, s1, s2),
        mean_wait_us: mean,
        var_wait_ms2: (var_us2 / 1_000_000) as u64,
        max_wait_us: waits.iter().copied().max().unwrap_or(0),
        preemptions: r.stats.preemptions,
        complete: r.tasks.len() == jobs.len() && !r.deadlock && r.window.is_some(),
    };
    ktrace!(
        "R3RUN",
        "run={} dist={} load={} policy={} q={} seed={} jobs={} done_in_window={} thr_x100={} util_bp={} cs_bp={} \
         jain_bp={} sjain_bp={} mean_wait_us={} var_wait_ms2={} max_wait_us={} switches_in_window={} preemptions={}",
        label, dist.name(), load, r.policy, quantum, seed, o.jobs, w.completed, o.throughput_x100, o.util_bp, o.cs_bp,
        na(o.jain_bp), o.slowdown_jain_bp, o.mean_wait_us, o.var_wait_ms2, o.max_wait_us, w.switches, o.preemptions
    );
    o
}

/// Mean of each field over seeds.
fn average(runs: &[Outcome]) -> Outcome {
    let n = runs.len().max(1) as u64;
    let m = |f: fn(&Outcome) -> u64| runs.iter().map(f).sum::<u64>() / n;
    Outcome {
        jobs: m(|o| o.jobs),
        throughput_x100: m(|o| o.throughput_x100),
        util_bp: m(|o| o.util_bp),
        cs_bp: m(|o| o.cs_bp),
        jain_bp: {
            let v: Vec<u64> = runs.iter().filter_map(|o| o.jain_bp).collect();
            (!v.is_empty()).then(|| v.iter().sum::<u64>() / v.len() as u64)
        },
        slowdown_jain_bp: m(|o| o.slowdown_jain_bp),
        mean_wait_us: m(|o| o.mean_wait_us),
        var_wait_ms2: m(|o| o.var_wait_ms2),
        max_wait_us: runs.iter().map(|o| o.max_wait_us).max().unwrap_or(0),
        preemptions: m(|o| o.preemptions),
        complete: runs.iter().all(|o| o.complete),
    }
}

/// Trace-friendly number or "na".
fn na(v: Option<u64>) -> String {
    v.map_or_else(|| String::from("na"), |x| format!("{}", x))
}

fn row(name: &str, a: &Outcome) {
    kinfo!(
        "  {:<6} thr {:>2}.{:02}/100ms  util {:>2}.{:01}%  cs {:>2}.{:02}%  wait mean {:>6} ms  var {:>8} ms2  max {:>6} ms  Jain {}  (slowdown-J {}.{:02})",
        name, a.throughput_x100 / 100, a.throughput_x100 % 100, a.util_bp / 100, (a.util_bp % 100) / 10,
        a.cs_bp / 100, a.cs_bp % 100, a.mean_wait_us / 1000, a.var_wait_ms2, a.max_wait_us / 1000,
        a.jain_bp.map_or_else(|| String::from("n/a"), |j| format!("{}.{:02}", j / 10_000, (j % 10_000) / 100)),
        a.slowdown_jain_bp / 10_000, (a.slowdown_jain_bp % 10_000) / 100
    );
}

pub fn run(s: &mut Suite, p: Params) {
    kinfo!(
        "R3 reproduction: {} seed(s), {} ms window, {} us switch cost, Jain over {} ms intervals",
        p.seeds, p.window, p.cs_us, FAIR_INTERVAL_MS
    );
    let mut all_complete = true;
    let mut np_preemptions = 0u64; // FCFS/SJF/HRRN must never be preempted
    let mut srtf_preemptions = 0u64;
    // results[dist][load][policy]
    let mut exp1: Vec<(Dist, u64, Vec<Outcome>)> = Vec::new();

    for dist in [Dist::Uniform, Dist::Bimodal] {
        for &load in p.loads {
            kinfo!("-- Experiment 1: {} workload, load {}.{}", dist.name(), load / 100, load % 100 / 10);
            let mut per_policy = Vec::new();
            for (name, kind, q) in POLICIES {
                let runs: Vec<Outcome> = (1..=p.seeds)
                    .map(|seed| one_run(&format!("r3_{}_{}_{}_s{}", dist.name(), load, name, seed), kind, q, dist, load, seed, &p))
                    .collect();
                let a = average(&runs);
                all_complete &= a.complete;
                let pre: u64 = runs.iter().map(|o| o.preemptions).sum();
                match kind {
                    PolicyKind::Fcfs | PolicyKind::Sjf | PolicyKind::Hrrn => np_preemptions += pre,
                    PolicyKind::Srtf => srtf_preemptions += pre,
                    _ => {}
                }
                row(name, &a);
                ktrace!(
                    "R3",
                    "exp=1 dist={} load={} policy={} seeds={} jobs={} thr_x100={} util_bp={} cs_bp={} jain_bp={} sjain_bp={} mean_wait_us={} var_wait_ms2={} max_wait_us={}",
                    dist.name(), load, name, p.seeds, a.jobs, a.throughput_x100, a.util_bp, a.cs_bp, na(a.jain_bp),
                    a.slowdown_jain_bp, a.mean_wait_us, a.var_wait_ms2, a.max_wait_us
                );
                per_policy.push(a);
            }
            exp1.push((dist, load, per_policy));
        }
    }

    kinfo!("-- Experiment 2: RR quantum sweep, bimodal workload, load 1.1");
    let mut sweep = Vec::new();
    for &q in p.quanta {
        let runs: Vec<Outcome> = (1..=p.seeds)
            .map(|seed| one_run(&format!("r3_sweep_q{}_s{}", q, seed), PolicyKind::RoundRobin, q, Dist::Bimodal, 110, seed, &p))
            .collect();
        let a = average(&runs);
        all_complete &= a.complete;
        row(&format!("q={}", q), &a);
        ktrace!(
            "R3",
            "exp=2 dist=bimodal load=110 policy=RR q={} seeds={} jobs={} thr_x100={} util_bp={} cs_bp={} jain_bp={} sjain_bp={} mean_wait_us={} var_wait_ms2={} max_wait_us={}",
            q, p.seeds, a.jobs, a.throughput_x100, a.util_bp, a.cs_bp, na(a.jain_bp), a.slowdown_jain_bp, a.mean_wait_us,
            a.var_wait_ms2, a.max_wait_us
        );
        sweep.push((q, a));
    }

    check!(s, "R3.all_jobs_complete", all_complete, "every generated job ran to completion in every run (no deadlock)");
    check!(s, "R3.nonpreemptive_never_preempted", np_preemptions == 0, "FCFS/SJF/HRRN preemptions: {}", np_preemptions);
    check!(s, "R3.srtf_preempts_on_shorter_arrival", srtf_preemptions > 0, "SRTF preemptions: {}", srtf_preemptions);

    if p.verdicts {
        verdicts(&exp1, &sweep);
    }
}

/// Score the study's hypotheses on the kernel's numbers. These are research
/// findings, not pass/fail checks: an unsupported hypothesis is a result.
fn verdicts(exp1: &[(Dist, u64, Vec<Outcome>)], sweep: &[(u32, Outcome)]) {
    let idx = |name: &str| POLICIES.iter().position(|(n, _, _)| *n == name).unwrap();
    let mut h1 = true;
    for (dist, load, v) in exp1 {
        if *load != 110 {
            continue;
        }
        let (sjf, srtf, rr2) = (&v[idx("SJF")], &v[idx("SRTF")], &v[idx("RR2")]);
        let (js, jr, j2) = (sjf.jain_bp.unwrap_or(10_000), srtf.jain_bp.unwrap_or(10_000), rr2.jain_bp.unwrap_or(0));
        let ok = sjf.throughput_x100 >= rr2.throughput_x100
            && srtf.throughput_x100 >= rr2.throughput_x100
            && js < 5000
            && jr < 5000
            && j2 >= 9000;
        kinfo!(
            "H1 [{}]: throughput SJF {} SRTF {} RR2 {} (x0.01 jobs/100ms); Jain SJF {} SRTF {} RR2 {} (bp) -> {}",
            dist.name(), sjf.throughput_x100, srtf.throughput_x100, rr2.throughput_x100, na(sjf.jain_bp), na(srtf.jain_bp),
            na(rr2.jain_bp), if ok { "supported" } else { "not supported" }
        );
        h1 &= ok;
    }
    let mut h2 = true;
    for (dist, load, v) in exp1 {
        if *load != 70 {
            continue;
        }
        let (lo, hi) = v.iter().fold((u64::MAX, 0), |(lo, hi), o| (lo.min(o.throughput_x100), hi.max(o.throughput_x100)));
        let spread_bp = (hi - lo) * 10_000 / hi.max(1);
        let ok = spread_bp <= 300;
        kinfo!("H2 [{}]: throughput at load 0.7 ranges {}..{} (spread {}.{:02}%) -> {}",
            dist.name(), lo, hi, spread_bp / 100, spread_bp % 100, if ok { "supported" } else { "not supported" });
        h2 &= ok;
    }
    let best = sweep.iter().max_by_key(|(_, o)| o.throughput_x100).map(|(q, o)| (*q, *o));
    let (first, last) = (sweep.first(), sweep.last());
    let h3 = match (best, first, last) {
        (Some((bq, b)), Some((fq, f)), Some((lq, l))) => {
            let peak_inside = bq != *fq && bq != *lq;
            let fairness_falls = matches!((l.jain_bp, f.jain_bp), (Some(a), Some(b)) if a < b);
            kinfo!(
                "H3: throughput peaks at q={} ({}); q={} gives {}, q={} gives {}; Jain falls from {} to {} -> {}",
                bq, b.throughput_x100, fq, f.throughput_x100, lq, l.throughput_x100, na(f.jain_bp), na(l.jain_bp),
                if peak_inside && fairness_falls { "supported" } else { "not supported" }
            );
            peak_inside && fairness_falls
        }
        _ => false,
    };
    for (h, ok) in [("H1", h1), ("H2", h2), ("H3", h3)] {
        ktrace!("VERDICT", "hypothesis={} supported={}", h, ok as u8);
    }
}
