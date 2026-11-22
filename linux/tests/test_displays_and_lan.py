"""Unit tests for multi-monitor geometry, fractional scaling and cloud-free LAN pairing."""

from __future__ import annotations

import io
import json
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

from deskland_linux.config import SettingsStore
from deskland_linux.displays import (
    MonitorInfo,
    compute_span_rect,
    filter_icons_for_monitor,
    parse_hyprland_monitors,
    parse_sway_outputs,
    parse_xrandr_monitors,
)
from deskland_linux.importer import import_theme_file
from deskland_linux.lan_sync import LanPairingServer, MAX_FAILED_ATTEMPTS


def test_parse_hyprland_sway_xrandr_and_fractional_scaling() -> None:
    hypr_json = json.dumps(
        [
            {"name": "eDP-1", "x": 0, "y": 0, "width": 2560, "height": 1600, "scale": 1.25, "focused": True, "transform": 0},
            {"name": "DP-1", "x": 2048, "y": 0, "width": 1920, "height": 1080, "scale": 1.0, "focused": False, "transform": 1},
        ]
    )
    mons = parse_hyprland_monitors(hypr_json)
    assert len(mons) == 2
    assert mons[0].width == 2048 and mons[0].height == 1280 and mons[0].scale == 1.25
    assert mons[0].phys_width == 2560 and mons[0].phys_height == 1600
    assert mons[1].width == 1080 and mons[1].height == 1920  # 90-degree rotated

    sway_json = json.dumps(
        [
            {"name": "HDMI-A-1", "active": True, "focused": True, "scale": 1.5, "rect": {"x": 0, "y": 0, "width": 2560, "height": 1440}},
            {"name": "DP-2", "active": False, "rect": {"x": 2560, "y": 0, "width": 1920, "height": 1080}},
        ]
    )
    sway = parse_sway_outputs(sway_json)
    assert len(sway) == 1
    assert sway[0].scale == 1.5 and sway[0].phys_width == 3840

    xrandr_text = (
        "Monitors: 2\n"
        " 0: +*eDP-1 1920/344x1080/194+0+0  eDP-1\n"
        " 1: +HDMI-1 2560/597x1440/336+1920+0  HDMI-1\n"
    )
    xmons = parse_xrandr_monitors(xrandr_text)
    assert len(xmons) == 2
    assert xmons[0].primary is True and xmons[1].x == 1920

    span = compute_span_rect(xmons)
    assert (span.x, span.y, span.width, span.height) == (0, 0, 4480, 1440)


def test_filter_icons_for_monitor_and_display_mode_roundtrip(tmp_path: Path) -> None:
    sec = MonitorInfo("HDMI-1", 1920, 0, 1920, 1080, 1.0, False)
    rects = [[100, 100, 160, 180], [2000, 120, 2080, 200]]
    sliced = filter_icons_for_monitor(rects, sec)
    assert sliced == [[80, 120, 160, 200]]

    st = SettingsStore(tmp_path)
    st.display_mode = "independent"
    st.save()
    reloaded = SettingsStore(tmp_path)
    reloaded.load()
    assert reloaded.display_mode == "independent"


def test_lan_pairing_server_auth_transfer_and_lockout(tmp_path: Path) -> None:
    custom_dir = tmp_path / "custom"
    src_html = tmp_path / "aurora-custom.html"
    src_html.write_text("<!doctype html><html><title>LAN Test</title><body>hi</body></html>", encoding="utf-8")
    imported = import_theme_file(src_html, custom_dir=custom_dir)

    srv = LanPairingServer(
        custom_dir=custom_dir,
        state_provider=lambda: {"theme": imported["id"], "params": {"speed": 1.25}},
        bind_host="127.0.0.1",
    )
    info = srv.start()
    try:
        assert info["active"] is True
        assert len(info["pin"]) == 4 and info["pin"].isdigit()
        base = f"http://127.0.0.1:{info['port']}"

        # 1. Valid PIN fetches info
        req = urllib.request.Request(f"{base}/pair/info", headers={"X-Diyar-Pin": info["pin"]})
        with urllib.request.urlopen(req) as resp:
            payload = json.loads(resp.read().decode("utf-8"))
        assert payload["activeTheme"] == imported["id"]
        assert payload["params"]["speed"] == 1.25
        assert payload["customThemes"][0]["id"] == imported["id"]

        # 2. Valid PIN downloads .deskland archive of custom theme
        dl_req = urllib.request.Request(
            f"{base}/pair/theme/{imported['id']}.deskland",
            headers={"X-Diyar-Pin": info["pin"]},
        )
        with urllib.request.urlopen(dl_req) as resp:
            raw_zip = resp.read()
        with zipfile.ZipFile(io.BytesIO(raw_zip), "r") as zf:
            assert "index.html" in zf.namelist()

        # 3. Wrong PIN gets 403 and eventually triggers lockout (410)
        for _ in range(MAX_FAILED_ATTEMPTS):
            try:
                urllib.request.urlopen(f"{base}/pair/info?pin=0000")
            except urllib.error.HTTPError as err:
                assert err.code == 403
            else:
                raise AssertionError("Expected 403 for invalid PIN")

        try:
            urllib.request.urlopen(f"{base}/pair/info?pin={info['pin']}")
        except urllib.error.HTTPError as err:
            assert err.code == 410
        else:
            raise AssertionError("Expected 410 lockout after too many failed attempts")
    finally:
        srv.stop()
