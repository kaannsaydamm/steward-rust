use crate::provider_client;
use crate::MySteward;
use anyhow::{bail, Result};
use steward_core::pb::{ChatEvent, ChatEventKind, ChatRequest};
use steward_core::provider_config::ProviderSettings;
use tokio::sync::mpsc;
use tonic::Status;

const MAX_AGENT_ROUNDS: usize = 12;

mod session;
mod tools;

pub type EventSender = mpsc::Sender<std::result::Result<ChatEvent, Status>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunResult {
    pub session_id: String,
    pub final_text: String,
}

pub async fn run(
    steward: &MySteward,
    request: ChatRequest,
    sender: EventSender,
) -> Result<AgentRunResult> {
    if request.message.trim().is_empty() {
        bail!("chat message cannot be empty");
    }
    let settings = ProviderSettings::load(&steward.provider_path)?;
    let (session_id, profile) = session::prepare(steward, &settings, &request)?;
    emit(
        &sender,
        event(&session_id, ChatEventKind::Session, "session ready"),
    )
    .await;
    let tools = if request.allow_tools {
        tools::available(steward)?
    } else {
        Vec::new()
    };
    let mut final_text = String::new();
    let mut prompt_tokens = 0_u64;
    let mut completion_tokens = 0_u64;

    for _round in 0..MAX_AGENT_ROUNDS {
        let messages = session::load_messages(steward, &session_id)?;
        let reply = provider_client::complete(&steward.http, &profile, &messages, &tools).await?;
        prompt_tokens = prompt_tokens.saturating_add(reply.prompt_tokens);
        completion_tokens = completion_tokens.saturating_add(reply.completion_tokens);
        if !reply.text.is_empty() {
            session::append(steward, &session_id, "assistant", &reply.text, "", "")?;
            final_text.push_str(&reply.text);
            emit(
                &sender,
                event(&session_id, ChatEventKind::Text, &reply.text),
            )
            .await;
        }
        if reply.tool_calls.is_empty() {
            let mut done = event(&session_id, ChatEventKind::Done, &final_text);
            done.prompt_tokens = i64::try_from(prompt_tokens).unwrap_or(i64::MAX);
            done.completion_tokens = i64::try_from(completion_tokens).unwrap_or(i64::MAX);
            emit(&sender, done).await;
            return Ok(AgentRunResult {
                session_id,
                final_text,
            });
        }
        for call in reply.tool_calls {
            tools::execute(steward, &sender, &session_id, call).await?;
        }
    }
    bail!("agent exceeded {MAX_AGENT_ROUNDS} model/tool rounds")
}

pub(super) fn event(session_id: &str, kind: ChatEventKind, content: &str) -> ChatEvent {
    ChatEvent {
        event_id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_owned(),
        kind: kind as i32,
        content: content.to_owned(),
        ..ChatEvent::default()
    }
}

pub(super) async fn emit(sender: &EventSender, event: ChatEvent) {
    let _ = sender.send(Ok(event)).await;
}

#[cfg(test)]
#[path = "agent_runtime_tests.rs"]
mod tests;
