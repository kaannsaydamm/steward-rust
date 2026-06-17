use steward_core::pb::{AgentLogEntry, WorkflowStatus};

pub fn status_lines(status: &WorkflowStatus) -> Vec<String> {
    let mut lines = vec![
        format!("workflow\t{}", status.workflow_id),
        format!("title\t{}", status.title),
        format!("phase\t{}", status.phase),
        format!("mode\t{}", status.mode),
        format!("progress\t{:.0}%", status.overall_progress),
        format!("agent\t{}", empty_dash(&status.current_agent)),
        format!("status\t{}", status.status_message),
        format!("approval\t{}", approval_label(status.requires_approval)),
    ];
    if !status.recent_events.is_empty() {
        lines.push("recent events".to_owned());
        for event in &status.recent_events {
            lines.push(format!(
                "- phase {} | {} | {:.0}%",
                event.phase, event.message, event.progress
            ));
        }
    }
    lines
}

pub fn summary_line(status: &WorkflowStatus) -> String {
    format!(
        "{} | {} | phase {} | mode {} | {:.0}% | {}",
        status.workflow_id,
        status.title,
        status.phase,
        status.mode,
        status.overall_progress,
        status.status_message
    )
}

pub fn log_line(entry: &AgentLogEntry) -> String {
    format!(
        "{:.3}\t{}\t{}\t{}",
        entry.timestamp, entry.level, entry.message, entry.detail
    )
}

fn approval_label(requires_approval: bool) -> &'static str {
    if requires_approval {
        "required"
    } else {
        "not required"
    }
}

fn empty_dash(value: &str) -> &str {
    if value.is_empty() {
        "-"
    } else {
        value
    }
}

#[cfg(test)]
#[path = "workflow_view_tests.rs"]
mod tests;
