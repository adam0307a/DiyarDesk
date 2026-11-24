# 第三方依赖 / Third-party dependencies

## Microsoft Edge WebView2

Windows 宿主使用 Microsoft Edge WebView2 SDK 与 WebView2 Runtime。构建脚本从 NuGet 获取开发库，本仓库不提交 SDK 二进制文件。

- 包：`Microsoft.Web.WebView2`，构建版本 `1.0.4191.47`。
- 包与许可信息：https://www.nuget.org/packages/Microsoft.Web.WebView2/1.0.4191.47
- 文档：https://learn.microsoft.com/microsoft-edge/webview2/

WebView2 及 Windows / .NET Framework 组件适用各自的许可条款，不因本项目的 GPL-3.0 许可而变更。

The host uses the Microsoft Edge WebView2 SDK and Runtime. SDK binaries are downloaded by the build script and are not checked into this repository. Microsoft components retain their respective licenses.

## NSIS

Windows 安装包使用 NSIS 3.13 制作。便携编译器仅在构建时下载，不作为项目源码提交。NSIS 及其压缩模块适用各自许可，详见 https://nsis.sourceforge.io/License 。安装包附带项目对应源码 ZIP。

## ThinkingOrbs

`prototype/thinking-orbs-engine.js` 与 `prototype/thinking-orbs.html` 中的 9 种 3D 点阵球体动画算法移植自 `haplollc/ThinkingOrbs`（其底层数学引擎源自 Jakub Antalik 的 `thinking-orbs`），采用 MIT 许可证发布：

- 仓库地址：https://github.com/haplollc/ThinkingOrbs
- 版权声明：Copyright (c) 2026 Haplo LLC; Copyright (c) 2026 Jakub Antalik
- 许可证：MIT License（https://opensource.org/licenses/MIT）

The 9 3D dotted orb designs in `prototype/thinking-orbs-engine.js` and `prototype/thinking-orbs.html` are ported from `haplollc/ThinkingOrbs` (originally Jakub Antalik's `thinking-orbs` web engine) under the MIT License.

## Quiet Snow (Snowhush Inspiration)

`prototype/quietsnow.html` ve `prototype/quietsnow-engine.js` dosyaları DiyarDesk için sıfırdan özgün (GPL-3.0) olarak yazılmıştır; ancak etkileşimli kar birikimi, pencere üstünden kayma/dökülme, imleçle süpürme ve dört mevsim atmosfer konsepti için `mbezruchko/Snowhush` projesinden ilham alınmıştır:

- Referans proje / Reference project: https://github.com/mbezruchko/Snowhush
- Not: `prototype/quietsnow.html` and `prototype/quietsnow-engine.js` are original GPL-3.0 implementations written from scratch for DiyarDesk, inspired by the interactive snow & seasonal desktop experience of `mbezruchko/Snowhush`.

## Liquid Glass Studio

`prototype/liquidglass.html` içindeki WebGL2 fiziksel Snell kırılımı, kromatik RGB dispersiyon, ayrık 2 geçişli Gaussian bulanıklık, süperelips SDF + `smin` sıvı kaynaşması ve LCH Fresnel/parlama shader matematiği MIT lisanslı `iyinchao/liquid-glass-studio` projesinden uyarlanmıştır:

- Depo / Repository: https://github.com/iyinchao/liquid-glass-studio
- Telif / Copyright: Copyright (c) 2024 Charles Yin
- Lisans / License: MIT License (https://opensource.org/licenses/MIT)
- LCH renk dönüşüm yardımcıları: `GLSL-Color-Functions` (`Rachmanin0xFF/GLSL-Color-Functions`)

The WebGL2 physical Snell refraction, chromatic RGB dispersion, 2-pass separable Gaussian blur, superellipse SDF with polynomial `smin` liquid merging, and LCH Fresnel/glare shader math in `prototype/liquidglass.html` are adapted from `iyinchao/liquid-glass-studio` under the MIT License.

## Studio Glass (ThreeUI Inspiration) & Three.js

`prototype/studio-glass.html` ve `prototype/studio-glass-engine.js` dosyaları DiyarDesk için sıfırdan özgün (GPL-3.0) olarak yazılmıştır; ancak beş ışık önayarı, hareketli ahşap jaluzi/panjur, dönebilen 3B tasarımcı monitörü, sürüklenebilir stüdyo eşyaları (klavye, fare, sandalye, bitkiler, mantar lamba) ve kırıcı cam kontrol panelleri konsepti için ThreeUI'ın Studio Glass çalışmasından ilham alınmıştır:

- Referans bağlantı / Reference link: https://threeui.com/studio-glass
- 3B motor / 3D engine: `prototype/lib/three.min.js` — **Three.js** (`three@0.170.0`, Copyright © 2010–2024 three.js authors, MIT License — https://github.com/mrdoob/three.js)
- Not: `prototype/studio-glass.html` and `prototype/studio-glass-engine.js` are original GPL-3.0 implementations written from scratch for DiyarDesk using bundled MIT-licensed Three.js, inspired by the Studio Glass interactive workspace concept at https://threeui.com/studio-glass.


## `@samasante/liquid-glass` (Optical Refraction Reference)

`native/diyardesk-gl/src/liquidglass/shaders.rs`, `prototype/liquidglass.html`, `native/diyardesk-gl/src/studio_glass/shaders.rs`, and `prototype/studio-glass-engine.js` incorporate dual-zone Snell optical rim/dome refraction, white-preserving multi-band spectral dispersion, and smooth SDF gradient optics inspired by `@samasante/liquid-glass`:

- Repository: https://github.com/samasante/liquid-glass

## Native Rust + OpenGL 3.3 Engine Crates (`native/diyardesk-gl`)

The native Rust renderer (`diyardesk_gl.dll`) uses the following open-source crates declared in `native/diyardesk-gl/Cargo.toml` (all permissive MIT / Apache-2.0 / Zlib licenses, compatible with GPL-3.0):

- `fontdue` (MIT / Apache-2.0 / Zlib) — TrueType/OpenType glyph rasterization
- `serde` & `serde_json` (MIT / Apache-2.0) — JSON serialization for host/engine state
- `base64` (MIT / Apache-2.0) — Base64 encoding/decoding for local widget artwork
- `image` (MIT / Apache-2.0) — Local PNG/JPEG/WebP image decoding
- `chrono` (MIT / Apache-2.0) — Local clock/date formatting
- `windows-sys` (MIT / Apache-2.0) — Win32 GDI/OpenGL/Windowing FFI bindings

## Platform Runtime Libraries (Linux, macOS, Android)

- **Linux (`linux/`):** Uses system-provided Python 3, PyGObject, GTK 3, WebKit2GTK, optional `gtk-layer-shell`, KDE Plasma 6 WebEngine/Kirigami QML APIs, and GNOME Shell JavaScript extension APIs under their respective open-source licenses (LGPL / GPL / MIT).
- **macOS (`macos/`):** Uses system Apple frameworks (`AppKit`, `WebKit`, `Foundation`).
- **Android (`android/`):** Uses Kotlin standard library, AndroidX (`core-ktx`, `appcompat`, `webkit` `WebViewAssetLoader`), and Material Components (Apache-2.0).
