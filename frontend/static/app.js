// Micro-OS control panel: terminal + live views fed by Server-Sent Events.
"use strict";

const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => document.querySelectorAll(sel);
const PALETTE = ["#c4b5fd", "#4ade80", "#fbbf24", "#f0abfc", "#7dd3fc", "#fb923c", "#a78bfa", "#2dd4bf", "#f87171", "#a3e635"];
const colorOf = (tid) => PALETTE[(tid - 1) % PALETTE.length];
const HISTORY_SECONDS = 60;
const RING_LEN = 2 * Math.PI * 32;

// Default arguments per program (long-running ones go to the background).
const PRESETS = {
  hello: "world 42",
  counter: "10 300 &",
  primes: "200000",
  fib: "60",
  spin: "10 &",
  memtest: "2048",
  template: "3",
};
const EXTRA_QUICK = ["spin 10 yield &", "sysinfo", "ps", "sched", "mem"];

let info = { version: "0.1.0", arch: "riscv64", max_tasks: 64 };
let snaps = [];
let lastTids = null;
let running = false;

// ================================================================ terminal
const MONO = '"JetBrains Mono", Consolas, monospace';
const term = new Terminal({
  fontFamily: MONO,
  fontSize: 13.5,
  lineHeight: 1.25,
  cursorBlink: true,
  cursorStyle: "bar",
  scrollback: 5000,
  allowTransparency: true, // the glass window behind the text shows through
  theme: {
    background: "rgba(0,0,0,0)", foreground: "#e9e4ff", cursor: "#c4b5fd", cursorAccent: "#110c25",
    selectionBackground: "#4c3a8a",
    black: "#2a2150", brightBlack: "#6f6795", red: "#f87171", green: "#4ade80", yellow: "#fbbf24",
    blue: "#93c5fd", magenta: "#c084fc", cyan: "#7dd3fc", white: "#e9e4ff",
  },
});
const fit = new FitAddon.FitAddon();
term.loadAddon(fit);
term.open($("#terminal"));
const refit = () => { try { fit.fit(); } catch (_) { /* hidden */ } };
refit();
// xterm measures the character cell when it opens. If the web font arrives
// later, glyphs no longer match the cell and columns drift; re-measure once
// the font is really loaded.
document.fonts.load(`13.5px "JetBrains Mono"`).then(() => {
  term.options.fontFamily = "monospace";
  term.options.fontFamily = MONO;
  refit();
});
window.addEventListener("resize", () => { refit(); drawCharts(); });

let pending = "";
let timer = null;
function sendInput(text) {
  pending += text;
  if (!timer) {
    timer = setTimeout(() => {
      const body = pending;
      pending = "";
      timer = null;
      fetch("/api/input", { method: "POST", body });
    }, 8);
  }
}
term.onData(sendInput);

function runCommand(cmd) {
  showView("terminal");
  sendInput(cmd + "\r");
  term.focus();
}

// F1-F4 as in the status bar.
const FKEYS = { F1: "help", F2: "sysinfo", F3: "ps", F4: "mem" };
document.addEventListener("keydown", (e) => {
  if (FKEYS[e.key]) { e.preventDefault(); runCommand(FKEYS[e.key]); }
}, true);
term.attachCustomKeyEventHandler((e) => !FKEYS[e.key]);
$$(".sb-keys button").forEach((b) => (b.onclick = () => runCommand(b.dataset.key)));

// ================================================================ views
function showView(name) {
  $$(".nav").forEach((n) => n.classList.toggle("active", n.dataset.view === name));
  $$(".view").forEach((v) => v.classList.toggle("active", v.id === `view-${name}`));
  if (name === "terminal") { refit(); term.focus(); }
  if (name === "tests") loadResults();
  drawCharts();
}
$$(".nav").forEach((n) => (n.onclick = () => showView(n.dataset.view)));

// ================================================================ events
function connect() {
  const es = new EventSource("/api/events");
  es.onopen = () => $("#conn-dot").classList.add("on");
  es.onmessage = (e) => {
    const m = JSON.parse(e.data);
    switch (m.type) {
      case "term": term.write(m.data); break;
      case "reset": term.reset(); resetLive(); break;
      case "status": setStatus(m); break;
      case "snap": onSnapshot(m); break;
      case "testlog": onTestLog(m); break;
      case "testdone": onTestDone(m); break;
    }
  };
  es.onerror = () => { $("#conn-dot").classList.remove("on"); setStatus({ running: false, lost: true }); };
}

const POLICY_NAMES = { rr: "Round Robin", prio: "Priority", aging: "Priority + aging", mlfq: "MLFQ",
  fcfs: "FCFS", sjf: "SJF", srtf: "SRTF", hrrn: "HRRN",
  RR: "Round Robin", PRIO: "Priority", "PRIO+AGING": "Priority + aging", MLFQ: "MLFQ",
  FCFS: "FCFS", SJF: "SJF", SRTF: "SRTF", HRRN: "HRRN" };

function setStatus(s) {
  running = !!s.running;
  $("#sb-dot").className = "dot " + (running ? "on" : "off");
  $("#power-state").textContent = s.lost ? "panel server unreachable" : running ? "machine running" : "machine powered off";
  if (running) {
    $("#policy").value = s.policy;
    $("#quantum").value = s.quantum;
    $("#sb-sched").textContent = `Scheduler: ${POLICY_NAMES[s.policy]} (q=${s.quantum})`;
  } else {
    $("#sb-sched").textContent = s.lost ? "Panel: disconnected" : "Machine: powered off";
  }
  $("#reboot").textContent = running ? "Reboot" : "Boot";
  $("#poweroff").disabled = !running;
}

function resetLive() {
  snaps = [];
  lastTids = null;
  $("#activity").innerHTML = '<li class="muted">booting…</li>';
  $("#task-rows").innerHTML = "";
  $("#mini-rows").innerHTML = "";
  drawCharts();
}

// ================================================================ snapshots
function onSnapshot(s) {
  snaps.push(s);
  const horizon = s.uptime_us - (HISTORY_SECONDS + 2) * 1e6;
  while (snaps.length > 2 && snaps[0].uptime_us < horizon) snaps.shift();
  const prev = snaps.length > 1 ? snaps[snaps.length - 2] : null;
  const dt = prev ? s.ticks - prev.ticks : 0;
  const prevCpu = new Map((prev ? prev.tasks : []).map((t) => [t.tid, t.cpu]));
  const shareOf = (t) => (dt > 0 ? Math.max(0, (t.cpu - (prevCpu.get(t.tid) ?? t.cpu)) / dt) : 0);

  // rings
  const busy = prev && dt > 0 ? Math.max(0, 1 - (s.idle - prev.idle) / dt) : 0;
  ring("#ring-cpu", busy, `${Math.round(busy * 100)}%`);
  const heap = s.heap_used / s.heap_cap;
  ring("#ring-mem", Math.max(heap, 0.01), heap < 0.1 ? `${(heap * 100).toFixed(1)}%` : `${Math.round(heap * 100)}%`);
  ring("#ring-tasks", s.tasks.length / info.max_tasks, `${s.tasks.length}/${info.max_tasks}`);

  // status bar uptime
  const sec = Math.floor(s.uptime_us / 1e6);
  const hh = String(Math.floor(sec / 3600)).padStart(2, "0");
  const mm = String(Math.floor((sec % 3600) / 60)).padStart(2, "0");
  const ss = String(sec % 60).padStart(2, "0");
  $("#sb-uptime").textContent = `Uptime: ${hh}:${mm}:${ss}`;

  // rail: running processes
  $("#mini-rows").innerHTML = s.tasks
    .map((t) => `<tr><td>${t.tid}</td><td class="name">${t.name}</td><td><span class="status ${t.state}">${cap(t.state)}</span></td></tr>`)
    .join("");

  // processes view
  $("#sched-line").textContent = `${POLICY_NAMES[s.policy] || s.policy}, quantum ${s.q} ticks`;
  $("#task-rows").innerHTML = s.tasks
    .map((t) => {
      const pct = Math.round(shareOf(t) * 100);
      const killable = t.name !== "shell" && t.name !== "monitor";
      return `<tr>
        <td class="num">${t.tid}</td>
        <td class="name"><i class="swatch" style="background:${colorOf(t.tid)}"></i>${t.name}</td>
        <td><span class="status ${t.state}">${cap(t.state)}</span></td>
        <td class="num">${t.prio}</td><td class="num">${t.level}</td>
        <td class="num">${t.cpu.toLocaleString()}</td><td class="num">${t.disp.toLocaleString()}</td>
        <td><div class="share"><div class="share-bar"><div style="width:${pct}%;background:${colorOf(t.tid)}"></div></div><span>${pct}%</span></div></td>
        <td>${killable ? `<button class="kill" data-tid="${t.tid}">stop</button>` : ""}</td>
      </tr>`;
    })
    .join("");
  $$(".kill").forEach((b) => (b.onclick = () => runCommand(`kill ${b.dataset.tid}`)));

  // memory view
  $("#heap-used").style.width = `${Math.max(heap * 100, 0.4)}%`;
  $("#heap-peak").style.left = `${(100 * s.heap_peak) / s.heap_cap}%`;
  $("#heap-pct").textContent = `${size(s.heap_used)} of ${size(s.heap_cap)}`;
  dl("#heap-stats", [
    ["in use", size(s.heap_used)], ["peak", size(s.heap_peak)], ["free blocks", s.free_blocks],
    ["largest free block", size(s.largest)], ["fragmentation", `${(s.frag_bp / 100).toFixed(2)}%`],
    ["allocations", s.allocs], ["frees", s.frees],
  ]);

  // scheduler view
  $("#sched-policy").textContent = POLICY_NAMES[s.policy] || s.policy;
  dl("#sched-stats", [
    ["timer ticks", s.ticks], ["idle ticks", s.idle], ["dispatches", s.disp], ["preemptions", s.pre],
    ["yields", s.yld], ["blocks", s.blk], ["wakeups", s.wake], ["ready queue", s.ready], ["tick length", `${s.tick_us} µs`],
  ]);

  trackActivity(s);
  drawCharts();
}

function ring(sel, frac, label) {
  const el = $(sel);
  const f = Math.min(1, Math.max(0, frac));
  el.querySelector(".val").style.strokeDashoffset = RING_LEN * (1 - f);
  el.querySelector(".tip").style.transform = `rotate(${f * 360}deg)`;
  el.querySelector("b").textContent = label;
}
const cap = (st) => st.charAt(0) + st.slice(1).toLowerCase();
const size = (b) => (b >= 1048576 ? `${(b / 1048576).toFixed(1)} MiB` : b >= 10240 ? `${(b / 1024).toFixed(1)} KiB` : `${b} B`);
function dl(sel, rows) {
  $(sel).innerHTML = rows.map(([k, v]) => `<dt>${k}</dt><dd>${typeof v === "number" ? v.toLocaleString() : v}</dd>`).join("");
}

function trackActivity(s) {
  const now = new Map(s.tasks.map((t) => [t.tid, t]));
  const list = $("#activity");
  const at = `${(s.uptime_us / 1e6).toFixed(1)} s`;
  const add = (cls, text) => {
    if (list.firstElementChild?.classList.contains("muted")) list.innerHTML = "";
    const li = document.createElement("li");
    li.innerHTML = `<time>${at}</time><span class="${cls}">${text}</span>`;
    list.prepend(li);
    while (list.children.length > 50) list.lastElementChild.remove();
  };
  if (lastTids) {
    for (const [tid, t] of now) if (!lastTids.has(tid)) add("start", `▶ ${t.name} started as pid ${tid} (priority ${t.prio})`);
    for (const [tid, t] of lastTids)
      if (!now.has(tid)) add("end", `■ ${t.name} (pid ${tid}) finished after ${t.cpu.toLocaleString()} CPU ticks`);
  } else {
    add("start", `kernel up: ${s.tasks.map((t) => `${t.name} (pid ${t.tid})`).join(", ")}`);
  }
  lastTids = now;
}

// ================================================================ charts
function setupCanvas(canvas) {
  const dpr = window.devicePixelRatio || 1;
  // Remember the CSS height once: canvas.height is overwritten below in device pixels.
  canvas.dataset.h ??= canvas.getAttribute("height");
  const w = canvas.clientWidth, h = Number(canvas.dataset.h);
  if (!w) return null;
  canvas.width = w * dpr;
  canvas.height = h * dpr;
  canvas.style.height = `${h}px`;
  const ctx = canvas.getContext("2d");
  ctx.scale(dpr, dpr);
  return { ctx, w, h };
}

function axes(ctx, w, h, labels) {
  const g = { padL: 46, padR: 8, padT: 8, padB: 20 };
  g.cw = w - g.padL - g.padR;
  g.ch = h - g.padT - g.padB;
  ctx.font = "11px Inter, sans-serif";
  ctx.fillStyle = "#8b82b3";
  ctx.strokeStyle = "rgba(167,139,250,0.14)";
  labels.forEach(([frac, text]) => {
    const y = g.padT + g.ch - frac * g.ch;
    ctx.beginPath(); ctx.moveTo(g.padL, y + 0.5); ctx.lineTo(g.padL + g.cw, y + 0.5); ctx.stroke();
    ctx.fillText(text, 4, y + 4);
  });
  ctx.fillText(`-${HISTORY_SECONDS}s`, g.padL, h - 4);
  ctx.fillText("now", g.padL + g.cw - 22, h - 4);
  return g;
}

function timeX(g) {
  const end = snaps.length ? snaps[snaps.length - 1].uptime_us : 0;
  const start = end - HISTORY_SECONDS * 1e6;
  return (t) => g.padL + ((t - start) / (end - start)) * g.cw;
}

function drawCharts() {
  if ($("#view-processes").classList.contains("active")) drawCpu();
  if ($("#view-memory").classList.contains("active")) drawLine("#heap-chart", (s) => s.heap_used, size, "#f0abfc");
  if ($("#view-scheduler").classList.contains("active")) drawSwitches();
}

function drawCpu() {
  const c = setupCanvas($("#cpu-chart"));
  if (!c) return;
  const { ctx, w, h } = c;
  const g = axes(ctx, w, h, [[0, "0%"], [0.25, "25%"], [0.5, "50%"], [0.75, "75%"], [1, "100%"]]);
  if (snaps.length < 2) return;
  const xOf = timeX(g);
  const frames = [];
  const tids = new Set();
  const names = new Map();
  for (let i = 1; i < snaps.length; i++) {
    const a = snaps[i - 1], b = snaps[i];
    const dt = b.ticks - a.ticks;
    if (dt <= 0) continue;
    const prevCpu = new Map(a.tasks.map((t) => [t.tid, t.cpu]));
    const shares = new Map();
    for (const t of b.tasks) {
      names.set(t.tid, t.name);
      const d = t.cpu - (prevCpu.get(t.tid) ?? t.cpu);
      if (d > 0) { shares.set(t.tid, d / dt); tids.add(t.tid); }
    }
    frames.push({ t: b.uptime_us, shares });
  }
  const order = [...tids].sort((x, y) => x - y);
  const base = frames.map(() => 0);
  for (const tid of order) {
    ctx.beginPath();
    frames.forEach((f, i) => {
      const x = xOf(f.t), y = g.padT + g.ch - Math.min(1, base[i] + (f.shares.get(tid) || 0)) * g.ch;
      i ? ctx.lineTo(x, y) : ctx.moveTo(x, y);
    });
    for (let i = frames.length - 1; i >= 0; i--) ctx.lineTo(xOf(frames[i].t), g.padT + g.ch - Math.min(1, base[i]) * g.ch);
    ctx.closePath();
    ctx.fillStyle = colorOf(tid) + "cc";
    ctx.fill();
    frames.forEach((f, i) => (base[i] += f.shares.get(tid) || 0));
  }
  $("#legend").innerHTML =
    order.map((tid) => `<span><i style="background:${colorOf(tid)}"></i>${names.get(tid)} (${tid})</span>`).join("") +
    '<span><i style="background:#ffffff12;border:1px solid #a78bfa44"></i>idle</span>';
}

function drawLine(sel, value, fmt, color, points) {
  const c = setupCanvas($(sel));
  if (!c) return;
  const { ctx, w, h } = c;
  const pts = points || snaps.map((s) => ({ t: s.uptime_us, v: value(s) }));
  const max = Math.max(1, ...pts.map((p) => p.v)) * 1.15;
  const g = axes(ctx, w, h, [[0, fmt(0)], [0.5, fmt(Math.round(max / 2))], [1, fmt(Math.round(max))]]);
  if (pts.length < 2) return;
  const xOf = timeX(g);
  const yOf = (v) => g.padT + g.ch - (v / max) * g.ch;
  const grad = ctx.createLinearGradient(0, g.padT, 0, g.padT + g.ch);
  grad.addColorStop(0, color + "66");
  grad.addColorStop(1, color + "00");
  ctx.beginPath();
  pts.forEach((p, i) => (i ? ctx.lineTo(xOf(p.t), yOf(p.v)) : ctx.moveTo(xOf(p.t), yOf(p.v))));
  ctx.lineTo(xOf(pts[pts.length - 1].t), g.padT + g.ch);
  ctx.lineTo(xOf(pts[0].t), g.padT + g.ch);
  ctx.closePath();
  ctx.fillStyle = grad;
  ctx.fill();
  ctx.beginPath();
  pts.forEach((p, i) => (i ? ctx.lineTo(xOf(p.t), yOf(p.v)) : ctx.moveTo(xOf(p.t), yOf(p.v))));
  ctx.strokeStyle = color;
  ctx.lineWidth = 2;
  ctx.stroke();
}

function drawSwitches() {
  const pts = [];
  for (let i = 1; i < snaps.length; i++) {
    const a = snaps[i - 1], b = snaps[i];
    const secs = ((b.ticks - a.ticks) * b.tick_us) / 1e6;
    if (secs > 0) pts.push({ t: b.uptime_us, v: (b.disp - a.disp) / secs });
  }
  drawLine("#switch-chart", null, (v) => Math.round(v).toLocaleString(), "#c4b5fd", pts);
}

// ================================================================ controls
$("#reboot").onclick = () =>
  fetch("/api/boot", {
    method: "POST",
    body: JSON.stringify({ policy: $("#policy").value, quantum: Number($("#quantum").value) || 4 }),
  }).then(() => showView("terminal"));
$("#poweroff").onclick = () => fetch("/api/stop", { method: "POST" });
$("#quick-run").onclick = () => runCommand($("#quick-select").value);

fetch("/api/info").then((r) => r.json()).then((i) => {
  info = i;
  $("#meta-version").textContent = `v${i.version}`;
  $("#meta-arch").textContent = i.arch;
});

fetch("/api/programs").then((r) => r.json()).then((progs) => {
  const sel = $("#quick-select");
  const cmds = progs.map((p) => `${p.name}${PRESETS[p.name] ? " " + PRESETS[p.name] : ""}`).concat(EXTRA_QUICK);
  sel.innerHTML = cmds.map((c) => `<option value="${c}">${c}</option>`).join("");
  $("#program-cards").innerHTML = progs
    .map((p) => `<div class="card">
        <h3>${p.name}</h3>
        <p>${p.about}</p>
        <code>${p.usage}</code>
        <div class="row"><input type="text" value="${PRESETS[p.name] || ""}" data-prog="${p.name}" aria-label="arguments for ${p.name}" placeholder="arguments">
        <button class="btn primary" data-run="${p.name}">Run</button></div>
      </div>`)
    .join("");
  $$("[data-run]").forEach((b) => {
    b.onclick = () => {
      const args = document.querySelector(`input[data-prog="${b.dataset.run}"]`).value.trim();
      runCommand(`${b.dataset.run}${args ? " " + args : ""}`);
    };
  });
});

// ================================================================ tests
function onTestLog(m) {
  const log = $("#test-log");
  if (m.start) {
    log.textContent = "";
    $("#run-tests").disabled = true;
    $("#run-tests").textContent = "Running…";
  }
  log.textContent += m.line + "\n";
  log.scrollTop = log.scrollHeight;
}

function onTestDone(m) {
  $("#run-tests").disabled = false;
  $("#run-tests").textContent = "Run all tests";
  $("#test-log").textContent += m.ok ? "\n✓ all passed\n" : `\n✗ failed (exit ${m.code})\n`;
  loadResults();
}

$("#run-tests").onclick = () => fetch("/api/tests", { method: "POST" });

function loadResults() {
  fetch("/api/results").then((r) => r.json()).then((res) => {
    const passed = res.checks.filter((c) => c.ok).length;
    $("#check-summary").textContent = res.checks.length ? `${passed} of ${res.checks.length} passed` : "no results yet — run the test suite";
    $("#checks").innerHTML = res.checks.map((c) => `<div class="check ${c.ok ? "" : "fail"}">${c.id}</div>`).join("");
    $("#plots").innerHTML = res.plots
      .map((p) => `<figure><img loading="lazy" src="/results/${p}?v=${res.stamp}" alt="${p}"><figcaption>${p.replace(".png", "")}</figcaption></figure>`)
      .join("");
    $$(".plots figure").forEach((f) => {
      f.onclick = () => {
        $("#lightbox img").src = f.querySelector("img").src;
        $("#lightbox").hidden = false;
      };
    });
  });
}
$("#lightbox").onclick = () => ($("#lightbox").hidden = true);

// ================================================================ starfield
(function starfield() {
  const canvas = $("#sky");
  const ctx = canvas.getContext("2d");
  const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  let stars = [];
  function seed() {
    const dpr = window.devicePixelRatio || 1;
    canvas.width = innerWidth * dpr;
    canvas.height = innerHeight * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const n = Math.round((innerWidth * innerHeight) / 5200);
    stars = Array.from({ length: n }, () => ({
      x: Math.random() * innerWidth,
      y: Math.random() * innerHeight,
      r: Math.random() < 0.08 ? 1.5 + Math.random() : 0.4 + Math.random() * 0.8,
      phase: Math.random() * Math.PI * 2,
      speed: 0.4 + Math.random() * 1.2,
      hue: Math.random() < 0.3 ? "233,213,255" : "255,255,255",
    }));
  }
  function draw(t) {
    ctx.clearRect(0, 0, innerWidth, innerHeight);
    for (const s of stars) {
      const a = still ? 0.7 : 0.35 + 0.65 * (0.5 + 0.5 * Math.sin(s.phase + (t / 1000) * s.speed));
      ctx.fillStyle = `rgba(${s.hue},${a})`;
      ctx.beginPath();
      ctx.arc(s.x, s.y, s.r, 0, Math.PI * 2);
      ctx.fill();
      if (s.r > 1.4) {
        ctx.fillStyle = `rgba(216,180,254,${a * 0.25})`;
        ctx.beginPath();
        ctx.arc(s.x, s.y, s.r * 3, 0, Math.PI * 2);
        ctx.fill();
      }
    }
    if (!still) requestAnimationFrame(draw);
  }
  seed();
  addEventListener("resize", seed);
  requestAnimationFrame(draw);
})();

connect();
term.focus();
