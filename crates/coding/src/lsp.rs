//! LSP server manager (§25.4, Tasks 10.1-10.2, D-006..D-011).
//!
//! One server per workspace/language over stdio JSON-RPC. The transport is
//! generic over an async duplex stream so the conformance suite runs against
//! an in-process mock server; real servers plug in via the same trait.

use anyhow::{Context as _, Result};
use async_trait::async_trait;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Language server transport: request/response JSON-RPC over any channel.
#[async_trait]
pub trait LspTransport: Send + Sync {
    async fn request(&self, method: &str, params: Value) -> anyhow::Result<Value>;
}

/// Language id for a file extension ("rs" -> "rust").
pub fn language_for(extension: &str) -> &'static str {
    match extension {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" => "cpp",
        _ => "plaintext",
    }
}

/// A connected language-server session.
#[derive(Clone, Debug)]
pub struct LspSession {
    pub workspace_root: String,
    pub language: &'static str,
}

/// Diagnostics normalized to the plan schema (D-009).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub file: String,
    pub line: u32,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub source: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl DiagnosticSeverity {
    fn from_lsp(code: u64) -> Self {
        match code {
            1 => Self::Error,
            2 => Self::Warning,
            3 => Self::Information,
            _ => Self::Hint,
        }
    }
}

/// Definition/reference location.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// The manager: session cache keyed by workspace+language.
pub struct LspManager {
    sessions: Mutex<BTreeMap<String, Arc<dyn LspTransport>>>,
}

impl Default for LspManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LspManager {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(BTreeMap::new()) }
    }

    /// Starts (or reuses) a server for a workspace/language.
    pub async fn start_server(
        &self,
        workspace_root: &str,
        language: &'static str,
        transport: Arc<dyn LspTransport>,
    ) -> Result<LspSession> {
        let key = format!("{workspace_root}::{language}");
        {
            let sessions = self.sessions.lock();
            if sessions.contains_key(&key) {
                return Ok(LspSession { workspace_root: workspace_root.into(), language });
            }
        }
        // initialize + initialized handshake (cached).
        transport
            .request(
                "initialize",
                json!({
                    "processId": std::process::id(),
                    "rootUri": format!("file://{workspace_root}"),
                    "capabilities": {}
                }),
            )
            .await
            .context("LSP initialize failed")?;
        transport.request("initialized", json!({})).await.ok();
        self.sessions.lock().insert(key, transport);
        Ok(LspSession { workspace_root: workspace_root.into(), language })
    }

    fn transport(&self, workspace_root: &str, language: &str) -> Result<Arc<dyn LspTransport>> {
        let key = format!("{workspace_root}::{language}");
        self.sessions
            .lock()
            .get(&key)
            .cloned()
            .context("no LSP server for this workspace/language; start one first")
    }

    /// Hover (D-006).
    pub async fn hover(&self, session: &LspSession, file: &str, line: u32, column: u32) -> Result<String> {
        let transport = self.transport(&session.workspace_root, session.language)?;
        let response = transport
            .request(
                "textDocument/hover",
                json!({
                    "textDocument": {"uri": format!("file://{file}")},
                    "position": {"line": line.saturating_sub(1), "character": column.saturating_sub(1)}
                }),
            )
            .await?;
        Ok(response
            .get("contents")
            .and_then(|c| c.as_str())
            .unwrap_or_default()
            .to_owned())
    }

    /// Definition (D-006).
    pub async fn definition(&self, session: &LspSession, file: &str, line: u32, column: u32) -> Result<Vec<Location>> {
        let transport = self.transport(&session.workspace_root, session.language)?;
        let response = transport
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": {"uri": format!("file://{file}")},
                    "position": {"line": line.saturating_sub(1), "character": column.saturating_sub(1)}
                }),
            )
            .await?;
        Ok(parse_locations(response))
    }

    /// References with exact ranges (D-007).
    pub async fn references(&self, session: &LspSession, file: &str, line: u32, column: u32, include_declaration: bool) -> Result<Vec<Location>> {
        let transport = self.transport(&session.workspace_root, session.language)?;
        let response = transport
            .request(
                "textDocument/references",
                json!({
                    "textDocument": {"uri": format!("file://{file}")},
                    "position": {"line": line.saturating_sub(1), "character": column.saturating_sub(1)},
                    "context": {"includeDeclaration": include_declaration}
                }),
            )
            .await?;
        Ok(parse_locations(response))
    }

    /// Diagnostics for a file (D-009).
    pub async fn diagnostics(&self, session: &LspSession, file: &str) -> Result<Vec<Diagnostic>> {
        let transport = self.transport(&session.workspace_root, session.language)?;
        let response = transport
            .request(
                "textDocument/diagnostic",
                json!({"textDocument": {"uri": format!("file://{file}")}}),
            )
            .await?;
        let items = response.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
        Ok(items
            .iter()
            .map(|item| Diagnostic {
                file: file.to_owned(),
                line: item.get("range").and_then(|r| r.get("start")).and_then(|s| s.get("line")).and_then(Value::as_u64).unwrap_or(0) as u32 + 1,
                severity: item.get("severity").and_then(Value::as_u64).map(DiagnosticSeverity::from_lsp).unwrap_or(DiagnosticSeverity::Hint),
                message: item.get("message").and_then(Value::as_str).unwrap_or_default().to_owned(),
                source: item.get("source").and_then(Value::as_str).unwrap_or("lsp").to_owned(),
            })
            .collect())
    }

    /// Rename across the workspace under a checkpoint (D-008): the workspace
    /// edit is returned for the caller to apply atomically.
    pub async fn rename(&self, session: &LspSession, file: &str, line: u32, column: u32, new_name: &str) -> Result<Value> {
        let transport = self.transport(&session.workspace_root, session.language)?;
        transport
            .request(
                "textDocument/rename",
                json!({
                    "textDocument": {"uri": format!("file://{file}")},
                    "position": {"line": line.saturating_sub(1), "character": column.saturating_sub(1)},
                    "newName": new_name
                }),
            )
            .await
    }
}

fn parse_locations(value: Value) -> Vec<Location> {
    let items = match value {
        Value::Array(items) => items,
        Value::Null => return Vec::new(),
        single => vec![single],
    };
    items
        .iter()
        .filter_map(|item| {
            let uri = item.get("uri")?.as_str()?.trim_start_matches("file://").to_owned();
            let start = item.get("range")?.get("start")?;
            Some(Location {
                file: uri,
                line: start.get("line")?.as_u64()? as u32 + 1,
                column: start.get("character")?.as_u64()? as u32 + 1,
            })
        })
        .collect()
}

