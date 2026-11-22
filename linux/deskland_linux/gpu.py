"""Single-GPU EGL vendor selection to avoid dual-driver stalls on hybrid laptops."""

from __future__ import annotations

import os
from collections.abc import MutableMapping
from pathlib import Path

_MESA_VENDORS = {"0x1002", "0x8086"}
_NVIDIA_VENDOR = "0x10de"


def _find_vendor_json(egl_dir: Path, needle: str) -> Path | None:
    if not egl_dir.is_dir():
        return None
    for path in sorted(egl_dir.glob("*.json")):
        if needle in path.name.lower():
            return path
    return None


def _connected_gpu_vendors(drm_root: Path) -> set[str]:
    vendors: set[str] = set()
    if not drm_root.is_dir():
        return vendors
    for status_file in sorted(drm_root.glob("card*-*/status")):
        try:
            if status_file.read_text(encoding="utf-8", errors="ignore").strip() != "connected":
                continue
        except OSError:
            continue
        card_name = status_file.parent.name.split("-", 1)[0]
        vendor_file = drm_root / card_name / "device" / "vendor"
        try:
            vendor = vendor_file.read_text(encoding="utf-8", errors="ignore").strip().lower()
        except OSError:
            continue
        if vendor:
            vendors.add(vendor)
    return vendors


def select_single_egl_vendor(
    environ: MutableMapping[str, str] | None = None,
    drm_root: Path = Path("/sys/class/drm"),
    egl_dir: Path = Path("/usr/share/glvnd/egl_vendor.d"),
) -> str | None:
    """Return an explicit EGL vendor JSON path on hybrid GPU systems, or None."""
    env = os.environ if environ is None else environ
    if env.get("__EGL_VENDOR_LIBRARY_FILENAMES") or env.get("DESKLAND_DISABLE_GPU_LOCK"):
        return None

    nvidia_json = _find_vendor_json(egl_dir, "nvidia")
    mesa_json = _find_vendor_json(egl_dir, "mesa")
    if not nvidia_json or not mesa_json:
        return None

    connected = _connected_gpu_vendors(drm_root)
    if not connected:
        return None

    if connected <= _MESA_VENDORS:
        return str(mesa_json)
    if connected == {_NVIDIA_VENDOR}:
        return str(nvidia_json)
    return None


def should_disable_dmabuf(
    environ: MutableMapping[str, str] | None = None,
    drm_root: Path = Path("/sys/class/drm"),
    disable_dmabuf_setting: bool = False,
) -> bool:
    """Return True when WebKitGTK DMA-BUF renderer should be disabled for stability."""
    env = os.environ if environ is None else environ
    if "WEBKIT_DISABLE_DMABUF_RENDERER" in env:
        return False
    if env.get("DESKLAND_KEEP_DMABUF"):
        return False
    if disable_dmabuf_setting or env.get("DESKLAND_DISABLE_DMABUF"):
        return True
    if env.get("WAYLAND_DISPLAY"):
        connected = _connected_gpu_vendors(drm_root)
        if _NVIDIA_VENDOR in connected:
            return True
    return False


def _read_disable_dmabuf_setting(settings_path: Path | None = None) -> bool:
    import json

    if settings_path is None:
        xdg = os.environ.get("XDG_CONFIG_HOME")
        base = Path(xdg) if xdg else (Path.home() / ".config")
        settings_path = base / "deskland" / "settings.json"
    try:
        if not settings_path.is_file():
            return False
        data = json.loads(settings_path.read_text(encoding="utf-8"))
        return bool(isinstance(data, dict) and data.get("disableDmabuf"))
    except (OSError, ValueError):
        return False


def configure_single_gpu_env(
    environ: MutableMapping[str, str] | None = None,
    settings_path: Path | None = None,
    drm_root: Path = Path("/sys/class/drm"),
    egl_dir: Path = Path("/usr/share/glvnd/egl_vendor.d"),
) -> str | None:
    """Set EGL vendor and WebKitGTK DMA-BUF fallback before GTK/WebKit initializes."""
    env = os.environ if environ is None else environ
    chosen = select_single_egl_vendor(environ=env, drm_root=drm_root, egl_dir=egl_dir)
    if chosen:
        env["__EGL_VENDOR_LIBRARY_FILENAMES"] = chosen
    if should_disable_dmabuf(
        environ=env,
        drm_root=drm_root,
        disable_dmabuf_setting=_read_disable_dmabuf_setting(settings_path),
    ):
        env["WEBKIT_DISABLE_DMABUF_RENDERER"] = "1"
    return chosen
