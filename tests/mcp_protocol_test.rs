use chronos_browser::mcp::protocol::*;
use chronos_browser::mcp::server::McpServer;

#[tokio::test]
async fn test_mcp_initialize_handshake() {
    let mut server = McpServer::default();

    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-ai-agent",
                "version": "1.0.0"
            }
        }
    });

    let resp = server
        .handle_message_str(&init_req.to_string())
        .await
        .expect("Initialize should return a response");

    assert_eq!(resp.jsonrpc, "2.0");
    assert_eq!(resp.id, Some(RequestId::Int(1)));
    assert!(resp.error.is_none());

    let result = resp.result.expect("Response should contain result payload");
    let init_result: InitializeResult =
        serde_json::from_value(result).expect("Failed to deserialize InitializeResult");

    assert_eq!(init_result.protocol_version, "2024-11-05");
    assert_eq!(init_result.server_info.name, "chronos-browser");
    assert!(init_result.capabilities.tools.is_some());

    // Send notifications/initialized notification (must return None, no response)
    let notif = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    let notif_resp = server.handle_message_str(&notif.to_string()).await;
    assert!(
        notif_resp.is_none(),
        "Notifications must not return any JSON-RPC response"
    );
}

#[tokio::test]
async fn test_mcp_ping() {
    let mut server = McpServer::default();

    let ping_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "ping-99",
        "method": "ping"
    });

    let resp = server
        .handle_message_str(&ping_req.to_string())
        .await
        .expect("Ping should return a response");

    assert_eq!(resp.jsonrpc, "2.0");
    assert_eq!(resp.id, Some(RequestId::Str("ping-99".to_string())));
    assert!(resp.error.is_none());
    assert!(resp.result.is_some());
}

#[tokio::test]
async fn test_mcp_tools_list() {
    let mut server = McpServer::default();

    let list_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 42,
        "method": "tools/list"
    });

    let resp = server
        .handle_message_str(&list_req.to_string())
        .await
        .expect("tools/list should return a response");

    assert_eq!(resp.jsonrpc, "2.0");
    assert_eq!(resp.id, Some(RequestId::Int(42)));
    assert!(resp.error.is_none());

    let result = resp.result.expect("Must have result field");
    let list_result: ListToolsResult =
        serde_json::from_value(result).expect("Failed to deserialize ListToolsResult");

    let tool_names: Vec<String> = list_result.tools.iter().map(|t| t.name.clone()).collect();

    // Verify all 10 expected tools are defined
    let expected_tools = [
        "browser_navigate",
        "browser_snapshot",
        "browser_click",
        "browser_fill",
        "browser_toggle_checkbox",
        "browser_press_key",
        "browser_scroll",
        "browser_manage_session",
        "browser_evaluate_js",
        "browser_debug_screenshot",
    ];

    for expected in &expected_tools {
        assert!(
            tool_names.contains(&expected.to_string()),
            "Missing expected tool: {}",
            expected
        );
    }

    assert_eq!(list_result.tools.len(), 10);
}

#[tokio::test]
async fn test_tool_schema_definitions() {
    let tools = get_tool_definitions();
    assert_eq!(tools.len(), 10);

    // 1. browser_navigate schema
    let nav_tool = tools
        .iter()
        .find(|t| t.name == "browser_navigate")
        .expect("browser_navigate missing");
    let nav_schema = &nav_tool.input_schema;
    assert_eq!(nav_schema["type"], "object");
    assert!(nav_schema["properties"]["url"].is_object());
    assert_eq!(nav_schema["properties"]["url"]["type"], "string");
    assert_eq!(nav_schema["required"][0], "url");

    // 2. browser_click schema
    let click_tool = tools
        .iter()
        .find(|t| t.name == "browser_click")
        .expect("browser_click missing");
    let click_schema = &click_tool.input_schema;
    assert_eq!(click_schema["type"], "object");
    assert!(click_schema["properties"]["ref"].is_object());
    assert!(click_schema["properties"]["x"].is_object());
    assert!(click_schema["properties"]["y"].is_object());

    // 3. browser_fill schema
    let fill_tool = tools
        .iter()
        .find(|t| t.name == "browser_fill")
        .expect("browser_fill missing");
    let fill_schema = &fill_tool.input_schema;
    assert_eq!(fill_schema["type"], "object");
    assert!(fill_schema["properties"]["ref"].is_object());
    assert!(fill_schema["properties"]["value"].is_object());
    let reqs: Vec<&str> = fill_schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(reqs.contains(&"ref"));
    assert!(reqs.contains(&"value"));

    // 4. browser_toggle_checkbox schema
    let toggle_tool = tools
        .iter()
        .find(|t| t.name == "browser_toggle_checkbox")
        .expect("browser_toggle_checkbox missing");
    let toggle_schema = &toggle_tool.input_schema;
    assert_eq!(toggle_schema["type"], "object");
    assert!(toggle_schema["properties"]["ref"].is_object());

    // 5. browser_manage_session schema
    let session_tool = tools
        .iter()
        .find(|t| t.name == "browser_manage_session")
        .expect("browser_manage_session missing");
    let session_schema = &session_tool.input_schema;
    assert!(session_schema["properties"]["action"].is_object());
}

#[tokio::test]
async fn test_mcp_error_handling() {
    let mut server = McpServer::default();

    // 1. Parse error for invalid JSON
    let parse_err_resp = server
        .handle_message_str("{ invalid json")
        .await
        .expect("Must return error response");
    assert_eq!(
        parse_err_resp.error.as_ref().unwrap().code,
        JsonRpcError::PARSE_ERROR
    );

    // 2. Method not found
    let unknown_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 100,
        "method": "completely/unknown/method"
    });
    let unknown_resp = server
        .handle_message_str(&unknown_req.to_string())
        .await
        .expect("Must return method not found error");
    assert_eq!(
        unknown_resp.error.as_ref().unwrap().code,
        JsonRpcError::METHOD_NOT_FOUND
    );

    // 3. Invalid tool call params
    let invalid_params_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 101,
        "method": "tools/call",
        "params": "not an object"
    });
    let invalid_params_resp = server
        .handle_message_str(&invalid_params_req.to_string())
        .await
        .expect("Must return invalid params error");
    assert_eq!(
        invalid_params_resp.error.as_ref().unwrap().code,
        JsonRpcError::INVALID_PARAMS
    );
}

#[test]
fn test_call_tool_result_serialization() {
    // Text result
    let text_res = CallToolResult::text("Navigated successfully to https://example.com");
    assert!(!text_res.is_error);
    let json = serde_json::to_string(&text_res).unwrap();
    assert!(json.contains("Navigated successfully"));
    assert!(json.contains("\"type\":\"text\""));

    // Error result
    let err_res = CallToolResult::error("Element [99] not found");
    assert!(err_res.is_error);
    let json = serde_json::to_string(&err_res).unwrap();
    assert!(json.contains("\"isError\":true"));
    assert!(json.contains("Element [99] not found"));

    // Image result
    let img_res = CallToolResult::image("iVBORw0KGgoAAAANSUhEUg==", "image/png");
    assert!(!img_res.is_error);
    let json = serde_json::to_string(&img_res).unwrap();
    assert!(json.contains("\"type\":\"image\""));
    assert!(json.contains("\"mimeType\":\"image/png\""));
    assert!(json.contains("iVBORw0KGgoAAAANSUhEUg=="));
}
