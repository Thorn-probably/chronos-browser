"""
Chronos Browser MCP Server - High-speed Model Context Protocol Server
backed by the resident Chronos daemon.
"""

from typing import Optional, List, Dict, Any
from mcp.server.mcpserver import MCPServer
from chronos_browser.client import client

app = MCPServer(
    name="chronos-browser",
    instructions="High-speed headed Chromium browser driver and automation suite for AI agents."
)

@app.tool()
async def browser_launch(
    url: Optional[str] = None,
    headed: bool = False,
    display: Optional[str] = None
) -> Dict[str, Any]:
    """Launch or verify Chrome browser (defaults to background mode). Set headed=True to show UI on monitor."""
    return client.send({"action": "launch", "url": url, "display": display, "headed": headed})

@app.tool()
async def browser_status() -> Dict[str, Any]:
    """Check browser status, active tabs, and CDP connection state."""
    return client.send({"action": "status"})

@app.tool()
async def browser_navigate(url: str, tab_index: Optional[int] = None) -> Dict[str, Any]:
    """Navigate a tab to the specified URL."""
    return client.send({"action": "goto", "url": url, "tab_index": tab_index})

@app.tool()
async def browser_click(
    target: str,
    wait_navigation: bool = False,
    tab_index: Optional[int] = None
) -> Dict[str, Any]:
    """Click an element identified by CSS selector, text, XPath, aria-label, title, or placeholder."""
    return client.send({"action": "click", "target": target, "wait_nav": wait_navigation, "tab_index": tab_index})

@app.tool()
async def browser_type(
    target: str,
    text: str,
    press_enter: bool = False,
    clear_first: bool = False,
    tab_index: Optional[int] = None
) -> Dict[str, Any]:
    """Type or fill text into an input, textarea, or contenteditable element."""
    return client.send({
        "action": "type",
        "target": target,
        "text": text,
        "press_enter": press_enter,
        "clear_first": clear_first,
        "tab_index": tab_index
    })

@app.tool()
async def browser_press_key(key: str, tab_index: Optional[int] = None) -> Dict[str, Any]:
    """Press a keyboard key (e.g. Enter, Escape, Tab, Backspace, ArrowDown)."""
    return client.send({"action": "press", "key": key, "tab_index": tab_index})

@app.tool()
async def browser_scroll(direction: str = "down", tab_index: Optional[int] = None) -> Dict[str, Any]:
    """Scroll page up, down, top, bottom, or by specific pixel amount."""
    return client.send({"action": "scroll", "direction": direction, "tab_index": tab_index})

@app.tool()
async def browser_get_content(
    format: str = "markdown",
    selector: Optional[str] = None,
    tab_index: Optional[int] = None
) -> Dict[str, Any]:
    """Extract page content in 'markdown', 'text', or 'html' format."""
    return client.send({"action": "get_content", "format": format, "selector": selector, "tab_index": tab_index})

@app.tool()
async def browser_get_links(tab_index: Optional[int] = None) -> Dict[str, Any]:
    """Extract all interactive hyperlinks from the current page."""
    return client.send({"action": "get_links", "tab_index": tab_index})

@app.tool()
async def browser_screenshot(
    output_path: Optional[str] = None,
    full_page: bool = False,
    tab_index: Optional[int] = None
) -> Dict[str, Any]:
    """Capture a screenshot of the active page and save it as PNG."""
    return client.send({"action": "screenshot", "output_path": output_path, "full_page": full_page, "tab_index": tab_index})

@app.tool()
async def browser_evaluate(expression: str, tab_index: Optional[int] = None) -> Dict[str, Any]:
    """Evaluate arbitrary JavaScript expression in the page context."""
    return client.send({"action": "evaluate", "expression": expression, "tab_index": tab_index})

@app.tool()
async def browser_tabs() -> Dict[str, Any]:
    """List all open tabs."""
    return client.send({"action": "tabs"})

@app.tool()
async def browser_new_tab(url: Optional[str] = None) -> Dict[str, Any]:
    """Open a new browser tab with optional URL."""
    return client.send({"action": "new_tab", "url": url})

@app.tool()
async def browser_switch_tab(tab_index: int) -> Dict[str, Any]:
    """Switch active focus to a specific tab index."""
    return client.send({"action": "switch_tab", "index": tab_index})

@app.tool()
async def browser_close_tab(tab_index: Optional[int] = None) -> Dict[str, Any]:
    """Close a specific tab or the current tab."""
    return client.send({"action": "close_tab", "index": tab_index or 0})

@app.tool()
async def browser_wait(target_or_seconds: str, tab_index: Optional[int] = None) -> Dict[str, Any]:
    """Wait for seconds (e.g. '2.5') or wait for a CSS/text selector to appear."""
    return client.send({"action": "wait", "target_or_seconds": target_or_seconds, "tab_index": tab_index})

@app.tool()
async def browser_batch(actions: List[Dict[str, Any]], tab_index: Optional[int] = None) -> Dict[str, Any]:
    """Execute a sequence of browser actions in a single rapid batch to eliminate round-trip overhead."""
    return client.send({"action": "batch", "actions": actions, "tab_index": tab_index})

@app.tool()
async def browser_run_code(code: str, tab_index: Optional[int] = None) -> Dict[str, Any]:
    """Execute Python automation code directly inside the resident browser daemon."""
    return client.send({"action": "exec_script", "code": code, "tab_index": tab_index})

@app.tool()
async def browser_stop() -> Dict[str, Any]:
    """Gracefully stop the background browser process."""
    return client.send({"action": "stop"})

def main():
    app.run("stdio")

if __name__ == "__main__":
    main()
