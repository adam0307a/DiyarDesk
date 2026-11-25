"""Desktop icon-position adapters.

Linux has no single cross-desktop API for desktop icon geometry. Each file
manager owns its own storage:

  * Nautilus / Desktop Icons NG / Nemo / Caja: GVfs metadata, readable with
    ``gio info -a metadata::<fm>-icon-position``.
  * Xfce: ~/.config/xfce4/desktop/icons.screen0-*.rc
  * Plasma: ~/.config/plasma-org.kde.plasma.desktop-appletsrc (best effort).

All adapters return rectangles in *physical pixels* relative to the primary
monitor origin. Positions that cannot be read fall back to a deterministic
grid so the wallpaper still has plausible icon islands.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import time
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import unquote, urlparse

CELL_W = 76
CELL_H = 100
GRID_X0 = 4
GRID_Y0 = 6
TASKBAR = 48

# Default XfdesktopIconView layout parameters from xfdesktop 4.20.
XFCE_DEFAULT_ICON_SIZE = 48
XFCE_CELL_PADDING = 6
XFCE_TEXT_WIDTH_PROPORTION = 1.9
XFCE_SCREEN_MARGIN = 8

META_KEYS = (
    "metadata::nautilus-icon-position",
    "metadata::ding-icon-position",
    "metadata::nemo-icon-position",
    "metadata::caja-icon-position",
)

_POS_RE = re.compile(r"(-?\d+(?:\.\d+)?)\s*[,;:]\s*(-?\d+(?:\.\d+)?)")
_ATTR_RE = re.compile(r"^\s+([A-Za-z0-9:_-]+):\s*(.*)$")


def _looks_like_url(value: str) -> bool:
    value = str(value)
    return value.startswith(("desktop:/", "file://", "/"))


def _clean_url_key(url: str) -> str:
    url = str(url).strip()
    if "://" in url and not url.startswith("desktop:/"):
        return unquote(urlparse(url).path)
    return url


def _xy_from_item(item, prefer_tail: bool = False) -> tuple[float, float] | None:
    if isinstance(item, dict) and "x" in item and "y" in item:
        try:
            return float(item["x"]), float(item["y"])
        except (TypeError, ValueError):
            return None
    if isinstance(item, (list, tuple)):
        if prefer_tail and len(item) >= 3:
            # changedPositions: ["1536x864", "col", "row"]
            item = item[-2:]
        if len(item) >= 2:
            try:
                return float(item[0]), float(item[1])
            except (TypeError, ValueError):
                return None
    return None


@dataclass(frozen=True)
class Rect:
    x: int
    y: int
    w: int
    h: int
    label: str = ""

    def as_array(self) -> list[int]:
        return [self.x, self.y, self.x + self.w, self.y + self.h]


def parse_position(value: str) -> tuple[float, float] | None:
    match = _POS_RE.search(value or "")
    if not match:
        return None
    return float(match.group(1)), float(match.group(2))


def desktop_entries(directory: Path) -> list[Path]:
    if not directory.is_dir():
        return []
    try:
        entries = [p for p in directory.iterdir() if not p.name.startswith(".")]
    except OSError:
        return []
    return sorted(entries, key=lambda p: p.name.lower())


def grid_rect(index: int, scale: int, screen_w: int, screen_h: int) -> Rect:
    cell_w = CELL_W * scale
    cell_h = CELL_H * scale
    usable_h = max(cell_h, screen_h - TASKBAR * scale)
    rows = max(1, usable_h // cell_h)
    col, row = divmod(index, rows)
    x = GRID_X0 * scale + col * cell_w
    y = GRID_Y0 * scale + row * cell_h
    return Rect(int(x), int(y), int(cell_w), int(cell_h))


def _rect_from_position(
    x: float, y: float, scale: int, *, physical: bool
) -> Rect:
    k = 1 if physical else scale
    return Rect(int(x * k), int(y * k), int(CELL_W * scale), int(CELL_H * scale))


def _rect_from_cell(col: float, row: float, scale: int) -> Rect:
    x = GRID_X0 * scale + int(col) * CELL_W * scale
    y = GRID_Y0 * scale + int(row) * CELL_H * scale
    return Rect(int(x), int(y), int(CELL_W * scale), int(CELL_H * scale))


def _xfce_grid_params(
    logical_w: int,
    logical_h: int,
    *,
    icon_size: int = XFCE_DEFAULT_ICON_SIZE,
    cell_padding: int = XFCE_CELL_PADDING,
    proportion: float = XFCE_TEXT_WIDTH_PROPORTION,
) -> tuple[float, int, int, int, int]:
    """Compute XfdesktopIconView grid metrics matching xfdesktop's layout math.

    Returns ``(cell_size, xmargin, ymargin, xspacing, yspacing)`` in logical px.
    """
    edge_total = XFCE_SCREEN_MARGIN * 2
    cell_size = icon_size * proportion + 2 * cell_padding
    if cell_size <= 0 or logical_w <= edge_total or logical_h <= edge_total:
        return (float(CELL_W), GRID_X0, GRID_Y0, 0, 0)
    ncols = int((logical_w - edge_total) / cell_size)
    nrows = int((logical_h - edge_total) / cell_size)
    if ncols <= 0 or nrows <= 0:
        return (float(CELL_W), GRID_X0, GRID_Y0, 0, 0)
    rem_w = int(logical_w - ncols * cell_size)
    xspacing = (rem_w - edge_total) // (ncols - 1) if ncols > 1 else 0
    xmargin = (rem_w - xspacing * (ncols - 1)) // 2
    rem_h = int(logical_h - nrows * cell_size)
    yspacing = (rem_h - edge_total) // (nrows - 1) if nrows > 1 else 0
    ymargin = (rem_h - yspacing * (nrows - 1)) // 2
    return (cell_size, xmargin, ymargin, xspacing, yspacing)


def xfce_cell_rect(
    col: float,
    row: float,
    scale: int,
    screen_w: int,
    screen_h: int,
    *,
    icon_size: int = XFCE_DEFAULT_ICON_SIZE,
) -> Rect:
    """Convert an XFCE desktop ``(col, row)`` slot into physical pixel coordinates."""
    eff_scale = max(1, int(scale))
    logical_w = max(1, int(screen_w) // eff_scale)
    logical_h = max(1, int(screen_h) // eff_scale)
    cell_size, xmargin, ymargin, xspacing, yspacing = _xfce_grid_params(
        logical_w, logical_h, icon_size=icon_size
    )
    x_logical = int(xmargin + int(col) * (cell_size + xspacing))
    y_logical = int(ymargin + int(row) * (cell_size + yspacing))
    cell_px = max(1, int(cell_size)) * eff_scale
    return Rect(x_logical * eff_scale, y_logical * eff_scale, cell_px, cell_px)


class IconAdapter:
    name = "base"

    def __init__(
        self,
        desktop: Path,
        scale: int,
        screen_w: int,
        screen_h: int,
        origin_x: int = 0,
        origin_y: int = 0,
    ) -> None:
        self.desktop = desktop
        self.scale = max(1, scale)
        self.screen_w = screen_w
        self.screen_h = screen_h
        self.origin_x = origin_x
        self.origin_y = origin_y
        self._entries = desktop_entries(desktop)
        self._physical = os.environ.get("DESKLAND_ICON_POS_PHYSICAL") == "1"

    def _refresh_entries(self) -> None:
        # Rescan the desktop folder on every read: files added or moved at
        # runtime must not wait for a restart before they appear.
        try:
            self._entries = desktop_entries(self.desktop)
        except OSError:
            pass

    def _grid_for(self, path: Path, index: int, used: set[tuple[int, int]]) -> Rect:
        for candidate in range(index, index + 400):
            rect = grid_rect(candidate, self.scale, self.screen_w, self.screen_h)
            key = (rect.x, rect.y)
            if key not in used:
                used.add(key)
                return Rect(rect.x, rect.y, rect.w, rect.h, path.name)
        rect = grid_rect(index, self.scale, self.screen_w, self.screen_h)
        return Rect(rect.x, rect.y, rect.w, rect.h, path.name)

    def read(self) -> tuple[bool, list[Rect]]:
        return True, []


class GioMetadataAdapter(IconAdapter):
    name = "gio-metadata"

    def read(self) -> tuple[bool, list[Rect]]:
        self._refresh_entries()
        if not self._entries:
            return True, []
        positions = self._read_metadata()
        used: set[tuple[int, int]] = set()
        rects: list[Rect] = []
        for index, path in enumerate(self._entries):
            pos = positions.get(str(path))
            if pos is None:
                rect = self._grid_for(path, index, used)
            else:
                rect = _rect_from_position(*pos, self.scale, physical=self._physical)
                used.add((rect.x, rect.y))
                rect = Rect(rect.x, rect.y, rect.w, rect.h, path.name)
            rects.append(rect)
        return True, rects

    def _read_metadata(self) -> dict[str, tuple[float, float]]:
        paths = [str(p) for p in self._entries]
        positions: dict[str, tuple[float, float]] = {}
        for key in META_KEYS:
            # One query per key: GVfs answers with nothing at all (no error)
            # if a single requested attribute is unknown to it, which used to
            # silently break the whole batch and always report 0 positions.
            cmd = ["gio", "info", "-a", key] + paths
            try:
                proc = subprocess.run(
                    cmd,
                    capture_output=True,
                    text=True,
                    timeout=4,
                    check=False,
                )
            except (OSError, subprocess.SubprocessError):
                continue
            positions.update(self._parse_output(proc.stdout))
        return positions

    @staticmethod
    def _parse_output(text: str) -> dict[str, tuple[float, float]]:
        result: dict[str, tuple[float, float]] = {}
        current: str | None = None
        for line in text.splitlines():
            if line.startswith("uri:"):
                uri = line[4:].strip()
                parsed = urlparse(uri)
                current = unquote(parsed.path) if parsed.scheme == "file" else uri
                continue
            match = _ATTR_RE.match(line)
            if not match or current is None:
                continue
            key, value = match.group(1), match.group(2).strip()
            if key in META_KEYS:
                pos = parse_position(value)
                if pos is not None:
                    result[current] = pos
        return result


class XfceAdapter(IconAdapter):
    name = "xfce"

    def _config_root(self) -> Path:
        return Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))

    def _icon_size(self) -> int:
        xml_path = (
            self._config_root()
            / "xfce4"
            / "xfconf"
            / "xfce-perchannel-xml"
            / "xfce4-desktop.xml"
        )
        try:
            root = ET.parse(xml_path).getroot()
        except (OSError, ET.ParseError):
            return XFCE_DEFAULT_ICON_SIZE
        # Navigate <channel><property name="desktop-icons"><property name="icon-size" value="..."/>
        for top in root.findall("./property[@name='desktop-icons']"):
            prop = top.find("./property[@name='icon-size']")
            if prop is not None:
                raw = prop.attrib.get("value")
                try:
                    val = int(raw or "")
                except ValueError:
                    return XFCE_DEFAULT_ICON_SIZE
                if 16 <= val <= 256:
                    return val
        return XFCE_DEFAULT_ICON_SIZE

    def _layout_file(self) -> Path | None:
        directory = self._config_root() / "xfce4" / "desktop"
        try:
            yaml_files = sorted(directory.glob("icons.screen0*.yaml"))
            if yaml_files:
                return yaml_files[0]
            files = sorted(directory.glob("icons.screen0-*.rc"))
        except OSError:
            return None
        return files[0] if files else None

    def read(self) -> tuple[bool, list[Rect]]:
        self._refresh_entries()
        if not self._entries:
            return True, []
        found: dict[str, tuple[float, float]] = {}
        layout = self._layout_file()
        if layout is not None:
            try:
                text = layout.read_text(encoding="utf-8", errors="replace")
                if layout.suffix == ".yaml":
                    found = self._parse_yaml(text)
                else:
                    found = self._parse(text)
            except OSError:
                found = {}
        icon_size = self._icon_size()
        used: set[tuple[int, int]] = set()
        rects: list[Rect] = []
        for index, path in enumerate(self._entries):
            pos = found.get(str(path)) or found.get(path.name)
            if pos is None:
                rect = self._grid_for(path, index, used)
            else:
                rect = xfce_cell_rect(
                    pos[0],
                    pos[1],
                    self.scale,
                    self.screen_w,
                    self.screen_h,
                    icon_size=icon_size,
                )
                used.add((rect.x, rect.y))
                rect = Rect(rect.x, rect.y, rect.w, rect.h, path.name)
            rects.append(rect)
        return True, rects

    @staticmethod
    def _unescape_yaml_key(key: str) -> str:
        if len(key) >= 2 and key[0] == '"' and key[-1] == '"':
            key = key[1:-1]
            return re.sub(
                r'\\x([0-9a-fA-F]{2})|\\u([0-9a-fA-F]{4})|\\(["\\/])',
                lambda m: (
                    chr(int(m.group(1), 16))
                    if m.group(1)
                    else chr(int(m.group(2), 16))
                    if m.group(2)
                    else m.group(3)
                ),
                key,
            )
        if len(key) >= 2 and key[0] == "'" and key[-1] == "'":
            return key[1:-1].replace("''", "'")
        return key

    @classmethod
    def _parse_yaml(cls, text: str) -> dict[str, tuple[float, float]]:
        result: dict[str, tuple[float, float]] = {}
        in_icons = False
        icons_indent = -1
        current_path: str | None = None
        cur_row: int | None = None
        cur_col: int | None = None

        def flush() -> None:
            if current_path is not None and cur_row is not None and cur_col is not None:
                result[current_path] = (cur_col, cur_row)

        for raw in text.splitlines():
            stripped = raw.strip()
            if not stripped or stripped.startswith("#"):
                continue
            indent = len(raw) - len(raw.lstrip(" "))
            if stripped == "icons:":
                flush()
                current_path = None
                in_icons = True
                icons_indent = indent
                continue
            if not in_icons:
                continue
            if indent <= icons_indent:
                flush()
                current_path = None
                in_icons = False
                continue
            if stripped.endswith(":") and indent == icons_indent + 2:
                flush()
                current_path = cls._unescape_yaml_key(stripped[:-1].strip())
                cur_row = None
                cur_col = None
            elif current_path is not None and ":" in stripped:
                k, v = stripped.split(":", 1)
                k = k.strip()
                v = v.strip()
                if k == "row":
                    try:
                        cur_row = int(v)
                    except ValueError:
                        pass
                elif k == "col":
                    try:
                        cur_col = int(v)
                    except ValueError:
                        pass
        flush()
        return result

    @staticmethod
    def _parse(text: str) -> dict[str, tuple[float, float]]:
        result: dict[str, tuple[float, float]] = {}
        section_row: int | None = None
        section_col: int | None = None
        for raw in text.splitlines():
            line = raw.strip()
            if line.startswith("[") and line.endswith("]"):
                section = line[1:-1]
                row_match = re.search(r"row\D*(\d+)", section, re.IGNORECASE)
                col_match = re.search(r"col\D*(\d+)", section, re.IGNORECASE)
                section_row = int(row_match.group(1)) if row_match else None
                section_col = int(col_match.group(1)) if col_match else None
                continue
            if "=" not in line or "file://" not in line:
                continue
            key, value = line.split("=", 1)
            value = value.strip()
            path = unquote(urlparse(value).path)
            pos = parse_position(key)
            if pos is None:
                row = re.search(r"row\D*(\d+)", key, re.IGNORECASE)
                col = re.search(r"col\D*(\d+)", key, re.IGNORECASE)
                r = int(row.group(1)) if row else section_row
                c = int(col.group(1)) if col else section_col
                if r is not None and c is not None:
                    pos = (c, r)
            if pos is not None:
                result[path] = pos
        return result


class PlasmaAdapter(IconAdapter):
    name = "plasma-best-effort"

    def _config_file(self) -> Path:
        root = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
        return root / "plasma-org.kde.plasma.desktop-appletsrc"

    def read(self) -> tuple[bool, list[Rect]]:
        self._refresh_entries()
        if not self._entries:
            return True, []
        found: dict[str, tuple[float, float]] = {}
        try:
            found = self._normalize_urls(self._parse(self._config_file().read_text(encoding="utf-8", errors="replace")))
        except OSError:
            pass
        used: set[tuple[int, int]] = set()
        rects: list[Rect] = []
        for index, path in enumerate(self._entries):
            pos = found.get(os.path.realpath(path)) or found.get(str(path)) or found.get(path.name)
            if pos is None:
                rect = self._grid_for(path, index, used)
            elif abs(pos[0]) <= 100 and abs(pos[1]) <= 100:
                rect = self._rect_from_plasma_cell(*pos)
            else:
                rect = _rect_from_position(*pos, self.scale, physical=self._physical)
            used.add((rect.x, rect.y))
            rects.append(Rect(rect.x, rect.y, rect.w, rect.h, path.name))
        return True, rects

    def _rect_from_plasma_cell(self, col: float, row: float) -> Rect:
        # KDE stores folder-view positions as grid cells (e.g. "5","2") in
        # logical pixels of the screen the grid belongs to. The desktop
        # fills the whole logical screen minus the bottom panel; the exact
        # cell size is not persisted, so derive it from the geometry and
        # allow tuning via env when a layout differs.
        logical_w = max(1, self.screen_w // self.scale)
        logical_h = max(1, self.screen_h // self.scale)
        cell_w = float(os.environ.get("DESKLAND_PLASMA_CELL_W", "") or logical_w / 22)
        cell_h = float(os.environ.get("DESKLAND_PLASMA_CELL_H", "") or logical_h / 16)
        off_x = float(os.environ.get("DESKLAND_PLASMA_ORIGIN_X", "") or 0)
        off_y = float(os.environ.get("DESKLAND_PLASMA_ORIGIN_Y", "") or 0)
        k = self.scale
        return Rect(
            int((off_x + col * cell_w) * k),
            int((off_y + row * cell_h) * k),
            int(cell_w * k),
            int(cell_h * k),
        )

    def _normalize_urls(self, raw: dict[str, tuple[float, float]]) -> dict[str, tuple[float, float]]:
        out: dict[str, tuple[float, float]] = {}
        for key, pos in raw.items():
            path = self._url_to_path(key)
            if path is None:
                continue
            try:
                resolved = Path(os.path.realpath(path))
            except OSError:
                resolved = path
            out[str(resolved)] = pos
        return out

    def _url_to_path(self, url: str) -> Path | None:
        if url.startswith("desktop:/"):
            rel = unquote(url[len("desktop:/"):])
            if not rel or rel.startswith("/"):
                return None
            return Path(os.path.realpath(self.desktop / rel))
        if url.startswith("file://"):
            return Path(unquote(urlparse(url).path))
        p = Path(url)
        return p if p.is_absolute() else self.desktop / p

    @staticmethod
    def _parse(text: str) -> dict[str, tuple[float, float]]:
        result: dict[str, tuple[float, float]] = {}
        overrides: dict[str, tuple[float, float]] = {}
        for line in text.splitlines():
            if "=" not in line:
                continue
            key, value = line.split("=", 1)
            key = key.strip().lower()
            value = value.strip()
            if not value.startswith("{"):
                continue
            try:
                data = json.loads(value)
            except ValueError:
                continue
            if not isinstance(data, dict):
                continue
            if key == "positions":
                for screen_key, item in data.items():
                    if _looks_like_url(screen_key):
                        # Legacy: flat {url: [x, y] | {"x":…,"y":…}}
                        pos = _xy_from_item(item)
                        if pos is not None:
                            result[_clean_url_key(screen_key)] = pos
                        continue
                    if isinstance(item, dict):
                        # Legacy nested: {"screen": {url: xy}}
                        for url, xy in item.items():
                            pos = _xy_from_item(xy)
                            if pos is not None:
                                result[_clean_url_key(url)] = pos
                        continue
                    if isinstance(item, list):
                        # KDE Plasma 6 folder view: {"1536x864": ["x","y",
                        # "desktop:/file", ...]} — flat [col, row, url]
                        # triples as strings. A trailing incomplete pair may
                        # be present and is ignored.
                        for i in range(0, len(item) - 2, 3):
                            try:
                                x = float(item[i])
                                y = float(item[i + 1])
                            except (TypeError, ValueError):
                                continue
                            url = str(item[i + 2])
                            if _looks_like_url(url):
                                result[_clean_url_key(url)] = (x, y)
                        continue
            elif key == "changedpositions":
                for url, item in data.items():
                    pos = _xy_from_item(item, prefer_tail=True)
                    if pos is not None:
                        overrides[_clean_url_key(url)] = pos
        # changedPositions is a delta on top of positions; it wins.
        result.update(overrides)
        return result


class GridAdapter(IconAdapter):
    name = "grid-fallback"

    def read(self) -> tuple[bool, list[Rect]]:
        self._refresh_entries()
        used: set[tuple[int, int]] = set()
        return True, [self._grid_for(p, i, used) for i, p in enumerate(self._entries)]


class NullIconAdapter(IconAdapter):
    name = "none"

    def read(self) -> tuple[bool, list[Rect]]:
        return False, []


GNOME_ICON_EXTENSION_HINTS = (
    "ding",
    "desktop-icons",
    "desktopicons",
)

_ICON_DAEMON_COMMS = frozenset(
    {
        "nemo-desktop",
        "caja",
        "pcmanfm",
        "pcmanfm-qt",
        "xfdesktop",
    }
)

_ICONLESS_WMS = (
    "hyprland",
    "sway",
    "niri",
    "river",
    "wayfire",
    "labwc",
    "mangowc",
    "i3",
    "bspwm",
    "dwm",
    "awesome",
    "qtile",
    "xmonad",
    "openbox",
    "fluxbox",
    "herbstluftwm",
)


def parse_gsettings_extension_list(raw: str) -> list[str]:
    """Parse `gsettings get org.gnome.shell enabled-extensions` output."""
    text = (raw or "").strip()
    if not text or text == "@as []":
        return []
    return re.findall(r"'([^']+)'|\"([^\"]+)\"", text) and [
        a or b for a, b in re.findall(r"'([^']+)'|\"([^\"]+)\"", text)
    ]


def has_gnome_icon_extension(
    enabled_extensions: list[str],
    *,
    user_extensions_disabled: bool = False,
) -> bool:
    if user_extensions_disabled:
        return False
    for ext_id in enabled_extensions:
        low = ext_id.lower()
        if any(hint in low for hint in GNOME_ICON_EXTENSION_HINTS):
            return True
    return False


def _has_running_icon_daemon() -> bool:
    proc_root = Path("/proc")
    try:
        for entry in proc_root.iterdir():
            if not entry.name.isdigit():
                continue
            try:
                comm = (entry / "comm").read_text(encoding="utf-8", errors="ignore").strip().lower()
                if comm in _ICON_DAEMON_COMMS:
                    return True
            except OSError:
                continue
    except OSError:
        pass
    return False


def detect_gnome_desktop_icons() -> bool:
    """Return True only when GNOME actually displays desktop icons (DING/etc. or icon daemon)."""
    if _has_running_icon_daemon():
        return True
    try:
        dis = subprocess.run(
            ["gsettings", "get", "org.gnome.shell", "disable-user-extensions"],
            capture_output=True,
            text=True,
            timeout=2,
            check=False,
        )
        if dis.returncode == 0 and dis.stdout.strip().lower() == "true":
            return False
        ext = subprocess.run(
            ["gsettings", "get", "org.gnome.shell", "enabled-extensions"],
            capture_output=True,
            text=True,
            timeout=2,
            check=False,
        )
        if ext.returncode == 0:
            enabled = parse_gsettings_extension_list(ext.stdout)
            return has_gnome_icon_extension(enabled, user_extensions_disabled=False)
    except (OSError, subprocess.SubprocessError):
        pass
    return False


class GnomeIconAdapter(GioMetadataAdapter):
    """GNOME desktop icon adapter: avoids phantom ~/Desktop icons when DING is absent."""

    name = "gnome-ding"

    def __init__(self, *args, detector=None, **kwargs) -> None:
        super().__init__(*args, **kwargs)
        self._detector = detector or detect_gnome_desktop_icons
        self._cached_visible: bool | None = None
        self._cached_at: float = 0.0

    def _icons_enabled(self) -> bool:
        now = time.monotonic()
        if self._cached_visible is None or (now - self._cached_at) >= 5.0:
            self._cached_visible = bool(self._detector())
            self._cached_at = now
        return self._cached_visible

    def read(self) -> tuple[bool, list[Rect]]:
        if not self._icons_enabled():
            return False, []
        return super().read()


class IconlessWaylandAdapter(GioMetadataAdapter):
    """Tiling Wayland WMs (Hyprland/Sway/Niri/River): report no icons unless a daemon runs."""

    name = "wayland-auto"

    def __init__(self, *args, daemon_check=None, **kwargs) -> None:
        super().__init__(*args, **kwargs)
        self._daemon_check = daemon_check or _has_running_icon_daemon
        self._cached_daemon: bool | None = None
        self._cached_at: float = 0.0

    def _has_daemon(self) -> bool:
        now = time.monotonic()
        if self._cached_daemon is None or (now - self._cached_at) >= 5.0:
            self._cached_daemon = bool(self._daemon_check())
            self._cached_at = now
        return self._cached_daemon

    def read(self) -> tuple[bool, list[Rect]]:
        if not self._has_daemon():
            return False, []
        return super().read()


def make_adapter(
    desktop: Path,
    scale: int,
    screen_w: int,
    screen_h: int,
    origin_x: int = 0,
    origin_y: int = 0,
    forced: str | None = None,
) -> IconAdapter:
    forced = (forced or os.environ.get("DESKLAND_ICON_ADAPTER") or "").lower()
    desktop_env = (
        os.environ.get("XDG_CURRENT_DESKTOP", "")
        + " "
        + os.environ.get("DESKTOP_SESSION", "")
    ).lower()
    common = dict(
        desktop=desktop,
        scale=scale,
        screen_w=screen_w,
        screen_h=screen_h,
        origin_x=origin_x,
        origin_y=origin_y,
    )
    if forced == "none":
        return NullIconAdapter(**common)
    if forced == "grid":
        return GridAdapter(**common)
    if forced == "gio":
        return GioMetadataAdapter(**common)
    if forced == "xfce" or "xfce" in desktop_env:
        return XfceAdapter(**common)
    if forced == "plasma" or "kde" in desktop_env or "plasma" in desktop_env:
        return PlasmaAdapter(**common)
    if forced == "gnome" or "gnome" in desktop_env:
        return GnomeIconAdapter(**common)
    if (
        any(wm in desktop_env for wm in _ICONLESS_WMS)
        or os.environ.get("HYPRLAND_INSTANCE_SIGNATURE")
        or os.environ.get("NIRI_SOCKET")
        or os.environ.get("SWAYSOCK")
    ):
        return IconlessWaylandAdapter(**common)
    return GioMetadataAdapter(**common)
