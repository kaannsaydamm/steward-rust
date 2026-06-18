use steward_core::pb::{
    InvokeToolResponse, SkillInfo, ToolInfo, ToolInvocationInfo, ToolRiskLevel, ToolRuntimeKind,
};

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
        "{}\tversion={}\tenabled={}\tsigned={}\tpublisher={}\ttools={}\t{}",
        skill.skill_id,
        skill.version,
        skill.enabled,
        skill.signed,
        publisher_fingerprint(&skill.publisher_key),
        skill.tool_ids.join(","),
        skill.name
    )
}

fn publisher_fingerprint(key: &str) -> &str {
    key.get(..12).unwrap_or(key)
}

pub fn invocation_response_line(response: &InvokeToolResponse) -> String {
    format!(
        "{}\tstatus={}\tapproval_required={}\t{}",
        response.invocation_id, response.status, response.requires_approval, response.message
    )
}

pub fn invocation_line(invocation: &ToolInvocationInfo) -> String {
    let detail = if invocation.error.is_empty() {
        invocation.output.replace('\n', " ")
    } else {
        invocation.error.replace('\n', " ")
    };
    let detail = compact_detail(&detail);
    format!(
        "{}\t{}\tstatus={}\tapproved={}\t{}",
        invocation.invocation_id,
        invocation.tool_id,
        invocation.status,
        invocation.approved,
        detail
    )
}

fn compact_detail(detail: &str) -> String {
    const MAX_CHARS: usize = 160;
    if detail.chars().count() <= MAX_CHARS {
        return detail.to_owned();
    }
    let mut compact = detail.chars().take(MAX_CHARS).collect::<String>();
    compact.push_str("...");
    compact
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
