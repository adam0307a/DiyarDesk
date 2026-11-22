"""Smart power, session-lock, battery-saver and reduced-motion detection for Linux."""

from __future__ import annotations

import os
import shutil
import subprocess
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

CommandRunner = Callable[[list[str], float], str | None]


@dataclass(frozen=True)
class PowerState:
    locked: bool = False
    on_battery: bool = False
    power_saver: bool = False
    reduced_motion: bool = False


def _default_run(cmd: list[str], timeout: float = 0.5) -> str | None:
    if not cmd or not shutil.which(cmd[0]):
        return None
    try:
        out = subprocess.check_output(
            cmd,
            stderr=subprocess.DEVNULL,
            timeout=timeout,
            text=True,
        )
        return out.strip()
    except (OSError, subprocess.SubprocessError):
        return None


def detect_on_battery(power_supply_root: Path = Path("/sys/class/power_supply")) -> bool:
    """Return True when the system is running on battery power (pure sysfs read)."""
    if not power_supply_root.is_dir():
        return False
    saw_ac_online: bool | None = None
    saw_discharging = False
    try:
        entries = sorted(power_supply_root.iterdir())
    except OSError:
        return False
    for dev in entries:
        if not dev.is_dir():
            continue
        try:
            dev_type = (dev / "type").read_text(encoding="utf-8", errors="ignore").strip().lower()
        except OSError:
            continue
        if dev_type == "mains":
            try:
                online = (dev / "online").read_text(encoding="utf-8", errors="ignore").strip()
                if online == "1":
                    saw_ac_online = True
                elif online == "0" and saw_ac_online is None:
                    saw_ac_online = False
            except OSError:
                pass
        elif dev_type == "battery":
            try:
                status = (dev / "status").read_text(encoding="utf-8", errors="ignore").strip().lower()
                if status == "discharging":
                    saw_discharging = True
            except OSError:
                pass
    if saw_ac_online is True:
        return False
    if saw_discharging:
        return True
    return saw_ac_online is False


def detect_power_saver(
    acpi_profile_path: Path = Path("/sys/firmware/acpi/platform_profile"),
    run_cmd: CommandRunner | None = None,
) -> bool:
    """Return True when Linux power-saver / low-power profile is active."""
    try:
        if acpi_profile_path.is_file():
            prof = acpi_profile_path.read_text(encoding="utf-8", errors="ignore").strip().lower()
            if prof in ("low-power", "power-saver"):
                return True
    except OSError:
        pass
    runner = run_cmd or _default_run
    out = runner(["powerprofilesctl", "get"], 0.4)
    if out and out.strip().lower() == "power-saver":
        return True
    return False


def detect_reduced_motion(
    gtk_ini_paths: list[Path] | None = None,
    run_cmd: CommandRunner | None = None,
) -> bool:
    """Return True when desktop animations are disabled (Reduce Motion)."""
    candidates = gtk_ini_paths
    if candidates is None:
        cfg = Path(os.environ.get("XDG_CONFIG_HOME") or (Path.home() / ".config"))
        candidates = [cfg / "gtk-3.0" / "settings.ini", cfg / "gtk-4.0" / "settings.ini"]
    for ini in candidates:
        try:
            if not ini.is_file():
                continue
            for raw_line in ini.read_text(encoding="utf-8", errors="ignore").splitlines():
                line = raw_line.strip().lower()
                if not line.startswith("gtk-enable-animations"):
                    continue
                if "=" in line:
                    val = line.split("=", 1)[1].strip()
                    if val in ("0", "false", "no"):
                        return True
                    if val in ("1", "true", "yes"):
                        return False
        except OSError:
            continue
    runner = run_cmd or _default_run
    out = runner(["gsettings", "get", "org.gnome.desktop.interface", "enable-animations"], 0.4)
    if out and out.strip().lower() == "false":
        return True
    return False


def detect_session_locked(
    environ: dict[str, str] | None = None,
    run_cmd: CommandRunner | None = None,
) -> bool:
    """Return True when the active desktop session is locked."""
    env = os.environ if environ is None else environ
    runner = run_cmd or _default_run
    session_id = env.get("XDG_SESSION_ID")
    if session_id:
        out = runner(["loginctl", "show-session", session_id, "-p", "LockedHint", "--value"], 0.4)
        if out is not None:
            return out.strip().lower() == "yes"
    out = runner(
        [
            "gdbus",
            "call",
            "--session",
            "--dest",
            "org.freedesktop.ScreenSaver",
            "--object-path",
            "/org/freedesktop/ScreenSaver",
            "--method",
            "org.freedesktop.ScreenSaver.GetActive",
        ],
        0.4,
    )
    if out and "true" in out.lower():
        return True
    return False


class PowerMonitor:
    """Cached, low-overhead power & accessibility state poller."""

    def __init__(
        self,
        interval_sec: float = 3.0,
        power_supply_root: Path = Path("/sys/class/power_supply"),
        acpi_profile_path: Path = Path("/sys/firmware/acpi/platform_profile"),
        gtk_ini_paths: list[Path] | None = None,
        run_cmd: CommandRunner | None = None,
    ) -> None:
        self.interval_sec = max(0.5, float(interval_sec))
        self._power_supply_root = power_supply_root
        self._acpi_profile_path = acpi_profile_path
        self._gtk_ini_paths = gtk_ini_paths
        self._run_cmd = run_cmd
        self._last_poll = 0.0
        self._state = PowerState()

    @property
    def state(self) -> PowerState:
        return self._state

    def poll(self, now: float | None = None, force: bool = False) -> PowerState:
        ts = time.monotonic() if now is None else now
        if not force and self._last_poll > 0.0 and (ts - self._last_poll) < self.interval_sec:
            return self._state
        self._last_poll = ts
        self._state = PowerState(
            locked=detect_session_locked(run_cmd=self._run_cmd),
            on_battery=detect_on_battery(self._power_supply_root),
            power_saver=detect_power_saver(self._acpi_profile_path, run_cmd=self._run_cmd),
            reduced_motion=detect_reduced_motion(self._gtk_ini_paths, run_cmd=self._run_cmd),
        )
        return self._state
