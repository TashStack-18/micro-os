//! Scheduling scenarios: TC1, TC2, TC3, TC6, cooperative vs preemptive,
//! the policy comparison (specialization) and the quantum sweep.

use alloc::{format, vec::Vec};

use microos_core::sched::PolicyKind;

use super::{run_specs, Options, Spec, Suite};
use crate::scheduler::{workload::Work, RunReport};

/// TC2 workload: five CPU-bound tasks with different burst lengths (ticks).
const TC2: [Spec; 5] = [
    Spec::cpu("T1", 12),
    Spec::cpu("T2", 24),
    Spec::cpu("T3", 8),
    Spec::cpu("T4", 30),
    Spec::cpu("T5", 16),
];

fn total_demand(specs: &[Spec]) -> u64 {
    specs.iter().map(|s| s.work.cpu_demand()).sum()
}

fn all_completed(r: &RunReport, n: usize) -> bool {
    r.tasks.len() == n && !r.deadlock
}

fn got_their_cpu(r: &RunReport, specs: &[Spec]) -> bool {
    specs.iter().all(|s| r.task(s.name).is_some_and(|t| t.cpu_ticks >= s.work.cpu_demand()))
}

/// TC1 — minimum configuration: two equal CPU-bound tasks.
pub fn tc1_two_tasks(s: &mut Suite, o: &Options) {
    let specs = [Spec::cpu("A", 20), Spec::cpu("B", 20)];
    let r = run_specs("tc1".into(), o.policy, o.quantum, true, &specs);
    check!(s, "TC1.complete", all_completed(&r, 2), "{} of 2 tasks finished", r.tasks.len());

    // While both are alive, the scheduler must alternate between them.
    let both_alive = 2 * (20 / o.quantum as usize).saturating_sub(1);
    let window = &r.order[..both_alive.min(r.order.len())];
    let alternates = window.windows(2).all(|w| w[0] != w[1]);
    check!(s, "TC1.alternation", alternates, "first {} dispatches alternate: {:?}", window.len(), window);

    let slices = (20 + o.quantum as u64 - 1) / o.quantum as u64;
    let ok = r.tasks.iter().all(|t| (slices..=slices + 1).contains(&t.dispatches));
    check!(s, "TC1.time_slices", ok, "each task needed ~{} slices of {} ticks", slices, o.quantum);
    check!(s, "TC1.cpu_received", got_their_cpu(&r, &specs), "both tasks received >= 20 CPU ticks");
}

/// TC2 — 5 tasks with mixed bursts; RR finishes them in burst order.
pub fn tc2_multiple_tasks(s: &mut Suite, o: &Options) {
    let r = run_specs("tc2".into(), o.policy, o.quantum, true, &TC2);
    check!(s, "TC2.complete", all_completed(&r, 5), "{} of 5 tasks finished", r.tasks.len());
    check!(s, "TC2.cpu_received", got_their_cpu(&r, &TC2), "every task received its full burst");
    if o.policy == PolicyKind::RoundRobin {
        let order = r.completion_order();
        let expected = ["T3", "T1", "T5", "T2", "T4"];
        check!(s, "TC2.rr_completion_order", order == expected, "completion order {:?}, expected {:?}", order, expected);
    }
}

/// TC3 — boundary cases: smallest quantum, quantum larger than any burst,
/// a single task, and a task with no work at all.
pub fn tc3_boundaries(s: &mut Suite, _o: &Options) {
    let demand = total_demand(&TC2);
    let r = run_specs("tc3_q1".into(), PolicyKind::RoundRobin, 1, true, &TC2);
    check!(
        s, "TC3.quantum_1", all_completed(&r, 5) && r.stats.dispatches + 5 >= demand,
        "q=1: {} dispatches for {} ticks of work (one switch per tick)", r.stats.dispatches, demand
    );

    let r = run_specs("tc3_qmax".into(), PolicyKind::RoundRobin, 1000, true, &TC2);
    let fcfs = r.completion_order() == ["T1", "T2", "T3", "T4", "T5"];
    check!(
        s, "TC3.quantum_huge_is_fcfs", all_completed(&r, 5) && r.stats.dispatches == 5 && r.stats.preemptions == 0 && fcfs,
        "q=1000: dispatches={} preemptions={} order={:?}", r.stats.dispatches, r.stats.preemptions, r.completion_order()
    );

    let r = run_specs("tc3_single".into(), PolicyKind::RoundRobin, 4, true, &[Spec::cpu("SOLO", 10)]);
    check!(
        s, "TC3.single_task", all_completed(&r, 1) && r.stats.dispatches == 3,
        "one task, 10 ticks, q=4: {} dispatches (expected 3; re-picked after each preemption)", r.stats.dispatches
    );

    let r = run_specs("tc3_empty".into(), PolicyKind::RoundRobin, 4, true, &[Spec::cpu("NOP", 0)]);
    check!(
        s, "TC3.zero_work", all_completed(&r, 1) && r.stats.dispatches == 1 && r.stats.preemptions == 0,
        "zero-length task: dispatches={} preemptions={}", r.stats.dispatches, r.stats.preemptions
    );
}

/// TC6 — run the TC2 workload three times and compare the dispatch traces.
pub fn tc6_repeatability(s: &mut Suite, o: &Options) {
    let runs: Vec<RunReport> = (1..=3)
        .map(|i| run_specs(format!("tc6_r{}", i), o.policy, o.quantum, true, &TC2))
        .collect();
    for (i, r) in runs.iter().enumerate() {
        ktrace!("REPEAT", "iter={} hash={:#018x} dispatches={} avg_turn={}", i + 1, r.order_hash(), r.stats.dispatches, r.avg_turnaround_us());
    }
    let same_order = runs.windows(2).all(|w| w[0].order_hash() == w[1].order_hash());
    check!(s, "TC6.same_dispatch_sequence", same_order, "dispatch-sequence hashes: {:#x?}", runs.iter().map(RunReport::order_hash).collect::<Vec<_>>());
    let same_completion = runs.windows(2).all(|w| w[0].completion_order() == w[1].completion_order());
    check!(s, "TC6.same_completion_order", same_completion, "{:?}", runs[0].completion_order());
    let turns: Vec<u64> = runs.iter().map(RunReport::avg_turnaround_us).collect();
    let spread = turns.iter().max().unwrap() - turns.iter().min().unwrap();
    kinfo!("TC6: avg turnaround per run {:?} us (spread {} us)", turns, spread);
}

/// Cooperative vs timer-driven scheduling with a CPU hog that never yields.
pub fn coop_vs_preemptive(s: &mut Suite, _o: &Options) {
    let specs = [
        Spec::cpu("HOG", 40),
        Spec::new("I1", 4, Work::CoopCpu(5)),
        Spec::new("I2", 4, Work::CoopCpu(5)),
        Spec::new("I3", 4, Work::CoopCpu(5)),
    ];
    let coop = run_specs("coop_off".into(), PolicyKind::RoundRobin, 4, false, &specs);
    let pre = run_specs("coop_on".into(), PolicyKind::RoundRobin, 4, true, &specs);
    let interactive_resp = |r: &RunReport| {
        let v: Vec<u64> = r.tasks.iter().filter(|t| t.name != "HOG").map(|t| t.response_us()).collect();
        v.iter().sum::<u64>() / v.len().max(1) as u64
    };
    let (rc, rp) = (interactive_resp(&coop), interactive_resp(&pre));
    let hog_disp = coop.task("HOG").map_or(0, |t| t.dispatches);
    check!(s, "COOP.hog_monopolises", hog_disp == 1, "without preemption HOG ran in {} dispatch(es)", hog_disp);
    check!(
        s, "COOP.preemption_improves_response", rp < rc,
        "avg response of yielding tasks: cooperative={}us preemptive={}us", rc, rp
    );
    ktrace!("COOP", "coop_resp={} preempt_resp={}", rc, rp);
}

/// Specialization (Track A): the same mixed workload under four policies.
pub fn policy_comparison(s: &mut Suite, _o: &Options) {
    let interactive = Work::Interactive { rounds: 10, cpu: 1, sleep: 3 };
    let specs = [
        Spec::new("CPU1", 3, Work::Cpu(30)),
        Spec::new("CPU2", 3, Work::Cpu(40)),
        Spec::new("CPU3", 3, Work::Cpu(50)),
        Spec::new("INT1", 1, interactive),
        Spec::new("INT2", 1, interactive),
        Spec::new("BATCH", 7, Work::Cpu(20)),
    ];
    let policies = [
        ("rr", PolicyKind::RoundRobin),
        ("prio", PolicyKind::Priority),
        ("aging", PolicyKind::PriorityAging),
        ("mlfq", PolicyKind::Mlfq),
    ];
    let reports: Vec<RunReport> = policies
        .iter()
        .map(|(tag, kind)| run_specs(format!("cmp_{}", tag), *kind, 4, true, &specs))
        .collect();

    let int_latency = |r: &RunReport| {
        let v: Vec<u64> = r.tasks.iter().filter(|t| t.name.starts_with("INT")).map(|t| t.latency_us()).collect();
        v.iter().sum::<u64>() / v.len().max(1) as u64
    };
    let batch = |r: &RunReport| r.task("BATCH").cloned().expect("BATCH missing");
    let cpu_done = |r: &RunReport| r.tasks.iter().filter(|t| t.name.starts_with("CPU")).map(|t| t.finish_us).max().unwrap_or(0);

    kinfo!("policy     avg_turn  avg_resp  int_lat  batch_resp  dispatches");
    for r in &reports {
        let b = batch(r);
        kinfo!(
            "{:<10} {:>8} {:>9} {:>8} {:>11} {:>11}",
            r.policy, r.avg_turnaround_us(), r.avg_response_us(), int_latency(r), b.response_us(), r.stats.dispatches
        );
        ktrace!(
            "CMP",
            "run={} policy={} avg_turn={} avg_resp={} int_lat={} batch_resp={} batch_turn={} dispatches={}",
            r.label, r.policy, r.avg_turnaround_us(), r.avg_response_us(), int_latency(r), b.response_us(),
            b.turnaround_us(), r.stats.dispatches
        );
        check!(s, "CMP.complete", all_completed(r, specs.len()), "{}: all {} tasks finished", r.policy, specs.len());
    }
    let (rr, prio, aging, mlfq) = (&reports[0], &reports[1], &reports[2], &reports[3]);

    check!(
        s, "CMP.prio_starves_batch", batch(prio).first_run_us >= cpu_done(prio),
        "PRIO: BATCH first ran at {}us, after all CPU tasks finished at {}us", batch(prio).first_run_us, cpu_done(prio)
    );
    check!(
        s, "CMP.aging_prevents_starvation", batch(aging).response_us() < batch(prio).response_us(),
        "BATCH response: PRIO={}us PRIO+AGING={}us", batch(prio).response_us(), batch(aging).response_us()
    );
    check!(
        s, "CMP.mlfq_favours_interactive", int_latency(mlfq) <= int_latency(rr),
        "interactive scheduling latency: RR={}us MLFQ={}us", int_latency(rr), int_latency(mlfq)
    );
}

/// Research link (CA-4): how the RR quantum trades switches for response.
pub fn quantum_sweep(s: &mut Suite, _o: &Options) {
    let mut specs: Vec<Spec> = TC2.to_vec();
    specs.push(Spec::new("INT", 4, Work::Interactive { rounds: 8, cpu: 1, sleep: 2 }));
    let mut prev_dispatches = u64::MAX;
    let mut monotonic = true;
    for q in [1u32, 2, 4, 8, 16, 32] {
        let r = run_specs(format!("sweep_q{}", q), PolicyKind::RoundRobin, q, true, &specs);
        let int = r.task("INT").map_or(0, |t| t.latency_us());
        ktrace!(
            "SWEEP",
            "q={} dispatches={} preemptions={} avg_turn={} avg_resp={} int_lat={} elapsed={}",
            q, r.stats.dispatches, r.stats.preemptions, r.avg_turnaround_us(), r.avg_response_us(), int, r.elapsed_us
        );
        check!(s, "SWEEP.complete", all_completed(&r, specs.len()), "q={}: all tasks finished", q);
        monotonic &= r.stats.dispatches <= prev_dispatches;
        prev_dispatches = r.stats.dispatches;
    }
    check!(s, "SWEEP.fewer_switches_with_larger_q", monotonic, "dispatch count never increases as q grows");
}
