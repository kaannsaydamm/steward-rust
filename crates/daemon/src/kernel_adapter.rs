//! Kernel adapter (Task 4.3): routes chat through the steward-kernel
//! TurnEngine behind the `agent_runtime = "v1" | "omega"` config flag.
//!
//! The adapter maps v1 `ModelMessage` history to kernel messages, exposes
//! enabled tools, and translates kernel events back to v1 ChatEvents so the
//! existing RPC/Web clients keep working unchanged.

use crate::provider_client;
use crate::MySteward;
use anyhow::{Context as _, Result};
use std::sync::Arc;
use steward_core::pb::{ChatEvent, ChatEventKind};
use steward_kernel::action::AgentAction;
use steward_kernel::agent::{TurnEngine, TurnOutcome};
use steward_kernel::budget::Budget;
use steward_kernel::services::{
    KernelMessage, KernelServices, ModelRequest, ModelResponse, ToolOutcome, ToolSchema,
};
use steward_kernel::success::SuccessPolicy;
use tokio::sync::mpsc;

/// Runtime selection. `V1` keeps the legacy loop; `Omega` uses the kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeFlavor {
    V1,
    Omega,
}

impl RuntimeFlavor {
    /// Reads `agent_runtime` from the environment/config; defaults to V1.
    pub fn resolve() -> Self {
        match std::env::var("STEWARD_AGENT_RUNTIME").as_deref() {
            Ok("omega") => Self::Omega,
            _ => Self::V1,
        }
    }
}

/// Provider-client bridge implementing `ModelService` over the v1 transports.
struct ProviderModel {
    steward: MySteward,
    profile: steward_core::provider_config::ProviderProfile,
}

#[async_trait::async_trait]
impl steward_kernel::services::ModelService for ProviderModel {
    async fn complete(&self, request: ModelRequest) -> Result<ModelResponse> {
        let messages: Vec<provider_client::ModelMessage> = request
            .messages
            .iter()
            .map(|m| {
                let role = match m.role.as_str() {
                    "system" => provider_client::MessageRole::System,
                    "assistant" => provider_client::MessageRole::Assistant,
                    "tool" => provider_client::MessageRole::Tool,
                    _ => provider_client::MessageRole::User,
                };
                provider_client::ModelMessage {
                    role,
                    content: m.content.clone(),
                    tool_call_id: m.tool_call_id.clone(),
                }
            })
            .collect();
        let tools: Vec<provider_client::ToolDefinition> = request
            .tools
            .iter()
            .map(|t| provider_client::ToolDefinition {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            })
            .collect();
        let reply =
            provider_client::complete(&self.steward.http, &self.profile, &messages, &tools).await?;
        Ok(ModelResponse {
            text: reply.text,
            actions: reply
                .tool_calls
                .into_iter()
                .map(|call| AgentAction::Tool {
                    call_id: call.id,
                    name: call.name,
                    arguments: call.arguments,
                })
                .collect(),
            prompt_tokens: reply.prompt_tokens,
            completion_tokens: reply.completion_tokens,
            cost_microusd: 0,
        })
    }
}

/// Kernel tool executor dispatching to the v1 tool invocation path (Phase 7
/// replaces this with the effect-based runtime).
struct DaemonTools {
    steward: MySteward,
    working_directory: String,
}

#[async_trait::async_trait]
impl steward_kernel::services::ToolExecutor for DaemonTools {
    async fn execute(&self, action: &AgentAction) -> Result<ToolOutcome> {
        let AgentAction::Tool {
            call_id,
            name,
            arguments,
        } = action
        else {
            anyhow::bail!("non-tool action reached the tool executor");
        };
        let tool_id = name.replace("__", ".");
        let arguments: std::collections::BTreeMap<String, String> = arguments
            .as_object()
            .map(|map| {
                map.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                    .collect()
            })
            .unwrap_or_default();
        let outcome = crate::tool_invocation::invoke(
            &self.steward,
            &tool_id,
            arguments,
            false,
            &self.working_directory,
        )
        .await
        .with_context(|| format!("tool '{tool_id}' failed"))?;
        let outcome = outcome.with_context(|| format!("tool '{tool_id}' returned no outcome"))?;
        Ok(ToolOutcome {
            call_id: call_id.clone(),
            ok: true,
            observation: if outcome.output.is_empty() {
                outcome.message
            } else {
                outcome.output
            },
        })
    }
}

/// Bridges kernel events into the v1 ChatEvent stream.
struct ChatEventSink {
    session_id: String,
    sender: mpsc::Sender<std::result::Result<ChatEvent, tonic::Status>>,
}

#[async_trait::async_trait]
impl steward_kernel::services::EventSink for ChatEventSink {
    async fn emit(&self, event: steward_kernel::event::KernelEvent) {
        use steward_kernel::event::KernelEvent;
        let chat = match event {
            KernelEvent::AssistantText { text } => Some((ChatEventKind::Text, text)),
            KernelEvent::TurnStarted { turn } => {
                Some((ChatEventKind::Session, format!("turn {turn}")))
            }
            KernelEvent::RunCompleted { final_text } => Some((ChatEventKind::Done, final_text)),
            KernelEvent::RunFailed { reason } => Some((ChatEventKind::Session, reason)),
            KernelEvent::ToolStarted { name, .. } => Some((ChatEventKind::ToolStart, name)),
            _ => None,
        };
        if let Some((kind, content)) = chat {
            let event = ChatEvent {
                event_id: uuid::Uuid::new_v4().to_string(),
                session_id: self.session_id.clone(),
                kind: kind as i32,
                content,
                ..ChatEvent::default()
            };
            let _ = self.sender.send(Ok(event)).await;
        }
    }
}

/// Runs a chat turn through the Omega kernel. Mirrors the v1 `run()` signature.
pub async fn run(
    steward: &MySteward,
    session_id: &str,
    message: &str,
    working_directory: &str,
    allow_tools: bool,
    sender: crate::agent_runtime::EventSender,
) -> Result<String> {
    let mut history: Vec<KernelMessage> = Vec::new();
    {
        let connection = steward.db.lock().map_err(|_| anyhow::anyhow!("db lock"))?;
        crate::session_store::append_message(&connection, session_id, "user", message, "", "")?;
        let stored = crate::session_store::get_session(&connection, session_id)?
            .with_context(|| format!("session '{session_id}' not found"))?
            .messages;
        history = stored
            .iter()
            .map(|m| KernelMessage {
                role: m.role.clone(),
                content: m.content.clone(),
                tool_call_id: (!m.tool_call_id.is_empty()).then(|| m.tool_call_id.clone()),
            })
            .collect();
    }

    let tools: Vec<ToolSchema> = {
        let connection = steward.db.lock().map_err(|_| anyhow::anyhow!("db lock"))?;
        if allow_tools {
            crate::tool_registry::list_tools(&connection)?
                .into_iter()
                .filter(|tool| tool.enabled && !tool.requires_approval)
                .map(|tool| ToolSchema {
                    name: tool.id.replace('.', "__"),
                    description: tool.description.clone(),
                    input_schema: crate::agent_runtime::tools::schema_for(&tool.id),
                })
                .collect()
        } else {
            Vec::new()
        }
    };

    let profile = {
        let settings =
            steward_core::provider_config::ProviderSettings::load(&steward.provider_path)?;
        settings.active()?.clone()
    };

    let services = KernelServices {
        models: Arc::new(ProviderModel {
            steward: steward.clone(),
            profile,
        }),
        tools: Arc::new(DaemonTools {
            steward: steward.clone(),
            working_directory: working_directory.to_owned(),
        }),
        success: Arc::new(steward_kernel::success::PolicyEvaluator {
            check: |_| Ok(false),
        }),
        events: Arc::new(ChatEventSink {
            session_id: session_id.to_owned(),
            sender: sender.clone(),
        }),
    };

    let outcome = TurnEngine::default()
        .run(
            &services,
            history,
            tools,
            &Budget::default(),
            &SuccessPolicy::AgentDeclared,
        )
        .await?;

    match outcome {
        TurnOutcome::Completed { final_text } => {
            let connection = steward.db.lock().map_err(|_| anyhow::anyhow!("db lock"))?;
            crate::session_store::append_message(
                &connection,
                session_id,
                "assistant",
                &final_text,
                "",
                "",
            )?;
            Ok(final_text)
        }
        TurnOutcome::Failed { reason } => anyhow::bail!("{reason}"),
        TurnOutcome::CeilingExceeded => anyhow::bail!("agent exceeded the emergency ceiling"),
    }
}
