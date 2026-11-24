# DiyarDesk — Repository & Development Guide (`AGENTS.md`)

> Canonical architectural reference and contributor/agent guide for the **DiyarDesk** repository (independent GPL-3.0 fork based on Deskland Preview).
> Communicate with the repository owner in **Turkish** when working interactively on this fork.

> [!IMPORTANT]
> **Native Rust + OpenGL 3.3 Öncelikli Tema Mimarisi (Zorunlu Ajan Kuralı):**
> DiyarDesk artık birincil duvar kağıdı motoru olarak **native Rust + OpenGL 3.3 (`native/diyardesk-gl` → `host/bin/diyardesk_gl.dll`)** kullanmaktadır. `prototype/*.html` dosyaları önceki emekler ve isteğe bağlı WebView/tarayıcı/içe aktarım uyumluluğu için korunmaktadır; ancak yeni temalar varsayılan olarak yalnızca HTML/JS şeklinde yazılmamalıdır.
>
> **Yeni bir tema istendiğinde, yapay zekâ ajanı kodlamaya başlamadan önce kullanıcıya mutlaka aşağıdakini sormalıdır (kullanıcı açıkça belirtmediyse):**
> 1. **Yalnızca Rust + OpenGL 3.3 (`native/diyardesk-gl`)** *(Varsayılan / Önerilen)*
> 2. **Yalnızca HTML5/Canvas + JS (`prototype/`)** *(WebView / tarayıcı / içe aktarılabilir tema)*
> 3. **Her ikisi birden (HTML/JS prototipi + Rust + OpenGL 3.3)**
>
> **Rust + OpenGL 3.3 Yeni Tema Entegrasyon Kontrol Listesi:**
> - Modüler klasör yapısı oluştur: `native/diyardesk-gl/src/<tema_id>/` (`mod.rs`, `shaders.rs`, gerekirse `field.rs`, `atlas.rs`, vb.).
> - Motor bağlantıları: `native/diyardesk-gl/src/lib.rs`, `state.rs`, `renderer.rs` (mevcut diğer temalara dokunmadan).
> - Windows Host kaydı: `host/DesktopHost.cs` içindeki `NativeGlThemes` (ve yalnızca GL olan temalarda `IsNativeGlPage`) listesine tema kimliğini ekle.
> - Ayarlar & yerelleştirme: `prototype/themes.json`, `prototype/i18n/en.json`, `prototype/i18n/tr.json` (ve ayarlar iframe'inde 404 olmaması için hafif bir `prototype/<tema_id>.html` kartı/yönlendirmesi).
> - OpenGL görsel temizlik kuralları:
>   - Hedef alfa kanalını korumak için `(gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE)` kullan (yarı saydam elemanların PNG/DWM üzerinde sütlü gri görünmesini engeller).
>   - Radyal glow sprite dokularını RGBA8 üret; dış 1 px kenarlık kesinlikle `alpha = 0` (`GL_CLAMP_TO_EDGE`).
>   - Önbellekleri `(w, h, icon_count)` yerine monotonik `rev: u64` sayacıyla geçersiz kıl; `resize(w, h)` yalnızca `> 0.5 px` değişimde `dirty = true` yapsın.
> - Doğrulama: `cargo test --lib` (determinizm, parametre/enum, ikon taşıma/uç durumlar ve GPU PNG görsel incelemesi) + `cargo build --release` → `host/bin/diyardesk_gl.dll` kopyalama.

---

## 1. Project Overview

**DiyarDesk** renders adaptive desktop wallpapers powered primarily by a **native Rust + OpenGL 3.3 Core engine (`native/diyardesk-gl`)** alongside legacy/portable HTML5/Canvas/WebGL prototypes (`prototype/`), reacting in real time to desktop icon positions (`icons` messages with icon bounding rectangles) without cloud accounts, telemetry, purchase flows, or auto-updater endpoints.

This fork (**DiyarDesk**) builds on upstream Deskland Preview (`tools/version.json`: `0.1.4`, GPL-3.0) using its own product identity and original icon, and includes:
1. **Shared Web Prototype (`prototype/`)** — All wallpaper theme pages, parameter definitions (`themes.json`), settings UI (`settings.html`), snapshot/video sharing (`share.js`), and UI translations (`i18n/en.json`, `i18n/zh-CN.json`, `i18n/zh-TW.json`, `i18n/tr.json`, `i18n/de.json`, `i18n/fr.json`, `i18n/ja.json`, `i18n/ko.json`).
2. **Linux Desktop Host (`linux/`)** — Python 3 + GTK3 + WebKit2GTK host with X11/Wayland layer-shell wallpaper support, static wallpaper fallback, KDE Plasma wallpaper plugin + IconSpy integration, system tray, fullscreen auto-pause, theme importer, and `.deb`/Arch packaging.
3. **Windows Desktop Host (`host/` & `tools/`)** — Upstream C#/WinForms/WebView2 host (`host/DesktopHost.cs`), PowerShell build script (`host/build.ps1`), and NSIS installer scripts (`tools/build-installer.ps1`, `tools/installer.nsi`).
4. **Android Tablet Live Wallpaper (`android/`)** — Native Kotlin `WallpaperService` (`DiyarWallpaperService`, `minSdk = 29`, `requiresSmallestWidthDp = "600"`), shared `settings.html` host, SAF theme importer, and interactive tablet icon/widget layout editor.

---

## 2. Repository Layout

| Path | Purpose |
| --- | --- |
| `prototype/` | Shared HTML/JS/CSS wallpaper themes, settings UI, i18n JSONs, assets, thumbnails |
| `prototype/themes.json` | Canonical theme registry (`meadow`, `beach`, `aurora`, `glade`, `rift`, `strike`, `pocket`, `roach`, `rosebrook`, `polarnight`, `desert`, `zen-archipelago`, `istanbul`) |
| `prototype/settings.html` | Settings window UI (theme grid, live preview iframe, controls, import button & `?` guide modal, snapshot/video share) |
| `prototype/share.js` | Shared privacy-safe snapshot/video capture helpers (sample icons & color blocks, never real user filenames) |
| `prototype/i18n/` | UI & theme localization dictionaries (`en.json`, `zh-CN.json`, `zh-TW.json`, `tr.json`, `de.json`, `fr.json`, `ja.json`, `ko.json`) |
| `prototype/assets/` | Sprite artwork and audio assets (palms, kite boy, black cat, `roach-default.mp3`) |
| `prototype/thumbs/` | Hand-crafted theme thumbnails (`beach.webp`, `meadow.webp`) and inline data (`thumbs-data.js`) |
| `prototype/serve.mjs` | Zero-dependency Node dev server (`http://localhost:5178/settings.html`) |
| `linux/deskland_linux/` | Modular Python Linux host package |
| `linux/deskland_linux/app.py` | Main `Gtk.Application` (`DesklandHost`), message dispatch, settings/wallpaper lifecycle |
| `linux/deskland_linux/config.py` | `APP_NAME = "DiyarDesk"`, XDG paths (`~/.config/deskland`), `SettingsStore`, `ThemeRegistry` merge, `I18n`, `Log` |
| `linux/deskland_linux/server.py` | Loopback HTTP server (`127.0.0.1:<port>`) serving `prototype/`, `/__share/`, `/__audio/`, and `/__custom/` |
| `linux/deskland_linux/bridge.py` | WebKitGTK `UserScript` bridge translating `window.chrome.webview` ↔ `window.webkit.messageHandlers` |
| `linux/deskland_linux/icons.py` | Desktop icon position adapters (GVfs/`gio info`, Xfce RC, KDE Plasma config + IconSpy, fallback grid) |
| `linux/deskland_linux/wallpaper.py` | Wallpaper renderer window (`X11 DESKTOP`, `gtk-layer-shell`, or transparent render surface for static/Plasma) |
| `linux/deskland_linux/static_wallpaper.py` | Fallback frame-to-PNG setter via `gsettings`, `qdbus` Plasma scripting, `xfconf-query`, `feh` |
| `linux/deskland_linux/importer.py` | Custom theme validation & import/removal engine (`.zip`/`.deskland`, `.html`, images, videos) |
| `linux/deskland_linux/pause.py` | Fullscreen / maximized window detector (X11 `xdotool`/`xprop`, `swaymsg`, Hyprland IPC) |
| `linux/deskland_linux/settings_window.py` | Dedicated WebKitGTK settings dialog |
| `linux/deskland_linux/tray.py` | AppIndicator3 / `Gtk.StatusIcon` system tray menu |
| `linux/deskland_linux/share.py` | PNG/WebM share decoding, file saving (`~/Pictures/DiyarDesk`), GTK clipboard & reveal |
| `linux/deskland_linux/autostart.py` | XDG autostart desktop entry manager (`~/.config/autostart/deskland.desktop`) |
| `linux/plasma-wallpaper/org.deskland.preview/` | Native KDE Plasma 6 WebEngine wallpaper plugin (`main.qml`, `inject.js`) |
| `linux/kde-iconspy/org.deskland.iconspy/` | KDE Plasma companion helper for live desktop icon coordinates |
| `linux/packaging/` | Desktop file, SVG icon, `build-deb.sh`, and Arch `arch/PKGBUILD` |
| `linux/tests/` | Pytest unit test suite (`test_autostart.py`, `test_config.py`, `test_icons.py`, `test_importer.py`, `test_share.py`) |
| `host/` | Windows C# WebView2 host (`DesktopHost.cs`, `Themes.cs`, `build.ps1`, `app.manifest`, icon) |
| `android/` | Native Android Tablet (`sw600dp+`) `WallpaperService`, layout editor, SAF importer & JUnit tests |
| `tools/` | `version.json` and Windows NSIS installer packaging scripts |
| `dist/` | Built release artifacts (`.exe`, `.apk`, `.deb`, `.pkg.tar.zst`, `.tar.gz`, manifests, SHA256 checksums) |

---

## 3. Themes & Prototype Rules

### Built-in Themes (`prototype/themes.json`)
All 13 built-in themes in this fork have `status: "ready"` and full source code (no locked placeholder themes):
- **`meadow`** (`meadow.html`, adaptive): Ink-and-wash meadow, icon-hugging clouds & shadows, animated kite boy and black cat.
- **`beach`** (`flat.html`, non-adaptive coastline around icons): Animated shoreline, waves, kayaks, seabirds, and coconut palms.
- **`aurora`** (`aurora.html`, adaptive): Top-down frozen fjord, ice shimmer, glowing aurora reflection, pine islands & lanterns (`aurora-pines.js`).
- **`glade`** (`glade.html`, adaptive): Forest glade canopy & trees (`glade-trees.js`).
- **`rift`** (`rift.html`, adaptive): Interactive geometric rift/chasm (`rift-props.js`).
- **`strike`** (`strike.html`, adaptive): Lightning/storm reaction around icons (`strike-props.js`).
- **`pocket`** (`pocket.html`, adaptive): Cozy desktop pocket world (`pocket-props.js`).
- **`roach`** (`roach.html`, adaptive): Reactive crawling roach world (`roach-props.js`) with optional audio (`prototype/assets/roach-default.mp3`) — keeps default audio **paused (`audioPlay: 0`)** until the user explicitly presses Play.
- **`rosebrook`** (`rosebrook.html`, adaptive): Blooming flowers/roses around icons over lush green grass & brook (`rosebrook-blooms.js`).
- **`polarnight`** (`polarnight.html`, adaptive): Side-view polar night lake horizon, triple aurora ribbons, shooting stars, snow forest clearing around icons, and warm shore cabin with smoke.
- **`desert`** (`desert.html`, adaptive): Overhead Sahara desert dunes with raised sunlit sand hills beneath dropped icons, turquoise date-palm oases, Bedouin tents, walking camel caravans, and robed Arab travelers (`desert-props.js`).

### Localization & Consistency
- **Supported UI languages**: **Simplified Chinese (`zh-CN`), Traditional Chinese (`zh-TW`), English (`en`), Turkish (`tr`), German (`de`), French (`fr`), Japanese (`ja`), and Korean (`ko`)**.
- `settings.html` resolves `t(key)` as `lang -> 'en' -> 'zh-CN' -> key`, while `el.textContent` is written by `applyStatic()` (do not embed HTML markup inside `i18n` values).
- Keep defaults in `prototype/themes.json` synchronized with fallback defaults in each theme page HTML and translations in `prototype/i18n/en.json`, `prototype/i18n/zh-CN.json`, `prototype/i18n/zh-TW.json`, `prototype/i18n/tr.json`, `prototype/i18n/de.json`, `prototype/i18n/fr.json`, `prototype/i18n/ja.json`, and `prototype/i18n/ko.json`.
- **Plasma bridge registration**: When adding or renaming a theme HTML page, verify `linux/plasma-wallpaper/org.deskland.preview/contents/ui/inject.js` handles/recognizes it alongside icon injection.
- **Thumbnails**: `prototype/thumbs/` and `prototype/thumbs-data.js` are hand-crafted; never regenerate or overwrite existing thumbnails unless explicitly requested.

### Rendering & Performance Targets
- Wallpapers run continuously in the background and target **30 fps on integrated GPUs**.
- Cache static terrain/background drawing in offscreen layers; avoid per-pixel full-screen fragment shader loops over large object counts.
- Draw canvases using physical pixels paired with fractional CSS display dimensions (`devicePixelRatio`).
- After editing any theme, verify: **icon movement response**, **settings iframe preview**, **fullscreen/window pause & resume**, and **privacy-safe snapshot/video share**.

---

## 4. Host ↔ Prototype Message Contract

Both Windows (`window.chrome.webview`) and Linux (`linux/deskland_linux/bridge.py`) use the same JSON/string message protocol:

- **Host → Wallpaper page**:
  - `{ t: 'settings', theme, params }` — apply active theme settings
  - `{ t: 'reset' }` — restore defaults
  - `{ t: 'icons', vis, r: [[left, top, right, bottom], ...] }` — icon bounding rects in **physical pixels** (`scale * logicalCoords`, unless `DESKLAND_ICON_POS_PHYSICAL=1`)
  - `{ t: 'pause', v: boolean }` — pause/resume animation
  - `{ t: 'snapshot' }` / `{ t: 'record' }` — trigger `desklandSnapshot()` / `desklandRecord()`
- **Wallpaper page → Host**:
  - `'ready'`
  - `'snapshot|<thumbDataUrl>|<fullPngDataUrl>'`
  - `'recprogress|<0..1>'`
  - `'record|<mime>|<posterDataUrl>|<videoDataUrl>'`
- **Settings UI → Host**:
  - `hello`, `set`, `reset`, `theme`, `lang`, `autostart`, `autostartWarnSeen`
  - `pickAudio`, `importTheme`, `removeTheme`
  - `share`, `shareVideo`, `openShare`, `revealShare`, `openWebsite`
- **Host → Settings UI**:
  - `init`, `icons`, `themeChanged`, `lang`, `autostart`, `recProgress`, `shareResult`, `audioPicked`, `themeImported`, `themeRemoved`, `importError`

---

## 5. Custom Theme Importer (`linux/deskland_linux/importer.py`)

Settings UI exposes `+ Import theme…` alongside an in-app **`?` format guide modal** (`#importHelpModal`).

- **Storage**: Imported themes live in `$XDG_CONFIG_HOME/deskland/themes/` (`~/.config/deskland/themes/`, override via `DESKLAND_CUSTOM_THEMES_DIR`), indexed in `registry.json` and served at `/__custom/<theme_id>/`. `ThemeRegistry.merged_json_bytes()` appends them seamlessly to `/themes.json`.
- **Supported Formats**:
  1. **Theme Packages (`.zip`, `.deskland`)**: Up to **150 MB** (`MAX_ARCHIVE_BYTES`) and **2,000 files** (`MAX_ARCHIVE_FILES`). Validates ZIP CRC integrity, rejects symlinks, absolute paths, and zip-slip `..` traversal. Strips a single wrapping directory if present. Reads `manifest.json` or `theme.json` for `name`, `desc`, `entry`/`page`, `adaptive`, `groups`; otherwise falls back to `index.html` or the first `.html` file.
  2. **Single Web Page (`.html`, `.htm`)**: Single-file HTML/Canvas/Shader wallpaper wrapped into `index.html`, reading `<title>` as default name.
  3. **Image Wallpapers (`.png`, `.jpg`, `.jpeg`, `.webp`, `.gif`, `.svg`, `.avif`, `.bmp`)**: Auto-wrapped into a responsive page with ambient zoom/pan, `brightness`, `motion`, `dimIcons`, `adaptive`, and `desklandSnapshot` support.
  4. **Video Wallpapers (`.mp4`, `.webm`, `.mkv`, `.mov`)**: Muted looping `<video>` wrapper with `brightness`, `speed`, and `dimIcons` controls.
- **UI Removal**: Clicking `Remove` (`#removeTheme`) on an imported theme invokes `remove_custom_theme`, removes its directory and `registry.json` entry, and switches back safely to a built-in theme if it was active.

---

## 6. Linux Architecture & KDE Plasma Notes

- **Wallpaper Surface Strategy (`wallpaper.py`, `static_wallpaper.py`)**:
  - **X11**: Uses a `Gdk.WindowTypeHint.DESKTOP` GTK window with empty input region, kept below desktop icon windows (`Nautilus`/`xfdesktop`/etc.).
  - **wlroots Wayland (Sway, Hyprland, river)**: Attaches to `wlr-layer-shell` `background` layer via `gtk3-layer-shell` (`gir1.2-gtklayershell-0.1`).
  - **KDE Plasma / GNOME Wayland**: Compositor shell/desktop containment covers standard layer-shell `background`, so Linux host uses either the native Plasma wallpaper plugin (`org.deskland.preview`) + `org.deskland.iconspy`, or `--static` fallback rendering frames on an unintrusive render surface (`DiyarDeskRender`, muted when offscreen/render-only) and applying snapshots natively (`qdbus` / `gsettings`). Do not steal foreground focus or track own/invisible/cloaked helper windows in fullscreen pause checks.
- **Installed Runtime Paths (local Fedora/Arch/Debian deployment)**:
  - Code & prototype assets: `/usr/lib/deskland/linux/deskland_linux/` (or `/usr/lib/deskland/deskland_linux/`) and `/usr/lib/deskland/prototype/`
  - User config, logs, audio copies, custom themes, rendered static frames: `~/.config/deskland/`
  - User autostart unit (systemd-managed setups): `systemctl --user restart app-deskland@autostart.service`

---

## 7. Build, Preview, Test & Verification Commands

### Browser Preview (UI & Theme Development)
```bash
node prototype/serve.mjs
# Open http://localhost:5178/settings.html (or ?lang=en)
```
Browser preview uses sample icon coordinates; real desktop icon tracking runs under Windows or Linux host.

### Linux Host Run & Unit Tests
```bash
# Run from source on Linux:
./linux/run.sh

# Run Python unit tests (22 tests covering autostart, config, icons, importer, share):
PYTHONPATH=linux uv run --with pytest pytest -v linux/tests
# (or `PYTHONPATH=linux pytest -v linux/tests` if pytest is installed in system Python)
```

### Android Tablet Build & Unit Tests
```bash
# Requires Android SDK (also runs via .github/workflows/build-android.yml):
gradle --project-dir android testDebugUnitTest assembleDebug
```

### Windows Host & Installer (on Windows)
```powershell
powershell -ExecutionPolicy Bypass -File host\build.ps1          # builds host/bin/Deskland.exe
powershell -ExecutionPolicy Bypass -File tools\build-installer.ps1  # NSIS installer -> dist/
```
Note: Editing `prototype/` HTML/JS/CSS/JSON does **not** require recompiling `host/DesktopHost.cs`; reload via tray menu **Reload** (or restart `app-deskland@autostart.service` on Linux).

---

## 8. Product Identity, Security & Licensing Guardrails

- **Product identity**: Display name is **`DiyarDesk`** (independent fork based on Deskland Preview GPL-3.0, with original logo in `linux/packaging/deskland.svg`). Internal runtime/compatibility IDs (`~/.config/deskland`, `/usr/lib/deskland`, `org.deskland.preview`, `org.deskland.iconspy`, `Software\DesklandPreview`, `host/bin/Deskland.exe`) are preserved so user configs, Plasma plugins, and installed binaries remain compatible without data loss.
- **Never commit** build/binary caches or secrets: `host/bin/`, `host/lib/`, `__pycache__/`, `.pytest_cache/`, personal config/credentials, or hardcoded user-local paths in runtime code.
- **Security & Legal**: Preserve `LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES.md`, and `TRADEMARKS.md`. Obtain explicit confirmation from the project owner before modifying trademark/branding assets or publishing releases. Do not add external auth/telemetry/store/update endpoints.
