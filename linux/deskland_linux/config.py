"""Paths, settings, logging, i18n and theme registry for the Linux host."""

from __future__ import annotations

import json
import os
import subprocess
import threading
import time
from pathlib import Path
from typing import Any

APP_NAME = "DiyarDesk"
APP_ID = "app.deskland.Preview"
VERSION = "0.1.4-linux.1"

THEME_DEFAULT = "papercut"
LANG_DEFAULT = "auto"
SUPPORTED_LANGS = ("zh-CN", "zh-TW", "en", "tr", "de", "fr", "ja", "ko")


def repo_root() -> Path:
    """Repository root when running from source; install root when packaged."""
    return Path(__file__).resolve().parents[2]


def prototype_dir() -> Path:
    override = os.environ.get("DESKLAND_PROTOTYPE")
    if override:
        return Path(override).expanduser().resolve()
    return repo_root() / "prototype"


def config_home() -> Path:
    base = os.environ.get("XDG_CONFIG_HOME")
    return Path(base).expanduser() if base else Path.home() / ".config"


def state_home() -> Path:
    base = os.environ.get("XDG_STATE_HOME")
    return Path(base).expanduser() if base else Path.home() / ".local" / "state"


def data_home() -> Path:
    base = os.environ.get("XDG_DATA_HOME")
    return Path(base).expanduser() if base else Path.home() / ".local" / "share"


def config_dir() -> Path:
    override = os.environ.get("DESKLAND_PROFILE_DIR")
    if override:
        return Path(override).expanduser().resolve()
    return config_home() / "deskland"


def log_path() -> Path:
    return config_dir() / "host.log"


def pictures_dir() -> Path:
    override = os.environ.get("DESKLAND_PICTURES_DIR")
    if override:
        return Path(override).expanduser().resolve()
    try:
        out = subprocess.run(
            ["xdg-user-dir", "PICTURES"],
            capture_output=True,
            text=True,
            timeout=2,
            check=False,
        ).stdout.strip()
        if out and Path(out).is_dir():
            return Path(out)
    except (OSError, subprocess.SubprocessError):
        pass
    return Path.home() / "Pictures"


def desktop_dir() -> Path:
    override = os.environ.get("DESKLAND_DESKTOP_DIR")
    if override:
        return Path(override).expanduser().resolve()
    try:
        out = subprocess.run(
            ["xdg-user-dir", "DESKTOP"],
            capture_output=True,
            text=True,
            timeout=2,
            check=False,
        ).stdout.strip()
        if out and Path(out).is_dir():
            return Path(out)
    except (OSError, subprocess.SubprocessError):
        pass
    return Path.home() / "Desktop"


def autostart_path() -> Path:
    return config_home() / "autostart" / "deskland.desktop"


def atomic_write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_bytes(data)
    os.replace(tmp, path)


class Log:
    def __init__(self, path: Path | None = None) -> None:
        self.path = path or log_path()
        self._lock = threading.Lock()

    def write(self, message: str) -> None:
        line = time.strftime("%Y-%m-%d %H:%M:%S ") + message + "\n"
        with self._lock:
            try:
                self.path.parent.mkdir(parents=True, exist_ok=True)
                with self.path.open("a", encoding="utf-8") as fh:
                    fh.write(line)
            except OSError:
                pass
        if os.environ.get("DESKLAND_DEBUG"):
            try:
                print(line, end="")
            except UnicodeError:
                print(line.encode("ascii", "replace").decode("ascii"), end="")

    def exception(self, message: str, exc: BaseException) -> None:
        self.write(f"{message}: {exc!r}")


VIRTUAL_BAR_MODES = ("auto", "always", "off")
_ALLOWED_ICON_TYPES = {"folder", "txt", "pdf", "xlsx", "png", "zip"}


def sanitize_virtual_icons(raw: Any) -> list[dict[str, Any]]:
    if not isinstance(raw, list):
        return []
    out: list[dict[str, Any]] = []
    for item in raw[:64]:
        if not isinstance(item, dict):
            continue
        try:
            x = int(round(float(item.get("x", 0))))
            y = int(round(float(item.get("y", 0))))
            w = max(24, min(256, int(round(float(item.get("w", 76))))))
            h = max(24, min(256, int(round(float(item.get("h", 86))))))
        except (TypeError, ValueError):
            continue
        icon_type = item.get("type")
        if icon_type not in _ALLOWED_ICON_TYPES:
            icon_type = "folder"
        name = item.get("name")
        if not isinstance(name, str) or not name.strip():
            name = "Yeni Klasör"
        out.append({"name": name.strip()[:48], "type": icon_type, "x": x, "y": y, "w": w, "h": h})
    return out


class SettingsStore:
    """theme + per-theme parameter overrides, persisted as JSON."""

    def __init__(self, directory: Path | None = None) -> None:
        self.path = (directory or config_dir()) / "settings.json"
        self.theme = THEME_DEFAULT
        self.lang = LANG_DEFAULT
        self.params: dict[str, dict[str, Any]] = {}
        self.virtual_icons: list[dict[str, Any]] = []
        self.virtual_bar_collapsed: bool | None = None
        self.virtual_bar_mode: str = "auto"
        self.low_mode: bool = False
        self.disable_auto_pause: bool = False
        self._lock = threading.Lock()

    def load(self) -> None:
        try:
            if not self.path.exists():
                return
            data = json.loads(self.path.read_text(encoding="utf-8"))
            if not isinstance(data, dict):
                return
            theme = data.get("theme")
            if isinstance(theme, str):
                self.theme = theme
            lang = data.get("lang")
            if isinstance(lang, str):
                self.lang = lang
            params = data.get("params")
            if isinstance(params, dict):
                self.params = {
                    str(k): dict(v)
                    for k, v in params.items()
                    if isinstance(v, dict)
                }
            self.virtual_icons = sanitize_virtual_icons(data.get("virtualIcons"))
            collapsed = data.get("virtualBarCollapsed")
            if isinstance(collapsed, bool) or collapsed is None:
                self.virtual_bar_collapsed = collapsed
            mode = data.get("virtualBarMode")
            if isinstance(mode, str) and mode in VIRTUAL_BAR_MODES:
                self.virtual_bar_mode = mode
            low_mode = data.get("lowMode")
            if isinstance(low_mode, bool):
                self.low_mode = low_mode
            disable_auto_pause = data.get("disableAutoPause")
            if isinstance(disable_auto_pause, bool):
                self.disable_auto_pause = disable_auto_pause
        except (OSError, ValueError, TypeError):
            self.theme = THEME_DEFAULT
            self.lang = LANG_DEFAULT
            self.params = {}
            self.virtual_icons = []
            self.virtual_bar_collapsed = None
            self.virtual_bar_mode = "auto"
            self.low_mode = False
            self.disable_auto_pause = False

    def save(self) -> None:
        with self._lock:
            payload = {
                "version": 1,
                "theme": self.theme,
                "lang": self.lang,
                "params": self.params,
                "virtualIcons": self.virtual_icons,
                "virtualBarCollapsed": self.virtual_bar_collapsed,
                "virtualBarMode": self.virtual_bar_mode,
                "lowMode": self.low_mode,
                "disableAutoPause": self.disable_auto_pause,
            }
            raw = json.dumps(payload, ensure_ascii=False, indent=1).encode("utf-8")
            try:
                atomic_write(self.path, raw)
            except OSError:
                pass

    def for_theme(self, theme: str) -> dict[str, Any]:
        return self.params.setdefault(theme, {})


class ThemeRegistry:
    def __init__(self, prototype: Path, log: Log, custom_dir: Path | None = None) -> None:
        self.path = prototype / "themes.json"
        if custom_dir is not None:
            self.custom_dir: Path | None = custom_dir
        elif os.environ.get("DESKLAND_CUSTOM_THEMES_DIR"):
            self.custom_dir = Path(os.environ["DESKLAND_CUSTOM_THEMES_DIR"]).expanduser().resolve()
        else:
            self.custom_dir = None
        self.log = log
        self.base_themes: list[dict[str, Any]] = []
        self.themes: list[dict[str, Any]] = []
        self._mtime_ns: int | None = None
        self._custom_mtime_ns: int | None = None
        self.load()

    def _custom_registry_path(self) -> Path | None:
        return (self.custom_dir / "registry.json") if self.custom_dir is not None else None

    def _stat_mtime(self, p: Path | None) -> int | None:
        if p is None:
            return None
        try:
            return p.stat().st_mtime_ns
        except OSError:
            return None

    def load(self) -> None:
        base: list[dict[str, Any]] = []
        try:
            self._mtime_ns = self.path.stat().st_mtime_ns
            data = json.loads(self.path.read_text(encoding="utf-8"))
            themes = data.get("themes") if isinstance(data, dict) else None
            if isinstance(themes, list):
                base = [t for t in themes if isinstance(t, dict)]
        except (OSError, ValueError) as exc:
            self.log.exception("themes.json okunamadı", exc)
            base = []
        self.base_themes = base

        custom_path = self._custom_registry_path()
        self._custom_mtime_ns = self._stat_mtime(custom_path)
        custom: list[dict[str, Any]] = []
        if custom_path is not None and custom_path.is_file():
            try:
                cdata = json.loads(custom_path.read_text(encoding="utf-8"))
                cthemes = cdata.get("themes") if isinstance(cdata, dict) else None
                if isinstance(cthemes, list):
                    custom = [t for t in cthemes if isinstance(t, dict)]
            except (OSError, ValueError) as exc:
                self.log.exception("custom registry.json okunamadı", exc)

        seen_ids = {t.get("id") for t in base if t.get("id")}
        merged = list(base)
        for item in custom:
            tid = item.get("id")
            if isinstance(tid, str) and tid and tid not in seen_ids:
                seen_ids.add(tid)
                merged.append(item)
        self.themes = merged

    def _reload_if_stale(self) -> None:
        mtime_ns = self._stat_mtime(self.path)
        cmtime_ns = self._stat_mtime(self._custom_registry_path())
        if mtime_ns != self._mtime_ns or cmtime_ns != self._custom_mtime_ns:
            self.load()

    def merged_json_bytes(self) -> bytes:
        self._reload_if_stale()
        return json.dumps({"themes": self.themes}, ensure_ascii=False, indent=2).encode("utf-8")

    def find(self, theme_id: str | None) -> dict[str, Any] | None:
        if not theme_id:
            return None
        self._reload_if_stale()
        for theme in self.themes:
            if theme.get("id") == theme_id:
                return theme
        return None

    def page_for(self, theme_id: str | None) -> str | None:
        theme = self.find(theme_id)
        if not theme or theme.get("status") != "ready":
            return None
        page = theme.get("page")
        return page if isinstance(page, str) and page else None

    def allowed(self, theme_id: str | None) -> bool:
        theme = self.find(theme_id)
        return bool(
            theme
            and theme.get("status") == "ready"
            and theme.get("product") == "free"
            and isinstance(theme.get("page"), str)
        )

    def defaults(self, theme_id: str) -> dict[str, Any]:
        theme = self.find(theme_id) or {}
        values: dict[str, Any] = {}
        if theme.get("adaptive"):
            values["adaptive"] = 1
        groups = theme.get("groups")
        if isinstance(groups, list):
            for group in groups:
                if not isinstance(group, dict):
                    continue
                for param in group.get("params") or []:
                    if isinstance(param, dict) and "key" in param:
                        values[str(param["key"])] = param.get("default")
        return values


class I18n:
    """Host-side strings from prototype/i18n/<code>.json -> host."""

    def __init__(self, prototype: Path, setting: str, log: Log) -> None:
        self.prototype = prototype
        self.log = log
        self.setting = setting
        self.system = self.detect_system()
        self.code = setting if setting in SUPPORTED_LANGS else self.system
        self._tables: dict[str, dict[str, str]] = {}

    @staticmethod
    def detect_system() -> str:
        for var in ("LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"):
            value = os.environ.get(var, "")
            if not value:
                continue
            first = value.split(":", 1)[0].split(".", 1)[0].split("@", 1)[0]
            tag = first.lower().replace("_", "-")
            if tag.startswith("zh"):
                if "hant" in tag or tag.endswith(("-tw", "-hk", "-mo")):
                    return "zh-TW"
                return "zh-CN"
            two = tag.split("-", 1)[0]
            if two in SUPPORTED_LANGS:
                return two
            if first:
                return "en"
        return "en"

    def _load(self, code: str) -> dict[str, str]:
        if code in self._tables:
            return self._tables[code]
        table: dict[str, str] = {}
        try:
            data = json.loads(
                (self.prototype / "i18n" / f"{code}.json").read_text(encoding="utf-8")
            )
            host = data.get("host") if isinstance(data, dict) else None
            if isinstance(host, dict):
                table = {str(k): str(v) for k, v in host.items() if isinstance(v, str)}
        except (OSError, ValueError) as exc:
            self.log.exception(f"i18n/{code}.json okunamadı", exc)
        self._tables[code] = table
        return table

    def set_lang(self, setting: str) -> None:
        self.setting = setting
        self.code = setting if setting in SUPPORTED_LANGS else self.system

    def t(self, key: str, **values: Any) -> str:
        for code in (self.code, "en", "zh-CN"):
            text = self._load(code).get(key)
            if text is not None:
                for name, value in values.items():
                    text = text.replace("{" + name + "}", str(value))
                return text
        return key
