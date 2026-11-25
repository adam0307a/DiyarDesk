"""Import external theme packages (.zip/.deskland), HTML pages, images, or videos."""

from __future__ import annotations

import html
import json
import os
import platform
import posixpath
import re
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path
from typing import Any

from .config import atomic_write, config_dir

ARCHIVE_EXTS = {".zip", ".deskland"}
HTML_EXTS = {".html", ".htm"}
IMAGE_EXTS = {".png", ".jpg", ".jpeg", ".webp", ".gif", ".svg", ".avif", ".bmp"}
VIDEO_EXTS = {".mp4", ".webm", ".mkv", ".mov"}
SHADER_EXTS = {".glsl", ".frag"}
RUST_SOURCE_EXTS = {".rs", ".toml", ".lock"}
NATIVE_BIN_EXTS = {".so", ".dll", ".dylib"}
SUPPORTED_EXTS = ARCHIVE_EXTS | HTML_EXTS | IMAGE_EXTS | VIDEO_EXTS | SHADER_EXTS

ARCHIVE_ALLOWED_EXTS = (
    HTML_EXTS
    | IMAGE_EXTS
    | VIDEO_EXTS
    | {
        ".css",
        ".js",
        ".mjs",
        ".json",
        ".glsl",
        ".vert",
        ".frag",
        ".wgsl",
        ".wasm",
        ".txt",
        ".md",
        ".xml",
        ".csv",
        ".map",
        ".ico",
        ".ogv",
        ".mp3",
        ".wav",
        ".ogg",
        ".opus",
        ".m4a",
        ".aac",
        ".flac",
        ".woff",
        ".woff2",
        ".ttf",
        ".otf",
        ".eot",
        ".obj",
        ".mtl",
        ".gltf",
        ".glb",
        ".bin",
        ".hdr",
        ".exr",
        ".ktx",
        ".ktx2",
        ".dds",
        ".ply",
        ".stl",
        ".fbx",
    }
)
ARCHIVE_ALLOWED_BASENAMES = {"license", "notice", "readme", "copying", "authors", "changelog"}

MAX_ARCHIVE_BYTES = 150 * 1024 * 1024
MAX_ARCHIVE_FILES = 2000


def _is_allowed_archive_member(file_name: str, *, allow_native: bool = False) -> bool:
    ext = Path(file_name).suffix.lower()
    if ext:
        if ext in ARCHIVE_ALLOWED_EXTS:
            return True
        if allow_native and ext in (RUST_SOURCE_EXTS | NATIVE_BIN_EXTS):
            return True
        return False
    return file_name.lower() in ARCHIVE_ALLOWED_BASENAMES


def _platform_bin_subdir() -> str:
    os_name = "windows" if sys.platform.startswith("win") else ("macos" if sys.platform == "darwin" else "linux")
    raw_arch = (platform.machine() or "x86_64").lower()
    arch = "x86_64" if raw_arch in ("amd64", "x86_64", "x64") else ("aarch64" if raw_arch in ("arm64", "aarch64") else raw_arch)
    return f"{os_name}-{arch}"


def _platform_lib_ext() -> str:
    if sys.platform.startswith("win"):
        return ".dll"
    if sys.platform == "darwin":
        return ".dylib"
    return ".so"


def custom_themes_dir() -> Path:
    override = os.environ.get("DESKLAND_CUSTOM_THEMES_DIR")
    if override:
        return Path(override).expanduser().resolve()
    return config_dir() / "themes"


def registry_path(custom_dir: Path) -> Path:
    return custom_dir / "registry.json"


def load_custom_themes(custom_dir: Path) -> list[dict[str, Any]]:
    path = registry_path(custom_dir)
    try:
        if not path.exists():
            return []
        data = json.loads(path.read_text(encoding="utf-8"))
        themes = data.get("themes") if isinstance(data, dict) else None
        if not isinstance(themes, list):
            return []
        return [t for t in themes if isinstance(t, dict)]
    except (OSError, ValueError):
        return []


def save_custom_themes(custom_dir: Path, themes: list[dict[str, Any]]) -> None:
    custom_dir.mkdir(parents=True, exist_ok=True)
    raw = json.dumps({"themes": themes}, ensure_ascii=False, indent=2).encode("utf-8")
    atomic_write(registry_path(custom_dir), raw)


def _slugify(name: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")
    if not slug:
        slug = "theme"
    return f"custom-{slug[:36]}"


def _unique_id(base: str, existing: set[str], custom_dir: Path) -> str:
    candidate = base
    n = 2
    while candidate in existing or (custom_dir / candidate).exists():
        candidate = f"{base}-{n}"
        n += 1
    return candidate


def _pretty_title(stem: str) -> str:
    cleaned = re.sub(r"[-_]+", " ", stem).strip()
    return cleaned or "Imported Theme"


def _extract_html_title(content: str, fallback: str) -> str:
    m = re.search(r"<title[^>]*>(.*?)</title>", content, flags=re.IGNORECASE | re.DOTALL)
    if m:
        title = re.sub(r"\s+", " ", html.unescape(m.group(1))).strip()
        if title:
            return title[:60]
    return fallback


def _safe_zip_members(
    zf: zipfile.ZipFile, *, allow_native: bool = False
) -> list[tuple[zipfile.ZipInfo, list[str]]]:
    entries: list[tuple[zipfile.ZipInfo, list[str]]] = []
    total_size = 0
    for info in zf.infolist():
        if info.is_dir():
            continue
        # Reject symlinks (Unix mode 0o120000 in upper 16 bits of external_attr)
        if (info.external_attr >> 16) & 0o170000 == 0o120000:
            raise ValueError("Archive contains unsupported symbolic link")
        norm = posixpath.normpath(info.filename.replace("\\", "/"))
        if norm.startswith("/") or norm == ".." or norm.startswith("../"):
            raise ValueError(f"Unsafe archive entry path: {info.filename}")
        parts = [p for p in norm.split("/") if p and p != "."]
        if not parts or any(p == ".." for p in parts):
            raise ValueError(f"Unsafe archive entry path: {info.filename}")
        if parts[0] == "__MACOSX":
            continue
        if not _is_allowed_archive_member(parts[-1], allow_native=allow_native):
            raise ValueError(f"Unsupported file type in theme archive: {info.filename}")
        total_size += max(0, info.file_size)
        if total_size > MAX_ARCHIVE_BYTES:
            raise ValueError("Theme archive exceeds 150 MB size limit")
        if len(entries) >= MAX_ARCHIVE_FILES:
            raise ValueError("Theme archive contains too many files")
        entries.append((info, parts))
    if not entries:
        raise ValueError("Archive contains no files")

    top_dirs = {parts[0] for _, parts in entries if len(parts) > 1}
    has_root_files = any(len(parts) == 1 for _, parts in entries)
    if not has_root_files and len(top_dirs) == 1:
        entries = [(info, parts[1:]) for info, parts in entries]
    return entries


def _find_entry_html(theme_dir: Path, preferred: str | None = None) -> str:
    if preferred is not None:
        if not isinstance(preferred, str) or not preferred.strip():
            raise ValueError("Invalid entry path in theme manifest")
        clean = preferred.strip().lstrip("/").replace("\\", "/")
        cand = (theme_dir / clean).resolve()
        try:
            cand.relative_to(theme_dir.resolve())
        except ValueError as exc:
            raise ValueError(f"Unsafe entry path in theme manifest: {preferred}") from exc
        if not cand.is_file() or cand.suffix.lower() not in HTML_EXTS:
            raise ValueError(f"Entry HTML file not found in archive: {clean}")
        return clean
    if (theme_dir / "index.html").is_file():
        return "index.html"
    root_html = sorted(
        p.name for p in theme_dir.iterdir() if p.is_file() and p.suffix.lower() in HTML_EXTS
    )
    if root_html:
        return root_html[0]
    nested_html = sorted(
        p.relative_to(theme_dir).as_posix()
        for p in theme_dir.rglob("*")
        if p.is_file() and p.suffix.lower() in HTML_EXTS
    )
    if nested_html:
        return nested_html[0]
    raise ValueError("No .html entry page found in archive")


def _sanitize_color(value: Any, fallback: str = "#4a9eff") -> str:
    if isinstance(value, str):
        s = value.strip()
        if re.match(r"^#[0-9a-fA-F]{6}$", s):
            return s.lower()
        if re.match(r"^#[0-9a-fA-F]{3}$", s):
            return ("#" + s[1] * 2 + s[2] * 2 + s[3] * 2).lower()
    return fallback


def _sanitize_groups(raw_groups: Any) -> list[dict[str, Any]]:
    if not isinstance(raw_groups, list):
        return []
    clean_groups: list[dict[str, Any]] = []
    for g in raw_groups:
        if not isinstance(g, dict) or not isinstance(g.get("params"), list):
            continue
        clean_params: list[dict[str, Any]] = []
        for p in g["params"]:
            if not isinstance(p, dict):
                continue
            key = str(p.get("key") or "").strip()
            label = str(p.get("label") or key).strip()
            if not key or not re.match(r"^[a-zA-Z0-9_-]+$", key):
                continue
            item: dict[str, Any] = {"key": key, "label": label[:48]}
            if isinstance(p.get("hint"), str):
                item["hint"] = p["hint"][:180]
            if str(p.get("type") or "").strip().lower() == "color":
                item["type"] = "color"
                item["default"] = _sanitize_color(p.get("default") if "default" in p else p.get("value"))
            elif isinstance(p.get("options"), list) and p["options"]:
                raw_opts = p["options"][:8]
                opts: list[str] = []
                vals: list[Any] = []
                has_obj_opts = False
                for o in raw_opts:
                    if isinstance(o, dict):
                        has_obj_opts = True
                        lbl = str(o.get("label") if o.get("label") is not None else o.get("value", "")).strip()
                        val = o.get("value") if "value" in o else lbl
                        opts.append(lbl[:48] or str(val)[:48])
                        vals.append(val)
                    else:
                        opts.append(str(o)[:48])
                raw_vals = p.get("values")
                if not has_obj_opts and isinstance(raw_vals, list) and len(raw_vals) >= len(opts):
                    vals = list(raw_vals[: len(opts)])
                raw_def = p.get("default")
                if vals:
                    item["options"] = opts
                    item["values"] = vals
                    if raw_def in vals:
                        item["default"] = raw_def
                    elif isinstance(raw_def, str) and raw_def in opts:
                        item["default"] = vals[opts.index(raw_def)]
                    elif isinstance(raw_def, (int, float)) and not isinstance(raw_def, bool):
                        idx = max(0, min(len(vals) - 1, int(raw_def)))
                        item["default"] = vals[idx]
                    else:
                        item["default"] = vals[0]
                else:
                    default_idx = int(raw_def) if isinstance(raw_def, (int, float)) and not isinstance(raw_def, bool) else 0
                    item["options"] = opts
                    item["default"] = max(0, min(len(opts) - 1, default_idx))
            else:
                try:
                    p_min = float(p.get("min", 0))
                    p_max = float(p.get("max", 1))
                    p_step = float(p.get("step", 0.01))
                    p_def = float(p.get("default", p_min))
                except (TypeError, ValueError):
                    continue
                if p_max <= p_min or p_step <= 0:
                    continue
                item["min"] = p_min
                item["max"] = p_max
                item["step"] = p_step
                item["default"] = max(p_min, min(p_max, p_def))
                if p.get("format") in ("times", "percent"):
                    item["format"] = p["format"]
                elif isinstance(p.get("unit"), str) and p["unit"]:
                    item["unit"] = p["unit"][:12]
            clean_params.append(item)
        if clean_params:
            clean_groups.append({"name": str(g.get("name") or "Settings")[:48], "params": clean_params})
    return clean_groups


def _groups_from_settings(raw_settings: Any) -> list[dict[str, Any]]:
    """Convert flat/grouped `manifest.settings` dict (or list) into DiyarDesk `groups`."""
    items: list[tuple[str, dict[str, Any]]] = []
    if isinstance(raw_settings, dict):
        for k, v in raw_settings.items():
            if isinstance(v, dict):
                items.append((str(v.get("key") or k), v))
    elif isinstance(raw_settings, list):
        for v in raw_settings:
            if isinstance(v, dict) and v.get("key"):
                items.append((str(v["key"]), v))
    if not items:
        return []

    grouped: dict[str, list[dict[str, Any]]] = {}
    for key, spec in items:
        group_name = str(spec.get("group") or spec.get("category") or "Settings").strip() or "Settings"
        ptype = str(spec.get("type") or "").strip().lower()
        if ptype in ("button", "label", "folderdropdown", "file"):
            continue
        param: dict[str, Any] = {
            "key": key,
            "label": spec.get("label") or spec.get("title") or spec.get("text") or key,
        }
        if isinstance(spec.get("hint"), str):
            param["hint"] = spec["hint"]
        elif isinstance(spec.get("description"), str):
            param["hint"] = spec["description"]

        raw_default = spec["default"] if "default" in spec else spec.get("value")

        if ptype == "color":
            param["type"] = "color"
            param["default"] = _sanitize_color(raw_default)
        elif ptype in ("checkbox", "boolean", "bool", "toggle"):
            param["options"] = ["Off", "On"]
            param["values"] = [False, True]
            if isinstance(raw_default, bool):
                param["default"] = raw_default
            elif isinstance(raw_default, (int, float)):
                param["default"] = bool(raw_default)
            elif isinstance(raw_default, str):
                param["default"] = raw_default.strip().lower() in ("1", "true", "yes", "on")
            else:
                param["default"] = False
        elif (isinstance(spec.get("options"), list) and spec["options"]) or (
            ptype in ("dropdown", "select") and isinstance(spec.get("items"), list) and spec["items"]
        ):
            param["options"] = spec.get("options") if isinstance(spec.get("options"), list) and spec["options"] else spec["items"]
            if "default" in spec or "value" in spec:
                param["default"] = raw_default
        else:
            param["min"] = spec.get("min", 0)
            param["max"] = spec.get("max", 1)
            param["step"] = spec.get("step", 0.01)
            param["default"] = raw_default if raw_default is not None else param["min"]
            if spec.get("format") in ("times", "percent"):
                param["format"] = spec["format"]
            elif isinstance(spec.get("unit"), str) and spec["unit"]:
                param["unit"] = spec["unit"]

        grouped.setdefault(group_name, []).append(param)

    return _sanitize_groups([{"name": gname, "params": params} for gname, params in grouped.items()])


def _shader_wrapper_html(title: str, glsl_source: str) -> str:
    safe_title = html.escape(title)
    encoded_glsl = json.dumps(glsl_source)
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{safe_title}</title>
<style>html,body,canvas{{margin:0;padding:0;width:100%;height:100%;overflow:hidden;background:#090c14;display:block;}}</style>
</head>
<body>
<canvas id="gl"></canvas>
<script>
(() => {{
  const canvas = document.getElementById('gl');
  const gl = canvas.getContext('webgl', {{ preserveDrawingBuffer: true, antialias: false }});
  if (!gl) return;
  const rawFrag = {encoded_glsl};
  const hasMainImage = /void\\s+mainImage\\s*\\(/.test(rawFrag);
  const header = [
    'precision highp float;',
    'uniform float u_time;',
    'uniform vec2 u_resolution;',
    'uniform vec2 u_mouse;',
    'uniform float u_speed;',
    'uniform float u_brightness;',
    'uniform float u_audio;',
    'uniform int u_iconCount;',
    'uniform vec4 u_icons[32];',
    'uniform vec3 iResolution;',
    'uniform float iTime;',
    'uniform vec4 iMouse;',
  ].join('\\n') + '\\n';
  const stripped = rawFrag.replace(/^\\s*precision\\s+\\w+\\s+float\\s*;/gm, '');
  const fragSrc = header + stripped + (hasMainImage
    ? '\\nvoid main(){{ vec4 c=vec4(0.0); mainImage(c, gl_FragCoord.xy); gl_FragColor=vec4(c.rgb*u_brightness, 1.0); }}\\n'
    : '');
  const vertSrc = 'attribute vec2 a_pos; void main(){{ gl_Position = vec4(a_pos, 0.0, 1.0); }}';
  function compile(type, src) {{
    const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) || 'shader compile error');
    return s;
  }}
  const prog = gl.createProgram();
  try {{
    gl.attachShader(prog, compile(gl.VERTEX_SHADER, vertSrc));
    gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, fragSrc));
    gl.linkProgram(prog);
  }} catch (err) {{
    document.body.innerHTML = '<pre style="color:#f78;padding:20px;font:13px monospace">' + String(err) + '</pre>';
    return;
  }}
  gl.useProgram(prog);
  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, -1,1, -1,1, 1,-1, 1,1]), gl.STATIC_DRAW);
  const locPos = gl.getAttribLocation(prog, 'a_pos');
  gl.enableVertexAttribArray(locPos);
  gl.vertexAttribPointer(locPos, 2, gl.FLOAT, false, 0, 0);

  const U = name => gl.getUniformLocation(prog, name);
  const uTime = U('u_time'), uRes = U('u_resolution'), uMouse = U('u_mouse'),
        uSpeed = U('u_speed'), uBright = U('u_brightness'), uAudio = U('u_audio'),
        uIconCount = U('u_iconCount'), uIcons = U('u_icons'),
        iRes = U('iResolution'), iTime = U('iTime'), iMouse = U('iMouse');

  const cfg = {{ speed: 1.0, brightness: 1.0 }};
  let paused = false, mouseX = 0, mouseY = 0, audioRms = 0, icons = new Float32Array(128), iconCount = 0;
  let simTime = 0, lastTs = performance.now();

  function resize() {{
    const dpr = Math.min(window.devicePixelRatio || 1, 1.5);
    canvas.width = Math.max(1, Math.floor(innerWidth * dpr));
    canvas.height = Math.max(1, Math.floor(innerHeight * dpr));
    gl.viewport(0, 0, canvas.width, canvas.height);
  }}
  addEventListener('resize', resize);
  resize();

  function applyMsg(raw) {{
    let m = raw;
    if (typeof m === 'string') {{ try {{ m = JSON.parse(m); }} catch (_) {{ return; }} }}
    if (!m || typeof m !== 'object') return;
    if (m.t === 'settings' && m.v) {{
      if (typeof m.v.speed === 'number') cfg.speed = m.v.speed;
      if (typeof m.v.brightness === 'number') cfg.brightness = m.v.brightness;
    }} else if (m.t === 'reset') {{
      cfg.speed = 1.0; cfg.brightness = 1.0;
    }} else if (m.t === 'pause') {{
      paused = !!m.v;
    }} else if (m.t === 'pointer' && typeof m.x === 'number') {{
      mouseX = m.x * (window.devicePixelRatio || 1);
      mouseY = canvas.height - m.y * (window.devicePixelRatio || 1);
    }} else if (m.t === 'audioLevels' && typeof m.rms === 'number') {{
      audioRms = m.rms;
    }} else if (m.t === 'icons' && Array.isArray(m.r)) {{
      iconCount = Math.min(32, m.r.length);
      for (let i = 0; i < iconCount; i++) {{
        const r = m.r[i] || [0,0,0,0];
        icons[i*4] = r[0] || 0; icons[i*4+1] = r[1] || 0; icons[i*4+2] = r[2] || 0; icons[i*4+3] = r[3] || 0;
      }}
    }}
  }}
  addEventListener('message', e => applyMsg(e.data));
  if (window.chrome && window.chrome.webview) window.chrome.webview.addEventListener('message', e => applyMsg(e.data));
  addEventListener('pointermove', e => {{ mouseX = e.clientX * (window.devicePixelRatio||1); mouseY = canvas.height - e.clientY * (window.devicePixelRatio||1); }}, {{ passive: true }});

  function render(ts) {{
    requestAnimationFrame(render);
    const dt = Math.min(0.1, Math.max(0, (ts - lastTs) / 1000));
    lastTs = ts;
    if (paused) return;
    simTime += dt * cfg.speed;
    if (uTime) gl.uniform1f(uTime, simTime);
    if (iTime) gl.uniform1f(iTime, simTime);
    if (uRes) gl.uniform2f(uRes, canvas.width, canvas.height);
    if (iRes) gl.uniform3f(iRes, canvas.width, canvas.height, 1.0);
    if (uMouse) gl.uniform2f(uMouse, mouseX, mouseY);
    if (iMouse) gl.uniform4f(iMouse, mouseX, mouseY, 0, 0);
    if (uSpeed) gl.uniform1f(uSpeed, cfg.speed);
    if (uBright) gl.uniform1f(uBright, cfg.brightness);
    if (uAudio) gl.uniform1f(uAudio, audioRms);
    if (uIconCount) gl.uniform1i(uIconCount, iconCount);
    if (uIcons) gl.uniform4fv(uIcons, icons);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  }}
  window.desklandSnapshot = () => canvas.toDataURL('image/png');
  requestAnimationFrame(render);
  if (new URLSearchParams(location.search).get('preview') === '1') parent.postMessage(JSON.stringify({{ t: 'ready' }}), '*');
  else if (window.chrome && window.chrome.webview) window.chrome.webview.postMessage('ready');
}})();
</script>
</body>
</html>
"""


def _validate_html_file(path: Path) -> str:
    raw = path.read_bytes()
    if not raw or b"\x00" in raw:
        raise ValueError(f"HTML file is empty or corrupted: {path.name}")
    return raw.decode("utf-8", errors="replace")


def _copy_limited(src_fp, dst_fp, remaining_bytes: int) -> int:
    written = 0
    while True:
        chunk = src_fp.read(65536)
        if not chunk:
            break
        written += len(chunk)
        if written > remaining_bytes:
            raise ValueError("Archive uncompressed size exceeds 150 MB limit")
        dst_fp.write(chunk)
    return written


def _find_archive_thumb(theme_dir: Path, theme_id: str, manifest_thumb: Any) -> str:
    if isinstance(manifest_thumb, str) and manifest_thumb.strip():
        val = manifest_thumb.strip()
        if val.startswith(("radial-gradient", "linear-gradient", "#")):
            return val
        rel = val.lstrip("/")
        cand = (theme_dir / rel).resolve()
        try:
            cand.relative_to(theme_dir.resolve())
            if cand.is_file():
                return f"url(__custom/{theme_id}/{rel}) center / cover no-repeat"
        except ValueError:
            pass
    for name in (
        "thumb.webp",
        "thumb.png",
        "thumb.jpg",
        "thumb.svg",
        "preview.webp",
        "preview.png",
        "preview.jpg",
        "preview.svg",
    ):
        if (theme_dir / name).is_file():
            return f"url(__custom/{theme_id}/{name}) center / cover no-repeat"
    return (
        "radial-gradient(circle at 28% 35%, rgba(120,210,255,0.45) 0%, transparent 45%), "
        "radial-gradient(circle at 75% 70%, rgba(180,120,255,0.38) 0%, transparent 48%), "
        "linear-gradient(145deg, #1b2738 0%, #243b55 55%, #16222f 100%)"
    )


def _media_wrapper_html(media_filename: str, is_video: bool, title: str) -> str:
    safe_title = html.escape(title)
    safe_media = html.escape(media_filename)
    tag = (
        f'<video id="media" src="{safe_media}" autoplay loop muted playsinline></video>'
        if is_video
        else f'<img id="media" src="{safe_media}" alt="{safe_title}">'
    )
    speed_block = (
        "  if (!isVideo) return;\n"
        "  media.playbackRate = Math.max(0.25, Math.min(2.0, P.speed || 1));\n"
    )
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{safe_title}</title>
<style>
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  html, body {{ width: 100%; height: 100%; overflow: hidden; background: #0d1117; }}
  #stage {{ position: fixed; inset: 0; overflow: hidden; }}
  #media {{
    position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover;
    transform-origin: center center; will-change: transform, filter;
  }}
  #halo {{
    position: fixed; inset: 0; width: 100%; height: 100%; pointer-events: none;
  }}
  #ghosts {{ position: fixed; inset: 0; pointer-events: none; display: none; }}
  body.preview #ghosts {{ display: block; }}
  .ghost {{
    position: absolute; display: flex; flex-direction: column; align-items: center;
    padding-top: 6px; opacity: .28;
  }}
  .ghost i {{ width: 40px; height: 40px; border-radius: 9px; background: #fff; box-shadow: 0 2px 6px rgba(0,0,0,.35); }}
  .ghost b {{ width: 46px; height: 6px; margin-top: 8px; border-radius: 3px; background: #fff; }}
</style>
</head>
<body>
<div id="stage">{tag}</div>
<canvas id="halo"></canvas>
<script>
'use strict';
const QS = new URLSearchParams(location.search);
const WALL = QS.has('wallpaper');
const PREVIEW = QS.has('preview');
const isVideo = {'true' if is_video else 'false'};
const P = {{ brightness: 1, dimIcons: 0.28, {'speed: 1' if is_video else 'motion: 0.35'}, adaptive: 1 }};
const DEFAULTS = Object.assign({{}}, P);
const media = document.getElementById('media');
const haloCv = document.getElementById('halo');
const haloCtx = haloCv.getContext('2d');
let icons = [];
let paused = false;
let t0 = performance.now();

function resize() {{
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  haloCv.width = Math.max(1, Math.round(innerWidth * dpr));
  haloCv.height = Math.max(1, Math.round(innerHeight * dpr));
  haloCtx.setTransform(dpr, 0, 0, dpr, 0, 0);
  drawHalo();
}}

function drawHalo() {{
  haloCtx.clearRect(0, 0, innerWidth, innerHeight);
  if (!P.adaptive || P.dimIcons <= 0.01 || !icons.length) return;
  const a = Math.min(0.75, Math.max(0, P.dimIcons)) * 0.65;
  for (const ic of icons) {{
    const cx = ic.x + ic.w * 0.5, cy = ic.y + ic.h * 0.5;
    const rad = Math.max(ic.w, ic.h) * 0.9;
    const g = haloCtx.createRadialGradient(cx, cy, rad * 0.15, cx, cy, rad);
    g.addColorStop(0, 'rgba(8, 12, 18, ' + a.toFixed(3) + ')');
    g.addColorStop(1, 'rgba(8, 12, 18, 0)');
    haloCtx.fillStyle = g;
    haloCtx.beginPath();
    haloCtx.arc(cx, cy, rad, 0, Math.PI * 2);
    haloCtx.fill();
  }}
}}

function syncVisuals(now) {{
  const b = Math.max(0.35, Math.min(1.5, P.brightness || 1));
  media.style.filter = 'brightness(' + b.toFixed(2) + ')';
  if (isVideo) {{
{speed_block}
    media.style.transform = 'scale(1)';
  }} else {{
    const amt = Math.max(0, Math.min(1, P.motion || 0));
    if (amt <= 0.01) {{
      media.style.transform = 'scale(1)';
    }} else {{
      const s = (now - t0) * 0.00035;
      const sc = 1 + amt * (0.04 + 0.03 * (0.5 + 0.5 * Math.sin(s)));
      const dx = Math.sin(s * 0.7) * amt * 10;
      const dy = Math.cos(s * 0.53) * amt * 7;
      media.style.transform = 'translate(' + dx.toFixed(1) + 'px,' + dy.toFixed(1) + 'px) scale(' + sc.toFixed(4) + ')';
    }}
  }}
}}

function tick(now) {{
  if (!paused) syncVisuals(now);
  requestAnimationFrame(tick);
}}

function applySettings(v) {{
  for (const k in v) {{
    if (k in P && typeof v[k] === 'number') P[k] = v[k];
  }}
  syncVisuals(performance.now());
  drawHalo();
}}

function drawGhosts() {{
  const box = document.getElementById('ghosts');
  if (!box) return;
  box.innerHTML = icons.map(ic => '<div class="ghost" style="left:' + ic.x + 'px;top:' + ic.y + 'px;width:' + ic.w + 'px;height:' + ic.h + 'px"><i></i><b></b></div>').join('');
}}

function handleMsg(m) {{
  if (!m) return;
  if (m.t === 'pause') {{
    paused = PREVIEW ? false : !!m.v;
    if (isVideo) {{
      if (paused) media.pause(); else media.play().catch(() => {{}});
    }}
    return;
  }}
  if (m.t === 'settings') {{ applySettings(m.v || {{}}); return; }}
  if (m.t === 'reset') {{ applySettings(Object.assign({{}}, DEFAULTS, m.v || {{}})); return; }}
  if (m.t === 'icons') {{
    const k = window.devicePixelRatio || 1;
    icons = m.vis ? (m.r || []).map(([l, t, r, b]) => ({{ x: l / k, y: t / k, w: (r - l) / k, h: (b - t) / k }})) : [];
    if (PREVIEW) drawGhosts();
    drawHalo();
  }}
}}

if (PREVIEW) {{
  document.body.classList.add('preview');
  const g = document.createElement('div');
  g.id = 'ghosts';
  document.body.appendChild(g);
  addEventListener('message', ev => {{
    if (ev.source === parent) handleMsg(typeof ev.data === 'string' ? JSON.parse(ev.data) : ev.data);
  }});
  parent.postMessage(JSON.stringify({{ t: 'ready' }}), '*');
}} else if (WALL && window.chrome && window.chrome.webview) {{
  window.chrome.webview.addEventListener('message', ev => handleMsg(JSON.parse(ev.data)));
  window.chrome.webview.postMessage('ready');
}}

window.desklandSnapshot = function () {{
  const cw = Math.max(320, innerWidth), ch = Math.max(180, innerHeight);
  const c = document.createElement('canvas');
  c.width = cw; c.height = ch;
  const ctx = c.getContext('2d');
  ctx.fillStyle = '#0d1117';
  ctx.fillRect(0, 0, cw, ch);
  try {{
    const mw = isVideo ? (media.videoWidth || cw) : (media.naturalWidth || cw);
    const mh = isVideo ? (media.videoHeight || ch) : (media.naturalHeight || ch);
    const sc = Math.max(cw / mw, ch / mh);
    const dw = mw * sc, dh = mh * sc;
    ctx.drawImage(media, (cw - dw) / 2, (ch - dh) / 2, dw, dh);
  }} catch (_) {{}}
  ctx.drawImage(haloCv, 0, 0, cw, ch);
  const tw = 760, th = Math.round(tw * ch / cw);
  const tc = document.createElement('canvas');
  tc.width = tw; tc.height = th;
  tc.getContext('2d').drawImage(c, 0, 0, tw, th);
  return {{ canvas: c, thumb: tc.toDataURL('image/jpeg', 0.88) }};
}};

addEventListener('resize', resize);
resize();
requestAnimationFrame(tick);
</script>
</body>
</html>
"""


def _media_params(is_video: bool) -> list[dict[str, Any]]:
    second = (
        {
            "key": "speed",
            "label": "Playback speed",
            "hint": "Speed multiplier for animated video playback",
            "min": 0.25,
            "max": 2.0,
            "step": 0.05,
            "default": 1,
            "format": "times",
        }
        if is_video
        else {
            "key": "motion",
            "label": "Gentle drift",
            "hint": "Slow ambient zoom and drift across the wallpaper",
            "min": 0,
            "max": 1,
            "step": 0.01,
            "default": 0.35,
            "format": "percent",
        }
    )
    return [
        {
            "name": "Appearance",
            "params": [
                {
                    "key": "brightness",
                    "label": "Brightness",
                    "hint": "Overall brightness of the imported wallpaper",
                    "min": 0.4,
                    "max": 1.4,
                    "step": 0.01,
                    "default": 1,
                    "format": "percent",
                },
                second,
                {
                    "key": "dimIcons",
                    "label": "Icon halo",
                    "hint": "Soft dark backdrop behind desktop icons for legibility",
                    "min": 0,
                    "max": 1,
                    "step": 0.01,
                    "default": 0.28,
                    "format": "percent",
                },
            ],
        }
    ]


def _normalize_manifest_engine(raw: Any) -> str:
    val = str(raw or "").strip().lower()
    if val in ("opengl-rust", "rust-opengl", "rust-gl", "rust", "plugin", "cdylib"):
        return "opengl-rust"
    if val in ("opengl-glsl", "opengl", "gl", "glsl", "shader"):
        return "opengl-glsl"
    if val in ("web", "html", "webview", "webkit"):
        return "web"
    return ""


def _native_gl_card_html(title: str, engine: str) -> str:
    safe_title = html.escape(title)
    badge = "Rust + OpenGL 3.3 Plugin" if engine == "opengl-rust" else "OpenGL 3.3 Core GLSL"
    return (
        "<!doctype html><html><head><meta charset='utf-8'>"
        f"<title>{safe_title}</title>"
        "<style>html,body{margin:0;height:100%;display:grid;place-items:center;"
        "background:radial-gradient(circle at 30% 30%,#1b2640,#0c101b 70%);color:#e6eefc;"
        "font-family:system-ui,sans-serif;text-align:center}"
        ".pill{display:inline-block;padding:6px 14px;border-radius:999px;"
        "background:rgba(50,197,184,.18);border:1px solid rgba(50,197,184,.45);"
        "color:#7ee8de;font-size:12px;margin-bottom:10px}</style></head>"
        f"<body><div><div class='pill'>{badge}</div><h2>{safe_title}</h2>"
        "<p style='opacity:.72;font-size:13px'>Bu tema yerel Rust + OpenGL 3.3 motorunda çalışır.</p>"
        "</div><script>try{window.chrome&&window.chrome.webview&&window.chrome.webview.postMessage('ready')}catch(e){}</script>"
        "</body></html>"
    )


def inspect_theme_archive(source: Path) -> dict[str, Any]:
    """Inspect a theme file/archive before import and classify its target engine(s)."""
    src = Path(source)
    if not src.is_file():
        raise ValueError(f"Selected file does not exist: {src}")
    if src.stat().st_size == 0:
        raise ValueError("Selected file is empty.")
    ext = src.suffix.lower()
    if ext not in SUPPORTED_EXTS:
        raise ValueError(f"Unsupported file type: {ext or 'none'}")

    title = _pretty_title(src.stem)
    if ext in SHADER_EXTS:
        head = src.read_bytes()[:2048].decode("utf-8", errors="ignore")
        if ("#version 330" in head or "#version 4" in head) and "mainImage(" not in head:
            return {
                "kind": "opengl-glsl",
                "engines": ["opengl-glsl"],
                "requires_native_trust": False,
                "has_rust_source": False,
                "has_prebuilt_binary": False,
                "name": title,
            }
        return {
            "kind": "web",
            "engines": ["web"],
            "requires_native_trust": False,
            "has_rust_source": False,
            "has_prebuilt_binary": False,
            "name": title,
        }

    if ext not in ARCHIVE_EXTS:
        return {
            "kind": "web",
            "engines": ["web"],
            "requires_native_trust": False,
            "has_rust_source": False,
            "has_prebuilt_binary": False,
            "name": title,
        }

    try:
        zf_ctx = zipfile.ZipFile(src, "r")
    except zipfile.BadZipFile as exc:
        raise ValueError("Archive is corrupted or not a valid ZIP file.") from exc

    with zf_ctx as zf:
        entries = _safe_zip_members(zf, allow_native=True)
        rel_paths = ["/".join(parts) for _, parts in entries]
        rel_lower = {p.lower(): info for (info, parts), p in zip(entries, rel_paths)}

        manifest: dict[str, Any] = {}
        for mname in ("manifest.json", "theme.json"):
            if mname in rel_lower:
                try:
                    raw = zf.read(rel_lower[mname]).decode("utf-8", errors="replace")
                    parsed = json.loads(raw)
                    if isinstance(parsed, dict):
                        manifest = parsed
                        break
                except (OSError, ValueError):
                    pass

        name = str(manifest.get("name") or manifest.get("title") or title)[:60]
        declared = _normalize_manifest_engine(manifest.get("engine"))

        has_cargo = "cargo.toml" in rel_lower
        has_rs = any(p.lower().endswith(".rs") for p in rel_paths)
        has_rust_source = has_cargo and has_rs
        has_prebuilt_binary = any(Path(p).suffix.lower() in NATIVE_BIN_EXTS for p in rel_paths)
        has_rust_plugin = has_rust_source or has_prebuilt_binary or declared == "opengl-rust"

        has_glsl = any(Path(p).suffix.lower() in SHADER_EXTS for p in rel_paths)
        has_html = any(Path(p).suffix.lower() in HTML_EXTS for p in rel_paths)

        # Reject archives containing disallowed/native extensions unless they are genuine Rust plugins
        if not has_rust_plugin:
            for _, parts in entries:
                if not _is_allowed_archive_member(parts[-1], allow_native=False):
                    raise ValueError(f"Unsupported file type in theme archive: {'/'.join(parts)}")

        engines: list[str] = []
        if has_rust_plugin:
            engines.append("opengl-rust")
        elif declared == "opengl-glsl" or (has_glsl and not has_html) or bool(manifest.get("shader")):
            engines.append("opengl-glsl")

        if has_html and declared not in ("opengl-glsl", "opengl-rust"):
            engines.append("web")
        elif has_html and declared in ("opengl-glsl", "opengl-rust"):
            # Dual package explicitly declaring OpenGL while also shipping a Web fallback
            if "web" not in engines and any(p.lower() == "index.html" for p in rel_paths):
                engines.append("web")

        if not engines:
            engines = ["web"]

        kind = "dual" if len(engines) > 1 else engines[0]
        return {
            "kind": kind,
            "engines": engines,
            "requires_native_trust": "opengl-rust" in engines,
            "has_rust_source": has_rust_source,
            "has_prebuilt_binary": has_prebuilt_binary,
            "name": name,
        }


def _safe_rel_file(theme_dir: Path, rel: str, allowed_exts: set[str] | None = None) -> str:
    clean = str(rel or "").strip().lstrip("/").replace("\\", "/")
    if not clean:
        raise ValueError("Empty relative path in theme manifest")
    cand = (theme_dir / clean).resolve()
    try:
        cand.relative_to(theme_dir.resolve())
    except ValueError as exc:
        raise ValueError(f"Unsafe path in theme manifest: {rel}") from exc
    if not cand.is_file():
        raise ValueError(f"Referenced asset not found in archive: {clean}")
    if allowed_exts is not None and cand.suffix.lower() not in allowed_exts:
        raise ValueError(f"Unsupported asset extension: {clean}")
    return clean


def _find_entry_glsl(theme_dir: Path, preferred: str | None = None) -> str:
    if preferred:
        return _safe_rel_file(theme_dir, preferred, SHADER_EXTS)
    for name in ("main.frag", "shader.frag", "main.glsl", "shader.glsl"):
        if (theme_dir / name).is_file():
            return name
    root_frag = sorted(
        p.name for p in theme_dir.iterdir() if p.is_file() and p.suffix.lower() in SHADER_EXTS
    )
    if root_frag:
        return root_frag[0]
    nested_frag = sorted(
        p.relative_to(theme_dir).as_posix()
        for p in theme_dir.rglob("*")
        if p.is_file() and p.suffix.lower() in SHADER_EXTS
    )
    if nested_frag:
        return nested_frag[0]
    raise ValueError("No .frag or .glsl fragment shader found in archive")


def _find_prebuilt_plugin(theme_dir: Path, preferred: str | None = None) -> str | None:
    ext = _platform_lib_ext()
    if preferred:
        clean = str(preferred).strip().lstrip("/").replace("\\", "/")
        cand = (theme_dir / clean).resolve()
        try:
            cand.relative_to(theme_dir.resolve())
            if cand.is_file() and cand.suffix.lower() == ext:
                return clean
        except ValueError:
            pass
    plat_dir = theme_dir / "bin" / _platform_bin_subdir()
    if plat_dir.is_dir():
        for p in sorted(plat_dir.iterdir()):
            if p.is_file() and p.suffix.lower() == ext:
                return p.relative_to(theme_dir).as_posix()
    for p in sorted(theme_dir.iterdir()):
        if p.is_file() and p.suffix.lower() == ext:
            return p.name
    return None


def _compile_rust_plugin(theme_dir: Path, timeout_sec: int = 300) -> str:
    """Compile a Rust `cdylib` theme plugin using system `cargo` into `bin/<plat>/plugin.{so,dll}`."""
    cargo_bin = shutil.which("cargo")
    if not cargo_bin:
        raise ValueError(
            "Rust eklenti paketini kaynak koddan derlemek için sistemde `cargo` (Rust araç zinciri) gerekli."
        )
    cargo_toml = theme_dir / "Cargo.toml"
    if not cargo_toml.is_file():
        raise ValueError("Rust eklenti paketinde Cargo.toml bulunamadı.")
    proc = subprocess.run(
        [cargo_bin, "build", "--release"],
        cwd=str(theme_dir),
        capture_output=True,
        text=True,
        timeout=timeout_sec,
        check=False,
    )
    if proc.returncode != 0:
        tail = (proc.stderr or proc.stdout or "").strip().splitlines()[-12:]
        raise ValueError("Rust eklentisi derlenemedi:\n" + "\n".join(tail))

    ext = _platform_lib_ext()
    rel_dir = theme_dir / "target" / "release"
    built = sorted(
        p for p in rel_dir.iterdir() if p.is_file() and p.suffix.lower() == ext
    ) if rel_dir.is_dir() else []
    if not built:
        raise ValueError(
            f"Cargo derlemesi tamamlandı ancak target/release altında {ext} (`crate-type = [\"cdylib\"]`) bulunamadı."
        )

    out_dir = theme_dir / "bin" / _platform_bin_subdir()
    out_dir.mkdir(parents=True, exist_ok=True)
    out_file = out_dir / f"plugin{ext}"
    shutil.copy2(built[0], out_file)
    shutil.rmtree(theme_dir / "target", ignore_errors=True)
    return out_file.relative_to(theme_dir).as_posix()


def import_theme_file(
    source: Path,
    custom_dir: Path | None = None,
    reserved_ids: set[str] | None = None,
    *,
    preferred_engine: str | None = None,
    allow_native_code: bool = False,
) -> dict[str, Any]:
    """Import a theme package (Web, OpenGL GLSL, or Rust cdylib plugin), HTML, shader, image, or video."""
    src = Path(source)
    if not src.is_file():
        raise ValueError(f"Selected file does not exist: {src}")
    if src.stat().st_size == 0:
        raise ValueError("Selected file is empty.")
    ext = src.suffix.lower()
    if ext not in SUPPORTED_EXTS:
        raise ValueError(f"Unsupported file type: {ext or 'none'}")
    if ext not in ARCHIVE_EXTS and src.stat().st_size > MAX_ARCHIVE_BYTES:
        raise ValueError("Selected file exceeds 150 MB size limit.")

    info_scan = inspect_theme_archive(src)
    chosen_engine = _normalize_manifest_engine(preferred_engine)
    if chosen_engine and chosen_engine not in info_scan["engines"]:
        chosen_engine = ""
    if not chosen_engine:
        chosen_engine = info_scan["engines"][0]

    if chosen_engine == "opengl-rust" and not allow_native_code:
        raise PermissionError(
            "Native Rust + OpenGL eklentisi çalıştırılabilir kod içerir; içe aktarmak için güvenlik onayı gereklidir."
        )

    target_root = custom_dir or custom_themes_dir()
    target_root.mkdir(parents=True, exist_ok=True)
    current = load_custom_themes(target_root)
    taken = set(reserved_ids or set()) | {str(t.get("id")) for t in current if t.get("id")}

    theme_id = _unique_id(_slugify(src.stem), taken, target_root)
    theme_dir = target_root / theme_id
    theme_dir.mkdir(parents=True, exist_ok=True)

    try:
        if ext in ARCHIVE_EXTS:
            try:
                zf_ctx = zipfile.ZipFile(src, "r")
            except zipfile.BadZipFile as exc:
                raise ValueError("Archive is corrupted or not a valid ZIP file.") from exc
            with zf_ctx as zf:
                entries = _safe_zip_members(zf, allow_native=(chosen_engine == "opengl-rust"))
                remaining = MAX_ARCHIVE_BYTES
                for info, parts in entries:
                    dest = (theme_dir / Path(*parts)).resolve()
                    dest.relative_to(theme_dir.resolve())
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    with zf.open(info, "r") as zin, dest.open("wb") as zout:
                        remaining -= _copy_limited(zin, zout, remaining)
            manifest: dict[str, Any] = {}
            for mname in ("manifest.json", "theme.json"):
                mp = theme_dir / mname
                if mp.is_file():
                    try:
                        parsed = json.loads(mp.read_text(encoding="utf-8"))
                    except (OSError, ValueError) as exc:
                        raise ValueError(f"Invalid {mname} in theme archive.") from exc
                    if not isinstance(parsed, dict):
                        raise ValueError(f"Invalid {mname} structure (expected JSON object).")
                    manifest = parsed
                    break
            if not manifest:
                lively_info = theme_dir / "LivelyInfo.json"
                if lively_info.is_file():
                    try:
                        li = json.loads(lively_info.read_text(encoding="utf-8"))
                        if isinstance(li, dict):
                            manifest = {
                                "name": li.get("Title"),
                                "desc": li.get("Desc"),
                                "entry": li.get("FileName"),
                                "thumb": li.get("Thumbnail") or li.get("Preview"),
                            }
                    except (OSError, ValueError):
                        pass
            lively_props_raw: Any = None
            lively_props = theme_dir / "LivelyProperties.json"
            if lively_props.is_file():
                try:
                    lively_props_raw = json.loads(lively_props.read_text(encoding="utf-8"))
                except (OSError, ValueError):
                    lively_props_raw = None
            thumb_spec = manifest.get("thumb") or manifest.get("thumbnail") or manifest.get("preview")
            groups = (
                _sanitize_groups(manifest.get("groups"))
                or _groups_from_settings(manifest.get("settings"))
                or _groups_from_settings(lively_props_raw)
            )

            if chosen_engine == "opengl-glsl":
                preferred_shader = manifest.get("shader") or manifest.get("fragment") or manifest.get("entry")
                if isinstance(preferred_shader, str) and preferred_shader.lower().endswith((".html", ".htm")):
                    preferred_shader = None
                entry = _find_entry_glsl(theme_dir, preferred_shader)
                vert_spec = manifest.get("vert") or manifest.get("vertex")
                vert_rel = _safe_rel_file(theme_dir, vert_spec, {".vert", ".glsl"}) if vert_spec else None
                textures_rel: list[str] = []
                raw_textures = manifest.get("textures")
                if isinstance(raw_textures, list):
                    for tpath in raw_textures[:4]:
                        if isinstance(tpath, str) and tpath.strip():
                            textures_rel.append(_safe_rel_file(theme_dir, tpath, IMAGE_EXTS))
                title = str(manifest.get("name") or manifest.get("title") or _pretty_title(src.stem))[:60]
                if not (theme_dir / "index.html").is_file():
                    (theme_dir / "index.html").write_text(_native_gl_card_html(title, "opengl-glsl"), encoding="utf-8")
                theme_obj = {
                    "id": theme_id,
                    "name": title,
                    "desc": str(manifest.get("desc") or manifest.get("description") or "Imported OpenGL 3.3 Core GLSL shader theme."),
                    "engine": "opengl-glsl",
                    "themeDir": str(theme_dir.resolve()),
                    "entry": entry,
                    "page": f"__custom/{theme_id}/index.html",
                    "status": "ready",
                    "product": "free",
                    "custom": True,
                    "imported": True,
                    "adaptive": bool(manifest.get("adaptive", True)),
                    "thumb": _find_archive_thumb(theme_dir, theme_id, thumb_spec),
                }
                if vert_rel:
                    theme_obj["vert"] = vert_rel
                if textures_rel:
                    theme_obj["textures"] = textures_rel
                if groups:
                    theme_obj["groups"] = groups

            elif chosen_engine == "opengl-rust":
                preferred_bin = manifest.get("plugin") or manifest.get("binary")
                entry = _find_prebuilt_plugin(theme_dir, preferred_bin)
                if entry is None:
                    entry = _compile_rust_plugin(theme_dir)
                title = str(manifest.get("name") or manifest.get("title") or _pretty_title(src.stem))[:60]
                if not (theme_dir / "index.html").is_file():
                    (theme_dir / "index.html").write_text(_native_gl_card_html(title, "opengl-rust"), encoding="utf-8")
                theme_obj = {
                    "id": theme_id,
                    "name": title,
                    "desc": str(manifest.get("desc") or manifest.get("description") or "Imported native Rust + OpenGL 3.3 plugin theme."),
                    "engine": "opengl-rust",
                    "themeDir": str(theme_dir.resolve()),
                    "entry": entry,
                    "page": f"__custom/{theme_id}/index.html",
                    "status": "ready",
                    "product": "free",
                    "custom": True,
                    "imported": True,
                    "adaptive": bool(manifest.get("adaptive", True)),
                    "thumb": _find_archive_thumb(theme_dir, theme_id, thumb_spec),
                }
                if groups:
                    theme_obj["groups"] = groups

            else:
                preferred_entry = (
                    manifest.get("entry")
                    or manifest.get("page")
                    or manifest.get("main")
                    or manifest.get("file")
                )
                if isinstance(preferred_entry, str) and not preferred_entry.lower().endswith((".html", ".htm")):
                    preferred_entry = None
                entry = _find_entry_html(theme_dir, preferred_entry)
                entry_html = _validate_html_file(theme_dir / entry)
                html_title = _extract_html_title(entry_html, _pretty_title(src.stem))
                theme_obj = {
                    "id": theme_id,
                    "name": str(manifest.get("name") or manifest.get("title") or html_title)[:60],
                    "desc": str(manifest.get("desc") or manifest.get("description") or "Imported custom theme package."),
                    "engine": "web",
                    "page": f"__custom/{theme_id}/{entry}",
                    "entry": entry,
                    "status": "ready",
                    "product": "free",
                    "custom": True,
                    "imported": True,
                    "adaptive": bool(manifest.get("adaptive", False)),
                    "thumb": _find_archive_thumb(theme_dir, theme_id, thumb_spec),
                }
                if groups:
                    theme_obj["groups"] = groups

        elif ext in SHADER_EXTS:
            raw_glsl = src.read_bytes()
            if not raw_glsl or b"\x00" in raw_glsl or len(raw_glsl) > 2 * 1024 * 1024:
                raise ValueError(f"Shader file is empty, too large, or corrupted: {src.name}")
            glsl_src = raw_glsl.decode("utf-8", errors="replace")
            title = _pretty_title(src.stem)
            (theme_dir / "main.frag").write_text(glsl_src, encoding="utf-8")
            if chosen_engine == "opengl-glsl":
                (theme_dir / "index.html").write_text(_native_gl_card_html(title, "opengl-glsl"), encoding="utf-8")
                theme_obj = {
                    "id": theme_id,
                    "name": title[:60],
                    "desc": "Imported OpenGL 3.3 Core GLSL fragment shader.",
                    "engine": "opengl-glsl",
                    "themeDir": str(theme_dir.resolve()),
                    "entry": "main.frag",
                    "page": f"__custom/{theme_id}/index.html",
                    "status": "ready",
                    "product": "free",
                    "custom": True,
                    "imported": True,
                    "adaptive": True,
                    "thumb": (
                        "radial-gradient(circle at 28% 35%, rgba(168,95,255,0.52) 0%, transparent 50%), "
                        "radial-gradient(circle at 75% 68%, rgba(72,210,255,0.42) 0%, transparent 48%), "
                        "linear-gradient(150deg, #131126 0%, #1d2642 55%, #111829 100%)"
                    ),
                    "groups": [
                        {
                            "name": "Shader",
                            "params": [
                                {"key": "speed", "label": "Speed", "min": 0.1, "max": 3.0, "step": 0.05, "default": 1.0, "format": "times"},
                                {"key": "brightness", "label": "Brightness", "min": 0.4, "max": 1.4, "step": 0.01, "default": 1.0, "format": "percent"},
                            ],
                        }
                    ],
                }
            else:
                (theme_dir / "shader.glsl").write_text(glsl_src, encoding="utf-8")
                (theme_dir / "index.html").write_text(_shader_wrapper_html(title, glsl_src), encoding="utf-8")
                theme_obj = {
                    "id": theme_id,
                    "name": title[:60],
                    "desc": "Imported GLSL WebGL fragment shader.",
                    "engine": "web",
                    "page": f"__custom/{theme_id}/index.html",
                    "entry": "index.html",
                    "status": "ready",
                    "product": "free",
                    "custom": True,
                    "imported": True,
                    "adaptive": True,
                    "thumb": (
                        "radial-gradient(circle at 28% 35%, rgba(168,95,255,0.52) 0%, transparent 50%), "
                        "radial-gradient(circle at 75% 68%, rgba(72,210,255,0.42) 0%, transparent 48%), "
                        "linear-gradient(150deg, #131126 0%, #1d2642 55%, #111829 100%)"
                    ),
                    "groups": [
                        {
                            "name": "Shader",
                            "params": [
                                {"key": "speed", "label": "Speed", "min": 0.1, "max": 3.0, "step": 0.05, "default": 1.0, "format": "times"},
                                {"key": "brightness", "label": "Brightness", "min": 0.4, "max": 1.4, "step": 0.01, "default": 1.0, "format": "percent"},
                            ],
                        }
                    ],
                }

        elif ext in HTML_EXTS:
            raw_html = _validate_html_file(src)
            (theme_dir / "index.html").write_text(raw_html, encoding="utf-8")
            title = _extract_html_title(raw_html, _pretty_title(src.stem))
            theme_obj = {
                "id": theme_id,
                "name": title,
                "desc": "Imported interactive HTML wallpaper.",
                "engine": "web",
                "page": f"__custom/{theme_id}/index.html",
                "entry": "index.html",
                "status": "ready",
                "product": "free",
                "custom": True,
                "imported": True,
                "adaptive": False,
                "thumb": (
                    "radial-gradient(circle at 30% 35%, rgba(98,195,255,0.45) 0%, transparent 48%), "
                    "radial-gradient(circle at 75% 70%, rgba(120,235,190,0.35) 0%, transparent 45%), "
                    "linear-gradient(150deg, #162231 0%, #243e54 55%, #172738 100%)"
                ),
            }

        else:
            is_video = ext in VIDEO_EXTS
            media_name = f"media{ext}"
            with src.open("rb") as min_f, (theme_dir / media_name).open("wb") as mout_f:
                _copy_limited(min_f, mout_f, MAX_ARCHIVE_BYTES)
            title = _pretty_title(src.stem)
            (theme_dir / "index.html").write_text(
                _media_wrapper_html(media_name, is_video, title),
                encoding="utf-8",
            )
            thumb = (
                "radial-gradient(circle at 32% 38%, rgba(255,170,95,0.45) 0%, transparent 48%), "
                "linear-gradient(150deg, #1e2130 0%, #342f48 55%, #1b1d29 100%)"
                if is_video
                else f"url(__custom/{theme_id}/{media_name}) center / cover no-repeat"
            )
            theme_obj = {
                "id": theme_id,
                "name": title,
                "desc": "Imported animated video wallpaper." if is_video else "Imported custom image wallpaper.",
                "engine": "web",
                "page": f"__custom/{theme_id}/index.html",
                "entry": "index.html",
                "status": "ready",
                "product": "free",
                "custom": True,
                "imported": True,
                "adaptive": True,
                "thumb": thumb,
                "groups": _media_params(is_video),
            }

        current.append(theme_obj)
        save_custom_themes(target_root, current)
        return theme_obj
    except Exception:
        shutil.rmtree(theme_dir, ignore_errors=True)
        raise


def remove_custom_theme(theme_id: str, custom_dir: Path | None = None) -> bool:
    """Remove an imported custom theme from registry and disk."""
    target_root = custom_dir or custom_themes_dir()
    current = load_custom_themes(target_root)
    kept = [t for t in current if t.get("id") != theme_id]
    if len(kept) == len(current):
        return False
    save_custom_themes(target_root, kept)
    theme_dir = (target_root / Path(theme_id).name).resolve()
    try:
        theme_dir.relative_to(target_root.resolve())
        if theme_dir.is_dir():
            shutil.rmtree(theme_dir, ignore_errors=True)
    except ValueError:
        pass
    return True
