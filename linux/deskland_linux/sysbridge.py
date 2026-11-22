"""Opt-in local System Stats (CPU/RAM/Battery) & Audio Reactive Spectrum bridge.

100% offline, default OFF (opt-in only). Pure Python — zero external dependencies.
"""
from __future__ import annotations

import math
import shutil
import struct
import subprocess
import threading
from pathlib import Path
from typing import Any


def parse_proc_stat_cpu(text: str) -> tuple[int, int] | None:
    for line in text.splitlines():
        if line.startswith("cpu "):
            parts = line.split()[1:]
            vals = [int(p) for p in parts[:8] if p.isdigit()]
            if len(vals) >= 4:
                idle = vals[3] + (vals[4] if len(vals) > 4 else 0)
                total = sum(vals)
                return idle, total
    return None


def parse_meminfo_pct(text: str) -> float:
    total = 0
    avail = 0
    for line in text.splitlines():
        if line.startswith("MemTotal:"):
            parts = line.split()
            if len(parts) >= 2 and parts[1].isdigit():
                total = int(parts[1])
        elif line.startswith("MemAvailable:"):
            parts = line.split()
            if len(parts) >= 2 and parts[1].isdigit():
                avail = int(parts[1])
    if total <= 0:
        return 0.0
    used = max(0, total - avail)
    return round((used / total) * 100.0, 1)


def read_battery_info(power_root: Path = Path("/sys/class/power_supply")) -> tuple[int, bool]:
    if not power_root.is_dir():
        return 100, True
    try:
        for entry in sorted(power_root.iterdir()):
            tfile = entry / "type"
            if not tfile.is_file():
                continue
            if tfile.read_text(encoding="utf-8", errors="replace").strip().lower() != "battery":
                continue
            cap_f = entry / "capacity"
            st_f = entry / "status"
            cap = 100
            if cap_f.is_file():
                raw = cap_f.read_text(encoding="utf-8", errors="replace").strip()
                if raw.isdigit():
                    cap = max(0, min(100, int(raw)))
            status = st_f.read_text(encoding="utf-8", errors="replace").strip().lower() if st_f.is_file() else ""
            charging = status in ("charging", "full", "not charging")
            return cap, charging
    except OSError:
        pass
    return 100, True


def compute_pcm16_spectrum(pcm_bytes: bytes, num_bands: int = 8) -> tuple[float, list[float]]:
    """Compute RMS and coarse N-band energy spectrum from mono 16-bit signed LE PCM."""
    count = len(pcm_bytes) // 2
    if count < 16:
        return 0.0, [0.0] * num_bands
    n = min(count, 256)
    samples = struct.unpack(f"<{n}h", pcm_bytes[: n * 2])
    norm = [s / 32768.0 for s in samples]
    rms = math.sqrt(sum(x * x for x in norm) / n)

    # Apply Hann window so off-bin tones distribute smoothly across adjacent bands
    windowed = [norm[i] * (0.5 - 0.5 * math.cos((2.0 * math.pi * i) / max(1, n - 1))) for i in range(n)]
    max_bin = max(2, n // 2)

    def _goertzel_mag(k: int) -> float:
        omega = (2.0 * math.pi * k) / n
        coeff = 2.0 * math.cos(omega)
        s0 = s1 = s2 = 0.0
        for x in windowed:
            s0 = x + coeff * s1 - s2
            s2 = s1
            s1 = s0
        power = max(0.0, s1 * s1 + s2 * s2 - coeff * s1 * s2)
        return math.sqrt(power) / (n * 0.25)

    # Logarithmically spaced frequency band centers with +/-1 neighbor capture
    edges = [1, 2, 4, 7, 12, 20, 34, 58, min(max_bin, 96)]
    bands: list[float] = []
    for b in range(num_bands):
        lo = min(max_bin - 1, edges[min(b, len(edges) - 2)])
        hi = min(max_bin - 1, max(lo, edges[min(b + 1, len(edges) - 1)]))
        mid = (lo + hi) // 2
        probes = sorted({lo, mid, hi})
        mag = max(_goertzel_mag(k) for k in probes)
        bands.append(round(min(1.0, mag), 3))
    return round(min(1.0, rms * 2.2), 3), bands


class SystemStatsSampler:
    """Stateful local CPU / RAM / Battery sampler."""

    def __init__(
        self,
        proc_stat: Path = Path("/proc/stat"),
        proc_meminfo: Path = Path("/proc/meminfo"),
        power_root: Path = Path("/sys/class/power_supply"),
    ) -> None:
        self._proc_stat = proc_stat
        self._proc_meminfo = proc_meminfo
        self._power_root = power_root
        self._prev_cpu: tuple[int, int] | None = None

    def sample(self) -> dict[str, Any]:
        cpu_pct = 0.0
        try:
            cur = parse_proc_stat_cpu(self._proc_stat.read_text(encoding="utf-8", errors="replace"))
            if cur is not None:
                if self._prev_cpu is not None:
                    d_idle = max(0, cur[0] - self._prev_cpu[0])
                    d_total = max(1, cur[1] - self._prev_cpu[1])
                    cpu_pct = round(max(0.0, min(100.0, (1.0 - (d_idle / d_total)) * 100.0)), 1)
                self._prev_cpu = cur
        except OSError:
            pass

        mem_pct = 0.0
        try:
            mem_pct = parse_meminfo_pct(self._proc_meminfo.read_text(encoding="utf-8", errors="replace"))
        except OSError:
            pass

        battery, charging = read_battery_info(self._power_root)
        return {
            "t": "systemStats",
            "cpu": cpu_pct,
            "mem": mem_pct,
            "battery": battery,
            "charging": charging,
        }


class LocalAudioMeter:
    """Opt-in PipeWire/PulseAudio loopback audio level reader (zero external Python deps)."""

    def __init__(self) -> None:
        self._proc: subprocess.Popen[bytes] | None = None
        self._thread: threading.Thread | None = None
        self._stop = threading.Event()
        self._lock = threading.Lock()
        self._rms: float = 0.0
        self._bands: list[float] = [0.0] * 8

    def start(self) -> bool:
        if self._proc is not None:
            return True
        cmd: list[str] | None = None
        if shutil.which("parec"):
            cmd = ["parec", "--format=s16le", "--rate=16000", "--channels=1", "--latency-msec=40"]
        elif shutil.which("pw-cat"):
            cmd = ["pw-cat", "--record", "--format=s16", "--rate=16000", "--channels=1", "-"]
        if cmd is None:
            return False
        try:
            self._proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        except OSError:
            self._proc = None
            return False
        self._stop.clear()
        self._thread = threading.Thread(target=self._loop, name="diyardesk-audio-meter", daemon=True)
        self._thread.start()
        return True

    def stop(self) -> None:
        self._stop.set()
        proc = self._proc
        self._proc = None
        if proc is not None:
            try:
                proc.terminate()
                proc.wait(timeout=1.0)
            except Exception:
                try:
                    proc.kill()
                except OSError:
                    pass
        with self._lock:
            self._rms = 0.0
            self._bands = [0.0] * 8

    def _loop(self) -> None:
        proc = self._proc
        if proc is None or proc.stdout is None:
            return
        while not self._stop.is_set():
            chunk = proc.stdout.read(512)
            if not chunk:
                break
            rms, bands = compute_pcm16_spectrum(chunk, num_bands=8)
            with self._lock:
                self._rms = rms
                self._bands = bands

    def latest_payload(self) -> dict[str, Any]:
        with self._lock:
            return {"t": "audioLevels", "rms": self._rms, "bands": list(self._bands)}
