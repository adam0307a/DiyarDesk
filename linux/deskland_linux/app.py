"""Gtk.Application orchestration for Deskland Preview on Linux."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import Any
from urllib.parse import quote

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gio", "2.0")

from gi.repository import Gio, GLib, Gtk  # noqa: E402

from . import autostart
from .config import (
    APP_ID,
    APP_NAME,
    DISPLAY_MODES,
    SUPPORTED_LANGS,
    VERSION,
    I18n,
    Log,
    SettingsStore,
    ThemeRegistry,
    config_dir,
    desktop_dir,
    pictures_dir,
    prototype_dir,
    repo_root,
)
from .crash_sentinel import DEFAULT_STABLE_SECONDS, SAFE_FALLBACK_ORDER, StartupCrashSentinel
from .diagnostics import export_diagnostics
from .displays import MonitorInfo, compute_span_rect, detect_compositor_monitors
from .importer import SUPPORTED_EXTS, custom_themes_dir, import_theme_file, remove_custom_theme
from .ipc import IpcServer
from .lan_sync import LanPairingServer
from .lockscreen import apply_linux_lockscreen, make_blurred_lockscreen
from .scheduler import PlaylistController
from .server import LocalServer
from .settings_window import SettingsWindow
from .share import copy_files, copy_image, open_path, reveal_path, save_share
from .tray import Tray
from .wallpaper import Wallpaper


def _icon_path() -> Path | None:
    candidates = (
        Path(__file__).resolve().parents[1] / "packaging" / "deskland.svg",
        repo_root() / "linux" / "packaging" / "deskland.svg",
        repo_root() / "packaging" / "deskland.svg",
        Path("/usr/share/icons/hicolor/scalable/apps/deskland.svg"),
    )
    for path in candidates:
        if path.exists():
            return path
    return None


class DesklandHost:
    def __init__(self, application: Gtk.Application) -> None:
        self.app = application
        self.version = VERSION
        self.log = Log()
        self.settings = SettingsStore()
        self.settings.load()
        self.i18n = I18n(prototype_dir(), self.settings.lang, self.log)
        self.custom_themes_dir = custom_themes_dir()
        self.themes = ThemeRegistry(prototype_dir(), self.log, custom_dir=self.custom_themes_dir)
        self.crash_sentinel = StartupCrashSentinel()
        self._stable_timeout = 0
        self.crash_sentinel.apply_to_settings(
            self.settings,
            safe_theme_resolver=self._resolve_safe_theme,
            log=self.log,
        )
        self.audio_dir = config_dir() / "audio"
        self.server = LocalServer(
            prototype_dir(),
            pictures_dir() / "DiyarDesk",
            audio_dir=self.audio_dir,
            custom_themes_dir=self.custom_themes_dir,
            themes_provider=self.themes.merged_json_bytes,
        )
        self.server.start()
        self.wallpaper: Wallpaper | None = None
        self.extra_wallpapers: list[Wallpaper] = []
        self._requested_backend = "auto"
        self._requested_adapter: str | None = None
        self.lan_pairing = LanPairingServer(
            custom_dir=self.custom_themes_dir,
            state_provider=lambda: {
                "theme": self.settings.theme,
                "params": dict(self.settings.for_theme(self.settings.theme)),
            },
        )
        self.playlist = PlaylistController(
            interval_minutes=self.settings.playlist_interval_min,
            themes=self.settings.playlist_themes,
        )
        self._playlist_timeout = 0
        self.ipc_server = IpcServer(self._dispatch_ipc)
        self.settings_window: SettingsWindow | None = None
        self.tray: Tray | None = None
        self.last_icons: dict[str, Any] | None = None
        self.last_share: Path | None = None
        self.manual_pause = False
        self.started = False
        self._save_timeout = 0
        self._quitting = False
        self._profile_isolated = bool(os.environ.get("DESKLAND_PROFILE_DIR"))
        self.log.write(f"Linux host başladı; veri={config_dir()}")

    # ---------------------------------------------------------------- helpers
    def desktop_dir(self) -> Path:
        return desktop_dir()

    @property
    def current_page(self) -> str:
        return self.themes.page_for(self.settings.theme) or "flat.html"

    def wallpaper_settings_message(self) -> dict[str, Any]:
        return {
            "t": "settings",
            "v": dict(self.settings.for_theme(self.settings.theme)),
        }

    def _resolve_safe_theme(self) -> str:
        for theme_id in SAFE_FALLBACK_ORDER:
            if self.themes.allowed(theme_id):
                return theme_id
        return "meadow"

    def choose_theme(self) -> None:
        if self.themes.allowed(self.settings.theme):
            return
        for theme_id in ("papercut", "software-world", "beach", "meadow"):
            if self.themes.allowed(theme_id):
                self.settings.theme = theme_id
                self.log.write(f"Geçersiz tema; {theme_id} seçildi")
                return
        self.log.write("Yüklenecek ücretsiz tema bulunamadı")
        self.settings.theme = "papercut"

    def _all_wallpapers(self) -> list[Wallpaper]:
        out: list[Wallpaper] = []
        if self.wallpaper is not None:
            out.append(self.wallpaper)
        out.extend(self.extra_wallpapers)
        return out

    def _shutdown_wallpapers(self) -> None:
        for wp in self._all_wallpapers():
            try:
                wp.shutdown()
            except Exception:
                pass
        self.wallpaper = None
        self.extra_wallpapers = []

    def _spawn_wallpapers(self, backend: str, icon_adapter: str | None) -> None:
        from gi.repository import Gdk

        self._shutdown_wallpapers()
        self._requested_backend = backend
        self._requested_adapter = icon_adapter
        mode = self.settings.display_mode
        display = Gdk.Display.get_default()
        gdk_monitors = (
            [display.get_monitor(i) for i in range(display.get_n_monitors())]
            if display is not None
            else []
        )
        comp_monitors = detect_compositor_monitors()

        if mode == "span" and len(comp_monitors) > 1:
            span = compute_span_rect(comp_monitors)
            self.wallpaper = Wallpaper(
                host=self,
                server=self.server,
                log=self.log,
                backend=backend,
                page=self.current_page,
                adapter=icon_adapter,
                monitor_info=span,
                is_primary=True,
            )
            self.wallpaper.load_page(self.current_page)
            return

        primary_mon = comp_monitors[0] if comp_monitors else None
        for cm in comp_monitors:
            if cm.primary:
                primary_mon = cm
                break
        primary_gdk = (
            display.get_primary_monitor() or (gdk_monitors[0] if gdk_monitors else None)
            if display is not None
            else None
        )
        self.wallpaper = Wallpaper(
            host=self,
            server=self.server,
            log=self.log,
            backend=backend,
            page=self.current_page,
            adapter=icon_adapter,
            monitor_info=primary_mon,
            gdk_monitor=primary_gdk,
            is_primary=True,
        )
        self.wallpaper.load_page(self.current_page)

        if mode in ("independent", "duplicate") and self.wallpaper.backend in ("layer-shell", "x11"):
            extras: list[tuple[MonitorInfo | None, Any]] = []
            if len(comp_monitors) > 1:
                for idx, cm in enumerate(comp_monitors):
                    if cm is primary_mon:
                        continue
                    gmon = gdk_monitors[idx] if idx < len(gdk_monitors) else None
                    extras.append((cm, gmon))
            elif len(gdk_monitors) > 1:
                for gmon in gdk_monitors:
                    if gmon == primary_gdk:
                        continue
                    g = gmon.get_geometry()
                    sc = float(gmon.get_scale_factor() or 1)
                    extras.append((MonitorInfo("secondary", g.x, g.y, g.width, g.height, sc, False), gmon))
            for minfo, gmon in extras:
                try:
                    ewp = Wallpaper(
                        host=self,
                        server=self.server,
                        log=self.log,
                        backend=self.wallpaper.backend,
                        page=self.current_page,
                        adapter=icon_adapter,
                        monitor_info=minfo,
                        gdk_monitor=gmon,
                        is_primary=False,
                    )
                    ewp.load_page(self.current_page)
                    self.extra_wallpapers.append(ewp)
                except Exception as exc:
                    self.log.exception("İkincil monitör duvar kağıdı başlatılamadı", exc)

    # ---------------------------------------------------------------- startup
    def start(
        self,
        show_settings: bool = True,
        backend: str = "auto",
        icon_adapter: str | None = None,
    ) -> None:
        if self.started:
            if show_settings:
                self.open_settings()
            return
        self.started = True
        # The wallpaper window is not an application window, so the
        # GApplication main loop would exit as soon as command-line handling
        # finishes without this hold.
        self.app.hold()
        self.choose_theme()
        try:
            self._spawn_wallpapers(backend=backend, icon_adapter=icon_adapter)
        except Exception as exc:
            self.log.exception("Duvar kağıdı başlatılamadı", exc)
            self._show_error(self.i18n.t("errStart", msg=str(exc)))
            self.app.release()
            self.app.quit()
            return
        self.tray = Tray(
            i18n=self.i18n,
            icon_path=_icon_path(),
            on_settings=self.open_settings,
            on_pause=self.set_manual_pause,
            on_reload=self.reload,
            on_autostart=self.set_autostart,
            on_exit=self.quit,
            on_low_mode=self.set_low_mode,
        )
        self.tray.set_autostart(self.autostart_enabled())
        self.tray.set_low_mode(self.settings.low_mode)
        self._apply_process_priority(self.settings.low_mode)
        if self.ipc_server.start():
            self.log.write(f"IPC soketi dinleniyor: {self.ipc_server.socket_path}")
        self._playlist_timeout = GLib.timeout_add_seconds(30, self._tick_playlist)
        self._stable_timeout = GLib.timeout_add_seconds(
            DEFAULT_STABLE_SECONDS, self._mark_startup_stable
        )
        self._welcome()
        if show_settings:
            self.open_settings()

    def _mark_startup_stable(self) -> bool:
        self._stable_timeout = 0
        self.crash_sentinel.mark_stable()
        return False

    def _welcome(self) -> None:
        flag = config_dir() / "welcome.shown"
        if flag.exists():
            return
        try:
            self.log.write(self.i18n.t("welcomeTitle"))
            flag.parent.mkdir(parents=True, exist_ok=True)
            flag.write_text("Linux\n", encoding="utf-8")
        except OSError:
            pass

    def _show_error(self, message: str) -> None:
        try:
            dialog = Gtk.MessageDialog(
                transient_for=None,
                flags=0,
                message_type=Gtk.MessageType.ERROR,
                buttons=Gtk.ButtonsType.OK,
                text=APP_NAME,
            )
            dialog.format_secondary_text(message)
            dialog.run()
            dialog.destroy()
        except Exception:
            self.log.write(message)

    # ---------------------------------------------------------------- windows
    def open_settings(self) -> None:
        if self._quitting:
            return
        if self.settings_window is None:
            url = self.server.url(f"settings.html?v={quote(self.version)}")
            self.settings_window = SettingsWindow(self, url)
        self.settings_window.show()

    def show_menu(self) -> None:
        if self.tray is not None:
            self.tray.popup()

    def reload(self) -> None:
        for wp in self._all_wallpapers():
            wp.reload()
        if self.wallpaper is not None:
            self.log.write("Tema yeniden yüklendi")

    def set_manual_pause(self, paused: bool) -> None:
        self.manual_pause = bool(paused)
        for wp in self._all_wallpapers():
            wp.set_manual_pause(self.manual_pause)
        if self.tray is not None:
            self.tray.set_paused(self.manual_pause)

    def autostart_enabled(self) -> bool:
        return False if self._profile_isolated else autostart.is_enabled()

    def set_autostart(self, enabled: bool) -> None:
        if self._profile_isolated:
            self.log.write("İzole profil: otomatik başlatma yazımı atlandı")
            return
        actual = autostart.set_enabled(enabled) and autostart.is_enabled()
        if self.tray is not None:
            self.tray.set_autostart(actual)
        self.log.write("Otomatik başlatma açık" if actual else "Otomatik başlatma kapalı")

    @staticmethod
    def _apply_process_priority(low_mode: bool) -> None:
        try:
            cur = os.getpriority(os.PRIO_PROCESS, 0)
            target = 10 if low_mode else 0
            if low_mode and cur < target:
                os.setpriority(os.PRIO_PROCESS, 0, target)
            elif not low_mode and cur > 0:
                os.setpriority(os.PRIO_PROCESS, 0, 0)
        except OSError:
            pass

    def set_low_mode(self, enabled: bool) -> None:
        val = bool(enabled)
        self.settings.low_mode = val
        self.schedule_save()
        self._apply_process_priority(val)
        if self.tray is not None:
            self.tray.set_low_mode(val)
        for wp in self._all_wallpapers():
            wp.set_low_mode(val)
        self._post_settings({"t": "lowMode", "v": val})
        self.log.write("Düşük performans modu açık" if val else "Düşük performans modu kapalı")

    def set_disable_auto_pause(self, enabled: bool) -> None:
        val = bool(enabled)
        self.settings.disable_auto_pause = val
        self.settings.save()
        for wp in self._all_wallpapers():
            wp.set_disable_auto_pause(val)
        self._post_settings({"t": "disableAutoPause", "v": val})
        self.log.write("Otomatik duraklatma devre dışı" if val else "Otomatik duraklatma etkin")

    def set_display_mode(self, mode: str) -> None:
        if mode not in DISPLAY_MODES or mode == self.settings.display_mode:
            return
        self.settings.display_mode = mode
        self.settings.save()
        try:
            self._spawn_wallpapers(self._requested_backend, self._requested_adapter)
        except Exception as exc:
            self.log.exception("Monitör kipi uygulanamadı", exc)
        self._post_settings({"t": "displayMode", "v": mode})
        self.log.write(f"Çoklu monitör kipi: {mode}")

    def set_solar_sync(self, enabled: bool) -> None:
        val = bool(enabled)
        self.settings.solar_sync = val
        self.schedule_save()
        for wp in self._all_wallpapers():
            wp.push_environment(force=True)
        self._post_settings({"t": "solarSync", "v": val})
        self.log.write("Güneş döngüsü senkronizasyonu açık" if val else "Güneş döngüsü senkronizasyonu kapalı")

    def set_window_aware(self, enabled: bool) -> None:
        val = bool(enabled)
        self.settings.window_aware = val
        self.schedule_save()
        for wp in self._all_wallpapers():
            wp._poll_icons_once()
        self._post_settings({"t": "windowAware", "v": val})
        self.log.write("Pencere duyarlı sahne açık" if val else "Pencere duyarlı sahne kapalı")

    def set_playlist(self, enabled: bool | None = None, interval_min: int | None = None) -> None:
        if enabled is not None:
            self.settings.playlist_enabled = bool(enabled)
        if interval_min is not None and int(interval_min) >= 1:
            self.settings.playlist_interval_min = max(1, min(1440, int(interval_min)))
        self.playlist.configure(self.settings.playlist_interval_min, self.settings.playlist_themes)
        self.playlist.mark_switched()
        self.schedule_save()
        self._post_settings(
            {
                "t": "playlistState",
                "enabled": self.settings.playlist_enabled,
                "intervalMin": self.settings.playlist_interval_min,
            }
        )

    def set_system_bridge(self, enabled: bool) -> None:
        val = bool(enabled)
        self.settings.system_bridge = val
        self.schedule_save()
        if not val:
            for wp in self._all_wallpapers():
                am = getattr(wp, "audio_meter", None)
                if am is not None:
                    am.stop()
        self._post_settings({"t": "systemBridge", "v": val})
        self.log.write("Yerel ses/sistem köprüsü açık" if val else "Yerel ses/sistem köprüsü kapalı")

    def set_prefer_mpv(self, enabled: bool) -> None:
        val = bool(enabled)
        self.settings.prefer_mpv = val
        self.schedule_save()
        for wp in self._all_wallpapers():
            wp.sync_mpv_video()
        self._post_settings({"t": "preferMpv", "v": val})
        self.log.write("mpv donanım video arka ucu tercih ediliyor" if val else "WebView video oynatıcı aktif")

    def sync_lockscreen(self) -> None:
        if self.wallpaper is None:
            self._post_settings({"t": "lockscreenSynced", "ok": False, "msg": self.i18n.t("errNotReady")})
            return

        def _done(_thumb: str | None, png: str | None, err: str | None) -> None:
            if err or not png:
                self._post_settings({"t": "lockscreenSynced", "ok": False, "msg": err or "snapshot failed"})
                return
            try:
                from .share import decode_data_url

                raw = decode_data_url(png)
                raw_path = config_dir() / "lockscreen-src.png"
                raw_path.parent.mkdir(parents=True, exist_ok=True)
                raw_path.write_bytes(raw)
                blurred = make_blurred_lockscreen(raw_path)
                backend = apply_linux_lockscreen(blurred)
                self.log.write(f"Kilit ekranı görseli eşitlendi ({backend}): {blurred}")
                self._post_settings({"t": "lockscreenSynced", "ok": True, "path": str(blurred), "backend": backend})
            except Exception as exc:
                self.log.exception("Kilit ekranı görseli oluşturulamadı", exc)
                self._post_settings({"t": "lockscreenSynced", "ok": False, "msg": str(exc)})

        self.wallpaper.request_snapshot(_done)

    def _ready_theme_ids(self) -> list[str]:
        return [
            str(t.get("id"))
            for t in self.themes.themes
            if t.get("id") and self.themes.allowed(str(t.get("id")))
        ]

    def _switch_theme_by_id(self, theme_id: str) -> bool:
        if not theme_id or not self.themes.allowed(theme_id):
            return False
        self._on_theme({"id": theme_id})
        self._post_settings({"t": "themeChanged", "id": theme_id})
        self.playlist.mark_switched()
        return True

    def _tick_playlist(self) -> bool:
        if self._quitting:
            self._playlist_timeout = 0
            return False
        if not self.settings.playlist_enabled or self.manual_pause:
            return True
        if self.playlist.should_rotate():
            nxt = self.playlist.next_theme(self._ready_theme_ids(), self.settings.theme)
            if nxt and nxt != self.settings.theme:
                self._switch_theme_by_id(nxt)
                self.log.write(f"Çalma listesi temayı değiştirdi: {nxt}")
            else:
                self.playlist.mark_switched()
        return True

    def _dispatch_ipc(self, req: dict[str, Any]) -> dict[str, Any]:
        done = threading.Event()
        box: dict[str, Any] = {}

        def _run() -> bool:
            try:
                box["resp"] = self._handle_ipc_on_main(req)
            except Exception as exc:
                box["resp"] = {"ok": False, "error": str(exc)}
            finally:
                done.set()
            return False

        GLib.idle_add(_run)
        if not done.wait(timeout=2.5):
            return {"ok": False, "error": "daemon busy"}
        return box.get("resp", {"ok": False, "error": "no response"})

    def _handle_ipc_on_main(self, req: dict[str, Any]) -> dict[str, Any]:
        cmd = str(req.get("cmd") or "").strip().lower()
        if cmd == "status":
            return {
                "ok": True,
                "theme": self.settings.theme,
                "paused": self.manual_pause,
                "lowMode": self.settings.low_mode,
                "displayMode": self.settings.display_mode,
                "solarSync": self.settings.solar_sync,
                "windowAware": self.settings.window_aware,
                "playlistEnabled": self.settings.playlist_enabled,
                "playlistIntervalMin": self.settings.playlist_interval_min,
            }
        if cmd == "setwp":
            tid = str(req.get("theme") or "").strip()
            if not self._switch_theme_by_id(tid):
                return {"ok": False, "error": f"unknown or unavailable theme: {tid}"}
            return {"ok": True, "theme": tid, "message": f"theme={tid}"}
        if cmd == "setprop":
            tid = str(req.get("theme") or self.settings.theme).strip()
            key = str(req.get("key") or "").strip()
            if not key or not self.themes.allowed(tid):
                return {"ok": False, "error": "invalid theme or key"}
            val = req.get("value")
            self._on_set({"theme": tid, "key": key, "value": val})
            return {"ok": True, "theme": tid, "key": key, "value": val, "message": f"{tid}.{key}={val}"}
        if cmd == "pause":
            self.set_manual_pause(True)
            return {"ok": True, "paused": True, "message": "paused"}
        if cmd == "play":
            self.set_manual_pause(False)
            return {"ok": True, "paused": False, "message": "playing"}
        if cmd == "toggle":
            self.set_manual_pause(not self.manual_pause)
            return {"ok": True, "paused": self.manual_pause, "message": "paused" if self.manual_pause else "playing"}
        if cmd == "random":
            nxt = PlaylistController.pick_random(self._ready_theme_ids(), self.settings.theme)
            if not nxt or not self._switch_theme_by_id(nxt):
                return {"ok": False, "error": "no ready theme available"}
            return {"ok": True, "theme": nxt, "message": f"theme={nxt}"}
        if cmd == "lowmode":
            st = str(req.get("state") or "toggle").lower()
            target = (not self.settings.low_mode) if st == "toggle" else (st == "on")
            self.set_low_mode(target)
            return {"ok": True, "lowMode": target, "message": f"lowMode={target}"}
        if cmd == "solar":
            st = str(req.get("state") or "toggle").lower()
            target = (not self.settings.solar_sync) if st == "toggle" else (st == "on")
            self.set_solar_sync(target)
            return {"ok": True, "solarSync": target, "message": f"solarSync={target}"}
        if cmd == "window-aware":
            st = str(req.get("state") or "toggle").lower()
            target = (not self.settings.window_aware) if st == "toggle" else (st == "on")
            self.set_window_aware(target)
            return {"ok": True, "windowAware": target, "message": f"windowAware={target}"}
        if cmd == "playlist":
            action = str(req.get("action") or "status").lower()
            interval = req.get("interval")
            if action == "on":
                self.set_playlist(True, int(interval) if interval else None)
            elif action == "off":
                self.set_playlist(False, int(interval) if interval else None)
            elif action == "next":
                nxt = self.playlist.next_theme(self._ready_theme_ids(), self.settings.theme)
                if nxt:
                    self._switch_theme_by_id(nxt)
            return {
                "ok": True,
                "playlistEnabled": self.settings.playlist_enabled,
                "playlistIntervalMin": self.settings.playlist_interval_min,
                "theme": self.settings.theme,
                "message": f"playlist={self.settings.playlist_enabled} interval={self.settings.playlist_interval_min}m theme={self.settings.theme}",
            }
        if cmd == "reload":
            self.reload()
            return {"ok": True, "message": "reloaded"}
        if cmd == "system-bridge":
            st = str(req.get("state") or "toggle").lower()
            target = (not self.settings.system_bridge) if st == "toggle" else (st == "on")
            self.set_system_bridge(target)
            return {"ok": True, "systemBridge": target, "message": f"systemBridge={target}"}
        if cmd == "mpv":
            st = str(req.get("state") or "toggle").lower()
            target = (not self.settings.prefer_mpv) if st == "toggle" else (st == "on")
            self.set_prefer_mpv(target)
            return {"ok": True, "preferMpv": target, "message": f"preferMpv={target}"}
        if cmd == "lockscreen-sync":
            self.sync_lockscreen()
            return {"ok": True, "message": "lockscreen sync requested"}
        if cmd == "exit":
            GLib.idle_add(self.quit)
            return {"ok": True, "message": "exiting"}
        return {"ok": False, "error": f"unknown command: {cmd}"}

    # ---------------------------------------------------------------- state
    def schedule_save(self) -> None:
        if self._save_timeout:
            GLib.source_remove(self._save_timeout)
        self._save_timeout = GLib.timeout_add(600, self._save_now)

    def _save_now(self) -> bool:
        self._save_timeout = 0
        self.settings.save()
        return False

    def on_icons_message(self, message: dict[str, Any]) -> None:
        self.last_icons = message
        if self.settings_window is not None:
            self.settings_window.post(message)

    def on_record_progress(self, progress: float) -> None:
        if self.settings_window is not None:
            self.settings_window.post({"t": "recProgress", "p": progress})

    def on_wallpaper_audio_play(self, play: bool) -> None:
        theme = self.settings.theme
        value = 1 if play else 0
        self.settings.for_theme(theme)["audioPlay"] = value
        self.schedule_save()
        if self.wallpaper is not None:
            self.wallpaper.set_settings({"audioPlay": value})
        self._post_settings({"t": "audioPlay", "theme": theme, "play": value})

    # ---------------------------------------------------------------- settings protocol
    def _post_settings(self, payload: dict[str, Any]) -> None:
        if self.settings_window is not None:
            self.settings_window.post(payload)

    def on_settings_message(self, text: str) -> None:
        try:
            message = json.loads(text)
        except ValueError:
            return
        if not isinstance(message, dict):
            return
        kind = message.get("t")
        if kind == "hello":
            self._send_init()
        elif kind == "set":
            self._on_set(message)
        elif kind == "pickAudio":
            self._on_pick_audio(message)
        elif kind == "importTheme":
            self._on_import_theme()
        elif kind == "removeTheme":
            self._on_remove_theme(message)
        elif kind == "reset":
            self._on_reset(message)
        elif kind == "theme":
            self._on_theme(message)
        elif kind == "lang":
            self._on_lang(message)
        elif kind == "virtualBarMode":
            self._on_virtual_bar_mode(message)
        elif kind == "lowMode":
            self.set_low_mode(bool(message.get("v")))
        elif kind == "disableAutoPause":
            self.set_disable_auto_pause(bool(message.get("v")))
        elif kind == "displayMode":
            mode = message.get("v")
            if isinstance(mode, str):
                self.set_display_mode(mode)
        elif kind == "solarSync":
            self.set_solar_sync(bool(message.get("v")))
        elif kind == "windowAware":
            self.set_window_aware(bool(message.get("v")))
        elif kind == "playlist":
            enabled = message.get("enabled")
            interval = message.get("intervalMin")
            self.set_playlist(
                enabled=bool(enabled) if isinstance(enabled, bool) else None,
                interval_min=int(interval) if isinstance(interval, (int, float)) else None,
            )
        elif kind == "systemBridge":
            self.set_system_bridge(bool(message.get("v")))
        elif kind == "preferMpv":
            self.set_prefer_mpv(bool(message.get("v")))
        elif kind == "syncLockscreen":
            self.sync_lockscreen()
        elif kind == "startLanPair":
            try:
                info = self.lan_pairing.start()
                self.log.write(f"LAN eşleştirme başladı: {info['host']}:{info['port']}")
                self._post_settings({"t": "lanPairState", "ok": True, **info})
            except Exception as exc:
                self.log.exception("LAN eşleştirme başlatılamadı", exc)
                self._post_settings({"t": "lanPairState", "ok": False, "active": False, "msg": str(exc)})
        elif kind == "stopLanPair":
            self.lan_pairing.stop()
            self._post_settings({"t": "lanPairState", "ok": True, "active": False})
        elif kind == "autostart":
            self.set_autostart(bool(message.get("v")))
            self._post_settings({"t": "autostart", "v": self.autostart_enabled()})
        elif kind == "share":
            self._share_image()
        elif kind == "saveSnapshot":
            self._finish_share_image(
                message.get("thumb"),
                message.get("png"),
                message.get("error"),
            )
        elif kind == "shareVideo":
            self._share_video()
        elif kind == "saveRecord":
            self._finish_share_video(
                message.get("mime"),
                message.get("poster"),
                message.get("video"),
                message.get("error"),
            )
        elif kind == "openShare":
            if self.last_share is not None:
                open_path(self.last_share)
        elif kind == "revealShare":
            if self.last_share is not None:
                reveal_path(self.last_share)
        elif kind == "exportDiagnostics":
            self._export_diagnostics()
        elif kind == "revealDiagnostics":
            last_diag = getattr(self, "last_diagnostics", None)
            if last_diag is not None:
                reveal_path(last_diag)
        elif kind == "openWebsite":
            self._open_url("https://deskland.app/")
        elif kind == "updateProgram":
            self._open_url("https://deskland.app/")
        elif kind == "autostartWarnSeen":
            pass

    def _export_diagnostics(self) -> None:
        try:
            wp = self.wallpaper
            backend = wp.backend if wp is not None else "none"
            screen = (wp.screen_w, wp.screen_h, wp.scale) if wp is not None else None
            pm = getattr(wp, "power_monitor", None) if wp is not None else None
            pstate = pm.poll(force=True) if pm is not None else None
            out = export_diagnostics(
                settings=self.settings,
                backend=backend,
                screen=screen,
                power_state=pstate,
                log_file=self.log.path,
            )
            self.last_diagnostics = out
            self.log.write(f"Tanılama günlüğü dışa aktarıldı: {out}")
            self._post_settings({"t": "diagExported", "ok": True, "path": str(out)})
        except Exception as exc:
            self.log.exception("Tanılama günlüğü dışa aktarılamadı", exc)
            self._post_settings({"t": "diagExported", "ok": False, "msg": str(exc)})

    def _send_init(self) -> None:
        screen_w = self.wallpaper.screen_w if self.wallpaper else 1920
        screen_h = self.wallpaper.screen_h if self.wallpaper else 1080
        self._post_settings(
            {
                "t": "init",
                "theme": self.settings.theme,
                "params": self.settings.params,
                "screen": {"w": screen_w, "h": screen_h},
                "autostart": self.autostart_enabled(),
                "autostartWarn": False,
                "autostartSw": "",
                "lang": self.settings.lang,
                "langEff": self.i18n.code,
                "langSys": self.i18n.system,
                "virtualBarMode": self.settings.virtual_bar_mode,
                "lowMode": self.settings.low_mode,
                "disableAutoPause": self.settings.disable_auto_pause,
                "displayMode": self.settings.display_mode,
                "solarSync": self.settings.solar_sync,
                "windowAware": self.settings.window_aware,
                "playlistEnabled": self.settings.playlist_enabled,
                "playlistIntervalMin": self.settings.playlist_interval_min,
                "systemBridge": self.settings.system_bridge,
                "preferMpv": self.settings.prefer_mpv,
                "canSaveSnapshot": True,
            }
        )
        if self.last_icons is not None:
            self._post_settings(self.last_icons)

    def _cleanup_audio_file(self, old_name: Any) -> None:
        if not isinstance(old_name, str) or not old_name:
            return
        try:
            target = (self.audio_dir / Path(old_name).name).resolve()
            target.relative_to(self.audio_dir.resolve())
            if target.is_file():
                target.unlink()
        except Exception:
            pass

    def _on_pick_audio(self, message: dict[str, Any]) -> None:
        theme = message.get("theme")
        if not isinstance(theme, str) or not self.themes.allowed(theme):
            return
        parent = self.settings_window.window if self.settings_window is not None else None
        dialog = Gtk.FileChooserDialog(
            title=self.i18n.t("audioPick") or "Choose audio",
            transient_for=parent,
            action=Gtk.FileChooserAction.OPEN,
        )
        dialog.add_buttons(
            Gtk.STOCK_CANCEL,
            Gtk.ResponseType.CANCEL,
            Gtk.STOCK_OPEN,
            Gtk.ResponseType.OK,
        )
        flt = Gtk.FileFilter()
        flt.set_name("Audio")
        for pat in ("*.mp3", "*.wav", "*.ogg", "*.opus", "*.flac", "*.m4a", "*.aac", "*.webm"):
            flt.add_pattern(pat)
        dialog.add_filter(flt)
        resp = dialog.run()
        chosen = dialog.get_filename()
        dialog.destroy()
        if resp != Gtk.ResponseType.OK or not chosen:
            return
        src = Path(chosen)
        if not src.is_file():
            return
        ext = src.suffix.lower() or ".audio"
        safe_name = f"{theme}_{int(time.time() * 1000)}{ext}"
        try:
            self.audio_dir.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, self.audio_dir / safe_name)
        except OSError as exc:
            self.log.write(f"Audio kopyalanamadı: {exc}")
            return
        params = self.settings.for_theme(theme)
        self._cleanup_audio_file(params.get("audioFile"))
        params["audioFile"] = safe_name
        params["audioName"] = src.name
        params["audioPlay"] = 1
        self.schedule_save()
        self._post_settings(
            {
                "t": "audioPicked",
                "theme": theme,
                "file": safe_name,
                "name": src.name,
                "play": 1,
            }
        )
        if theme == self.settings.theme and self.wallpaper is not None:
            self.wallpaper.set_settings({"audioFile": safe_name, "audioName": src.name, "audioPlay": 1})

    def _on_import_theme(self) -> None:
        parent = self.settings_window.window if self.settings_window is not None else None
        dialog = Gtk.FileChooserDialog(
            title=self.i18n.t("importTheme") or "Import theme",
            transient_for=parent,
            action=Gtk.FileChooserAction.OPEN,
        )
        dialog.add_buttons(
            Gtk.STOCK_CANCEL, Gtk.ResponseType.CANCEL,
            Gtk.STOCK_OPEN, Gtk.ResponseType.OK,
        )
        flt = Gtk.FileFilter()
        flt.set_name("Theme packages, HTML, images, videos")
        for ext in sorted(SUPPORTED_EXTS):
            flt.add_pattern(f"*{ext}")
            flt.add_pattern(f"*{ext.upper()}")
        dialog.add_filter(flt)
        all_flt = Gtk.FileFilter()
        all_flt.set_name("All files")
        all_flt.add_pattern("*")
        dialog.add_filter(all_flt)
        resp = dialog.run()
        chosen = dialog.get_filename()
        dialog.destroy()
        if resp != Gtk.ResponseType.OK or not chosen:
            return
        try:
            reserved = {str(t.get("id")) for t in self.themes.base_themes if t.get("id")}
            theme_obj = import_theme_file(Path(chosen), custom_dir=self.custom_themes_dir, reserved_ids=reserved)
        except Exception as exc:
            self.log.exception("Tema içe aktarılamadı", exc)
            self._post_settings({"t": "importError", "msg": str(exc)})
            return
        self.themes.load()
        self.settings.theme = theme_obj["id"]
        self.schedule_save()
        if self.wallpaper is not None:
            self.wallpaper.load_page(theme_obj["page"])
        self._post_settings({"t": "themeImported", "theme": theme_obj, "select": True})
        self.log.write(f"Tema içe aktarıldı: {theme_obj['id']}")

    def _on_remove_theme(self, message: dict[str, Any]) -> None:
        theme_id = message.get("id")
        if not isinstance(theme_id, str) or not theme_id:
            return
        if not remove_custom_theme(theme_id, custom_dir=self.custom_themes_dir):
            return
        self.settings.params.pop(theme_id, None)
        self.themes.load()
        if self.settings.theme == theme_id:
            self.choose_theme()
            if self.wallpaper is not None:
                self.wallpaper.load_page(self.current_page)
        self.schedule_save()
        self._post_settings({"t": "themeRemoved", "id": theme_id, "active": self.settings.theme})
        self.log.write(f"Özel tema kaldırıldı: {theme_id}")

    def _on_set(self, message: dict[str, Any]) -> None:
        theme = message.get("theme")
        key = message.get("key")
        if not isinstance(theme, str) or not isinstance(key, str):
            return
        if not self.themes.allowed(theme):
            return
        params = self.settings.for_theme(theme)
        if key == "audioFile" and not message.get("value"):
            self._cleanup_audio_file(params.get("audioFile"))
        params[key] = message.get("value")
        if theme == self.settings.theme:
            for wp in self._all_wallpapers():
                wp.set_settings({key: message.get("value")})
        self.schedule_save()

    def _on_reset(self, message: dict[str, Any]) -> None:
        theme = message.get("theme")
        if not isinstance(theme, str):
            return
        self.settings.params.pop(theme, None)
        if theme == self.settings.theme:
            for wp in self._all_wallpapers():
                wp.reset()
        self.schedule_save()
        self.log.write(f"Varsayılanlara dönüldü: {theme}")

    def _on_theme(self, message: dict[str, Any]) -> None:
        theme_id = message.get("id")
        if not isinstance(theme_id, str) or not self.themes.allowed(theme_id):
            self._post_settings({"t": "themeChanged", "id": self.settings.theme})
            self.log.write(f"Tema reddedildi: {theme_id}")
            return
        page = self.themes.page_for(theme_id)
        if not page or theme_id == self.settings.theme:
            return
        self.settings.theme = theme_id
        self.schedule_save()
        for wp in self._all_wallpapers():
            wp.load_page(page)
        self.log.write(f"Tema değişti: {theme_id}")

    def _on_lang(self, message: dict[str, Any]) -> None:
        value = message.get("v")
        if not isinstance(value, str):
            return
        if value != "auto" and value not in SUPPORTED_LANGS:
            return
        self.settings.lang = value
        self.i18n.set_lang(value)
        if self.tray is not None:
            self.tray.retranslate()
        self.schedule_save()
        self._post_settings(
            {
                "t": "lang",
                "v": self.settings.lang,
                "eff": self.i18n.code,
                "sys": self.i18n.system,
            }
        )

    def _on_virtual_bar_mode(self, message: dict[str, Any]) -> None:
        value = message.get("v")
        if value not in ("auto", "always", "off"):
            return
        self.settings.virtual_bar_mode = value
        self.schedule_save()
        if self.wallpaper is not None:
            self.wallpaper.send_virtual_state()

    def on_virtual_icons_saved(self, raw: Any) -> None:
        from .config import sanitize_virtual_icons

        self.settings.virtual_icons = sanitize_virtual_icons(raw)
        self.schedule_save()

    def on_virtual_bar_collapsed(self, value: Any) -> None:
        if isinstance(value, bool) or value is None:
            self.settings.virtual_bar_collapsed = value
            self.schedule_save()

    def _open_url(self, url: str) -> None:
        try:
            subprocess.Popen(
                ["xdg-open", url],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        except OSError:
            pass

    # ---------------------------------------------------------------- share
    def _finish_share_image(
        self, thumb: str | None, png: str | None, error: str | None
    ) -> None:
        if error is not None or png is None:
            self._post_settings(
                {
                    "t": "shareResult",
                    "ok": False,
                    "msg": error or self.i18n.t("errNoImage"),
                }
            )
            return
        try:
            path = save_share(png, pictures_dir() / "DiyarDesk", "image")
            self.last_share = path
            copy_image(path)
            self._post_settings(
                {
                    "t": "shareResult",
                    "ok": True,
                    "thumb": thumb,
                    "path": str(path),
                }
            )
            self.log.write(f"Paylaşım görseli: {path}")
        except Exception as exc:
            self.log.exception("Paylaşım görseli kaydedilemedi", exc)
            self._post_settings(
                {"t": "shareResult", "ok": False, "msg": str(exc)}
            )

    def _share_image(self) -> None:
        if self.wallpaper is None:
            self._post_settings(
                {"t": "shareResult", "ok": False, "msg": self.i18n.t("errNotReady")}
            )
            return
        self.wallpaper.request_snapshot(self._finish_share_image)

    def _finish_share_video(
        self,
        mime: str | None,
        poster: str | None,
        video: str | None,
        error: str | None,
    ) -> None:
        if error is not None or video is None:
            self._post_settings(
                {
                    "t": "shareResult",
                    "ok": False,
                    "msg": error or self.i18n.t("errNoVideo"),
                }
            )
            return
        try:
            path = save_share(
                video,
                pictures_dir() / "DiyarDesk",
                "video",
                mime or "",
            )
            self.last_share = path
            copy_files([path])
            self._post_settings(
                {
                    "t": "shareResult",
                    "ok": True,
                    "kind": "video",
                    "thumb": poster,
                    "path": str(path),
                    "url": self.server.url(f"__share/{quote(path.name)}"),
                }
            )
            self.log.write(f"Paylaşım videosu: {path}")
        except Exception as exc:
            self.log.exception("Paylaşım videosu kaydedilemedi", exc)
            self._post_settings(
                {"t": "shareResult", "ok": False, "msg": str(exc)}
            )

    def _share_video(self) -> None:
        if self.wallpaper is None:
            self._post_settings(
                {"t": "shareResult", "ok": False, "msg": self.i18n.t("errNotReady")}
            )
            return
        self.wallpaper.request_record(self._finish_share_video)

    # ---------------------------------------------------------------- quit
    def quit(self) -> None:
        if self._quitting:
            return
        self._quitting = True
        self.log.write("Çıkış")
        if self._save_timeout:
            GLib.source_remove(self._save_timeout)
        if self._playlist_timeout:
            GLib.source_remove(self._playlist_timeout)
            self._playlist_timeout = 0
        if self._stable_timeout:
            GLib.source_remove(self._stable_timeout)
            self._stable_timeout = 0
        self.crash_sentinel.mark_stable()
        self.settings.save()
        if self.settings_window is not None:
            self.settings_window.destroy()
            self.settings_window = None
        self._shutdown_wallpapers()
        self.ipc_server.stop()
        self.lan_pairing.stop()
        if self.tray is not None:
            self.tray.destroy()
            self.tray = None
        self.server.stop()
        self.app.release()
        self.app.quit()


class DesklandApplication(Gtk.Application):
    def __init__(self) -> None:
        super().__init__(
            application_id=APP_ID,
            flags=Gio.ApplicationFlags.HANDLES_COMMAND_LINE,
        )
        self.host: DesklandHost | None = None
        # Do not register the custom flags with add_main_option(): GLib then
        # consumes them and do_command_line() never sees "--exit" / "--static"
        # / "--autostart" in get_arguments(). HANDLES_COMMAND_LINE leaves
        # unregistered options in the argument list for us to parse.

    def do_startup(self) -> None:
        Gtk.Application.do_startup(self)
        self.host = DesklandHost(self)

    def do_command_line(self, command_line: Gio.ApplicationCommandLine) -> int:
        args = [str(a) for a in command_line.get_arguments()[1:]]
        host = self.host
        if host is None:
            return 1
        if "--exit" in args:
            host.quit()
            return 0
        if "--menu" in args:
            host.show_menu()
            return 0
        if "--settings" in args:
            if not host.started:
                host.start(show_settings=True, **_backend_args(args))
            else:
                host.open_settings()
            return 0
        host.start(
            show_settings="--autostart" not in args,
            **_backend_args(args),
        )
        return 0

    def do_activate(self) -> None:
        if self.host is not None and not self.host.started:
            self.host.start(show_settings=True)


def _backend_args(args: list[str]) -> dict[str, str | None]:
    backend = (
        "static"
        if "--static" in args
        else "x11"
        if "--x11" in args
        else "gnome-shell"
        if "--gnome-shell" in args
        else "layer-shell"
        if "--layer-shell" in args
        else "auto"
    )
    adapter = None
    if "--icon-adapter" in args:
        index = args.index("--icon-adapter")
        if index + 1 < len(args):
            adapter = args[index + 1]
    return {"backend": backend, "icon_adapter": adapter}


def main(argv: list[str] | None = None) -> int:
    argv = list(argv or sys.argv)
    app = DesklandApplication()
    return app.run(argv)
