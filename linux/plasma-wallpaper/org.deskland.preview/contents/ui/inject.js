/* Deskland wallpaper bridge.
 *
 * Injected into the theme page at document creation. Gives the page the
 * `window.chrome.webview` shim it expects (same shape as the desktop host)
 * and feeds it two message types:
 *
 *   {t:"icons", vis:true, r:[[l,t,r,b], ...]}   physical pixels
 *   {t:"settings", v:{...}}                     theme parameters
 *
 * Icon positions come from ~/.cache/deskland/icons.json written by the
 * `org.deskland.iconspy` plasmoid running inside the desktop containment
 * (real pixel rectangles `[l, t, r, b]`, refreshed every 500 ms with a
 * timestamp `ts`). When the file is absent, unreadable, or older than 10 s,
 * we fall back to `vis: false` (island-free scene).
 */
(function () {
  'use strict';

  var QS = new URLSearchParams(location.search);
  var HOME = decodeURIComponent(QS.get('home') || '/');
  var CURRENT_THEME = QS.get('theme') || 'papercut';
  var ICONS_JSON = HOME + '/.cache/deskland/icons.json';
  var SETTINGS = HOME + '/.config/deskland/settings.json';
  var PAUSE_JSON = HOME + '/.cache/deskland/pause.json';
  var ICONS_TTL_MS = 90000;

  var listeners = [];
  var lastIcons = null;
  var lastSettings = null;
  var hostPaused = false;
  var filePaused = false;
  var lastPauseSent = null;
  var lastLowModeSent = null;
  var disableAutoPause = false;
  window.__desklandLowModeState = false;
  window.__desklandDisableAutoPause = false;
  try {
    if (HOME && HOME !== '/') {
      var initXhr = new XMLHttpRequest();
      initXhr.open('GET', 'file://' + SETTINGS, false);
      initXhr.send(null);
      if (initXhr.responseText) {
        var initCfg = JSON.parse(initXhr.responseText);
        if (initCfg && typeof initCfg.lowMode === 'boolean') {
          window.__desklandInitialLowMode = initCfg.lowMode;
          window.__desklandLowModeState = initCfg.lowMode;
          lastLowModeSent = initCfg.lowMode;
        }
        if (initCfg && typeof initCfg.disableAutoPause === 'boolean') {
          disableAutoPause = initCfg.disableAutoPause;
          window.__desklandDisableAutoPause = disableAutoPause;
        }
      }
    }
  } catch (e) {}

  window.chrome = window.chrome || {};
  function replayCachedTo(fn) {
    if (lastLowModeSent !== null) {
      try { fn({ data: JSON.stringify({ t: 'lowMode', v: lastLowModeSent }) }); } catch (e) {}
    }
    if (lastSettings) {
      try { fn({ data: lastSettings }); } catch (e) {}
    }
    if (lastIcons) {
      try { fn({ data: lastIcons }); } catch (e) {}
    }
    if (lastPauseSent === true) {
      try { fn({ data: JSON.stringify({ t: 'pause', v: true }) }); } catch (e) {}
    }
  }
  var pendingMoveIcons = '';
  window.__desklandTakeMoveIcons = function () {
    var out = pendingMoveIcons;
    pendingMoveIcons = '';
    return out;
  };
  window.chrome.webview = {
    /* Replay latest icons/settings when the page signals ready so heavy pages never miss initial polls. */
    postMessage: function (m) {
      if (m === 'ready') {
        if (lastLowModeSent !== null) window.__desklandDeliver(JSON.stringify({ t: 'lowMode', v: lastLowModeSent }));
        if (lastSettings) window.__desklandDeliver(lastSettings);
        if (lastIcons) window.__desklandDeliver(lastIcons);
        return;
      }
      try {
        var obj = typeof m === 'string' ? JSON.parse(m) : m;
        if (obj && (obj.t === 'moveIcons' || obj.type === 'moveIcons')) {
          pendingMoveIcons = typeof m === 'string' ? m : JSON.stringify(obj);
        }
      } catch (e) {}
    },
    addEventListener: function (type, fn) {
      if (type === 'message' && typeof fn === 'function') {
        listeners.push(fn);
        setTimeout(function () { replayCachedTo(fn); }, 0);
      }
    },
  };
  window.__desklandDeliver = function (json) {
    for (var i = 0; i < listeners.length; i++) {
      try { listeners[i]({ data: json }); } catch (e) { /* keep others alive */ }
    }
  };

  /* ---------------- icons (from org.deskland.iconspy) ---------------- */

  function deliverIcons(vis, rects, names) {
    var payload = { t: 'icons', vis: !!vis, r: rects || [] };
    if (Array.isArray(names) && names.length) payload.n = names;
    var msg = JSON.stringify(payload);
    window.__desklandLastIcons = msg;   /* debug aid */
    if (msg !== lastIcons) {
      lastIcons = msg;
      window.__desklandDeliver(msg);
    }
  }

  function pollIcons() {
    var xhr = new XMLHttpRequest();
    try { xhr.open('GET', 'file://' + ICONS_JSON, true); } catch (e) { deliverIcons(false, []); return; }
    xhr.onreadystatechange = function () {
      if (xhr.readyState !== 4) return;
      var data = null;
      try {
        if (xhr.responseText) data = JSON.parse(xhr.responseText);
      } catch (e) {
        data = null;
      }
      if (data && data.vis === true && Array.isArray(data.r) && typeof data.ts === 'number') {
        var age = Date.now() - data.ts;
        if (age <= ICONS_TTL_MS && age >= -5000) {
          deliverIcons(true, data.r, Array.isArray(data.n) ? data.n : []);
          return;
        }
      }
      deliverIcons(false, [], []);
    };
    try { xhr.send(); } catch (e) { deliverIcons(false, []); }
  }

  /* ---------------- settings ---------------- */

  var THEME_PAGES = {
    beach: 'flat.html',
    meadow: 'meadow.html',
    aurora: 'aurora.html',
    glade: 'glade.html',
    rift: 'rift.html',
    strike: 'strike.html',
    pocket: 'pocket.html',
    roach: 'roach.html',
    rosebrook: 'rosebrook.html',
    polarnight: 'polarnight.html',
    desert: 'desert.html',
    'zen-archipelago': 'zen-archipelago.html',
    istanbul: 'istanbul.html',
    'solar-system': 'solar-system.html',
    winter: 'winter.html',
    neumorph: 'neumorph.html',
    claymorph: 'claymorph.html',
    neobrutalism: 'neobrutalism.html',
    voxelcraft: 'voxelcraft.html',
    liquidglass: 'liquidglass.html',
    'software-world': 'software-world.html',
    'thinking-orbs': 'thinking-orbs.html',
    papercut: 'papercut.html',
    quietsnow: 'quietsnow.html',
    'studio-glass': 'studio-glass.html',
    'material-impulse': 'material-impulse.html',
    'material-impulse-gl': 'material-impulse-gl.html',
  };

  var CUSTOM_DIR = HOME + '/.config/deskland/themes';
  var PROTO_BASE = (function () {
    var p = QS.get('proto');
    if (p) return decodeURIComponent(p);
    var cur = location.href.split('?')[0];
    if (cur.indexOf('/.config/deskland/themes/') === -1) return cur.replace(/[^/]+$/, '');
    return 'file:///usr/lib/deskland/prototype/';
  })();

  function navigateToThemeUrl(baseUrl, themeId) {
    location.replace(
      baseUrl
      + '?wallpaper=1&vicons=0&v=' + Date.now()
      + '&theme=' + encodeURIComponent(themeId)
      + '&home=' + encodeURIComponent(HOME)
      + '&proto=' + encodeURIComponent(PROTO_BASE)
    );
  }

  function tryCustomTheme(themeId) {
    var xhr = new XMLHttpRequest();
    try { xhr.open('GET', 'file://' + CUSTOM_DIR + '/registry.json', true); } catch (e) { return; }
    xhr.onreadystatechange = function () {
      if (xhr.readyState !== 4) return;
      var reg;
      try { reg = JSON.parse(xhr.responseText); } catch (e) { return; }
      var list = (reg && Array.isArray(reg.themes)) ? reg.themes : [];
      for (var i = 0; i < list.length; i++) {
        var item = list[i];
        if (item && item.id === themeId && item.entry) {
          navigateToThemeUrl('file://' + CUSTOM_DIR + '/' + encodeURIComponent(themeId) + '/' + item.entry, themeId);
          return;
        }
      }
    };
    try { xhr.send(); } catch (e) {}
  }

  function tryBuiltinOrCustomTheme(themeId) {
    var xhr = new XMLHttpRequest();
    try { xhr.open('GET', PROTO_BASE + 'themes.json', true); } catch (e) { tryCustomTheme(themeId); return; }
    xhr.onreadystatechange = function () {
      if (xhr.readyState !== 4) return;
      try {
        var reg = JSON.parse(xhr.responseText);
        var list = (reg && Array.isArray(reg.themes)) ? reg.themes : [];
        for (var i = 0; i < list.length; i++) {
          var item = list[i];
          if (item && item.id === themeId && item.page && item.status === 'ready') {
            navigateToThemeUrl(PROTO_BASE + item.page, themeId);
            return;
          }
        }
      } catch (e) {}
      tryCustomTheme(themeId);
    };
    try { xhr.send(); } catch (e) { tryCustomTheme(themeId); }
  }

  function pollSettings() {
    var xhr = new XMLHttpRequest();
    try { xhr.open('GET', 'file://' + SETTINGS, true); } catch (e) { return; }
    xhr.onreadystatechange = function () {
      if (xhr.readyState !== 4) return;
      var data;
      try { data = JSON.parse(xhr.responseText); } catch (e) { return; }
      if (!data) return;
      var lowMode = !!data.lowMode;
      window.__desklandLowModeState = lowMode;
      window.__desklandRustOpenGl = !!data.rustOpenGl;
      if (lowMode !== lastLowModeSent) {
        lastLowModeSent = lowMode;
        window.__desklandDeliver(JSON.stringify({ t: 'lowMode', v: lowMode }));
      }
      var nextDisableAutoPause = !!data.disableAutoPause;
      if (nextDisableAutoPause !== disableAutoPause) {
        disableAutoPause = nextDisableAutoPause;
        window.__desklandDisableAutoPause = disableAutoPause;
        syncPause();
      } else {
        window.__desklandDisableAutoPause = nextDisableAutoPause;
      }
      var theme = data.theme || 'meadow';
      window.__desklandThemeId = theme;
      var page = THEME_PAGES[theme];
      if (theme !== CURRENT_THEME) {
        if (page) {
          navigateToThemeUrl(PROTO_BASE + page, theme);
        } else {
          tryBuiltinOrCustomTheme(theme);
        }
        return;
      }
      var params = (data.params && data.params[theme]) || {};
      var resolved = {};
      for (var k in params) {
        if (Object.prototype.hasOwnProperty.call(params, k)) resolved[k] = params[k];
      }
      if (typeof resolved.audioFile === 'string') {
        resolved.audioSrc = resolved.audioFile
          ? 'file://' + HOME + '/.config/deskland/audio/' + encodeURIComponent(resolved.audioFile)
          : '';
      }
      var msg = JSON.stringify({ t: 'settings', v: resolved });
      window.__desklandLastSettings = msg;   /* debug aid */
      if (msg !== lastSettings) {
        lastSettings = msg;
        window.__desklandDeliver(msg);
      }
    };
    try { xhr.send(); } catch (e) { /* keep polling */ }
  }

  function syncPause() {
    var effective = !!((hostPaused && !disableAutoPause) || filePaused);
    if (effective !== lastPauseSent) {
      lastPauseSent = effective;
      window.__desklandDeliver(JSON.stringify({ t: 'pause', v: effective }));
    }
  }

  function pollPause() {
    var xhr = new XMLHttpRequest();
    try { xhr.open('GET', 'file://' + PAUSE_JSON, true); } catch (e) { return; }
    xhr.onreadystatechange = function () {
      if (xhr.readyState !== 4) return;
      var next = false;
      try {
        var parsed = xhr.responseText ? JSON.parse(xhr.responseText) : null;
        next = !!(parsed && parsed.paused);
      } catch (e) {
        next = false;
      }
      if (next !== filePaused) {
        filePaused = next;
        syncPause();
      }
    };
    try { xhr.send(); } catch (e) {}
  }

  window.__desklandSetHostPaused = function (v) {
    hostPaused = !!v;
    syncPause();
  };

  window.__desklandForcePoll = function () {
    pollIcons();
    pollSettings();
    pollPause();
  };

  var downPos = null;
  window.__desklandDispatchPointer = function (kind, x, y) {
    if (typeof window.roachHostPointer === 'function') {
      window.roachHostPointer(kind, x, y);
      return;
    }
    if (kind === 'leave') {
      downPos = null;
      if (typeof PointerEvent === 'function') {
        var leaveEvt = new PointerEvent('pointerleave', {
          bubbles: false,
          cancelable: true,
          pointerId: 1,
          pointerType: 'mouse',
          isPrimary: true
        });
        try { window.dispatchEvent(leaveEvt); } catch (e) {}
        if (document.documentElement) {
          try { document.documentElement.dispatchEvent(new PointerEvent('pointerleave', { bubbles: false, pointerId: 1, pointerType: 'mouse', isPrimary: true })); } catch (e) {}
        }
      }
      return;
    }
    var evtType = kind === 'down' ? 'pointerdown'
      : ((kind === 'move' || kind === 'hover') ? 'pointermove'
      : (kind === 'up' || kind === 'cancel' ? 'pointerup' : ''));
    if (!evtType || typeof PointerEvent !== 'function') return;
    var cx = Number(x) || 0;
    var cy = Number(y) || 0;
    if (kind === 'down') downPos = { x: cx, y: cy };
    var target = document.elementFromPoint(cx, cy) || document.body || document.documentElement;
    if (!target || typeof target.dispatchEvent !== 'function') return;
    target.dispatchEvent(new PointerEvent(evtType, {
      bubbles: true,
      cancelable: true,
      pointerId: 1,
      pointerType: 'mouse',
      isPrimary: true,
      clientX: cx,
      clientY: cy,
      button: evtType === 'pointermove' ? -1 : 0,
      buttons: (evtType === 'pointerup' || kind === 'hover') ? 0 : 1
    }));
    if (kind === 'up' && downPos) {
      var moved = Math.hypot(cx - downPos.x, cy - downPos.y);
      downPos = null;
      if (moved < 8) {
        target.dispatchEvent(new MouseEvent('click', {
          bubbles: true,
          cancelable: true,
          clientX: cx,
          clientY: cy,
          button: 0
        }));
      }
    } else if (kind === 'cancel') {
      downPos = null;
    }
  };

  setInterval(pollIcons, 600);
  setInterval(pollSettings, 1200);
  setInterval(pollPause, 1500);
  pollIcons();
  pollSettings();
  pollPause();
})();
