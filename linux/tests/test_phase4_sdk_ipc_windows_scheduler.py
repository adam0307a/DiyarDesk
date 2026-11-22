"""Phase 4 unit tests: Theme SDK (color + LivelyProperties), IPC/CLI, Window-Aware & Scheduler."""
from __future__ import annotations

import json
import random
import socket
import zipfile
from pathlib import Path

import pytest

from deskland_linux.config import SettingsStore
from deskland_linux.ctl import build_command_payload
from deskland_linux.importer import import_theme_file
from deskland_linux.ipc import IpcServer, send_ipc_command
from deskland_linux.scheduler import PlaylistController, compute_solar_phase
from deskland_linux.windows import (
    parse_hyprland_clients,
    parse_niri_windows,
    parse_sway_windows,
)


def test_import_lively_package_with_color_dropdown_and_slider(tmp_path: Path) -> None:
    pkg = tmp_path / "lively-neon.zip"
    lively_info = {
        "Title": "Neon Pulse",
        "Desc": "Imported Lively shader",
        "FileName": "scene.html",
    }
    lively_props = {
        "accentColor": {"type": "color", "text": "Accent", "value": "#0af"},
        "speed": {"type": "slider", "text": "Speed", "min": 0.1, "max": 3.0, "step": 0.1, "value": 1.2},
        "mode": {"type": "dropdown", "text": "Mode", "items": ["Calm", "Storm"], "value": 1},
        "glow": {"type": "checkbox", "text": "Glow", "value": True},
        "resetBtn": {"type": "button", "text": "Reset"},
    }
    with zipfile.ZipFile(pkg, "w") as zf:
        zf.writestr("scene.html", "<!doctype html><title>Ignored</title><body>ok</body>")
        zf.writestr("LivelyInfo.json", json.dumps(lively_info))
        zf.writestr("LivelyProperties.json", json.dumps(lively_props))

    entry = import_theme_file(pkg, custom_dir=tmp_path / "custom")
    assert entry["name"] == "Neon Pulse"
    assert entry["desc"] == "Imported Lively shader"
    assert entry["entry"] == "scene.html"
    params = {p["key"]: p for g in entry["groups"] for p in g["params"]}
    assert "resetBtn" not in params
    assert params["accentColor"]["type"] == "color"
    assert params["accentColor"]["default"] == "#00aaff"
    assert params["accentColor"]["label"] == "Accent"
    assert params["mode"]["options"] == ["Calm", "Storm"]
    assert params["mode"]["default"] == 1
    assert params["glow"]["default"] is True
    assert params["speed"]["default"] == pytest.approx(1.2)


def test_ctl_argument_parsing() -> None:
    payload, as_json, sock = build_command_payload(["--json", "setprop", "speed", "1.5", "--theme", "meadow"])
    assert as_json is True
    assert sock is None
    assert payload == {"cmd": "setprop", "key": "speed", "value": 1.5, "theme": "meadow"}

    p2, _, _ = build_command_payload(["playlist", "on", "--interval", "45"])
    assert p2 == {"cmd": "playlist", "action": "on", "interval": 45}

    p3, _, _ = build_command_payload(["solar", "on"])
    assert p3 == {"cmd": "solar", "state": "on"}


def test_ipc_server_roundtrip(tmp_path: Path) -> None:
    if not hasattr(socket, "AF_UNIX"):
        pytest.skip("AF_UNIX not supported on this platform")
    sock_path = tmp_path / "diyardesk-test.sock"
    seen: list[dict] = []

    def handler(req: dict) -> dict:
        seen.append(req)
        if req.get("cmd") == "status":
            return {"ok": True, "theme": "istanbul", "paused": False}
        return {"ok": False, "error": "unknown"}

    server = IpcServer(handler, socket_path=sock_path)
    if not server.start():
        pytest.skip("AF_UNIX bind unavailable in this environment")
    try:
        resp = send_ipc_command({"cmd": "status"}, socket_path=sock_path)
        assert resp["ok"] is True
        assert resp["theme"] == "istanbul"
        assert seen == [{"cmd": "status"}]
    finally:
        server.stop()
    assert not sock_path.exists()


def test_window_aware_parsers_hyprland_sway_niri() -> None:
    hypr = json.dumps([
        {"address": "0x1a", "mapped": True, "hidden": False, "workspace": {"id": 1}, "at": [100, 80], "size": [600, 400], "class": "kitty"},
        {"address": "0x1b", "mapped": True, "hidden": False, "workspace": {"id": 2}, "at": [0, 0], "size": [500, 500], "class": "firefox"},
        {"address": "0x1c", "mapped": True, "hidden": False, "workspace": {"id": 1}, "at": [0, 0], "size": [300, 300], "class": "DiyarDesk"},
    ])
    hw = parse_hyprland_clients(hypr, active_workspace_id=1)
    assert len(hw) == 1
    assert hw[0].as_wire_list(scale=1.5) == [0x1A, 150, 120, 1050, 720]

    sway = json.dumps({
        "type": "root",
        "nodes": [
            {"type": "con", "id": 42, "pid": 1234, "visible": True, "app_id": "alacritty", "rect": {"x": 20, "y": 40, "width": 800, "height": 500}},
            {"type": "con", "id": 43, "pid": 1235, "visible": False, "app_id": "hidden", "rect": {"x": 0, "y": 0, "width": 500, "height": 500}},
        ],
    })
    sw = parse_sway_windows(sway)
    assert len(sw) == 1
    assert sw[0].as_icon_rect() == [20, 40, 820, 540]

    niri = json.dumps([
        {"id": 7, "app_id": "foot", "layout": {"tile_size": [900.0, 600.0], "tile_pos_in_workspace_view": [50.0, 30.0]}},
        {"id": 8, "app_id": "deskland-linux", "layout": {"tile_size": [400.0, 400.0], "tile_pos_in_workspace_view": [0.0, 0.0]}},
    ])
    nw = parse_niri_windows(niri)
    assert len(nw) == 1
    assert nw[0].wid == 7
    assert nw[0].as_icon_rect() == [50, 30, 950, 630]


def test_solar_phase_and_playlist_controller(tmp_path: Path) -> None:
    dawn = compute_solar_phase(hour=6.5, dark_mode=False, solar_sync=True)
    noon = compute_solar_phase(hour=12.0, dark_mode=False, solar_sync=True)
    dusk = compute_solar_phase(hour=18.5, dark_mode=True, solar_sync=True)
    night = compute_solar_phase(hour=23.0, dark_mode=True, solar_sync=True)

    assert dawn["phase"] == "dawn"
    assert noon["phase"] == "day" and noon["daylight"] == pytest.approx(1.0)
    assert dusk["phase"] == "dusk" and dusk["darkMode"] is True
    assert night["phase"] == "night" and night["daylight"] == 0.0

    pl = PlaylistController(interval_minutes=15, themes=["meadow", "istanbul"])
    pl.mark_switched(now=100.0)
    assert pl.should_rotate(now=100.0 + 10 * 60) is False
    assert pl.should_rotate(now=100.0 + 16 * 60) is True
    assert pl.next_theme(["meadow", "istanbul", "aurora"], "meadow") == "istanbul"
    assert pl.next_theme(["meadow", "istanbul", "aurora"], "istanbul") == "meadow"
    pick = PlaylistController.pick_random(["meadow", "istanbul"], "meadow", rng=random.Random(1))
    assert pick == "istanbul"

    # SettingsStore persistence roundtrip for Phase 4 fields
    st = SettingsStore(directory=tmp_path)
    st.solar_sync = True
    st.window_aware = True
    st.playlist_enabled = True
    st.playlist_interval_min = 45
    st.playlist_themes = ["meadow", "aurora"]
    st.save()

    st2 = SettingsStore(directory=tmp_path)
    st2.load()
    assert st2.solar_sync is True
    assert st2.window_aware is True
    assert st2.playlist_enabled is True
    assert st2.playlist_interval_min == 45
    assert st2.playlist_themes == ["meadow", "aurora"]
