"""
Chronos Browser MCP Server - Exposes browser automation as Model Context Protocol tools.
"""

import os
import sys
import time
import asyncio
from pathlib import Path
from typing import Optional, List, Dict, Any

from mcp.server.mcpserver import MCPServer
from chronos_browser.driver import (
    AsyncBrowserManager,
    ensure_browser_process,
    is_cdp_ready,
    find_browser_binary,
    prune_scratch_dir,
    CDP_URL,
    CDP_PORT,
    DEFAULT_DISPLAY,
    SCRATCH_DIR
)

app = MCPServer(
    name="chronos-browser",
    instructions="High-speed headed Chromium browser driver and automation suite for AI agents."
)

mgr = AsyncBrowserManager()

@app.tool()
async def browser_launch(url: Optional[str] = None, display: Optional[str] = None) -> Dict[str, Any]:
    """Launch or verify headed Chromium browser on DISPLAY and optionally navigate to a URL."""
    disp = display or DEFAULT_DISPLAY
    ready = ensure_browser_process(display=disp)
    if not ready:
        return {"status": "error", "message": f"Could not launch browser on {disp}"}

    result = {
        "status": "ready",
        "cdp_url": CDP_URL,
        "display": disp,
        "browser_bin": find_browser_binary()
    }
    if url:
        result["navigation"] = await browser_navigate(url=url)
    return result

@app.tool()
async def browser_status() -> Dict[str, Any]:
    """Check browser status, active tabs, and CDP connection state."""
    ready = is_cdp_ready()
    if not ready:
        return {"running": False, "cdp_port": CDP_PORT}

    try:
        context = await mgr.get_context()
        tabs = []
        for i, page in enumerate(context.pages):
            try:
                tabs.append({
                    "index": i,
                    "title": await page.title(),
                    "url": page.url
                })
            except Exception:
                tabs.append({"index": i, "title": "[unavailable]", "url": ""})

        return {
            "running": True,
            "cdp_url": CDP_URL,
            "display": DEFAULT_DISPLAY,
            "tab_count": len(tabs),
            "tabs": tabs
        }
    except Exception as e:
        return {"running": True, "error": str(e)}

@app.tool()
async def browser_navigate(url: str, tab_index: Optional[int] = None) -> Dict[str, Any]:
    """Navigate a tab to the specified URL."""
    if not (url.startswith("http://") or url.startswith("https://") or url.startswith("chrome://") or url.startswith("file://")):
        url = "https://" + url

    page = await mgr.get_page(tab_index)
    await page.goto(url, timeout=45000, wait_until="domcontentloaded")
    await page.bring_to_front()
    return {
        "status": "navigated",
        "title": await page.title(),
        "url": page.url
    }

@app.tool()
async def browser_click(
    target: str,
    wait_navigation: bool = False,
    tab_index: Optional[int] = None
) -> Dict[str, Any]:
    """Click an element identified by CSS selector, text, XPath, aria-label, title, or placeholder."""
    page = await mgr.get_page(tab_index)
    loc = await mgr.resolve_locator(page, target)
    await loc.click(timeout=12000)
    if wait_navigation:
        try:
            await page.wait_for_load_state("domcontentloaded", timeout=10000)
        except Exception:
            pass
    return {"status": "clicked", "target": target, "url": page.url}

@app.tool()
async def browser_type(
    target: str,
    text: str,
    press_enter: bool = False,
    clear_first: bool = False,
    tab_index: Optional[int] = None
) -> Dict[str, Any]:
    """Type or fill text into an input, textarea, or contenteditable element."""
    page = await mgr.get_page(tab_index)
    loc = await mgr.resolve_locator(page, target)
    await loc.click(timeout=10000)

    if clear_first:
        try:
            await loc.fill("")
        except Exception:
            await page.keyboard.press("Control+A")
            await page.keyboard.press("Backspace")

    await page.keyboard.type(text)

    if press_enter:
        await page.keyboard.press("Enter")
        await asyncio.sleep(0.5)

    return {
        "status": "typed",
        "target": target,
        "text_length": len(text),
        "pressed_enter": press_enter
    }

@app.tool()
async def browser_press_key(key: str, tab_index: Optional[int] = None) -> Dict[str, Any]:
    """Press a keyboard key (e.g. Enter, Escape, Tab, Backspace, ArrowDown)."""
    page = await mgr.get_page(tab_index)
    await page.keyboard.press(key)
    return {"status": "key_pressed", "key": key}

@app.tool()
async def browser_scroll(direction: str = "down", tab_index: Optional[int] = None) -> Dict[str, Any]:
    """Scroll page up, down, top, bottom, or by specific pixel amount."""
    page = await mgr.get_page(tab_index)
    d = direction.lower()
    if d == "down":
        await page.evaluate("window.scrollBy(0, window.innerHeight * 0.8)")
    elif d == "up":
        await page.evaluate("window.scrollBy(0, -window.innerHeight * 0.8)")
    elif d == "top":
        await page.evaluate("window.scrollTo(0, 0)")
    elif d == "bottom":
        await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
    else:
        try:
            pixels = int(d)
            await page.evaluate(f"window.scrollBy(0, {pixels})")
        except ValueError:
            return {"status": "error", "message": f"Invalid scroll value: {direction}"}
    return {"status": "scrolled", "direction": direction}

@app.tool()
async def browser_get_content(
    format: str = "markdown",
    selector: Optional[str] = None,
    tab_index: Optional[int] = None
) -> Dict[str, Any]:
    """Extract page content in 'markdown', 'text', or 'html' format."""
    page = await mgr.get_page(tab_index)

    if format == "html":
        if selector:
            loc = await mgr.resolve_locator(page, selector)
            content = await loc.evaluate("el => el.outerHTML")
        else:
            content = await page.content()
    elif format == "text":
        if selector:
            loc = await mgr.resolve_locator(page, selector)
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
                if (tag === 'h4') return `\\n#### ${children.trim()}\\n\\n`;
                if (tag === 'p') return `\\n${children.trim()}\\n\\n`;
                if (tag === 'li') return `\\n* ${children.trim()}`;
                if (tag === 'a') {
                    const href = node.getAttribute('href');
                    return href ? `[${children.trim()}](${href})` : children;
                }
                if (tag === 'pre' || tag === 'code') return `\\n\`\`\`\\n${node.textContent}\\n\`\`\`\\n`;
                if (tag === 'blockquote') return `\\n> ${children.trim()}\\n\\n`;
                return children;
            }
            const body = document.querySelector('main') || document.querySelector('article') || document.body;
            return nodeToMd(body).replace(/\\n{3,}/g, '\\n\\n').trim();
        }""")

    return {
        "title": await page.title(),
        "url": page.url,
        "format": format,
        "content": content
    }

@app.tool()
async def browser_get_links(tab_index: Optional[int] = None) -> List[Dict[str, str]]:
    """Extract all interactive hyperlinks from the current page."""
    page = await mgr.get_page(tab_index)
    links = await page.evaluate("""() => {
        const results = [];
        const els = document.querySelectorAll('a[href]');
        for (const el of els) {
            const text = el.innerText.trim();
            const href = el.href;
            if (href && !href.startsWith('javascript:')) {
                results.push({
                    text: text || el.getAttribute('aria-label') || '[no text]',
                    href: href
                });
            }
        }
        return results;
    }""")
    return links

@app.tool()
async def browser_screenshot(
    output_path: Optional[str] = None,
    full_page: bool = False,
    tab_index: Optional[int] = None
) -> Dict[str, Any]:
    """Capture a screenshot of the active page and save it as PNG."""
    page = await mgr.get_page(tab_index)
    if output_path:
        out = Path(output_path).expanduser().resolve()
    else:
        SCRATCH_DIR.mkdir(parents=True, exist_ok=True)
        prune_scratch_dir()
        out = SCRATCH_DIR / f"screenshot_{int(time.time()*1000)}.png"

    out.parent.mkdir(parents=True, exist_ok=True)
    await page.bring_to_front()
    await page.screenshot(path=str(out), full_page=full_page, timeout=15000, animations="disabled")

    return {
        "screenshot_path": str(out),
        "url": page.url,
        "title": await page.title()
    }

@app.tool()
async def browser_evaluate(expression: str, tab_index: Optional[int] = None) -> Any:
    """Evaluate arbitrary JavaScript expression in the page context."""
    page = await mgr.get_page(tab_index)
    result = await page.evaluate(expression)
    return {"result": result}

@app.tool()
async def browser_tabs() -> List[Dict[str, Any]]:
    """List all open tabs."""
    context = await mgr.get_context()
    tabs = []
    for i, p in enumerate(context.pages):
        try:
            tabs.append({
                "index": i,
                "title": await p.title(),
                "url": p.url
            })
        except Exception:
            tabs.append({"index": i, "title": "[unavailable]", "url": ""})
    return tabs

@app.tool()
async def browser_new_tab(url: Optional[str] = None) -> Dict[str, Any]:
    """Open a new browser tab with optional URL."""
    context = await mgr.get_context()
    page = await context.new_page()
    if url:
        if not (url.startswith("http://") or url.startswith("https://")):
            url = "https://" + url
        await page.goto(url, timeout=45000, wait_until="domcontentloaded")
    await page.bring_to_front()
    idx = len(context.pages) - 1
    return {
        "status": "opened",
        "tab_index": idx,
        "title": await page.title(),
        "url": page.url
    }

@app.tool()
async def browser_switch_tab(tab_index: int) -> Dict[str, Any]:
    """Switch active focus to a specific tab index."""
    page = await mgr.get_page(tab_index)
    await page.bring_to_front()
    return {
        "status": "focused",
        "tab_index": tab_index,
        "title": await page.title(),
        "url": page.url
    }

@app.tool()
async def browser_close_tab(tab_index: Optional[int] = None) -> Dict[str, Any]:
    """Close a specific tab or the current tab."""
    context = await mgr.get_context()
    pages = context.pages
    if not pages:
        return {"status": "no_tabs"}
    idx = tab_index if tab_index is not None else 0
    if 0 <= idx < len(pages):
        p = pages[idx]
        title = await p.title()
        await p.close()
        return {"status": "closed", "index": idx, "title": title}
    return {"status": "error", "message": f"Index {idx} out of range"}

@app.tool()
async def browser_wait(target_or_seconds: str, tab_index: Optional[int] = None) -> Dict[str, Any]:
    """Wait for seconds (e.g. '2.5') or wait for a CSS/text selector to appear."""
    page = await mgr.get_page(tab_index)
    try:
        sec = float(target_or_seconds)
        await asyncio.sleep(sec)
        return {"status": "waited", "duration": sec}
    except ValueError:
        loc = await mgr.resolve_locator(page, target_or_seconds)
        await loc.wait_for(timeout=30000)
        return {"status": "ready", "target": target_or_seconds}

@app.tool()
async def browser_batch(actions: List[Dict[str, Any]], tab_index: Optional[int] = None) -> Dict[str, Any]:
    """
    Execute a sequence of browser actions in a single rapid batch to eliminate round-trip overhead.
    
    Example actions:
    [
        {"action": "navigate", "url": "https://google.com"},
        {"action": "type", "target": "textarea[name='q']", "text": "Playwright MCP", "press_enter": true},
        {"action": "wait", "target_or_seconds": "2"},
        {"action": "screenshot"}
    ]
    """
    results = []
    page = await mgr.get_page(tab_index)

    for i, act in enumerate(actions):
        action_name = act.get("action", "").lower()
        try:
            if action_name in ["navigate", "goto"]:
                u = act["url"]
                if not (u.startswith("http://") or u.startswith("https://")):
                    u = "https://" + u
                await page.goto(u, timeout=45000, wait_until="domcontentloaded")
                results.append({"step": i, "action": action_name, "status": "ok", "url": page.url})

            elif action_name == "click":
                loc = await mgr.resolve_locator(page, act["target"])
                await loc.click(timeout=10000)
                if act.get("wait_nav"):
                    await page.wait_for_load_state("domcontentloaded", timeout=10000)
                results.append({"step": i, "action": action_name, "status": "ok", "target": act["target"]})

            elif action_name in ["type", "fill"]:
                loc = await mgr.resolve_locator(page, act["target"])
                await loc.click(timeout=10000)
                if act.get("clear_first"):
                    try:
                        await loc.fill("")
                    except Exception:
                        await page.keyboard.press("Control+A")
                        await page.keyboard.press("Backspace")
                await page.keyboard.type(act["text"])
                if act.get("press_enter"):
                    await page.keyboard.press("Enter")
                    await asyncio.sleep(0.5)
                results.append({"step": i, "action": action_name, "status": "ok", "target": act["target"]})

            elif action_name == "press":
                await page.keyboard.press(act["key"])
                results.append({"step": i, "action": action_name, "status": "ok", "key": act["key"]})

            elif action_name == "wait":
                target = str(act.get("target_or_seconds", act.get("target", "1")))
                try:
                    sec = float(target)
                    await asyncio.sleep(sec)
                except ValueError:
                    loc = await mgr.resolve_locator(page, target)
                    await loc.wait_for(timeout=30000)
                results.append({"step": i, "action": action_name, "status": "ok", "waited": target})

            elif action_name in ["screenshot", "ss"]:
                p_out = act.get("output_path")
                if not p_out:
                    SCRATCH_DIR.mkdir(parents=True, exist_ok=True)
                    p_out = str(SCRATCH_DIR / f"batch_{int(time.time()*1000)}.png")
                await page.screenshot(path=p_out, full_page=act.get("full_page", False))
                results.append({"step": i, "action": action_name, "status": "ok", "path": p_out})

            elif action_name == "evaluate":
                res = await page.evaluate(act["expression"])
                results.append({"step": i, "action": action_name, "status": "ok", "result": res})

            else:
                results.append({"step": i, "action": action_name, "status": "error", "error": f"Unknown action: {action_name}"})

        except Exception as e:
            results.append({"step": i, "action": action_name, "status": "error", "error": str(e)})
            break

    return {
        "status": "completed",
        "total_steps": len(actions),
        "steps_executed": len(results),
        "results": results
    }

@app.tool()
async def browser_stop() -> Dict[str, Any]:
    """Gracefully stop the background browser process."""
    await mgr.stop()
    return {"status": "stopped"}

def main():
    app.run("stdio")

if __name__ == "__main__":
    main()
