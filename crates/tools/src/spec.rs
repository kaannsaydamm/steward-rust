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
