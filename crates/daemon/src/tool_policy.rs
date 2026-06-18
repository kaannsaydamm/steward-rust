use crate::tool_registry::ToolDefinition;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyDecision {
    Allowed,
    RequiresApproval,
    DeniedDisabled,
}

pub const fn evaluate(tool: &ToolDefinition, approved: bool) -> PolicyDecision {
    match (tool.enabled, tool.requires_approval, approved) {
        (false, _, _) => PolicyDecision::DeniedDisabled,
        (true, true, false) => PolicyDecision::RequiresApproval,
        (true, false, _) | (true, true, true) => PolicyDecision::Allowed,
    }
}

#[cfg(test)]
#[path = "tool_policy_tests.rs"]
mod tests;
