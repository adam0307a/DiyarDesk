"""Loopback HTTP server for the prototype pages and generated share files."""

from __future__ import annotations

import mimetypes
import threading
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Callable
from urllib.parse import unquote, urlparse

mimetypes.add_type("image/webp", ".webp")
mimetypes.add_type("text/javascript", ".mjs")
mimetypes.add_type("audio/mp4", ".m4a")
mimetypes.add_type("audio/ogg", ".opus")
mimetypes.add_type("video/mp4", ".mp4")
mimetypes.add_type("video/webm", ".webm")


class _Handler(SimpleHTTPRequestHandler):
    share_dir: Path
    audio_dir: Path | None
    custom_themes_dir: Path | None
    themes_provider: Callable[[], bytes] | None
    prototype_dir: Path

    def __init__(
        self,
        *args,
        directory: str,
        share_dir: Path,
        audio_dir: Path | None = None,
        custom_themes_dir: Path | None = None,
        themes_provider: Callable[[], bytes] | None = None,
        **kwargs,
    ) -> None:
        self.share_dir = share_dir
        self.audio_dir = audio_dir
        self.custom_themes_dir = custom_themes_dir
        self.themes_provider = themes_provider
        self.prototype_dir = Path(directory)
        super().__init__(*args, directory=directory, **kwargs)

    def _send_bytes(self, data: bytes, ctype: str) -> None:
        total = len(data)
        rng = self.headers.get("Range")
        if rng and rng.startswith("bytes="):
            spec = rng[6:].split(",", 1)[0].strip()
            parts = spec.split("-", 1)
            if len(parts) == 2:
                try:
                    start = int(parts[0]) if parts[0] else max(0, total - int(parts[1]))
                    end = int(parts[1]) if parts[0] and parts[1] else total - 1
                    end = min(end, total - 1)
                    if 0 <= start <= end < total:
                        chunk = data[start : end + 1]
                        self.send_response(206)
                        self.send_header("Content-Type", ctype)
                        self.send_header("Accept-Ranges", "bytes")
                        self.send_header("Content-Range", f"bytes {start}-{end}/{total}")
                        self.send_header("Content-Length", str(len(chunk)))
                        self.send_header("Cache-Control", "no-store")
                        self.end_headers()
                        self.wfile.write(chunk)
                        return
                except ValueError:
                    pass
        self.send_response(200)
        self.send_header("Content-Type", ctype)
        self.send_header("Accept-Ranges", "bytes")
        self.send_header("Content-Length", str(total))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def _serve_file(self, base: Path | None, raw_name: str, nested: bool = False) -> None:
        if base is None:
            self.send_error(404)
            return
        if nested:
            rel = raw_name.lstrip("/").replace("\\", "/")
            target = (base / rel).resolve()
        else:
            name = Path(raw_name).name
            target = (base / name).resolve()
        try:
            target.relative_to(base.resolve())
        except ValueError:
            self.send_error(403)
            return
        if not target.is_file():
            if nested and target.name == "share.js" and (self.prototype_dir / "share.js").is_file():
                target = self.prototype_dir / "share.js"
            else:
                self.send_error(404)
                return
        ctype = mimetypes.guess_type(target.name)[0] or "application/octet-stream"
        self._send_bytes(target.read_bytes(), ctype)

    def do_GET(self) -> None:  # noqa: N802 - stdlib name
        path = unquote(urlparse(self.path).path)
        if path == "/themes.json" and self.themes_provider is not None:
            self._send_bytes(self.themes_provider(), "application/json; charset=utf-8")
            return
        if path.startswith("/__share/"):
            self._serve_file(self.share_dir, path[len("/__share/") :])
            return
        if path.startswith("/__audio/"):
            self._serve_file(self.audio_dir, path[len("/__audio/") :])
            return
        if path.startswith("/__custom/"):
            self._serve_file(self.custom_themes_dir, path[len("/__custom/") :], nested=True)
            return
        super().do_GET()

    def log_message(self, fmt: str, *args) -> None:
        return


class LocalServer:
    def __init__(
        self,
        prototype: Path,
        share_dir: Path,
        audio_dir: Path | None = None,
        custom_themes_dir: Path | None = None,
        themes_provider: Callable[[], bytes] | None = None,
    ) -> None:
        self.prototype = prototype
        self.share_dir = share_dir
        self.audio_dir = audio_dir
        self.custom_themes_dir = custom_themes_dir
        self.themes_provider = themes_provider
        self.httpd: ThreadingHTTPServer | None = None
        self.thread: threading.Thread | None = None
        self.port = 0

    def start(self) -> int:
        if self.httpd is not None:
            return self.port
        handler = lambda *args, **kwargs: _Handler(  # noqa: E731
            *args,
            directory=str(self.prototype),
            share_dir=self.share_dir,
            audio_dir=self.audio_dir,
            custom_themes_dir=self.custom_themes_dir,
            themes_provider=self.themes_provider,
            **kwargs,
        )
        self.httpd = ThreadingHTTPServer(("127.0.0.1", 0), handler)
        self.httpd.daemon_threads = True
        self.port = self.httpd.server_address[1]
        self.thread = threading.Thread(
            target=self.httpd.serve_forever,
            name="deskland-http",
            daemon=True,
        )
        self.thread.start()
        return self.port

    def stop(self) -> None:
        if self.httpd is None:
            return
        try:
            self.httpd.shutdown()
            self.httpd.server_close()
        finally:
            self.httpd = None
            self.thread = None

    def url(self, path: str) -> str:
        return f"http://127.0.0.1:{self.port}/{path.lstrip('/')}"
