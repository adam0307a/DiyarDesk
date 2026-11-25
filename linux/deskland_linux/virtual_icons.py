"""Native virtual desktop icons for iconless Linux desktops (GNOME, Hyprland, Niri, Sway, etc.).

Provides:
- Pure-Python layout/mutation helpers (`add_virtual_icon`, `shuffle_virtual_icons`,
  `clear_virtual_icons`, `merge_icon_rects`) — 100% unit-testable without GTK.
- `VirtualIconsOverlay`: zero-WebKit GTK3 + Cairo overlay placed inside a `Gtk.Overlay`
  above `NativeGlArea` (`Gtk.GLArea`), offering draggable vector desktop icons and a
  collapsible floating pill bar (`+ İkon`, `Karıştır`, `Temizle`, `Gizle`) matching
  `prototype/wall-icons.js`.
"""

from __future__ import annotations

import math
import random
from typing import Any, Callable

DEFAULT_ICON_W = 76
DEFAULT_ICON_H = 86
MAX_VIRTUAL_ICONS = 64

VIRTUAL_ICON_TYPES: tuple[str, ...] = ("folder", "txt", "pdf", "xlsx", "png", "zip")

VIRTUAL_ICON_TYPE_LABELS: tuple[tuple[str, str, str], ...] = (
    ("folder", "Klasör", "Folder"),
    ("txt", "Metin (.txt)", "Text (.txt)"),
    ("pdf", "PDF (.pdf)", "PDF (.pdf)"),
    ("xlsx", "Tablo (.xlsx)", "Sheet (.xlsx)"),
    ("png", "Görsel (.png)", "Image (.png)"),
    ("zip", "Arşiv (.zip)", "Archive (.zip)"),
)

VIRTUAL_ICON_PRESETS: tuple[tuple[str, str], ...] = (
    ("Yeni Klasör", "folder"),
    ("Notlar.txt", "txt"),
    ("Belgeler", "folder"),
    ("Fatura.pdf", "pdf"),
    ("Tablo.xlsx", "xlsx"),
    ("Ekran Görüntüsü.png", "png"),
    ("Arşiv.zip", "zip"),
    ("Projeler", "folder"),
    ("Hatırlatma.txt", "txt"),
    ("Yedekler", "folder"),
)


def _clamp_pos(
    x: float,
    y: float,
    w: int,
    h: int,
    logical_w: int,
    logical_h: int,
    safe_insets: tuple[int, int, int, int] = (0, 0, 0, 0),
) -> tuple[int, int]:
    top, right, bottom, left = safe_insets
    min_x = max(14, int(left) + 14)
    min_y = max(14, int(top) + 14)
    max_x = max(min_x, int(logical_w) - int(w) - max(14, int(right) + 14))
    max_y = max(min_y, int(logical_h) - int(h) - max(76, int(bottom) + 76))
    cx = int(round(max(min_x, min(max_x, x))))
    cy = int(round(max(min_y, min(max_y, y))))
    return cx, cy


def _next_preset(existing: list[dict[str, Any]]) -> tuple[str, str]:
    used = {str(it.get("name") or "") for it in existing}
    base, icon_type = VIRTUAL_ICON_PRESETS[len(existing) % len(VIRTUAL_ICON_PRESETS)]
    if base not in used:
        return base, icon_type
    n = 2
    while f"{base} ({n})" in used:
        n += 1
    return f"{base} ({n})", icon_type


def _pick_free_position(
    existing: list[dict[str, Any]],
    w: int,
    h: int,
    logical_w: int,
    logical_h: int,
    safe_insets: tuple[int, int, int, int] = (0, 0, 0, 0),
    rng: random.Random | None = None,
) -> tuple[int, int]:
    r = rng or random
    top, right, bottom, left = safe_insets
    min_x = max(32, int(left) + 28)
    min_y = max(32, int(top) + 24)
    max_x = max(min_x + 1, int(logical_w) - w - max(32, int(right) + 28))
    max_y = max(min_y + 1, int(logical_h) - h - max(96, int(bottom) + 88))

    best_pos = _clamp_pos(min_x, min_y, w, h, logical_w, logical_h, safe_insets)
    best_min_dist = -1.0
    min_sep = max(w, h) * 1.18

    for _ in range(36):
        cx = r.uniform(min_x, max_x)
        cy = r.uniform(min_y, max_y)
        px, py = _clamp_pos(cx, cy, w, h, logical_w, logical_h, safe_insets)
        if not existing:
            return px, py
        nearest = min(
            math.hypot((px - float(it.get("x", 0))), (py - float(it.get("y", 0))))
            for it in existing
        )
        if nearest >= min_sep:
            return px, py
        if nearest > best_min_dist:
            best_min_dist = nearest
            best_pos = (px, py)
    return best_pos


def add_virtual_icon(
    icons: list[dict[str, Any]],
    logical_w: int,
    logical_h: int,
    safe_insets: tuple[int, int, int, int] = (0, 0, 0, 0),
    rng: random.Random | None = None,
) -> list[dict[str, Any]]:
    out = [dict(it) for it in icons[:MAX_VIRTUAL_ICONS]]
    if len(out) >= MAX_VIRTUAL_ICONS:
        return out
    name, icon_type = _next_preset(out)
    x, y = _pick_free_position(
        out, DEFAULT_ICON_W, DEFAULT_ICON_H, logical_w, logical_h, safe_insets, rng=rng
    )
    out.append(
        {
            "name": name,
            "type": icon_type,
            "x": x,
            "y": y,
            "w": DEFAULT_ICON_W,
            "h": DEFAULT_ICON_H,
        }
    )
    return out


def add_virtual_icons_batch(
    icons: list[dict[str, Any]],
    count: int,
    logical_w: int,
    logical_h: int,
    safe_insets: tuple[int, int, int, int] = (0, 0, 0, 0),
    rng: random.Random | None = None,
) -> list[dict[str, Any]]:
    cur = [dict(it) for it in icons[:MAX_VIRTUAL_ICONS]]
    for _ in range(max(0, min(MAX_VIRTUAL_ICONS, int(count)))):
        if len(cur) >= MAX_VIRTUAL_ICONS:
            break
        cur = add_virtual_icon(cur, logical_w, logical_h, safe_insets, rng=rng)
    return cur


def shuffle_virtual_icons(
    icons: list[dict[str, Any]],
    logical_w: int,
    logical_h: int,
    safe_insets: tuple[int, int, int, int] = (0, 0, 0, 0),
    rng: random.Random | None = None,
) -> list[dict[str, Any]]:
    placed: list[dict[str, Any]] = []
    for item in icons[:MAX_VIRTUAL_ICONS]:
        w = max(24, min(256, int(item.get("w", DEFAULT_ICON_W))))
        h = max(24, min(256, int(item.get("h", DEFAULT_ICON_H))))
        x, y = _pick_free_position(
            placed, w, h, logical_w, logical_h, safe_insets, rng=rng
        )
        updated = dict(item)
        updated["x"] = x
        updated["y"] = y
        updated["w"] = w
        updated["h"] = h
        placed.append(updated)
    return placed


def clear_virtual_icons() -> list[dict[str, Any]]:
    return []


def infer_icon_type(name: str, fallback: str = "folder") -> str:
    low = str(name or "").strip().lower()
    if low.endswith(".pdf"):
        return "pdf"
    if low.endswith((".xlsx", ".xls", ".csv", ".ods")):
        return "xlsx"
    if low.endswith((".png", ".jpg", ".jpeg", ".webp", ".gif", ".svg", ".bmp")):
        return "png"
    if low.endswith((".zip", ".tar", ".gz", ".xz", ".7z", ".rar", ".zst")):
        return "zip"
    if low.endswith((".txt", ".md", ".doc", ".docx", ".rtf", ".odt", ".log")):
        return "txt"
    return fallback if fallback in VIRTUAL_ICON_TYPES else "folder"


def rename_virtual_icon(
    icons: list[dict[str, Any]],
    index: int,
    new_name: str,
    icon_type: str | None = None,
) -> list[dict[str, Any]]:
    """Return a copy of `icons` with item `index` renamed (and optionally re-typed)."""
    out = [dict(it) for it in icons[:MAX_VIRTUAL_ICONS]]
    if not (0 <= int(index) < len(out)):
        return out
    cleaned = str(new_name or "").strip()[:48]
    item = out[int(index)]
    if cleaned:
        item["name"] = cleaned
    if icon_type is not None and icon_type in VIRTUAL_ICON_TYPES:
        item["type"] = icon_type
    elif cleaned and "." in cleaned:
        item["type"] = infer_icon_type(cleaned, str(item.get("type") or "folder"))
    return out


def virtual_icons_to_physical(
    icons: list[dict[str, Any]],
    scale: float,
    mode: str = "auto",
) -> tuple[list[list[int]], list[str]]:
    """Convert virtual icons (logical CSS px) into physical-pixel [l, t, r, b] rects."""
    if mode == "off" or not icons:
        return [], []
    sc = max(0.5, float(scale or 1.0))
    rects: list[list[int]] = []
    labels: list[str] = []
    for it in icons[:MAX_VIRTUAL_ICONS]:
        try:
            x = float(it.get("x", 0))
            y = float(it.get("y", 0))
            w = max(24.0, float(it.get("w", DEFAULT_ICON_W)))
            h = max(24.0, float(it.get("h", DEFAULT_ICON_H)))
        except (TypeError, ValueError):
            continue
        l = int(round(x * sc))
        t = int(round(y * sc))
        r = int(round((x + w) * sc))
        b = int(round((y + h) * sc))
        rects.append([l, t, r, b])
        labels.append(str(it.get("name") or ""))
    return rects, labels


def merge_host_and_virtual_icons(
    real_visible: bool,
    real_rects: list[list[int]],
    real_labels: list[str],
    virtual_icons: list[dict[str, Any]],
    scale: float,
    mode: str = "auto",
) -> tuple[bool, list[list[int]], list[str]]:
    """Merge real desktop icons with virtual icons in physical pixels."""
    base_rects = list(real_rects) if real_visible else []
    base_labels = list(real_labels) if real_visible and len(real_labels) == len(base_rects) else [""] * len(base_rects)
    v_rects, v_labels = virtual_icons_to_physical(virtual_icons, scale, mode=mode)
    merged_rects = base_rects + v_rects
    merged_labels = base_labels + v_labels
    return (len(merged_rects) > 0), merged_rects, merged_labels


# ---------------------------------------------------------------------------
# GTK3 + Cairo Desktop Virtual Icons Overlay (Zero WebKit)
# ---------------------------------------------------------------------------

_BAR_CSS = b"""
.dd-vicons-bar {
    background-color: rgba(14, 24, 34, 0.84);
    border: 1px solid rgba(180, 225, 255, 0.24);
    border-radius: 10px;
    padding: 5px 9px;
    color: #eaf4fb;
}
.dd-vicons-title {
    color: #eaf4fb;
    font-weight: 700;
    font-size: 12px;
}
.dd-vicons-btn {
    background-image: none;
    background-color: rgba(255, 255, 255, 0.11);
    color: #eef6fc;
    border: 1px solid rgba(255, 255, 255, 0.20);
    border-radius: 6px;
    padding: 3px 8px;
    min-height: 22px;
    min-width: 22px;
    font-size: 11px;
}
.dd-vicons-btn:hover {
    background-color: rgba(130, 210, 255, 0.26);
}
.dd-vicons-btn:checked {
    background-color: rgba(130, 210, 255, 0.34);
    border-color: rgba(130, 210, 255, 0.58);
}
"""


def _rounded_rect(cr: Any, x: float, y: float, w: float, h: float, r: float) -> None:
    r = min(r, w * 0.5, h * 0.5)
    cr.new_sub_path()
    cr.arc(x + w - r, y + r, r, -math.pi / 2, 0)
    cr.arc(x + w - r, y + h - r, r, 0, math.pi / 2)
    cr.arc(x + r, y + h - r, r, math.pi / 2, math.pi)
    cr.arc(x + r, y + r, r, math.pi, 3 * math.pi / 2)
    cr.close_path()


def _draw_icon_glyph(cr: Any, icon_type: str, x: float, y: float, size: float = 46.0) -> None:
    s = size / 48.0
    cr.save()
    cr.translate(x, y)
    cr.scale(s, s)

    # Soft drop shadow
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.34)
    _rounded_rect(cr, 7, 13, 36, 29, 5)
    cr.fill()

    if icon_type == "folder":
        cr.set_source_rgb(0.85, 0.57, 0.13)
        _rounded_rect(cr, 5, 13, 38, 26, 4)
        cr.fill()
        cr.set_source_rgb(0.91, 0.65, 0.19)
        cr.move_to(5, 15)
        cr.line_to(5, 11)
        cr.line_to(18, 11)
        cr.line_to(21, 15)
        cr.line_to(43, 15)
        cr.line_to(43, 18)
        cr.line_to(5, 18)
        cr.close_path()
        cr.fill()
        cr.set_source_rgb(1.0, 0.79, 0.30)
        _rounded_rect(cr, 5, 17, 38, 22, 4)
        cr.fill()
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.35)
        _rounded_rect(cr, 8, 19, 32, 2, 1)
        cr.fill()
    elif icon_type == "png":
        cr.set_source_rgb(0.97, 0.98, 0.99)
        _rounded_rect(cr, 6, 8, 36, 32, 4)
        cr.fill()
        cr.set_source_rgb(0.54, 0.82, 0.95)
        _rounded_rect(cr, 9, 11, 30, 24, 2)
        cr.fill()
        cr.set_source_rgb(1.0, 0.82, 0.28)
        cr.arc(30, 18, 3.4, 0, math.tau)
        cr.fill()
        cr.set_source_rgb(0.27, 0.66, 0.35)
        cr.move_to(9, 35)
        cr.line_to(18, 23)
        cr.line_to(25, 32)
        cr.line_to(30, 26)
        cr.line_to(39, 35)
        cr.close_path()
        cr.fill()
    elif icon_type == "zip":
        cr.set_source_rgb(0.48, 0.31, 0.78)
        _rounded_rect(cr, 10, 6, 28, 36, 4)
        cr.fill()
        cr.set_source_rgb(0.96, 0.93, 1.0)
        for zy in (8, 14, 20):
            cr.rectangle(22, zy, 4, 4)
        cr.fill()
        cr.set_source_rgb(1.0, 0.82, 0.28)
        _rounded_rect(cr, 20, 26, 8, 8, 2)
        cr.fill()
    else:
        # Document base (txt, pdf, xlsx)
        if icon_type == "pdf":
            cr.set_source_rgb(1.0, 0.96, 0.96)
        elif icon_type == "xlsx":
            cr.set_source_rgb(0.96, 0.99, 0.97)
        else:
            cr.set_source_rgb(0.97, 0.98, 0.99)
        cr.move_to(10, 5)
        cr.line_to(30, 5)
        cr.line_to(38, 13)
        cr.line_to(38, 43)
        cr.line_to(10, 43)
        cr.close_path()
        cr.fill()
        # Folded corner
        cr.set_source_rgba(0.62, 0.68, 0.76, 0.55)
        cr.move_to(30, 5)
        cr.line_to(30, 13)
        cr.line_to(38, 13)
        cr.close_path()
        cr.fill()
        if icon_type == "pdf":
            cr.set_source_rgb(0.85, 0.22, 0.17)
            _rounded_rect(cr, 8, 21, 26, 12, 3)
            cr.fill()
            cr.set_source_rgb(1.0, 1.0, 1.0)
            cr.select_font_face("Sans", 0, 1)
            cr.set_font_size(8.5)
            cr.move_to(12, 30)
            cr.show_text("PDF")
        elif icon_type == "xlsx":
            cr.set_source_rgb(0.12, 0.49, 0.27)
            _rounded_rect(cr, 8, 21, 26, 12, 3)
            cr.fill()
            cr.set_source_rgb(1.0, 1.0, 1.0)
            cr.select_font_face("Sans", 0, 1)
            cr.set_font_size(8.0)
            cr.move_to(11, 30)
            cr.show_text("XLS")
        else:
            cr.set_source_rgb(0.49, 0.55, 0.63)
            cr.set_line_width(2.2)
            for ly, lw in ((20, 18), (26, 18), (32, 12)):
                cr.move_to(15, ly)
                cr.line_to(15 + lw, ly)
            cr.stroke()

    cr.restore()


class VirtualIconsOverlay:
    """Native GTK3 overlay rendering draggable virtual icons + floating control pill over `NativeGlArea`."""

    def __init__(
        self,
        gl_widget: Any,
        *,
        logical_w: int,
        logical_h: int,
        safe_insets: tuple[int, int, int, int] = (0, 0, 0, 0),
        on_icons_changed: Callable[[list[dict[str, Any]], bool], None],
        on_collapsed_changed: Callable[[bool], None],
        on_pass_mouse: Callable[[int, float, float], None] | None = None,
        on_ui_rects_changed: Callable[[list[tuple[int, int, int, int]]], None] | None = None,
    ) -> None:
        import gi

        gi.require_version("Gtk", "3.0")
        gi.require_version("Gdk", "3.0")
        from gi.repository import Gdk, Gtk

        self._Gtk = Gtk
        self._Gdk = Gdk
        self._logical_w = max(320, int(logical_w))
        self._logical_h = max(240, int(logical_h))
        self._safe_insets = safe_insets
        self._on_icons_changed = on_icons_changed
        self._on_collapsed_changed = on_collapsed_changed
        self._on_pass_mouse = on_pass_mouse
        self._on_ui_rects_changed = on_ui_rects_changed

        self._icons: list[dict[str, Any]] = []
        self._mode: str = "auto"
        self._user_collapsed: bool | None = None
        self._has_real_icons: bool = False
        self._ui_hidden: bool = False
        self._drag_index: int = -1
        self._drag_ox: float = 0.0
        self._drag_oy: float = 0.0
        self._drag_moved: bool = False
        self._hover_index: int = -1

        provider = Gtk.CssProvider()
        try:
            provider.load_from_data(_BAR_CSS)
            screen = Gdk.Screen.get_default()
            if screen is not None:
                Gtk.StyleContext.add_provider_for_screen(
                    screen, provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
                )
        except Exception:
            pass

        self.overlay = Gtk.Overlay()
        self.overlay.add(gl_widget)

        # Transparent Cairo drawing area for virtual icons + pointer handling
        canvas = Gtk.DrawingArea()
        canvas.set_app_paintable(True)
        canvas.set_hexpand(True)
        canvas.set_vexpand(True)
        canvas.add_events(
            Gdk.EventMask.POINTER_MOTION_MASK
            | Gdk.EventMask.BUTTON_PRESS_MASK
            | Gdk.EventMask.BUTTON_RELEASE_MASK
            | Gdk.EventMask.LEAVE_NOTIFY_MASK
        )
        canvas.connect("draw", self._on_draw)
        canvas.connect("button-press-event", self._on_button_press)
        canvas.connect("motion-notify-event", self._on_motion)
        canvas.connect("button-release-event", self._on_button_release)
        canvas.connect("leave-notify-event", self._on_leave)
        self._canvas = canvas
        self.overlay.add_overlay(canvas)

        # Bottom-right collapsible floating control pill
        bar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        bar.set_halign(Gtk.Align.END)
        bar.set_valign(Gtk.Align.END)
        top, right, bottom, _left = safe_insets
        bar.set_margin_end(max(14, int(right) + 14))
        bar.set_margin_bottom(max(22, int(bottom) + 22))
        bar.get_style_context().add_class("dd-vicons-bar")

        self._btn_collapse = Gtk.Button(label="‹")
        self._btn_collapse.get_style_context().add_class("dd-vicons-btn")
        self._btn_collapse.connect("clicked", self._on_toggle_collapse)

        self._lbl_title = Gtk.Label(label="Sanal İkonlar")
        self._lbl_title.get_style_context().add_class("dd-vicons-title")

        self._btn_add = Gtk.Button(label="+ İkon")
        self._btn_add.get_style_context().add_class("dd-vicons-btn")
        self._btn_add.connect("clicked", self._on_add_clicked)

        self._btn_shuffle = Gtk.Button(label="Karıştır")
        self._btn_shuffle.get_style_context().add_class("dd-vicons-btn")
        self._btn_shuffle.connect("clicked", self._on_shuffle_clicked)

        self._btn_clear = Gtk.Button(label="Temizle")
        self._btn_clear.get_style_context().add_class("dd-vicons-btn")
        self._btn_clear.connect("clicked", self._on_clear_clicked)

        self._btn_hide = Gtk.ToggleButton(label="Gizle")
        self._btn_hide.get_style_context().add_class("dd-vicons-btn")
        self._btn_hide.connect("toggled", self._on_hide_toggled)

        for w in (
            self._btn_collapse,
            self._lbl_title,
            self._btn_add,
            self._btn_shuffle,
            self._btn_clear,
            self._btn_hide,
        ):
            bar.pack_start(w, False, False, 0)

        self._bar = bar
        self._bar_items = (
            self._btn_add,
            self._btn_shuffle,
            self._btn_clear,
            self._btn_hide,
        )
        self.overlay.add_overlay(bar)
        self.overlay.set_overlay_pass_through(bar, False)
        self._sync_ui()

    @property
    def widget(self) -> Any:
        return self.overlay

    def set_state(
        self,
        icons: list[dict[str, Any]],
        *,
        mode: str = "auto",
        collapsed: bool | None = None,
        has_real_icons: bool | None = None,
    ) -> None:
        self._icons = [dict(it) for it in icons[:MAX_VIRTUAL_ICONS]]
        if mode in ("auto", "always", "off"):
            self._mode = mode
        self._user_collapsed = collapsed
        if has_real_icons is not None:
            self._has_real_icons = bool(has_real_icons)
        self._sync_ui()

    def set_has_real_icons(self, has_real: bool) -> None:
        val = bool(has_real)
        if val != self._has_real_icons:
            self._has_real_icons = val
            self._sync_ui()

    def _effective_collapsed(self) -> bool:
        if self._user_collapsed is not None:
            return bool(self._user_collapsed)
        return self._has_real_icons and len(self._icons) == 0

    def _sync_ui(self) -> None:
        enabled = self._mode != "off"
        self._bar.set_visible(enabled)
        if enabled:
            collapsed = self._effective_collapsed()
            self._btn_collapse.set_label("›" if collapsed else "‹")
            for w in self._bar_items:
                w.set_visible(not collapsed)
        self._canvas.queue_draw()
        self._emit_ui_rects()

    def _emit_ui_rects(self) -> None:
        if self._on_ui_rects_changed is None:
            return
        rects: list[tuple[int, int, int, int]] = []
        if self._mode != "off":
            alloc = self._bar.get_allocation()
            if alloc.width > 1 and alloc.height > 1:
                rects.append((alloc.x, alloc.y, alloc.width, alloc.height))
            if not self._ui_hidden:
                for it in self._icons:
                    rects.append(
                        (
                            int(it.get("x", 0)),
                            int(it.get("y", 0)),
                            int(it.get("w", DEFAULT_ICON_W)),
                            int(it.get("h", DEFAULT_ICON_H)),
                        )
                    )
        self._on_ui_rects_changed(rects)

    # ------------------------------------------------------------------ bar actions
    def _on_toggle_collapse(self, _btn: Any) -> None:
        nxt = not self._effective_collapsed()
        self._user_collapsed = nxt
        self._sync_ui()
        self._on_collapsed_changed(nxt)

    def _on_add_clicked(self, _btn: Any) -> None:
        self._ui_hidden = False
        self._btn_hide.set_active(False)
        self._icons = add_virtual_icon(
            self._icons, self._logical_w, self._logical_h, self._safe_insets
        )
        self._sync_ui()
        self._on_icons_changed([dict(i) for i in self._icons], True)

    def _on_shuffle_clicked(self, _btn: Any) -> None:
        if not self._icons:
            return
        self._ui_hidden = False
        self._btn_hide.set_active(False)
        self._icons = shuffle_virtual_icons(
            self._icons, self._logical_w, self._logical_h, self._safe_insets
        )
        self._sync_ui()
        self._on_icons_changed([dict(i) for i in self._icons], True)

    def _on_clear_clicked(self, _btn: Any) -> None:
        self._icons = clear_virtual_icons()
        self._ui_hidden = False
        self._btn_hide.set_active(False)
        self._sync_ui()
        self._on_icons_changed([], True)

    def _on_hide_toggled(self, btn: Any) -> None:
        self._ui_hidden = bool(btn.get_active())
        self._canvas.queue_draw()
        self._emit_ui_rects()

    # ------------------------------------------------------------------ drawing & hit test
    def _hit_icon(self, px: float, py: float) -> int:
        if self._mode == "off" or self._ui_hidden:
            return -1
        for idx in range(len(self._icons) - 1, -1, -1):
            it = self._icons[idx]
            x = float(it.get("x", 0))
            y = float(it.get("y", 0))
            w = float(it.get("w", DEFAULT_ICON_W))
            h = float(it.get("h", DEFAULT_ICON_H))
            if x <= px <= x + w and y <= py <= y + h:
                return idx
        return -1

    def _on_draw(self, _widget: Any, cr: Any) -> bool:
        if self._mode == "off" or self._ui_hidden or not self._icons:
            return False

        cr.select_font_face("Sans", 0, 0)
        cr.set_font_size(11.0)
        for idx, it in enumerate(self._icons):
            x = float(it.get("x", 0))
            y = float(it.get("y", 0))
            w = float(it.get("w", DEFAULT_ICON_W))
            h = float(it.get("h", DEFAULT_ICON_H))
            if idx == self._hover_index or idx == self._drag_index:
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.16)
                _rounded_rect(cr, x, y, w, h, 10.0)
                cr.fill()
            glyph_x = x + (w - 46.0) * 0.5
            glyph_y = y + 6.0
            _draw_icon_glyph(cr, str(it.get("type") or "folder"), glyph_x, glyph_y, 46.0)

            label = str(it.get("name") or "")
            if len(label) > 13:
                label = label[:12] + "…"
            ext = cr.text_extents(label)
            tx = x + max(2.0, (w - ext.width) * 0.5)
            ty = y + h - 10.0
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.86)
            cr.move_to(tx + 1.0, ty + 1.0)
            cr.show_text(label)
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.98)
            cr.move_to(tx, ty)
            cr.show_text(label)
        return False

    def _open_rename_dialog(self, index: int) -> None:
        if not (0 <= index < len(self._icons)):
            return
        Gtk = self._Gtk
        item = self._icons[index]
        cur_name = str(item.get("name") or "")
        cur_type = str(item.get("type") or "folder")

        dlg = Gtk.Dialog(title="Sanal İkonu Yeniden Adlandır")
        dlg.set_default_size(360, -1)
        dlg.set_resizable(False)
        dlg.set_position(Gtk.WindowPosition.CENTER)
        dlg.add_button("İptal", Gtk.ResponseType.CANCEL)
        ok_btn = dlg.add_button("Kaydet", Gtk.ResponseType.OK)
        ok_btn.get_style_context().add_class("suggested-action")
        dlg.set_default_response(Gtk.ResponseType.OK)

        area = dlg.get_content_area()
        area.set_spacing(10)
        area.set_margin_top(12)
        area.set_margin_bottom(12)
        area.set_margin_start(14)
        area.set_margin_end(14)

        lbl_name = Gtk.Label(label="İkon adı:")
        lbl_name.set_xalign(0.0)
        entry = Gtk.Entry()
        entry.set_max_length(48)
        entry.set_text(cur_name)
        entry.set_activates_default(True)
        entry.select_region(0, -1)

        lbl_type = Gtk.Label(label="Simge türü:")
        lbl_type.set_xalign(0.0)
        combo = Gtk.ComboBoxText()
        for tid, label_tr, _label_en in VIRTUAL_ICON_TYPE_LABELS:
            combo.append(tid, label_tr)
        combo.set_active_id(cur_type if cur_type in VIRTUAL_ICON_TYPES else "folder")

        hint = Gtk.Label(
            label="İpucu: Cephe Kuşatması teması ikon diline göre (TR / DE / EN) bayrak seçer."
        )
        hint.set_xalign(0.0)
        hint.set_line_wrap(True)
        hint.set_max_width_chars(40)

        area.pack_start(lbl_name, False, False, 0)
        area.pack_start(entry, False, False, 0)
        area.pack_start(lbl_type, False, False, 0)
        area.pack_start(combo, False, False, 0)
        area.pack_start(hint, False, False, 0)
        dlg.show_all()
        entry.grab_focus()

        resp = dlg.run()
        new_name = entry.get_text().strip()[:48]
        new_type = combo.get_active_id() or cur_type
        dlg.destroy()

        if resp == Gtk.ResponseType.OK and new_name and 0 <= index < len(self._icons):
            chosen_type = new_type if new_type != cur_type else None
            self._icons = rename_virtual_icon(self._icons, index, new_name, icon_type=chosen_type)
            self._sync_ui()
            self._on_icons_changed([dict(i) for i in self._icons], True)

    def _show_icon_context_menu(self, index: int, event: Any) -> None:
        Gtk = self._Gtk
        menu = Gtk.Menu()

        mi_rename = Gtk.MenuItem(label="Yeniden Adlandır…")
        mi_rename.connect("activate", lambda *_: self._open_rename_dialog(index))
        menu.append(mi_rename)

        mi_del = Gtk.MenuItem(label="Kaldır")

        def _on_delete(*_args: Any) -> None:
            if 0 <= index < len(self._icons):
                self._icons.pop(index)
                self._drag_index = -1
                self._hover_index = -1
                self._sync_ui()
                self._on_icons_changed([dict(i) for i in self._icons], True)

        mi_del.connect("activate", _on_delete)
        menu.append(mi_del)
        menu.show_all()
        self._ctx_menu = menu
        try:
            menu.popup_at_pointer(event)
        except Exception:
            menu.popup(None, None, None, None, getattr(event, "button", 3), getattr(event, "time", 0))

    def _on_button_press(self, _widget: Any, event: Any) -> bool:
        btn = getattr(event, "button", 1)
        hit = self._hit_icon(float(event.x), float(event.y))
        if btn == 3 and hit >= 0:
            self._show_icon_context_menu(hit, event)
            return True
        if btn != 1:
            return False
        if hit >= 0:
            if event.type == self._Gdk.EventType.DOUBLE_BUTTON_PRESS:
                self._icons.pop(hit)
                self._drag_index = -1
                self._hover_index = -1
                self._sync_ui()
                self._on_icons_changed([dict(i) for i in self._icons], True)
                return True
            it = self._icons[hit]
            self._drag_index = hit
            self._drag_moved = False
            self._drag_ox = float(event.x) - float(it.get("x", 0))
            self._drag_oy = float(event.y) - float(it.get("y", 0))
            return True
        if self._on_pass_mouse is not None:
            self._on_pass_mouse(1, float(event.x), float(event.y))
        return False

    def _on_motion(self, _widget: Any, event: Any) -> bool:
        px, py = float(event.x), float(event.y)
        if 0 <= self._drag_index < len(self._icons):
            it = self._icons[self._drag_index]
            w = int(it.get("w", DEFAULT_ICON_W))
            h = int(it.get("h", DEFAULT_ICON_H))
            nx, ny = _clamp_pos(
                px - self._drag_ox,
                py - self._drag_oy,
                w,
                h,
                self._logical_w,
                self._logical_h,
                self._safe_insets,
            )
            if abs(nx - int(it.get("x", 0))) + abs(ny - int(it.get("y", 0))) >= 2:
                self._drag_moved = True
                it["x"] = nx
                it["y"] = ny
                self._canvas.queue_draw()
                self._on_icons_changed([dict(i) for i in self._icons], False)
            return True
        hit = self._hit_icon(px, py)
        if hit != self._hover_index:
            self._hover_index = hit
            self._canvas.queue_draw()
        if self._on_pass_mouse is not None:
            self._on_pass_mouse(0, px, py)
        return False

    def _on_button_release(self, _widget: Any, event: Any) -> bool:
        if getattr(event, "button", 1) != 1:
            return False
        if self._drag_index >= 0:
            moved = self._drag_moved
            self._drag_index = -1
            self._drag_moved = False
            if moved:
                self._emit_ui_rects()
                self._on_icons_changed([dict(i) for i in self._icons], True)
            return True
        if self._on_pass_mouse is not None:
            self._on_pass_mouse(2, float(event.x), float(event.y))
        return False

    def _on_leave(self, _widget: Any, _event: Any) -> bool:
        if self._hover_index != -1:
            self._hover_index = -1
            self._canvas.queue_draw()
        if self._on_pass_mouse is not None:
            self._on_pass_mouse(3, -1.0, -1.0)
        return False

    def destroy(self) -> None:
        try:
            self.overlay.destroy()
        except Exception:
            pass
