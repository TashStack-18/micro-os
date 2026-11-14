#!/usr/bin/env python3
"""Record a real kernel session from the running desktop, for the GitHub Pages replay.

GitHub Pages can only serve static files, so it cannot run QEMU. This script
captures a real session instead: it boots the kernel through the running
panel (scripts/frontend.sh), records every Server-Sent Event the desktop
receives (boot trace, terminal output, snapshots, scheduler flow, heap map,
system calls, journal) with its timing, and saves the read-only API answers
and project files. frontend/replay.js plays it back in the browser.

    scripts/frontend.sh &
    python3 scripts/record_replay.py [--seconds 75] [--out site/replay]
"""
import argparse
import json
import threading
import time
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Commands typed into the kernel shell during the recording, with the time
# (seconds after the shell is ready) at which each is sent.
SCRIPT = [
    (2, "help"),
    (5, "spin 40 &"),
    (6, "pingpong 300 120 &"),
    (7, "primes 4000000 &"),
    (10, "ps"),
    (14, "sched"),
    (18, "user hello 2"),
    (23, "user priv"),
    (27, "user badptr"),
    (31, "user unknown"),
    (35, "mem"),
    (39, "sysinfo"),
    (44, "user spin 20 &"),
    (48, "ps"),
    (55, "fib 60"),
    (60, "uptime"),
]

# Folders the Files app may browse in the replay, and text files it may open.
BROWSE = ["", "kernel", "kernel/src", "kernel/src/programs", "kernel/src/user", "kernel/src/scheduler",
          "kernel/src/trap", "kernel/src/task", "kernel/src/sync", "kernel/src/memory", "kernel/src/boot",
          "kernel/src/timer", "kernel/src/shell", "kernel/src/scenarios", "core", "core/src", "core/src/sched",
          "core/src/mem", "docs", "results", "scripts", "analysis", "linux", "distro", "frontend"]
TEXT_EXT = {".rs", ".S", ".md", ".py", ".sh", ".toml", ".ld", ".csv", ".txt"}
MAX_FILE = 200_000


def get(url, path):
    with urllib.request.urlopen(url + path, timeout=20) as r:
        return json.loads(r.read())


def post(url, path, body):
    data = body if isinstance(body, bytes) else json.dumps(body).encode()
    urllib.request.urlopen(urllib.request.Request(url + path, data=data, method="POST"), timeout=20).read()


def record_events(url, seconds, events, ready):
    t0 = time.time()
    with urllib.request.urlopen(url + "/api/events", timeout=seconds + 30) as r:
        for raw in r:
            line = raw.decode("utf-8", "replace").rstrip("\n")
            if not line.startswith("data: "):
                continue
            msg = json.loads(line[6:])
            events.append([round((time.time() - t0) * 1000), msg])
            if msg.get("type") == "snap":
                ready.set()
            if time.time() - t0 > seconds:
                break


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--url", default="http://localhost:8080")
    ap.add_argument("--seconds", type=int, default=75)
    ap.add_argument("--out", default=str(ROOT / "site/replay"))
    a = ap.parse_args()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)

    post(a.url, "/api/boot", {"policy": "rr", "quantum": 4})
    time.sleep(0.3)
    events, ready = [], threading.Event()
    th = threading.Thread(target=record_events, args=(a.url, a.seconds, events, ready), daemon=True)
    th.start()
    ready.wait(30)
    time.sleep(1.5)
    start = time.time()
    for at, cmd in SCRIPT:
        time.sleep(max(0, at - (time.time() - start)))
        post(a.url, "/api/input", (cmd + "\r").encode())
        print(f"  t={at:>2}s  {cmd}")
    th.join(a.seconds + 30)

    # The server replays history to a new subscriber first; drop anything
    # before this boot's reset so the replay starts from power-on.
    first = next((i for i, (_, m) in enumerate(events) if m.get("type") == "reset"), 0)
    events = events[first:]
    base = events[0][0] if events else 0
    events = [[t - base, m] for t, m in events]
    (out / "session.json").write_text(json.dumps({
        "recorded": time.strftime("%Y-%m-%d %H:%M UTC", time.gmtime()),
        "duration_ms": events[-1][0] if events else 0,
        "commands": [c for _, c in SCRIPT],
        "events": events,
    }, separators=(",", ":")))

    api = {name: get(a.url, f"/api/{name}") for name in ("info", "programs", "results", "research")}
    fs = {"ls": {}, "file": {}}
    for d in BROWSE:
        try:
            listing = get(a.url, "/api/ls?path=" + urllib.parse.quote(d))
        except Exception:
            continue
        fs["ls"][d] = listing
        for e in listing.get("entries", []):
            p = Path(e["path"])
            if not e["dir"] and p.suffix in TEXT_EXT and (e.get("size") or 0) <= MAX_FILE:
                try:
                    fs["file"][e["path"]] = get(a.url, "/api/file?path=" + urllib.parse.quote(e["path"]))
                except Exception:
                    pass
    (out / "api.json").write_text(json.dumps(api, separators=(",", ":")))
    (out / "fs.json").write_text(json.dumps(fs, separators=(",", ":")))
    kinds = {}
    for _, m in events:
        kinds[m["type"]] = kinds.get(m["type"], 0) + 1
    print(f"recorded {len(events)} events over {events[-1][0] / 1000:.1f} s: {kinds}")
    print(f"{len(fs['ls'])} folders, {len(fs['file'])} files -> {out}")


if __name__ == "__main__":
    main()
