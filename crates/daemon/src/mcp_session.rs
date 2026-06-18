use crate::mcp_protocol::{
    initialize_request, initialized_notification, parse_initialize, parse_tool_call,
    parse_tools_list, tool_call_request, tools_list_request, ServerIdentity,
};
use crate::mcp_registry::{AdapterConfig, DiscoveredTool};
use anyhow::{bail, Context as _, Result};
use std::collections::BTreeMap;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_FRAME_BYTES: usize = 1024 * 1024;

pub struct StartedSession {
    pub identity: ServerIdentity,
    pub tools: Vec<DiscoveredTool>,
}

pub struct McpSession {
    pub child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    pub identity: ServerIdentity,
    pub tool_count: usize,
}

impl McpSession {
    pub async fn connect(config: &AdapterConfig) -> Result<(Self, StartedSession)> {
        let mut command = Command::new(&config.command);
        command
            .args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if let Some(cwd) = &config.cwd {
            command.current_dir(cwd);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("starting MCP adapter '{}'", config.id))?;
        let stdin = child
            .stdin
            .take()
            .context("MCP adapter stdin unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("MCP adapter stdout unavailable")?;
        let mut session = Self {
            child,
            stdin: Some(stdin),
            stdout: BufReader::new(stdout),
            next_id: 1,
            identity: ServerIdentity {
                name: String::new(),
                version: String::new(),
                protocol_version: String::new(),
            },
            tool_count: 0,
        };
        let initialize_id = session.next_request_id();
        let response = session
            .request(initialize_request(initialize_id), initialize_id)
            .await?;
        let identity = parse_initialize(&response, initialize_id)?;
        session.send(initialized_notification()).await?;
        let tools = session.discover_tools().await?;
        session.identity = identity.clone();
        session.tool_count = tools.len();
        Ok((session, StartedSession { identity, tools }))
    }

    pub async fn call_tool(
        &mut self,
        remote_name: &str,
        arguments: &BTreeMap<String, String>,
    ) -> Result<String> {
        let id = self.next_request_id();
        let response = self
            .request(tool_call_request(id, remote_name, arguments), id)
            .await?;
        parse_tool_call(&response, id).map_err(Into::into)
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        self.stdin.take();
        if timeout(SHUTDOWN_TIMEOUT, self.child.wait()).await.is_err() {
            self.child.start_kill()?;
            timeout(SHUTDOWN_TIMEOUT, self.child.wait())
                .await
                .context("MCP adapter did not stop after kill")??;
        }
        Ok(())
    }

    async fn discover_tools(&mut self) -> Result<Vec<DiscoveredTool>> {
        let mut tools = Vec::new();
        let mut cursor = None;
        loop {
            let id = self.next_request_id();
            let response = self
                .request(tools_list_request(id, cursor.as_deref()), id)
                .await?;
            let page = parse_tools_list(&response, id)?;
            tools.extend(page.tools);
            cursor = page.next_cursor;
            if cursor.is_none() {
                return Ok(tools);
            }
        }
    }

    async fn request(&mut self, request: serde_json::Value, expected_id: u64) -> Result<Vec<u8>> {
        self.send(request).await?;
        match timeout(REQUEST_TIMEOUT, self.read_response(expected_id)).await {
            Ok(response) => response,
            Err(_) => {
                self.send(serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/cancelled",
                    "params": {"requestId": expected_id, "reason": "Steward request timeout"}
                }))
                .await?;
                bail!("MCP request timed out")
            }
        }
    }

    async fn read_response(&mut self, expected_id: u64) -> Result<Vec<u8>> {
        loop {
            let frame = self.read_frame().await?;
            let value: serde_json::Value = serde_json::from_slice(&frame)?;
            if value.get("id").and_then(serde_json::Value::as_u64) == Some(expected_id)
                && value.get("method").is_none()
            {
                return Ok(frame);
            }
            if let (Some(id), Some(method)) = (
                value.get("id").cloned(),
                value.get("method").and_then(serde_json::Value::as_str),
            ) {
                let response = if method == "ping" {
                    serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {}})
                } else {
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {"code": -32601, "message": "method not supported by Steward"}
                    })
                };
                self.send(response).await?;
            }
        }
    }

    async fn read_frame(&mut self) -> Result<Vec<u8>> {
        let mut frame = Vec::new();
        loop {
            let buffer = self.stdout.fill_buf().await?;
            if buffer.is_empty() {
                bail!("MCP adapter closed stdout");
            }
            let newline = buffer.iter().position(|byte| *byte == b'\n');
            let take = newline.map_or(buffer.len(), |index| index + 1);
            if frame.len() + take > MAX_FRAME_BYTES {
                bail!("MCP frame exceeds {MAX_FRAME_BYTES} bytes");
            }
            frame.extend_from_slice(&buffer[..take]);
            self.stdout.consume(take);
            if newline.is_some() {
                return Ok(frame);
            }
        }
    }

    async fn send(&mut self, value: serde_json::Value) -> Result<()> {
        let stdin = self.stdin.as_mut().context("MCP adapter stdin is closed")?;
        let mut bytes = serde_json::to_vec(&value)?;
        bytes.push(b'\n');
        stdin.write_all(&bytes).await?;
        stdin.flush().await?;
        Ok(())
    }

    fn next_request_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}
