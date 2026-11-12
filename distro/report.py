#!/usr/bin/env python3
"""Boot-time report for Micro-OS Linux stages.

    python3 distro/report.py RAW_LOG_DIR IMAGE_DIR RESULTS_DIR

Reads the serial logs written by boottime.sh (systemd-analyze output from
inside the guest) and the .info files written by build.sh, and writes
RESULTS_DIR/distro_boottime.csv, .md and .png.
"""
import csv
import re
import statistics
import sys
from pathlib import Path

STEP = re.compile(r"([\d.]+)(min|ms|s)\b")
PHASE = re.compile(r"((?:\d+min )?[\d.]+(?:ms|s)) \((firmware|loader|kernel|initrd|userspace)\)")
TOTAL = re.compile(r"= ((?:\d+min )?[\d.]+(?:ms|s))")
REACHED = re.compile(r"(\S+\.target) reached after ((?:\d+min )?[\d.]+(?:ms|s)) in userspace")


def seconds(text):
    total = 0.0
    for value, unit in STEP.findall(text):
        total += float(value) * {"min": 60, "s": 1, "ms": 0.001}[unit]
    return total


def parse(log):
    text = log.read_text(errors="replace").replace("\r", "")
    line = next((l for l in text.splitlines() if l.startswith("Startup finished")), None)
    if not line:
        return None
    r = {k: 0.0 for k in ("firmware", "loader", "kernel", "initrd", "userspace")}
    for value, phase in PHASE.findall(line):
        r[phase] = seconds(value)
    r["total"] = seconds(TOTAL.search(line).group(1))
    m = REACHED.search(text)
    r["target"], r["target_s"] = (m.group(1), seconds(m.group(2))) if m else ("", 0.0)
    blame = text.split("--- slowest units", 1)[1].split("---", 1)[0].strip().splitlines() if "--- slowest units" in text else []
    r["slowest"] = blame[0].strip() if blame else ""
    return r


def info(path):
    d = {}
    if path.exists():
        for line in path.read_text().splitlines():
            if "=" in line:
                k, v = line.split("=", 1)
                d[k] = v
    return d


def main():
    raw, images, results = (Path(p) for p in sys.argv[1:4])
    results.mkdir(parents=True, exist_ok=True)
    rows, stages = [], []
    for s in range(7):
        logs = sorted(raw.glob(f"s{s}-boot*.log"), key=lambda p: int(p.stem.split("boot")[1]))
        if not logs:
            continue
        meta = info(images / f"microos-s{s}.info")
        boots = []
        for log in logs:
            i = int(log.stem.split("boot")[1])
            r = parse(log)
            if r:
                boots.append((i, r))
                rows.append({"stage": s, "name": meta.get("name", ""), "boot": i, "first_boot": int(i == 0),
                             **{k: round(v, 3) if isinstance(v, float) else v for k, v in r.items()}})
        steady = [r for i, r in boots if i > 0]
        first = next((r for i, r in boots if i == 0), None)
        if not steady:
            continue
        mean = lambda k: statistics.mean(r[k] for r in steady)
        sd = lambda k: statistics.stdev(r[k] for r in steady) if len(steady) > 1 else 0.0
        stages.append({"stage": s, "name": meta.get("name", f"s{s}"), "title": meta.get("title", ""),
                       "n": len(steady), "kernel": mean("kernel"), "userspace": mean("userspace"),
                       "total": mean("total"), "total_sd": sd("total"), "first": first["total"] if first else None,
                       "target": steady[0]["target"], "target_s": mean("target_s"),
                       "packages": meta.get("packages_installed", ""), "size_mb": meta.get("root_used_mb", ""),
                       "kernel_version": meta.get("kernel", ""), "slowest": steady[-1]["slowest"]})

    with open(results / "distro_boottime.csv", "w", newline="") as f:
        if rows:
            w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
            w.writeheader()
            w.writerows(rows)

    L = ["# Micro-OS Linux: boot time after each component", "",
         "Measured by `systemd-analyze` inside the guest. VM: KVM, 4 vCPUs, 4 GiB RAM, virtio disk and NIC, "
         "UEFI with Secure Boot (shim → GRUB → signed kernel). Mean ± standard deviation of the normal boots; "
         "*first boot* is a fresh image's first start-up (one-time setup). systemd cannot time the firmware "
         "and GRUB phases (that needs systemd-boot), so *total* = kernel (incl. initramfs) + userspace.", "",
         "| stage | component added | packages | root size | kernel (s) | userspace (s) | **total (s)** | change | first boot (s) | ready target (s) | slowest unit |",
         "|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|"]
    prev = None
    for st in stages:
        delta = "" if prev is None else f"{st['total'] - prev:+.2f}"
        first = f"{st['first']:.2f}" if st["first"] is not None else "–"
        L.append(f"| S{st['stage']} | {st['title'].split(' ', 1)[1] if st['title'] else st['name']} | {st['packages']} | "
                 f"{st['size_mb']} MiB | {st['kernel']:.2f} | {st['userspace']:.2f} | **{st['total']:.2f} ± {st['total_sd']:.2f}** | "
                 f"{delta} | {first} | {st['target']} {st['target_s']:.2f} | {st['slowest']} |")
        prev = st["total"]
    if stages:
        L += ["", f"Kernel: {stages[-1]['kernel_version']}. {stages[0]['n']} normal boots per stage."]
    (results / "distro_boottime.md").write_text("\n".join(L) + "\n")

    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError:
        return
    if not stages:
        return
    labels = [f"S{st['stage']}\n{st['name']}" for st in stages]
    xs = range(len(stages))
    fig, ax = plt.subplots(figsize=(10, 4.8))
    k = [st["kernel"] for st in stages]
    u = [st["userspace"] for st in stages]
    ax.bar(xs, k, color="#7c4dff", label="kernel + initramfs")
    ax.bar(xs, u, bottom=k, color="#c4b5fd", label="userspace (systemd)",
           yerr=[st["total_sd"] for st in stages], capsize=3)
    firsts = [st["first"] for st in stages]
    ax.scatter(xs, [f if f is not None else float("nan") for f in firsts], marker="D", color="#e76f51", zorder=3,
               label="first boot")
    for x, st in zip(xs, stages):
        ax.text(x, st["total"] + 0.15, f"{st['total']:.2f}s", ha="center", fontsize=9)
    ax.set_xticks(list(xs), labels, fontsize=8)
    ax.set_ylabel("boot time (s)")
    ax.set_title("Micro-OS Linux: boot time after adding each component (UEFI VM, KVM)")
    ax.legend(fontsize=8)
    fig.tight_layout()
    fig.savefig(results / "distro_boottime.png", dpi=120)
    plt.close(fig)


if __name__ == "__main__":
    main()
