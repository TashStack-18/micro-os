#!/usr/bin/env python3
"""Web front end for the Micro-OS (shown on screen as NEXUS OS).

Boots the kernel in QEMU (`test=shell monitor=1`) and bridges it to a
browser page:

* the serial console streams to an in-page terminal, and keystrokes go back;
* `#T` trace lines from the kernel feed the live views: `SNAP` (tasks,
  scheduler counters, heap totals), `HEAPMAP` (how full each slice of the heap
  is), and the scheduler's own `DISPATCH` / `OFF` / `WAKE` / `SPAWN` / `EXIT`
  events, summarised every snapshot into a "flow" message and a journal;
* the project's files on the host can be browsed (read-only), programs in
  kernel/src/programs can be edited, and the kernel rebuilt with cargo on the
  host, after which the VM reboots and runs the program;
* the test suite can be run and its results and plots browsed.

Run with `scripts/frontend.sh`, then open http://localhost:8080 .
Uses only the Python standard library.
"""
import argparse
import codecs
import collections
import json
import os
import queue
import re
import shutil
import subprocess
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

ROOT = Path(__file__).resolve().parent.parent
STATIC = Path(__file__).resolve().parent / "static"
ELF = ROOT / "kernel/target/riscv64gc-unknown-none-elf/release/microos"
PROGRAMS_DIR = ROOT / "kernel/src/programs"
TRACE = re.compile(r"#T (\d+) (\d+) (\w+)\s*(.*)")
KV = re.compile(r"(\w+)=(\S+)")
PROGRAM = re.compile(r'Program \{ name: "(\w+)", usage: "([^"]*)", about: "([^"]*)"')
SAFE_NAME = re.compile(r"^[\w.-]+$")
PROGRAM_NAME = re.compile(r"^[a-z][a-z0-9_]{1,23}$")
POLICIES = {"rr", "prio", "aging", "mlfq", "fcfs", "sjf", "srtf", "hrrn"}
POLICY_EVENTS = {"AGE", "BOOST", "DEMOTE"}
RAM_MB = 128
HISTORY_LIMIT = 200_000   # characters of console output replayed to new browsers
JOURNAL_LIMIT = 400       # journal entries replayed to new browsers
PATH_SAMPLE = 64          # dispatch order kept per flow window
# File browser: never list these, never read past this size.
HIDDEN = {".git", "target", "__pycache__", "node_modules", ".impeccable", "out", "build"}
TEXT_SUFFIXES = {".rs", ".md", ".toml", ".py", ".sh", ".txt", ".csv", ".ld", ".S", ".s", ".json",
                 ".html", ".css", ".js", ".cfg", ".yml", ".yaml", ".log", ".gitignore", ".service"}
MAX_TEXT = 400_000
CARGO = shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo")


class Hub:
    """Fan-out of messages to every connected browser (Server-Sent Events)."""

    def __init__(self):
        self.lock = threading.Lock()
        self.clients = []
        self.history = []
        self.history_len = 0
        self.status = {"type": "status", "running": False}
        self.last_snap = None
        self.last_heapmap = None
        self.boot = []  # boot stages of the current machine, replayed to new browsers
        self.journal = collections.deque(maxlen=JOURNAL_LIMIT)

    def subscribe(self):
        q = queue.Queue()
        with self.lock:
            self.clients.append(q)
            q.put(self.status)
            for b in self.boot:
                q.put(b)
            if self.journal:
                q.put({"type": "journal_replay", "entries": list(self.journal)})
            if self.history:
                q.put({"type": "term", "data": "".join(self.history)})
            if self.last_heapmap:
                q.put(self.last_heapmap)
            if self.last_snap:
                q.put(self.last_snap)
        return q

    def unsubscribe(self, q):
        with self.lock:
            if q in self.clients:
                self.clients.remove(q)

    def publish(self, msg):
        with self.lock:
            kind = msg["type"]
            if kind == "term":
                self.history.append(msg["data"])
                self.history_len += len(msg["data"])
                while self.history_len > HISTORY_LIMIT and len(self.history) > 1:
                    self.history_len -= len(self.history.pop(0))
            elif kind == "snap":
                self.last_snap = msg
            elif kind == "heapmap":
                self.last_heapmap = msg
            elif kind == "boot":
                self.boot.append(msg)
            elif kind == "journal":
                self.journal.append(msg)
            elif kind == "status":
                self.status = msg
            elif kind == "reset":
                self.history, self.history_len, self.last_snap, self.last_heapmap, self.boot = [], 0, None, None, []
            for q in self.clients:
                q.put(msg)

    def note(self, source, kind, text, t=None, **fields):
        """Add a journal entry. `source` is "kernel" or "panel"; kernel
        entries carry the kernel's own timestamp (µs since power-on)."""
        self.publish({"type": "journal", "source": source, "kind": kind, "text": text,
                      "t": t, "wall": time.time(), **fields})


def parse_snapshot(fields):
    tasks = []
    for item in fields.pop("tasks", "").split(";"):
        parts = item.split(":")
        if len(parts) == 7:
            tid, name, state, prio, level, cpu, disp = parts
            tasks.append({"tid": int(tid), "name": name, "state": state, "prio": int(prio),
                          "level": int(level), "cpu": int(cpu), "disp": int(disp)})
    snap = {k: (int(v) if v.lstrip("-").isdigit() else v) for k, v in fields.items()}
    snap.update(type="snap", tasks=tasks)
    return snap


class Flow:
    """Summary of the scheduler's own trace events between two snapshots:
    how often each task was dispatched and in what order, why tasks left the
    CPU, and who woke whom. Nothing here is estimated: every count is a
    trace line the kernel printed."""

    def __init__(self):
        self.reset()

    def reset(self):
        self.disp = collections.Counter()
        self.path = collections.deque(maxlen=PATH_SAMPLE)
        self.off = collections.Counter()
        self.wakes = collections.Counter()      # (by, tid) -> n, task wake-ups
        self.timer_wakes = collections.Counter()  # tid -> n, sleeps that ended
        self.waited = 0
        self.events = 0
        self.syscalls = collections.Counter()          # name -> n
        self.sys_by_tid = collections.Counter()        # tid -> n
        self.sys_recent = collections.deque(maxlen=24)  # [t, tid, nr, name, ret]

    def message(self, t):
        msg = {"type": "flow", "t": t, "events": self.events,
               "disp": {str(k): v for k, v in self.disp.items()},
               "path": list(self.path), "off": dict(self.off),
               "wakes": [[by, tid, n] for (by, tid), n in self.wakes.items()],
               "timer_wakes": {str(k): v for k, v in self.timer_wakes.items()},
               "waited": self.waited,
               "syscalls": dict(self.syscalls), "sys_by_tid": {str(k): v for k, v in self.sys_by_tid.items()},
               "sys_recent": list(self.sys_recent)}
        self.reset()
        return msg


class StreamSplitter:
    """Separates kernel trace lines (`#T ...`) from terminal text.

    The kernel writes each trace line whole, under the console lock, so the
    text from `#T ` to the next newline is always one event, even when it
    starts mid-line (e.g. right after the shell prompt).
    """

    def __init__(self, hub, on_ready=None):
        self.hub = hub
        self.buf = ""
        self.ready = False
        self.on_ready = on_ready
        self.flow = Flow()
        self.names = {}
        self.seen_edges = set()
        self.user_tids = set()

    def feed(self, text):
        self.buf += text
        out = []
        while True:
            i = self.buf.find("#T ")
            if i < 0:
                # Hold back a possible partial "#T" at the end of the buffer.
                keep = 2 if self.buf.endswith("#T") else 1 if self.buf.endswith("#") else 0
                out.append(self.buf[: len(self.buf) - keep])
                self.buf = self.buf[len(self.buf) - keep:]
                break
            nl = self.buf.find("\n", i)
            if nl < 0:
                out.append(self.buf[:i])
                self.buf = self.buf[i:]
                break
            out.append(self.buf[:i])
            self.event(self.buf[i:nl].rstrip("\r"))
            self.buf = self.buf[nl + 1:]
        text = "".join(out)
        if text:
            self.hub.publish({"type": "term", "data": text})

    def name(self, tid):
        return self.names.get(tid, f"pid {tid}")

    def event(self, line):
        m = TRACE.match(line)
        if not m:
            return
        kind = m[3]
        fields = dict(KV.findall(m[4]))
        if m[3] == "USER_KILL":
            # cause= can hold several words: take everything up to " sepc=".
            mc = re.search(r"cause=(.*?) sepc=", m[4])
            if mc:
                fields["cause"] = mc[1]
        t = int(m[1])
        flow = self.flow
        if kind == "DISPATCH":
            tid = int(fields.get("tid", 0))
            flow.disp[tid] += 1
            flow.path.append(tid)
            flow.waited += int(fields.get("waited", 0) or 0)
            flow.events += 1
        elif kind == "OFF":
            flow.off[fields.get("why", "?")] += 1
            flow.events += 1
        elif kind == "WAKE":
            tid, by = int(fields.get("tid", 0)), fields.get("by", "?")
            flow.events += 1
            if by == "timer":
                flow.timer_wakes[tid] += 1
            elif by.isdigit():
                flow.wakes[(int(by), tid)] += 1
                edge = (int(by), tid)
                if edge not in self.seen_edges:
                    self.seen_edges.add(edge)
                    self.hub.note("kernel", "wake", f"{self.name(int(by))} woke {self.name(tid)} for the first time",
                                  t, by=int(by), tid=tid)
        elif kind == "SPAWN":
            tid = int(fields.get("tid", 0))
            self.names[tid] = fields.get("name", "?")
            delay = fields.get("delay", "0")
            extra = f", starts after {delay} ticks" if delay not in ("0", None) else ""
            self.hub.note("kernel", "spawn", f"{self.names[tid]} spawned as pid {tid} (priority {fields.get('prio', '?')}{extra})",
                          t, tid=tid)
        elif kind == "EXIT":
            tid = int(fields.get("tid", 0))
            cpu_us = int(fields.get("cpu_us", 0) or 0)
            self.hub.note("kernel", "exit",
                          f"{fields.get('name', self.name(tid))} (pid {tid}) exited: {cpu_us / 1000:.1f} ms CPU, "
                          f"{fields.get('disp', '?')} dispatches, {fields.get('pre', '?')} preemptions, "
                          f"waited {int(fields.get('wait', 0) or 0) / 1000:.1f} ms", t, tid=tid)
        elif kind in POLICY_EVENTS:
            tid = int(fields.get("tid", 0) or 0)
            detail = " ".join(f"{k}={v}" for k, v in fields.items() if k != "tid")
            self.hub.note("kernel", "policy", f"{kind.lower()}: {self.name(tid)} (pid {tid}) {detail}".rstrip(), t, tid=tid)
        elif kind == "SYSCALL":
            tid, nr, ret = int(fields.get("tid", 0)), int(fields.get("nr", 0)), int(fields.get("ret", 0))
            name = fields.get("name", "?")
            flow.syscalls[name] += 1
            flow.sys_by_tid[tid] += 1
            flow.sys_recent.append([t, tid, nr, name, ret])
            flow.events += 1
        elif kind == "USER_ENTER":
            tid = int(fields.get("tid", 0))
            name = fields.get("name", self.name(tid))
            self.user_tids.add(tid)
            self.hub.publish({"type": "user", "event": "enter", "t": t, "tid": tid, "name": name,
                              "entry": fields.get("entry"), "usp": fields.get("usp")})
            self.hub.note("kernel", "user", f"{name} (pid {tid}) entered U-mode at {fields.get('entry', '?')}", t, tid=tid)
        elif kind == "USER_EXIT":
            tid, code = int(fields.get("tid", 0)), int(fields.get("code", 0))
            self.user_tids.discard(tid)
            self.hub.publish({"type": "user", "event": "exit", "t": t, "tid": tid, "code": code, "name": self.name(tid)})
            if code != -9:  # a kill is reported by USER_KILL just before
                self.hub.note("kernel", "user", f"{self.name(tid)} (pid {tid}) left U-mode with exit code {code}", t, tid=tid)
        elif kind == "USER_KILL":
            tid = int(fields.get("tid", 0))
            cause = fields.get("cause", "fault").replace("_", " ")
            self.user_tids.discard(tid)
            self.hub.publish({"type": "user", "event": "kill", "t": t, "tid": tid, "cause": cause,
                              "sepc": fields.get("sepc"), "name": self.name(tid)})
            self.hub.note("kernel", "kill", f"the kernel killed {self.name(tid)} (pid {tid}): {cause} at {fields.get('sepc', '?')}", t, tid=tid)
        elif kind == "DEADLOCK":
            self.hub.note("kernel", "deadlock", "deadlock detected: " + " ".join(f"{k}={v}" for k, v in fields.items()), t)
        elif kind == "HEAPMAP":
            raw = fields.get("map", "")
            self.hub.publish({"type": "heapmap", "t": t, "slices": int(fields.get("slices", 0) or 0),
                              "slice_bytes": int(fields.get("slice_bytes", 0) or 0),
                              "map": [int(raw[i:i + 2], 16) for i in range(0, len(raw) - 1, 2)]})
        elif kind == "BOOT":
            self.hub.publish({"type": "boot", "stage": fields.pop("stage", "?"), "t": t, **fields})
        elif kind == "SNAP":
            snap = parse_snapshot(fields)
            snap["uptime_us"] = t
            for task in snap["tasks"]:
                self.names[task["tid"]] = task["name"]
            if not self.ready:
                # The first snapshot comes from the monitor task: the scheduler
                # is dispatching and the shell has printed its banner.
                self.ready = True
                self.hub.publish({"type": "boot", "stage": "sched", "t": t, "policy": snap.get("policy"),
                                  "q": snap.get("q"), "tasks": len(snap["tasks"])})
                self.hub.publish({"type": "boot", "stage": "shell", "t": t})
                self.hub.note("kernel", "boot", f"kernel up: {snap.get('policy')} scheduler, quantum {snap.get('q')} ticks", t)
                if self.on_ready:
                    self.on_ready()
            snap["user_tids"] = sorted(self.user_tids & {task["tid"] for task in snap["tasks"]})
            self.hub.publish(flow.message(t))
            self.hub.publish(snap)


class Machine:
    """One QEMU instance running the Micro-OS shell."""

    def __init__(self, hub):
        self.hub = hub
        self.lock = threading.Lock()
        self.proc = None
        self.policy, self.quantum = "rr", 4
        self.after_ready = None  # shell command to type once the shell is up

    def boot(self, policy=None, quantum=None, then=None):
        with self.lock:
            self._stop_locked()
            self.policy = policy or self.policy
            self.quantum = quantum or self.quantum
            self.after_ready = then
            self.hub.publish({"type": "reset"})
            cmd = [
                "qemu-system-riscv64", "-machine", "virt", "-cpu", "rv64", "-smp", "1", "-m", f"{RAM_MB}M",
                "-bios", "default", "-display", "none", "-serial", "stdio", "-monitor", "none",
                "-no-reboot", "-kernel", str(ELF),
                # trace=1: the scheduler's DISPATCH/OFF/WAKE/SPAWN/EXIT lines feed the
                # flow and journal; they never reach the terminal (StreamSplitter).
                "-append", f"test=shell monitor=1 trace=1 policy={self.policy} quantum={self.quantum}",
            ]
            proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, bufsize=0)
            self.proc = proc
            self.hub.publish({"type": "status", "running": True, "policy": self.policy, "quantum": self.quantum})
            self.hub.note("panel", "power", f"machine booted ({self.policy}, quantum {self.quantum})")
            threading.Thread(target=self._pump, args=(proc,), daemon=True).start()

    def _stop_locked(self):
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        self.proc = None

    def stop(self):
        with self.lock:
            self._stop_locked()
        self.hub.publish({"type": "status", "running": False})
        self.hub.note("panel", "power", "machine powered off")

    def send(self, data: bytes):
        proc = self.proc
        if proc and proc.poll() is None:
            try:
                proc.stdin.write(data)
                proc.stdin.flush()
            except BrokenPipeError:
                pass

    def _ready(self):
        cmd, self.after_ready = self.after_ready, None
        if cmd:
            # Give the shell a moment to print its prompt, then type the command.
            threading.Timer(0.4, lambda: self.send((cmd + "\r").encode())).start()

    def _pump(self, proc):
        splitter = StreamSplitter(self.hub, on_ready=self._ready)
        decoder = codecs.getincrementaldecoder("utf-8")("replace")
        fd = proc.stdout.fileno()
        while True:
            chunk = os.read(fd, 65536)
            if not chunk:
                break
            splitter.feed(decoder.decode(chunk))
        code = proc.wait()
        if self.proc is proc:
            self.hub.publish({"type": "term", "data": f"\r\n\x1b[90m[machine powered off, exit code {code}]\x1b[0m\r\n"})
            self.hub.publish({"type": "status", "running": False, "exit_code": code})


class TestRunner:
    def __init__(self, hub):
        self.hub = hub
        self.running = False

    def start(self):
        if self.running:
            return False
        self.running = True
        threading.Thread(target=self._run, daemon=True).start()
        return True

    def _run(self):
        self.hub.publish({"type": "testlog", "line": "$ scripts/test_all.sh", "start": True})
        self.hub.note("panel", "tests", "test suite started")
        proc = subprocess.Popen(["bash", "scripts/test_all.sh"], cwd=ROOT, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, text=True, errors="replace")
        for line in proc.stdout:
            self.hub.publish({"type": "testlog", "line": line.rstrip("\n")})
        code = proc.wait()
        self.running = False
        self.hub.publish({"type": "testdone", "ok": code == 0, "code": code})
        self.hub.note("panel", "tests", "test suite passed" if code == 0 else f"test suite failed (exit {code})")


# rustc --message-format=short: "src/programs/x.rs:3:5: error[E0425]: cannot find ..."
DIAG = re.compile(r"^(src/[\w/.-]+\.rs):(\d+):(\d+): (error|warning)(\[\w+\])?: (.*)$")


class Builder:
    """Rebuilds the kernel on the host with cargo (the kernel has no compiler
    of its own), then reboots the VM and, optionally, runs a program."""

    def __init__(self, hub, machine):
        self.hub = hub
        self.machine = machine
        self.running = False

    def start(self, run=None):
        if self.running:
            return False
        self.running = True
        threading.Thread(target=self._run, args=(run,), daemon=True).start()
        return True

    def _run(self, run):
        self.hub.publish({"type": "buildlog", "line": "$ cargo build --release   (on the host, in WSL)", "start": True})
        self.hub.note("panel", "build", "kernel build started on the host" + (f" to run `{run}`" if run else ""))
        started = time.time()
        diags = []
        env = dict(os.environ, CARGO_TERM_COLOR="never")
        env["PATH"] = str(Path(CARGO).parent) + os.pathsep + env.get("PATH", "")
        try:
            proc = subprocess.Popen([CARGO, "build", "--release", "--message-format=short"], cwd=ROOT / "kernel",
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace", env=env)
        except OSError as e:
            self.running = False
            self.hub.publish({"type": "builddone", "ok": False, "code": -1, "diags": [], "secs": 0, "error": str(e)})
            return
        for line in proc.stdout:
            line = line.rstrip("\n")
            m = DIAG.match(line.strip())
            if m:
                diags.append({"file": "kernel/" + m[1], "line": int(m[2]), "col": int(m[3]),
                              "level": m[4], "code": (m[5] or "").strip("[]"), "msg": m[6]})
            self.hub.publish({"type": "buildlog", "line": line})
        code = proc.wait()
        secs = round(time.time() - started, 1)
        ok = code == 0
        self.running = False
        self.hub.publish({"type": "builddone", "ok": ok, "code": code, "diags": diags, "secs": secs, "run": run})
        errors = sum(d["level"] == "error" for d in diags)
        self.hub.note("panel", "build", f"kernel built in {secs} s" if ok else f"build failed: {errors} error(s) in {secs} s")
        if ok:
            self.machine.boot(then=run)


def programs():
    src = (PROGRAMS_DIR / "mod.rs").read_text()
    return [{"name": n, "usage": u, "about": a, "file": f"kernel/src/programs/{n}.rs"}
            for n, u, a in PROGRAM.findall(src)]


def info():
    cargo = (ROOT / "kernel/Cargo.toml").read_text()
    sched = (ROOT / "kernel/src/scheduler/mod.rs").read_text()
    version = re.search(r'^version = "([^"]+)"', cargo, re.M)
    max_tasks = re.search(r"pub const MAX_TASKS: usize = (\d+);", sched)
    return {"version": version[1] if version else "?", "arch": "riscv64",
            "max_tasks": int(max_tasks[1]) if max_tasks else 64, "ram_mb": RAM_MB}


def results():
    checks, summary = [], None
    log = ROOT / "traces/all.log"
    if log.exists():
        for line in log.read_text(errors="replace").splitlines():
            m = TRACE.match(line.strip())
            if not m:
                continue
            fields = dict(KV.findall(m[4]))
            if m[3] == "CHECK":
                checks.append({"id": fields.get("id"), "ok": fields.get("ok") == "1"})
            elif m[3] == "SUMMARY":
                summary = {k: int(v) for k, v in fields.items()}
    plots = sorted(p.name for p in (ROOT / "results").glob("*.png")) if (ROOT / "results").exists() else []
    stamp = int(log.stat().st_mtime) if log.exists() else 0
    return {"checks": checks, "summary": summary, "plots": plots, "stamp": stamp}


def md_tables(text):
    """Markdown tables under each '## ' heading: {heading: [[cell, ...], ...]}."""
    out, head = {}, None
    for line in text.splitlines():
        if line.startswith("## "):
            head = line[3:].strip()
        elif line.startswith("|") and head and not set(line.replace("|", "").strip()) <= set("-: "):
            out.setdefault(head, []).append([c.strip() for c in line.strip().strip("|").split("|")])
    return out


def research():
    """R3 study on the kernel (results/research_sync.md) and the Micro-OS Linux
    boot times (results/distro_boottime.md, distro_verify.md)."""
    res = {"r3": None, "distro": None, "figures": []}
    r3 = ROOT / "results/research_sync.md"
    if r3.exists():
        t = md_tables(r3.read_text(errors="replace"))
        pick = lambda key: next((v for k, v in t.items() if k.lower().startswith(key)), None)
        res["r3"] = {"hypotheses": pick("hypotheses"), "bimodal": pick("table 1"), "uniform": pick("table 2"),
                     "mtime": int(r3.stat().st_mtime)}
    d = ROOT / "results/distro_boottime.md"
    if d.exists():
        rows = next(iter(md_tables("## t\n" + d.read_text(errors="replace")).values()), [])
        verify = {}
        v = ROOT / "results/distro_verify.md"
        if v.exists():
            stage = None
            for line in v.read_text(errors="replace").splitlines():
                mm = re.match(r"## (S\d)", line)
                if mm:
                    stage = mm[1]
                mm = re.match(r"Result: passed=(\d+) failed=(\d+)", line)
                if mm and stage:
                    verify[stage] = {"passed": int(mm[1]), "failed": int(mm[2])}
        res["distro"] = {"rows": rows, "verify": verify, "mtime": int(d.stat().st_mtime)}
    res["figures"] = sorted(p.name for p in (ROOT / "results").glob("r3_*.png")) + \
        (["distro_boottime.png"] if (ROOT / "results/distro_boottime.png").exists() else [])
    return res


# ---------------------------------------------------------------- files on the host
def resolve(rel):
    """Map a project-relative path to a real path inside ROOT, or None."""
    rel = (rel or "").strip().lstrip("/")
    if any(part in HIDDEN or part.startswith(".") and part not in (".gitignore",) for part in Path(rel).parts):
        return None
    p = (ROOT / rel).resolve()
    try:
        p.relative_to(ROOT)
    except ValueError:
        return None
    return p


def editable(p: Path):
    return p.parent == PROGRAMS_DIR and p.suffix == ".rs" and p.stem != "mod"


def list_dir(rel):
    p = resolve(rel)
    if p is None or not p.is_dir():
        return None
    entries = []
    for c in sorted(p.iterdir(), key=lambda c: (not c.is_dir(), c.name.lower())):
        if c.name in HIDDEN or (c.name.startswith(".") and c.name != ".gitignore"):
            continue
        st = c.stat()
        entries.append({"name": c.name, "path": str(c.relative_to(ROOT)), "dir": c.is_dir(),
                        "size": st.st_size if c.is_file() else None, "mtime": int(st.st_mtime),
                        "editable": c.is_file() and editable(c)})
    return {"path": str(p.relative_to(ROOT)) if p != ROOT else "", "entries": entries}


def read_file(rel):
    p = resolve(rel)
    if p is None or not p.is_file():
        return None
    out = {"path": str(p.relative_to(ROOT)), "size": p.stat().st_size, "editable": editable(p),
           "mtime": int(p.stat().st_mtime)}
    if p.suffix == ".png" and p.parent == ROOT / "results":
        out.update(kind="image", url=f"/results/{p.name}")
    elif p.suffix in TEXT_SUFFIXES or p.name in ("Makefile", "LICENSE"):
        if out["size"] > MAX_TEXT:
            out.update(kind="toolarge")
        else:
            out.update(kind="text", text=p.read_text(errors="replace"))
    else:
        out.update(kind="binary")
    return out


def save_program(name, source, create):
    """Write kernel/src/programs/<name>.rs; a new program is also registered
    in programs/mod.rs so the shell can run it after the next build."""
    if not PROGRAM_NAME.match(name or "") or name in ("mod",):
        return {"error": "program names are 2-24 lowercase letters, digits or _ and start with a letter"}, 400
    if len(source) > 200_000:
        return {"error": "source is larger than 200 KB"}, 400
    path = PROGRAMS_DIR / f"{name}.rs"
    if create and path.exists():
        return {"error": f"a program called {name} already exists"}, 409
    if not create and not path.exists():
        return {"error": f"no program called {name}"}, 404
    path.write_text(source)
    if create:
        mod = PROGRAMS_DIR / "mod.rs"
        src = mod.read_text()
        mods = list(re.finditer(r"^mod \w+;\n", src, re.M))
        at = mods[-1].end() if mods else 0
        src = src[:at] + f"mod {name};\n" + src[at:]
        entry = (f'    Program {{ name: "{name}", usage: "{name} [args...]", '
                 f'about: "your program (written in NEXUS Code)", main: {name}::main }},\n')
        end = src.index("];", src.index("pub const PROGRAMS"))
        src = src[:end] + entry + src[end:]
        mod.write_text(src)
    return {"ok": True, "path": f"kernel/src/programs/{name}.rs", "created": bool(create)}, 200


class Handler(BaseHTTPRequestHandler):
    hub = machine = tests = builder = None
    CONTENT_TYPES = {".woff2": "font/woff2", ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8",
                     ".css": "text/css; charset=utf-8", ".png": "image/png", ".svg": "image/svg+xml",
                     ".md": "text/plain; charset=utf-8"}

    def log_message(self, fmt, *args):
        pass  # keep the terminal quiet

    def send_json(self, obj, status=200):
        body = json.dumps(obj).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def send_file(self, path: Path):
        if not path.is_file():
            self.send_error(404)
            return
        body = path.read_bytes()
        self.send_response(200)
        self.send_header("Content-Type", self.CONTENT_TYPES.get(path.suffix, "application/octet-stream"))
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def body(self):
        return self.rfile.read(int(self.headers.get("Content-Length", 0) or 0))

    def do_GET(self):
        url = urlparse(self.path)
        path, query = url.path, parse_qs(url.query)
        arg = lambda k: (query.get(k) or [""])[0]
        if path == "/":
            return self.send_file(STATIC / "index.html")
        if path == "/api/events":
            return self.stream_events()
        if path == "/api/programs":
            return self.send_json(programs())
        if path == "/api/info":
            return self.send_json(info())
        if path == "/api/results":
            return self.send_json(results())
        if path == "/api/research":
            return self.send_json(research())
        if path == "/api/ls":
            listing = list_dir(arg("path"))
            return self.send_json(listing) if listing else self.send_json({"error": "no such folder"}, 404)
        if path == "/api/file":
            f = read_file(arg("path"))
            return self.send_json(f) if f else self.send_json({"error": "no such file"}, 404)
        for prefix, base in (("/static/fonts/", STATIC / "fonts"), ("/static/", STATIC), ("/results/", ROOT / "results")):
            if path.startswith(prefix):
                name = path[len(prefix):]
                if SAFE_NAME.match(name):
                    return self.send_file(base / name)
        self.send_error(404)

    def do_POST(self):
        path = urlparse(self.path).path
        if path == "/api/input":
            self.machine.send(self.body())
            return self.send_json({"ok": True})
        if path == "/api/boot":
            req = json.loads(self.body() or b"{}")
            policy = req.get("policy", "rr")
            quantum = int(req.get("quantum", 4))
            if policy not in POLICIES or not 1 <= quantum <= 1000:
                return self.send_json({"error": "bad policy or quantum"}, 400)
            self.machine.boot(policy, quantum)
            return self.send_json({"ok": True})
        if path == "/api/stop":
            self.machine.stop()
            return self.send_json({"ok": True})
        if path == "/api/tests":
            return self.send_json({"started": self.tests.start()})
        if path == "/api/program":
            req = json.loads(self.body() or b"{}")
            res, status = save_program(req.get("name", ""), req.get("source", ""), bool(req.get("create")))
            if status == 200:
                self.hub.note("panel", "file", ("created " if res["created"] else "saved ") + res["path"])
            return self.send_json(res, status)
        if path == "/api/build":
            req = json.loads(self.body() or b"{}")
            run = (req.get("run") or "").strip()
            if run and not re.match(r"^[a-z][a-z0-9_]{1,23}( [\w .&=-]{0,60})?$", run):
                return self.send_json({"error": "bad command to run"}, 400)
            return self.send_json({"started": self.builder.start(run or None)})
        self.send_error(404)

    def stream_events(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        q = self.hub.subscribe()
        try:
            while True:
                try:
                    msg = q.get(timeout=15)
                    self.wfile.write(f"data: {json.dumps(msg)}\n\n".encode())
                except queue.Empty:
                    self.wfile.write(b": keep-alive\n\n")
                self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError):
            pass
        finally:
            self.hub.unsubscribe(q)


def main():
    ap = argparse.ArgumentParser(description="NEXUS OS (Micro-OS) web front end")
    ap.add_argument("--port", type=int, default=8080)
    ap.add_argument("--policy", default="rr", choices=sorted(POLICIES))
    ap.add_argument("--quantum", type=int, default=4)
    a = ap.parse_args()
    if not ELF.exists():
        raise SystemExit(f"kernel not built: {ELF} (run scripts/frontend.sh)")

    hub = Hub()
    machine = Machine(hub)
    Handler.hub, Handler.machine, Handler.tests = hub, machine, TestRunner(hub)
    Handler.builder = Builder(hub, machine)
    machine.boot(a.policy, a.quantum)
    server = ThreadingHTTPServer(("127.0.0.1", a.port), Handler)
    server.daemon_threads = True
    print(f"NEXUS OS (Micro-OS) front end: http://localhost:{a.port}   (Ctrl-C to stop)")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        machine.stop()


if __name__ == "__main__":
    main()
