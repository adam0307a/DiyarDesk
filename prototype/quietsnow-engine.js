'use strict';
/*
 * DiyarDesk · Quiet Snow (Sessiz Kar) Engine
 * Original GPL-3.0 physics & atmospheric precipitation renderer for DiyarDesk.
 * Supports 4 realistic seasonal modes (Snow, Rain, Autumn Leaves, Cherry Blossom),
 * 4 atmospheric backdrops, per-icon & ground heightfield/heap accumulation,
 * icon-move shedding with lumps & powder bursts, and granular cursor interaction.
 */
(function (global) {
  const TAU = Math.PI * 2;
  const clamp = (v, lo, hi) => (v < lo ? lo : v > hi ? hi : v);
  const lerp = (a, b, t) => a + (b - a) * t;
  const smoothstep = (e0, e1, x) => {
    const t = clamp((x - e0) / (e1 - e0 || 1), 0, 1);
    return t * t * (3 - 2 * t);
  };

  function mulberry32(seed) {
    let a = seed | 0;
    return function () {
      a = (a + 0x6D2B79F5) | 0;
      let t = Math.imul(a ^ (a >>> 15), 1 | a);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  function mixRgb(a, b, t) {
    return [
      Math.round(a[0] + (b[0] - a[0]) * t),
      Math.round(a[1] + (b[1] - a[1]) * t),
      Math.round(a[2] + (b[2] - a[2]) * t)
    ];
  }

  function rgba(c, a) {
    return 'rgba(' + c[0] + ',' + c[1] + ',' + c[2] + ',' + a.toFixed(3) + ')';
  }

  // Atmospheric palettes:
  // 0: Quiet Night, 1: Blue Hour, 2: Misty Morning, 3: Streetlamp Warmth
  const ATMOS = [
    {
      name: 'night',
      sky: ['#070d18', '#0e1a2d', '#172940', '#213750'],
      haze: [24, 42, 64],
      mtnFar: [14, 26, 44],
      mtnNear: [10, 20, 35],
      snowTop: [242, 248, 255],
      snowMid: [212, 226, 242],
      snowDeep: [132, 158, 190],
      snowOutline: [180, 204, 232],
      shadowAlpha: 0.44,
      flakeTint: [236, 245, 255],
      rainTint: [195, 220, 248],
      lamp: false,
      stars: 0.9,
      lightBg: false
    },
    {
      name: 'bluehour',
      sky: ['#0c1c35', '#1b3559', '#355980', '#607e9c'],
      haze: [76, 108, 138],
      mtnFar: [33, 56, 84],
      mtnNear: [21, 39, 62],
      snowTop: [246, 250, 255],
      snowMid: [218, 230, 244],
      snowDeep: [138, 165, 198],
      snowOutline: [190, 210, 235],
      shadowAlpha: 0.36,
      flakeTint: [242, 248, 255],
      rainTint: [210, 230, 252],
      lamp: false,
      stars: 0.25,
      lightBg: false
    },
    {
      name: 'morning',
      sky: ['#7f95aa', '#9fb3c4', '#bcccd9', '#d2dde7'],
      haze: [198, 212, 224],
      mtnFar: [122, 142, 162],
      mtnNear: [95, 115, 136],
      snowTop: [252, 254, 255],
      snowMid: [232, 240, 248],
      snowDeep: [164, 184, 206],
      snowOutline: [136, 160, 186],
      shadowAlpha: 0.28,
      flakeTint: [252, 254, 255],
      rainTint: [230, 240, 250],
      lamp: false,
      stars: 0,
      lightBg: true
    },
    {
      name: 'streetlamp',
      sky: ['#0a0f1a', '#131d2d', '#1c2a3e', '#263448'],
      haze: [36, 46, 62],
      mtnFar: [20, 28, 42],
      mtnNear: [14, 20, 32],
      snowTop: [255, 246, 232],
      snowMid: [228, 226, 230],
      snowDeep: [132, 148, 174],
      snowOutline: [220, 206, 190],
      shadowAlpha: 0.48,
      flakeTint: [248, 244, 238],
      rainTint: [232, 222, 204],
      lamp: true,
      stars: 0.45,
      lightBg: false
    }
  ];

  // Subtle backdrop hue shift when switching to Autumn or Blossom so scene feels cohesive
  const MODE_TINT = [
    null,              // 0: Snow (native cold/neutral)
    [16, 28, 42, 0.16],// 1: Rain (slightly deeper slate)
    [68, 42, 24, 0.18],// 2: Autumn (warm amber dusk tint)
    [82, 48, 66, 0.14] // 3: Blossom (soft rose-mauve hint)
  ];

  const AUTUMN_COLORS = [
    [[188, 52, 36], [134, 28, 22]],   // Crimson maple
    [[218, 96, 32], [156, 54, 18]],   // Burnt orange
    [[232, 162, 44], [174, 108, 24]], // Golden amber
    [[196, 122, 46], [132, 72, 26]],  // Ochre rust
    [[158, 66, 42], [102, 38, 24]],   // Chestnut
    [[212, 134, 52], [148, 84, 30]]   // Warm honey
  ];

  const BLOSSOM_COLORS = [
    [[255, 228, 238], [240, 178, 202]],
    [[255, 242, 247], [244, 198, 216]],
    [[250, 204, 222], [222, 146, 176]],
    [[255, 218, 232], [234, 162, 192]]
  ];

  // =========================================================================
  // PRE-BAKED OFFSCREEN SPRITE ATLASES (Zero per-frame shadowBlur / gradients)
  // =========================================================================
  function createSprites() {
    const out = {
      snowSoft: [],
      snowCrystal: [],
      snowBokeh: [],
      leaves: [],   // [species][colorIdx] -> canvas
      petals: [],   // [colorIdx] -> canvas
      flower: null
    };

    // 1. Soft granular snow puffs (4 subtle organic shapes)
    for (let v = 0; v < 4; v++) {
      const sz = 28;
      const c = document.createElement('canvas');
      c.width = c.height = sz;
      const g = c.getContext('2d');
      const r = mulberry32(101 + v * 37);
      const cx = sz / 2, cy = sz / 2;
      for (let i = 0; i < 4; i++) {
        const ox = (i === 0 ? 0 : (r() - 0.5) * 4.2);
        const oy = (i === 0 ? 0 : (r() - 0.5) * 4.2);
        const rad = i === 0 ? 10.5 : (5.5 + r() * 4.0);
        const gr = g.createRadialGradient(cx + ox, cy + oy, 0, cx + ox, cy + oy, rad);
        gr.addColorStop(0, 'rgba(255,255,255,0.92)');
        gr.addColorStop(0.45, 'rgba(244,250,255,0.68)');
        gr.addColorStop(0.78, 'rgba(220,236,252,0.22)');
        gr.addColorStop(1, 'rgba(220,236,252,0)');
        g.fillStyle = gr;
        g.beginPath();
        g.arc(cx + ox, cy + oy, rad, 0, TAU);
        g.fill();
      }
      out.snowSoft.push(c);
    }

    // 2. Delicate 6-arm snow crystals (3 variations)
    for (let v = 0; v < 3; v++) {
      const sz = 36;
      const c = document.createElement('canvas');
      c.width = c.height = sz;
      const g = c.getContext('2d');
      const cx = sz / 2, cy = sz / 2;
      const glow = g.createRadialGradient(cx, cy, 0, cx, cy, sz * 0.46);
      glow.addColorStop(0, 'rgba(255,255,255,0.65)');
      glow.addColorStop(0.5, 'rgba(235,246,255,0.24)');
      glow.addColorStop(1, 'rgba(220,238,255,0)');
      g.fillStyle = glow;
      g.fillRect(0, 0, sz, sz);

      g.save();
      g.translate(cx, cy);
      g.strokeStyle = 'rgba(255,255,255,0.88)';
      g.lineCap = 'round';
      for (let a = 0; a < 6; a++) {
        g.save();
        g.rotate((a * Math.PI) / 3);
        g.lineWidth = 1.35;
        g.beginPath();
        g.moveTo(0, 0);
        g.lineTo(0, -13.5);
        g.stroke();
        g.lineWidth = 0.95;
        const b1 = 5.2 + v * 0.7;
        const b2 = 9.5 + (v % 2) * 0.8;
        for (const s of [-1, 1]) {
          g.beginPath();
          g.moveTo(0, -b1);
          g.lineTo(s * 3.8, -b1 - 2.6);
          g.moveTo(0, -b2);
          g.lineTo(s * 2.8, -b2 - 2.2);
          g.stroke();
        }
        g.restore();
      }
      g.restore();
      out.snowCrystal.push(c);
    }

    // 3. Near-camera bokeh snow (2 sizes)
    for (let v = 0; v < 2; v++) {
      const sz = 48;
      const c = document.createElement('canvas');
      c.width = c.height = sz;
      const g = c.getContext('2d');
      const gr = g.createRadialGradient(sz / 2, sz / 2, sz * 0.08, sz / 2, sz / 2, sz * 0.48);
      gr.addColorStop(0, v === 0 ? 'rgba(255,255,255,0.48)' : 'rgba(255,252,244,0.36)');
      gr.addColorStop(0.55, 'rgba(240,248,255,0.22)');
      gr.addColorStop(0.82, 'rgba(225,240,255,0.06)');
      gr.addColorStop(1, 'rgba(225,240,255,0)');
      g.fillStyle = gr;
      g.beginPath();
      g.arc(sz / 2, sz / 2, sz * 0.48, 0, TAU);
      g.fill();
      out.snowBokeh.push(c);
    }

    // 4. Autumn leaf sprites (3 species x 6 colors)
    function drawLeafShape(g, species, sz) {
      const s = sz * 0.42;
      g.beginPath();
      if (species === 0) {
        // Maple-like 5-lobe leaf
        g.moveTo(0, s * 0.92);
        g.quadraticCurveTo(-s * 0.22, s * 0.56, -s * 0.78, s * 0.48);
        g.quadraticCurveTo(-s * 0.55, s * 0.2, -s * 0.96, -s * 0.04);
        g.quadraticCurveTo(-s * 0.52, -s * 0.14, -s * 0.62, -s * 0.62);
        g.quadraticCurveTo(-s * 0.24, -s * 0.42, 0, -s * 1.0);
        g.quadraticCurveTo(s * 0.24, -s * 0.42, s * 0.62, -s * 0.62);
        g.quadraticCurveTo(s * 0.52, -s * 0.14, s * 0.96, -s * 0.04);
        g.quadraticCurveTo(s * 0.55, s * 0.2, s * 0.78, s * 0.48);
        g.quadraticCurveTo(s * 0.22, s * 0.56, 0, s * 0.92);
      } else if (species === 1) {
        // Oak-like scalloped leaf
        g.moveTo(0, s * 0.95);
        g.bezierCurveTo(-s * 0.36, s * 0.72, -s * 0.68, s * 0.42, -s * 0.42, s * 0.22);
        g.bezierCurveTo(-s * 0.82, s * 0.02, -s * 0.72, -s * 0.34, -s * 0.36, -s * 0.44);
        g.bezierCurveTo(-s * 0.54, -s * 0.74, -s * 0.22, -s * 0.98, 0, -s * 0.96);
        g.bezierCurveTo(s * 0.22, -s * 0.98, s * 0.54, -s * 0.74, s * 0.36, -s * 0.44);
        g.bezierCurveTo(s * 0.72, -s * 0.34, s * 0.82, s * 0.02, s * 0.42, s * 0.22);
        g.bezierCurveTo(s * 0.68, s * 0.42, s * 0.36, s * 0.72, 0, s * 0.95);
      } else {
        // Birch / elm tapered leaf
        g.moveTo(0, s * 0.92);
        g.bezierCurveTo(-s * 0.62, s * 0.52, -s * 0.68, -s * 0.28, 0, -s * 0.98);
        g.bezierCurveTo(s * 0.68, -s * 0.28, s * 0.62, s * 0.52, 0, s * 0.92);
      }
      g.closePath();
    }

    for (let sp = 0; sp < 3; sp++) {
      const list = [];
      for (let ci = 0; ci < AUTUMN_COLORS.length; ci++) {
        const sz = 44;
        const c = document.createElement('canvas');
        c.width = c.height = sz;
        const g = c.getContext('2d');
        g.save();
        g.translate(sz / 2, sz / 2);
        const [hi, lo] = AUTUMN_COLORS[ci];
        const gr = g.createLinearGradient(-12, -16, 12, 18);
        gr.addColorStop(0, rgba(hi, 0.98));
        gr.addColorStop(0.65, rgba(mixRgb(hi, lo, 0.52), 0.96));
        gr.addColorStop(1, rgba(lo, 0.95));
        drawLeafShape(g, sp, sz);
        g.fillStyle = gr;
        g.fill();
        g.strokeStyle = rgba(lo, 0.55);
        g.lineWidth = 0.8;
        g.stroke();

        // Stem & delicate midrib veins
        g.strokeStyle = rgba(mixRgb(lo, [50, 24, 12], 0.45), 0.62);
        g.lineWidth = 1.05;
        g.lineCap = 'round';
        g.beginPath();
        g.moveTo(0, sz * 0.44);
        g.lineTo(0, -sz * 0.34);
        g.stroke();
        g.lineWidth = 0.65;
        for (let k = -1; k <= 1; k += 2) {
          g.beginPath();
          g.moveTo(0, sz * 0.12);
          g.lineTo(k * sz * 0.2, -sz * 0.04);
          g.moveTo(0, -sz * 0.08);
          g.lineTo(k * sz * 0.16, -sz * 0.22);
          g.stroke();
        }
        g.restore();
        list.push(c);
      }
      out.leaves.push(list);
    }

    // 5. Cherry blossom petals (spoon-curved, delicate apical notch)
    for (let ci = 0; ci < BLOSSOM_COLORS.length; ci++) {
      const sz = 32;
      const c = document.createElement('canvas');
      c.width = c.height = sz;
      const g = c.getContext('2d');
      g.save();
      g.translate(sz / 2, sz / 2);
      const [hi, lo] = BLOSSOM_COLORS[ci];
      const gr = g.createRadialGradient(-2, -3, 1, 0, 2, 13);
      gr.addColorStop(0, rgba(hi, 0.98));
      gr.addColorStop(0.7, rgba(mixRgb(hi, lo, 0.45), 0.92));
      gr.addColorStop(1, rgba(lo, 0.86));
      g.beginPath();
      g.moveTo(0, 11);
      g.bezierCurveTo(-8.5, 6, -9.5, -5.5, -3.5, -11.2);
      g.quadraticCurveTo(-1.2, -12.4, 0, -9.0); // sakura notch
      g.quadraticCurveTo(1.2, -12.4, 3.5, -11.2);
      g.bezierCurveTo(9.5, -5.5, 8.5, 6, 0, 11);
      g.closePath();
      g.fillStyle = gr;
      g.fill();
      // Subtle translucent highlight spoon sheen
      g.fillStyle = 'rgba(255,255,255,0.34)';
      g.beginPath();
      g.ellipse(-2.2, -1.5, 2.2, 5.2, -0.25, 0, TAU);
      g.fill();
      g.restore();
      out.petals.push(c);
    }

    // 6. Whole cherry blossom flower on a slender stalk
    {
      const sz = 44;
      const c = document.createElement('canvas');
      c.width = c.height = sz;
      const g = c.getContext('2d');
      g.save();
      g.translate(sz / 2, sz / 2);
      // Thin stalk
      g.strokeStyle = 'rgba(134, 88, 96, 0.72)';
      g.lineWidth = 1.2;
      g.beginPath();
      g.moveTo(0, 2);
      g.quadraticCurveTo(5, 11, 9, 17);
      g.stroke();
      for (let p = 0; p < 5; p++) {
        g.save();
        g.rotate((p * TAU) / 5);
        g.drawImage(out.petals[p % out.petals.length], -8, -17, 16, 17);
        g.restore();
      }
      // Golden-rose stamen core
      const cg = g.createRadialGradient(0, 0, 0, 0, 0, 4.5);
      cg.addColorStop(0, 'rgba(255, 224, 130, 0.95)');
      cg.addColorStop(0.55, 'rgba(226, 126, 152, 0.85)');
      cg.addColorStop(1, 'rgba(226, 126, 152, 0)');
      g.fillStyle = cg;
      g.beginPath();
      g.arc(0, 0, 4.5, 0, TAU);
      g.fill();
      g.restore();
      out.flower = c;
    }

    return out;
  }

  // =========================================================================
  // MAIN ENGINE CLASS
  // =========================================================================
  class QuietSnowEngine {
    constructor(canvas, defaults) {
      this.cv = canvas;
      this.ctx = canvas.getContext('2d', { alpha: false });
      this.P = Object.assign({}, defaults);
      this.DEF = Object.assign({}, defaults);
      this.sprites = createSprites();

      this.W = Math.max(320, innerWidth || 1280);
      this.H = Math.max(200, innerHeight || 720);
      this.scale = 1;
      this.lowMode = false;
      this.simTime = 0;
      this.bgCanvas = document.createElement('canvas');
      this.bgCtx = this.bgCanvas.getContext('2d', { alpha: false });
      this.bgDirty = true;

      // Ground snow heightfield
      this.cellW = 4;
      this.groundN = 0;
      this.groundH = new Float32Array(0);
      this.groundSparkles = [];

      // Tracked icon surfaces & open desktop window surfaces (Z-ordered front-to-back)
      this.surfaces = [];    // icon caps: { id, x, y, w, h, vx, vy, capN, capH, wet }
      this.winSurfaces = []; // open windows: { id, hwnd, x, y, w, h, vx, vy, capN, capH, wet, isWindow: true }
      this.nextSurfId = 1;

      // Active precipitation & physics particles
      this.flakes = [];       // falling snow / leaves / petals
      this.rainStreaks = [];  // falling rain lines
      this.rainBeads = [];    // window/icon running & hanging drops
      this.rainTrails = [];   // thin wet bead trails left behind running drops
      this.splashes = [];     // rain splash droplets
      this.lumps = [];        // falling snow lumps shed from moved/closed icons
      this.powders = [];      // fine powder burst / spindrift particles
      this.heaps = [];        // settled individual leaves / blossom petals (ground & icon)

      // Wind gust state
      this.gustVal = 0;
      this.gustTarget = 0;
      this.gustTimer = 2.0;

      // Pointer tracking for granular sweeping & smearing
      this.ptr = {
        active: false,
        x: -9999,
        y: -9999,
        px: -9999,
        py: -9999,
        vx: 0,
        vy: 0,
        speed: 0,
        down: false,
        lastMove: 0
      };

      this._lastMode = this.P.mode;
      this._allocGround(true);
      this._reconcilePools(true);
    }

    setLowMode(v) {
      const next = !!v;
      if (next !== this.lowMode) {
        this.lowMode = next;
        this._reconcilePools(false);
      }
    }

    getQualityFactor() {
      if (this.lowMode) return 0.45;
      return this.P.quality === 0 ? 0.58 : (this.P.quality === 2 ? 1.25 : 0.92);
    }

    resize(W, H, scale) {
      const prevW = this.W;
      this.W = Math.max(240, W);
      this.H = Math.max(160, H);
      this.scale = scale || 1;
      this.bgDirty = true;

      if (Math.abs(prevW - this.W) > 12 || this.groundN === 0) {
        this._allocGround(this.groundN === 0);
      }
      this._reconcilePools(false);
    }

    applySettings(patch) {
      let modeChanged = false;
      let bgChanged = false;
      let accChanged = false;
      for (const k in patch) {
        if (!(k in this.P) || typeof patch[k] !== 'number') continue;
        if (this.P[k] === patch[k]) continue;
        this.P[k] = patch[k];
        if (k === 'mode') { modeChanged = true; bgChanged = true; }
        if (k === 'atmosphere' || k === 'backdrop') bgChanged = true;
        if (k === 'accumulate' || k === 'adaptive' || k === 'windows') accChanged = true;
      }
      if (bgChanged) this.bgDirty = true;
      if (modeChanged) {
        this._onModeSwitch();
      }
      if (accChanged) {
        if (this.P.accumulate === 0) {
          this.groundH.fill(0);
          for (const s of this.surfaces) { s.capH.fill(0); s.wet = 0; }
          for (const w of this.winSurfaces) { w.capH.fill(0); w.wet = 0; }
          this.heaps.length = 0;
        } else {
          if (this.P.accumulate === 1 || !this.P.adaptive) {
            for (const s of this.surfaces) { s.capH.fill(0); s.wet = 0; }
            this.heaps = this.heaps.filter(h => h.surfId === 0 || h.isWindow);
          }
          if (!this.P.windows) {
            for (const w of this.winSurfaces) { w.capH.fill(0); w.wet = 0; }
            this.heaps = this.heaps.filter(h => !h.isWindow);
          }
        }
      }
      this._reconcilePools(false);
    }

    _onModeSwitch() {
      this._lastMode = this.P.mode;
      this.flakes.length = 0;
      this.rainStreaks.length = 0;
      this.rainBeads.length = 0;
      this.rainTrails.length = 0;
      this.splashes.length = 0;
      this.lumps.length = 0;
      this.powders.length = 0;
      this.heaps.length = 0;
      this.groundH.fill(0);
      for (const s of this.surfaces) {
        s.capH.fill(0);
        s.wet = 0;
      }
      for (const w of this.winSurfaces) {
        w.capH.fill(0);
        w.wet = 0;
      }
      if (this.P.mode === 0 && this.P.accumulate > 0) {
        this._seedInitialSnow();
      } else if ((this.P.mode === 2 || this.P.mode === 3) && this.P.accumulate > 0) {
        this._seedInitialHeaps();
      } else if (this.P.mode === 1) {
        this._seedInitialRainBeads();
      }
      this._reconcilePools(true);
    }

    _allocGround(warmStart) {
      const oldH = this.groundH;
      const oldN = this.groundN;
      this.cellW = this.W > 1600 ? 5 : 4;
      this.groundN = Math.max(40, Math.ceil(this.W / this.cellW) + 1);
      this.groundH = new Float32Array(this.groundN);
      if (oldN > 0 && !warmStart) {
        for (let i = 0; i < this.groundN; i++) {
          const oi = Math.min(oldN - 1, Math.round((i / (this.groundN - 1)) * (oldN - 1)));
          this.groundH[i] = oldH[oi];
        }
      } else if (warmStart && this.P.mode === 0 && this.P.accumulate > 0) {
        this._seedInitialSnow();
      }
      // Re-seed subtle surface sparkle positions
      this.groundSparkles = [];
      const r = mulberry32(409);
      const count = 56;
      for (let i = 0; i < count; i++) {
        this.groundSparkles.push({
          u: r(),
          depthFrac: 0.08 + r() * 0.52,
          phase: r() * TAU,
          freq: 1.4 + r() * 2.6,
          size: 0.9 + r() * 1.4
        });
      }
    }

    _seedInitialSnow() {
      const baseMax = (10 + 18 * this.P.depth) * Math.min(1.15, this.H / 900);
      for (let i = 0; i < this.groundN; i++) {
        const u = i / Math.max(1, this.groundN - 1);
        const drift =
          0.52 +
          0.26 * Math.sin(u * 6.4 + 0.7) +
          0.14 * Math.sin(u * 15.2 + 2.1) +
          0.08 * Math.cos(u * 27.0);
        this.groundH[i] = Math.max(3, baseMax * clamp(drift, 0.25, 1.0));
      }
      for (const s of this.surfaces) {
        this._seedSurfaceCap(s);
      }
      for (const w of this.winSurfaces) {
        this._seedWindowCap(w);
      }
    }

    _seedSurfaceCap(s) {
      if (this.P.mode !== 0 || this.P.accumulate < 2 || !this.P.adaptive) return;
      const capMax = (4.5 + 6.5 * this.P.depth);
      for (let i = 0; i < s.capN; i++) {
        const u = i / Math.max(1, s.capN - 1);
        const env = Math.pow(Math.sin(u * Math.PI), 0.58);
        const lump = 0.82 + 0.18 * Math.sin(u * 9.3 + s.id * 1.7);
        s.capH[i] = capMax * env * lump;
      }
    }

    _seedWindowCap(w) {
      if (this.P.mode !== 0 || this.P.accumulate === 0 || !this.P.windows) return;
      const capMax = 4.0 + 5.5 * this.P.depth;
      for (let i = 0; i < w.capN; i++) {
        const u = i / Math.max(1, w.capN - 1);
        const env = this._windowEdgeEnv(u, w.w);
        const wave = 0.72 + 0.18 * Math.sin(u * 7.5 + (w.hwnd % 17)) + 0.10 * Math.cos(u * 19.0);
        w.capH[i] = capMax * env * wave;
      }
    }

    _windowEdgeEnv(u, width) {
      // Smoothly taper within ~14px of Windows 11 rounded corners
      const pad = Math.min(0.18, 16 / Math.max(60, width));
      const left = smoothstep(0, pad, u);
      const right = smoothstep(0, pad, 1 - u);
      return Math.pow(left * right, 0.72);
    }

    _isOccludedByHigherWindow(x, y, maxIdx) {
      for (let i = 0; i < maxIdx; i++) {
        const w = this.winSurfaces[i];
        if (x >= w.x && x <= w.x + w.w && y >= w.y && y <= w.y + w.h) return true;
      }
      return false;
    }

    _seedInitialHeaps() {
      if (this.P.accumulate === 0) return;
      const mode = this.P.mode;
      const groundCount = Math.round(36 * this.P.depth);
      for (let i = 0; i < groundCount; i++) {
        const x = Math.random() * this.W;
        const y = this.H - 4 - Math.random() * (10 + 14 * this.P.depth);
        this.heaps.push(this._makeHeapItem(mode, x, y, 0, 0, 0));
      }
      if (this.P.accumulate >= 2 && this.P.adaptive) {
        for (const s of this.surfaces) {
          const cnt = Math.min(7, Math.max(3, Math.round(s.w / 14)));
          for (let i = 0; i < cnt; i++) {
            const ox = s.w * (0.14 + 0.72 * Math.random());
            const oy = -1 - Math.random() * 4.5;
            this.heaps.push(this._makeHeapItem(mode, s.x + ox, s.y + oy, s.id, ox, oy, false));
          }
        }
      }
      if (this.P.windows) {
        for (const w of this.winSurfaces) {
          const cnt = Math.min(28, Math.max(6, Math.round(w.w / 32)));
          for (let i = 0; i < cnt; i++) {
            const ox = 12 + (w.w - 24) * Math.random();
            const oy = -1.5 - Math.random() * 5.5;
            this.heaps.push(this._makeHeapItem(mode, w.x + ox, w.y + oy, w.id, ox, oy, true));
          }
        }
      }
    }

    _seedInitialRainBeads() {
      const cnt = Math.round(28 * this.getQualityFactor());
      for (let i = 0; i < cnt; i++) {
        this.rainBeads.push({
          x: Math.random() * this.W,
          y: Math.random() * this.H * 0.85,
          r: 1.4 + Math.random() * 2.8,
          vy: 0,
          vx: 0,
          stuck: Math.random() * 3.5,
          surfId: 0,
          hanging: false,
          mass: 0.4 + Math.random() * 0.6,
          wobble: Math.random() * TAU
        });
      }
    }

    _makeHeapItem(mode, x, y, surfId, ox, oy, isWindow) {
      const isLeaf = mode === 2;
      return {
        mode,
        x,
        y,
        surfId,
        isWindow: !!isWindow,
        ox,
        oy,
        rot: (Math.random() - 0.5) * (isLeaf ? 2.4 : TAU),
        scale: isLeaf ? (0.52 + Math.random() * 0.42) : (0.55 + Math.random() * 0.42),
        squash: isLeaf ? (0.45 + Math.random() * 0.38) : (0.52 + Math.random() * 0.36),
        species: isLeaf ? ((Math.random() * 3) | 0) : 0,
        colorIdx: isLeaf
          ? ((Math.random() * AUTUMN_COLORS.length) | 0)
          : ((Math.random() * BLOSSOM_COLORS.length) | 0),
        age: Math.random() * 6,
        alpha: 0.92
      };
    }

    // =========================================================================
    // ICON SURFACE TRACKING & SHEDDING PHYSICS
    // =========================================================================
    setIcons(list) {
      const incoming = (this.P.adaptive && Array.isArray(list)) ? list : [];
      const prev = this.surfaces;
      const usedPrev = new Set();
      const next = [];

      for (const ic of incoming) {
        if (!ic || ic.w < 12 || ic.h < 12) continue;
        // Match nearest previous surface
        let best = null;
        let bestD2 = 160 * 160;
        for (const p of prev) {
          if (usedPrev.has(p)) continue;
          const dx = p.x - ic.x;
          const dy = p.y - ic.y;
          const d2 = dx * dx + dy * dy;
          if (d2 < bestD2) {
            bestD2 = d2;
            best = p;
          }
        }
        if (best) {
          usedPrev.add(best);
          const dx = ic.x - best.x;
          const dy = ic.y - best.y;
          const moved = Math.hypot(dx, dy);
          if (moved > 5) {
            this._shedSurface(best, dx, dy, moved > 24 ? 1.0 : moved / 24);
          }
          best.vx = dx;
          best.vy = dy;
          best.x = ic.x;
          best.y = ic.y;
          if (Math.abs(best.w - ic.w) > 6) {
            const newN = Math.max(8, Math.ceil(ic.w / 4) + 1);
            const newH = new Float32Array(newN);
            for (let i = 0; i < newN; i++) {
              const oi = Math.min(best.capN - 1, Math.round((i / (newN - 1)) * (best.capN - 1)));
              newH[i] = best.capH[oi];
            }
            best.capN = newN;
            best.capH = newH;
          }
          best.w = ic.w;
          best.h = ic.h;
          next.push(best);
        } else {
          const capN = Math.max(8, Math.ceil(ic.w / 4) + 1);
          const s = {
            id: this.nextSurfId++,
            x: ic.x,
            y: ic.y,
            w: ic.w,
            h: ic.h,
            vx: 0,
            vy: 0,
            capN,
            capH: new Float32Array(capN),
            wet: 0
          };
          if (this.simTime < 1.2) {
            this._seedSurfaceCap(s);
          }
          next.push(s);
        }
      }

      // Any disappeared/minimized icon drops its accumulated snow/leaves/water
      for (const p of prev) {
        if (!usedPrev.has(p)) {
          this._shedSurface(p, 0, 12, 1.0, true);
        }
      }

      this.surfaces = next;
    }

    // =========================================================================
    // OPEN DESKTOP WINDOW SURFACE TRACKING (Z-ordered front-to-back)
    // =========================================================================
    setWindows(list) {
      const incoming = Array.isArray(list) ? list : [];
      const prevByHwnd = new Map();
      for (const p of this.winSurfaces) prevByHwnd.set(p.hwnd, p);
      const seen = new Set();
      const next = [];

      for (let idx = 0; idx < incoming.length; idx++) {
        const w = incoming[idx];
        if (!w || w.w < 60 || w.h < 40) continue;
        const hwnd = w.id;
        seen.add(hwnd);
        const existing = prevByHwnd.get(hwnd);
        if (existing) {
          const dx = w.x - existing.x;
          const dy = w.y - existing.y;
          const moved = Math.hypot(dx, dy);
          // Detect window shake (sudden velocity reversal), downward pull, or fast drag
          const shook = (existing.vx * dx < -24) || (existing.vy * dy < -24);
          if (dy > 3.5 || moved > 9 || shook) {
            // Snow naturally slides & drops off a dragged/lowered/shaken window instead of floating behind it
            const shedFactor = shook
              ? 0.92
              : (dy > 4 ? clamp(0.45 + dy / 22, 0.45, 0.96) : clamp((moved - 6) / 22, 0.25, 0.92));
            this._shedSurface(existing, dx, dy, shedFactor, shedFactor > 0.88);
          } else if (moved > 1.5 && this.P.mode === 0) {
            // Subtle inertia: gently shift remaining snow cap opposite to slow horizontal drag
            const shift = clamp(Math.round(-dx * 0.08), -2, 2);
            if (shift !== 0) {
              const arr = existing.capH;
              const n = existing.capN;
              if (shift > 0) {
                for (let i = n - 1; i >= shift; i--) arr[i] = arr[i - shift] * 0.96;
                for (let i = 0; i < shift; i++) arr[i] *= 0.6;
              } else {
                const sAbs = -shift;
                for (let i = 0; i < n - sAbs; i++) arr[i] = arr[i + sAbs] * 0.96;
                for (let i = n - sAbs; i < n; i++) arr[i] *= 0.6;
              }
            }
          }
          existing.vx = dx;
          existing.vy = dy;
          existing.x = w.x;
          existing.y = w.y;
          // Follow window movement for remaining settled leaves/petals & hanging drops
          if (moved > 0.2) {
            for (const h of this.heaps) {
              if (h.surfId === existing.id) {
                h.x = existing.x + h.ox;
                h.y = existing.y + h.oy;
              }
            }
            for (const b of this.rainBeads) {
              if (b.surfId === existing.id) {
                b.x += dx;
                b.y += dy;
              }
            }
          }
          if (Math.abs(existing.w - w.w) > 8) {
            const newN = clamp(Math.ceil(w.w / 6) + 1, 20, 220);
            const newH = new Float32Array(newN);
            for (let i = 0; i < newN; i++) {
              const oi = Math.min(existing.capN - 1, Math.round((i / (newN - 1)) * (existing.capN - 1)));
              newH[i] = existing.capH[oi];
            }
            existing.capN = newN;
            existing.capH = newH;
          }
          existing.w = w.w;
          existing.h = w.h;
          existing.zOrder = idx;
          next.push(existing);
        } else {
          const capN = clamp(Math.ceil(w.w / 6) + 1, 20, 220);
          const surf = {
            id: 'w:' + hwnd,
            hwnd,
            x: w.x,
            y: w.y,
            w: w.w,
            h: w.h,
            vx: 0,
            vy: 0,
            capN,
            capH: new Float32Array(capN),
            wet: 0,
            isWindow: true,
            zOrder: idx
          };
          // Fresh gentle dusting when a window appears while snowing
          if (this.P.mode === 0 && this.P.accumulate > 0 && this.P.windows) {
            this._seedWindowCap(surf);
          }
          next.push(surf);
        }
      }

      // Closed or minimized windows drop their accumulated snow in lumps!
      for (const p of this.winSurfaces) {
        if (!seen.has(p.hwnd)) {
          this._shedSurface(p, 0, 16, 1.0, true);
        }
      }

      this.winSurfaces = next;
    }

    _shedSurface(s, dx, dy, factor, fullDrop) {
      if (this.P.mode === 0) {
        // Turn icon or window snow cap into falling lumps that burst into powder on impact
        const step = s.isWindow ? Math.max(1, Math.floor(s.capN / 16)) : Math.max(1, Math.floor(s.capN / 7));
        for (let i = 0; i < s.capN; i += step) {
          const h = s.capH[i];
          if (h < 1.2) continue;
          const dropAmt = fullDrop ? h : h * clamp(factor * 0.72, 0.25, 0.92);
          s.capH[i] = Math.max(0, h - dropAmt);
          const u = i / Math.max(1, s.capN - 1);
          const lx = s.x + u * s.w;
          const ly = s.y - h * 0.45;
          if (this.lumps.length < 180) {
            this.lumps.push({
              x: lx,
              y: ly,
              vx: clamp(dx * 0.35, -90, 90) + (Math.random() - 0.5) * 48,
              vy: clamp(dy * 0.22, -30, 45) - Math.random() * 32,
              r: clamp(2.4 + dropAmt * 0.42, 2.2, 9.2),
              mass: dropAmt,
              rot: Math.random() * TAU,
              vrot: (Math.random() - 0.5) * 5,
              srcId: fullDrop ? 0 : s.id
            });
          }
        }
        if (fullDrop) s.capH.fill(0);
      } else if (this.P.mode === 1) {
        // Throw rain beads off moved/removed icon
        for (const b of this.rainBeads) {
          if (b.surfId === s.id) {
            b.surfId = 0;
            b.hanging = false;
            b.stuck = 0;
            b.vx += clamp(dx * 0.6, -120, 120) + (Math.random() - 0.5) * 35;
            b.vy = Math.max(b.vy, 60 + Math.random() * 80);
          }
        }
      } else {
        // Scatter settled leaves / blossom petals back into falling particles
        const kept = [];
        for (const h of this.heaps) {
          if (h.surfId === s.id) {
            if (fullDrop || Math.random() < factor * 0.85) {
              this._spawnFlakeFromHeap(h, clamp(dx * 1.4, -140, 140), -30 - Math.random() * 60);
              continue;
            } else {
              h.x = s.x + h.ox;
              h.y = s.y + h.oy;
            }
          }
          kept.push(h);
        }
        this.heaps = kept;
      }
    }

    _spawnFlakeFromHeap(h, vx, vy) {
      if (this.flakes.length > 520) return;
      this.flakes.push({
        x: h.x,
        y: h.y - 2,
        z: 0.72 + Math.random() * 0.26,
        vx: vx + (Math.random() - 0.5) * 55,
        vy: vy,
        size: h.mode === 2 ? (16 + Math.random() * 11) : (10 + Math.random() * 8),
        rot: h.rot,
        vrot: (Math.random() - 0.5) * 3.2,
        pitch: Math.random() * TAU,
        vpitch: 1.8 + Math.random() * 3.2,
        swayPhase: Math.random() * TAU,
        swayFreq: 1.1 + Math.random() * 1.2,
        swayAmp: 24 + Math.random() * 28,
        species: h.species,
        colorIdx: h.colorIdx,
        kind: 0,
        wholeFlower: false,
        alpha: 0.95
      });
    }

    // =========================================================================
    // POINTER / CURSOR INTERACTION (Sweeping snow, tossing leaves, smearing rain)
    // =========================================================================
    onPointerMove(x, y, down) {
      const p = this.ptr;
      const now = this.simTime;
      if (!p.active) {
        p.px = x;
        p.py = y;
      } else {
        p.px = p.x;
        p.py = p.y;
      }
      p.x = x;
      p.y = y;
      const dt = Math.max(0.012, Math.min(0.08, now - p.lastMove || 0.016));
      p.vx = lerp(p.vx, clamp((p.x - p.px) / dt, -1400, 1400), 0.55);
      p.vy = lerp(p.vy, clamp((p.y - p.py) / dt, -1400, 1400), 0.55);
      p.speed = Math.hypot(p.vx, p.vy);
      p.active = true;
      if (typeof down === 'boolean') p.down = down;
      p.lastMove = now;

      if (this.P.cursor > 0 && p.speed > 18) {
        this._interactAtPointer();
      }
    }

    onPointerDown(x, y) {
      this.onPointerMove(x, y, true);
      if (this.P.cursor > 0) {
        this.ptr.speed = Math.max(this.ptr.speed, 260);
        this._interactAtPointer();
      }
    }

    onPointerUp() {
      this.ptr.down = false;
    }

    onPointerLeave() {
      this.ptr.active = false;
      this.ptr.down = false;
      this.ptr.vx = this.ptr.vy = this.ptr.speed = 0;
    }

    _interactAtPointer() {
      const p = this.ptr;
      const strength = this.P.cursor === 2 ? 1.0 : 0.55;
      const rad = (p.down ? 52 : 38) * (0.75 + 0.35 * strength);
      const speedNorm = clamp(p.speed / 420, 0.18, 2.2) * strength;

      if (this.P.mode === 0) {
        // 1. Sweep ground heightfield
        if (this.P.accumulate > 0 && p.y > this.H - (110 + rad)) {
          const iMin = Math.max(0, Math.floor((p.x - rad) / this.cellW));
          const iMax = Math.min(this.groundN - 1, Math.ceil((p.x + rad) / this.cellW));
          const dir = p.vx >= 0 ? 1 : -1;
          for (let i = iMin; i <= iMax; i++) {
            const cx = i * this.cellW;
            const cy = this.H - this.groundH[i];
            const dist = Math.hypot(cx - p.x, cy - p.y);
            if (dist < rad && this.groundH[i] > 0.5) {
              const falloff = 1 - dist / rad;
              const cut = Math.min(this.groundH[i], falloff * 5.2 * speedNorm);
              this.groundH[i] -= cut;
              // Push part of the snow forward/sideways into a mini berm
              const pushIdx = clamp(i + dir * Math.round(2 + falloff * 4), 0, this.groundN - 1);
              this.groundH[pushIdx] += cut * 0.42;
              // Loft fine snow powder into the air
              if (cut > 0.5 && Math.random() < 0.45 * strength && this.powders.length < 220) {
                this._spawnPowder(cx, cy - 2, p.vx * 0.24 + (Math.random() - 0.5) * 90, -50 - Math.random() * 110 * speedNorm, 1.4 + Math.random() * 2.4);
              }
            }
          }
        }

        // 2. Sweep icon & open window snow caps
        const sweepList = [];
        if (this.P.accumulate >= 2 && this.P.adaptive) {
          for (const s of this.surfaces) sweepList.push(s);
        }
        if (this.P.accumulate > 0 && this.P.windows) {
          for (const w of this.winSurfaces) sweepList.push(w);
        }
        for (const s of sweepList) {
          if (p.x + rad < s.x || p.x - rad > s.x + s.w || p.y + rad < s.y - 44 || p.y - rad > s.y + 24) continue;
          const cellW = s.w / Math.max(1, s.capN - 1);
          const dir = p.vx >= 0 ? 1 : -1;
          for (let i = 0; i < s.capN; i++) {
            const cx = s.x + i * cellW;
            const cy = s.y - s.capH[i];
            const dist = Math.hypot(cx - p.x, cy - p.y);
            if (dist < rad * 0.9 && s.capH[i] > 0.4) {
              const falloff = 1 - dist / (rad * 0.9);
              const cut = Math.min(s.capH[i], falloff * 5.0 * speedNorm);
              s.capH[i] -= cut;
              const pushIdx = i + dir * 2;
              if (pushIdx >= 0 && pushIdx < s.capN) {
                s.capH[pushIdx] += cut * 0.32;
              } else if (cut > 0.8 && this.lumps.length < 180) {
                this.lumps.push({
                  x: cx, y: cy,
                  vx: p.vx * 0.25 + dir * 40,
                  vy: -20 - Math.random() * 40,
                  r: clamp(2 + cut * 0.5, 2, 5.8),
                  mass: cut,
                  rot: Math.random() * TAU,
                  vrot: (Math.random() - 0.5) * 6,
                  srcId: s.id
                });
              }
              if (cut > 0.4 && Math.random() < 0.55 && this.powders.length < 240) {
                this._spawnPowder(cx, cy - 1, p.vx * 0.28 + (Math.random() - 0.5) * 80, -40 - Math.random() * 95 * speedNorm, 1.3 + Math.random() * 2.2);
              }
            }
          }
        }
      } else if (this.P.mode === 1) {
        // Smear rain beads under cursor
        for (const b of this.rainBeads) {
          const dist = Math.hypot(b.x - p.x, b.y - p.y);
          if (dist < rad * 0.95) {
            const f = (1 - dist / (rad * 0.95)) * speedNorm;
            b.stuck = 0;
            b.hanging = false;
            b.vx += p.vx * 0.38 * f + (Math.random() - 0.5) * 50;
            b.vy += p.vy * 0.35 * f + 30 * f;
            b.r = Math.max(1.0, b.r * (1 - 0.14 * f));
            if (Math.random() < 0.35 && this.rainTrails.length < 240) {
              this.rainTrails.push({ x: b.x, y: b.y, r: Math.max(0.7, b.r * 0.45), a: 0.48 });
            }
          }
        }
      } else {
        // Scatter settled autumn leaves or cherry blossom petals
        const kept = [];
        for (const h of this.heaps) {
          const dist = Math.hypot(h.x - p.x, h.y - p.y);
          if (dist < rad * 1.15 && Math.random() < (1 - dist / (rad * 1.15)) * 0.92 * strength) {
            const awayX = (h.x - p.x) / (dist || 1);
            this._spawnFlakeFromHeap(
              h,
              p.vx * 0.32 + awayX * 95 * speedNorm + (Math.random() - 0.5) * 50,
              -70 - Math.random() * 110 * speedNorm
            );
          } else {
            kept.push(h);
          }
        }
        this.heaps = kept;
      }
    }

    _spawnPowder(x, y, vx, vy, r) {
      this.powders.push({
        x, y, vx, vy, r,
        life: 0.45 + Math.random() * 0.75,
        maxLife: 1.0,
        alpha: 0.55 + Math.random() * 0.35
      });
      this.powders[this.powders.length - 1].maxLife = this.powders[this.powders.length - 1].life;
    }

    // =========================================================================
    // PARTICLE POOL MANAGEMENT
    // =========================================================================
    _targetCount() {
      const q = this.getQualityFactor();
      const areaScale = clamp((this.W * this.H) / (1920 * 1080), 0.45, 1.35);
      const inten = clamp(this.P.intensity, 0.1, 1.6);
      if (this.P.mode === 0) return Math.round(380 * q * areaScale * inten);
      if (this.P.mode === 1) return Math.round(240 * q * areaScale * inten);
      if (this.P.mode === 2) return Math.round(84 * q * areaScale * inten);
      return Math.round(145 * q * areaScale * inten);
    }

    _reconcilePools(scatterY) {
      const target = this._targetCount();
      if (this.P.mode === 1) {
        while (this.rainStreaks.length < target) {
          this.rainStreaks.push(this._makeRainStreak(scatterY));
        }
        if (this.rainStreaks.length > target) this.rainStreaks.length = target;
      } else {
        while (this.flakes.length < target) {
          this.flakes.push(this._makeFlake(scatterY));
        }
        if (this.flakes.length > target) this.flakes.length = target;
      }
    }

    _makeFlake(scatterY) {
      const mode = this.P.mode;
      const z = Math.pow(Math.random(), 0.85) * 0.85 + 0.15; // 0.15 (far) .. 1.0 (near)
      const x = Math.random() * (this.W + 240) - 120;
      const y = scatterY ? Math.random() * this.H : (-20 - Math.random() * 90);

      if (mode === 0) {
        // Snow: distant fine dust, mid soft tufts, occasional crystals, rare near bokeh
        let kind = 0; // 0=soft, 1=crystal, 2=bokeh
        let size = lerp(2.2, 8.2, z);
        if (z > 0.88 && Math.random() < 0.34) {
          kind = 2;
          size = lerp(14, 28, Math.random());
        } else if (z > 0.56 && Math.random() < 0.24) {
          kind = 1;
          size = lerp(6.5, 13.5, Math.random());
        }
        return {
          x, y, z,
          vx: (Math.random() - 0.5) * 14,
          vy: lerp(24, 92, z) * (0.82 + Math.random() * 0.36),
          size,
          rot: Math.random() * TAU,
          vrot: (Math.random() - 0.5) * (kind === 1 ? 1.4 : 0.6),
          pitch: 0,
          vpitch: 0,
          swayPhase: Math.random() * TAU,
          swayFreq: 0.55 + Math.random() * 1.15,
          swayAmp: lerp(10, 36, z),
          species: (Math.random() * 4) | 0,
          colorIdx: (Math.random() * 3) | 0,
          kind,
          wholeFlower: false,
          alpha: kind === 2 ? lerp(0.24, 0.46, Math.random()) : lerp(0.32, 0.94, z)
        };
      } else if (mode === 2) {
        // Autumn leaves
        return {
          x, y, z,
          vx: (Math.random() - 0.5) * 24,
          vy: lerp(34, 78, z) * (0.8 + Math.random() * 0.4),
          size: lerp(14, 30, z),
          rot: Math.random() * TAU,
          vrot: (Math.random() - 0.5) * 2.2,
          pitch: Math.random() * TAU,
          vpitch: 1.4 + Math.random() * 2.6,
          swayPhase: Math.random() * TAU,
          swayFreq: 0.75 + Math.random() * 0.85,
          swayAmp: lerp(26, 68, z),
          species: (Math.random() * 3) | 0,
          colorIdx: (Math.random() * AUTUMN_COLORS.length) | 0,
          kind: 0,
          wholeFlower: false,
          alpha: lerp(0.55, 0.98, z)
        };
      } else {
        // Cherry blossom petals + occasional whole flower on stalk
        const whole = z > 0.45 && Math.random() < 0.08;
        return {
          x, y, z,
          vx: (Math.random() - 0.5) * 20,
          vy: lerp(26, 64, z) * (whole ? 1.12 : (0.8 + Math.random() * 0.4)),
          size: whole ? lerp(18, 28, z) : lerp(8, 17, z),
          rot: Math.random() * TAU,
          vrot: (Math.random() - 0.5) * (whole ? 3.2 : 2.4),
          pitch: Math.random() * TAU,
          vpitch: 1.8 + Math.random() * 3.4,
          swayPhase: Math.random() * TAU,
          swayFreq: 0.9 + Math.random() * 1.1,
          swayAmp: lerp(22, 58, z),
          species: 0,
          colorIdx: (Math.random() * BLOSSOM_COLORS.length) | 0,
          kind: 0,
          wholeFlower: whole,
          alpha: lerp(0.52, 0.96, z)
        };
      }
    }

    _makeRainStreak(scatterY) {
      const z = Math.random() * 0.85 + 0.15;
      return {
        x: Math.random() * (this.W + 300) - 150,
        y: scatterY ? Math.random() * this.H : (-30 - Math.random() * 140),
        z,
        speed: lerp(460, 1080, z) * (0.85 + Math.random() * 0.3),
        len: lerp(14, 42, z),
        thick: lerp(0.65, 1.55, z),
        alpha: lerp(0.14, 0.52, z)
      };
    }

    // =========================================================================
    // SIMULATION STEP
    // =========================================================================
    step(dt) {
      this.simTime += dt;
      // Decay pointer velocity when stationary
      if (this.simTime - this.ptr.lastMove > 0.09) {
        this.ptr.vx *= Math.pow(0.04, dt);
        this.ptr.vy *= Math.pow(0.04, dt);
        this.ptr.speed = Math.hypot(this.ptr.vx, this.ptr.vy);
      }

      // Smooth organic wind gusts
      this.gustTimer -= dt;
      if (this.gustTimer <= 0) {
        this.gustTimer = 2.4 + Math.random() * 4.8;
        const gAmp = this.P.gusts;
        this.gustTarget = gAmp > 0.02
          ? (Math.random() < 0.65 ? (Math.sign(this.P.wind || 1) * (0.4 + Math.random() * 0.9)) : ((Math.random() - 0.5) * 1.4)) * gAmp
          : 0;
      }
      this.gustVal = lerp(this.gustVal, this.gustTarget, clamp(dt * 0.85, 0, 1));
      const windPx = (this.P.wind * 72) + (this.gustVal * 115);

      if (this.P.mode === 0) {
        this._stepSnow(dt, windPx);
      } else if (this.P.mode === 1) {
        this._stepRain(dt, windPx);
      } else {
        this._stepFoliage(dt, windPx);
      }

      // Step shared powder / spindrift particles
      for (let i = this.powders.length - 1; i >= 0; i--) {
        const pt = this.powders[i];
        pt.life -= dt;
        if (pt.life <= 0) {
          this.powders.splice(i, 1);
          continue;
        }
        pt.vx = lerp(pt.vx, windPx * 0.75, clamp(dt * 2.2, 0, 1));
        pt.vy += 125 * dt;
        pt.x += pt.vx * dt;
        pt.y += pt.vy * dt;
      }
    }

    _stepSnow(dt, windPx) {
      const W = this.W, H = this.H;
      const t = this.simTime;
      const maxGround = (22 + 54 * this.P.depth) * clamp(H / 900, 0.55, 1.25);
      const maxCap = 9 + 14 * this.P.depth;
      const maxWinCap = 12 + 16 * this.P.depth;
      const accGround = this.P.accumulate >= 1;
      const accIcons = this.P.accumulate >= 2 && this.P.adaptive && this.surfaces.length > 0;
      const accWins = this.P.accumulate >= 1 && !!this.P.windows && this.winSurfaces.length > 0;
      const ptrOn = this.P.cursor > 0 && this.ptr.active && this.ptr.speed > 20;
      const ptrRad = this.P.cursor === 2 ? 78 : 52;

      for (let i = 0; i < this.flakes.length; i++) {
        const f = this.flakes[i];
        const turb = Math.sin(t * f.swayFreq + f.swayPhase + f.y * 0.008) * f.swayAmp;
        const targetVx = windPx * (0.45 + 0.65 * f.z) + turb;
        f.vx = lerp(f.vx, targetVx, clamp(dt * 2.8, 0, 1));

        // Cursor wake turbulence on airborne flakes
        if (ptrOn && f.z > 0.35) {
          const dx = f.x - this.ptr.x;
          const dy = f.y - this.ptr.y;
          const d2 = dx * dx + dy * dy;
          if (d2 < ptrRad * ptrRad) {
            const d = Math.sqrt(d2) || 1;
            const push = (1 - d / ptrRad) * (this.P.cursor === 2 ? 1.0 : 0.55);
            f.vx += (this.ptr.vx * 0.38 + (dx / d) * 110) * push * dt * 8;
            f.vy += (this.ptr.vy * 0.25 + (dy / d) * 65) * push * dt * 5;
          }
        }

        // Relax vy back toward natural terminal fall speed
        const naturalVy = lerp(24, 92, f.z);
        f.vy = lerp(f.vy, naturalVy, clamp(dt * 1.4, 0, 1));
        f.x += f.vx * dt;
        f.y += f.vy * dt;
        f.rot += f.vrot * dt;

        // Wrap horizontally
        if (f.x < -120) f.x = W + 110;
        else if (f.x > W + 120) f.x = -110;

        // Check open window top edges first (front-to-back Z-order)
        let settled = false;
        if (accWins && f.kind !== 2 && f.z > 0.20 && f.vy > 0) {
          for (let wi = 0; wi < this.winSurfaces.length; wi++) {
            const w = this.winSurfaces[wi];
            if (f.x < w.x + 5 || f.x > w.x + w.w - 5) continue;
            const u = (f.x - w.x) / w.w;
            const ci = clamp(Math.round(u * (w.capN - 1)), 0, w.capN - 1);
            const surfY = w.y - w.capH[ci];
            if (f.y >= surfY - 3 && f.y <= w.y + 10) {
              if (!this._isOccludedByHigherWindow(f.x, w.y, wi)) {
                const env = this._windowEdgeEnv(u, w.w);
                if (w.capH[ci] < maxWinCap * env) {
                  const add = (1.55 + 1.85 * f.z) * (0.65 + 0.72 * this.P.depth);
                  this._depositWindowCap(w, ci, add, maxWinCap);
                }
                settled = true;
                break;
              }
            }
          }
        }

        // Check icon top accumulation (skip icons hidden behind open windows)
        if (!settled && accIcons && f.kind !== 2 && f.z > 0.24 && f.vy > 0) {
          for (let si = 0; si < this.surfaces.length; si++) {
            const s = this.surfaces[si];
            if (f.x < s.x + 2 || f.x > s.x + s.w - 2) continue;
            const u = (f.x - s.x) / s.w;
            const ci = clamp(Math.round(u * (s.capN - 1)), 0, s.capN - 1);
            const surfY = s.y - s.capH[ci];
            if (f.y >= surfY - 2 && f.y <= s.y + 8) {
              if (accWins && this._isOccludedByHigherWindow(f.x, s.y, this.winSurfaces.length)) continue;
              const env = Math.pow(Math.sin(u * Math.PI), 0.55);
              const limit = maxCap * env;
              if (s.capH[ci] < limit) {
                const add = (1.05 + 1.35 * f.z) * (0.60 + 0.70 * this.P.depth);
                this._depositCap(s, ci, add, maxCap);
              }
              settled = true;
              break;
            }
          }
        }

        // Check ground accumulation
        if (!settled) {
          const gi = clamp(Math.round(f.x / this.cellW), 0, this.groundN - 1);
          const groundY = accGround ? (H - this.groundH[gi]) : (H + 20);
          if (f.y >= groundY) {
            if (accGround && f.kind !== 2 && f.z > 0.22) {
              const add = (1.15 + 1.45 * f.z) * (0.60 + 0.72 * this.P.depth);
              this._depositGround(gi, add, maxGround);
            }
            settled = true;
          }
        }

        if (settled || f.y > H + 40) {
          const nf = this._makeFlake(false);
          this.flakes[i] = nf;
        }
      }

      // Step falling snow lumps shed from icons
      for (let i = this.lumps.length - 1; i >= 0; i--) {
        const l = this.lumps[i];
        l.vy += 680 * dt;
        l.vx = lerp(l.vx, windPx * 0.4, clamp(dt * 1.2, 0, 1));
        l.x += l.vx * dt;
        l.y += l.vy * dt;
        l.rot += l.vrot * dt;

        let hit = false;
        // Can land on an open window below
        if (accWins && l.vy > 40) {
          for (let wi = 0; wi < this.winSurfaces.length; wi++) {
            const w = this.winSurfaces[wi];
            if (w.id === l.srcId) continue;
            if (l.x >= w.x + 6 && l.x <= w.x + w.w - 6 && l.y >= w.y - 8 && l.y <= w.y + 12) {
              if (!this._isOccludedByHigherWindow(l.x, w.y, wi)) {
                const ci = clamp(Math.round(((l.x - w.x) / w.w) * (w.capN - 1)), 0, w.capN - 1);
                this._depositWindowCap(w, ci, l.mass * 0.65, maxWinCap);
                this._burstPowder(l.x, w.y - w.capH[ci], Math.min(9, Math.round(4 + l.r)));
                hit = true;
                break;
              }
            }
          }
        }
        // Can land on another icon below
        if (!hit && accIcons && l.vy > 40) {
          for (const s of this.surfaces) {
            if (s.id === l.srcId) continue;
            if (l.x >= s.x + 3 && l.x <= s.x + s.w - 3 && l.y >= s.y - 6 && l.y <= s.y + 10) {
              const ci = clamp(Math.round(((l.x - s.x) / s.w) * (s.capN - 1)), 0, s.capN - 1);
              this._depositCap(s, ci, l.mass * 0.55, maxCap);
              this._burstPowder(l.x, s.y - s.capH[ci], Math.min(8, Math.round(3 + l.r)));
              hit = true;
              break;
            }
          }
        }
        const gi = clamp(Math.round(l.x / this.cellW), 0, this.groundN - 1);
        const gy = H - this.groundH[gi];
        if (!hit && l.y >= gy) {
          if (accGround) this._depositGround(gi, l.mass * 0.75, maxGround);
          this._burstPowder(l.x, gy - 2, Math.min(10, Math.round(4 + l.r)));
          hit = true;
        }
        if (hit || l.y > H + 40) {
          this.lumps.splice(i, 1);
        }
      }

      // Granular relaxation (angle-of-repose avalanche) & balanced natural melt
      // Keep fresh/cleared drifts accumulating briskly; melt gently prevents overflow on deep drifts
      const meltRate = this.P.melt > 0.01 ? (0.016 + this.P.melt * 0.095) * dt : 0;
      if (accGround) {
        this._relaxArray(this.groundH, 1.35, meltRate, maxGround);
      }
      if (accIcons) {
        for (const s of this.surfaces) {
          this._relaxCap(s, 1.05, meltRate, maxCap);
        }
      }
      if (accWins) {
        for (let wi = 0; wi < this.winSurfaces.length; wi++) {
          this._relaxWindowCap(this.winSurfaces[wi], wi, 1.25, meltRate, maxWinCap);
        }
      }

      // Spindrift: when gusts blow over snow drifts, lift delicate powder wisps
      if (accGround && Math.abs(windPx) > 48 && this.P.gusts > 0.15 && Math.random() < clamp(Math.abs(windPx) / 260, 0.08, 0.55) && this.powders.length < 180) {
        const gi = (Math.random() * this.groundN) | 0;
        if (this.groundH[gi] > 5) {
          this._spawnPowder(gi * this.cellW, H - this.groundH[gi] - 1, windPx * (0.9 + Math.random() * 0.5), -18 - Math.random() * 34, 1.2 + Math.random() * 1.6);
        }
      }
    }

    _depositGround(ci, amount, maxH) {
      const h = this.groundH;
      const n = this.groundN;
      const weights = [0.16, 0.34, 0.52, 0.34, 0.16];
      for (let k = -2; k <= 2; k++) {
        const idx = ci + k;
        if (idx >= 0 && idx < n) {
          h[idx] = Math.min(maxH, h[idx] + amount * weights[k + 2]);
        }
      }
    }

    _depositCap(s, ci, amount, maxCap) {
      const h = s.capH;
      const n = s.capN;
      const weights = [0.24, 0.56, 0.24];
      for (let k = -1; k <= 1; k++) {
        const idx = ci + k;
        if (idx >= 0 && idx < n) {
          const u = idx / Math.max(1, n - 1);
          const env = Math.pow(Math.sin(u * Math.PI), 0.55);
          h[idx] = Math.min(maxCap * env, h[idx] + amount * weights[k + 1]);
        }
      }
    }

    _depositWindowCap(w, ci, amount, maxCap) {
      const h = w.capH;
      const n = w.capN;
      const weights = [0.10, 0.24, 0.44, 0.64, 0.44, 0.24, 0.10];
      for (let k = -3; k <= 3; k++) {
        const idx = ci + k;
        if (idx >= 0 && idx < n) {
          const u = idx / Math.max(1, n - 1);
          const env = this._windowEdgeEnv(u, w.w);
          h[idx] = Math.min(maxCap * env, h[idx] + amount * weights[k + 3]);
        }
      }
    }

    _relaxWindowCap(w, winIdx, maxSlope, melt, maxCap) {
      const arr = w.capH;
      const n = w.capN;
      for (let i = 0; i < n - 1; i++) {
        const diff = arr[i] - arr[i + 1];
        if (Math.abs(diff) > maxSlope) {
          const move = (diff - Math.sign(diff) * maxSlope) * 0.36;
          arr[i] -= move;
          arr[i + 1] += move;
        }
      }
      for (let i = n - 2; i >= 0; i--) {
        const diff = arr[i] - arr[i + 1];
        if (Math.abs(diff) > maxSlope) {
          const move = (diff - Math.sign(diff) * maxSlope) * 0.36;
          arr[i] -= move;
          arr[i + 1] += move;
        }
      }
      const step = w.w / Math.max(1, n - 1);
      for (let i = 0; i < n; i++) {
        const u = i / Math.max(1, n - 1);
        if (winIdx > 0 && this._isOccludedByHigherWindow(w.x + i * step, w.y, winIdx)) {
          arr[i] *= 0.72;
          if (arr[i] < 0.25) arr[i] = 0;
          continue;
        }
        const lim = maxCap * this._windowEdgeEnv(u, w.w);
        if (arr[i] > lim) arr[i] = lerp(arr[i], lim, 0.25);
        if (melt > 0 && arr[i] > 0) {
          const depthRatio = smoothstep(lim * 0.30, lim * 0.85, arr[i]);
          arr[i] = Math.max(0, arr[i] - melt * (0.08 + 1.25 * depthRatio));
        }
      }
    }

    _relaxArray(arr, maxSlope, melt, cap) {
      const n = arr.length;
      if (n < 3) return;
      // Two alternating passes keep drifts organic & smooth without flat-topping
      for (let i = 0; i < n - 1; i++) {
        const diff = arr[i] - arr[i + 1];
        if (Math.abs(diff) > maxSlope) {
          const move = (diff - Math.sign(diff) * maxSlope) * 0.34;
          arr[i] -= move;
          arr[i + 1] += move;
        }
      }
      for (let i = n - 2; i >= 0; i--) {
        const diff = arr[i] - arr[i + 1];
        if (Math.abs(diff) > maxSlope) {
          const move = (diff - Math.sign(diff) * maxSlope) * 0.34;
          arr[i] -= move;
          arr[i + 1] += move;
        }
      }
      if (melt > 0) {
        for (let i = 0; i < n; i++) {
          if (arr[i] > 0) {
            const depthRatio = smoothstep(cap * 0.30, cap * 0.85, arr[i]);
            arr[i] = Math.max(0, arr[i] - melt * (0.08 + 1.35 * depthRatio));
          }
        }
      }
    }

    _relaxCap(s, maxSlope, melt, maxCap) {
      const arr = s.capH;
      const n = s.capN;
      for (let i = 0; i < n - 1; i++) {
        const diff = arr[i] - arr[i + 1];
        if (Math.abs(diff) > maxSlope) {
          const move = (diff - Math.sign(diff) * maxSlope) * 0.36;
          arr[i] -= move;
          arr[i + 1] += move;
        }
      }
      for (let i = 0; i < n; i++) {
        const u = i / Math.max(1, n - 1);
        const env = Math.pow(Math.sin(u * Math.PI), 0.52);
        const lim = maxCap * env;
        if (arr[i] > lim) arr[i] = lerp(arr[i], lim, 0.25);
        if (melt > 0 && arr[i] > 0) {
          const depthRatio = smoothstep(lim * 0.30, lim * 0.85, arr[i]);
          arr[i] = Math.max(0, arr[i] - melt * (0.08 + 1.25 * depthRatio));
        }
      }
    }

    _burstPowder(x, y, count) {
      for (let i = 0; i < count; i++) {
        if (this.powders.length >= 240) break;
        const ang = -Math.PI * (0.1 + Math.random() * 0.8);
        const spd = 28 + Math.random() * 88;
        this._spawnPowder(
          x + (Math.random() - 0.5) * 10,
          y,
          Math.cos(ang) * spd,
          Math.sin(ang) * spd,
          1.3 + Math.random() * 2.5
        );
      }
    }

    // =========================================================================
    // RAIN SIMULATION (Streaks, Splashes, Beaded Glass Drops & Hanging Drops)
    // =========================================================================
    _stepRain(dt, windPx) {
      const W = this.W, H = this.H;
      const accIcons = this.P.accumulate >= 2 && this.P.adaptive && this.surfaces.length > 0;
      const accWins = this.P.accumulate >= 1 && !!this.P.windows && this.winSurfaces.length > 0;
      const angleX = windPx * 1.35;

      // 1. Falling rain streaks
      for (let i = 0; i < this.rainStreaks.length; i++) {
        const r = this.rainStreaks[i];
        const vx = angleX * (0.45 + 0.55 * r.z);
        r.x += vx * dt;
        r.y += r.speed * dt;
        if (r.x < -160) r.x = W + 140;
        else if (r.x > W + 160) r.x = -140;

        let hit = false;
        // Splashes on open window top edges + hanging droplets along bottom edges
        if (accWins && r.z > 0.30) {
          for (let wi = 0; wi < this.winSurfaces.length; wi++) {
            const w = this.winSurfaces[wi];
            if (r.x >= w.x + 6 && r.x <= w.x + w.w - 6 && r.y >= w.y - 2 && r.y <= w.y + 18) {
              if (!this._isOccludedByHigherWindow(r.x, w.y, wi)) {
                w.wet = Math.min(1, w.wet + 0.06);
                this._spawnSplash(r.x, w.y - 1, 0.88);
                // Occasionally spawn a hanging droplet under the window's bottom edge
                if (Math.random() < 0.08 && this.rainBeads.length < 165) {
                  this.rainBeads.push({
                    x: w.x + 14 + Math.random() * Math.max(10, w.w - 28),
                    y: w.y + w.h + 1,
                    r: 1.5 + Math.random() * 1.4,
                    vx: 0,
                    vy: 0,
                    stuck: 0,
                    surfId: w.id,
                    hanging: true,
                    mass: 0.45,
                    wobble: Math.random() * TAU
                  });
                }
                hit = true;
                break;
              }
            }
          }
        }
        if (!hit && accIcons && r.z > 0.35) {
          for (let si = 0; si < this.surfaces.length; si++) {
            const s = this.surfaces[si];
            if (r.x >= s.x + 2 && r.x <= s.x + s.w - 2 && r.y >= s.y && r.y <= s.y + 18) {
              if (accWins && this._isOccludedByHigherWindow(r.x, s.y, this.winSurfaces.length)) continue;
              s.wet = Math.min(1, s.wet + 0.08);
              this._spawnSplash(r.x, s.y, 0.72);
              // Spatter glass / spawn running bead on icon
              if (Math.random() < 0.24 && this.rainBeads.length < 165) {
                this.rainBeads.push({
                  x: r.x,
                  y: s.y + 2 + Math.random() * 6,
                  r: 1.3 + Math.random() * 2.1,
                  vx: 0,
                  vy: 0,
                  stuck: 0.4 + Math.random() * 2.2,
                  surfId: s.id,
                  hanging: false,
                  mass: 0.4 + Math.random() * 0.5,
                  wobble: Math.random() * TAU
                });
              }
              hit = true;
              break;
            }
          }
        }
        if (!hit && r.y >= H - 3) {
          if (r.z > 0.32 && Math.random() < 0.65) {
            this._spawnSplash(r.x, H - 2, 0.95);
          }
          hit = true;
        }
        if (hit) {
          this.rainStreaks[i] = this._makeRainStreak(false);
        }
      }

      // Periodically spatter fine glass beads across the screen
      const maxBeads = Math.round((this.lowMode ? 55 : 130) * clamp(this.P.intensity, 0.3, 1.4));
      if (this.rainBeads.length < maxBeads && Math.random() < dt * 16 * this.P.intensity) {
        this.rainBeads.push({
          x: Math.random() * W,
          y: Math.random() * H * 0.78,
          r: 1.1 + Math.random() * 2.5,
          vx: 0,
          vy: 0,
          stuck: 0.5 + Math.random() * 3.0,
          surfId: 0,
          hanging: false,
          mass: 0.35 + Math.random() * 0.65,
          wobble: Math.random() * TAU
        });
      }

      // 2. Running beads, merging, trails & hanging drops under icon bottom edges
      for (let i = this.rainBeads.length - 1; i >= 0; i--) {
        const b = this.rainBeads[i];
        b.wobble += dt * 5;
        if (b.hanging) {
          b.r += dt * (0.28 + 0.35 * this.P.intensity);
          if (b.r > 4.2 + Math.random() * 1.4) {
            // Drop gets heavy enough to detach
            b.hanging = false;
            b.surfId = 0;
            b.vy = 45;
          }
          continue;
        }

        if (b.stuck > 0) {
          b.stuck -= dt * (0.7 + 0.5 * b.r * 0.35);
          // Slight chance of coalescing with nearby micro-spray grows the drop
          if (Math.random() < dt * 0.8 * this.P.intensity) b.r = Math.min(5.2, b.r + 0.25);
          if (b.r > 3.1) b.stuck = 0;
          continue;
        }

        // Lurch downwards with surface-tension micro-jitter
        const grav = b.surfId ? 140 : 210;
        b.vy = clamp(b.vy + grav * dt * (0.5 + b.r * 0.25), 0, 260 + b.r * 45);
        b.vx = lerp(b.vx, Math.sin(b.wobble) * 14 + windPx * 0.08, clamp(dt * 4, 0, 1));
        b.x += b.vx * dt;
        b.y += b.vy * dt;

        // Leave beaded wet trail behind moving drops
        if (b.vy > 32 && Math.random() < dt * 15 && this.rainTrails.length < 260) {
          this.rainTrails.push({ x: b.x + (Math.random() - 0.5) * 1.2, y: b.y - b.r, r: Math.max(0.65, b.r * 0.34), a: 0.42 });
          b.r = Math.max(1.1, b.r - 0.04);
        }

        // Catch on specks occasionally
        if (b.r < 3.0 && Math.random() < dt * 1.4) {
          b.vy *= 0.12;
          b.stuck = 0.25 + Math.random() * 1.1;
        }

        // Gather smaller beads along its path
        for (let j = i - 1; j >= 0; j--) {
          const o = this.rainBeads[j];
          if (o.hanging) continue;
          const dx = b.x - o.x;
          const dy = b.y - o.y;
          const sr = b.r + o.r + 1.5;
          if (dx * dx + dy * dy < sr * sr) {
            b.r = Math.min(5.8, Math.hypot(b.r, o.r));
            b.vy += 22;
            b.stuck = 0;
            this.rainBeads.splice(j, 1);
            i--;
          }
        }

        // Hang under bottom edge of icon when reaching it
        if (b.surfId) {
          const s = this.surfaces.find(u => u.id === b.surfId);
          if (!s) {
            b.surfId = 0;
          } else if (b.y >= s.y + s.h - 2) {
            b.y = s.y + s.h + 1;
            b.vy = 0;
            b.vx = 0;
            b.hanging = true;
          }
        }

        if (b.y >= H - 2) {
          this._spawnSplash(b.x, H - 2, 0.55);
          this.rainBeads.splice(i, 1);
        }
      }

      // Evaporate wet trails & icon wetness
      const evap = (0.14 + this.P.melt * 0.35) * dt;
      for (let i = this.rainTrails.length - 1; i >= 0; i--) {
        this.rainTrails[i].a -= evap;
        if (this.rainTrails[i].a <= 0.02) this.rainTrails.splice(i, 1);
      }
      for (const s of this.surfaces) {
        s.wet = Math.max(0, s.wet - dt * 0.18);
      }
      for (const w of this.winSurfaces) {
        w.wet = Math.max(0, w.wet - dt * 0.16);
      }

      // 3. Splashes
      for (let i = this.splashes.length - 1; i >= 0; i--) {
        const sp = this.splashes[i];
        sp.life -= dt;
        if (sp.life <= 0) { this.splashes.splice(i, 1); continue; }
        sp.vy += 420 * dt;
        sp.x += sp.vx * dt;
        sp.y += sp.vy * dt;
      }
    }

    _spawnSplash(x, y, scale) {
      const cnt = this.lowMode ? 2 : 3;
      for (let k = 0; k < cnt; k++) {
        if (this.splashes.length > 160) break;
        const ang = -Math.PI * (0.15 + Math.random() * 0.7);
        const spd = (45 + Math.random() * 95) * scale;
        this.splashes.push({
          x, y,
          vx: Math.cos(ang) * spd,
          vy: Math.sin(ang) * spd,
          r: (0.9 + Math.random() * 1.1) * scale,
          life: 0.18 + Math.random() * 0.18,
          maxLife: 0.32
        });
      }
    }

    // =========================================================================
    // AUTUMN LEAVES & CHERRY BLOSSOM SIMULATION
    // =========================================================================
    _stepFoliage(dt, windPx) {
      const W = this.W, H = this.H;
      const t = this.simTime;
      const mode = this.P.mode;
      const accGround = this.P.accumulate >= 1;
      const accIcons = this.P.accumulate >= 2 && this.P.adaptive && this.surfaces.length > 0;
      const accWins = this.P.accumulate >= 1 && !!this.P.windows && this.winSurfaces.length > 0;
      const maxHeaps = Math.round((this.lowMode ? 140 : 380) * (0.5 + 0.75 * this.P.depth));
      const ptrOn = this.P.cursor > 0 && this.ptr.active && this.ptr.speed > 20;
      const ptrRad = this.P.cursor === 2 ? 90 : 60;

      for (let i = 0; i < this.flakes.length; i++) {
        const f = this.flakes[i];
        // Pendulum glide & aerodynamic flutter
        const glide = Math.sin(t * f.swayFreq + f.swayPhase) * f.swayAmp;
        const lift = Math.cos(t * f.swayFreq * 2 + f.swayPhase) * (mode === 2 ? 18 : 14);
        const targetVx = windPx * (0.55 + 0.65 * f.z) + glide;
        f.vx = lerp(f.vx, targetVx, clamp(dt * 2.4, 0, 1));
        const baseVy = (mode === 2 ? lerp(34, 76, f.z) : lerp(24, 60, f.z)) + lift;
        f.vy = lerp(f.vy, baseVy, clamp(dt * 1.8, 0, 1));

        if (ptrOn && f.z > 0.35) {
          const dx = f.x - this.ptr.x;
          const dy = f.y - this.ptr.y;
          const d2 = dx * dx + dy * dy;
          if (d2 < ptrRad * ptrRad) {
            const d = Math.sqrt(d2) || 1;
            const push = (1 - d / ptrRad) * (this.P.cursor === 2 ? 1.0 : 0.55);
            f.vx += (this.ptr.vx * 0.45 + (dx / d) * 140) * push * dt * 8;
            f.vy += (this.ptr.vy * 0.35 - 90) * push * dt * 5;
            f.vrot += (Math.random() - 0.5) * 6 * push;
          }
        }

        f.x += f.vx * dt;
        f.y += f.vy * dt;
        f.rot += f.vrot * dt;
        f.pitch += f.vpitch * dt;

        if (f.x < -120) f.x = W + 110;
        else if (f.x > W + 120) f.x = -110;

        let settled = false;
        // Settle on open window top edges (front-to-back Z-order)
        if (accWins && f.z > 0.36 && f.vy > 0) {
          for (let wi = 0; wi < this.winSurfaces.length; wi++) {
            const w = this.winSurfaces[wi];
            if (f.x >= w.x + 10 && f.x <= w.x + w.w - 10 && f.y >= w.y - 10 && f.y <= w.y + 8) {
              if (!this._isOccludedByHigherWindow(f.x, w.y, wi)) {
                if (this.heaps.length >= maxHeaps) this.heaps.shift();
                const ox = f.x - w.x;
                const oy = -1.5 - Math.random() * (5 + 8 * this.P.depth);
                this.heaps.push({
                  mode, x: w.x + ox, y: w.y + oy, surfId: w.id, isWindow: true, ox, oy,
                  rot: f.rot,
                  scale: clamp(f.size / (mode === 2 ? 30 : 19), 0.5, 0.96),
                  squash: 0.45 + Math.random() * 0.35,
                  species: f.species,
                  colorIdx: f.colorIdx,
                  age: 0,
                  alpha: 0.95
                });
                settled = true;
                break;
              }
            }
          }
        }
        // Settle on icon top edges
        if (!settled && accIcons && f.z > 0.42 && f.vy > 0) {
          for (let si = 0; si < this.surfaces.length; si++) {
            const s = this.surfaces[si];
            if (f.x >= s.x + 6 && f.x <= s.x + s.w - 6 && f.y >= s.y - 6 && f.y <= s.y + 8) {
              if (accWins && this._isOccludedByHigherWindow(f.x, s.y, this.winSurfaces.length)) continue;
              if (this.heaps.length < maxHeaps) {
                const ox = f.x - s.x;
                const oy = -1 - Math.random() * (4 + 6 * this.P.depth);
                this.heaps.push({
                  mode, x: s.x + ox, y: s.y + oy, surfId: s.id, isWindow: false, ox, oy,
                  rot: f.rot,
                  scale: clamp(f.size / (mode === 2 ? 32 : 20), 0.48, 0.95),
                  squash: 0.45 + Math.random() * 0.35,
                  species: f.species,
                  colorIdx: f.colorIdx,
                  age: 0,
                  alpha: 0.94
                });
              }
              settled = true;
              break;
            }
          }
        }

        // Settle in individual leaf/petal heaps at bottom
        if (!settled && f.y >= H - 4 - Math.random() * (12 * this.P.depth)) {
          if (accGround && f.z > 0.35) {
            if (this.heaps.length >= maxHeaps) {
              this.heaps.shift();
            }
            const pileDepth = 5 + 22 * this.P.depth * (0.4 + 0.6 * Math.sin(f.x * 0.008) * Math.sin(f.x * 0.008));
            this.heaps.push({
              mode,
              x: f.x,
              y: H - 4 - Math.random() * pileDepth,
              surfId: 0,
              ox: 0,
              oy: 0,
              rot: f.rot,
              scale: clamp(f.size / (mode === 2 ? 30 : 18), 0.5, 0.98),
              squash: 0.42 + Math.random() * 0.38,
              species: f.species,
              colorIdx: f.colorIdx,
              age: 0,
              alpha: 0.95
            });
          }
          settled = true;
        }

        if (settled || f.y > H + 40) {
          this.flakes[i] = this._makeFlake(false);
        }
      }

      // Wither leaves / wilt cherry blossom petals gently when melt > 0
      if (this.P.melt > 0.01) {
        const rate = (0.2 + this.P.melt * 1.1) * dt;
        for (let i = this.heaps.length - 1; i >= 0; i--) {
          const h = this.heaps[i];
          h.age += rate;
          if (h.age > 14) {
            h.alpha -= dt * (0.14 + this.P.melt * 0.25);
            if (h.alpha <= 0.04) this.heaps.splice(i, 1);
          }
        }
      }
    }

    // =========================================================================
    // OFFSCREEN ATMOSPHERE BACKDROP RENDERER
    // =========================================================================
    _buildBackdrop() {
      const W = this.W, H = this.H, S = this.scale;
      this.bgCanvas.width = Math.max(1, Math.round(W * S));
      this.bgCanvas.height = Math.max(1, Math.round(H * S));
      const g = this.bgCtx;
      g.setTransform(S, 0, 0, S, 0, 0);

      const atm = ATMOS[this.P.atmosphere] || ATMOS[0];
      const skyG = g.createLinearGradient(0, 0, 0, H);
      skyG.addColorStop(0, atm.sky[0]);
      skyG.addColorStop(0.42, atm.sky[1]);
      skyG.addColorStop(0.76, atm.sky[2]);
      skyG.addColorStop(1, atm.sky[3]);
      g.fillStyle = skyG;
      g.fillRect(0, 0, W, H);

      // Gentle seasonal mood tint
      const mt = MODE_TINT[this.P.mode];
      if (mt) {
        g.fillStyle = 'rgba(' + mt[0] + ',' + mt[1] + ',' + mt[2] + ',' + mt[3] + ')';
        g.fillRect(0, 0, W, H);
      }

      const r = mulberry32(2026 + this.P.atmosphere * 97);
      const bd = clamp(this.P.backdrop, 0, 1);

      // Subtle distant stars in dark atmospheres
      if (atm.stars > 0 && bd > 0.1) {
        const nStars = Math.round(95 * atm.stars * bd);
        for (let i = 0; i < nStars; i++) {
          const sx = r() * W;
          const sy = r() * H * 0.56;
          const sr = 0.5 + r() * 1.15;
          const sa = (0.14 + r() * 0.42) * atm.stars * (1 - sy / (H * 0.65));
          g.fillStyle = 'rgba(235,244,255,' + sa.toFixed(3) + ')';
          g.beginPath();
          g.arc(sx, sy, sr, 0, TAU);
          g.fill();
        }
      }

      // Soft horizon atmospheric glow
      const hGlow = g.createRadialGradient(W * 0.52, H * 0.78, W * 0.05, W * 0.52, H * 0.78, Math.max(W, H) * 0.78);
      hGlow.addColorStop(0, rgba(atm.haze, atm.lightBg ? 0.32 : 0.24));
      hGlow.addColorStop(1, rgba(atm.haze, 0));
      g.fillStyle = hGlow;
      g.fillRect(0, 0, W, H);

      // Distant atmospheric ridge & misty pine silhouette (soft, non-distracting)
      if (bd > 0.04) {
        const drawRidge = (baseY, amp, freq, phase, color, alpha) => {
          g.fillStyle = rgba(color, alpha * bd);
          g.beginPath();
          g.moveTo(0, H);
          const step = Math.max(12, Math.round(W / 90));
          for (let x = 0; x <= W + step; x += step) {
            const u = x / W;
            const y = baseY
              - Math.sin(u * freq + phase) * amp
              - Math.sin(u * freq * 2.3 + phase * 1.7) * (amp * 0.42)
              - Math.cos(u * freq * 5.1 + phase * 0.6) * (amp * 0.16);
            g.lineTo(x, y);
          }
          g.lineTo(W, H);
          g.closePath();
          g.fill();
        };

        drawRidge(H * 0.74, H * 0.075, 4.2, 0.8, atm.mtnFar, 0.55);
        // Atmospheric fog veil between far and near ridges
        const fog1 = g.createLinearGradient(0, H * 0.62, 0, H * 0.92);
        fog1.addColorStop(0, rgba(atm.haze, 0));
        fog1.addColorStop(0.6, rgba(atm.haze, (atm.lightBg ? 0.34 : 0.22) * bd));
        fog1.addColorStop(1, rgba(atm.haze, 0.04));
        g.fillStyle = fog1;
        g.fillRect(0, H * 0.62, W, H * 0.32);

        drawRidge(H * 0.84, H * 0.042, 6.5, 2.3, atm.mtnNear, 0.68);

        // Very soft distant evergreen edge along lower horizon
        g.fillStyle = rgba(atm.mtnNear, 0.48 * bd);
        const treeStep = Math.max(14, W / 78);
        for (let x = -10; x < W + 20; x += treeStep) {
          const th = (14 + r() * 28) * clamp(H / 900, 0.6, 1.25);
          const tw = th * (0.28 + r() * 0.12);
          const ty = H * 0.875 + (r() - 0.5) * H * 0.03;
          g.beginPath();
          g.moveTo(x, ty - th);
          g.lineTo(x - tw, ty + 18);
          g.lineTo(x + tw, ty + 18);
          g.closePath();
          g.fill();
        }

        // Low ground mist
        const fog2 = g.createLinearGradient(0, H * 0.78, 0, H);
        fog2.addColorStop(0, rgba(atm.haze, 0));
        fog2.addColorStop(1, rgba(atm.haze, (atm.lightBg ? 0.42 : 0.26) * bd));
        g.fillStyle = fog2;
        g.fillRect(0, H * 0.78, W, H * 0.22);
      }

      // Streetlamp warm cone & bokeh halo (Atmosphere 3)
      if (atm.lamp) {
        const lx = W * 0.80;
        const ly = H * 0.12;
        const cone = g.createRadialGradient(lx, ly, 6, lx - W * 0.06, ly + H * 0.32, Math.max(W, H) * 0.68);
        cone.addColorStop(0, 'rgba(255, 224, 168, 0.44)');
        cone.addColorStop(0.16, 'rgba(255, 196, 118, 0.24)');
        cone.addColorStop(0.45, 'rgba(242, 164, 88, 0.08)');
        cone.addColorStop(1, 'rgba(242, 164, 88, 0)');
        g.fillStyle = cone;
        g.fillRect(0, 0, W, H);

        // Soft distant streetlamp orb
        const orb = g.createRadialGradient(lx, ly, 0, lx, ly, 26);
        orb.addColorStop(0, 'rgba(255, 248, 226, 0.92)');
        orb.addColorStop(0.35, 'rgba(255, 212, 140, 0.45)');
        orb.addColorStop(1, 'rgba(255, 190, 110, 0)');
        g.fillStyle = orb;
        g.beginPath();
        g.arc(lx, ly, 26, 0, TAU);
        g.fill();
      }

      // Subtle photographic vignette
      const vig = g.createRadialGradient(W * 0.5, H * 0.48, Math.min(W, H) * 0.35, W * 0.5, H * 0.5, Math.max(W, H) * 0.78);
      vig.addColorStop(0, 'rgba(0,0,0,0)');
      vig.addColorStop(1, atm.lightBg ? 'rgba(30,48,68,0.16)' : 'rgba(2,6,14,0.34)');
      g.fillStyle = vig;
      g.fillRect(0, 0, W, H);

      this.bgDirty = false;
    }

    // =========================================================================
    // MAIN FRAME RENDER
    // =========================================================================
    draw(now) {
      if (this.bgDirty) this._buildBackdrop();

      const ctx = this.ctx;
      const W = this.W, H = this.H, S = this.scale;
      const atm = ATMOS[this.P.atmosphere] || ATMOS[0];

      // Blit cached backdrop
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.globalAlpha = 1;
      ctx.globalCompositeOperation = 'source-over';
      ctx.drawImage(this.bgCanvas, 0, 0);
      ctx.setTransform(S, 0, 0, S, 0, 0);

      if (this.P.mode === 0) {
        this._drawSnowScene(ctx, atm, now);
      } else if (this.P.mode === 1) {
        this._drawRainScene(ctx, atm);
      } else {
        this._drawFoliageScene(ctx, atm);
      }

      // Shared powder & spindrift layer
      if (this.powders.length > 0) {
        const sp = this.sprites.snowSoft[0];
        for (let i = 0; i < this.powders.length; i++) {
          const pt = this.powders[i];
          const k = clamp(pt.life / pt.maxLife, 0, 1);
          ctx.globalAlpha = pt.alpha * k;
          const d = pt.r * 2.8;
          ctx.drawImage(sp, pt.x - d / 2, pt.y - d / 2, d, d);
        }
        ctx.globalAlpha = 1;
      }
    }

    // --- SNOW RENDER ---
    _drawSnowScene(ctx, atm, now) {
      const W = this.W, H = this.H;
      const lampX = W * 0.80, lampY = H * 0.12;

      // 1. Far & Mid snow flakes behind icon snow caps
      for (let i = 0; i < this.flakes.length; i++) {
        const f = this.flakes[i];
        if (f.z >= 0.72) continue;
        this._drawFlakeSprite(ctx, f, atm, lampX, lampY);
      }

      // 2. Icon snow caps (rounded, shaded, with soft underside contact shadow)
      if (this.P.accumulate >= 2 && this.P.adaptive) {
        for (let si = 0; si < this.surfaces.length; si++) {
          this._drawIconSnowCap(ctx, this.surfaces[si], atm);
        }
      }

      // 2b. Open window top-edge snow drifts (drawn back-to-front so front windows stay clean)
      if (this.P.accumulate >= 1 && this.P.windows) {
        for (let wi = this.winSurfaces.length - 1; wi >= 0; wi--) {
          this._drawWindowSnowCap(ctx, this.winSurfaces[wi], atm, now);
        }
      }

      // 3. Falling snow lumps shed from icons
      if (this.lumps.length > 0) {
        for (let i = 0; i < this.lumps.length; i++) {
          const l = this.lumps[i];
          const sp = this.sprites.snowSoft[i & 3];
          ctx.globalAlpha = 0.92;
          const d = l.r * 2.5;
          ctx.drawImage(sp, l.x - d / 2, l.y - d / 2, d, d);
        }
        ctx.globalAlpha = 1;
      }

      // 4. Ground snow drift heightfield with subsurface shading & sparkles
      if (this.P.accumulate >= 1) {
        this._drawGroundSnow(ctx, atm, now);
      }

      // 5. Near-foreground snowflakes & bokeh
      for (let i = 0; i < this.flakes.length; i++) {
        const f = this.flakes[i];
        if (f.z < 0.72) continue;
        this._drawFlakeSprite(ctx, f, atm, lampX, lampY);
      }
      ctx.globalAlpha = 1;
    }

    _drawFlakeSprite(ctx, f, atm, lampX, lampY) {
      let sp;
      if (f.kind === 2) sp = this.sprites.snowBokeh[f.species & 1];
      else if (f.kind === 1) sp = this.sprites.snowCrystal[f.colorIdx % this.sprites.snowCrystal.length];
      else sp = this.sprites.snowSoft[f.species & 3];

      let alpha = f.alpha;
      // Warm streetlamp cone boosts flake brilliance when passing through the light
      if (atm.lamp) {
        const d = Math.hypot(f.x - lampX, f.y - lampY);
        const boost = smoothstep(this.W * 0.45, 30, d);
        alpha = Math.min(1, alpha + boost * 0.32);
      }
      ctx.globalAlpha = alpha;
      const d = f.size;
      if (f.kind === 1) {
        ctx.save();
        ctx.translate(f.x, f.y);
        ctx.rotate(f.rot);
        ctx.drawImage(sp, -d / 2, -d / 2, d, d);
        ctx.restore();
      } else {
        ctx.drawImage(sp, f.x - d / 2, f.y - d / 2, d, d);
      }
    }

    _drawGroundSnow(ctx, atm, now) {
      const W = this.W, H = this.H;
      const arr = this.groundH;
      const n = this.groundN;
      let maxH = 0;
      for (let i = 0; i < n; i++) if (arr[i] > maxH) maxH = arr[i];
      if (maxH < 0.5) return;

      const buildPath = (yOffset) => {
        ctx.beginPath();
        ctx.moveTo(0, H + 4);
        ctx.lineTo(0, H - arr[0] + yOffset);
        for (let i = 1; i < n - 1; i++) {
          const x0 = i * this.cellW;
          const y0 = H - arr[i] + yOffset;
          const xm = x0 + this.cellW * 0.5;
          const ym = H - (arr[i] + arr[i + 1]) * 0.5 + yOffset;
          ctx.quadraticCurveTo(x0, y0, xm, ym);
        }
        ctx.lineTo(W, H - arr[n - 1] + yOffset);
        ctx.lineTo(W, H + 4);
        ctx.closePath();
      };

      // Subsurface shaded snow body
      const grad = ctx.createLinearGradient(0, H - maxH - 4, 0, H);
      grad.addColorStop(0, rgba(atm.snowTop, 0.98));
      grad.addColorStop(0.38, rgba(atm.snowMid, 0.98));
      grad.addColorStop(1, rgba(atm.snowDeep, 0.99));
      buildPath(0);
      ctx.fillStyle = grad;
      ctx.fill();

      // Subtle cool rim-shadow depth ridge just below the sunlit crest
      ctx.save();
      ctx.clip();
      buildPath(Math.min(10, maxH * 0.24));
      ctx.fillStyle = rgba(atm.snowDeep, 0.16);
      ctx.fill();
      ctx.restore();

      // Delicate sunlit crest line
      ctx.beginPath();
      ctx.moveTo(0, H - arr[0]);
      for (let i = 1; i < n - 1; i++) {
        const x0 = i * this.cellW;
        const y0 = H - arr[i];
        const xm = x0 + this.cellW * 0.5;
        const ym = H - (arr[i] + arr[i + 1]) * 0.5;
        ctx.quadraticCurveTo(x0, y0, xm, ym);
      }
      ctx.lineTo(W, H - arr[n - 1]);
      ctx.strokeStyle = atm.lightBg ? rgba(atm.snowOutline, 0.55) : 'rgba(255,255,255,0.52)';
      ctx.lineWidth = 1.25;
      ctx.stroke();

      // Microscopic ice crystal sparkles on snow drift surface
      if (this.P.sparkle > 0.02 && !this.lowMode) {
        const t = now * 0.001;
        const spAmp = this.P.sparkle;
        for (let i = 0; i < this.groundSparkles.length; i++) {
          const sp = this.groundSparkles[i];
          const gi = clamp(Math.floor(sp.u * (n - 1)), 0, n - 1);
          const h = arr[gi];
          if (h < 4) continue;
          const tw = Math.pow(Math.max(0, Math.sin(t * sp.freq + sp.phase)), 12);
          if (tw < 0.08) continue;
          const sx = gi * this.cellW;
          const sy = H - h * (1 - sp.depthFrac);
          const r = sp.size * (0.7 + 0.6 * tw);
          ctx.fillStyle = atm.lamp
            ? 'rgba(255,244,210,' + (tw * spAmp * 0.92).toFixed(3) + ')'
            : 'rgba(255,255,255,' + (tw * spAmp * 0.92).toFixed(3) + ')';
          ctx.beginPath();
          ctx.arc(sx, sy, r, 0, TAU);
          ctx.fill();
        }
      }
    }

    _drawIconSnowCap(ctx, s, atm) {
      const arr = s.capH;
      const n = s.capN;
      let maxH = 0;
      for (let i = 0; i < n; i++) if (arr[i] > maxH) maxH = arr[i];
      if (maxH < 0.6) return;

      const step = s.w / Math.max(1, n - 1);
      const baseY = s.y + 2.5;

      // Soft contact shadow cast onto top of icon
      ctx.fillStyle = 'rgba(10, 18, 32,' + (atm.shadowAlpha * 0.45).toFixed(3) + ')';
      ctx.beginPath();
      ctx.ellipse(s.x + s.w * 0.5, baseY + 1.5, s.w * 0.44, Math.min(4.2, 1.5 + maxH * 0.18), 0, 0, TAU);
      ctx.fill();

      // Organic snow pillow curving smoothly around rounded icon corners
      ctx.beginPath();
      ctx.moveTo(s.x + 1, baseY);
      for (let i = 0; i < n - 1; i++) {
        const x0 = s.x + i * step;
        const y0 = baseY - arr[i];
        const xm = x0 + step * 0.5;
        const ym = baseY - (arr[i] + arr[i + 1]) * 0.5;
        ctx.quadraticCurveTo(x0, y0, xm, ym);
      }
      ctx.lineTo(s.x + s.w - 1, baseY - arr[n - 1]);
      // Slight rounded lower lip hanging naturally on the icon's top bevel
      ctx.quadraticCurveTo(s.x + s.w * 0.5, baseY + Math.min(3.2, maxH * 0.22), s.x + 1, baseY);
      ctx.closePath();

      const cg = ctx.createLinearGradient(0, baseY - maxH - 1, 0, baseY + 3);
      cg.addColorStop(0, rgba(atm.snowTop, 0.98));
      cg.addColorStop(0.55, rgba(atm.snowMid, 0.96));
      cg.addColorStop(1, rgba(atm.snowDeep, 0.95));
      ctx.fillStyle = cg;
      ctx.fill();

      ctx.strokeStyle = atm.lightBg ? rgba(atm.snowOutline, 0.5) : 'rgba(255,255,255,0.48)';
      ctx.lineWidth = 0.9;
      ctx.stroke();
    }

    _drawWindowSnowCap(ctx, w, atm, now) {
      const arr = w.capH;
      const n = w.capN;
      let maxH = 0;
      for (let i = 0; i < n; i++) if (arr[i] > maxH) maxH = arr[i];
      if (maxH < 0.7) return;

      const step = w.w / Math.max(1, n - 1);
      // Anchor slightly behind the window's top edge so there is never a gap
      const baseY = w.y + 2.5;
      const visAlpha = smoothstep(0.65, 2.2, maxH);

      ctx.save();
      ctx.globalAlpha = visAlpha;

      ctx.beginPath();
      ctx.moveTo(w.x + 4, baseY + 1);
      ctx.lineTo(w.x + 4, baseY - arr[0]);
      for (let i = 1; i < n - 1; i++) {
        const x0 = w.x + i * step;
        const y0 = baseY - arr[i];
        const xm = x0 + step * 0.5;
        const ym = baseY - (arr[i] + arr[i + 1]) * 0.5;
        ctx.quadraticCurveTo(x0, y0, xm, ym);
      }
      ctx.lineTo(w.x + w.w - 4, baseY - arr[n - 1]);
      ctx.lineTo(w.x + w.w - 4, baseY + 1);
      ctx.closePath();

      const cg = ctx.createLinearGradient(0, baseY - maxH - 2, 0, baseY + 2);
      cg.addColorStop(0, rgba(atm.snowTop, 0.99));
      cg.addColorStop(0.48, rgba(atm.snowMid, 0.98));
      cg.addColorStop(1, rgba(atm.snowDeep, 0.98));
      ctx.fillStyle = cg;
      ctx.fill();

      // Sunlit / moonlight top crest rim (only when snow has real body, never a floating thin line)
      if (maxH > 1.8) {
        ctx.beginPath();
        ctx.moveTo(w.x + 4, baseY - arr[0]);
        for (let i = 1; i < n - 1; i++) {
          const x0 = w.x + i * step;
          const y0 = baseY - arr[i];
          const xm = x0 + step * 0.5;
          const ym = baseY - (arr[i] + arr[i + 1]) * 0.5;
          ctx.quadraticCurveTo(x0, y0, xm, ym);
        }
        ctx.lineTo(w.x + w.w - 4, baseY - arr[n - 1]);
        ctx.strokeStyle = atm.lightBg ? rgba(atm.snowOutline, 0.55) : 'rgba(255,255,255,0.56)';
        ctx.lineWidth = 1.1;
        ctx.stroke();
      }
      ctx.restore();

      // Subtle micro-sparkles along the window snow ridge
      if (this.P.sparkle > 0.05 && !this.lowMode && maxH > 3) {
        const t = now * 0.001;
        const spCount = Math.min(10, Math.max(3, Math.floor(w.w / 110)));
        for (let k = 0; k < spCount; k++) {
          const u = (k + 0.5) / spCount;
          const ci = clamp(Math.floor(u * (n - 1)), 0, n - 1);
          const h = arr[ci];
          if (h < 3) continue;
          const tw = Math.pow(Math.max(0, Math.sin(t * (1.8 + (k % 3) * 0.7) + k * 2.1 + (w.hwnd % 11))), 12);
          if (tw < 0.1) continue;
          ctx.fillStyle = atm.lamp
            ? 'rgba(255,244,210,' + (tw * this.P.sparkle * 0.9).toFixed(3) + ')'
            : 'rgba(255,255,255,' + (tw * this.P.sparkle * 0.9).toFixed(3) + ')';
          ctx.beginPath();
          ctx.arc(w.x + ci * step, baseY - h * 0.65, 1.0 + tw * 0.9, 0, TAU);
          ctx.fill();
        }
      }
    }

    // --- RAIN RENDER ---
    _drawRainScene(ctx, atm) {
      const windPx = (this.P.wind * 72) + (this.gustVal * 115);
      const slopeX = windPx * 1.35;

      // 1. Subtle wet sheen along icon & open window top edges
      if (this.P.accumulate >= 2 && this.P.adaptive) {
        for (const s of this.surfaces) {
          if (s.wet < 0.04) continue;
          const wg = ctx.createLinearGradient(s.x, s.y, s.x, s.y + 8);
          wg.addColorStop(0, rgba(atm.rainTint, 0.34 * s.wet));
          wg.addColorStop(1, rgba(atm.rainTint, 0));
          ctx.fillStyle = wg;
          ctx.fillRect(s.x + 4, s.y, s.w - 8, 7);
        }
      }
      if (this.P.accumulate >= 1 && this.P.windows) {
        for (const w of this.winSurfaces) {
          if (w.wet < 0.04) continue;
          const wg = ctx.createLinearGradient(w.x, w.y - 4, w.x, w.y + 2);
          wg.addColorStop(0, rgba(atm.rainTint, 0));
          wg.addColorStop(1, rgba(atm.rainTint, 0.42 * w.wet));
          ctx.fillStyle = wg;
          ctx.fillRect(w.x + 8, w.y - 4, Math.max(0, w.w - 16), 5);
        }
      }

      // 2. Falling rain streaks
      ctx.lineCap = 'round';
      for (let i = 0; i < this.rainStreaks.length; i++) {
        const r = this.rainStreaks[i];
        const dx = (slopeX * (0.45 + 0.55 * r.z) / r.speed) * r.len;
        ctx.strokeStyle = rgba(atm.rainTint, r.alpha * (0.65 + 0.35 * this.P.sparkle));
        ctx.lineWidth = r.thick;
        ctx.beginPath();
        ctx.moveTo(r.x - dx, r.y - r.len);
        ctx.lineTo(r.x, r.y);
        ctx.stroke();
      }

      // 3. Wet beaded trails left by running drops
      for (let i = 0; i < this.rainTrails.length; i++) {
        const tr = this.rainTrails[i];
        ctx.fillStyle = rgba(atm.rainTint, tr.a * 0.52);
        ctx.beginPath();
        ctx.arc(tr.x, tr.y, tr.r, 0, TAU);
        ctx.fill();
      }

      // 4. Glass & icon water beads (refractive highlight + dark lower rim)
      for (let i = 0; i < this.rainBeads.length; i++) {
        const b = this.rainBeads[i];
        const stretch = b.hanging ? 1.38 : (b.vy > 20 ? clamp(1 + b.vy / 260, 1.05, 1.65) : 1.08);
        const rx = b.r;
        const ry = b.r * stretch;

        // Soft drop shadow
        ctx.fillStyle = 'rgba(4, 10, 20, 0.24)';
        ctx.beginPath();
        ctx.ellipse(b.x + 0.6, b.y + 1.1, rx * 1.02, ry * 0.96, 0, 0, TAU);
        ctx.fill();

        // Translucent bead body
        ctx.fillStyle = rgba(atm.rainTint, 0.22 + 0.14 * this.P.sparkle);
        ctx.beginPath();
        ctx.ellipse(b.x, b.y, rx, ry, 0, 0, TAU);
        ctx.fill();

        // Lower caustic bright crescent + crisp top specular dot
        ctx.strokeStyle = 'rgba(255,255,255,' + (0.42 + 0.36 * this.P.sparkle).toFixed(3) + ')';
        ctx.lineWidth = Math.max(0.6, rx * 0.26);
        ctx.beginPath();
        ctx.arc(b.x, b.y + ry * 0.16, rx * 0.72, 0.25, Math.PI - 0.25);
        ctx.stroke();

        ctx.fillStyle = 'rgba(255,255,255,' + (0.65 + 0.3 * this.P.sparkle).toFixed(3) + ')';
        ctx.beginPath();
        ctx.arc(b.x - rx * 0.28, b.y - ry * 0.28, Math.max(0.55, rx * 0.28), 0, TAU);
        ctx.fill();
      }

      // 5. Splash droplets
      for (let i = 0; i < this.splashes.length; i++) {
        const sp = this.splashes[i];
        const k = clamp(sp.life / sp.maxLife, 0, 1);
        ctx.fillStyle = rgba(atm.rainTint, 0.68 * k);
        ctx.beginPath();
        ctx.arc(sp.x, sp.y, sp.r, 0, TAU);
        ctx.fill();
      }
    }

    // --- AUTUMN LEAVES & CHERRY BLOSSOM RENDER ---
    _drawFoliageScene(ctx, atm) {
      const isLeaf = this.P.mode === 2;

      // 1. Far falling leaves / petals
      for (let i = 0; i < this.flakes.length; i++) {
        const f = this.flakes[i];
        if (f.z >= 0.7) continue;
        this._drawFoliageItem(ctx, f, isLeaf, atm);
      }

      // 2. Settled individual leaf / petal heaps (ground + icon caps)
      for (let i = 0; i < this.heaps.length; i++) {
        const h = this.heaps[i];
        const sp = isLeaf
          ? this.sprites.leaves[h.species % 3][h.colorIdx % AUTUMN_COLORS.length]
          : this.sprites.petals[h.colorIdx % BLOSSOM_COLORS.length];
        ctx.save();
        ctx.translate(h.x, h.y);
        ctx.rotate(h.rot);
        ctx.scale(h.scale, h.scale * h.squash);
        // Subtle browning/fading when wilting
        ctx.globalAlpha = h.alpha;
        ctx.drawImage(sp, -sp.width / 2, -sp.height / 2);
        if (h.age > 8) {
          ctx.globalAlpha = clamp((h.age - 8) / 14, 0, 0.42) * h.alpha;
          ctx.fillStyle = isLeaf ? 'rgba(62,34,16,0.8)' : 'rgba(138,92,80,0.85)';
          ctx.beginPath();
          ctx.ellipse(0, 0, sp.width * 0.28, sp.height * 0.28, 0, 0, TAU);
          ctx.fill();
        }
        ctx.restore();
      }

      // 3. Foreground tumbling leaves / petals
      for (let i = 0; i < this.flakes.length; i++) {
        const f = this.flakes[i];
        if (f.z < 0.7) continue;
        this._drawFoliageItem(ctx, f, isLeaf, atm);
      }
      ctx.globalAlpha = 1;
    }

    _drawFoliageItem(ctx, f, isLeaf, atm) {
      const sp = isLeaf
        ? this.sprites.leaves[f.species % 3][f.colorIdx % AUTUMN_COLORS.length]
        : (f.wholeFlower ? this.sprites.flower : this.sprites.petals[f.colorIdx % BLOSSOM_COLORS.length]);
      const tilt = 0.26 + 0.74 * Math.abs(Math.cos(f.pitch));
      ctx.save();
      ctx.translate(f.x, f.y);
      ctx.rotate(f.rot);
      ctx.scale(1, f.wholeFlower ? (0.65 + 0.35 * tilt) : tilt);
      ctx.globalAlpha = f.alpha;
      const d = f.size;
      ctx.drawImage(sp, -d / 2, -d / 2, d, d);

      // Translucent sunlight/lamp glint when petal turns flat toward light
      if (!isLeaf && !f.wholeFlower && this.P.sparkle > 0.1 && tilt > 0.86) {
        ctx.globalAlpha = (tilt - 0.86) * 4.2 * this.P.sparkle * (atm.lamp ? 0.95 : 0.65);
        ctx.fillStyle = atm.lamp ? '#fff3d6' : '#ffffff';
        ctx.beginPath();
        ctx.ellipse(0, 0, d * 0.22, d * 0.32, 0, 0, TAU);
        ctx.fill();
      }
      ctx.restore();
    }
  }

  global.QuietSnowEngine = QuietSnowEngine;
})(window);
