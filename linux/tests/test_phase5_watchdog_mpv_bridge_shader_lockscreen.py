"""Phase 5 unit tests: Watchdog, mpv backend, System/Audio bridge, GLSL import, Lockscreen, Panels/Workspaces & 8-lang i18n."""
from __future__ import annotations

import json
import math
import struct
from pathlib import Path

from deskland_linux.config import SettingsStore
from deskland_linux.ctl import build_command_payload
from deskland_linux.importer import import_theme_file
from deskland_linux.lockscreen import make_blurred_lockscreen
from deskland_linux.mpv_player import (
    build_mpv_command,
    clamp_video_speed,
    find_theme_video_file,
    map_brightness_to_mpv,
    select_video_backend,
)
from deskland_linux.sysbridge import (
    SystemStatsSampler,
    compute_pcm16_spectrum,
    parse_meminfo_pct,
    parse_proc_stat_cpu,
    read_battery_info,
)
from deskland_linux.watchdog import (
    ProcessSample,
    ResourceWatchdog,
    parse_proc_stat_ticks,
    parse_proc_status_rss_mb,
)
from deskland_linux.windows import parse_hyprland_layers, parse_workspace_id


def test_resource_watchdog_ram_and_cpu_strikes() -> None:
    assert parse_proc_status_rss_mb("Name:\tWebKitWebProcess\nVmRSS:\t204800 kB\n") == 200.0
    stat_line = "1234 (WebKitWebProc) R 1 2 3 4 5 6 7 8 9 10 150 50 0 0 20 0 1 0 100"
    assert parse_proc_stat_ticks(stat_line) == 200

    wd = ResourceWatchdog(max_rss_mb=1024.0, max_cpu_pct=90.0, strikes_needed=3, clk_tck=100)
    # Normal load -> never trips
    assert wd.evaluate([ProcessSample(10, 300.0, 100)], 10.0) is None
    assert wd.evaluate([ProcessSample(10, 310.0, 120)], 20.0) is None
    # Sustained RSS breach -> trips on 3rd strike
    assert wd.evaluate([ProcessSample(10, 1200.0, 130)], 30.0) is None
    assert wd.evaluate([ProcessSample(10, 1300.0, 140)], 40.0) is None
    trip = wd.evaluate([ProcessSample(10, 1400.0, 150)], 50.0)
    assert trip is not None and "memory_limit" in trip

    # Sustained CPU spin -> 1000 ticks over 10s = 100% CPU
    wd.reset()
    assert wd.evaluate([ProcessSample(20, 200.0, 1000)], 100.0) is None
    assert wd.evaluate([ProcessSample(20, 200.0, 2000)], 110.0) is None
    assert wd.evaluate([ProcessSample(20, 200.0, 3000)], 120.0) is None
    trip_cpu = wd.evaluate([ProcessSample(20, 200.0, 4000)], 130.0)
    assert trip_cpu is not None and "cpu_limit" in trip_cpu


def test_mpv_command_and_video_discovery(tmp_path: Path) -> None:
    tdir = tmp_path / "custom-vid"
    tdir.mkdir()
    vid = tdir / "media.mp4"
    vid.write_bytes(b"fake-mp4")
    assert find_theme_video_file(tdir) == vid
    cmd = build_mpv_command(vid, wid=12345, speed=1.5, brightness=1.2)
    assert cmd[0] == "mpv"
    assert "--hwdec=auto-safe" in cmd
    assert "--wid=12345" in cmd
    assert "--speed=1.50" in cmd
    assert str(vid) == cmd[-1]

    assert clamp_video_speed(0.05) == 0.25
    assert clamp_video_speed(5.0) == 2.0
    assert map_brightness_to_mpv(1.0) == 0
    assert map_brightness_to_mpv(0.4) == -36

    # Wayland & X11 zero-WebKitGTK backend selection
    assert select_video_backend("layer-shell", has_libmpv=True, has_gst_gtk=True, has_mpv_cli=True) == "libmpv-gl"
    assert select_video_backend("layer-shell", has_libmpv=False, has_gst_gtk=True, has_mpv_cli=True) == "gstreamer"
    assert select_video_backend("layer-shell", has_libmpv=False, has_gst_gtk=False, has_mpv_cli=True) is None
    assert select_video_backend("x11", has_libmpv=False, has_gst_gtk=False, has_mpv_cli=True) == "mpv-wid"
    assert select_video_backend("static", has_libmpv=True, has_gst_gtk=True, has_mpv_cli=True) is None
    assert select_video_backend("plasma-plugin", has_libmpv=True, has_gst_gtk=True, has_mpv_cli=True) is None


def test_system_stats_and_audio_spectrum(tmp_path: Path) -> None:
    stat_file = tmp_path / "stat"
    mem_file = tmp_path / "meminfo"
    pwr_dir = tmp_path / "power"
    bat = pwr_dir / "BAT0"
    bat.mkdir(parents=True)
    (bat / "type").write_text("Battery\n", encoding="utf-8")
    (bat / "capacity").write_text("64\n", encoding="utf-8")
    (bat / "status").write_text("Discharging\n", encoding="utf-8")

    stat_file.write_text("cpu  100 0 100 800 0 0 0 0\n", encoding="utf-8")
    mem_file.write_text("MemTotal:       1000000 kB\nMemAvailable:    250000 kB\n", encoding="utf-8")
    assert parse_proc_stat_cpu(stat_file.read_text(encoding="utf-8")) == (800, 1000)
    assert parse_meminfo_pct(mem_file.read_text(encoding="utf-8")) == 75.0
    assert read_battery_info(pwr_dir) == (64, False)

    sampler = SystemStatsSampler(proc_stat=stat_file, proc_meminfo=mem_file, power_root=pwr_dir)
    first = sampler.sample()
    assert first["mem"] == 75.0 and first["battery"] == 64 and first["charging"] is False
    stat_file.write_text("cpu  150 0 150 900 0 0 0 0\n", encoding="utf-8")
    second = sampler.sample()
    assert second["cpu"] == 50.0

    # Silence vs sine wave spectrum
    rms0, bands0 = compute_pcm16_spectrum(b"\x00\x00" * 256)
    assert rms0 == 0.0 and all(b == 0.0 for b in bands0)

    sine = struct.pack("<256h", *[int(16000 * math.sin(2 * math.pi * 8 * i / 256)) for i in range(256)])
    rms1, bands1 = compute_pcm16_spectrum(sine)
    assert rms1 > 0.2 and max(bands1) > 0.2


def test_single_glsl_shader_import(tmp_path: Path) -> None:
    shader = tmp_path / "plasma_wave.frag"
    shader.write_text(
        "void mainImage(out vec4 fragColor, in vec2 fragCoord) {\n"
        "  vec2 uv = fragCoord / u_resolution;\n"
        "  fragColor = vec4(uv, 0.5 + 0.5 * sin(u_time), 1.0);\n"
        "}\n",
        encoding="utf-8",
    )
    entry = import_theme_file(shader, custom_dir=tmp_path / "custom")
    assert entry["name"] == "plasma wave"
    assert entry["adaptive"] is True
    assert entry["groups"][0]["name"] == "Shader"
    wrapped = (tmp_path / "custom" / entry["id"] / "index.html").read_text(encoding="utf-8")
    assert "uniform vec4 u_icons[32];" in wrapped
    assert "desklandSnapshot" in wrapped


def test_lockscreen_and_panel_workspace_parsers(tmp_path: Path) -> None:
    src = tmp_path / "frame.png"
    src.write_bytes(b"\x89PNG\r\n\x1a\nfake")
    out = make_blurred_lockscreen(src, dest_png=tmp_path / "lockscreen.png")
    assert out.is_file() and out.stat().st_size > 0

    layers_json = json.dumps({
        "DP-1": {
            "levels": {
                "0": [{"namespace": "diyardesk", "x": 0, "y": 0, "w": 1920, "h": 1080}],
                "2": [{"namespace": "waybar", "x": 0, "y": 0, "w": 1920, "h": 34}],
            }
        }
    })
    panels = parse_hyprland_layers(layers_json)
    assert len(panels) == 1
    assert panels[0].name == "waybar"
    assert panels[0].as_rect(scale=1.5) == [0, 0, 2880, 51]

    assert parse_workspace_id('{"id": 3}', "hyprland") == 3
    assert parse_workspace_id('[{"num": 1, "focused": false}, {"num": 4, "focused": true}]', "sway") == 4
    assert parse_workspace_id('[{"idx": 2, "is_active": true}]', "niri") == 2


def test_phase5_settings_ctl_and_all_8_i18n_parity(tmp_path: Path) -> None:
    st = SettingsStore(directory=tmp_path)
    st.system_bridge = True
    st.prefer_mpv = True
    st.save()
    st2 = SettingsStore(directory=tmp_path)
    st2.load()
    assert st2.system_bridge is True and st2.prefer_mpv is True

    p1, _, _ = build_command_payload(["system-bridge", "on"])
    p2, _, _ = build_command_payload(["mpv", "off"])
    p3, _, _ = build_command_payload(["lockscreen-sync"])
    assert p1 == {"cmd": "system-bridge", "state": "on"}
    assert p2 == {"cmd": "mpv", "state": "off"}
    assert p3 == {"cmd": "lockscreen-sync"}

    i18n_dir = Path(__file__).resolve().parents[2] / "prototype" / "i18n"
    en_keys = set(json.loads((i18n_dir / "en.json").read_text(encoding="utf-8"))["ui"])
    for code in ("en", "tr", "zh-CN", "zh-TW", "de", "fr", "ja", "ko"):
        keys = set(json.loads((i18n_dir / f"{code}.json").read_text(encoding="utf-8"))["ui"])
        assert not (en_keys - keys), f"{code}.json missing keys: {en_keys - keys}"
