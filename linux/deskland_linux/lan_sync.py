"""Cloud-free local Wi-Fi (LAN) pairing and theme transfer server for Android companions.

Disabled by default; starts only on explicit user action with a short-lived
4-digit PIN, automatic expiration, and brute-force lockout.
"""

from __future__ import annotations

import io
import json
import secrets
import socket
import threading
import time
import zipfile
from collections.abc import Callable
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, urlparse

from .config import APP_NAME, VERSION
from .importer import MAX_ARCHIVE_BYTES, load_custom_themes

PAIR_TTL_SECONDS = 600
MAX_FAILED_ATTEMPTS = 5


def detect_lan_ip() -> str:
    """Best-effort detection of the primary private LAN IPv4 address without sending packets."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        sock.connect(("192.0.2.1", 80))
        ip = sock.getsockname()[0]
        if ip and not ip.startswith("127."):
            return ip
    except OSError:
        pass
    finally:
        sock.close()
    return "127.0.0.1"


def build_theme_deskland_bytes(custom_dir: Path, theme_id: str) -> bytes:
    """Pack an imported custom theme directory into an in-memory .deskland ZIP archive."""
    safe_name = Path(theme_id).name
    if not safe_name or safe_name != theme_id or not safe_name.startswith("custom-"):
        raise ValueError("Invalid custom theme id")
    theme_dir = (custom_dir / safe_name).resolve()
    theme_dir.relative_to(custom_dir.resolve())
    if not theme_dir.is_dir():
        raise FileNotFoundError(f"Custom theme directory not found: {theme_id}")

    buf = io.BytesIO()
    total = 0
    with zipfile.ZipFile(buf, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for file_path in sorted(theme_dir.rglob("*")):
            if not file_path.is_file() or file_path.is_symlink():
                continue
            resolved = file_path.resolve()
            resolved.relative_to(theme_dir)
            size = resolved.stat().st_size
            total += size
            if total > MAX_ARCHIVE_BYTES:
                raise ValueError("Theme exceeds maximum transfer size")
            arcname = resolved.relative_to(theme_dir).as_posix()
            zf.write(resolved, arcname)
    return buf.getvalue()


class LanPairingServer:
    """Short-lived, PIN-authenticated LAN transfer server for Android tablet pairing."""

    def __init__(
        self,
        custom_dir: Path,
        state_provider: Callable[[], dict[str, Any]],
        ttl_seconds: int = PAIR_TTL_SECONDS,
        bind_host: str = "0.0.0.0",
    ) -> None:
        self.custom_dir = custom_dir
        self.state_provider = state_provider
        self.ttl_seconds = max(30, int(ttl_seconds))
        self.bind_host = bind_host
        self._lock = threading.Lock()
        self.httpd: ThreadingHTTPServer | None = None
        self.thread: threading.Thread | None = None
        self.pin: str = ""
        self.port: int = 0
        self.host_ip: str = "127.0.0.1"
        self.expires_at: float = 0.0
        self.failed_attempts: int = 0

    @property
    def active(self) -> bool:
        with self._lock:
            return self.httpd is not None and time.monotonic() < self.expires_at and self.failed_attempts < MAX_FAILED_ATTEMPTS

    def start(self) -> dict[str, Any]:
        self.stop()
        pin = f"{secrets.randbelow(9000) + 1000:04d}"
        host_ip = detect_lan_ip()
        parent = self

        class _PairHandler(BaseHTTPRequestHandler):
            def _check_auth(self, parsed_query: dict[str, list[str]]) -> bool:
                with parent._lock:
                    if time.monotonic() >= parent.expires_at or parent.failed_attempts >= MAX_FAILED_ATTEMPTS:
                        self.send_error(410, "Pairing session expired")
                        return False
                    supplied = (
                        self.headers.get("X-Diyar-Pin")
                        or (parsed_query.get("pin", [""])[0] if parsed_query else "")
                    ).strip()
                    if not supplied or not secrets.compare_digest(supplied, parent.pin):
                        parent.failed_attempts += 1
                        self.send_error(403, "Invalid pairing PIN")
                        return False
                    return True

            def _send_bytes(self, data: bytes, ctype: str, extra_headers: dict[str, str] | None = None) -> None:
                self.send_response(200)
                self.send_header("Content-Type", ctype)
                self.send_header("Content-Length", str(len(data)))
                self.send_header("Cache-Control", "no-store")
                if extra_headers:
                    for k, v in extra_headers.items():
                        self.send_header(k, v)
                self.end_headers()
                self.wfile.write(data)

            def do_GET(self) -> None:  # noqa: N802
                parsed = urlparse(self.path)
                query = parse_qs(parsed.query)
                if not self._check_auth(query):
                    return
                if parsed.path == "/pair/info":
                    state = parent.state_provider() or {}
                    customs = load_custom_themes(parent.custom_dir)
                    payload = {
                        "app": APP_NAME,
                        "version": VERSION,
                        "activeTheme": state.get("theme", "papercut"),
                        "params": state.get("params", {}),
                        "customThemes": [
                            {
                                "id": t.get("id"),
                                "name": t.get("name"),
                                "desc": t.get("desc", ""),
                            }
                            for t in customs
                            if t.get("id")
                        ],
                    }
                    raw = json.dumps(payload, ensure_ascii=False).encode("utf-8")
                    self._send_bytes(raw, "application/json; charset=utf-8")
                    return
                if parsed.path.startswith("/pair/theme/") and parsed.path.endswith(".deskland"):
                    theme_id = parsed.path[len("/pair/theme/") : -len(".deskland")]
                    try:
                        archive = build_theme_deskland_bytes(parent.custom_dir, theme_id)
                    except (ValueError, FileNotFoundError):
                        self.send_error(404, "Theme not found")
                        return
                    self._send_bytes(
                        archive,
                        "application/zip",
                        {"Content-Disposition": f'attachment; filename="{Path(theme_id).name}.deskland"'},
                    )
                    return
                self.send_error(404)

            def log_message(self, fmt: str, *args: Any) -> None:
                return

        httpd = ThreadingHTTPServer((self.bind_host, 0), _PairHandler)
        httpd.daemon_threads = True
        port = httpd.server_address[1]
        with self._lock:
            self.httpd = httpd
            self.pin = pin
            self.port = port
            self.host_ip = host_ip
            self.expires_at = time.monotonic() + self.ttl_seconds
            self.failed_attempts = 0
        self.thread = threading.Thread(target=httpd.serve_forever, name="diyardesk-lan-pair", daemon=True)
        self.thread.start()
        return self.info()

    def info(self) -> dict[str, Any]:
        with self._lock:
            remaining = max(0, int(self.expires_at - time.monotonic())) if self.httpd else 0
            return {
                "active": self.httpd is not None and remaining > 0 and self.failed_attempts < MAX_FAILED_ATTEMPTS,
                "host": self.host_ip,
                "port": self.port,
                "pin": self.pin,
                "uri": f"diyardesk://pair?host={self.host_ip}&port={self.port}&pin={self.pin}" if self.httpd else "",
                "expiresIn": remaining,
            }

    def stop(self) -> None:
        with self._lock:
            httpd = self.httpd
            self.httpd = None
            self.pin = ""
            self.port = 0
            self.expires_at = 0.0
            self.failed_attempts = 0
        if httpd is not None:
            try:
                httpd.shutdown()
                httpd.server_close()
            except OSError:
                pass
        self.thread = None
