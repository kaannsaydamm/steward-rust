use crate::tool_audit::{self, InvocationStatus, NewInvocation};
use crate::tool_executors;
use crate::tool_policy::{self, PolicyDecision};
use crate::tool_registry;
use crate::MySteward;
use anyhow::{Context, Result};
use std::collections::BTreeMap;

pub struct InvocationOutcome {
    pub invocation_id: String,
    pub status: InvocationStatus,
    pub output: String,
    pub message: String,
    pub requires_approval: bool,
}

struct OutcomeDraft {
    status: InvocationStatus,
    output: String,
    message: String,
    requires_approval: bool,
}

pub async fn invoke(
    steward: &MySteward,
    tool_id: &str,
    arguments: BTreeMap<String, String>,
    approved: bool,
    working_directory: &str,
) -> Result<Option<InvocationOutcome>> {
    let tool = {
        let connection = steward
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
        tool_registry::get_tool(&connection, tool_id)?
    };
    let Some(tool) = tool else {
        return Ok(None);
    };
    let input_json = serde_json::to_string(&arguments).context("serializing tool arguments")?;
    match tool_policy::evaluate(&tool, approved) {
        PolicyDecision::DeniedDisabled => record_outcome(
            steward,
            tool_id,
            approved,
            &input_json,
            OutcomeDraft {
                status: InvocationStatus::Denied,
                output: String::new(),
                message: "tool is disabled".to_owned(),
                requires_approval: false,
            },
        ),
        PolicyDecision::RequiresApproval => record_outcome(
            steward,
            tool_id,
            approved,
            &input_json,
            OutcomeDraft {
                status: InvocationStatus::PendingApproval,
                output: String::new(),
                message: "explicit approval required".to_owned(),
                requires_approval: true,
            },
        ),
        PolicyDecision::Allowed => {
            let execution = if tool_id == "wasm.run" {
                crate::wasm_sandbox::execute_workspace_file(
                    &steward.wasm_engine,
                    &arguments,
                    working_directory,
                )
                .await
            } else {
                tool_executors::execute(steward, tool_id, &arguments, working_directory).await
            };
            match execution {
                Ok(output) => record_outcome(
                    steward,
                    tool_id,
                    approved,
                    &input_json,
                    OutcomeDraft {
                        status: InvocationStatus::Succeeded,
                        output: steward_core::secret_redaction::redact(&output),
                        message: "tool invocation succeeded".to_owned(),
                        requires_approval: false,
                    },
                ),
                Err(error) => record_outcome(
                    steward,
                    tool_id,
                    approved,
                    &input_json,
                    OutcomeDraft {
                        status: InvocationStatus::Failed,
                        output: String::new(),
                        message: format!("{error:#}"),
                        requires_approval: false,
                    },
                ),
            }
        }
    }
    .map(Some)
}

fn record_outcome(
    steward: &MySteward,
    tool_id: &str,
    approved: bool,
    input_json: &str,
    draft: OutcomeDraft,
) -> Result<InvocationOutcome> {
    let connection = steward
        .db
        .lock()
        .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
    let invocation_id = tool_audit::record(
        &connection,
        NewInvocation {
            tool_id,
            approved,
            input_json: truncate_utf8(input_json, 16_384),
            status: draft.status,
            output: truncate_utf8(&draft.output, 4_096),
            error: if draft.status == InvocationStatus::Succeeded {
                ""
            } else {
                truncate_utf8(&draft.message, 2_048)
            },
        },
    )?;
    Ok(InvocationOutcome {
        invocation_id,
        status: draft.status,
        output: draft.output,
        message: draft.message,
        requires_approval: draft.requires_approval,
    })
}

fn truncate_utf8(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

#[cfg(test)]
#[path = "tool_invocation_tests.rs"]
mod tests;
