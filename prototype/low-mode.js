(function () {
  'use strict';
  if (window.__desklandLowModeInstalled) return;
  window.__desklandLowModeInstalled = true;

  var qsLow = false;
  try {
    qsLow = new URLSearchParams(window.location.search || '').get('low') === '1';
  } catch (e) {}

  var lowMode = !!(window.__desklandInitialLowMode || qsLow);
  var isPreview = false;
  try {
    isPreview = new URLSearchParams(window.location.search || '').get('preview') === '1';
  } catch (e) {}
  var MIN_FRAME_MS = 66; // ~15 FPS strict ceiling in Low Mode
  var REDUCED_MOTION_MS = 83; // ~12 FPS when OS Reduce Motion is enabled
  var IDLE_WINDOW_MS = 3000;
  var LOW_DPR_CAP = 0.75;
  var adaptiveFps = !isPreview;
  var onBattery = false;
  var hostReducedMotion = false;
  var cssReducedMotion = false;
  var customMaxFps = 0;
  var lastActivityAt = typeof performance !== 'undefined' ? performance.now() : 0;
  var lastIconsSig = '';
  var currentEnv = { hour: 12, phase: 'day', daylight: 1, darkMode: false, solarSync: false };

  function applySolarFilter(env) {
    var root = document.documentElement;
    if (!root) return;
    if (!env || !env.solarSync) {
      root.style.removeProperty('filter');
      return;
    }
    var dl = typeof env.daylight === 'number' ? Math.max(0, Math.min(1, env.daylight)) : 1;
    var bright = (0.76 + dl * 0.24).toFixed(3);
    var sat = (0.90 + dl * 0.12).toFixed(3);
    var sepia = (env.phase === 'dawn' || env.phase === 'dusk') ? '0.10' : '0';
    root.style.filter = 'brightness(' + bright + ') saturate(' + sat + ')' + (sepia !== '0' ? ' sepia(' + sepia + ')' : '');
  }

  try {
    if (typeof window.matchMedia === 'function') {
      var rmMq = window.matchMedia('(prefers-reduced-motion: reduce)');
      cssReducedMotion = !!rmMq.matches;
      var onRm = function (ev) { cssReducedMotion = !!ev.matches; };
      if (typeof rmMq.addEventListener === 'function') rmMq.addEventListener('change', onRm);
      else if (typeof rmMq.addListener === 'function') rmMq.addListener(onRm);
    }
  } catch (e) {}

  function markActivity() {
    if (typeof performance !== 'undefined') lastActivityAt = performance.now();
  }

  function currentTargetFrameMs(now) {
    if (window.__desklandRecording) return 0;
    var target = 0;
    if (lowMode) {
      target = MIN_FRAME_MS;
    } else if (hostReducedMotion || cssReducedMotion) {
      target = REDUCED_MOTION_MS;
    } else if (adaptiveFps) {
      var active = (now - lastActivityAt) < IDLE_WINDOW_MS;
      if (onBattery) target = active ? 33 : 55; // ~30 FPS active / ~18 FPS idle on battery
      else target = active ? 20 : 50; // ~50 FPS active / ~20 FPS idle on AC
    }
    if (customMaxFps > 0) {
      var customMs = Math.floor(1000 / customMaxFps);
      if (customMs > target) target = customMs;
    }
    return target;
  }

  var origRAF = typeof window.requestAnimationFrame === 'function'
    ? window.requestAnimationFrame.bind(window)
    : null;
  var origCAF = typeof window.cancelAnimationFrame === 'function'
    ? window.cancelAnimationFrame.bind(window)
    : null;
  var lastFrameAt = 0;
  var nextHandle = 100000000;
  var handles = Object.create(null);
  var styleInjected = false;

  function ensureLowModeStyle() {
    if (styleInjected) return;
    var root = document.documentElement;
    if (!root) return;
    styleInjected = true;
    root.classList.toggle('dk-low-mode', lowMode);
    var style = document.createElement('style');
    style.id = 'dk-low-mode-style';
    style.textContent =
      'html.dk-low-mode *, html.dk-low-mode *::before, html.dk-low-mode *::after {' +
      '  backdrop-filter: none !important;' +
      '  -webkit-backdrop-filter: none !important;' +
      '}';
    (document.head || root).appendChild(style);
  }

  function syncDomClass() {
    var root = document.documentElement;
    if (root) {
      ensureLowModeStyle();
      root.classList.toggle('dk-low-mode', lowMode);
    } else {
      document.addEventListener('DOMContentLoaded', ensureLowModeStyle, { once: true });
    }
  }

  // Intercept heavy Canvas2D Gaussian shadowBlur & filter blur while Low Mode is active
  if (typeof CanvasRenderingContext2D !== 'undefined') {
    try {
      var proto = CanvasRenderingContext2D.prototype;
      var shadowDesc = Object.getOwnPropertyDescriptor(proto, 'shadowBlur');
      if (shadowDesc && shadowDesc.configurable && shadowDesc.set) {
        Object.defineProperty(proto, 'shadowBlur', {
          configurable: true,
          enumerable: shadowDesc.enumerable,
          get: shadowDesc.get,
          set: function (v) {
            shadowDesc.set.call(this, lowMode ? 0 : v);
          }
        });
      }
      var filterDesc = Object.getOwnPropertyDescriptor(proto, 'filter');
      if (filterDesc && filterDesc.configurable && filterDesc.set) {
        Object.defineProperty(proto, 'filter', {
          configurable: true,
          enumerable: filterDesc.enumerable,
          get: filterDesc.get,
          set: function (v) {
            if (lowMode && typeof v === 'string' && v.indexOf('blur') !== -1) {
              filterDesc.set.call(this, 'none');
              return;
            }
            filterDesc.set.call(this, v);
          }
        });
      }
      var smoothDesc = Object.getOwnPropertyDescriptor(proto, 'imageSmoothingQuality');
      if (smoothDesc && smoothDesc.configurable && smoothDesc.set) {
        Object.defineProperty(proto, 'imageSmoothingQuality', {
          configurable: true,
          enumerable: smoothDesc.enumerable,
          get: smoothDesc.get,
          set: function (v) {
            smoothDesc.set.call(this, lowMode ? 'low' : v);
          }
        });
      }
    } catch (e) {}
  }

  // Timer-backed rAF gate: genuinely lets the WebView/compositor sleep between frames
  if (origRAF && origCAF) {
    window.requestAnimationFrame = function (cb) {
      if (typeof cb !== 'function') {
        return origRAF(cb);
      }
      var now = performance.now();
      var frameMs = currentTargetFrameMs(now);
      if (frameMs <= 0) {
        return origRAF(cb);
      }
      var id = nextHandle++;
      var wait = Math.max(0, frameMs - (now - lastFrameAt));
      var entry = { canceled: false, timer: 0, raf: 0 };
      handles[id] = entry;
      var dispatch = function () {
        if (entry.canceled) {
          delete handles[id];
          return;
        }
        entry.raf = origRAF(function (ts) {
          delete handles[id];
          if (entry.canceled) return;
          lastFrameAt = performance.now();
          cb(ts);
        });
      };
      // Wake slightly before the target VSync so origRAF catches the target tick cleanly
      // instead of missing it by 1-2ms of setTimeout jitter.
      var timerDelay = Math.max(0, Math.round(wait - 8));
      if (timerDelay <= 4) {
        dispatch();
      } else {
        entry.timer = window.setTimeout(dispatch, timerDelay);
      }
      return id;
    };

    window.cancelAnimationFrame = function (id) {
      var entry = handles[id];
      if (entry) {
        entry.canceled = true;
        if (entry.timer) window.clearTimeout(entry.timer);
        if (entry.raf) origCAF(entry.raf);
        delete handles[id];
        return;
      }
      origCAF(id);
    };
  }

  function applyLowMode(enabled) {
    var next = !!enabled;
    var changed = next !== lowMode;
    lowMode = next;
    syncDomClass();
    if (changed) {
      try {
        window.dispatchEvent(new CustomEvent('deskland:lowmode', { detail: { lowMode: lowMode } }));
      } catch (e) {}
      try {
        window.dispatchEvent(new Event('resize'));
      } catch (e) {}
    }
  }

  function handleMsg(ev) {
    var m = ev && ev.data !== undefined ? ev.data : ev;
    if (typeof m === 'string') {
      if (m.charAt(0) !== '{') return;
      try { m = JSON.parse(m); } catch (e) { return; }
    }
    if (!m || typeof m !== 'object') return;
    if (m.t === 'lowMode') {
      applyLowMode(m.v);
    } else if (m.t === 'perf') {
      if (typeof m.adaptiveFps === 'boolean') adaptiveFps = m.adaptiveFps;
      if (typeof m.onBattery === 'boolean') onBattery = m.onBattery;
      if (typeof m.reducedMotion === 'boolean') hostReducedMotion = m.reducedMotion;
      if (typeof m.maxFps === 'number' && isFinite(m.maxFps) && m.maxFps >= 0) customMaxFps = m.maxFps;
      markActivity();
    } else if (m.t === 'activity') {
      markActivity();
    } else if (m.t === 'icons') {
      var sig = JSON.stringify(m.r || []);
      if (sig !== lastIconsSig) {
        lastIconsSig = sig;
        markActivity();
      }
    } else if (m.t === 'qualityProfile') {
      var qp = String(m.v || '').toLowerCase();
      if (qp === 'eco') customMaxFps = 15;
      else if (qp === 'balanced') customMaxFps = 30;
      else if (qp === 'high') customMaxFps = 60;
      else customMaxFps = 0;
    } else if (m.t === 'systemStats') {
      window.__desklandSystemStats = m;
      try { window.dispatchEvent(new CustomEvent('deskland:systemStats', { detail: m })); } catch (e) {}
    } else if (m.t === 'audioLevels') {
      window.__desklandAudioLevels = m;
      try { window.dispatchEvent(new CustomEvent('deskland:audioLevels', { detail: m })); } catch (e) {}
    } else if (m.t === 'windows') {
      window.__desklandWindows = Array.isArray(m.r) ? m.r : [];
      try { window.dispatchEvent(new CustomEvent('deskland:windows', { detail: m })); } catch (e) {}
    } else if (m.t === 'workspace') {
      window.__desklandWorkspace = m;
      try { window.dispatchEvent(new CustomEvent('deskland:workspace', { detail: m })); } catch (e) {}
    } else if (m.t === 'environment') {
      currentEnv = {
        hour: typeof m.hour === 'number' ? m.hour : currentEnv.hour,
        phase: typeof m.phase === 'string' ? m.phase : currentEnv.phase,
        daylight: typeof m.daylight === 'number' ? m.daylight : currentEnv.daylight,
        darkMode: !!m.darkMode,
        solarSync: !!m.solarSync
      };
      window.__desklandEnvironment = currentEnv;
      applySolarFilter(currentEnv);
      try {
        window.dispatchEvent(new CustomEvent('deskland:environment', { detail: currentEnv }));
      } catch (e) {}
    } else if (m.t === 'settings' || m.t === 'init') {
      var v = m.v || m.params || m;
      if (v && typeof v.lowMode === 'boolean') {
        applyLowMode(v.lowMode);
      }
      markActivity();
    }
  }

  try {
    window.addEventListener('pointermove', markActivity, { passive: true });
    window.addEventListener('pointerdown', markActivity, { passive: true });
  } catch (e) {}

  Object.defineProperty(window, '__desklandLowMode', {
    configurable: true,
    enumerable: true,
    get: function () { return lowMode; }
  });

  window.__desklandSetLowMode = applyLowMode;

  window.__desklandScale = function (normalMax, previewScale) {
    if (typeof previewScale === 'number' && previewScale > 0) {
      var qs = window.location.search || '';
      if (qs.indexOf('preview=1') !== -1) return previewScale;
    }
    var nativeDpr = window.devicePixelRatio || 1;
    var maxCap = lowMode ? LOW_DPR_CAP : (typeof normalMax === 'number' && normalMax > 0 ? normalMax : 1.5);
    return Math.min(nativeDpr, maxCap);
  };

  window.addEventListener('message', handleMsg, false);
  if (window.chrome && window.chrome.webview && typeof window.chrome.webview.addEventListener === 'function') {
    try { window.chrome.webview.addEventListener('message', handleMsg); } catch (e) {}
  }
  syncDomClass();
})();
