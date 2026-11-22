"""Local blurred static lockscreen image sync for Linux desktops.

Takes a rendered PNG snapshot of the active wallpaper, creates a softly blurred
lockscreen copy under `~/.cache/deskland/lockscreen.png` (using `ffmpeg` or
ImageMagick `convert`/`magick` if available, else direct copy), and applies it
to GNOME (`org.gnome.desktop.screensaver`) or KDE (`kwriteconfig6 kscreenlockerrc`).
"""
from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path


def lockscreen_image_path() -> Path:
    cache = Path(os.environ.get("XDG_CACHE_HOME") or (Path.home() / ".cache"))
    return cache / "deskland" / "lockscreen.png"


def make_blurred_lockscreen(source_png: Path, dest_png: Path | None = None) -> Path:
    out = dest_png or lockscreen_image_path()
    out.parent.mkdir(parents=True, exist_ok=True)
    if not source_png.is_file():
        raise FileNotFoundError(f"Source wallpaper frame not found: {source_png}")

    tmp = out.with_suffix(".tmp.png")
    applied_blur = False
    if shutil.which("ffmpeg"):
        proc = subprocess.run(
            ["ffmpeg", "-y", "-i", str(source_png), "-vf", "gblur=sigma=14", str(tmp)],
            capture_output=True,
            timeout=10.0,
            check=False,
        )
        applied_blur = proc.returncode == 0 and tmp.is_file() and tmp.stat().st_size > 0
    if not applied_blur and shutil.which("magick"):
        proc = subprocess.run(
            ["magick", str(source_png), "-blur", "0x12", str(tmp)],
            capture_output=True,
            timeout=10.0,
            check=False,
        )
        applied_blur = proc.returncode == 0 and tmp.is_file() and tmp.stat().st_size > 0
    if not applied_blur and shutil.which("convert"):
        proc = subprocess.run(
            ["convert", str(source_png), "-blur", "0x12", str(tmp)],
            capture_output=True,
            timeout=10.0,
            check=False,
        )
        applied_blur = proc.returncode == 0 and tmp.is_file() and tmp.stat().st_size > 0

    if applied_blur:
        os.replace(tmp, out)
    else:
        if tmp.exists():
            try:
                tmp.unlink()
            except OSError:
                pass
        shutil.copy2(source_png, out)
    return out


def apply_linux_lockscreen(image_path: Path) -> str:
    uri = image_path.resolve().as_uri()
    desktop = (os.environ.get("XDG_CURRENT_DESKTOP", "") + " " + os.environ.get("DESKTOP_SESSION", "")).lower()

    if "gnome" in desktop and shutil.which("gsettings"):
        r = subprocess.run(
            ["gsettings", "set", "org.gnome.desktop.screensaver", "picture-uri", uri],
            capture_output=True,
            timeout=3.0,
            check=False,
        )
        if r.returncode == 0:
            return "gnome-screensaver"

    if ("kde" in desktop or "plasma" in desktop):
        kw = shutil.which("kwriteconfig6") or shutil.which("kwriteconfig5")
        if kw:
            subprocess.run(
                [kw, "--file", "kscreenlockerrc", "--group", "Greeter", "--group", "Wallpaper",
                 "--group", "org.kde.image", "--group", "General", "--key", "Image", uri],
                capture_output=True,
                timeout=3.0,
                check=False,
            )
            return "kde-kscreenlocker"

    return "saved-file"
