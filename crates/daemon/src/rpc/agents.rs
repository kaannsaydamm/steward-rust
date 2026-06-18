use crate::{state, MySteward};
use futures_util::Stream;
use std::pin::Pin;
use steward_core::pb::*;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

pub type LogStream = Pin<Box<dyn Stream<Item = Result<AgentLogEntry, Status>> + Send>>;

pub async fn list(
    steward: &MySteward,
    _request: Request<ListAgentsRequest>,
) -> Result<Response<ListAgentsResponse>, Status> {
    let agents = steward.agents.lock().await;
    let agents = agents.iter().map(state::InternalAgent::to_proto).collect();
    Ok(Response::new(ListAgentsResponse { agents }))
}

pub async fn logs(
    steward: &MySteward,
    request: Request<GetAgentLogRequest>,
) -> Result<Response<LogStream>, Status> {
    let req = request.into_inner();
    let key = format!("{}/{}", req.workflow_id, req.agent_id);
    let entries = steward
        .agent_logs
        .lock()
        .await
        .get(&key)
        .cloned()
        .unwrap_or_default();
    let (tx, rx) = mpsc::channel(128);
    tokio::spawn(async move {
        for entry in entries {
            if tx.send(Ok(entry)).await.is_err() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });
    Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
}
