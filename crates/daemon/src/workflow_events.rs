use std::time::{SystemTime, UNIX_EPOCH};
use steward_core::pb::{ApprovalRequest, WorkflowEvent};

pub fn workflow_event(
    workflow_id: &str,
    title: &str,
    description: &str,
    phase: i32,
    phase_name: &str,
    agent_id: &str,
    progress: f32,
) -> WorkflowEvent {
    WorkflowEvent {
        workflow_id: workflow_id.to_owned(),
        phase,
        agent_id: agent_id.to_owned(),
        message: format!("Phase: {phase_name}"),
        detail: format!("Entering {phase_name} phase for workflow '{title}'"),
        progress,
        requires_approval: phase == 7,
        approval: approval_request(title, description, phase),
    }
}

pub fn cancelled_event(workflow_id: &str) -> WorkflowEvent {
    WorkflowEvent {
        workflow_id: workflow_id.to_owned(),
        phase: 12,
        agent_id: String::new(),
        message: "Workflow Cancelled".into(),
        detail: "Workflow was cancelled by user.".into(),
        progress: 0.0,
        requires_approval: false,
        approval: None,
    }
}

pub fn current_unix_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

fn approval_request(title: &str, description: &str, phase: i32) -> Option<ApprovalRequest> {
    if phase != 7 {
        return None;
    }
    Some(ApprovalRequest {
        title: "Plan Approval Required".into(),
        description: format!("Review the execution plan for: {title}"),
        options: vec!["Parallel".into(), "Sequential".into(), "Hybrid".into()],
        plan_summary: format!("Execution plan for: {description}"),
        recommended_mode: 0,
    })
}
