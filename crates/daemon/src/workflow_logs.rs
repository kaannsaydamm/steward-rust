use crate::workflow_events::current_unix_seconds;
use std::collections::HashMap;
use std::sync::Arc;
use steward_core::pb::AgentLogEntry;

pub async fn log_agent_phase(
    agent_logs: &Arc<tokio::sync::Mutex<HashMap<String, Vec<AgentLogEntry>>>>,
    workflow_id: &str,
    title: &str,
    phase_name: &str,
    agent_id: &str,
) {
    if agent_id.is_empty() {
        return;
    }
    let mut logs = agent_logs.lock().await;
    let key = format!("{workflow_id}/{agent_id}");
    logs.entry(key)
        .or_insert_with(Vec::new)
        .push(AgentLogEntry {
            timestamp: current_unix_seconds(),
            level: "info".into(),
            message: format!("Starting phase: {phase_name}"),
            detail: format!("Workflow: {title} - Phase: {phase_name}"),
        });
}
