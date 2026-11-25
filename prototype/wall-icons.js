// DiyarDesk · Shared interactive virtual icons bar for built-in wallpaper themes.
// Loads in <head> before each theme's startHostedMode() so window.chrome.webview is
// wrapped transparently without modifying individual theme renderers.
(function () {
  'use strict';

  const qs = new URLSearchParams(location.search);
  if (!qs.has('wallpaper') || qs.has('preview') || qs.get('vicons') === '0' || window.self !== window.top) {
    return;
  }

  const LS_ICONS = 'deskland.wallIcons.v1';
  const LS_COLLAPSED = 'deskland.wallIcons.collapsed.v1';
  const LS_MODE = 'deskland.wallIcons.mode.v1';

  const PRESETS = [
    ['Yeni Klasör', 'folder'],
    ['Notlar.txt', 'txt'],
    ['Belgeler', 'folder'],
    ['Fatura.pdf', 'pdf'],
    ['Tablo.xlsx', 'xlsx'],
    ['Ekran Görüntüsü.png', 'png'],
    ['Arşiv.zip', 'zip'],
    ['Projeler', 'folder'],
    ['Hatırlatma.txt', 'txt'],
    ['Yedekler', 'folder'],
  ];

  const ICONS_SVG = {
    folder: '<rect x="5" y="13" width="38" height="26" rx="4" fill="#d99220"/><path d="M5 13a4 4 0 0 1 4-4h9l3 4h18a4 4 0 0 1 4 4v2H5z" fill="#e8a730"/><rect x="5" y="17" width="38" height="22" rx="4" fill="#ffc94d"/><rect x="8" y="19" width="32" height="2" rx="1" fill="#fff" opacity=".35"/>',
    txt: '<path d="M10 5h20l8 8v30H10z" fill="#f7f9fc"/><path d="M30 5v8h8z" fill="#d3dbe5"/><path d="M15 20h18M15 26h18M15 32h12" stroke="#7d8da1" stroke-width="2.2" stroke-linecap="round"/>',
    pdf: '<path d="M10 5h20l8 8v30H10z" fill="#fff5f5"/><path d="M30 5v8h8z" fill="#f2c4c4"/><rect x="8" y="21" width="24" height="12" rx="3" fill="#d9382c"/><path d="M13 30v-6h3c1.6 0 2.5.8 2.5 2s-.9 2-2.5 2h-1.5v2H13zm8-6h2.2c2 0 3.2 1.2 3.2 3s-1.2 3-3.2 3H21v-6z" fill="#fff"/>',
    xlsx: '<path d="M10 5h20l8 8v30H10z" fill="#f4fbf6"/><path d="M30 5v8h8z" fill="#c2e4cc"/><rect x="8" y="20" width="22" height="14" rx="3" fill="#1e7e46"/><path d="M14 23l5 8m0-8l-5 8" stroke="#fff" stroke-width="2" stroke-linecap="round"/>',
    png: '<rect x="6" y="8" width="36" height="32" rx="4" fill="#f7f9fc"/><rect x="9" y="11" width="30" height="24" rx="2" fill="#8ad0f2"/><circle cx="30" cy="18" r="3.4" fill="#ffd247"/><path d="M9 35l9-12 7 9 5-6 9 9z" fill="#46a85a"/>',
    zip: '<path d="M10 6h28v36H10z" rx="4" fill="#8c64d8"/><rect x="10" y="6" width="28" height="36" rx="4" fill="#7a4ec7"/><path d="M22 8h4v4h-4zm0 6h4v4h-4zm0 6h4v4h-4z" fill="#f5edff"/><rect x="20" y="26" width="8" height="8" rx="2" fill="#ffd247"/>',
  };

  let realHost = null;
  const pageListeners = [];
  let hostIcons = { vis: false, r: [], n: [] };
  let addedIcons = [];
  let barMode = 'auto';        // 'auto' | 'always' | 'off'
  let userCollapsed = null;    // null (auto) | true | false
  let uiHidden = false;
  let hostReadyReceived = false;
  let presetSeq = 0;
  let saveTimer = 0;
  let commitRaf = 0;
  let domReady = false;
  let lastUiRectSig = '';

  const layerEl = () => document.getElementById('dk-wall-ui-layer');
  const barEl = () => document.getElementById('dk-ctrl-bar');

  function esc(s) {
    return String(s).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  }

  function sanitizeIcon(raw) {
    if (!raw || typeof raw !== 'object') return null;
    const x = Number(raw.x), y = Number(raw.y);
    const w = Number(raw.w) || 76, h = Number(raw.h) || 86;
    if (!Number.isFinite(x) || !Number.isFinite(y)) return null;
    const type = ICONS_SVG[raw.type] ? raw.type : 'folder';
    const name = typeof raw.name === 'string' && raw.name.trim() ? raw.name.trim().slice(0, 48) : 'Yeni Klasör';
    return { name, type, x: Math.round(x), y: Math.round(y), w: Math.round(w), h: Math.round(h) };
  }

  function loadLocalFallback() {
    try {
      const savedIcons = JSON.parse(localStorage.getItem(LS_ICONS) || 'null');
      if (Array.isArray(savedIcons)) {
        addedIcons = savedIcons.map(sanitizeIcon).filter(Boolean);
        presetSeq = addedIcons.length;
      }
      const savedCollapsed = localStorage.getItem(LS_COLLAPSED);
      if (savedCollapsed === '1') userCollapsed = true;
      else if (savedCollapsed === '0') userCollapsed = false;
      const savedMode = localStorage.getItem(LS_MODE);
      if (savedMode === 'auto' || savedMode === 'always' || savedMode === 'off') barMode = savedMode;
    } catch (_) {}
  }

  function postToRealHost(payload) {
    if (!realHost || typeof realHost.postMessage !== 'function') return;
    realHost.postMessage(typeof payload === 'string' ? payload : JSON.stringify(payload));
  }

  function persistIconsNow() {
    const clean = addedIcons.map(({ name, type, x, y, w, h }) => ({ name, type, x, y, w, h }));
    try { localStorage.setItem(LS_ICONS, JSON.stringify(clean)); } catch (_) {}
    postToRealHost({ t: 'saveVirtualIcons', v: clean });
  }

  function schedulePersistIcons() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(persistIconsNow, 220);
  }

  function persistCollapsed(val) {
    try {
      if (val === null) localStorage.removeItem(LS_COLLAPSED);
      else localStorage.setItem(LS_COLLAPSED, val ? '1' : '0');
    } catch (_) {}
    postToRealHost({ t: 'saveVirtualBarCollapsed', v: val });
  }

  function hasRealHostIcons() {
    return !!(hostIcons.vis && Array.isArray(hostIcons.r) && hostIcons.r.length > 0);
  }

  function effectiveEnabled() {
    return barMode !== 'off';
  }

  function effectiveCollapsed() {
    if (userCollapsed !== null) return !!userCollapsed;
    return hasRealHostIcons() && addedIcons.length === 0;
  }

  function dispatchMergedIcons() {
    const k = window.devicePixelRatio || 1;
    const base = hostIcons.vis && Array.isArray(hostIcons.r) ? hostIcons.r : [];
    const baseNames = base.map((_, i) =>
      Array.isArray(hostIcons.n) && hostIcons.n[i] ? String(hostIcons.n[i]) : ''
    );
    const virt = effectiveEnabled()
      ? addedIcons.map(ic => [
          Math.round(ic.x * k),
          Math.round(ic.y * k),
          Math.round((ic.x + ic.w) * k),
          Math.round((ic.y + ic.h) * k),
        ])
      : [];
    const virtNames = effectiveEnabled() ? addedIcons.map(ic => String(ic.name || '')) : [];
    const merged = base.concat(virt);
    const mergedNames = baseNames.concat(virtNames);
    const obj = { t: 'icons', vis: merged.length > 0, r: merged };
    if (mergedNames.some(Boolean)) obj.n = mergedNames;
    const payload = JSON.stringify(obj);
    const ev = { data: payload, source: window, origin: location.origin };
    pageListeners.slice().forEach(fn => {
      try { fn(ev); } catch (err) { console.error('wall-icons listener:', err); }
    });
  }

  function scheduleMergedDispatch() {
    if (commitRaf) return;
    commitRaf = requestAnimationFrame(() => {
      commitRaf = 0;
      dispatchMergedIcons();
    });
  }

  function reportBarRect() {
    const bar = barEl();
    const layer = layerEl();
    if (!bar) return;
    let rect = null;
    const rects = [];
    if (bar.style.display !== 'none') {
      const r = bar.getBoundingClientRect();
      if (r.width > 0 && r.height > 0) {
        rect = [Math.floor(r.left), Math.floor(r.top), Math.ceil(r.width), Math.ceil(r.height)];
        rects.push(rect);
      }
      if (layer && layer.style.display !== 'none') {
        const els = layer.querySelectorAll('.dk-wall-icon');
        for (let i = 0; i < els.length; i++) {
          const ir = els[i].getBoundingClientRect();
          if (ir.width > 0 && ir.height > 0) {
            rects.push([Math.floor(ir.left), Math.floor(ir.top), Math.ceil(ir.width), Math.ceil(ir.height)]);
          }
        }
      }
    }
    const sig = JSON.stringify(rects);
    if (sig === lastUiRectSig) return;
    lastUiRectSig = sig;
    postToRealHost({ t: 'uiRect', r: rect });
    postToRealHost({ t: 'uiRects', src: 'vicons', r: rects });
  }

  function syncBarUi() {
    const bar = barEl();
    const layer = layerEl();
    if (!bar || !layer) return;
    const enabled = effectiveEnabled();
    bar.style.display = enabled ? 'flex' : 'none';
    layer.style.display = enabled && !uiHidden ? 'block' : 'none';
    if (enabled) {
      const collapsed = effectiveCollapsed();
      bar.classList.toggle('collapsed', collapsed);
      const btn = document.getElementById('dk-btn-collapse');
      if (btn) {
        btn.textContent = collapsed ? '›' : '‹';
        btn.setAttribute('aria-expanded', String(!collapsed));
        btn.title = collapsed ? 'Genişlet' : 'Daralt';
      }
      const hideBtn = document.getElementById('dk-btn-hide');
      if (hideBtn) hideBtn.classList.toggle('active', uiHidden);
    }
    requestAnimationFrame(reportBarRect);
    try { window.dispatchEvent(new CustomEvent('deskland:vicons-layout')); } catch (_) {}
  }

  function clampPos(x, y, w, h) {
    const vw = window.innerWidth || 1280;
    const vh = window.innerHeight || 720;
    return {
      x: Math.round(Math.max(12, Math.min(Math.max(12, vw - w - 12), x))),
      y: Math.round(Math.max(12, Math.min(Math.max(12, vh - h - 72), y))),
    };
  }

  function inferIconType(name, fallback) {
    const low = String(name || '').trim().toLowerCase();
    if (low.endsWith('.pdf')) return 'pdf';
    if (/\.(xlsx|xls|csv|ods)$/.test(low)) return 'xlsx';
    if (/\.(png|jpe?g|webp|gif|svg|bmp)$/.test(low)) return 'png';
    if (/\.(zip|tar|gz|xz|7z|rar|zst)$/.test(low)) return 'zip';
    if (/\.(txt|md|docx?|rtf|odt|log)$/.test(low)) return 'txt';
    return ICONS_SVG[fallback] ? fallback : 'folder';
  }

  function startInlineRename(el, item) {
    const labelEl = el.querySelector('.dk-label');
    if (!labelEl || el.querySelector('.dk-rename-input')) return;
    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'dk-rename-input';
    input.maxLength = 48;
    input.value = item.name;
    labelEl.replaceWith(input);
    input.focus();
    input.select();

    let done = false;
    const finish = commit => {
      if (done) return;
      done = true;
      if (commit) {
        const next = input.value.trim().slice(0, 48);
        if (next) {
          item.name = next;
          if (next.includes('.')) item.type = inferIconType(next, item.type);
        }
      }
      renderIconsDom();
      syncBarUi();
      dispatchMergedIcons();
      if (commit) persistIconsNow();
    };

    input.addEventListener('pointerdown', ev => ev.stopPropagation());
    input.addEventListener('dblclick', ev => ev.stopPropagation());
    input.addEventListener('keydown', ev => {
      ev.stopPropagation();
      if (ev.key === 'Enter') { ev.preventDefault(); finish(true); }
      else if (ev.key === 'Escape') { ev.preventDefault(); finish(false); }
    });
    input.addEventListener('blur', () => finish(true));
  }

  function renderIconsDom() {
    const layer = layerEl();
    if (!layer) return;
    layer.innerHTML = '';
    addedIcons.forEach(item => {
      const p = clampPos(item.x, item.y, item.w, item.h);
      item.x = p.x; item.y = p.y;
      const el = document.createElement('div');
      el.className = 'dk-wall-icon';
      el.title = 'Sağ tık: Yeniden adlandır · Çift tık: Kaldır';
      el.style.left = item.x + 'px';
      el.style.top = item.y + 'px';
      el.innerHTML = `<div class="dk-glyph"><svg viewBox="0 0 48 48">${ICONS_SVG[item.type] || ICONS_SVG.folder}</svg></div><div class="dk-label">${esc(item.name)}</div>`;
      let dragging = false, ox = 0, oy = 0, moved = false;
      el.addEventListener('pointerdown', e => {
        if (e.button !== 0 || e.target.closest('.dk-rename-input')) return;
        e.stopPropagation();
        dragging = true;
        moved = false;
        ox = e.clientX - item.x;
        oy = e.clientY - item.y;
        el.setPointerCapture(e.pointerId);
      });
      el.addEventListener('pointermove', e => {
        if (!dragging) return;
        const np = clampPos(e.clientX - ox, e.clientY - oy, item.w, item.h);
        if (!moved && Math.hypot(np.x - item.x, np.y - item.y) < 3) return;
        moved = true;
        item.x = np.x; item.y = np.y;
        el.style.left = item.x + 'px';
        el.style.top = item.y + 'px';
        scheduleMergedDispatch();
      });
      const finishDrag = e => {
        if (!dragging) return;
        dragging = false;
        if (moved) {
          e.stopPropagation();
          scheduleMergedDispatch();
          schedulePersistIcons();
          requestAnimationFrame(reportBarRect);
        }
      };
      el.addEventListener('pointerup', finishDrag);
      el.addEventListener('pointercancel', finishDrag);
      el.addEventListener('contextmenu', e => {
        e.preventDefault();
        e.stopPropagation();
        startInlineRename(el, item);
      });
      el.addEventListener('dblclick', e => {
        if (e.target.closest('.dk-rename-input')) return;
        e.stopPropagation();
        addedIcons = addedIcons.filter(x => x !== item);
        renderIconsDom();
        syncBarUi();
        dispatchMergedIcons();
        persistIconsNow();
      });
      layer.appendChild(el);
    });
  }

  function nextTurkishName() {
    const [base, type] = PRESETS[presetSeq++ % PRESETS.length];
    const used = new Set(addedIcons.map(i => i.name));
    if (!used.has(base)) return [base, type];
    let n = 2;
    while (used.has(`${base} (${n})`)) n++;
    return [`${base} (${n})`, type];
  }

  function handleRealHostEvent(ev) {
    let msg = null;
    try { msg = typeof ev.data === 'string' ? JSON.parse(ev.data) : ev.data; } catch (_) {}
    if (!msg || typeof msg !== 'object') {
      pageListeners.slice().forEach(fn => { try { fn(ev); } catch (_) {} });
      return;
    }
    if (msg.t === 'icons') {
      hostIcons = {
        vis: !!msg.vis,
        r: Array.isArray(msg.r) ? msg.r : [],
        n: Array.isArray(msg.n) ? msg.n : [],
      };
      if (domReady) syncBarUi();
      dispatchMergedIcons();
      return;
    }
    if (msg.t === 'virtualState') {
      if (Array.isArray(msg.icons)) {
        addedIcons = msg.icons.map(sanitizeIcon).filter(Boolean);
        presetSeq = addedIcons.length;
        try { localStorage.setItem(LS_ICONS, JSON.stringify(addedIcons)); } catch (_) {}
      }
      if (typeof msg.collapsed === 'boolean' || msg.collapsed === null) {
        userCollapsed = msg.collapsed;
        try {
          if (userCollapsed === null) localStorage.removeItem(LS_COLLAPSED);
          else localStorage.setItem(LS_COLLAPSED, userCollapsed ? '1' : '0');
        } catch (_) {}
      }
      if (msg.mode === 'auto' || msg.mode === 'always' || msg.mode === 'off') {
        barMode = msg.mode;
        try { localStorage.setItem(LS_MODE, barMode); } catch (_) {}
      }
      if (domReady) {
        renderIconsDom();
        syncBarUi();
      }
      if (hostReadyReceived) dispatchMergedIcons();
      return;
    }
    pageListeners.slice().forEach(fn => {
      try { fn(ev); } catch (err) { console.error('wall-icons forward:', err); }
    });
  }

  function installWebViewProxy() {
    realHost = window.chrome && window.chrome.webview ? window.chrome.webview : null;
    if (realHost && typeof realHost.addEventListener === 'function') {
      realHost.addEventListener('message', handleRealHostEvent);
    }
    const proxy = {
      addEventListener(type, fn) {
        if (type === 'message' && typeof fn === 'function' && pageListeners.indexOf(fn) < 0) {
          pageListeners.push(fn);
        }
      },
      removeEventListener(type, fn) {
        if (type !== 'message') return;
        const idx = pageListeners.indexOf(fn);
        if (idx >= 0) pageListeners.splice(idx, 1);
      },
      postMessage(data) {
        if (data === 'ready') {
          hostReadyReceived = true;
          if (!realHost) {
            setTimeout(() => dispatchMergedIcons(), 40);
            return;
          }
        }
        postToRealHost(data);
      },
    };
    try { window.chrome = window.chrome || {}; } catch (_) {}
    try {
      Object.defineProperty(window.chrome, 'webview', {
        configurable: true, enumerable: true, writable: false, value: proxy,
      });
    } catch (_) {
      try { window.chrome.webview = proxy; } catch (_) {}
    }
  }

  function mountDom() {
    if (domReady || !document.body) return;
    domReady = true;

    const style = document.createElement('style');
    style.textContent = `
      #dk-wall-ui-layer{position:fixed;inset:0;pointer-events:none;z-index:2147482900;overflow:hidden}
      .dk-wall-icon{position:fixed;width:76px;height:86px;border-radius:10px;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:5px;cursor:grab;pointer-events:auto;user-select:none;color:#fff;font:11px/1.25 "Segoe UI","Noto Sans",system-ui,sans-serif;text-shadow:0 1px 4px rgba(0,0,0,.92),0 0 2px #000}
      .dk-wall-icon:hover{background:rgba(255,255,255,.14)}
      .dk-wall-icon:active{cursor:grabbing}
      .dk-wall-icon .dk-glyph{width:46px;height:46px;filter:drop-shadow(0 3px 6px rgba(0,0,0,.55))}
      .dk-wall-icon .dk-glyph svg{width:100%;height:100%;display:block}
      .dk-wall-icon .dk-label{max-width:72px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;text-align:center}
      .dk-wall-icon .dk-rename-input{width:74px;padding:1px 4px;border-radius:4px;border:1px solid rgba(130,210,255,.78);background:rgba(12,22,32,.94);color:#fff;font:11px/1.25 "Segoe UI","Noto Sans",system-ui,sans-serif;text-align:center;outline:none;box-sizing:border-box}
      #dk-ctrl-bar{position:fixed;right:14px;bottom:64px;z-index:2147483000;display:flex;gap:8px;align-items:center;background:rgba(14,26,34,.84);backdrop-filter:blur(12px);-webkit-backdrop-filter:blur(12px);border:1px solid rgba(180,225,255,.22);padding:7px 11px;border-radius:10px;color:#eaf4fb;font:12px/1.2 "Segoe UI","Noto Sans",system-ui,sans-serif;pointer-events:auto;box-shadow:0 8px 24px rgba(0,0,0,.4)}
      #dk-ctrl-bar.collapsed{gap:6px;padding:6px 10px}
      #dk-ctrl-bar.collapsed .dk-ctrl-item{display:none}
      #dk-ctrl-bar .dk-ctrl-toggle{padding:3px 7px;font-weight:700;font-size:13px;line-height:1;min-width:24px;display:inline-flex;align-items:center;justify-content:center}
      #dk-ctrl-bar .dk-ctrl-title{white-space:nowrap;user-select:none}
      #dk-ctrl-bar button{background:rgba(255,255,255,.10);color:#eef6fc;border:1px solid rgba(255,255,255,.18);border-radius:6px;padding:4px 8px;font:11px/1.2 inherit;cursor:pointer;outline:none}
      #dk-ctrl-bar button:hover{background:rgba(130,210,255,.24)}
      #dk-ctrl-bar button.active{background:rgba(130,210,255,.30);border-color:rgba(130,210,255,.55)}
    `;
    document.head.appendChild(style);

    const layer = document.createElement('div');
    layer.id = 'dk-wall-ui-layer';

    const bar = document.createElement('div');
    bar.id = 'dk-ctrl-bar';
    bar.innerHTML = `
      <button id="dk-btn-collapse" class="dk-ctrl-toggle" title="Daralt" aria-expanded="true">‹</button>
      <span class="dk-ctrl-title"><b>Sanal İkonlar</b></span>
      <button id="dk-btn-add" class="dk-ctrl-item">+ Icon</button>
      <button id="dk-btn-shuffle" class="dk-ctrl-item">Shuffle</button>
      <button id="dk-btn-clear" class="dk-ctrl-item">Clear</button>
      <button id="dk-btn-hide" class="dk-ctrl-item" title="İkonları Gizle / Göster">Hide UI</button>
    `;
    bar.addEventListener('pointerdown', e => e.stopPropagation());

    document.body.appendChild(layer);
    document.body.appendChild(bar);

    document.getElementById('dk-btn-collapse').addEventListener('click', () => {
      const next = !effectiveCollapsed();
      userCollapsed = next;
      syncBarUi();
      persistCollapsed(next);
    });

    document.getElementById('dk-btn-add').addEventListener('click', () => {
      const [name, type] = nextTurkishName();
      uiHidden = false;
      const vw = window.innerWidth || 1280;
      const vh = window.innerHeight || 720;
      const w = 76, h = 86;
      const p = clampPos(64 + Math.random() * Math.max(40, vw - w - 128), 54 + Math.random() * Math.max(40, vh - h - 160), w, h);
      addedIcons.push({ name, type, x: p.x, y: p.y, w, h });
      renderIconsDom();
      syncBarUi();
      dispatchMergedIcons();
      persistIconsNow();
    });

    document.getElementById('dk-btn-shuffle').addEventListener('click', () => {
      if (!addedIcons.length) return;
      uiHidden = false;
      const vw = window.innerWidth || 1280;
      const vh = window.innerHeight || 720;
      addedIcons.forEach(ic => {
        const p = clampPos(48 + Math.random() * Math.max(40, vw - ic.w - 96), 44 + Math.random() * Math.max(40, vh - ic.h - 140), ic.w, ic.h);
        ic.x = p.x; ic.y = p.y;
      });
      renderIconsDom();
      syncBarUi();
      dispatchMergedIcons();
      persistIconsNow();
    });

    document.getElementById('dk-btn-clear').addEventListener('click', () => {
      addedIcons = [];
      uiHidden = false;
      renderIconsDom();
      syncBarUi();
      dispatchMergedIcons();
      persistIconsNow();
    });

    document.getElementById('dk-btn-hide').addEventListener('click', () => {
      uiHidden = !uiHidden;
      syncBarUi();
    });

    window.addEventListener('resize', () => {
      renderIconsDom();
      reportBarRect();
    });

    renderIconsDom();
    syncBarUi();
  }

  loadLocalFallback();
  installWebViewProxy();
  if (document.body) mountDom();
  else document.addEventListener('DOMContentLoaded', mountDom, { once: true });
})();
