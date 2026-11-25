#!/bin/sh
# Build a Debian package without extra tooling. Run from the repository root:
#   sh linux/packaging/build-deb.sh
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
LINUX=$(CDPATH= cd -- "$HERE/.." && pwd)
ROOT=$(CDPATH= cd -- "$LINUX/.." && pwd)
VERSION="$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['version'] + '~linux1')" "$ROOT/tools/version.json" 2>/dev/null || echo "0.5.4~linux1")"
NATIVE_GL_SO=""
if [ -f "$ROOT/native/diyardesk-gl/target/release/libdiyardesk_gl.so" ]; then
  NATIVE_GL_SO="$ROOT/native/diyardesk-gl/target/release/libdiyardesk_gl.so"
elif [ -f "$LINUX/lib/libdiyardesk_gl.so" ]; then
  NATIVE_GL_SO="$LINUX/lib/libdiyardesk_gl.so"
fi
if [ -n "$NATIVE_GL_SO" ]; then
  ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
else
  ARCH="all"
fi
PKG="diyardesk"
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

install -d "$STAGE/DEBIAN"
install -d "$STAGE/usr/lib/deskland/linux/deskland_linux"
install -d "$STAGE/usr/lib/deskland/prototype"
install -d "$STAGE/usr/bin"
install -d "$STAGE/usr/share/applications"
install -d "$STAGE/usr/share/icons/hicolor/scalable/apps"
install -d "$STAGE/usr/share/plasma/wallpapers"
install -d "$STAGE/usr/share/plasma/plasmoids"
install -d "$STAGE/usr/share/gnome-shell/extensions"
install -d "$STAGE/usr/share/doc/$PKG"

cp -a "$LINUX/deskland_linux/." "$STAGE/usr/lib/deskland/linux/deskland_linux/"
cp -a "$ROOT/prototype/." "$STAGE/usr/lib/deskland/prototype/"
if [ -n "$NATIVE_GL_SO" ]; then
  install -m 755 "$NATIVE_GL_SO" "$STAGE/usr/lib/deskland/libdiyardesk_gl.so"
fi
cp -a "$LINUX/plasma-wallpaper/org.deskland.preview" "$STAGE/usr/share/plasma/wallpapers/"
cp -a "$LINUX/kde-iconspy/org.deskland.iconspy" "$STAGE/usr/share/plasma/plasmoids/"
cp -a "$LINUX/gnome-extension/deskland@deskland.app" "$STAGE/usr/share/gnome-shell/extensions/"
# Compile gettext catalogs (msgfmt) into the system locale tree when present;
# system extensions bind to Config.LOCALEDIR when no locale/ subfolder is shipped.
if [ -f "$LINUX/gnome-extension/build-mo.sh" ]; then
  sh "$LINUX/gnome-extension/build-mo.sh" "$STAGE/usr/share/locale"
fi
rm -rf "$STAGE/usr/share/gnome-shell/extensions/deskland@deskland.app/locale" \
       "$STAGE/usr/share/gnome-shell/extensions/deskland@deskland.app/po"
install -m 644 "$HERE/deskland.desktop" "$STAGE/usr/share/applications/deskland.desktop"
install -m 644 "$HERE/deskland.svg" "$STAGE/usr/share/icons/hicolor/scalable/apps/deskland.svg"
install -m 644 "$ROOT/LICENSE" "$STAGE/usr/share/doc/$PKG/copyright"

cat > "$STAGE/usr/bin/deskland-linux" <<'EOF'
#!/bin/sh
export PYTHONPATH="/usr/lib/deskland/linux${PYTHONPATH:+:$PYTHONPATH}"
export DESKLAND_PROTOTYPE="${DESKLAND_PROTOTYPE:-/usr/lib/deskland/prototype}"
exec python3 -m deskland_linux "$@"
EOF
chmod 755 "$STAGE/usr/bin/deskland-linux"
ln -sf /usr/bin/deskland-linux "$STAGE/usr/bin/diyardesk"
install -m 755 "$LINUX/bin/diyardesk-ctl" "$STAGE/usr/bin/diyardesk-ctl"

cat > "$STAGE/DEBIAN/control" <<EOF
Package: $PKG
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Replaces: deskland-preview
Conflicts: deskland-preview
Maintainer: DiyarDesk contributors
Depends: python3 (>= 3.10), python3-gi, python3-gi-cairo, gir1.2-gtk-3.0, gir1.2-webkit2-4.0 | gir1.2-webkit2-4.1, gir1.2-ayatanaappindicator3-0.1 | gir1.2-appindicator3-0.1, libayatana-appindicator3-1 | libappindicator3-1, xdg-utils
Recommends: gir1.2-gtklayershell-0.1, xdotool, gstreamer1.0-plugins-good
Description: Adaptive wallpaper that follows desktop icons
 DiyarDesk (based on Deskland Preview) renders a WebGL/Canvas scene behind the
 desktop and reshapes it around the real positions of desktop icons. Includes
 adaptive themes, custom theme import, tray controls, session autostart, KDE
 Plasma 6 integration and privacy-safe sharing.
EOF

install -d "$ROOT/dist"
OUT="$ROOT/dist/${PKG}_${VERSION}_${ARCH}.deb"
if command -v dpkg-deb >/dev/null 2>&1; then
  dpkg-deb --build --root-owner-group "$STAGE" "$OUT"
else
  DEB_TMP=$(mktemp -d)
  printf '2.0\n' > "$DEB_TMP/debian-binary"
  tar --numeric-owner --owner=0 --group=0 -C "$STAGE/DEBIAN" -czf "$DEB_TMP/control.tar.gz" .
  tar --numeric-owner --owner=0 --group=0 --exclude='./DEBIAN' -C "$STAGE" -czf "$DEB_TMP/data.tar.gz" .
  rm -f "$OUT"
  (cd "$DEB_TMP" && ar rc "$OUT" debian-binary control.tar.gz data.tar.gz)
  rm -rf "$DEB_TMP"
fi
echo "built: $OUT"
