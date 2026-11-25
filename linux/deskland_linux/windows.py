"""Window-Aware Scenery reader for Linux compositors (Hyprland, Sway, niri).

Pure Python — zero GTK dependency so it stays fully unit-testable.
Reads visible non-fullscreen floating/tiled window bounding rectangles so
themes can react to open windows even on iconless tiling Wayland desktops.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class WindowRect:
    wid: int
    left: int
    top: int
    right: int
    bottom: int
    app_id: str = ""

    def as_wire_list(self, scale: float = 1.0, offset_x: int = 0, offset_y: int = 0) -> list[int]:
        s = max(0.5, float(scale))
        return [
            int(self.wid),
            int(round((self.left - offset_x) * s)),
            int(round((self.top - offset_y) * s)),
            int(round((self.right - offset_x) * s)),
            int(round((self.bottom - offset_y) * s)),
        ]

    def as_icon_rect(self, scale: float = 1.0, offset_x: int = 0, offset_y: int = 0) -> list[int]:
        s = max(0.5, float(scale))
        return [
            int(round((self.left - offset_x) * s)),
            int(round((self.top - offset_y) * s)),
            int(round((self.right - offset_x) * s)),
            int(round((self.bottom - offset_y) * s)),
        ]


def _stable_id(raw: Any, fallback: int) -> int:
    if isinstance(raw, int):
        return raw & 0x7FFFFFFF
    if isinstance(raw, str):
        try:
            return int(raw, 16) & 0x7FFFFFFF
        except ValueError:
            return abs(hash(raw)) & 0x7FFFFFFF
    return fallback


def parse_hyprland_clients(raw_json: str, active_workspace_id: int | None = None) -> list[WindowRect]:
    try:
        data = json.loads(raw_json)
    except (ValueError, TypeError):
        return []
    if not isinstance(data, list):
        return []
    out: list[WindowRect] = []
    for idx, c in enumerate(data):
        if not isinstance(c, dict):
            continue
        if not c.get("mapped", True) or c.get("hidden", False):
            continue
        if active_workspace_id is not None:
            ws = c.get("workspace")
            if isinstance(ws, dict) and ws.get("id") != active_workspace_id:
                continue
        at = c.get("at")
        size = c.get("size")
        if not (isinstance(at, list) and len(at) >= 2 and isinstance(size, list) and len(size) >= 2):
            continue
        w, h = int(size[0]), int(size[1])
        if w < 48 or h < 48:
            continue
        cls = str(c.get("class") or c.get("initialClass") or "")
        if "deskland" in cls.lower() or "diyardesk" in cls.lower():
            continue
        x, y = int(at[0]), int(at[1])
        out.append(WindowRect(_stable_id(c.get("address"), idx + 1), x, y, x + w, y + h, cls))
    return out


def parse_sway_windows(raw_json: str) -> list[WindowRect]:
    try:
        root = json.loads(raw_json)
    except (ValueError, TypeError):
        return []
    out: list[WindowRect] = []

    def walk(node: Any) -> None:
        if not isinstance(node, dict):
            return
        ntype = node.get("type")
        if ntype in ("con", "floating_con") and node.get("pid") and node.get("visible", True):
            app = str(node.get("app_id") or (node.get("window_properties") or {}).get("class") or "")
            if "deskland" not in app.lower() and "diyardesk" not in app.lower():
                rect = node.get("rect")
                if isinstance(rect, dict):
                    w = int(rect.get("width", 0))
                    h = int(rect.get("height", 0))
                    if w >= 48 and h >= 48:
                        x = int(rect.get("x", 0))
                        y = int(rect.get("y", 0))
                        out.append(WindowRect(_stable_id(node.get("id"), len(out) + 1), x, y, x + w, y + h, app))
        for key in ("nodes", "floating_nodes"):
            children = node.get(key)
            if isinstance(children, list):
                for ch in children:
                    walk(ch)

    walk(root)
    return out


def parse_niri_windows(raw_json: str) -> list[WindowRect]:
    """Parse `niri msg -j windows` JSON output."""
    try:
        data = json.loads(raw_json)
    except (ValueError, TypeError):
        return []
    if not isinstance(data, list):
        return []
    out: list[WindowRect] = []
    for idx, w in enumerate(data):
        if not isinstance(w, dict):
            continue
        app = str(w.get("app_id") or "")
        if "deskland" in app.lower() or "diyardesk" in app.lower():
            continue
        layout = w.get("layout") if isinstance(w.get("layout"), dict) else {}
        tile_size = layout.get("tile_size") or layout.get("window_size")
        pos = layout.get("tile_pos_in_workspace_view") or layout.get("pos_in_scrolling_layout") or [0, 0]
        if not (isinstance(tile_size, list) and len(tile_size) >= 2):
            continue
        width = int(round(float(tile_size[0])))
        height = int(round(float(tile_size[1])))
        if width < 48 or height < 48:
            continue
        x = int(round(float(pos[0]))) if isinstance(pos, list) and len(pos) >= 2 else 0
        y = int(round(float(pos[1]))) if isinstance(pos, list) and len(pos) >= 2 else 0
        out.append(WindowRect(_stable_id(w.get("id"), idx + 1), x, y, x + width, y + height, app))
    return out


def _run_cmd(args: list[str], timeout: float = 0.6) -> str | None:
    if not shutil.which(args[0]):
        return None
    try:
        proc = subprocess.run(args, capture_output=True, text=True, timeout=timeout, check=False)
        if proc.returncode == 0 and proc.stdout:
            return proc.stdout
    except (OSError, subprocess.SubprocessError):
        return None
    return None


@dataclass(frozen=True)
class PanelRect:
    name: str
    left: int
    top: int
    right: int
    bottom: int

    def as_rect(self, scale: float = 1.0, offset_x: int = 0, offset_y: int = 0) -> list[int]:
        s = max(0.5, float(scale))
        return [
            int(round((self.left - offset_x) * s)),
            int(round((self.top - offset_y) * s)),
            int(round((self.right - offset_x) * s)),
            int(round((self.bottom - offset_y) * s)),
        ]


def parse_hyprland_layers(raw_json: str) -> list[PanelRect]:
    """Parse `hyprctl -j layers` for Waybar/panel/dock surfaces."""
    try:
        data = json.loads(raw_json)
    except (ValueError, TypeError):
        return []
    if not isinstance(data, dict):
        return []
    panels: list[PanelRect] = []
    for _mon, info in data.items():
        if not isinstance(info, dict):
            continue
        levels = info.get("levels")
        if not isinstance(levels, dict):
            continue
        # Levels 2 (top) and 3 (overlay) host panels/Waybar/docks
        for lvl_key in ("2", "3"):
            entries = levels.get(lvl_key)
            if not isinstance(entries, list):
                continue
            for item in entries:
                if not isinstance(item, dict):
                    continue
                ns = str(item.get("namespace") or "")
                if "deskland" in ns.lower() or "diyardesk" in ns.lower():
                    continue
                w = int(item.get("w", 0))
                h = int(item.get("h", 0))
                if w < 16 or h < 16:
                    continue
                x = int(item.get("x", 0))
                y = int(item.get("y", 0))
                panels.append(PanelRect(ns or "panel", x, y, x + w, y + h))
    return panels


def parse_workspace_id(raw_json: str, compositor: str) -> int | None:
    try:
        data = json.loads(raw_json)
    except (ValueError, TypeError):
        return None
    if compositor == "hyprland" and isinstance(data, dict):
        wid = data.get("id")
        return int(wid) if isinstance(wid, int) else None
    if compositor == "sway" and isinstance(data, list):
        for ws in data:
            if isinstance(ws, dict) and ws.get("focused"):
                num = ws.get("num")
                return int(num) if isinstance(num, int) else None
    if compositor == "niri" and isinstance(data, list):
        for ws in data:
            if isinstance(ws, dict) and ws.get("is_active"):
                idx = ws.get("idx") or ws.get("id")
                return int(idx) if isinstance(idx, int) else None
    return None


def detect_workspace_state() -> tuple[int | None, list[PanelRect]]:
    if os.environ.get("HYPRLAND_INSTANCE_SIGNATURE"):
        ws_raw = _run_cmd(["hyprctl", "-j", "activeworkspace"])
        lay_raw = _run_cmd(["hyprctl", "-j", "layers"])
        ws_id = parse_workspace_id(ws_raw, "hyprland") if ws_raw else None
        panels = parse_hyprland_layers(lay_raw) if lay_raw else []
        return ws_id, panels
    if os.environ.get("SWAYSOCK"):
        ws_raw = _run_cmd(["swaymsg", "-t", "get_workspaces"])
        return (parse_workspace_id(ws_raw, "sway") if ws_raw else None), []
    if os.environ.get("NIRI_SOCKET"):
        ws_raw = _run_cmd(["niri", "msg", "-j", "workspaces"])
        return (parse_workspace_id(ws_raw, "niri") if ws_raw else None), []
    return None, []


def parse_gnome_shell_windows(raw_json: str) -> list[WindowRect]:
    """Parse `org.deskland.GnomeShell.ListWindows` JSON payload."""
    try:
        data = json.loads(raw_json)
    except (ValueError, TypeError):
        return []
    if not isinstance(data, list):
        return []
    out: list[WindowRect] = []
    for idx, item in enumerate(data):
        if not isinstance(item, (list, tuple)) or len(item) < 5:
            continue
        try:
            wid = _stable_id(item[0], idx + 1)
            l, t, r, b = int(item[1]), int(item[2]), int(item[3]), int(item[4])
        except (TypeError, ValueError):
            continue
        if (r - l) < 60 or (b - t) < 40:
            continue
        cls = str(item[5]) if len(item) >= 6 else ""
        if "diyardesk" in cls.lower() or "deskland" in cls.lower():
            continue
        out.append(WindowRect(wid, l, t, r, b, cls))
    return out


def _query_gnome_shell_windows() -> list[WindowRect]:
    try:
        import gi

        gi.require_version("Gio", "2.0")
        from gi.repository import Gio

        bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        res = bus.call_sync(
            "org.deskland.GnomeShell",
            "/org/deskland/GnomeShell",
            "org.deskland.GnomeShell",
            "ListWindows",
            None,
            None,
            Gio.DBusCallFlags.NONE,
            250,
            None,
        )
        if res is not None:
            unpacked = res.unpack()
            if unpacked and isinstance(unpacked[0], str):
                return parse_gnome_shell_windows(unpacked[0])
    except Exception:
        pass
    return []


def detect_active_windows() -> list[WindowRect]:
    if os.environ.get("HYPRLAND_INSTANCE_SIGNATURE"):
        ws_id = None
        ws_raw = _run_cmd(["hyprctl", "-j", "activeworkspace"])
        if ws_raw:
            try:
                ws_id = json.loads(ws_raw).get("id")
            except (ValueError, TypeError):
                ws_id = None
        raw = _run_cmd(["hyprctl", "-j", "clients"])
        if raw:
            return parse_hyprland_clients(raw, active_workspace_id=ws_id)

    if os.environ.get("SWAYSOCK"):
        raw = _run_cmd(["swaymsg", "-t", "get_tree"])
        if raw:
            return parse_sway_windows(raw)

    if os.environ.get("NIRI_SOCKET"):
        raw = _run_cmd(["niri", "msg", "-j", "windows"])
        if raw:
            return parse_niri_windows(raw)

    desktop = (os.environ.get("XDG_CURRENT_DESKTOP") or "").lower()
    if "gnome" in desktop or os.environ.get("GNOME_DESKTOP_SESSION_ID"):
        wins = _query_gnome_shell_windows()
        if wins:
            return wins

    return []
