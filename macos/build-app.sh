#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MACOS_DIR="$ROOT/macos"
VER="$(python3 -c "import json; print(json.load(open('$ROOT/tools/version.json'))['version'])")"
ARCH="$(uname -m)"
OUT_DIR="$ROOT/out-macos"
APP_BUNDLE="$OUT_DIR/DiyarDesk.app"

mkdir -p "$OUT_DIR"

echo "==> Running macOS Swift unit tests..."
swift test --package-path "$MACOS_DIR"

echo "==> Building DiyarDeskMac (release, $ARCH)..."
swift build --package-path "$MACOS_DIR" -c release
BIN_PATH="$(swift build --package-path "$MACOS_DIR" -c release --show-bin-path)/DiyarDeskMac"

rm -rf "$APP_BUNDLE"
mkdir -p "$APP_BUNDLE/Contents/MacOS" "$APP_BUNDLE/Contents/Resources" "$APP_BUNDLE/Contents/Frameworks"

cp "$BIN_PATH" "$APP_BUNDLE/Contents/MacOS/DiyarDesk"
chmod +x "$APP_BUNDLE/Contents/MacOS/DiyarDesk"

# Build & bundle native Rust + OpenGL 3.3 Core renderer (libdiyardesk_gl.dylib)
RUST_DIR="$ROOT/native/diyardesk-gl"
if [[ -f "$RUST_DIR/Cargo.toml" ]] && command -v cargo >/dev/null 2>&1; then
  echo "==> Building native Rust OpenGL renderer (libdiyardesk_gl.dylib)..."
  cargo build --release --manifest-path "$RUST_DIR/Cargo.toml"
  DYLIB_SRC="$RUST_DIR/target/release/libdiyardesk_gl.dylib"
  if [[ -f "$DYLIB_SRC" ]]; then
    cp "$DYLIB_SRC" "$APP_BUNDLE/Contents/Frameworks/libdiyardesk_gl.dylib"
    if command -v install_name_tool >/dev/null 2>&1; then
      install_name_tool -id "@rpath/libdiyardesk_gl.dylib" "$APP_BUNDLE/Contents/Frameworks/libdiyardesk_gl.dylib" || true
    fi
  fi
fi

# Bundle shared prototype/ web themes & settings UI
cp -a "$ROOT/prototype" "$APP_BUNDLE/Contents/Resources/prototype"
rm -f "$APP_BUNDLE/Contents/Resources/prototype/serve.mjs"

cat > "$APP_BUNDLE/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>DiyarDesk</string>
  <key>CFBundleDisplayName</key>
  <string>DiyarDesk</string>
  <key>CFBundleIdentifier</key>
  <string>io.github.adam0307a.diyardesk</string>
  <key>CFBundleVersion</key>
  <string>${VER}</string>
  <key>CFBundleShortVersionString</key>
  <string>${VER}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleExecutable</key>
  <string>DiyarDesk</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

# Ad-hoc sign so Gatekeeper local execution & WebKit entitlements work cleanly
codesign --force --deep --sign - "$APP_BUNDLE"

DMG_NAME="DiyarDesk-${VER}-macos-${ARCH}.dmg"
DMG_PATH="$OUT_DIR/$DMG_NAME"
rm -f "$DMG_PATH"

STAGE="$(mktemp -d)"
cp -a "$APP_BUNDLE" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "DiyarDesk ${VER}" -srcfolder "$STAGE" -ov -format UDZO "$DMG_PATH"
rm -rf "$STAGE"

(cd "$OUT_DIR" && shasum -a 256 "$DMG_NAME" > "${DMG_NAME}.sha256")
ls -lh "$OUT_DIR"
