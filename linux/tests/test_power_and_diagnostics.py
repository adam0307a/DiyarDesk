"""Unit tests for Linux power management, DMA-BUF fallback, log rotation and diagnostics."""

from __future__ import annotations

import json
from pathlib import Path

from deskland_linux.config import Log, SettingsStore
from deskland_linux.diagnostics import collect_diagnostics, export_diagnostics, sanitize_diagnostic_text
from deskland_linux.gpu import configure_single_gpu_env, should_disable_dmabuf
from deskland_linux.power import (
    PowerMonitor,
    PowerState,
    detect_on_battery,
    detect_power_saver,
    detect_reduced_motion,
    detect_session_locked,
)


def test_detect_on_battery_sysfs(tmp_path: Path) -> None:
    ps = tmp_path / "power_supply"
    (ps / "AC0").mkdir(parents=True)
    (ps / "AC0" / "type").write_text("Mains\n", encoding="utf-8")
    (ps / "AC0" / "online").write_text("1\n", encoding="utf-8")
    (ps / "BAT0").mkdir(parents=True)
    (ps / "BAT0" / "type").write_text("Battery\n", encoding="utf-8")
    (ps / "BAT0" / "status").write_text("Charging\n", encoding="utf-8")
    assert detect_on_battery(ps) is False

    (ps / "AC0" / "online").write_text("0\n", encoding="utf-8")
    (ps / "BAT0" / "status").write_text("Discharging\n", encoding="utf-8")
    assert detect_on_battery(ps) is True


def test_detect_power_saver_and_reduced_motion(tmp_path: Path) -> None:
    prof = tmp_path / "platform_profile"
    prof.write_text("balanced\n", encoding="utf-8")
    assert detect_power_saver(prof, run_cmd=lambda _c, _t: "balanced") is False
    assert detect_power_saver(prof, run_cmd=lambda _c, _t: "power-saver") is True
    prof.write_text("low-power\n", encoding="utf-8")
    assert detect_power_saver(prof, run_cmd=lambda _c, _t: None) is True

    gtk_ini = tmp_path / "settings.ini"
    gtk_ini.write_text("[Settings]\ngtk-enable-animations = false\n", encoding="utf-8")
    assert detect_reduced_motion([gtk_ini], run_cmd=lambda _c, _t: None) is True

    gtk_ini.write_text("[Settings]\ngtk-enable-animations = 1\n", encoding="utf-8")
    assert detect_reduced_motion([gtk_ini], run_cmd=lambda _c, _t: None) is False

    assert detect_reduced_motion([tmp_path / "missing.ini"], run_cmd=lambda _c, _t: "false") is True


def test_detect_session_locked_and_monitor_cache(tmp_path: Path) -> None:
    calls: list[list[str]] = []

    def fake_run(cmd: list[str], _timeout: float) -> str | None:
        calls.append(cmd)
        if cmd[0] == "loginctl":
            return "yes"
        return None

    assert detect_session_locked({"XDG_SESSION_ID": "42"}, run_cmd=fake_run) is True
    assert any(c[0] == "loginctl" for c in calls)

    calls.clear()
    pm = PowerMonitor(
        interval_sec=10.0,
        power_supply_root=tmp_path / "ps",
        acpi_profile_path=tmp_path / "prof",
        gtk_ini_paths=[tmp_path / "missing.ini"],
        run_cmd=lambda _c, _t: None,
    )
    s1 = pm.poll(now=100.0)
    s2 = pm.poll(now=102.0)
    assert s1 == s2 == PowerState()


def test_should_disable_dmabuf_nvidia_wayland_and_setting(tmp_path: Path) -> None:
    drm = tmp_path / "drm"
    (drm / "card0" / "device").mkdir(parents=True)
    (drm / "card0" / "device" / "vendor").write_text("0x10de\n", encoding="utf-8")
    (drm / "card0-DP-1").mkdir(parents=True)
    (drm / "card0-DP-1" / "status").write_text("connected\n", encoding="utf-8")

    # Wayland + NVIDIA -> disables DMABUF automatically
    assert should_disable_dmabuf({"WAYLAND_DISPLAY": "wayland-0"}, drm_root=drm) is True
    # Explicit user override respected
    assert should_disable_dmabuf({"WAYLAND_DISPLAY": "wayland-0", "WEBKIT_DISABLE_DMABUF_RENDERER": "0"}, drm_root=drm) is False
    assert should_disable_dmabuf({"WAYLAND_DISPLAY": "wayland-0", "DESKLAND_KEEP_DMABUF": "1"}, drm_root=drm) is False
    # X11 without setting -> stays enabled
    assert should_disable_dmabuf({"DISPLAY": ":0"}, drm_root=drm) is False
    # Explicit settings.json flag -> disables DMABUF
    settings_json = tmp_path / "settings.json"
    settings_json.write_text(json.dumps({"disableDmabuf": True}), encoding="utf-8")
    env: dict[str, str] = {"DISPLAY": ":0"}
    configure_single_gpu_env(environ=env, settings_path=settings_json, drm_root=tmp_path / "empty_drm", egl_dir=tmp_path / "empty_egl")
    assert env.get("WEBKIT_DISABLE_DMABUF_RENDERER") == "1"


def test_log_rotation_and_diagnostics_export(tmp_path: Path) -> None:
    log_file = tmp_path / "host.log"
    log = Log(log_file)
    log.MAX_BYTES = 64
    log.write("first line that is long enough to exceed sixty four bytes when written to disk")
    log.write("second line triggers rotation")
    backup = tmp_path / "host.log.1"
    assert backup.is_file()
    assert "first line" in backup.read_text(encoding="utf-8")
    assert "second line" in log_file.read_text(encoding="utf-8")

    store = SettingsStore(tmp_path)
    store.theme = "aurora"
    store.low_mode = True
    store.disable_dmabuf = True
    store.save()

    reloaded = SettingsStore(tmp_path)
    reloaded.load()
    assert reloaded.disable_dmabuf is True

    fake_home = tmp_path / "home_alice"
    fake_home.mkdir()
    log.write(f"opened {fake_home / 'secret.zip'}")
    report = collect_diagnostics(
        settings=reloaded,
        backend="layer-shell",
        screen=(1920, 1080, 1),
        power_state=PowerState(locked=False, on_battery=True, power_saver=False, reduced_motion=True),
        log_file=log_file,
        home_dir=fake_home,
    )
    assert "Active Theme:    aurora" in report
    assert "on_battery=True" in report
    assert "reduced_motion=True" in report
    assert str(fake_home.resolve()) not in report
    assert sanitize_diagnostic_text(f"path={fake_home.resolve()}/x", home_dir=fake_home) == "path=~/x"

    exported = export_diagnostics(
        output_dir=tmp_path / "diag_out",
        settings=reloaded,
        backend="layer-shell",
        log_file=log_file,
        home_dir=fake_home,
    )
    assert exported.is_file()
    assert "DiyarDesk Diagnostics Report" in exported.read_text(encoding="utf-8")
