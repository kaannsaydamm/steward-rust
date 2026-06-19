use crate::{maintenance, MySteward};
use steward_core::pb::{MaintenanceRequest, MaintenanceStatus, PruneResponse};
use tonic::{Request, Response, Status};

pub async fn status(
    steward: &MySteward,
    _request: Request<MaintenanceRequest>,
) -> Result<Response<MaintenanceStatus>, Status> {
    Ok(Response::new(MaintenanceStatus {
        retention_days: i32::from(steward.retention.retention_days),
        max_completed_workflows: i32::from(steward.retention.max_completed_workflows),
    }))
}

pub async fn prune(
    steward: &MySteward,
    _request: Request<MaintenanceRequest>,
) -> Result<Response<PruneResponse>, Status> {
    let connection = steward
        .db
        .lock()
        .map_err(|_| Status::internal("Database lock failed"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| Status::internal(error.to_string()))?
        .as_secs_f64();
    let report = maintenance::prune(&connection, steward.retention, now)
        .map_err(|error| Status::internal(error.to_string()))?;
    Ok(Response::new(PruneResponse {
        tool_invocations: i64::try_from(report.tool_invocations).unwrap_or(i64::MAX),
        workflows: i64::try_from(report.workflows).unwrap_or(i64::MAX),
    }))
}
