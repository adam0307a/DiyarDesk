"""Multi-monitor geometry, fractional scaling and per-display icon slicing."""

from __future__ import annotations

import json
import re
import shutil
import subprocess
from dataclasses import dataclass

DISPLAY_MODES = ("primary", "independent", "span", "duplicate")


@dataclass(frozen=True)
class MonitorInfo:
    name: str
    x: int
    y: int
    width: int
    height: int
    scale: float = 1.0
    primary: bool = False

    @property
    def phys_width(self) -> int:
        return max(1, int(round(self.width * self.scale)))

    @property
    def phys_height(self) -> int:
        return max(1, int(round(self.height * self.scale)))

    @property
    def phys_x(self) -> int:
        return int(round(self.x * self.scale))

    @property
    def phys_y(self) -> int:
        return int(round(self.y * self.scale))


def parse_hyprland_monitors(raw_json: str) -> list[MonitorInfo]:
    """Parse ``hyprctl monitors -j`` preserving fractional scale values."""
    try:
        data = json.loads(raw_json)
    except ValueError:
        return []
    if not isinstance(data, list):
        return []
    out: list[MonitorInfo] = []
    for idx, item in enumerate(data):
        if not isinstance(item, dict) or item.get("disabled"):
            continue
        try:
            scale = float(item.get("scale", 1.0) or 1.0)
            if scale <= 0:
                scale = 1.0
            raw_w = int(item.get("width", 0))
            raw_h = int(item.get("height", 0))
            transform = int(item.get("transform", 0))
            if transform % 2 == 1:
                raw_w, raw_h = raw_h, raw_w
            logical_w = max(1, int(round(raw_w / scale)))
            logical_h = max(1, int(round(raw_h / scale)))
            out.append(
                MonitorInfo(
                    name=str(item.get("name") or f"monitor-{idx}"),
                    x=int(item.get("x", 0)),
                    y=int(item.get("y", 0)),
                    width=logical_w,
                    height=logical_h,
                    scale=scale,
                    primary=bool(item.get("focused") or idx == 0),
                )
            )
        except (TypeError, ValueError):
            continue
    return out


def parse_sway_outputs(raw_json: str) -> list[MonitorInfo]:
    """Parse ``swaymsg -r -t get_outputs`` preserving fractional scale values."""
    try:
        data = json.loads(raw_json)
    except ValueError:
        return []
    if not isinstance(data, list):
        return []
    out: list[MonitorInfo] = []
    for idx, item in enumerate(data):
        if not isinstance(item, dict) or not item.get("active"):
            continue
        rect = item.get("rect")
        if not isinstance(rect, dict):
            continue
        try:
            scale = float(item.get("scale", 1.0) or 1.0)
            if scale <= 0:
                scale = 1.0
            out.append(
                MonitorInfo(
                    name=str(item.get("name") or f"output-{idx}"),
                    x=int(rect.get("x", 0)),
                    y=int(rect.get("y", 0)),
                    width=max(1, int(rect.get("width", 1920))),
                    height=max(1, int(rect.get("height", 1080))),
                    scale=scale,
                    primary=bool(item.get("focused") or idx == 0),
                )
            )
        except (TypeError, ValueError):
            continue
    return out


_XRANDR_LINE = re.compile(
    r"^\s*\d+:\s+(\+?)(\*?)(\S+)\s+(\d+)(?:/\d+)?x(\d+)(?:/\d+)?\+(\-?\d+)\+(\-?\d+)"
)


def parse_xrandr_monitors(raw_text: str) -> list[MonitorInfo]:
    """Parse ``xrandr --listmonitors`` output."""
    out: list[MonitorInfo] = []
    for line in (raw_text or "").splitlines():
        m = _XRANDR_LINE.match(line)
        if not m:
            continue
        is_primary = "*" in (m.group(1) + m.group(2))
        out.append(
            MonitorInfo(
                name=m.group(3),
                width=max(1, int(m.group(4))),
                height=max(1, int(m.group(5))),
                x=int(m.group(6)),
                y=int(m.group(7)),
                scale=1.0,
                primary=is_primary,
            )
        )
    if out and not any(mon.primary for mon in out):
        first = out[0]
        out[0] = MonitorInfo(first.name, first.x, first.y, first.width, first.height, first.scale, True)
    return out


def detect_compositor_monitors() -> list[MonitorInfo]:
    """Try Hyprland, Sway, then xrandr for accurate multi-monitor + fractional scale info."""
    for cmd, parser in (
        (["hyprctl", "monitors", "-j"], parse_hyprland_monitors),
        (["swaymsg", "-r", "-t", "get_outputs"], parse_sway_outputs),
        (["xrandr", "--listmonitors"], parse_xrandr_monitors),
    ):
        if not shutil.which(cmd[0]):
            continue
        try:
            raw = subprocess.check_output(cmd, stderr=subprocess.DEVNULL, timeout=1.0, text=True)
        except (OSError, subprocess.SubprocessError):
            continue
        mons = parser(raw)
        if mons:
            return mons
    return []


def compute_span_rect(monitors: list[MonitorInfo]) -> MonitorInfo:
    """Return a unified bounding box spanning all active monitors."""
    if not monitors:
        return MonitorInfo("span", 0, 0, 1920, 1080, 1.0, True)
    min_x = min(m.x for m in monitors)
    min_y = min(m.y for m in monitors)
    max_x = max(m.x + m.width for m in monitors)
    max_y = max(m.y + m.height for m in monitors)
    max_scale = max(m.scale for m in monitors)
    return MonitorInfo(
        name="span",
        x=min_x,
        y=min_y,
        width=max(1, max_x - min_x),
        height=max(1, max_y - min_y),
        scale=max_scale,
        primary=True,
    )


def filter_icons_for_monitor(
    rects: list[list[int]],
    monitor: MonitorInfo,
) -> list[list[int]]:
    """Slice global physical icon rects [l, t, r, b] to those belonging to *monitor*."""
    mx0 = monitor.phys_x
    my0 = monitor.phys_y
    mx1 = mx0 + monitor.phys_width
    my1 = my0 + monitor.phys_height
    local: list[list[int]] = []
    for r in rects:
        if len(r) < 4:
            continue
        cx = (r[0] + r[2]) // 2
        cy = (r[1] + r[3]) // 2
        if mx0 <= cx < mx1 and my0 <= cy < my1:
            local.append([r[0] - mx0, r[1] - my0, r[2] - mx0, r[3] - my0])
    return local
