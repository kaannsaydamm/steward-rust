use crate::client;
use crate::workflow_view;
use anyhow::Result;
use tokio::time::{sleep, Duration};

#[derive(Clone, Copy, Debug)]
pub struct WatchOptions {
    pub interval_ms: u64,
    pub max_ticks: usize,
    pub keep_waiting_for_approval: bool,
}

impl WatchOptions {
    pub const fn interactive() -> Self {
        Self {
            interval_ms: 500,
            max_ticks: 12,
            keep_waiting_for_approval: false,
        }
    }
}

pub async fn collect(host: &str, workflow_id: &str, options: WatchOptions) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    let mut ticks = 0usize;

    loop {
        let status = client::get_workflow_status(host, workflow_id).await?;
        lines.push(workflow_view::summary_line(&status));
        ticks += 1;

        if is_terminal_phase(status.phase) {
            break;
        }
        if status.requires_approval && !options.keep_waiting_for_approval {
            lines.push(format!("approval required: {}", status.workflow_id));
            break;
        }
        if options.max_ticks > 0 && ticks >= options.max_ticks {
            lines.push(format!("watch stopped after {ticks} ticks"));
            break;
        }

        sleep(Duration::from_millis(options.interval_ms)).await;
    }

    Ok(lines)
}

fn is_terminal_phase(phase: i32) -> bool {
    matches!(phase, 10..=12)
}

#[cfg(test)]
#[path = "workflow_watch_tests.rs"]
mod tests;
