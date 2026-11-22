import json

from deskland_linux.config import SettingsStore, ThemeRegistry, I18n, sanitize_virtual_icons


class DummyLog:
    def write(self, _message):
        pass

    def exception(self, _message, _exc):
        pass


def test_settings_roundtrip(tmp_path):
    store = SettingsStore(tmp_path)
    store.theme = "meadow"
    store.lang = "en"
    store.for_theme("meadow")["sigma"] = 120
    store.save()

    other = SettingsStore(tmp_path)
    other.load()
    assert other.theme == "meadow"
    assert other.lang == "en"
    assert other.for_theme("meadow")["sigma"] == 120


def test_settings_tolerates_broken_json(tmp_path):
    (tmp_path / "settings.json").write_text("{broken", encoding="utf-8")
    store = SettingsStore(tmp_path)
    store.load()
    assert store.theme == "papercut"


def test_theme_registry_free_only(tmp_path):
    data = {
        "themes": [
            {"id": "beach", "status": "ready", "product": "free", "page": "flat.html"},
            {"id": "locked", "status": "locked", "locked": True},
        ]
    }
    (tmp_path / "themes.json").write_text(json.dumps(data), encoding="utf-8")
    registry = ThemeRegistry(tmp_path, DummyLog())
    assert registry.page_for("beach") == "flat.html"
    assert registry.allowed("beach") is True
    assert registry.allowed("locked") is False


def test_theme_registry_reloads_when_themes_json_updates(tmp_path):
    themes_file = tmp_path / "themes.json"
    themes_file.write_text(
        json.dumps({"themes": [{"id": "beach", "status": "ready", "product": "free", "page": "flat.html"}]}),
        encoding="utf-8",
    )
    registry = ThemeRegistry(tmp_path, DummyLog())
    assert registry.allowed("roach") is False

    themes_file.write_text(
        json.dumps({
            "themes": [
                {"id": "beach", "status": "ready", "product": "free", "page": "flat.html"},
                {"id": "roach", "status": "ready", "product": "free", "page": "roach.html"},
            ]
        }),
        encoding="utf-8",
    )
    assert registry.allowed("roach") is True
    assert registry.page_for("roach") == "roach.html"


def test_i18n_fallback(tmp_path):
    (tmp_path / "i18n").mkdir()
    (tmp_path / "i18n" / "en.json").write_text(
        json.dumps({"host": {"trayPause": "Pause {name}"}}), encoding="utf-8"
    )
    i18n = I18n(tmp_path, "en", DummyLog())
    assert i18n.t("trayPause", name="now") == "Pause now"
    assert i18n.t("missingKey") == "missingKey"


def test_virtual_icons_settings_roundtrip_and_sanitize(tmp_path):
    store = SettingsStore(tmp_path)
    store.virtual_icons = sanitize_virtual_icons([
        {"name": "Yeni Klasör", "type": "folder", "x": 120.4, "y": 80.7, "w": 76, "h": 86},
        {"name": "", "type": "unknown", "x": "bad"},
        {"name": "Notlar.txt", "type": "txt", "x": 200, "y": 140, "w": 500, "h": 10},
    ])
    store.virtual_bar_collapsed = True
    store.virtual_bar_mode = "always"
    store.save()

    loaded = SettingsStore(tmp_path)
    loaded.load()
    assert loaded.virtual_bar_collapsed is True
    assert loaded.virtual_bar_mode == "always"
    assert loaded.virtual_icons == [
        {"name": "Yeni Klasör", "type": "folder", "x": 120, "y": 81, "w": 76, "h": 86},
        {"name": "Notlar.txt", "type": "txt", "x": 200, "y": 140, "w": 256, "h": 24},
    ]


def test_low_mode_setting_roundtrip(tmp_path):
    store = SettingsStore(tmp_path)
    assert store.low_mode is False
    store.low_mode = True
    store.save()

    loaded = SettingsStore(tmp_path)
    loaded.load()
    assert loaded.low_mode is True


def test_disable_auto_pause_setting_roundtrip(tmp_path):
    store = SettingsStore(tmp_path)
    assert store.disable_auto_pause is False
    store.disable_auto_pause = True
    store.save()

    loaded = SettingsStore(tmp_path)
    loaded.load()
    assert loaded.disable_auto_pause is True


def test_turkish_system_detection_and_i18n(tmp_path, monkeypatch):
    (tmp_path / "i18n").mkdir()
    (tmp_path / "i18n" / "en.json").write_text(
        json.dumps({"host": {"traySettings": "Settings…"}}), encoding="utf-8"
    )
    (tmp_path / "i18n" / "tr.json").write_text(
        json.dumps({"host": {"traySettings": "Ayarlar…"}}), encoding="utf-8"
    )
    for var in ("LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"):
        monkeypatch.delenv(var, raising=False)
    monkeypatch.setenv("LC_ALL", "tr_TR.UTF-8")

    assert I18n.detect_system() == "tr"
    i18n = I18n(tmp_path, "auto", DummyLog())
    assert i18n.code == "tr"
    assert i18n.t("traySettings") == "Ayarlar…"
