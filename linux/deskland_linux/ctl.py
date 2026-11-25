"""Command-line controller (`diyardesk-ctl`) for scripting & keybinding DiyarDesk."""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from .ipc import send_ipc_command


def _parse_value(raw: str) -> Any:
    low = raw.strip().lower()
    if low == "true":
        return True
    if low == "false":
        return False
    try:
        if "." in raw:
            return float(raw)
        return int(raw)
    except ValueError:
        return raw


def build_command_payload(argv: list[str]) -> tuple[dict[str, Any], bool, Path | None]:
    parser = argparse.ArgumentParser(
        prog="diyardesk-ctl",
        description="Local CLI controller for DiyarDesk (scripting, Waybar/Polybar, Hyprland/Sway/niri keybinds).",
    )
    parser.add_argument("--socket", type=Path, default=None, help="Override IPC socket path")
    parser.add_argument("--json", action="store_true", dest="as_json", help="Print raw JSON response")

    sub = parser.add_subparsers(dest="cmd", required=True)
    sub.add_parser("status", help="Show active theme, pause state, display mode, and playlist status")
    p_setwp = sub.add_parser("setwp", help="Switch active wallpaper theme by id")
    p_setwp.add_argument("theme", help="Theme id (e.g. meadow, istanbul, aurora)")
    p_prop = sub.add_parser("setprop", help="Set a parameter value on the active (or specified) theme")
    p_prop.add_argument("key", help="Parameter key")
    p_prop.add_argument("value", help="Parameter value (number, bool, #hex, or string)")
    p_prop.add_argument("--theme", default="", help="Optional theme id (defaults to active theme)")
    sub.add_parser("pause", help="Pause wallpaper animation")
    sub.add_parser("play", help="Resume wallpaper animation")
    sub.add_parser("toggle", help="Toggle pause/play state")
    sub.add_parser("random", help="Switch to a random ready theme")
    p_low = sub.add_parser("lowmode", help="Toggle or set low-power mode")
    p_low.add_argument("state", choices=["on", "off", "toggle"], nargs="?", default="toggle")
    p_solar = sub.add_parser("solar", help="Toggle or set solar day/night sync")
    p_solar.add_argument("state", choices=["on", "off", "toggle"], nargs="?", default="toggle")
    p_win = sub.add_parser("window-aware", help="Toggle or set window-aware scenery")
    p_win.add_argument("state", choices=["on", "off", "toggle"], nargs="?", default="toggle")
    p_pl = sub.add_parser("playlist", help="Control local theme playlist rotation")
    p_pl.add_argument("action", choices=["on", "off", "next", "status"], default="status", nargs="?")
    p_pl.add_argument("--interval", type=int, default=0, help="Rotation interval in minutes")
    p_sb = sub.add_parser("system-bridge", help="Toggle or set opt-in local audio & CPU/RAM/battery bridge")
    p_sb.add_argument("state", choices=["on", "off", "toggle"], nargs="?", default="toggle")
    p_mpv = sub.add_parser("mpv", help="Toggle or set hardware-accelerated mpv video backend")
    p_mpv.add_argument("state", choices=["on", "off", "toggle"], nargs="?", default="toggle")
    p_gl = sub.add_parser("rust-gl", help="Toggle or set native Rust OpenGL 3.3 wallpaper engine")
    p_gl.add_argument("state", choices=["on", "off", "toggle"], nargs="?", default="toggle")
    p_vi = sub.add_parser(
        "vicons", help="Manage virtual desktop icons (add, shuffle, clear, rename, list, mode)"
    )
    p_vi.add_argument(
        "action",
        choices=["add", "shuffle", "clear", "rename", "list", "mode", "status"],
        nargs="?",
        default="status",
    )
    p_vi.add_argument(
        "args",
        nargs="*",
        help="Count for 'add', mode (auto|always|off) for 'mode', or '<index> <new name>' for 'rename'",
    )
    sub.add_parser("lockscreen-sync", help="Render blurred static lockscreen image from active wallpaper")
    sub.add_parser("reload", help="Reload wallpaper renderer")
    sub.add_parser("exit", help="Quit DiyarDesk cleanly")

    args = parser.parse_args(argv)
    payload: dict[str, Any] = {"cmd": args.cmd}
    if args.cmd == "setwp":
        payload["theme"] = args.theme
    elif args.cmd == "setprop":
        payload["key"] = args.key
        payload["value"] = _parse_value(args.value)
        if args.theme:
            payload["theme"] = args.theme
    elif args.cmd in ("lowmode", "solar", "window-aware", "system-bridge", "mpv", "rust-gl"):
        payload["state"] = args.state
    elif args.cmd == "vicons":
        payload["action"] = args.action
        extra = list(args.args or [])
        if args.action == "add" and extra:
            try:
                payload["count"] = max(1, int(extra[0]))
            except ValueError:
                payload["count"] = 1
        elif args.action == "mode" and extra:
            payload["mode"] = extra[0]
        elif args.action == "rename":
            if len(extra) < 2:
                parser.error("vicons rename requires <index> <new_name>")
            try:
                payload["index"] = int(extra[0])
            except ValueError:
                parser.error(f"invalid icon index: {extra[0]}")
            payload["name"] = " ".join(extra[1:]).strip()
    elif args.cmd == "playlist":
        payload["action"] = args.action
        if args.interval > 0:
            payload["interval"] = args.interval
    return payload, bool(args.as_json), args.socket


def main(argv: list[str] | None = None) -> int:
    payload, as_json, sock_path = build_command_payload(sys.argv[1:] if argv is None else argv)
    resp = send_ipc_command(payload, socket_path=sock_path)
    if as_json:
        print(json.dumps(resp, ensure_ascii=False))
    else:
        if resp.get("ok"):
            if "theme" in resp and payload["cmd"] == "status":
                print(
                    f"theme={resp.get('theme')} paused={resp.get('paused')} "
                    f"lowMode={resp.get('lowMode')} rustOpenGl={resp.get('rustOpenGl')} "
                    f"displayMode={resp.get('displayMode')} "
                    f"solarSync={resp.get('solarSync')} playlist={resp.get('playlistEnabled')}"
                )
            else:
                print(resp.get("message") or "ok")
        else:
            print(f"error: {resp.get('error', 'unknown error')}", file=sys.stderr)
    return 0 if resp.get("ok") else 1


if __name__ == "__main__":
    raise SystemExit(main())
