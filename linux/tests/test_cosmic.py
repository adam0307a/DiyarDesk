"""Unit tests for Linux COSMIC desktop support, backend selection & pause parsing."""

from __future__ import annotations

import json

from deskland_linux import pause as pause_mod
from deskland_linux.desktop_env import (
    COSMIC_IGNORED_CLASSES,
    is_cosmic_desktop,
    parse_lswt_fullscreen,
    select_wallpaper_backend,
)


def test_is_cosmic_desktop_detection() -> None:
    assert is_cosmic_desktop({"XDG_CURRENT_DESKTOP": "COSMIC"}) is True
    assert is_cosmic_desktop({"XDG_CURRENT_DESKTOP": "pop:COSMIC"}) is True
    assert is_cosmic_desktop({"DESKTOP_SESSION": "cosmic"}) is True
    assert is_cosmic_desktop({"COSMIC_DATA_CONTROL_ENABLED": "1"}) is True
    assert is_cosmic_desktop({"XDG_CURRENT_DESKTOP": "GNOME"}) is False
    assert is_cosmic_desktop({"XDG_CURRENT_DESKTOP": "KDE"}) is False


def test_select_wallpaper_backend_cosmic_and_other_desktops() -> None:
    # COSMIC Wayland uses native wlr-layer-shell when available, else static fallback
    assert (
        select_wallpaper_backend(
            "auto",
            {"WAYLAND_DISPLAY": "wayland-1", "XDG_CURRENT_DESKTOP": "COSMIC"},
            has_layer_shell=True,
        )
        == "layer-shell"
    )
    assert (
        select_wallpaper_backend(
            "auto",
            {"WAYLAND_DISPLAY": "wayland-1", "XDG_CURRENT_DESKTOP": "COSMIC"},
            has_layer_shell=False,
        )
        == "static"
    )

    # Existing desktop rules remain intact
    assert (
        select_wallpaper_backend(
            "auto",
            {"WAYLAND_DISPLAY": "wayland-0", "XDG_CURRENT_DESKTOP": "GNOME"},
            has_gnome_extension=True,
        )
        == "gnome-shell"
    )
    assert (
        select_wallpaper_backend(
            "auto",
            {"WAYLAND_DISPLAY": "wayland-0", "XDG_CURRENT_DESKTOP": "GNOME"},
            has_gnome_extension=False,
            has_layer_shell=True,
        )
        == "static"
    )
    assert (
        select_wallpaper_backend(
            "auto",
            {"WAYLAND_DISPLAY": "wayland-0", "XDG_CURRENT_DESKTOP": "KDE"},
            has_plasma_plugin=True,
        )
        == "plasma-plugin"
    )
    assert (
        select_wallpaper_backend(
            "auto",
            {"DISPLAY": ":0", "XDG_CURRENT_DESKTOP": "XFCE"},
        )
        == "static"
    )
    assert (
        select_wallpaper_backend(
            "auto",
            {"DISPLAY": ":0", "XDG_CURRENT_DESKTOP": "i3"},
        )
        == "x11"
    )


def test_parse_lswt_fullscreen_json_and_text() -> None:
    # Shell panels / cosmic-bg ignored even if marked maximized/fullscreen
    shell_only = json.dumps(
        {
            "toplevels": [
                {"app-id": "cosmic-panel", "maximized": True, "activated": True},
                {"app-id": "cosmic-bg", "fullscreen": True, "activated": True},
            ]
        }
    )
    assert parse_lswt_fullscreen(shell_only, COSMIC_IGNORED_CLASSES) is False

    # Active maximized app triggers pause; minimized app does not
    minimized = json.dumps(
        {
            "toplevels": [
                {"app-id": "firefox", "maximized": True, "activated": True, "minimized": True},
            ]
        }
    )
    assert parse_lswt_fullscreen(minimized, COSMIC_IGNORED_CLASSES) is False

    active_max = json.dumps(
        {
            "toplevels": [
                {"app-id": "firefox", "maximized": True, "activated": True, "minimized": False},
            ]
        }
    )
    assert parse_lswt_fullscreen(active_max, COSMIC_IGNORED_CLASSES) is True

    # Fullscreen game triggers pause
    fullscreen_app = json.dumps(
        {
            "toplevels": [
                {"app-id": "steam_app_123", "fullscreen": True, "activated": False},
            ]
        }
    )
    assert parse_lswt_fullscreen(fullscreen_app, COSMIC_IGNORED_CLASSES) is True

    # Plain-text lswt fallback
    assert parse_lswt_fullscreen("app-id: mpv state: fullscreen activated\n") is True
    assert parse_lswt_fullscreen("app-id: cosmic-osd state: fullscreen\n") is False


def test_pause_monitor_cosmic_uses_lswt(monkeypatch) -> None:
    class _DummyLog:
        def __init__(self) -> None:
            self.messages: list[str] = []

        def write(self, msg: str) -> None:
            self.messages.append(msg)

    monkeypatch.setenv("WAYLAND_DISPLAY", "wayland-1")
    monkeypatch.setenv("XDG_CURRENT_DESKTOP", "COSMIC")
    monkeypatch.delenv("SWAYSOCK", raising=False)
    monkeypatch.delenv("HYPRLAND_INSTANCE_SIGNATURE", raising=False)
    monkeypatch.delenv("DESKLAND_PAUSE_HELPER", raising=False)
    monkeypatch.setattr(pause_mod.shutil, "which", lambda name: "/usr/bin/lswt" if name == "lswt" else None)

    sample = json.dumps(
        {"toplevels": [{"app-id": "org.mozilla.firefox", "fullscreen": True, "activated": True}]}
    )
    monkeypatch.setattr(pause_mod, "_run", lambda cmd, timeout=1.5: sample)

    mon = pause_mod.PauseMonitor(0, 0, 1920, 1080, 1, _DummyLog())
    assert mon.should_pause() is True
