import json

import pytest

from deskland_linux import ctl
from deskland_linux.config import SettingsStore
from deskland_linux.presets import (
    MAX_PRESET_BYTES,
    apply_preset_to_settings,
    build_preset_payload,
    load_preset_file,
    save_preset_file,
)


def test_preset_roundtrip_and_apply(tmp_path):
    store = SettingsStore(tmp_path)
    store.theme = "istanbul"
    store.solar_sync = True
    store.window_aware = False
    store.low_mode = True
    store.for_theme("istanbul")["lanternWarmth"] = 0.88

    payload = build_preset_payload(store, theme_name="Bosphorus Night")
    assert payload["format"] == "diyardesk-preset"
    assert payload["theme"] == "istanbul"
    assert payload["params"]["lanternWarmth"] == 0.88

    preset_path = save_preset_file(tmp_path / "bosphorus.deskland-preset", payload)
    loaded = load_preset_file(preset_path, lambda tid: tid in {"papercut", "istanbul"})

    other = SettingsStore(tmp_path / "other")
    summary = apply_preset_to_settings(other, loaded)
    assert summary["theme"] == "istanbul"
    assert summary["name"] == "Bosphorus Night"
    assert other.theme == "istanbul"
    assert other.solar_sync is True
    assert other.window_aware is False
    assert other.low_mode is True
    assert other.for_theme("istanbul")["lanternWarmth"] == 0.88


def test_preset_rejects_unknown_theme_broken_json_and_oversized(tmp_path):
    broken = tmp_path / "broken.deskland-preset"
    broken.write_text("{not-json", encoding="utf-8")
    with pytest.raises(ValueError):
        load_preset_file(broken, lambda _t: True)

    unknown = tmp_path / "unknown.deskland-preset"
    unknown.write_text(json.dumps({"theme": "nope"}), encoding="utf-8")
    with pytest.raises(ValueError):
        load_preset_file(unknown, lambda tid: tid == "papercut")

    huge = tmp_path / "huge.deskland-preset"
    huge.write_text(" " * (MAX_PRESET_BYTES + 16), encoding="utf-8")
    with pytest.raises(ValueError):
        load_preset_file(huge, lambda _t: True)


def test_ctl_apply_preset_payload(tmp_path):
    f = tmp_path / "demo.deskland-preset"
    payload, as_json, sock = ctl.build_command_payload(["apply-preset", str(f)])
    assert payload["cmd"] == "apply-preset"
    assert payload["path"] == str(f.resolve())
    assert as_json is False
    assert sock is None
