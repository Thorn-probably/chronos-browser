# Chronos-Browser

A fast, lightweight browser and Model Context Protocol (MCP) server written in Rust. It eliminates token waste from vision/screenshot models by parsing web pages into a compact, numeric-referenced semantic tree.

## Highlights

- **Pure Rust Engine**: Communicates directly with Chromium over WebSocket Chrome DevTools Protocol (CDP). No Node.js or Playwright needed.
- **Zero-Screenshot Workflow**: Turns pages into actionable element maps (`[1] Button "Submit"`, `[2] Input "Username"`). Actions target numeric IDs directly.
- **Dialog & Overlay Isolation**: Automatically detects modal dialogs and presents modal controls first so agents never click occluded backdrop elements.
- **Table & Content Parsing**: Extracts tables and key content into structured text alongside controls.
- **Session Persistence**: Stores sessions in named profiles (`~/.local/share/chronos-browser/profiles/`). Log in once (e.g. WhatsApp Web QR code in headful mode), then automate headlessly.
- **Memory Footprint**: Capped V8 heaps and resource limits keep active RAM around 350 MB PSS. `sleep` mode drops idle RAM to ~3.8 MB with instant wake.

## Installation

Requires Rust 1.75+ and a Chromium binary (`google-chrome`, `chromium`, or `brave`).

```bash
cargo build --release
cargo install --path .
```

## CLI Usage

Run directly from your shell:

```bash
# Open page headlessly and print snapshot
chronos-browser open "https://news.ycombinator.com"

# Open in visible window with named profile (for QR scan or manual login)
chronos-browser open "https://web.whatsapp.com" --profile whatsapp --headful

# List saved persistent profiles
chronos-browser profiles
```

## MCP Configuration

### Antigravity (`~/.gemini/config/mcp_config.json`)
```json
{
  "mcpServers": {
    "chronos-browser": {
      "command": "chronos-browser",
      "args": ["mcp"]
    }
  }
}
```

### Claude Desktop (`claude_desktop_config.json`)
```json
{
  "mcpServers": {
    "chronos-browser": {
      "command": "chronos-browser",
      "args": ["mcp"]
    }
  }
}
```

## Verification

```bash
cargo test --lib --tests
```
