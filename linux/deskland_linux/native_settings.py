"""Zero-WebKit native GTK3 + OpenGL 3.3 Core settings window for DiyarDesk on Linux.

Mirrors Windows' `NativeSettingsForm.cs`: when `rustOpenGl` is enabled, opens a
lightweight native GTK3 window with a live `NativeGlArea` (OpenGL 3.3 Core)
preview and zero `WebKitWebProcess` overhead. Switching `rustOpenGl` off
transitions seamlessly back to `SettingsWindow` (`settings.html` in WebKitGTK).
"""

from __future__ import annotations

import json
import random
from pathlib import Path
from typing import Any

from .config import DISPLAY_MODES, SUPPORTED_LANGS, prototype_dir
from .native_gl import NativeGlArea, NativeGlLibrary, is_native_gl_page, is_native_gl_theme
from .virtual_icons import VIRTUAL_ICON_TYPES, VIRTUAL_ICON_TYPE_LABELS, infer_icon_type


class SettingsI18n:
    """Pure-Python reader for prototype/i18n/<code>.json (`ui` and `themes` tables)."""

    def __init__(self, proto: Path | None = None, code: str = "en") -> None:
        self.proto = proto or prototype_dir()
        self.code = code if code in SUPPORTED_LANGS else "en"
        self._cache: dict[str, dict[str, Any]] = {}

    def set_code(self, code: str) -> None:
        self.code = code if code in SUPPORTED_LANGS else "en"

    def _table(self, code: str) -> dict[str, Any]:
        if code in self._cache:
            return self._cache[code]
        data: dict[str, Any] = {}
        try:
            raw = (self.proto / "i18n" / f"{code}.json").read_text(encoding="utf-8")
            parsed = json.loads(raw)
            if isinstance(parsed, dict):
                data = parsed
        except (OSError, ValueError):
            data = {}
        self._cache[code] = data
        return data

    def _chain(self) -> tuple[str, ...]:
        seen: list[str] = []
        for c in (self.code, "en", "zh-CN"):
            if c not in seen:
                seen.append(c)
        return tuple(seen)

    def lang_display_name(self, code: str) -> str:
        name = self._table(code).get("name")
        return str(name) if isinstance(name, str) and name else code

    def ui(self, key: str, fallback: str | None = None) -> str:
        for code in self._chain():
            ui_map = self._table(code).get("ui")
            if isinstance(ui_map, dict):
                val = ui_map.get(key)
                if isinstance(val, str) and val:
                    return val
        return fallback if fallback is not None else key

    def theme_entry(self, theme_id: str, code: str) -> dict[str, Any]:
        themes = self._table(code).get("themes")
        if isinstance(themes, dict):
            entry = themes.get(theme_id)
            if isinstance(entry, dict):
                return entry
        return {}

    def theme_name(self, theme_id: str, fallback: str = "") -> str:
        for code in self._chain():
            val = self.theme_entry(theme_id, code).get("name")
            if isinstance(val, str) and val:
                return val
        return fallback or theme_id

    def theme_desc(self, theme_id: str, fallback: str = "") -> str:
        for code in self._chain():
            val = self.theme_entry(theme_id, code).get("desc")
            if isinstance(val, str) and val:
                return val
        return fallback

    def group_name(self, theme_id: str, group_idx: int, fallback: str = "") -> str:
        for code in self._chain():
            groups = self.theme_entry(theme_id, code).get("groups")
            if isinstance(groups, list) and 0 <= group_idx < len(groups):
                val = groups[group_idx]
                if isinstance(val, str) and val:
                    return val
        return fallback

    def _param_entry(self, theme_id: str, key: str, code: str) -> dict[str, Any]:
        params = self.theme_entry(theme_id, code).get("params")
        if isinstance(params, dict):
            val = params.get(key)
            if isinstance(val, dict):
                return val
        return {}

    def param_label(self, theme_id: str, key: str, fallback: str = "") -> str:
        for code in self._chain():
            val = self._param_entry(theme_id, key, code).get("label")
            if isinstance(val, str) and val:
                return val
        return fallback or key

    def param_hint(self, theme_id: str, key: str, fallback: str = "") -> str:
        for code in self._chain():
            val = self._param_entry(theme_id, key, code).get("hint")
            if isinstance(val, str) and val:
                return val
        return fallback

    def param_options(self, theme_id: str, key: str, fallback: list[str]) -> list[str]:
        for code in self._chain():
            opts = self._param_entry(theme_id, key, code).get("options")
            if isinstance(opts, list) and len(opts) == len(fallback):
                return [str(x) for x in opts]
        return [str(x) for x in fallback]


def format_param_value(param: dict[str, Any], value: Any) -> str:
    """Format a slider parameter value consistently with prototype/settings.html."""
    try:
        num = float(value)
    except (TypeError, ValueError):
        return str(value)
    fmt = param.get("format")
    unit = str(param.get("unit") or "")
    step = float(param.get("step", 1) or 1)
    if fmt == "times":
        return f"{num:.2f}×" if step < 0.1 else f"{num:.1f}×"
    if fmt == "pct":
        return f"{int(round(num * 100))}%"
    if abs(step - round(step)) < 1e-6:
        return f"{int(round(num))}{unit}"
    decimals = 2 if step < 0.1 else 1
    return f"{num:.{decimals}f}{unit}"


def is_toggle_param(param: dict[str, Any]) -> bool:
    if param.get("options") or param.get("type") in ("audio", "color"):
        return False
    return (
        param.get("min") == 0
        and param.get("max") == 1
        and param.get("step") == 1
        and not param.get("unit")
        and not param.get("format")
    )


_CSS = b"""
.dd-sidebar {
    border-right: 1px solid alpha(@theme_fg_color, 0.12);
}
.dd-theme-row {
    padding: 8px 10px;
    border-radius: 8px;
    margin: 2px 6px;
}
.dd-badge-gl {
    background-color: rgba(46, 160, 67, 0.22);
    color: #3fb950;
    border-radius: 999px;
    padding: 1px 8px;
    font-size: 10px;
    font-weight: 700;
}
.dd-badge-web {
    background-color: rgba(56, 139, 253, 0.20);
    color: #58a6ff;
    border-radius: 999px;
    padding: 1px 8px;
    font-size: 10px;
    font-weight: 700;
}
.dd-card {
    background-color: alpha(@theme_fg_color, 0.045);
    border: 1px solid alpha(@theme_fg_color, 0.10);
    border-radius: 12px;
    padding: 12px 14px;
}
.dd-banner-gl {
    background-color: rgba(46, 160, 67, 0.12);
    border: 1px solid rgba(63, 185, 80, 0.35);
    border-radius: 12px;
    padding: 12px 14px;
}
.dd-group-title {
    font-weight: 700;
    font-size: 12px;
    opacity: 0.78;
}
.dd-dim {
    opacity: 0.72;
    font-size: 12px;
}
.dd-preview-frame {
    border-radius: 12px;
    border: 1px solid alpha(@theme_fg_color, 0.16);
    background-color: #0b0e14;
}
"""


class NativeSettingsWindow:
    """Native GTK3 settings window with embedded live Rust + OpenGL 3.3 preview."""

    is_native_gtk = True

    def __init__(self, host: Any) -> None:
        import gi

        gi.require_version("Gtk", "3.0")
        gi.require_version("Gdk", "3.0")
        from gi.repository import Gdk, GLib, Gtk, Pango

        self._Gtk = Gtk
        self._Gdk = Gdk
        self._GLib = GLib
        self._Pango = Pango
        self.host = host
        self._i18n = SettingsI18n(prototype_dir(), host.i18n.code)
        self._closed = False
        self._syncing = False
        self._preview_gl: NativeGlArea | None = None
        self._theme_rows: dict[str, Any] = {}
        self._filter_text = ""

        # Install lightweight CSS styling once per screen
        provider = Gtk.CssProvider()
        try:
            provider.load_from_data(_CSS)
            screen = Gdk.Screen.get_default()
            if screen is not None:
                Gtk.StyleContext.add_provider_for_screen(
                    screen, provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
                )
        except Exception:
            pass

        self.window = Gtk.Window(type=Gtk.WindowType.TOPLEVEL)
        self.window.set_title("DiyarDesk")
        self.window.set_default_size(1140, 780)
        self.window.set_size_request(920, 640)
        self.window.set_position(Gtk.WindowPosition.CENTER)
        self.window.connect("delete-event", self._on_delete)
        self.window.connect("window-state-event", self._on_window_state)

        self._build_headerbar()
        self._build_body()
        self._populate_theme_list()
        self._select_theme_row(self.host.settings.theme)
        self._rebuild_theme_controls()
        self._sync_system_controls()
        self._sync_preview()

    # ------------------------------------------------------------------ UI build
    def _build_headerbar(self) -> None:
        Gtk = self._Gtk
        hb = Gtk.HeaderBar()
        hb.set_show_close_button(True)
        hb.set_title("DiyarDesk")
        hb.set_subtitle("Rust OpenGL 3.3 Core · GTK3")

        # Left: Import theme button
        self._import_btn = Gtk.Button(label="+ " + self._i18n.ui("importTheme", "Import theme"))
        self._import_btn.connect("clicked", lambda *_: self.host.on_settings_message(json.dumps({"t": "importTheme"})))
        hb.pack_start(self._import_btn)

        # Right: Language selector
        self._lang_combo = Gtk.ComboBoxText()
        self._lang_codes: list[str] = ["auto", *SUPPORTED_LANGS]
        self._populate_lang_combo()
        self._lang_combo.connect("changed", self._on_lang_changed)
        hb.pack_end(self._lang_combo)

        # Right: Prominent Rust OpenGL switch
        gl_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        self._hdr_gl_lbl = Gtk.Label(label=self._i18n.ui("rustOpenGl", "Rust OpenGL"))
        self._hdr_gl_lbl.get_style_context().add_class("dd-badge-gl")
        self._hdr_gl_switch = Gtk.Switch()
        self._hdr_gl_switch.set_valign(Gtk.Align.CENTER)
        self._hdr_gl_switch.set_active(bool(self.host.settings.rust_opengl))
        self._hdr_gl_switch.set_sensitive(NativeGlLibrary.is_available())
        self._hdr_gl_switch.connect("notify::active", self._on_rust_gl_toggled)
        gl_box.pack_start(self._hdr_gl_lbl, False, False, 0)
        gl_box.pack_start(self._hdr_gl_switch, False, False, 0)
        hb.pack_end(gl_box)

        self.window.set_titlebar(hb)

    def _populate_lang_combo(self) -> None:
        self._syncing = True
        try:
            self._lang_combo.remove_all()
            self._lang_combo.append("auto", self._i18n.ui("langAuto", "Auto"))
            for code in SUPPORTED_LANGS:
                self._lang_combo.append(code, self._i18n.lang_display_name(code))
            cur = self.host.settings.lang
            self._lang_combo.set_active_id(cur if cur in self._lang_codes else "auto")
        finally:
            self._syncing = False

    def _build_body(self) -> None:
        Gtk = self._Gtk
        root = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=0)

        # --- Left sidebar (theme collection) ---
        sidebar = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        sidebar.set_size_request(310, -1)
        sidebar.get_style_context().add_class("dd-sidebar")

        search_wrap = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        search_wrap.set_margin_top(10)
        search_wrap.set_margin_start(10)
        search_wrap.set_margin_end(10)
        self._search_entry = Gtk.SearchEntry()
        self._search_entry.set_placeholder_text(self._i18n.ui("themes", "Themes") + "…")
        self._search_entry.connect("search-changed", self._on_search_changed)
        search_wrap.pack_start(self._search_entry, True, True, 0)
        sidebar.pack_start(search_wrap, False, False, 0)

        list_scroll = Gtk.ScrolledWindow()
        list_scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self._theme_list = Gtk.ListBox()
        self._theme_list.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self._theme_list.connect("row-selected", self._on_theme_row_selected)
        list_scroll.add(self._theme_list)
        sidebar.pack_start(list_scroll, True, True, 0)

        self._remove_theme_btn = Gtk.Button(label=self._i18n.ui("removeTheme", "Remove theme"))
        self._remove_theme_btn.set_margin_start(10)
        self._remove_theme_btn.set_margin_end(10)
        self._remove_theme_btn.set_margin_bottom(10)
        self._remove_theme_btn.connect("clicked", self._on_remove_clicked)
        sidebar.pack_end(self._remove_theme_btn, False, False, 0)

        root.pack_start(sidebar, False, False, 0)

        # --- Right main pane (live preview + tabs) ---
        right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        right.set_margin_top(12)
        right.set_margin_bottom(12)
        right.set_margin_start(14)
        right.set_margin_end(14)

        # Live OpenGL 3.3 preview container
        self._preview_frame = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        self._preview_frame.set_size_request(-1, 250)
        self._preview_frame.get_style_context().add_class("dd-preview-frame")
        self._preview_fallback_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        self._preview_fallback_box.set_valign(Gtk.Align.CENTER)
        self._preview_fallback_box.set_halign(Gtk.Align.CENTER)
        self._preview_fallback_lbl = Gtk.Label()
        self._preview_fallback_lbl.set_line_wrap(True)
        self._preview_fallback_lbl.set_justify(Gtk.Justification.CENTER)
        self._preview_fallback_box.pack_start(self._preview_fallback_lbl, False, False, 0)
        self._preview_frame.pack_start(self._preview_fallback_box, True, True, 0)
        right.pack_start(self._preview_frame, False, False, 0)

        # Action & status bar under preview
        action_bar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self._theme_title_lbl = Gtk.Label()
        self._theme_title_lbl.set_xalign(0.0)
        self._theme_title_lbl.set_ellipsize(self._Pango.EllipsizeMode.END)
        action_bar.pack_start(self._theme_title_lbl, True, True, 0)

        self._reset_btn = Gtk.Button(label=self._i18n.ui("reset", "Reset"))
        self._reset_btn.connect("clicked", self._on_reset_clicked)
        action_bar.pack_end(self._reset_btn, False, False, 0)

        self._snap_btn = Gtk.Button(label=self._i18n.ui("shareImage", "Snapshot"))
        self._snap_btn.connect("clicked", lambda *_: self.host.on_settings_message(json.dumps({"t": "share"})))
        action_bar.pack_end(self._snap_btn, False, False, 0)

        self._lockscreen_btn = Gtk.Button(label=self._i18n.ui("syncLockscreen", "Lockscreen"))
        self._lockscreen_btn.connect("clicked", lambda *_: self.host.on_settings_message(json.dumps({"t": "syncLockscreen"})))
        action_bar.pack_end(self._lockscreen_btn, False, False, 0)

        right.pack_start(action_bar, False, False, 0)

        # Virtual icons quick-bar (for iconless desktops like GNOME, Hyprland, Niri, Sway)
        vbar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        vbar.get_style_context().add_class("dd-card")
        self._vicons_title_lbl = Gtk.Label()
        self._vicons_title_lbl.set_xalign(0.0)
        self._vicons_count_lbl = Gtk.Label()
        self._vicons_count_lbl.get_style_context().add_class("dd-badge-web")

        self._vicons_add_btn = Gtk.Button(label="+ İkon")
        self._vicons_add_btn.connect("clicked", lambda *_: self.host.add_virtual_icons(1))

        self._vicons_add5_btn = Gtk.Button(label="+5")
        self._vicons_add5_btn.connect("clicked", lambda *_: self.host.add_virtual_icons(5))

        self._vicons_rename_btn = Gtk.Button(label="✎ İsimler")
        self._vicons_rename_btn.connect("clicked", self._on_rename_vicons_clicked)

        self._vicons_shuffle_btn = Gtk.Button(label="Karıştır")
        self._vicons_shuffle_btn.connect("clicked", lambda *_: self.host.shuffle_virtual_icons())

        self._vicons_clear_btn = Gtk.Button(label="Temizle")
        self._vicons_clear_btn.connect("clicked", lambda *_: self.host.clear_virtual_icons())

        self._vicons_mode_combo = Gtk.ComboBoxText()
        self._vicons_mode_combo.append("auto", self._i18n.ui("virtualBarAuto", "Icons: Auto"))
        self._vicons_mode_combo.append("always", self._i18n.ui("virtualBarAlways", "Icons: Always"))
        self._vicons_mode_combo.append("off", self._i18n.ui("virtualBarOff", "Icons: Off"))
        self._vicons_mode_combo.connect(
            "changed",
            lambda c: self._if_not_syncing(
                lambda: self.host.set_virtual_bar_mode(c.get_active_id() or "auto")
            ),
        )

        vbar.pack_start(self._vicons_title_lbl, False, False, 0)
        vbar.pack_start(self._vicons_count_lbl, False, False, 0)
        vbar.pack_end(self._vicons_mode_combo, False, False, 0)
        vbar.pack_end(self._vicons_clear_btn, False, False, 0)
        vbar.pack_end(self._vicons_shuffle_btn, False, False, 0)
        vbar.pack_end(self._vicons_rename_btn, False, False, 0)
        vbar.pack_end(self._vicons_add5_btn, False, False, 0)
        vbar.pack_end(self._vicons_add_btn, False, False, 0)
        right.pack_start(vbar, False, False, 0)

        self._status_lbl = Gtk.Label()
        self._status_lbl.set_xalign(0.0)
        self._status_lbl.get_style_context().add_class("dd-dim")
        self._status_lbl.set_no_show_all(True)
        right.pack_start(self._status_lbl, False, False, 0)

        # Notebook: Theme Controls + System Settings
        self._notebook = Gtk.Notebook()

        # Tab 1: Theme parameters
        params_scroll = Gtk.ScrolledWindow()
        params_scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self._params_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        self._params_box.set_margin_top(10)
        self._params_box.set_margin_bottom(12)
        self._params_box.set_margin_start(8)
        self._params_box.set_margin_end(8)
        params_scroll.add(self._params_box)
        self._tab_params_lbl = Gtk.Label(label=self._i18n.ui("customize", "Customize"))
        self._notebook.append_page(params_scroll, self._tab_params_lbl)

        # Tab 2: App & System settings
        sys_scroll = Gtk.ScrolledWindow()
        sys_scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self._sys_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        self._sys_box.set_margin_top(10)
        self._sys_box.set_margin_bottom(12)
        self._sys_box.set_margin_start(8)
        self._sys_box.set_margin_end(8)
        self._build_system_tab()
        sys_scroll.add(self._sys_box)
        self._tab_sys_lbl = Gtk.Label(label=self._i18n.ui("settings", "System"))
        self._notebook.append_page(sys_scroll, self._tab_sys_lbl)

        right.pack_start(self._notebook, True, True, 0)
        root.pack_start(right, True, True, 0)
        self.window.add(root)

    def _build_system_tab(self) -> None:
        Gtk = self._Gtk
        box = self._sys_box

        # Rust OpenGL banner card
        gl_card = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        gl_card.get_style_context().add_class("dd-banner-gl")
        gl_info = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=3)
        self._sys_gl_title = Gtk.Label()
        self._sys_gl_title.set_xalign(0.0)
        self._sys_gl_desc = Gtk.Label()
        self._sys_gl_desc.set_xalign(0.0)
        self._sys_gl_desc.set_line_wrap(True)
        self._sys_gl_desc.get_style_context().add_class("dd-dim")
        gl_info.pack_start(self._sys_gl_title, False, False, 0)
        gl_info.pack_start(self._sys_gl_desc, False, False, 0)
        self._sys_gl_switch = Gtk.Switch()
        self._sys_gl_switch.set_valign(Gtk.Align.CENTER)
        self._sys_gl_switch.set_sensitive(NativeGlLibrary.is_available())
        self._sys_gl_switch.connect("notify::active", self._on_rust_gl_toggled)
        gl_card.pack_start(gl_info, True, True, 0)
        gl_card.pack_end(self._sys_gl_switch, False, False, 0)
        box.pack_start(gl_card, False, False, 0)

        def make_switch_row(key: str, default_lbl: str, on_change):
            row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
            row.get_style_context().add_class("dd-card")
            lbl = Gtk.Label(label=self._i18n.ui(key, default_lbl))
            lbl.set_xalign(0.0)
            lbl.set_line_wrap(True)
            sw = Gtk.Switch()
            sw.set_valign(Gtk.Align.CENTER)
            sw.connect("notify::active", lambda s, *_: self._if_not_syncing(lambda: on_change(bool(s.get_active()))))
            row.pack_start(lbl, True, True, 0)
            row.pack_end(sw, False, False, 0)
            box.pack_start(row, False, False, 0)
            return lbl, sw

        self._autostart_lbl, self._autostart_sw = make_switch_row(
            "autostart", "Start on login", lambda v: self.host.set_autostart(v)
        )
        self._low_mode_lbl, self._low_mode_sw = make_switch_row(
            "lowMode", "Low-power mode (15 FPS)", lambda v: self.host.set_low_mode(v)
        )
        self._no_pause_lbl, self._no_pause_sw = make_switch_row(
            "disableAutoPause", "Keep running behind fullscreen windows", lambda v: self.host.set_disable_auto_pause(v)
        )
        self._solar_lbl, self._solar_sw = make_switch_row(
            "solarSync", "Solar day/night sync", lambda v: self.host.set_solar_sync(v)
        )
        self._win_aware_lbl, self._win_aware_sw = make_switch_row(
            "windowAware", "Window-aware scenery", lambda v: self.host.set_window_aware(v)
        )
        self._sys_bridge_lbl, self._sys_bridge_sw = make_switch_row(
            "systemBridge", "Local audio & system stats bridge", lambda v: self.host.set_system_bridge(v)
        )
        self._mpv_lbl, self._mpv_sw = make_switch_row(
            "preferMpv", "Hardware mpv video backend (X11)", lambda v: self.host.set_prefer_mpv(v)
        )

        # Multi-monitor display mode
        dm_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        dm_row.get_style_context().add_class("dd-card")
        self._display_mode_lbl = Gtk.Label(label=self._i18n.ui("displayMode", "Multi-monitor mode"))
        self._display_mode_lbl.set_xalign(0.0)
        self._display_mode_combo = Gtk.ComboBoxText()
        for mode in DISPLAY_MODES:
            self._display_mode_combo.append(mode, mode)
        self._display_mode_combo.connect(
            "changed",
            lambda c: self._if_not_syncing(lambda: self.host.set_display_mode(c.get_active_id() or "primary")),
        )
        dm_row.pack_start(self._display_mode_lbl, True, True, 0)
        dm_row.pack_end(self._display_mode_combo, False, False, 0)
        box.pack_start(dm_row, False, False, 0)

        # Playlist rotation
        pl_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        pl_row.get_style_context().add_class("dd-card")
        self._playlist_lbl = Gtk.Label(label=self._i18n.ui("playlist", "Theme playlist rotation"))
        self._playlist_lbl.set_xalign(0.0)
        self._playlist_spin = Gtk.SpinButton.new_with_range(1, 1440, 5)
        self._playlist_spin.connect(
            "value-changed",
            lambda sp: self._if_not_syncing(lambda: self.host.set_playlist(interval_min=int(sp.get_value()))),
        )
        self._playlist_sw = Gtk.Switch()
        self._playlist_sw.set_valign(Gtk.Align.CENTER)
        self._playlist_sw.connect(
            "notify::active",
            lambda s, *_: self._if_not_syncing(lambda: self.host.set_playlist(enabled=bool(s.get_active()))),
        )
        pl_row.pack_start(self._playlist_lbl, True, True, 0)
        pl_row.pack_end(self._playlist_sw, False, False, 0)
        pl_row.pack_end(self._playlist_spin, False, False, 0)
        box.pack_start(pl_row, False, False, 0)

        # Diagnostics export
        diag_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        diag_row.get_style_context().add_class("dd-card")
        self._diag_lbl = Gtk.Label(label=self._i18n.ui("diagnostics", "Diagnostics"))
        self._diag_lbl.set_xalign(0.0)
        self._diag_btn = Gtk.Button(label=self._i18n.ui("exportDiagnostics", "Export log"))
        self._diag_btn.connect("clicked", lambda *_: self.host.on_settings_message(json.dumps({"t": "exportDiagnostics"})))
        diag_row.pack_start(self._diag_lbl, True, True, 0)
        diag_row.pack_end(self._diag_btn, False, False, 0)
        box.pack_start(diag_row, False, False, 0)

    def _if_not_syncing(self, fn) -> None:
        if not self._syncing:
            fn()

    # ------------------------------------------------------------------ Theme list
    def _populate_theme_list(self) -> None:
        Gtk = self._Gtk
        Pango = self._Pango
        self._syncing = True
        try:
            for child in list(self._theme_list.get_children()):
                self._theme_list.remove(child)
            self._theme_rows.clear()

            needle = self._filter_text.strip().lower()
            for theme in self.host.themes.themes:
                tid = str(theme.get("id") or "")
                if not tid or not self.host.themes.allowed(tid):
                    continue
                name = self._i18n.theme_name(tid, str(theme.get("name") or tid))
                desc = self._i18n.theme_desc(tid, str(theme.get("desc") or ""))
                if needle and needle not in tid.lower() and needle not in name.lower() and needle not in desc.lower():
                    continue

                row = Gtk.ListBoxRow()
                row._theme_id = tid
                row.get_style_context().add_class("dd-theme-row")
                vbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=3)

                top = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
                nlbl = Gtk.Label()
                nlbl.set_markup(f"<b>{self._GLib.markup_escape_text(name)}</b>")
                nlbl.set_xalign(0.0)
                nlbl.set_ellipsize(Pango.EllipsizeMode.END)
                top.pack_start(nlbl, True, True, 0)

                is_gl = is_native_gl_theme(tid) or is_native_gl_page(str(theme.get("page") or ""))
                badge = Gtk.Label(label="Rust GL" if is_gl else "HTML/Web")
                badge.get_style_context().add_class("dd-badge-gl" if is_gl else "dd-badge-web")
                top.pack_end(badge, False, False, 0)
                vbox.pack_start(top, False, False, 0)

                if desc:
                    dlbl = Gtk.Label(label=desc)
                    dlbl.set_xalign(0.0)
                    dlbl.set_ellipsize(Pango.EllipsizeMode.END)
                    dlbl.set_max_width_chars(34)
                    dlbl.get_style_context().add_class("dd-dim")
                    vbox.pack_start(dlbl, False, False, 0)

                row.add(vbox)
                self._theme_list.add(row)
                self._theme_rows[tid] = row

            self._theme_list.show_all()
        finally:
            self._syncing = False

    def _select_theme_row(self, theme_id: str) -> None:
        self._syncing = True
        try:
            row = self._theme_rows.get(theme_id)
            if row is not None:
                self._theme_list.select_row(row)
        finally:
            self._syncing = False

    def _on_search_changed(self, entry: Any) -> None:
        self._filter_text = entry.get_text() or ""
        self._populate_theme_list()
        self._select_theme_row(self.host.settings.theme)

    def _on_theme_row_selected(self, _listbox: Any, row: Any) -> None:
        if self._syncing or row is None:
            return
        tid = getattr(row, "_theme_id", None)
        if not tid:
            return
        if tid != self.host.settings.theme:
            self.host.on_settings_message(json.dumps({"t": "theme", "id": tid}))
        self._rebuild_theme_controls()
        self._sync_preview()

    def _on_remove_clicked(self, _btn: Any) -> None:
        tid = self.host.settings.theme
        self.host.on_settings_message(json.dumps({"t": "removeTheme", "id": tid}))

    # ------------------------------------------------------------------ Live GL preview
    def _ensure_preview_gl(self) -> NativeGlArea | None:
        if self._preview_gl is not None:
            return self._preview_gl
        if not NativeGlLibrary.is_available():
            return None
        try:
            area = NativeGlArea(640, 260, 1.0, log=self.host.log, target_fps=20)
            area.widget.set_size_request(-1, 250)
            self._preview_gl = area
            self._preview_frame.pack_start(area.widget, True, True, 0)
            area.widget.show()
            return area
        except Exception:
            return None

    def _sync_preview(self) -> None:
        tid = self.host.settings.theme
        theme_meta = self.host.themes.find(tid) or {}
        page = str(theme_meta.get("page") or "")
        can_gl = NativeGlLibrary.is_available() and (is_native_gl_theme(tid) or is_native_gl_page(page))
        visible = self.window.get_visible()

        if can_gl:
            area = self._ensure_preview_gl()
            if area is not None:
                self._preview_fallback_box.hide()
                area.widget.show()
                area.set_low_mode(bool(self.host.settings.low_mode))
                area.post_json({"t": "lowMode", "v": bool(self.host.settings.low_mode)})
                area.post_json({"t": "theme", "id": tid})
                area.post_json({"t": "settings", "v": dict(self.host.settings.for_theme(tid))})
                area.post_json(self._scaled_preview_icons())
                area.set_paused(not visible)
                return

        if self._preview_gl is not None:
            self._preview_gl.set_paused(True)
            self._preview_gl.widget.hide()
        name = self._i18n.theme_name(tid, tid)
        self._preview_fallback_lbl.set_markup(
            f"<b>{self._GLib.markup_escape_text(name)}</b>\n"
            f"<small>HTML / CSS / JS · WebKit / WebEngine</small>"
        )
        self._preview_fallback_box.show_all()

    def _scaled_preview_icons(self) -> dict[str, Any]:
        src = self.host.last_icons
        if not isinstance(src, dict) or not src.get("vis") or not src.get("r"):
            return {"t": "icons", "vis": False, "r": []}
        wp = getattr(self.host, "wallpaper", None)
        sw = max(1, int(getattr(wp, "screen_w", 1920)))
        sh = max(1, int(getattr(wp, "screen_h", 1080)))
        pw = max(64, int(getattr(self._preview_gl, "_width_px", 640)))
        ph = max(64, int(getattr(self._preview_gl, "_height_px", 250)))
        sx = pw / sw
        sy = ph / sh
        scaled = []
        names: list[str] = []
        raw_names = src.get("n") if isinstance(src.get("n"), list) else []
        for idx, r in enumerate(src.get("r") or []):
            if isinstance(r, (list, tuple)) and len(r) >= 4:
                scaled.append([
                    int(round(r[0] * sx)),
                    int(round(r[1] * sy)),
                    int(round(r[2] * sx)),
                    int(round(r[3] * sy)),
                ])
                names.append(str(raw_names[idx]) if idx < len(raw_names) else "")
        out: dict[str, Any] = {"t": "icons", "vis": len(scaled) > 0, "r": scaled}
        if any(names):
            out["n"] = names
        return out

    # ------------------------------------------------------------------ Theme controls
    def _rebuild_theme_controls(self) -> None:
        Gtk = self._Gtk
        Gdk = self._Gdk
        tid = self.host.settings.theme
        theme = self.host.themes.find(tid) or {}
        is_custom = bool(theme.get("custom"))
        self._remove_theme_btn.set_visible(is_custom)

        name = self._i18n.theme_name(tid, str(theme.get("name") or tid))
        desc = self._i18n.theme_desc(tid, str(theme.get("desc") or ""))
        self._theme_title_lbl.set_markup(f"<b>{self._GLib.markup_escape_text(name)}</b>")

        for child in list(self._params_box.get_children()):
            self._params_box.remove(child)

        if desc:
            dlbl = Gtk.Label(label=desc)
            dlbl.set_xalign(0.0)
            dlbl.set_line_wrap(True)
            dlbl.get_style_context().add_class("dd-dim")
            self._params_box.pack_start(dlbl, False, False, 0)

        saved = self.host.settings.for_theme(tid)

        # Adaptive switch
        if theme.get("adaptive"):
            card = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
            card.get_style_context().add_class("dd-card")
            lbl = Gtk.Label(label=self._i18n.ui("adaptive", "React to desktop icons"))
            lbl.set_xalign(0.0)
            sw = Gtk.Switch()
            sw.set_valign(Gtk.Align.CENTER)
            sw.set_active(bool(saved.get("adaptive", 1)))
            sw.connect(
                "notify::active",
                lambda s, *_: self._set_param(tid, "adaptive", 1 if s.get_active() else 0),
            )
            card.pack_start(lbl, True, True, 0)
            card.pack_end(sw, False, False, 0)
            self._params_box.pack_start(card, False, False, 0)

        # Parameter groups
        groups = theme.get("groups")
        if isinstance(groups, list):
            for g_idx, group in enumerate(groups):
                if not isinstance(group, dict):
                    continue
                params = group.get("params") or []
                if not params:
                    continue
                g_card = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
                g_card.get_style_context().add_class("dd-card")

                g_title = self._i18n.group_name(tid, g_idx, str(group.get("name") or ""))
                if g_title:
                    glbl = Gtk.Label(label=g_title)
                    glbl.set_xalign(0.0)
                    glbl.get_style_context().add_class("dd-group-title")
                    g_card.pack_start(glbl, False, False, 0)

                for param in params:
                    if not isinstance(param, dict) or "key" not in param:
                        continue
                    row = self._build_param_widget(tid, param, saved)
                    if row is not None:
                        g_card.pack_start(row, False, False, 0)

                self._params_box.pack_start(g_card, False, False, 0)

        # Randomize seed shortcut for procedural themes
        if "seed" not in [p.get("key") for g in (groups or []) if isinstance(g, dict) for p in (g.get("params") or []) if isinstance(p, dict)]:
            seed_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
            seed_row.get_style_context().add_class("dd-card")
            slbl = Gtk.Label(label=self._i18n.ui("randomize", "New variation (seed)"))
            slbl.set_xalign(0.0)
            sbtn = Gtk.Button(label="🎲")
            sbtn.connect("clicked", lambda *_: self._set_param(tid, "seed", random.randint(1, 999999)))
            seed_row.pack_start(slbl, True, True, 0)
            seed_row.pack_end(sbtn, False, False, 0)
            self._params_box.pack_start(seed_row, False, False, 0)

        self._params_box.show_all()

    def _build_param_widget(self, tid: str, param: dict[str, Any], saved: dict[str, Any]) -> Any:
        Gtk = self._Gtk
        Gdk = self._Gdk
        key = str(param["key"])
        label = self._i18n.param_label(tid, key, str(param.get("label") or key))
        hint = self._i18n.param_hint(tid, key, str(param.get("hint") or ""))
        cur_val = saved.get(key, param.get("default"))

        # 1. Audio control
        if param.get("type") == "audio":
            row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
            lbl = Gtk.Label(label=label)
            lbl.set_xalign(0.0)
            if hint:
                lbl.set_tooltip_text(hint)
            play_btn = Gtk.ToggleButton(label="▶ / ❚❚")
            play_btn.set_active(bool(saved.get("audioPlay", 0)))
            play_btn.connect(
                "toggled",
                lambda b: self._set_param(tid, "audioPlay", 1 if b.get_active() else 0),
            )
            pick_btn = Gtk.Button(label=self._i18n.ui("audioPick", "Choose…"))
            pick_btn.connect(
                "clicked",
                lambda *_: self.host.on_settings_message(json.dumps({"t": "pickAudio", "theme": tid})),
            )
            row.pack_start(lbl, True, True, 0)
            row.pack_end(pick_btn, False, False, 0)
            row.pack_end(play_btn, False, False, 0)
            return row

        # 2. Color picker
        if param.get("type") == "color":
            row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
            lbl = Gtk.Label(label=label)
            lbl.set_xalign(0.0)
            if hint:
                lbl.set_tooltip_text(hint)
            rgba = Gdk.RGBA()
            rgba.parse(str(cur_val or "#ffffff"))
            cbtn = Gtk.ColorButton.new_with_rgba(rgba)

            def _on_color(btn):
                c = btn.get_rgba()
                hex_s = f"#{int(round(c.red*255)):02x}{int(round(c.green*255)):02x}{int(round(c.blue*255)):02x}"
                self._set_param(tid, key, hex_s)

            cbtn.connect("color-set", _on_color)
            row.pack_start(lbl, True, True, 0)
            row.pack_end(cbtn, False, False, 0)
            return row

        # 3. Options dropdown
        if isinstance(param.get("options"), list):
            row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
            lbl = Gtk.Label(label=label)
            lbl.set_xalign(0.0)
            if hint:
                lbl.set_tooltip_text(hint)
            opts = self._i18n.param_options(tid, key, [str(x) for x in param["options"]])
            values = param.get("values") if isinstance(param.get("values"), list) else None
            combo = Gtk.ComboBoxText()
            active_idx = 0
            for idx, opt_lbl in enumerate(opts):
                combo.append_text(opt_lbl)
                if values is not None:
                    if idx < len(values) and str(values[idx]) == str(cur_val):
                        active_idx = idx
                else:
                    try:
                        if int(cur_val) == idx:
                            active_idx = idx
                    except (TypeError, ValueError):
                        pass
            combo.set_active(active_idx)

            def _on_opt(c, _vals=values, _k=key):
                idx = c.get_active()
                if idx < 0:
                    return
                val = _vals[idx] if (_vals is not None and idx < len(_vals)) else idx
                self._set_param(tid, _k, val)

            combo.connect("changed", _on_opt)
            row.pack_start(lbl, True, True, 0)
            row.pack_end(combo, False, False, 0)
            return row

        # 4. Binary toggle
        if is_toggle_param(param):
            row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
            lbl = Gtk.Label(label=label)
            lbl.set_xalign(0.0)
            if hint:
                lbl.set_tooltip_text(hint)
            sw = Gtk.Switch()
            sw.set_valign(Gtk.Align.CENTER)
            try:
                sw.set_active(bool(int(cur_val)))
            except (TypeError, ValueError):
                sw.set_active(bool(cur_val))
            sw.connect("notify::active", lambda s, *_, _k=key: self._set_param(tid, _k, 1 if s.get_active() else 0))
            row.pack_start(lbl, True, True, 0)
            row.pack_end(sw, False, False, 0)
            return row

        # 5. Numeric slider
        if "min" in param and "max" in param:
            vbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
            top = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
            lbl = Gtk.Label(label=label)
            lbl.set_xalign(0.0)
            if hint:
                lbl.set_tooltip_text(hint)
            lo = float(param["min"])
            hi = float(param["max"])
            step = float(param.get("step", 1) or 1)
            try:
                num = float(cur_val if cur_val is not None else lo)
            except (TypeError, ValueError):
                num = lo
            num = max(lo, min(hi, num))
            val_lbl = Gtk.Label(label=format_param_value(param, num))
            val_lbl.get_style_context().add_class("dd-dim")
            top.pack_start(lbl, True, True, 0)
            top.pack_end(val_lbl, False, False, 0)

            scale = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, lo, hi, step)
            scale.set_draw_value(False)
            scale.set_value(num)

            def _on_slide(sc, _k=key, _p=param, _vl=val_lbl, _st=step):
                raw = sc.get_value()
                snapped = round(raw / _st) * _st
                val: Any = int(round(snapped)) if abs(_st - round(_st)) < 1e-6 else round(snapped, 4)
                _vl.set_text(format_param_value(_p, val))
                self._set_param(tid, _k, val)

            scale.connect("value-changed", _on_slide)
            vbox.pack_start(top, False, False, 0)
            vbox.pack_start(scale, False, False, 0)
            return vbox

        return None

    def _set_param(self, theme_id: str, key: str, value: Any) -> None:
        self.host.on_settings_message(
            json.dumps({"t": "set", "theme": theme_id, "key": key, "value": value})
        )
        if self._preview_gl is not None and theme_id == self.host.settings.theme:
            self._preview_gl.post_json({"t": "settings", "v": {key: value}})

    def _on_reset_clicked(self, _btn: Any) -> None:
        tid = self.host.settings.theme
        self.host.on_settings_message(json.dumps({"t": "reset", "theme": tid}))
        self._rebuild_theme_controls()
        if self._preview_gl is not None:
            self._preview_gl.post_json({"t": "reset"})
            self._preview_gl.post_json({"t": "settings", "v": dict(self.host.settings.for_theme(tid))})

    # ------------------------------------------------------------------ System events
    def _on_rust_gl_toggled(self, sw: Any, _pspec: Any) -> None:
        if self._syncing:
            return
        self.host.set_rust_opengl(bool(sw.get_active()))

    def _on_lang_changed(self, combo: Any) -> None:
        if self._syncing:
            return
        code = combo.get_active_id() or "auto"
        self.host.on_settings_message(json.dumps({"t": "lang", "v": code}))

    def _sync_system_controls(self) -> None:
        s = self.host.settings
        self._syncing = True
        try:
            self._hdr_gl_switch.set_active(bool(s.rust_opengl))
            self._sys_gl_switch.set_active(bool(s.rust_opengl))
            self._sys_gl_title.set_markup(
                f"<b>{self._GLib.markup_escape_text(self._i18n.ui('rustOpenGl', 'Rust OpenGL 3.3'))}</b>"
            )
            self._sys_gl_desc.set_text(
                self._i18n.ui(
                    "rustOpenGlBannerDesc",
                    "Runs built-in themes and settings natively on Rust OpenGL 3.3 + GTK3 with zero WebKit/WebEngine processes.",
                )
            )
            self._autostart_sw.set_active(bool(self.host.autostart_enabled()))
            self._low_mode_sw.set_active(bool(s.low_mode))
            self._no_pause_sw.set_active(bool(s.disable_auto_pause))
            self._solar_sw.set_active(bool(s.solar_sync))
            self._win_aware_sw.set_active(bool(s.window_aware))
            self._sys_bridge_sw.set_active(bool(s.system_bridge))
            self._mpv_sw.set_active(bool(s.prefer_mpv))
            self._display_mode_combo.set_active_id(s.display_mode)
            self._playlist_sw.set_active(bool(s.playlist_enabled))
            self._playlist_spin.set_value(int(s.playlist_interval_min))
            self._sync_virtual_icons_ui()
        finally:
            self._syncing = False

    def _on_rename_vicons_clicked(self, _btn: Any) -> None:
        Gtk = self._Gtk
        icons = [dict(it) for it in (self.host.settings.virtual_icons or [])]
        if not icons:
            return
        is_tr = self._i18n.code == "tr"
        dlg = Gtk.Dialog(
            title="Sanal İkon İsimlerini Düzenle" if is_tr else "Rename Virtual Icons",
            transient_for=self.window,
            modal=True,
        )
        dlg.set_default_size(480, min(520, max(240, 120 + len(icons) * 44)))
        dlg.add_button("İptal" if is_tr else "Cancel", Gtk.ResponseType.CANCEL)
        ok_btn = dlg.add_button("Kaydet" if is_tr else "Save", Gtk.ResponseType.OK)
        ok_btn.get_style_context().add_class("suggested-action")
        dlg.set_default_response(Gtk.ResponseType.OK)

        area = dlg.get_content_area()
        area.set_spacing(10)
        area.set_margin_top(12)
        area.set_margin_bottom(12)
        area.set_margin_start(14)
        area.set_margin_end(14)

        hint = Gtk.Label(
            label=(
                "Masaüstündeki sanal ikonlara sağ tıklayarak da isim değiştirebilirsiniz. "
                "Cephe Kuşatması teması ikon diline (TR / DE / EN) göre bayrak seçer."
                if is_tr
                else "Tip: You can also right-click any virtual icon on the desktop to rename it. "
                "Frontline Pocket selects flags based on icon language (TR / DE / EN)."
            )
        )
        hint.set_xalign(0.0)
        hint.set_line_wrap(True)
        hint.get_style_context().add_class("dd-dim")
        area.pack_start(hint, False, False, 0)

        scroll = Gtk.ScrolledWindow()
        scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        rows_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        rows_box.set_margin_end(4)

        editors: list[tuple[int, Any, Any, str]] = []
        for idx, it in enumerate(icons):
            row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
            idx_lbl = Gtk.Label(label=f"#{idx}")
            idx_lbl.set_width_chars(3)
            idx_lbl.set_xalign(1.0)
            idx_lbl.get_style_context().add_class("dd-dim")

            entry = Gtk.Entry()
            entry.set_max_length(48)
            entry.set_text(str(it.get("name") or ""))
            entry.set_activates_default(True)

            cur_type = str(it.get("type") or "folder")
            combo = Gtk.ComboBoxText()
            for tid, lbl_tr, lbl_en in VIRTUAL_ICON_TYPE_LABELS:
                combo.append(tid, lbl_tr if is_tr else lbl_en)
            combo.set_active_id(cur_type if cur_type in VIRTUAL_ICON_TYPES else "folder")

            row.pack_start(idx_lbl, False, False, 0)
            row.pack_start(entry, True, True, 0)
            row.pack_end(combo, False, False, 0)
            rows_box.pack_start(row, False, False, 0)
            editors.append((idx, entry, combo, cur_type))

        scroll.add(rows_box)
        area.pack_start(scroll, True, True, 0)
        dlg.show_all()

        resp = dlg.run()
        updated = [dict(it) for it in icons]
        if resp == Gtk.ResponseType.OK:
            for idx, entry, combo, orig_type in editors:
                new_name = entry.get_text().strip()[:48]
                new_type = combo.get_active_id() or orig_type
                if new_name:
                    updated[idx]["name"] = new_name
                if new_type != orig_type and new_type in VIRTUAL_ICON_TYPES:
                    updated[idx]["type"] = new_type
                elif new_name and "." in new_name:
                    updated[idx]["type"] = infer_icon_type(new_name, orig_type)
        dlg.destroy()

        if resp == Gtk.ResponseType.OK:
            self.host.update_virtual_icons(updated)

    def _sync_virtual_icons_ui(self) -> None:
        s = self.host.settings
        count = len(s.virtual_icons)
        is_tr = self._i18n.code == "tr"
        title = "Sanal İkonlar" if is_tr else "Virtual Icons"
        self._vicons_title_lbl.set_markup(f"<b>{self._GLib.markup_escape_text(title)}</b>")
        self._vicons_count_lbl.set_text(f"{count} {'ikon' if is_tr else 'icons'}")
        self._vicons_add_btn.set_label("+ İkon" if is_tr else "+ Icon")
        self._vicons_rename_btn.set_label("✎ İsimler" if is_tr else "✎ Names")
        self._vicons_shuffle_btn.set_label("Karıştır" if is_tr else "Shuffle")
        self._vicons_clear_btn.set_label("Temizle" if is_tr else "Clear")
        self._vicons_rename_btn.set_sensitive(count > 0)
        self._vicons_shuffle_btn.set_sensitive(count > 0)
        self._vicons_clear_btn.set_sensitive(count > 0)
        prev_sync = self._syncing
        self._syncing = True
        try:
            self._vicons_mode_combo.set_active_id(s.virtual_bar_mode)
        finally:
            self._syncing = prev_sync

    def _retranslate(self) -> None:
        self._i18n.set_code(self.host.i18n.code)
        self._import_btn.set_label("+ " + self._i18n.ui("importTheme", "Import theme"))
        self._hdr_gl_lbl.set_text(self._i18n.ui("rustOpenGl", "Rust OpenGL"))
        self._search_entry.set_placeholder_text(self._i18n.ui("themes", "Themes") + "…")
        self._remove_theme_btn.set_label(self._i18n.ui("removeTheme", "Remove theme"))
        self._reset_btn.set_label(self._i18n.ui("reset", "Reset"))
        self._snap_btn.set_label(self._i18n.ui("shareImage", "Snapshot"))
        self._lockscreen_btn.set_label(self._i18n.ui("syncLockscreen", "Lockscreen"))
        self._tab_params_lbl.set_text(self._i18n.ui("customize", "Customize"))
        self._tab_sys_lbl.set_text(self._i18n.ui("settings", "System"))
        self._autostart_lbl.set_text(self._i18n.ui("autostart", "Start on login"))
        self._low_mode_lbl.set_text(self._i18n.ui("lowMode", "Low-power mode"))
        self._no_pause_lbl.set_text(self._i18n.ui("disableAutoPause", "Keep running behind fullscreen windows"))
        self._solar_lbl.set_text(self._i18n.ui("solarSync", "Solar day/night sync"))
        self._win_aware_lbl.set_text(self._i18n.ui("windowAware", "Window-aware scenery"))
        self._sys_bridge_lbl.set_text(self._i18n.ui("systemBridge", "Local audio & system stats bridge"))
        self._mpv_lbl.set_text(self._i18n.ui("preferMpv", "Hardware mpv video backend (X11)"))
        self._display_mode_lbl.set_text(self._i18n.ui("displayMode", "Multi-monitor mode"))
        self._playlist_lbl.set_text(self._i18n.ui("playlist", "Theme playlist rotation"))
        self._diag_lbl.set_text(self._i18n.ui("diagnostics", "Diagnostics"))
        self._diag_btn.set_label(self._i18n.ui("exportDiagnostics", "Export log"))
        self._populate_lang_combo()
        self._populate_theme_list()
        self._select_theme_row(self.host.settings.theme)
        self._rebuild_theme_controls()
        self._sync_system_controls()

    def _show_status(self, text: str) -> None:
        self._status_lbl.set_text(text)
        self._status_lbl.show()

    # ------------------------------------------------------------------ Host protocol
    def post(self, payload: Any) -> None:
        if self._closed or not isinstance(payload, dict):
            return
        kind = payload.get("t")
        if kind == "icons":
            if self._preview_gl is not None:
                self._preview_gl.post_json(self._scaled_preview_icons())
        elif kind == "themeChanged":
            tid = str(payload.get("id") or self.host.settings.theme)
            self._select_theme_row(tid)
            self._rebuild_theme_controls()
            self._sync_preview()
        elif kind in ("themeImported", "themeRemoved"):
            self._populate_theme_list()
            self._select_theme_row(self.host.settings.theme)
            self._rebuild_theme_controls()
            self._sync_preview()
        elif kind == "importError":
            self._show_status(str(payload.get("msg") or "Import failed"))
        elif kind == "virtualState":
            self._sync_virtual_icons_ui()
            if self._preview_gl is not None:
                self._sync_preview()
        elif kind == "lang":
            self._retranslate()
        elif kind in (
            "rustOpenGl",
            "lowMode",
            "disableAutoPause",
            "displayMode",
            "solarSync",
            "windowAware",
            "playlistState",
            "systemBridge",
            "preferMpv",
            "autostart",
        ):
            self._sync_system_controls()
            if kind == "lowMode" and self._preview_gl is not None:
                self._preview_gl.set_low_mode(bool(payload.get("v")))
                self._preview_gl.post_json(payload)
        elif kind == "shareResult":
            if payload.get("ok"):
                self._show_status(f"Kaydedildi: {payload.get('path', '')}")
            else:
                self._show_status(str(payload.get("msg") or "Snapshot failed"))
        elif kind == "lockscreenSynced":
            if payload.get("ok"):
                self._show_status(f"Kilit ekranı güncellendi: {payload.get('path', '')}")
            else:
                self._show_status(str(payload.get("msg") or "Lockscreen sync failed"))
        elif kind == "diagExported":
            if payload.get("ok"):
                self._show_status(f"Tanılama günlüğü: {payload.get('path', '')}")
            else:
                self._show_status(str(payload.get("msg") or "Diagnostics failed"))

    # ------------------------------------------------------------------ Window lifecycle
    def _on_window_state(self, _window: Any, event: Any) -> bool:
        iconified = self._Gdk.WindowState.ICONIFIED
        if (event.changed_mask & iconified) != 0 and self._preview_gl is not None:
            is_min = bool(event.new_window_state & iconified)
            self._preview_gl.set_paused(is_min or not self.window.get_visible())
        return False

    def _on_delete(self, _window: Any, _event: Any) -> bool:
        self.hide()
        return True

    def show(self) -> None:
        if self._closed:
            return
        self._sync_system_controls()
        self.window.show_all()
        self._remove_theme_btn.set_visible(
            bool((self.host.themes.find(self.host.settings.theme) or {}).get("custom"))
        )
        self.window.present()
        self._sync_preview()

    def hide(self) -> None:
        if self._preview_gl is not None:
            self._preview_gl.set_paused(True)
        self.window.hide()

    def destroy(self) -> None:
        if self._closed:
            return
        self._closed = True
        if self._preview_gl is not None:
            try:
                self._preview_gl.destroy()
            except Exception:
                pass
            self._preview_gl = None
        try:
            self.window.destroy()
        except Exception:
            pass
