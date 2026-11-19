use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 Request ID (can be integer, string, or null).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Int(i64),
    Str(String),
}

/// JSON-RPC 2.0 Request or Notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    #[serde(default = "default_jsonrpc_version")]
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<RequestId>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

fn default_jsonrpc_version() -> String {
    "2.0".to_string()
}

/// JSON-RPC 2.0 Error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;

    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(code: i64, message: impl Into<String>, data: Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }
}

/// JSON-RPC 2.0 Response object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<RequestId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<RequestId>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<RequestId>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// MCP Server Info definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// MCP Server Capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    #[serde(default)]
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolsCapability {
    #[serde(rename = "listChanged", default)]
    pub list_changed: bool,
}

/// MCP Initialize response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
}

/// Tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// MCP Tools List response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolsResult {
    pub tools: Vec<Tool>,
}

/// Tool Content item in tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    #[serde(rename = "resource")]
    Resource { resource: Value },
}

/// Result returned from `tools/call`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolResult {
    pub content: Vec<ToolContent>,
    #[serde(rename = "isError", default, skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
}

impl CallToolResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text { text: text.into() }],
            is_error: false,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text { text: message.into() }],
            is_error: true,
        }
    }

    pub fn image(base64_data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Image {
                data: base64_data.into(),
                mime_type: mime_type.into(),
            }],
            is_error: false,
        }
    }
}

/// Parameters for `tools/call`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Option<Value>,
}

/// Return the definitions of all available MCP tools in Chronos Browser.
pub fn get_tool_definitions() -> Vec<Tool> {
    vec![
        Tool {
            name: "browser_navigate".to_string(),
            description: "Navigate to a given URL in the browser and wait for page load.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The destination URL to navigate to (e.g. 'https://example.com')."
                    }
                },
                "required": ["url"]
            }),
        },
        Tool {
            name: "browser_snapshot".to_string(),
            description: "Get a clean, token-efficient semantic accessibility snapshot of the current page with numbered references [1], [2] for interactive elements, dialogs, inputs, and buttons.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "browser_click".to_string(),
            description: "Click an interactive element identified by its numeric reference [ref] from the snapshot, or direct (x, y) coordinates.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "ref": {
                        "type": "integer",
                        "description": "The numeric reference ID of the element from browser_snapshot."
                    },
                    "x": {
                        "type": "number",
                        "description": "Optional direct X coordinate to click."
                    },
                    "y": {
                        "type": "number",
                        "description": "Optional direct Y coordinate to click."
                    }
                }
            }),
        },
        Tool {
            name: "browser_fill".to_string(),
            description: "Type text into an input, textarea, or contenteditable element identified by its numeric reference [ref].".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "ref": {
                        "type": "integer",
                        "description": "The numeric reference ID of the element from browser_snapshot."
                    },
                    "value": {
                        "type": "string",
                        "description": "The text value to input."
                    },
                    "clear": {
                        "type": "boolean",
                        "description": "Whether to clear existing text before typing (default: true)."
                    }
                },
                "required": ["ref", "value"]
            }),
        },
        Tool {
            name: "browser_toggle_checkbox".to_string(),
            description: "Toggle or set the checked state of a checkbox or radio button identified by [ref].".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "ref": {
                        "type": "integer",
                        "description": "The numeric reference ID of the checkbox/radio from browser_snapshot."
                    },
                    "checked": {
                        "type": "boolean",
                        "description": "Optional desired checked state. If omitted, toggles current state."
                    }
                },
                "required": ["ref"]
            }),
        },
        Tool {
            name: "browser_press_key".to_string(),
            description: "Press a key or keyboard shortcut (e.g. 'Enter', 'Escape', 'Tab', 'Backspace', 'ArrowDown').".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "Key name such as 'Enter', 'Tab', 'Escape', 'Backspace', 'ArrowDown'."
                    }
                },
                "required": ["key"]
            }),
        },
        Tool {
            name: "browser_scroll".to_string(),
            description: "Scroll the page vertically or horizontally, or to top/bottom.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "direction": {
                        "type": "string",
                        "enum": ["up", "down", "top", "bottom"],
                        "description": "Quick scroll direction (e.g. 'down', 'up', 'top', 'bottom')."
                    },
                    "delta_x": {
                        "type": "number",
                        "description": "Horizontal scroll delta in pixels."
                    },
                    "delta_y": {
                        "type": "number",
                        "description": "Vertical scroll delta in pixels (e.g. 500 to scroll down)."
                    }
                }
            }),
        },
        Tool {
            name: "browser_manage_session".to_string(),
            description: "Manage persistent browser profiles and headless vs headful mode.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["list_profiles", "switch_profile", "set_headful", "get_status", "restart", "sleep", "hibernate", "purge_memory"],
                        "description": "Action to perform."
                    },
                    "profile": {
                        "type": "string",
                        "description": "Profile name for 'switch_profile'."
                    },
                    "headful": {
                        "type": "boolean",
                        "description": "Set headful mode to true (visible window) or false (headless)."
                    }
                },
                "required": ["action"]
            }),
        },
        Tool {
            name: "browser_evaluate_js".to_string(),
            description: "Evaluate raw JavaScript expression in the context of the active browser page.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "script": {
                        "type": "string",
                        "description": "JavaScript code or expression to evaluate."
                    }
                },
                "required": ["script"]
            }),
        },
        Tool {
            name: "browser_debug_screenshot".to_string(),
            description: "Take a visual screenshot of the current page and return it as an image for visual inspection.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "full_page": {
                        "type": "boolean",
                        "description": "Whether to capture the entire scrollable page (default: false)."
                    }
                }
            }),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definitions() {
        let tools = get_tool_definitions();
        assert_eq!(tools.len(), 10);
        let names: Vec<_> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"browser_navigate"));
        assert!(names.contains(&"browser_snapshot"));
        assert!(names.contains(&"browser_click"));
        assert!(names.contains(&"browser_fill"));
        assert!(names.contains(&"browser_toggle_checkbox"));
        assert!(names.contains(&"browser_press_key"));
        assert!(names.contains(&"browser_scroll"));
        assert!(names.contains(&"browser_manage_session"));
        assert!(names.contains(&"browser_evaluate_js"));
        assert!(names.contains(&"browser_debug_screenshot"));
    }

    #[test]
    fn test_jsonrpc_request_response_serialization() {
        let req_json = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let req: JsonRpcRequest = serde_json::from_str(req_json).unwrap();
        assert_eq!(req.id, Some(RequestId::Int(1)));
        assert_eq!(req.method, "tools/list");

        let resp = JsonRpcResponse::success(req.id, serde_json::json!({"tools": []}));
        let resp_json = serde_json::to_string(&resp).unwrap();
        assert!(resp_json.contains(r#""jsonrpc":"2.0""#));
        assert!(resp_json.contains(r#""tools":[]"#));
    }
}
