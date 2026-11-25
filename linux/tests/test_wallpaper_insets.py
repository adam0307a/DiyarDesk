"""Tests for layer-shell safe perimeter inset detection."""

from __future__ import annotations

from unittest.mock import MagicMock

from deskland_linux import wallpaper as wallpaper_mod
from deskland_linux.pause import PauseMonitor
from deskland_linux.wallpaper import (
    Wallpaper,
    _build_safe_inset_script,
    _is_x11_display,
    compute_caelestia_insets,
    detect_layer_shell_safe_insets,
    load_layer_shell,
)


def test_compute_caelestia_insets_defaults_and_custom_thresholds() -> None:
    # Default: border=10, sidebar=80, +8 buffer -> 98 on top/right/bottom, +48 left
    assert compute_caelestia_insets({}) == (98, 98, 98, 146)

    custom = {
        "border": {"thickness": 12},
        "sidebar": {"enabled": True, "dragThreshold": 60},
        "dashboard": {"enabled": False, "dragThreshold": 120},
        "launcher": {"enabled": True, "dragThreshold": 40},
        "session": {"enabled": True, "dragThreshold": 30},
    }
    assert compute_caelestia_insets(custom) == (80, 80, 80, 128)


def test_detect_layer_shell_safe_insets_backends_and_layers(monkeypatch) -> None:
    monkeypatch.delenv("DESKLAND_SAFE_INSETS", raising=False)
    sample_layers = {
        "eDP-1": {
            "levels": {
                "0": [{"namespace": "deskland"}],
                "2": [{"namespace": "caelestia-drawers"}],
            }
        }
    }

    assert detect_layer_shell_safe_insets("x11", layers_data=sample_layers) == (0, 0, 0, 0)
    assert detect_layer_shell_safe_insets("static", layers_data=sample_layers) == (0, 0, 0, 0)
    assert detect_layer_shell_safe_insets(
        "layer-shell", layers_data=sample_layers, caelestia_cfg={}
    ) == (98, 98, 98, 146)

    no_caelestia = {"eDP-1": {"levels": {"2": [{"namespace": "waybar"}]}}}
    assert detect_layer_shell_safe_insets("layer-shell", layers_data=no_caelestia) == (0, 0, 0, 0)

    monkeypatch.setenv("DESKLAND_SAFE_INSETS", "10,20,30,40")
    assert detect_layer_shell_safe_insets("layer-shell", layers_data=no_caelestia) == (10, 20, 30, 40)


def test_safe_inset_script_marks_window_and_skips_opt_out() -> None:
    script = _build_safe_inset_script((98, 98, 98, 146))
    assert "window.__desklandSafeInsets" in script
    assert "desklandSafe" in script
    assert '"bottom": 98' in script


def test_static_and_plasma_stay_paused_while_x11_and_layer_shell_run_live() -> None:
    for backend, expected_pause in (
        ("static", True),
        ("plasma-plugin", True),
        ("x11", False),
        ("layer-shell", False),
        ("gnome-shell", False),
    ):
        fake = Wallpaper.__new__(Wallpaper)
        fake._closed = False
        fake.backend = backend
        fake._manual_pause = False
        fake._last_pause = -1
        fake._ready = True
        fake.bridge = MagicMock()
        fake.log = MagicMock()
        fake._sync_native_audio = MagicMock()

        fake._apply_pause(False)

        fake.bridge.post.assert_called_once_with({"t": "pause", "v": expected_pause})
        assert fake._last_pause == (1 if expected_pause else 0)


def test_load_layer_shell_respects_is_supported(monkeypatch) -> None:
    import gi.repository

    fake_ls = MagicMock()
    fake_ls.is_supported.return_value = False
    monkeypatch.setattr(wallpaper_mod.gi, "require_version", lambda *a: None)
    monkeypatch.setattr(gi.repository, "GtkLayerShell", fake_ls, raising=False)
    assert load_layer_shell() is None

    fake_ls.is_supported.return_value = True
    assert load_layer_shell() is fake_ls


def test_is_x11_display_distinguishes_xwayland_from_x11(monkeypatch) -> None:
    class FakeWaylandDisplay:
        pass

    FakeWaylandDisplay.__name__ = "GdkWaylandDisplay"
    monkeypatch.setenv("DISPLAY", ":0")
    monkeypatch.setenv("WAYLAND_DISPLAY", "wayland-0")
    monkeypatch.setattr(wallpaper_mod.Gdk.Display, "get_default", staticmethod(lambda: FakeWaylandDisplay()))
    assert _is_x11_display() is False

    class FakeX11Display:
        pass

    FakeX11Display.__name__ = "GdkX11Display"
    monkeypatch.setattr(wallpaper_mod.Gdk.Display, "get_default", staticmethod(lambda: FakeX11Display()))
    assert _is_x11_display() is True


def test_select_backend_gnome_wayland_with_xwayland_picks_static(monkeypatch) -> None:
    monkeypatch.setenv("WAYLAND_DISPLAY", "wayland-0")
    monkeypatch.setenv("DISPLAY", ":0")
    monkeypatch.setenv("XDG_CURRENT_DESKTOP", "GNOME")
    monkeypatch.setenv("DESKTOP_SESSION", "gnome")
    monkeypatch.setattr(wallpaper_mod, "load_layer_shell", lambda: None)
    monkeypatch.setattr(wallpaper_mod, "has_gnome_wallpaper_extension", lambda: False)
    fake = Wallpaper.__new__(Wallpaper)
    assert fake._select_backend("auto") == "static"

    monkeypatch.setattr(wallpaper_mod, "has_gnome_wallpaper_extension", lambda: True)
    assert fake._select_backend("auto") == "gnome-shell"
    assert detect_layer_shell_safe_insets("gnome-shell") == (36, 0, 0, 0)


def test_pause_monitor_uses_gnome_extension_on_wayland(monkeypatch) -> None:
    monkeypatch.setenv("WAYLAND_DISPLAY", "wayland-0")
    monkeypatch.delenv("SWAYSOCK", raising=False)
    monkeypatch.delenv("HYPRLAND_INSTANCE_SIGNATURE", raising=False)
    monkeypatch.delenv("DESKLAND_PAUSE_HELPER", raising=False)
    log = MagicMock()
    mon = PauseMonitor(0, 0, 1920, 1080, 1, log)
    monkeypatch.setattr(mon, "_gnome_extension", lambda: True)
    assert mon.should_pause() is True
    log.write.assert_not_called()
    monkeypatch.setattr(mon, "_gnome_extension", lambda: False)
    assert mon.should_pause() is False


def test_wallpaper_url_for_disables_virtual_icons_on_static_backends():
    class DummyServer:
        def url(self, path):
            return f"http://127.0.0.1:9999/{path}"

    class DummyHost:
        version = "0.1.5"

    win = Wallpaper.__new__(Wallpaper)
    win.server = DummyServer()
    win.host = DummyHost()
    win._page = "meadow.html"
    win._audio_supported = True
    win._safe_insets = (36, 0, 0, 0)

    win.backend = "gnome-shell"
    assert "vicons=0" not in win.url_for()
    assert "safeTop=36" in win.url_for()

    win.backend = "static"
    assert "vicons=0" in win.url_for()
    assert "mute=1" in win.url_for()

    win.backend = "plasma-plugin"
    assert "vicons=0" in win.url_for()
    assert "low=1" not in win.url_for()

    win._low_mode = True
    assert "low=1" in win.url_for()


def test_pause_monitor_low_mode_aggressive_coverage():
    log = MagicMock()
    mon = PauseMonitor(0, 0, 1920, 1080, 1, log)
    # 70% window coverage should NOT pause in normal mode, but MUST pause in Low Mode.
    assert mon._rect_covers(100, 80, 1600, 920) is False
    mon.low_mode = True
    assert mon._rect_covers(100, 80, 1600, 920) is True
    # Small floating dialog (< 65%) should still NOT pause even in Low Mode.
    assert mon._rect_covers(300, 200, 600, 400) is False


def test_disable_auto_pause_bypasses_window_pause_but_keeps_manual_pause() -> None:
    fake = Wallpaper.__new__(Wallpaper)
    fake._closed = False
    fake.backend = "x11"
    fake._manual_pause = False
    fake._disable_auto_pause = False
    fake._last_pause = -1
    fake._ready = True
    fake._pause_busy = False
    fake.bridge = MagicMock()
    fake.log = MagicMock()
    fake._sync_native_audio = MagicMock()
    fake.pause_monitor = MagicMock()
    fake.pause_monitor.should_pause.return_value = True

    # Default: auto-pause pauses when fullscreen/maximized window is detected.
    fake._apply_pause(True)
    fake.bridge.post.assert_called_with({"t": "pause", "v": True})

    # When disable_auto_pause is enabled, auto=True does not pause wallpaper.
    fake.bridge.reset_mock()
    fake._disable_auto_pause = True
    fake._last_pause = -1
    fake._apply_pause(True)
    fake.bridge.post.assert_called_with({"t": "pause", "v": False})

    # Manual tray pause still pauses even when disable_auto_pause is active.
    fake.bridge.reset_mock()
    fake._manual_pause = True
    fake._last_pause = -1
    fake._apply_pause(True)
    fake.bridge.post.assert_called_with({"t": "pause", "v": True})


def test_plasma_bridge_honors_disable_auto_pause() -> None:
    from pathlib import Path

    ui_dir = (
        Path(__file__).resolve().parents[1]
        / "plasma-wallpaper"
        / "org.deskland.preview"
        / "contents"
        / "ui"
    )
    main_qml = (ui_dir / "main.qml").read_text(encoding="utf-8")
    web_qml = (ui_dir / "WebEngineLayer.qml").read_text(encoding="utf-8")
    native_qml = (ui_dir / "NativeGlLayer.qml").read_text(encoding="utf-8")
    inject_js = (ui_dir / "inject.js").read_text(encoding="utf-8")
    assert "property bool disableAutoPause: false" in main_qml
    assert "!disableAutoPause" in main_qml
    assert "window.__desklandDisableAutoPause" in web_qml
    assert "DiyarDeskGlItem" in native_qml
    assert "window.__desklandDisableAutoPause" in inject_js
    assert "(hostPaused && !disableAutoPause) || filePaused" in inject_js

