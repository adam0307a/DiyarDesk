"""Pure-Python Linux desktop environment detection, backend selection & COSMIC helpers.

Kept free of GTK/GI imports so backend selection and Wayland compositor helpers
remain 100% unit-testable on any platform.
"""

from __future__ import annotations

import json
import os
from typing import Iterable, Mapping

COSMIC_PANEL_INSETS: tuple[int, int, int, int] = (36, 0, 0, 0)

COSMIC_IGNORED_CLASSES: tuple[str, ...] = (
    "cosmic-comp",
    "cosmic-panel",
    "cosmic-bg",
    "cosmic-osd",
    "cosmic-launcher",
    "cosmic-workspaces",
)


def desktop_env_string(env: Mapping[str, str] | None = None) -> str:
    src = env if env is not None else os.environ
    return (
        src.get("XDG_CURRENT_DESKTOP", "") + " " + src.get("DESKTOP_SESSION", "")
    ).lower()


def is_cosmic_desktop(env: Mapping[str, str] | None = None) -> bool:
    src = env if env is not None else os.environ
    desktop = desktop_env_string(src)
    if "cosmic" in desktop:
        return True
    return bool(src.get("COSMIC_DATA_CONTROL_ENABLED"))


def select_wallpaper_backend(
    requested: str,
    env: Mapping[str, str] | None = None,
    *,
    has_plasma_plugin: bool = False,
    has_gnome_extension: bool = False,
    has_layer_shell: bool = False,
) -> str:
    """Choose the best wallpaper backend for the current Linux desktop session."""
    if requested in ("layer-shell", "gnome-shell", "x11", "static", "plasma-plugin"):
        return requested

    src = env if env is not None else os.environ
    wayland = bool(src.get("WAYLAND_DISPLAY"))
    desktop_env = desktop_env_string(src)

    if ("kde" in desktop_env or "plasma" in desktop_env) and has_plasma_plugin:
        return "plasma-plugin"

    if wayland:
        if "gnome" in desktop_env and has_gnome_extension:
            return "gnome-shell"
        # COSMIC (cosmic-comp) implements wlr-layer-shell natively.
        if is_cosmic_desktop(src):
            return "layer-shell" if has_layer_shell else "static"
        # KWin and GNOME Shell (without our extension) keep their own
        # desktop/containment surface above normal layers and Mutter has no
        # wlr-layer-shell support. Fall back to static rather than XWayland.
        if "kde" in desktop_env or "plasma" in desktop_env or "gnome" in desktop_env:
            return "static"
        if has_layer_shell:
            return "layer-shell"
        return "static"

    if src.get("DISPLAY"):
        # xfdesktop renders both wallpaper and icons inside one opaque DESKTOP
        # window, so an X11 window either covers icons or hides behind it.
        if "xfce" in desktop_env:
            return "static"
        return "x11"

    return "static"


def parse_lswt_fullscreen(
    output: str,
    ignored_classes: Iterable[str] = COSMIC_IGNORED_CLASSES,
    *,
    low_mode: bool = False,
) -> bool:
    """Parse ``lswt -j`` (JSON) or plain ``lswt`` output for active fullscreen/maximized windows."""
    text = (output or "").strip()
    if not text:
        return False
    ignored = tuple(c.lower() for c in ignored_classes)

    if text.startswith("{") or text.startswith("["):
        try:
            data = json.loads(text)
        except ValueError:
            return False
        toplevels = data.get("toplevels") if isinstance(data, dict) else data
        if not isinstance(toplevels, list):
            return False
        for item in toplevels:
            if not isinstance(item, dict):
                continue
            app_id = str(item.get("app-id") or item.get("app_id") or "").lower()
            if app_id and any(ign in app_id for ign in ignored):
                continue
            if item.get("minimized"):
                continue
            fullscreen = bool(item.get("fullscreen"))
            maximized = bool(item.get("maximized"))
            activated = bool(item.get("activated") or item.get("focused"))
            if fullscreen:
                return True
            if maximized and (activated or low_mode):
                return True
        return False

    # Plain-text lswt output fallback (one window per line or block)
    for line in text.splitlines():
        low = line.lower()
        if any(ign in low for ign in ignored):
            continue
        if "minimized" in low:
            continue
        if "fullscreen" in low:
            return True
        if "maximized" in low and ("activated" in low or "focused" in low or low_mode):
            return True
    return False
