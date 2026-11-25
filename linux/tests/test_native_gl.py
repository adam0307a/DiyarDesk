"""Tests for Linux Native Rust + OpenGL 3.3 Core bridge and fallback behavior."""

from __future__ import annotations

from pathlib import Path

import pytest

from deskland_linux.config import SettingsStore
from deskland_linux.ctl import build_command_payload
from deskland_linux.native_gl import (
    NATIVE_GL_THEMES,
    NativeGlHeadless,
    NativeGlLibrary,
    find_native_gl_library,
    is_native_gl_page,
    is_native_gl_theme,
    should_use_native_gl,
)
from deskland_linux.native_settings import SettingsI18n, format_param_value, is_toggle_param


def test_should_use_native_gl_routing_preserves_html_css_js_support():
    # Native themes activate when rust_opengl=True and library is available
    for tid in ("papercut", "meadow", "beach", "istanbul", "studio-glass", "liquidglass", "ai-mascots"):
        assert is_native_gl_theme(tid)
        assert should_use_native_gl(f"{tid}.html", tid, rust_opengl=True, available=True)

    # Turning rust_opengl off returns standard themes to HTML/CSS/JS WebKitGTK
    assert not should_use_native_gl("papercut.html", "papercut", rust_opengl=False, available=True)
    assert not should_use_native_gl("meadow.html", "meadow", rust_opengl=False, available=True)

    # GL-only pages always prefer native GL when available
    assert is_native_gl_page("material-impulse-gl.html")
    assert is_native_gl_page("ai-mascots.html")
    assert should_use_native_gl("material-impulse-gl.html", "material-impulse-gl", rust_opengl=False, available=True)
    assert should_use_native_gl("ai-mascots.html", "ai-mascots", rust_opengl=False, available=True)

    # Custom HTML/CSS/JS, image, and video themes NEVER hijack into native GL
    assert not should_use_native_gl("__custom/my-html-theme/index.html", "my-html-theme", rust_opengl=True, available=True)
    assert not should_use_native_gl("__custom/video-bg/index.html", "video-bg", rust_opengl=True, available=True)
    assert not is_native_gl_theme("custom-html")

    # Missing library always falls back cleanly to WebKitGTK
    assert not should_use_native_gl("papercut.html", "papercut", rust_opengl=True, available=False)
    assert not should_use_native_gl("ai-mascots.html", "ai-mascots", rust_opengl=True, available=False)


def test_settings_store_persists_rust_opengl(tmp_path: Path):
    store = SettingsStore(directory=tmp_path)
    assert store.rust_opengl is False
    store.rust_opengl = True
    store.save()

    reloaded = SettingsStore(directory=tmp_path)
    reloaded.load()
    assert reloaded.rust_opengl is True


def test_ctl_parses_rust_gl_subcommand():
    payload, as_json, _ = build_command_payload(["rust-gl", "on"])
    assert payload == {"cmd": "rust-gl", "state": "on"}
    assert not as_json

    payload2, as_json2, _ = build_command_payload(["--json", "rust-gl", "off"])
    assert payload2 == {"cmd": "rust-gl", "state": "off"}
    assert as_json2

    payload3, _, _ = build_command_payload(["rust-gl"])
    assert payload3 == {"cmd": "rust-gl", "state": "toggle"}


def test_is_native_gl_theme_excludes_custom_html_themes():
    assert is_native_gl_theme("software-world")
    assert is_native_gl_theme("papercut")
    assert not is_native_gl_theme("custom-diyardesk-software-world")
    assert not should_use_native_gl(
        "__custom/custom-diyardesk-software-world/index.html",
        "custom-diyardesk-software-world",
        rust_opengl=True,
        available=True,
    )


def test_native_settings_i18n_and_param_formatting():
    tr = SettingsI18n(code="tr")
    assert tr.ui("rustOpenGl") == "Rust OpenGL"
    assert "Kâğıt" in tr.theme_name("papercut")
    assert tr.group_name("papercut", 0) != ""
    assert tr.param_label("papercut", "time") != ""
    opts = tr.param_options("papercut", "time", ["0", "1", "2", "3", "4"])
    assert len(opts) == 5 and opts[0] != "0"

    assert is_toggle_param({"key": "showBoy", "min": 0, "max": 1, "step": 1, "default": 1})
    assert not is_toggle_param({"key": "flow", "min": 0, "max": 2, "step": 0.1, "default": 1})
    assert not is_toggle_param({"key": "palette", "options": ["A", "B"], "default": 0})
    assert format_param_value({"step": 0.05, "format": "times"}, 1.25) == "1.25×"
    assert format_param_value({"step": 0.05, "format": "pct"}, 0.75) == "75%"
    assert format_param_value({"step": 1, "unit": "px"}, 18) == "18px"


def test_live_libdiyardesk_gl_headless_png_render(tmp_path: Path):
    lib_path = find_native_gl_library()
    if lib_path is None:
        pytest.skip("libdiyardesk_gl.so not built yet")

    lib = NativeGlLibrary.get(force_reload=True)
    assert lib is not None
    assert lib.path == lib_path
    for tid in NATIVE_GL_THEMES:
        assert lib.is_native_theme(tid), f"Rust library did not recognize native theme {tid}"
    assert not lib.is_native_theme("custom-html-theme")

    headless = NativeGlHeadless(320, 180, 1.0)
    try:
        headless.post_json({"t": "theme", "id": "istanbul"})
        headless.post_json({"t": "icons", "vis": True, "r": [[24, 24, 80, 96]]})
        out_png = tmp_path / "istanbul_linux_gl.png"
        assert headless.render_to_png(out_png, dt=0.033)
        assert out_png.is_file()
        raw = out_png.read_bytes()
        assert raw[:8] == b"\x89PNG\r\n\x1a\n"
        assert len(raw) > 2000
    finally:
        headless.destroy()


def test_frame_pacer_throttles_and_recovers_from_stall():
    from deskland_linux.native_gl import FramePacer

    pacer = FramePacer(target_fps=30, stall_sec=1.2)
    pacer.reset(10.0)
    # First frame queues immediately and marks render_pending
    assert pacer.should_queue_render(1_000_000, 10.0) is True
    assert pacer.render_pending is True
    # Subsequent frame while still pending is suppressed (no commit stacking)
    assert pacer.should_queue_render(1_040_000, 10.04) is False

    # Render completes -> dt clamped & next frame throttled to ~30fps
    dt = pacer.on_render_complete(10.033)
    assert 0.03 <= dt <= 0.04
    assert pacer.render_pending is False
    assert pacer.should_queue_render(1_016_000, 10.049) is False
    assert pacer.should_queue_render(1_035_000, 10.068) is True

    # Stall recovery after stall_sec (e.g. OBS/PipeWire DMA-BUF renegotiation)
    assert pacer.should_queue_render(1_500_000, 10.5) is False
    assert pacer.should_queue_render(2_400_000, 11.4) is True
    assert pacer.is_stalled(11.4) is True

    # Low-power mode caps effective FPS at 15
    pacer.set_low_mode(True)
    assert pacer.effective_fps == 15
