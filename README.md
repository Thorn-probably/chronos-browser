# Chronos Browser

**High-Speed Headed Browser Engine & Model Context Protocol (MCP) Server for Autonomous AI Agents.**

Chronos Browser solves the fundamental bottleneck of AI browser automation: latency and setup overhead. Instead of spinning up new browser instances, CDP connections, or ad-hoc Python scripts for every interaction, Chronos uses a **resident background daemon** communicating over a high-performance **UNIX domain socket** (`/tmp/chronos-browser.sock`).

Every click, typing stroke, navigation, or DOM inspection completes in **2 to 15 milliseconds**.

---

## Key Features

- ⚡ **Sub-15ms Latency**: Resident background daemon keeps Playwright and the browser context persistently warm in memory. Zero process-startup lag.
- 🌐 **Native Google Chrome**: Prioritizes system `/usr/bin/google-chrome-stable` with hardware GPU acceleration, VA-API video decoding, and natural color rendering (no forced dark mode).
- 🐍 **Embedded Script Runner**: Run complex Playwright logic instantly inside the resident daemon (`chronos-browser run "await page.title()"`) without writing or spawning Python files.
- 🔌 **Standard Model Context Protocol (MCP)**: 18 high-performance tools ready for Hermes, Claude Desktop, Cursor, or Goose.
- ⌨️ **Smart Locators**: Automatically resolves CSS, text content, XPath, aria-labels, placeholders, and button titles.
- 🗂️ **Tab & Profile Isolation**: Manages multiple tabs, headless or headed modes (`DISPLAY=:1`), and persistent sessions (WhatsApp Web, Google Workspace, GitHub).

---

## Architecture

```
+--------------------------------------------------------+
|          AI Agent / User CLI / Claude / Cursor         |
+--------------------------------------------------------+
                           |
            JSON-RPC over UNIX Domain Socket
               (/tmp/chronos-browser.sock)
                           | (< 15ms latency)
+--------------------------------------------------------+
|                 Chronos Resident Daemon                |
|  - In-memory Playwright instance                       |
|  - Active Page/Context references                      |
|  - Embedded Python Snippet Execution Engine            |
+--------------------------------------------------------+
                           |
             Persistent Chrome DevTools Protocol
                           | (127.0.0.1:9222)
+--------------------------------------------------------+
|             Google Chrome Stable (Headed)              |
|  - Hardware GPU Acceleration (VA-API / OpenGL)         |
|  - Persistent Profile (~/.config/chronos-chrome-prof)  |
+--------------------------------------------------------+
```

---

## Installation

```bash
git clone https://github.com/Thorn-probably/chronos-browser.git ~/Documents/MAIN/NEW/chronos-browser
cd ~/Documents/MAIN/NEW/chronos-browser
pip install -e .
```

To create convenience aliases / symlinks:
```bash
ln -sf $(which chronos-browser) ~/.local/bin/ordis-browser
ln -sf $(which chronos-browser-mcp) ~/.local/bin/ordis-browser-mcp
```

---

## CLI Quickstart

The CLI automatically starts the resident daemon on first use:

```bash
# Check status and open tabs
chronos-browser status

# Navigate
chronos-browser goto "https://web.whatsapp.com"
chronos-browser search "autonomous coding agents"

# Instant DOM Interaction (< 15ms)
chronos-browser click "button#submit"
chronos-browser click "Shashank Jangid"
chronos-browser type "input[type='text']" "Hello world" --press-enter
chronos-browser press "Enter"
chronos-browser scroll down

# Instant Content Extraction
chronos-browser markdown
chronos-browser text "div.message-in"
chronos-browser links
chronos-browser screenshot /tmp/preview.png

# Embedded Script Runner (No separate Python files needed!)
chronos-browser run "await page.title()"
chronos-browser run "await page.locator('textarea').fill('Fast agent'); await page.keyboard.press('Enter')"

# Tab Management
chronos-browser tabs
chronos-browser new-tab "https://github.com"
chronos-browser switch-tab 1
chronos-browser close-tab 0

# Stop Browser
chronos-browser stop
```

---

## MCP Server Integration

Add Chronos Browser to your MCP configuration:

### Claude Desktop (`~/.config/Claude/claude_desktop_config.json`) / Cursor:
```json
{
  "mcpServers": {
    "chronos-browser": {
      "command": "chronos-browser-mcp"
    }
  }
}
```

### Hermes Agent (`~/.hermes/config.yaml`):
```yaml
mcp_servers:
  chronos-browser:
    command: /home/pranshoo/.local/bin/chronos-browser-mcp
    enabled: true
```

### Available MCP Tools (18 Tools):
| Tool | Description |
|---|---|
| `browser_launch` | Launch or ensure headed Chromium/Chrome is running |
| `browser_status` | Return running state, active tabs, and port |
| `browser_navigate` | Navigate active or specified tab to URL |
| `browser_click` | Click elements by selector, text, aria-label, or title |
| `browser_type` | Type/fill text into inputs, textareas, contenteditable |
| `browser_press_key` | Send keyboard keys (Enter, Escape, Tab, etc.) |
| `browser_scroll` | Scroll up, down, top, bottom, or exact delta |
| `browser_get_content` | Extract clean markdown, text, or HTML |
| `browser_get_links` | Return all interactive page links with URLs |
| `browser_screenshot` | Capture viewport or full-page PNG |
| `browser_evaluate` | Run arbitrary JavaScript in page context |
| `browser_tabs` | List open tabs with title and URL |
| `browser_new_tab` | Open a new tab |
| `browser_switch_tab` | Focus specific tab index |
| `browser_close_tab` | Close specified tab index |
| `browser_wait` | Wait for selector or duration |
| `browser_batch` | Execute chained multi-step actions in one request |
| `browser_run_code` | Execute Python snippets directly in resident daemon |
| `browser_stop` | Terminate browser and resident daemon |

---

## License

MIT License. See [LICENSE](LICENSE) for details.
