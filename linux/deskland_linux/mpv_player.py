"""Hardware-accelerated video wallpaper backends for Linux (Wayland & X11).

Supports three native, zero-WebKitGTK backends:
1. ``libmpv-gl``: ``libmpv.so`` OpenGL render API inside ``Gtk.GLArea`` — works
   natively on **both Wayland (gtk-layer-shell / GNOME) and X11** with hardware
   decoding (VA-API / NVDEC / Vulkan) and zero external processes.
2. ``gstreamer``: ``Gst.Element`` (``gtkglsink`` / ``gtksink``) inside GTK — works
   on **both Wayland and X11** when ``libmpv`` is unavailable.
3. ``mpv-wid``: optional ``mpv --wid`` subprocess on X11 when neither ``libmpv``
   nor GStreamer GTK sinks are available.

Safely falls back to WebKitGTK ``<video>`` only when no native backend succeeds.
"""
from __future__ import annotations

import ctypes
import ctypes.util
import shutil
import subprocess
from pathlib import Path
from typing import Any


VIDEO_EXTS = {".mp4", ".webm", ".mkv", ".mov"}


def is_mpv_available() -> bool:
    return shutil.which("mpv") is not None


def clamp_video_speed(speed: float) -> float:
    return max(0.25, min(2.0, float(speed)))


def map_brightness_to_mpv(brightness: float) -> int:
    # mpv brightness is -100..100 (0 = neutral); map 0.4..1.4 -> -36..+24
    return int(round((max(0.4, min(1.4, float(brightness))) - 1.0) * 60.0))


def find_libmpv_path() -> str | None:
    found = ctypes.util.find_library("mpv")
    if found:
        return found
    for candidate in (
        "libmpv.so.2",
        "libmpv.so.1",
        "libmpv.so",
        "/usr/lib/libmpv.so.2",
        "/usr/lib/x86_64-linux-gnu/libmpv.so.2",
        "/usr/lib/aarch64-linux-gnu/libmpv.so.2",
        "/usr/lib64/libmpv.so.2",
        "/usr/lib/x86_64-linux-gnu/libmpv.so.1",
        "/usr/lib64/libmpv.so.1",
    ):
        try:
            ctypes.CDLL(candidate)
            return candidate
        except OSError:
            continue
    return None


def has_gstreamer_gtk() -> bool:
    try:
        import gi  # type: ignore[import-not-found]

        gi.require_version("Gst", "1.0")
        from gi.repository import Gst  # type: ignore[import-not-found]

        if not Gst.is_initialized():
            Gst.init(None)
        return bool(
            Gst.ElementFactory.find("gtkglsink") is not None
            or Gst.ElementFactory.find("gtksink") is not None
        )
    except Exception:
        return False


def select_video_backend(
    display_backend: str,
    *,
    has_libmpv: bool,
    has_gst_gtk: bool,
    has_mpv_cli: bool,
) -> str | None:
    """Choose the best zero-WebKitGTK video backend for ``display_backend``."""
    if display_backend in ("plasma-plugin", "static"):
        return None
    if has_libmpv:
        return "libmpv-gl"
    if has_gst_gtk:
        return "gstreamer"
    if display_backend == "x11" and has_mpv_cli:
        return "mpv-wid"
    return None


def has_any_native_video_backend(display_backend: str = "wayland") -> bool:
    return (
        select_video_backend(
            display_backend,
            has_libmpv=find_libmpv_path() is not None,
            has_gst_gtk=has_gstreamer_gtk(),
            has_mpv_cli=is_mpv_available(),
        )
        is not None
    )


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
    spd = clamp_video_speed(speed)
    mpv_bright = map_brightness_to_mpv(brightness)
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
    """Manages an optional ``mpv`` subprocess attached to an X11 wallpaper window ID."""

    def __init__(self, log: Any = None) -> None:
        self._log = log
        self._proc: subprocess.Popen[bytes] | None = None
        self._current_video: Path | None = None

    @property
    def active(self) -> bool:
        return self._proc is not None and self._proc.poll() is None

    def start(
        self,
        video_path: Path,
        wid: int | None = None,
        speed: float = 1.0,
        brightness: float = 1.0,
    ) -> bool:
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
                self._log.write(f"mpv donanim video arka ucu baslatildi: {video_path.name}")
            return True
        except OSError as exc:
            if self._log is not None:
                self._log.exception("mpv baslatilamadi; WebView oynaticiya donuluyor", exc)
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


# ----------------------------------------------------------------------------
# Zero-WebKitGTK Wayland + X11 Gtk.GLArea (`libmpv`) backend
# ----------------------------------------------------------------------------
class _MpvRenderParam(ctypes.Structure):
    _fields_ = [("type", ctypes.c_int), ("data", ctypes.c_void_p)]


class _MpvOpenGlInitParams(ctypes.Structure):
    _fields_ = [
        ("get_proc_address", ctypes.c_void_p),
        ("get_proc_address_ctx", ctypes.c_void_p),
    ]


class _MpvOpenGlFbo(ctypes.Structure):
    _fields_ = [
        ("fbo", ctypes.c_int),
        ("w", ctypes.c_int),
        ("h", ctypes.c_int),
        ("internal_format", ctypes.c_int),
    ]


class LibMpvGlArea:
    """Renders hardware-decoded video directly into ``Gtk.GLArea`` via ``libmpv``."""

    backend_name = "libmpv-gl"

    def __init__(
        self,
        video_path: Path,
        *,
        speed: float = 1.0,
        brightness: float = 1.0,
        log: Any = None,
    ) -> None:
        import gi  # type: ignore[import-not-found]

        gi.require_version("Gtk", "3.0")
        from gi.repository import GLib, Gtk  # type: ignore[import-not-found]

        lib_path = find_libmpv_path()
        if not lib_path:
            raise RuntimeError("libmpv not found")

        self._Gtk = Gtk
        self._GLib = GLib
        self._log = log
        self._video_path = video_path
        self._speed = clamp_video_speed(speed)
        self._brightness = map_brightness_to_mpv(brightness)
        self._paused = False
        self._destroyed = False
        self._render_pending = False

        self._libmpv = ctypes.CDLL(lib_path)
        self._bind_mpv_symbols()
        self._libegl, self._egl_get_proc = self._load_gl_resolver("EGL")
        self._libgl, self._glx_get_proc = self._load_gl_resolver("GL")

        self._mpv = self._libmpv.mpv_create()
        if not self._mpv:
            raise RuntimeError("mpv_create returned NULL")

        for opt, val in (
            (b"vo", b"libmpv"),
            (b"hwdec", b"auto-safe"),
            (b"ao", b"null"),
            (b"mute", b"yes"),
            (b"loop-file", b"inf"),
            (b"keep-open", b"yes"),
            (b"panscan", b"1.0"),
            (b"terminal", b"no"),
            (b"msg-level", b"all=warn"),
        ):
            self._libmpv.mpv_set_option_string(self._mpv, opt, val)

        if self._libmpv.mpv_initialize(self._mpv) < 0:
            self._libmpv.mpv_terminate_destroy(self._mpv)
            self._mpv = None
            raise RuntimeError("mpv_initialize failed")

        self._render_ctx = ctypes.c_void_p(0)
        self._proc_cb = None
        self._update_cb = None

        self.widget = Gtk.GLArea()
        self.widget.set_auto_render(False)
        self.widget.set_has_alpha(False)
        self.widget.set_has_depth_buffer(False)
        self.widget.set_has_stencil_buffer(False)
        self.widget.set_hexpand(True)
        self.widget.set_vexpand(True)
        self.widget.connect("realize", self._on_realize)
        self.widget.connect("render", self._on_render)
        self.widget.connect("unrealize", self._on_unrealize)

    def _bind_mpv_symbols(self) -> None:
        m = self._libmpv
        m.mpv_create.restype = ctypes.c_void_p
        m.mpv_set_option_string.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p]
        m.mpv_set_option_string.restype = ctypes.c_int
        m.mpv_initialize.argtypes = [ctypes.c_void_p]
        m.mpv_initialize.restype = ctypes.c_int
        m.mpv_set_property_string.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p]
        m.mpv_set_property_string.restype = ctypes.c_int
        m.mpv_command.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_char_p)]
        m.mpv_command.restype = ctypes.c_int
        m.mpv_terminate_destroy.argtypes = [ctypes.c_void_p]
        m.mpv_terminate_destroy.restype = None

        m.mpv_render_context_create.argtypes = [
            ctypes.POINTER(ctypes.c_void_p),
            ctypes.c_void_p,
            ctypes.POINTER(_MpvRenderParam),
        ]
        m.mpv_render_context_create.restype = ctypes.c_int
        m.mpv_render_context_set_update_callback.argtypes = [
            ctypes.c_void_p,
            ctypes.c_void_p,
            ctypes.c_void_p,
        ]
        m.mpv_render_context_set_update_callback.restype = None
        m.mpv_render_context_render.argtypes = [
            ctypes.c_void_p,
            ctypes.POINTER(_MpvRenderParam),
        ]
        m.mpv_render_context_render.restype = ctypes.c_int
        m.mpv_render_context_free.argtypes = [ctypes.c_void_p]
        m.mpv_render_context_free.restype = None

    @staticmethod
    def _load_gl_resolver(kind: str) -> tuple[Any, Any]:
        names = ("libEGL.so.1", "libEGL.so") if kind == "EGL" else ("libGL.so.1", "libGL.so")
        sym = "eglGetProcAddress" if kind == "EGL" else "glXGetProcAddressARB"
        for so in names:
            try:
                lib = ctypes.CDLL(so)
                fn = getattr(lib, sym, None)
                if fn is not None:
                    fn.argtypes = [ctypes.c_char_p]
                    fn.restype = ctypes.c_void_p
                    return lib, fn
            except OSError:
                continue
        return None, None

    def _on_realize(self, area: Any) -> None:
        if self._destroyed or self._mpv is None:
            return
        area.make_current()
        if area.get_error() is not None:
            return

        get_proc_type = ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_void_p, ctypes.c_char_p)
        egl_fn = self._egl_get_proc
        glx_fn = self._glx_get_proc

        def _get_proc(_ctx: Any, name: bytes) -> int:
            for resolver in (egl_fn, glx_fn):
                if resolver is not None:
                    ptr = resolver(name)
                    if ptr:
                        return int(ptr)
            return 0

        self._proc_cb = get_proc_type(_get_proc)
        init_params = _MpvOpenGlInitParams(
            get_proc_address=ctypes.cast(self._proc_cb, ctypes.c_void_p).value,
            get_proc_address_ctx=None,
        )
        api_type = ctypes.create_string_buffer(b"opengl")
        params = (_MpvRenderParam * 3)(
            _MpvRenderParam(1, ctypes.cast(api_type, ctypes.c_void_p).value),
            _MpvRenderParam(2, ctypes.addressof(init_params)),
            _MpvRenderParam(0, None),
        )
        ctx = ctypes.c_void_p(0)
        rc = self._libmpv.mpv_render_context_create(ctypes.byref(ctx), self._mpv, params)
        if rc < 0 or not ctx:
            raise RuntimeError(f"mpv_render_context_create failed ({rc})")
        self._render_ctx = ctx

        update_cb_type = ctypes.CFUNCTYPE(None, ctypes.c_void_p)

        def _on_mpv_update(_cb_ctx: Any) -> None:
            if not self._destroyed and not self._render_pending:
                self._render_pending = True
                self._GLib.idle_add(self._schedule_redraw)

        self._update_cb = update_cb_type(_on_mpv_update)
        self._libmpv.mpv_render_context_set_update_callback(
            self._render_ctx,
            ctypes.cast(self._update_cb, ctypes.c_void_p),
            None,
        )

        # Load and play the video file muted + looping.
        self.update_params(self._speed, 1.0)
        self._apply_brightness_int(self._brightness)
        args = (ctypes.c_char_p * 3)(
            b"loadfile",
            str(self._video_path).encode("utf-8"),
            None,
        )
        self._libmpv.mpv_command(self._mpv, args)
        if self._paused:
            self._libmpv.mpv_set_property_string(self._mpv, b"pause", b"yes")

    def _schedule_redraw(self) -> bool:
        self._render_pending = False
        if not self._destroyed:
            self.widget.queue_render()
        return False

    def _on_render(self, area: Any, _gl_ctx: Any) -> bool:
        if self._destroyed or not self._render_ctx:
            return False
        alloc = area.get_allocation()
        scale = max(1, int(area.get_scale_factor() or 1))
        w = max(1, int(alloc.width) * scale)
        h = max(1, int(alloc.height) * scale)

        fbo_val = ctypes.c_int(0)
        if self._libgl is not None:
            try:
                gl_get_int = self._libgl.glGetIntegerv
                gl_get_int.argtypes = [ctypes.c_uint, ctypes.POINTER(ctypes.c_int)]
                gl_get_int(0x8CA6, ctypes.byref(fbo_val))  # GL_FRAMEBUFFER_BINDING
            except Exception:
                fbo_val = ctypes.c_int(0)

        fbo_param = _MpvOpenGlFbo(fbo=fbo_val.value, w=w, h=h, internal_format=0)
        flip_y = ctypes.c_int(1)
        params = (_MpvRenderParam * 3)(
            _MpvRenderParam(3, ctypes.addressof(fbo_param)),  # MPV_RENDER_PARAM_OPENGL_FBO
            _MpvRenderParam(4, ctypes.addressof(flip_y)),     # MPV_RENDER_PARAM_FLIP_Y
            _MpvRenderParam(0, None),
        )
        self._libmpv.mpv_render_context_render(self._render_ctx, params)
        return True

    def _apply_brightness_int(self, mpv_bright: int) -> None:
        if self._destroyed or not self._mpv:
            return
        self._libmpv.mpv_set_property_string(
            self._mpv, b"brightness", str(int(mpv_bright)).encode("ascii")
        )

    def update_params(self, speed: float, brightness: float) -> None:
        self._speed = clamp_video_speed(speed)
        self._brightness = map_brightness_to_mpv(brightness)
        if self._destroyed or not self._mpv:
            return
        self._libmpv.mpv_set_property_string(
            self._mpv, b"speed", f"{self._speed:.2f}".encode("ascii")
        )
        self._apply_brightness_int(self._brightness)

    def set_paused(self, paused: bool) -> None:
        p = bool(paused)
        if self._paused == p:
            return
        self._paused = p
        if not self._destroyed and self._mpv:
            self._libmpv.mpv_set_property_string(
                self._mpv, b"pause", b"yes" if p else b"no"
            )

    def _on_unrealize(self, _area: Any) -> None:
        self._cleanup_mpv()

    def _cleanup_mpv(self) -> None:
        if self._render_ctx:
            try:
                self._libmpv.mpv_render_context_set_update_callback(
                    self._render_ctx, None, None
                )
                self._libmpv.mpv_render_context_free(self._render_ctx)
            except Exception:
                pass
            self._render_ctx = ctypes.c_void_p(0)
        if self._mpv:
            try:
                self._libmpv.mpv_terminate_destroy(self._mpv)
            except Exception:
                pass
            self._mpv = None
        self._proc_cb = None
        self._update_cb = None

    def destroy(self) -> None:
        if self._destroyed:
            return
        self._destroyed = True
        self._cleanup_mpv()
        try:
            self.widget.destroy()
        except Exception:
            pass


# ----------------------------------------------------------------------------
# Zero-WebKitGTK GStreamer GTK sink fallback (Wayland & X11)
# ----------------------------------------------------------------------------
class GstVideoWidget:
    """Hardware/accelerated GStreamer ``playbin`` + ``gtkglsink``/``gtksink`` widget."""

    backend_name = "gstreamer"

    def __init__(
        self,
        video_path: Path,
        *,
        speed: float = 1.0,
        brightness: float = 1.0,
        log: Any = None,
    ) -> None:
        import gi  # type: ignore[import-not-found]

        gi.require_version("Gst", "1.0")
        from gi.repository import Gst  # type: ignore[import-not-found]

        if not Gst.is_initialized():
            Gst.init(None)

        self._Gst = Gst
        self._log = log
        self._speed = clamp_video_speed(speed)
        self._brightness = float(brightness)
        self._paused = False
        self._destroyed = False

        sink = Gst.ElementFactory.make("gtkglsink", "dd_sink")
        if sink is None:
            sink = Gst.ElementFactory.make("gtksink", "dd_sink")
        if sink is None:
            raise RuntimeError("No gtkglsink or gtksink element available")

        self.widget = sink.props.widget
        self.widget.set_hexpand(True)
        self.widget.set_vexpand(True)
        try:
            sink.set_property("force-aspect-ratio", False)
        except Exception:
            pass

        self._playbin = Gst.ElementFactory.make("playbin", "dd_playbin")
        if self._playbin is None:
            raise RuntimeError("GStreamer playbin unavailable")
        self._playbin.set_property("uri", video_path.resolve().as_uri())
        self._playbin.set_property("video-sink", sink)
        fakesink = Gst.ElementFactory.make("fakesink", "dd_null_audio")
        if fakesink is not None:
            self._playbin.set_property("audio-sink", fakesink)
        self._playbin.set_property("mute", True)
        self._playbin.connect("about-to-finish", self._on_about_to_finish)

        bus = self._playbin.get_bus()
        if bus is not None:
            bus.add_signal_watch()
            bus.connect("message::eos", self._on_eos)

        self._playbin.set_state(Gst.State.PLAYING)

    def _on_about_to_finish(self, playbin: Any) -> None:
        if not self._destroyed:
            uri = playbin.get_property("current-uri")
            if uri:
                playbin.set_property("uri", uri)

    def _on_eos(self, _bus: Any, _msg: Any) -> None:
        if self._destroyed:
            return
        Gst = self._Gst
        self._playbin.seek(
            self._speed,
            Gst.Format.TIME,
            Gst.SeekFlags.FLUSH | Gst.SeekFlags.KEY_UNIT,
            Gst.SeekType.SET,
            0,
            Gst.SeekType.NONE,
            0,
        )

    def update_params(self, speed: float, brightness: float) -> None:
        self._speed = clamp_video_speed(speed)
        self._brightness = float(brightness)

    def set_paused(self, paused: bool) -> None:
        p = bool(paused)
        if self._paused == p or self._destroyed:
            return
        self._paused = p
        Gst = self._Gst
        self._playbin.set_state(Gst.State.PAUSED if p else Gst.State.PLAYING)

    def destroy(self) -> None:
        if self._destroyed:
            return
        self._destroyed = True
        try:
            bus = self._playbin.get_bus()
            if bus is not None:
                bus.remove_signal_watch()
        except Exception:
            pass
        try:
            self._playbin.set_state(self._Gst.State.NULL)
        except Exception:
            pass
        try:
            self.widget.destroy()
        except Exception:
            pass


def create_native_video_widget(
    video_path: Path,
    display_backend: str,
    *,
    speed: float = 1.0,
    brightness: float = 1.0,
    log: Any = None,
) -> LibMpvGlArea | GstVideoWidget | None:
    """Instantiate the best Wayland/X11 native widget for ``video_path``, or ``None``."""
    if not video_path.is_file() or display_backend in ("plasma-plugin", "static"):
        return None
    if find_libmpv_path() is not None:
        try:
            return LibMpvGlArea(
                video_path, speed=speed, brightness=brightness, log=log
            )
        except Exception as exc:
            if log is not None:
                log.exception("libmpv Gtk.GLArea baslatilamadi; GStreamer deneniyor", exc)
    if has_gstreamer_gtk():
        try:
            return GstVideoWidget(
                video_path, speed=speed, brightness=brightness, log=log
            )
        except Exception as exc:
            if log is not None:
                log.exception("GStreamer video widget baslatilamadi", exc)
    return None
