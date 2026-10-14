"""
Chronos Browser CLI - Sub-millisecond terminal interface powered by resident daemon.
"""

import os
import sys
import json
import shlex
import argparse
from pathlib import Path

from chronos_browser.client import client

def main():
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--tab", type=int, default=None, help="Target tab index")
    common.add_argument("--json", action="store_true", help="Output results in JSON format")

    parser = argparse.ArgumentParser(description="Chronos Browser CLI (High Speed)", parents=[common])
    subparsers = parser.add_subparsers(dest="command", required=True)

    # launch
    p_launch = subparsers.add_parser("launch", aliases=["start"], parents=[common])
    p_launch.add_argument("url", nargs="?", default=None)
    p_launch.add_argument("--display", default=None)
    p_launch.add_argument("--focus", action="store_true", help="Bring window to front on launch")

    # status
    p_status = subparsers.add_parser("status", parents=[common])

    # goto
    p_goto = subparsers.add_parser("goto", aliases=["open"], parents=[common])
    p_goto.add_argument("url")

    # search
    p_search = subparsers.add_parser("search", parents=[common])
    p_search.add_argument("query", nargs="+")

    # click
    p_click = subparsers.add_parser("click", parents=[common])
    p_click.add_argument("target")
    p_click.add_argument("--wait", action="store_true")

    # fill
    p_fill = subparsers.add_parser("fill", parents=[common])
    p_fill.add_argument("selector")
    p_fill.add_argument("value")
    p_fill.add_argument("--press-enter", action="store_true")

    # type
    p_type = subparsers.add_parser("type", parents=[common])
    p_type.add_argument("selector")
    p_type.add_argument("value")
    p_type.add_argument("--press-enter", action="store_true")

    # press
    p_press = subparsers.add_parser("press", parents=[common])
    p_press.add_argument("key")

    # scroll
    p_scroll = subparsers.add_parser("scroll", parents=[common])
    p_scroll.add_argument("direction")

    # screenshot
    p_ss = subparsers.add_parser("screenshot", aliases=["ss"], parents=[common])
    p_ss.add_argument("output", nargs="?", default=None)
    p_ss.add_argument("--full", action="store_true")

    # text
    p_text = subparsers.add_parser("text", parents=[common])
    p_text.add_argument("selector", nargs="?", default=None)

    # markdown
    p_md = subparsers.add_parser("markdown", aliases=["md"], parents=[common])

    # html
    p_html = subparsers.add_parser("html", parents=[common])
    p_html.add_argument("selector", nargs="?", default=None)

    # links
    p_links = subparsers.add_parser("links", parents=[common])

    # eval
    p_eval = subparsers.add_parser("eval", parents=[common])
    p_eval.add_argument("expr", nargs="+")

    # wait
    p_wait = subparsers.add_parser("wait", parents=[common])
    p_wait.add_argument("target")

    # tabs
    p_tabs = subparsers.add_parser("tabs", aliases=["list"], parents=[common])

    # new-tab
    p_newtab = subparsers.add_parser("new-tab", parents=[common])
    p_newtab.add_argument("url", nargs="?", default=None)

    # switch-tab
    p_switch = subparsers.add_parser("switch-tab", aliases=["tab"], parents=[common])
    p_switch.add_argument("index", type=int)
    p_switch.add_argument("--focus", action="store_true")

    # focus
    p_focus = subparsers.add_parser("focus", parents=[common])

    # close-tab
    p_closetab = subparsers.add_parser("close-tab", parents=[common])
    p_closetab.add_argument("index", type=int, nargs="?", default=0)

    # batch
    p_batch = subparsers.add_parser("batch", parents=[common])
    p_batch.add_argument("commands", nargs="+")

    # run (embedded python script runner)
    p_run = subparsers.add_parser("run", parents=[common])
    p_run.add_argument("code_or_file", nargs="+")

    # mcp (run MCP server)
    p_mcp = subparsers.add_parser("mcp", aliases=["serve"], parents=[common])

    # stop
    p_stop = subparsers.add_parser("stop", parents=[common])

    args = parser.parse_args()
    cmd = args.command

    if cmd in ["mcp", "serve"]:
        from chronos_browser.server import main as server_main
        server_main()
        return

    # Dispatch to resident daemon via client
    payload = {"tab_index": args.tab}

    if cmd in ["launch", "start"]:
        payload.update({"action": "launch", "url": args.url, "display": args.display, "focus": args.focus})
    elif cmd == "status":
        payload.update({"action": "status"})
    elif cmd in ["goto", "open"]:
        payload.update({"action": "goto", "url": args.url})
    elif cmd == "search":
        q = " ".join(args.query)
        payload.update({"action": "goto", "url": f"https://www.google.com/search?q={q}"})
    elif cmd == "click":
        payload.update({"action": "click", "target": args.target, "wait_nav": args.wait})
    elif cmd == "fill":
        payload.update({"action": "type", "target": args.selector, "value": args.value, "clear_first": True, "press_enter": args.press_enter})
    elif cmd == "type":
        payload.update({"action": "type", "target": args.selector, "value": args.value, "press_enter": args.press_enter})
    elif cmd == "press":
        payload.update({"action": "press", "key": args.key})
    elif cmd == "scroll":
        payload.update({"action": "scroll", "direction": args.direction})
    elif cmd in ["screenshot", "ss"]:
        payload.update({"action": "screenshot", "output_path": args.output, "full_page": args.full})
    elif cmd == "text":
        payload.update({"action": "get_content", "format": "text", "selector": args.selector})
    elif cmd in ["markdown", "md"]:
        payload.update({"action": "get_content", "format": "markdown"})
    elif cmd == "html":
        payload.update({"action": "get_content", "format": "html", "selector": args.selector})
    elif cmd == "links":
        payload.update({"action": "get_links"})
    elif cmd == "eval":
        payload.update({"action": "evaluate", "expression": " ".join(args.expr)})
    elif cmd == "wait":
        payload.update({"action": "wait", "target": args.target})
    elif cmd in ["tabs", "list"]:
        payload.update({"action": "tabs"})
    elif cmd == "new-tab":
        payload.update({"action": "new_tab", "url": args.url})
    elif cmd in ["switch-tab", "tab"]:
        payload.update({"action": "switch_tab", "index": args.index, "focus": args.focus})
    elif cmd == "focus":
        payload.update({"action": "focus"})
    elif cmd == "close-tab":
        payload.update({"action": "close_tab", "index": args.index})
    elif cmd == "batch":
        raw = " ".join(args.commands)
        subcmds = [c.strip() for c in raw.split(";") if c.strip()]
        batch_acts = []
        for c in subcmds:
            parts = shlex.split(c)
            act = parts[0].lower()
            if act in ["goto", "open"]:
                batch_acts.append({"action": "goto", "url": parts[1]})
            elif act == "click":
                batch_acts.append({"action": "click", "target": parts[1]})
            elif act in ["type", "fill"]:
                batch_acts.append({"action": "type", "target": parts[1], "value": parts[2] if len(parts)>2 else "", "press_enter": "--enter" in parts})
            elif act == "press":
                batch_acts.append({"action": "press", "key": parts[1]})
            elif act == "wait":
                batch_acts.append({"action": "wait", "target": parts[1]})
            elif act in ["screenshot", "ss"]:
                batch_acts.append({"action": "screenshot", "output_path": parts[1] if len(parts)>1 else None})
        payload.update({"action": "batch", "actions": batch_acts})
    elif cmd == "run":
        target = " ".join(args.code_or_file)
        if os.path.exists(target):
            with open(target, "r") as f:
                code = f.read()
        else:
            code = target
        payload.update({"action": "exec_script", "code": code})
    elif cmd == "stop":
        payload.update({"action": "stop"})

    resp = client.send(payload)

    if args.json:
        print(json.dumps(resp, indent=2))
        return

    if resp.get("status") == "error":
        print(f"Error: {resp.get('error') or resp.get('message')}", file=sys.stderr)
        sys.exit(1)

    # Format human output
    if cmd == "status":
        print(f"Chronos Browser: RUNNING (Binary: {resp.get('browser_bin')}, Port: {resp.get('cdp_port')})")
        print(f"Open Tabs ({resp.get('tab_count', 0)}):")
        for t in resp.get("tabs", []):
            print(f"  [{t['index']}] {t['title']} -> {t['url']}")
    elif cmd in ["goto", "open", "launch"]:
        print(f"Navigated: [{resp.get('title')}] {resp.get('url')}")
    elif cmd == "click":
        print(f"Clicked: {resp.get('target')}")
    elif cmd in ["type", "fill"]:
        print(f"Typed into {resp.get('target')}")
    elif cmd in ["screenshot", "ss"]:
        print(f"Screenshot saved: {resp.get('screenshot_path')}")
    elif cmd in ["text", "markdown", "md", "html"]:
        print(resp.get("content", ""))
    elif cmd == "links":
        for l in resp.get("links", []):
            print(f"- {l['text']}: {l['href']}")
    elif cmd in ["tabs", "list"]:
        for t in resp.get("tabs", []):
            print(f"[{t['index']}] {t['title']} ({t['url']})")
    elif cmd == "run":
        if "return_value" in resp and resp["return_value"] is not None:
            print(resp["return_value"])
        else:
            print("Script executed successfully.")
    else:
        print(json.dumps(resp))

if __name__ == "__main__":
    main()
