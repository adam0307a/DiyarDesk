"""Local Unix-domain socket IPC server & client for `diyardesk-ctl`.

100% local-only: binds to `$XDG_RUNTIME_DIR/diyardesk.sock` (or `~/.config/deskland/diyardesk.sock`)
with owner-only permissions (`0o600`). No TCP ports, no network exposure.
"""
from __future__ import annotations

import json
import os
import socket
import threading
from pathlib import Path
from typing import Any, Callable

from .config import config_dir


def default_socket_path() -> Path:
    override = os.environ.get("DIYARDESK_IPC_SOCKET")
    if override:
        return Path(override)
    runtime_dir = os.environ.get("XDG_RUNTIME_DIR")
    if runtime_dir and Path(runtime_dir).is_dir():
        return Path(runtime_dir) / "diyardesk.sock"
    return config_dir() / "diyardesk.sock"


class IpcServer:
    """Background Unix-domain socket server accepting newline-delimited JSON commands."""

    def __init__(
        self,
        handler: Callable[[dict[str, Any]], dict[str, Any]],
        socket_path: Path | None = None,
    ) -> None:
        self._handler = handler
        self.socket_path = Path(socket_path) if socket_path is not None else default_socket_path()
        self._sock: socket.socket | None = None
        self._thread: threading.Thread | None = None
        self._stop = threading.Event()

    def start(self) -> bool:
        if not hasattr(socket, "AF_UNIX"):
            return False
        self.socket_path.parent.mkdir(parents=True, exist_ok=True)
        try:
            if self.socket_path.exists():
                self.socket_path.unlink()
        except OSError:
            pass
        try:
            sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            sock.bind(str(self.socket_path))
            try:
                os.chmod(self.socket_path, 0o600)
            except OSError:
                pass
            sock.listen(8)
            sock.settimeout(0.5)
        except OSError:
            return False

        self._sock = sock
        self._stop.clear()
        self._thread = threading.Thread(target=self._loop, name="diyardesk-ipc", daemon=True)
        self._thread.start()
        return True

    def stop(self) -> None:
        self._stop.set()
        if self._sock is not None:
            try:
                self._sock.close()
            except OSError:
                pass
            self._sock = None
        if self._thread is not None and self._thread.is_alive():
            self._thread.join(timeout=1.5)
        self._thread = None
        try:
            if self.socket_path.exists():
                self.socket_path.unlink()
        except OSError:
            pass

    def _loop(self) -> None:
        sock = self._sock
        if sock is None:
            return
        while not self._stop.is_set():
            try:
                conn, _ = sock.accept()
            except socket.timeout:
                continue
            except OSError:
                break
            try:
                self._serve_client(conn)
            finally:
                try:
                    conn.close()
                except OSError:
                    pass

    def _serve_client(self, conn: socket.socket) -> None:
        conn.settimeout(3.0)
        buf = b""
        while len(buf) < 65536:
            try:
                chunk = conn.recv(4096)
            except OSError:
                break
            if not chunk:
                break
            buf += chunk
            if b"\n" in buf:
                break
        line = buf.split(b"\n", 1)[0].strip()
        if not line:
            resp: dict[str, Any] = {"ok": False, "error": "empty request"}
        else:
            try:
                req = json.loads(line.decode("utf-8"))
                if not isinstance(req, dict):
                    resp = {"ok": False, "error": "request must be a JSON object"}
                else:
                    resp = self._handler(req)
            except Exception as exc:
                resp = {"ok": False, "error": str(exc)}
        try:
            conn.sendall((json.dumps(resp, ensure_ascii=False) + "\n").encode("utf-8"))
        except OSError:
            pass


def send_ipc_command(
    payload: dict[str, Any],
    socket_path: Path | None = None,
    timeout: float = 3.0,
) -> dict[str, Any]:
    if not hasattr(socket, "AF_UNIX"):
        return {"ok": False, "error": "AF_UNIX sockets are not supported on this platform"}
    path = Path(socket_path) if socket_path is not None else default_socket_path()
    if not path.exists():
        return {"ok": False, "error": f"DiyarDesk daemon is not running (socket not found: {path})"}
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.settimeout(timeout)
    try:
        sock.connect(str(path))
        sock.sendall((json.dumps(payload, ensure_ascii=False) + "\n").encode("utf-8"))
        buf = b""
        while len(buf) < 65536:
            chunk = sock.recv(4096)
            if not chunk:
                break
            buf += chunk
            if b"\n" in buf:
                break
        line = buf.split(b"\n", 1)[0].strip()
        if not line:
            return {"ok": False, "error": "empty response from daemon"}
        return json.loads(line.decode("utf-8"))
    except OSError as exc:
        return {"ok": False, "error": f"IPC connection failed: {exc}"}
    finally:
        try:
            sock.close()
        except OSError:
            pass
