"""
Chronos Browser CLI - High-speed terminal command line interface.
"""

import os
import sys
import time
import json
import shlex
import argparse
import subprocess
import urllib.parse
from pathlib import Path

from chronos_browser.driver import (
    CDP_URL,
    CDP_PORT,
    DEFAULT_DISPLAY,
    DEFAULT_PROFILE,
    SCRATCH_DIR,
    find_browser_binary,
    ensure_browser_process,
    is_cdp_ready,
    get_cdp_pages,
    prune_scratch_dir
)

def get_playwright_context():
    from playwright.sync_api import sync_playwright
    p = sync_playwright().start()
    try:
        browser = p.chromium.connect_over_cdp(CDP_URL)
        context = browser.contexts[0] if browser.contexts else browser.new_context()
        return p, browser, context
    except Exception as e:
        p.stop()
        raise e

def resolve_page(context, tab_idx=None):
    pages = context.pages
    if not pages:
        return context.new_page()
    if tab_idx is not None:
        if 0 <= tab_idx < len(pages):
            return pages[tab_idx]
        raise IndexError(f"Tab index {tab_idx} out of range (0-{len(pages)-1})")
    return pages[0]

def smart_locator(page, target: str):
    if any(target.startswith(p) for p in ["text=", "css=", "xpath=", "role=", "id="]):
        return page.locator(target).first
    if target.startswith("//") or target.startswith("(//"):
        return page.locator(f"xpath={target}").first
    try:
        loc = page.locator(target)
        if loc.count() > 0:
            return loc.first
    except Exception:
        pass
    for attr in ["aria-label", "title", "placeholder", "name"]:
        try:
            loc = page.locator(f"[{attr}*='{target}']")
            if loc.count() > 0:
                return loc.first
        except Exception:
            pass
    return page.get_by_text(target, exact=False).first

# CLI Handlers
def cmd_launch(args):
    display = args.display or DEFAULT_DISPLAY
    ready = ensure_browser_process(display)
    if not ready:
        print(f"Error: Failed to launch Chromium on {display}", file=sys.stderr)
        sys.exit(1)

    if args.url:
        p, browser, context = get_playwright_context()
        try:
            page = resolve_page(context, args.tab)
            page.goto(args.url, timeout=30000)
            print(f"Launched and navigated to: {page.url} ({page.title()})")
        finally:
            p.stop()
    else:
        print(f"Chronos Browser is running (DISPLAY={display}, CDP={CDP_URL}, BIN={find_browser_binary()})")

def cmd_status(args):
    ready = is_cdp_ready()
    if not ready:
        if args.json:
            print(json.dumps({"running": False, "cdp_port": CDP_PORT}))
        else:
            print(f"Chronos Browser: STOPPED (port {CDP_PORT} not reachable)")
        return

    pages = get_cdp_pages()
    data = {
        "running": True,
        "cdp_port": CDP_PORT,
        "display": DEFAULT_DISPLAY,
        "browser_bin": find_browser_binary(),
        "tab_count": len(pages),
        "tabs": [{"index": i, "title": t.get("title", ""), "url": t.get("url", "")} for i, t in enumerate(pages)]
    }
    if args.json:
        print(json.dumps(data, indent=2))
    else:
        print(f"Chronos Browser: RUNNING (CDP port {CDP_PORT})")
        print(f"Tabs open ({len(pages)}):")
        for t in data["tabs"]:
            print(f"  [{t['index']}] {t['title']} -> {t['url']}")

def cmd_goto(args):
    ensure_browser_process()
    p, browser, context = get_playwright_context()
    try:
        page = resolve_page(context, args.tab)
        url = args.url
        if not (url.startswith("http://") or url.startswith("https://") or url.startswith("chrome://") or url.startswith("file://")):
            url = "https://" + url
        page.goto(url, timeout=45000, wait_until="domcontentloaded")
        page.bring_to_front()
        if args.json:
            print(json.dumps({"title": page.title(), "url": page.url}))
        else:
            print(f"Navigated: [{page.title()}] {page.url}")
    finally:
        p.stop()

def cmd_search(args):
    ensure_browser_process()
    query = " ".join(args.query)
    search_url = f"https://www.google.com/search?q={urllib.parse.quote(query)}"
    p, browser, context = get_playwright_context()
    try:
        page = resolve_page(context, args.tab)
        page.goto(search_url, timeout=45000, wait_until="domcontentloaded")
        page.bring_to_front()
        if args.json:
            print(json.dumps({"query": query, "title": page.title(), "url": page.url}))
        else:
            print(f"Searched: {query}\nTitle: {page.title()}\nURL: {page.url}")
    finally:
        p.stop()

def cmd_new_tab(args):
    ensure_browser_process()
    p, browser, context = get_playwright_context()
    try:
        page = context.new_page()
        if args.url:
            url = args.url if args.url.startswith("http") else f"https://{args.url}"
            page.goto(url, timeout=45000, wait_until="domcontentloaded")
        page.bring_to_front()
        tab_idx = len(context.pages) - 1
        print(f"Opened Tab [{tab_idx}]: {page.title()} ({page.url})")
    finally:
        p.stop()

def cmd_close_tab(args):
    ensure_browser_process()
    p, browser, context = get_playwright_context()
    try:
        pages = context.pages
        if not pages:
            print("No tabs open.")
            return
        idx = args.tab if args.tab is not None else 0
        if 0 <= idx < len(pages):
            t = pages[idx]
            title = t.title()
            t.close()
            print(f"Closed tab [{idx}]: {title}")
    finally:
        p.stop()

def cmd_click(args):
    ensure_browser_process()
    p, browser, context = get_playwright_context()
    try:
        page = resolve_page(context, args.tab)
        loc = smart_locator(page, args.target)
        loc.click(timeout=10000)
        if args.wait:
            page.wait_for_load_state("domcontentloaded", timeout=10000)
        print(f"Clicked: {args.target}")
    finally:
        p.stop()

def cmd_fill(args):
    ensure_browser_process()
    p, browser, context = get_playwright_context()
    try:
        page = resolve_page(context, args.tab)
        loc = smart_locator(page, args.selector)
        loc.fill(args.value, timeout=10000)
        if args.press_enter:
            loc.press("Enter")
        print(f"Filled: {args.selector}")
    finally:
        p.stop()

def cmd_type(args):
    ensure_browser_process()
    p, browser, context = get_playwright_context()
    try:
        page = resolve_page(context, args.tab)
        loc = smart_locator(page, args.selector)
        loc.click(timeout=10000)
        page.keyboard.type(args.value)
        if args.press_enter:
            page.keyboard.press("Enter")
        print(f"Typed: {args.value}")
    finally:
        p.stop()

def cmd_press(args):
    ensure_browser_process()
    p, browser, context = get_playwright_context()
    try:
        page = resolve_page(context, args.tab)
        page.keyboard.press(args.key)
        print(f"Pressed: {args.key}")
    finally:
        p.stop()

def cmd_screenshot(args):
    ensure_browser_process()
    p, browser, context = get_playwright_context()
    try:
        page = resolve_page(context, args.tab)
        if args.output:
            out_path = Path(args.output).expanduser().resolve()
        else:
            SCRATCH_DIR.mkdir(parents=True, exist_ok=True)
            prune_scratch_dir()
            out_path = SCRATCH_DIR / f"screenshot_{int(time.time()*1000)}.png"
        out_path.parent.mkdir(parents=True, exist_ok=True)
        page.screenshot(path=str(out_path), full_page=args.full)
        print(f"Screenshot saved: {out_path}")
    finally:
        p.stop()

def cmd_text(args):
    ensure_browser_process()
    p, browser, context = get_playwright_context()
    try:
        page = resolve_page(context, args.tab)
        if args.selector:
            loc = smart_locator(page, args.selector)
            text = loc.inner_text()
        else:
            text = page.evaluate("() => document.body ? document.body.innerText : ''")
        print(text)
    finally:
        p.stop()

def cmd_markdown(args):
    ensure_browser_process()
    p, browser, context = get_playwright_context()
    try:
        page = resolve_page(context, args.tab)
        from chronos_browser.server import browser_get_content
        import asyncio
        data = asyncio.run(browser_get_content(format="markdown", tab_index=args.tab))
        print(data.get("content", ""))
    finally:
        p.stop()

def cmd_batch(args):
    ensure_browser_process()
    raw = " ".join(args.commands)
    subcmds = [c.strip() for c in raw.split(";") if c.strip()]
    p, browser, context = get_playwright_context()
    try:
        page = resolve_page(context, args.tab)
        for cmd_str in subcmds:
            parts = shlex.split(cmd_str)
            if not parts:
                continue
            action = parts[0].lower()
            a_args = parts[1:]
            if action in ["goto", "open"]:
                u = a_args[0] if a_args[0].startswith("http") else f"https://{a_args[0]}"
                page.goto(u, wait_until="domcontentloaded")
                print(f"-> Navigated: {u}")
            elif action == "click":
                loc = smart_locator(page, a_args[0])
                loc.click()
                print(f"-> Clicked: {a_args[0]}")
            elif action in ["fill", "type"]:
                loc = smart_locator(page, a_args[0])
                val = a_args[1] if len(a_args) > 1 else ""
                loc.fill(val)
                print(f"-> Filled: {a_args[0]}")
            elif action == "wait":
                try:
                    time.sleep(float(a_args[0]))
                except ValueError:
                    smart_locator(page, a_args[0]).wait_for()
                print(f"-> Waited: {a_args[0]}")
            elif action == "press":
                page.keyboard.press(a_args[0])
                print(f"-> Pressed: {a_args[0]}")
            elif action in ["screenshot", "ss"]:
                out = a_args[0] if a_args else str(SCRATCH_DIR / f"ss_{int(time.time()*1000)}.png")
                page.screenshot(path=out)
                print(f"-> Screenshot: {out}")
    finally:
        p.stop()

def cmd_mcp(args):
    from chronos_browser.server import main as server_main
    server_main()

def cmd_stop(args):
    if not is_cdp_ready():
        print("Chronos Browser is not running.")
        return
    subprocess.run(["pkill", "-9", "-f", f"remote-debugging-port={CDP_PORT}"], check=False)
    print("Chronos Browser stopped.")

def main():
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--tab", type=int, default=None, help="Target tab index")
    common.add_argument("--json", action="store_true", help="Output results in JSON format")

    parser = argparse.ArgumentParser(description="Chronos Browser CLI", parents=[common])
    subparsers = parser.add_subparsers(dest="command", required=True)

    # launch
    p_launch = subparsers.add_parser("launch", aliases=["start"], parents=[common])
    p_launch.add_argument("url", nargs="?", default=None)
    p_launch.add_argument("--display", default=None)
    p_launch.set_defaults(func=cmd_launch)

    # status
    p_status = subparsers.add_parser("status", parents=[common])
    p_status.set_defaults(func=cmd_status)

    # goto
    p_goto = subparsers.add_parser("goto", aliases=["open"], parents=[common])
    p_goto.add_argument("url")
    p_goto.set_defaults(func=cmd_goto)

    # search
    p_search = subparsers.add_parser("search", parents=[common])
    p_search.add_argument("query", nargs="+")
    p_search.set_defaults(func=cmd_search)

    # click
    p_click = subparsers.add_parser("click", parents=[common])
    p_click.add_argument("target")
    p_click.add_argument("--wait", action="store_true")
    p_click.set_defaults(func=cmd_click)

    # fill
    p_fill = subparsers.add_parser("fill", parents=[common])
    p_fill.add_argument("selector")
    p_fill.add_argument("value")
    p_fill.add_argument("--press-enter", action="store_true")
    p_fill.set_defaults(func=cmd_fill)

    # type
    p_type = subparsers.add_parser("type", parents=[common])
    p_type.add_argument("selector")
    p_type.add_argument("value")
    p_type.add_argument("--press-enter", action="store_true")
    p_type.set_defaults(func=cmd_type)

    # press
    p_press = subparsers.add_parser("press", parents=[common])
    p_press.add_argument("key")
    p_press.set_defaults(func=cmd_press)

    # screenshot
    p_ss = subparsers.add_parser("screenshot", parents=[common])
    p_ss.add_argument("output", nargs="?", default=None)
    p_ss.add_argument("--full", action="store_true")
    p_ss.set_defaults(func=cmd_screenshot)

    # text
    p_text = subparsers.add_parser("text", parents=[common])
    p_text.add_argument("selector", nargs="?", default=None)
    p_text.set_defaults(func=cmd_text)

    # markdown
    p_md = subparsers.add_parser("markdown", aliases=["md"], parents=[common])
    p_md.set_defaults(func=cmd_markdown)

    # new-tab
    p_newtab = subparsers.add_parser("new-tab", parents=[common])
    p_newtab.add_argument("url", nargs="?", default=None)
    p_newtab.set_defaults(func=cmd_new_tab)

    # close-tab
    p_closetab = subparsers.add_parser("close-tab", parents=[common])
    p_closetab.set_defaults(func=cmd_close_tab)

    # batch
    p_batch = subparsers.add_parser("batch", parents=[common])
    p_batch.add_argument("commands", nargs="+")
    p_batch.set_defaults(func=cmd_batch)

    # mcp
    p_mcp = subparsers.add_parser("mcp", aliases=["serve"], parents=[common])
    p_mcp.set_defaults(func=cmd_mcp)

    # stop
    p_stop = subparsers.add_parser("stop", parents=[common])
    p_stop.set_defaults(func=cmd_stop)

    args = parser.parse_args()
    args.func(args)

if __name__ == "__main__":
    main()
