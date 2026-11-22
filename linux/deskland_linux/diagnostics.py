"""Privacy-safe diagnostic report generator and exporter for Linux."""

from __future__ import annotations

import os
import platform
import time
from pathlib import Path
from typing import Any

from .config import APP_NAME, VERSION, config_dir, log_path
from .gpu import _connected_gpu_vendors
from .power import PowerState


def sanitize_diagnostic_text(text: str, home_dir: Path | None = None) -> str:
    """Mask the user's home directory path to avoid leaking OS usernames."""
    home = str((home_dir or Path.home()).resolve())
    if not home:
        return text
    return text.replace(home, "~")


def _tail_lines(path: Path, limit: int = 200) -> list[str]:
    try:
        if not path.is_file():
            return ["(log file does not exist yet)"]
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        return lines[-limit:]
    except OSError as exc:
        return [f"(could not read log file: {exc})"]


def collect_diagnostics(
    *,
    settings: Any = None,
    backend: str = "unknown",
    screen: tuple[int, int, int] | None = None,
    power_state: PowerState | None = None,
    log_file: Path | None = None,
    home_dir: Path | None = None,
) -> str:
    """Build a human-readable, privacy-safe diagnostics text report."""
    env = os.environ
    drm_vendors = sorted(_connected_gpu_vendors(Path("/sys/class/drm")))
    lines = [
        f"=== {APP_NAME} Diagnostics Report ===",
        f"Generated (UTC): {time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())}",
        f"App Version:     {VERSION}",
        f"Platform:        {platform.platform()}",
        f"Python:          {platform.python_version()}",
        f"Desktop:         {env.get('XDG_CURRENT_DESKTOP') or '(unset)'} / session={env.get('XDG_SESSION_TYPE') or '(unset)'}",
        f"Display:         WAYLAND_DISPLAY={bool(env.get('WAYLAND_DISPLAY'))}, DISPLAY={bool(env.get('DISPLAY'))}",
        f"Wallpaper:       backend={backend}, screen={screen if screen else '(unknown)'}",
        f"GPU Vendors:     {', '.join(drm_vendors) if drm_vendors else '(none detected)'}",
        f"EGL Vendor Env:  {env.get('__EGL_VENDOR_LIBRARY_FILENAMES') or '(default)'}",
        f"DMABUF Disabled: {env.get('WEBKIT_DISABLE_DMABUF_RENDERER') or '0'}",
    ]
    if settings is not None:
        lines.extend(
            [
                f"Active Theme:    {getattr(settings, 'theme', '(unknown)')}",
                f"Language:        {getattr(settings, 'lang', '(unknown)')}",
                f"Low Mode:        {bool(getattr(settings, 'low_mode', False))}",
                f"Auto-Pause Off:  {bool(getattr(settings, 'disable_auto_pause', False))}",
                f"Disable DMABUF:  {bool(getattr(settings, 'disable_dmabuf', False))}",
            ]
        )
    if power_state is not None:
        lines.append(
            "Power State:     "
            f"locked={power_state.locked}, on_battery={power_state.on_battery}, "
            f"power_saver={power_state.power_saver}, reduced_motion={power_state.reduced_motion}"
        )
    lines.append("")
    lines.append("=== Recent host.log (last 200 lines) ===")
    lines.extend(_tail_lines(log_file or log_path(), limit=200))
    return sanitize_diagnostic_text("\n".join(lines) + "\n", home_dir=home_dir)


def export_diagnostics(
    *,
    output_dir: Path | None = None,
    settings: Any = None,
    backend: str = "unknown",
    screen: tuple[int, int, int] | None = None,
    power_state: PowerState | None = None,
    log_file: Path | None = None,
    home_dir: Path | None = None,
) -> Path:
    """Write diagnostics report to disk and return the saved file path."""
    target_dir = output_dir or (config_dir() / "diagnostics")
    target_dir.mkdir(parents=True, exist_ok=True)
    stamp = time.strftime("%Y%m%d-%H%M%S")
    out_path = target_dir / f"diyardesk-diagnostics-{stamp}.txt"
    report = collect_diagnostics(
        settings=settings,
        backend=backend,
        screen=screen,
        power_state=power_state,
        log_file=log_file,
        home_dir=home_dir,
    )
    out_path.write_text(report, encoding="utf-8")
    return out_path
