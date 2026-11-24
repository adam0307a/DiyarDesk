"""Startup crash sentinel (Safe Mode) for Linux host.

Pure Python (no GTK imports) so it stays 100% unit-testable on any platform.
If DiyarDesk fails to stay up for ``DEFAULT_STABLE_SECONDS`` on two consecutive
launches within ``DEFAULT_WINDOW_SECONDS`` (for example due to a broken custom
WebGL shader or GPU/WebKit crash), the next launch automatically falls back to
a known-safe built-in theme and saves settings so the user is never trapped in
a startup crash loop.
"""

from __future__ import annotations

import json
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from .config import Log, SettingsStore, atomic_write, config_dir

DEFAULT_CRASH_THRESHOLD = 2
DEFAULT_WINDOW_SECONDS = 180
DEFAULT_STABLE_SECONDS = 12
SAFE_FALLBACK_ORDER = ("papercut", "meadow", "beach", "software-world")


@dataclass(frozen=True)
class CrashSentinelResult:
    fell_back: bool
    previous_theme: str
    active_theme: str
    consecutive_crashes: int


class StartupCrashSentinel:
    """Tracks unfinished launches via ``startup.sentinel`` inside config_dir."""

    def __init__(
        self,
        directory: Path | None = None,
        *,
        crash_threshold: int = DEFAULT_CRASH_THRESHOLD,
        window_seconds: int = DEFAULT_WINDOW_SECONDS,
        clock: Callable[[], float] | None = None,
    ) -> None:
        self.directory = directory or config_dir()
        self.path = self.directory / "startup.sentinel"
        self.crash_threshold = max(1, int(crash_threshold))
        self.window_seconds = max(1, int(window_seconds))
        self._clock = clock or time.time

    def _read_state(self) -> tuple[int, int, str]:
        try:
            if not self.path.exists():
                return 0, 0, ""
            raw = self.path.read_text(encoding="utf-8")
            data = json.loads(raw)
            if not isinstance(data, dict):
                return 0, 0, ""
            count = int(data.get("count", 0))
            ts = int(data.get("ts", 0))
            theme = data.get("theme")
            return max(0, count), max(0, ts), theme if isinstance(theme, str) else ""
        except (OSError, ValueError, TypeError):
            return 0, 0, ""

    def _write_state(self, count: int, ts: int, theme: str) -> None:
        try:
            payload = json.dumps(
                {"count": int(count), "ts": int(ts), "theme": str(theme or "")},
                ensure_ascii=False,
            ).encode("utf-8")
            atomic_write(self.path, payload)
        except OSError:
            pass

    def begin_startup(
        self,
        current_theme: str,
        *,
        safe_theme_resolver: Callable[[], str] | None = None,
    ) -> CrashSentinelResult:
        now = int(self._clock())
        prev_count, prev_ts, prev_theme = self._read_state()
        elapsed = now - prev_ts
        within_window = prev_ts > 0 and 0 <= elapsed <= self.window_seconds

        if within_window and prev_count >= self.crash_threshold:
            safe_theme = (safe_theme_resolver() if safe_theme_resolver is not None else "") or "meadow"
            old_theme = current_theme or prev_theme
            fell_back = old_theme.lower() != safe_theme.lower()
            self._write_state(1, now, safe_theme)
            return CrashSentinelResult(
                fell_back=fell_back,
                previous_theme=old_theme,
                active_theme=safe_theme,
                consecutive_crashes=prev_count,
            )

        next_count = (prev_count + 1) if within_window else 1
        self._write_state(next_count, now, current_theme)
        return CrashSentinelResult(
            fell_back=False,
            previous_theme=current_theme,
            active_theme=current_theme,
            consecutive_crashes=prev_count if within_window else 0,
        )

    def mark_stable(self) -> bool:
        try:
            if self.path.exists():
                self.path.unlink()
                return True
        except OSError:
            pass
        return False

    def apply_to_settings(
        self,
        settings: SettingsStore,
        *,
        safe_theme_resolver: Callable[[], str] | None = None,
        log: Log | None = None,
    ) -> CrashSentinelResult:
        result = self.begin_startup(settings.theme, safe_theme_resolver=safe_theme_resolver)
        if result.fell_back:
            settings.theme = result.active_theme
            settings.save()
            if log is not None:
                log.write(
                    f"Safe Mode: {result.consecutive_crashes} ardışık açılış çökmesi tespit edildi "
                    f"(tema '{result.previous_theme}'), yerleşik güvenli temaya dönüldü: '{result.active_theme}'"
                )
        elif result.consecutive_crashes >= self.crash_threshold and log is not None:
            log.write(
                f"Safe Mode: '{result.active_theme}' temasında {result.consecutive_crashes} ardışık açılış çökmesi "
                "tespit edildi; çökme sayacı sıfırlandı"
            )
        return result
