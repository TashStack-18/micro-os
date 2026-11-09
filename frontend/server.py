#!/usr/bin/env python3
"""Web control panel for the Micro-OS.

Boots the kernel in QEMU (`test=shell monitor=1`) and bridges it to a
browser page:

* the serial console streams to an in-page terminal, and keystrokes go back;
* `#T SNAP` lines from the kernel's monitor task feed the live dashboards;
* the test suite can be run and its results and plots browsed.

Run with `scripts/frontend.sh`, then open http://localhost:8080 .
Uses only the Python standard library.
"""
import argparse
import codecs
import json
import os
import queue
import re
import subprocess
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parent.parent
STATIC = Path(__file__).resolve().parent / "static"
ELF = ROOT / "kernel/target/riscv64gc-unknown-none-elf/release/microos"
TRACE = re.compile(r"#T (\d+) (\d+) (\w+)\s*(.*)")
KV = re.compile(r"(\w+)=(\S+)")
PROGRAM = re.compile(r'Program \{ name: "(\w+)", usage: "([^"]*)", about: "([^"]*)"')
SAFE_NAME = re.compile(r"^[\w.-]+$")
POLICIES = {"rr", "prio", "aging", "mlfq"}
HISTORY_LIMIT = 200_000  # characters of console output replayed to new browsers


class Hub:
    """Fan-out of messages to every connected browser (Server-Sent Events)."""

    def __init__(self):
        self.lock = threading.Lock()
        self.clients = []
        self.history = []
        self.history_len = 0
        self.status = {"type": "status", "running": False}
        self.last_snap = None

    def subscribe(self):
        q = queue.Queue()
        with self.lock:
            self.clients.append(q)
            q.put(self.status)
            if self.history:
                q.put({"type": "term", "data": "".join(self.history)})
            if self.last_snap:
                q.put(self.last_snap)
        return q

    def unsubscribe(self, q):
        with self.lock:
            if q in self.clients:
                self.clients.remove(q)

    def publish(self, msg):
        with self.lock:
            if msg["type"] == "term":
                self.history.append(msg["data"])
                self.history_len += len(msg["data"])
                while self.history_len > HISTORY_LIMIT and len(self.history) > 1:
                    self.history_len -= len(self.history.pop(0))
            elif msg["type"] == "snap":
                self.last_snap = msg
            elif msg["type"] == "status":
                self.status = msg
            elif msg["type"] == "reset":
                self.history, self.history_len, self.last_snap = [], 0, None
            for q in self.clients:
                q.put(msg)


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


class StreamSplitter:
    """Separates kernel trace lines (`#T ...`) from terminal text.

    The kernel writes each trace line whole, under the console lock, so the
    text from `#T ` to the next newline is always one event, even when it
    starts mid-line (e.g. right after the shell prompt).
    """

    def __init__(self, hub):
        self.hub = hub
        self.buf = ""

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

    def event(self, line):
        m = TRACE.match(line)
        if not m:
            return
        fields = dict(KV.findall(m[4]))
        if m[3] == "SNAP":
            snap = parse_snapshot(fields)
            snap["uptime_us"] = int(m[1])
            self.hub.publish(snap)


class Machine:
    """One QEMU instance running the Micro-OS shell."""

    def __init__(self, hub):
        self.hub = hub
        self.lock = threading.Lock()
        self.proc = None

    def boot(self, policy="rr", quantum=4):
        with self.lock:
            self._stop_locked()
            self.hub.publish({"type": "reset"})
            cmd = [
                "qemu-system-riscv64", "-machine", "virt", "-cpu", "rv64", "-smp", "1", "-m", "128M",
                "-bios", "default", "-display", "none", "-serial", "stdio", "-monitor", "none",
                "-no-reboot", "-kernel", str(ELF),
                "-append", f"test=shell monitor=1 policy={policy} quantum={quantum}",
            ]
            proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, bufsize=0)
            self.proc = proc
            self.hub.publish({"type": "status", "running": True, "policy": policy, "quantum": quantum})
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

    def send(self, data: bytes):
        proc = self.proc
        if proc and proc.poll() is None:
            try:
                proc.stdin.write(data)
                proc.stdin.flush()
            except BrokenPipeError:
                pass

    def _pump(self, proc):
        splitter = StreamSplitter(self.hub)
        decoder = codecs.getincrementaldecoder("utf-8")("replace")
        fd = proc.stdout.fileno()
        while True:
            chunk = os.read(fd, 4096)
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
        proc = subprocess.Popen(["bash", "scripts/test_all.sh"], cwd=ROOT, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, text=True, errors="replace")
        for line in proc.stdout:
            self.hub.publish({"type": "testlog", "line": line.rstrip("\n")})
        code = proc.wait()
        self.running = False
        self.hub.publish({"type": "testdone", "ok": code == 0, "code": code})


def programs():
    src = (ROOT / "kernel/src/programs/mod.rs").read_text()
    return [{"name": n, "usage": u, "about": a} for n, u, a in PROGRAM.findall(src)]


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


class Handler(BaseHTTPRequestHandler):
    hub = machine = tests = None
    CONTENT_TYPES = {".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8",
                     ".css": "text/css; charset=utf-8", ".png": "image/png", ".md": "text/plain; charset=utf-8"}

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
        path = urlparse(self.path).path
        if path == "/":
            return self.send_file(STATIC / "index.html")
        if path == "/api/events":
            return self.stream_events()
        if path == "/api/programs":
            return self.send_json(programs())
        if path == "/api/results":
            return self.send_json(results())
        for prefix, base in (("/static/", STATIC), ("/results/", ROOT / "results")):
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
    ap = argparse.ArgumentParser(description="Micro-OS web control panel")
    ap.add_argument("--port", type=int, default=8080)
    ap.add_argument("--policy", default="rr", choices=sorted(POLICIES))
    ap.add_argument("--quantum", type=int, default=4)
    a = ap.parse_args()
    if not ELF.exists():
        raise SystemExit(f"kernel not built: {ELF} (run scripts/frontend.sh)")

    hub = Hub()
    Handler.hub, Handler.machine, Handler.tests = hub, Machine(hub), TestRunner(hub)
    Handler.machine.boot(a.policy, a.quantum)
    server = ThreadingHTTPServer(("127.0.0.1", a.port), Handler)
    server.daemon_threads = True
    print(f"Micro-OS control panel: http://localhost:{a.port}   (Ctrl-C to stop)")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        Handler.machine.stop()


if __name__ == "__main__":
    main()
