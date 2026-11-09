// Micro-OS control panel: terminal + live dashboards fed by Server-Sent Events.
"use strict";

const $ = (sel) => document.querySelector(sel);
const PALETTE = ["#7c9cff", "#34d399", "#fbbf24", "#f472b6", "#60a5fa", "#a78bfa", "#fb923c", "#2dd4bf", "#f87171", "#a3e635"];
const colorOf = (tid) => PALETTE[(tid - 1) % PALETTE.length];
const HISTORY_SECONDS = 60;

// Default arguments for the quick-run chips; long-running ones go to the background.
const QUICK_ARGS = {
  hello: "world 42",
  counter: "10 300 &",
  primes: "200000",
  fib: "60",
  spin: "10 &",
  memtest: "2048",
  template: "3",
};
const EXTRA_CHIPS = ["spin 10 yield &", "ps", "sched", "mem", "help"];

// ---------------------------------------------------------------- terminal
const term = new Terminal({
  fontFamily: '"JetBrains Mono", Consolas, monospace',
  fontSize: 13,
  lineHeight: 1.2,
  cursorBlink: true,
  scrollback: 5000,
  theme: {
    background: "#070b13", foreground: "#d7def0", cursor: "#7c9cff", selectionBackground: "#2a3a66",
    black: "#1f2937", brightBlack: "#6b7280", green: "#34d399", yellow: "#fbbf24", blue: "#7c9cff", red: "#f87171",
  },
});
const fit = new FitAddon.FitAddon();
term.loadAddon(fit);
term.open($("#terminal"));
fit.fit();
window.addEventListener("resize", () => fit.fit());

let pendingInput = "";
let inputTimer = null;
function sendInput(text) {
  pendingInput += text;
  if (!inputTimer) {
    inputTimer = setTimeout(() => {
      const body = pendingInput;
      pendingInput = "";
      inputTimer = null;
      fetch("/api/input", { method: "POST", body });
    }, 8);
  }
}
term.onData(sendInput);

function runCommand(cmd) {
  sendInput(cmd + "\r");
  term.focus();
}

// ---------------------------------------------------------------- state
let snaps = [];        // recent snapshots (oldest first)
let lastTids = null;   // tid -> task, from the previous snapshot

function resetLive() {
  snaps = [];
  lastTids = null;
  $("#activity").innerHTML = '<li class="muted">booting…</li>';
  $("#task-rows").innerHTML = "";
  drawChart();
}

// ---------------------------------------------------------------- events
function connect() {
  const es = new EventSource("/api/events");
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
  es.onerror = () => setStatus({ running: false, disconnected: true });
}

function setStatus(s) {
  const pill = $("#status");
  if (s.disconnected) {
    pill.className = "pill off";
    pill.textContent = "server unreachable";
    return;
  }
  pill.className = "pill " + (s.running ? "on" : "off");
  const names = { rr: "Round Robin", prio: "Priority", aging: "Priority + aging", mlfq: "MLFQ" };
  pill.textContent = s.running ? `Running · ${names[s.policy] || s.policy} · q=${s.quantum}` : "Powered off";
  if (s.running) {
    $("#policy").value = s.policy;
    $("#quantum").value = s.quantum;
  }
  $("#reboot").textContent = s.running ? "Reboot" : "Boot";
  $("#poweroff").disabled = !s.running;
}

// ---------------------------------------------------------------- snapshots
function onSnapshot(s) {
  snaps.push(s);
  const horizon = s.uptime_us - (HISTORY_SECONDS + 2) * 1e6;
  while (snaps.length > 2 && snaps[0].uptime_us < horizon) snaps.shift();
  const prev = snaps.length > 1 ? snaps[snaps.length - 2] : null;

  const dt = prev ? s.ticks - prev.ticks : 0;
  const prevCpu = new Map((prev ? prev.tasks : []).map((t) => [t.tid, t.cpu]));
  const shareOf = (t) => (dt > 0 ? Math.max(0, (t.cpu - (prevCpu.get(t.tid) ?? t.cpu)) / dt) : 0);

  // tiles
  if (prev && dt > 0) {
    const busy = 1 - (s.idle - prev.idle) / dt;
    $("#t-cpu").textContent = `${Math.round(Math.max(0, busy) * 100)}%`;
    const seconds = (dt * s.tick_us) / 1e6;
    $("#t-sw").textContent = Math.round((s.disp - prev.disp) / seconds).toLocaleString();
  }
  $("#t-cpu-sub").textContent = `${s.ready} ready in queue`;
  const counts = { RUNNING: 0, READY: 0, BLOCKED: 0 };
  s.tasks.forEach((t) => (counts[t.state] = (counts[t.state] || 0) + 1));
  $("#t-tasks").textContent = s.tasks.length;
  $("#t-tasks-sub").textContent = `${counts.READY} ready · ${counts.BLOCKED} blocked`;
  $("#t-up").textContent = `${(s.uptime_us / 1e6).toFixed(1)} s`;
  $("#t-ticks").textContent = `${s.ticks.toLocaleString()} ticks of ${s.tick_us} µs`;

  // tasks table
  $("#task-rows").innerHTML = s.tasks
    .map((t) => {
      const pct = Math.round(shareOf(t) * 100);
      return `<tr>
        <td class="num">${t.tid}</td>
        <td class="name"><i class="dot" style="background:${colorOf(t.tid)}"></i>${t.name}</td>
        <td><span class="state ${t.state}">${t.state}</span></td>
        <td class="num">${t.prio}</td>
        <td class="num">${t.level}</td>
        <td class="num">${t.cpu.toLocaleString()}</td>
        <td><div class="share"><div class="share-bar"><div style="width:${pct}%;background:${colorOf(t.tid)}"></div></div><span>${pct}%</span></div></td>
      </tr>`;
    })
    .join("");

  // scheduler + heap
  const policyNames = { RR: "Round Robin", PRIO: "Priority", "PRIO+AGING": "Priority + aging", MLFQ: "MLFQ" };
  $("#sched-info").textContent = `${policyNames[s.policy] || s.policy}, quantum ${s.q} ticks`;
  $("#sched-policy").textContent = s.policy;
  dl("#sched-stats", [
    ["dispatches", s.disp], ["preemptions", s.pre], ["yields", s.yld], ["blocks", s.blk],
    ["wakeups", s.wake], ["idle ticks", s.idle], ["ready queue", s.ready],
  ]);
  const usedPct = (100 * s.heap_used) / s.heap_cap;
  $("#heap-used").style.width = `${Math.max(usedPct, 0.4)}%`;
  $("#heap-peak").style.left = `${(100 * s.heap_peak) / s.heap_cap}%`;
  $("#heap-pct").textContent = `${kib(s.heap_used)} of ${kib(s.heap_cap)}`;
  dl("#heap-stats", [
    ["in use", bytes(s.heap_used)], ["peak", bytes(s.heap_peak)], ["free blocks", s.free_blocks],
    ["largest free", kib(s.largest)], ["fragmentation", `${(s.frag_bp / 100).toFixed(2)}%`],
    ["allocs / frees", `${s.allocs.toLocaleString()} / ${s.frees.toLocaleString()}`],
  ]);

  trackActivity(s);
  drawChart();
}

function dl(sel, rows) {
  $(sel).innerHTML = rows
    .map(([k, v]) => `<dt>${k}</dt><dd>${typeof v === "number" ? v.toLocaleString() : v}</dd>`)
    .join("");
}
const kib = (b) => (b >= 1048576 ? `${(b / 1048576).toFixed(1)} MiB` : `${(b / 1024).toFixed(1)} KiB`);
const bytes = (b) => (b < 10240 ? `${b} B` : kib(b));

function trackActivity(s) {
  const now = new Map(s.tasks.map((t) => [t.tid, t]));
  const list = $("#activity");
  const at = `${(s.uptime_us / 1e6).toFixed(1)} s`;
  const add = (cls, text) => {
    if (list.firstElementChild?.classList.contains("muted")) list.innerHTML = "";
    const li = document.createElement("li");
    li.innerHTML = `<time>${at}</time><span class="${cls}">${text}</span>`;
    list.prepend(li);
    while (list.children.length > 40) list.lastElementChild.remove();
  };
  if (lastTids) {
    for (const [tid, t] of now) if (!lastTids.has(tid)) add("start", `▶ ${t.name} started as pid ${tid} (priority ${t.prio})`);
    for (const [tid, t] of lastTids)
      if (!now.has(tid)) add("end", `■ ${t.name} (pid ${tid}) finished after ${t.cpu.toLocaleString()} CPU ticks`);
  } else {
    add("start", `kernel up: ${s.tasks.map((t) => t.name).join(", ")}`);
  }
  lastTids = now;
}

// ---------------------------------------------------------------- chart
function drawChart() {
  const canvas = $("#cpu-chart");
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth, h = 170;
  canvas.width = w * dpr;
  canvas.height = h * dpr;
  const ctx = canvas.getContext("2d");
  ctx.scale(dpr, dpr);
  ctx.clearRect(0, 0, w, h);

  const padL = 34, padB = 18, padT = 6, cw = w - padL - 6, ch = h - padB - padT;
  ctx.font = "11px Inter, sans-serif";
  ctx.fillStyle = "#8a94a8";
  ctx.strokeStyle = "#1f2a3c";
  ctx.lineWidth = 1;
  for (const p of [0, 25, 50, 75, 100]) {
    const y = padT + ch - (p / 100) * ch;
    ctx.beginPath(); ctx.moveTo(padL, y + 0.5); ctx.lineTo(padL + cw, y + 0.5); ctx.stroke();
    ctx.fillText(`${p}%`, 2, y + 4);
  }

  if (snaps.length < 2) return;
  const end = snaps[snaps.length - 1].uptime_us;
  const start = end - HISTORY_SECONDS * 1e6;
  const xOf = (t) => padL + ((t - start) / (end - start)) * cw;
  ctx.fillText(`-${HISTORY_SECONDS}s`, padL, h - 4);
  ctx.fillText("now", padL + cw - 22, h - 4);

  // frames: per-interval CPU share of each task
  const frames = [];
  const tids = new Set();
  for (let i = 1; i < snaps.length; i++) {
    const a = snaps[i - 1], b = snaps[i];
    const dt = b.ticks - a.ticks;
    if (dt <= 0) continue;
    const prevCpu = new Map(a.tasks.map((t) => [t.tid, t.cpu]));
    const shares = new Map();
    for (const t of b.tasks) {
      const d = t.cpu - (prevCpu.get(t.tid) ?? t.cpu);
      if (d > 0) { shares.set(t.tid, d / dt); tids.add(t.tid); }
    }
    frames.push({ t: b.uptime_us, shares });
  }
  const order = [...tids].sort((x, y) => x - y);
  const base = frames.map(() => 0);
  const names = new Map();
  snaps.forEach((s) => s.tasks.forEach((t) => names.set(t.tid, t.name)));

  for (const tid of order) {
    ctx.beginPath();
    frames.forEach((f, i) => {
      const x = xOf(f.t), y = padT + ch - Math.min(1, base[i] + (f.shares.get(tid) || 0)) * ch;
      i ? ctx.lineTo(x, y) : ctx.moveTo(x, y);
    });
    for (let i = frames.length - 1; i >= 0; i--) {
      ctx.lineTo(xOf(frames[i].t), padT + ch - Math.min(1, base[i]) * ch);
    }
    ctx.closePath();
    ctx.fillStyle = colorOf(tid) + "cc";
    ctx.fill();
    frames.forEach((f, i) => (base[i] += f.shares.get(tid) || 0));
  }

  $("#legend").innerHTML =
    order.map((tid) => `<span><i style="background:${colorOf(tid)}"></i>${names.get(tid)} (${tid})</span>`).join("") +
    '<span><i style="background:#1a2336;border:1px solid #2b3650"></i>idle</span>';
}
window.addEventListener("resize", drawChart);

// ---------------------------------------------------------------- controls
$("#reboot").onclick = () =>
  fetch("/api/boot", {
    method: "POST",
    body: JSON.stringify({ policy: $("#policy").value, quantum: Number($("#quantum").value) || 4 }),
  }).then(() => term.focus());
$("#poweroff").onclick = () => fetch("/api/stop", { method: "POST" });

document.querySelectorAll(".tab-btn").forEach((b) => {
  b.onclick = () => {
    document.querySelectorAll(".tab-btn").forEach((x) => x.classList.toggle("active", x === b));
    document.querySelectorAll(".tab").forEach((t) => t.classList.toggle("active", t.id === `tab-${b.dataset.tab}`));
    if (b.dataset.tab === "live") { fit.fit(); drawChart(); term.focus(); }
    if (b.dataset.tab === "tests") loadResults();
  };
});

fetch("/api/programs").then((r) => r.json()).then((progs) => {
  const box = $("#programs");
  const chip = (cmd, title) => {
    const b = document.createElement("button");
    b.className = "chip";
    b.textContent = cmd;
    b.title = title || "";
    b.onclick = () => runCommand(cmd);
    box.appendChild(b);
  };
  progs.forEach((p) => chip(`${p.name}${QUICK_ARGS[p.name] ? " " + QUICK_ARGS[p.name] : ""}`, `${p.usage} — ${p.about}`));
  EXTRA_CHIPS.forEach((c) => chip(c));
});

// ---------------------------------------------------------------- tests tab
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
    $("#check-summary").textContent = res.checks.length
      ? `${passed} of ${res.checks.length} passed`
      : "no results yet — run the test suite";
    $("#checks").innerHTML = res.checks.map((c) => `<div class="check ${c.ok ? "" : "fail"}">${c.id}</div>`).join("");
    $("#plots").innerHTML = res.plots
      .map((p) => `<figure><img loading="lazy" src="/results/${p}?v=${res.stamp}" alt="${p}"><figcaption>${p.replace(".png", "")}</figcaption></figure>`)
      .join("");
    document.querySelectorAll(".plots figure").forEach((f) => {
      f.onclick = () => {
        const lb = $("#lightbox");
        lb.querySelector("img").src = f.querySelector("img").src;
        lb.hidden = false;
      };
    });
  });
}
$("#lightbox").onclick = () => ($("#lightbox").hidden = true);

connect();
term.focus();
