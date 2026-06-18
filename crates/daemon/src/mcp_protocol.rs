use crate::mcp_registry::DiscoveredTool;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-03-26", "2024-11-05"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerIdentity {
    pub name: String,
    pub version: String,
    pub protocol_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolPage {
    pub tools: Vec<DiscoveredTool>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("invalid MCP JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("unexpected MCP JSON-RPC version or response id")]
    UnexpectedResponse,
    #[error("MCP remote error {code}: {message}")]
    Remote { code: i64, message: String },
    #[error("MCP server negotiated unsupported protocol version '{0}'")]
    UnsupportedVersion(String),
    #[error("MCP tool returned an error: {0}")]
    Tool(String),
}

#[derive(Deserialize)]
struct RpcEnvelope<T> {
    jsonrpc: String,
    id: u64,
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeResult {
    protocol_version: String,
    server_info: ImplementationInfo,
}

#[derive(Deserialize)]
struct ImplementationInfo {
    name: String,
    version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolsListResult {
    tools: Vec<RemoteTool>,
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteTool {
    name: String,
    #[serde(default)]
    description: String,
    input_schema: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolCallResult {
    #[serde(default)]
    content: Vec<ContentBlock>,
    structured_content: Option<Value>,
    #[serde(default)]
    is_error: bool,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

pub fn initialize_request(id: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": LATEST_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": "steward",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    })
}

pub fn initialized_notification() -> Value {
    json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
}

pub fn tools_list_request(id: u64, cursor: Option<&str>) -> Value {
    let params = cursor.map_or_else(|| json!({}), |cursor| json!({"cursor": cursor}));
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/list", "params": params})
}

pub fn tool_call_request(id: u64, name: &str, arguments: &BTreeMap<String, String>) -> Value {
    let arguments = arguments
        .iter()
        .map(|(key, value)| {
            let parsed =
                serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.clone()));
            (key.clone(), parsed)
        })
        .collect::<Map<_, _>>();
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {"name": name, "arguments": arguments}
    })
}

pub fn parse_initialize(bytes: &[u8], expected_id: u64) -> Result<ServerIdentity, ProtocolError> {
    let result: InitializeResult = parse_response(bytes, expected_id)?;
    if !SUPPORTED_PROTOCOL_VERSIONS.contains(&result.protocol_version.as_str()) {
        return Err(ProtocolError::UnsupportedVersion(result.protocol_version));
    }
    Ok(ServerIdentity {
        name: result.server_info.name,
        version: result.server_info.version,
        protocol_version: result.protocol_version,
    })
}

pub fn parse_tools_list(bytes: &[u8], expected_id: u64) -> Result<ToolPage, ProtocolError> {
    let result: ToolsListResult = parse_response(bytes, expected_id)?;
    let tools = result
        .tools
        .into_iter()
        .map(|tool| {
            Ok(DiscoveredTool {
                name: tool.name,
                description: tool.description,
                input_schema_json: serde_json::to_string(&tool.input_schema)?,
            })
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()?;
    Ok(ToolPage {
        tools,
        next_cursor: result.next_cursor,
    })
}

pub fn parse_tool_call(bytes: &[u8], expected_id: u64) -> Result<String, ProtocolError> {
    let result: ToolCallResult = parse_response(bytes, expected_id)?;
    let mut parts = result
        .content
        .into_iter()
        .filter(|block| block.kind == "text")
        .filter_map(|block| block.text)
        .collect::<Vec<_>>();
    if let Some(structured) = result.structured_content {
        parts.push(serde_json::to_string(&structured)?);
    }
    let output = parts.join("\n");
    if result.is_error {
        return Err(ProtocolError::Tool(output));
    }
    Ok(output)
}

fn parse_response<T: DeserializeOwned>(bytes: &[u8], expected_id: u64) -> Result<T, ProtocolError> {
    let response: RpcEnvelope<T> = serde_json::from_slice(bytes)?;
    if response.jsonrpc != "2.0" || response.id != expected_id {
        return Err(ProtocolError::UnexpectedResponse);
    }
    if let Some(error) = response.error {
        return Err(ProtocolError::Remote {
            code: error.code,
            message: error.message,
        });
    }
    response.result.ok_or(ProtocolError::UnexpectedResponse)
}

#[cfg(test)]
#[path = "mcp_protocol_tests.rs"]
mod tests;
