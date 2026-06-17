use anyhow::{Context as _, Result};
use steward_core::pb::steward_service_client::StewardServiceClient;
use steward_core::pb::{
    AgentInfo, ApprovePlanRequest, CancelWorkflowRequest, ExecuteTaskRequest, ListAgentsRequest,
    ListWorkflowsRequest, MemoryEntry, PingRequest, RecallMemoryRequest, StartWorkflowRequest,
    StoreMemoryRequest, WorkflowEvent, WorkflowStatus,
};
use steward_core::pb::{AgentLogEntry, GetAgentLogRequest, GetWorkflowStatusRequest};
use tonic::transport::Channel;
use tonic::Request;

pub type Client = StewardServiceClient<Channel>;

pub async fn connect(host: &str) -> Result<Client> {
    StewardServiceClient::connect(host.to_owned())
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

fn unix_seconds() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}
