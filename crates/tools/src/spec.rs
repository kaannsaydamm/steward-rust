//! Normalized ToolSpec (§23.1): built-ins and MCP tools unify here.

use crate::effects::Effect;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    /// Canonical id (`fs.read`); wire names may differ.
    pub id: String,
    pub title: String,
    pub description: String,
    /// JSON schema for arguments.
    pub schema: Value,
    /// Declared effects — the ONLY authorization input (S-001).
    pub effects: Vec<Effect>,
    pub risk: RiskLevel,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Registry plugin that owns this tool (`builtin`, MCP server id...).
    pub provider: String,
}

impl ToolSpec {
    /// JSON-schema shape exposed to models.
    pub fn model_schema(&self) -> Value {
        self.schema.clone()
    }

    pub fn declares(&self, effect: &Effect) -> bool {
        self.effects.contains(effect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn read_tool() -> ToolSpec {
        ToolSpec {
            id: "fs.read".into(),
            title: "Read files".into(),
            description: "Read workspace files".into(),
            schema: json!({"type": "object"}),
            effects: vec![Effect::FilesystemRead],
            risk: RiskLevel::Low,
            tags: vec![],
            provider: "builtin".into(),
        }
    }

    #[test]
    fn builtin_spec_declares_effects() {
        let tool = read_tool();
        assert!(tool.declares(&Effect::FilesystemRead));
        assert!(!tool.declares(&Effect::FilesystemWrite));
        assert_eq!(tool.risk, RiskLevel::Low);
    }

    #[test]
    fn mcp_and_builtin_unify_on_one_shape() {
        // An MCP-discovered tool maps to the same ToolSpec — only `provider`
        // differs. This is the normalization contract (Task 7.1).
        let mcp_tool = ToolSpec {
            provider: "mcp:github".into(),
            ..read_tool()
        };
        assert_eq!(mcp_tool.effects, read_tool().effects);
        assert_eq!(mcp_tool.model_schema(), read_tool().model_schema());
    }

    #[test]
    fn spec_serializes_roundtrip() {
        let tool = read_tool();
        let json = serde_json::to_string(&tool).unwrap();
        let back: ToolSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, tool);
    }
}

#[cfg(test)]
mod parity_tools_tests {
    use super::*;
    use crate::effects::Effect;
    use serde_json::json;

    /// Effect declarations for the parity tool expansion. The policy is the
    /// ONLY authorization input — these specs are the contract (AGENTS.md #2).
    fn spec(id: &str, effects: Vec<Effect>, risk: RiskLevel) -> ToolSpec {
        ToolSpec {
            id: id.into(),
            title: id.into(),
            description: String::new(),
            schema: json!({}),
            effects,
            risk,
            tags: vec![],
            provider: "builtin".into(),
        }
    }

    #[test]
    fn sqlite_query_declares_read_only_effects() {
        let tool = spec("sqlite.query", vec![Effect::FilesystemRead], RiskLevel::Medium);
        assert!(tool.declares(&Effect::FilesystemRead));
        // No write effect: the tool can never be authorized to mutate.
        assert!(!tool.declares(&Effect::FilesystemWrite));
        assert!(!tool.declares(&Effect::FilesystemDelete));
    }

    #[test]
    fn pdf_extract_is_read_only() {
        let tool = spec("pdf.extract", vec![Effect::FilesystemRead], RiskLevel::Low);
        assert!(tool.declares(&Effect::FilesystemRead));
        assert!(!tool.declares(&Effect::ProcessExecute));
    }

    #[test]
    fn checkpoint_save_load_pair_gates_on_write() {
        let save = spec("checkpoint.save", vec![], RiskLevel::Low);
        let load = spec("checkpoint.load", vec![Effect::FilesystemWrite], RiskLevel::Medium);
        // Saving snapshots without touching the file: no effects.
        assert!(save.effects.is_empty());
        // Loading rewrites the file: the write effect must gate it.
        assert!(load.declares(&Effect::FilesystemWrite));
    }

    #[test]
    fn think_is_side_effect_free() {
        let tool = spec("think", vec![Effect::MemoryWrite], RiskLevel::Low);
        // Think appends a scratchpad note: memory write declared, nothing else.
        assert_eq!(tool.effects.len(), 1);
        assert!(!tool.declares(&Effect::NetworkHttp));
    }

    #[test]
    fn todo_list_declares_memory_effects() {
        let tool = spec("todo.list", vec![Effect::MemoryWrite], RiskLevel::Low);
        assert!(tool.declares(&Effect::MemoryWrite));
        assert!(!tool.declares(&Effect::FilesystemWrite));
    }

    #[test]
    fn policy_denies_ungranted_write_for_checkpoint_load() {
        let load = spec("checkpoint.load", vec![Effect::FilesystemWrite], RiskLevel::Medium);
        let policy = crate::EffectPolicy {
            allowed: vec![],
            denied: vec![],
        };
        // No grants: the write effect requires approval (never silently allowed).
        assert!(matches!(
            policy.evaluate(&load, None),
            crate::PolicyDecision::ApprovalRequired { .. }
        ));
    }
}
