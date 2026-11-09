#!/usr/bin/env python3
"""Turn a Micro-OS serial log into tables, plots and a Markdown summary.

    python3 analysis/analyze.py traces/all.log            # writes into results/
    python3 analysis/analyze.py traces/all.log -o out/

Only lines of the form `#T <time_us> <tick> <EVENT> key=value ...` are read;
see kernel/src/kernel/console.rs for the format.
"""
import argparse
import csv
import re
import statistics
from collections import defaultdict
from pathlib import Path

TRACE = re.compile(r"^#T (\d+) (\d+) (\w+)\s*(.*)$")
KV = re.compile(r"(\w+)=(\S+)")

GANTT_RUNS = ["tc1", "tc2", "coop_off", "coop_on", "cmp_rr", "cmp_prio", "cmp_aging", "cmp_mlfq", "prodcons"]


def num(v):
    try:
        return int(v, 0)
    except (TypeError, ValueError):
        return v


def parse(path):
    """Return (runs, events_by_type). Each run holds its own event list."""
    runs, by_type, current = [], defaultdict(list), None
    with open(path, encoding="utf-8", errors="replace") as f:
        for raw in f:
            m = TRACE.match(raw.strip())
            if not m:
                continue
            ev = {"ts": int(m[1]), "tick": int(m[2]), "ev": m[3]}
            ev.update({k: num(v) for k, v in KV.findall(m[4])})
            by_type[ev["ev"]].append(ev)
            if ev["ev"] == "RUN_BEGIN":
                current = {"label": ev["run"], "policy": ev["policy"], "q": ev["q"],
                           "preempt": ev["preempt"], "start": ev["ts"], "events": []}
                runs.append(current)
            elif current is not None:
                current["events"].append(ev)
                if ev["ev"] == "RUN_END":
                    current["end"] = ev
                    current = None
    return runs, by_type


def task_rows(run):
    rows = []
    for e in run["events"]:
        if e["ev"] != "EXIT":
            continue
        turnaround = e["finish"] - e["arrival"]
        rows.append({
            "run": run["label"], "policy": run["policy"], "q": run["q"],
            "tid": e["tid"], "name": e["name"], "prio": e["prio"],
            "turnaround_us": turnaround, "response_us": e["first"] - e["arrival"],
            "wait_us": e["wait"], "latency_us": e["wait"] // max(e["disp"], 1),
            "cpu_ticks": e["cpu"], "dispatches": e["disp"], "preemptions": e["pre"],
            "yields": e["yld"], "blocks": e["blk"], "stack_bytes": e["stack"],
        })
    return rows


def jain(values):
    """Jain's fairness index: 1.0 = perfectly equal, 1/n = one task gets everything."""
    values = [v for v in values if v > 0]
    if not values:
        return float("nan")
    return sum(values) ** 2 / (len(values) * sum(v * v for v in values))


def run_summary(run, rows, tick_us):
    end = run.get("end", {})
    # Progress rate = CPU time received / time spent in the system.
    rates = [r["cpu_ticks"] * tick_us / r["turnaround_us"] for r in rows if r["turnaround_us"] > 0]
    mean = lambda k: statistics.mean(r[k] for r in rows) if rows else 0
    return {
        "run": run["label"], "policy": run["policy"], "q": run["q"], "preempt": run["preempt"],
        "tasks": len(rows), "elapsed_us": end.get("elapsed_us", 0),
        "dispatches": end.get("dispatches", 0), "preemptions": end.get("preemptions", 0),
        "avg_turnaround_us": round(mean("turnaround_us")), "avg_response_us": round(mean("response_us")),
        "avg_wait_us": round(mean("wait_us")), "jain_fairness": round(jain(rates), 3) if rates else "",
        # Throughput: tasks completed per second of (virtual) run time.
        "throughput_tps": round(len(rows) / (end["elapsed_us"] / 1e6), 1) if end.get("elapsed_us") else "",
        "deadlock": end.get("deadlock", 0),
    }


def write_csv(path, rows):
    if not rows:
        return
    with open(path, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)


def segments(run):
    """(tid, name, start_us, end_us, why) for every time a task held the CPU."""
    open_, names, out = {}, {}, []
    for e in run["events"]:
        if e["ev"] == "DISPATCH":
            open_[e["tid"]] = e["ts"]
            names[e["tid"]] = e["name"]
        elif e["ev"] == "OFF" and e["tid"] in open_:
            out.append((e["tid"], names[e["tid"]], open_.pop(e["tid"]), e["ts"], e["why"]))
    return out


def plots(outdir, runs, by_type, summaries):
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError:
        print("matplotlib not installed: skipping plots")
        return []
    made = []
    colors = {"preempt": "#4878d0", "yield": "#6acc64", "block": "#ee854a", "exit": "#956cb4"}

    for run in runs:
        if run["label"] not in GANTT_RUNS:
            continue
        segs = segments(run)
        if not segs:
            continue
        tids = sorted({s[0] for s in segs})
        label_of = {s[0]: f"{s[1]} (tid {s[0]})" for s in segs}
        t0 = run["start"]
        fig, ax = plt.subplots(figsize=(11, 0.45 * len(tids) + 1.6))
        for tid, _, a, b, why in segs:
            ax.broken_barh([((a - t0) / 1000, max((b - a) / 1000, 0.05))], (tids.index(tid) - 0.4, 0.8),
                           color=colors.get(why, "grey"))
        ax.set_yticks(range(len(tids)), [label_of[t] for t in tids])
        ax.invert_yaxis()
        ax.set_xlabel("time since run start (ms)")
        ax.set_title(f"CPU timeline: {run['label']}  ({run['policy']}, q={run['q']}, preempt={run['preempt']})")
        handles = [plt.Rectangle((0, 0), 1, 1, color=c) for c in colors.values()]
        ax.legend(handles, [f"left by {k}" for k in colors], loc="upper right", fontsize=8)
        fig.tight_layout()
        name = f"gantt_{run['label']}.png"
        fig.savefig(outdir / name, dpi=120)
        plt.close(fig)
        made.append(name)

    sweep = by_type.get("SWEEP", [])
    if sweep:
        q = [e["q"] for e in sweep]
        fig, ax1 = plt.subplots(figsize=(8, 4.5))
        ax1.plot(q, [e["avg_turn"] / 1000 for e in sweep], "o-", label="avg turnaround (ms)")
        ax1.plot(q, [e["int_lat"] / 1000 for e in sweep], "s-", label="interactive latency (ms)")
        ax1.set_xscale("log", base=2)
        ax1.set_xlabel("RR quantum (ticks)")
        ax1.set_ylabel("ms")
        ax2 = ax1.twinx()
        ax2.plot(q, [e["dispatches"] for e in sweep], "^--", color="grey", label="dispatches")
        ax2.set_ylabel("dispatches (context switches)")
        lines = ax1.get_legend_handles_labels()[0] + ax2.get_legend_handles_labels()[0]
        ax1.legend(lines, [l.get_label() for l in lines], loc="upper center")
        ax1.set_title("Round Robin quantum sweep")
        fig.tight_layout()
        fig.savefig(outdir / "quantum_sweep.png", dpi=120)
        plt.close(fig)
        made.append("quantum_sweep.png")

    cmp_ = by_type.get("CMP", [])
    if cmp_:
        metrics = [("avg_turn", "avg turnaround"), ("avg_resp", "avg response"),
                   ("int_lat", "interactive latency"), ("batch_resp", "BATCH response")]
        fig, ax = plt.subplots(figsize=(9, 4.5))
        width = 0.8 / len(cmp_)
        for i, e in enumerate(cmp_):
            xs = [j + i * width for j in range(len(metrics))]
            ax.bar(xs, [e[k] / 1000 for k, _ in metrics], width, label=e["policy"])
        ax.set_xticks([j + 0.4 - width / 2 for j in range(len(metrics))], [m for _, m in metrics])
        ax.set_ylabel("ms (lower is better)")
        ax.set_title("Same workload, four scheduling policies")
        ax.legend()
        fig.tight_layout()
        fig.savefig(outdir / "policy_compare.png", dpi=120)
        plt.close(fig)
        made.append("policy_compare.png")

    locks = by_type.get("LOCK", [])
    if locks:
        fig, (a1, a2) = plt.subplots(1, 2, figsize=(10, 4))
        names = [e["variant"] for e in locks]
        a1.bar(names, [e["elapsed_us"] / 1000 for e in locks], color="#4878d0")
        a1.set_ylabel("run time (ms)")
        a1.set_title("Time to finish 4 x 1000 increments")
        a2.bar(names, [e["lost"] for e in locks], color="#d65f5f")
        a2.set_ylabel("lost updates")
        a2.set_title("Correctness")
        fig.tight_layout()
        fig.savefig(outdir / "locks.png", dpi=120)
        plt.close(fig)
        made.append("locks.png")

    mem = by_type.get("MEM", [])
    if mem:
        fig, axes = plt.subplots(1, 3, figsize=(12, 4))
        for ax, key, title in zip(axes, ["ok", "frag_avg_bp", "ns_per_op"],
                                  ["successful allocations", "avg fragmentation (%)", "ns per operation"]):
            labels = [f"{e['alloc']}\n{e['profile']}" for e in mem]
            vals = [e[key] / 100 if key == "frag_avg_bp" else e[key] for e in mem]
            ax.bar(labels, vals, color=["#4878d0", "#6acc64", "#ee854a"] * 2)
            ax.set_title(title)
            ax.tick_params(axis="x", labelsize=7)
        fig.tight_layout()
        fig.savefig(outdir / "allocators.png", dpi=120)
        plt.close(fig)
        made.append("allocators.png")
    return made


def markdown(outdir, summaries, by_type, made):
    checks = by_type.get("CHECK", [])
    passed = sum(1 for c in checks if c["ok"] == 1)
    lines = ["# Micro-OS results", "",
             f"**Checks:** {passed} passed, {len(checks) - passed} failed", ""]
    failed = [c["id"] for c in checks if c["ok"] != 1]
    if failed:
        lines += ["Failed: " + ", ".join(failed), ""]
    lines += ["## Scheduling runs", "",
              "| run | policy | q | tasks | dispatches | avg turnaround (ms) | avg response (ms) | avg wait (ms) | throughput (tasks/s) | Jain fairness |",
              "|---|---|---:|---:|---:|---:|---:|---:|---:|---:|"]
    for s in summaries:
        lines.append(f"| {s['run']} | {s['policy']} | {s['q']} | {s['tasks']} | {s['dispatches']} | "
                     f"{s['avg_turnaround_us'] / 1000:.1f} | {s['avg_response_us'] / 1000:.1f} | "
                     f"{s['avg_wait_us'] / 1000:.1f} | {s['throughput_tps']} | {s['jain_fairness']} |")
    if by_type.get("LOCK"):
        lines += ["", "## Synchronization (4 tasks x 1000 increments)", "",
                  "| variant | result | lost | time (ms) | contended | spins | mutex wait (ms) |",
                  "|---|---:|---:|---:|---:|---:|---:|"]
        for e in by_type["LOCK"]:
            lines.append(f"| {e['variant']} | {e['result']} | {e['lost']} | {e['elapsed_us'] / 1000:.1f} | "
                         f"{e['contended']} | {e['spins']} | {e['wait_us'] / 1000:.1f} |")
    if by_type.get("MEM"):
        lines += ["", "## Allocators (4000 random operations, 256 KiB region)", "",
                  "| allocator | profile | ok | failed | ns/op | peak (B) | avg frag | max frag |",
                  "|---|---|---:|---:|---:|---:|---:|---:|"]
        for e in by_type["MEM"]:
            lines.append(f"| {e['alloc']} | {e['profile']} | {e['ok']} | {e['failed']} | {e['ns_per_op']} | "
                         f"{e['peak']} | {e['frag_avg_bp'] / 100:.2f}% | {e['frag_max_bp'] / 100:.2f}% |")
    if made:
        lines += ["", "## Plots", ""] + [f"![{m}]({m})" for m in made]
    (outdir / "summary.md").write_text("\n".join(lines) + "\n")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("log", type=Path)
    ap.add_argument("-o", "--outdir", type=Path, default=Path("results"))
    a = ap.parse_args()
    a.outdir.mkdir(parents=True, exist_ok=True)

    runs, by_type = parse(a.log)
    tick_us = next((e["tick_us"] for e in by_type.get("BOOT", []) if "tick_us" in e), 1000)
    all_rows, summaries = [], []
    for run in runs:
        rows = task_rows(run)
        all_rows += rows
        summaries.append(run_summary(run, rows, tick_us))
    write_csv(a.outdir / "tasks.csv", all_rows)
    write_csv(a.outdir / "runs.csv", summaries)
    for kind in ["LOCK", "MEM", "SWEEP", "CMP"]:
        write_csv(a.outdir / f"{kind.lower()}.csv", [{k: v for k, v in e.items() if k != "ev"} for e in by_type.get(kind, [])])
    made = plots(a.outdir, runs, by_type, summaries)
    markdown(a.outdir, summaries, by_type, made)
    checks = by_type.get("CHECK", [])
    print(f"{len(runs)} runs, {len(all_rows)} task exits, {sum(c['ok'] == 1 for c in checks)}/{len(checks)} checks passed")
    print(f"wrote {a.outdir}/summary.md, CSV tables and {len(made)} plots")


if __name__ == "__main__":
    main()
