use crate::cdp::launcher::{self, ChromeProcess, LaunchOptions};
use crate::cdp::target::PageTarget;
use crate::cdp::CdpClient;
use crate::error::Result;
use crate::mcp::protocol::*;
use crate::semantic::formatter::format_snapshot;
use crate::semantic::registry::ElementRegistry;
use crate::semantic::{DETECTOR_JS, DOM_SETTLE_JS};
use crate::session::SessionManager;
use base64::prelude::*;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::debug;

/// High-performance Model Context Protocol (MCP) Server for Chronos Browser.
pub struct McpServer {
    session_manager: SessionManager,
    chrome_process: Option<ChromeProcess>,
    client: Option<Arc<CdpClient>>,
    target: Option<PageTarget>,
    current_registry: Option<ElementRegistry>,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new(SessionManager::new())
    }
}

impl McpServer {
    /// Creates a new McpServer with the given session manager.
    pub fn new(session_manager: SessionManager) -> Self {
        Self {
            session_manager,
            chrome_process: None,
            client: None,
            target: None,
            current_registry: None,
        }
    }

    /// Access the session manager.
    pub fn session_manager(&self) -> &SessionManager {
        &self.session_manager
    }

    /// Access mutable session manager.
    pub fn session_manager_mut(&mut self) -> &mut SessionManager {
        &mut self.session_manager
    }

    /// Ensures that Chrome is launched and attached to an active PageTarget.
    pub async fn ensure_browser(&mut self) -> Result<&PageTarget> {
        if self.target.is_some() {
            return Ok(self.target.as_ref().unwrap());
        }

        let profile_dir = self
            .session_manager
            .ensure_profile_dir(self.session_manager.active_profile())?;

        let options = LaunchOptions::new()
            .headless(!self.session_manager.is_headful())
            .user_data_dir(profile_dir);

        eprintln!(
            "[chronos] Launching browser (profile: '{}', headful: {})...",
            self.session_manager.active_profile(),
            self.session_manager.is_headful()
        );

        let process = launcher::launch(options).await?;
        let client = CdpClient::connect(process.ws_url()).await?;
        let target = PageTarget::attach_or_create(client.clone()).await?;

        self.chrome_process = Some(process);
        self.client = Some(client);
        self.target = Some(target);
        self.current_registry = None;

        Ok(self.target.as_ref().unwrap())
    }

    /// Closes the current browser process and clears CDP client and target state.
    pub async fn close_browser(&mut self) -> Result<()> {
        self.target = None;
        self.client = None;
        if let Some(mut proc) = self.chrome_process.take() {
            let _ = proc.shutdown().await;
        }
        self.current_registry = None;
        Ok(())
    }

    /// Runs the MCP stdio loop, reading requests line-by-line from stdin
    /// and writing JSON-RPC responses to stdout.
    pub async fn run_stdio(&mut self) -> Result<()> {
        eprintln!("[chronos] Chronos Browser MCP Server running on stdio");
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();
        let mut stdout = tokio::io::stdout();

        while let Some(line) = reader.next_line().await? {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let response = self.handle_message_str(line).await;

            if let Some(resp) = response {
                let resp_json = serde_json::to_string(&resp)?;
                stdout.write_all(resp_json.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
        }

        // Clean up browser on stdin EOF
        let _ = self.close_browser().await;
        Ok(())
    }

    /// Processes an incoming raw JSON string and produces an optional JSON-RPC response.
    pub async fn handle_message_str(&mut self, message_str: &str) -> Option<JsonRpcResponse> {
        let request: JsonRpcRequest = match serde_json::from_str(message_str) {
            Ok(req) => req,
            Err(e) => {
                return Some(JsonRpcResponse::error(
                    None,
                    JsonRpcError::new(JsonRpcError::PARSE_ERROR, format!("Parse error: {}", e)),
                ));
            }
        };

        self.handle_request(request).await
    }

    /// Handles a parsed JSON-RPC request.
    pub async fn handle_request(&mut self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let id = req.id.clone();
        let is_notification = id.is_none();

        match req.method.as_str() {
            // MCP Handshake: initialize
            "initialize" => {
                let init_result = InitializeResult {
                    protocol_version: "2024-11-05".to_string(),
                    capabilities: ServerCapabilities {
                        tools: Some(ToolsCapability { list_changed: false }),
                    },
                    server_info: ServerInfo {
                        name: "chronos-browser".to_string(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                    },
                };
                Some(JsonRpcResponse::success(
                    id,
                    serde_json::to_value(init_result).unwrap(),
                ))
            }

            // Notification: initialized
            "notifications/initialized" => {
                debug!("Client sent notifications/initialized");
                None
            }

            // MCP Ping
            "ping" => Some(JsonRpcResponse::success(id, serde_json::json!({}))),

            // MCP Tools: list
            "tools/list" => {
                let tools = get_tool_definitions();
                let list_result = ListToolsResult { tools };
                Some(JsonRpcResponse::success(
                    id,
                    serde_json::to_value(list_result).unwrap(),
                ))
            }

            // MCP Tools: call
            "tools/call" => {
                let params = match req.params {
                    Some(p) => match serde_json::from_value::<CallToolParams>(p) {
                        Ok(p) => p,
                        Err(e) => {
                            return Some(JsonRpcResponse::error(
                                id,
                                JsonRpcError::new(
                                    JsonRpcError::INVALID_PARAMS,
                                    format!("Invalid tool call params: {}", e),
                                ),
                            ));
                        }
                    },
                    None => {
                        return Some(JsonRpcResponse::error(
                            id,
                            JsonRpcError::new(
                                JsonRpcError::INVALID_PARAMS,
                                "Missing params for tools/call",
                            ),
                        ));
                    }
                };

                let tool_result = self
                    .dispatch_tool(&params.name, params.arguments.unwrap_or_default())
                    .await;
                Some(JsonRpcResponse::success(
                    id,
                    serde_json::to_value(tool_result).unwrap(),
                ))
            }

            // Unknown method
            _ => {
                if is_notification {
                    None
                } else {
                    Some(JsonRpcResponse::error(
                        id,
                        JsonRpcError::new(
                            JsonRpcError::METHOD_NOT_FOUND,
                            format!("Method not found: {}", req.method),
                        ),
                    ))
                }
            }
        }
    }

    /// Dispatches a tool execution by name with its arguments.
    pub async fn dispatch_tool(
        &mut self,
        tool_name: &str,
        args: serde_json::Value,
    ) -> CallToolResult {
        match tool_name {
            "browser_navigate" => self.tool_navigate(args).await,
            "browser_snapshot" => self.tool_snapshot(args).await,
            "browser_click" => self.tool_click(args).await,
            "browser_fill" => self.tool_fill(args).await,
            "browser_toggle_checkbox" => self.tool_toggle_checkbox(args).await,
            "browser_press_key" => self.tool_press_key(args).await,
            "browser_scroll" => self.tool_scroll(args).await,
            "browser_manage_session" => self.tool_manage_session(args).await,
            "browser_evaluate_js" => self.tool_evaluate_js(args).await,
            "browser_debug_screenshot" => self.tool_debug_screenshot(args).await,
            unknown => CallToolResult::error(format!("Unknown tool: {}", unknown)),
        }
    }

    // --- Tool Handlers ---

    async fn tool_navigate(&mut self, args: serde_json::Value) -> CallToolResult {
        let url = match args.get("url").and_then(|u| u.as_str()) {
            Some(u) => u,
            None => return CallToolResult::error("Missing required parameter 'url'"),
        };

        if let Err(e) = self.ensure_browser().await {
            return CallToolResult::error(format!("Failed to initialize browser: {}", e));
        }

        let target = self.target.as_ref().unwrap();
        if let Err(e) = target.navigate(url).await {
            return CallToolResult::error(format!("Failed to navigate to {}: {}", url, e));
        }

        // Micro-settle wait
        let _ = target.evaluate(DOM_SETTLE_JS).await;

        // Take snapshot after navigation
        self.take_snapshot().await
    }

    async fn tool_snapshot(&mut self, _args: serde_json::Value) -> CallToolResult {
        if let Err(e) = self.ensure_browser().await {
            return CallToolResult::error(format!("Failed to initialize browser: {}", e));
        }

        self.take_snapshot().await
    }

    async fn tool_click(&mut self, args: serde_json::Value) -> CallToolResult {
        if let Err(e) = self.ensure_browser().await {
            return CallToolResult::error(format!("Failed to initialize browser: {}", e));
        }

        let (x, y) = if let Some(ref_id) = args.get("ref").and_then(|r| r.as_u64()) {
            let ref_id = ref_id as u32;
            if self.current_registry.is_none() {
                let _ = self.take_snapshot().await;
            }

            match self
                .current_registry
                .as_ref()
                .and_then(|r| r.get(ref_id))
            {
                Some(el) => el.center,
                None => {
                    return CallToolResult::error(format!(
                        "Element [{}] not found. Take a new snapshot.",
                        ref_id
                    ));
                }
            }
        } else if let (Some(x), Some(y)) = (
            args.get("x").and_then(|v| v.as_f64()),
            args.get("y").and_then(|v| v.as_f64()),
        ) {
            (x, y)
        } else {
            return CallToolResult::error("Provide either 'ref' or both 'x' and 'y' coordinates.");
        };

        let target = self.target.as_ref().unwrap();
        if let Err(e) = target.dispatch_mouse_click(x, y).await {
            return CallToolResult::error(format!("Click failed: {}", e));
        }

        let _ = target.evaluate(DOM_SETTLE_JS).await;
        self.take_snapshot().await
    }

    async fn tool_fill(&mut self, args: serde_json::Value) -> CallToolResult {
        if let Err(e) = self.ensure_browser().await {
            return CallToolResult::error(format!("Failed to initialize browser: {}", e));
        }

        let ref_id = match args.get("ref").and_then(|r| r.as_u64()) {
            Some(r) => r as u32,
            None => return CallToolResult::error("Missing required parameter 'ref'"),
        };

        let value = match args.get("value").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return CallToolResult::error("Missing required parameter 'value'"),
        };

        let clear = args
            .get("clear")
            .and_then(|c| c.as_bool())
            .unwrap_or(true);

        if self.current_registry.is_none() {
            let _ = self.take_snapshot().await;
        }

        let center = match self
            .current_registry
            .as_ref()
            .and_then(|r| r.get(ref_id))
        {
            Some(el) => el.center,
            None => {
                return CallToolResult::error(format!(
                    "Element [{}] not found. Take a new snapshot.",
                    ref_id
                ));
            }
        };

        let target = self.target.as_ref().unwrap();
        // Focus the element by clicking its center
        if let Err(e) = target.dispatch_mouse_click(center.0, center.1).await {
            return CallToolResult::error(format!("Failed to focus element: {}", e));
        }

        if clear {
            // Clear content via JS evaluation on document.activeElement
            let clear_script = r#"
                (() => {
                    const el = document.activeElement;
                    if (!el) return;
                    if ('value' in el) {
                        el.value = '';
                        el.dispatchEvent(new Event('input', { bubbles: true }));
                        el.dispatchEvent(new Event('change', { bubbles: true }));
                    } else if (el.isContentEditable) {
                        el.textContent = '';
                        el.dispatchEvent(new Event('input', { bubbles: true }));
                    }
                })()
            "#;
            let _ = target.evaluate(clear_script).await;
        }

        // Insert text
        if let Err(e) = target.dispatch_key_input(value).await {
            return CallToolResult::error(format!("Failed to type text: {}", e));
        }

        let _ = target.evaluate(DOM_SETTLE_JS).await;
        self.take_snapshot().await
    }

    async fn tool_toggle_checkbox(&mut self, args: serde_json::Value) -> CallToolResult {
        if let Err(e) = self.ensure_browser().await {
            return CallToolResult::error(format!("Failed to initialize browser: {}", e));
        }

        let ref_id = match args.get("ref").and_then(|r| r.as_u64()) {
            Some(r) => r as u32,
            None => return CallToolResult::error("Missing required parameter 'ref'"),
        };

        let desired_checked = args.get("checked").and_then(|c| c.as_bool());

        if self.current_registry.is_none() {
            let _ = self.take_snapshot().await;
        }

        let (center, current_state) = match self
            .current_registry
            .as_ref()
            .and_then(|r| r.get(ref_id))
        {
            Some(el) => (el.center, el.checked.unwrap_or(false)),
            None => {
                return CallToolResult::error(format!(
                    "Element [{}] not found. Take a new snapshot.",
                    ref_id
                ));
            }
        };

        // If desired_checked is already matching, no need to click
        if let Some(target_state) = desired_checked {
            if target_state == current_state {
                return CallToolResult::text(format!(
                    "Checkbox [{}] is already checked={}",
                    ref_id, target_state
                ));
            }
        }

        let target = self.target.as_ref().unwrap();
        if let Err(e) = target.dispatch_mouse_click(center.0, center.1).await {
            return CallToolResult::error(format!("Failed to toggle checkbox: {}", e));
        }

        let _ = target.evaluate(DOM_SETTLE_JS).await;
        self.take_snapshot().await
    }

    async fn tool_press_key(&mut self, args: serde_json::Value) -> CallToolResult {
        if let Err(e) = self.ensure_browser().await {
            return CallToolResult::error(format!("Failed to initialize browser: {}", e));
        }

        let key = match args.get("key").and_then(|k| k.as_str()) {
            Some(k) => k,
            None => return CallToolResult::error("Missing required parameter 'key'"),
        };

        let target = self.target.as_ref().unwrap();
        if let Err(e) = target.dispatch_key_input(key).await {
            return CallToolResult::error(format!("Failed to press key '{}': {}", key, e));
        }

        let _ = target.evaluate(DOM_SETTLE_JS).await;
        self.take_snapshot().await
    }

    async fn tool_scroll(&mut self, args: serde_json::Value) -> CallToolResult {
        if let Err(e) = self.ensure_browser().await {
            return CallToolResult::error(format!("Failed to initialize browser: {}", e));
        }

        let target = self.target.as_ref().unwrap();

        if let Some(direction) = args.get("direction").and_then(|d| d.as_str()) {
            match direction {
                "down" => {
                    let _ = target.scroll(0.0, 500.0).await;
                }
                "up" => {
                    let _ = target.scroll(0.0, -500.0).await;
                }
                "top" => {
                    let _ = target.evaluate("window.scrollTo(0, 0)").await;
                }
                "bottom" => {
                    let _ = target
                        .evaluate("window.scrollTo(0, document.body.scrollHeight)")
                        .await;
                }
                other => {
                    return CallToolResult::error(format!(
                        "Invalid direction: '{}'. Use 'up', 'down', 'top', or 'bottom'.",
                        other
                    ));
                }
            }
        } else {
            let delta_x = args.get("delta_x").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let delta_y = args.get("delta_y").and_then(|v| v.as_f64()).unwrap_or(500.0);
            if let Err(e) = target.scroll(delta_x, delta_y).await {
                return CallToolResult::error(format!("Scroll failed: {}", e));
            }
        }

        let _ = target.evaluate(DOM_SETTLE_JS).await;
        self.take_snapshot().await
    }

    async fn tool_manage_session(&mut self, args: serde_json::Value) -> CallToolResult {
        let action = match args.get("action").and_then(|a| a.as_str()) {
            Some(a) => a,
            None => return CallToolResult::error("Missing required parameter 'action'"),
        };

        match action {
            "list_profiles" => match self.session_manager.list_profiles() {
                Ok(profiles) => CallToolResult::text(format!(
                    "Active profile: '{}'\nAvailable profiles: {:?}",
                    self.session_manager.active_profile(),
                    profiles
                )),
                Err(e) => CallToolResult::error(format!("Failed to list profiles: {}", e)),
            },

            "switch_profile" => {
                let profile = match args.get("profile").and_then(|p| p.as_str()) {
                    Some(p) => p,
                    None => {
                        return CallToolResult::error(
                            "Missing 'profile' parameter for switch_profile",
                        )
                    }
                };

                let _ = self.close_browser().await;
                self.session_manager.set_active_profile(profile);

                CallToolResult::text(format!("Switched to profile '{}'", profile))
            }

            "set_headful" => {
                let headful = match args.get("headful").and_then(|h| h.as_bool()) {
                    Some(h) => h,
                    None => {
                        return CallToolResult::error("Missing 'headful' parameter for set_headful")
                    }
                };

                self.session_manager.set_headful(headful);
                let _ = self.close_browser().await;

                CallToolResult::text(format!(
                    "Headful mode set to: {}. Browser restarted on next operation.",
                    headful
                ))
            }

            "get_status" => {
                let status = serde_json::json!({
                    "active_profile": self.session_manager.active_profile(),
                    "headful": self.session_manager.is_headful(),
                    "browser_running": self.target.is_some(),
                });
                CallToolResult::text(serde_json::to_string_pretty(&status).unwrap())
            }

            "sleep" | "hibernate" => {
                let _ = self.close_browser().await;
                CallToolResult::text("Browser hibernated. Chrome process shut down to 0MB RAM. Will wake automatically on next tool call.")
            }

            "purge_memory" => {
                if let Some(ref target) = self.target {
                    let _ = target.purge_memory().await;
                    CallToolResult::text("Purged JavaScript heap and triggered V8 garbage collection.")
                } else {
                    CallToolResult::text("Browser is not currently running.")
                }
            }

            "restart" => {
                let _ = self.close_browser().await;
                if let Err(e) = self.ensure_browser().await {
                    return CallToolResult::error(format!("Restart failed: {}", e));
                }
                CallToolResult::text("Browser restarted successfully.")
            }

            other => CallToolResult::error(format!("Unknown session action: {}", other)),
        }
    }

    async fn tool_evaluate_js(&mut self, args: serde_json::Value) -> CallToolResult {
        if let Err(e) = self.ensure_browser().await {
            return CallToolResult::error(format!("Failed to initialize browser: {}", e));
        }

        let script = match args.get("script").and_then(|s| s.as_str()) {
            Some(s) => s,
            None => return CallToolResult::error("Missing required parameter 'script'"),
        };

        let target = self.target.as_ref().unwrap();
        match target.evaluate(script).await {
            Ok(val) => {
                let text = match val {
                    serde_json::Value::String(s) => s,
                    other => serde_json::to_string_pretty(&other).unwrap_or_default(),
                };
                CallToolResult::text(text)
            }
            Err(e) => CallToolResult::error(format!("JavaScript evaluation error: {}", e)),
        }
    }

    async fn tool_debug_screenshot(&mut self, _args: serde_json::Value) -> CallToolResult {
        if let Err(e) = self.ensure_browser().await {
            return CallToolResult::error(format!("Failed to initialize browser: {}", e));
        }

        let target = self.target.as_ref().unwrap();
        match target.screenshot().await {
            Ok(bytes) => {
                let b64 = BASE64_STANDARD.encode(bytes);
                CallToolResult::image(b64, "image/png")
            }
            Err(e) => CallToolResult::error(format!("Failed to capture screenshot: {}", e)),
        }
    }

    /// Evaluates `DETECTOR_JS` in the active page target and formats the semantic snapshot.
    async fn take_snapshot(&mut self) -> CallToolResult {
        let target = match self.target.as_ref() {
            Some(t) => t,
            None => return CallToolResult::error("Browser target not initialized"),
        };

        let eval_res = match target.evaluate(DETECTOR_JS).await {
            Ok(res) => res,
            Err(e) => {
                return CallToolResult::error(format!("Failed to evaluate detector script: {}", e));
            }
        };

        match ElementRegistry::from_value(eval_res) {
            Ok(registry) => {
                let text = format_snapshot(&registry);
                self.current_registry = Some(registry);
                // Aggressively purge memory and collect garbage after taking snapshot
                let _ = target.purge_memory().await;
                CallToolResult::text(text)
            }
            Err(e) => CallToolResult::error(format!(
                "Failed to parse semantic snapshot from page: {}",
                e
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_mcp_initialize() {
        let tmp = tempdir().unwrap();
        let mgr = SessionManager::with_profiles_dir(tmp.path().join("profiles"));
        let mut server = McpServer::new(mgr);

        let req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let resp = server.handle_message_str(req).await.expect("Expected response");
        assert_eq!(resp.id, Some(RequestId::Int(1)));
        let result = resp.result.expect("Expected result");
        assert_eq!(result["serverInfo"]["name"], "chronos-browser");
        assert_eq!(result["protocolVersion"], "2024-11-05");
    }

    #[tokio::test]
    async fn test_mcp_tools_list() {
        let tmp = tempdir().unwrap();
        let mgr = SessionManager::with_profiles_dir(tmp.path().join("profiles"));
        let mut server = McpServer::new(mgr);

        let req = r#"{"jsonrpc":"2.0","id":"req-2","method":"tools/list"}"#;
        let resp = server.handle_message_str(req).await.expect("Expected response");
        assert_eq!(resp.id, Some(RequestId::Str("req-2".to_string())));
        let result = resp.result.expect("Expected result");
        let tools = result["tools"].as_array().expect("Expected tools array");
        assert_eq!(tools.len(), 10);
    }

    #[tokio::test]
    async fn test_mcp_ping() {
        let tmp = tempdir().unwrap();
        let mgr = SessionManager::with_profiles_dir(tmp.path().join("profiles"));
        let mut server = McpServer::new(mgr);

        let req = r#"{"jsonrpc":"2.0","id":99,"method":"ping"}"#;
        let resp = server.handle_message_str(req).await.expect("Expected response");
        assert_eq!(resp.id, Some(RequestId::Int(99)));
        assert_eq!(resp.result, Some(serde_json::json!({})));
    }

    #[tokio::test]
    async fn test_mcp_manage_session_tool() {
        let tmp = tempdir().unwrap();
        let mgr = SessionManager::with_profiles_dir(tmp.path().join("profiles"));
        let mut server = McpServer::new(mgr);

        // List profiles
        let res = server
            .dispatch_tool("browser_manage_session", serde_json::json!({ "action": "list_profiles" }))
            .await;
        assert!(!res.is_error);

        // Switch profile
        let res = server
            .dispatch_tool(
                "browser_manage_session",
                serde_json::json!({ "action": "switch_profile", "profile": "work" }),
            )
            .await;
        assert!(!res.is_error);
        assert_eq!(server.session_manager().active_profile(), "work");

        // Set headful
        let res = server
            .dispatch_tool(
                "browser_manage_session",
                serde_json::json!({ "action": "set_headful", "headful": true }),
            )
            .await;
        assert!(!res.is_error);
        assert!(server.session_manager().is_headful());
    }

    #[tokio::test]
    async fn test_mcp_method_not_found() {
        let tmp = tempdir().unwrap();
        let mgr = SessionManager::with_profiles_dir(tmp.path().join("profiles"));
        let mut server = McpServer::new(mgr);

        let req = r#"{"jsonrpc":"2.0","id":42,"method":"non_existent"}"#;
        let resp = server.handle_message_str(req).await.expect("Expected response");
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, JsonRpcError::METHOD_NOT_FOUND);
    }
}

