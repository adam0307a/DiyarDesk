# Security Policy

## Supported Versions

DiyarDesk is maintained as a community fork of Deskland Preview (GPL-3.0). Security fixes are applied to the latest active minor release series:

| Version | Supported |
| --- | --- |
| `0.2.x` | Yes |
| `< 0.2.0` | No |

## Reporting a Vulnerability

If you discover a security vulnerability in DiyarDesk (such as an archive extraction bug, WebView sandbox escape, local HTTP server issue, or privilege/IPC flaw), **please do not open a public GitHub issue** with exploit details.

Instead, report it privately via one of the following channels:

1. **GitHub Security Advisories (preferred):** Open a private vulnerability report on the repository's **Security -> Advisories -> Report a vulnerability** page (`https://github.com/adam0307a/DiyarDesk/security/advisories/new`).
2. **Maintainer contact:** If GitHub Private Vulnerability Reporting is unavailable, contact the repository maintainer directly through their GitHub profile before disclosing details publicly.

Please include:
- Affected platform(s) (`Windows`, `Linux`, `Android`, or `prototype/`) and DiyarDesk version
- Clear reproduction steps or a minimal proof-of-concept archive/file
- Expected vs. actual behavior and potential impact

We aim to acknowledge reports within **72 hours** and coordinate a fix and release advisory before public disclosure.

## Security Model & Guarantees

DiyarDesk is designed to run **100% locally** with zero accounts, zero telemetry, and zero background network calls:

- **Isolated Local Origins:** Built-in and imported wallpapers are served exclusively from isolated local origins (`app.local` / `custom.local` / `audio.local` / `share.local` on Windows WebView2, loopback `127.0.0.1` on Linux WebKitGTK, and `https://appassets.androidplatform.net` on Android `WebViewAssetLoader`) rather than `file://`.
- **Network & Navigation Containment:** Strict `Content-Security-Policy` headers and WebView navigation/resource policies block external network requests, remote scripts/iframes, popups, and script dialogs from wallpaper pages.
- **Custom Theme Archive Hardening:** Importing `.zip` / `.deskland` packages enforces Zip Slip path-traversal checks, symlink rejection, an allowlist of safe web/shader/media/font/asset file extensions, and strict uncompressed size (`150 MB`) and file count (`2,000` files) limits across Windows, Linux, and Android.
- **Privacy-Safe Sharing:** Desktop snapshots and video recordings render synthetic sample icons and color blocks so real user filenames and desktop contents are never exposed.

## Scope Notes

- User-imported HTML/JS themes execute JavaScript inside an isolated WebView to render Canvas/WebGL visuals. While external network access, local filesystem access, host object bridges, and unsafe archive file types are blocked, users should still only import third-party themes from sources they trust.
- Windows release binaries are currently built in public CI without a paid Authenticode certificate; always verify release SHA-256 checksums published alongside each GitHub Release.
