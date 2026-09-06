//! Normalized actions a model turn can take (§16).

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentAction {
    /// Call a tool by name with JSON arguments.
    Tool {
        call_id: String,
        name: String,
        arguments: Value,
    },
    /// Execute code in a managed runtime (Phase 14 bridges this to RLM).
    Code { language: String, source: String },
    /// Spawn a subagent under a profile.
    Spawn { profile_id: String, task: String },
    /// A final answer for this run.
    Final { text: String },
}
