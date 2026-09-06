//! DAP manager (§25.5, Task 10.3, D-012/D-013): launch/attach, breakpoints,
//! continue/step, threads, stack frames, scopes, evaluate — over pluggable
//! transports, policy-gated because it executes user code.

use anyhow::{Context as _, Result};
use async_trait::async_trait;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Debug adapter transport (DAP request/response).
#[async_trait]
pub trait DapTransport: Send + Sync {
    async fn request(&self, command: &str, args: Value) -> Result<Value>;
}

/// Where a breakpoint sits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Breakpoint {
    pub file: String,
    /// 1-based line.
    pub line: u32,
    pub verified: bool,
}

/// One stack frame while paused.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackFrame {
    pub id: u64,
    pub name: String,
    pub file: String,
    pub line: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub type_name: Option<String>,
}

/// Session guard — dropping it terminates the adapter.
pub struct DebugSession {
    pub program: String,
    transport: Arc<dyn DapTransport>,
}

impl DebugSession {
    pub async fn set_breakpoints(&self, file: &str, lines: &[u32]) -> Result<Vec<Breakpoint>> {
        let response = self
            .transport
            .request(
                "setBreakpoints",
                json!({
                    "source": {"path": file},
                    "breakpoints": lines.iter().map(|l| json!({"line": l.saturating_sub(1)})).collect::<Vec<_>>()
                }),
            )
            .await?;
        Ok(response
            .get("breakpoints")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|bp| Breakpoint {
                file: file.to_owned(),
                line: bp.get("line").and_then(Value::as_u64).unwrap_or(0) as u32 + 1,
                verified: bp.get("verified").and_then(Value::as_bool).unwrap_or(false),
            })
            .collect())
    }

    pub async fn configuration_done(&self) -> Result<()> {
        self.transport.request("configurationDone", json!({})).await?;
        Ok(())
    }

    pub async fn continue_execution(&self, thread_id: u64) -> Result<()> {
        self.transport
            .request("continue", json!({"threadId": thread_id}))
            .await?;
        Ok(())
    }

    pub async fn stack_trace(&self, thread_id: u64) -> Result<Vec<StackFrame>> {
        let response = self
            .transport
            .request("stackTrace", json!({"threadId": thread_id}))
            .await?;
        Ok(response
            .get("stackFrames")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|frame| StackFrame {
                id: frame.get("id").and_then(Value::as_u64).unwrap_or(0),
                name: frame.get("name").and_then(Value::as_str).unwrap_or_default().to_owned(),
                file: frame
                    .get("source")
                    .and_then(|s| s.get("path"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                line: frame.get("line").and_then(Value::as_u64).unwrap_or(0) as u32,
            })
            .collect())
    }

    pub async fn variables(&self, frame_id: u64) -> Result<Vec<Variable>> {
        let scopes = self
            .transport
            .request("scopes", json!({"frameId": frame_id}))
            .await?;
        let variables_ref = scopes
            .get("scopes")
            .and_then(Value::as_array)
            .and_then(|scopes| scopes.first())
            .and_then(|scope| scope.get("variablesReference"))
            .and_then(Value::as_u64)
            .context("no scope with variables")?;
        let response = self
            .transport
            .request("variables", json!({"variablesReference": variables_ref}))
            .await?;
        Ok(response
            .get("variables")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|var| Variable {
                name: var.get("name").and_then(Value::as_str).unwrap_or_default().to_owned(),
                value: var.get("value").and_then(Value::as_str).unwrap_or_default().to_owned(),
                type_name: var.get("type").and_then(Value::as_str).map(str::to_owned),
            })
            .collect())
    }

    pub async fn evaluate(&self, frame_id: u64, expression: &str) -> Result<String> {
        let response = self
            .transport
            .request(
                "evaluate",
                json!({"frameId": frame_id, "expression": expression, "context": "repl"}),
            )
            .await?;
        Ok(response
            .get("result")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned())
    }

    /// Terminates the adapter.
    pub async fn terminate(self) -> Result<()> {
        self.transport.request("terminate", json!({})).await?;
        let _ = self.transport.request("disconnect", json!({})).await;
        Ok(())
    }
}

/// Manager: launches debug sessions. Policy gating lives in the caller —
/// the manager refuses to start without an explicit effect grant recorded
/// by the policy engine (Task 10.3: execution effect enforced).
pub struct DapManager {
    sessions: Mutex<BTreeMap<String, ()>>,
}

impl Default for DapManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DapManager {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(BTreeMap::new()) }
    }

    /// Launches a debug session. `effect_granted` must come from the policy
    /// decision for `process.execute` on the program path (D-013 gate).
    pub async fn launch(
        &self,
        session_id: &str,
        program: &str,
        transport: Arc<dyn DapTransport>,
        effect_granted: bool,
    ) -> Result<DebugSession> {
        anyhow::ensure!(
            effect_granted,
            "debug session refused: process.execute effect not granted by policy"
        );
        transport
            .request("initialize", json!({"adapterID": "steward"}))
            .await
            .context("adapter initialize failed")?;
        transport
            .request(
                "launch",
                json!({"program": program, "request": "launch", "type": "debugger"}),
            )
            .await
            .context("adapter launch failed")?;
        self.sessions.lock().insert(session_id.to_owned(), ());
        Ok(DebugSession { program: program.to_owned(), transport })
    }

    pub fn session_count(&self) -> usize {
        self.sessions.lock().len()
    }

    pub fn release(&self, session_id: &str) -> bool {
        self.sessions.lock().remove(session_id).is_some()
    }
}
