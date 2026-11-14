// NEXUS kernel aurora: the desktop wallpaper, lit by the running kernel.
// Each live task is a soft field of light whose size and brightness follow
// its CPU share; a dispatch makes its field flare, a kill flashes rose, and
// the heap in use glows along the horizon. With no kernel the sky is dark.
// Same interface as Terrain (setPeaks / setHeap / pushFlow / kill / kick).
"use strict";

(function () {
  const STILL = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  // Task light colours: coral, amber, cyan, violet, mint, rose, sky, lime.
  const HUES = [[255, 138, 101], [255, 196, 107], [94, 211, 243], [167, 139, 250], [110, 231, 183], [244, 114, 182], [125, 170, 255], [190, 230, 120]];
  const hueOf = (id) => HUES[(id * 5 + 3) % HUES.length];

  class Aurora {
    constructor(canvas, opts = {}) {
      this.canvas = canvas;
      this.ctx = canvas.getContext("2d");
      this.opts = { scale: 8, covered: null, ...opts };   // drawn at 1/8 resolution, scaled up smoothly
      this.small = document.createElement("canvas");
      this.sctx = this.small.getContext("2d");
      this.fields = new Map();   // id -> {h, th, flare, x, y, phase}
      this.heap = 0; this.heapT = 0;
      this.kills = [];
      this.status = "";
      this.motion = true;
      this.running = false;
      this.last = 0;
      this.gap = 1000 / 12;
      this.resize();
      new ResizeObserver(() => { this.resize(); this.kick(); }).observe(canvas);
      document.addEventListener("visibilitychange", () => this.kick());
    }

    resize() {
      const w = this.canvas.clientWidth, h = this.canvas.clientHeight;
      if (!w || !h) { this.w = 0; return; }
      this.w = w; this.h = h;
      this.canvas.width = w; this.canvas.height = h;
      this.small.width = Math.ceil(w / this.opts.scale); this.small.height = Math.ceil(h / this.opts.scale);
    }

    setPeaks(list, status = "") {
      this.status = status;
      const seen = new Set();
      list.forEach((p, i) => {
        seen.add(p.id);
        let f = this.fields.get(p.id);
        if (!f) {
          // A stable home on the sky for each task, from its id.
          const a = (p.id * 2.399963) % (Math.PI * 2);
          f = { h: 0, flare: 0, x: 0.5 + Math.cos(a) * (0.18 + 0.05 * (p.id % 4)), y: 0.42 + Math.sin(a) * 0.22, phase: p.id * 1.7 };
          this.fields.set(p.id, f);
        }
        f.th = Math.max(0.12, Math.min(1, p.h));
        f.user = !!p.user;
        if (p.rgb) f.rgb = p.rgb;
        f.lead = !!p.lead;
      });
      for (const [id, f] of this.fields) if (!seen.has(id)) f.th = 0;
      this.kick();
    }
    setHeap(map) {
      if (!map) return;
      const used = map.reduce((a, v) => a + v, 0) / (map.length * 255);
      this.heapT = Math.min(1, 0.25 + used * 30);   // the heap is mostly empty: lift small use into view
      this.kick();
    }
    pushFlow(f) {
      for (const [tid, n] of Object.entries(f.disp || {})) {
        const fl = this.fields.get(Number(tid));
        if (fl) fl.flare = Math.min(1, fl.flare + Math.min(0.5, n / 40));
      }
      this.kick();
    }
    kill(id) { this.kills.push({ id, t0: performance.now() }); this.kick(); }
    setMotion(on) { this.motion = on; this.kick(); }

    kick() {
      if (this.running) return;
      this.running = true;
      requestAnimationFrame((t) => this.frame(t));
    }
    frame(t) {
      if (document.hidden || !this.w || (this.opts.covered && this.opts.covered())) { this.running = false; return; }
      if (t - this.last < this.gap) { requestAnimationFrame((u) => this.frame(u)); return; }
      const dt = Math.min(0.2, (t - (this.last || t)) / 1000);
      this.last = t;
      let busy = this.step(dt);
      this.draw(t / 1000);
      busy = busy || this.kills.length > 0 || (this.motion && !STILL && this.fields.size > 0);
      if (busy) requestAnimationFrame((u) => this.frame(u));
      else this.running = false;
    }
    step(dt) {
      const k = STILL ? 1 : 1 - Math.exp(-dt * 1.6);
      let moving = false;
      for (const [id, f] of this.fields) {
        const d = f.th - f.h;
        if (Math.abs(d) > 0.004) { f.h += d * k; moving = true; } else f.h = f.th;
        f.flare *= Math.exp(-dt * 2.5);
        if (f.th === 0 && f.h < 0.01) this.fields.delete(id);
      }
      const dh = this.heapT - this.heap;
      if (Math.abs(dh) > 0.003) { this.heap += dh * k; moving = true; }
      return moving;
    }
    draw(time) {
      const c = this.sctx, W = this.small.width, H = this.small.height;
      // Twilight ground.
      const g = c.createLinearGradient(0, 0, 0, H);
      g.addColorStop(0, "#090c18"); g.addColorStop(0.6, "#0e1328"); g.addColorStop(1, "#141a35");
      c.globalCompositeOperation = "source-over";
      c.fillStyle = g; c.fillRect(0, 0, W, H);
      c.globalCompositeOperation = "lighter";
      const drift = this.motion && !STILL ? time : 0;
      // Heap horizon.
      const hz = c.createRadialGradient(W * 0.5, H * 1.15, 0, W * 0.5, H * 1.15, W * 0.75);
      hz.addColorStop(0, `rgba(110,140,255,${0.18 + 0.32 * this.heap})`); hz.addColorStop(1, "rgba(110,140,255,0)");
      c.fillStyle = hz; c.fillRect(0, 0, W, H);
      // One field of light per task. The three busiest tasks are saturated fields that read
      // across a room; every other task is a dim haze, so many tasks never add up to white.
      const n = this.fields.size;
      const shrink = Math.min(1, 2.4 / Math.sqrt(Math.max(1, n)));
      c.globalCompositeOperation = "screen";
      const order = [...this.fields.entries()].sort((a, b) => Number(a[1].lead) - Number(b[1].lead));
      for (const [id, f] of order) {
        const [r, gg, b] = f.rgb || hueOf(id);
        const x = (f.x + Math.sin(drift * 0.05 + f.phase) * 0.06) * W;
        const y = (f.y + Math.cos(drift * 0.04 + f.phase) * 0.05) * H;
        const rad = f.lead ? Math.max(W, H) * (0.2 + 0.22 * f.h + 0.05 * f.flare)
                           : Math.max(W, H) * (0.1 + 0.12 * f.h) * shrink;
        const a = f.lead ? Math.min(0.62, 0.34 + 0.3 * f.h + 0.15 * f.flare)
                         : Math.min(0.14, 0.05 + 0.1 * f.h + 0.05 * f.flare);
        const rg = c.createRadialGradient(x, y, 0, x, y, rad);
        rg.addColorStop(0, `rgba(${r},${gg},${b},${a})`);
        rg.addColorStop(0.45, `rgba(${r},${gg},${b},${a * 0.35})`);
        rg.addColorStop(1, `rgba(${r},${gg},${b},0)`);
        c.fillStyle = rg; c.fillRect(0, 0, W, H);
      }
      // A kill flashes rose from the task's field.
      const now = performance.now();
      this.kills = this.kills.filter((k) => now - k.t0 < 1800);
      for (const kk of this.kills) {
        const f = this.fields.get(kk.id) || { x: 0.5, y: 0.45 };
        const p = (now - kk.t0) / 1800;
        const x = f.x * W, y = f.y * H, rad = Math.max(W, H) * (0.1 + 0.5 * p);
        const rg = c.createRadialGradient(x, y, rad * 0.6, x, y, rad);
        rg.addColorStop(0, `rgba(255,80,110,0)`); rg.addColorStop(0.8, `rgba(255,80,110,${0.5 * (1 - p)})`); rg.addColorStop(1, "rgba(255,80,110,0)");
        c.fillStyle = rg; c.fillRect(0, 0, W, H);
      }
      c.globalCompositeOperation = "source-over";
      const ctx = this.ctx;
      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = "high";
      ctx.drawImage(this.small, 0, 0, W, H, 0, 0, this.canvas.width, this.canvas.height);
    }
  }
  window.Aurora = Aurora;
})();
