"""
Chronos Browser Core Driver & Process Management.
Prioritizes Google Chrome Stable for standard rendering, hardware GPU acceleration,
and native codecs.
"""

import os
import sys
import time
import json
import shutil
import asyncio
import subprocess
import urllib.request
import urllib.parse
from pathlib import Path
from typing import Optional, List, Dict, Any

CDP_PORT = int(os.environ.get("CHRONOS_CDP_PORT", os.environ.get("ORDIS_CDP_PORT", "9222")))
CDP_URL = f"http://127.0.0.1:{CDP_PORT}"
DEFAULT_DISPLAY = os.environ.get("DISPLAY", ":1")
SOCKET_PATH = Path(os.environ.get("CHRONOS_SOCKET", "/tmp/chronos-browser.sock"))

# Prefer persistent existing profiles
def get_default_profile() -> str:
    if "CHRONOS_CHROME_PROFILE" in os.environ:
        return os.environ["CHRONOS_CHROME_PROFILE"]
    ordis_prof = Path.home() / ".config" / "ordis-chrome-profile"
    chronos_prof = Path.home() / ".config" / "chronos-chrome-profile"
    if ordis_prof.exists() and not chronos_prof.exists():
        return str(ordis_prof)
    return str(chronos_prof)

DEFAULT_PROFILE = get_default_profile()
SCRATCH_DIR = Path.home() / ".chronos-browser" / "scratch"

# Browser Candidates: Google Chrome Stable first
BROWSER_CANDIDATES = [
    os.environ.get("CHRONOS_CHROME_BIN"),
    os.environ.get("ORDIS_CHROME_BIN"),
    "/usr/bin/google-chrome-stable",
    "/usr/bin/google-chrome",
    shutil.which("google-chrome-stable"),
    shutil.which("google-chrome"),
    shutil.which("chromium"),
    shutil.which("chromium-browser"),
    shutil.which("brave-browser"),
]

def find_browser_binary() -> str:
    for candidate in BROWSER_CANDIDATES:
        if candidate and os.path.exists(candidate) and os.access(candidate, os.X_OK):
            return candidate
    # Fallback to playwright cache if system chrome is absent
    pw_cache = Path.home() / ".cache" / "ms-playwright"
    if pw_cache.exists():
        for p in pw_cache.glob("chromium-*/chrome-linux64/chrome"):
            return str(p)
    raise RuntimeError("No Google Chrome or Chromium binary found.")

def is_cdp_ready(port: int = CDP_PORT) -> bool:
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/json/version", timeout=0.8) as resp:
            return resp.status == 200
    except Exception:
        return False

def get_cdp_pages(port: int = CDP_PORT) -> List[Dict[str, Any]]:
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list", timeout=1.0) as resp:
            targets = json.loads(resp.read().decode())
            return [t for t in targets if t.get("type") == "page"]
    except Exception:
        return []

def prune_scratch_dir(max_files: int = 30):
    if not SCRATCH_DIR.exists():
        return
    try:
        files = sorted([f for f in SCRATCH_DIR.iterdir() if f.is_file()], key=lambda x: x.stat().st_mtime, reverse=True)
        for f in files[max_files:]:
            f.unlink(missing_ok=True)
    except Exception:
        pass

def ensure_browser_process(
    display: Optional[str] = None,
    profile_dir: Optional[str] = None,
    port: int = CDP_PORT
) -> bool:
    if is_cdp_ready(port):
        return True

    bin_path = find_browser_binary()
    disp = display or DEFAULT_DISPLAY
    prof = profile_dir or DEFAULT_PROFILE

    os.makedirs(prof, exist_ok=True)
    os.makedirs(SCRATCH_DIR, exist_ok=True)

    cmd = [
        bin_path,
        f"--user-data-dir={prof}",
        f"--remote-debugging-port={port}",
        "--ozone-platform=x11",
        "--no-first-run",
        "--no-default-browser-check",
        # Performance & GPU hardware acceleration
        "--enable-gpu-rasterization",
        "--enable-zero-copy",
        "--ignore-gpu-blocklist",
        "--enable-features=VaapiVideoDecoder,CanvasOopRasterization",
        "--autoplay-policy=no-user-gesture-required",
        "--disable-background-timer-throttling",
        "--disable-backgrounding-occluded-windows",
        "--disable-renderer-backgrounding",
    ]

    env = os.environ.copy()
    env["DISPLAY"] = disp

    subprocess.Popen(
        cmd,
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True
    )

    for _ in range(30):
        if is_cdp_ready(port):
            return True
        time.sleep(0.15)

    return False
