use crate::tool_registry::{self, RiskLevel, ToolRuntime};
use crate::MySteward;
use steward_core::pb::{
    ListSkillsRequest, ListSkillsResponse, ListToolsRequest, ListToolsResponse, SkillInfo, ToolInfo,
};
use tonic::{Request, Response, Status};

pub async fn list_tools(
    steward: &MySteward,
    request: Request<ListToolsRequest>,
) -> Result<Response<ListToolsResponse>, Status> {
    let include_disabled = request.into_inner().include_disabled;
    let connection = steward
        .db
        .lock()
        .map_err(|_| Status::internal("Database lock failed"))?;
    let tools = tool_registry::list_tools(&connection)
        .map_err(|error| Status::internal(error.to_string()))?
        .into_iter()
        .filter(|tool| include_disabled || tool.enabled)
        .map(|tool| ToolInfo {
            tool_id: tool.id,
            name: tool.name,
            description: tool.description,
            runtime: runtime_code(tool.runtime),
            risk: risk_code(tool.risk),
            enabled: tool.enabled,
            requires_approval: tool.requires_approval,
        })
        .collect();
    Ok(Response::new(ListToolsResponse { tools }))
}

pub async fn list_skills(
    steward: &MySteward,
    request: Request<ListSkillsRequest>,
) -> Result<Response<ListSkillsResponse>, Status> {
    let include_disabled = request.into_inner().include_disabled;
    let connection = steward
        .db
        .lock()
        .map_err(|_| Status::internal("Database lock failed"))?;
    let skills = tool_registry::list_skills(&connection)
        .map_err(|error| Status::internal(error.to_string()))?
        .into_iter()
        .filter(|skill| include_disabled || skill.enabled)
        .map(|skill| SkillInfo {
            skill_id: skill.id,
            name: skill.name,
            description: skill.description,
            version: skill.version,
            enabled: skill.enabled,
            tool_ids: skill.tool_ids,
        })
        .collect();
    Ok(Response::new(ListSkillsResponse { skills }))
}

const fn runtime_code(runtime: ToolRuntime) -> i32 {
    match runtime {
        ToolRuntime::Builtin => 1,
        ToolRuntime::Wasm => 2,
        ToolRuntime::Process => 3,
        ToolRuntime::Mcp => 4,
    }
}

const fn risk_code(risk: RiskLevel) -> i32 {
    match risk {
        RiskLevel::Low => 1,
        RiskLevel::Medium => 2,
        RiskLevel::High => 3,
    }
}
