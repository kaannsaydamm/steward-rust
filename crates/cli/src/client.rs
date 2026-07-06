use anyhow::{Context as _, Result};
use std::time::Duration;
use steward_core::pb::steward_service_client::StewardServiceClient;
use steward_core::pb::{
    AgentInfo, ApprovePlanRequest, CancelWorkflowRequest, ExecuteTaskRequest,
    GetKnowledgeGraphRequest, GetKnowledgeGraphResponse, GetSecuritySettingsRequest,
    InstallSkillRequest, InvokeToolRequest, InvokeToolResponse, ListAgentsRequest,
    ListMcpAdaptersRequest, ListSkillsRequest, ListToolInvocationsRequest, ListToolsRequest,
    ListWorkflowsRequest, MaintenanceRequest, MaintenanceStatus, McpAdapterActionRequest,
    McpAdapterInfo, MemoryEntry, PingRequest, PruneResponse, RecallMemoryRequest,
    RegisterMcpAdapterRequest, SecuritySettingsInfo, SetToolEnabledRequest, SkillInfo,
    StartWorkflowRequest, StoreMemoryRequest, ToolInfo, ToolInvocationInfo, WorkflowEvent,
    WorkflowStatus,
};
use steward_core::pb::{AgentLogEntry, GetAgentLogRequest, GetWorkflowStatusRequest};
use tonic::transport::{Channel, Endpoint};
use tonic::Request;

pub type Client = StewardServiceClient<Channel>;

pub async fn connect(host: &str) -> Result<Client> {
    let endpoint = Endpoint::from_shared(host.to_owned())
        .with_context(|| format!("invalid steward daemon endpoint {host}"))?
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(30));
    StewardServiceClient::connect(endpoint)
        .await
        .with_context(|| format!("connecting to steward daemon at {host}"))
}

pub async fn ping(host: &str) -> Result<String> {
    let mut client = connect(host).await?;
    let response = client
        .ping(Request::new(PingRequest {}))
        .await
        .context("calling Ping")?
        .into_inner();
    Ok(response.status)
}

pub async fn execute_task(host: &str, task: &str) -> Result<String> {
    let mut client = connect(host).await?;
    let response = client
        .execute_task(Request::new(ExecuteTaskRequest {
            task: task.to_owned(),
        }))
        .await
        .context("calling ExecuteTask")?
        .into_inner();
    Ok(response.status)
}

pub async fn store_memory(
    host: &str,
    memory_type: i32,
    content: &str,
    metadata: Vec<(String, String)>,
    entities: Vec<String>,
) -> Result<String> {
    let mut client = connect(host).await?;
    let response = client
        .store_memory(Request::new(StoreMemoryRequest {
            entry: Some(MemoryEntry {
                id: String::new(),
                memory_type,
                content: content.to_owned(),
                metadata: metadata.into_iter().collect(),
                entities,
                timestamp: unix_seconds(),
                embedding: Vec::new(),
            }),
        }))
        .await
        .context("calling StoreMemory")?
        .into_inner();
    Ok(response.id)
}

pub async fn recall_memory(
    host: &str,
    query: &str,
    memory_type: i32,
    limit: i32,
) -> Result<Vec<MemoryEntry>> {
    let mut client = connect(host).await?;
    let response = client
        .recall_memory(Request::new(RecallMemoryRequest {
            query: query.to_owned(),
            memory_type,
            limit,
            query_embedding: Vec::new(),
        }))
        .await
        .context("calling RecallMemory")?
        .into_inner();
    Ok(response.entries)
}

pub async fn list_workflows(host: &str) -> Result<Vec<WorkflowStatus>> {
    let mut client = connect(host).await?;
    let response = client
        .list_workflows(Request::new(ListWorkflowsRequest {
            phase_filter: 0,
            limit: 50,
        }))
        .await
        .context("calling ListWorkflows")?
        .into_inner();
    Ok(response.workflows)
}

pub async fn get_workflow_status(host: &str, workflow_id: &str) -> Result<WorkflowStatus> {
    let mut client = connect(host).await?;
    let response = client
        .get_workflow_status(Request::new(GetWorkflowStatusRequest {
            workflow_id: workflow_id.to_owned(),
        }))
        .await
        .context("calling GetWorkflowStatus")?
        .into_inner();
    Ok(response)
}

pub async fn get_agent_logs(
    host: &str,
    workflow_id: &str,
    agent_id: &str,
    limit: usize,
) -> Result<Vec<AgentLogEntry>> {
    let mut client = connect(host).await?;
    let mut stream = client
        .get_agent_log(Request::new(GetAgentLogRequest {
            workflow_id: workflow_id.to_owned(),
            agent_id: agent_id.to_owned(),
        }))
        .await
        .context("calling GetAgentLog")?
        .into_inner();
    let mut entries = Vec::new();
    while let Some(entry) = stream.message().await.context("reading agent log stream")? {
        entries.push(entry);
        if entries.len() >= limit {
            break;
        }
    }
    Ok(entries)
}

pub async fn start_workflow(
    host: &str,
    title: &str,
    description: &str,
    target_repo: &str,
) -> Result<Vec<WorkflowEvent>> {
    let mut client = connect(host).await?;
    let mut stream = client
        .start_workflow(Request::new(StartWorkflowRequest {
            title: title.to_owned(),
            description: description.to_owned(),
            target_repo: target_repo.to_owned(),
            files: Vec::new(),
            constraints: Default::default(),
            definition_id: String::new(),
        }))
        .await
        .context("calling StartWorkflow")?
        .into_inner();

    let mut events = Vec::new();
    while let Some(event) = stream.message().await.context("reading workflow stream")? {
        let requires_approval = event.requires_approval;
        events.push(event);
        if requires_approval {
            break;
        }
    }
    Ok(events)
}

pub async fn approve_workflow(host: &str, workflow_id: &str, mode: i32) -> Result<String> {
    let mut client = connect(host).await?;
    let response = client
        .approve_plan(Request::new(ApprovePlanRequest {
            workflow_id: workflow_id.to_owned(),
            mode,
            approved: true,
            feedback: String::new(),
        }))
        .await
        .context("calling ApprovePlan")?
        .into_inner();
    Ok(response.message)
}

pub async fn cancel_workflow(host: &str, workflow_id: &str, reason: &str) -> Result<bool> {
    let mut client = connect(host).await?;
    let response = client
        .cancel_workflow(Request::new(CancelWorkflowRequest {
            workflow_id: workflow_id.to_owned(),
            reason: reason.to_owned(),
        }))
        .await
        .context("calling CancelWorkflow")?
        .into_inner();
    Ok(response.cancelled)
}

pub async fn list_agents(host: &str) -> Result<Vec<AgentInfo>> {
    let mut client = connect(host).await?;
    let response = client
        .list_agents(Request::new(ListAgentsRequest {}))
        .await
        .context("calling ListAgents")?
        .into_inner();
    Ok(response.agents)
}

pub async fn list_tools(host: &str) -> Result<Vec<ToolInfo>> {
    let mut client = connect(host).await?;
    let response = client
        .list_tools(Request::new(ListToolsRequest {
            include_disabled: true,
        }))
        .await
        .context("calling ListTools")?
        .into_inner();
    Ok(response.tools)
}

pub async fn set_tool_enabled(host: &str, tool_id: &str, enabled: bool) -> Result<ToolInfo> {
    let mut client = connect(host).await?;
    Ok(client
        .set_tool_enabled(Request::new(SetToolEnabledRequest {
            tool_id: tool_id.to_owned(),
            enabled,
        }))
        .await
        .context("calling SetToolEnabled")?
        .into_inner())
}

pub async fn list_skills(host: &str) -> Result<Vec<SkillInfo>> {
    let mut client = connect(host).await?;
    let response = client
        .list_skills(Request::new(ListSkillsRequest {
            include_disabled: true,
        }))
        .await
        .context("calling ListSkills")?
        .into_inner();
    Ok(response.skills)
}

pub async fn install_skill(host: &str, bundle: Vec<u8>) -> Result<SkillInfo> {
    let mut client = connect(host).await?;
    client
        .install_skill(Request::new(InstallSkillRequest { bundle }))
        .await
        .context("calling InstallSkill")?
        .into_inner()
        .skill
        .context("InstallSkill returned no skill")
}

pub async fn register_mcp(
    host: &str,
    adapter_id: &str,
    name: &str,
    command: &str,
    arguments: Vec<String>,
    cwd: &str,
) -> Result<McpAdapterInfo> {
    let mut client = connect(host).await?;
    Ok(client
        .register_mcp_adapter(Request::new(RegisterMcpAdapterRequest {
            adapter_id: adapter_id.to_owned(),
            name: name.to_owned(),
            command: command.to_owned(),
            arguments,
            cwd: cwd.to_owned(),
        }))
        .await
        .context("calling RegisterMcpAdapter")?
        .into_inner())
}

pub async fn list_mcp(host: &str) -> Result<Vec<McpAdapterInfo>> {
    let mut client = connect(host).await?;
    Ok(client
        .list_mcp_adapters(Request::new(ListMcpAdaptersRequest {}))
        .await
        .context("calling ListMcpAdapters")?
        .into_inner()
        .adapters)
}

pub async fn start_mcp(host: &str, adapter_id: &str) -> Result<McpAdapterInfo> {
    mcp_action(host, adapter_id, true).await
}

pub async fn stop_mcp(host: &str, adapter_id: &str) -> Result<McpAdapterInfo> {
    mcp_action(host, adapter_id, false).await
}

pub async fn remove_mcp(host: &str, adapter_id: &str) -> Result<bool> {
    let mut client = connect(host).await?;
    Ok(client
        .remove_mcp_adapter(Request::new(McpAdapterActionRequest {
            adapter_id: adapter_id.to_owned(),
        }))
        .await
        .context("calling RemoveMcpAdapter")?
        .into_inner()
        .removed)
}

async fn mcp_action(host: &str, adapter_id: &str, start: bool) -> Result<McpAdapterInfo> {
    let mut client = connect(host).await?;
    let request = Request::new(McpAdapterActionRequest {
        adapter_id: adapter_id.to_owned(),
    });
    if start {
        Ok(client
            .start_mcp_adapter(request)
            .await
            .context("calling StartMcpAdapter")?
            .into_inner())
    } else {
        Ok(client
            .stop_mcp_adapter(request)
            .await
            .context("calling StopMcpAdapter")?
            .into_inner())
    }
}

pub async fn invoke_tool(
    host: &str,
    tool_id: &str,
    arguments: std::collections::BTreeMap<String, String>,
    approved: bool,
) -> Result<InvokeToolResponse> {
    let mut client = connect(host).await?;
    let working_directory = std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    let response = client
        .invoke_tool(Request::new(InvokeToolRequest {
            tool_id: tool_id.to_owned(),
            arguments: arguments.into_iter().collect(),
            approved,
            working_directory,
        }))
        .await
        .context("calling InvokeTool")?
        .into_inner();
    Ok(response)
}

pub async fn get_security_settings(host: &str) -> Result<SecuritySettingsInfo> {
    let mut client = connect(host).await?;
    Ok(client
        .get_security_settings(Request::new(GetSecuritySettingsRequest {}))
        .await
        .context("calling GetSecuritySettings")?
        .into_inner())
}

pub async fn save_security_settings(
    host: &str,
    process_exec_allowlist: Vec<String>,
) -> Result<SecuritySettingsInfo> {
    let mut client = connect(host).await?;
    Ok(client
        .save_security_settings(Request::new(SecuritySettingsInfo {
            process_exec_allowlist,
        }))
        .await
        .context("calling SaveSecuritySettings")?
        .into_inner())
}

pub async fn list_tool_invocations(host: &str, limit: i32) -> Result<Vec<ToolInvocationInfo>> {
    let mut client = connect(host).await?;
    let response = client
        .list_tool_invocations(Request::new(ListToolInvocationsRequest { limit }))
        .await
        .context("calling ListToolInvocations")?
        .into_inner();
    Ok(response.invocations)
}

pub async fn get_knowledge_graph(
    host: &str,
    entity_filter: &str,
    depth: i32,
    limit: i32,
) -> Result<GetKnowledgeGraphResponse> {
    let mut client = connect(host).await?;
    Ok(client
        .get_knowledge_graph(Request::new(GetKnowledgeGraphRequest {
            entity_filter: entity_filter.to_owned(),
            depth,
            limit,
        }))
        .await
        .context("calling GetKnowledgeGraph")?
        .into_inner())
}

pub async fn maintenance_status(host: &str) -> Result<MaintenanceStatus> {
    let mut client = connect(host).await?;
    Ok(client
        .get_maintenance_status(Request::new(MaintenanceRequest {}))
        .await
        .context("calling GetMaintenanceStatus")?
        .into_inner())
}

pub async fn prune_now(host: &str) -> Result<PruneResponse> {
    let mut client = connect(host).await?;
    Ok(client
        .prune_now(Request::new(MaintenanceRequest {}))
        .await
        .context("calling PruneNow")?
        .into_inner())
}

fn unix_seconds() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}
