"""Portable `.deskland-preset` file serialization, validation, and application helpers.

Pure Python (no GTK dependency) so it can be unit-tested on any OS.
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Callable

from .config import SettingsStore

MAX_PRESET_BYTES = 262_144


def build_preset_payload(
    settings: SettingsStore, theme_name: str | None = None
) -> dict[str, Any]:
    tid = settings.theme or "papercut"
    return {
        "format": "diyardesk-preset",
        "version": 1,
        "name": theme_name or tid,
        "theme": tid,
        "params": dict(settings.for_theme(tid)),
        "solarSync": bool(settings.solar_sync),
        "windowAware": bool(settings.window_aware),
        "lowMode": bool(settings.low_mode),
    }


def validate_preset_payload(
    raw: Any, allowed_theme: Callable[[str], bool]
) -> dict[str, Any]:
    if not isinstance(raw, dict):
        raise ValueError("Preset root must be a JSON object")
    theme_id = raw.get("theme")
    if not isinstance(theme_id, str) or not theme_id.strip():
        raise ValueError("Missing theme id")
    tid = theme_id.strip()
    if not allowed_theme(tid):
        raise ValueError(f"Unknown or unavailable theme: {tid}")

    params_in = raw.get("params")
    clean_params: dict[str, Any] = {}
    if isinstance(params_in, dict):
        for k, v in params_in.items():
            if isinstance(k, str) and k and isinstance(v, (bool, int, float, str)):
                clean_params[k] = v

    name_val = raw.get("name")
    out: dict[str, Any] = {
        "format": "diyardesk-preset",
        "version": int(raw.get("version") or 1)
        if isinstance(raw.get("version"), (int, float))
        else 1,
        "name": name_val.strip()
        if isinstance(name_val, str) and name_val.strip()
        else tid,
        "theme": tid,
        "params": clean_params,
    }
    for flag in ("solarSync", "windowAware", "lowMode"):
        if isinstance(raw.get(flag), bool):
            out[flag] = raw[flag]
    return out


def load_preset_file(
    path: Path, allowed_theme: Callable[[str], bool]
) -> dict[str, Any]:
    target = Path(path)
    if not target.is_file():
        raise FileNotFoundError(f"Preset file not found: {target}")
    size = target.stat().st_size
    if size > MAX_PRESET_BYTES:
        raise ValueError(f"Preset file exceeds {MAX_PRESET_BYTES} bytes")
    data = json.loads(target.read_text(encoding="utf-8"))
    return validate_preset_payload(data, allowed_theme)


def save_preset_file(path: Path, payload: dict[str, Any]) -> Path:
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    return target


def apply_preset_to_settings(
    settings: SettingsStore, preset: dict[str, Any]
) -> dict[str, Any]:
    tid = str(preset["theme"])
    pmap = settings.for_theme(tid)
    params = preset.get("params")
    if isinstance(params, dict):
        for k, v in params.items():
            pmap[k] = v
    if isinstance(preset.get("solarSync"), bool):
        settings.solar_sync = bool(preset["solarSync"])
    if isinstance(preset.get("windowAware"), bool):
        settings.window_aware = bool(preset["windowAware"])
    if isinstance(preset.get("lowMode"), bool):
        settings.low_mode = bool(preset["lowMode"])
    settings.theme = tid
    return {
        "theme": tid,
        "name": str(preset.get("name") or tid),
        "solarSync": settings.solar_sync,
        "windowAware": settings.window_aware,
        "lowMode": settings.low_mode,
    }
