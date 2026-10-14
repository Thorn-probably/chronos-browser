"""
Chronos Resident Daemon - Keeps Playwright and Google Chrome connection in memory
over a fast local UNIX domain socket (/tmp/chronos-browser.sock).
Execution latency per action: < 15ms.
"""

import os
import sys
import time
import json
import signal
import asyncio
import traceback
from pathlib import Path
from typing import Optional, Dict, Any, List

from playwright.async_api import async_playwright, Browser, BrowserContext, Page, Locator

from chronos_browser.driver import (
    CDP_URL,
    CDP_PORT,
    DEFAULT_DISPLAY,
    DEFAULT_PROFILE,
    SOCKET_PATH,
    SCRATCH_DIR,
    find_browser_binary,
    ensure_browser_process,
    is_cdp_ready,
    prune_scratch_dir
)

class ChronosDaemon:
    def __init__(self):
        self.playwright = None
        self.browser: Optional[Browser] = None
        self.context: Optional[BrowserContext] = None
        self.running = False
        self._server = None

    async def init_browser(self):
        if not ensure_browser_process():
            raise RuntimeError(f"Could not start Google Chrome binary at {find_browser_binary()}")

        if self.playwright is None:
            self.playwright = await async_playwright().start()

        self.browser = await self.playwright.chromium.connect_over_cdp(CDP_URL)
        if not self.browser.contexts:
            self.context = await self.browser.new_context()
        else:
            self.context = self.browser.contexts[0]

    async def get_page(self, tab_index: Optional[int] = None) -> Page:
        if not self.browser or not self.browser.is_connected() or not self.context:
            await self.init_browser()

        pages = self.context.pages
        if not pages:
            return await self.context.new_page()

        if tab_index is not None:
            if 0 <= tab_index < len(pages):
                p = pages[tab_index]
                await p.bring_to_front()
                return p
            raise IndexError(f"Tab index {tab_index} out of range (0-{len(pages)-1})")

        p = pages[0]
        await p.bring_to_front()
        return p

    async def resolve_locator(self, page: Page, target: str) -> Locator:
        if any(target.startswith(p) for p in ["text=", "css=", "xpath=", "role=", "id="]):
            return page.locator(target).first
        if target.startswith("//") or target.startswith("(//"):
            return page.locator(f"xpath={target}").first

        # Direct CSS locator
        try:
            loc = page.locator(target)
            if await loc.count() > 0:
                return loc.first
        except Exception:
            pass

        # Attributes (aria-label, placeholder, title, name)
        for attr in ["aria-label", "placeholder", "title", "name"]:
            try:
                loc = page.locator(f"[{attr}*='{target}']")
                if await loc.count() > 0:
                    return loc.first
            except Exception:
                pass

        # Fallback to visible text
        return page.get_by_text(target, exact=False).first

    async def handle_request(self, payload: Dict[str, Any]) -> Dict[str, Any]:
        action = payload.get("action", "").lower()
        tab_idx = payload.get("tab_index")

        try:
            if action == "ping":
                return {"status": "pong", "time": time.time()}

            elif action in ["status", "info"]:
                ready = is_cdp_ready()
                if not ready:
                    return {"running": False, "cdp_port": CDP_PORT}
                tabs = []
                if self.context:
                    for i, p in enumerate(self.context.pages):
                        try:
                            tabs.append({"index": i, "title": await p.title(), "url": p.url})
                        except Exception:
                            tabs.append({"index": i, "title": "[unavailable]", "url": ""})
                return {
                    "running": True,
                    "browser_bin": find_browser_binary(),
                    "display": DEFAULT_DISPLAY,
                    "cdp_port": CDP_PORT,
                    "tab_count": len(tabs),
                    "tabs": tabs
                }

            elif action in ["launch", "start"]:
                display = payload.get("display") or DEFAULT_DISPLAY
                ensure_browser_process(display=display)
                await self.init_browser()
                url = payload.get("url")
                if url:
                    page = await self.get_page(tab_idx)
                    u = url if (url.startswith("http://") or url.startswith("https://")) else f"https://{url}"
                    await page.goto(u, timeout=45000, wait_until="domcontentloaded")
                    return {"status": "launched", "title": await page.title(), "url": page.url}
                return {"status": "launched", "browser_bin": find_browser_binary()}

            elif action in ["goto", "navigate", "open"]:
                page = await self.get_page(tab_idx)
                u = payload["url"]
                if not (u.startswith("http://") or u.startswith("https://") or u.startswith("chrome://") or u.startswith("file://")):
                    u = "https://" + u
                await page.goto(u, timeout=45000, wait_until="domcontentloaded")
                await page.bring_to_front()
                return {"status": "navigated", "title": await page.title(), "url": page.url}

            elif action == "click":
                page = await self.get_page(tab_idx)
                target = payload["target"]
                loc = await self.resolve_locator(page, target)
                await loc.click(timeout=10000)
                if payload.get("wait_nav"):
                    try:
                        await page.wait_for_load_state("domcontentloaded", timeout=10000)
                    except Exception:
                        pass
                return {"status": "clicked", "target": target, "url": page.url}

            elif action in ["type", "fill"]:
                page = await self.get_page(tab_idx)
                target = payload["target"]
                text = payload.get("text", payload.get("value", ""))
                loc = await self.resolve_locator(page, target)
                await loc.click(timeout=10000)
                if payload.get("clear_first"):
                    try:
                        await loc.fill("")
                    except Exception:
                        await page.keyboard.press("Control+A")
                        await page.keyboard.press("Backspace")
                await page.keyboard.type(text)
                if payload.get("press_enter"):
                    await page.keyboard.press("Enter")
                    await asyncio.sleep(0.5)
                return {"status": "typed", "target": target, "text_length": len(text)}

            elif action == "press":
                page = await self.get_page(tab_idx)
                key = payload["key"]
                await page.keyboard.press(key)
                return {"status": "pressed", "key": key}

            elif action == "scroll":
                page = await self.get_page(tab_idx)
                direction = str(payload.get("direction", "down")).lower()
                if direction == "down":
                    await page.evaluate("window.scrollBy(0, window.innerHeight * 0.8)")
                elif direction == "up":
                    await page.evaluate("window.scrollBy(0, -window.innerHeight * 0.8)")
                elif direction == "top":
                    await page.evaluate("window.scrollTo(0, 0)")
                elif direction == "bottom":
                    await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
                else:
                    await page.evaluate(f"window.scrollBy(0, {int(direction)})")
                return {"status": "scrolled", "direction": direction}

            elif action in ["screenshot", "ss"]:
                page = await self.get_page(tab_idx)
                out = payload.get("output_path")
                if not out:
                    SCRATCH_DIR.mkdir(parents=True, exist_ok=True)
                    prune_scratch_dir()
                    out = str(SCRATCH_DIR / f"screenshot_{int(time.time()*1000)}.png")
                Path(out).parent.mkdir(parents=True, exist_ok=True)
                await page.screenshot(path=out, full_page=payload.get("full_page", False))
                return {"status": "captured", "screenshot_path": out, "title": await page.title(), "url": page.url}

            elif action in ["get_content", "content", "markdown", "text", "html"]:
                page = await self.get_page(tab_idx)
                fmt = payload.get("format", action if action in ["markdown", "text", "html"] else "markdown")
                sel = payload.get("selector")
                if fmt == "html":
                    if sel:
                        loc = await self.resolve_locator(page, sel)
                        content = await loc.evaluate("el => el.outerHTML")
                    else:
                        content = await page.content()
                elif fmt == "text":
                    if sel:
                        loc = await self.resolve_locator(page, sel)
                        content = await loc.inner_text()
                    else:
                        content = await page.evaluate("() => document.body ? document.body.innerText : ''")
                else: # markdown
                    content = await page.evaluate("""() => {
                        function nodeToMd(node) {
                            if (!node) return '';
                            if (node.nodeType === 3) return node.textContent.trim() ? node.textContent : ' ';
                            if (node.nodeType !== 1) return '';
                            const tag = node.tagName.toLowerCase();
                            if (['script', 'style', 'noscript', 'svg'].includes(tag)) return '';
                            let children = Array.from(node.childNodes).map(nodeToMd).join('');
                            if (!children.trim() && !['img', 'hr', 'br'].includes(tag)) return '';
                            if (tag === 'h1') return `\\n# ${children.trim()}\\n\\n`;
                            if (tag === 'h2') return `\\n## ${children.trim()}\\n\\n`;
                            if (tag === 'h3') return `\\n### ${children.trim()}\\n\\n`;
                            if (tag === 'p') return `\\n${children.trim()}\\n\\n`;
                            if (tag === 'li') return `\\n* ${children.trim()}`;
                            if (tag === 'a') {
                                const href = node.getAttribute('href');
                                return href ? `[${children.trim()}](${href})` : children;
                            }
                            if (tag === 'pre' || tag === 'code') return `\\n\`\`\`\\n${node.textContent}\\n\`\`\`\\n`;
                            return children;
                        }
                        const body = document.querySelector('main') || document.querySelector('article') || document.body;
                        return nodeToMd(body).replace(/\\n{3,}/g, '\\n\\n').trim();
                    }""")
                return {"status": "ok", "format": fmt, "content": content, "url": page.url, "title": await page.title()}

            elif action == "get_links":
                page = await self.get_page(tab_idx)
                links = await page.evaluate("""() => {
                    const results = [];
                    for (const el of document.querySelectorAll('a[href]')) {
                        const href = el.href;
                        const text = el.innerText.trim() || el.getAttribute('aria-label') || '';
                        if (href && !href.startsWith('javascript:')) results.push({text, href});
                    }
                    return results;
                }""")
                return {"status": "ok", "links": links}

            elif action in ["evaluate", "eval"]:
                page = await self.get_page(tab_idx)
                res = await page.evaluate(payload["expression"])
                return {"status": "ok", "result": res}

            elif action == "tabs":
                page = await self.get_page(tab_idx)
                tabs = []
                for i, p in enumerate(self.context.pages):
                    tabs.append({"index": i, "title": await p.title(), "url": p.url})
                return {"status": "ok", "tabs": tabs}

            elif action == "new_tab":
                await self.init_browser()
                page = await self.context.new_page()
                url = payload.get("url")
                if url:
                    u = url if url.startswith("http") else f"https://{url}"
                    await page.goto(u, timeout=45000, wait_until="domcontentloaded")
                await page.bring_to_front()
                return {"status": "opened", "index": len(self.context.pages)-1, "title": await page.title(), "url": page.url}

            elif action == "switch_tab":
                page = await self.get_page(payload["index"])
                return {"status": "focused", "index": payload["index"], "title": await page.title(), "url": page.url}

            elif action == "close_tab":
                idx = payload.get("index", 0)
                if self.context and 0 <= idx < len(self.context.pages):
                    p = self.context.pages[idx]
                    t = await p.title()
                    await p.close()
                    return {"status": "closed", "index": idx, "title": t}
                return {"status": "error", "message": f"Tab index {idx} out of range"}

            elif action == "wait":
                page = await self.get_page(tab_idx)
                target = str(payload.get("target_or_seconds", payload.get("target", "1")))
                try:
                    s = float(target)
                    await asyncio.sleep(s)
                    return {"status": "waited", "seconds": s}
                except ValueError:
                    loc = await self.resolve_locator(page, target)
                    await loc.wait_for(timeout=30000)
                    return {"status": "ready", "target": target}

            elif action == "batch":
                actions = payload.get("actions", [])
                results = []
                for i, sub in enumerate(actions):
                    res = await self.handle_request(sub)
                    results.append({"step": i, "action": sub.get("action"), "result": res})
                    if res.get("status") == "error":
                        break
                return {"status": "completed", "results": results}

            elif action in ["exec_script", "run"]:
                code = payload.get("code", "").strip()
                page = await self.get_page(tab_idx)
                
                # Check if it's a single expression
                lines = code.splitlines()
                if len(lines) == 1 and not any(code.startswith(k) for k in ["import ", "from ", "def ", "class ", "return "]):
                    body = f"    return {code}"
                else:
                    body = "\n".join(f"    {l}" for l in lines)

                func_def = f"async def __run_snippet__(page, context, browser, playwright, asyncio, time):\n{body}"
                scope = {}
                exec(func_def, scope)
                ret_val = await scope["__run_snippet__"](page, self.context, self.browser, self.playwright, asyncio, time)
                return {"status": "ok", "return_value": ret_val}

            elif action in ["stop", "kill"]:
                if self.browser:
                    try:
                        await self.browser.close()
                    except Exception:
                        pass
                if self.playwright:
                    try:
                        await self.playwright.stop()
                    except Exception:
                        pass
                self.running = False
                return {"status": "stopped"}

            else:
                return {"status": "error", "message": f"Unknown action: {action}"}

        except Exception as e:
            return {"status": "error", "error": str(e), "traceback": traceback.format_exc()}

    async def client_connected(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter):
        try:
            while True:
                line = await reader.readline()
                if not line:
                    break
                payload = json.loads(line.decode().strip())
                response = await self.handle_request(payload)
                writer.write(json.dumps(response).encode() + b"\n")
                await writer.drain()
        except Exception:
            pass
        finally:
            writer.close()
            await writer.wait_closed()

    async def start(self):
        if SOCKET_PATH.exists():
            SOCKET_PATH.unlink()

        SOCKET_PATH.parent.mkdir(parents=True, exist_ok=True)
        await self.init_browser()

        self._server = await asyncio.start_unix_server(self.client_connected, path=str(SOCKET_PATH))
        self.running = True

        # Signal handlers
        loop = asyncio.get_running_loop()
        for sig in (signal.SIGTERM, signal.SIGINT):
            loop.add_signal_handler(sig, lambda: asyncio.create_task(self.shutdown()))

        async with self._server:
            await self._server.serve_forever()

    async def shutdown(self):
        self.running = False
        if self._server:
            self._server.close()
            await self._server.wait_closed()
        if SOCKET_PATH.exists():
            SOCKET_PATH.unlink()
        if self.browser:
            await self.browser.close()
        if self.playwright:
            await self.playwright.stop()
        sys.exit(0)

def main():
    daemon = ChronosDaemon()
    try:
        asyncio.run(daemon.start())
    except (KeyboardInterrupt, SystemExit):
        pass

if __name__ == "__main__":
    main()
