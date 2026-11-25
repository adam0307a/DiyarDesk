"""Wallpaper surface: WebKit page + X11/layer-shell/static attachment."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import threading
import time
from pathlib import Path
from typing import Any, Callable

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
gi.require_version("Gio", "2.0")

from gi.repository import Gdk, Gio, GLib, Gtk  # noqa: E402

from .audio import NativeAudioPlayer, resolve_theme_audio_path  # noqa: E402
from .bridge import BridgeWebView  # noqa: E402
from .config import config_dir  # noqa: E402
from .desktop_env import (  # noqa: E402
    COSMIC_PANEL_INSETS,
    is_cosmic_desktop,
    select_wallpaper_backend,
)
from .displays import MonitorInfo, detect_compositor_monitors  # noqa: E402
from .icons import make_adapter  # noqa: E402
from .mpv_player import MpvVideoBackend, find_theme_video_file  # noqa: E402
from .native_gl import (  # noqa: E402
    NativeGlArea,
    NativeGlHeadless,
    NativeGlLibrary,
    should_use_native_gl,
)
from .pause import PauseMonitor  # noqa: E402
from .power import PowerMonitor, PowerState  # noqa: E402
from .scheduler import compute_solar_phase, detect_os_dark_mode  # noqa: E402
from .static_wallpaper import StaticWallpaper  # noqa: E402
from .sysbridge import LocalAudioMeter, SystemStatsSampler  # noqa: E402
from .virtual_icons import VirtualIconsOverlay, merge_host_and_virtual_icons  # noqa: E402
from .watchdog import ResourceWatchdog, find_webkit_child_pids, sample_processes  # noqa: E402
from .windows import detect_active_windows, detect_workspace_state  # noqa: E402


def plasma_pause_path() -> Path:
    """Return path to the KDE Plasma wallpaper manual-pause state file."""
    base = Path(os.environ.get("XDG_CACHE_HOME") or (Path.home() / ".cache"))
    return base / "deskland" / "pause.json"


def load_layer_shell():
    try:
        gi.require_version("GtkLayerShell", "0.1")
        from gi.repository import GtkLayerShell  # noqa: F401

        # Compositors without wlr-layer-shell (e.g. GNOME Shell / Mutter) still
        # ship the library and assert inside init_for_window() if not guarded.
        if hasattr(GtkLayerShell, "is_supported") and not GtkLayerShell.is_supported():
            return None
        return GtkLayerShell
    except (ValueError, ImportError):
        return None


def _is_x11_display() -> bool:
    display = Gdk.Display.get_default()
    if display is not None:
        name = type(display).__name__.lower()
        if "x11" in name:
            return True
        if "wayland" in name:
            return False
    return bool(os.environ.get("DISPLAY")) and not os.environ.get("WAYLAND_DISPLAY")


def _has_gst_autoaudiosink() -> bool:
    """Return False when GStreamer lacks autoaudiosink (prevents WebKitGTK hang)."""
    try:
        gi.require_version("Gst", "1.0")
        from gi.repository import Gst

        Gst.init(None)
        return Gst.ElementFactory.find("autoaudiosink") is not None
    except Exception:
        pass
    gst_inspect = shutil.which("gst-inspect-1.0")
    if not gst_inspect:
        return True
    try:
        proc = subprocess.run(
            [gst_inspect, "autoaudiosink"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=2,
            check=False,
        )
        return proc.returncode == 0
    except Exception:
        return True


def _has_plasma_wallpaper_plugin(plugin_id: str = "org.deskland.preview") -> bool:
    data_home = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share"))
    candidates = [data_home / "plasma" / "wallpapers" / plugin_id]
    for base in os.environ.get("XDG_DATA_DIRS", "/usr/local/share:/usr/share").split(":"):
        if base:
            candidates.append(Path(base) / "plasma" / "wallpapers" / plugin_id)
    return any((c / "metadata.json").is_file() for c in candidates)


def call_gnome_shell_extension(method: str, timeout_ms: int = 300) -> Any | None:
    """Invoke a zero-arg method on org.deskland.GnomeShell and unpack the result."""
    try:
        bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        res = bus.call_sync(
            "org.deskland.GnomeShell",
            "/org/deskland/GnomeShell",
            "org.deskland.GnomeShell",
            method,
            None,
            None,
            Gio.DBusCallFlags.NO_AUTO_START,
            timeout_ms,
            None,
        )
        unpacked = res.unpack()
        return unpacked[0] if unpacked else None
    except Exception:
        return None


def has_gnome_wallpaper_extension() -> bool:
    return call_gnome_shell_extension("Ping") == "ok"


def compute_caelestia_insets(shell_config: dict[str, Any] | None = None) -> tuple[int, int, int, int]:
    """Return safe (top, right, bottom, left) px to clear Caelestia's empty-workspace drawer mask."""
    cfg = shell_config if isinstance(shell_config, dict) else {}
    border_cfg = cfg.get("border") if isinstance(cfg.get("border"), dict) else {}
    try:
        border_thickness = max(0, int(border_cfg.get("thickness", 10)))
    except (TypeError, ValueError):
        border_thickness = 10

    defaults = {"dashboard": 50, "launcher": 50, "session": 30, "sidebar": 80}
    thresholds: list[int] = [0]
    for panel, default_val in defaults.items():
        panel_cfg = cfg.get(panel) if isinstance(cfg.get(panel), dict) else {}
        if panel_cfg.get("enabled", True):
            try:
                thresholds.append(max(0, int(panel_cfg.get("dragThreshold", default_val))))
            except (TypeError, ValueError):
                thresholds.append(default_val)

    drag_padding = max(thresholds)
    if drag_padding <= 0:
        return (0, 0, 0, 0)

    edge = border_thickness + drag_padding + 8
    # Caelestia places its left bar inside bar.clampedWidth (~48px) before dragMaskPadding.
    left = edge + 48
    return (edge, edge, edge, left)


def _has_layer_namespace(layers_data: Any, target: str) -> bool:
    if not isinstance(layers_data, dict):
        return False
    for monitor_info in layers_data.values():
        if not isinstance(monitor_info, dict):
            continue
        levels = monitor_info.get("levels")
        if not isinstance(levels, dict):
            continue
        for level_key in ("2", "3", "top", "overlay"):
            entries = levels.get(level_key)
            if not isinstance(entries, list):
                continue
            for entry in entries:
                if isinstance(entry, dict) and entry.get("namespace") == target:
                    return True
    return False


def detect_layer_shell_safe_insets(
    backend: str,
    layers_data: Any = None,
    caelestia_cfg: dict[str, Any] | None = None,
) -> tuple[int, int, int, int]:
    """Detect edge zones captured by upper layer-shell/shell panels."""
    if backend not in ("layer-shell", "gnome-shell"):
        return (0, 0, 0, 0)

    raw_override = os.environ.get("DESKLAND_SAFE_INSETS", "").strip()
    if raw_override:
        parts = [p.strip() for p in raw_override.split(",")]
        if len(parts) == 4:
            try:
                vals = tuple(max(0, min(400, int(p))) for p in parts)
                return (vals[0], vals[1], vals[2], vals[3])
            except ValueError:
                pass

    if backend == "gnome-shell":
        # GNOME Shell's top bar (~32px) sits above the wallpaper surface.
        return (36, 0, 0, 0)

    if backend == "layer-shell" and is_cosmic_desktop():
        # COSMIC's default top panel sits above the background layer.
        return COSMIC_PANEL_INSETS

    if layers_data is None:
        if not os.environ.get("HYPRLAND_INSTANCE_SIGNATURE"):
            return (0, 0, 0, 0)
        hyprctl = shutil.which("hyprctl")
        if not hyprctl:
            return (0, 0, 0, 0)
        try:
            proc = subprocess.run(
                [hyprctl, "-j", "layers"],
                capture_output=True,
                text=True,
                timeout=1.5,
                check=False,
            )
            if proc.returncode != 0 or not proc.stdout:
                return (0, 0, 0, 0)
            layers_data = json.loads(proc.stdout)
        except (OSError, subprocess.SubprocessError, ValueError):
            return (0, 0, 0, 0)

    if not _has_layer_namespace(layers_data, "caelestia-drawers"):
        return (0, 0, 0, 0)

    if caelestia_cfg is None:
        cfg_path = (
            Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
            / "caelestia"
            / "shell.json"
        )
        try:
            if cfg_path.is_file():
                caelestia_cfg = json.loads(cfg_path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            caelestia_cfg = None

    return compute_caelestia_insets(caelestia_cfg)


def _build_safe_inset_script(insets: tuple[int, int, int, int]) -> str:
    """Build a UserScript that keeps small fixed corner UI controls inside clickable bounds."""
    top, right, bottom, left = insets
    payload = json.dumps({"top": top, "right": right, "bottom": bottom, "left": left})
    return f"""(function () {{
  var SAFE = {payload};
  window.__desklandSafeInsets = SAFE;
  var INTERACTIVE = 'button,select,input,a,[role="button"]';
  function adjustElement(el) {{
    if (!el || el.nodeType !== 1 || el.dataset.desklandSafe === '1') return;
    var cs = getComputedStyle(el);
    if (cs.position !== 'fixed' || cs.display === 'none' || cs.visibility === 'hidden' || cs.pointerEvents === 'none') return;
    var r = el.getBoundingClientRect();
    if (r.width <= 12 || r.height <= 12) return;
    if (r.width >= window.innerWidth * 0.75 || r.height >= window.innerHeight * 0.45) return;
    if (!el.matches(INTERACTIVE) && !el.querySelector(INTERACTIVE)) return;

    if (SAFE.top > 0 && r.top < SAFE.top && (r.bottom + (SAFE.top - r.top)) <= (window.innerHeight - SAFE.bottom)) {{
      el.style.top = SAFE.top + 'px';
      el.style.bottom = 'auto';
    }} else if (SAFE.bottom > 0 && (window.innerHeight - r.bottom) < SAFE.bottom) {{
      el.style.bottom = SAFE.bottom + 'px';
      el.style.top = 'auto';
    }}
    if (SAFE.left > 0 && r.left < SAFE.left && (r.right + (SAFE.left - r.left)) <= (window.innerWidth - SAFE.right)) {{
      el.style.left = SAFE.left + 'px';
      el.style.right = 'auto';
    }} else if (SAFE.right > 0 && (window.innerWidth - r.right) < SAFE.right) {{
      el.style.right = SAFE.right + 'px';
      el.style.left = 'auto';
    }}
  }}
  function applyAll() {{
    var nodes = document.body ? document.body.querySelectorAll('*') : [];
    for (var i = 0; i < nodes.length; i++) adjustElement(nodes[i]);
  }}
  var timer = 0;
  function schedule() {{
    if (timer) return;
    timer = setTimeout(function () {{ timer = 0; applyAll(); }}, 40);
  }}
  if (document.readyState === 'loading') {{
    document.addEventListener('DOMContentLoaded', function () {{
      applyAll();
      setTimeout(applyAll, 200);
      setTimeout(applyAll, 700);
    }});
  }} else {{
    applyAll();
    setTimeout(applyAll, 200);
    setTimeout(applyAll, 700);
  }}
  try {{
    var obs = new MutationObserver(schedule);
    obs.observe(document.documentElement, {{ subtree: true, childList: true, attributes: true, attributeFilter: ['class', 'style'] }});
    setTimeout(function () {{ obs.disconnect(); }}, 8000);
  }} catch (_) {{}}
}})();"""


def primary_geometry() -> tuple[Any, int]:
    display = Gdk.Display.get_default()
    if display is None:
        raise RuntimeError("GDK display yok")
    monitor = display.get_primary_monitor() or display.get_monitor(0)
    if monitor is None:
        raise RuntimeError("Monitör bulunamadı")
    return monitor.get_geometry(), max(1, monitor.get_scale_factor())


class Wallpaper:
    """Owns the wallpaper view and all host <-> page messages."""

    def __init__(
        self,
        host: Any,
        server: Any,
        log: Any,
        backend: str = "auto",
        page: str = "flat.html",
        adapter: str | None = None,
        monitor_info: MonitorInfo | None = None,
        gdk_monitor: Any = None,
        is_primary: bool = True,
        theme_id: str | None = None,
    ) -> None:
        self.host = host
        self.server = server
        self.log = log
        self._page = page
        self._theme_id = str(
            theme_id
            or getattr(getattr(self.host, "settings", None), "theme", "")
            or ""
        )
        self._native_gl: NativeGlArea | None = None
        self._native_headless: NativeGlHeadless | None = None
        self._virtual_overlay: VirtualIconsOverlay | None = None
        self._real_icons_raw: tuple[bool, list[list[int]], list[str]] = (False, [], [])
        self._ui_rects: list[tuple[int, int, int, int]] = []
        self._adapter_lock = threading.Lock()
        self._adapter = adapter
        self._last_icons: str | None = None
        self._last_pause: int = -1
        self._manual_pause = False
        self._ready = False
        self._icon_busy = False
        self._pause_busy = False
        self._snapshot_done: Callable[[str | None, str | None, str | None], None] | None = None
        self._snapshot_timeout = 0
        self._pending_snapshot = False
        self._record_done: Callable[[str | None, str | None, str | None, str | None], None] | None = None
        self._record_timeout = 0
        self._pending_record = False
        self._static_timeout = 0
        self._static_file: Path | None = None
        self._static_snapshot_pending = False
        self._icon_timeout = 0
        self._pause_timeout = 0
        self._pointer_timeout = 0
        self._pointer_active = False
        self._last_pointer: tuple[int, int] | None = None
        self._low_mode = bool(getattr(getattr(self.host, "settings", None), "low_mode", False))
        self._disable_auto_pause = bool(getattr(getattr(self.host, "settings", None), "disable_auto_pause", False))
        self._closed = False
        self._is_primary = bool(is_primary)

        if monitor_info is not None:
            geo = Gdk.Rectangle()
            geo.x = monitor_info.x
            geo.y = monitor_info.y
            geo.width = monitor_info.width
            geo.height = monitor_info.height
            scale: float = float(monitor_info.scale)
        else:
            geo, int_scale = primary_geometry()
            scale = float(int_scale)
            # Prefer exact fractional scale from Wayland compositor when available.
            for cm in detect_compositor_monitors():
                if cm.primary:
                    scale = float(cm.scale)
                    break
        self.geo = geo
        self.scale = scale
        self.screen_w = max(1, int(round(geo.width * scale)))
        self.screen_h = max(1, int(round(geo.height * scale)))
        self.monitor = gdk_monitor or (
            Gdk.Display.get_default().get_primary_monitor()
            or Gdk.Display.get_default().get_monitor(0)
        )
        self.icon_adapter = make_adapter(
            self.host.desktop_dir(),
            scale,
            self.screen_w,
            self.screen_h,
            origin_x=geo.x,
            origin_y=geo.y,
            forced=self._adapter,
        )
        self.pause_monitor = PauseMonitor(
            int(round(geo.x * scale)),
            int(round(geo.y * scale)),
            self.screen_w,
            self.screen_h,
            scale,
            log,
        )
        self.pause_monitor.low_mode = self._low_mode
        self.power_monitor = PowerMonitor()
        self._last_power_perf: tuple[bool, bool] | None = None
        self._crash_times: list[float] = []
        self._watchdog_timeout = 0
        self.resource_watchdog = ResourceWatchdog()
        self._last_watchdog_check = 0.0
        self.mpv_backend = MpvVideoBackend(log)
        self.stats_sampler = SystemStatsSampler()
        self.audio_meter = LocalAudioMeter()
        self._last_stats_ts = 0.0
        self.backend = self._select_backend(backend)
        self._safe_insets = detect_layer_shell_safe_insets(self.backend)
        self.static_wallpaper = StaticWallpaper(log)
        self._native_audio = NativeAudioPlayer(log)
        self._audio_supported = _has_gst_autoaudiosink()
        if not self._audio_supported:
            self.log.write(
                "GStreamer autoaudiosink bulunamadı; duvar kağıdı sesi yerel oynatıcıya yönlendirildi"
            )

        self.bridge: BridgeWebView | None = None
        self.web = None
        if self.backend != "plasma-plugin" and not self._should_use_native_gl(
            self._page, self._theme_id
        ):
            self._init_bridge()
        self.window: Gtk.Window | None = None
        self._ui_rect: tuple[int, int, int, int] | None = None
        self._create_surface()
        self.log.write(
            f"Duvar kağıdı yüzeyi: backend={self.backend}, "
            f"ekran={self.screen_w}x{self.screen_h}@{scale}, "
            f"simge adaptörü={self.icon_adapter.name}"
        )
        self._reschedule_poll_timers()

    def _is_low_mode(self) -> bool:
        settings = getattr(self.host, "settings", None)
        if settings is not None and hasattr(settings, "low_mode"):
            self._low_mode = bool(settings.low_mode)
        return bool(getattr(self, "_low_mode", False))

    def _reschedule_poll_timers(self) -> None:
        if getattr(self, "_icon_timeout", 0):
            GLib.source_remove(self._icon_timeout)
            self._icon_timeout = 0
        if getattr(self, "_pause_timeout", 0):
            GLib.source_remove(self._pause_timeout)
            self._pause_timeout = 0
        if getattr(self, "_pointer_timeout", 0):
            GLib.source_remove(self._pointer_timeout)
            self._pointer_timeout = 0
        if self._closed:
            return
        low = self._is_low_mode()
        if self.backend == "plasma-plugin":
            icon_interval = 3500 if low else 2000
        else:
            icon_interval = 1500 if low else 400
        pause_interval = 1000 if low else 500
        self._icon_timeout = GLib.timeout_add(icon_interval, self._poll_icons)
        self._pause_timeout = GLib.timeout_add(pause_interval, self._poll_pause)
        if self.backend == "x11":
            pointer_interval = 80 if low else 40
            self._pointer_timeout = GLib.timeout_add(pointer_interval, self._poll_x11_pointer)

    def _init_bridge(self) -> None:
        if self.bridge is not None:
            return
        self.bridge = BridgeWebView(self._on_message)
        server_port = getattr(getattr(self, "server", None), "port", None)
        if hasattr(self.bridge, "install_navigation_guard"):
            self.bridge.install_navigation_guard(allowed_port=server_port or None)
        if hasattr(self.bridge, "set_process_terminated_handler"):
            self.bridge.set_process_terminated_handler(self._on_web_process_terminated)
        low_js_path = getattr(getattr(self, "server", None), "prototype", None)
        if low_js_path is not None:
            candidate = Path(low_js_path) / "low-mode.js"
            if candidate.is_file():
                try:
                    self.bridge.add_user_script(candidate.read_text(encoding="utf-8"), at_end=False)
                except OSError:
                    pass
        if any(self._safe_insets):
            self.bridge.add_user_script(_build_safe_inset_script(self._safe_insets), at_end=True)
            self.log.write(f"Katman güvenli kenar boşluğu: {self._safe_insets}")
        self.web = self.bridge.webview
        if not self._audio_supported and hasattr(self.web, "set_is_muted"):
            try:
                self.web.set_is_muted(True)
            except Exception:
                pass

    def _write_plasma_pause(self, paused: bool) -> None:
        path = plasma_pause_path()
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            tmp = path.with_suffix(".json.tmp")
            tmp.write_text(
                json.dumps({"paused": bool(paused), "ts": int(time.time() * 1000)}),
                encoding="utf-8",
            )
            os.replace(tmp, path)
        except OSError as exc:
            self.log.exception("Plasma pause durumu yazılamadı", exc)

    # ---------------------------------------------------------------- surface
    def _select_backend(self, requested: str) -> str:
        return select_wallpaper_backend(
            requested,
            os.environ,
            has_plasma_plugin=_has_plasma_wallpaper_plugin(),
            has_gnome_extension=has_gnome_wallpaper_extension(),
            has_layer_shell=load_layer_shell() is not None,
        )

    def _ensure_plasma_plugin(self) -> None:
        dbus = "qdbus6" if shutil.which("qdbus6") else "qdbus"
        if not shutil.which(dbus):
            return
        script = (
            "var ds = desktops();"
            "for (var i = 0; i < ds.length; i++) {"
            'if (ds[i].wallpaperPlugin !== "org.deskland.preview") {'
            'ds[i].wallpaperPlugin = "org.deskland.preview";'
            "}"
            "}"
        )
        try:
            subprocess.run(
                [
                    dbus,
                    "org.kde.plasmashell",
                    "/PlasmaShell",
                    "org.kde.PlasmaShell.evaluateScript",
                    script,
                ],
                capture_output=True,
                timeout=5,
                check=False,
            )
        except (OSError, subprocess.SubprocessError):
            pass

    def _attach_layer_shell(
        self, window: Gtk.Window, layer_shell: Any, namespace: str
    ) -> None:
        layer_shell.init_for_window(window)
        layer_shell.set_namespace(window, namespace)
        layer = layer_shell.Layer.BACKGROUND
        if namespace == "deskland":
            override = os.environ.get("DESKLAND_LAYER", "").lower()
            if override in ("background", "bottom", "top", "overlay"):
                layer = getattr(layer_shell.Layer, override.upper())
        layer_shell.set_layer(window, layer)
        for edge in (
            layer_shell.Edge.TOP,
            layer_shell.Edge.BOTTOM,
            layer_shell.Edge.LEFT,
            layer_shell.Edge.RIGHT,
        ):
            layer_shell.set_anchor(window, edge, True)
        layer_shell.set_exclusive_zone(window, -1)
        layer_shell.set_keyboard_mode(window, layer_shell.KeyboardMode.NONE)
        if self.monitor is not None:
            layer_shell.set_monitor(window, self.monitor)

    def _create_hidden_render_surface(self, log_line: str) -> None:
        # OffscreenWindow has no GdkWindow, so WebKitGTK cannot create a GL
        # context. On Wayland, attach as a background layer-shell surface so KWin
        # never gives it an xdg_toplevel titlebar, taskbar icon, or click area.
        surface = Gtk.Window(type=Gtk.WindowType.TOPLEVEL)
        surface.set_title("DiyarDeskRender")
        surface.set_decorated(False)
        surface.set_resizable(False)
        surface.set_skip_taskbar_hint(True)
        surface.set_skip_pager_hint(True)
        surface.set_accept_focus(False)
        surface.set_focus_on_map(False)
        surface.set_can_focus(False)
        surface.set_opacity(0.0)
        layer_shell = (
            load_layer_shell() if os.environ.get("WAYLAND_DISPLAY") else None
        )
        offscreen_xy: tuple[int, int] | None = None
        if layer_shell is not None:
            self._attach_layer_shell(surface, layer_shell, "deskland-render")
        else:
            surface.set_size_request(self.geo.width, self.geo.height)
            surface.resize(self.geo.width, self.geo.height)
            if _is_x11_display():
                # Keep X11 static/Plasma render surfaces off-screen and unmanaged:
                # set_opacity(0.0) has no effect without a compositor, so mapping
                # a DESKTOP window at (0,0) would cover xfdesktop icons visually
                # while passing clicks through underneath.
                off_x = min(0, self.geo.x) - max(1, self.geo.width) - 4096
                off_y = min(0, self.geo.y) - max(1, self.geo.height) - 4096
                offscreen_xy = (off_x, off_y)
                surface.connect("realize", self._on_x11_offscreen_realize, off_x, off_y)
                surface.move(off_x, off_y)
            else:
                surface.set_keep_below(True)
        if hasattr(self.web, "set_is_muted"):
            try:
                self.web.set_is_muted(True)
            except Exception:
                pass
        surface.connect("realize", self._on_realize)
        surface.add(self.web)
        surface.show_all()
        if offscreen_xy is not None:
            surface.move(*offscreen_xy)
        elif layer_shell is None:
            # Wayland compositors without layer-shell (GNOME Mutter) ignore
            # window.move(), so iconify keeps the 0%-opacity render helper out
            # of the user's workspace while preserving WebGL snapshot capture.
            surface.iconify()
        self.window = surface
        self.log.write(log_line)

    def _ensure_share_renderer(self) -> None:
        if self._closed or self.backend != "plasma-plugin" or self.bridge is not None:
            return
        self._init_bridge()
        self._create_hidden_render_surface(
            "KDE Plasma paylaşım oluşturucusu geçici olarak başlatıldı"
        )
        if self.bridge is not None:
            self._ready = False
            self.bridge.load_uri(self.url_for(self._page))

    def _release_share_renderer(self) -> bool:
        if (
            self.backend != "plasma-plugin"
            or self._snapshot_done is not None
            or self._record_done is not None
        ):
            return False
        self._ready = False
        self._pending_snapshot = False
        self._pending_record = False
        if self.bridge is not None:
            try:
                self.bridge.load_uri("about:blank")
            except Exception:
                pass
        if self.window is not None:
            try:
                self.window.destroy()
            except Exception:
                pass
            self.window = None
        self.bridge = None
        self.web = None
        return False

    def _create_surface(self) -> None:
        if self.backend == "plasma-plugin":
            # org.deskland.preview renders live inside plasmashell and reads
            # settings.json + icons.json directly. Avoid keeping a second idle
            # WebKitWebProcess alive; spawn the hidden renderer lazily only
            # when share snapshot/recording is explicitly requested.
            self._ensure_plasma_plugin()
            self._write_plasma_pause(self._manual_pause)
            self.log.write("KDE Plasma duvar kağıdı eklentisi aktif: org.deskland.preview")
            return
        if self.backend == "static":
            if self._should_use_native_gl(self._page, self._theme_id):
                return
            self._create_hidden_render_surface(
                "Statik fallback: GNOME/KWin Wayland veya --static"
            )
            return

        window = Gtk.Window(type=Gtk.WindowType.TOPLEVEL)
        window.set_title("DiyarDeskWallpaper")
        window.set_decorated(False)
        window.set_resizable(False)
        window.set_skip_taskbar_hint(True)
        window.set_skip_pager_hint(True)
        window.set_accept_focus(False)
        window.set_focus_on_map(False)
        window.set_can_focus(False)
        window.connect("realize", self._on_realize)

        if self.backend == "layer-shell":
            layer_shell = load_layer_shell()
            if layer_shell is None:
                self.backend = "x11" if os.environ.get("DISPLAY") else "static"
                return self._create_surface()
            self._attach_layer_shell(window, layer_shell, "deskland")
        elif self.backend == "gnome-shell":
            window.set_default_size(self.geo.width, self.geo.height)
            window.set_size_request(self.geo.width, self.geo.height)
            window.resize(self.geo.width, self.geo.height)
        else:
            window.set_type_hint(Gdk.WindowTypeHint.DESKTOP)
            window.set_keep_below(True)
            window.stick()
            window.set_default_size(self.geo.width, self.geo.height)
            window.set_size_request(self.geo.width, self.geo.height)
            window.move(self.geo.x, self.geo.y)
            window.resize(self.geo.width, self.geo.height)

        if self.web is not None:
            window.add(self.web)
        window.show_all()
        self.window = window
        if self.backend == "gnome-shell":
            GLib.idle_add(lambda: (call_gnome_shell_extension("LowerWallpaper"), False)[1])
        elif self.backend == "x11":
            GLib.timeout_add_seconds(2, self._keep_below)

    def _apply_input_region(self) -> None:
        if self._closed or self.window is None:
            return
        gdk_window = self.window.get_window()
        if gdk_window is None:
            return
        try:
            import cairo

            click_through_layer_shell = bool(os.environ.get("DESKLAND_CLICK_THROUGH"))
            if self.backend in ("layer-shell", "gnome-shell") and not click_through_layer_shell:
                # Wayland layer-shell desktops have no underlying desktop-icon
                # window, so allow pointer clicks/drags on uncovered desktop areas.
                region = cairo.Region(
                    cairo.RectangleInt(0, 0, max(1, self.geo.width), max(1, self.geo.height))
                )
                pass_through = False
            elif self.backend == "x11" and (
                getattr(self, "_ui_rects", None)
                or (
                    self._ui_rect is not None
                    and self._ui_rect[2] > 0
                    and self._ui_rect[3] > 0
                )
            ):
                region = cairo.Region()
                ui_rects = list(getattr(self, "_ui_rects", []) or [])
                if not ui_rects and self._ui_rect is not None:
                    ui_rects = [self._ui_rect]
                for x, y, w, h in ui_rects:
                    if w > 0 and h > 0:
                        region.union(cairo.RectangleInt(x, y, w, h))
                pass_through = False
            else:
                region = cairo.Region()
                pass_through = True

            targets: list[tuple[Any, Any]] = [
                (self.window, gdk_window),
            ]
            v_overlay = getattr(self, "_virtual_overlay", None)
            native_gl = getattr(self, "_native_gl", None)
            active_widget = (
                v_overlay.widget
                if v_overlay is not None
                else (native_gl.widget if native_gl is not None else self.web)
            )
            if active_widget is not None:
                targets.append((active_widget, active_widget.get_window()))
            try:
                for child_gw in gdk_window.get_children():
                    targets.append((None, child_gw))
            except (AttributeError, TypeError):
                pass

            for widget, gdk_win in targets:
                if widget is not None:
                    try:
                        widget.input_shape_combine_region(region)
                    except (AttributeError, TypeError):
                        pass
                if gdk_win is not None:
                    try:
                        gdk_win.set_pass_through(pass_through)
                    except (AttributeError, TypeError):
                        pass
                    try:
                        gdk_win.input_shape_combine_region(region, 0, 0)
                    except (AttributeError, TypeError):
                        pass
        except (ImportError, AttributeError, TypeError):
            pass

    def _on_ui_rect(self, raw: Any) -> None:
        if isinstance(raw, (list, tuple)) and len(raw) == 4:
            try:
                x, y, w, h = (int(raw[0]), int(raw[1]), int(raw[2]), int(raw[3]))
                if w > 0 and h > 0:
                    self._ui_rect = (x, y, w, h)
                    self._apply_input_region()
                    return
            except (TypeError, ValueError):
                pass
        self._ui_rect = None
        self._apply_input_region()

    @staticmethod
    def _on_x11_offscreen_realize(window: Gtk.Window, off_x: int, off_y: int) -> None:
        gdk_window = window.get_window()
        if gdk_window is not None:
            try:
                gdk_window.set_override_redirect(True)
            except (AttributeError, TypeError):
                pass
        window.move(off_x, off_y)

    def _on_realize(self, window: Gtk.Window) -> None:
        gdk_window = window.get_window()
        if gdk_window is None:
            return
        self._apply_input_region()
        if self.backend == "x11":
            try:
                gdk_window.set_type_hint(Gdk.WindowTypeHint.DESKTOP)
                gdk_window.lower()
            except (AttributeError, TypeError):
                pass

    def _keep_below(self) -> bool:
        if self._closed or self.backend != "x11" or self.window is None:
            return False
        gdk_window = self.window.get_window()
        if gdk_window is not None:
            try:
                gdk_window.lower()
            except (AttributeError, TypeError):
                pass
        return True

    # ---------------------------------------------------------------- native GL
    @property
    def native_gl_active(self) -> bool:
        native_gl = getattr(self, "_native_gl", None)
        native_hl = getattr(self, "_native_headless", None)
        return bool(
            (native_gl is not None and native_gl.is_running)
            or (native_hl is not None and native_hl.is_running)
            or native_gl is not None
        )

    def _should_use_native_gl(self, page: str, theme_id: str) -> bool:
        if self.backend == "plasma-plugin":
            return False
        settings = getattr(self.host, "settings", None)
        rust_opengl = bool(getattr(settings, "rust_opengl", False))
        return should_use_native_gl(
            page,
            theme_id,
            rust_opengl=rust_opengl,
            available=NativeGlLibrary.is_available(),
        )

    def _post_active(self, payload: Any) -> None:
        native_gl = getattr(self, "_native_gl", None)
        native_hl = getattr(self, "_native_headless", None)
        if native_gl is not None:
            native_gl.post_json(payload)
        elif native_hl is not None:
            native_hl.post_json(payload)
        elif self.bridge is not None:
            self.bridge.post(payload)

    def _attach_window_child(self, widget: Any) -> None:
        if self.window is None or widget is None:
            return
        try:
            for child in list(self.window.get_children()):
                if child is not widget:
                    self.window.remove(child)
        except Exception:
            pass
        if widget.get_parent() is None:
            self.window.add(widget)
        self.window.show_all()
        self._apply_input_region()

    def _release_webview(self) -> None:
        if self.bridge is not None:
            try:
                self.bridge.load_uri("about:blank")
            except Exception:
                pass
        if self.web is not None:
            try:
                parent = self.web.get_parent()
                if parent is not None:
                    parent.remove(self.web)
            except Exception:
                pass
            try:
                self.web.destroy()
            except Exception:
                pass
        self.bridge = None
        self.web = None
        if self.backend == "static" and self.window is not None:
            try:
                self.window.destroy()
            except Exception:
                pass
            self.window = None

    def _stop_native_gl(self) -> None:
        v_overlay = getattr(self, "_virtual_overlay", None)
        if v_overlay is not None:
            try:
                parent = v_overlay.widget.get_parent()
                if parent is not None:
                    parent.remove(v_overlay.widget)
            except Exception:
                pass
            try:
                v_overlay.destroy()
            except Exception:
                pass
            self._virtual_overlay = None
        native_gl = getattr(self, "_native_gl", None)
        if native_gl is not None:
            try:
                parent = native_gl.widget.get_parent()
                if parent is not None:
                    parent.remove(native_gl.widget)
            except Exception:
                pass
            try:
                native_gl.destroy()
            except Exception:
                pass
            self._native_gl = None
        native_hl = getattr(self, "_native_headless", None)
        if native_hl is not None:
            try:
                native_hl.destroy()
            except Exception:
                pass
            self._native_headless = None

    def _merged_native_icons_message(self) -> dict[str, Any]:
        real_vis, real_rects, real_labels = getattr(
            self, "_real_icons_raw", (False, [], [])
        )
        settings = getattr(self.host, "settings", None)
        v_icons = list(getattr(settings, "virtual_icons", []) or [])
        v_mode = str(getattr(settings, "virtual_bar_mode", "auto") or "auto")
        scale = float(getattr(self, "scale", 1.0) or 1.0)
        vis, rects, labels = merge_host_and_virtual_icons(
            real_vis, real_rects, real_labels, v_icons, scale, mode=v_mode
        )
        msg: dict[str, Any] = {"t": "icons", "vis": vis, "r": rects}
        if any(labels):
            msg["n"] = labels
        return msg

    def push_merged_native_icons(self) -> None:
        if self._closed:
            return
        native_gl = getattr(self, "_native_gl", None)
        native_hl = getattr(self, "_native_headless", None)
        if native_gl is None and native_hl is None:
            return
        msg = self._merged_native_icons_message()
        encoded = json.dumps(msg, ensure_ascii=False, separators=(",", ":"))
        if native_gl is not None:
            native_gl.post_json(encoded)
        if native_hl is not None:
            native_hl.post_json(encoded)
            if self.backend == "static":
                self._schedule_static_render(180)
        self.host.on_icons_message(msg)

    def _on_overlay_virtual_icons_changed(
        self, icons: list[dict[str, Any]], persist: bool
    ) -> None:
        self.host.on_virtual_icons_saved(icons, persist=persist, source_wallpaper=self)
        self.push_merged_native_icons()

    def _on_overlay_ui_rects(self, rects: list[tuple[int, int, int, int]]) -> None:
        self._ui_rects = list(rects)
        self._apply_input_region()

    def _sync_native_gl_state(self, theme_id: str) -> None:
        low = self._is_low_mode()
        target = self._native_gl or self._native_headless
        if target is None:
            return
        target.set_low_mode(low)
        target.post_json({"t": "lowMode", "v": low})
        target.post_json({"t": "theme", "id": theme_id})
        settings = getattr(self.host, "settings", None)
        solar_sync = bool(getattr(settings, "solar_sync", False))
        target.post_json(compute_solar_phase(dark_mode=detect_os_dark_mode(), solar_sync=solar_sync))
        target.post_json(self.host.wallpaper_settings_message())
        self.push_merged_native_icons()
        if getattr(self, "_last_windows", None) is not None:
            target.post_json(self._last_windows)
        if getattr(self, "_last_workspace", None) is not None:
            target.post_json(self._last_workspace)
        v_overlay = getattr(self, "_virtual_overlay", None)
        if v_overlay is not None and settings is not None:
            real_vis, real_rects, _ = getattr(self, "_real_icons_raw", (False, [], []))
            v_overlay.set_state(
                list(getattr(settings, "virtual_icons", []) or []),
                mode=str(getattr(settings, "virtual_bar_mode", "auto") or "auto"),
                collapsed=getattr(settings, "virtual_bar_collapsed", None),
                has_real_icons=bool(real_vis and real_rects),
            )

    def _start_native_gl(self, theme_id: str) -> bool:
        if self._closed or not NativeGlLibrary.is_available():
            return False
        if self.backend == "static":
            if self._native_headless is None:
                try:
                    self._native_headless = NativeGlHeadless(
                        self.screen_w, self.screen_h, self.scale, log=self.log
                    )
                except Exception as exc:
                    self.log.exception("Headless Rust OpenGL başlatılamadı; WebKitGTK kullanılacak", exc)
                    self._native_headless = None
                    return False
            self._release_webview()
            self._ready = True
            self._sync_native_gl_state(theme_id)
            self._sync_native_audio()
            self._last_icons = None
            self._last_pause = -1
            self._kick_polls()
            self._schedule_static_render(120)
            return True

        if self.window is None:
            return False
        if self._native_gl is None:
            try:
                self._native_gl = NativeGlArea(
                    self.screen_w,
                    self.screen_h,
                    self.scale,
                    on_message=self._on_message,
                    log=self.log,
                )
            except Exception as exc:
                self.log.exception("Gtk.GLArea Rust OpenGL başlatılamadı; WebKitGTK kullanılacak", exc)
                self._native_gl = None
                return False
            try:
                self._virtual_overlay = VirtualIconsOverlay(
                    self._native_gl.widget,
                    logical_w=self.geo.width,
                    logical_h=self.geo.height,
                    safe_insets=self._safe_insets,
                    on_icons_changed=self._on_overlay_virtual_icons_changed,
                    on_collapsed_changed=self.host.on_virtual_bar_collapsed,
                    on_pass_mouse=self._native_gl.dispatch_mouse,
                    on_ui_rects_changed=self._on_overlay_ui_rects,
                )
                self._attach_window_child(self._virtual_overlay.widget)
            except Exception as exc:
                self.log.exception("Sanal ikon katmanı başlatılamadı; doğrudan GL kullanılacak", exc)
                self._virtual_overlay = None
                self._attach_window_child(self._native_gl.widget)
        self._release_webview()
        self._ready = True
        self._sync_native_gl_state(theme_id)
        self._sync_native_audio()
        self._last_icons = None
        self._last_pause = -1
        self._kick_polls()
        return True

    # ---------------------------------------------------------------- pages
    @property
    def page(self) -> str:
        return self._page

    def url_for(self, page: str | None = None) -> str:
        page = page or self._page
        mute = (
            "&mute=1"
            if self.backend in ("plasma-plugin", "static") or not self._audio_supported
            else ""
        )
        vicons = "&vicons=0" if self.backend in ("plasma-plugin", "static") else ""
        safe = ""
        top, right, bottom, left = self._safe_insets
        if top or right or bottom or left:
            safe = f"&safeTop={top}&safeRight={right}&safeBottom={bottom}&safeLeft={left}"
        low = "&low=1" if self._is_low_mode() else ""
        return self.server.url(f"{page}?wallpaper=1{mute}{vicons}{safe}{low}&v={self.host.version}")

    def load_page(self, page: str, theme_id: str | None = None) -> None:
        self._native_audio.stop()
        self._ui_rect = None
        self._apply_input_region()
        self._page = page
        self._theme_id = str(
            theme_id
            or getattr(getattr(self.host, "settings", None), "theme", "")
            or getattr(self, "_theme_id", "")
        )
        self._ready = False
        self._last_pause = -1

        if self._should_use_native_gl(page, self._theme_id) and self._start_native_gl(self._theme_id):
            self.log.write(f"Yerel Rust OpenGL teması yüklendi: {self._theme_id}")
            return

        self._stop_native_gl()
        if self.backend == "plasma-plugin":
            return
        if self.bridge is None:
            self._init_bridge()
            if self.backend == "static":
                if self.window is None:
                    self._create_hidden_render_surface(
                        "Statik fallback: GNOME/KWin Wayland veya --static"
                    )
            elif self.window is not None and self.web is not None:
                self._attach_window_child(self.web)
        if self.bridge is None:
            return
        self._last_icons = None
        self.bridge.load_uri(self.url_for(page))
        self.log.write(f"Tema sayfası yüklendi: {page}")

    def reload(self) -> None:
        if self.backend == "plasma-plugin":
            # Re-trigger settings/pause check inside plasmashell; lazy share
            # renderer reloads only if currently active.
            self._write_plasma_pause(self._manual_pause)
            if self.bridge is None:
                return
        if getattr(self, "_native_gl", None) is not None or getattr(self, "_native_headless", None) is not None:
            self._sync_native_gl_state(getattr(self, "_theme_id", ""))
            if self.backend == "static":
                self._schedule_static_render(120)
            return
        self._ready = False
        self._last_icons = None
        self._last_pause = -1
        if self.bridge is not None:
            self.bridge.reload()

    # ---------------------------------------------------------------- messages
    def _on_message(self, text: str) -> None:
        if text == "ready":
            self._on_ready()
            return
        if text.startswith("snapshot|"):
            self._on_snapshot(text)
            return
        if text.startswith("recprogress|"):
            try:
                self.host.on_record_progress(float(text.split("|", 1)[1]))
            except ValueError:
                pass
            return
        if text.startswith("record|"):
            self._on_record(text)
            return
        try:
            message = json.loads(text)
        except ValueError:
            return
        if not isinstance(message, dict):
            return
        kind = message.get("t")
        if kind == "ready":
            self._on_ready()
        elif kind == "uiRect":
            self._on_ui_rect(message.get("r"))
        elif kind == "setAudioPlay":
            self.host.on_wallpaper_audio_play(bool(message.get("v")))
        elif kind == "saveVirtualIcons":
            self.host.on_virtual_icons_saved(message.get("v"))
        elif kind == "saveVirtualBarCollapsed":
            self.host.on_virtual_bar_collapsed(message.get("v"))

    def send_virtual_state(self) -> None:
        if self._closed:
            return
        settings = getattr(self.host, "settings", None)
        if settings is None:
            return
        v_overlay = getattr(self, "_virtual_overlay", None)
        if v_overlay is not None:
            real_vis, real_rects, _ = getattr(self, "_real_icons_raw", (False, [], []))
            v_overlay.set_state(
                list(settings.virtual_icons),
                mode=settings.virtual_bar_mode,
                collapsed=settings.virtual_bar_collapsed,
                has_real_icons=bool(real_vis and real_rects),
            )
        if getattr(self, "_native_gl", None) is not None or getattr(self, "_native_headless", None) is not None:
            self.push_merged_native_icons()
        if not self._ready or self.bridge is None:
            return
        self.bridge.post(
            {
                "t": "virtualState",
                "icons": settings.virtual_icons,
                "collapsed": settings.virtual_bar_collapsed,
                "mode": settings.virtual_bar_mode,
            }
        )

    def _on_ready(self) -> None:
        self._ready = True
        self.send_virtual_state()
        if self.bridge is not None:
            self.bridge.post({"t": "lowMode", "v": self._is_low_mode()})
            pm = getattr(self, "power_monitor", None)
            if pm is not None:
                st = pm.state
                self.bridge.post(
                    {
                        "t": "perf",
                        "adaptiveFps": True,
                        "onBattery": st.on_battery,
                        "reducedMotion": st.reduced_motion,
                    }
                )
            self.bridge.post(self.host.wallpaper_settings_message())
            if self._last_icons is not None:
                self.bridge.post(self._last_icons)
        self._sync_native_audio()
        self.sync_mpv_video()
        if self.backend == "plasma-plugin":
            if self._pending_snapshot and self.bridge is not None:
                self._pending_snapshot = False
                self.bridge.post({"t": "pause", "v": False})
                self.bridge.post({"t": "snapshot"})
            if self._pending_record and self.bridge is not None:
                self._pending_record = False
                self.bridge.post({"t": "pause", "v": False})
                self.bridge.post({"t": "record"})
            return
        self._last_icons = None
        self._last_pause = -1
        self._kick_polls()

    def _kick_polls(self) -> None:
        GLib.idle_add(self._poll_icons_once)
        GLib.idle_add(self._poll_pause_once)

    def _sync_native_audio(self) -> None:
        if self._closed or self._audio_supported or self.backend not in (
            "layer-shell",
            "gnome-shell",
            "x11",
        ):
            self._native_audio.stop()
            return
        theme_id = self.host.settings.theme
        theme_meta = self.host.themes.find(theme_id)
        params = self.host.settings.for_theme(theme_id)
        play = bool(params.get("audioPlay"))
        paused = self._last_pause != 0 or self._manual_pause
        path = resolve_theme_audio_path(
            theme_meta,
            params,
            getattr(self.host, "audio_dir", None),
            self.server.prototype,
        )
        self._native_audio.sync(play=play, paused=paused, path=path)

    def set_settings(self, values: dict[str, Any]) -> None:
        if values:
            self._post_active({"t": "settings", "v": values})
            if "audioPlay" in values or "audioFile" in values:
                self._sync_native_audio()
            if self.backend == "static":
                self._schedule_static_render(160 if getattr(self, "_native_headless", None) is not None else 400)

    def reset(self) -> None:
        self._post_active({"t": "reset"})
        self._sync_native_audio()
        if self.backend == "static":
            self._schedule_static_render(160 if getattr(self, "_native_headless", None) is not None else 400)

    def set_manual_pause(self, value: bool) -> None:
        self._manual_pause = bool(value)
        if self.backend == "plasma-plugin":
            self._write_plasma_pause(self._manual_pause)
        self._last_pause = -1
        self._poll_pause_once()

    def set_low_mode(self, enabled: bool) -> None:
        self._low_mode = bool(enabled)
        self.pause_monitor.low_mode = self._low_mode
        self._reschedule_poll_timers()
        native_gl = getattr(self, "_native_gl", None)
        if native_gl is not None:
            native_gl.set_low_mode(self._low_mode)
        native_hl = getattr(self, "_native_headless", None)
        if native_hl is not None:
            native_hl.set_low_mode(self._low_mode)
        self._post_active({"t": "lowMode", "v": self._low_mode})
        self._last_pause = -1
        self._poll_pause_once()
        if self.backend == "static":
            self._schedule_static_render(1600 if self._low_mode else 400)

    def set_disable_auto_pause(self, enabled: bool) -> None:
        self._disable_auto_pause = bool(enabled)
        self._last_pause = -1
        self._poll_pause_once()

    # ---------------------------------------------------------------- icons
    def _poll_icons(self) -> bool:
        if self._closed:
            return False
        if self._icon_busy:
            return True
        self._icon_busy = True
        threading.Thread(target=self._read_icons, daemon=True).start()
        return True

    def _read_icons(self) -> None:
        win_message: dict[str, Any] | None = None
        ws_message: dict[str, Any] | None = None
        try:
            visible, rects = self.icon_adapter.read()
            raw_rects = [rect.as_array() for rect in rects]
            settings = getattr(getattr(self, "host", None), "settings", None)
            window_aware = bool(getattr(settings, "window_aware", False))
            if window_aware:
                wins = detect_active_windows()
                ws_id, panels = detect_workspace_state()
                ox = getattr(getattr(self, "geo", None), "x", 0)
                oy = getattr(getattr(self, "geo", None), "y", 0)
                sc = float(getattr(self, "scale", 1.0) or 1.0)
                win_message = {
                    "t": "windows",
                    "r": [w.as_wire_list(scale=sc, offset_x=ox, offset_y=oy) for w in wins],
                }
                ws_message = {
                    "t": "workspace",
                    "id": ws_id,
                    "panels": [p.as_rect(scale=sc, offset_x=ox, offset_y=oy) for p in panels],
                }
                if not raw_rects and wins:
                    # On iconless tiling Wayland desktops (Hyprland/Sway/niri), let themes
                    # react seamlessly to open windows when window-aware scenery is enabled.
                    raw_rects = [w.as_icon_rect(scale=sc, offset_x=ox, offset_y=oy) for w in wins]
                    visible = True
            message: dict[str, Any] = {
                "t": "icons",
                "vis": bool(visible),
                "r": raw_rects,
            }
            if any(rect.label for rect in rects):
                message["n"] = [rect.label for rect in rects]
        except Exception as exc:  # adapter must never kill the timer
            self.log.exception("Simge okuma hatası", exc)
            message = None
        GLib.idle_add(self._apply_icons, message, win_message, ws_message)

    def _poll_icons_once(self) -> bool:
        if not self._icon_busy:
            self._poll_icons()
        return False

    def _apply_icons(
        self,
        message: dict[str, Any] | None,
        win_message: dict[str, Any] | None = None,
        ws_message: dict[str, Any] | None = None,
    ) -> bool:
        self._icon_busy = False
        if self._closed:
            return False
        if win_message is not None:
            w_enc = json.dumps(win_message, ensure_ascii=False, separators=(",", ":"))
            if w_enc != getattr(self, "_last_windows", None):
                self._last_windows = w_enc
                if self._ready:
                    self._post_active(w_enc)
        if ws_message is not None:
            ws_enc = json.dumps(ws_message, ensure_ascii=False, separators=(",", ":"))
            if ws_enc != getattr(self, "_last_workspace", None):
                self._last_workspace = ws_enc
                if self._ready:
                    self._post_active(ws_enc)
        if message is None:
            return False
        self._real_icons_raw = (
            bool(message.get("vis")),
            list(message.get("r") or []),
            list(message.get("n") or []),
        )
        v_overlay = getattr(self, "_virtual_overlay", None)
        if v_overlay is not None:
            v_overlay.set_has_real_icons(bool(self._real_icons_raw[0] and self._real_icons_raw[1]))
        encoded = json.dumps(message, ensure_ascii=False, separators=(",", ":"))
        if encoded == self._last_icons:
            return False
        self._last_icons = encoded
        if self._ready:
            if getattr(self, "_native_gl", None) is not None or getattr(self, "_native_headless", None) is not None:
                self.push_merged_native_icons()
            elif self.bridge is not None:
                self.bridge.post(encoded)
                self.host.on_icons_message(message)
        else:
            self.host.on_icons_message(message)
        if self.backend == "static":
            # Static themes snap morph transitions immediately while paused, so a
            # short debounce avoids extra redraw bursts without keeping WebGL active.
            delay = (
                220
                if getattr(self, "_native_headless", None) is not None
                else (900 if self._static_file is None else 500)
            )
            self._schedule_static_render(delay)
        self.log.write(f"Simge değişimi: {len(message['r'])} adet")
        return False

    def _on_web_process_terminated(self, reason: str) -> None:
        if self._closed:
            return
        now = time.monotonic()
        recent = [ts for ts in getattr(self, "_crash_times", []) if now - ts < 60.0]
        recent.append(now)
        self._crash_times = recent
        self.log.write(f"WebKitWebProcess sonlandı (neden={reason}, son 60 sn={len(recent)})")
        if len(recent) > 3:
            self.log.write("WebKitWebProcess art arda çöktü; sonsuz yeniden yükleme durduruldu")
            return
        delay_ms = 1000 * (2 ** (len(recent) - 1))
        if getattr(self, "_watchdog_timeout", 0):
            GLib.source_remove(self._watchdog_timeout)
        self._watchdog_timeout = GLib.timeout_add(delay_ms, self._watchdog_reload)

    def _watchdog_reload(self) -> bool:
        self._watchdog_timeout = 0
        if self._closed or self.bridge is None:
            return False
        self.log.write("Watchdog: duvar kağıdı WebView yeniden yükleniyor")
        self.load_page(self._page)
        return False

    # ---------------------------------------------------------------- pause
    def _poll_pause(self) -> bool:
        if self._closed:
            return False
        if self.backend in ("plasma-plugin", "static"):
            # Static and Plasma render surfaces stay paused at rest so XFCE/X11
            # and Plasma hosts keep near-zero idle GPU/CPU usage.
            GLib.idle_add(self._apply_pause, True)
            return False
        if self._pause_busy:
            return True
        self._pause_busy = True
        threading.Thread(target=self._read_pause, daemon=True).start()
        return True

    def _read_pause(self) -> None:
        power_state = PowerState()
        pm = getattr(self, "power_monitor", None)
        if pm is not None:
            try:
                power_state = pm.poll()
            except Exception as exc:
                self.log.exception("Güç durumu okuma hatası", exc)
        try:
            disable_auto = bool(getattr(self, "_disable_auto_pause", False))
            if power_state.locked:
                auto = True
            elif disable_auto:
                auto = False
            elif power_state.power_saver:
                auto = True
            else:
                auto = self.pause_monitor.should_pause()
        except Exception as exc:
            self.log.exception("Duraklatma algılama hatası", exc)
            auto = False
        GLib.idle_add(self._apply_pause, auto, power_state)

    def _poll_pause_once(self) -> bool:
        if not self._pause_busy:
            self._poll_pause()
        return False

    def push_environment(self, force: bool = False) -> None:
        host = getattr(self, "host", None)
        if host is None or self._closed or not self._ready:
            return
        if (
            self.bridge is None
            and getattr(self, "_native_gl", None) is None
            and getattr(self, "_native_headless", None) is None
        ):
            return
        settings = getattr(host, "settings", None)
        solar_sync = bool(getattr(settings, "solar_sync", False))
        now = time.monotonic()
        last_ts = float(getattr(self, "_last_env_ts", 0.0))
        if not force and (now - last_ts) < 60.0:
            return
        self._last_env_ts = now
        payload = compute_solar_phase(dark_mode=detect_os_dark_mode(), solar_sync=solar_sync)
        encoded = json.dumps(payload, ensure_ascii=False, separators=(",", ":"))
        if force or encoded != getattr(self, "_last_env", None):
            self._last_env = encoded
            self._post_active(encoded)

    def sync_mpv_video(self) -> None:
        mpv = getattr(self, "mpv_backend", None)
        host = getattr(self, "host", None)
        if mpv is None or host is None:
            return
        settings = getattr(host, "settings", None)
        prefer_mpv = bool(getattr(settings, "prefer_mpv", False))
        custom_dir = getattr(host, "custom_themes_dir", None)
        page = str(getattr(self, "_page", "") or "")
        if prefer_mpv and self.backend == "x11" and page.startswith("__custom/") and custom_dir is not None:
            tid = page.split("/", 2)[1]
            vpath = find_theme_video_file(Path(custom_dir) / tid)
            if vpath is not None and self.window is not None:
                gdk_win = self.window.get_window()
                wid = gdk_win.get_xid() if gdk_win is not None and hasattr(gdk_win, "get_xid") else None
                params = settings.for_theme(tid) if settings is not None else {}
                if mpv.start(vpath, wid=wid, speed=float(params.get("speed", 1.0)), brightness=float(params.get("brightness", 1.0))):
                    return
        mpv.stop()

    def _tick_system_bridge_and_watchdog(self) -> None:
        host = getattr(self, "host", None)
        if self._closed or host is None:
            return
        now = time.monotonic()
        # 1. Runaway script CPU/RAM watchdog every 10s
        rw = getattr(self, "resource_watchdog", None)
        if rw is not None and (now - float(getattr(self, "_last_watchdog_check", 0.0))) >= 10.0:
            self._last_watchdog_check = now
            pids = find_webkit_child_pids()
            if pids:
                reason = rw.evaluate(sample_processes(pids), now)
                if reason is not None:
                    self.log.write(f"Kaçak betik sınır aşımı algılandı ({reason}); duvar kağıdı sıfırlanıyor")
                    rw.reset()
                    self._on_web_process_terminated(reason)
        # 2. Opt-in local System Stats & Audio Reactive bridge
        settings = getattr(host, "settings", None)
        bridge_on = bool(getattr(settings, "system_bridge", False))
        am = getattr(self, "audio_meter", None)
        if not bridge_on:
            if am is not None:
                am.stop()
            return
        if self._last_pause == 1 or not self._ready or self.bridge is None:
            return
        if am is not None:
            am.start()
            self.bridge.post(am.latest_payload())
        sampler = getattr(self, "stats_sampler", None)
        if sampler is not None and (now - float(getattr(self, "_last_stats_ts", 0.0))) >= 2.0:
            self._last_stats_ts = now
            self.bridge.post(sampler.sample())

    def _apply_pause(self, auto: bool, power_state: PowerState | None = None) -> bool:
        self._pause_busy = False
        if self._closed:
            return False
        self.push_environment(force=False)
        self._tick_system_bridge_and_watchdog()
        if power_state is not None and self._ready and self.bridge is not None:
            perf_sig = (power_state.on_battery, power_state.reduced_motion)
            if perf_sig != getattr(self, "_last_power_perf", None):
                self._last_power_perf = perf_sig
                self.bridge.post(
                    {
                        "t": "perf",
                        "adaptiveFps": True,
                        "onBattery": power_state.on_battery,
                        "reducedMotion": power_state.reduced_motion,
                    }
                )
        locked = bool(power_state and power_state.locked)
        effective_auto = True if locked else (False if getattr(self, "_disable_auto_pause", False) else bool(auto))
        pause = (
            True
            if self.backend in ("plasma-plugin", "static")
            else bool(effective_auto or self._manual_pause)
        )
        value = 1 if pause else 0
        if value == self._last_pause:
            return False
        self._last_pause = value
        self._sync_native_audio()
        native_gl = getattr(self, "_native_gl", None)
        native_hl = getattr(self, "_native_headless", None)
        if pause and getattr(self, "_pointer_active", False):
            self._pointer_active = False
            self._last_pointer = None
            if native_gl is not None:
                native_gl.dispatch_mouse(3, -1.0, -1.0)
            elif self._ready and self.bridge is not None:
                self.bridge.post({"t": "pointer", "kind": "leave"})
        if native_gl is not None:
            native_gl.set_paused(pause)
        if native_hl is not None:
            native_hl.set_paused(pause)
        if self._ready and self.bridge is not None:
            self.bridge.post({"t": "pause", "v": pause})
        self.log.write("Duraklatıldı" if pause else "Devam ediyor")
        return False

    # ---------------------------------------------------------------- X11 hover
    @staticmethod
    def _is_x11_point_over_desktop(screen: Any, rx: int, ry: int) -> bool:
        """Return True when (rx, ry) is not occluded by a visible non-desktop X11 window."""
        if screen is None or not hasattr(screen, "get_window_stack"):
            return True
        try:
            stack = screen.get_window_stack() or []
        except Exception:
            return True
        desktop_hint = Gdk.WindowTypeHint.DESKTOP
        dock_hint = Gdk.WindowTypeHint.DOCK
        hidden_mask = Gdk.WindowState.ICONIFIED | Gdk.WindowState.WITHDRAWN
        for win in stack:
            if win is None:
                continue
            try:
                if hasattr(win, "is_visible") and not win.is_visible():
                    continue
                if win.get_state() & hidden_mask:
                    continue
                if win.get_type_hint() in (desktop_hint, dock_hint):
                    continue
                wx, wy, ww, wh = win.get_geometry()
                if ww > 0 and wh > 0 and wx <= rx < wx + ww and wy <= ry < wy + wh:
                    return False
            except Exception:
                continue
        return True

    def _poll_x11_pointer(self) -> bool:
        if self._closed or self.backend != "x11":
            self._pointer_timeout = 0
            return False
        if not self._ready or self._last_pause != 0:
            return True
        native_gl = getattr(self, "_native_gl", None)
        if self.bridge is None and native_gl is None:
            return True
        try:
            display = Gdk.Display.get_default()
            seat = display.get_default_seat() if display is not None else None
            pointer = seat.get_pointer() if seat is not None else None
            if pointer is None:
                return True
            screen, rx, ry = pointer.get_position()
            inside = (
                self.geo.x <= rx < self.geo.x + self.geo.width
                and self.geo.y <= ry < self.geo.y + self.geo.height
            )
            over_desktop = inside and self._is_x11_point_over_desktop(screen, rx, ry)
            if over_desktop:
                lx = int(rx - self.geo.x)
                ly = int(ry - self.geo.y)
                last = self._last_pointer
                if not self._pointer_active or last is None or abs(lx - last[0]) + abs(ly - last[1]) >= 2:
                    self._pointer_active = True
                    self._last_pointer = (lx, ly)
                    if native_gl is not None:
                        native_gl.dispatch_mouse(0, float(lx), float(ly))
                    elif self.bridge is not None:
                        self.bridge.post({"t": "pointer", "kind": "hover", "x": lx, "y": ly})
            elif self._pointer_active:
                self._pointer_active = False
                self._last_pointer = None
                if native_gl is not None:
                    native_gl.dispatch_mouse(3, -1.0, -1.0)
                elif self.bridge is not None:
                    self.bridge.post({"t": "pointer", "kind": "leave"})
        except Exception:
            pass
        return True

    # ---------------------------------------------------------------- share
    def request_snapshot(
        self, done: Callable[[str | None, str | None, str | None], None]
    ) -> None:
        if self._snapshot_done is not None:
            done(None, None, self.host.i18n.t("errBusyImage"))
            return
        native_gl = getattr(self, "_native_gl", None)
        if native_gl is not None:
            native_gl.request_snapshot(done)
            return
        native_hl = getattr(self, "_native_headless", None)
        if native_hl is not None:
            thumb, full, err = native_hl.render_snapshot_data_urls(dt=0.033)
            done(thumb, full, err)
            return
        if self.backend == "plasma-plugin":
            settings = getattr(self.host, "settings", None)
            theme_id = str(getattr(settings, "theme", "") or getattr(self, "_theme_id", ""))
            if should_use_native_gl(
                self._page,
                theme_id,
                rust_opengl=bool(getattr(settings, "rust_opengl", False)),
                available=NativeGlLibrary.is_available(),
            ):
                try:
                    headless = NativeGlHeadless(self.screen_w, self.screen_h, self.scale, log=self.log)
                    headless.post_json({"t": "theme", "id": theme_id})
                    headless.post_json(self.host.wallpaper_settings_message())
                    if self._last_icons is not None:
                        headless.post_json(self._last_icons)
                    thumb, full, err = headless.render_snapshot_data_urls(dt=0.033)
                    headless.destroy()
                    if not err and full:
                        done(thumb, full, None)
                        return
                except Exception as exc:
                    self.log.exception("Plasma headless OpenGL snapshot hatası; WebKit fallback", exc)
            self._snapshot_done = done
            self._snapshot_timeout = GLib.timeout_add_seconds(
                20, self._snapshot_timed_out
            )
            if self.bridge is None:
                self._pending_snapshot = True
                self._ensure_share_renderer()
            elif not self._ready:
                self._pending_snapshot = True
            else:
                self.bridge.post({"t": "pause", "v": False})
                self.bridge.post({"t": "snapshot"})
            return
        if not self._ready or self.bridge is None:
            done(None, None, self.host.i18n.t("errNotReady"))
            return
        self._snapshot_done = done
        self._snapshot_timeout = GLib.timeout_add_seconds(
            20, self._snapshot_timed_out
        )
        self.bridge.post({"t": "snapshot"})

    def _snapshot_timed_out(self) -> bool:
        self._finish_snapshot(None, None, self.host.i18n.t("errImageTimeout"))
        self._snapshot_timeout = 0
        return False

    def _on_snapshot(self, text: str) -> None:
        parts = text.split("|", 2)
        if len(parts) < 3:
            self._finish_snapshot(None, None, self.host.i18n.t("errBadData"))
        elif parts[1] == "error":
            self._finish_snapshot(None, None, parts[2])
        else:
            self._finish_snapshot(parts[1], parts[2], None)

    def _finish_snapshot(
        self, thumb: str | None, png: str | None, error: str | None
    ) -> None:
        if self._snapshot_timeout:
            GLib.source_remove(self._snapshot_timeout)
            self._snapshot_timeout = 0
        done = self._snapshot_done
        self._snapshot_done = None
        if self.backend == "plasma-plugin":
            GLib.idle_add(self._release_share_renderer)
        if done is not None:
            done(thumb, png, error)

    def request_record(
        self,
        done: Callable[[str | None, str | None, str | None, str | None], None],
    ) -> None:
        if self._record_done is not None:
            done(None, None, None, self.host.i18n.t("errBusyVideo"))
            return
        if self.backend == "plasma-plugin":
            self._record_done = done
            self._record_timeout = GLib.timeout_add_seconds(45, self._record_timed_out)
            if self.bridge is None:
                self._pending_record = True
                self._ensure_share_renderer()
            elif not self._ready:
                self._pending_record = True
            else:
                self.bridge.post({"t": "pause", "v": False})
                self.bridge.post({"t": "record"})
            return
        if not self._ready or self.bridge is None:
            done(None, None, None, self.host.i18n.t("errNotReady"))
            return
        self._record_done = done
        self._record_timeout = GLib.timeout_add_seconds(45, self._record_timed_out)
        self.bridge.post({"t": "record"})

    def _record_timed_out(self) -> bool:
        self._finish_record(None, None, None, self.host.i18n.t("errVideoTimeout"))
        self._record_timeout = 0
        return False

    def _on_record(self, text: str) -> None:
        parts = text.split("|", 3)
        if len(parts) >= 3 and parts[1] == "error":
            self._finish_record(None, None, None, parts[2])
        elif len(parts) < 4:
            self._finish_record(None, None, None, self.host.i18n.t("errBadData"))
        else:
            self._finish_record(parts[1], parts[2], parts[3], None)

    def _finish_record(
        self,
        mime: str | None,
        poster: str | None,
        video: str | None,
        error: str | None,
    ) -> None:
        if self._record_timeout:
            GLib.source_remove(self._record_timeout)
            self._record_timeout = 0
        done = self._record_done
        self._record_done = None
        if self.backend == "plasma-plugin":
            GLib.idle_add(self._release_share_renderer)
        if done is not None:
            done(mime, poster, video, error)

    # ---------------------------------------------------------------- static
    def _schedule_static_render(self, delay_ms: int = 2000) -> None:
        if self.backend != "static" or self._closed:
            return
        if self._static_timeout:
            GLib.source_remove(self._static_timeout)
        self._static_timeout = GLib.timeout_add(delay_ms, self._static_render)

    def _static_render(self) -> bool:
        self._static_timeout = 0
        if self._closed:
            return False
        native_hl = getattr(self, "_native_headless", None)
        if native_hl is not None:
            target = config_dir() / f"render-{int(time.time() * 1000)}.png"
            try:
                was_paused = self._last_pause == 1
                native_hl.set_paused(False)
                ok = native_hl.render_to_png(target, dt=0.033)
                if was_paused:
                    native_hl.set_paused(True)
                if ok and self.static_wallpaper.apply(target):
                    previous = self._static_file
                    self._static_file = target
                    if previous is not None and previous != target:
                        try:
                            previous.unlink()
                        except OSError:
                            pass
            except Exception as exc:
                self.log.exception("Headless Rust OpenGL statik kare oluşturulamadı", exc)
            return False
        if self.web is None or self.bridge is None:
            return False
        if self._last_pause == 1 and self._ready and not self._static_snapshot_pending:
            # WebKitGTK only composites WebGL canvas updates inside a
            # requestAnimationFrame tick; briefly unpause so one frame paints.
            self._static_snapshot_pending = True
            self.bridge.post({"t": "pause", "v": False})
            self._static_timeout = GLib.timeout_add(160, self._static_render)
            return False
        self._static_snapshot_pending = False
        try:
            self.web.get_snapshot(
                self.web.get_snapshot_region() if hasattr(self.web, "get_snapshot_region") else 0,
                0,
                None,
                self._snapshot_finished,
                None,
            )
        except (TypeError, AttributeError):
            # Older/newer Web signatures vary; retry with explicit enums.
            try:
                from gi.repository import WebKit2

                self.web.get_snapshot(
                    WebKit2.SnapshotRegion.VISIBLE,
                    WebKit2.SnapshotOptions.NONE,
                    None,
                    self._snapshot_finished,
                    None,
                )
            except Exception as exc:
                self.log.exception("Statik kare alınamadı", exc)
        return False

    def _snapshot_finished(self, webview, result, _user_data) -> None:
        if self._last_pause == 1 and self._ready and not self._closed:
            self.bridge.post({"t": "pause", "v": True})
        try:
            surface = webview.get_snapshot_finish(result)
        except Exception as exc:
            self.log.exception("Snapshot tamamlanamadı", exc)
            return
        if surface is None:
            return
        # A new filename each time: Plasma (and some other desktops) cache the
        # wallpaper texture by URI, so rewriting render.png would keep showing
        # the previous frame after a theme or icon change.
        target = config_dir() / f"render-{int(time.time() * 1000)}.png"
        try:
            surface.write_to_png(str(target))
        except Exception as exc:
            self.log.exception("render.png yazılamadı", exc)
            return
        if self.static_wallpaper.apply(target):
            previous = self._static_file
            self._static_file = target
            if previous is not None and previous != target:
                try:
                    previous.unlink()
                except OSError:
                    pass

    # ---------------------------------------------------------------- lifecycle
    def shutdown(self) -> None:
        if self._closed:
            return
        self._closed = True
        self._native_audio.stop()
        mpv = getattr(self, "mpv_backend", None)
        if mpv is not None:
            mpv.stop()
        am = getattr(self, "audio_meter", None)
        if am is not None:
            am.stop()
        if self.backend == "plasma-plugin":
            self._write_plasma_pause(False)
        if self._snapshot_timeout:
            GLib.source_remove(self._snapshot_timeout)
        if self._record_timeout:
            GLib.source_remove(self._record_timeout)
        if self._static_timeout:
            GLib.source_remove(self._static_timeout)
        if getattr(self, "_icon_timeout", 0):
            GLib.source_remove(self._icon_timeout)
            self._icon_timeout = 0
        if getattr(self, "_pause_timeout", 0):
            GLib.source_remove(self._pause_timeout)
            self._pause_timeout = 0
        if getattr(self, "_pointer_timeout", 0):
            GLib.source_remove(self._pointer_timeout)
            self._pointer_timeout = 0
        if getattr(self, "_watchdog_timeout", 0):
            GLib.source_remove(self._watchdog_timeout)
            self._watchdog_timeout = 0
        self.static_wallpaper.restore()
        self._stop_native_gl()
        if self._static_file is not None:
            try:
                self._static_file.unlink()
            except OSError:
                pass
            self._static_file = None
        if self.window is not None:
            try:
                self.window.destroy()
            except Exception:
                pass
            self.window = None
