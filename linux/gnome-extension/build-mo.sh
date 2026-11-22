#!/bin/sh
# Compile the GNOME Shell bridge gettext catalogs from po/ to .mo.
#
# Usage: build-mo.sh [OUTPUT_LOCALE_DIR]
#   OUTPUT_LOCALE_DIR defaults to deskland@deskland.app/locale.
#
# Requires msgfmt from gettext.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
EXT="$HERE/deskland@deskland.app"
OUT=${1:-"$EXT/locale"}

if ! command -v msgfmt >/dev/null 2>&1; then
  echo "build-mo.sh: msgfmt not found; install gettext" >&2
  exit 1
fi

for po in "$EXT"/po/*.po; do
  [ -e "$po" ] || continue
  lang=$(basename "$po" .po)
  install -d "$OUT/$lang/LC_MESSAGES"
  msgfmt -c -o "$OUT/$lang/LC_MESSAGES/deskland.mo" "$po"
done
