use serde_json::{json, Value};
use std::io::{self, BufRead as _, Write as _};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    let mut lines = stdin.lock().lines();
    while let Some(line) = lines.next() {
        let request: Value = serde_json::from_str(&line?)?;
        let Some(method) = request.get("method").and_then(Value::as_str) else {
            continue;
        };
        let response = match method {
            "initialize" => {
                serde_json::to_writer(
                    &mut stdout,
                    &json!({"jsonrpc": "2.0", "id": 900, "method": "ping"}),
                )?;
                stdout.write_all(b"\n")?;
                stdout.flush()?;
                let ping_response: Value =
                    serde_json::from_str(&lines.next().ok_or("missing ping response")??)?;
                if ping_response["id"] != 900 || ping_response.get("result").is_none() {
                    return Err("invalid ping response".into());
                }
                Some(json!({
                    "jsonrpc": "2.0",
                    "id": request["id"],
                    "result": {
                        "protocolVersion": "2025-11-25",
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "steward-fixture", "version": "1.0.0"}
                    }
                }))
            }
            "notifications/initialized" => None,
            "tools/list" => Some(json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "result": {
                    "tools": [{
                        "name": "echo",
                        "description": "Echo text",
                        "inputSchema": {
                            "type": "object",
                            "properties": {"text": {"type": "string"}},
                            "required": ["text"]
                        }
                    }]
                }
            })),
            "tools/call" => Some(json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "result": {
                    "content": [{"type": "text", "text": request["params"]["arguments"]["text"]}],
                    "isError": false
                }
            })),
            _ => Some(json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "error": {"code": -32601, "message": "method not found"}
            })),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut stdout, &response)?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
        }
    }
    Ok(())
}
