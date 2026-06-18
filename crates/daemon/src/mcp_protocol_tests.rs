use super::{
    initialize_request, initialized_notification, parse_initialize, parse_tool_call,
    parse_tools_list, tool_call_request, tools_list_request, LATEST_PROTOCOL_VERSION,
};
use std::collections::BTreeMap;

#[test]
fn initialize_messages_follow_negotiated_lifecycle() {
    let request = initialize_request(1);
    let notification = initialized_notification();
    let response = br#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"fixture","version":"1.0.0"}}}"#;

    let identity = parse_initialize(response, 1).expect("parse initialize response");

    assert_eq!(
        request["params"]["protocolVersion"],
        LATEST_PROTOCOL_VERSION
    );
    assert_eq!(request["params"]["capabilities"], serde_json::json!({}));
    assert_eq!(notification["method"], "notifications/initialized");
    assert_eq!(identity.name, "fixture");
    assert_eq!(identity.protocol_version, "2025-11-25");
}

#[test]
fn tools_list_preserves_schema_and_pagination_cursor() {
    let request = tools_list_request(2, Some("next-page"));
    let response = br#"{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"echo","description":"Echo input","inputSchema":{"type":"object","properties":{"text":{"type":"string"}}}}],"nextCursor":"page-2"}}"#;

    let page = parse_tools_list(response, 2).expect("parse tools list");

    assert_eq!(request["params"]["cursor"], "next-page");
    assert_eq!(page.tools[0].name, "echo");
    assert!(page.tools[0].input_schema_json.contains("properties"));
    assert_eq!(page.next_cursor.as_deref(), Some("page-2"));
}

#[test]
fn tool_call_converts_json_arguments_and_text_result() {
    let arguments = BTreeMap::from([
        ("count".to_owned(), "2".to_owned()),
        ("text".to_owned(), "hello".to_owned()),
    ]);
    let request = tool_call_request(3, "echo", &arguments);
    let response = br#"{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"hello hello"}],"isError":false}}"#;

    let output = parse_tool_call(response, 3).expect("parse tool call");

    assert_eq!(request["params"]["arguments"]["count"], 2);
    assert_eq!(request["params"]["arguments"]["text"], "hello");
    assert_eq!(output, "hello hello");
}

#[test]
fn tool_call_surfaces_remote_errors() {
    let response = br#"{"jsonrpc":"2.0","id":4,"error":{"code":-32602,"message":"invalid text"}}"#;

    let error = parse_tool_call(response, 4).expect_err("surface MCP error");

    assert!(error.to_string().contains("-32602"));
    assert!(error.to_string().contains("invalid text"));
}
