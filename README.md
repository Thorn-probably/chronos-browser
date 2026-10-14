# Chronos Browser ⚡

**High-Performance Headed Chromium Driver & Model Context Protocol (MCP) Server for AI Agents.**

Chronos Browser provides instantaneous, robust browser automation for AI coding assistants and autonomous agents (Hermes Agent, Claude Desktop, Cursor, Goose, Zed, Windsurf, etc.). 

Unlike naive wrappers that spawn a fresh browser process or disconnect CDP on every action, Chronos Browser keeps an active, in-memory connection to a persistent headed Chromium session with hardware GPU acceleration, natural rendering (no forced contrast or dark filters), and a multi-action pipeline that eliminates LLM round-trip latency.

---

## ✨ Key Features

- **Standard MCP Server**: 18 specialized browser automation tools callable by any MCP client.
- **Persistent Profile & State**: Retains logins, cookies, localStorage, and session state across runs (`~/.config/chronos-chrome-profile`).
- **Zero Inversion / Forced Filters**: Clean, natural color rendering for all web pages and screenshots.
- **Pipeline Batching (`browser_batch`)**: Execute multi-step sequences (e.g. *navigate → wait → fill input → press Enter → screenshot*) in a single tool call.
- **Smart Element Locators**: Intelligently resolves CSS selectors, text matches, XPath, `aria-label`, `placeholder`, and interactive roles.
- **Dual Interface**: Run as an MCP server via stdio, or control directly via the `chronos-browser` CLI.

---

## 🚀 Quickstart & Installation

### Requirements
- Linux / macOS (X11, Wayland, or headless Xvfb)
- Python 3.10+
- Chromium, Google Chrome, or Brave installed

### Install from Source
```bash
git clone https://github.com/Thorn-probably/chronos-browser.git
cd chronos-browser
pip install -e .
```

Ensure Playwright dependencies are installed:
```bash
playwright install chromium
```

---

## 🤖 Connecting to AI Agents (MCP Setup)

Chronos Browser communicates over standard I/O via the Model Context Protocol (`mcp`).

### 1. Hermes Agent (`~/.hermes/config.yaml`)
```yaml
mcp_servers:
  chronos-browser:
    command: chronos-browser-mcp
    enabled: true
```

### 2. Claude Desktop (`claude_desktop_config.json`)
```json
{
  "mcpServers": {
    "chronos-browser": {
      "command": "chronos-browser-mcp"
    }
  }
}
```

### 3. Cursor (`~/.cursor/mcp.json` or project `.cursor/mcp.json`)
```json
{
  "mcpServers": {
    "chronos-browser": {
      "command": "chronos-browser-mcp"
    }
  }
}
```

### 4. Direct CLI execution as MCP server
```bash
chronos-browser mcp
# or
chronos-browser-mcp
```

---

## 🛠️ Available MCP Tools

| Tool | Parameters | Description |
|---|---|---|
| `browser_launch` | `url`, `display` | Start headed Chromium if not already running. |
| `browser_status` | none | Check browser status, active tabs, and CDP state. |
| `browser_navigate` | `url`, `tab_index` | Navigate to URL (auto-prefixes `https://`). |
| `browser_click` | `target`, `wait_navigation`, `tab_index` | Smart click (CSS, text, XPath, aria-label, title). |
| `browser_type` | `target`, `text`, `press_enter`, `clear_first`, `tab_index` | Fill/type text into input, textarea, or `contenteditable`. |
| `browser_press_key` | `key`, `tab_index` | Press keyboard key (`Enter`, `Escape`, `Tab`, `ArrowDown`, etc.). |
| `browser_scroll` | `direction` (`up`/`down`/`top`/`bottom`/`pixels`) | Scroll viewport smoothly. |
| `browser_screenshot` | `output_path`, `full_page`, `tab_index` | Capture screenshot to PNG. |
| `browser_get_content` | `format` (`markdown`/`text`/`html`), `selector` | Extract clean DOM markdown, text, or HTML. |
| `browser_get_links` | `tab_index` | Extract all clickable links (`text`, `href`). |
| `browser_evaluate` | `expression`, `tab_index` | Evaluate JavaScript in the page context. |
| `browser_tabs` | none | List all open tabs with indexes, titles, and URLs. |
| `browser_new_tab` | `url` | Open and focus a new tab. |
| `browser_switch_tab` | `tab_index` | Switch active focus to a tab index. |
| `browser_close_tab` | `tab_index` | Close tab. |
| `browser_wait` | `target_or_seconds` | Wait for seconds (`"2"`) or for selector to appear. |
| `browser_batch` | `actions: List[Dict]` | **Execute multi-action pipelines in 1 round trip.** |
| `browser_stop` | none | Gracefully stop the browser process. |

---

## ⚡ The `browser_batch` Tool

To prevent latency from sequential round trips across multi-step actions, pass a batch list:

```json
{
  "actions": [
    {"action": "navigate", "url": "https://news.ycombinator.com"},
    {"action": "wait", "target_or_seconds": "1.5"},
    {"action": "click", "target": "past"},
    {"action": "screenshot", "output_path": "/tmp/hn_past.png"}
  ]
}
```

---

## 💻 CLI Commands

Chronos Browser also includes a full CLI for fast terminal operations:

```bash
# Launch browser
chronos-browser launch "https://github.com"

# Check status & list tabs
chronos-browser status

# Navigation
chronos-browser goto "https://example.com"
chronos-browser search "model context protocol"

# Interaction
chronos-browser click "Sign In"
chronos-browser fill "input[name='q']" "playwright automation" --press-enter
chronos-browser type "textarea[name='comment']" "Hello world!" --press-enter
chronos-browser press "Enter"

# Content Extraction & Screenshots
chronos-browser markdown
chronos-browser text
chronos-browser screenshot /tmp/capture.png

# Tab Management
chronos-browser new-tab "https://news.ycombinator.com"
chronos-browser close-tab --tab 1

# Semicolon Batch Execution
chronos-browser batch "goto https://example.com ; wait 1 ; screenshot"

# Stop
chronos-browser stop
```

---

## ⚙️ Environment Variables

| Variable | Default | Description |
|---|---|---|
| `DISPLAY` | `:1` | X11 display target. |
| `CHRONOS_CDP_PORT` | `9222` | Remote debugging CDP port. |
| `CHRONOS_CHROME_PROFILE` | `~/.config/chronos-chrome-profile` | Directory for persistent browser user profile. |
| `CHRONOS_CHROME_BIN` | auto-detected | Override path to custom Chromium/Chrome binary. |

---

## 📄 License

MIT License. Copyright (c) 2026 Pranshoo Mittal.
