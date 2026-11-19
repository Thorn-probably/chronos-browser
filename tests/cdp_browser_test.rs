use chronos_browser::cdp::launcher::{self, LaunchOptions};
use chronos_browser::cdp::target::PageTarget;
use chronos_browser::cdp::CdpClient;
use chronos_browser::semantic::take_semantic_snapshot;

#[tokio::test]
async fn test_cdp_headless_chrome_full_workflow() {
    // 1. Launch headless Chrome with dynamic port
    let options = LaunchOptions::new()
        .headless(true)
        .window_size(1280, 800);

    let mut process = match launcher::launch(options).await {
        Ok(proc) => proc,
        Err(e) => {
            eprintln!("Skipping CDP browser test (Chrome binary unavailable): {}", e);
            return;
        }
    };

    assert!(!process.ws_url().is_empty());
    assert!(process.ws_url().starts_with("ws://"));
    assert!(process.pid().is_some());

    // 2. Connect CdpClient over WebSocket
    let client = CdpClient::connect(process.ws_url())
        .await
        .expect("WebSocket connection to Chrome DevTools Protocol failed");

    // 3. Attach or create PageTarget
    let target = PageTarget::attach_or_create(client)
        .await
        .expect("Failed to attach to browser page target");

    assert!(!target.target_id().is_empty());
    assert!(!target.session_id().is_empty());

    // 4. Navigate to interactive test page
    let html = r##"<!DOCTYPE html>
<html>
<head>
  <title>CDP Browser Test Page</title>
  <style>
    body { font-family: sans-serif; margin: 20px; }
    button { padding: 10px 20px; cursor: pointer; }
    input { padding: 8px; width: 200px; }
  </style>
</head>
<body>
  <h1 id="header">Chronos CDP Verification</h1>
  
  <div style="margin: 20px 0;">
    <button id="counter-btn" onclick="incrementCounter()">Click Count: <span id="count">0</span></button>
  </div>

  <div style="margin: 20px 0;">
    <label for="input-name">Full Name</label>
    <input id="input-name" type="text" placeholder="Type name here" />
  </div>

  <div style="margin: 20px 0;">
    <label><input id="opt-in" type="checkbox" checked /> Send updates</label>
  </div>

  <div style="margin: 20px 0;">
    <a id="test-link" href="https://example.com/learn">Learn More</a>
  </div>

  <script>
    window.clickCounter = 0;
    function incrementCounter() {
      window.clickCounter += 1;
      document.getElementById('count').innerText = window.clickCounter;
    }
  </script>
</body>
</html>"##;

    let encoded_html = urlencoding(html);
    let data_url = format!("data:text/html;charset=utf-8,{}", encoded_html);

    target
        .navigate(&data_url)
        .await
        .expect("Navigation to data URL failed");

    // 5. Verify page title, URL and DOM content
    let title = target.title().await.expect("Failed to get page title");
    assert_eq!(title, "CDP Browser Test Page");

    let url = target.url().await.expect("Failed to get page url");
    assert!(url.starts_with("data:text/html"));

    let content = target.content().await.expect("Failed to get HTML content");
    assert!(content.contains("Chronos CDP Verification"));
    assert!(content.contains("counter-btn"));

    // 6. Test evaluate() expression execution
    let eval_math = target.evaluate("40 + 2").await.expect("Eval math failed");
    assert_eq!(eval_math, 42);

    let initial_count = target
        .evaluate("window.clickCounter")
        .await
        .expect("Eval counter failed");
    assert_eq!(initial_count, 0);

    // 7. Extract interactive elements using semantic detector
    let (registry, formatted) = take_semantic_snapshot(&target)
        .await
        .expect("take_semantic_snapshot failed");

    assert!(registry.len() >= 4);
    assert!(formatted.contains("CDP Browser Test Page"));

    // Locate the counter button in the semantic registry
    let button_el = registry
        .find_by_role_and_name("button", "Click Count")
        .expect("Counter button should be detected in registry");
    assert_eq!(button_el.role, "button");
    let (btn_x, btn_y) = button_el.center;
    assert!(btn_x > 0.0 && btn_y > 0.0);

    // 8. Test mouse click dispatch on the button's exact coordinates
    target
        .dispatch_mouse_click(btn_x, btn_y)
        .await
        .expect("dispatch_mouse_click failed");

    // Allow click event to process in DOM
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let updated_count = target
        .evaluate("window.clickCounter")
        .await
        .expect("Eval counter after click failed");
    assert_eq!(
        updated_count, 1,
        "Clicking at ({}, {}) should have incremented window.clickCounter to 1",
        btn_x, btn_y
    );

    // Click it a second time
    target
        .dispatch_mouse_click(btn_x, btn_y)
        .await
        .expect("Second click failed");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let second_count = target
        .evaluate("window.clickCounter")
        .await
        .expect("Eval counter after second click failed");
    assert_eq!(second_count, 2);

    // 9. Test keyboard typing via dispatch_key_input
    // Focus the text input first
    let input_el = registry
        .find_by_role_and_name("input", "Full Name")
        .or_else(|| registry.elements().iter().find(|e| e.tag == "input" && e.role == "input"))
        .expect("Input field should be found");

    target
        .dispatch_mouse_click(input_el.center.0, input_el.center.1)
        .await
        .expect("Click input to focus failed");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Type text into the focused field
    target
        .dispatch_key_input("Chronos Agent")
        .await
        .expect("dispatch_key_input failed");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let typed_val = target
        .evaluate("document.getElementById('input-name').value")
        .await
        .expect("Eval input value failed");
    assert_eq!(
        typed_val, "Chronos Agent",
        "Input field should contain typed text"
    );

    // 10. Test scroll
    target.scroll(0.0, 50.0).await.expect("scroll failed");

    // 11. Test screenshot capture
    let png_bytes = target.screenshot().await.expect("screenshot failed");
    assert!(!png_bytes.is_empty(), "Screenshot bytes must not be empty");
    // Verify standard PNG file header: 0x89 0x50 0x4E 0x47 0x0D 0x0A 0x1A 0x0A
    let png_header = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    assert!(
        png_bytes.len() >= 8 && png_bytes[..8] == png_header,
        "Screenshot does not start with valid PNG magic bytes"
    );

    // 12. Clean shutdown of process
    process
        .shutdown()
        .await
        .expect("Chrome process clean shutdown failed");
}

fn urlencoding(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
