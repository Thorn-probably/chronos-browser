use crate::cdp::client::CdpClient;
use crate::error::{ChronosError, Result};
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

/// High-level page abstraction over a CDP session.
#[derive(Clone)]
pub struct PageTarget {
    client: Arc<CdpClient>,
    target_id: String,
    session_id: String,
}

impl PageTarget {
    /// Creates a new PageTarget by attaching to a newly created browser target.
    pub async fn new_page(client: Arc<CdpClient>, url: Option<&str>) -> Result<Self> {
        let initial_url = url.unwrap_or("about:blank");
        let res = client
            .send_browser(
                "Target.createTarget",
                Some(serde_json::json!({ "url": initial_url })),
            )
            .await?;

        let target_id = res
            .get("targetId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ChronosError::TargetNotFound(
                    "Target.createTarget did not return a targetId".to_string(),
                )
            })?
            .to_string();

        let attach_res = client
            .send_browser(
                "Target.attachToTarget",
                Some(serde_json::json!({
                    "targetId": &target_id,
                    "flatten": true
                })),
            )
            .await?;

        let session_id = attach_res
            .get("sessionId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ChronosError::TargetNotFound(
                    "Target.attachToTarget did not return a sessionId".to_string(),
                )
            })?
            .to_string();

        let page = Self {
            client,
            target_id,
            session_id,
        };

        page.init_domains().await?;
        Ok(page)
    }

    /// Attaches to an existing default page target or creates one if none exists.
    pub async fn attach_or_create(client: Arc<CdpClient>) -> Result<Self> {
        let targets = client.send_browser("Target.getTargets", None).await?;
        if let Some(target_list) = targets.get("targetInfos").and_then(|v| v.as_array()) {
            for target in target_list {
                if target.get("type").and_then(|t| t.as_str()) == Some("page") {
                    if let Some(target_id) = target.get("targetId").and_then(|id| id.as_str()) {
                        let attach_res = client
                            .send_browser(
                                "Target.attachToTarget",
                                Some(serde_json::json!({
                                    "targetId": target_id,
                                    "flatten": true
                                })),
                            )
                            .await?;

                        if let Some(session_id) =
                            attach_res.get("sessionId").and_then(|s| s.as_str())
                        {
                            let page = Self {
                                client: Arc::clone(&client),
                                target_id: target_id.to_string(),
                                session_id: session_id.to_string(),
                            };
                            page.init_domains().await?;
                            return Ok(page);
                        }
                    }
                }
            }
        }

        Self::new_page(client, None).await
    }

    /// Attaches directly to a known target_id and session_id.
    pub fn from_session(client: Arc<CdpClient>, target_id: String, session_id: String) -> Self {
        Self {
            client,
            target_id,
            session_id,
        }
    }

    /// Initializes essential CDP domains for the session.
    pub async fn init_domains(&self) -> Result<()> {
        self.call("Page.enable", None).await?;
        self.call("Runtime.enable", None).await?;
        self.call("DOM.enable", None).await?;
        let _ = self.call("CSS.enable", None).await;
        Ok(())
    }

    /// Sends a CDP command to this page's session.
    pub async fn call(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        self.client
            .send_session(&self.session_id, method, params)
            .await
    }

    /// Target ID of this page.
    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    /// Session ID of this page.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Reference to the underlying CDP client.
    pub fn client(&self) -> &Arc<CdpClient> {
        &self.client
    }

    /// Navigates the page to the specified URL and waits for initial load.
    pub async fn navigate(&self, url: &str) -> Result<serde_json::Value> {
        debug!("Navigating to: {}", url);
        let mut event_rx = self.client.subscribe_events();

        let nav_res = self
            .call("Page.navigate", Some(serde_json::json!({ "url": url })))
            .await?;

        // Wait for page load event or readyState complete with a fallback timeout
        let sid = self.session_id.clone();
        let wait_result = tokio::time::timeout(Duration::from_secs(15), async {
            while let Ok(event) = event_rx.recv().await {
                if event.session_id.as_deref() == Some(&sid) {
                    if event.method == "Page.loadEventFired"
                        || event.method == "Page.frameStoppedLoading"
                    {
                        break;
                    }
                }
            }
        })
        .await;

        if wait_result.is_err() {
            debug!("Timed out waiting for Page.loadEventFired; verifying document.readyState");
            // Check if document is at least interactive or complete
            let _ = self
                .evaluate("document.readyState")
                .await
                .map_err(|e| debug!("readyState check: {}", e));
        }

        Ok(nav_res)
    }

    /// Evaluates a JavaScript expression in the context of the page, returning its value.
    pub async fn evaluate(&self, expression: &str) -> Result<serde_json::Value> {
        let res = self
            .call(
                "Runtime.evaluate",
                Some(serde_json::json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                    "userGesture": true
                })),
            )
            .await?;

        if let Some(exception_details) = res.get("exceptionDetails") {
            let text = exception_details
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("JavaScript execution exception");
            let desc = exception_details
                .get("exception")
                .and_then(|e| e.get("description"))
                .and_then(|d| d.as_str())
                .unwrap_or(text);
            return Err(ChronosError::ExecutionFailed(desc.to_string()));
        }

        let value = res
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        Ok(value)
    }

    /// Forcibly purges JavaScript memory and triggers garbage collection to keep RAM minimal.
    pub async fn purge_memory(&self) -> Result<()> {
        let _ = self.call("HeapProfiler.collectGarbage", None).await;
        let _ = self.call("Memory.forciblyPurgeJavaScriptMemory", None).await;
        let _ = self
            .call(
                "Memory.simulatePressureNotification",
                Some(serde_json::json!({ "level": "critical" })),
            )
            .await;
        Ok(())
    }

    /// Dispatches a mouse click at the given viewport coordinates (x, y).
    pub async fn dispatch_mouse_click(&self, x: f64, y: f64) -> Result<()> {
        debug!("Dispatching mouse click at ({}, {})", x, y);

        // 1. Move mouse to target
        self.call(
            "Input.dispatchMouseEvent",
            Some(serde_json::json!({
                "type": "mouseMoved",
                "x": x,
                "y": y
            })),
        )
        .await?;

        // 2. Mouse pressed
        self.call(
            "Input.dispatchMouseEvent",
            Some(serde_json::json!({
                "type": "mousePressed",
                "x": x,
                "y": y,
                "button": "left",
                "clickCount": 1
            })),
        )
        .await?;

        tokio::time::sleep(Duration::from_millis(50)).await;

        // 3. Mouse released
        self.call(
            "Input.dispatchMouseEvent",
            Some(serde_json::json!({
                "type": "mouseReleased",
                "x": x,
                "y": y,
                "button": "left",
                "clickCount": 1
            })),
        )
        .await?;

        Ok(())
    }

    /// Dispatches keyboard input (special keys like Enter/Tab or regular text).
    pub async fn dispatch_key_input(&self, text_or_key: &str) -> Result<()> {
        debug!("Dispatching key input: {:?}", text_or_key);

        match text_or_key {
            "Enter" | "Return" => {
                self.call(
                    "Input.dispatchKeyEvent",
                    Some(serde_json::json!({
                        "type": "rawKeyDown",
                        "windowsVirtualKeyCode": 13,
                        "unmodifiedText": "\r",
                        "text": "\r",
                        "key": "Enter",
                        "code": "Enter"
                    })),
                )
                .await?;
                self.call(
                    "Input.dispatchKeyEvent",
                    Some(serde_json::json!({
                        "type": "keyUp",
                        "windowsVirtualKeyCode": 13,
                        "unmodifiedText": "\r",
                        "text": "\r",
                        "key": "Enter",
                        "code": "Enter"
                    })),
                )
                .await?;
            }
            "Backspace" => {
                self.call(
                    "Input.dispatchKeyEvent",
                    Some(serde_json::json!({
                        "type": "rawKeyDown",
                        "windowsVirtualKeyCode": 8,
                        "key": "Backspace",
                        "code": "Backspace"
                    })),
                )
                .await?;
                self.call(
                    "Input.dispatchKeyEvent",
                    Some(serde_json::json!({
                        "type": "keyUp",
                        "windowsVirtualKeyCode": 8,
                        "key": "Backspace",
                        "code": "Backspace"
                    })),
                )
                .await?;
            }
            "Tab" => {
                self.call(
                    "Input.dispatchKeyEvent",
                    Some(serde_json::json!({
                        "type": "rawKeyDown",
                        "windowsVirtualKeyCode": 9,
                        "key": "Tab",
                        "code": "Tab"
                    })),
                )
                .await?;
                self.call(
                    "Input.dispatchKeyEvent",
                    Some(serde_json::json!({
                        "type": "keyUp",
                        "windowsVirtualKeyCode": 9,
                        "key": "Tab",
                        "code": "Tab"
                    })),
                )
                .await?;
            }
            "Escape" => {
                self.call(
                    "Input.dispatchKeyEvent",
                    Some(serde_json::json!({
                        "type": "rawKeyDown",
                        "windowsVirtualKeyCode": 27,
                        "key": "Escape",
                        "code": "Escape"
                    })),
                )
                .await?;
                self.call(
                    "Input.dispatchKeyEvent",
                    Some(serde_json::json!({
                        "type": "keyUp",
                        "windowsVirtualKeyCode": 27,
                        "key": "Escape",
                        "code": "Escape"
                    })),
                )
                .await?;
            }
            "ArrowDown" => {
                self.call(
                    "Input.dispatchKeyEvent",
                    Some(serde_json::json!({
                        "type": "rawKeyDown",
                        "windowsVirtualKeyCode": 40,
                        "key": "ArrowDown",
                        "code": "ArrowDown"
                    })),
                )
                .await?;
                self.call(
                    "Input.dispatchKeyEvent",
                    Some(serde_json::json!({
                        "type": "keyUp",
                        "windowsVirtualKeyCode": 40,
                        "key": "ArrowDown",
                        "code": "ArrowDown"
                    })),
                )
                .await?;
            }
            "ArrowUp" => {
                self.call(
                    "Input.dispatchKeyEvent",
                    Some(serde_json::json!({
                        "type": "rawKeyDown",
                        "windowsVirtualKeyCode": 38,
                        "key": "ArrowUp",
                        "code": "ArrowUp"
                    })),
                )
                .await?;
                self.call(
                    "Input.dispatchKeyEvent",
                    Some(serde_json::json!({
                        "type": "keyUp",
                        "windowsVirtualKeyCode": 38,
                        "key": "ArrowUp",
                        "code": "ArrowUp"
                    })),
                )
                .await?;
            }
            text => {
                // High-performance text insertion (handles unicode, emoji, contenteditable)
                self.call(
                    "Input.insertText",
                    Some(serde_json::json!({
                        "text": text
                    })),
                )
                .await?;
            }
        }

        Ok(())
    }

    /// Scrolls the page viewport by the specified delta (x, y).
    pub async fn scroll(&self, x: f64, y: f64) -> Result<()> {
        debug!("Scrolling by delta ({}, {})", x, y);

        // 1. Dispatch mouseWheel input event
        let _ = self
            .call(
                "Input.dispatchMouseEvent",
                Some(serde_json::json!({
                    "type": "mouseWheel",
                    "x": 0.0,
                    "y": 0.0,
                    "deltaX": x,
                    "deltaY": y
                })),
            )
            .await;

        // 2. Also scroll window to ensure reliability across all page layouts
        let script = format!("window.scrollBy({}, {});", x, y);
        let _ = self.evaluate(&script).await;

        Ok(())
    }

    /// Captures a screenshot of the current page as PNG bytes.
    pub async fn screenshot(&self) -> Result<Vec<u8>> {
        let res = self
            .call(
                "Page.captureScreenshot",
                Some(serde_json::json!({ "format": "png" })),
            )
            .await?;

        let data_b64 = res
            .get("data")
            .and_then(|d| d.as_str())
            .ok_or_else(|| ChronosError::ExecutionFailed("No image data in screenshot response".into()))?;

        use base64::prelude::*;
        BASE64_STANDARD
            .decode(data_b64)
            .map_err(|e| ChronosError::Internal(format!("Failed to decode screenshot base64: {}", e)))
    }

    /// Returns the current page title.
    pub async fn title(&self) -> Result<String> {
        let val = self.evaluate("document.title").await?;
        Ok(val.as_str().unwrap_or_default().to_string())
    }

    /// Returns the current URL of the page.
    pub async fn url(&self) -> Result<String> {
        let val = self.evaluate("window.location.href").await?;
        Ok(val.as_str().unwrap_or_default().to_string())
    }

    /// Returns the full HTML content of the page.
    pub async fn content(&self) -> Result<String> {
        let val = self
            .evaluate("document.documentElement.outerHTML")
            .await?;
        Ok(val.as_str().unwrap_or_default().to_string())
    }

    /// Waits for an element matching a CSS selector to appear in the DOM.
    pub async fn wait_for_selector(&self, selector: &str, timeout_ms: u64) -> Result<()> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_millis(timeout_ms);
        let expr = format!("!!document.querySelector({:?})", selector);

        while start.elapsed() < timeout {
            if let Ok(res) = self.evaluate(&expr).await {
                if res.as_bool() == Some(true) {
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Err(ChronosError::ElementNotFound(format!(
            "Selector {:?} not found within {}ms",
            selector, timeout_ms
        )))
    }

    /// Closes this target tab.
    pub async fn close(&self) -> Result<()> {
        self.client
            .send_browser(
                "Target.closeTarget",
                Some(serde_json::json!({ "targetId": &self.target_id })),
            )
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cdp::launcher::{launch, LaunchOptions};

    #[tokio::test]
    async fn test_target_navigation_and_evaluation() {
        let mut proc = launch(LaunchOptions::default()).await.expect("Failed to launch Chrome");
        let client = CdpClient::connect(proc.ws_url()).await.expect("Failed to connect to CDP");
        let page = PageTarget::attach_or_create(client).await.expect("Failed to attach to page");

        let html = "data:text/html,<html><head><title>Chronos Page</title></head><body><div id='content'>Ready</div></body></html>";
        page.navigate(html).await.expect("Failed to navigate");

        let title = page.title().await.expect("Failed to get title");
        assert_eq!(title, "Chronos Page");

        let eval_res = page.evaluate("2 + 2").await.expect("Failed to evaluate");
        assert_eq!(eval_res, 4);

        let div_text = page.evaluate("document.getElementById('content').textContent").await.expect("Failed to get text");
        assert_eq!(div_text, "Ready");

        proc.shutdown().await.expect("Failed to shutdown Chrome");
    }
}

