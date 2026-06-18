use steward_core::pb::{SkillInfo, ToolInfo, ToolRiskLevel, ToolRuntimeKind};

pub fn tool_line(tool: &ToolInfo) -> String {
    format!(
        "{}\truntime={}\trisk={}\tenabled={}\tapproval={}\t{}",
        tool.tool_id,
        runtime_name(tool.runtime),
        risk_name(tool.risk),
        tool.enabled,
        tool.requires_approval,
        tool.name
    )
}

pub fn skill_line(skill: &SkillInfo) -> String {
    format!(
        "{}\tversion={}\tenabled={}\ttools={}\t{}",
        skill.skill_id,
        skill.version,
        skill.enabled,
        skill.tool_ids.join(","),
        skill.name
    )
}

fn runtime_name(code: i32) -> &'static str {
    match ToolRuntimeKind::try_from(code) {
        Ok(ToolRuntimeKind::Builtin) => "builtin",
        Ok(ToolRuntimeKind::Wasm) => "wasm",
        Ok(ToolRuntimeKind::Process) => "process",
        Ok(ToolRuntimeKind::Mcp) => "mcp",
        Ok(ToolRuntimeKind::Unspecified) | Err(_) => "unknown",
    }
}

fn risk_name(code: i32) -> &'static str {
    match ToolRiskLevel::try_from(code) {
        Ok(ToolRiskLevel::Low) => "low",
        Ok(ToolRiskLevel::Medium) => "medium",
        Ok(ToolRiskLevel::High) => "high",
        Ok(ToolRiskLevel::Unspecified) | Err(_) => "unknown",
    }
}
