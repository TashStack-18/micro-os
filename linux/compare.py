#!/usr/bin/env python3
"""Run the Micro-OS scheduling workloads on Linux and compare the results.

The same task sets as the kernel's `tc2` and `sched_cmp` scenarios are run
as real Linux processes, all pinned to ONE CPU (the Micro-OS has one hart),
under four Linux scheduling configurations:

    eevdf       SCHED_OTHER, nice 0 for everyone (Linux's default scheduler;
                kernel 6.6+ uses EEVDF, the successor of CFS)
    eevdf_nice  SCHED_OTHER with nice values derived from Micro-OS priorities
    rr          SCHED_RR, one real-time priority for everyone
    rr_prio     SCHED_RR with real-time priorities derived from Micro-OS
                priorities (strict priority, like the Micro-OS PRIO policy)

Per task we measure the same things the kernel reports: turnaround,
response, CPU time, time spent waiting on the run queue (/proc/self/schedstat),
number of times scheduled, and context switches. Each configuration is
repeated several times because Linux timing is not deterministic.

    sudo python3 linux/compare.py            # needs root for SCHED_RR / negative nice
    python3 linux/compare.py --reps 10

Writes results/linux_tasks.csv, results/linux_summary.csv,
results/linux_comparison.md and results/linux_vs_microos_*.png.
"""
import argparse
import csv
import multiprocessing as mp
import os
import platform
import statistics
import time
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RESULTS = ROOT / "results"

# (name, Micro-OS priority 0=high..7=low, work). Work is ("cpu", ms) or
# ("int", rounds, cpu_ms, sleep_ms). One Micro-OS tick = 1 ms.
WORKLOADS = {
    "tc2": [
        ("T1", 4, ("cpu", 12)), ("T2", 4, ("cpu", 24)), ("T3", 4, ("cpu", 8)),
        ("T4", 4, ("cpu", 30)), ("T5", 4, ("cpu", 16)),
    ],
    "cmp": [
        ("CPU1", 3, ("cpu", 30)), ("CPU2", 3, ("cpu", 40)), ("CPU3", 3, ("cpu", 50)),
        ("INT1", 1, ("int", 10, 1, 3)), ("INT2", 1, ("int", 10, 1, 3)),
        ("BATCH", 7, ("cpu", 20)),
    ],
}
POLICIES = ["eevdf", "eevdf_nice", "rr", "rr_prio"]

# Which Micro-OS runs (labels in traces/all.log) each workload is compared with.
MICROOS_RUNS = {
    "tc2": ["tc2"],
    "cmp": ["cmp_rr", "cmp_prio", "cmp_aging", "cmp_mlfq"],
}


def nice_for(prio):
    """Micro-OS 0..7 (4 = default) -> Linux nice; 3 nice steps per level."""
    return (prio - 4) * 3


def rt_prio_for(prio):
    """Micro-OS 0..7 -> SCHED_RR priority (higher number = more important on Linux)."""
    return 20 - prio


def read_schedstat():
    run_ns, wait_ns, slices = (int(x) for x in Path("/proc/self/schedstat").read_text().split())
    return run_ns, wait_ns, slices


def read_ctxsw():
    vol = invol = 0
    for line in Path("/proc/self/status").read_text().splitlines():
        if line.startswith("voluntary_ctxt_switches"):
            vol = int(line.split()[1])
        elif line.startswith("nonvoluntary_ctxt_switches"):
            invol = int(line.split()[1])
    return vol, invol


def burn(ms):
    """Spin until this process has received `ms` of CPU time."""
    start = time.process_time_ns()
    target = ms * 1_000_000
    while time.process_time_ns() - start < target:
        pass


def worker(cpu, policy, spec, ready, go, out):
    name, prio, work = spec
    os.sched_setaffinity(0, {cpu})
    if policy == "eevdf_nice":
        os.setpriority(os.PRIO_PROCESS, 0, nice_for(prio))
    elif policy == "rr":
        os.sched_setscheduler(0, os.SCHED_RR, os.sched_param(10))
    elif policy == "rr_prio":
        os.sched_setscheduler(0, os.SCHED_RR, os.sched_param(rt_prio_for(prio)))
    base_stat, base_sw = read_schedstat(), read_ctxsw()
    ready.release()
    go.wait()  # blocked (not runnable) until the parent releases everyone
    first = time.monotonic_ns()
    if work[0] == "cpu":
        burn(work[1])
    else:
        _, rounds, cpu_ms, sleep_ms = work
        for _ in range(rounds):
            burn(cpu_ms)
            time.sleep(sleep_ms / 1000)
    finish = time.monotonic_ns()
    stat, sw = read_schedstat(), read_ctxsw()
    out.put({
        "name": name, "prio": prio, "first_ns": first, "finish_ns": finish,
        "run_ns": stat[0] - base_stat[0], "wait_ns": stat[1] - base_stat[1], "slices": stat[2] - base_stat[2],
        "vol_cs": sw[0] - base_sw[0], "invol_cs": sw[1] - base_sw[1],
    })


def run_once(workload, policy, cpu):
    specs = WORKLOADS[workload]
    ready, go, out = mp.Semaphore(0), mp.Event(), mp.Queue()
    procs = [mp.Process(target=worker, args=(cpu, policy, s, ready, go, out)) for s in specs]
    for p in procs:
        p.start()
    for _ in procs:
        ready.acquire()
    time.sleep(0.05)  # let every worker block on `go`
    release = time.monotonic_ns()
    go.set()
    rows = [out.get(timeout=30) for _ in procs]
    for p in procs:
        p.join()
    rows.sort(key=lambda r: r["finish_ns"])
    for order, r in enumerate(rows):
        r["turnaround_us"] = (r["finish_ns"] - release) // 1000
        r["response_us"] = (r["first_ns"] - release) // 1000
        r["cpu_us"] = r["run_ns"] // 1000
        r["runq_wait_us"] = r["wait_ns"] // 1000
        r["latency_us"] = r["wait_ns"] // max(r["slices"], 1) // 1000
        r["finish_rank"] = order + 1
    return rows


def jain(values):
    values = [v for v in values if v > 0]
    return sum(values) ** 2 / (len(values) * sum(v * v for v in values)) if values else float("nan")


def summarise(rows):
    """Aggregate metrics of one run (same definitions as analysis/analyze.py)."""
    by = {r["name"]: r for r in rows}
    elapsed = max(r["turnaround_us"] for r in rows)
    ints = [by[n]["latency_us"] for n in by if n.startswith("INT")]
    return {
        "avg_turnaround_us": statistics.mean(r["turnaround_us"] for r in rows),
        "avg_response_us": statistics.mean(r["response_us"] for r in rows),
        "avg_wait_us": statistics.mean(r["turnaround_us"] - r["cpu_us"] for r in rows),
        "context_switches": sum(r["vol_cs"] + r["invol_cs"] for r in rows),
        "throughput_tps": len(rows) / (elapsed / 1e6),
        "jain": jain([r["cpu_us"] / r["turnaround_us"] for r in rows if r["turnaround_us"] > 0]),
        "int_latency_us": statistics.mean(ints) if ints else None,
        "batch_response_us": by["BATCH"]["response_us"] if "BATCH" in by else None,
        "order": " ".join(r["name"] for r in sorted(rows, key=lambda r: r["finish_rank"])),
    }


def microos_reference():
    """Per-run metrics from the kernel's trace (traces/all.log), via analyze.py's parser."""
    import sys
    sys.path.insert(0, str(ROOT / "analysis"))
    import analyze  # noqa: E402

    log = ROOT / "traces/all.log"
    if not log.exists():
        return {}
    runs, _ = analyze.parse(log)
    ref = {}
    for run in runs:
        rows = analyze.task_rows(run)
        if not rows:
            continue
        by = {r["name"]: r for r in rows}
        end = run.get("end", {})
        ints = [by[n]["latency_us"] for n in by if n.startswith("INT")]
        ref[run["label"]] = {
            "policy": run["policy"],
            "avg_turnaround_us": statistics.mean(r["turnaround_us"] for r in rows),
            "avg_response_us": statistics.mean(r["response_us"] for r in rows),
            "avg_wait_us": statistics.mean(r["wait_us"] for r in rows),
            "context_switches": end.get("dispatches", 0),
            "throughput_tps": len(rows) / (end.get("elapsed_us", 1) / 1e6),
            "jain": jain([r["cpu_ticks"] * 1000 / r["turnaround_us"] for r in rows if r["turnaround_us"] > 0]),
            "int_latency_us": statistics.mean(ints) if ints else None,
            "batch_response_us": by["BATCH"]["response_us"] if "BATCH" in by else None,
            "order": " ".join(r["name"] for r in rows),
            "tasks": {r["name"]: r for r in rows},
        }
    return ref


def fmt_ms(us, sd=None):
    if us is None:
        return "–"
    return f"{us / 1000:.1f}" + (f" ± {sd / 1000:.1f}" if sd is not None else "")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--reps", type=int, default=5)
    ap.add_argument("--cpu", type=int, default=os.cpu_count() - 1, help="CPU to pin every task to")
    a = ap.parse_args()
    if os.geteuid() != 0:
        raise SystemExit("run as root: SCHED_RR and negative nice values need it (sudo python3 linux/compare.py)")
    RESULTS.mkdir(exist_ok=True)
    # The parent stays off the measured CPU so it never competes with the tasks.
    os.sched_setaffinity(0, set(range(os.cpu_count())) - {a.cpu} or {a.cpu})

    env = {
        "kernel": platform.release(),
        "cpus": os.cpu_count(),
        "rr_timeslice_ms": Path("/proc/sys/kernel/sched_rr_timeslice_ms").read_text().strip(),
    }
    base_slice = Path("/sys/kernel/debug/sched/base_slice_ns")
    env["eevdf_base_slice_ms"] = f"{int(base_slice.read_text()) / 1e6:g}" if base_slice.exists() else "?"
    print(f"Linux {env['kernel']}, {env['cpus']} CPUs, tasks pinned to CPU {a.cpu}; "
          f"SCHED_RR slice {env['rr_timeslice_ms']} ms, EEVDF base slice {env['eevdf_base_slice_ms']} ms")

    task_rows, summaries = [], defaultdict(list)
    for workload in WORKLOADS:
        for policy in POLICIES:
            for rep in range(1, a.reps + 1):
                rows = run_once(workload, policy, a.cpu)
                for r in rows:
                    task_rows.append({"workload": workload, "policy": policy, "rep": rep,
                                      **{k: r[k] for k in ("name", "prio", "turnaround_us", "response_us", "cpu_us",
                                                           "runq_wait_us", "latency_us", "slices", "vol_cs",
                                                           "invol_cs", "finish_rank")}})
                summaries[(workload, policy)].append(summarise(rows))
            s = summaries[(workload, policy)]
            print(f"  {workload:<4} {policy:<11} avg turnaround {fmt_ms(statistics.mean(x['avg_turnaround_us'] for x in s))} ms, "
                  f"orders seen: {len(set(x['order'] for x in s))}")

    with open(RESULTS / "linux_tasks.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(task_rows[0]))
        w.writeheader()
        w.writerows(task_rows)

    metrics = ["avg_turnaround_us", "avg_response_us", "avg_wait_us", "context_switches",
               "throughput_tps", "jain", "int_latency_us", "batch_response_us"]
    summary_rows = []
    for (workload, policy), runs in summaries.items():
        row = {"system": "linux", "workload": workload, "policy": policy, "reps": len(runs)}
        for m in metrics:
            vals = [r[m] for r in runs if r[m] is not None]
            row[m] = round(statistics.mean(vals), 1) if vals else ""
            row[m + "_sd"] = round(statistics.stdev(vals), 1) if len(vals) > 1 else 0
        row["distinct_orders"] = len({r["order"] for r in runs})
        row["order"] = max({r["order"] for r in runs}, key=[r["order"] for r in runs].count)
        summary_rows.append(row)
    ref = microos_reference()
    for workload, labels in MICROOS_RUNS.items():
        for label in labels:
            if label not in ref:
                continue
            m = ref[label]
            row = {"system": "micro-os", "workload": workload, "policy": label, "reps": 1}
            for k in metrics:
                row[k] = round(m[k], 1) if m[k] is not None else ""
                row[k + "_sd"] = 0
            row["distinct_orders"] = 1
            row["order"] = m["order"]
            summary_rows.append(row)
    with open(RESULTS / "linux_summary.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(summary_rows[0]))
        w.writeheader()
        w.writerows(summary_rows)

    write_markdown(env, a, summary_rows)
    plots(summary_rows, task_rows, ref)
    print(f"wrote {RESULTS}/linux_summary.csv, linux_tasks.csv, linux_comparison.md and plots")


def write_markdown(env, a, rows):
    L = ["# Linux vs Micro-OS: measured results", "",
         f"Linux {env['kernel']} (WSL2), all tasks pinned to CPU {a.cpu}; {a.reps} repetitions per Linux "
         f"configuration (mean ± standard deviation). SCHED_RR time slice {env['rr_timeslice_ms']} ms; "
         f"EEVDF base slice {env['eevdf_base_slice_ms']} ms. Micro-OS: one deterministic run "
         "(QEMU instruction counting), 4 ms quantum.", ""]
    for workload in WORKLOADS:
        L += [f"## Workload `{workload}`", "",
              "| system | policy | avg turnaround (ms) | avg response (ms) | avg wait (ms) | context switches | "
              "interactive latency (ms) | BATCH response (ms) | Jain | completion order | orders seen |",
              "|---|---|---:|---:|---:|---:|---:|---:|---:|---|---:|"]
        for r in rows:
            if r["workload"] != workload:
                continue
            sd = r["system"] == "linux"
            L.append(
                f"| {r['system']} | {r['policy']} | {fmt_ms(r['avg_turnaround_us'], r['avg_turnaround_us_sd'] if sd else None)} | "
                f"{fmt_ms(r['avg_response_us'], r['avg_response_us_sd'] if sd else None)} | "
                f"{fmt_ms(r['avg_wait_us'], r['avg_wait_us_sd'] if sd else None)} | {r['context_switches']} | "
                f"{fmt_ms(r['int_latency_us'] or None)} | {fmt_ms(r['batch_response_us'] or None)} | "
                f"{r['jain']} | {r['order']} | {r['distinct_orders']} |")
        L.append("")
    L += ["Context switches: Linux counts voluntary + involuntary switches of the measured processes; "
          "Micro-OS counts dispatches.", ""]
    (RESULTS / "linux_comparison.md").write_text("\n".join(L) + "\n")


def plots(rows, task_rows, ref):
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError:
        print("matplotlib not installed: skipping plots")
        return
    colors = {"micro-os": "#7c4dff", "eevdf": "#2a9d8f", "eevdf_nice": "#8ab17d", "rr": "#e76f51", "rr_prio": "#f4a261"}

    # 1. tc2 per-task turnaround: Micro-OS RR vs each Linux policy (mean ± sd)
    names = [n for n, _, _ in WORKLOADS["tc2"]]
    series = []
    if "tc2" in ref:
        series.append(("Micro-OS RR (q=4 ms)", [ref["tc2"]["tasks"][n]["turnaround_us"] / 1000 for n in names], None, colors["micro-os"]))
    for p in POLICIES:
        per = defaultdict(list)
        for t in task_rows:
            if t["workload"] == "tc2" and t["policy"] == p:
                per[t["name"]].append(t["turnaround_us"] / 1000)
        series.append((f"Linux {p}", [statistics.mean(per[n]) for n in names],
                       [statistics.stdev(per[n]) if len(per[n]) > 1 else 0 for n in names], colors[p]))
    fig, ax = plt.subplots(figsize=(10, 4.6))
    width = 0.8 / len(series)
    for i, (label, vals, err, c) in enumerate(series):
        xs = [j + i * width for j in range(len(names))]
        ax.bar(xs, vals, width, yerr=err, capsize=2, label=label, color=c)
    bursts = {n: w[1] for n, _, w in WORKLOADS["tc2"]}
    ax.set_xticks([j + 0.4 - width / 2 for j in range(len(names))], [f"{n}\n({bursts[n]} ms CPU)" for n in names])
    ax.set_ylabel("turnaround (ms)")
    ax.set_title("TC2 workload on one CPU: Micro-OS Round Robin vs Linux")
    ax.legend(fontsize=8)
    fig.tight_layout()
    fig.savefig(RESULTS / "linux_vs_microos_tc2.png", dpi=120)
    plt.close(fig)

    # 2. cmp workload: key metrics for all 8 configurations
    cmp_rows = [r for r in rows if r["workload"] == "cmp"]
    metrics = [("avg_turnaround_us", "avg turnaround"), ("avg_response_us", "avg response"),
               ("int_latency_us", "interactive latency"), ("batch_response_us", "BATCH response")]
    fig, axes = plt.subplots(1, len(metrics), figsize=(14, 4.4))
    for ax, (key, title) in zip(axes, metrics):
        labels = [("MOS " if r["system"] == "micro-os" else "Linux ") + r["policy"].replace("cmp_", "") for r in cmp_rows]
        vals = [(r[key] or 0) / 1000 for r in cmp_rows]
        errs = [(r.get(key + "_sd") or 0) / 1000 for r in cmp_rows]
        cs = [colors["micro-os"] if r["system"] == "micro-os" else colors[r["policy"]] for r in cmp_rows]
        ax.bar(range(len(vals)), vals, yerr=errs, capsize=2, color=cs)
        ax.set_xticks(range(len(vals)), labels, rotation=55, ha="right", fontsize=8)
        ax.set_title(title, fontsize=10)
        ax.set_ylabel("ms")
    fig.suptitle("Mixed workload (3 CPU-bound, 2 interactive, 1 low-priority batch): Micro-OS policies vs Linux")
    fig.tight_layout()
    fig.savefig(RESULTS / "linux_vs_microos_cmp.png", dpi=120)
    plt.close(fig)

    # 3. repeatability: spread of avg turnaround across repetitions
    fig, ax = plt.subplots(figsize=(8, 4))
    labels, data = [], []
    for workload in WORKLOADS:
        for p in POLICIES:
            per_rep = defaultdict(list)
            for t in task_rows:
                if t["workload"] == workload and t["policy"] == p:
                    per_rep[t["rep"]].append(t["turnaround_us"] / 1000)
            labels.append(f"{workload}\n{p}")
            data.append([statistics.mean(v) for v in per_rep.values()])
    ax.boxplot(data, tick_labels=labels) if "tick_labels" in plt.boxplot.__code__.co_varnames else ax.boxplot(data, labels=labels)
    ax.tick_params(axis="x", labelsize=7)
    ax.set_ylabel("avg turnaround per repetition (ms)")
    ax.set_title("Linux run-to-run variation (Micro-OS: identical every run)")
    fig.tight_layout()
    fig.savefig(RESULTS / "linux_repeatability.png", dpi=120)
    plt.close(fig)


if __name__ == "__main__":
    main()
