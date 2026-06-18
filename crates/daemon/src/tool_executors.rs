use crate::MySteward;
use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};
use steward_knowledge::{MemoryEntry, MemoryType};

pub async fn execute(
    steward: &MySteward,
    tool_id: &str,
    arguments: &BTreeMap<String, String>,
) -> Result<String> {
    match tool_id {
        "memory.recall" => recall_memory(steward, arguments),
        "memory.store" => store_memory(steward, arguments),
        "workflow.inspect" => inspect_workflow(steward, arguments).await,
        _ if tool_id.starts_with("mcp.") => {
            crate::mcp_lifecycle::invoke(steward, tool_id, arguments).await
        }
        _ => bail!("no executor registered for {tool_id}"),
    }
}

fn recall_memory(steward: &MySteward, arguments: &BTreeMap<String, String>) -> Result<String> {
    let query = arguments.get("query").map_or("", String::as_str);
    let limit = arguments
        .get("limit")
        .map_or(Ok(10_usize), |value| value.parse::<usize>())
        .context("limit must be a positive integer")?
        .clamp(1, 50);
    let memories = steward.knowledge.recall_memory(query, None, limit)?;
    if memories.is_empty() {
        return Ok("no memories".to_owned());
    }
    Ok(memories
        .into_iter()
        .map(|memory| format!("{}\t{}", memory.id, memory.content.replace('\n', " ")))
        .collect::<Vec<_>>()
        .join("\n"))
}

fn store_memory(steward: &MySteward, arguments: &BTreeMap<String, String>) -> Result<String> {
    let content = arguments
        .get("content")
        .map(String::as_str)
        .filter(|content| !content.trim().is_empty())
        .context("content is required")?;
    let mut metadata = HashMap::new();
    metadata.insert("source".to_owned(), "tool-invocation".to_owned());
    let id = steward.knowledge.store_memory(MemoryEntry {
        id: String::new(),
        memory_type: MemoryType::LongTerm,
        content: content.to_owned(),
        metadata,
        entities: Vec::new(),
        timestamp: unix_seconds(),
    })?;
    Ok(format!("remembered\t{id}"))
}

async fn inspect_workflow(
    steward: &MySteward,
    arguments: &BTreeMap<String, String>,
) -> Result<String> {
    let workflow_id = arguments
        .get("workflow_id")
        .map(String::as_str)
        .context("workflow_id is required")?;
    let workflows = steward.workflows.lock().await;
    let state = workflows
        .get(workflow_id)
        .with_context(|| format!("workflow {workflow_id} not found"))?;
    Ok(format!(
        "{}\tphase={}\tmode={}\tprogress={:.0}%\t{}",
        state.status.workflow_id,
        state.status.phase,
        state.status.mode,
        state.status.overall_progress,
        state.status.status_message
    ))
}

fn unix_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}
