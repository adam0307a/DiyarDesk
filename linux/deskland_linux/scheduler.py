"""Solar day/night cycle calculator & local theme playlist scheduler.

100% offline: uses local wall-clock time only (no geolocation, no weather APIs).
"""
from __future__ import annotations

import datetime
import math
import random
import time
from typing import Any


def compute_solar_phase(
    hour: float | None = None,
    dark_mode: bool = False,
    solar_sync: bool = False,
) -> dict[str, Any]:
    """Compute a normalized environment payload from local hour (0..24)."""
    if hour is None:
        now = datetime.datetime.now()
        hour = now.hour + now.minute / 60.0
    h = float(hour) % 24.0
    if 5.0 <= h < 8.0:
        phase = "dawn"
    elif 8.0 <= h < 17.0:
        phase = "day"
    elif 17.0 <= h < 20.0:
        phase = "dusk"
    else:
        phase = "night"
    # Smooth sine curve peaking at 12:00 (1.0) and floored at 0.0 between 18:00..06:00
    daylight = max(0.0, min(1.0, math.sin(((h - 6.0) / 12.0) * math.pi)))
    return {
        "t": "environment",
        "hour": round(h, 2),
        "phase": phase,
        "daylight": round(daylight, 3),
        "darkMode": bool(dark_mode),
        "solarSync": bool(solar_sync),
    }


def detect_os_dark_mode() -> bool:
    """Best-effort local OS dark-theme detection on Linux (XDG/GTK/KDE)."""
    import os
    import shutil
    import subprocess

    if os.environ.get("DESKLAND_DARK_MODE") in ("1", "true", "yes"):
        return True
    if shutil.which("gsettings"):
        try:
            out = subprocess.run(
                ["gsettings", "get", "org.gnome.desktop.interface", "color-scheme"],
                capture_output=True, text=True, timeout=0.6, check=False,
            ).stdout.lower()
            if "dark" in out:
                return True
            if "light" in out or "default" in out:
                return False
        except (OSError, subprocess.SubprocessError):
            pass
    gtk_theme = os.environ.get("GTK_THEME", "").lower()
    if "dark" in gtk_theme:
        return True
    return False


class PlaylistController:
    """Deterministic or shuffled local theme rotation scheduler."""

    def __init__(self, interval_minutes: int = 30, themes: list[str] | None = None) -> None:
        self.interval_minutes = max(1, min(1440, int(interval_minutes)))
        self.themes: list[str] = list(themes) if themes else []
        self._last_switch: float = time.monotonic()

    def configure(self, interval_minutes: int, themes: list[str] | None = None) -> None:
        self.interval_minutes = max(1, min(1440, int(interval_minutes)))
        if themes is not None:
            self.themes = [t for t in themes if isinstance(t, str) and t]

    def mark_switched(self, now: float | None = None) -> None:
        self._last_switch = time.monotonic() if now is None else float(now)

    def should_rotate(self, now: float | None = None) -> bool:
        t = time.monotonic() if now is None else float(now)
        return (t - self._last_switch) >= (self.interval_minutes * 60.0)

    def next_theme(self, ready_ids: list[str], current_id: str) -> str | None:
        pool = [tid for tid in self.themes if tid in ready_ids] if self.themes else list(ready_ids)
        if not pool:
            return None
        if len(pool) == 1:
            return pool[0]
        try:
            idx = pool.index(current_id)
            return pool[(idx + 1) % len(pool)]
        except ValueError:
            return pool[0]

    @staticmethod
    def pick_random(ready_ids: list[str], current_id: str, rng: random.Random | None = None) -> str | None:
        if not ready_ids:
            return None
        choices = [tid for tid in ready_ids if tid != current_id] or list(ready_ids)
        r = rng if rng is not None else random
        return r.choice(choices)
