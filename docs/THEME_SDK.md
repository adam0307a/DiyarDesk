# DiyarDesk Theme SDK & Custom Theme Guide

DiyarDesk supports importing custom wallpapers directly from **Settings → `+ Import theme…`** across **Windows**, **Linux**, **macOS**, and **Android Tablets**. This guide documents the supported package formats, manifest schema, host ↔ theme message protocol, and local browser preview workflow.

---

## 1. Supported Import Formats

| Format | Extensions | Notes |
| --- | --- | --- |
| **Theme Package** | `.zip`, `.deskland` | Multi-file HTML/JS/CSS/WebGL/asset bundle with optional `manifest.json` or `theme.json`. Limits: **150 MB** uncompressed, **2,000 files** max. |
| **Single HTML Page** | `.html`, `.htm` | Self-contained Canvas/WebGL/CSS scene. Uses `<title>` as default theme name. |
| **Fragment Shader** | `.glsl`, `.frag` | Shadertoy-style fragment shaders auto-wrapped into a full-screen WebGL canvas. |
| **Static / Animated Image** | `.png`, `.jpg`, `.jpeg`, `.webp`, `.gif`, `.svg`, `.avif`, `.bmp` | Auto-wrapped with ambient pan/zoom, brightness, motion, and `dimIcons` controls. |
| **Video Wallpaper** | `.mp4`, `.webm`, `.mkv`, `.mov` | Muted looping playback with brightness, speed, and `dimIcons` controls. |

> **Security & Sandbox Rules:** Packages are scanned for Zip Slip (`..`) traversal, symlinks, and disallowed binary/executable extensions. Wallpaper pages run under an isolated local origin (`custom.local` / `127.0.0.1` / `WebViewAssetLoader`) with a strict **Content-Security-Policy** that blocks external internet calls and remote scripts. Bundle all scripts, shaders, fonts, and textures locally inside your archive.

---

## 2. Theme Package Layout & `manifest.json`

A typical `.zip` or `.deskland` archive looks like:

```text
my-adaptive-theme.deskland
├── manifest.json       # (or theme.json) optional metadata & customizable controls
├── index.html          # entry page (referenced by "entry" or auto-detected)
├── scene.js
└── textures/
    └── noise.png
```

### Example `manifest.json`
```json
{
  "name": "Starlight Reef",
  "desc": "Bioluminescent coral clusters that glow around desktop icons.",
  "entry": "index.html",
  "adaptive": true,
  "groups": [
    {
      "label": "Atmosphere",
      "items": [
        {
          "key": "glow",
          "label": "Glow Intensity",
          "type": "range",
          "min": 0,
          "max": 100,
          "step": 1,
          "default": 65
        },
        {
          "key": "palette",
          "label": "Palette",
          "type": "select",
          "default": "lagoon",
          "options": [
            { "value": "lagoon", "label": "Lagoon" },
            { "value": "abyss", "label": "Deep Abyss" }
          ]
        }
      ]
    }
  ]
}
```

- If a single top-level wrapper folder exists inside the archive, the importer automatically strips it.
- If `manifest.json` / `theme.json` is omitted, DiyarDesk looks for `index.html` (or the first `.html` file) and derives the theme name from `<title>` or the filename.

---

## 3. Host ↔ Theme Message Protocol

DiyarDesk injects a unified `window.chrome.webview` bridge on Windows, Linux (WebKitGTK), macOS, KDE Plasma 6, and Android, so one script works everywhere.

### Receiving Messages from the Host
```js
const state = {
  icons: [],        // Array of [left, top, right, bottom] in physical pixels
  iconsVisible: true,
  paused: false,
  params: { glow: 65, palette: 'lagoon' },
};

function handleHostMessage(raw) {
  const msg = typeof raw === 'string' ? JSON.parse(raw) : raw;
  if (!msg || !msg.t) return;

  switch (msg.t) {
    case 'icons':
      // r: [[left, top, right, bottom], ...] in physical pixels
      // Divide by window.devicePixelRatio to convert to CSS pixels.
      state.iconsVisible = msg.vis !== false;
      state.icons = Array.isArray(msg.r) ? msg.r : [];
      onIconsChanged();
      break;

    case 'settings':
      if (msg.params && typeof msg.params === 'object') {
        Object.assign(state.params, msg.params);
      }
      break;

    case 'reset':
      state.params = { glow: 65, palette: 'lagoon' };
      break;

    case 'pause':
      state.paused = !!msg.v;
      break;

    case 'snapshot':
      if (typeof window.desklandSnapshot === 'function') window.desklandSnapshot();
      break;

    case 'record':
      if (typeof window.desklandRecord === 'function') window.desklandRecord();
      break;
  }
}

if (window.chrome && window.chrome.webview) {
  window.chrome.webview.addEventListener('message', (e) => handleHostMessage(e.data));
  // Signal the host that the theme is ready to receive initial settings and icon rectangles
  window.chrome.webview.postMessage('ready');
}
```

### Coordinate System & HiDPI Tips
- Icon rects arrive as `[left, top, right, bottom]` in **physical pixels**.
- Size your `<canvas>` backing buffer to `Math.round(innerWidth * devicePixelRatio)` × `Math.round(innerHeight * devicePixelRatio)` while keeping CSS width/height at `100vw` / `100vh`.
- When `state.paused === true`, stop scheduling `requestAnimationFrame` work (or skip heavy GPU draws) so battery/fullscreen auto-pause saves power immediately.

---

## 4. Testing Locally in Any Browser

You do not need to rebuild the native hosts while iterating on a web theme:

```bash
node prototype/serve.mjs
# Open http://localhost:5178/settings.html
```

The built-in settings preview injects sample draggable icon rectangles so you can verify:
1. Smooth icon movement and clustering response
2. Settings slider / dropdown updates (`settings` and `reset`)
3. Pause / resume behavior (`pause`)
4. HiDPI scaling (`devicePixelRatio`) and 30 FPS efficiency on integrated GPUs

---

## 5. Native Rust + OpenGL 3.3 Built-In Themes

Built-in themes in DiyarDesk prioritize the native **Rust + OpenGL 3.3 Core** engine (`native/diyardesk-gl/`) for zero-WebView2 RAM usage and instant startup. If you want to contribute a built-in native GL theme, see `AGENTS.md` and existing modules under `native/diyardesk-gl/src/` (for example `liquidglass/`, `studio_glass/`, `meadow/`).
