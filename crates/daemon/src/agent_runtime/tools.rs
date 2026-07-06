use super::session;
use super::{emit, event, EventSender};
use crate::provider_client::{ModelToolCall, ToolDefinition};
use crate::{tool_invocation, tool_registry, MySteward};
use anyhow::{Context as _, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use steward_core::pb::ChatEventKind;

pub(super) fn available(steward: &MySteward) -> Result<Vec<ToolDefinition>> {
    let connection = steward
        .db
        .lock()
        .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
    Ok(tool_registry::list_tools(&connection)?
        .into_iter()
        .filter(|tool| tool.enabled && !tool.requires_approval)
        .map(|tool| ToolDefinition {
            name: tool.id.replace('.', "__"),
            description: tool.description,
            input_schema: schema_for(&tool.id),
        })
        .collect())
}

pub(super) async fn execute(
    steward: &MySteward,
    sender: &EventSender,
    session_id: &str,
    call: ModelToolCall,
    working_directory: &str,
) -> Result<()> {
    let tool_id = call.name.replace("__", ".");
    let mut start = event(
        session_id,
        ChatEventKind::ToolStart,
        "tool invocation started",
    );
    start.tool_name = tool_id.clone();
    start.tool_call_id = call.id.clone();
    start.arguments_json = call.arguments.to_string();
    emit(sender, start).await;
    let outcome = tool_invocation::invoke(
        steward,
        &tool_id,
        argument_map(&call.arguments)?,
        false,
        working_directory,
    )
    .await?
    .with_context(|| format!("model requested unknown tool '{tool_id}'"))?;
    let content = if outcome.output.is_empty() {
        outcome.message
    } else {
        outcome.output
    };
    session::append(steward, session_id, "tool", &content, &tool_id, &call.id)?;
    session::append(
        steward,
        session_id,
        "user",
        &format!("Tool {tool_id} returned:\n{content}"),
        "",
        "",
    )?;
    let mut result = event(session_id, ChatEventKind::ToolResult, &content);
    result.tool_name = tool_id;
    result.tool_call_id = call.id;
    result.is_error = outcome.status != crate::tool_audit::InvocationStatus::Succeeded;
    emit(sender, result).await;
    Ok(())
}

fn argument_map(value: &Value) -> Result<BTreeMap<String, String>> {
    let object = value
        .as_object()
        .context("tool arguments must be a JSON object")?;
    Ok(object
        .iter()
        .map(|(key, value)| {
            let value = value
                .as_str()
                .map_or_else(|| value.to_string(), str::to_owned);
            (key.clone(), value)
        })
        .collect())
}

fn schema_for(tool_id: &str) -> Value {
    match tool_id {
        "memory.recall" => {
            json!({"type":"object","properties":{"query":{"type":"string"},"limit":{"type":"integer"}},"required":["query"]})
        }
        "memory.store" => {
            json!({"type":"object","properties":{"content":{"type":"string"}},"required":["content"]})
        }
        "workflow.inspect" => {
            json!({"type":"object","properties":{"workflow_id":{"type":"string"}},"required":["workflow_id"]})
        }
        "fs.read" => {
            json!({"type":"object","properties":{"path":{"type":"string","description":"path relative to the working directory"}},"required":["path"]})
        }
        "fs.search" => {
            json!({"type":"object","properties":{"query":{"type":"string","description":"substring to match against file names"}},"required":["query"]})
        }
        "process.exec" => {
            json!({"type":"object","properties":{"command":{"type":"string","description":"shell command line to execute"}},"required":["command"]})
        }
        _ => json!({"type":"object","additionalProperties":true}),
    }
}
