# Chronos-Browser Agent Skill Reference

Instructions and specifications for AI agents interacting with web pages via `chronos-browser`.

---

## 1. Mental Model

`chronos-browser` treats web pages as structured state machines instead of visual pixels.
Every interaction consists of:
1. **Snapshot**: Receive a text tree of interactive elements with assigned numeric IDs (`[ref]`), detected modals, and tables.
2. **Action**: Target elements directly by ID (`click(ref)`, `fill(ref, text)`, `toggle(ref)`).
3. **Micro-Settle**: The engine automatically waits for DOM mutations to settle before returning the updated diff.

Screenshots and coordinate guessing are completely unnecessary.

---

## 2. Tool Reference

### Navigation & Discovery
- `browser_navigate`: Go to a URL. Returns the initial page snapshot.
  - `url` (string, required): Full URL (`https://...` or `data:...`).
- `browser_snapshot`: Refresh and return the current semantic element tree.

### Interactions
- `browser_click`: Click an element.
  - `ref` (integer, optional): Element ID from the snapshot.
  - `x`, `y` (number, optional): Fallback coordinate click.
- `browser_fill`: Type into an input, textarea, or `contenteditable` container.
  - `ref` (integer, required): Target element ID.
  - `value` (string, required): Text to insert.
  - `clear` (boolean, optional, default: true): Clear existing content before typing.
- `browser_toggle_checkbox`: Change checkbox or radio status.
  - `ref` (integer, required): Checkbox element ID.
  - `checked` (boolean, optional): Explicit target state (true/false). Toggles if omitted.
- `browser_press_key`: Send keyboard input.
  - `key` (string, required): Key identifier (`Enter`, `Tab`, `Escape`, `Backspace`, `ArrowDown`).
- `browser_scroll`: Scroll view.
  - `direction` (string: `"up"` | `"down"` | `"top"` | `"bottom"`).
  - `delta_y` (number, optional): Pixel offset.

### Session & Lifecycle
- `browser_manage_session`: Control profiles, window visibility, and RAM.
  - `action`:
    - `"list_profiles"`: Show saved profile directories.
    - `"switch_profile"`: Load named profile via `profile` argument.
    - `"set_headful"`: Toggle window visibility (`headful: true/false`).
    - `"purge_memory"`: Trigger V8 garbage collection and memory compaction.
    - `"sleep"` / `"hibernate"`: Terminate Chrome process (drops RAM to ~3.8 MB). Auto-wakes on next action.
    - `"get_status"`: Return active profile and process state.

### Diagnostics (Emergency Fallback)
- `browser_evaluate_js`: Run arbitrary JavaScript in page context.
  - `script` (string, required): JS expression.
- `browser_debug_screenshot`: Capture visual PNG (base64).

---

## 3. Working with Common Scenarios

### Modal Overlays & Cookie Banners
When a modal opens, Chronos surfaces it at the top of the snapshot:
```text
=== ACTIVE DIALOG: Confirm Action ===
[1] Button "Accept"
[2] Button "Decline"

Page Elements:
[3] Input "Search"
```
Always address dialog elements first before interacting with background page elements.

### Contenteditable Fields (e.g. WhatsApp Web)
Chat inputs in WhatsApp Web are `div[contenteditable="true"]`.
Chronos identifies them with role `Contenteditable`.
To send a message:
1. `browser_fill(ref=chat_ref, value="Hello")`
2. `browser_press_key(key="Enter")`

### WhatsApp Web Authentication Flow
1. Start in headful mode using a dedicated profile:
   `browser_manage_session(action="switch_profile", profile="whatsapp")`
   `browser_manage_session(action="set_headful", headful=true)`
2. Navigate to WhatsApp Web:
   `browser_navigate(url="https://web.whatsapp.com")`
3. Prompt human to scan the QR code.
4. Once scanned, switch back to headless mode:
   `browser_manage_session(action="set_headful", headful=false)`
5. Session remains saved in `~/.local/share/chronos-browser/profiles/whatsapp/` indefinitely.
