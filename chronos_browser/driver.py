"""
Chronos Browser Core Driver & Process Lifecycle Management.
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
from typing import Optional, List, Dict, Any, Union

CDP_PORT = int(os.environ.get("CHRONOS_CDP_PORT", os.environ.get("ORDIS_CDP_PORT", "9222")))
CDP_URL = f"http://127.0.0.1:{CDP_PORT}"
DEFAULT_DISPLAY = os.environ.get("DISPLAY", ":1")
DEFAULT_PROFILE = os.environ.get(
    "CHRONOS_CHROME_PROFILE",
    str(Path.home() / ".config" / "chronos-chrome-profile")
)
SCRATCH_DIR = Path.home() / ".chronos-browser" / "scratch"

def get_browser_candidates() -> List[str]:
    candidates = [
        os.environ.get("CHRONOS_CHROME_BIN"),
        os.environ.get("ORDIS_CHROME_BIN"),
    ]
    # Check common system paths
    system_bins = [
        "google-chrome-stable",
        "google-chrome",
        "chromium",
        "chromium-browser",
        "brave-browser",
    ]
    for b in system_bins:
        which = shutil.which(b)
        if which:
            candidates.append(which)

    # Check playwright cache directory
    pw_cache = Path.home() / ".cache" / "ms-playwright"
    if pw_cache.exists():
        for p in pw_cache.glob("chromium-*/chrome-linux64/chrome"):
            candidates.append(str(p))

    # Standard fallback locations
    candidates.extend([
        "/usr/bin/google-chrome-stable",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/usr/bin/brave-browser",
    ])
    return [c for c in candidates if c]

def find_browser_binary() -> str:
    for candidate in get_browser_candidates():
        if candidate and os.path.exists(candidate) and os.access(candidate, os.X_OK):
            return candidate
    raise RuntimeError("No compatible Chromium or Chrome binary found on system.")

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

def prune_scratch_dir(max_files: int = 30, max_age_days: int = 3):
    if not SCRATCH_DIR.exists():
        return
    try:
        now = time.time()
        files = sorted(
            [f for f in SCRATCH_DIR.iterdir() if f.is_file()],
            key=lambda x: x.stat().st_mtime,
            reverse=True
        )
        remaining = []
        for f in files:
            if (now - f.stat().st_mtime) / 86400 > max_age_days:
                f.unlink(missing_ok=True)
            else:
                remaining.append(f)
        for f in remaining[max_files:]:
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

class AsyncBrowserManager:
    """Manages persistent async Playwright connection to running Chromium."""
    def __init__(self):
        self.playwright = None
        self.browser = None
        self.context = None
        self._lock = asyncio.Lock()

    async def get_context(self):
        from playwright.async_api import async_playwright

        async with self._lock:
            if self.browser and self.browser.is_connected() and self.context:
                return self.context

            if not ensure_browser_process():
                raise RuntimeError(f"Could not connect to or start Chromium on {CDP_URL}")

            if self.playwright is None:
                self.playwright = await async_playwright().start()

            try:
                self.browser = await self.playwright.chromium.connect_over_cdp(CDP_URL)
                self.context = self.browser.contexts[0] if self.browser.contexts else await self.browser.new_context()
                return self.context
            except Exception:
                if self.playwright:
                    try:
                        await self.playwright.stop()
                    except Exception:
                        pass
                self.playwright = await async_playwright().start()
                self.browser = await self.playwright.chromium.connect_over_cdp(CDP_URL)
                self.context = self.browser.contexts[0] if self.browser.contexts else await self.browser.new_context()
                return self.context

    async def get_page(self, tab_index: Optional[int] = None):
        context = await self.get_context()
        pages = context.pages
        if not pages:
            return await context.new_page()

        if tab_index is not None:
            if 0 <= tab_index < len(pages):
                page = pages[tab_index]
                await page.bring_to_front()
                return page
            raise IndexError(f"Tab index {tab_index} out of range (0-{len(pages)-1})")

        page = pages[0]
        await page.bring_to_front()
        return page

    async def resolve_locator(self, page, target: str):
        if any(target.startswith(p) for p in ["text=", "css=", "xpath=", "role=", "id="]):
            return page.locator(target).first
        if target.startswith("//") or target.startswith("(//"):
            return page.locator(f"xpath={target}").first

        try:
            loc = page.locator(target)
            if await loc.count() > 0:
                return loc.first
        except Exception:
            pass

        for attr in ["aria-label", "title", "placeholder", "name"]:
            try:
                loc = page.locator(f"[{attr}*='{target}']")
                if await loc.count() > 0:
                    return loc.first
            except Exception:
                pass

        return page.get_by_text(target, exact=False).first

    async def stop(self):
        if is_cdp_ready():
            try:
                if self.browser:
                    await self.browser.close()
                if self.playwright:
                    await self.playwright.stop()
            except Exception:
                pass
            subprocess.run(["pkill", "-9", "-f", f"remote-debugging-port={CDP_PORT}"], check=False)
