use crate::tool_registry::{self, RiskLevel, ToolRuntime};
use crate::MySteward;
use crate::{skill_installation, tool_audit, tool_invocation};
use steward_core::pb::{
    InstallSkillRequest, InstallSkillResponse, InvokeToolRequest, InvokeToolResponse,
    ListSkillsRequest, ListSkillsResponse, ListToolInvocationsRequest, ListToolInvocationsResponse,
    ListToolsRequest, ListToolsResponse, SkillInfo, ToolInfo, ToolInvocationInfo,
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
        .map(skill_info)
        .collect();
    Ok(Response::new(ListSkillsResponse { skills }))
}

pub async fn install_skill(
    steward: &MySteward,
    request: Request<InstallSkillRequest>,
) -> Result<Response<InstallSkillResponse>, Status> {
    let bundle = skill_installation::decode_bundle(&request.into_inner().bundle)
        .map_err(|error| Status::invalid_argument(error.to_string()))?;
    let connection = steward
        .db
        .lock()
        .map_err(|_| Status::internal("Database lock failed"))?;
    let skill = skill_installation::install(&connection, &bundle)
        .map_err(|error| Status::failed_precondition(error.to_string()))?;
    Ok(Response::new(InstallSkillResponse {
        skill: Some(skill_info(skill)),
    }))
}

fn skill_info(skill: tool_registry::SkillDefinition) -> SkillInfo {
    SkillInfo {
        skill_id: skill.id,
        name: skill.name,
        description: skill.description,
        version: skill.version,
        enabled: skill.enabled,
        tool_ids: skill.tool_ids,
        signed: !skill.publisher_key.is_empty(),
        publisher_key: skill.publisher_key,
        manifest_digest: skill.manifest_digest,
        installed_at: skill.installed_at,
    }
}

pub async fn invoke_tool(
    steward: &MySteward,
    request: Request<InvokeToolRequest>,
) -> Result<Response<InvokeToolResponse>, Status> {
    let req = request.into_inner();
    let outcome = tool_invocation::invoke(
        steward,
        &req.tool_id,
        req.arguments.into_iter().collect(),
        req.approved,
    )
    .await
    .map_err(|error| Status::internal(error.to_string()))?
    .ok_or_else(|| Status::not_found(format!("tool '{}' not found", req.tool_id)))?;
    Ok(Response::new(InvokeToolResponse {
        invocation_id: outcome.invocation_id,
        status: invocation_status(outcome.status).to_owned(),
        output: outcome.output,
        message: outcome.message,
        requires_approval: outcome.requires_approval,
    }))
}

pub async fn list_invocations(
    steward: &MySteward,
    request: Request<ListToolInvocationsRequest>,
) -> Result<Response<ListToolInvocationsResponse>, Status> {
    let limit = usize::try_from(request.into_inner().limit.max(1)).unwrap_or(20);
    let connection = steward
        .db
        .lock()
        .map_err(|_| Status::internal("Database lock failed"))?;
    let invocations = tool_audit::list_recent(&connection, limit)
        .map_err(|error| Status::internal(error.to_string()))?
        .into_iter()
        .map(|record| ToolInvocationInfo {
            invocation_id: record.invocation_id,
            tool_id: record.tool_id,
            approved: record.approved,
            input_json: record.input_json,
            status: invocation_status(record.status).to_owned(),
            output: record.output,
            error: record.error,
            created_at: record.created_at,
        })
        .collect();
    Ok(Response::new(ListToolInvocationsResponse { invocations }))
}

const fn invocation_status(status: tool_audit::InvocationStatus) -> &'static str {
    match status {
        tool_audit::InvocationStatus::PendingApproval => "pending_approval",
        tool_audit::InvocationStatus::Denied => "denied",
        tool_audit::InvocationStatus::Succeeded => "succeeded",
        tool_audit::InvocationStatus::Failed => "failed",
    }
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
