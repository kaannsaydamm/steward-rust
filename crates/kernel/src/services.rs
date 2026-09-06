//! Service traits the kernel orchestrates but never implements (§15).

use crate::action::AgentAction;
use crate::event::KernelEvent;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// One normalized message in the conversation history.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KernelMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// Normalized model request.
#[derive(Clone, Debug, Default)]
pub struct ModelRequest {
    pub messages: Vec<KernelMessage>,
    /// Tool schemas the model may call this turn.
    pub tools: Vec<ToolSchema>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Normalized model reply: text and/or structured actions.
#[derive(Clone, Debug, Default)]
pub struct ModelResponse {
    pub text: String,
    pub actions: Vec<AgentAction>,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cost_microusd: u64,
}

/// The model abstraction. `ScriptedModel` (steward-evals) implements this;
/// the daemon's provider clients implement it for real traffic.
#[async_trait]
pub trait ModelService: Send + Sync {
    async fn complete(&self, request: ModelRequest) -> Result<ModelResponse>;
}

/// A single tool execution outcome.
#[derive(Clone, Debug)]
pub struct ToolOutcome {
    pub call_id: String,
    pub ok: bool,
    /// Compact observation returned to the model (Context Engine decides).
    pub observation: String,
}

/// Executes proposed tool actions behind policy/approval (Phase 7).
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, action: &AgentAction) -> Result<ToolOutcome>;
}

/// What the model sees next turn: the canonical tool observation history.
#[derive(Clone, Debug, Default)]
pub struct Observation {
    pub messages: Vec<KernelMessage>,
}

/// Sink for kernel events (daemon persists to run_events + Wire).
#[async_trait]
pub trait EventSink: Send + Sync {
    async fn emit(&self, event: KernelEvent);
}

/// Convenience: no-op sink for tests that inspect state instead.
pub struct NullSink;

#[async_trait]
impl EventSink for NullSink {
    async fn emit(&self, _event: KernelEvent) {}
}

/// Aggregated kernel services (§15 constructor shape).
#[derive(Clone)]
pub struct KernelServices {
    pub models: Arc<dyn ModelService>,
    pub tools: Arc<dyn ToolExecutor>,
    pub success: Arc<dyn crate::success::SuccessEvaluator>,
    pub events: Arc<dyn EventSink>,
}
