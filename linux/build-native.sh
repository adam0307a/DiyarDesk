#!/usr/bin/env bash
# Build DiyarDesk native Rust + OpenGL 3.3 Core library (libdiyardesk_gl.so)
# and optional KDE Plasma 6 Qt6 QML GL plugin (libdiyardesk_plasma_gl.so).
#
# Usage:
#   ./linux/build-native.sh                 # build into repo (linux/lib/ & plasma build dir)
#   ./linux/build-native.sh --install-user  # also install into ~/.local/lib & ~/.local/share/plasma
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALL_USER=0
SKIP_PLASMA=0

for arg in "$@"; do
    case "$arg" in
        --install-user) INSTALL_USER=1 ;;
        --skip-plasma)  SKIP_PLASMA=1 ;;
        -h|--help)
            echo "Usage: $0 [--install-user] [--skip-plasma]"
            exit 0
            ;;
        *)
            echo "Unknown option: $arg" >&2
            exit 1
            ;;
    esac
done

echo "==> Building Rust OpenGL 3.3 Core renderer (libdiyardesk_gl.so)..."
(cd "$ROOT/native/diyardesk-gl" && cargo build --release)

GL_SO="$ROOT/native/diyardesk-gl/target/release/libdiyardesk_gl.so"
if [[ ! -f "$GL_SO" ]]; then
    echo "ERROR: Expected $GL_SO was not produced." >&2
    exit 1
fi

mkdir -p "$ROOT/linux/lib"
cp -f "$GL_SO" "$ROOT/linux/lib/libdiyardesk_gl.so"
echo "==> Installed: $ROOT/linux/lib/libdiyardesk_gl.so"

PLASMA_SO=""
if [[ "$SKIP_PLASMA" -eq 0 ]] && command -v cmake >/dev/null 2>&1; then
    if pkg-config --exists Qt6Quick Qt6OpenGL 2>/dev/null || [[ -d /usr/include/qt6/QtQuick ]]; then
        echo "==> Building KDE Plasma 6 Qt6 GL QML plugin (libdiyardesk_plasma_gl.so)..."
        cmake -S "$ROOT/linux/plasma-gl-plugin" \
              -B "$ROOT/linux/plasma-gl-plugin/build" \
              -DCMAKE_BUILD_TYPE=Release
        cmake --build "$ROOT/linux/plasma-gl-plugin/build" -j"$(nproc)"
        PLASMA_SO="$ROOT/linux/plasma-gl-plugin/build/libdiyardesk_plasma_gl.so"
        echo "==> Built: $PLASMA_SO"
    else
        echo "==> Qt6Quick/Qt6OpenGL headers not found; skipping KDE Plasma GL plugin."
    fi
fi

if [[ "$INSTALL_USER" -eq 1 ]]; then
    mkdir -p "$HOME/.local/lib" "$HOME/.local/share/deskland"
    cp -f "$GL_SO" "$HOME/.local/lib/libdiyardesk_gl.so"
    ln -sfn "$ROOT/prototype" "$HOME/.local/share/deskland/prototype"
    echo "==> User library installed: $HOME/.local/lib/libdiyardesk_gl.so"
    echo "==> User prototype linked:  $HOME/.local/share/deskland/prototype -> $ROOT/prototype"

    PLASMA_DEST="$HOME/.local/share/plasma/wallpapers/org.deskland.preview"
    mkdir -p "$PLASMA_DEST/contents/ui/gl"
    cp -rf "$ROOT/linux/plasma-wallpaper/org.deskland.preview/." "$PLASMA_DEST/"
    if [[ -n "$PLASMA_SO" && -f "$PLASMA_SO" ]]; then
        cp -f "$PLASMA_SO" "$PLASMA_DEST/contents/ui/gl/libdiyardesk_plasma_gl.so"
        cp -f "$ROOT/linux/plasma-gl-plugin/qmldir" "$PLASMA_DEST/contents/ui/gl/qmldir"
        cp -f "$GL_SO" "$PLASMA_DEST/contents/ui/gl/libdiyardesk_gl.so"
        echo "==> User KDE Plasma 6 hybrid plugin installed: $PLASMA_DEST"
    fi
fi

echo "==> Done."
