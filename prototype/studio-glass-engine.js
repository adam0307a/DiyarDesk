/*
 * DiyarDesk · Studio Glass 3D Engine (Three.js + WebGL2 Multi-Pass Glass Refraction)
 *
 * Original GPL-3.0 implementation inspired by the Studio Glass concept
 * (https://threeui.com/studio-glass).
 *
 * Features:
 *  - Real 3D Three.js designer studio workspace with PBR wood, plaster, metal,
 *    foliage, ceramic, and frosted glass materials + soft shadow mapping.
 *  - Five atmospheric lighting presets (Morning, Thunderstorm, Afternoon,
 *    Sunset, Night + Auto time-of-day) with smooth transitions, volumetric
 *    window light shafts, animated rain-on-glass shader & lightning flashes.
 *  - Direct 3D Raycaster interaction: drag venetian blinds, swivel the studio
 *    display, move the mechanical keyboard, mouse, pen cup, notebook, office
 *    chair, windowsill plants (with spring leaf sway), and toggle/adjust the
 *    mushroom desk lamp.
 *  - Multi-pass WebGL2 refractive frosted-glass shader pass for desktop icon
 *    plaques and optional interactive Studio Lighting glass control cards.
 */
(function (global) {
  'use strict';

  const MAX_GLASS = 56; // up to 48 desktop icons + 7 optional HUD glass cards + 1 cursor lens

  // 0: Morning, 1: Thunderstorm, 2: Afternoon, 3: Sunset, 4: Night
  const PRESETS = [
    {
      id: 'morning',
      label: 'Morning',
      sub: 'Soft sun through the window',
      pct: 58,
      sunColor: 0xffe4be, sunInt: 3.4, sunPos: [-7.5, 5.2, -1.6],
      hemiSky: 0xaed4cc, hemiGround: 0x524234, hemiInt: 1.15,
      ambColor: 0x5d7269, ambInt: 0.56,
      lampInt: 1.55, lampGlow: 0.92,
      screenInt: 1.05,
      skyTop: [76, 118, 118], skyMid: [130, 166, 152], skyBot: [224, 218, 190],
      fogCol: [78, 108, 98],
      shaftCol: [1.0, 0.90, 0.72], shaftAlpha: 0.22,
      rain: 0.0, thunder: 0.0, dust: 0.72, stars: 0.0,
      rimTint: [1.0, 0.93, 0.82],
      exposure: 1.08,
      wallTint: 0x1d342e, nicheTint: 0x4b421f
    },
    {
      id: 'thunderstorm',
      label: 'Thunderstorm',
      sub: 'Rain on glass',
      pct: 36,
      sunColor: 0xa8cde4, sunInt: 1.85, sunPos: [-7.0, 5.6, -1.2],
      hemiSky: 0x7a9eb6, hemiGround: 0x363d45, hemiInt: 0.96,
      ambColor: 0x425866, ambInt: 0.50,
      lampInt: 2.65, lampGlow: 1.18,
      screenInt: 1.30,
      skyTop: [30, 46, 56], skyMid: [50, 74, 86], skyBot: [86, 112, 122],
      fogCol: [42, 62, 68],
      shaftCol: [0.68, 0.84, 0.96], shaftAlpha: 0.14,
      rain: 1.0, thunder: 1.0, dust: 0.10, stars: 0.0,
      rimTint: [0.74, 0.89, 0.98],
      exposure: 1.02,
      wallTint: 0x17292a, nicheTint: 0x3a321b
    },
    {
      id: 'afternoon',
      label: 'Afternoon',
      sub: 'Crisp studio daylight',
      pct: 100,
      sunColor: 0xfff8ec, sunInt: 4.2, sunPos: [-7.0, 6.8, -0.6],
      hemiSky: 0xc8e2f2, hemiGround: 0x4a3e33, hemiInt: 1.32,
      ambColor: 0x5c6a70, ambInt: 0.62,
      lampInt: 0.32, lampGlow: 0.28,
      screenInt: 0.75,
      skyTop: [106, 162, 214], skyMid: [168, 206, 232], skyBot: [236, 244, 248],
      fogCol: [128, 160, 166],
      shaftCol: [1.0, 0.98, 0.93], shaftAlpha: 0.26,
      rain: 0.0, thunder: 0.0, dust: 0.55, stars: 0.0,
      rimTint: [0.95, 0.98, 1.0],
      exposure: 1.18,
      wallTint: 0x223b35, nicheTint: 0x5c4f24
    },
    {
      id: 'sunset',
      label: 'Sunset',
      sub: 'Low amber light',
      pct: 64,
      sunColor: 0xff9f4e, sunInt: 3.7, sunPos: [-8.2, 3.3, -2.0],
      hemiSky: 0xc69484, hemiGround: 0x4d342b, hemiInt: 1.08,
      ambColor: 0x644336, ambInt: 0.52,
      lampInt: 2.25, lampGlow: 1.08,
      screenInt: 1.15,
      skyTop: [82, 58, 96], skyMid: [196, 108, 76], skyBot: [254, 192, 118],
      fogCol: [134, 84, 66],
      shaftCol: [1.0, 0.68, 0.36], shaftAlpha: 0.30,
      rain: 0.0, thunder: 0.0, dust: 0.90, stars: 0.08,
      rimTint: [1.0, 0.78, 0.54],
      exposure: 1.08,
      wallTint: 0x1e2b27, nicheTint: 0x54381d
    },
    {
      id: 'night',
      label: 'Night',
      sub: 'Moonlight & warm lamp',
      pct: 22,
      sunColor: 0x86b8ff, sunInt: 1.65, sunPos: [-7.6, 5.8, -1.8],
      hemiSky: 0x5272aa, hemiGround: 0x2c3040, hemiInt: 0.82,
      ambColor: 0x334466, ambInt: 0.44,
      lampInt: 3.4, lampGlow: 1.38,
      screenInt: 1.45,
      skyTop: [14, 24, 48], skyMid: [24, 44, 82], skyBot: [52, 82, 126],
      fogCol: [22, 36, 56],
      shaftCol: [0.56, 0.76, 1.0], shaftAlpha: 0.18,
      rain: 0.0, thunder: 0.0, dust: 0.32, stars: 1.0,
      rimTint: [0.68, 0.84, 1.0],
      exposure: 1.00,
      wallTint: 0x122122, nicheTint: 0x352c18
    }
  ];

  function clamp(v, lo, hi) { return v < lo ? lo : (v > hi ? hi : v); }
  function lerp(a, b, t) { return a + (b - a) * t; }
  function autoPresetIndex() {
    const h = new Date().getHours();
    if (h >= 6 && h < 11) return 0;   // Morning
    if (h >= 11 && h < 17) return 2;  // Afternoon
    if (h >= 17 && h < 20) return 3;  // Sunset
    return 4;                         // Night
  }

  // Deterministic PRNG for procedural textures & foliage
  function makeRng(seed) {
    let s = seed >>> 0;
    return function () {
      s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
      return s / 4294967296;
    };
  }

  // =========================================================================
  // Procedural PBR Textures (Zero external images, 100% offline)
  // =========================================================================
  function createWalnutTexture(THREE) {
    const c = document.createElement('canvas');
    c.width = 512; c.height = 512;
    const ctx = c.getContext('2d');
    const rng = makeRng(4207);

    const g = ctx.createLinearGradient(0, 0, 512, 0);
    g.addColorStop(0, '#5a351e');
    g.addColorStop(0.35, '#724326');
    g.addColorStop(0.68, '#532f1a');
    g.addColorStop(1, '#683c21');
    ctx.fillStyle = g;
    ctx.fillRect(0, 0, 512, 512);

    // Rich walnut grain bands
    for (let y = 0; y < 512; y += 2) {
      const wave = Math.sin(y * 0.045) * 14 + Math.sin(y * 0.13) * 5;
      const lum = (rng() - 0.5) * 26 + Math.sin((y + wave) * 0.18) * 18;
      ctx.fillStyle = lum > 0
        ? `rgba(176,112,64,${(lum / 210).toFixed(3)})`
        : `rgba(24,11,5,${(-lum / 145).toFixed(3)})`;
      ctx.fillRect(0, y, 512, 2);
    }
    // Fine horizontal wood pores
    for (let i = 0; i < 650; i++) {
      const x = rng() * 512, y = rng() * 512;
      const len = 18 + rng() * 95;
      ctx.fillStyle = rng() > 0.35 ? 'rgba(26,12,6,0.22)' : 'rgba(186,122,74,0.12)';
      ctx.fillRect(x, y, len, 1.2);
    }
    const tex = new THREE.CanvasTexture(c);
    tex.colorSpace = THREE.SRGBColorSpace;
    tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
    tex.repeat.set(2.2, 1.2);
    return tex;
  }

  function createPlasterTexture(THREE, baseHex) {
    const c = document.createElement('canvas');
    c.width = 512; c.height = 512;
    const ctx = c.getContext('2d');
    const rng = makeRng(8813);

    ctx.fillStyle = baseHex;
    ctx.fillRect(0, 0, 512, 512);

    // Venetian plaster cloudy trowel variation
    for (let i = 0; i < 160; i++) {
      const x = rng() * 512, y = rng() * 512;
      const r = 24 + rng() * 96;
      const grad = ctx.createRadialGradient(x, y, 2, x, y, r);
      const bright = rng() > 0.5;
      grad.addColorStop(0, bright ? 'rgba(255,255,255,0.045)' : 'rgba(0,0,0,0.065)');
      grad.addColorStop(1, 'rgba(0,0,0,0)');
      ctx.fillStyle = grad;
      ctx.beginPath();
      ctx.arc(x, y, r, 0, Math.PI * 2);
      ctx.fill();
    }
    // Fine mineral speckle
    for (let i = 0; i < 1800; i++) {
      ctx.fillStyle = rng() > 0.5 ? 'rgba(255,255,255,0.025)' : 'rgba(0,0,0,0.04)';
      ctx.fillRect(rng() * 512, rng() * 512, 1.5, 1.5);
    }
    const tex = new THREE.CanvasTexture(c);
    tex.colorSpace = THREE.SRGBColorSpace;
    tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
    tex.repeat.set(2, 2);
    return tex;
  }

  function createMonitorWallpaperTexture(THREE) {
    const c = document.createElement('canvas');
    c.width = 1024; c.height = 640;
    const ctx = c.getContext('2d');
    const rng = makeRng(9931);

    // Alpine mountain lake scene inspired by classic studio monitor photography
    const sky = ctx.createLinearGradient(0, 0, 0, 390);
    sky.addColorStop(0, '#6a8298');
    sky.addColorStop(0.42, '#9eb3bf');
    sky.addColorStop(0.75, '#d6cfc6');
    sky.addColorStop(1, '#e6d6c3');
    ctx.fillStyle = sky;
    ctx.fillRect(0, 0, 1024, 640);

    // Soft clouds behind peak
    for (let i = 0; i < 18; i++) {
      const cx = rng() * 1024, cy = 80 + rng() * 210, rw = 90 + rng() * 180, rh = 24 + rng() * 48;
      const cg = ctx.createRadialGradient(cx, cy, 4, cx, cy, rw);
      cg.addColorStop(0, 'rgba(255,248,238,0.34)');
      cg.addColorStop(1, 'rgba(255,248,238,0)');
      ctx.fillStyle = cg;
      ctx.beginPath();
      ctx.ellipse(cx, cy, rw, rh, 0, 0, Math.PI * 2);
      ctx.fill();
    }

    // Distant blue-grey mountain range
    ctx.fillStyle = '#5d7182';
    ctx.beginPath();
    ctx.moveTo(0, 395);
    ctx.lineTo(130, 290); ctx.lineTo(260, 330); ctx.lineTo(410, 250);
    ctx.lineTo(660, 275); ctx.lineTo(820, 240); ctx.lineTo(1024, 315);
    ctx.lineTo(1024, 420); ctx.lineTo(0, 420);
    ctx.closePath();
    ctx.fill();

    // Hero sunlit Matterhorn-like central peak
    // Shadow side (left)
    ctx.fillStyle = '#4f5d6b';
    ctx.beginPath();
    ctx.moveTo(275, 420);
    ctx.lineTo(548, 92);
    ctx.lineTo(568, 420);
    ctx.closePath();
    ctx.fill();
    // Sunlit warm terracotta/alpenglow face (right)
    const peakG = ctx.createLinearGradient(548, 92, 745, 415);
    peakG.addColorStop(0, '#c98863');
    peakG.addColorStop(0.55, '#9d684f');
    peakG.addColorStop(1, '#5d4a46');
    ctx.fillStyle = peakG;
    ctx.beginPath();
    ctx.moveTo(548, 92);
    ctx.lineTo(618, 198);
    ctx.lineTo(695, 295);
    ctx.lineTo(805, 420);
    ctx.lineTo(540, 420);
    ctx.closePath();
    ctx.fill();

    // Snow streaks along mountain ridges
    ctx.fillStyle = 'rgba(246,242,236,0.45)';
    ctx.beginPath();
    ctx.moveTo(548, 92); ctx.lineTo(525, 165); ctx.lineTo(552, 152); ctx.lineTo(568, 215); ctx.lineTo(576, 148);
    ctx.closePath();
    ctx.fill();

    // Misty valley band
    const mist = ctx.createLinearGradient(0, 345, 0, 455);
    mist.addColorStop(0, 'rgba(222,226,228,0)');
    mist.addColorStop(0.5, 'rgba(222,226,228,0.52)');
    mist.addColorStop(1, 'rgba(222,226,228,0)');
    ctx.fillStyle = mist;
    ctx.fillRect(0, 345, 1024, 110);

    // Alpine lake with reflection
    const lake = ctx.createLinearGradient(0, 425, 0, 640);
    lake.addColorStop(0, '#8ba0ad');
    lake.addColorStop(0.45, '#596e7d');
    lake.addColorStop(1, '#26343e');
    ctx.fillStyle = lake;
    ctx.fillRect(0, 425, 1024, 215);

    // Peak reflection in lake
    ctx.fillStyle = 'rgba(176,126,96,0.30)';
    ctx.beginPath();
    ctx.moveTo(465, 425); ctx.lineTo(552, 595); ctx.lineTo(645, 425);
    ctx.closePath();
    ctx.fill();

    // Left & right dark pine forest banks framing the lake
    function drawForestBank(isLeft) {
      ctx.fillStyle = '#182423';
      ctx.beginPath();
      if (isLeft) {
        ctx.moveTo(0, 380); ctx.lineTo(390, 445); ctx.lineTo(445, 640); ctx.lineTo(0, 640);
      } else {
        ctx.moveTo(1024, 370); ctx.lineTo(645, 448); ctx.lineTo(595, 640); ctx.lineTo(1024, 640);
      }
      ctx.closePath();
      ctx.fill();

      // Pine tree spires along bank
      for (let i = 0; i < 42; i++) {
        const t = i / 41;
        const x = isLeft ? t * 410 : 1024 - t * 400;
        const y = lerp(388, 456, t) + (rng() - 0.5) * 18;
        const h = 28 + (1 - t) * 58 + rng() * 24;
        const w = h * 0.24;
        ctx.fillStyle = i % 3 === 0 ? '#111b1b' : '#1b2b27';
        ctx.beginPath();
        ctx.moveTo(x, y - h);
        ctx.lineTo(x - w, y + 12);
        ctx.lineTo(x + w, y + 12);
        ctx.closePath();
        ctx.fill();
      }
    }
    drawForestBank(true);
    drawForestBank(false);

    const tex = new THREE.CanvasTexture(c);
    tex.colorSpace = THREE.SRGBColorSpace;
    tex.anisotropy = 4;
    return tex;
  }

  function createOutdoorForestTexture(THREE) {
    const c = document.createElement('canvas');
    c.width = 768; c.height = 768;
    const ctx = c.getContext('2d');
    const rng = makeRng(5521);

    ctx.clearRect(0, 0, 768, 768);
    // Three misty depth layers of woodland trees outside the window
    const layers = [
      { alpha: 0.34, yBase: 510, count: 14, scale: 0.72, col: '#2b4740' },
      { alpha: 0.58, yBase: 560, count: 11, scale: 0.95, col: '#1c342e' },
      { alpha: 0.85, yBase: 630, count: 8,  scale: 1.25, col: '#11221e' }
    ];
    for (const L of layers) {
      ctx.save();
      ctx.globalAlpha = L.alpha;
      ctx.fillStyle = L.col;
      for (let i = 0; i < L.count; i++) {
        const tx = (i / Math.max(1, L.count - 1)) * 768 + (rng() - 0.5) * 75;
        const th = (240 + rng() * 190) * L.scale;
        const ty = L.yBase + (rng() - 0.5) * 40;
        // Trunk & organic leafy branches
        ctx.strokeStyle = L.col;
        ctx.lineWidth = (7 + rng() * 8) * L.scale;
        ctx.beginPath();
        ctx.moveTo(tx, 768);
        ctx.quadraticCurveTo(tx + (rng() - 0.5) * 40, ty - th * 0.4, tx + (rng() - 0.5) * 50, ty - th);
        ctx.stroke();

        const clusters = 7 + Math.floor(rng() * 6);
        for (let k = 0; k < clusters; k++) {
          const frac = 0.25 + (k / clusters) * 0.75;
          const cx = tx + (rng() - 0.5) * 115 * L.scale;
          const cy = ty - th * frac + (rng() - 0.5) * 45;
          const rx = (34 + rng() * 58) * L.scale;
          const ry = (24 + rng() * 44) * L.scale;
          ctx.beginPath();
          ctx.ellipse(cx, cy, rx, ry, rng() * Math.PI, 0, Math.PI * 2);
          ctx.fill();
        }
      }
      ctx.restore();
    }

    const tex = new THREE.CanvasTexture(c);
    tex.colorSpace = THREE.SRGBColorSpace;
    return tex;
  }

  // =========================================================================
  // Custom GLSL Shaders (Outdoor Rain Glass, Volumetric Shafts, Glass Overlay)
  // =========================================================================
  const WINDOW_GLASS_VERT = `
    varying vec2 vUv;
    void main() {
      vUv = uv;
      gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
    }
  `;

  const WINDOW_GLASS_FRAG = `
    precision highp float;
    varying vec2 vUv;
    uniform float uTime;
    uniform float uRain;
    uniform float uWeather;
    uniform vec3 uSkyTop;
    uniform vec3 uSkyMid;
    uniform vec3 uSkyBot;
    uniform float uStars;
    uniform float uFlash;
    uniform sampler2D uForest;

    float hash21(vec2 p) {
      p = fract(p * vec2(123.34, 456.21));
      p += dot(p, p + 45.32);
      return fract(p.x * p.y);
    }

    // Procedural sliding rain beads on window pane
    float rainLayer(vec2 uv, float t, float scale) {
      vec2 a = vec2(scale * 1.15, scale * 0.42);
      vec2 st = uv * a;
      st.y += t * 0.22;
      vec2 id = floor(st);
      vec2 gv = fract(st) - 0.5;
      float n = hash21(id);
      float y = -sin(t * 1.4 + n * 6.2831 + sin(t * 1.4 + n * 6.2831) * 0.45) * 0.38;
      float x = (n - 0.5) * 0.72 + sin(st.y * 2.5 + n * 12.0) * 0.06;
      vec2 p = (gv - vec2(x, y)) / vec2(1.0, 2.3);
      float drop = smoothstep(0.085, 0.02, length(p));
      // Wet trail above drop
      float trailY = clamp((gv.y - y) * 1.8, 0.0, 1.0);
      float trail = smoothstep(0.045, 0.0, abs(gv.x - x)) * smoothstep(0.0, 0.08, gv.y - y) * (1.0 - trailY) * 0.45;
      return clamp(drop + trail, 0.0, 1.0);
    }

    void main() {
      vec3 sky = mix(uSkyBot, uSkyMid, smoothstep(0.0, 0.52, vUv.y));
      sky = mix(sky, uSkyTop, smoothstep(0.45, 1.0, vUv.y));

      // Night stars
      if (uStars > 0.02) {
        vec2 sid = floor(vUv * vec2(44.0, 34.0));
        vec2 sgv = fract(vUv * vec2(44.0, 34.0)) - 0.5;
        float h = hash21(sid);
        if (h > 0.965 && vUv.y > 0.34) {
          float tw = 0.55 + 0.45 * sin(uTime * 2.2 + h * 40.0);
          sky += vec3(0.85, 0.92, 1.0) * smoothstep(0.18, 0.02, length(sgv)) * uStars * tw;
        }
      }

      // Soft lightning flash illumination on storm clouds
      sky += vec3(0.65, 0.80, 1.0) * uFlash * 0.55;

      // Rain distortion & glass droplets
      float rainAmt = uRain * uWeather;
      float drops = 0.0;
      vec2 fUv = vUv;
      if (rainAmt > 0.02) {
        float r1 = rainLayer(vUv, uTime * 1.15, 10.0);
        float r2 = rainLayer(vUv + vec2(0.37, 0.19), uTime * 0.85, 16.0);
        drops = clamp(r1 + r2 * 0.7, 0.0, 1.0) * rainAmt;
        fUv += vec2(drops * 0.014, -drops * 0.018);
      }

      vec4 forest = texture2D(uForest, clamp(fUv, 0.002, 0.998));
      // Convert sRGB sky gradient to linear so it blends naturally with sRGB-decoded forest texture and ACES post-pass
      vec3 skyLin = pow(max(sky, vec3(0.0)), vec3(2.2));
      vec3 botLin = pow(max(uSkyBot, vec3(0.0)), vec3(2.2));
      vec3 col = mix(skyLin, mix(forest.rgb, botLin * 0.65, 0.42), forest.a * 0.86);

      // Raindrop specular glints & diagonal streaks
      if (rainAmt > 0.02) {
        col += vec3(0.65, 0.82, 0.96) * drops * 0.28;
        float streak = sin((vUv.x * 75.0 + vUv.y * 16.0) + uTime * 18.0);
        streak = smoothstep(0.96, 1.0, streak) * rainAmt * 0.12;
        col += vec3(0.65, 0.82, 0.96) * streak;
      }

      // Subtle glass reflection sheen
      float glare = smoothstep(0.65, 0.0, abs((vUv.x * 0.7 + vUv.y * 0.5) - 0.55)) * 0.035;
      col += vec3(1.0) * glare;

      gl_FragColor = vec4(col, 1.0);
    }
  `;

  const SHAFT_VERT = `
    varying vec2 vUv;
    void main() {
      vUv = uv;
      gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
    }
  `;

  const SHAFT_FRAG = `
    precision highp float;
    varying vec2 vUv;
    uniform vec3 uColor;
    uniform float uAlpha;
    uniform float uBlinds;
    uniform float uTime;

    void main() {
      // Fade along beam length and edges
      float along = smoothstep(0.0, 0.14, vUv.x) * (1.0 - smoothstep(0.18, 1.0, vUv.x));
      float edgeY = smoothstep(0.0, 0.16, vUv.y) * (1.0 - smoothstep(0.84, 1.0, vUv.y));
      // Striped blind shadow modulation across upper part of shaft
      float blindRegion = smoothstep(1.0 - uBlinds * 0.72 - 0.05, 1.0 - uBlinds * 0.72 + 0.05, vUv.y);
      float stripes = 0.5 + 0.5 * sin(vUv.y * 54.0);
      float slatMask = mix(1.0, mix(0.18, 1.0, smoothstep(0.25, 0.75, stripes) * (0.28 + 0.72 * uBlinds)), blindRegion);
      float shimmer = 0.92 + 0.08 * sin(uTime * 0.6 + vUv.y * 5.0);
      float a = along * edgeY * slatMask * uAlpha * shimmer;
      gl_FragColor = vec4(uColor, a);
    }
  `;

  // Post-process multi-pass glass shaders
  const POST_VERT = `
    varying vec2 vUv;
    void main() {
      vUv = uv;
      gl_Position = vec4(position.xy, 0.0, 1.0);
    }
  `;

  const POST_BLUR_FRAG = `
    precision highp float;
    varying vec2 vUv;
    uniform sampler2D uTex;
    uniform vec2 uTexel;
    uniform vec2 uDir;
    uniform float uRadius;
    uniform int uTaps;

    void main() {
      float sigma = max(0.8, uRadius * 0.48);
      vec3 sum = texture2D(uTex, vUv).rgb;
      float wSum = 1.0;
      for (int i = 1; i <= 14; i++) {
        if (i > uTaps) break;
        float fi = float(i);
        float w = exp(-0.5 * (fi * fi) / (sigma * sigma));
        vec2 off = uDir * uTexel * (fi * uRadius / max(1.0, float(uTaps)));
        sum += texture2D(uTex, clamp(vUv + off, vec2(0.001), vec2(0.999))).rgb * w;
        sum += texture2D(uTex, clamp(vUv - off, vec2(0.001), vec2(0.999))).rgb * w;
        wSum += 2.0 * w;
      }
      gl_FragColor = vec4(sum / wSum, 1.0);
    }
  `;

  // Real Apple-grade Liquid Glass Optical Shader for Studio Glass 3D (1:1 with studio_glass/shaders.rs)
  // Two-zone crease-free Snell liquid lip + convex dome + 5-band HDR spectral dispersion + dual-lobe sheen
  // Optical refraction model inspired by @samasante/liquid-glass (MIT License)
  const POST_GLASS_FRAG = `
    precision highp float;
    varying vec2 vUv;

    #define MAX_GLASS ${MAX_GLASS}
    #define PI 3.14159265359

    uniform sampler2D uScene;
    uniform sampler2D uBlur;
    uniform vec2 uRes;                 // CSS px
    uniform int uCount;
    uniform vec4 uRects[MAX_GLASS];    // cx, cy, halfW, halfH (CSS px, top-left origin)
    uniform vec4 uMeta[MAX_GLASS];     // cornerRadiusPx, style(0=card,1=prism,2=pill), active(0..1), fillFrac(0..1 for brightness slider)
    uniform float uRefraction;
    uniform float uFrost;
    uniform vec3 uRimTint;
    uniform vec2 uLightPos;            // normalized 0..1
    uniform int uPulseIdx;
    uniform float uPulseAge;
    uniform vec3 uPulseColA;
    uniform vec3 uPulseColB;
    uniform vec3 uPulseColC;
    uniform float uTime;
    uniform float uExposure;

    vec3 acesFilm(vec3 x) {
      x = max(x, vec3(0.0));
      return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
    }

    vec3 gradeScene(vec3 lin) {
      return pow(acesFilm(lin * uExposure), vec3(1.0 / 2.2));
    }

    float safeAsin(float x) {
      return asin(clamp(x, -1.0, 1.0));
    }

    float gHash(vec2 p) {
      p = fract(p * vec2(234.34, 435.345));
      p += dot(p, p + 34.23);
      return fract(p.x * p.y);
    }

    float sdRoundBox(vec2 p, vec2 b, float r) {
      vec2 q = abs(p) - b + vec2(r);
      return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
    }

    float sdOctagon(vec2 p, vec2 b, float c) {
      vec2 q = abs(p) - b;
      float dBox = max(q.x, q.y);
      float dDiag = (abs(p.x) + abs(p.y) - (b.x + b.y - c)) * 0.70710678;
      return max(dBox, dDiag);
    }

    float evalOne(vec2 px, vec4 rc, vec4 mt) {
      if (mt.y > 0.5 && mt.y < 1.5) {
        return sdOctagon(px - rc.xy, rc.zw, min(rc.z, rc.w) * 0.36);
      }
      return sdRoundBox(px - rc.xy, rc.zw, mt.x);
    }

    float evalField(vec2 px, out vec4 bestRc, out vec4 bestMt, out int bestIdx) {
      float d = 1e6;
      bestRc = vec4(0.0, 0.0, 32.0, 32.0);
      bestMt = vec4(14.0, 0.0, 0.0, 0.0);
      bestIdx = -1;
      for (int i = 0; i < MAX_GLASS; i++) {
        if (i >= uCount) break;
        float di = evalOne(px, uRects[i], uMeta[i]);
        if (di < d) {
          d = di;
          bestRc = uRects[i];
          bestMt = uMeta[i];
          bestIdx = i;
        }
      }
      return d;
    }

    float evalOnly(vec2 px) {
      vec4 a, b;
      int idx;
      return evalField(px, a, b, idx);
    }

    vec2 getSmoothGradOne(vec2 px, vec4 rc, vec4 mt, float e) {
      vec2 e1 = vec2( 1.0,  1.0);
      vec2 e2 = vec2(-1.0,  1.0);
      vec2 e3 = vec2( 1.0, -1.0);
      vec2 e4 = vec2(-1.0, -1.0);
      return (e1 * evalOne(px + e * e1, rc, mt)
            + e2 * evalOne(px + e * e2, rc, mt)
            + e3 * evalOne(px + e * e3, rc, mt)
            + e4 * evalOne(px + e * e4, rc, mt)) / (4.0 * e);
    }

    vec3 sampleLinearLayer(vec2 uv, float blurMix) {
      vec2 suv = clamp(uv, vec2(0.0015), vec2(0.9985));
      vec3 sharp = texture2D(uScene, suv).rgb;
      if (blurMix <= 0.005) return sharp;
      vec3 soft = texture2D(uBlur, suv).rgb;
      return mix(sharp, soft, blurMix);
    }

    vec3 sampleSpectralHDR(vec2 uv, vec2 uvOffset, float blurMix, float spread) {
      if (spread <= 0.001) {
        return sampleLinearLayer(uv + uvOffset, blurMix);
      }
      vec3 cR = sampleLinearLayer(uv + uvOffset * (1.0 - spread * 1.00), blurMix);
      vec3 cY = sampleLinearLayer(uv + uvOffset * (1.0 - spread * 0.45), blurMix);
      vec3 cG = sampleLinearLayer(uv + uvOffset,                        blurMix);
      vec3 cC = sampleLinearLayer(uv + uvOffset * (1.0 + spread * 0.45), blurMix);
      vec3 cB = sampleLinearLayer(uv + uvOffset * (1.0 + spread * 1.00), blurMix);

      float r = cR.r * 0.62 + cY.r * 0.38;
      float g = cY.g * 0.24 + cG.g * 0.52 + cC.g * 0.24;
      float b = cC.b * 0.38 + cB.b * 0.62;
      return vec3(r, g, b);
    }

    void main() {
      // Three.js WebGLRenderTarget vUv.y=0 at bottom -> CSS px has y=0 at top
      vec2 px = vec2(vUv.x * uRes.x, (1.0 - vUv.y) * uRes.y);
      vec3 baseCol = gradeScene(texture2D(uScene, vUv).rgb);

      if (uCount == 0) {
        gl_FragColor = vec4(baseCol, 1.0);
        return;
      }

      vec4 rc, mt;
      int bestIdx;
      float d = evalField(px, rc, mt, bestIdx);

      // Soft glass drop shadow + subtle bright caustic focus rim ONLY outside glass footprint
      float shD = evalOnly(px - vec2(0.0, 9.0));
      float shadowMask = smoothstep(-1.5, 2.5, d);
      float outerShadow = exp(-max(shD, 0.0) / 22.0) * 0.32;
      float contactAO = exp(-max(shD, 0.0) / 5.5) * 0.20;
      vec3 outCol = baseCol * (1.0 - clamp(outerShadow + contactAO, 0.0, 0.50) * shadowMask);
      float causticBand = exp(-pow((shD - 3.5) / 8.0, 2.0)) * 0.075 * shadowMask * clamp(uRefraction, 0.0, 1.4);
      outCol = min(vec3(1.0), outCol + mix(vec3(1.0), uRimTint, 0.55) * causticBand);

      if (d < 2.2) {
        float depthPx = max(0.0, -d);
        vec2 localN = (px - rc.xy) / max(vec2(12.0), rc.zw);

        // Tight gradient for crisp specular bevel & directional sheen
        vec2 gRim = getSmoothGradOne(px, rc, mt, 1.1);
        float gRimLen = length(gRim);
        vec2 rimNormalCSS = gRimLen > 1e-4 ? (gRim / gRimLen) : vec2(0.0, -1.0);
        vec2 rimNormalUV = vec2(rimNormalCSS.x, -rimNormalCSS.y);

        // Dual-kernel crease-free optical gradients (smoothly attenuate to 0 on medial axis)
        vec2 gLip = getSmoothGradOne(px, rc, mt, 2.3);
        float gLipLen = length(gLip);
        vec2 lipNormalUV = vec2(gLip.x, -gLip.y) * smoothstep(0.40, 0.96, gLipLen);

        vec2 gDome = getSmoothGradOne(px, rc, mt, 7.2);
        float gDomeLen = length(gDome);
        vec2 domeNormalUV = vec2(gDome.x, -gDome.y) * smoothstep(0.48, 0.98, gDomeLen);

        // Prismatic click burst shockwave inside clicked card
        float pulseOn = (bestIdx == uPulseIdx && uPulseAge >= 0.0 && uPulseAge < 1.75) ? 1.0 : 0.0;
        float pNorm = clamp(uPulseAge / 1.65, 0.0, 1.0);
        float pRise = smoothstep(0.0, 0.16, pNorm);
        float pFall = 1.0 - smoothstep(0.45, 1.0, pNorm);
        float pEnv = pulseOn * pRise * pFall;
        vec2 ringCenter = vec2(0.02, 0.16);
        vec2 ovalP = (localN - ringCenter) / vec2(0.86, 0.56);
        float ovalR = length(ovalP);
        float waveR = mix(0.28, 0.94, pow(pNorm, 0.52));
        float ringWave = exp(-pow((ovalR - waveR) / 0.14, 2.0)) * pEnv;
        float ringHalo = exp(-pow((ovalR - waveR) / 0.42, 2.0)) * pEnv;

        // @samasante/liquid-glass two-zone optical profile
        float minHalf = min(rc.z, rc.w);
        float lipWidth = min(9.5 + 6.5 * uRefraction, max(6.0, minHalf * 0.48));
        float uLip = clamp(depthPx / lipWidth, 0.0, 1.0);
        float sLip = 1.0 - uLip;
        float lipEnvelope = sLip * sLip * (3.0 - 2.0 * sLip);
        float ior = 1.38 + 0.42 * uRefraction;
        float thetaI = safeAsin(pow(sLip, 1.25));
        float thetaT = safeAsin(sin(thetaI) / ior);
        float snellSlope = (depthPx >= lipWidth) ? 0.0 : max(0.0, -tan(thetaT - thetaI));
        float lipBend = snellSlope * lipEnvelope * (1.0 + 1.40 * pow(sLip, 2.4));

        float domeFalloff = exp(-depthPx / (18.0 + 10.0 * uRefraction));
        vec2 waveDir = ovalR > 1e-3 ? normalize(ovalP) : vec2(0.0, 1.0);
        vec2 dispPxVec = (lipNormalUV * (lipBend * 48.0) + domeNormalUV * (domeFalloff * 8.5)) * uRefraction
                       - vec2(waveDir.x, -waveDir.y) * (ringWave * 18.0);
        vec2 refrUV = -dispPxVec / uRes;

        // Crystal-clear unblurred liquid lip -> soft etched frost in interior
        float interiorFrost = clamp(uFrost * 0.42, 0.0, 0.82);
        float blurMix = mix(interiorFrost * 0.06, interiorFrost, smoothstep(0.18, 0.94, uLip));

        // 5-band white-preserving HDR chromatic dispersion
        bool isPrism = (mt.y > 0.5 && mt.y < 1.5);
        float dispScale = (isPrism ? 0.25 : 0.19) * clamp(uRefraction, 0.0, 1.6);
        float spread = dispScale * (0.05 + 1.48 * lipEnvelope) + ringWave * 0.24;

        vec3 glassHdr = sampleSpectralHDR(vUv, refrUV, blurMix, spread);
        vec3 glassCol = gradeScene(glassHdr);

        // Delicate crystal glass clarity + subtle dark readability veil
        float smoke = mix(0.11, 0.05, mt.z);
        glassCol = mix(glassCol, vec3(0.06, 0.08, 0.10), smoke);
        float luma = dot(glassCol, vec3(0.2126, 0.7152, 0.0722));
        glassCol = clamp(mix(vec3(luma), glassCol, 1.08) + vec3(0.018), 0.0, 1.0);

        // Special brightness pill fill (mt.y > 1.5 means brightness slider)
        if (mt.y > 1.5) {
          float localY = clamp(((rc.y + rc.w) - px.y) / max(1.0, rc.w * 2.0), 0.0, 1.0);
          float filled = 1.0 - smoothstep(mt.w - 0.015, mt.w + 0.015, localY);
          vec3 ivory = vec3(0.93, 0.92, 0.88);
          glassCol = mix(glassCol, ivory, filled * 0.90);
        }

        // Subtle internal total-internal-reflection (TIR) dark micro-contour
        float innerDarkBand = exp(-pow((depthPx - 3.4) / 2.1, 2.0)) * 0.13 * clamp(uRefraction, 0.25, 1.4);
        glassCol *= (1.0 - innerDarkBand);

        // Soft inner rim glow tinted by studio window/lamp light
        float activeBoost = 1.0 + mt.z * 0.65;
        float innerGlow = exp(-depthPx / 11.5) * (0.13 + 0.09 * uRefraction) * activeBoost;
        vec3 glowCol = mix(vec3(1.0), uRimTint, 0.45);
        glassCol = mix(glassCol, max(glassCol, glowCol), clamp(innerGlow * 0.50, 0.0, 0.34));

        // Dual-lobe directional specular sheen aligned dynamically with 3D studio window light
        vec2 toLightCSS = uLightPos * uRes - px;
        vec2 lightUV = normalize(vec2(toLightCSS.x, -toLightCSS.y) + vec2(1e-4, 0.0));
        float keyDir = dot(rimNormalUV, lightUV);
        float oppDir = dot(rimNormalUV, -lightUV);
        float primarySheen = pow(max(keyDir, 0.0), 2.4);
        float secondarySheen = pow(max(oppDir, 0.0), 3.6) * 0.56;
        float sheenDir = clamp(primarySheen + secondarySheen, 0.0, 1.0);

        float sheenBand = clamp(pow(clamp(1.0 - depthPx / 12.5 + 0.04, 0.0, 1.0), 4.2), 0.0, 1.0);
        vec3 sheenTint = mix(vec3(1.0), uRimTint, 0.26);
        glassCol = mix(glassCol, min(vec3(1.0), glassCol * 0.55 + sheenTint * 0.82), clamp(sheenDir * sheenBand * (0.72 + 0.18 * mt.z), 0.0, 0.88));

        // Delicate top-arc crystal surface sheen
        float topArc = smoothstep(0.20, 0.95, keyDir) * exp(-depthPx / 19.0) * (0.08 + 0.06 * mt.z) * smoothstep(0.25, 0.95, gRimLen);
        glassCol = min(vec3(1.0), glassCol + mix(vec3(1.0), uRimTint, 0.35) * topArc);

        // Razor-sharp 1.5px polished crystal outer bevel rim
        float rimLine = smoothstep(2.1, 0.35, depthPx);
        float bevelStrength = rimLine * (0.30 + 0.70 * sheenDir) * (0.78 + 0.22 * mt.z);
        glassCol = mix(glassCol, mix(vec3(1.0), uRimTint, 0.22), clamp(bevelStrength, 0.0, 0.92));

        // Iridescent prismatic luminous ring + sparkles + chromatic edge bloom on click!
        if (pEnv > 0.004) {
          float ang = atan(ovalP.y, ovalP.x);
          float s1 = 0.5 + 0.5 * sin(ang * 1.0 + pNorm * 5.2);
          float s2 = 0.5 + 0.5 * cos(ang * 1.0 - pNorm * 3.8 + 1.2);
          vec3 prism = mix(uPulseColA, uPulseColB, s1);
          prism = mix(prism, uPulseColC, s2 * 0.65);

          float cometAng = -2.2 + pNorm * 6.8;
          float dAng = atan(sin(ang - cometAng), cos(ang - cometAng));
          float comet = exp(-pow(dAng / 0.65, 2.0)) * ringWave;

          vec2 spGrid = floor((localN + vec2(0.0, -pNorm * 0.25)) * 22.0);
          vec2 spF = fract((localN + vec2(0.0, -pNorm * 0.25)) * 22.0) - 0.5;
          float spH = gHash(spGrid);
          float sparkle = step(0.84, spH)
                        * smoothstep(0.26, 0.04, length(spF))
                        * (0.45 + 0.55 * sin(uTime * 14.0 + spH * 50.0))
                        * ringHalo * 1.65;

          float cornerWash = smoothstep(0.05, 1.05, localN.y) * pEnv * 0.42;
          vec3 washCol = mix(uPulseColC, uPulseColB, clamp(localN.x * 0.5 + 0.5, 0.0, 1.0));

          glassCol += prism * (ringHalo * 0.58 + ringWave * 0.72);
          glassCol += vec3(1.0, 0.96, 0.92) * (ringWave * 0.48 + comet * 0.95 + sparkle);
          glassCol += washCol * (cornerWash + rimLine * pEnv * 0.65);
        }

        float mask = 1.0 - smoothstep(-1.0, 1.0, d);
        outCol = mix(outCol, clamp(glassCol, 0.0, 1.0), mask);
      }

      gl_FragColor = vec4(clamp(outCol, 0.0, 1.0), 1.0);
    }
  `;

  // =========================================================================
  // StudioGlassEngine
  // =========================================================================
  function StudioGlassEngine(canvas, defaults, hudEl) {
    this.cv = canvas;
    this.hudEl = hudEl || null;
    this.P = Object.assign({
      adaptive: 1,
      preset: 1,          // 0=Auto, 1=Morning, 2=Thunderstorm, 3=Afternoon, 4=Sunset, 5=Night
      blinds: 0.70,       // 0..1 open
      shafts: 0.80,       // 0..1 volumetric beam intensity
      weather: 0.78,      // 0..1 rain & dust density
      monitorAngle: -10,  // -35..35 deg
      deskLamp: 0.70,     // 0..1 brightness multiplier
      parallax: 0.55,     // 0..1 camera depth parallax
      refraction: 0.85,   // 0..1.4 optical refraction
      frost: 0.50,        // 0..1 frost blur
      iconGlass: 0,       // 0=Card, 1=Prism, 2=Off
      showHud: 1,         // 0=Off, 1=On (interactive glass lighting bar)
      quality: 1          // 0=Low, 1=Balanced, 2=High
    }, defaults || {});

    this.W = 1280;
    this.H = 720;
    this.scale = 1;
    this.lowMode = false;
    this.icons = [];
    this.hudCards = []; // synced DOM bounding boxes of Studio Lighting glass cards
    this.time = 0;

    // Interactive state (direct dragging in 3D + HUD override)
    this.interactivePreset = null;
    this.presetBrightness = PRESETS.map(p => p.pct / 100);
    this.interactiveBrightness = PRESETS[0].pct / 100;
    this.interactiveBlinds = null;
    this.interactiveMonitor = null;
    this.lampManual = null; // null = auto by preset, true/false = user toggled

    // Prismatic click burst state for HUD glass cards
    this.pulseCardIdx = -1;
    this.pulseAge = 99;
    this.pulsePreset = 0;

    this.curBlinds = this.P.blinds;
    this.curMonitor = this.P.monitorAngle;
    this.ptr = { x: 0.5, y: 0.5, tx: 0.5, ty: 0.5, ndcX: 0, ndcY: 0, px: 0, py: 0, active: false };

    // 3D Raycasting drag state
    this.draggables = [];
    this.dragItem = null;
    this.dragPlane = null;
    this.dragOffset = null;
    this.dragStartX = 0;
    this.dragStartY = 0;
    this.dragStartVal = 0;
    this.dragMoved = false;

    // Spring leaf sway for windowsill plants
    this.plantSway = [
      { ang: 0, vel: 0 },
      { ang: 0, vel: 0 }
    ];

    // Lightning state for Thunderstorm preset
    this.flash = 0;
    this.nextFlash = 2.8;

    // Smooth interpolated mood values
    const idx = this._targetPresetIndex();
    this.mood = this._cloneMood(PRESETS[idx]);

    this._initThree();
  }

  StudioGlassEngine.prototype._cloneMood = function (src) {
    const THREE = global.THREE;
    return {
      sunColor: new THREE.Color(src.sunColor),
      sunInt: src.sunInt,
      sunPos: new THREE.Vector3(src.sunPos[0], src.sunPos[1], src.sunPos[2]),
      hemiSky: new THREE.Color(src.hemiSky),
      hemiGround: new THREE.Color(src.hemiGround),
      hemiInt: src.hemiInt,
      ambColor: new THREE.Color(src.ambColor),
      ambInt: src.ambInt,
      lampInt: src.lampInt,
      lampGlow: src.lampGlow,
      screenInt: src.screenInt,
      skyTop: new THREE.Color(src.skyTop[0] / 255, src.skyTop[1] / 255, src.skyTop[2] / 255),
      skyMid: new THREE.Color(src.skyMid[0] / 255, src.skyMid[1] / 255, src.skyMid[2] / 255),
      skyBot: new THREE.Color(src.skyBot[0] / 255, src.skyBot[1] / 255, src.skyBot[2] / 255),
      shaftCol: new THREE.Color(src.shaftCol[0], src.shaftCol[1], src.shaftCol[2]),
      shaftAlpha: src.shaftAlpha,
      rain: src.rain,
      thunder: src.thunder,
      dust: src.dust,
      stars: src.stars,
      rimTint: new THREE.Vector3(src.rimTint[0], src.rimTint[1], src.rimTint[2]),
      exposure: src.exposure,
      wallTint: new THREE.Color(src.wallTint),
      nicheTint: new THREE.Color(src.nicheTint)
    };
  };

  StudioGlassEngine.prototype._targetPresetIndex = function () {
    if (this.interactivePreset !== null) return clamp(this.interactivePreset, 0, PRESETS.length - 1);
    const p = Math.round(this.P.preset || 0);
    if (p <= 0) return autoPresetIndex();
    return clamp(p - 1, 0, PRESETS.length - 1);
  };

  StudioGlassEngine.prototype.getActivePresetIndex = function () {
    return this._targetPresetIndex();
  };

  StudioGlassEngine.prototype.selectPresetFromHud = function (idx, hudCardSlot) {
    const next = clamp(idx, 0, PRESETS.length - 1);
    this.interactivePreset = next;
    this.interactiveBrightness = this.presetBrightness[next];
    this.lampManual = null;
    this.triggerCardPulse(hudCardSlot !== undefined ? hudCardSlot : next, next);
    if (next === 1) {
      // Immediate subtle storm lightning flash when clicking Thunderstorm
      this.flash = 0.92;
      this.nextFlash = 4.2;
    }
  };

  StudioGlassEngine.prototype.triggerCardPulse = function (cardSlot, presetIdx) {
    this.pulseCardIdx = cardSlot;
    this.pulseAge = 0.0;
    this.pulsePreset = clamp(presetIdx || 0, 0, PRESETS.length - 1);
  };

  StudioGlassEngine.prototype.setBrightnessFromHud = function (val) {
    const clamped = clamp(val, 0.08, 1.0);
    this.interactiveBrightness = clamped;
    this.presetBrightness[this._targetPresetIndex()] = clamped;
  };

  StudioGlassEngine.prototype._initThree = function () {
    const THREE = global.THREE;
    if (!THREE) throw new Error('Three.js failed to load');

    this.renderer = new THREE.WebGLRenderer({
      canvas: this.cv,
      antialias: true,
      alpha: false,
      powerPreference: 'default',
      preserveDrawingBuffer: false
    });
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.06;
    this.renderer.shadowMap.enabled = true;
    this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;

    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x0c1412);

    this.camera = new THREE.PerspectiveCamera(36, 16 / 9, 0.1, 45);
    this.baseCamPos = new THREE.Vector3(0.45, 2.02, 5.75);
    this.baseCamTarget = new THREE.Vector3(-0.08, 1.48, -0.55);
    this.camera.position.copy(this.baseCamPos);
    this.camera.lookAt(this.baseCamTarget);

    this.raycaster = new THREE.Raycaster();
    this.dragPlane = new THREE.Plane();
    this.dragOffset = new THREE.Vector3();
    this._hitPt = new THREE.Vector3();

    this._buildLights();
    this._buildRoomAndWindow();
    this._buildDeskAndProps();
    this._buildChair();
    this._buildSideTableAndLamp();
    this._buildVolumetricShaftsAndDust();
    this._initPostProcess();
  };

  StudioGlassEngine.prototype._buildLights = function () {
    const THREE = global.THREE;
    this.hemiLight = new THREE.HemisphereLight(0x8fb8b2, 0x26211c, 0.95);
    this.scene.add(this.hemiLight);

    this.ambLight = new THREE.AmbientLight(0x35443e, 0.45);
    this.scene.add(this.ambLight);

    // Main sun / moon directional light streaming through the left window
    this.sunLight = new THREE.DirectionalLight(0xffe4be, 3.1);
    this.sunLight.position.set(-7.5, 5.2, -1.6);
    this.sunLight.target.position.set(1.2, 0.8, 0.4);
    this.sunLight.castShadow = true;
    this.sunLight.shadow.mapSize.set(1024, 1024);
    this.sunLight.shadow.camera.near = 1.0;
    this.sunLight.shadow.camera.far = 18.0;
    this.sunLight.shadow.camera.left = -5.5;
    this.sunLight.shadow.camera.right = 5.5;
    this.sunLight.shadow.camera.top = 5.0;
    this.sunLight.shadow.camera.bottom = -3.5;
    this.sunLight.shadow.bias = -0.0005;
    this.scene.add(this.sunLight);
    this.scene.add(this.sunLight.target);

    // Warm bounce from mushroom lamp on right side table
    this.lampLight = new THREE.PointLight(0xffb45a, 1.4, 7.5, 1.7);
    this.lampLight.position.set(2.92, 1.68, -1.15);
    this.lampLight.castShadow = true;
    this.lampLight.shadow.mapSize.set(512, 512);
    this.lampLight.shadow.bias = -0.001;
    this.scene.add(this.lampLight);

    // Soft screen glow spilling onto keyboard & desk
    this.screenLight = new THREE.PointLight(0xd2dff0, 1.15, 5.4, 1.5);
    this.screenLight.position.set(0.45, 1.72, 0.25);
    this.scene.add(this.screenLight);

    // Subtle front-left window bounce fill so walnut desk grain, keyboard & chair stay readable
    this.deskFillLight = new THREE.DirectionalLight(0xd6e4df, 0.68);
    this.deskFillLight.position.set(-2.2, 3.4, 3.8);
    this.deskFillLight.target.position.set(0.5, 0.9, 0.2);
    this.scene.add(this.deskFillLight);
    this.scene.add(this.deskFillLight.target);
  };

  StudioGlassEngine.prototype._buildRoomAndWindow = function () {
    const THREE = global.THREE;
    const plasterGreenTex = createPlasterTexture(THREE, '#1f3630');
    const plasterNicheTex = createPlasterTexture(THREE, '#534822');
    this.walnutTex = createWalnutTexture(THREE);

    this.mainWallMat = new THREE.MeshStandardMaterial({
      map: plasterGreenTex,
      color: 0xffffff,
      roughness: 0.90,
      metalness: 0.02
    });
    this.nicheWallMat = new THREE.MeshStandardMaterial({
      map: plasterNicheTex,
      color: 0xffffff,
      roughness: 0.88,
      metalness: 0.02
    });

    // Dark studio floor
    const floor = new THREE.Mesh(
      new THREE.PlaneGeometry(16, 14),
      new THREE.MeshStandardMaterial({ color: 0x161412, roughness: 0.86, metalness: 0.05 })
    );
    floor.rotation.x = -Math.PI / 2;
    floor.position.set(0, 0, 0);
    floor.receiveShadow = true;
    this.scene.add(floor);

    // Main dark forest-green textured studio back wall (behind monitor)
    const backWall = new THREE.Mesh(new THREE.BoxGeometry(5.6, 5.4, 0.28), this.mainWallMat);
    backWall.position.set(-0.2, 2.5, -1.85);
    backWall.receiveShadow = true;
    this.scene.add(backWall);

    // Right architectural recess wall (warm ochre / mustard accent like reference)
    const rightBackWall = new THREE.Mesh(new THREE.BoxGeometry(4.4, 5.4, 0.25), this.nicheWallMat);
    rightBackWall.position.set(4.3, 2.5, -2.35);
    rightBackWall.receiveShadow = true;
    this.scene.add(rightBackWall);

    // Vertical corner step pillar between green wall and warm ochre niche
    const cornerStep = new THREE.Mesh(new THREE.BoxGeometry(0.42, 5.4, 0.62), this.nicheWallMat);
    cornerStep.position.set(2.42, 2.5, -1.98);
    cornerStep.castShadow = true;
    cornerStep.receiveShadow = true;
    this.scene.add(cornerStep);

    // Ceiling plane to bounce soft light
    const ceiling = new THREE.Mesh(
      new THREE.PlaneGeometry(16, 14),
      new THREE.MeshStandardMaterial({ color: 0x141a18, roughness: 0.95 })
    );
    ceiling.rotation.x = Math.PI / 2;
    ceiling.position.set(0, 4.9, 0);
    this.scene.add(ceiling);

    // -----------------------------------------------------------------------
    // Angled Left Architectural Corner Wall with Deep Wooden Window
    // -----------------------------------------------------------------------
    this.windowGroup = new THREE.Group();
    this.windowGroup.position.set(-3.12, 0, -0.32);
    this.windowGroup.rotation.y = 0.72; // angled gracefully into the studio camera
    this.scene.add(this.windowGroup);

    // Wall surround around window opening (top, bottom, left, right)
    const wallSideMat = this.mainWallMat;
    const wBottom = new THREE.Mesh(new THREE.BoxGeometry(4.4, 1.02, 0.34), wallSideMat);
    wBottom.position.set(0, 0.51, 0);
    wBottom.receiveShadow = true;
    this.windowGroup.add(wBottom);

    const wTop = new THREE.Mesh(new THREE.BoxGeometry(4.4, 1.35, 0.34), wallSideMat);
    wTop.position.set(0, 4.25, 0);
    wTop.receiveShadow = true;
    this.windowGroup.add(wTop);

    const wLeft = new THREE.Mesh(new THREE.BoxGeometry(0.85, 2.75, 0.34), wallSideMat);
    wLeft.position.set(-1.78, 2.32, 0);
    wLeft.receiveShadow = true;
    this.windowGroup.add(wLeft);

    const wRight = new THREE.Mesh(new THREE.BoxGeometry(0.85, 2.75, 0.34), wallSideMat);
    wRight.position.set(1.78, 2.32, 0);
    wRight.receiveShadow = true;
    this.windowGroup.add(wRight);

    // Deep walnut wooden window frame, mullions & sill
    const woodFrameMat = new THREE.MeshStandardMaterial({
      map: this.walnutTex,
      color: 0x6b452c,
      roughness: 0.58,
      metalness: 0.04
    });

    const sill = new THREE.Mesh(new THREE.BoxGeometry(2.96, 0.09, 0.52), woodFrameMat);
    sill.position.set(0, 1.04, 0.14);
    sill.castShadow = true;
    sill.receiveShadow = true;
    this.windowGroup.add(sill);

    // Outer frame rails
    const fTop = new THREE.Mesh(new THREE.BoxGeometry(2.84, 0.10, 0.24), woodFrameMat);
    fTop.position.set(0, 3.60, 0.02);
    this.windowGroup.add(fTop);

    const fLeft = new THREE.Mesh(new THREE.BoxGeometry(0.10, 2.58, 0.24), woodFrameMat);
    fLeft.position.set(-1.36, 2.32, 0.02);
    this.windowGroup.add(fLeft);

    const fRight = new THREE.Mesh(new THREE.BoxGeometry(0.10, 2.58, 0.24), woodFrameMat);
    fRight.position.set(1.36, 2.32, 0.02);
    this.windowGroup.add(fRight);

    // Center vertical & horizontal mullion crossbars + brass handle
    const mullV = new THREE.Mesh(new THREE.BoxGeometry(0.085, 2.52, 0.18), woodFrameMat);
    mullV.position.set(0.08, 2.32, 0.02);
    mullV.castShadow = true;
    this.windowGroup.add(mullV);

    const mullH = new THREE.Mesh(new THREE.BoxGeometry(2.68, 0.065, 0.15), woodFrameMat);
    mullH.position.set(0, 2.78, 0.02);
    mullH.castShadow = true;
    this.windowGroup.add(mullH);

    const brassMat = new THREE.MeshStandardMaterial({ color: 0xc9a462, roughness: 0.32, metalness: 0.82 });
    const handle = new THREE.Mesh(new THREE.BoxGeometry(0.025, 0.14, 0.05), brassMat);
    handle.position.set(0.08, 2.15, 0.12);
    this.windowGroup.add(handle);

    // Outdoor sky + misty woodland + animated rain-on-glass shader pane
    this.forestTex = createOutdoorForestTexture(THREE);
    this.windowPaneMat = new THREE.ShaderMaterial({
      vertexShader: WINDOW_GLASS_VERT,
      fragmentShader: WINDOW_GLASS_FRAG,
      uniforms: {
        uTime: { value: 0 },
        uRain: { value: 0 },
        uWeather: { value: 0.78 },
        uSkyTop: { value: new THREE.Color(0.27, 0.42, 0.42) },
        uSkyMid: { value: new THREE.Color(0.46, 0.60, 0.54) },
        uSkyBot: { value: new THREE.Color(0.83, 0.81, 0.70) },
        uStars: { value: 0 },
        uFlash: { value: 0 },
        uForest: { value: this.forestTex }
      }
    });
    const windowPane = new THREE.Mesh(new THREE.PlaneGeometry(2.66, 2.52), this.windowPaneMat);
    windowPane.position.set(0, 2.32, -0.06);
    this.windowGroup.add(windowPane);

    // -----------------------------------------------------------------------
    // 3D Wooden Venetian Blinds (Draggable vertically!)
    // -----------------------------------------------------------------------
    this.blindsGroup = new THREE.Group();
    this.blindsGroup.position.set(0, 3.55, 0.12);
    this.windowGroup.add(this.blindsGroup);

    const slatMat = new THREE.MeshStandardMaterial({
      map: this.walnutTex,
      color: 0x8a5b39,
      roughness: 0.55,
      metalness: 0.05
    });
    // Top wooden valance box
    const valance = new THREE.Mesh(new THREE.BoxGeometry(2.74, 0.12, 0.14), slatMat);
    valance.position.set(0, 0.02, 0.02);
    this.blindsGroup.add(valance);

    this.blindSlats = [];
    const slatCount = 26;
    const slatGeo = new THREE.BoxGeometry(2.62, 0.014, 0.088);
    for (let i = 0; i < slatCount; i++) {
      const slat = new THREE.Mesh(slatGeo, slatMat);
      slat.castShadow = true;
      slat.receiveShadow = true;
      this.blindsGroup.add(slat);
      this.blindSlats.push(slat);
    }

    // Weighted bottom blind rail
    this.blindBottomRail = new THREE.Mesh(new THREE.BoxGeometry(2.64, 0.042, 0.095), slatMat);
    this.blindBottomRail.castShadow = true;
    this.blindsGroup.add(this.blindBottomRail);

    // Invisible hit collider for smooth blinds dragging
    const blindsHit = new THREE.Mesh(
      new THREE.PlaneGeometry(2.74, 2.48),
      new THREE.MeshBasicMaterial({ visible: false })
    );
    blindsHit.position.set(0, 2.34, 0.17);
    this.windowGroup.add(blindsHit);
    this._registerDraggable(blindsHit, { kind: 'blinds', target: this.blindsGroup });

    // -----------------------------------------------------------------------
    // Two Interactive Potted Plants on the Windowsill (Draggable + Leaf Sway)
    // -----------------------------------------------------------------------
    this.plantLeft = this._createPottedPlant(0.18, 0.22, 16, 101);
    this.plantLeft.position.set(-0.66, 1.09, 0.20);
    this.windowGroup.add(this.plantLeft);
    this._registerDraggable(this.plantLeft, {
      kind: 'sillSlide',
      target: this.plantLeft,
      plantIdx: 0,
      minX: -1.12,
      maxX: -0.05
    });

    this.plantRight = this._createPottedPlant(0.14, 0.17, 11, 202);
    this.plantRight.position.set(0.68, 1.09, 0.20);
    this.windowGroup.add(this.plantRight);
    this._registerDraggable(this.plantRight, {
      kind: 'sillSlide',
      target: this.plantRight,
      plantIdx: 1,
      minX: 0.15,
      maxX: 1.12
    });
  };

  StudioGlassEngine.prototype._createPottedPlant = function (potR, potH, leafCount, seed) {
    const THREE = global.THREE;
    const rng = makeRng(seed);
    const g = new THREE.Group();

    // Matte dark ceramic pot
    const potMat = new THREE.MeshStandardMaterial({ color: 0x26292a, roughness: 0.78, metalness: 0.05 });
    const pot = new THREE.Mesh(new THREE.CylinderGeometry(potR, potR * 0.74, potH, 18), potMat);
    pot.position.y = potH * 0.5;
    pot.castShadow = true;
    pot.receiveShadow = true;
    g.add(pot);

    // Soil disk
    const soil = new THREE.Mesh(
      new THREE.CylinderGeometry(potR * 0.88, potR * 0.88, 0.02, 14),
      new THREE.MeshStandardMaterial({ color: 0x221914, roughness: 0.96 })
    );
    soil.position.y = potH * 0.94;
    g.add(soil);

    // Foliage crown (separate child group so leaves sway smoothly with spring physics)
    const crown = new THREE.Group();
    crown.position.y = potH * 0.92;
    g.add(crown);
    g.userData.crown = crown;

    const leafMatA = new THREE.MeshStandardMaterial({ color: 0x243b2a, roughness: 0.62, side: THREE.DoubleSide });
    const leafMatB = new THREE.MeshStandardMaterial({ color: 0x304d34, roughness: 0.56, side: THREE.DoubleSide });
    const stemMat = new THREE.MeshStandardMaterial({ color: 0x2a3b26, roughness: 0.82 });

    for (let i = 0; i < leafCount; i++) {
      const ang = (i / leafCount) * Math.PI * 2 + (rng() - 0.5) * 0.45;
      const tilt = 0.25 + rng() * 0.68;
      const len = potH * (0.95 + rng() * 1.15);

      const branch = new THREE.Group();
      branch.rotation.y = ang;
      branch.rotation.z = tilt;

      const stem = new THREE.Mesh(new THREE.CylinderGeometry(0.006, 0.009, len, 5), stemMat);
      stem.position.y = len * 0.5;
      branch.add(stem);

      const leaf = new THREE.Mesh(
        new THREE.SphereGeometry(potR * (0.34 + rng() * 0.24), 7, 6),
        i % 2 === 0 ? leafMatA : leafMatB
      );
      leaf.scale.set(1.35, 0.28, 0.72);
      leaf.position.set(0, len, 0);
      leaf.rotation.z = 0.35;
      leaf.castShadow = true;
      branch.add(leaf);
      crown.add(branch);
    }

    return g;
  };

  StudioGlassEngine.prototype._buildDeskAndProps = function () {
    const THREE = global.THREE;

    // -----------------------------------------------------------------------
    // Walnut Designer Desk
    // -----------------------------------------------------------------------
    this.deskY = 0.92;
    const deskMat = new THREE.MeshStandardMaterial({
      map: this.walnutTex,
      color: 0xa26c44,
      roughness: 0.42,
      metalness: 0.04
    });

    const deskTop = new THREE.Mesh(new THREE.BoxGeometry(5.1, 0.095, 1.92), deskMat);
    deskTop.position.set(0.55, this.deskY - 0.0475, 0.08);
    deskTop.castShadow = true;
    deskTop.receiveShadow = true;
    this.scene.add(deskTop);

    // Desk steel/wood side legs
    const legMat = new THREE.MeshStandardMaterial({ color: 0x1b1c1f, roughness: 0.55, metalness: 0.5 });
    const legGeo = new THREE.BoxGeometry(0.08, this.deskY - 0.08, 1.56);
    const legL = new THREE.Mesh(legGeo, legMat);
    legL.position.set(-1.82, (this.deskY - 0.08) * 0.5, 0.08);
    const legR = new THREE.Mesh(legGeo, legMat);
    legR.position.set(2.92, (this.deskY - 0.08) * 0.5, 0.08);
    this.scene.add(legL, legR);

    // -----------------------------------------------------------------------
    // 3D Swiveling Studio Display Monitor (Draggable swivel!)
    // -----------------------------------------------------------------------
    this.monitorGroup = new THREE.Group();
    this.monitorGroup.position.set(0.62, this.deskY, -0.34);
    this.scene.add(this.monitorGroup);

    const aluMat = new THREE.MeshStandardMaterial({ color: 0xc2c7ce, roughness: 0.28, metalness: 0.85 });
    const bezelMat = new THREE.MeshStandardMaterial({ color: 0x111317, roughness: 0.25, metalness: 0.35 });

    // Stand foot & vertical L-arm
    const foot = new THREE.Mesh(new THREE.BoxGeometry(0.56, 0.018, 0.42), aluMat);
    foot.position.set(0, 0.009, 0.02);
    foot.castShadow = true;
    foot.receiveShadow = true;
    this.monitorGroup.add(foot);

    const neck = new THREE.Mesh(new THREE.BoxGeometry(0.28, 0.42, 0.024), aluMat);
    neck.position.set(0, 0.22, -0.10);
    neck.rotation.x = 0.06;
    neck.castShadow = true;
    this.monitorGroup.add(neck);

    // Swiveling screen head
    this.monitorHead = new THREE.Group();
    this.monitorHead.position.set(0, 0.82, -0.05);
    this.monitorGroup.add(this.monitorHead);

    const backHousing = new THREE.Mesh(new THREE.BoxGeometry(1.78, 1.06, 0.048), aluMat);
    backHousing.castShadow = true;
    backHousing.receiveShadow = true;
    this.monitorHead.add(backHousing);

    const frontBezel = new THREE.Mesh(new THREE.BoxGeometry(1.75, 1.03, 0.014), bezelMat);
    frontBezel.position.z = 0.025;
    this.monitorHead.add(frontBezel);

    this.monitorTex = createMonitorWallpaperTexture(THREE);
    this.monitorScreenMat = new THREE.MeshStandardMaterial({
      map: this.monitorTex,
      emissiveMap: this.monitorTex,
      emissive: new THREE.Color(0xffffff),
      emissiveIntensity: 0.72,
      roughness: 0.14,
      metalness: 0.05
    });
    const screen = new THREE.Mesh(new THREE.PlaneGeometry(1.66, 0.95), this.monitorScreenMat);
    screen.position.z = 0.034;
    this.monitorHead.add(screen);

    this._registerDraggable(this.monitorHead, { kind: 'monitor', target: this.monitorHead });

    // -----------------------------------------------------------------------
    // 3D Mechanical Keyboard (Draggable on desk!)
    // -----------------------------------------------------------------------
    this.keyboard = new THREE.Group();
    this.keyboard.position.set(0.32, this.deskY, 0.40);
    this.keyboard.rotation.y = 0.04;
    this.scene.add(this.keyboard);

    const kbBase = new THREE.Mesh(
      new THREE.BoxGeometry(0.84, 0.036, 0.30),
      new THREE.MeshStandardMaterial({ color: 0x26292e, roughness: 0.42, metalness: 0.62 })
    );
    kbBase.position.y = 0.018;
    kbBase.castShadow = true;
    kbBase.receiveShadow = true;
    this.keyboard.add(kbBase);

    // 60+ Instanced 3D keycaps (sculpted dark charcoal + warm cream accent keys)
    const keyGeo = new THREE.BoxGeometry(0.046, 0.022, 0.044);
    const keyMat = new THREE.MeshStandardMaterial({ color: 0x3b3e45, roughness: 0.55, metalness: 0.08 });
    const rows = 5, cols = 14;
    const keysInst = new THREE.InstancedMesh(keyGeo, keyMat, rows * cols);
    keysInst.castShadow = true;
    keysInst.receiveShadow = true;
    const dummy = new THREE.Object3D();
    const colA = new THREE.Color(0x3a3d42);
    const colB = new THREE.Color(0x5a574f);
    const colAccent = new THREE.Color(0xb8844c);
    let idx = 0;
    for (let r = 0; r < rows; r++) {
      for (let c = 0; c < cols; c++) {
        const kx = -0.365 + c * 0.056;
        const kz = -0.104 + r * 0.052;
        const isSpace = r === 4 && c >= 4 && c <= 8;
        dummy.position.set(kx, 0.042 + (4 - r) * 0.0025, kz);
        dummy.scale.set(isSpace ? 1.15 : 1, 1, 1);
        dummy.updateMatrix();
        keysInst.setMatrixAt(idx, dummy.matrix);
        const isEdge = c === 0 || c === cols - 1 || (r === 4 && (c <= 2 || c >= 10));
        keysInst.setColorAt(idx, (r === 0 && c === 0) || (r === 2 && c === cols - 1) ? colAccent : (isEdge ? colB : colA));
        idx++;
      }
    }
    keysInst.instanceMatrix.needsUpdate = true;
    if (keysInst.instanceColor) keysInst.instanceColor.needsUpdate = true;
    this.keyboard.add(keysInst);

    this._registerDraggable(this.keyboard, {
      kind: 'deskDrag',
      target: this.keyboard,
      minX: -1.25, maxX: 2.1, minZ: -0.05, maxZ: 0.82
    });

    // -----------------------------------------------------------------------
    // 3D Wireless Studio Mouse (Draggable on desk!)
    // -----------------------------------------------------------------------
    this.mouseObj = new THREE.Group();
    this.mouseObj.position.set(1.06, this.deskY, 0.48);
    this.mouseObj.rotation.y = -0.14;
    this.scene.add(this.mouseObj);

    const mouseBody = new THREE.Mesh(
      new THREE.SphereGeometry(0.085, 18, 14),
      new THREE.MeshStandardMaterial({ color: 0x2c2f34, roughness: 0.36, metalness: 0.18 })
    );
    mouseBody.scale.set(0.86, 0.44, 1.38);
    mouseBody.position.y = 0.028;
    mouseBody.castShadow = true;
    mouseBody.receiveShadow = true;
    this.mouseObj.add(mouseBody);

    const wheel = new THREE.Mesh(
      new THREE.CylinderGeometry(0.012, 0.012, 0.022, 10),
      new THREE.MeshStandardMaterial({ color: 0x7a808a, roughness: 0.35, metalness: 0.7 })
    );
    wheel.rotation.z = Math.PI / 2;
    wheel.position.set(0, 0.055, -0.036);
    this.mouseObj.add(wheel);

    this._registerDraggable(this.mouseObj, {
      kind: 'deskDrag',
      target: this.mouseObj,
      minX: -1.1, maxX: 2.35, minZ: -0.05, maxZ: 0.86
    });

    // -----------------------------------------------------------------------
    // Ceramic Pen Cup with Pencils (Draggable!)
    // -----------------------------------------------------------------------
    this.penCup = new THREE.Group();
    this.penCup.position.set(-0.36, this.deskY, 0.10);
    this.scene.add(this.penCup);

    const cup = new THREE.Mesh(
      new THREE.CylinderGeometry(0.072, 0.064, 0.18, 16),
      new THREE.MeshStandardMaterial({ color: 0x3b3530, roughness: 0.85, metalness: 0.04 })
    );
    cup.position.y = 0.09;
    cup.castShadow = true;
    cup.receiveShadow = true;
    this.penCup.add(cup);

    const penCols = [0x1c1f24, 0xc89a56, 0x3b6c6a, 0x8f4338, 0xd9d2c5];
    for (let i = 0; i < penCols.length; i++) {
      const pen = new THREE.Mesh(
        new THREE.CylinderGeometry(0.007, 0.007, 0.24, 7),
        new THREE.MeshStandardMaterial({ color: penCols[i], roughness: 0.45 })
      );
      const a = (i / penCols.length) * Math.PI * 2;
      pen.position.set(Math.cos(a) * 0.024, 0.145, Math.sin(a) * 0.024);
      pen.rotation.z = (i - 2) * 0.09;
      pen.rotation.x = Math.sin(a) * 0.08;
      pen.castShadow = true;
      this.penCup.add(pen);
    }

    this._registerDraggable(this.penCup, {
      kind: 'deskDrag',
      target: this.penCup,
      minX: -1.5, maxX: 2.3, minZ: -0.52, maxZ: 0.80
    });

    // -----------------------------------------------------------------------
    // Black Leather Sketchbook / Notebook (Draggable!)
    // -----------------------------------------------------------------------
    this.notebook = new THREE.Group();
    this.notebook.position.set(-0.84, this.deskY, 0.28);
    this.notebook.rotation.y = 0.16;
    this.scene.add(this.notebook);

    const nbCover = new THREE.Mesh(
      new THREE.BoxGeometry(0.32, 0.022, 0.42),
      new THREE.MeshStandardMaterial({ color: 0x1e1f22, roughness: 0.72 })
    );
    nbCover.position.y = 0.011;
    nbCover.castShadow = true;
    nbCover.receiveShadow = true;
    this.notebook.add(nbCover);

    const nbPages = new THREE.Mesh(
      new THREE.BoxGeometry(0.305, 0.016, 0.402),
      new THREE.MeshStandardMaterial({ color: 0xe2dacb, roughness: 0.85 })
    );
    nbPages.position.set(0.006, 0.011, 0);
    this.notebook.add(nbPages);

    this._registerDraggable(this.notebook, {
      kind: 'deskDrag',
      target: this.notebook,
      minX: -1.55, maxX: 2.2, minZ: -0.45, maxZ: 0.76
    });
  };

  StudioGlassEngine.prototype._buildChair = function () {
    const THREE = global.THREE;

    // Ergonomic studio task chair in the foreground-left (draggable X & rotatable!)
    this.chair = new THREE.Group();
    this.chair.position.set(-0.56, 0, 1.38);
    this.chair.rotation.y = 0.42;
    this.scene.add(this.chair);

    const fabricMat = new THREE.MeshStandardMaterial({ color: 0x1c2123, roughness: 0.86, metalness: 0.04 });
    const shellMat = new THREE.MeshStandardMaterial({ color: 0x14171a, roughness: 0.48, metalness: 0.35 });
    const chromeMat = new THREE.MeshStandardMaterial({ color: 0x626973, roughness: 0.32, metalness: 0.82 });

    // Seat cushion
    const seat = new THREE.Mesh(new THREE.BoxGeometry(0.76, 0.095, 0.72), fabricMat);
    seat.position.set(0, 0.64, 0);
    seat.castShadow = true;
    seat.receiveShadow = true;
    this.chair.add(seat);

    // Curved ergonomic backrest
    const back = new THREE.Mesh(new THREE.BoxGeometry(0.74, 0.68, 0.08), fabricMat);
    back.position.set(0, 1.02, 0.34);
    back.rotation.x = 0.14;
    back.castShadow = true;
    back.receiveShadow = true;
    this.chair.add(back);

    const backRim = new THREE.Mesh(new THREE.BoxGeometry(0.77, 0.71, 0.04), shellMat);
    backRim.position.set(0, 1.02, 0.375);
    backRim.rotation.x = 0.14;
    this.chair.add(backRim);

    // Armrests
    for (const side of [-1, 1]) {
      const armPost = new THREE.Mesh(new THREE.BoxGeometry(0.04, 0.24, 0.05), shellMat);
      armPost.position.set(side * 0.39, 0.78, 0.05);
      const armTop = new THREE.Mesh(new THREE.BoxGeometry(0.07, 0.032, 0.38), shellMat);
      armTop.position.set(side * 0.39, 0.90, 0.04);
      armTop.castShadow = true;
      this.chair.add(armPost, armTop);
    }

    // Center hydraulic column & 5-star wheel base
    const col = new THREE.Mesh(new THREE.CylinderGeometry(0.042, 0.05, 0.56, 12), chromeMat);
    col.position.y = 0.34;
    this.chair.add(col);
    for (let i = 0; i < 5; i++) {
      const a = (i / 5) * Math.PI * 2;
      const spoke = new THREE.Mesh(new THREE.BoxGeometry(0.04, 0.035, 0.38), shellMat);
      spoke.position.set(Math.sin(a) * 0.18, 0.07, Math.cos(a) * 0.18);
      spoke.rotation.y = a;
      this.chair.add(spoke);
    }

    this._registerDraggable(this.chair, {
      kind: 'chairDrag',
      target: this.chair,
      minX: -1.55,
      maxX: 1.55
    });
  };

  StudioGlassEngine.prototype._buildSideTableAndLamp = function () {
    const THREE = global.THREE;

    // Right walnut side console table
    const sideY = 1.04;
    const tableMat = new THREE.MeshStandardMaterial({
      map: this.walnutTex,
      color: 0x8d5935,
      roughness: 0.48,
      metalness: 0.04
    });
    const shelf = new THREE.Mesh(new THREE.BoxGeometry(1.75, 0.075, 0.85), tableMat);
    shelf.position.set(2.96, sideY - 0.0375, -1.25);
    shelf.castShadow = true;
    shelf.receiveShadow = true;
    this.scene.add(shelf);

    // -----------------------------------------------------------------------
    // Iconic Frosted Glass Mushroom Lamp (Click to toggle on/off!)
    // -----------------------------------------------------------------------
    this.lampGroup = new THREE.Group();
    this.lampGroup.position.set(2.92, sideY, -1.16);
    this.scene.add(this.lampGroup);

    this.lampGlassMat = new THREE.MeshStandardMaterial({
      color: 0xfff0d8,
      emissive: new THREE.Color(0xffb562),
      emissiveIntensity: 0.72,
      roughness: 0.34,
      metalness: 0.04
    });
    // Tapered stem base
    const stem = new THREE.Mesh(new THREE.CylinderGeometry(0.085, 0.135, 0.34, 24), this.lampGlassMat);
    stem.position.y = 0.17;
    stem.castShadow = true;
    this.lampGroup.add(stem);

    // Soft mushroom dome shade
    const dome = new THREE.Mesh(
      new THREE.SphereGeometry(0.285, 28, 18, 0, Math.PI * 2, 0, Math.PI * 0.56),
      this.lampGlassMat
    );
    dome.scale.set(1.0, 0.78, 1.0);
    dome.position.y = 0.31;
    dome.castShadow = true;
    this.lampGroup.add(dome);

    const underCap = new THREE.Mesh(
      new THREE.CircleGeometry(0.275, 24),
      new THREE.MeshBasicMaterial({ color: 0xffd79e })
    );
    underCap.rotation.x = Math.PI / 2;
    underCap.position.y = 0.295;
    this.lampGroup.add(underCap);
    this.lampUnderCap = underCap;

    this._registerDraggable(this.lampGroup, { kind: 'lampToggle', target: this.lampGroup });

    // Stacked art books & delicate ceramic vase with twig on side table (Draggable!)
    this.sideBooks = new THREE.Group();
    this.sideBooks.position.set(2.42, sideY, -1.16);
    this.scene.add(this.sideBooks);

    const bCols = [0x6e3729, 0x2c3b36, 0xc7b99e];
    for (let i = 0; i < bCols.length; i++) {
      const book = new THREE.Mesh(
        new THREE.BoxGeometry(0.46 - i * 0.04, 0.044, 0.34 - i * 0.02),
        new THREE.MeshStandardMaterial({ color: bCols[i], roughness: 0.75 })
      );
      book.position.set(0, 0.022 + i * 0.044, 0);
      book.rotation.y = (i - 1) * 0.05;
      book.castShadow = true;
      book.receiveShadow = true;
      this.sideBooks.add(book);
    }

    // Small ceramic bud vase + botanical sprig sitting on the books
    const vase = new THREE.Mesh(
      new THREE.SphereGeometry(0.068, 16, 14),
      new THREE.MeshStandardMaterial({ color: 0x9c8974, roughness: 0.68 })
    );
    vase.scale.set(1.0, 1.25, 1.0);
    vase.position.set(0.02, 0.21, 0.0);
    vase.castShadow = true;
    this.sideBooks.add(vase);

    const twig = new THREE.Mesh(
      new THREE.CylinderGeometry(0.004, 0.006, 0.34, 5),
      new THREE.MeshStandardMaterial({ color: 0x3b3127, roughness: 0.9 })
    );
    twig.position.set(-0.04, 0.38, 0);
    twig.rotation.z = 0.42;
    this.sideBooks.add(twig);

    this._registerDraggable(this.sideBooks, {
      kind: 'sideDrag',
      target: this.sideBooks,
      minX: 2.18,
      maxX: 2.62
    });

    // Second smaller book stack on the far right
    const rightBooks = new THREE.Group();
    rightBooks.position.set(3.38, sideY, -1.18);
    this.scene.add(rightBooks);
    for (let i = 0; i < 2; i++) {
      const bk = new THREE.Mesh(
        new THREE.BoxGeometry(0.36 - i * 0.03, 0.042, 0.28),
        new THREE.MeshStandardMaterial({ color: i === 0 ? 0x2b333a : 0x7b563a, roughness: 0.78 })
      );
      bk.position.y = 0.021 + i * 0.042;
      bk.castShadow = true;
      rightBooks.add(bk);
    }
  };

  StudioGlassEngine.prototype._buildVolumetricShaftsAndDust = function () {
    const THREE = global.THREE;

    // Volumetric light beam planes angled diagonally from left window across studio
    this.shaftMat = new THREE.ShaderMaterial({
      vertexShader: SHAFT_VERT,
      fragmentShader: SHAFT_FRAG,
      transparent: true,
      depthWrite: false,
      blending: THREE.AdditiveBlending,
      side: THREE.DoubleSide,
      uniforms: {
        uColor: { value: new THREE.Color(1.0, 0.9, 0.72) },
        uAlpha: { value: 0.24 },
        uBlinds: { value: 0.70 },
        uTime: { value: 0 }
      }
    });

    this.shaftGroup = new THREE.Group();
    this.shaftGroup.position.set(-1.35, 2.25, -0.35);
    this.shaftGroup.rotation.set(0.05, 0.24, -0.36);
    this.scene.add(this.shaftGroup);

    const beamGeo = new THREE.PlaneGeometry(5.6, 2.45);
    for (let i = 0; i < 3; i++) {
      const mesh = new THREE.Mesh(beamGeo, this.shaftMat);
      mesh.position.set(i * 0.16, -i * 0.12, (i - 1) * 0.42);
      this.shaftGroup.add(mesh);
    }

    // Floating 3D sunlit dust motes
    const rng = makeRng(7123);
    const count = 90;
    const pos = new Float32Array(count * 3);
    this.dustMeta = [];
    for (let i = 0; i < count; i++) {
      const x = -2.4 + rng() * 5.2;
      const y = 1.05 + rng() * 2.6;
      const z = -1.3 + rng() * 2.2;
      pos[i * 3 + 0] = x;
      pos[i * 3 + 1] = y;
      pos[i * 3 + 2] = z;
      this.dustMeta.push({ x, y, z, ph: rng() * Math.PI * 2, sp: 0.35 + rng() * 0.65 });
    }
    const dustGeo = new THREE.BufferGeometry();
    dustGeo.setAttribute('position', new THREE.BufferAttribute(pos, 3));
    this.dustMat = new THREE.PointsMaterial({
      color: 0xffe9c4,
      size: 0.032,
      transparent: true,
      opacity: 0.42,
      depthWrite: false,
      blending: THREE.AdditiveBlending
    });
    this.dustPoints = new THREE.Points(dustGeo, this.dustMat);
    this.scene.add(this.dustPoints);
  };

  StudioGlassEngine.prototype._initPostProcess = function () {
    const THREE = global.THREE;

    const useHalfFloat = !!(
      this.renderer.capabilities.isWebGL2 &&
      (this.renderer.extensions.has('EXT_color_buffer_float') || this.renderer.extensions.has('EXT_color_buffer_half_float'))
    );
    const rtOpts = {
      minFilter: THREE.LinearFilter,
      magFilter: THREE.LinearFilter,
      format: THREE.RGBAFormat,
      type: useHalfFloat ? THREE.HalfFloatType : THREE.UnsignedByteType,
      depthBuffer: true,
      stencilBuffer: false
    };
    this.sceneRT = new THREE.WebGLRenderTarget(640, 360, rtOpts);
    this.blurRT1 = new THREE.WebGLRenderTarget(320, 180, { ...rtOpts, depthBuffer: false });
    this.blurRT2 = new THREE.WebGLRenderTarget(320, 180, { ...rtOpts, depthBuffer: false });

    this.postCam = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
    const quadGeo = new THREE.PlaneGeometry(2, 2);

    this.blurMat = new THREE.ShaderMaterial({
      vertexShader: POST_VERT,
      fragmentShader: POST_BLUR_FRAG,
      depthTest: false,
      depthWrite: false,
      uniforms: {
        uTex: { value: null },
        uTexel: { value: new THREE.Vector2(1 / 320, 1 / 180) },
        uDir: { value: new THREE.Vector2(1, 0) },
        uRadius: { value: 8.0 },
        uTaps: { value: 8 }
      }
    });
    this.blurScene = new THREE.Scene();
    this.blurScene.add(new THREE.Mesh(quadGeo, this.blurMat));

    this.glassRects = [];
    this.glassMeta = [];
    for (let i = 0; i < MAX_GLASS; i++) {
      this.glassRects.push(new THREE.Vector4(0, 0, 0, 0));
      this.glassMeta.push(new THREE.Vector4(14, 0, 0, 0));
    }

    this.compMat = new THREE.ShaderMaterial({
      vertexShader: POST_VERT,
      fragmentShader: POST_GLASS_FRAG,
      depthTest: false,
      depthWrite: false,
      uniforms: {
        uScene: { value: this.sceneRT.texture },
        uBlur: { value: this.blurRT2.texture },
        uRes: { value: new THREE.Vector2(1280, 720) },
        uCount: { value: 0 },
        uRects: { value: this.glassRects },
        uMeta: { value: this.glassMeta },
        uRefraction: { value: 0.85 },
        uFrost: { value: 0.50 },
        uRimTint: { value: new THREE.Vector3(1.0, 0.93, 0.82) },
        uLightPos: { value: new THREE.Vector2(0.14, 0.25) },
        uPulseIdx: { value: -1 },
        uPulseAge: { value: 99.0 },
        uPulseColA: { value: new THREE.Vector3(1.0, 0.78, 0.42) },
        uPulseColB: { value: new THREE.Vector3(1.0, 0.46, 0.74) },
        uPulseColC: { value: new THREE.Vector3(0.45, 0.88, 1.0) },
        uTime: { value: 0.0 },
        uExposure: { value: 1.12 }
      }
    });
    this.compScene = new THREE.Scene();
    this.compScene.add(new THREE.Mesh(quadGeo, this.compMat));
  };

  StudioGlassEngine.prototype._registerDraggable = function (obj, meta) {
    obj.traverse(ch => {
      if (ch.isMesh) {
        ch.userData.dragMeta = meta;
        this.draggables.push(ch);
      }
    });
  };

  StudioGlassEngine.prototype.setLowMode = function (v) {
    this.lowMode = !!v;
    this.renderer.shadowMap.enabled = !this.lowMode;
  };

  StudioGlassEngine.prototype.applySettings = function (params) {
    const prevBlinds = this.P.blinds;
    const prevMon = this.P.monitorAngle;
    const prevPreset = this.P.preset;
    Object.assign(this.P, params || {});
    if (Math.abs(this.P.blinds - prevBlinds) > 0.001) this.interactiveBlinds = null;
    if (Math.abs(this.P.monitorAngle - prevMon) > 0.05) this.interactiveMonitor = null;
    if (this.P.preset !== prevPreset) {
      this.interactivePreset = null;
      this.lampManual = null;
      this.interactiveBrightness = this.presetBrightness[this._targetPresetIndex()];
    }
  };

  StudioGlassEngine.prototype.setIcons = function (list) {
    this.icons = Array.isArray(list) ? list : [];
  };

  StudioGlassEngine.prototype.setHudCards = function (cards) {
    this.hudCards = Array.isArray(cards) ? cards : [];
  };

  StudioGlassEngine.prototype.resize = function (w, h, scale) {
    this.W = Math.max(240, w || 1280);
    this.H = Math.max(160, h || 720);
    this.scale = scale || 1;
    const pw = Math.max(2, Math.round(this.W * this.scale));
    const ph = Math.max(2, Math.round(this.H * this.scale));

    this.renderer.setSize(pw, ph, false);
    this.camera.aspect = this.W / this.H;
    this.camera.updateProjectionMatrix();

    this.sceneRT.setSize(pw, ph);
    const div = this.lowMode ? 3 : 2;
    const bw = Math.max(2, Math.ceil(pw / div));
    const bh = Math.max(2, Math.ceil(ph / div));
    this.blurRT1.setSize(bw, bh);
    this.blurRT2.setSize(bw, bh);
    this.blurMat.uniforms.uTexel.value.set(1 / bw, 1 / bh);
    this.compMat.uniforms.uRes.value.set(this.W, this.H);
  };

  // =========================================================================
  // Direct 3D Pointer Raycasting (Move keyboard, mouse, chair, plants, blinds, monitor, lamp!)
  // =========================================================================
  StudioGlassEngine.prototype._updateRay = function (x, y) {
    this.ptr.px = x;
    this.ptr.py = y;
    this.ptr.tx = clamp(x / this.W, 0, 1);
    this.ptr.ty = clamp(y / this.H, 0, 1);
    this.ptr.ndcX = (x / this.W) * 2 - 1;
    this.ptr.ndcY = -(y / this.H) * 2 + 1;
    this.ptr.active = true;
    this.raycaster.setFromCamera({ x: this.ptr.ndcX, y: this.ptr.ndcY }, this.camera);
  };

  StudioGlassEngine.prototype.onPointerDown = function (x, y) {
    const THREE = global.THREE;
    this._updateRay(x, y);
    const hits = this.raycaster.intersectObjects(this.draggables, false);
    if (!hits.length) {
      this.dragItem = null;
      return false;
    }

    const hit = hits[0];
    const meta = hit.object.userData.dragMeta;
    if (!meta) return false;

    this.dragItem = meta;
    this.dragStartX = x;
    this.dragStartY = y;
    this.dragMoved = false;

    if (meta.kind === 'blinds') {
      this.dragStartVal = this.interactiveBlinds !== null ? this.interactiveBlinds : this.P.blinds;
    } else if (meta.kind === 'monitor') {
      this.dragStartVal = this.interactiveMonitor !== null ? this.interactiveMonitor : this.P.monitorAngle;
    } else if (meta.kind === 'deskDrag') {
      this.dragPlane.setFromNormalAndCoplanarPoint(new THREE.Vector3(0, 1, 0), new THREE.Vector3(0, this.deskY, 0));
      if (this.raycaster.ray.intersectPlane(this.dragPlane, this._hitPt)) {
        this.dragOffset.copy(meta.target.position).sub(this._hitPt);
      }
    } else if (meta.kind === 'chairDrag') {
      this.dragPlane.setFromNormalAndCoplanarPoint(new THREE.Vector3(0, 1, 0), new THREE.Vector3(0, 0.75, 0));
      if (this.raycaster.ray.intersectPlane(this.dragPlane, this._hitPt)) {
        this.dragOffset.copy(meta.target.position).sub(this._hitPt);
      }
      this.dragStartVal = meta.target.rotation.y;
    } else if (meta.kind === 'sillSlide') {
      this.dragStartVal = meta.target.position.x;
      this.plantSway[meta.plantIdx].vel += 2.6;
    } else if (meta.kind === 'sideDrag') {
      this.dragStartVal = meta.target.position.x;
    }
    return true;
  };

  StudioGlassEngine.prototype.onPointerMove = function (x, y, pressed) {
    this._updateRay(x, y);
    if (!pressed || !this.dragItem) {
      // Hovering over windowsill plants gently rustles them
      if (!pressed && !this.lowMode) {
        const hits = this.raycaster.intersectObjects(this.draggables, false);
        if (hits.length && hits[0].object.userData.dragMeta) {
          const m = hits[0].object.userData.dragMeta;
          if (m.kind === 'sillSlide') {
            this.plantSway[m.plantIdx].vel += 0.32;
          }
          this.cv.style.cursor = m.kind === 'lampToggle' ? 'pointer' : 'grab';
        } else {
          this.cv.style.cursor = 'default';
        }
      }
      return;
    }

    const dx = x - this.dragStartX;
    const dy = y - this.dragStartY;
    if (Math.hypot(dx, dy) > 4) this.dragMoved = true;
    this.cv.style.cursor = 'grabbing';

    const meta = this.dragItem;
    if (meta.kind === 'blinds') {
      this.interactiveBlinds = clamp(this.dragStartVal - dy / Math.max(180, this.H * 0.45), 0, 1);
    } else if (meta.kind === 'monitor') {
      this.interactiveMonitor = clamp(this.dragStartVal + (dx / Math.max(160, this.W * 0.25)) * 45, -35, 35);
    } else if (meta.kind === 'deskDrag') {
      if (this.raycaster.ray.intersectPlane(this.dragPlane, this._hitPt)) {
        meta.target.position.x = clamp(this._hitPt.x + this.dragOffset.x, meta.minX, meta.maxX);
        meta.target.position.z = clamp(this._hitPt.z + this.dragOffset.z, meta.minZ, meta.maxZ);
      }
    } else if (meta.kind === 'chairDrag') {
      if (this.raycaster.ray.intersectPlane(this.dragPlane, this._hitPt)) {
        const nextX = clamp(this._hitPt.x + this.dragOffset.x, meta.minX, meta.maxX);
        const velX = nextX - meta.target.position.x;
        meta.target.position.x = nextX;
        meta.target.rotation.y = clamp(meta.target.rotation.y + velX * 0.35, -0.55, 0.95);
      }
    } else if (meta.kind === 'sillSlide') {
      const nextX = clamp(this.dragStartVal + dx * 0.005, meta.minX, meta.maxX);
      const delta = nextX - meta.target.position.x;
      meta.target.position.x = nextX;
      this.plantSway[meta.plantIdx].vel -= delta * 18.0;
    } else if (meta.kind === 'sideDrag') {
      meta.target.position.x = clamp(this.dragStartVal + dx * 0.004, meta.minX, meta.maxX);
    }
  };

  StudioGlassEngine.prototype.onPointerUp = function () {
    if (this.dragItem && this.dragItem.kind === 'lampToggle' && !this.dragMoved) {
      const autoOn = this.mood.lampInt > 0.6;
      this.lampManual = this.lampManual === null ? !autoOn : !this.lampManual;
    }
    this.dragItem = null;
    this.cv.style.cursor = 'default';
  };

  StudioGlassEngine.prototype.onPointerLeave = function () {
    this.dragItem = null;
    this.ptr.active = false;
    this.ptr.tx = 0.5;
    this.ptr.ty = 0.5;
    this.cv.style.cursor = 'default';
  };

  // =========================================================================
  // Simulation Step
  // =========================================================================
  StudioGlassEngine.prototype.step = function (dt) {
    const THREE = global.THREE;
    this.time += dt;

    const target = PRESETS[this._targetPresetIndex()];
    const k = 1 - Math.exp(-dt * 3.4);
    const m = this.mood;

    m.sunColor.lerp(new THREE.Color(target.sunColor), k);
    m.sunInt = lerp(m.sunInt, target.sunInt, k);
    m.sunPos.lerp(new THREE.Vector3(target.sunPos[0], target.sunPos[1], target.sunPos[2]), k);
    m.hemiSky.lerp(new THREE.Color(target.hemiSky), k);
    m.hemiGround.lerp(new THREE.Color(target.hemiGround), k);
    m.hemiInt = lerp(m.hemiInt, target.hemiInt, k);
    m.ambColor.lerp(new THREE.Color(target.ambColor), k);
    m.ambInt = lerp(m.ambInt, target.ambInt, k);
    m.lampInt = lerp(m.lampInt, target.lampInt, k);
    m.lampGlow = lerp(m.lampGlow, target.lampGlow, k);
    m.screenInt = lerp(m.screenInt, target.screenInt, k);
    m.skyTop.lerp(new THREE.Color(target.skyTop[0] / 255, target.skyTop[1] / 255, target.skyTop[2] / 255), k);
    m.skyMid.lerp(new THREE.Color(target.skyMid[0] / 255, target.skyMid[1] / 255, target.skyMid[2] / 255), k);
    m.skyBot.lerp(new THREE.Color(target.skyBot[0] / 255, target.skyBot[1] / 255, target.skyBot[2] / 255), k);
    m.shaftCol.lerp(new THREE.Color(target.shaftCol[0], target.shaftCol[1], target.shaftCol[2]), k);
    m.shaftAlpha = lerp(m.shaftAlpha, target.shaftAlpha, k);
    m.rain = lerp(m.rain, target.rain, k);
    m.thunder = lerp(m.thunder, target.thunder, k);
    m.dust = lerp(m.dust, target.dust, k);
    m.stars = lerp(m.stars, target.stars, k);
    m.rimTint.lerp(new THREE.Vector3(target.rimTint[0], target.rimTint[1], target.rimTint[2]), k);
    m.exposure = lerp(m.exposure, target.exposure, k);
    m.wallTint.lerp(new THREE.Color(target.wallTint), k);
    m.nicheTint.lerp(new THREE.Color(target.nicheTint), k);

    // Advance prismatic click pulse timer
    if (this.pulseAge < 10.0) {
      this.pulseAge += dt;
    }

    // Occasional distant lightning flash in Thunderstorm preset
    if (m.thunder > 0.35 && this.P.weather > 0.1) {
      this.nextFlash -= dt;
      if (this.nextFlash <= 0) {
        this.flash = 0.85 + Math.random() * 0.35;
        this.nextFlash = 4.5 + Math.random() * 6.5;
      }
    }
    this.flash = Math.max(0, this.flash - dt * 3.8);

    // Smooth blinds & monitor swivel
    const sk = 1 - Math.exp(-dt * 8.0);
    const goalBlinds = this.interactiveBlinds !== null ? this.interactiveBlinds : this.P.blinds;
    const goalMon = this.interactiveMonitor !== null ? this.interactiveMonitor : this.P.monitorAngle;
    this.curBlinds = lerp(this.curBlinds, goalBlinds, sk);
    this.curMonitor = lerp(this.curMonitor, goalMon, sk);

    // Update 3D venetian blind slats (vertical pull + slat tilt!)
    const maxDrop = 2.28;
    const dropH = maxDrop * (1.0 - this.curBlinds * 0.72);
    const slatTilt = lerp(1.32, 0.24, this.curBlinds);
    const nSlats = this.blindSlats.length;
    for (let i = 0; i < nSlats; i++) {
      const frac = (i + 1) / nSlats;
      const slat = this.blindSlats[i];
      slat.position.y = -frac * dropH;
      slat.rotation.x = slatTilt;
    }
    this.blindBottomRail.position.y = -dropH - 0.03;

    // Monitor swivel
    this.monitorHead.rotation.y = (this.curMonitor * Math.PI) / 180;

    // Spring leaf sway physics on both windowsill plants
    const plants = [this.plantLeft, this.plantRight];
    for (let i = 0; i < 2; i++) {
      const s = this.plantSway[i];
      const breeze = Math.sin(this.time * (1.2 + i * 0.35) + i * 2.0) * 0.018 * (0.4 + 0.6 * this.curBlinds);
      const force = -(s.ang - breeze) * 24.0 - s.vel * 4.5;
      s.vel += force * dt;
      s.ang += s.vel * dt;
      const crown = plants[i] && plants[i].userData.crown;
      if (crown) {
        crown.rotation.z = clamp(s.ang, -0.35, 0.35);
        crown.rotation.x = clamp(s.ang * 0.45, -0.2, 0.2);
      }
    }

    // Gentle camera parallax
    const pk = 1 - Math.exp(-dt * 4.8);
    this.ptr.x = lerp(this.ptr.x, this.ptr.tx, pk);
    this.ptr.y = lerp(this.ptr.y, this.ptr.ty, pk);
    const par = this.P.parallax * (this.lowMode ? 0.35 : 1.0);
    this.camera.position.x = this.baseCamPos.x + (this.ptr.x - 0.5) * 0.42 * par;
    this.camera.position.y = this.baseCamPos.y - (this.ptr.y - 0.5) * 0.22 * par;
    this.camera.lookAt(this.baseCamTarget);

    // Apply interpolated lighting & brightness multiplier (bright & airy at 100%, cozy at low %)
    const bNorm = this.interactiveBrightness;
    const bright = 0.46 + bNorm * 1.18;
    const sunMul = (0.34 + 0.86 * this.curBlinds) * bright;
    this.sunLight.color.copy(m.sunColor);
    this.sunLight.intensity = m.sunInt * sunMul + this.flash * 4.5;
    this.sunLight.position.copy(m.sunPos);

    this.hemiLight.color.copy(m.hemiSky);
    this.hemiLight.groundColor.copy(m.hemiGround);
    this.hemiLight.intensity = m.hemiInt * bright;

    this.ambLight.color.copy(m.ambColor);
    this.ambLight.intensity = m.ambInt * bright;
    if (this.deskFillLight) {
      this.deskFillLight.color.copy(m.hemiSky);
      this.deskFillLight.intensity = 0.52 * bright * (0.50 + 0.50 * this.curBlinds);
    }

    const lampEnabled = this.lampManual === null ? true : this.lampManual;
    const lampMul = lampEnabled ? this.P.deskLamp * (0.58 + 0.62 * bright) : 0;
    this.lampLight.intensity = m.lampInt * lampMul;
    this.lampGlassMat.emissiveIntensity = m.lampGlow * lampMul;
    this.lampUnderCap.visible = lampMul > 0.04;

    this.screenLight.intensity = m.screenInt * (0.65 + 0.35 * bright);
    this.monitorScreenMat.emissiveIntensity = 0.45 + 0.36 * bright;
    this.renderer.toneMappingExposure = m.exposure * (0.76 + 0.62 * bNorm);

    // Update outdoor window shader
    const wu = this.windowPaneMat.uniforms;
    wu.uTime.value = this.time;
    wu.uRain.value = m.rain;
    wu.uWeather.value = this.P.weather;
    wu.uSkyTop.value.copy(m.skyTop);
    wu.uSkyMid.value.copy(m.skyMid);
    wu.uSkyBot.value.copy(m.skyBot);
    wu.uStars.value = m.stars;
    wu.uFlash.value = this.flash;

    // Update volumetric window shafts & dust motes
    const su = this.shaftMat.uniforms;
    su.uTime.value = this.time;
    su.uColor.value.copy(m.shaftCol);
    su.uBlinds.value = this.curBlinds;
    su.uAlpha.value = m.shaftAlpha * this.P.shafts * (0.16 + 0.96 * this.curBlinds) * bright;

    this.dustMat.color.copy(m.shaftCol);
    this.dustMat.opacity = clamp(m.dust * this.P.weather * this.P.shafts * (0.2 + 0.8 * this.curBlinds) * 0.55, 0, 0.72);
    if (this.dustMat.opacity > 0.01) {
      const attr = this.dustPoints.geometry.getAttribute('position');
      const arr = attr.array;
      for (let i = 0; i < this.dustMeta.length; i++) {
        const d = this.dustMeta[i];
        arr[i * 3 + 0] = d.x + Math.sin(this.time * 0.32 * d.sp + d.ph) * 0.16;
        arr[i * 3 + 1] = d.y + Math.cos(this.time * 0.25 * d.sp + d.ph) * 0.12;
      }
      attr.needsUpdate = true;
    }
  };

  // =========================================================================
  // Multi-Pass Render (3D Three.js Scene -> Frost Blur -> Refractive Glass)
  // =========================================================================
  StudioGlassEngine.prototype._syncGlassUniforms = function () {
    let idx = 0;

    // 1. Optional Studio Lighting HUD glass cards (captured from live DOM rects)
    if (this.P.showHud !== 0 && this.hudCards.length) {
      for (let i = 0; i < this.hudCards.length && idx < MAX_GLASS; i++) {
        const c = this.hudCards[i];
        if (!c || c.w <= 2 || c.h <= 2) continue;
        this.glassRects[idx].set(c.x + c.w * 0.5, c.y + c.h * 0.5, c.w * 0.5, c.h * 0.5);
        this.glassMeta[idx].set(c.r || 18, c.style || 0, c.active ? 1.0 : 0.0, c.fill || 0.0);
        idx++;
      }
    }

    // 2. Adaptive desktop icon refractive glass plaques
    const iconStyle = Math.round(this.P.iconGlass || 0);
    if (this.P.adaptive !== 0 && iconStyle !== 2 && this.icons.length) {
      for (let i = 0; i < this.icons.length && idx < MAX_GLASS; i++) {
        const ic = this.icons[i];
        const hw = Math.max(22, ic.w * 0.5) + 6;
        const hh = Math.max(26, ic.h * 0.5) + 4;
        const rad = iconStyle === 0 ? Math.min(hw, hh) * 0.42 : 12;
        this.glassRects[idx].set(ic.x + ic.w * 0.5, ic.y + ic.h * 0.5, hw, hh);
        this.glassMeta[idx].set(rad, iconStyle === 1 ? 1.0 : 0.0, 0.15, 0.0);
        idx++;
      }
    }

    const cu = this.compMat.uniforms;
    cu.uCount.value = idx;
    cu.uRefraction.value = this.P.refraction;
    cu.uFrost.value = this.P.frost;
    cu.uRimTint.value.copy(this.mood.rimTint);
    cu.uTime.value = this.time;
    cu.uExposure.value = this.renderer.toneMappingExposure;
    cu.uPulseIdx.value = this.pulseCardIdx;
    cu.uPulseAge.value = this.pulseAge;

    // Curated 3-color iridescent spectrum per lighting preset
    const PULSE_PALETTES = [
      [[1.00, 0.80, 0.42], [1.00, 0.48, 0.76], [0.42, 0.88, 1.00]], // Morning: gold -> rose -> cyan
      [[0.34, 0.78, 1.00], [0.66, 0.52, 1.00], [0.90, 0.97, 1.00]], // Thunderstorm: electric cyan -> violet -> ice
      [[1.00, 0.95, 0.76], [0.46, 0.84, 1.00], [0.40, 0.92, 0.82]], // Afternoon: sunlight -> sky -> mint
      [[1.00, 0.54, 0.24], [0.98, 0.42, 0.68], [1.00, 0.86, 0.36]], // Sunset: amber -> coral pink -> gold
      [[0.50, 0.56, 1.00], [0.76, 0.50, 1.00], [0.30, 0.82, 0.96]]  // Night: moon indigo -> orchid -> aqua
    ];
    const pal = PULSE_PALETTES[this.pulsePreset] || PULSE_PALETTES[0];
    cu.uPulseColA.value.set(pal[0][0], pal[0][1], pal[0][2]);
    cu.uPulseColB.value.set(pal[1][0], pal[1][1], pal[1][2]);
    cu.uPulseColC.value.set(pal[2][0], pal[2][1], pal[2][2]);
    return idx;
  };

  StudioGlassEngine.prototype.draw = function () {
    const glCount = this._syncGlassUniforms();

    // Fast path when no glass plaques/HUD are visible
    if (glCount === 0) {
      this.renderer.setRenderTarget(null);
      this.renderer.render(this.scene, this.camera);
      return;
    }

    // Pass 1: Render 3D Three.js Studio into offscreen RenderTarget
    this.renderer.setRenderTarget(this.sceneRT);
    this.renderer.render(this.scene, this.camera);

    // Pass 2 & 3: Separable Gaussian Frost Blur
    const taps = this.lowMode ? 5 : ([6, 9, 13][this.P.quality] || 9);
    const radius = 1.6 + this.P.frost * (this.lowMode ? 6.5 : 13.5);
    this.blurMat.uniforms.uTaps.value = taps;
    this.blurMat.uniforms.uRadius.value = radius;

    this.blurMat.uniforms.uTex.value = this.sceneRT.texture;
    this.blurMat.uniforms.uDir.value.set(1, 0);
    this.renderer.setRenderTarget(this.blurRT1);
    this.renderer.render(this.blurScene, this.postCam);

    this.blurMat.uniforms.uTex.value = this.blurRT1.texture;
    this.blurMat.uniforms.uDir.value.set(0, 1);
    this.renderer.setRenderTarget(this.blurRT2);
    this.renderer.render(this.blurScene, this.postCam);

    // Pass 4: Composite Refractive Glass Cards & Icon Plaques over 3D Studio
    this.renderer.setRenderTarget(null);
    this.renderer.render(this.compScene, this.postCam);
  };

  StudioGlassEngine.PRESETS = PRESETS;
  global.StudioGlassEngine = StudioGlassEngine;
})(window);
