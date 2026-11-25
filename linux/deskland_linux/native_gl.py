"""Native Rust + OpenGL 3.3 Core bridge (`libdiyardesk_gl.so`) for Linux.

Provides:
- Pure-Python theme/library discovery helpers (unit-testable without GTK/GPU).
- `NativeGlLibrary`: ctypes bindings to `libdiyardesk_gl.so`.
- `NativeGlArea`: `Gtk.GLArea`-backed OpenGL 3.3 Core wallpaper surface for X11,
  Wayland wlr-layer-shell (Hyprland/Sway/COSMIC/River/Niri), and GNOME Shell.
- `NativeGlHeadless`: headless EGL offscreen renderer for `--static` fallback
  and snapshot capture without spawning any WebKitWebProcess.
"""

from __future__ import annotations

import base64
import ctypes
import io
import json
import os
import tempfile
import time
from pathlib import Path
from typing import Any, Callable

from .config import repo_root

# Canonical native Rust + OpenGL 3.3 theme IDs (kept in sync with
# `native/diyardesk-gl/src/lib.rs` and `host/DesktopHost.cs`).
NATIVE_GL_THEMES: frozenset[str] = frozenset(
    {
        "material-impulse-gl",
        "material-impulse",
        "liquidglass",
        "thinking-orbs",
        "solar-system",
        "studio-glass",
        "beach",
        "meadow",
        "desert",
        "rosebrook",
        "glade",
        "zen-archipelago",
        "aurora",
        "polarnight",
        "quietsnow",
        "winter",
        "neumorph",
        "claymorph",
        "neobrutalism",
        "voxelcraft",
        "papercut",
        "istanbul",
        "software-world",
        "rift",
        "strike",
        "pocket",
        "roach",
        "ai-mascots",
        "cat-shore",
        "persephone",
    }
)

NATIVE_GL_ONLY_PAGES: frozenset[str] = frozenset(
    {
        "material-impulse-gl.html",
        "ai-mascots.html",
        "cat-shore.html",
        "persephone.html",
    }
)


def is_native_gl_page(page: str | None) -> bool:
    if not page:
        return False
    name = page.rsplit("/", 1)[-1].split("?", 1)[0].strip().lower()
    return name in NATIVE_GL_ONLY_PAGES


def is_native_gl_theme(theme_id: str | None) -> bool:
    if not theme_id:
        return False
    return theme_id.strip().lower() in NATIVE_GL_THEMES


def should_use_native_gl(
    page: str | None,
    theme_id: str | None,
    *,
    rust_opengl: bool,
    available: bool,
) -> bool:
    """Return True when the native Rust OpenGL 3.3 engine should render this theme."""
    if not available:
        return False
    if is_native_gl_page(page):
        return True
    if not rust_opengl:
        return False
    return is_native_gl_theme(theme_id)


def candidate_library_paths() -> list[Path]:
    paths: list[Path] = []
    override = os.environ.get("DESKLAND_GL_LIB", "").strip()
    if override:
        paths.append(Path(override).expanduser())
    root = repo_root()
    paths.extend(
        [
            root / "native" / "diyardesk-gl" / "target" / "release" / "libdiyardesk_gl.so",
            root / "linux" / "lib" / "libdiyardesk_gl.so",
            root / "native" / "diyardesk-gl" / "target" / "debug" / "libdiyardesk_gl.so",
            Path("/usr/lib/deskland/libdiyardesk_gl.so"),
            Path("/usr/lib64/deskland/libdiyardesk_gl.so"),
            Path("/usr/local/lib/libdiyardesk_gl.so"),
            Path("/usr/lib/libdiyardesk_gl.so"),
            Path("/usr/lib64/libdiyardesk_gl.so"),
        ]
    )
    return paths


def find_native_gl_library() -> Path | None:
    for p in candidate_library_paths():
        try:
            if p.is_file():
                return p
        except OSError:
            continue
    return None


def _to_utf8_bytes(payload: Any) -> bytes:
    if isinstance(payload, str):
        return payload.encode("utf-8")
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def _png_file_to_data_urls(png_path: Path) -> tuple[str, str]:
    """Read a rendered PNG file and produce (thumb_data_url, full_png_data_url)."""
    raw = png_path.read_bytes()
    full_url = "data:image/png;base64," + base64.b64encode(raw).decode("ascii")
    thumb_url = full_url
    try:
        import gi

        gi.require_version("GdkPixbuf", "2.0")
        from gi.repository import GdkPixbuf

        pb = GdkPixbuf.Pixbuf.new_from_file_at_scale(str(png_path), 480, 270, True)
        ok, buf = pb.save_to_bufferv("png", [], [])
        if ok and buf:
            thumb_url = "data:image/png;base64," + base64.b64encode(bytes(buf)).decode("ascii")
    except Exception:
        pass
    return thumb_url, full_url


class NativeGlLibrary:
    """Singleton ctypes binding to `libdiyardesk_gl.so`."""

    _instance: "NativeGlLibrary | None" = None
    _checked: bool = False

    def __init__(self, path: Path) -> None:
        self.path = path
        lib = ctypes.CDLL(str(path))
        self._lib = lib

        c_void_p = ctypes.c_void_p
        c_int = ctypes.c_int
        c_uint32 = ctypes.c_uint32
        c_float = ctypes.c_float
        c_char_p = ctypes.c_char_p

        lib.ddgl_is_native_theme.argtypes = [c_char_p]
        lib.ddgl_is_native_theme.restype = c_int

        # Embedded GL context API (Gtk.GLArea / Qt6 FBO)
        lib.ddgl_embedded_create.argtypes = [c_int, c_int, c_float]
        lib.ddgl_embedded_create.restype = c_void_p
        lib.ddgl_embedded_destroy.argtypes = [c_void_p]
        lib.ddgl_embedded_destroy.restype = None
        lib.ddgl_embedded_resize.argtypes = [c_void_p, c_int, c_int, c_float]
        lib.ddgl_embedded_resize.restype = None
        lib.ddgl_embedded_set_paused.argtypes = [c_void_p, c_int]
        lib.ddgl_embedded_set_paused.restype = None
        lib.ddgl_embedded_set_low_mode.argtypes = [c_void_p, c_int]
        lib.ddgl_embedded_set_low_mode.restype = None
        lib.ddgl_embedded_post_json.argtypes = [c_void_p, c_char_p]
        lib.ddgl_embedded_post_json.restype = None
        lib.ddgl_embedded_mouse.argtypes = [c_void_p, c_int, c_float, c_float]
        lib.ddgl_embedded_mouse.restype = c_int
        lib.ddgl_embedded_key.argtypes = [c_void_p, c_int, c_int, c_char_p]
        lib.ddgl_embedded_key.restype = None
        lib.ddgl_embedded_poll_event.argtypes = [c_void_p, ctypes.c_char_p, c_int]
        lib.ddgl_embedded_poll_event.restype = c_int
        lib.ddgl_embedded_render.argtypes = [c_void_p, c_float, c_uint32]
        lib.ddgl_embedded_render.restype = c_int
        lib.ddgl_embedded_render_png.argtypes = [c_void_p, c_uint32, c_char_p]
        lib.ddgl_embedded_render_png.restype = c_int

        # Headless EGL offscreen API (static fallback / offscreen snapshot)
        lib.ddgl_headless_create.argtypes = [c_int, c_int, c_float]
        lib.ddgl_headless_create.restype = c_void_p
        lib.ddgl_headless_destroy.argtypes = [c_void_p]
        lib.ddgl_headless_destroy.restype = None
        lib.ddgl_headless_resize.argtypes = [c_void_p, c_int, c_int, c_float]
        lib.ddgl_headless_resize.restype = None
        lib.ddgl_headless_set_paused.argtypes = [c_void_p, c_int]
        lib.ddgl_headless_set_paused.restype = None
        lib.ddgl_headless_set_low_mode.argtypes = [c_void_p, c_int]
        lib.ddgl_headless_set_low_mode.restype = None
        lib.ddgl_headless_post_json.argtypes = [c_void_p, c_char_p]
        lib.ddgl_headless_post_json.restype = None
        lib.ddgl_headless_mouse.argtypes = [c_void_p, c_int, c_float, c_float]
        lib.ddgl_headless_mouse.restype = c_int
        lib.ddgl_headless_key.argtypes = [c_void_p, c_int, c_int, c_char_p]
        lib.ddgl_headless_key.restype = None
        lib.ddgl_headless_poll_event.argtypes = [c_void_p, ctypes.c_char_p, c_int]
        lib.ddgl_headless_poll_event.restype = c_int
        lib.ddgl_headless_render_png.argtypes = [c_void_p, c_float, c_char_p]
        lib.ddgl_headless_render_png.restype = c_int

    @classmethod
    def get(cls, *, force_reload: bool = False) -> "NativeGlLibrary | None":
        if os.environ.get("DESKLAND_DISABLE_NATIVE_GL") == "1":
            return None
        if cls._checked and not force_reload:
            return cls._instance
        cls._checked = True
        path = find_native_gl_library()
        if path is None:
            cls._instance = None
            return None
        try:
            cls._instance = cls(path)
        except OSError:
            cls._instance = None
        return cls._instance

    @classmethod
    def is_available(cls) -> bool:
        return cls.get() is not None

    @property
    def raw(self) -> ctypes.CDLL:
        return self._lib

    def is_native_theme(self, theme_id: str) -> bool:
        if not theme_id:
            return False
        return bool(self._lib.ddgl_is_native_theme(theme_id.encode("utf-8")))


class FramePacer:
    """Compositor-friendly frame pacer & stall detector for Wayland/X11 `Gtk.GLArea`.

    Prevents stacking unconsumed `queue_render()` commits during Mutter/OBS PipeWire
    DMA-BUF renegotiation and automatically recovers if a compositor temporarily drops
    a frame callback.
    """

    def __init__(self, target_fps: int = 30, stall_sec: float = 1.2) -> None:
        self._base_fps = max(5, min(60, int(target_fps)))
        self._low_mode = False
        self._stall_sec = max(0.3, float(stall_sec))
        self.render_pending = False
        self._last_tick_us = 0
        self._last_render_mono = 0.0
        self._pending_since_mono = 0.0

    @property
    def effective_fps(self) -> int:
        return min(15, self._base_fps) if self._low_mode else self._base_fps

    def set_low_mode(self, low: bool) -> None:
        self._low_mode = bool(low)

    def reset(self, now_mono: float) -> None:
        self.render_pending = False
        self._last_tick_us = 0
        self._last_render_mono = now_mono
        self._pending_since_mono = 0.0

    def should_queue_render(self, frame_time_us: int, now_mono: float) -> bool:
        if self.render_pending:
            if (now_mono - self._pending_since_mono) < self._stall_sec:
                return False
            # Previous queued frame stalled (e.g. Wayland DMA-BUF renegotiation) -> recover
            self.render_pending = False

        min_interval_us = max(8_000, int(1_000_000 / self.effective_fps) - 2_000)
        if self._last_tick_us > 0 and frame_time_us > self._last_tick_us:
            if (frame_time_us - self._last_tick_us) < min_interval_us:
                return False
        self._last_tick_us = frame_time_us
        self.render_pending = True
        self._pending_since_mono = now_mono
        return True

    def on_render_complete(self, now_mono: float) -> float:
        self.render_pending = False
        if self._last_render_mono <= 0.0:
            self._last_render_mono = now_mono
            return 0.016
        dt = max(0.0, min(0.08, now_mono - self._last_render_mono))
        self._last_render_mono = now_mono
        return dt

    def is_stalled(self, now_mono: float) -> bool:
        if self._last_render_mono <= 0.0:
            return False
        return (now_mono - self._last_render_mono) >= self._stall_sec


class NativeGlArea:
    """GTK3 `Gtk.GLArea` hosting `ddgl_embedded_*` for live Linux desktop rendering."""

    def __init__(
        self,
        width_px: int,
        height_px: int,
        scale: float,
        on_message: Callable[[str], None] | None = None,
        log: Any = None,
        target_fps: int = 30,
    ) -> None:
        import gi

        gi.require_version("Gtk", "3.0")
        gi.require_version("Gdk", "3.0")
        from gi.repository import Gdk, GLib, Gtk

        self._Gdk = Gdk
        self._GLib = GLib
        self._lib = NativeGlLibrary.get()
        if self._lib is None:
            raise RuntimeError("libdiyardesk_gl.so yüklenemedi")

        self._width_px = max(64, int(width_px))
        self._height_px = max(64, int(height_px))
        self._scale = max(0.5, float(scale))
        self._on_message = on_message
        self._log = log
        self._engine: int | None = None
        self._paused = False
        self._low_mode = False
        self._destroyed = False
        self._pending_json: list[bytes] = []
        self._tick_cb_id = 0
        self._tick_id = 0
        self._watchdog_id = 0
        self._pacer = FramePacer(target_fps=target_fps, stall_sec=1.2)
        self._last_frame_ts = time.monotonic()
        self._poll_buf = ctypes.create_string_buffer(16384)
        self._pending_snapshot_cb: Callable[[str | None, str | None, str | None], None] | None = None

        area = Gtk.GLArea()
        area.set_required_version(3, 3)
        area.set_has_depth_buffer(False)
        area.set_has_alpha(False)
        area.set_auto_render(False)
        area.set_hexpand(True)
        area.set_vexpand(True)
        area.add_events(
            Gdk.EventMask.POINTER_MOTION_MASK
            | Gdk.EventMask.BUTTON_PRESS_MASK
            | Gdk.EventMask.BUTTON_RELEASE_MASK
            | Gdk.EventMask.LEAVE_NOTIFY_MASK
        )
        area.connect("realize", self._on_realize)
        area.connect("unrealize", self._on_unrealize)
        area.connect("resize", self._on_resize)
        area.connect("render", self._on_render)
        area.connect("motion-notify-event", self._on_motion)
        area.connect("button-press-event", self._on_button_press)
        area.connect("button-release-event", self._on_button_release)
        area.connect("leave-notify-event", self._on_leave)
        self.widget = area

    @property
    def is_running(self) -> bool:
        return not self._destroyed and self._engine is not None

    def _log_write(self, msg: str) -> None:
        if self._log is not None:
            try:
                self._log.write(msg)
            except Exception:
                pass

    def _on_realize(self, area: Any) -> None:
        if self._destroyed:
            return
        area.make_current()
        err = area.get_error()
        if err is not None:
            self._log_write(f"Gtk.GLArea OpenGL 3.3 bağlam hatası: {err}")
            return
        alloc = area.get_allocation()
        sf = max(1, int(area.get_scale_factor() or 1))
        w = max(64, int(round(alloc.width * sf))) if alloc.width > 1 else self._width_px
        h = max(64, int(round(alloc.height * sf))) if alloc.height > 1 else self._height_px
        self._width_px = w
        self._height_px = h

        eng = self._lib.raw.ddgl_embedded_create(w, h, float(self._scale))
        if not eng:
            self._log_write("ddgl_embedded_create başarısız oldu")
            return
        self._engine = eng
        self._lib.raw.ddgl_embedded_set_low_mode(eng, 1 if self._low_mode else 0)
        self._lib.raw.ddgl_embedded_set_paused(eng, 1 if self._paused else 0)
        for raw_msg in self._pending_json:
            self._lib.raw.ddgl_embedded_post_json(eng, raw_msg)
        self._pending_json.clear()
        now = time.monotonic()
        self._last_frame_ts = now
        self._pacer.reset(now)
        self._log_write(
            f"Linux Rust OpenGL 3.3 Core motoru başlatıldı ({w}x{h}, scale={self._scale:.2f})"
        )
        self._reschedule_tick()
        area.queue_render()

    def _on_unrealize(self, area: Any) -> None:
        self._stop_tick()
        if self._engine is not None:
            try:
                area.make_current()
                self._lib.raw.ddgl_embedded_destroy(self._engine)
            except Exception:
                pass
            self._engine = None

    def _on_resize(self, _area: Any, width: int, height: int) -> None:
        self._width_px = max(64, int(width))
        self._height_px = max(64, int(height))
        if self._engine is not None:
            self._lib.raw.ddgl_embedded_resize(
                self._engine, self._width_px, self._height_px, float(self._scale)
            )
            self.widget.queue_render()

    def _on_render(self, _area: Any, _gl_context: Any) -> bool:
        if self._destroyed or self._engine is None:
            return False
        now = time.monotonic()
        dt = self._pacer.on_render_complete(now)
        self._last_frame_ts = now
        # 0xFFFFFFFF tells Rust to auto-detect Gtk.GLArea's currently bound FBO.
        self._lib.raw.ddgl_embedded_render(self._engine, float(dt), 0xFFFFFFFF)
        if self._pending_snapshot_cb is not None:
            cb = self._pending_snapshot_cb
            self._pending_snapshot_cb = None
            self._capture_snapshot_current_fbo(cb)
        self._drain_events()
        return True

    def _capture_snapshot_current_fbo(
        self, cb: Callable[[str | None, str | None, str | None], None]
    ) -> None:
        tmp_path = Path(tempfile.gettempdir()) / f"diyardesk-gl-snap-{int(time.time() * 1000)}.png"
        try:
            ok = self._lib.raw.ddgl_embedded_render_png(
                self._engine, 0xFFFFFFFF, str(tmp_path).encode("utf-8")
            )
            if ok and tmp_path.is_file():
                thumb, full = _png_file_to_data_urls(tmp_path)
                self._GLib.idle_add(lambda: (cb(thumb, full, None), False)[1])
            else:
                self._GLib.idle_add(lambda: (cb(None, None, "OpenGL snapshot failed"), False)[1])
        except Exception as exc:
            msg = str(exc)
            self._GLib.idle_add(lambda: (cb(None, None, msg), False)[1])
        finally:
            try:
                tmp_path.unlink()
            except OSError:
                pass

    def _stop_tick(self) -> None:
        if self._tick_cb_id:
            try:
                self.widget.remove_tick_callback(self._tick_cb_id)
            except Exception:
                pass
            self._tick_cb_id = 0
        if self._tick_id:
            try:
                self._GLib.source_remove(self._tick_id)
            except Exception:
                pass
            self._tick_id = 0
        if self._watchdog_id:
            try:
                self._GLib.source_remove(self._watchdog_id)
            except Exception:
                pass
            self._watchdog_id = 0

    def _reschedule_tick(self) -> None:
        self._stop_tick()
        if self._destroyed or self._engine is None or self._paused:
            return
        self._pacer.set_low_mode(self._low_mode)
        if hasattr(self.widget, "add_tick_callback"):
            self._tick_cb_id = int(self.widget.add_tick_callback(self._on_frame_tick))
            # Lightweight stall-recovery watchdog (e.g. OBS/PipeWire DMA-BUF renegotiation)
            self._watchdog_id = int(self._GLib.timeout_add(600, self._on_watchdog))
        else:
            interval_ms = max(16, int(round(1000.0 / self._pacer.effective_fps)))
            self._tick_id = int(self._GLib.timeout_add(interval_ms, self._on_tick))

    def _on_frame_tick(self, _widget: Any, frame_clock: Any) -> bool:
        if self._destroyed or self._engine is None or self._paused:
            self._tick_cb_id = 0
            return False
        now = time.monotonic()
        try:
            frame_us = int(frame_clock.get_frame_time())
        except Exception:
            frame_us = int(now * 1_000_000)
        if self._pacer.should_queue_render(frame_us, now):
            self.widget.queue_render()
        return True

    def _on_watchdog(self) -> bool:
        if self._destroyed or self._engine is None or self._paused:
            self._watchdog_id = 0
            return False
        now = time.monotonic()
        if self._pacer.is_stalled(now):
            self._pacer.render_pending = False
            try:
                self.widget.queue_draw()
            except Exception:
                pass
            self.widget.queue_render()
        return True

    def _on_tick(self) -> bool:
        if self._destroyed or self._engine is None or self._paused:
            self._tick_id = 0
            return False
        now = time.monotonic()
        if self._pacer.should_queue_render(int(now * 1_000_000), now):
            self.widget.queue_render()
        return True

    def _drain_events(self) -> None:
        if self._engine is None or self._on_message is None:
            return
        for _ in range(16):
            n = self._lib.raw.ddgl_embedded_poll_event(
                self._engine, self._poll_buf, len(self._poll_buf)
            )
            if n <= 0:
                break
            try:
                msg = self._poll_buf.raw[:n].decode("utf-8", errors="replace")
                self._on_message(msg)
            except Exception:
                pass

    def _on_motion(self, _widget: Any, event: Any) -> bool:
        self.dispatch_mouse(0, float(event.x), float(event.y))
        return False

    def _on_button_press(self, _widget: Any, event: Any) -> bool:
        if getattr(event, "button", 1) == 1:
            self.dispatch_mouse(1, float(event.x), float(event.y))
        return False

    def _on_button_release(self, _widget: Any, event: Any) -> bool:
        if getattr(event, "button", 1) == 1:
            self.dispatch_mouse(2, float(event.x), float(event.y))
        return False

    def _on_leave(self, _widget: Any, _event: Any) -> bool:
        self.dispatch_mouse(3, -1.0, -1.0)
        return False

    def post_json(self, payload: Any) -> None:
        if self._destroyed:
            return
        raw = _to_utf8_bytes(payload)
        if self._engine is None:
            if len(self._pending_json) < 128:
                self._pending_json.append(raw)
            return
        self._lib.raw.ddgl_embedded_post_json(self._engine, raw)
        if self._paused:
            self.widget.queue_render()

    def set_paused(self, paused: bool) -> None:
        val = bool(paused)
        self._paused = val
        if self._engine is not None:
            self._lib.raw.ddgl_embedded_set_paused(self._engine, 1 if val else 0)
        now = time.monotonic()
        self._last_frame_ts = now
        self._pacer.reset(now)
        self._reschedule_tick()
        if not val and self._engine is not None and not self._destroyed:
            self.widget.queue_render()

    def set_low_mode(self, low: bool) -> None:
        val = bool(low)
        self._low_mode = val
        if self._engine is not None:
            self._lib.raw.ddgl_embedded_set_low_mode(self._engine, 1 if val else 0)
        self._reschedule_tick()

    def dispatch_mouse(self, kind: int, css_x: float, css_y: float) -> bool:
        if self._destroyed or self._engine is None:
            return False
        handled = bool(self._lib.raw.ddgl_embedded_mouse(self._engine, int(kind), float(css_x), float(css_y)))
        if kind in (1, 2):
            self._drain_events()
        return handled

    def request_snapshot(
        self, cb: Callable[[str | None, str | None, str | None], None]
    ) -> None:
        if self._destroyed or self._engine is None:
            cb(None, None, "Native OpenGL renderer not ready")
            return
        self._pending_snapshot_cb = cb
        self.widget.queue_render()

    def destroy(self) -> None:
        if self._destroyed:
            return
        self._destroyed = True
        self._stop_tick()
        if self._engine is not None:
            try:
                self.widget.make_current()
                self._lib.raw.ddgl_embedded_destroy(self._engine)
            except Exception:
                pass
            self._engine = None
        try:
            self.widget.destroy()
        except Exception:
            pass


class NativeGlHeadless:
    """Headless EGL OpenGL 3.3 engine for `--static` fallback & offscreen PNG snapshots."""

    def __init__(self, width_px: int, height_px: int, scale: float, log: Any = None) -> None:
        self._lib = NativeGlLibrary.get()
        if self._lib is None:
            raise RuntimeError("libdiyardesk_gl.so yüklenemedi")
        self._log = log
        self._width_px = max(64, int(width_px))
        self._height_px = max(64, int(height_px))
        self._scale = max(0.5, float(scale))
        eng = self._lib.raw.ddgl_headless_create(
            self._width_px, self._height_px, float(self._scale)
        )
        if not eng:
            raise RuntimeError("ddgl_headless_create (EGL OpenGL 3.3) başarısız oldu")
        self._engine: int | None = eng

    @property
    def is_running(self) -> bool:
        return self._engine is not None

    def resize(self, width_px: int, height_px: int, scale: float) -> None:
        if self._engine is None:
            return
        self._width_px = max(64, int(width_px))
        self._height_px = max(64, int(height_px))
        self._scale = max(0.5, float(scale))
        self._lib.raw.ddgl_headless_resize(
            self._engine, self._width_px, self._height_px, float(self._scale)
        )

    def post_json(self, payload: Any) -> None:
        if self._engine is None:
            return
        self._lib.raw.ddgl_headless_post_json(self._engine, _to_utf8_bytes(payload))

    def set_low_mode(self, low: bool) -> None:
        if self._engine is None:
            return
        self._lib.raw.ddgl_headless_set_low_mode(self._engine, 1 if low else 0)

    def set_paused(self, paused: bool) -> None:
        if self._engine is None:
            return
        self._lib.raw.ddgl_headless_set_paused(self._engine, 1 if paused else 0)

    def render_to_png(self, out_path: Path, dt: float = 0.033) -> bool:
        if self._engine is None:
            return False
        out_path.parent.mkdir(parents=True, exist_ok=True)
        return bool(
            self._lib.raw.ddgl_headless_render_png(
                self._engine, float(dt), str(out_path).encode("utf-8")
            )
        )

    def render_snapshot_data_urls(self, dt: float = 0.033) -> tuple[str | None, str | None, str | None]:
        tmp_path = Path(tempfile.gettempdir()) / f"diyardesk-gl-headless-{int(time.time() * 1000)}.png"
        try:
            if not self.render_to_png(tmp_path, dt=dt):
                return None, None, "Headless OpenGL snapshot failed"
            thumb, full = _png_file_to_data_urls(tmp_path)
            return thumb, full, None
        except Exception as exc:
            return None, None, str(exc)
        finally:
            try:
                tmp_path.unlink()
            except OSError:
                pass

    def destroy(self) -> None:
        if self._engine is not None:
            try:
                self._lib.raw.ddgl_headless_destroy(self._engine)
            except Exception:
                pass
            self._engine = None
