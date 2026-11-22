"""Runaway script resource watchdog (RAM & CPU ceiling) for Linux WebKitWebProcess.

Pure Python (reads `/proc` on Linux) so it stays 100% unit-testable without GTK.
Detects leaking or infinite-loop third-party themes and signals the host to reload
or pause before the desktop becomes unresponsive.
"""
from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class ProcessSample:
    pid: int
    rss_mb: float
    cpu_ticks: int


def parse_proc_status_rss_mb(status_text: str) -> float:
    for line in status_text.splitlines():
        if line.startswith("VmRSS:"):
            parts = line.split()
            if len(parts) >= 2:
                try:
                    return int(parts[1]) / 1024.0
                except ValueError:
                    return 0.0
    return 0.0


def parse_proc_stat_ticks(stat_text: str) -> int:
    # Field 14 (utime) and 15 (stime) after the last ')' of comm
    idx = stat_text.rfind(")")
    if idx < 0:
        return 0
    fields = stat_text[idx + 2 :].split()
    if len(fields) < 13:
        return 0
    try:
        return int(fields[11]) + int(fields[12])
    except ValueError:
        return 0


def find_webkit_child_pids(parent_pid: int | None = None, proc_root: Path = Path("/proc")) -> list[int]:
    if not proc_root.is_dir():
        return []
    ppid = os.getpid() if parent_pid is None else int(parent_pid)
    pids: list[int] = []
    try:
        for entry in proc_root.iterdir():
            if not entry.name.isdigit():
                continue
            try:
                status = (entry / "status").read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue
            name = ""
            child_ppid = -1
            for line in status.splitlines():
                if line.startswith("Name:"):
                    name = line.split(":", 1)[1].strip().lower()
                elif line.startswith("PPid:"):
                    try:
                        child_ppid = int(line.split(":", 1)[1].strip())
                    except ValueError:
                        child_ppid = -1
            if child_ppid == ppid and ("webkit" in name or "webprocess" in name):
                pids.append(int(entry.name))
    except OSError:
        return []
    return pids


def sample_processes(pids: list[int], proc_root: Path = Path("/proc")) -> list[ProcessSample]:
    samples: list[ProcessSample] = []
    for pid in pids:
        base = proc_root / str(pid)
        try:
            rss = parse_proc_status_rss_mb((base / "status").read_text(encoding="utf-8", errors="replace"))
            ticks = parse_proc_stat_ticks((base / "stat").read_text(encoding="utf-8", errors="replace"))
            samples.append(ProcessSample(pid=pid, rss_mb=rss, cpu_ticks=ticks))
        except OSError:
            continue
    return samples


class ResourceWatchdog:
    """Tracks WebKitWebProcess RSS & CPU over consecutive polls to catch runaway scripts."""

    def __init__(
        self,
        max_rss_mb: float = 1536.0,
        max_cpu_pct: float = 92.0,
        strikes_needed: int = 3,
        clk_tck: int = 100,
    ) -> None:
        self.max_rss_mb = max(256.0, float(max_rss_mb))
        self.max_cpu_pct = max(50.0, float(max_cpu_pct))
        self.strikes_needed = max(1, int(strikes_needed))
        self.clk_tck = max(1, int(clk_tck))
        self._prev_ticks: dict[int, int] = {}
        self._prev_ts: float | None = None
        self._strikes: int = 0
        self.last_rss_mb: float = 0.0
        self.last_cpu_pct: float = 0.0

    def reset(self) -> None:
        self._prev_ticks.clear()
        self._prev_ts = None
        self._strikes = 0

    def evaluate(self, samples: list[ProcessSample], now_sec: float) -> str | None:
        """Return a trip reason string if sustained runaway usage is detected, else None."""
        if not samples:
            self._strikes = 0
            self.last_rss_mb = 0.0
            self.last_cpu_pct = 0.0
            return None

        total_rss = sum(s.rss_mb for s in samples)
        cpu_pct = 0.0
        if self._prev_ts is not None and now_sec > self._prev_ts:
            dt = now_sec - self._prev_ts
            delta_ticks = 0
            for s in samples:
                prev = self._prev_ticks.get(s.pid)
                if prev is not None and s.cpu_ticks >= prev:
                    delta_ticks += s.cpu_ticks - prev
            cpu_pct = (delta_ticks / (dt * self.clk_tck)) * 100.0

        self._prev_ticks = {s.pid: s.cpu_ticks for s in samples}
        self._prev_ts = now_sec
        self.last_rss_mb = round(total_rss, 1)
        self.last_cpu_pct = round(cpu_pct, 1)

        reason = None
        if total_rss >= self.max_rss_mb:
            reason = f"memory_limit ({self.last_rss_mb:.0f} MB >= {self.max_rss_mb:.0f} MB)"
        elif cpu_pct >= self.max_cpu_pct:
            reason = f"cpu_limit ({self.last_cpu_pct:.0f}% >= {self.max_cpu_pct:.0f}%)"

        if reason is not None:
            self._strikes += 1
            if self._strikes >= self.strikes_needed:
                self._strikes = 0
                return reason
        else:
            self._strikes = 0
        return None
