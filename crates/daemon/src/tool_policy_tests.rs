use super::{evaluate, PolicyDecision};
use crate::tool_registry::{RiskLevel, ToolDefinition, ToolRuntime};

#[test]
fn disabled_tool_is_denied_even_with_approval() {
    let tool = tool(false, true);

    assert_eq!(evaluate(&tool, true), PolicyDecision::DeniedDisabled);
}

#[test]
fn approval_gated_tool_waits_for_explicit_approval() {
    let tool = tool(true, true);

    assert_eq!(evaluate(&tool, false), PolicyDecision::RequiresApproval);
    assert_eq!(evaluate(&tool, true), PolicyDecision::Allowed);
}

#[test]
fn enabled_low_risk_tool_runs_without_approval() {
    let tool = tool(true, false);

    assert_eq!(evaluate(&tool, false), PolicyDecision::Allowed);
}

fn tool(enabled: bool, requires_approval: bool) -> ToolDefinition {
    ToolDefinition {
        id: "test.tool".to_owned(),
        name: "Test tool".to_owned(),
        description: "Policy fixture".to_owned(),
        runtime: ToolRuntime::Builtin,
        risk: RiskLevel::Low,
        enabled,
        requires_approval,
    }
}
