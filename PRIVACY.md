# Privacy Policy & Local-First Architecture

**DiyarDesk** is built to run **100% locally on your device**. There are no user accounts, no cloud sync servers, no analytics, no crash-upload telemetry, no advertising SDKs, and no background auto-updater phone-home requests.

---

## 1. Zero Data Collection

DiyarDesk does **not** collect, transmit, store remotely, or share any of the following:
- Personal identifiers, account credentials, or hardware fingerprints
- Desktop filenames, icon labels, folder structures, or window titles
- Usage statistics, active theme choices, or session telemetry
- Screenshots, recordings, or audio streams

---

## 2. How Desktop Data Is Processed Locally

To make wallpapers react to your desktop in real time, DiyarDesk reads certain OS state **strictly in memory**:

| Local Signal | Purpose | Leaves Your Device? |
| --- | --- | --- |
| **Desktop icon bounding rectangles** | Adapts scenery, clouds, ripples, sand dunes, or glass refraction around icon positions | **Never** |
| **Foreground / visible window bounds** | Auto-pauses rendering when fullscreen or tiled windows cover the desktop, and supports Window-Aware snow/effects | **Never** |
| **Power / battery / RDP / lock state** | Automatically pauses rendering on battery saver, remote desktop, or locked screens | **Never** |
| **Optional local clock / solar angle** | Drives local day/night transitions (Solar Sync) using local time/coordinates | **Never** |
| **Optional audio loopback / system metrics** | Drives local audio visualizer or system-bridge widgets when explicitly enabled by the user | **Never** |

---

## 3. Local Files Stored on Disk

Settings, imported themes, and logs stay strictly inside your local user profile:

- **Windows:** `%LOCALAPPDATA%\DesklandPreview\` (settings, imported themes, crash sentinel, logs) and `HKCU\Software\DesklandPreview` (autostart/preferences)
- **Linux:** `~/.config/deskland/` (settings, imported themes, static render cache, logs) and `~/Pictures/DiyarDesk/` (user-triggered snapshots/recordings)
- **macOS:** Application Support & Preferences directories in your local user account
- **Android Tablets:** App-private storage (`Context.filesDir`) and SAF-imported theme copies

Removing DiyarDesk and deleting its local config folder completely erases all stored state.

---

## 4. Privacy-Safe Sharing (`share.js`)

When you explicitly click **Share Snapshot** or **Share Video**, DiyarDesk renders **synthetic placeholder icons and color blocks** instead of your real desktop icons or filenames, so you can share your wallpaper layout publicly without exposing private files.

---

## 5. Network Isolation & Optional Local LAN Sync

- **Built-in & Imported Web Themes:** Served only from isolated local origins (`app.local` / `custom.local` on Windows WebView2, `127.0.0.1` loopback on Linux, `WebViewAssetLoader` on Android) with strict `Content-Security-Policy` headers and navigation/resource filters that block external internet requests from wallpaper pages.
- **Native Rust + OpenGL 3.3 Mode (`native/diyardesk-gl`):** Renders directly through local OpenGL without any embedded browser or networking stack.
- **Optional LAN Sync (Disabled by Default):** If you manually enable LAN Sync to mirror wallpaper state between your desktop and Android tablet, communication occurs directly over your own local Wi-Fi/LAN subnet and never touches an external server.
- **External Links:** Links such as opening the project repository or support page only launch your default browser when you explicitly click them.

---

## 6. Verification & Security Reporting

DiyarDesk is licensed under **GPL-3.0** — anyone can inspect the source code, build from source, or verify release SHA-256 checksums on GitHub Releases. If you discover a privacy or network-isolation bug, please report it privately following [SECURITY.md](SECURITY.md).
