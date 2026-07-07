use crate::mcp_catalog;
use crate::mcp_lifecycle::{self, AdapterInfo};
use crate::mcp_registry::AdapterConfig;
use crate::mcp_runtime::AdapterState;
use crate::MySteward;
use steward_core::pb::{
    ListMcpAdaptersRequest, ListMcpAdaptersResponse, ListMcpCatalogRequest, ListMcpCatalogResponse,
    McpAdapterActionRequest, McpAdapterInfo, McpCatalogEntry, RegisterMcpAdapterRequest,
    RemoveMcpAdapterResponse,
};
use tonic::{Request, Response, Status};

pub async fn register(
    steward: &MySteward,
    request: Request<RegisterMcpAdapterRequest>,
) -> Result<Response<McpAdapterInfo>, Status> {
    let request = request.into_inner();
    let info = mcp_lifecycle::register(
        steward,
        AdapterConfig {
            id: request.adapter_id,
            name: request.name,
            command: request.command,
            args: request.arguments,
            cwd: (!request.cwd.is_empty()).then_some(request.cwd),
        },
    )
    .await
    .map_err(|error| Status::invalid_argument(error.to_string()))?;
    Ok(Response::new(to_proto(info)))
}

pub async fn list(
    steward: &MySteward,
    _request: Request<ListMcpAdaptersRequest>,
) -> Result<Response<ListMcpAdaptersResponse>, Status> {
    let adapters = mcp_lifecycle::list(steward)
        .await
        .map_err(|error| Status::internal(error.to_string()))?
        .into_iter()
        .map(to_proto)
        .collect();
    Ok(Response::new(ListMcpAdaptersResponse { adapters }))
}

pub async fn start(
    steward: &MySteward,
    request: Request<McpAdapterActionRequest>,
) -> Result<Response<McpAdapterInfo>, Status> {
    let id = request.into_inner().adapter_id;
    let info = mcp_lifecycle::start(steward, &id)
        .await
        .map_err(|error| Status::failed_precondition(error.to_string()))?;
    Ok(Response::new(to_proto(info)))
}

pub async fn stop(
    steward: &MySteward,
    request: Request<McpAdapterActionRequest>,
) -> Result<Response<McpAdapterInfo>, Status> {
    let id = request.into_inner().adapter_id;
    let info = mcp_lifecycle::stop(steward, &id)
        .await
        .map_err(|error| Status::failed_precondition(error.to_string()))?;
    Ok(Response::new(to_proto(info)))
}

pub async fn remove(
    steward: &MySteward,
    request: Request<McpAdapterActionRequest>,
) -> Result<Response<RemoveMcpAdapterResponse>, Status> {
    let id = request.into_inner().adapter_id;
    let removed = mcp_lifecycle::remove(steward, &id)
        .await
        .map_err(|error| Status::internal(error.to_string()))?;
    Ok(Response::new(RemoveMcpAdapterResponse { removed }))
}

pub async fn list_catalog(
    _steward: &MySteward,
    _request: Request<ListMcpCatalogRequest>,
) -> Result<Response<ListMcpCatalogResponse>, Status> {
    let entries = mcp_catalog::entries()
        .into_iter()
        .map(|entry| McpCatalogEntry {
            catalog_id: entry.catalog_id.to_owned(),
            name: entry.name.to_owned(),
            publisher: entry.publisher.to_owned(),
            description: entry.description.to_owned(),
            command: entry.command.to_owned(),
            args: entry.args.iter().map(|arg| (*arg).to_owned()).collect(),
        })
        .collect();
    Ok(Response::new(ListMcpCatalogResponse { entries }))
}

fn to_proto(info: AdapterInfo) -> McpAdapterInfo {
    McpAdapterInfo {
        adapter_id: info.config.id,
        name: info.config.name,
        command: info.config.command,
        arguments: info.config.args,
        cwd: info.config.cwd.unwrap_or_default(),
        status: match info.status.state {
            AdapterState::Stopped => "stopped",
            AdapterState::Running => "running",
            AdapterState::Failed => "failed",
        }
        .to_owned(),
        server_name: info.status.server_name,
        server_version: info.status.server_version,
        tool_count: i32::try_from(info.status.tool_count).unwrap_or(i32::MAX),
        error: info.status.error,
    }
}
