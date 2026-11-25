#!/usr/bin/env bash
# DiyarDesk — Tek komutla yerel Linux güncelleme betiği
# Kullanım:
#   ./linux/update-local.sh          # Yerel değişiklikleri derleyip sisteme kurar ve yeniden başlatır
#   ./linux/update-local.sh --pull   # Önce git pull yapar, sonra derleyip kurar
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

if [ "${1:-}" = "--pull" ]; then
  echo "[1/5] GitHub'dan son değişiklikler çekiliyor..."
  git pull --ff-only
fi

echo "[2/5] Rust + OpenGL 3.3 motoru derleniyor (libdiyardesk_gl.so)..."
cargo build --release --locked --manifest-path native/diyardesk-gl/Cargo.toml
mkdir -p linux/lib
cp native/diyardesk-gl/target/release/libdiyardesk_gl.so linux/lib/libdiyardesk_gl.so

echo "[3/5] Sistem dosyaları güncelleniyor (/usr/lib/deskland ve GNOME eklentisi)..."
sudo bash -euo pipefail -c "
  install -d /usr/lib/deskland/linux/deskland_linux /usr/lib/deskland/prototype /usr/share/gnome-shell/extensions/deskland@deskland.app
  install -m 755 '$ROOT_DIR/linux/lib/libdiyardesk_gl.so' /usr/lib/deskland/libdiyardesk_gl.so
  rm -rf /usr/lib/deskland/linux/deskland_linux/*
  cp -a '$ROOT_DIR/linux/deskland_linux/.' /usr/lib/deskland/linux/deskland_linux/
  cp -a '$ROOT_DIR/prototype/.' /usr/lib/deskland/prototype/
  cp -a '$ROOT_DIR/linux/gnome-extension/deskland@deskland.app/.' /usr/share/gnome-shell/extensions/deskland@deskland.app/
  chown -R root:root /usr/lib/deskland /usr/share/gnome-shell/extensions/deskland@deskland.app
  find /usr/lib/deskland/linux/deskland_linux -type d -name '__pycache__' -prune -exec rm -rf {} +
  find /usr/lib/deskland/linux/deskland_linux -name '*.pyc' -delete
"

echo "[4/5] Eski DiyarDesk örneği durduruluyor..."
systemctl --user stop diyardesk-session.service 2>/dev/null || true
systemctl --user reset-failed diyardesk-session.service 2>/dev/null || true
pkill -f "python3 -m deskland_linux" 2>/dev/null || true
sleep 0.6

echo "[5/5] Güncel DiyarDesk başlatılıyor..."
if command -v systemd-run >/dev/null 2>&1; then
  systemd-run --user --unit=diyardesk-session --collect /usr/bin/diyardesk --autostart >/dev/null
else
  nohup /usr/bin/diyardesk --autostart >/dev/null 2>&1 &
fi

sleep 1.5
if command -v diyardesk-ctl >/dev/null 2>&1; then
  diyardesk-ctl status || true
fi
echo "Tamamlandı! DiyarDesk güncellendi ve çalışıyor."
