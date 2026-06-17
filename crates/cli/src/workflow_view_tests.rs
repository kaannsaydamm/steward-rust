use super::{log_line, status_lines, summary_line};
use steward_core::pb::{AgentLogEntry, WorkflowEvent, WorkflowStatus};

#[test]
fn workflow_status_lines_include_recent_events() {
    let status = WorkflowStatus {
        workflow_id: "wf-1".to_owned(),
        title: "ship steward".to_owned(),
        phase: 7,
        mode: 2,
        overall_progress: 70.0,
        current_agent: String::new(),
        status_message: "Awaiting approval".to_owned(),
        recent_events: vec![WorkflowEvent {
            workflow_id: "wf-1".to_owned(),
            phase: 7,
            agent_id: String::new(),
            message: "Phase: Awaiting Approval".to_owned(),
            detail: String::new(),
            progress: 70.0,
            requires_approval: true,
            approval: None,
        }],
        requires_approval: true,
        pending_approval: None,
    };

    let lines = status_lines(&status);

    assert!(lines.contains(&"workflow\twf-1".to_owned()));
    assert!(lines.contains(&"agent\t-".to_owned()));
    assert!(lines.contains(&"approval\trequired".to_owned()));
    assert!(lines
        .iter()
        .any(|line| line.contains("phase 7 | Phase: Awaiting Approval | 70%")));
    assert!(summary_line(&status).contains("wf-1 | ship steward | phase 7"));
}

#[test]
fn agent_log_line_is_tabular() {
    let entry = AgentLogEntry {
        timestamp: 12.34567,
        level: "info".to_owned(),
        message: "Starting phase".to_owned(),
        detail: "Workflow detail".to_owned(),
    };

    assert_eq!(
        log_line(&entry),
        "12.346\tinfo\tStarting phase\tWorkflow detail"
    );
}
