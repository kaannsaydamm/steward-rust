use crate::MySteward;
use futures_util::Stream;
use std::pin::Pin;
use steward_core::pb::*;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

pub type ResponseStream = Pin<Box<dyn Stream<Item = Result<WorkflowEvent, Status>> + Send>>;

pub async fn start(
    steward: &MySteward,
    request: Request<StartWorkflowRequest>,
) -> Result<Response<ResponseStream>, Status> {
    let (tx, rx) = mpsc::channel(128);
    steward
        .workflow_runtime()
        .start(request.into_inner(), tx)
        .await?;
    Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
}

pub async fn status(
    steward: &MySteward,
    request: Request<GetWorkflowStatusRequest>,
) -> Result<Response<WorkflowStatus>, Status> {
    let workflow_id = request.into_inner().workflow_id;
    let workflows = steward.workflows.lock().await;
    let state = workflows
        .get(&workflow_id)
        .ok_or_else(|| Status::not_found(format!("Workflow '{workflow_id}' not found")))?;
    Ok(Response::new(state.status.clone()))
}

pub async fn list(
    steward: &MySteward,
    _request: Request<ListWorkflowsRequest>,
) -> Result<Response<ListWorkflowsResponse>, Status> {
    let workflows = steward.workflows.lock().await;
    let workflows = workflows
        .values()
        .map(|state| state.status.clone())
        .collect();
    Ok(Response::new(ListWorkflowsResponse { workflows }))
}

pub async fn cancel(
    steward: &MySteward,
    request: Request<CancelWorkflowRequest>,
) -> Result<Response<CancelWorkflowResponse>, Status> {
    let req = request.into_inner();
    let mut workflows = steward.workflows.lock().await;
    let Some(state) = workflows.get_mut(&req.workflow_id) else {
        return Ok(Response::new(CancelWorkflowResponse { cancelled: false }));
    };
    state.cancelled = true;
    state.status.phase = 12;
    state.status.status_message = format!("Cancelled: {}", req.reason);
    let snapshot = state.clone();
    drop(workflows);
    steward
        .workflow_runtime()
        .persist_workflow_state(&snapshot)?;
    Ok(Response::new(CancelWorkflowResponse { cancelled: true }))
}

pub async fn approve(
    steward: &MySteward,
    request: Request<ApprovePlanRequest>,
) -> Result<Response<ApprovePlanResponse>, Status> {
    let req = request.into_inner();
    let mut workflows = steward.workflows.lock().await;
    let state = workflows
        .get_mut(&req.workflow_id)
        .ok_or_else(|| Status::not_found(format!("Workflow '{}' not found", req.workflow_id)))?;
    state.approved = req.approved;
    state.mode = req.mode;
    state.status.requires_approval = false;
    state.status.pending_approval = None;
    if req.approved {
        state.status.mode = req.mode;
        state.status.status_message = format!("Plan approved — executing in mode {}", req.mode);
    } else {
        state.status.status_message = format!("Plan rejected: {}", req.feedback);
    }
    let snapshot = state.clone();
    let message = state.status.status_message.clone();
    drop(workflows);
    steward
        .workflow_runtime()
        .persist_workflow_state(&snapshot)?;
    Ok(Response::new(ApprovePlanResponse {
        accepted: req.approved,
        message,
    }))
}
