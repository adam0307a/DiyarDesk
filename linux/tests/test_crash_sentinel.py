"""Unit tests for Linux StartupCrashSentinel (Safe Mode fallback)."""

from __future__ import annotations

import json
from pathlib import Path

from deskland_linux.config import Log, SettingsStore
from deskland_linux.crash_sentinel import StartupCrashSentinel


def test_normal_startup_and_mark_stable(tmp_path: Path) -> None:
    now = [1_700_000_000.0]
    sentinel = StartupCrashSentinel(tmp_path, clock=lambda: now[0])

    res1 = sentinel.begin_startup("custom-broken-shader", safe_theme_resolver=lambda: "papercut")
    assert not res1.fell_back
    assert res1.active_theme == "custom-broken-shader"
    assert res1.consecutive_crashes == 0
    assert sentinel.path.exists()
    state = json.loads(sentinel.path.read_text(encoding="utf-8"))
    assert state["count"] == 1
    assert state["theme"] == "custom-broken-shader"

    assert sentinel.mark_stable() is True
    assert not sentinel.path.exists()
    assert sentinel.mark_stable() is False

    # Next launch after clean stable run starts fresh at count=1
    now[0] += 30.0
    res2 = sentinel.begin_startup("custom-broken-shader", safe_theme_resolver=lambda: "papercut")
    assert not res2.fell_back
    assert res2.consecutive_crashes == 0
    state2 = json.loads(sentinel.path.read_text(encoding="utf-8"))
    assert state2["count"] == 1


def test_two_consecutive_crashes_trigger_safe_mode_fallback(tmp_path: Path) -> None:
    now = [1_700_000_100.0]
    settings = SettingsStore(tmp_path)
    settings.theme = "custom-crashy-webgl"
    settings.save()
    log = Log(tmp_path / "host.log")

    sentinel = StartupCrashSentinel(tmp_path, clock=lambda: now[0])

    # Launch 1 crashes before mark_stable()
    r1 = sentinel.apply_to_settings(settings, safe_theme_resolver=lambda: "papercut", log=log)
    assert not r1.fell_back
    assert settings.theme == "custom-crashy-webgl"

    # Launch 2 within window also crashes before mark_stable()
    now[0] += 20.0
    r2 = sentinel.apply_to_settings(settings, safe_theme_resolver=lambda: "papercut", log=log)
    assert not r2.fell_back
    assert r2.consecutive_crashes == 1
    assert settings.theme == "custom-crashy-webgl"
    assert json.loads(sentinel.path.read_text(encoding="utf-8"))["count"] == 2

    # Launch 3 within window -> Safe Mode triggers and persists safe built-in theme
    now[0] += 15.0
    r3 = sentinel.apply_to_settings(settings, safe_theme_resolver=lambda: "papercut", log=log)
    assert r3.fell_back is True
    assert r3.previous_theme == "custom-crashy-webgl"
    assert r3.active_theme == "papercut"
    assert r3.consecutive_crashes == 2
    assert settings.theme == "papercut"

    # Verify fallback was persisted to settings.json and sentinel count reset to 1
    reloaded = SettingsStore(tmp_path)
    reloaded.load()
    assert reloaded.theme == "papercut"
    assert json.loads(sentinel.path.read_text(encoding="utf-8"))["count"] == 1
    assert "Safe Mode" in log.path.read_text(encoding="utf-8")


def test_crashes_outside_window_do_not_trigger_safe_mode(tmp_path: Path) -> None:
    now = [1_700_001_000.0]
    sentinel = StartupCrashSentinel(tmp_path, window_seconds=180, clock=lambda: now[0])

    sentinel.begin_startup("custom-shader", safe_theme_resolver=lambda: "meadow")
    now[0] += 30.0
    sentinel.begin_startup("custom-shader", safe_theme_resolver=lambda: "meadow")
    assert json.loads(sentinel.path.read_text(encoding="utf-8"))["count"] == 2

    # Wait longer than the 180 s window -> count resets, no Safe Mode fallback
    now[0] += 181.0
    res = sentinel.begin_startup("custom-shader", safe_theme_resolver=lambda: "meadow")
    assert not res.fell_back
    assert res.active_theme == "custom-shader"
    assert res.consecutive_crashes == 0
    assert json.loads(sentinel.path.read_text(encoding="utf-8"))["count"] == 1


def test_corrupt_sentinel_file_is_recovered_gracefully(tmp_path: Path) -> None:
    sentinel = StartupCrashSentinel(tmp_path, clock=lambda: 1_700_002_000.0)
    sentinel.path.write_text("{not valid json!!!", encoding="utf-8")

    res = sentinel.begin_startup("aurora", safe_theme_resolver=lambda: "meadow")
    assert not res.fell_back
    assert res.active_theme == "aurora"
    data = json.loads(sentinel.path.read_text(encoding="utf-8"))
    assert data["count"] == 1
    assert data["theme"] == "aurora"


def test_already_on_safe_theme_resets_without_fallback_flag(tmp_path: Path) -> None:
    now = [1_700_003_000.0]
    settings = SettingsStore(tmp_path)
    settings.theme = "papercut"
    log = Log(tmp_path / "host.log")
    sentinel = StartupCrashSentinel(tmp_path, clock=lambda: now[0])

    sentinel.apply_to_settings(settings, safe_theme_resolver=lambda: "papercut", log=log)
    now[0] += 10.0
    sentinel.apply_to_settings(settings, safe_theme_resolver=lambda: "papercut", log=log)
    now[0] += 10.0
    res = sentinel.apply_to_settings(settings, safe_theme_resolver=lambda: "papercut", log=log)

    assert res.fell_back is False
    assert res.active_theme == "papercut"
    assert res.consecutive_crashes == 2
    assert json.loads(sentinel.path.read_text(encoding="utf-8"))["count"] == 1
    assert "Safe Mode" in log.path.read_text(encoding="utf-8")


def test_clock_skew_future_timestamp_does_not_trigger_safe_mode(tmp_path: Path) -> None:
    now = [1_700_004_000.0]
    sentinel = StartupCrashSentinel(tmp_path, clock=lambda: now[0])
    sentinel.path.write_text(
        json.dumps({"count": 5, "ts": int(now[0] + 600), "theme": "custom-shader"}),
        encoding="utf-8",
    )

    res = sentinel.begin_startup("custom-shader", safe_theme_resolver=lambda: "meadow")
    assert not res.fell_back
    assert res.active_theme == "custom-shader"
    assert json.loads(sentinel.path.read_text(encoding="utf-8"))["count"] == 1
