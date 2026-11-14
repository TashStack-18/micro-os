// Replay mode for the static GitHub Pages copy of the desktop.
//
// GitHub Pages cannot run QEMU or server.py, so this file stands in for the
// server: it plays back a recording of a real kernel session (made with
// scripts/record_replay.py) through the same EventSource and fetch calls the
// desktop uses live. Nothing is simulated: every event is one the real
// kernel produced, replayed with its original timing. Actions that need a
// live machine (typing, reboot, build, tests) explain that instead.
"use strict";
(function () {
  const BASE = "replay/";
  const REPO = "https://github.com/TashStack-18/micro-os";
  const load = (f) => fetch(BASE + f).then((r) => r.json());
  const realFetch = window.fetch.bind(window);
  const data = { session: null, api: null, fs: null };
  const ready = Promise.all([
    realFetch(BASE + "session.json").then((r) => r.json()).then((j) => (data.session = j)),
    realFetch(BASE + "api.json").then((r) => r.json()).then((j) => (data.api = j)),
    realFetch(BASE + "fs.json").then((r) => r.json()).then((j) => (data.fs = j)),
  ]);

  // ------------------------------------------------------------ banner
  function banner() {
    const b = document.createElement("div");
    b.id = "replay-banner";
    b.setAttribute("role", "status");
    if (data.session) b.title = "Recorded " + data.session.recorded + " from the real kernel in QEMU; replayed with its original timing.";
    b.innerHTML =
      '<span class="rb-dot"></span><b>Recorded session</b><span class="rb-text"> of the real kernel</span>' +
      '<a href="' + REPO + '#where-everything-runs" target="_blank" rel="noopener">Run it live</a>';
    const css = document.createElement("style");
    css.textContent =
      "#replay-banner{position:fixed;left:14px;bottom:84px;z-index:9000;display:flex;align-items:center;gap:8px;" +
      "padding:7px 14px;border-radius:999px;background:rgba(14,17,32,.86);color:#e8e6f0;font:500 12.5px/1.2 system-ui,sans-serif;" +
      "border:1px solid rgba(255,255,255,.14);box-shadow:0 6px 20px rgba(0,0,0,.35);white-space:nowrap;max-width:calc(100vw - 24px)}" +
      "#replay-banner .rb-dot{width:7px;height:7px;border-radius:50%;background:#e2655a}" +
      "#replay-banner a{color:#9fc4ff;text-decoration:underline;margin-left:4px}" +
      "#replay-banner .rb-text{overflow:hidden;text-overflow:ellipsis}" +
      "@media (max-width:640px){#replay-banner .rb-text{display:none}}" +
      "#replay-note{position:fixed;left:50%;top:78px;transform:translateX(-50%);z-index:9001;padding:10px 16px;border-radius:10px;" +
      "background:rgba(14,17,32,.94);color:#f1eff7;font:500 13px/1.4 system-ui,sans-serif;border:1px solid rgba(255,255,255,.16);" +
      "box-shadow:0 8px 24px rgba(0,0,0,.4);max-width:min(520px,calc(100vw - 24px));text-align:center}";
    document.head.appendChild(css);
    document.body.appendChild(b);
  }
  let noteTimer = null;
  function note(text) {
    let n = document.getElementById("replay-note");
    if (!n) {
      n = document.createElement("div");
      n.id = "replay-note";
      n.setAttribute("role", "alert");
      document.body.appendChild(n);
    }
    n.textContent = text;
    n.hidden = false;
    clearTimeout(noteTimer);
    noteTimer = setTimeout(() => (n.hidden = true), 4200);
  }
  const LIVE_ONLY = "This is a recorded session of the real kernel, so it can't take input. Run it locally to type commands, reboot, build and run tests.";

  // ------------------------------------------------------------ event stream
  class ReplayEventSource {
    constructor() {
      this.onopen = null;
      this.onmessage = null;
      this.onerror = null;
      this.closed = false;
      this.timers = [];
      ready.then(() => {
        if (!document.getElementById("replay-banner")) banner();
        this.onopen && this.onopen({});
        this.play(true);
      }).catch(() => this.onerror && this.onerror({}));
    }
    emit(msg) {
      if (!this.closed && this.onmessage) this.onmessage({ data: JSON.stringify(msg) });
    }
    play(first) {
      const s = data.session;
      if (!first) this.emit({ type: "reset" });
      for (const [t, msg] of s.events) this.timers.push(setTimeout(() => this.emit(msg), t));
      // Loop: after the recording ends, pause, then replay from power-on.
      this.timers.push(setTimeout(() => { this.timers = []; this.play(false); }, s.duration_ms + 6000));
    }
    addEventListener(type, fn) { if (type === "message") this.onmessage = fn; }
    close() { this.closed = true; this.timers.forEach(clearTimeout); }
  }
  window.EventSource = ReplayEventSource;

  // ------------------------------------------------------------ API calls
  const json = (obj, status = 200) =>
    new Response(JSON.stringify(obj), { status, headers: { "Content-Type": "application/json" } });

  window.fetch = async function (input, init) {
    const url = new URL(typeof input === "string" ? input : input.url, location.href);
    const i = url.pathname.indexOf("/api/");
    if (i < 0) return realFetch(input, init);
    const name = url.pathname.slice(i + 5);
    const method = ((init && init.method) || "GET").toUpperCase();
    await ready;
    if (method !== "GET") {
      note(LIVE_ONLY);
      return json({ ok: false, error: "recorded session: run the OS locally to do this" }, 403);
    }
    const arg = url.searchParams.get("path") || "";
    if (name === "ls") return data.fs.ls[arg] ? json(data.fs.ls[arg]) : json({ error: "not included in the recording" }, 404);
    if (name === "file") return data.fs.file[arg] ? json(data.fs.file[arg]) : json({ error: "not included in the recording" }, 404);
    if (data.api[name]) return json(data.api[name]);
    return json({ error: "not available in the recording" }, 404);
  };
})();
