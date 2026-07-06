use crate::provider_client::{MessageRole, ModelMessage};
use crate::{session_store, MySteward};
use anyhow::{bail, Context as _, Result};
use steward_core::pb::ChatRequest;
use steward_core::provider_config::{ProviderProfile, ProviderSettings};

const SYSTEM_PROMPT: &str = "You are Steward, a local-first software operator. Use available tools only when they materially help. Never claim a tool ran unless its result is present. Keep responses direct and evidence-based.";

pub(super) fn prepare(
    steward: &MySteward,
    settings: &ProviderSettings,
    request: &ChatRequest,
) -> Result<(String, ProviderProfile)> {
    let connection = steward
        .db
        .lock()
        .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
    let (session_id, profile) = if request.session_id.is_empty() {
        let profile = settings.active()?.clone();
        let session = session_store::create_session(
            &connection,
            &profile.profile_id,
            &profile.model,
            &request.message,
        )?;
        session_store::append_message(
            &connection,
            &session.session_id,
            "system",
            SYSTEM_PROMPT,
            "",
            "",
        )?;
        (session.session_id, profile)
    } else {
        let session = session_store::get_session(&connection, &request.session_id)?
            .with_context(|| format!("chat session '{}' not found", request.session_id))?;
        let profile = settings
            .get(&session.summary.provider_profile)
            .with_context(|| {
                format!(
                    "provider profile '{}' used by session is missing",
                    session.summary.provider_profile
                )
            })?
            .clone();
        (session.summary.session_id, profile)
    };
    session_store::append_message(
        &connection,
        &session_id,
        "user",
        request.message.trim(),
        "",
        "",
    )?;
    Ok((session_id, profile))
}

pub(super) fn load_messages(steward: &MySteward, session_id: &str) -> Result<Vec<ModelMessage>> {
    let connection = steward
        .db
        .lock()
        .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
    let session = session_store::get_session(&connection, session_id)?
        .with_context(|| format!("chat session '{session_id}' disappeared"))?;
    session
        .messages
        .into_iter()
        .map(|message| {
            let role = match message.role.as_str() {
                "system" => MessageRole::System,
                "user" => MessageRole::User,
                "assistant" => MessageRole::Assistant,
                "tool" => MessageRole::Tool,
                other => bail!("unknown stored message role '{other}'"),
            };
            Ok(ModelMessage {
                role,
                content: message.content,
                tool_call_id: (!message.tool_call_id.is_empty()).then_some(message.tool_call_id),
            })
        })
        .collect()
}

pub(super) fn append(
    steward: &MySteward,
    session_id: &str,
    role: &str,
    content: &str,
    tool_name: &str,
    tool_call_id: &str,
) -> Result<()> {
    let connection = steward
        .db
        .lock()
        .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
    session_store::append_message(
        &connection,
        session_id,
        role,
        content,
        tool_name,
        tool_call_id,
    )?;
    Ok(())
}
