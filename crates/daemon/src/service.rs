use crate::rpc::{agents, basic, knowledge, registry, workflows};
use crate::MySteward;
use steward_core::pb::steward_service_server::StewardService;
use steward_core::pb::*;
use tonic::{Request, Response, Status};

#[tonic::async_trait]
impl StewardService for MySteward {
    async fn ping(&self, request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        basic::ping(request).await
    }

    async fn run_plugin(
        &self,
        request: Request<RunPluginRequest>,
    ) -> Result<Response<RunPluginResponse>, Status> {
        basic::run_plugin(self, request).await
    }

    async fn execute_task(
        &self,
        request: Request<ExecuteTaskRequest>,
    ) -> Result<Response<ExecuteTaskResponse>, Status> {
        basic::execute_task(self, request).await
    }

    async fn store_memory(
        &self,
        request: Request<StoreMemoryRequest>,
    ) -> Result<Response<StoreMemoryResponse>, Status> {
        knowledge::store_memory(self, request).await
    }

    async fn recall_memory(
        &self,
        request: Request<RecallMemoryRequest>,
    ) -> Result<Response<RecallMemoryResponse>, Status> {
        knowledge::recall_memory(self, request).await
    }

    async fn graph_query(
        &self,
        request: Request<GraphQueryRequest>,
    ) -> Result<Response<GraphQueryResponse>, Status> {
        knowledge::graph_query(self, request).await
    }

    async fn get_knowledge_graph(
        &self,
        request: Request<GetKnowledgeGraphRequest>,
    ) -> Result<Response<GetKnowledgeGraphResponse>, Status> {
        knowledge::get_knowledge_graph(self, request).await
    }

    type StartWorkflowStream = workflows::ResponseStream;

    async fn start_workflow(
        &self,
        request: Request<StartWorkflowRequest>,
    ) -> Result<Response<Self::StartWorkflowStream>, Status> {
        workflows::start(self, request).await
    }

    async fn get_workflow_status(
        &self,
        request: Request<GetWorkflowStatusRequest>,
    ) -> Result<Response<WorkflowStatus>, Status> {
        workflows::status(self, request).await
    }

    async fn list_workflows(
        &self,
        request: Request<ListWorkflowsRequest>,
    ) -> Result<Response<ListWorkflowsResponse>, Status> {
        workflows::list(self, request).await
    }

    async fn cancel_workflow(
        &self,
        request: Request<CancelWorkflowRequest>,
    ) -> Result<Response<CancelWorkflowResponse>, Status> {
        workflows::cancel(self, request).await
    }

    async fn approve_plan(
        &self,
        request: Request<ApprovePlanRequest>,
    ) -> Result<Response<ApprovePlanResponse>, Status> {
        workflows::approve(self, request).await
    }

    async fn list_agents(
        &self,
        request: Request<ListAgentsRequest>,
    ) -> Result<Response<ListAgentsResponse>, Status> {
        agents::list(self, request).await
    }

    type GetAgentLogStream = agents::LogStream;

    async fn get_agent_log(
        &self,
        request: Request<GetAgentLogRequest>,
    ) -> Result<Response<Self::GetAgentLogStream>, Status> {
        agents::logs(self, request).await
    }

    async fn list_tools(
        &self,
        request: Request<ListToolsRequest>,
    ) -> Result<Response<ListToolsResponse>, Status> {
        registry::list_tools(self, request).await
    }

    async fn list_skills(
        &self,
        request: Request<ListSkillsRequest>,
    ) -> Result<Response<ListSkillsResponse>, Status> {
        registry::list_skills(self, request).await
    }

    async fn invoke_tool(
        &self,
        request: Request<InvokeToolRequest>,
    ) -> Result<Response<InvokeToolResponse>, Status> {
        registry::invoke_tool(self, request).await
    }

    async fn list_tool_invocations(
        &self,
        request: Request<ListToolInvocationsRequest>,
    ) -> Result<Response<ListToolInvocationsResponse>, Status> {
        registry::list_invocations(self, request).await
    }
}
