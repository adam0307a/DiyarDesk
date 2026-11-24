# Contributing to DiyarDesk

Thank you for your interest in contributing to **DiyarDesk** (`GPL-3.0`)!  
DiyarDesk is an independent community fork based on Deskland Preview, featuring a zero-WebView2 **native Rust + OpenGL 3.3 Core** engine alongside portable **HTML5 / Canvas / WebGL** themes across **Windows**, **Linux**, **macOS**, and **Android Tablets (`sw600dp+`)**.

---

## Quick Ways to Contribute

- **Report bugs** with reproduction steps, OS/desktop environment details, and renderer mode (`Rust OpenGL 3.3` vs. `WebView2 / WebKitGTK`).
- **Create or improve themes** (see [docs/THEME_SDK.md](docs/THEME_SDK.md) for custom HTML/Canvas/WebGL/GLSL themes, or `native/diyardesk-gl/` for native Rust + OpenGL 3.3 themes).
- **Improve translations** in `prototype/i18n/` (`en`, `tr`, `zh-CN`, `zh-TW`, `de`, `fr`, `ja`, `ko`) and `linux/gnome-extension/deskland@deskland.app/po/`.
- **Fix platform issues** across Windows (`host/`), Linux (`linux/`), macOS (`macos/`), and Android (`android/`).

---

## Repository Map

| Directory | Description |
| --- | --- |
| `native/diyardesk-gl/` | Primary native Rust + OpenGL 3.3 Core wallpaper & widget renderer (`diyardesk_gl.dll`) |
| `prototype/` | Shared HTML5/Canvas/WebGL themes, `themes.json`, `settings.html`, `share.js`, `i18n/` |
| `host/` | Windows C# / WinForms / WebView2 + Native GL desktop host (`DesktopHost.cs`, `build.ps1`) |
| `linux/` | Python 3 + GTK3 + WebKit2GTK host, KDE Plasma 6 plugin, GNOME Shell extension, packaging |
| `macos/` | Native macOS host application |
| `android/` | Kotlin `WallpaperService` for Android Tablets (`sw600dp+`) & icon layout editor |
| `tools/` | Version metadata (`version.json`), NSIS installer scripts, WinGet manifests |

---

## Local Development & Verification

### 1. Theme & UI Preview (All Platforms)
```bash
node prototype/serve.mjs
# Open http://localhost:5178/settings.html (or ?lang=en / ?lang=tr)
```

### 2. Native Rust + OpenGL 3.3 Engine (`native/diyardesk-gl`)
```bash
cd native/diyardesk-gl
cargo test --lib
cargo build --release
```

### 3. Windows Host (`host/`)
```powershell
powershell -ExecutionPolicy Bypass -File host\build.ps1
# Produces host\bin\Deskland.exe (and bundles host\bin\diyardesk_gl.dll)
```
> Editing `prototype/` files does not require recompiling `host/DesktopHost.cs`; use tray menu **Reload** to test live changes.

### 4. Linux Host & Unit Tests (`linux/`)
```bash
./linux/run.sh
PYTHONPATH=linux pytest -v linux/tests
```

### 5. Android Tablet Build & Tests (`android/`)
```bash
gradle --project-dir android testDebugUnitTest assembleDebug
```

---

## Engineering & Quality Guidelines

1. **Performance First (30 FPS Integrated GPU Target):**
   - Wallpapers run continuously in the background. Cache static terrain/layers offscreen and avoid uncached per-pixel loops over large object counts.
   - Verify **icon drag response**, **settings preview**, **fullscreen/window pause & resume**, and **privacy-safe share** (`prototype/share.js`) after theme edits.
2. **Localization Consistency:**
   - Keep default parameter values in `prototype/themes.json` synchronized with theme fallback defaults and `prototype/i18n/*.json`.
   - Never embed raw HTML inside `i18n` JSON values (`settings.html` writes via `textContent`).
3. **Zero Telemetry & Offline Guarantee:**
   - Do not add cloud accounts, analytics, remote CDNs, external update checks, or outbound network calls in built-in themes or hosts. See [PRIVACY.md](PRIVACY.md).
4. **Do Not Commit Build Artifacts or Secrets:**
   - Keep `host/bin/`, `host/lib/`, `target/`, `__pycache__/`, `.pytest_cache/`, and local machine paths out of commits.
5. **Licensing & Brand Hygiene:**
   - All contributions are licensed under **GPL-3.0** (`LICENSE`).
   - Preserve `LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES.md`, and `TRADEMARKS.md`. Credit any third-party algorithms or libraries in `THIRD_PARTY_NOTICES.md`.

---

## Pull Request Checklist

- [ ] Code builds cleanly and relevant tests pass (`cargo test --lib`, `pytest`, `host\build.ps1`, or `gradle testDebugUnitTest`).
- [ ] Changes stay focused and avoid unrelated formatting churn.
- [ ] New user-facing strings are added to `prototype/i18n/en.json` (and `tr.json` / other locales where possible).
- [ ] Existing hand-crafted thumbnails in `prototype/thumbs/` are left untouched unless the PR intentionally updates that theme's visuals.
- [ ] For security-sensitive bugs, do **not** open a public PR/issue first — follow [SECURITY.md](SECURITY.md).
