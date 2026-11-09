#!/usr/bin/env python3
"""Paper-style tables and figures for the R3 study, measured on the Micro-OS.

    python3 analysis/research.py traces/research.log -o results

Reads the kernel's `#T ... R3RUN` (one per run/seed) and `#T ... VERDICT`
events, and writes:

    results/r3_runs.csv                    every kernel run
    results/r3_exp1.csv, r3_exp2.csv       means and 95% CIs per configuration
    results/r3_fig1..4_*.png               the study's four figures, from kernel data
    results/r3_kernel_vs_simulator.png     kernel vs the study's simulator
    results/research_sync.md               tables + hypothesis verdicts

The simulator's numbers (SIM_*) are copied from the R3 report (Tables 1-4).
"""
import argparse
import csv
import math
import re
import statistics
from collections import defaultdict
from pathlib import Path

TRACE = re.compile(r"#T (\d+) (\d+) (\w+)\s*(.*)$")
KV = re.compile(r"(\w+)=(\S+)")
POLICIES = ["FCFS", "SJF", "SRTF", "HRRN", "RR2", "RR8"]
LABEL = {"RR2": "RR (q=2)", "RR8": "RR (q=8)"}

# R3 report, simulator results (30 runs, 5000 ms window, 0.2 ms switch cost).
SIM_T1_BIMODAL = {  # throughput, util %, cs %, mean wait, wait var, jain
    "FCFS": (6.29, 95.2, 1.26, 392.5, 61272, 0.12), "SJF": (6.94, 95.0, 1.39, 102.1, 160433, 0.28),
    "SRTF": (6.96, 94.4, 2.25, 89.5, 180899, 0.23), "HRRN": (6.85, 95.1, 1.37, 140.7, 53001, 0.26),
    "RR2": (6.29, 88.0, 9.17, 390.0, 395105, 0.99), "RR8": (6.48, 93.7, 3.01, 297.2, 177951, 0.93),
}
SIM_T2_UNIFORM = {
    "FCFS": (6.05, 97.3, 1.21, 366.4, 44449, 0.12), "SJF": (6.42, 97.2, 1.28, 213.2, 463618, 0.12),
    "SRTF": (6.41, 96.9, 1.65, 216.2, 486898, 0.13), "HRRN": (6.23, 97.2, 1.25, 290.8, 58042, 0.15),
    "RR2": (4.95, 89.5, 9.34, 919.3, 415345, 0.96), "RR8": (5.63, 95.7, 2.92, 571.7, 162104, 0.78),
}
SIM_T3_BIMODAL = {  # throughput at 0.7/0.9/1.1, Jain at 0.7/0.9/1.1
    "FCFS": ((4.49, 5.65, 6.29), (0.89, 0.42, 0.12)), "SJF": ((4.51, 5.77, 6.94), (0.91, 0.54, 0.28)),
    "SRTF": ((4.53, 5.79, 6.96), (0.87, 0.51, 0.23)), "HRRN": ((4.51, 5.75, 6.85), (0.92, 0.57, 0.26)),
    "RR2": ((4.49, 5.61, 6.29), (1.00, 1.00, 0.99)), "RR8": ((4.51, 5.68, 6.48), (1.00, 0.99, 0.93)),
}
SIM_T4_SWEEP = {  # q: (throughput, util %, cs %, jain)
    0.5: (5.60, 70.0, 28.0, 1.00), 1: (6.03, 81.2, 16.5, 1.00), 2: (6.29, 88.0, 9.2, 0.99), 4: (6.42, 91.7, 5.2, 0.97),
    8: (6.48, 93.7, 3.0, 0.93), 16: (6.41, 94.5, 2.1, 0.80), 32: (6.31, 94.9, 1.6, 0.53), 64: (6.27, 95.1, 1.3, 0.21),
    128: (6.29, 95.2, 1.3, 0.12),
}
SIM_R = {"uniform": -0.94, "bimodal": -0.51}


def parse(path):
    runs, verdicts, params = [], {}, {}
    for line in open(path, encoding="utf-8", errors="replace"):
        m = TRACE.search(line.strip())
        if not m:
            continue
        kv = dict(KV.findall(m[4]))
        if m[3] == "R3RUN":
            r = {k: (int(v) if v.isdigit() else v) for k, v in kv.items()}
            r["exp"] = 2 if r["run"].startswith("r3_sweep") else 1
            r["label"] = r["run"].split("_")[3] if r["exp"] == 1 else "RR"
            runs.append(r)
        elif m[3] == "VERDICT":
            verdicts[kv["hypothesis"]] = kv["supported"] == "1"
    m = re.search(r"R3 reproduction: (\d+) seed\(s\), (\d+) ms window, (\d+) us switch cost", open(path, errors="replace").read())
    if m:
        params = {"seeds": int(m[1]), "window": int(m[2]), "cs_us": int(m[3])}
    return runs, verdicts, params


def ci95(xs):
    if len(xs) < 2:
        return 0.0
    return 1.96 * statistics.stdev(xs) / math.sqrt(len(xs))


def metrics(rows):
    """Mean and CI of the paper's metrics over seeds (paper units)."""
    def col(key, scale):
        # "na" = no task stayed for a whole fairness interval in that run
        return [r[key] / scale for r in rows if isinstance(r[key], int)]
    out = {}
    for name, key, scale in [("throughput", "thr_x100", 100), ("util", "util_bp", 100), ("cs", "cs_bp", 100),
                             ("mean_wait", "mean_wait_us", 1000), ("wait_var", "var_wait_ms2", 1),
                             ("max_wait", "max_wait_us", 1000), ("jain", "jain_bp", 10000), ("sjain", "sjain_bp", 10000)]:
        xs = col(key, scale)
        out[name] = statistics.mean(xs) if xs else float("nan")
        out[name + "_ci"] = ci95(xs)
    out["n"] = len(rows)
    return out


def pearson(xs, ys):
    if len(xs) < 3 or statistics.pstdev(xs) == 0 or statistics.pstdev(ys) == 0:
        return float("nan")
    mx, my = statistics.mean(xs), statistics.mean(ys)
    num = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    return num / math.sqrt(sum((x - mx) ** 2 for x in xs) * sum((y - my) ** 2 for y in ys))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("log", type=Path)
    ap.add_argument("-o", "--outdir", type=Path, default=Path("results"))
    a = ap.parse_args()
    a.outdir.mkdir(parents=True, exist_ok=True)
    runs, verdicts, params = parse(a.log)
    if not runs:
        raise SystemExit("no R3RUN events found: run scripts/research.sh first")

    with open(a.outdir / "r3_runs.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=sorted({k for r in runs for k in r}))
        w.writeheader()
        w.writerows(runs)

    exp1 = defaultdict(list)
    exp2 = defaultdict(list)
    for r in runs:
        if r["exp"] == 1:
            exp1[(r["dist"], r["load"], r["label"])].append(r)
        else:
            exp2[r["q"]].append(r)
    e1 = {k: metrics(v) for k, v in exp1.items()}
    e2 = {k: metrics(v) for k, v in sorted(exp2.items())}

    for name, table, keys in [("r3_exp1.csv", e1, ["dist", "load", "policy"]), ("r3_exp2.csv", e2, ["q"])]:
        with open(a.outdir / name, "w", newline="") as f:
            fields = keys + list(next(iter(table.values())).keys())
            w = csv.DictWriter(f, fieldnames=fields)
            w.writeheader()
            for k, m in table.items():
                kk = k if isinstance(k, tuple) else (k,)
                w.writerow({**dict(zip(keys, kk)), **{x: round(y, 4) if isinstance(y, float) else y for x, y in m.items()}})

    loads = sorted({k[1] for k in e1})
    corr = {}
    for dist in ("uniform", "bimodal"):
        pts = [(e1[(dist, 110, p)]["throughput"], e1[(dist, 110, p)]["jain"]) for p in POLICIES if (dist, 110, p) in e1]
        corr[dist] = pearson([x for x, _ in pts], [y for _, y in pts])

    figures = plots(a.outdir, e1, e2, loads)
    write_markdown(a.outdir, e1, e2, loads, corr, verdicts, params, figures)
    print(f"{len(runs)} kernel runs; verdicts {verdicts}; wrote {a.outdir}/research_sync.md and {len(figures)} figures")


def fmt(m, key, digits=2, ci=True):
    v = m[key]
    s = f"{v:,.{digits}f}"
    return s + (f" ± {m[key + '_ci']:.{digits}f}" if ci and m.get(key + "_ci") else "")


def write_markdown(outdir, e1, e2, loads, corr, verdicts, params, figures):
    p = params or {"seeds": "?", "window": "?", "cs_us": "?"}
    L = ["# R3 on the Micro-OS kernel: measured results", "",
         f"Kernel runs: {p['seeds']} seed(s) per configuration, {p['window']} ms arrival window, "
         f"{p['cs_us']} µs modelled context-switch cost, Jain's index over 250 ms intervals. "
         "Mean ± 95% CI (1.96·s/√n) where n > 1. Simulator values are from the R3 report "
         "(30 seeds, 5000 ms window).", ""]
    L += ["## Hypotheses", "", "| hypothesis | simulator (R3 report) | Micro-OS kernel |", "|---|---|---|"]
    for h, text in [("H1", "overload: SJF/SRTF highest throughput, Jain < 0.5; RR fair (≈1)"),
                    ("H2", "underload: all policies complete about the same number of jobs"),
                    ("H3", "RR: tiny q loses throughput to switching, large q loses fairness")]:
        k = verdicts.get(h)
        L.append(f"| {h}: {text} | supported | {'supported' if k else 'not supported' if k is not None else 'not evaluated'} |")
    L.append("")

    for title, dist, sim in [("Table 1: bimodal workload, ρ = 1.1", "bimodal", SIM_T1_BIMODAL),
                             ("Table 2: uniform workload, ρ = 1.1", "uniform", SIM_T2_UNIFORM)]:
        L += [f"## {title}", "",
              "| policy | throughput (jobs/100 ms) | CPU util. (%) | context switch (%) | mean wait (ms) | wait variance (ms²) | max wait (ms) | Jain's index | simulator: throughput / Jain |",
              "|---|---:|---:|---:|---:|---:|---:|---:|---:|"]
        for pol in POLICIES:
            m = e1.get((dist, 110, pol))
            if not m:
                continue
            s = sim[pol]
            L.append(f"| {LABEL.get(pol, pol)} | {fmt(m, 'throughput')} | {fmt(m, 'util', 1, False)} | {fmt(m, 'cs', 2, False)} | "
                     f"{fmt(m, 'mean_wait', 1, False)} | {m['wait_var']:,.0f} | {m['max_wait']:,.0f} | {fmt(m, 'jain')} | {s[0]:.2f} / {s[5]:.2f} |")
        L.append("")
    L += [f"Correlation between throughput and Jain's index across the six policies at ρ = 1.1: "
          f"uniform r = {corr['uniform']:.2f} (simulator −0.94), bimodal r = {corr['bimodal']:.2f} (simulator −0.51).", ""]

    L += ["## Table 3: effect of load (bimodal workload)", "",
          "| policy | " + " | ".join(f"thr ρ={l / 100:.1f}" for l in loads) + " | " + " | ".join(f"J ρ={l / 100:.1f}" for l in loads) + " | simulator J ρ=0.7/0.9/1.1 |",
          "|---|" + "---:|" * (2 * len(loads) + 1)]
    for pol in POLICIES:
        ms = [e1.get(("bimodal", l, pol)) for l in loads]
        if not all(ms):
            continue
        sj = SIM_T3_BIMODAL[pol][1]
        L.append(f"| {LABEL.get(pol, pol)} | " + " | ".join(f"{m['throughput']:.2f}" for m in ms) + " | "
                 + " | ".join(f"{m['jain']:.2f}" for m in ms) + f" | {sj[0]:.2f} / {sj[1]:.2f} / {sj[2]:.2f} |")
    L.append("")

    L += ["## Table 4: Round Robin quantum sweep (bimodal, ρ = 1.1)", "",
          "| quantum q (ms) | " + " | ".join(str(q) for q in e2) + " |",
          "|---|" + "---:|" * len(e2)]
    for name, key, d in [("throughput", "throughput", 2), ("CPU util. (%)", "util", 1), ("context sw. (%)", "cs", 1), ("Jain's index", "jain", 2)]:
        L.append(f"| {name} | " + " | ".join(f"{e2[q][key]:.{d}f}" for q in e2) + " |")
    L.append("| simulator throughput | " + " | ".join(f"{SIM_T4_SWEEP[q][0]:.2f}" if q in SIM_T4_SWEEP else "–" for q in e2) + " |")
    L.append("| simulator Jain | " + " | ".join(f"{SIM_T4_SWEEP[q][3]:.2f}" if q in SIM_T4_SWEEP else "–" for q in e2) + " |")
    L.append("")
    m2 = e1.get(("bimodal", 110, "SRTF")), e1.get(("bimodal", 110, "RR2"))
    if all(m2):
        L += [f"Slowdown-based fairness (bimodal, ρ = 1.1): SRTF {m2[0]['sjain']:.2f}, RR (q=2) {m2[1]['sjain']:.2f} "
              "(simulator: 0.89 and 0.31).", ""]
    if figures:
        L += ["## Figures", ""] + [f"![{f}]({f})" for f in figures]
    (outdir / "research_sync.md").write_text("\n".join(L) + "\n")


def plots(outdir, e1, e2, loads):
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError:
        return []
    made = []
    colors = {"FCFS": "#1f77b4", "SJF": "#ff7f0e", "SRTF": "#2ca02c", "HRRN": "#d62728", "RR2": "#9467bd", "RR8": "#8c564b"}
    markers = {"FCFS": "o", "SJF": "s", "SRTF": "^", "HRRN": "D", "RR2": "v", "RR8": "P"}

    fig, axes = plt.subplots(1, 2, figsize=(11, 4.4))
    for ax, dist in zip(axes, ("uniform", "bimodal")):
        for pol in POLICIES:
            m = e1.get((dist, 110, pol))
            if m:
                ax.errorbar(m["throughput"], m["jain"], xerr=m["throughput_ci"], yerr=m["jain_ci"], fmt=markers[pol],
                            color=colors[pol], capsize=3, label=LABEL.get(pol, pol))
                ax.annotate(LABEL.get(pol, pol), (m["throughput"], m["jain"]), textcoords="offset points", xytext=(5, 5), fontsize=8)
        ax.set_title(f"{dist} workload, load = 1.1 (Micro-OS kernel)")
        ax.set_xlabel("Throughput (jobs / 100 ms)")
        ax.set_ylabel("Jain's index (CPU allocation)")
        ax.set_ylim(0, 1.05)
        ax.grid(alpha=.3)
    fig.tight_layout()
    fig.savefig(outdir / "r3_fig1_throughput_vs_fairness.png", dpi=120)
    plt.close(fig)
    made.append("r3_fig1_throughput_vs_fairness.png")

    fig, axes = plt.subplots(1, 2, figsize=(11, 4.2))
    for ax, dist in zip(axes, ("uniform", "bimodal")):
        for pol in POLICIES:
            ys = [e1[(dist, l, pol)]["jain"] for l in loads if (dist, l, pol) in e1]
            if len(ys) == len(loads):
                ax.plot([l / 100 for l in loads], ys, marker=markers[pol], color=colors[pol], label=LABEL.get(pol, pol))
        ax.set_title(f"{dist} workload (Micro-OS kernel)")
        ax.set_xlabel("Offered load ρ")
        ax.set_ylabel("Jain's index (CPU allocation)")
        ax.set_ylim(0, 1.05)
        ax.grid(alpha=.3)
    axes[1].legend(fontsize=8)
    fig.tight_layout()
    fig.savefig(outdir / "r3_fig2_fairness_vs_load.png", dpi=120)
    plt.close(fig)
    made.append("r3_fig2_fairness_vs_load.png")

    fig, ax = plt.subplots(figsize=(9, 4))
    width = 0.8 / len(loads)
    for i, l in enumerate(loads):
        vals = [max(e1.get(("bimodal", l, p), {}).get("wait_var", 0), 1) for p in POLICIES]
        ax.bar([j + i * width for j in range(len(POLICIES))], vals, width, label=f"load {l / 100:.1f}")
    ax.set_yscale("log")
    ax.set_xticks([j + 0.4 - width / 2 for j in range(len(POLICIES))], [LABEL.get(p, p) for p in POLICIES])
    ax.set_ylabel("Waiting-time variance (ms², log)")
    ax.set_title("Waiting-time variance, bimodal workload (Micro-OS kernel)")
    ax.legend()
    fig.tight_layout()
    fig.savefig(outdir / "r3_fig3_wait_variance.png", dpi=120)
    plt.close(fig)
    made.append("r3_fig3_wait_variance.png")

    qs = list(e2)
    fig, ax1 = plt.subplots(figsize=(9, 4.4))
    ax1.errorbar(qs, [e2[q]["throughput"] for q in qs], yerr=[e2[q]["throughput_ci"] for q in qs], marker="o",
                 color="#1f77b4", capsize=3, label="kernel throughput")
    sim_q = [q for q in SIM_T4_SWEEP if q >= 1]
    ax1.plot(sim_q, [SIM_T4_SWEEP[q][0] for q in sim_q], ":", color="#1f77b4", alpha=.6, label="simulator throughput")
    ax1.set_xscale("log", base=2)
    ax1.set_xlabel("RR time quantum q (ms, log scale)")
    ax1.set_ylabel("Throughput (jobs / 100 ms)", color="#1f77b4")
    ax2 = ax1.twinx()
    ax2.plot(qs, [e2[q]["jain"] for q in qs], "s--", color="#d62728", label="kernel Jain")
    ax2.plot(sim_q, [SIM_T4_SWEEP[q][3] for q in sim_q], ":", color="#d62728", alpha=.6, label="simulator Jain")
    ax2.set_ylabel("Jain's index (CPU allocation)", color="#d62728")
    ax2.set_ylim(0, 1.05)
    lines = ax1.get_legend_handles_labels()[0] + ax2.get_legend_handles_labels()[0]
    ax1.legend(lines, [l.get_label() for l in lines], fontsize=8, loc="center right")
    ax1.set_title("RR quantum sweep (bimodal, load 1.1): Micro-OS kernel vs R3 simulator")
    fig.tight_layout()
    fig.savefig(outdir / "r3_fig4_quantum_sweep.png", dpi=120)
    plt.close(fig)
    made.append("r3_fig4_quantum_sweep.png")

    fig, axes = plt.subplots(1, 2, figsize=(11, 4))
    for ax, (key, idx, title) in zip(axes, [("throughput", 0, "Throughput (jobs / 100 ms)"), ("jain", 5, "Jain's index")]):
        xs = range(len(POLICIES))
        ax.bar([x - 0.2 for x in xs], [SIM_T1_BIMODAL[p][idx] for p in POLICIES], 0.4, label="R3 simulator", color="#b0b0b0")
        ax.bar([x + 0.2 for x in xs], [e1.get(("bimodal", 110, p), {}).get(key, 0) for p in POLICIES], 0.4,
               label="Micro-OS kernel", color="#7c4dff")
        ax.set_xticks(list(xs), [LABEL.get(p, p) for p in POLICIES], fontsize=8)
        ax.set_title(title + " — bimodal, load 1.1")
        ax.legend(fontsize=8)
    fig.tight_layout()
    fig.savefig(outdir / "r3_kernel_vs_simulator.png", dpi=120)
    plt.close(fig)
    made.append("r3_kernel_vs_simulator.png")
    return made


if __name__ == "__main__":
    main()
