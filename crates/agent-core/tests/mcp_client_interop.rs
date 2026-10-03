use qmoosa_agent_core::PolkadotAgentKit;
use serde_json::json;

#[test]
fn test_mcp_client_handshake_initialize() {
    let kit = PolkadotAgentKit::new();

    let client_request = json!({
        "jsonrpc": "2.0",
        "id": "req-1",
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "external-mcp-client",
                "version": "1.0.0"
            }
        }
    });

    let response = kit.handle_mcp_request(&client_request);

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], "req-1");
    assert_eq!(response["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(
        response["result"]["serverInfo"]["name"],
        "qmoosa-polkadot-agent-kit"
    );
    assert!(response["result"]["capabilities"]["tools"].is_object());
}

#[test]
fn test_mcp_client_tools_list_schema_conformance() {
    let kit = PolkadotAgentKit::new();

    let client_request = json!({
        "jsonrpc": "2.0",
        "id": "req-2",
        "method": "tools/list"
    });

    let response = kit.handle_mcp_request(&client_request);
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], "req-2");

    let tools = response["result"]["tools"].as_array().expect("tools array");
    assert!(!tools.is_empty(), "Tool list should not be empty");

    for tool in tools {
        assert!(tool["name"].is_string(), "Tool must have name");
        assert!(
            tool["description"].is_string(),
            "Tool must have description"
        );
        let schema = &tool["inputSchema"];
        assert_eq!(
            schema["type"], "object",
            "Tool inputSchema type must be object"
        );
        assert!(
            schema["properties"].is_object(),
            "Tool inputSchema must define properties"
        );
    }
}

#[test]
fn test_mcp_client_tool_call_native_transfer() {
    let kit = PolkadotAgentKit::new();

    let client_request = json!({
        "jsonrpc": "2.0",
        "id": "req-3",
        "method": "tools/call",
        "params": {
            "name": "polkadot_transfer_native",
            "arguments": {
                "recipient": "5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd",
                "amount_plancks": "500000000000"
            }
        }
    });

    let response = kit.handle_mcp_request(&client_request);
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], "req-3");

    let content_text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    let parsed: serde_json::Value = serde_json::from_str(content_text).expect("valid json payload");

    assert_eq!(parsed["action"], "Balances.transfer_keep_alive");
    assert_eq!(
        parsed["recipient"],
        "5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd"
    );
    assert_eq!(parsed["amount_plancks"], "500000000000");
    assert_eq!(parsed["requires_wallet_signature"], true);
}

#[test]
fn test_mcp_client_tool_call_asset_transfer() {
    let kit = PolkadotAgentKit::new();

    let client_request = json!({
        "jsonrpc": "2.0",
        "id": "req-4",
        "method": "tools/call",
        "params": {
            "name": "polkadot_transfer_asset",
            "arguments": {
                "asset_id": 1984,
                "recipient": "5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd",
                "amount": "1000000000000000000"
            }
        }
    });

    let response = kit.handle_mcp_request(&client_request);
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], "req-4");

    let content_text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    let parsed: serde_json::Value = serde_json::from_str(content_text).expect("valid json payload");

    assert_eq!(parsed["action"], "Assets.transfer_keep_alive");
    assert_eq!(parsed["asset_id"], 1984);
}

#[test]
fn test_mcp_client_tool_call_xcm_transfer() {
    let kit = PolkadotAgentKit::new();

    let client_request = json!({
        "jsonrpc": "2.0",
        "id": "req-5",
        "method": "tools/call",
        "params": {
            "name": "polkadot_xcm_transfer",
            "arguments": {
                "destination_para_id": 2000,
                "recipient": "5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd",
                "amount_plancks": "10000000000"
            }
        }
    });

    let response = kit.handle_mcp_request(&client_request);
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], "req-5");

    let content_text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    let parsed: serde_json::Value = serde_json::from_str(content_text).expect("valid json payload");

    assert_eq!(parsed["action"], "PolkadotXcm.limited_teleport_assets");
    assert_eq!(parsed["destination_para_id"], 2000);
}

#[test]
fn test_mcp_client_error_handling() {
    let kit = PolkadotAgentKit::new();

    // 1. Unknown method (-32601)
    let bad_method_req = json!({
        "jsonrpc": "2.0",
        "id": "err-1",
        "method": "resources/list"
    });
    let bad_method_res = kit.handle_mcp_request(&bad_method_req);
    assert_eq!(bad_method_res["error"]["code"], -32601);

    // 2. Missing method (-32600)
    let no_method_req = json!({
        "jsonrpc": "2.0",
        "id": "err-2"
    });
    let no_method_res = kit.handle_mcp_request(&no_method_req);
    assert_eq!(no_method_res["error"]["code"], -32600);

    // 3. Unknown tool (-32000)
    let unknown_tool_req = json!({
        "jsonrpc": "2.0",
        "id": "err-3",
        "method": "tools/call",
        "params": {
            "name": "non_existent_tool",
            "arguments": {}
        }
    });
    let unknown_tool_res = kit.handle_mcp_request(&unknown_tool_req);
    assert_eq!(unknown_tool_res["error"]["code"], -32000);
    assert!(unknown_tool_res["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Tool not found"));
}
