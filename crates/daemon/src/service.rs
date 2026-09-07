use crate::rpc::{
    agent_profiles, agents, approvals, artifacts, basic, chat, cron, footer, knowledge,
    maintenance, marketplace, mcp, providers, registry, security, workflows,
};
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

    type ChatStream = chat::ChatStream;

    async fn chat(
        &self,
        request: Request<ChatRequest>,
    ) -> Result<Response<Self::ChatStream>, Status> {
        chat::start(self, request).await
    }

    async fn list_provider_catalog(
        &self,
        request: Request<ListProviderCatalogRequest>,
    ) -> Result<Response<ListProviderCatalogResponse>, Status> {
        providers::catalog(request).await
    }

    async fn list_provider_profiles(
        &self,
        request: Request<ListProviderProfilesRequest>,
    ) -> Result<Response<ListProviderProfilesResponse>, Status> {
        providers::list(self, request).await
    }

    async fn save_provider_profile(
        &self,
        request: Request<SaveProviderProfileRequest>,
    ) -> Result<Response<ProviderProfileInfo>, Status> {
        providers::save(self, request).await
    }

    async fn activate_provider_profile(
        &self,
        request: Request<ActivateProviderProfileRequest>,
    ) -> Result<Response<ProviderProfileInfo>, Status> {
        providers::activate(self, request).await
    }

    async fn delete_provider_profile(
        &self,
        request: Request<DeleteProviderProfileRequest>,
    ) -> Result<Response<DeleteProviderProfileResponse>, Status> {
        providers::delete(self, request).await
    }

    async fn list_provider_models(
        &self,
        request: Request<ListProviderModelsRequest>,
    ) -> Result<Response<ListProviderModelsResponse>, Status> {
        providers::list_models(self, request).await
    }

    async fn list_chat_sessions(
        &self,
        request: Request<ListChatSessionsRequest>,
    ) -> Result<Response<ListChatSessionsResponse>, Status> {
        chat::list_sessions(self, request).await
    }

    async fn get_chat_session(
        &self,
        request: Request<GetChatSessionRequest>,
    ) -> Result<Response<ChatSession>, Status> {
        chat::get_session(self, request).await
    }

    async fn delete_chat_session(
        &self,
        request: Request<DeleteChatSessionRequest>,
    ) -> Result<Response<DeleteChatSessionResponse>, Status> {
        chat::delete_session(self, request).await
    }

    async fn compact_chat_session(
        &self,
        request: Request<CompactChatSessionRequest>,
    ) -> Result<Response<CompactChatSessionResponse>, Status> {
        chat::compact_session(self, request).await
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

    async fn save_workflow_definition(
        &self,
        request: Request<SaveWorkflowDefinitionRequest>,
    ) -> Result<Response<WorkflowDefinition>, Status> {
        workflows::save_definition(self, request).await
    }

    async fn list_workflow_definitions(
        &self,
        request: Request<ListWorkflowDefinitionsRequest>,
    ) -> Result<Response<ListWorkflowDefinitionsResponse>, Status> {
        workflows::list_definitions(self, request).await
    }

    async fn delete_workflow_definition(
        &self,
        request: Request<DeleteWorkflowDefinitionRequest>,
    ) -> Result<Response<DeleteWorkflowDefinitionResponse>, Status> {
        workflows::delete_definition(self, request).await
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

    async fn set_tool_enabled(
        &self,
        request: Request<SetToolEnabledRequest>,
    ) -> Result<Response<ToolInfo>, Status> {
        registry::set_tool_enabled(self, request).await
    }

    async fn list_skills(
        &self,
        request: Request<ListSkillsRequest>,
    ) -> Result<Response<ListSkillsResponse>, Status> {
        registry::list_skills(self, request).await
    }

    async fn install_skill(
        &self,
        request: Request<InstallSkillRequest>,
    ) -> Result<Response<InstallSkillResponse>, Status> {
        registry::install_skill(self, request).await
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

    async fn register_mcp_adapter(
        &self,
        request: Request<RegisterMcpAdapterRequest>,
    ) -> Result<Response<McpAdapterInfo>, Status> {
        mcp::register(self, request).await
    }

    async fn list_mcp_adapters(
        &self,
        request: Request<ListMcpAdaptersRequest>,
    ) -> Result<Response<ListMcpAdaptersResponse>, Status> {
        mcp::list(self, request).await
    }

    async fn list_mcp_catalog(
        &self,
        request: Request<ListMcpCatalogRequest>,
    ) -> Result<Response<ListMcpCatalogResponse>, Status> {
        mcp::list_catalog(self, request).await
    }

    async fn start_mcp_adapter(
        &self,
        request: Request<McpAdapterActionRequest>,
    ) -> Result<Response<McpAdapterInfo>, Status> {
        mcp::start(self, request).await
    }

    async fn stop_mcp_adapter(
        &self,
        request: Request<McpAdapterActionRequest>,
    ) -> Result<Response<McpAdapterInfo>, Status> {
        mcp::stop(self, request).await
    }

    async fn remove_mcp_adapter(
        &self,
        request: Request<McpAdapterActionRequest>,
    ) -> Result<Response<RemoveMcpAdapterResponse>, Status> {
        mcp::remove(self, request).await
    }

    async fn get_maintenance_status(
        &self,
        request: Request<MaintenanceRequest>,
    ) -> Result<Response<MaintenanceStatus>, Status> {
        maintenance::status(self, request).await
    }

    async fn prune_now(
        &self,
        request: Request<MaintenanceRequest>,
    ) -> Result<Response<PruneResponse>, Status> {
        maintenance::prune(self, request).await
    }

    async fn get_security_settings(
        &self,
        request: Request<GetSecuritySettingsRequest>,
    ) -> Result<Response<SecuritySettingsInfo>, Status> {
        security::get(self, request).await
    }

    async fn save_security_settings(
        &self,
        request: Request<SecuritySettingsInfo>,
    ) -> Result<Response<SecuritySettingsInfo>, Status> {
        security::save(self, request).await
    }

    async fn list_cron_jobs(
        &self,
        request: Request<ListCronJobsRequest>,
    ) -> Result<Response<ListCronJobsResponse>, Status> {
        cron::list(self, request).await
    }

    async fn create_cron_job(
        &self,
        request: Request<CreateCronJobRequest>,
    ) -> Result<Response<CronJobInfo>, Status> {
        cron::create(self, request).await
    }

    async fn set_cron_job_enabled(
        &self,
        request: Request<SetCronJobEnabledRequest>,
    ) -> Result<Response<CronJobInfo>, Status> {
        cron::set_enabled(self, request).await
    }

    async fn delete_cron_job(
        &self,
        request: Request<DeleteCronJobRequest>,
    ) -> Result<Response<DeleteCronJobResponse>, Status> {
        cron::delete(self, request).await
    }

    async fn run_cron_job_now(
        &self,
        request: Request<RunCronJobNowRequest>,
    ) -> Result<Response<CronJobInfo>, Status> {
        cron::run_now(self, request).await
    }

    async fn list_file_checkpoints(
        &self,
        request: Request<ListFileCheckpointsRequest>,
    ) -> Result<Response<ListFileCheckpointsResponse>, Status> {
        registry::list_file_checkpoints(self, request).await
    }

    async fn rollback_file_checkpoint(
        &self,
        request: Request<RollbackFileCheckpointRequest>,
    ) -> Result<Response<RollbackFileCheckpointResponse>, Status> {
        registry::rollback_file_checkpoint(self, request).await
    }

    async fn create_artifact(
        &self,
        request: Request<CreateArtifactRequest>,
    ) -> Result<Response<ArtifactInfo>, Status> {
        artifacts::create(self, request).await
    }

    async fn list_artifacts(
        &self,
        request: Request<ListArtifactsRequest>,
    ) -> Result<Response<ListArtifactsResponse>, Status> {
        artifacts::list(self, request).await
    }

    async fn get_artifact(
        &self,
        request: Request<GetArtifactRequest>,
    ) -> Result<Response<ArtifactInfo>, Status> {
        artifacts::get(self, request).await
    }

    async fn delete_artifact(
        &self,
        request: Request<DeleteArtifactRequest>,
    ) -> Result<Response<DeleteArtifactResponse>, Status> {
        artifacts::delete(self, request).await
    }

    async fn search_connector_marketplace(
        &self,
        request: Request<SearchConnectorMarketplaceRequest>,
    ) -> Result<Response<SearchConnectorMarketplaceResponse>, Status> {
        marketplace::search_connectors(self, request).await
    }

    async fn search_skill_marketplace(
        &self,
        request: Request<SearchSkillMarketplaceRequest>,
    ) -> Result<Response<SearchSkillMarketplaceResponse>, Status> {
        marketplace::search_skills(self, request).await
    }

    async fn install_skill_marketplace_entry(
        &self,
        request: Request<InstallSkillMarketplaceEntryRequest>,
    ) -> Result<Response<ArtifactInfo>, Status> {
        marketplace::install_skill(self, request).await
    }

    async fn list_pending_approvals(
        &self,
        request: Request<ListPendingApprovalsRequest>,
    ) -> Result<Response<ListPendingApprovalsResponse>, Status> {
        approvals::list_pending(self, request).await
    }

    async fn respond_approval(
        &self,
        request: Request<RespondApprovalRequest>,
    ) -> Result<Response<RespondApprovalResponse>, Status> {
        approvals::respond(self, request).await
    }

    async fn list_slash_commands(
        &self,
        request: Request<ListSlashCommandsRequest>,
    ) -> Result<Response<ListSlashCommandsResponse>, Status> {
        Ok(Response::new(ListSlashCommandsResponse {
            commands: steward_harness::slash::rpc_catalog(),
        }))
    }

    async fn get_footer_data(
        &self,
        request: Request<GetFooterDataRequest>,
    ) -> Result<Response<FooterData>, Status> {
        footer::get_footer_data(self, request).await
    }

    async fn save_agent_profile(
        &self,
        request: Request<SaveAgentProfileRequest>,
    ) -> Result<Response<AgentProfileInfo>, Status> {
        agent_profiles::save(self, request).await
    }

    async fn delete_agent_profile(
        &self,
        request: Request<DeleteAgentProfileRequest>,
    ) -> Result<Response<DeleteAgentProfileResponse>, Status> {
        agent_profiles::delete(self, request).await
    }

    async fn list_agent_profiles(
        &self,
        request: Request<ListAgentProfilesRequest>,
    ) -> Result<Response<ListAgentProfilesResponse>, Status> {
        agent_profiles::list(self, request).await
    }

    async fn import_agent_profile(
        &self,
        request: Request<ImportAgentProfileRequest>,
    ) -> Result<Response<AgentProfileInfo>, Status> {
        agent_profiles::import(self, request).await
    }

    async fn export_agent_profile(
        &self,
        request: Request<ExportAgentProfileRequest>,
    ) -> Result<Response<ExportAgentProfileResponse>, Status> {
        agent_profiles::export(self, request).await
    }
}
