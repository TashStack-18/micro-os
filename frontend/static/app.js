// NEXUS OS desktop: boot sequence, window manager, terminal, code editor,
// files, monitors, journal and the live kernel topography. Every value comes
// from the kernel (Server-Sent Events from frontend/server.py) or from a real
// host action (files on disk, cargo builds, test runs).
"use strict";

const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => root.querySelectorAll(sel);
const css = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
const STILL = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const NARROW = window.matchMedia("(max-width: 820px)");
const HISTORY_SECONDS = 60;
// Task colours for charts: coral, celadon, chalk, then quieter tones of the same world.
// The same task colours the aurora uses, so a task looks the same everywhere.
const TASK_HUES = ["#ff8a65", "#ffc46b", "#5ed3f3", "#a78bfa", "#6ee7b7", "#f472b6", "#7daaff", "#bee678"];
const PALETTE = TASK_HUES;
// The three busiest tasks own coral, cyan and mint, and keep their colour for as long as
// they stay in the top three. Every other task takes a quieter hue from the rest of the set.
const LEAD = ["#ff8a65", "#5ed3f3", "#6ee7b7"];
const OTHER_HUES = ["#ffc46b", "#a78bfa", "#f472b6", "#7daaff", "#bee678"];
const leadSlot = new Map();   // tid -> 0..2
function updateLeads(shares) {
  const top = shares.filter(([, s]) => s > 0.02).sort((a, b) => b[1] - a[1]).slice(0, 3).map(([t]) => t);
  for (const t of [...leadSlot.keys()]) if (!top.includes(t)) leadSlot.delete(t);
  for (const t of top) {
    if (leadSlot.has(t)) continue;
    const used = new Set(leadSlot.values());
    for (let i = 0; i < 3; i++) if (!used.has(i)) { leadSlot.set(t, i); break; }
  }
}
const colorOf = (tid) => (leadSlot.has(tid) ? LEAD[leadSlot.get(tid)] : OTHER_HUES[(tid * 5 + 3) % OTHER_HUES.length]);
const rgbOf = (hex) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));

const PRESETS = { hello: "world 42", counter: "10 300 &", primes: "200000", fib: "60", spin: "10 &", memtest: "2048", template: "3" };
const POLICY_NAMES = { rr: "Round Robin", prio: "Priority", aging: "Priority + aging", mlfq: "MLFQ", fcfs: "FCFS", sjf: "SJF", srtf: "SRTF", hrrn: "HRRN" };
const POLICY_SHORT = { rr: "RR", prio: "PRIO", aging: "PRIO+AGING", mlfq: "MLFQ", fcfs: "FCFS", sjf: "SJF", srtf: "SRTF", hrrn: "HRRN" };
// Snapshots name policies the kernel's way (RR, PRIO+AGING, ...); boot arguments use rr, aging, ...
const PKEY = { rr: "rr", prio: "prio", "prio+aging": "aging", aging: "aging", mlfq: "mlfq", fcfs: "fcfs", sjf: "sjf", srtf: "srtf", hrrn: "hrrn" };
const pkey = (p) => PKEY[String(p || "").toLowerCase()] || String(p || "").toLowerCase();

let info = { version: "0.1.0", arch: "riscv64", max_tasks: 512, ram_mb: 128 };
let snaps = [];
let running = false;
let lastFlow = null;

// ================================================================ dialog (themed, replaces prompt/confirm)
function ask({ title, body = "", ok = "OK", label = "", value = null, danger = false }) {
  return new Promise((resolve) => {
    const d = $("#nx-dialog");
    $("#dlg-title").textContent = title;
    $("#dlg-body").textContent = body;
    $("#dlg-body").hidden = !body;
    $("#dlg-field").hidden = value === null;
    $("#dlg-label").textContent = label;
    $("#dlg-input").value = value ?? "";
    $("#dlg-ok").textContent = ok;
    $("#dlg-ok").classList.toggle("danger", danger);
    d.returnValue = "";
    d.addEventListener("close", () => resolve(d.returnValue === "ok" ? (value === null ? true : $("#dlg-input").value.trim()) : null), { once: true });
    d.showModal();
    // Confirmations start on Cancel, so Enter never triggers the destructive choice.
    (value === null ? $("#dlg-cancel") : $("#dlg-input")).focus();
    if (value !== null) $("#dlg-input").select();
  });
}

// ================================================================ toast
let toastTimer = 0;
function toast(text) {
  const t = $("#toast");
  t.textContent = text;
  t.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (t.hidden = true), 3200);
}

// ================================================================ terminal
const MONO = '"Spline Sans Mono", Consolas, monospace';
const termSize = () => (NARROW.matches ? 11.5 : 13);
const term = new Terminal({
  fontFamily: MONO,
  fontSize: termSize(),
  lineHeight: 1.3,
  cursorBlink: !STILL,
  cursorStyle: "bar",
  scrollback: 5000,
  allowTransparency: true,
  theme: {
    background: "rgba(0,0,0,0)", foreground: "#eef0ff", cursor: "#8b9cff", cursorAccent: "#0a0e1a",
    selectionBackground: "#3a4378",
    // The kernel shell prints basic ANSI colours; NEXUS gives them its palette.
    black: "#1a1f36", brightBlack: "#8c94b4",
    red: "#ff8a65", brightRed: "#ffa284",
    green: "#6ee7b7", brightGreen: "#9af0cd",
    yellow: "#ffc46b", brightYellow: "#ffd694",
    blue: "#8b9cff", brightBlue: "#aab6ff",
    magenta: "#c084fc", brightMagenta: "#d4a8fd",
    cyan: "#5ed3f3", brightCyan: "#8de0f7",
    white: "#eef0ff", brightWhite: "#ffffff",
  },
});
const fit = new FitAddon.FitAddon();
term.loadAddon(fit);
term.open($("#terminal"));
const refit = () => { try { fit.fit(); } catch (_) { /* hidden */ } };
// A cell measured while hidden or before the font arrived is wrong; re-measure when shown.
function remeasure() {
  term.options.fontFamily = "monospace";
  term.options.fontFamily = MONO;
  refit();
}
document.fonts.load(`${termSize()}px "Spline Sans Mono"`).then(remeasure);
NARROW.addEventListener("change", () => { term.options.fontSize = termSize(); refit(); });

let pending = "", inputTimer = null;
function sendInput(text) {
  pending += text;
  if (!inputTimer) {
    inputTimer = setTimeout(() => {
      const body = pending;
      pending = "";
      inputTimer = null;
      fetch("/api/input", { method: "POST", body });
    }, 8);
  }
}
term.onData(sendInput);
function runCommand(cmd) {
  openApp("terminal");
  sendInput(cmd + "\r");
  term.focus();
}

const FKEYS = { F1: "help", F2: "sysinfo", F3: "ps", F4: "mem" };
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && document.body.classList.contains("booting")) { e.preventDefault(); boot.finish(); return; }
  if (e.key === "Escape" && !$("#launcher").hidden) { e.preventDefault(); closeLauncher(true); return; }
  if (e.key === "Escape" && !$("#ctxmenu").hidden) { e.preventDefault(); closeCtx(); return; }
  if (e.key === "Escape" && !$("#tour").hidden && !e.target.closest(".xterm")) { e.preventDefault(); tour.end(); return; }
  if (FKEYS[e.key]) { e.preventDefault(); runCommand(FKEYS[e.key]); }
}, true);
term.attachCustomKeyEventHandler((e) => {
  if (FKEYS[e.key]) return false;
  if (e.key === "Tab" && e.shiftKey) {
    if (e.type === "keydown") { e.preventDefault(); term.blur(); $('.dock-app[data-app="terminal"]').focus(); }
    return false;
  }
  return true;
});
$$(".sb-keys button").forEach((b) => (b.onclick = () => runCommand(b.dataset.key)));

// ================================================================ window manager
const APPS = [
  ["terminal", "Terminal", "i-terminal"],
  ["code", "Code", "i-code"],
  ["files", "Files", "i-files"],
  ["processes", "Processes", "i-processes"],
  ["memory", "Memory", "i-memory"],
  ["syscalls", "System calls", "i-syscall"],
  ["journal", "Journal", "i-journal"],
  ["topo", "Kernel Map", "i-topo"],
  ["research", "Research", "i-research"],
  ["system", "System", "i-system"],
];
const TITLE = Object.fromEntries(APPS.map(([a, t]) => [a, t]));
const ws = $("#workspace");
const STORE = "nexus.desktop.v1";
let geo = {};
try { geo = JSON.parse(localStorage.getItem(STORE)) || {}; } catch (_) { geo = {}; }
const save = () => { try { localStorage.setItem(STORE, JSON.stringify(geo)); } catch (_) { /* private mode */ } };
const win = (app) => $(`#win-${app}`);
let zTop = 10, focused = null, cascade = 0;

function defaultGeo(app) {
  const W = ws.clientWidth, H = ws.clientHeight, m = 22, band = 96;   // band: the dock
  const usable = H - band;
  if (app === "terminal") return { x: m, y: m, w: Math.min(Math.round(W * 0.47), 720), h: Math.min(usable - 2 * m, 520) };
  if (app === "journal") { const w = Math.min(330, Math.round(W * 0.22)); return { x: W - w - m, y: m, w, h: usable - 2 * m }; }
  if (app === "code") return { x: Math.round(W * 0.05), y: m, w: Math.max(560, Math.min(W - 3 * m - 340, 980)), h: usable - 2 * m };
  if (app === "topo") return { x: Math.round(W * 0.06), y: m, w: Math.min(W - 2 * m, 1100), h: Math.min(usable - 2 * m, 680) };
  if (app === "syscalls") return { x: Math.round(W * 0.2), y: m, w: Math.min(W - 2 * m, 820), h: usable - 2 * m };
  if (app === "research") return { x: Math.round(W * 0.1), y: m, w: Math.min(W - 2 * m, 980), h: usable - 2 * m };
  const w = Math.min(W - 2 * m, app === "files" || app === "processes" ? 900 : 760);
  const h = Math.min(usable - 2 * m, 600);
  const step = (cascade++ % 6) * 26;
  return { x: Math.min(W - w - m, 110 + step), y: Math.min(usable - h - m, 40 + step), w, h };
}

function clampGeo(src) {
  const g = { ...src };
  const W = ws.clientWidth, H = ws.clientHeight;
  g.w = Math.min(Math.max(g.w, 300), W);
  g.h = Math.min(Math.max(g.h, 160), H);
  g.x = Math.min(Math.max(g.x, 80 - g.w), W - 80);
  g.y = Math.min(Math.max(g.y, 0), H - 44);
  g.x = Math.min(g.x, Math.max(0, W - g.w));      // never past the right edge after a resize
  return g;
}

function place(app) {
  const g = clampGeo(geo[app]);
  const el = win(app);
  Object.assign(el.style, { left: `${g.x}px`, top: `${g.y}px`, width: `${g.w}px`, height: `${g.h}px` });
  el.classList.toggle("max", !!geo[app].max);
}

const isShown = (app) => {
  const el = win(app);
  return !el.hidden && !el.classList.contains("min") && (!NARROW.matches || app === focused);
};

function openApp(app) {
  const el = win(app);
  geo[app] = geo[app] && geo[app].w ? geo[app] : defaultGeo(app);
  geo[app].open = true;
  geo[app].min = false;
  el.hidden = false;
  el.classList.remove("min");
  place(app);
  focusWin(app, true);
  onShow(app);
  save();
}

function focusWin(app, moveFocus) {
  const el = win(app);
  el.style.zIndex = ++zTop;
  focused = app;
  $$(".win").forEach((w) => w.classList.toggle("focused", w === el));
  $("#app-name").textContent = TITLE[app];
  updateDock();
  if (typeof renderMenus === "function") renderMenus();
  if (app === "terminal") term.focus();
  else if (moveFocus && !el.contains(document.activeElement)) el.focus();
  requestAnimationFrame(() => {
    if (app === "terminal") remeasure();
    drawCharts();
  });
  ground.kick();
}

function topVisible() {
  let best = null, z = -1;
  for (const [app] of APPS) {
    const el = win(app);
    if (el.hidden || el.classList.contains("min")) continue;
    const zi = Number(el.style.zIndex) || 0;
    if (zi > z) { z = zi; best = app; }
  }
  return best;
}

function afterHide() {
  const next = topVisible();
  if (next) focusWin(next, true);
  else {
    focused = null;
    $("#app-name").textContent = "Desktop";
    $$(".win").forEach((w) => w.classList.remove("focused"));
    if (typeof renderMenus === "function") renderMenus();
    updateDock();
    $(".dock-app").focus();
  }
  ground.kick();
}
function minimize(app) { geo[app].min = true; win(app).classList.add("min"); save(); afterHide(); }
function closeWin(app) { geo[app].open = false; geo[app].min = false; win(app).hidden = true; save(); afterHide(); }
function toggleMax(app) { geo[app].max = !geo[app].max; place(app); save(); focusWin(app); ground.kick(); }

function updateDock() {
  $$(".dock-app[data-app]").forEach((b) => {
    const el = win(b.dataset.app);
    const open = !el.hidden;
    b.classList.toggle("open", open);
    b.classList.toggle("focused", b.dataset.app === focused && open && !el.classList.contains("min"));
    b.setAttribute("aria-pressed", String(b.classList.contains("focused")));
  });
}

function onShow(app) {
  if (app === "tests" || app === "system") loadResults();
  if (app === "files" && !files.loaded) files.go("");
  if (app === "code" && !code.loaded) code.init();
  if (app === "topo") ensureTopo();
  if (app === "research") loadResearch();
}

// Dock and launcher come from APPS so they never disagree.
const icon = (id) => `<svg aria-hidden="true"><use href="#${id}"/></svg>`;
// Original app icons: a glyph on a tile of the app's own colour.
const TILE = { terminal: ["#3a4160", "#141828"], code: ["#7d8bff", "#3d47d9"], files: ["#5ab8ff", "#1f74d1"], processes: ["#ff9f6b", "#e0553a"],
  memory: ["#5fe3b3", "#1b9b75"], syscalls: ["#c48cfc", "#7535e8"], journal: ["#ffd46e", "#df951c"], topo: ["#5ed3f3", "#2585b3"],
  research: ["#f67ab9", "#c0256f"], system: ["#a3abc0", "#59617a"] };
$("#dock").innerHTML = APPS.map(([app, title, ic], i) =>
  `${i === 4 || i === 8 ? '<i class="dock-sep" aria-hidden="true"></i>' : ""}<button class="dock-app" data-app="${app}" aria-label="${title}" aria-pressed="false" style="--c1:${TILE[app][0]};--c2:${TILE[app][1]}"><span class="tile">${icon(ic)}</span><span class="tip">${title}</span></button>`).join("");
// Magnification: icons near the pointer grow, like a real dock.
(function magnify() {
  const dock = $("#dock");
  if (STILL || NARROW.matches) return;
  dock.addEventListener("pointermove", (e) => {
    for (const b of dock.querySelectorAll(".dock-app")) {
      const r = b.getBoundingClientRect();
      const d = Math.abs(e.clientX - (r.left + r.width / 2));
      const k = Math.max(0, 1 - d / 140);
      b.style.transform = `scale(${1 + 0.42 * k * k})`;
      b.style.margin = `0 ${10 * k * k}px`;
    }
  });
  dock.addEventListener("pointerleave", () => dock.querySelectorAll(".dock-app").forEach((b) => { b.style.transform = ""; b.style.margin = ""; }));
})();

$$(".dock-app[data-app]").forEach((b) => (b.onclick = () => {
  const app = b.dataset.app, el = win(app);
  if (el.hidden || el.classList.contains("min")) openApp(app);
  else if (focused === app) minimize(app);
  else focusWin(app, true);
}));

async function power() {
  if (!running) { bootMachine(); return; }
  const yes = await ask({ title: "Power off the machine?", body: "The kernel stops and the terminal session ends. You can boot it again from the dock or the NEXUS menu.", ok: "Power off", danger: true });
  if (yes) fetch("/api/stop", { method: "POST" });
}

$$(".win").forEach((el) => {
  const app = el.dataset.app;
  el.addEventListener("pointerdown", () => { if (focused !== app) focusWin(app); }, true);
  el.querySelectorAll("[data-win]").forEach((b) => (b.onclick = (e) => {
    e.stopPropagation();
    ({ min: minimize, max: toggleMax, close: closeWin })[b.dataset.win](app);
  }));
  const bar = el.querySelector(".win-bar");
  bar.addEventListener("dblclick", (e) => { if (!e.target.closest("button") && !NARROW.matches) toggleMax(app); });
  bar.addEventListener("pointerdown", (e) => {
    if (e.button !== 0 || e.target.closest("button") || geo[app].max || NARROW.matches) return;
    const g = geo[app], sx = e.clientX, sy = e.clientY, ox = g.x, oy = g.y;
    bar.setPointerCapture(e.pointerId);
    el.classList.add("dragging");
    const move = (ev) => { g.x = ox + ev.clientX - sx; g.y = oy + ev.clientY - sy; place(app); ground.kick(); };
    const up = () => { el.classList.remove("dragging"); bar.removeEventListener("pointermove", move); Object.assign(g, clampGeo(g)); save(); };
    bar.addEventListener("pointermove", move);
    bar.addEventListener("pointerup", up, { once: true });
    bar.addEventListener("pointercancel", up, { once: true });
  });
  const grip = el.querySelector(".win-grip");
  grip.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    const g = geo[app], sx = e.clientX, sy = e.clientY, ow = g.w, oh = g.h;
    grip.setPointerCapture(e.pointerId);
    const move = (ev) => { g.w = ow + ev.clientX - sx; g.h = oh + ev.clientY - sy; place(app); ground.kick(); };
    const up = () => { grip.removeEventListener("pointermove", move); Object.assign(g, clampGeo(g)); save(); };
    grip.addEventListener("pointermove", move);
    grip.addEventListener("pointerup", up, { once: true });
    grip.addEventListener("pointercancel", up, { once: true });
  });
});

let redraw = 0;
const ro = new ResizeObserver((entries) => {
  const t = entries.find((e) => e.target.id === "win-terminal");
  if (t && t.contentRect.width) remeasure();
  cancelAnimationFrame(redraw);
  redraw = requestAnimationFrame(drawCharts);
});
$$(".win").forEach((el) => ro.observe(el));
window.addEventListener("resize", () => { for (const [app] of APPS) if (geo[app] && !win(app).hidden) place(app); });

// launcher
function closeLauncher(refocus) {
  $("#launcher").hidden = true;
  $("#launcher-btn").setAttribute("aria-expanded", "false");
  if (refocus) $("#launcher-btn").focus();
}
$("#launcher-btn").onclick = (e) => {
  e.stopPropagation();
  const open = $("#launcher").hidden;
  $("#launcher").hidden = !open;
  $("#launcher-btn").setAttribute("aria-expanded", String(open));
  if (open) $("#launcher button").focus();
};
document.addEventListener("pointerdown", (e) => {
  if (!$("#launcher").hidden && !e.target.closest("#launcher, #launcher-btn")) closeLauncher(false);
});
$("#launcher").addEventListener("click", (e) => {
  const b = e.target.closest("button");
  if (!b) return;
  closeLauncher(false);
  if (b.dataset.app) openApp(b.dataset.app);
  if (b.dataset.action === "reboot") { openApp("system"); selectTab("machine"); $("#policy").focus(); }
  if (b.dataset.action === "tour") tour.start();
  if (b.dataset.action === "about") { openApp("system"); selectTab("about"); }
  if (b.dataset.action === "mission") mission(true);
  if (b.dataset.action === "lock") lock.show();
  if (b.dataset.action === "poweroff") power();
});
$("#launcher").addEventListener("keydown", (e) => {
  if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
  e.preventDefault();
  const items = [...$$("#launcher button")];
  const i = items.indexOf(document.activeElement);
  items[(i + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length].focus();
});

// clock: the viewer's local time
function tick() {
  const d = new Date();
  const hm = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const big = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
  const date = d.toLocaleDateString([], { weekday: "long", day: "numeric", month: "long" });
  $("#clock").innerHTML = `<span class="clock-d">${d.toLocaleDateString([], { weekday: "short", day: "numeric", month: "short" })}</span> ${hm}`;
  $("#clock").dateTime = d.toISOString();
  $("#desk-time").textContent = big;
  $("#desk-date").textContent = date;
  $("#lock-clock").textContent = big;
  $("#lock-date").textContent = date;
}
tick();
setInterval(tick, 5000);

// ================================================================ topography
const visibleRects = () => [...$$(".win")].filter((w) => !w.hidden && !w.classList.contains("min") && !w.classList.contains("max"))
  .map((w) => w.getBoundingClientRect());
const groundCovered = () => [...$$(".win")].some((w) => {
  if (w.hidden || w.classList.contains("min")) return false;
  if (w.classList.contains("max") || NARROW.matches) return true;
  // A focused window over ~40% of the desktop hides most of the sky: stop re-blurring it.
  return w.classList.contains("focused") && w.offsetWidth * w.offsetHeight > 0.4 * ws.clientWidth * ws.clientHeight;
});
const ground = new Aurora($("#wallpaper"), { covered: groundCovered });
try { ground.setMotion(localStorage.getItem("nexus.motion") !== "off"); } catch (_) { /* private mode */ }
let topo = null;
function ensureTopo() {
  if (!topo) topo = new Terrain($("#topo-canvas"), { heapBand: 32, scale: 1.08 });
  topo.setPeaks(currentPeaks, running ? "" : "machine off");
  if (lastHeap) topo.setHeap(lastHeap.map);
}

let currentPeaks = [];
let userTids = new Set();
let lastHeap = null;

// ================================================================ boot: real BOOT trace, drawn as contours
const hex = (v) => (v ? String(v).toLowerCase() : "–");
const mib = (b) => `${Math.round(Number(b) / 1048576)} MiB`;
const BOOT_STAGES = [
  ["entry", "hart entry", (e) => `hart ${e.hart} · dtb ${hex(e.dtb)}`],
  ["args", "firmware", (e) => (e.fw ? `${e.fw} · SBI v${e.sbi}` : "command line read")],
  ["trap", "trap vector", () => "installed"],
  ["heap", "kernel heap", (e) => `${mib(e.size)} at ${hex(e.start)}`],
  ["timer", "timer", (e) => `${e.tick_us} µs tick`],
  ["sched", "scheduler", (e) => `${POLICY_SHORT[pkey(e.policy)] || e.policy} · q=${e.q}`],
  ["shell", "shell", () => "ready"],
];
const bootTerrain = new Aurora($("#boot-canvas"), { scale: 10 });

const boot = {
  events: new Map(), shown: 0, queue: null, done: false, watchdog: null,
  start() {
    this.events.clear();
    this.shown = 0;
    this.done = false;
    $("#boot-title").textContent = "Starting the kernel";
    $("#boot-rows").innerHTML = BOOT_STAGES.map(([id, label]) =>
      `<li data-stage="${id}"><span class="k">${label}</span><span class="v">·</span><span class="t"></span></li>`).join("");
    bootTerrain.setPeaks([]);
    document.body.classList.add("booting");
    $(".app").inert = true;
    clearTimeout(this.watchdog);
    this.watchdog = setTimeout(() => this.finish(), 12000);
  },
  add(ev) {
    if (this.done) return;
    this.events.set(ev.stage, ev);
    if (!this.queue) this.step();
  },
  step() {
    const next = BOOT_STAGES[this.shown];
    if (!next || !this.events.has(next[0])) { this.queue = null; return; }
    const [id, , fmt] = next;
    const ev = this.events.get(id);
    const rows = $$("#boot-rows li");
    rows.forEach((li, i) => { li.classList.toggle("done", i < this.shown); li.classList.toggle("active", i === this.shown); });
    const li = rows[this.shown];
    li.querySelector(".v").textContent = fmt(ev);
    li.querySelector(".t").textContent = `${(ev.t / 1000).toFixed(2)} ms`;
    $("#boot-title").textContent = `${BOOT_STAGES[this.shown][1]}: ${fmt(ev)}`;
    this.shown++;
    // Each stage the kernel reports raises one more summit in the survey.
    bootTerrain.setPeaks(BOOT_STAGES.slice(0, this.shown).map(([sid], i) => ({ id: i + 1, label: sid, sub: "", state: i === this.shown - 1 ? "RUNNING" : "READY", h: 0.55 + 0.06 * i, weight: 0.5 })));
    if (this.shown === BOOT_STAGES.length) {
      $("#boot-title").textContent = "Kernel ready";
      this.queue = setTimeout(() => this.finish(), STILL ? 300 : 1000);
      return;
    }
    this.queue = setTimeout(() => { this.queue = null; this.step(); }, STILL ? 0 : 300);
  },
  finish() {
    clearTimeout(this.queue);
    clearTimeout(this.watchdog);
    this.queue = null;
    this.done = true;
    document.body.classList.remove("booting");
    $(".app").inert = false;
    remeasure();
    ground.kick();
    if (!lock.seen) lock.show();
    else if (focused === "terminal") term.focus();
  },
};
$("#boot-skip").onclick = () => boot.finish();

// ================================================================ events
function connect() {
  const es = new EventSource("/api/events");
  let first = true;
  es.onopen = () => $("#conn-dot").classList.add("on");
  es.onmessage = (e) => {
    const m = JSON.parse(e.data);
    switch (m.type) {
      case "status":
        if (first) { first = false; m.running ? boot.start() : boot.finish(); }
        setStatus(m);
        break;
      case "boot": boot.add(m); break;
      case "term": term.write(m.data); break;
      case "reset": term.reset(); resetLive(); boot.start(); break;
      case "snap": onSnapshot(m); break;
      case "flow": onFlow(m); break;
      case "heapmap": onHeapMap(m); break;
      case "user": onUser(m); break;
      case "journal": journal.add(m, true); break;
      case "journal_replay": m.entries.forEach((j) => journal.add(j, false)); break;
      case "testlog": onTestLog(m); break;
      case "testdone": onTestDone(m); break;
      case "buildlog": code.log(m); break;
      case "builddone": code.done(m); break;
    }
  };
  es.onerror = () => { $("#conn-dot").classList.remove("on"); setStatus({ running: false, lost: true }); };
}

function setStatus(s) {
  running = !!s.running;
  markStale(!running, s.lost ? "Disconnected from the kernel: last values seen" : "Machine powered off: last values seen");
  const st = $("#machine-state");
  st.classList.toggle("on", running);
  st.querySelector("b").textContent = s.lost ? "Lost" : running ? "Running" : "Off";
  $("#power-state").textContent = s.lost ? "front-end server unreachable" : running ? `running · ${POLICY_NAMES[s.policy] || s.policy}, q=${s.quantum}` : "powered off";
  if (running) {
    $("#policy").value = s.policy;
    $("#quantum").value = s.quantum;
    $("#k-policy").textContent = `${POLICY_SHORT[s.policy] || s.policy} · q=${s.quantum}`;
  } else {
    $("#k-policy").textContent = "–";
    if (!s.lost && document.body.classList.contains("booting")) boot.finish();
  }
  $("#reboot").textContent = running ? "Reboot" : "Boot";
  $("#poweroff").disabled = !running;
  $("#launcher-power span").textContent = running ? "Power off" : "Boot the machine";
  if (!$("#cc").hidden) cc.sync();
}

function markStale(stale, why) {
  document.body.classList.toggle("stale", stale);
  if (!stale) return;
  $("#k-cpu").textContent = "–"; $("#k-cpu-bar").style.width = "0";
  $("#k-events").textContent = "–";
  $("#task-rows").innerHTML = `<tr class="empty-row"><td colspan="10">${why}</td></tr>`;
  currentPeaks = [];
  ground.setPeaks([], "machine off");
  topo?.setPeaks([], "machine off");
}

function resetLive() {
  snaps = [];
  $("#task-rows").innerHTML = "";
  drawCharts();
}

// ================================================================ snapshots
const size = (b) => (b >= 1048576 ? `${(b / 1048576).toFixed(1)} MiB` : b >= 10240 ? `${(b / 1024).toFixed(1)} KiB` : `${b} B`);
function dl(sel, rows) {
  $(sel).innerHTML = rows.map(([k, v]) => `<dt>${k}</dt><dd>${typeof v === "number" ? v.toLocaleString() : v}</dd>`).join("");
}

function onSnapshot(s) {
  if (!running) return;
  document.body.classList.remove("stale");
  snaps.push(s);
  const horizon = s.uptime_us - (HISTORY_SECONDS + 2) * 1e6;
  while (snaps.length > 2 && snaps[0].uptime_us < horizon) snaps.shift();
  const prev = snaps.length > 1 ? snaps[snaps.length - 2] : null;
  const dt = prev ? s.ticks - prev.ticks : 0;
  const prevCpu = new Map((prev ? prev.tasks : []).map((t) => [t.tid, t.cpu]));
  const shareOf = (t) => (dt > 0 ? Math.max(0, (t.cpu - (prevCpu.get(t.tid) ?? t.cpu)) / dt) : 0);
  const pol = pkey(s.policy);

  const sec = Math.floor(s.uptime_us / 1e6);
  // system bar
  const busy = prev && dt > 0 ? Math.max(0, 1 - (s.idle - prev.idle) / dt) : 0;
  $("#k-cpu").textContent = `${Math.round(busy * 100)}%`;
  $("#k-cpu-bar").style.width = `${busy * 100}%`;
  const heap = s.heap_used / s.heap_cap;
  $("#k-heap").textContent = heap < 0.1 ? `${(heap * 100).toFixed(1)}%` : `${Math.round(heap * 100)}%`;
  $("#k-heap-bar").style.width = `${Math.max(heap * 100, 1)}%`;
  $("#k-tick").textContent = s.ticks.toLocaleString();
  $("#k-tasks").textContent = s.tasks.length;
  $("#k-policy").textContent = `${POLICY_SHORT[pol] || s.policy} · q=${s.q}`;

  userTids = new Set(s.user_tids || []);
  // topography: one peak per task
  const flowDisp = lastFlow ? lastFlow.disp : {};
  const totalDisp = Object.values(flowDisp).reduce((a, b) => a + b, 0) || 1;
  updateLeads(s.tasks.map((t) => [t.tid, shareOf(t)]));
  currentPeaks = s.tasks.map((t) => {
    const share = shareOf(t);
    return {
      id: t.tid, label: t.name, state: t.state, rgb: rgbOf(colorOf(t.tid)), lead: leadSlot.has(t.tid),
      sub: `pid ${t.tid}${userTids.has(t.tid) ? " · U-mode" : ""} · ${Math.round(share * 100)}%`,
      user: userTids.has(t.tid),
      h: 0.2 + 0.8 * share, weight: 0.15 + 0.85 * ((flowDisp[t.tid] || 0) / totalDisp),
    };
  });
  ground.setPeaks(currentPeaks);
  topo?.setPeaks(currentPeaks);
  widgets.onSnap(s, busy, heap, sec);

  // processes
  $("#sched-line").textContent = `${POLICY_NAMES[pol] || s.policy} · quantum ${s.q} ticks`;
  $("#task-rows").innerHTML = s.tasks.map((t) => {
    const p = Math.round(shareOf(t) * 100);
    const killable = t.name !== "shell" && t.name !== "monitor";
    return `<tr>
      <td class="num">${t.tid}</td>
      <td class="name"><i class="swatch" style="background:${colorOf(t.tid)}"></i>${esc(t.name)}</td>
      <td><span class="status ${t.state}">${t.state.toLowerCase()}</span></td>
      <td class="num">${t.prio}</td><td class="num">${t.level}</td>
      <td class="num">${t.cpu.toLocaleString()}</td><td class="num">${t.disp.toLocaleString()}</td>
      <td class="num">${(flowDisp[t.tid] || 0).toLocaleString()}</td>
      <td><div class="share"><div class="bar"><i style="width:${p}%"></i></div><span>${p}%</span></div></td>
      <td>${killable ? `<button class="kill" data-tid="${t.tid}" aria-label="stop ${esc(t.name)}, pid ${t.tid}">Stop</button>` : ""}</td>
    </tr>`;
  }).join("");
  $$(".kill").forEach((b) => (b.onclick = () => runCommand(`kill ${b.dataset.tid}`)));

  // memory
  $("#heap-pct").textContent = `${size(s.heap_used)} of ${size(s.heap_cap)} in use`;
  dl("#heap-stats", [
    ["in use", size(s.heap_used)], ["peak", size(s.heap_peak)], ["free blocks", s.free_blocks],
    ["largest free block", size(s.largest)], ["fragmentation", `${(s.frag_bp / 100).toFixed(2)}%`],
    ["allocations", s.allocs], ["frees", s.frees],
  ]);

  // system
  $("#sched-policy").textContent = POLICY_NAMES[pol] || s.policy;
  dl("#sched-stats", [
    ["timer ticks", s.ticks], ["idle ticks", s.idle], ["dispatches", s.disp], ["preemptions", s.pre],
    ["yields", s.yld], ["blocks", s.blk], ["wake-ups", s.wake], ["ready queue", s.ready], ["tick length", `${s.tick_us} µs`],
  ]);

  drawCharts();
}

const OFF_LABEL = { preempt: "preempted", yield: "yielded the CPU", block: "blocked (sleep, mutex or semaphore)", exit: "exited" };
function onFlow(f) {
  const prevT = lastFlow ? lastFlow.t : null;
  lastFlow = f;
  ground.pushFlow(f);
  topo?.pushFlow(f);
  sys.onFlow(f);
  widgets.onFlow(f);
  const secs = prevT ? (f.t - prevT) / 1e6 : 0.25;
  const rate = secs > 0 ? Math.round(f.events / secs) : 0;
  $("#k-events").textContent = rate.toLocaleString();
  const off = Object.entries(f.off || {});
  dl("#off-reasons", off.length ? off.sort((a, b) => b[1] - a[1]).map(([k, n]) => [OFF_LABEL[k] || k, n]) : [["no task left the CPU", "–"]]);
  const sum = (o) => Object.values(o || {}).reduce((a, b) => a + b, 0);
  const per = (n) => (secs > 0 ? `${Math.round(n / secs).toLocaleString()} /s` : "–");
  dl("#topo-rates", [
    ["dispatches", per(sum(f.disp))],
    ["task wake-ups", per((f.wakes || []).reduce((a, w) => a + w[2], 0))],
    ["timer wake-ups", per(sum(f.timer_wakes))],
    ["preemptions", per((f.off || {}).preempt || 0)],
    ["trace events", per(f.events)],
  ]);
}

function onHeapMap(m) {
  lastHeap = m;
  widgets.onHeap(m);
  ground.setHeap(m.map);
  topo?.setHeap(m.map);
  const grid = $("#heapmap");
  if (grid.children.length !== m.map.length) grid.innerHTML = m.map.map(() => "<i></i>").join("");
  const cells = grid.children;
  const used = m.map.filter((v) => v > 0).length;
  m.map.forEach((v, i) => {
    const k = v / 255;
    cells[i].style.backgroundColor = v === 0 ? "" : `rgba(185,213,196,${0.25 + 0.75 * k})`;
    cells[i].title = `slice ${i}: ${Math.round(k * 100)}% used`;
  });
  grid.setAttribute("aria-label", `Heap map: ${used} of ${m.map.length} slices hold allocations`);
  $("#heapmap-hint").textContent = `${m.slices} slices of ${size(m.slice_bytes)} · ${used} in use · kernel t=${(m.t / 1e6).toFixed(1)} s`;
}

// ================================================================ charts
function setupCanvas(canvas) {
  const dpr = window.devicePixelRatio || 1;
  canvas.dataset.h ??= canvas.getAttribute("height");
  const w = canvas.clientWidth, h = Number(canvas.dataset.h);
  if (!w) return null;
  canvas.width = w * dpr; canvas.height = h * dpr; canvas.style.height = `${h}px`;
  const ctx = canvas.getContext("2d");
  ctx.scale(dpr, dpr);
  return { ctx, w, h };
}
function axes(ctx, w, h, labels) {
  const g = { padL: 58, padR: 6, padT: 8, padB: 22 };
  g.cw = w - g.padL - g.padR; g.ch = h - g.padT - g.padB;
  ctx.font = '13px "Spline Sans Mono", monospace';
  ctx.fillStyle = css("--slate"); ctx.strokeStyle = "rgba(130,148,154,0.22)"; ctx.lineWidth = 1;
  labels.forEach(([frac, text]) => {
    const y = Math.round(g.padT + g.ch - frac * g.ch) + 0.5;
    ctx.setLineDash(frac === 0 ? [] : [2, 4]);
    ctx.beginPath(); ctx.moveTo(g.padL, y); ctx.lineTo(g.padL + g.cw, y); ctx.stroke();
    ctx.fillText(text, 0, y + 4);
  });
  ctx.setLineDash([]);
  ctx.fillText(`-${HISTORY_SECONDS}s`, g.padL, h - 4);
  ctx.textAlign = "right"; ctx.fillText("now", g.padL + g.cw, h - 4); ctx.textAlign = "left";
  return g;
}
function timeX(g) {
  const end = snaps.length ? snaps[snaps.length - 1].uptime_us : 0;
  const start = end - HISTORY_SECONDS * 1e6;
  return (t) => g.padL + ((t - start) / (end - start)) * g.cw;
}
function drawCharts() {
  if (isShown("processes")) drawCpu();
  if (isShown("memory")) drawLine("#heap-chart", (s) => s.heap_used, size, css("--celadon"));
  if (isShown("system") && !$("#sys-machine").hidden) drawSwitches();
}
function drawCpu() {
  const c = setupCanvas($("#cpu-chart"));
  if (!c) return;
  const { ctx, w, h } = c;
  const g = axes(ctx, w, h, [[0, "0%"], [0.5, "50%"], [1, "100%"]]);
  if (snaps.length < 2) return;
  const xOf = timeX(g);
  const frames = [], tids = new Set(), names = new Map();
  for (let i = 1; i < snaps.length; i++) {
    const a = snaps[i - 1], b = snaps[i], dt = b.ticks - a.ticks;
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
    ctx.fillStyle = colorOf(tid) + "c8";
    ctx.fill();
    frames.forEach((f, i) => (base[i] += f.shares.get(tid) || 0));
  }
  $("#legend").innerHTML = order.map((tid) => `<span><i style="background:${colorOf(tid)}"></i>${esc(names.get(tid))} (${tid})</span>`).join("") ||
    "<span>no task used the CPU in the last 60 s</span>";
}
function drawLine(sel, value, fmt, color, points) {
  const c = setupCanvas($(sel));
  if (!c) return;
  const { ctx, w, h } = c;
  const pts = points || snaps.map((s) => ({ t: s.uptime_us, v: value(s) }));
  const max = Math.max(1, ...pts.map((p) => p.v)) * 1.15;
  const g = axes(ctx, w, h, [[0, fmt(0)], [0.5, fmt(Math.round(max / 2))], [1, fmt(Math.round(max))]]);
  if (pts.length < 2) return;
  const xOf = timeX(g), yOf = (v) => g.padT + g.ch - (v / max) * g.ch;
  ctx.beginPath();
  pts.forEach((p, i) => (i ? ctx.lineTo(xOf(p.t), yOf(p.v)) : ctx.moveTo(xOf(p.t), yOf(p.v))));
  ctx.lineTo(xOf(pts[pts.length - 1].t), g.padT + g.ch); ctx.lineTo(xOf(pts[0].t), g.padT + g.ch); ctx.closePath();
  ctx.fillStyle = color + "22"; ctx.fill();
  ctx.beginPath();
  pts.forEach((p, i) => (i ? ctx.lineTo(xOf(p.t), yOf(p.v)) : ctx.moveTo(xOf(p.t), yOf(p.v))));
  ctx.strokeStyle = color; ctx.lineWidth = 1.6; ctx.stroke();
}
function drawSwitches() {
  const pts = [];
  for (let i = 1; i < snaps.length; i++) {
    const a = snaps[i - 1], b = snaps[i], secs = ((b.ticks - a.ticks) * b.tick_us) / 1e6;
    if (secs > 0) pts.push({ t: b.uptime_us, v: (b.disp - a.disp) / secs });
  }
  drawLine("#switch-chart", null, (v) => Math.round(v).toLocaleString(), css("--coral"), pts);
}

// ================================================================ journal
const journal = {
  filter: "all",
  count: 0,
  group(e) {
    if (e.source === "panel") return "panel";
    if (e.kind === "spawn" || e.kind === "exit" || e.kind === "boot" || e.kind === "user" || e.kind === "kill") return "task";
    if (e.kind === "wake") return "wake";
    return "policy";
  },
  add(e, fresh) {
    const list = $("#journal");
    list.querySelector(".empty")?.remove();
    const li = document.createElement("li");
    const g = this.group(e);
    li.className = `k-${e.kind} g-${g}${e.source === "panel" ? " panel" : ""}${fresh && !STILL ? " fresh" : ""}`;
    li.dataset.group = g;
    const when = e.t != null ? `${(e.t / 1e6).toFixed(3)} s` : new Date(e.wall * 1000).toLocaleTimeString([], { hour12: false });
    const title = e.t != null ? "kernel time since power-on" : "time on this computer (panel action)";
    li.innerHTML = `<time title="${title}">${when}</time><i class="dot"></i><span>${esc(e.text)}</span>`;
    li.hidden = this.filter !== "all" && this.filter !== g;
    list.prepend(li);
    while (list.children.length > 400) list.lastElementChild.remove();
    this.count++;
    $("#journal-count").textContent = `${this.count.toLocaleString()} events`;
    if (fresh) notifyFromJournal(e);
  },
  setFilter(f) {
    this.filter = f;
    $$(".chip").forEach((c) => { const on = c.dataset.filter === f; c.classList.toggle("on", on); c.setAttribute("aria-pressed", String(on)); });
    $$("#journal li[data-group]").forEach((li) => (li.hidden = f !== "all" && li.dataset.group !== f));
  },
};
$("#journal").innerHTML = '<li class="empty">Kernel events appear here: tasks spawning and exiting, wake-ups, scheduler decisions, builds and reboots.</li>';
$$(".chip").forEach((c) => (c.onclick = () => journal.setFilter(c.dataset.filter)));

// ================================================================ files (the project on the host)
const files = {
  cwd: "", loaded: false, selected: null,
  async go(path) {
    this.loaded = true;
    const r = await fetch(`/api/ls?path=${encodeURIComponent(path)}`);
    if (!r.ok) { toast("That folder is not available."); return; }
    const d = await r.json();
    this.cwd = d.path;
    const parts = d.path ? d.path.split("/") : [];
    $("#crumbs").innerHTML = `<button data-path="">micro-os</button>` +
      parts.map((p, i) => `<span class="sep">/</span><button data-path="${esc(parts.slice(0, i + 1).join("/"))}">${esc(p)}</button>`).join("");
    $$("#crumbs button").forEach((b) => (b.onclick = () => this.go(b.dataset.path)));
    $("#files-up").disabled = !d.path;
    $("#flist").innerHTML = d.entries.map((e) => `<li><button role="option" aria-selected="false" class="${e.dir ? "dir" : "file"}" data-path="${esc(e.path)}" data-dir="${e.dir ? 1 : 0}">
        <svg><use href="#${e.dir ? "i-folder" : "i-file"}"/></svg><span class="nm">${esc(e.name)}${e.editable ? '<span class="edit-tag">editable</span>' : ""}</span>
        <span class="sz">${e.dir ? "" : size(e.size)}</span></button></li>`).join("") ||
      '<li><span class="hint" style="display:block;padding:10px 12px">empty folder</span></li>';
    $$("#flist button").forEach((b) => (b.onclick = () => (b.dataset.dir === "1" ? this.go(b.dataset.path) : this.open(b.dataset.path, b))));
  },
  async open(path, btn) {
    $$("#flist button").forEach((b) => b.setAttribute("aria-selected", String(b === btn)));
    const r = await fetch(`/api/file?path=${encodeURIComponent(path)}`);
    const pv = $("#fpreview");
    if (!r.ok) { pv.innerHTML = '<p class="empty">This file could not be read.</p>'; return; }
    const f = await r.json();
    const parts = f.path.split("/"), fname = parts.pop();
    const head = `<div class="fhead"><b>${esc(fname)}</b><span class="hint folder">${esc(parts.join("/"))} · ${size(f.size)} · ${f.editable ? "editable program" : "read-only"}</span>${f.editable ? `<button class="btn primary" id="open-in-code">Open in Code</button>` : ""}</div>`;
    if (f.kind === "text") pv.innerHTML = head + `<pre>${esc(f.text)}</pre>`;
    else if (f.kind === "image") pv.innerHTML = head + `<img src="${f.url}" alt="${esc(f.path)}">`;
    else if (f.kind === "toolarge") pv.innerHTML = head + '<p class="empty">Too large to preview here.</p>';
    else pv.innerHTML = head + '<p class="empty">Binary file: no text preview.</p>';
    $("#open-in-code")?.addEventListener("click", () => { openApp("code"); code.load(f.path.split("/").pop().replace(".rs", "")); });
    pv.scrollTop = 0;
  },
};
$("#files-up").onclick = () => files.go(files.cwd.split("/").slice(0, -1).join("/"));

// ================================================================ code: Rust editor + host build
const RUST_KW = new Set("as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while".split(" "));
const RUST_TY = new Set("u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize f32 f64 bool char str String Vec Option Result Box Some None Ok Err".split(" "));
function highlightRust(src, errLines) {
  const out = [];
  const re = /(\/\/[^\n]*|\/\*[\s\S]*?\*\/)|("(?:\\.|[^"\\])*")|('(?:\\.|[^'\\])')|(\b\d[\d_]*(?:\.\d+)?(?:[iu](?:8|16|32|64|128|size)|f32|f64)?\b)|(\b[A-Za-z_]\w*!)|(\b[A-Za-z_]\w*\b)/g;
  let last = 0, m;
  while ((m = re.exec(src))) {
    out.push(esc(src.slice(last, m.index)));
    const [t] = m;
    if (m[1]) out.push(`<span class="co">${esc(t)}</span>`);
    else if (m[2] || m[3]) out.push(`<span class="st">${esc(t)}</span>`);
    else if (m[4]) out.push(`<span class="nu">${esc(t)}</span>`);
    else if (m[5]) out.push(`<span class="ma">${esc(t)}</span>`);
    else if (RUST_KW.has(t)) out.push(`<span class="kw">${t}</span>`);
    else if (RUST_TY.has(t) || /^[A-Z]/.test(t)) out.push(`<span class="ty">${t}</span>`);
    else out.push(t);
    last = re.lastIndex;
  }
  out.push(esc(src.slice(last)));
  const html = out.join("");
  if (!errLines.size) return html + "\n";
  // Mark error lines by wrapping each source line.
  return html.split("\n").map((l, i) => (errLines.has(i + 1) ? `<span class="line-err">${l || " "}</span>` : l)).join("\n") + "\n";
}

const code = {
  loaded: false, name: null, path: null, dirty: false, diags: [], programs: [], building: false,
  async init() {
    this.loaded = true;
    await this.refreshList();
    this.load(this.programs.find((p) => p.name === "hello") ? "hello" : this.programs[0]?.name);
  },
  async refreshList() {
    this.programs = await (await fetch("/api/programs")).json();
    $("#code-prog").innerHTML = this.programs.map((p) => `<option value="${p.name}">${p.name}.rs</option>`).join("");
    if (this.name) $("#code-prog").value = this.name;
  },
  async load(name) {
    if (!name) return;
    if (this.dirty && this.name !== name && !(await ask({ title: `Discard changes to ${this.name}.rs?`, body: "Your edits since the last save will be lost.", ok: "Discard", danger: true }))) { $("#code-prog").value = this.name; return; }
    const path = `kernel/src/programs/${name}.rs`;
    const r = await fetch(`/api/file?path=${encodeURIComponent(path)}`);
    if (!r.ok) { toast(`Could not open ${name}.rs`); return; }
    const f = await r.json();
    this.name = name; this.path = path;
    $("#code-prog").value = name;
    $("#code-path").textContent = path;
    $("#ed-text").value = f.text;
    $("#ed-text").readOnly = !f.editable;
    $("#code-args").value = `${name}${PRESETS[name] ? " " + PRESETS[name] : ""}`;
    this.setDirty(false);
    this.render();
  },
  setDirty(d) {
    this.dirty = d;
    $("#code-save").disabled = !d;
    $("#code-path").textContent = (this.path || "") + (d ? " · unsaved" : "");
  },
  render() {
    const ta = $("#ed-text"), src = ta.value;
    const errLines = new Set(this.diags.filter((d) => d.file === this.path && d.level === "error").map((d) => d.line));
    $("#ed-hl").innerHTML = highlightRust(src, errLines);
    const n = src.split("\n").length;
    $("#ed-gutter").innerHTML = Array.from({ length: n }, (_, i) => (errLines.has(i + 1) ? `<span class="err">${i + 1}</span>` : i + 1)).join("\n");
    const hl = $("#ed-hl");
    ta.style.width = `${Math.max(hl.scrollWidth, $("#ed-scroll").clientWidth)}px`;
    ta.style.height = `${Math.max(hl.scrollHeight, $("#ed-scroll").clientHeight)}px`;
  },
  async save() {
    if (!this.name) return false;
    const r = await fetch("/api/program", { method: "POST", body: JSON.stringify({ name: this.name, source: $("#ed-text").value }) });
    const res = await r.json();
    if (!r.ok) { toast(res.error || "Could not save"); return false; }
    this.setDirty(false);
    toast(`Saved ${res.path}`);
    return true;
  },
  async create() {
    const name = await ask({ title: "New program", body: "It starts from template.rs and is added to kernel/src/programs; Build & run compiles it into the kernel.", label: "Name (lowercase letters, digits, _)", value: "myprog", ok: "Create" });
    if (!name) return;
    const tpl = await (await fetch("/api/file?path=kernel/src/programs/template.rs")).json();
    const source = tpl.text.replace("//! Copy this file to start your own program (see programs/mod.rs).", `//! ${name}: written in NEXUS Code.`).replace(/template \(pid/g, `${name} (pid`);
    const r = await fetch("/api/program", { method: "POST", body: JSON.stringify({ name, source, create: true }) });
    const res = await r.json();
    if (!r.ok) { toast(res.error || "Could not create the program"); return; }
    this.dirty = false;
    await this.refreshList();
    await this.load(name);
    toast(`Created ${res.path}; Build & run to add it to the kernel`);
  },
  async build() {
    if (this.building) return;
    if (this.dirty && !(await this.save())) return;
    const run = $("#code-args").value.trim();
    const r = await fetch("/api/build", { method: "POST", body: JSON.stringify({ run }) });
    const res = await r.json();
    if (!r.ok) { toast(res.error || "Could not start the build"); return; }
    if (!res.started) toast("A build is already running");
  },
  log(m) {
    const log = $("#buildlog");
    if (m.start) {
      this.building = true;
      log.textContent = "";
      this.diags = [];
      $("#diags").innerHTML = "";
      $("#build-state").textContent = "Building on the host…";
      $("#build-state").className = "busy";
      $("#code-build").disabled = true;
      this.render();
    }
    log.textContent += m.line + "\n";
    log.scrollTop = log.scrollHeight;
  },
  done(m) {
    this.building = false;
    $("#code-build").disabled = false;
    this.diags = m.diags || [];
    const errs = this.diags.filter((d) => d.level === "error").length;
    const st = $("#build-state");
    if (m.ok) {
      st.textContent = `Built in ${m.secs} s · rebooting${m.run ? ` and running ${m.run}` : ""}`;
      st.className = "ok";
    } else {
      st.textContent = m.error ? `Build could not start: ${m.error}` : `Build failed: ${errs} error${errs === 1 ? "" : "s"} · the VM keeps the previous kernel`;
      st.className = "fail";
    }
    $("#diags").innerHTML = this.diags.map((d, i) =>
      `<li class="${d.level}"><button data-i="${i}"><span class="lvl">${d.level}${d.code ? " " + d.code : ""}</span><span>${esc(d.file.replace("kernel/src/", ""))}:${d.line}:${d.col} · ${esc(d.msg)}</span></button></li>`).join("");
    $$("#diags button").forEach((b) => (b.onclick = () => this.jump(this.diags[Number(b.dataset.i)])));
    this.render();
    if (m.ok && m.run) { minimize("code"); openApp("terminal"); }
  },
  async jump(d) {
    const name = d.file.match(/programs\/(\w+)\.rs$/)?.[1];
    if (!name) { openApp("files"); toast(`${d.file}:${d.line} is outside the programs folder`); return; }
    if (name !== this.name) await this.load(name);
    const ta = $("#ed-text"), lines = ta.value.split("\n");
    const pos = lines.slice(0, d.line - 1).reduce((a, l) => a + l.length + 1, 0) + Math.max(0, d.col - 1);
    ta.focus();
    ta.setSelectionRange(pos, pos);
    const lh = parseFloat(getComputedStyle(ta).lineHeight) || 21;
    $("#ed-scroll").scrollTop = Math.max(0, (d.line - 4) * lh);
  },
};
$("#code-prog").onchange = (e) => code.load(e.target.value);
$("#code-save").onclick = () => code.save();
$("#code-new").onclick = () => code.create();
$("#code-build").onclick = () => code.build();
$("#ed-text").addEventListener("input", () => { code.setDirty(true); code.render(); });
$("#ed-text").addEventListener("keydown", (e) => {
  if (e.key === "Tab" && !e.shiftKey && !e.target.readOnly) {
    e.preventDefault();
    const ta = e.target, s = ta.selectionStart;
    ta.setRangeText("    ", s, ta.selectionEnd, "end");
    code.setDirty(true); code.render();
  }
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") { e.preventDefault(); code.save(); }
  if ((e.ctrlKey || e.metaKey) && e.key === "Enter") { e.preventDefault(); code.build(); }
});
$("#ed-scroll").addEventListener("scroll", () => { $("#ed-gutter").scrollTop = $("#ed-scroll").scrollTop; });
new ResizeObserver(() => code.loaded && code.render()).observe($("#ed-scroll"));

// ================================================================ system: machine, tests, about
function selectTab(name) {
  $$('.tabs [role="tab"]').forEach((t) => {
    const on = t.id === `tab-${name}`;
    t.setAttribute("aria-selected", String(on));
    $(`#${t.getAttribute("aria-controls")}`).hidden = !on;
  });
  if (name === "tests") loadResults();
  drawCharts();
}
$$('.tabs [role="tab"]').forEach((t) => (t.onclick = () => selectTab(t.id.replace("tab-", ""))));

const bootMachine = () =>
  fetch("/api/boot", { method: "POST", body: JSON.stringify({ policy: $("#policy").value, quantum: Number($("#quantum").value) || 4 }) })
    .then(() => openApp("terminal"));
$("#reboot").onclick = bootMachine;
$("#poweroff").onclick = () => power();

function onTestLog(m) {
  const log = $("#test-log");
  if (m.start) { log.textContent = ""; $("#run-tests").disabled = true; $("#run-tests").textContent = "Running…"; }
  log.textContent += m.line + "\n";
  log.scrollTop = log.scrollHeight;
}
function onTestDone(m) {
  $("#run-tests").disabled = false;
  $("#run-tests").textContent = "Run all tests";
  $("#test-log").textContent += m.ok ? "\nAll passed.\n" : `\nFailed (exit code ${m.code}). Scroll up for the first failing check.\n`;
  loadResults();
}
$("#run-tests").onclick = () => fetch("/api/tests", { method: "POST" });

function loadResults() {
  fetch("/api/results").then((r) => r.json()).then((res) => {
    const passed = res.checks.filter((c) => c.ok).length;
    $("#check-summary").textContent = res.checks.length ? `${passed} of ${res.checks.length} passed` : "no results yet; run the suite";
    $("#checks").innerHTML = res.checks.map((c) => `<div class="check ${c.ok ? "" : "fail"}">${esc(c.id)}</div>`).join("");
    $("#plots").innerHTML = res.plots.map((p) => `<figure><img loading="lazy" src="/results/${p}?v=${res.stamp}" alt="${p.replace(".png", "")}"><figcaption>${p.replace(".png", "")}</figcaption></figure>`).join("");
    $$(".plots figure").forEach((f) => (f.onclick = () => {
      const img = f.querySelector("img");
      $("#lightbox").innerHTML = `<img src="${img.src}" alt="${img.alt}">`;
      $("#lightbox").hidden = false;
    }));
  });
}
$("#lightbox").onclick = () => ($("#lightbox").hidden = true);

fetch("/api/info").then((r) => r.json()).then((i) => {
  info = { ...info, ...i };
  const line = `Micro-OS kernel v${info.version} · ${info.arch} · QEMU virt · ${info.ram_mb} MiB`;
  $("#boot-machine").textContent = line;
  $("#meta-machine").textContent = line;
  $("#about-machine").textContent = `${line} · up to ${info.max_tasks} tasks`;
  $("#lock-machine").textContent = `Micro-OS kernel v${info.version} · ${info.arch} · QEMU virt · 1 hart · ${info.ram_mb} MiB`;
});

// ================================================================ notifications
const notes = {
  list: [], unread: 0,
  push(n) {
    n.at = new Date();
    this.list.unshift(n);
    this.list.length = Math.min(this.list.length, 60);
    if ($("#notif-center").hidden) { this.unread++; this.badge(); }
    this.renderCenter();
    if (document.body.classList.contains("booting") || this.quiet) return;
    const li = document.createElement("li");
    li.className = `notif tone-${n.tone || "info"}`;
    li.innerHTML = `<svg aria-hidden="true"><use href="#${n.icon || "i-bell"}"/></svg><div><b>${esc(n.title)}</b><span>${esc(n.body || "")}</span></div>` +
      `<button class="notif-x" aria-label="Dismiss">${icon("i-close")}</button>`;
    li.querySelector(".notif-x").onclick = (e) => { e.stopPropagation(); li.remove(); };
    if (n.app) { li.classList.add("go"); li.onclick = () => { openApp(n.app); li.remove(); }; }
    const stack = $("#notifs");
    stack.prepend(li);
    while (stack.children.length > 3) stack.lastElementChild.remove();
    setTimeout(() => li.remove(), n.tone === "danger" ? 11000 : 7000);
  },
  badge() {
    const b = $("#bell-count");
    b.hidden = this.unread === 0;
    b.textContent = this.unread > 9 ? "9+" : this.unread;
  },
  renderCenter() {
    $("#notif-list").innerHTML = this.list.map((n) => `<li class="tone-${n.tone || "info"}"><time>${n.at.toLocaleTimeString([], { hour12: false })}</time><div><b>${esc(n.title)}</b><span>${esc(n.body || "")}</span></div></li>`).join("") ||
      '<li class="empty">Nothing yet. Kernel kills, deadlocks, builds and test runs appear here.</li>';
  },
};
notes.renderCenter();
$("#bell").onclick = (e) => {
  e.stopPropagation();
  const c = $("#notif-center"), open = c.hidden;
  c.hidden = !open;
  $("#bell").setAttribute("aria-expanded", String(open));
  if (open) { notes.unread = 0; notes.badge(); }
};
$("#notif-clear").onclick = () => { notes.list = []; notes.renderCenter(); };
document.addEventListener("pointerdown", (e) => {
  if (!$("#notif-center").hidden && !e.target.closest("#notif-center, #bell")) { $("#notif-center").hidden = true; $("#bell").setAttribute("aria-expanded", "false"); }
});

function notifyFromJournal(e) {
  const t = e.text;
  if (e.kind === "kill") notes.push({ tone: "danger", icon: "i-syscall", title: "The kernel stopped a program", body: t, app: "syscalls" });
  else if (e.kind === "deadlock") notes.push({ tone: "danger", icon: "i-processes", title: "Deadlock detected", body: t, app: "journal" });
  else if (e.kind === "build") {
    if (/failed/.test(t)) notes.push({ tone: "danger", icon: "i-code", title: "Build failed", body: t, app: "code" });
    else if (/built in/.test(t)) notes.push({ tone: "ok", icon: "i-code", title: "Kernel rebuilt on the host", body: `${t}; the VM is rebooting with it`, app: "terminal" });
  } else if (e.kind === "tests" && !/started/.test(t)) notes.push({ tone: /passed/.test(t) ? "ok" : "danger", icon: "i-system", title: "Test suite finished", body: t, app: "system" });
  else if (e.kind === "power" && /off/.test(t)) notes.push({ tone: "info", icon: "i-power", title: "Machine powered off", body: "Boot it again from the dock or the NEXUS menu." });
}

// ================================================================ system calls (U-mode)
const ERRNO = { "-14": "EFAULT: bad pointer, refused", "-22": "EINVAL: invalid argument", "-38": "ENOSYS: no such system call", "-130": "interrupted (Ctrl-C)" };
const sys = {
  counts: {}, total: 0, rows: [], procs: new Map(),
  meaning(name, ret) {
    if (ret < 0) return ERRNO[String(ret)] || `error ${ret}`;
    if (name === "write") return `wrote ${ret} byte${ret === 1 ? "" : "s"}`;
    if (name === "getpid") return `pid ${ret}`;
    if (name === "uptime_ms") return `${ret} ms since boot`;
    if (name === "sleep_ms") return "slept";
    if (name === "yield") return "gave up the CPU";
    return "ok";
  },
  onFlow(f) {
    for (const [name, n] of Object.entries(f.syscalls || {})) { this.counts[name] = (this.counts[name] || 0) + n; this.total += n; }
    if ((f.sys_recent || []).length) {
      for (const r of f.sys_recent) this.rows.unshift(r);
      this.rows.length = Math.min(this.rows.length, 80);
      this.render();
    }
  },
  render() {
    const entries = Object.entries(this.counts).sort((a, b) => b[1] - a[1]);
    dl("#sys-counts", entries.length ? entries : [["no calls yet", "–"]]);
    $("#sys-total").textContent = `${this.total.toLocaleString()} calls since this page opened`;
    $("#sys-rows").innerHTML = this.rows.map(([t, tid, nr, name, ret]) => `<tr class="${ret < 0 ? "err" : ""}"><td class="num">${(t / 1e6).toFixed(3)}</td><td class="num">${tid}</td>` +
      `<td>${esc(name)}</td><td class="num">${nr}</td><td class="num">${ret}</td><td class="meaning">${esc(this.meaning(name, ret))}</td></tr>`).join("") ||
      '<tr class="empty-row"><td colspan="6">Waiting for a system call.</td></tr>';
  },
  renderProcs() {
    const items = [...this.procs.values()].reverse().slice(0, 12);
    $("#uprocs").innerHTML = items.map((p) => `<li class="${p.state}"><span class="pid">${p.tid}</span><span class="nm">${esc(p.name)}</span><i></i><span class="st">${esc(p.label)}</span></li>`).join("") ||
      '<li class="empty">No program has entered U-mode yet. Run one above.</li>';
  },
};
function onUser(m) {
  const p = sys.procs.get(m.tid) || { tid: m.tid, name: m.name };
  if (m.event === "enter") Object.assign(p, { state: "run", label: "running in U-mode", name: m.name });
  if (m.event === "exit" && p.state !== "kill") Object.assign(p, { state: "exit", label: `exited, code ${m.code}` });
  if (m.event === "kill") { Object.assign(p, { state: "kill", label: `killed: ${m.cause}` }); ground.kill(m.tid); topo?.kill(m.tid); }
  sys.procs.delete(m.tid);
  sys.procs.set(m.tid, p);
  sys.renderProcs();
}
$$("[data-user]").forEach((b) => (b.onclick = () => {
  sendInput(`user ${b.dataset.user}\r`);
  toast(`Running "user ${b.dataset.user}" in the kernel shell`);
}));

// ================================================================ research
async function loadResearch() {
  const r = await (await fetch("/api/research")).json();
  const verdict = (v) => `<span class="verdict ${/support/i.test(v) && !/not/i.test(v) ? "yes" : "no"}">${esc(v)}</span>`;
  const hyps = (r.r3?.hypotheses || []).slice(1);
  $("#hyps").innerHTML = hyps.map(([h, sim, k]) => {
    const [id, ...rest] = h.split(":");
    return `<div class="hyp"><b>${esc(id)}</b><p>${esc(rest.join(":").trim())}</p><dl><dt>simulator</dt><dd>${verdict(sim)}</dd><dt>this kernel</dt><dd>${verdict(k)}</dd></dl></div>`;
  }).join("") || '<p class="hint">results/research_sync.md not found; run scripts/research.sh.</p>';
  const t = r.r3?.bimodal || [];
  $("#r3-table").innerHTML = t.length ? `<thead><tr>${t[0].map((c, i) => `<th class="${i ? "num" : ""}">${esc(c)}</th>`).join("")}</tr></thead><tbody>` +
    t.slice(1).map((row) => `<tr>${row.map((c, i) => `<td class="${i ? "num" : "name"}">${esc(c)}</td>`).join("")}</tr>`).join("") + "</tbody>" : "";
  $("#r3-figs").innerHTML = r.figures.filter((f) => f.startsWith("r3_")).map((f) => `<figure><img loading="lazy" src="/results/${f}" alt="${f.replace(".png", "")}"><figcaption>${f.replace(".png", "")}</figcaption></figure>`).join("");
  $$("#r3-figs figure").forEach((fig) => (fig.onclick = () => { const img = fig.querySelector("img"); $("#lightbox").innerHTML = `<img src="${img.src}" alt="${img.alt}">`; $("#lightbox").hidden = false; }));
  // Boot time per stage: kernel + userspace, drawn to one scale.
  const rows = (r.distro?.rows || []).slice(1);
  const num = (v) => parseFloat(String(v).replace(/[*±].*$/, "").replace(/\*/g, "")) || 0;
  const max = Math.max(1, ...rows.map((x) => num(x[4]) + num(x[5])));
  $("#strata").innerHTML = rows.map((x) => {
    const k = num(x[4]), u = num(x[5]), ver = r.distro.verify[x[0]];
    return `<div class="stratum"><div class="st-head"><b>${esc(x[0])}</b><span>${esc(x[1])}</span></div>
      <div class="st-bar"><i class="k" style="width:${(k / max) * 100}%" title="kernel ${k} s"></i><i class="u" style="width:${(u / max) * 100}%" title="userspace ${u} s"></i></div>
      <div class="st-num"><b>${esc(x[6].replace(/\*/g, ""))} s</b><span>${esc(x[7] || "")}</span>${ver ? `<span class="checks-ok">${ver.passed} checks${ver.failed ? `, ${ver.failed} failed` : " pass"}</span>` : ""}</div></div>`;
  }).join("") + '<div class="strata-key"><span><i class="k"></i>kernel + initramfs</span><span><i class="u"></i>userspace</span></div>' ||
    '<p class="hint">results/distro_boottime.md not found.</p>';
}
$$(".rtab").forEach((t) => (t.onclick = () => {
  $$(".rtab").forEach((u) => { const on = u === t; u.setAttribute("aria-selected", String(on)); $(`#${u.getAttribute("aria-controls")}`).hidden = !on; });
}));

// ================================================================ desktop menu and window arrangement
function tileWindows() {
  const open = APPS.map(([a]) => a).filter((a) => !win(a).hidden && !win(a).classList.contains("min"));
  if (!open.length) return;
  const W = ws.clientWidth, H = ws.clientHeight - 96, g = 12;
  const cols = Math.ceil(Math.sqrt(open.length)), rows = Math.ceil(open.length / cols);
  open.forEach((a, i) => {
    const c = i % cols, r = Math.floor(i / cols);
    geo[a] = { ...geo[a], max: false, x: g + c * ((W - g) / cols), y: g + r * ((H - g) / rows), w: (W - g) / cols - g, h: (H - g) / rows - g };
    place(a);
  });
  save();
  ground.kick();
}
function showDesktop() { APPS.forEach(([a]) => { if (!win(a).hidden && !win(a).classList.contains("min")) { geo[a].min = true; win(a).classList.add("min"); } }); save(); afterHide(); }
function closeCtx() { $("#ctxmenu").hidden = true; }
ws.addEventListener("contextmenu", (e) => {
  if (e.target.closest(".win")) return;
  e.preventDefault();
  const m = $("#ctxmenu"), r = ws.getBoundingClientRect();
  m.hidden = false;
  m.style.left = `${Math.min(e.clientX - r.left, r.width - m.offsetWidth - 8)}px`;
  m.style.top = `${Math.min(e.clientY - r.top, r.height - m.offsetHeight - 8)}px`;
  m.querySelector("button").focus();
});
document.addEventListener("pointerdown", (e) => { if (!$("#ctxmenu").hidden && !e.target.closest("#ctxmenu")) closeCtx(); });
$("#ctxmenu").addEventListener("keydown", (e) => { if (e.key === "Escape") closeCtx(); });
$("#ctxmenu").addEventListener("click", (e) => {
  const b = e.target.closest("button");
  if (!b) return;
  closeCtx();
  const a = b.dataset.ctx;
  if (a === "terminal" || a === "topo") openApp(a);
  if (a === "tile") tileWindows();
  if (a === "clear") showDesktop();
  if (a === "tour") tour.start();
  if (a === "reboot") { openApp("system"); selectTab("machine"); }
});

// ================================================================ launcher search: apps, programs, shell commands
let programList = [];
fetch("/api/programs").then((r) => r.json()).then((p) => (programList = p));
function launcherHits(q) {
  q = q.trim();
  const hits = [];
  if (!q) return hits;
  const lq = q.toLowerCase();
  for (const [app, title, ic] of APPS) if (title.toLowerCase().includes(lq)) hits.push({ label: `Open ${title}`, ic, run: () => openApp(app) });
  const word = lq.split(/\s+/)[0];
  for (const p of programList) {
    if (p.name.startsWith(word)) {
      const cmd = lq.includes(" ") || p.name === lq ? q : `${p.name}${PRESETS[p.name] ? " " + PRESETS[p.name] : ""}`;
      hits.push({ label: `Run "${cmd}" in the kernel shell`, sub: p.about, ic: "i-terminal", run: () => runCommand(cmd) });
    }
  }
  if (!hits.length) hits.push({ label: `Run "${q}" in the kernel shell`, ic: "i-terminal", run: () => runCommand(q) });
  return hits.slice(0, 7);
}
const spot = {
  sel: 0,
  open() {
    $("#spotlight").hidden = false;
    $("#launcher-q").value = "";
    this.render("");
    $("#launcher-q").focus();
  },
  close() { $("#spotlight").hidden = true; },
  render(q) {
    // With nothing typed, list the apps.
    const hits = q.trim() ? launcherHits(q) : APPS.map(([app, title, ic]) => ({ label: title, ic, run: () => openApp(app) }));
    this.hits = hits;
    this.sel = 0;
    $("#launcher-hits").innerHTML = hits.map((h, i) => `<button data-hit="${i}" class="${i === 0 ? "sel" : ""}">${icon(h.ic)}<span>${esc(h.label)}${h.sub ? `<small>${esc(h.sub)}</small>` : ""}</span></button>`).join("");
  },
  move(d) {
    const btns = $$("#launcher-hits button");
    if (!btns.length) return;
    btns[this.sel]?.classList.remove("sel");
    this.sel = (this.sel + d + btns.length) % btns.length;
    btns[this.sel].classList.add("sel");
    btns[this.sel].scrollIntoView({ block: "nearest" });
  },
  run(i) { const h = this.hits[i]; this.close(); if (h) h.run(); },
};
$("#launcher-q").addEventListener("input", (e) => spot.render(e.target.value));
$("#launcher-q").addEventListener("keydown", (e) => {
  if (e.key === "ArrowDown") { e.preventDefault(); spot.move(1); }
  if (e.key === "ArrowUp") { e.preventDefault(); spot.move(-1); }
  if (e.key === "Enter") { e.preventDefault(); spot.run(spot.sel); }
  if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); spot.close(); }
});
$("#launcher-hits").addEventListener("click", (e) => { const b = e.target.closest("[data-hit]"); if (b) spot.run(Number(b.dataset.hit)); });
$("#spot-btn").onclick = (e) => { e.stopPropagation(); spot.open(); };
document.addEventListener("pointerdown", (e) => { if (!$("#spotlight").hidden && !e.target.closest("#spotlight, #spot-btn")) spot.close(); });
document.addEventListener("keydown", (e) => {
  if ((e.ctrlKey || e.metaKey) && (e.key.toLowerCase() === "k" || e.code === "Space") && !document.body.classList.contains("booting")) {
    e.preventDefault(); $("#spotlight").hidden ? spot.open() : spot.close();
  }
  if (e.key === "F3" && e.ctrlKey) { e.preventDefault(); mission(!ws.classList.contains("mission")); }
}, true);

// ================================================================ guided tour (every step uses the real kernel)
const TOUR = [
  { title: "This desktop is lit by a real kernel", text: "Each glow on the wallpaper is a task of the Rust kernel running in QEMU; it brightens as the task gets more CPU. The widgets on the right are live kernel instruments: CPU history, the scheduler's real dispatch order, the heap, and system calls. Nothing here is simulated.", app: null },
  { title: "The kernel's own shell", text: "The terminal is the kernel's serial console. Commands run as kernel tasks scheduled alongside everything else.", app: "terminal", cmd: "help" },
  { title: "A CPU-bound task lights up", text: "spin keeps the CPU busy. A new glow rises on the wallpaper, the CPU widget climbs, and the Scheduler widget's ribbon shows the real order in which tasks get the CPU (the kernel's DISPATCH trace).", app: "terminal", cmd: "spin 10 &" },
  { title: "Tasks waking each other", text: "pingpong runs two tasks that hand two semaphores back and forth. In the Kernel Map each dashed arc is a real WAKE event: one task released the semaphore the other was blocked on. The kernel has no message-passing IPC; this is how its tasks hand off work.", app: "topo", cmd: "pingpong 60 60 &" },
  { title: "Crossing into the kernel: system calls", text: "This program runs in U-mode and can only reach the kernel with ecall. Each write, getpid and sleep appears here as it happens.", app: "syscalls", cmd: "user hello 2" },
  { title: "The kernel protects itself", text: "Now a U-mode program tries a privileged instruction. The CPU traps, the kernel kills only that program, and keeps running. A notification appears.", app: "syscalls", cmd: "user priv" },
  { title: "Where the heap is used", text: "The heap map is read from the kernel's free list once a second: 128 slices of its 16 MiB heap, brighter is fuller.", app: "memory" },
  { title: "Write a program, build the kernel", text: "Edit a program in Rust and press Build & run. Cargo compiles the kernel on the host (the kernel has no compiler), the VM reboots and runs your program, and it lights up on the wallpaper.", app: "code" },
  { title: "Eight schedulers, one reboot away", text: "Open Control Center from the menu bar and reboot with Round Robin, Priority, aging, MLFQ, FCFS, SJF, SRTF or HRRN. The test suite in Settings runs 68 checks on the real kernel.", app: "system" },
  { title: "The research, measured on this kernel", text: "The R3 study's three hypotheses hold on the kernel as in the simulator, and the Micro-OS Linux boot time is measured after every component.", app: "research" },
];
const tour = {
  i: 0,
  start() { this.i = 0; $("#tour").hidden = false; this.show(); },
  end() { $("#tour").hidden = true; },
  show() {
    const st = TOUR[this.i];
    $("#tour-step").textContent = `${this.i + 1} of ${TOUR.length}`;
    $("#tour-title").textContent = st.title;
    $("#tour-text").textContent = st.text;
    $("#tour-cmd").hidden = !st.cmd;
    $("#tour-cmd").innerHTML = st.cmd ? `kernel shell: <code>${esc(st.cmd)}</code>` : "";
    $("#tour-run").hidden = !st.cmd;
    $("#tour-run").textContent = st.cmd ? `Run ${st.cmd.split(" ")[0] === "user" ? st.cmd.split(" ").slice(0, 2).join(" ") : st.cmd.split(" ")[0]}` : "";
    $("#tour-back").disabled = this.i === 0;
    $("#tour-next").textContent = this.i === TOUR.length - 1 ? "Finish" : "Next";
    if (st.app) openApp(st.app); else showDesktop();
    $("#tour-next").focus();
  },
  run() {
    const st = TOUR[this.i];
    if (!st.cmd) return;
    sendInput(st.cmd + "\r");
    if (st.app && st.app !== "terminal") focusWin(st.app);
    toast(`Ran "${st.cmd}" in the kernel shell`);
  },
};
$("#tour-next").onclick = () => (tour.i === TOUR.length - 1 ? tour.end() : (tour.i++, tour.show()));
$("#tour-back").onclick = () => { if (tour.i > 0) { tour.i--; tour.show(); } };
$("#tour-run").onclick = () => tour.run();
$("#tour-end").onclick = () => tour.end();
$("#tour").addEventListener("keydown", (e) => { if (e.key === "Escape") { e.stopPropagation(); tour.end(); } });

// ================================================================ desktop widgets (all live)
const widgets = {
  cpu: [], ribbon: [], sysCount: 0,
  onSnap(s, busy, heap, sec) {
    this.cpu.push(busy);
    if (this.cpu.length > 240) this.cpu.shift();
    $("#w-cpu").textContent = `${Math.round(busy * 100)}%`;
    $("#w-sched").textContent = `${POLICY_SHORT[pkey(s.policy)] || s.policy} · q=${s.q} · last 3 s`;
    $("#w-heap").textContent = size(s.heap_used);
    const hh = String(Math.floor(sec / 3600)).padStart(2, "0"), mm = String(Math.floor((sec % 3600) / 60)).padStart(2, "0"), ss = String(sec % 60).padStart(2, "0");
    $("#desk-up").textContent = `kernel up ${hh}:${mm}:${ss} · ${s.tasks.length} tasks · ${POLICY_NAMES[pkey(s.policy)] || s.policy}`;
    this.names = new Map(s.tasks.map((t) => [t.tid, t.name]));
    const lit = currentPeaks.filter((p) => p.h > 0.25).sort((a, b) => b.h - a.h)[0];
    $("#desk-light").innerHTML = lit
      ? `<i style="background:${colorOf(lit.id)}"></i>brightest light on the wallpaper: ${esc(lit.label)} (pid ${lit.id}), ${Math.round((lit.h - 0.2) / 0.8 * 100)}% CPU`
      : "each light on the wallpaper is a running task";
    this.drawCpu();
  },
  onFlow(f) {
    // The real dispatch order of this window, collapsed into runs of the same task.
    const runs = [];
    for (const tid of f.path || []) { if (runs.length && runs[runs.length - 1][0] === tid) runs[runs.length - 1][1]++; else runs.push([tid, 1]); }
    this.ribbon.push(runs.length ? runs : [[0, 1]]);
    if (this.ribbon.length > 12) this.ribbon.shift();
    this.drawRibbon();
    const recent = f.sys_recent || [];
    if (recent.length) {
      this.sysCount += Object.values(f.syscalls || {}).reduce((a, b) => a + b, 0);
      $("#w-sys").textContent = `${this.sysCount.toLocaleString()} call${this.sysCount === 1 ? "" : "s"}`;
      const list = $("#w-sys-list");
      if (list.querySelector(".empty")) list.innerHTML = "";
      for (const [, tid, , name, ret] of recent.slice(-4)) {
        const li = document.createElement("li");
        li.className = ret < 0 ? "err" : "";
        li.innerHTML = `<span class="pid">${tid}</span><span>${esc(name)}</span><span>${ret}</span>`;
        list.prepend(li);
      }
      while (list.children.length > 4) list.lastElementChild.remove();
    }
  },
  onHeap(m) {
    const box = $("#w-dots");
    if (box.children.length !== m.map.length) box.innerHTML = m.map.map(() => "<i></i>").join("");
    m.map.forEach((v, i) => box.children[i].classList.toggle("on", v > 0));
    const used = m.map.filter((v) => v > 0).length;
    $("#w-heap-foot").textContent = `${used}/${m.slices} slices in use · 16 MiB heap`;
  },
  drawCpu() {
    const c = $("#w-cpu-chart"), dpr = window.devicePixelRatio || 1, w = c.clientWidth, h = 54;
    if (!w) return;
    c.width = w * dpr; c.height = h * dpr; c.style.height = `${h}px`;
    const x = c.getContext("2d"); x.scale(dpr, dpr);
    const n = 60, data = this.cpu.slice(-n), bw = w / n;
    data.forEach((v, i) => {
      const bh = Math.max(1.5, v * (h - 2));
      x.fillStyle = v > 0.85 ? "#ff8a65" : v > 0.4 ? "#ffc46b" : "#6ee7b7";
      x.globalAlpha = 0.35 + 0.65 * ((i + 1) / data.length);
      x.fillRect(w - (data.length - i) * bw + 1, h - bh, Math.max(1, bw - 2), bh);
    });
  },
  drawRibbon() {
    const c = $("#w-ribbon"), dpr = window.devicePixelRatio || 1, w = c.clientWidth, h = 40;
    if (!w) return;
    c.width = w * dpr; c.height = h * dpr; c.style.height = `${h}px`;
    const x = c.getContext("2d"); x.scale(dpr, dpr);
    const slot = w / 12;
    const seen = new Map();
    const totals = new Map();
    for (const runs of this.ribbon) for (const [tid, n] of runs) if (tid) totals.set(tid, (totals.get(tid) || 0) + n);
    const top = [...leadSlot.entries()].sort((a, b) => a[1] - b[1]).map(([t]) => t);
    const ribColor = (tid) => (leadSlot.has(tid) ? LEAD[leadSlot.get(tid)] : "rgba(255,255,255,.22)");
    this.ribbon.forEach((runs, i) => {
      // Per quarter-second: how the dispatches were shared, one band per task, stacked.
      const per = new Map();
      for (const [tid, n] of runs) per.set(tid, (per.get(tid) || 0) + n);
      const total = [...per.values()].reduce((a, b) => a + b, 0) || 1;
      let py = 2;
      const px = w - (this.ribbon.length - i) * slot;
      for (const [tid, n] of [...per.entries()].sort((a, b) => a[0] - b[0])) {
        const hh = (n / total) * (h - 4);
        x.fillStyle = tid ? ribColor(tid) : "rgba(255,255,255,.1)";
        x.beginPath(); x.roundRect(px + 1.5, py, slot - 3, Math.max(1, hh - 1.5), 3); x.fill();
        if (tid) seen.set(tid, (seen.get(tid) || 0) + n);
        py += hh;
      }
    });
    const others = [...totals.keys()].some((t) => !leadSlot.has(t));
    $("#w-ribbon-key").innerHTML = top.length
      ? top.map((tid) => `<span class="ot"><i class="sw" style="background:${ribColor(tid)}"></i>${esc((this.names && this.names.get(tid)) || "task")} ${tid}</span>`).join("") +
        (others ? `<span class="ot"><i class="sw" style="background:rgba(255,255,255,.3)"></i>others</span>` : "")
      : "who got the CPU, each quarter-second";
  },
};
$$(".widget").forEach((w) => (w.onclick = () => openApp(w.dataset.open)));

// ================================================================ lock screen
const lock = {
  seen: false,
  show() {
    const last = boot.events.get("shell") || [...boot.events.values()].pop();
    $("#lock-boot").textContent = last ? `kernel booted in ${(last.t / 1000).toFixed(1)} ms after power-on · ${running ? "running" : "off"}` : (running ? "kernel running" : "kernel off");
    $("#lock").hidden = false;
    $("#lock").classList.remove("leaving");
    $(".app").inert = true;
    tick();
    $("#lock-enter").focus();
  },
  hide() {
    if ($("#lock").hidden) return;
    this.seen = true;
    $("#lock").classList.add("leaving");
    $(".app").inert = false;
    setTimeout(() => { $("#lock").hidden = true; $("#lock").classList.remove("leaving"); }, STILL ? 0 : 520);
    ground.kick();
  },
};
$("#lock-enter").onclick = () => lock.hide();
$("#lock").addEventListener("pointerdown", (e) => { if (!e.target.closest("button")) lock.hide(); });
document.addEventListener("keydown", (e) => { if (!$("#lock").hidden && !document.body.classList.contains("booting")) { e.preventDefault(); lock.hide(); } }, true);

// ================================================================ Control Center
const cc = {
  policy: "rr",
  open() { $("#cc").hidden = false; document.body.classList.add("cc-open"); $("#cc-btn").setAttribute("aria-expanded", "true"); this.sync(); $("#cc-policies button[aria-checked='true']")?.focus(); },
  close(refocus) { $("#cc").hidden = true; document.body.classList.remove("cc-open"); $("#cc-btn").setAttribute("aria-expanded", "false"); if (refocus) $("#cc-btn").focus(); },
  sync() {
    this.policy = $("#policy").value || "rr";
    $$("#cc-policies button").forEach((b) => b.setAttribute("aria-checked", String(b.dataset.p === this.policy)));
    $("#cc-quantum").value = $("#quantum").value; $("#cc-qv").textContent = $("#quantum").value;
    $("#cc-power-label").textContent = running ? "Power off" : "Boot";
  },
};
$("#cc-policies").innerHTML = Object.keys(POLICY_SHORT).map((p) => `<button role="radio" data-p="${p}" aria-checked="false" title="${POLICY_NAMES[p]}">${POLICY_SHORT[p] === "PRIO+AGING" ? "AGING" : POLICY_SHORT[p]}</button>`).join("");
$$("#cc-policies button").forEach((b) => (b.onclick = () => { cc.policy = b.dataset.p; $$("#cc-policies button").forEach((x) => x.setAttribute("aria-checked", String(x === b))); }));
$("#cc-quantum").oninput = (e) => ($("#cc-qv").textContent = e.target.value);
$("#cc-reboot").onclick = () => { $("#policy").value = cc.policy; $("#quantum").value = $("#cc-quantum").value; cc.close(); bootMachine(); toast(`Rebooting the kernel with ${POLICY_NAMES[cc.policy]}, quantum ${$("#cc-quantum").value}`); };
$("#cc-motion").setAttribute("aria-pressed", String(ground.motion));
$("#cc-motion").onclick = () => {
  const on = $("#cc-motion").getAttribute("aria-pressed") !== "true";
  $("#cc-motion").setAttribute("aria-pressed", String(on));
  ground.setMotion(on);
  try { localStorage.setItem("nexus.motion", on ? "on" : "off"); } catch (_) { /* private mode */ }
};
$("#cc-dnd").onclick = () => { const on = $("#cc-dnd").getAttribute("aria-pressed") !== "true"; $("#cc-dnd").setAttribute("aria-pressed", String(on)); notes.quiet = on; };
$("#cc-mission").onclick = () => { cc.close(); mission(true); };
$("#cc-power").onclick = () => { cc.close(); power(); };
$("#cc-btn").onclick = (e) => { e.stopPropagation(); $("#cc").hidden ? cc.open() : cc.close(); };
$("#capsule").onclick = (e) => { e.stopPropagation(); $("#cc").hidden ? cc.open() : cc.close(); };
document.addEventListener("pointerdown", (e) => { if (!$("#cc").hidden && !e.target.closest("#cc, #cc-btn, #capsule")) cc.close(); });
$("#cc").addEventListener("keydown", (e) => { if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); cc.close(true); } });

// ================================================================ Mission Control: every open window at once
function mission(on) {
  const open = APPS.map(([a]) => a).filter((a) => !win(a).hidden && !win(a).classList.contains("min"));
  $$(".mission-label").forEach((l) => l.remove());
  if (!on || !open.length) {
    ws.classList.remove("mission");
    $$(".win").forEach((w) => { w.style.transform = ""; w.tabIndex = -1; });
    if (on && !open.length) toast("No windows are open");
    return;
  }
  ws.classList.add("mission");
  setTimeout(() => win(open[0]).focus(), 50);
  const W = ws.clientWidth, H = ws.clientHeight - 110, pad = 36;
  const cols = Math.ceil(Math.sqrt(open.length)), rows = Math.ceil(open.length / cols);
  const cw = (W - pad * (cols + 1)) / cols, ch = (H - pad * (rows + 1)) / rows;
  open.forEach((a, i) => {
    const el = win(a);
    // A window still in its open animation is scaled and shifted, and the animation would
    // override the transform set here: finish it, then measure layout size, not the transformed box.
    el.getAnimations().forEach((an) => an.finish());
    const r = { left: el.offsetLeft, top: el.offsetTop, width: el.offsetWidth, height: el.offsetHeight };
    const k = Math.min(cw / r.width, ch / r.height, 1);
    const c = i % cols, row = Math.floor(i / cols);
    const tx = pad + c * (cw + pad) + (cw - r.width * k) / 2, ty = pad + row * (ch + pad) + (ch - r.height * k) / 2;
    el.style.transformOrigin = "0 0";
    el.style.transform = `translate(${tx - r.left}px, ${ty - r.top}px) scale(${k})`;
    el.tabIndex = 0;
    const l = document.createElement("span");
    l.className = "mission-label"; l.textContent = TITLE[a];
    l.style.left = `${tx + (r.width * k) / 2}px`; l.style.top = `${ty + r.height * k + 8}px`;
    ws.appendChild(l);
  });
}
ws.addEventListener("click", (e) => {
  if (!ws.classList.contains("mission")) return;
  const w = e.target.closest(".win");
  mission(false);
  if (w) focusWin(w.dataset.app, true);
}, true);
document.addEventListener("keydown", (e) => {
  if (!ws.classList.contains("mission")) return;
  if (e.key === "Escape") { e.preventDefault(); mission(false); }
  const w = document.activeElement?.closest?.(".win");
  if ((e.key === "Enter" || e.key === " ") && w) { e.preventDefault(); mission(false); focusWin(w.dataset.app, true); }
}, true);

// ================================================================ menu bar menus (all real actions)
const MENUS = () => {
  const app = focused;
  const appMenu = {
    terminal: [["Clear the screen", () => runCommand("clear")], ["Stop the program (Ctrl+C)", () => sendInput("\x03")], ["Kernel help", () => runCommand("help")], ["System info", () => runCommand("sysinfo")]],
    code: [["New program…", () => code.create()], ["Save", () => code.save(), "Ctrl+S"], ["Build & run", () => code.build(), "Ctrl+Enter"]],
    files: [["Go to programs", () => files.go("kernel/src/programs")], ["Go to results", () => files.go("results")], ["Project root", () => files.go("")]],
    syscalls: [["Run user hello", () => sendInput("user hello 2\r")], ["Run user priv (gets killed)", () => sendInput("user priv\r")], ["Run user badptr", () => sendInput("user badptr\r")]],
    system: [["Run the test suite", () => { selectTab("tests"); fetch("/api/tests", { method: "POST" }); }], ["About NEXUS", () => selectTab("about")]],
  }[app];
  const menus = [];
  const MENU_NAME = { terminal: "Shell", code: "File", files: "Go", syscalls: "Run", system: "Tests" };
  if (appMenu) menus.push([MENU_NAME[app], appMenu]);
  menus.push(["Window", [
    ["Minimize", () => focused && minimize(focused), null, !focused], ["Zoom", () => focused && toggleMax(focused), null, !focused], ["Close", () => focused && closeWin(focused), null, !focused],
    "-", ["Arrange windows", () => tileWindows()], ["Mission Control", () => mission(true), "Ctrl+F3"], ["Show the desktop", () => showDesktop()]]]);
  menus.push(["Kernel", [
    ["Control Center…", () => cc.open()], ["Reboot with a scheduler…", () => { openApp("system"); selectTab("machine"); }],
    [running ? "Power off" : "Boot", () => power()], "-", ["Journal", () => openApp("journal")], ["Kernel Map", () => openApp("topo")]]]);
  menus.push(["Help", [["Guided tour", () => tour.start()], ["Search", () => spot.open(), "Ctrl+K"]]]);
  return menus;
};
let menuOpen = null;
function renderMenus() {
  const ms = MENUS();
  $("#mb-menus").innerHTML = ms.map(([name], i) => `<button data-m="${i}" aria-haspopup="menu" aria-expanded="false">${esc(name)}</button>`).join("");
  $$("#mb-menus button").forEach((b) => (b.onclick = (e) => { e.stopPropagation(); menuOpen === Number(b.dataset.m) ? closeMenu() : openMenu(Number(b.dataset.m)); }));
}
function openMenu(i) {
  const ms = MENUS(), [, items] = ms[i], btn = $(`#mb-menus [data-m="${i}"]`), pop = $("#mb-pop");
  menuOpen = i;
  $$("#mb-menus button").forEach((b) => b.setAttribute("aria-expanded", String(b === btn)));
  pop.innerHTML = items.map((it, k) => it === "-" ? "<hr>" : `<button role="menuitem" data-k="${k}" ${it[3] ? "disabled" : ""}><span>${esc(it[0])}</span>${it[2] ? `<span class="kbd">${it[2]}</span>` : ""}</button>`).join("");
  pop.style.left = `${btn.getBoundingClientRect().left}px`;
  pop.hidden = false;
  pop._items = items;
  pop.querySelector("button:not([disabled])")?.focus();
}
function closeMenu(refocus) {
  if (menuOpen === null) return;
  const btn = $(`#mb-menus [data-m="${menuOpen}"]`);
  $("#mb-pop").hidden = true;
  $$("#mb-menus button").forEach((b) => b.setAttribute("aria-expanded", "false"));
  menuOpen = null;
  if (refocus) btn?.focus();
}
$("#mb-pop").addEventListener("click", (e) => {
  const b = e.target.closest("[data-k]");
  if (!b || b.disabled) return;
  const it = $("#mb-pop")._items[Number(b.dataset.k)];
  closeMenu();
  it[1]();
});
$("#mb-pop").addEventListener("keydown", (e) => {
  const items = [...$$("#mb-pop button:not([disabled])")], i = items.indexOf(document.activeElement);
  if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); items[(i + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length]?.focus(); }
  if (e.key === "ArrowRight" || e.key === "ArrowLeft") { e.preventDefault(); const n = MENUS().length; openMenu((menuOpen + (e.key === "ArrowRight" ? 1 : -1) + n) % n); }
  if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); closeMenu(true); }
});
document.addEventListener("pointerdown", (e) => { if (menuOpen !== null && !e.target.closest("#mb-pop, #mb-menus")) closeMenu(); });
renderMenus();

// ================================================================ first layout
function initDesktop() {
  const remembered = APPS.filter(([app]) => geo[app] && geo[app].open);
  if (!remembered.length) {
    geo = {};
    $("#app-name").textContent = "Desktop";
    return;
  }
  for (const [app] of remembered) {
    const el = win(app);
    el.hidden = false;
    place(app);
    el.classList.toggle("min", !!geo[app].min);
    el.style.zIndex = ++zTop;
    onShow(app);
  }
  const top = geo.terminal && geo.terminal.open && !geo.terminal.min ? "terminal" : topVisible();
  if (top) focusWin(top); else afterHide();
}
initDesktop();

// Deep links, handy for setting up a demo: #app=topo&max&noboot&power
const link = new URLSearchParams(location.hash.slice(1));
if (link.has("app") && TITLE[link.get("app")]) {
  const app = link.get("app");
  openApp(app);
  if (link.has("max") && !geo[app].max) toggleMax(app);
}
if (link.has("noboot")) { lock.seen = true; boot.start = function () { this.done = true; }; boot.finish(); }
if (link.has("power")) setTimeout(() => running && power(), 1500);
if (link.has("tour")) setTimeout(() => tour.start(), 600);
connect();
