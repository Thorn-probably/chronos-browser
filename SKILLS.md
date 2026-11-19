# Chronos-Browser: Agent Skill & MCP Manual

Comprehensive guide for AI agents and orchestrators controlling `chronos-browser` via Model Context Protocol (MCP).

---

## 1. Operating Rules for Agents

1. **Never request or wait for screenshots.** Chronos maps the DOM into numeric reference IDs (`[1]`, `[2]`). Interacting with numbers uses 95% fewer tokens and has zero vision failure rate.
2. **Always inspect dialog headers first.** If the snapshot starts with `=== ACTIVE DIALOG: <title> ===`, any click outside the dialog will fail or be blocked by an overlay backdrop. Clear or resolve the modal using its IDs first.
3. **Use `browser_fill` for all text inputs.** This works identically on `<input>`, `<textarea>`, and `contenteditable` elements (such as WhatsApp Web or Notion).
4. **Sleep when done.** If your task is waiting on the user or entering a long pause, call `browser_manage_session(action="sleep")` to drop RAM from 360 MB down to 3.6 MB.

---

## 2. MCP Tool Specifications & Example Payloads

### `browser_navigate`
Opens a URL and returns the initial parsed semantic snapshot.
```json
{
  "name": "browser_navigate",
  "arguments": {
    "url": "https://example.com/login"
  }
}
```
**Returns:** Text snapshot of the loaded page with numeric element IDs.

---

### `browser_snapshot`
Refreshes and prints the current page state, active dialogs, tables, and elements.
```json
{
  "name": "browser_snapshot",
  "arguments": {}
}
```

---

### `browser_click`
Clicks an element by its numeric reference ID from the latest snapshot.
```json
{
  "name": "browser_click",
  "arguments": {
    "ref": 3
  }
}
```
*Note:* Automatically waits for DOM mutations to settle before returning the updated snapshot.

---

### `browser_fill`
Focuses an element, clears previous content by default, and types the given value.
```json
{
  "name": "browser_fill",
  "arguments": {
    "ref": 1,
    "value": "admin@example.com",
    "clear": true
  }
}
```

---

### `browser_toggle_checkbox`
Sets or toggles checkboxes and radio buttons.
```json
{
  "name": "browser_toggle_checkbox",
  "arguments": {
    "ref": 4,
    "checked": true
  }
}
```

---

### `browser_press_key`
Dispatches keyboard events. Supported keys include: `"Enter"`, `"Tab"`, `"Escape"`, `"Backspace"`, `"ArrowDown"`, `"ArrowUp"`.
```json
{
  "name": "browser_press_key",
  "arguments": {
    "key": "Enter"
  }
}
```

---

### `browser_scroll`
Scrolls the page or viewport.
```json
{
  "name": "browser_scroll",
  "arguments": {
    "direction": "down",
    "delta_y": 500
  }
}
```

---

### `browser_manage_session`
Controls persistent profiles, visibility, and memory.
```json
{
  "name": "browser_manage_session",
  "arguments": {
    "action": "sleep"
  }
}
```
**Valid `action` values:**
- `"sleep"` or `"hibernate"`: Closes the Chromium process to drop RAM to 3.6 MB. Keeps session cookies and IndexedDB on disk. Wakes automatically on the next action.
- `"purge_memory"`: Triggers V8 garbage collection and critical memory pressure eviction without closing the page.
- `"set_headful"`: Sets `"headful": true` to display the actual Chrome UI on screen (e.g. for user QR scan), or `false` for headless.
- `"switch_profile"`: Changes active directory using `"profile": "name"`.
- `"list_profiles"`: Lists all persistent profile directories.
- `"get_status"`: Returns JSON status of browser PID, profile, and headful mode.

---

## 3. Step-by-Step Practical Recipes

### Recipe A: Logging into a Website
1. Navigate:
   `browser_navigate(url="https://site.com/login")`
2. Inspect snapshot response:
   ```text
   Page: Login - https://site.com/login
   Interactive Elements:
   [1] Input "Email address" placeholder="you@domain.com"
   [2] Input "Password" placeholder="••••••••"
   [3] Checkbox "Remember me" checked=false
   [4] Button "Sign In"
   ```
3. Fill credentials:
   `browser_fill(ref=1, value="user@domain.com")`
   `browser_fill(ref=2, value="SecretPassword123")`
   `browser_toggle_checkbox(ref=3, checked=true)`
4. Click submit:
   `browser_click(ref=4)`
5. Inspect the resulting dashboard snapshot returned by `browser_click`.

---

### Recipe B: Dismissing Popups and Modals
When a site opens a cookie consent banner or terms modal:
```text
Page: App Dashboard - https://site.com/app

=== ACTIVE DIALOG: Accept Terms & Privacy ===
[1] Button "Accept All"
[2] Button "Reject Non-Essential"

Page Elements:
[3] Input "Search records"
[4] Button "Create New"
```
1. Click the dialog action first:
   `browser_click(ref=1)`
2. The modal closes, and the background elements update their ref IDs in the next snapshot.

---

### Recipe C: Reading Tabular Data
Chronos parses HTML tables into delimited markdown rows automatically:
```text
Page: Database Logs - https://site.com/logs

Tables & Data:
Timestamp | IP Address | Status
2026-10-17 05:26:37 UTC | 192.168.1.45 | OK
2026-10-17 05:28:10 UTC | 10.0.0.12 | FAILED

Interactive Elements:
[1] Button "Next Page →"
[2] Button "Export CSV"
```
Read the data directly from the text block. No custom JavaScript or screenshot OCR needed.

---

### Recipe D: WhatsApp Web Messaging
WhatsApp chat inputs use `contenteditable` divs instead of traditional inputs.
1. Target the chat input:
   `browser_fill(ref=<chat_input_ref>, value="Meeting in 10 minutes.")`
2. Send via keyboard:
   `browser_press_key(key="Enter")`

---

### Recipe E: Saving Memory on Long Pauses
When the agent finishes an interaction or is waiting for user feedback:
`browser_manage_session(action="sleep")`
- Chrome exits cleanly.
- IndexedDB, cookies, and tokens stay saved in `~/.local/share/chronos-browser/profiles/`.
- RAM drops to **3.6 MB**.
- Calling `browser_navigate` or `browser_snapshot` on your next turn immediately revives the session.
