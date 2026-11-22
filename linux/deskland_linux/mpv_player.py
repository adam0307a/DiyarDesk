"""Optional hardware-accelerated `mpv` video wallpaper backend for Linux (VA-API/NVDEC).

When enabled (`prefer_mpv=True`) and `mpv` is installed, imported video themes
(`.mp4`, `.webm`, `.mkv`, `.mov`) can play via hardware decoding with near-zero CPU
instead of WebView `<video>`. Safely falls back to WebView if `mpv` is absent.
"""
from __future__ import annotations

import shutil
import subprocess
from pathlib import Path
from typing import Any


VIDEO_EXTS = {".mp4", ".webm", ".mkv", ".mov"}


def is_mpv_available() -> bool:
    return shutil.which("mpv") is not None


def find_theme_video_file(theme_dir: Path) -> Path | None:
    if not theme_dir.is_dir():
        return None
    for child in sorted(theme_dir.iterdir()):
        if child.is_file() and child.suffix.lower() in VIDEO_EXTS:
            return child
    return None


def build_mpv_command(
    video_path: Path,
    wid: int | None = None,
    speed: float = 1.0,
    brightness: float = 1.0,
) -> list[str]:
    spd = max(0.25, min(2.0, float(speed)))
    # mpv brightness is -100..100 (0 = neutral); map 0.4..1.4 -> -35..+25
    mpv_bright = int(round((max(0.4, min(1.4, float(brightness))) - 1.0) * 60.0))
    cmd = [
        "mpv",
        "--no-audio",
        "--loop-file=inf",
        "--hwdec=auto-safe",
        "--no-osc",
        "--no-osd-bar",
        "--no-input-default-bindings",
        "--no-input-cursor",
        "--really-quiet",
        "--keep-open=yes",
        f"--speed={spd:.2f}",
        f"--brightness={mpv_bright}",
    ]
    if wid is not None and wid > 0:
        cmd.append(f"--wid={int(wid)}")
    cmd.append(str(video_path))
    return cmd


class MpvVideoBackend:
    """Manages an optional `mpv` subprocess attached to a wallpaper window ID."""

    def __init__(self, log: Any = None) -> None:
        self._log = log
        self._proc: subprocess.Popen[bytes] | None = None
        self._current_video: Path | None = None

    @property
    def active(self) -> bool:
        return self._proc is not None and self._proc.poll() is None

    def start(self, video_path: Path, wid: int | None = None, speed: float = 1.0, brightness: float = 1.0) -> bool:
        self.stop()
        if not is_mpv_available() or not video_path.is_file():
            return False
        cmd = build_mpv_command(video_path, wid=wid, speed=speed, brightness=brightness)
        try:
            self._proc = subprocess.Popen(
                cmd,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            self._current_video = video_path
            if self._log is not None:
                self._log.write(f"mpv donanım video arka ucu başlatıldı: {video_path.name}")
            return True
        except OSError as exc:
            if self._log is not None:
                self._log.exception("mpv başlatılamadı; WebView oynatıcıya dönülüyor", exc)
            self._proc = None
            return False

    def stop(self) -> None:
        proc = self._proc
        self._proc = None
        self._current_video = None
        if proc is None:
            return
        try:
            if proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(timeout=1.5)
                except subprocess.TimeoutExpired:
                    proc.kill()
        except OSError:
            pass
