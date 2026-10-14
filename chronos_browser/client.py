"""
Chronos Client - Fast UNIX socket client that talks to the resident daemon.
Auto-spawns daemon on demand.
Latency: 2 - 15ms.
"""

import os
import sys
import time
import json
import socket
import subprocess
from pathlib import Path
from typing import Dict, Any, Optional

from chronos_browser.driver import SOCKET_PATH

class ChronosClient:
    def __init__(self, socket_path: Path = SOCKET_PATH):
        self.socket_path = socket_path

    def is_daemon_alive(self) -> bool:
        if not self.socket_path.exists():
            return False
        try:
            s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            s.settimeout(0.5)
            s.connect(str(self.socket_path))
            s.sendall(json.dumps({"action": "ping"}).encode() + b"\n")
            data = s.recv(1024)
            s.close()
            return b"pong" in data
        except Exception:
            return False

    def ensure_daemon(self, timeout: float = 6.0) -> bool:
        if self.is_daemon_alive():
            return True

        if self.socket_path.exists():
            try:
                self.socket_path.unlink()
            except Exception:
                pass

        # Spawn resident daemon in background
        env = os.environ.copy()
        python_bin = sys.executable
        cmd = [python_bin, "-m", "chronos_browser.daemon"]

        subprocess.Popen(
            cmd,
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True
        )

        start = time.time()
        while time.time() - start < timeout:
            if self.is_daemon_alive():
                return True
            time.sleep(0.1)

        return False

    def send(self, payload: Dict[str, Any], timeout: float = 60.0) -> Dict[str, Any]:
        if not self.ensure_daemon():
            raise RuntimeError("Could not connect to or spawn Chronos resident daemon.")

        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.settimeout(timeout)
        s.connect(str(self.socket_path))
        try:
            req = json.dumps(payload).encode() + b"\n"
            s.sendall(req)
            buffer = b""
            while b"\n" not in buffer:
                chunk = s.recv(65536)
                if not chunk:
                    break
                buffer += chunk
            return json.loads(buffer.decode().strip())
        finally:
            s.close()

# Global singleton client
client = ChronosClient()
