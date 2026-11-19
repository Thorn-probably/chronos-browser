use chronos_browser::cdp::launcher::{self, LaunchOptions};
use chronos_browser::cdp::target::PageTarget;
use chronos_browser::cdp::CdpClient;
use chronos_browser::semantic::formatter::format_snapshot;
use chronos_browser::semantic::registry::{
    ActionType, BoundingBox, ElementRegistry, ModalInfo, RawElement, RawSnapshot,
};
use chronos_browser::semantic::take_semantic_snapshot;

#[test]
fn test_semantic_registry_and_formatter_unit() {
    // Construct sample RawSnapshot with modal, inputs, checkboxes, contenteditable
    let raw = RawSnapshot {
        title: "Test Dashboard".to_string(),
        url: "https://example.com/app".to_string(),
        modal: Some(ModalInfo {
            title: "Confirm Transaction".to_string(),
            role: "dialog".to_string(),
            bounds: Some(BoundingBox {
                x: 100.0,
                y: 100.0,
                width: 400.0,
                height: 300.0,
            }),
        }),
        elements: vec![
            // Element outside modal
            RawElement {
                tag: "input".to_string(),
                role: "input".to_string(),
                name: "Username".to_string(),
                value: Some("alice".to_string()),
                placeholder: Some("Enter username".to_string()),
                checked: None,
                disabled: false,
                center: (200.0, 50.0),
                bounds: Some(BoundingBox {
                    x: 100.0,
                    y: 40.0,
                    width: 200.0,
                    height: 30.0,
                }),
                in_modal: false,
                in_viewport: true,
                action_type: ActionType::Input,
            },
            // Checkbox outside modal
            RawElement {
                tag: "input".to_string(),
                role: "checkbox".to_string(),
                name: "Remember me".to_string(),
                value: None,
                placeholder: None,
                checked: Some(true),
                disabled: false,
                center: (120.0, 85.0),
                bounds: None,
                in_modal: false,
                in_viewport: true,
                action_type: ActionType::Toggle,
            },
            // Checkbox unchecked outside modal
            RawElement {
                tag: "input".to_string(),
                role: "checkbox".to_string(),
                name: "Subscribe to emails".to_string(),
                value: None,
                placeholder: None,
                checked: Some(false),
                disabled: false,
                center: (120.0, 115.0),
                bounds: None,
                in_modal: false,
                in_viewport: true,
                action_type: ActionType::Toggle,
            },
            // Contenteditable outside modal
            RawElement {
                tag: "div".to_string(),
                role: "contenteditable".to_string(),
                name: "Comments".to_string(),
                value: Some("Draft text".to_string()),
                placeholder: None,
                checked: None,
                disabled: false,
                center: (300.0, 200.0),
                bounds: None,
                in_modal: false,
                in_viewport: true,
                action_type: ActionType::Input,
            },
            // Elements inside active modal (must be prioritized with top IDs)
            RawElement {
                tag: "button".to_string(),
                role: "button".to_string(),
                name: "Approve Payment".to_string(),
                value: None,
                placeholder: None,
                checked: None,
                disabled: false,
                center: (250.0, 320.0),
                bounds: None,
                in_modal: true,
                in_viewport: true,
                action_type: ActionType::Click,
            },
            RawElement {
                tag: "button".to_string(),
                role: "button".to_string(),
                name: "Cancel".to_string(),
                value: None,
                placeholder: None,
                checked: None,
                disabled: false,
                center: (350.0, 320.0),
                bounds: None,
                in_modal: true,
                in_viewport: true,
                action_type: ActionType::Click,
            },
        ],
        tables: Vec::new(),
    };

    let registry = ElementRegistry::from_raw(raw);
    assert_eq!(registry.len(), 6);
    assert!(registry.active_modal().is_some());

    // Modal elements must be prioritized first!
    let el1 = registry.get(1).expect("Element [1] must exist");
    assert_eq!(el1.name, "Approve Payment");
    assert!(el1.in_modal);
    assert_eq!(el1.action_type, ActionType::Click);

    let el2 = registry.get(2).expect("Element [2] must exist");
    assert_eq!(el2.name, "Cancel");
    assert!(el2.in_modal);

    // Followed by page elements
    let el3 = registry.get(3).expect("Element [3] must exist");
    assert_eq!(el3.name, "Username");
    assert_eq!(el3.value.as_deref(), Some("alice"));
    assert!(!el3.in_modal);

    let el4 = registry.get(4).expect("Element [4] must exist");
    assert_eq!(el4.name, "Remember me");
    assert_eq!(el4.checked, Some(true));

    let el5 = registry.get(5).expect("Element [5] must exist");
    assert_eq!(el5.name, "Subscribe to emails");
    assert_eq!(el5.checked, Some(false));

    let el6 = registry.get(6).expect("Element [6] must exist");
    assert_eq!(el6.role, "contenteditable");
    assert_eq!(el6.value.as_deref(), Some("Draft text"));

    // Format snapshot and verify representation
    let formatted = format_snapshot(&registry);

    // Active dialog banner must be at the top
    assert!(
        formatted.contains("=== ACTIVE DIALOG: Confirm Transaction ==="),
        "Formatted text should highlight active dialog:\n{}",
        formatted
    );

    // Active dialog items
    assert!(formatted.contains("[1] Button \"Approve Payment\""));
    assert!(formatted.contains("[2] Button \"Cancel\""));

    // Page items header
    assert!(formatted.contains("Page Elements:"));
    assert!(formatted.contains("[3] Input \"Username\" value=\"alice\""));
    assert!(formatted.contains("[4] Checkbox \"Remember me\" checked=true"));
    assert!(formatted.contains("[5] Checkbox \"Subscribe to emails\" checked=false"));
    assert!(formatted.contains("[6] Contenteditable \"Comments\" value=\"Draft text\""));
}

#[tokio::test]
async fn test_detector_js_in_headless_chrome() {
    let options = LaunchOptions::new().headless(true);

    let mut process = match launcher::launch(options).await {
        Ok(proc) => proc,
        Err(e) => {
            eprintln!("Skipping Chrome headless test (no Chrome available): {}", e);
            return;
        }
    };

    let client = CdpClient::connect(process.ws_url())
        .await
        .expect("Failed to connect to CDP WebSocket");

    let target = PageTarget::attach_or_create(client)
        .await
        .expect("Failed to attach to page target");

    // Sample HTML with dialog, inputs, checkboxes, and contenteditable
    let html_content = r##"<!DOCTYPE html>
<html>
<head><title>Chronos Semantic Test</title></head>
<body>
  <h1>Test Form</h1>
  <div class="form-group">
    <label for="username">User Name</label>
    <input id="username" type="text" placeholder="Your username" value="chronos_user" />
  </div>
  <div class="form-group">
    <label><input id="chk-remember" type="checkbox" checked /> Keep me logged in</label>
    <label><input id="chk-terms" type="checkbox" /> I accept the terms</label>
  </div>
  <div class="form-group">
    <div id="editable-area" contenteditable="true" aria-label="Rich Notes">Meeting notes here</div>
  </div>
  <button id="btn-save">Save Profile</button>
  <a href="#more-info">More details</a>

  <dialog id="popup-dialog" open>
    <h3>Cookie Notification</h3>
    <button id="btn-accept-cookies">Accept Cookies</button>
    <button id="btn-reject-cookies">Decline</button>
  </dialog>
</body>
</html>"##;

    let encoded_html = urlencoding_html(html_content);
    let data_url = format!("data:text/html;charset=utf-8,{}", encoded_html);

    target
        .navigate(&data_url)
        .await
        .expect("Navigation to data URL failed");

    // Take semantic snapshot using detector.js
    let (registry, formatted) = take_semantic_snapshot(&target)
        .await
        .expect("take_semantic_snapshot failed");

    assert_eq!(registry.title(), "Chronos Semantic Test");
    assert!(
        registry.active_modal().is_some(),
        "Expected active modal from <dialog open>"
    );

    let modal = registry.active_modal().expect("ModalInfo should be present");
    assert!(
        modal.title.contains("Cookie Notification"),
        "Modal title should be 'Cookie Notification', got: '{}'",
        modal.title
    );

    // Verify modal elements are placed at top IDs
    let modal_els = registry.modal_elements();
    assert_eq!(modal_els.len(), 2);
    let modal_names: Vec<&str> = modal_els.iter().map(|e| e.name.as_str()).collect();
    assert!(modal_names.contains(&"Accept Cookies"));
    assert!(modal_names.contains(&"Decline"));

    // Verify background page elements:
    let page_els = registry.page_elements();

    // Check username input
    let username_el = page_els
        .iter()
        .find(|e| e.name.contains("User Name") || e.placeholder.as_deref() == Some("Your username"))
        .expect("Username input must be found");
    assert_eq!(username_el.tag, "input");
    assert_eq!(username_el.value.as_deref(), Some("chronos_user"));

    // Check checkboxes
    let chk_remember = page_els
        .iter()
        .find(|e| e.name.contains("Keep me logged in"))
        .expect("chk-remember must be found");
    assert_eq!(chk_remember.role, "checkbox");
    assert_eq!(chk_remember.checked, Some(true));

    let chk_terms = page_els
        .iter()
        .find(|e| e.name.contains("I accept the terms"))
        .expect("chk-terms must be found");
    assert_eq!(chk_terms.role, "checkbox");
    assert_eq!(chk_terms.checked, Some(false));

    // Check contenteditable
    let contenteditable_el = page_els
        .iter()
        .find(|e| e.role == "contenteditable")
        .expect("Contenteditable element must be found");
    assert_eq!(
        contenteditable_el.value.as_deref(),
        Some("Meeting notes here")
    );

    // Verify formatted string
    assert!(formatted.contains("=== ACTIVE DIALOG: Cookie Notification ==="));
    assert!(formatted.contains("Button \"Accept Cookies\""));
    assert!(formatted.contains("Button \"Decline\""));
    assert!(formatted.contains("checked=true"));
    assert!(formatted.contains("checked=false"));

    // Clean shutdown
    process.shutdown().await.expect("Process shutdown failed");
}

fn urlencoding_html(html: &str) -> String {
    // Simple percent-encoder for HTML data URLs without extra crates
    let mut out = String::new();
    for b in html.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
