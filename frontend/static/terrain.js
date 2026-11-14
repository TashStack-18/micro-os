// NEXUS kernel topography. A contour map whose relief is the running kernel:
//   peaks   = tasks (height: CPU share over the last snapshot window),
//   ridge   = the heap map (how full each 1/128 of the heap is),
//   coral   = the scheduler's real dispatch order (DISPATCH trace),
//   celadon = wake-ups (WAKE by=task) and timer wake-ups (WAKE by=timer).
// Nothing is invented: with no data the map is flat and says so.
"use strict";

(function () {
  const C = {
    carbon: "#111b20", slate: "130,148,154", chalk: "#e9e4d8", chalk2: "#cbc8bd",
    coral: "215,122,84", celadon: "185,213,196",
  };
  const UI = '"Onest", "Segoe UI", sans-serif';
  const DATA = '"Spline Sans Mono", Consolas, monospace';
  const STILL = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const GOLDEN = Math.PI * (3 - Math.sqrt(5));

  class Terrain {
    /**
     * @param {HTMLCanvasElement} canvas
     * @param {{cell?: number, avoid?: () => DOMRect[], heapBand?: number, labels?: boolean, frame?: boolean, scale?: number}} opts
     */
    constructor(canvas, opts = {}) {
      this.canvas = canvas;
      this.ctx = canvas.getContext("2d");
      this.opts = { cell: 12, heapBand: 26, labels: true, frame: true, scale: 1, covered: null, ...opts };
      this.layer = document.createElement("canvas");   // contours, redrawn when the relief moves
      this.shade = document.createElement("canvas");   // height shading at grid resolution
      this.peaks = new Map();   // id -> {id,label,sub,state,h,th,x,y,tx,ty,s,ts,alive,fade}
      this.heap = null;         // [0..255] per slice
      this.heapEase = null;
      this.hops = [];           // dispatch traverse segments
      this.arcs = [];           // wake-up arcs
      this.pulses = [];         // dispatch landings and timer ticks
      this.region = null;       // survey region {x,y,w,h}
      this.status = "";         // message when there is nothing to draw
      this.dirty = true;
      this.running = false;
      this.last = 0;
      this.fpsGap = 1000 / 20;
      this.reliefGap = 1000 / 8;    // the contour field is recomputed at most 8 times a second
      this.lastRelief = 0;
      this.resize();
      this.ro = new ResizeObserver(() => { this.resize(); this.kick(); });
      this.ro.observe(canvas);
      document.addEventListener("visibilitychange", () => this.kick());
    }

    resize() {
      const dpr = Math.min(window.devicePixelRatio || 1, 1.5);
      const w = this.canvas.clientWidth, h = this.canvas.clientHeight;
      if (!w || !h) { this.w = 0; return; }
      this.w = w; this.h = h; this.dpr = dpr;
      for (const c of [this.canvas, this.layer]) { c.width = Math.round(w * dpr); c.height = Math.round(h * dpr); }
      this.cols = Math.ceil(w / this.opts.cell) + 1;
      this.rows = Math.ceil(h / this.opts.cell) + 1;
      this.shade.width = this.cols; this.shade.height = this.rows;
      this.field = new Float32Array(this.cols * this.rows);
      this.dirty = true;
    }

    // ------------------------------------------------------------ model input
    /** peaks: [{id, label, sub, state, h (0..1), weight}] */
    setPeaks(list, status = "") {
      this.status = status;
      const seen = new Set();
      list.forEach((p) => {
        seen.add(p.id);
        let q = this.peaks.get(p.id);
        if (!q) {
          q = { id: p.id, h: 0, x: NaN, y: NaN, s: 40, alive: true, fade: 0 };
          this.peaks.set(p.id, q);
        }
        Object.assign(q, { label: p.label, sub: p.sub, state: p.state, th: Math.max(0, Math.min(1, p.h)), alive: true,
          ws: p.weight ?? 0.3, user: !!p.user });
      });
      for (const q of this.peaks.values()) if (!seen.has(q.id)) { q.alive = false; q.th = 0; }
      this.layout();
      this.kick();
    }

    setHeap(map) {
      this.heap = map ? map.map((v) => v / 255) : null;
      if (this.heap && (!this.heapEase || this.heapEase.length !== this.heap.length)) this.heapEase = this.heap.map(() => 0);
      this.kick();
    }

    /** One snapshot window of the scheduler's own trace (server "flow"). */
    pushFlow(f, windowMs = 250) {
      const now = performance.now();
      // Dispatch order: collapse repeats of the same task into one landing.
      const seq = [];
      for (const tid of f.path || []) if (seq[seq.length - 1] !== tid) seq.push(tid);
      const counts = f.disp || {};
      if (seq.length === 1) {
        const n = Math.min(6, counts[seq[0]] || 1);
        for (let i = 0; i < n; i++) this.pulses.push({ id: seq[0], t0: now + (i * windowMs) / n, kind: "land" });
      } else {
        const hops = seq.slice(-12);
        const step = windowMs / Math.max(1, hops.length - 1);
        for (let i = 1; i < hops.length; i++) {
          this.hops.push({ a: hops[i - 1], b: hops[i], t0: now + (i - 1) * step, dur: Math.max(60, step) });
        }
      }
      for (const [by, tid, n] of f.wakes || []) this.arcs.push({ a: by, b: tid, n, t0: now });
      for (const [tid, n] of Object.entries(f.sys_by_tid || {})) {
        this.pulses.push({ id: Number(tid), t0: now, kind: "sys", n });
      }
      for (const [tid, n] of Object.entries(f.timer_wakes || {})) {
        this.pulses.push({ id: Number(tid), t0: now, kind: "timer", n });
      }
      // Keep the overlays bounded.
      if (this.hops.length > 36) this.hops.splice(0, this.hops.length - 36);
      if (this.arcs.length > 60) this.arcs.splice(0, this.arcs.length - 60);
      if (this.pulses.length > 120) this.pulses.splice(0, this.pulses.length - 120);
      this.kick();
    }

    /** The kernel killed this task (USER_KILL): a red ring spreads from its summit. */
    kill(id) {
      this.pulses.push({ id, t0: performance.now(), kind: "kill" });
      this.kick();
    }

    // ------------------------------------------------------------ layout
    surveyRegion() {
      const band = this.opts.heapBand;
      const full = { x: 0, y: 0, w: this.w, h: this.h - band };
      if (!this.opts.avoid) return full;
      const ws = this.canvas.getBoundingClientRect();
      // Mark a coarse grid of the workspace covered by windows, then take the
      // largest all-free rectangle (histogram method): the survey sits there.
      const G = 32, cols = Math.max(1, Math.floor(this.w / G)), rows = Math.max(1, Math.floor((this.h - band) / G));
      const free = Array.from({ length: rows }, () => new Uint8Array(cols).fill(1));
      for (const r of this.opts.avoid()) {
        const x0 = Math.max(0, Math.floor((r.left - ws.left - 8) / G)), x1 = Math.min(cols - 1, Math.floor((r.right - ws.left + 8) / G));
        const y0 = Math.max(0, Math.floor((r.top - ws.top - 8) / G)), y1 = Math.min(rows - 1, Math.floor((r.bottom - ws.top + 8) / G));
        for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) free[y][x] = 0;
      }
      const hgt = new Uint16Array(cols);
      let best = null, bestArea = 0;
      for (let y = 0; y < rows; y++) {
        for (let x = 0; x < cols; x++) hgt[x] = free[y][x] ? hgt[x] + 1 : 0;
        const stack = [];
        for (let x = 0; x <= cols; x++) {
          const hh = x < cols ? hgt[x] : 0;
          let start = x;
          while (stack.length && stack[stack.length - 1][1] >= hh) {
            const [sx, sh] = stack.pop();
            // Favour usable shapes: area, but penalise very thin strips.
            const w = (x - sx) * G, h = sh * G;
            const area = w * h * Math.min(1, Math.min(w, h) / 260);
            if (area > bestArea) { bestArea = area; best = { x: sx * G, y: (y - sh + 1) * G, w, h }; }
            start = sx;
          }
          stack.push([start, hh]);
        }
      }
      if (!best || best.w < 240 || best.h < 200) return full;
      return best;
    }

    layout() {
      if (!this.w) return;
      const r = (this.region = this.surveyRegion());
      if (this.opts.onRegion) this.opts.onRegion(r);
      const live = [...this.peaks.values()].filter((p) => p.alive).sort((p, q) => p.id - q.id);
      const n = live.length;
      const cx = r.x + r.w / 2, cy = r.y + r.h / 2;
      // Spread over the whole open region: its own width and height, not a circle.
      const k = n <= 1 ? 0 : n <= 3 ? 0.62 : 0.8;
      const rx = (r.w / 2 - 90) * k, ry = (r.h / 2 - 50) * k;
      live.forEach((p, i) => {
        const rad = n <= 1 ? 0 : Math.sqrt((i + 0.7) / (n + 0.2));
        const ang = i * GOLDEN + 0.9;
        p.tx = cx + Math.cos(ang) * rad * Math.max(0, rx);
        p.ty = cy + Math.sin(ang) * rad * Math.max(0, ry);
        p.ts = Math.max(28, Math.min(r.w * 0.9, r.h) * (0.06 + 0.08 * Math.sqrt(p.ws)));
        if (Number.isNaN(p.x)) { p.x = p.tx; p.y = p.ty; p.s = p.ts; }
      });
    }

    // ------------------------------------------------------------ loop
    kick() {
      this.dirty = true;
      if (this.running) return;
      this.running = true;
      requestAnimationFrame((t) => this.frame(t));
    }

    frame(t) {
      if (document.hidden || !this.w || (this.opts.covered && this.opts.covered())) { this.running = false; return; }
      if (t - this.last < this.fpsGap - 1) { requestAnimationFrame((u) => this.frame(u)); return; }
      const dt = Math.min(0.1, (t - (this.last || t)) / 1000);
      this.last = t;
      const moving = this.ease(STILL ? 1 : dt);
      if ((moving || this.dirty) && (STILL || t - this.lastRelief >= this.reliefGap)) {
        this.relief(); this.dirty = false; this.lastRelief = t;
      }
      const busy = this.paint(performance.now());
      if (moving || busy || this.dirty) requestAnimationFrame((u) => this.frame(u));
      else this.running = false;
    }

    ease(dt) {
      const k = STILL ? 1 : 1 - Math.exp(-dt * 5);
      let moving = false;
      if (this.opts.avoid) {
        const r = this.surveyRegion();
        const q = this.region;
        if (!q || Math.abs(r.x - q.x) > 2 || Math.abs(r.w - q.w) > 2 || Math.abs(r.y - q.y) > 2 || Math.abs(r.h - q.h) > 2) this.layout();
      }
      for (const [id, p] of this.peaks) {
        for (const [a, b] of [["h", "th"], ["x", "tx"], ["y", "ty"], ["s", "ts"]]) {
          const d = p[b] - p[a];
          if (Math.abs(d) > (a === "h" ? 0.006 : 0.6)) { p[a] += d * k; moving = true; } else p[a] = p[b];
        }
        if (!p.alive && p.h < 0.01) this.peaks.delete(id);
      }
      if (this.heap && this.heapEase) {
        for (let i = 0; i < this.heap.length; i++) {
          const d = this.heap[i] - this.heapEase[i];
          if (Math.abs(d) > 0.006) { this.heapEase[i] += d * k; moving = true; } else this.heapEase[i] = this.heap[i];
        }
      }
      return moving;
    }

    // ------------------------------------------------------------ relief: height field, shading, contours
    relief() {
      this.bandGeometry();
      const { cols, rows, field } = this;
      const cell = this.opts.cell, band = this.opts.heapBand, H = this.h;
      const peaks = [...this.peaks.values()].filter((p) => p.h > 0.004);
      const heap = this.heapEase;
      // Heap ridge height per grid column, smoothed over +-10 slices (same place, readable width).
      const colV = new Float32Array(cols);
      if (heap) {
        for (let i = 0; i < cols; i++) {
          const f = (i * cell - this.bandX0) / this.bandBw - 0.5;
          if (f < -0.5) continue;
          const c0 = Math.round(Math.max(0, Math.min(heap.length - 1, f)));
          let v = 0, wsum = 0;
          for (let d = -10; d <= 10; d++) {
            const kk = c0 + d;
            if (kk < 0 || kk >= heap.length) continue;
            const wgt = 11 - Math.abs(d);
            v += heap[kk] * wgt; wsum += wgt;
          }
          colV[i] = wsum ? Math.min(1, (v / wsum) * 3) : 0;
        }
      }
      for (let j = 0; j < rows; j++) {
        const y = j * cell;
        // The heap ridge rises from the bottom edge.
        const ridgeY = Math.exp(-Math.max(0, H - band - y) / (H * 0.09));
        for (let i = 0; i < cols; i++) {
          const x = i * cell;
          let z = 0;
          for (const p of peaks) {
            const dx = x - p.x, dy = y - p.y, d2 = dx * dx + dy * dy, s2 = p.s * p.s;
            if (d2 < 9 * s2) z += p.h * Math.exp(-d2 / (2 * s2));
          }
          if (heap) {
            const v = colV[i];
            z += v * 0.32 * ridgeY;
          }
          field[j * cols + i] = z;
        }
      }
      // Shading: one pixel per grid cell, scaled up smoothly.
      const sctx = this.shade.getContext("2d");
      const img = sctx.createImageData(cols, rows);
      for (let n = 0; n < field.length; n++) {
        const z = Math.min(1, field[n]);
        img.data[n * 4] = 17 + z * 40; img.data[n * 4 + 1] = 27 + z * 54; img.data[n * 4 + 2] = 32 + z * 54; img.data[n * 4 + 3] = 255;
      }
      sctx.putImageData(img, 0, 0);

      const ctx = this.layer.getContext("2d");
      const dpr = this.dpr;
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.imageSmoothingEnabled = true;
      ctx.drawImage(this.shade, 0, 0, cols, rows, 0, 0, cols * cell * dpr, rows * cell * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      const step = 0.09;
      for (let L = 1; L * step < 1.55; L++) {
        const level = L * step;
        const index = L % 5 === 0;
        ctx.beginPath();
        this.march(level, ctx);
        ctx.strokeStyle = `rgba(${C.slate},${index ? 0.85 : 0.48})`;
        ctx.lineWidth = index ? 1.5 : 1;
        ctx.stroke();
      }
      // Survey ticks and the heap band change only with the relief: bake them into the layer.
      if (this.opts.frame) this.graticule(ctx);
      this.heapBand(ctx);
    }

    // Marching squares for one level, appended to the current path.
    march(level, ctx) {
      const { cols, rows, field } = this;
      const c = this.opts.cell;
      for (let j = 0; j < rows - 1; j++) {
        for (let i = 0; i < cols - 1; i++) {
          const a = field[j * cols + i], b = field[j * cols + i + 1];
          const d = field[(j + 1) * cols + i], e = field[(j + 1) * cols + i + 1];
          const idx = (a > level ? 8 : 0) | (b > level ? 4 : 0) | (e > level ? 2 : 0) | (d > level ? 1 : 0);
          if (idx === 0 || idx === 15) continue;
          const x = i * c, y = j * c;
          const top = [x + c * ((level - a) / (b - a)), y];
          const right = [x + c, y + c * ((level - b) / (e - b))];
          const bottom = [x + c * ((level - d) / (e - d)), y + c];
          const left = [x, y + c * ((level - a) / (d - a))];
          const seg = (p, q) => { ctx.moveTo(p[0], p[1]); ctx.lineTo(q[0], q[1]); };
          switch (idx) {
            case 1: case 14: seg(left, bottom); break;
            case 2: case 13: seg(bottom, right); break;
            case 3: case 12: seg(left, right); break;
            case 4: case 11: seg(top, right); break;
            case 5: seg(left, top); seg(bottom, right); break;
            case 6: case 9: seg(top, bottom); break;
            case 7: case 8: seg(left, top); break;
            case 10: seg(left, bottom); seg(top, right); break;
          }
        }
      }
    }

    // ------------------------------------------------------------ paint: relief + live overlays
    paint(now) {
      const ctx = this.ctx, dpr = this.dpr;
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.fillStyle = C.carbon;
      ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);
      ctx.drawImage(this.layer, 0, 0);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      let busy = false;
      busy = this.drawHops(ctx, now) || busy;
      busy = this.drawArcs(ctx, now) || busy;
      busy = this.drawPulses(ctx, now) || busy;
      this.drawPeaks(ctx);
      if (this.status) {
        ctx.font = `400 ${13 * this.opts.scale}px ${UI}`;
        ctx.fillStyle = `rgba(${C.slate},0.9)`;
        ctx.textAlign = "center";
        const r = this.region || { x: 0, w: this.w, h: this.h };
        ctx.fillText(this.status, r.x + r.w / 2, (r.h || this.h) * 0.5);
        ctx.textAlign = "left";
      }
      return busy;
    }

    graticule(ctx) {
      // Survey ticks along the top and left edges every 80 px.
      ctx.strokeStyle = `rgba(${C.slate},0.35)`;
      ctx.lineWidth = 1;
      ctx.beginPath();
      for (let x = 80; x < this.w; x += 80) { ctx.moveTo(x + 0.5, 0); ctx.lineTo(x + 0.5, x % 400 === 0 ? 9 : 5); }
      for (let y = 80; y < this.h - this.opts.heapBand; y += 80) { ctx.moveTo(0, y + 0.5); ctx.lineTo(y % 400 === 0 ? 9 : 5, y + 0.5); }
      ctx.stroke();
    }

    bandGeometry() {
      const n = this.heap ? this.heap.length : 128;
      this.bandX0 = 112 * this.opts.scale;
      this.bandBw = (this.w - this.bandX0 - 12) / n;
    }

    heapBand(ctx) {
      const band = this.opts.heapBand;
      if (!band) return;
      this.bandGeometry();
      const y0 = this.h - band;
      ctx.fillStyle = "rgba(12,20,24,0.78)";
      ctx.fillRect(0, y0, this.w, band);
      ctx.strokeStyle = `rgba(${C.slate},0.3)`;
      ctx.beginPath(); ctx.moveTo(0, y0 + 0.5); ctx.lineTo(this.w, y0 + 0.5); ctx.stroke();
      const heap = this.heapEase;
      ctx.font = `400 ${13 * this.opts.scale}px ${UI}`;
      ctx.fillStyle = `rgba(${C.slate},1)`;
      ctx.fillText(heap ? "heap map" : "heap map …", 10, y0 + band / 2 + 5);
      if (!heap) return;
      const n = heap.length, bw = this.bandBw;
      for (let i = 0; i < n; i++) {
        const v = heap[i];
        const hh = v > 0 ? Math.max(3, v * (band - 8)) : 1;
        ctx.fillStyle = v > 0 ? `rgba(${C.celadon},${0.55 + v * 0.45})` : `rgba(${C.slate},0.3)`;
        ctx.fillRect(this.bandX0 + i * bw, this.h - 4 - hh, Math.max(1, bw - 1), hh);
      }
    }

    pos(id) {
      const p = this.peaks.get(id);
      return p && !Number.isNaN(p.x) ? p : null;
    }

    drawHops(ctx, now) {
      let busy = false;
      const LIFE = 700;
      this.hops = this.hops.filter((h) => now - h.t0 < h.dur + LIFE);
      for (const h of this.hops) {
        const a = this.pos(h.a), b = this.pos(h.b);
        if (!a || !b) continue;
        busy = true;
        const p = STILL ? 1 : Math.max(0, Math.min(1, (now - h.t0) / h.dur));
        if (p <= 0) continue;
        const age = Math.max(0, now - h.t0 - h.dur);
        const alpha = STILL ? 0.7 : 0.95 * (1 - age / LIFE) ** 2;
        const x = a.x + (b.x - a.x) * p, y = a.y + (b.y - a.y) * p;
        ctx.strokeStyle = `rgba(${C.coral},${alpha})`;
        ctx.lineWidth = 1.6;
        ctx.beginPath(); ctx.moveTo(a.x, a.y); ctx.lineTo(x, y); ctx.stroke();
        if (p < 1) {
          ctx.fillStyle = `rgba(${C.coral},1)`;
          ctx.fillRect(x - 2.5, y - 2.5, 5, 5);
        }
      }
      return busy;
    }

    drawArcs(ctx, now) {
      let busy = false;
      const DUR = 700, LIFE = 1600;
      this.arcLabels = [];
      this.arcs = this.arcs.filter((w) => now - w.t0 < DUR + LIFE);
      for (const w of this.arcs) {
        const a = this.pos(w.a), b = this.pos(w.b);
        if (!a || !b) continue;
        busy = true;
        const p = STILL ? 1 : Math.min(1, (now - w.t0) / DUR);
        const age = Math.max(0, now - w.t0 - DUR);
        const alpha = STILL ? 0.8 : 1 - age / LIFE;
        const mx = (a.x + b.x) / 2, my = (a.y + b.y) / 2;
        const dx = b.x - a.x, dy = b.y - a.y, len = Math.hypot(dx, dy) || 1;
        const cx = mx - (dy / len) * len * 0.3, cy = my + (dx / len) * len * 0.3;
        ctx.strokeStyle = `rgba(${C.celadon},${alpha})`;
        ctx.lineWidth = 1.5;
        ctx.setLineDash([4, 3]);
        ctx.beginPath();
        const N = 24;
        for (let i = 0; i <= N * p; i++) {
          const t = i / N, u = 1 - t;
          const x = u * u * a.x + 2 * u * t * cx + t * t * b.x, y = u * u * a.y + 2 * u * t * cy + t * t * b.y;
          i ? ctx.lineTo(x, y) : ctx.moveTo(x, y);
        }
        ctx.stroke();
        ctx.setLineDash([]);
        if (w.n > 1 && p === 1) {
          // One count per pair of tasks: the newest arc's.
          const key = `${Math.min(w.a, w.b)}-${Math.max(w.a, w.b)}`;
          const old = this.arcLabels.find((l) => l.key === key);
          if (!old) this.arcLabels.push({ key, text: `${w.n} wake-ups`, x: cx + 4, y: cy, alpha });
          else if (alpha > old.alpha) Object.assign(old, { text: `${w.n} wake-ups`, x: cx + 4, y: cy, alpha });
        }
      }
      return busy;
    }

    drawPulses(ctx, now) {
      let busy = false;
      const LIFE = 650;
      this.pulses = this.pulses.filter((q) => now - q.t0 < (q.kind === "kill" ? 1600 : LIFE));
      for (const q of this.pulses) {
        const p = this.pos(q.id);
        if (!p || now < q.t0) { busy = true; continue; }
        busy = true;
        const life = q.kind === "kill" ? 1600 : LIFE;
        if (now - q.t0 > life) continue;
        const k = (now - q.t0) / life;
        if (q.kind === "kill") {
          ctx.strokeStyle = `rgba(228,131,107,${1 - k})`;
          ctx.lineWidth = 2;
          ctx.beginPath(); ctx.arc(p.x, p.y, 10 + k * 46, 0, Math.PI * 2); ctx.stroke();
        } else if (q.kind === "sys") {
          // System calls from a U-mode task: chalk ticks around the summit, one per call (up to 12).
          const n = Math.min(12, q.n);
          ctx.strokeStyle = `rgba(233,228,216,${0.9 * (1 - k)})`;
          ctx.lineWidth = 1.5;
          ctx.beginPath();
          for (let i = 0; i < n; i++) {
            const a = (i / n) * Math.PI * 2 - Math.PI / 2, r0 = 13, r1 = 17 + 4 * (1 - k);
            ctx.moveTo(p.x + Math.cos(a) * r0, p.y + Math.sin(a) * r0);
            ctx.lineTo(p.x + Math.cos(a) * r1, p.y + Math.sin(a) * r1);
          }
          ctx.stroke();
        } else if (q.kind === "land") {
          ctx.strokeStyle = `rgba(${C.coral},${0.8 * (1 - k)})`;
          ctx.lineWidth = 1.2;
          ctx.beginPath(); ctx.arc(p.x, p.y, 6 + k * 16, 0, Math.PI * 2); ctx.stroke();
        } else {
          // A sleep that ended on a timer tick: a short celadon mark rising off the summit.
          const len = 10 + Math.min(22, 5 * Math.log2(1 + q.n));
          ctx.strokeStyle = `rgba(${C.celadon},${0.9 * (1 - k)})`;
          ctx.lineWidth = 1.5;
          ctx.beginPath(); ctx.moveTo(p.x + 9, p.y - 4); ctx.lineTo(p.x + 9, p.y - 4 - len * (0.4 + 0.6 * k)); ctx.stroke();
        }
      }
      return busy;
    }

    drawPeaks(ctx) {
      const sc = this.opts.scale;
      // Boxes already taken: every peak's ring first, then each label as it is placed.
      const placed = [...this.peaks.values()].filter((q) => !Number.isNaN(q.x)).map((q) => ({ x: q.x - 13, y: q.y - 13, w: 26, h: 26 }));
      const cxr = this.region ? this.region.x + this.region.w / 2 : this.w / 2;
      for (const p of [...this.peaks.values()].sort((a, b) => a.y - b.y)) {
        if (Number.isNaN(p.x) || (!p.alive && p.h < 0.05)) continue;
        const col = p.state === "RUNNING" ? C.coral : p.state === "READY" ? C.celadon : C.slate;
        const fade = p.alive ? 1 : Math.min(1, p.h * 4);
        // Survey benchmark: a cross on the summit and a ring for the state.
        ctx.strokeStyle = `rgba(233,228,216,${0.85 * fade})`;
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(p.x - 5, p.y); ctx.lineTo(p.x + 5, p.y); ctx.moveTo(p.x, p.y - 5); ctx.lineTo(p.x, p.y + 5);
        ctx.stroke();
        ctx.strokeStyle = `rgba(${col},${fade})`;
        ctx.lineWidth = 1.5;
        if (p.user) ctx.setLineDash([3, 3]);   // a U-mode task: the dashed ring is the privilege boundary
        ctx.beginPath(); ctx.arc(p.x, p.y, 9, 0, Math.PI * 2); ctx.stroke();
        ctx.setLineDash([]);
        if (p.state === "RUNNING") { ctx.fillStyle = `rgba(${col},${fade})`; ctx.beginPath(); ctx.arc(p.x, p.y, 3, 0, Math.PI * 2); ctx.fill(); }
        if (!this.opts.labels) continue;
        ctx.font = `500 ${15 * sc}px ${UI}`;
        const tw = ctx.measureText(p.label).width;
        ctx.font = `400 ${13 * sc}px ${DATA}`;
        const sw = ctx.measureText(p.sub).width;
        const bw = Math.max(tw, sw) + 8, bh = 36 * sc;
        // Label on the outer side of the peak; slide down until it overlaps nothing.
        const right = p.x >= cxr - 4;
        let lx = right ? p.x + 16 : p.x - 16 - bw + 4, ly = p.y - 2;
        // Labels stay inside the open region so windows never cover them.
        const reg = this.region || { x: 0, w: this.w };
        const top = 15 * sc;
        const hits = (x, y) => placed.some((b) => x - 4 < b.x + b.w && x - 4 + bw > b.x && y - top < b.y + b.h && y - top + bh > b.y);
        // Try beside the peak, then above/below it, then step further down.
        const tries = [[lx, ly], [lx, ly - bh - 8], [lx, ly + bh + 8], [right ? p.x - 16 - bw + 4 : p.x + 16, ly]];
        for (let k = 2; k < 8; k++) tries.push([lx, ly + k * (bh + 4)]);
        const spot = tries.find(([x, y]) => !hits(x, y) && x - 4 >= reg.x && x + bw <= reg.x + reg.w + 4) || tries[0];
        [lx, ly] = spot;
        placed.push({ x: lx - 4, y: ly - top, w: bw, h: bh });
        if (Math.abs(ly - (p.y - 2)) > 1) {
          ctx.strokeStyle = `rgba(${C.slate},${0.6 * fade})`;
          ctx.lineWidth = 1;
          ctx.beginPath(); ctx.moveTo(p.x, p.y); ctx.lineTo(right ? lx - 4 : lx + bw - 4, ly - 4); ctx.stroke();
        }
        ctx.fillStyle = `rgba(17,27,32,${0.78 * fade})`;
        ctx.fillRect(lx - 4, ly - top, bw, bh);
        ctx.font = `500 ${15 * sc}px ${UI}`;
        ctx.fillStyle = `rgba(233,228,216,${fade})`;
        ctx.fillText(p.label, lx, ly);
        ctx.font = `400 ${13 * sc}px ${DATA}`;
        ctx.fillStyle = `rgba(${col === C.slate ? "175,190,195" : col},${fade})`;
        ctx.fillText(p.sub, lx, ly + 17 * sc);
      }
      // Wake-up counts join the same collision set and stay inside the region.
      const reg = this.region || { x: 0, y: 0, w: this.w, h: this.h };
      ctx.font = `400 ${13 * sc}px ${DATA}`;
      for (const a of this.arcLabels || []) {
        const tw = ctx.measureText(a.text).width + 8, th = 18 * sc;
        let x = Math.max(reg.x + 6, Math.min(a.x, reg.x + reg.w - tw - 4)), y = a.y;
        const hit = (xx, yy) => placed.some((b) => xx - 4 < b.x + b.w && xx - 4 + tw > b.x && yy - 13 * sc < b.y + b.h && yy - 13 * sc + th > b.y);
        let k = 0;
        while (hit(x, y) && k++ < 8) y += th + 2;
        if (hit(x, y)) continue;   // no free spot: the arc speaks for itself
        placed.push({ x: x - 4, y: y - 13 * sc, w: tw, h: th });
        ctx.fillStyle = `rgba(17,27,32,${0.78 * a.alpha})`;
        ctx.fillRect(x - 4, y - 13 * sc, tw, th);
        ctx.fillStyle = `rgba(${C.celadon},${a.alpha})`;
        ctx.fillText(a.text, x, y);
      }
    }
  }

  window.Terrain = Terrain;
})();
