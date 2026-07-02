use serde::Serialize;
use serde_json::Value;
use steward_core::provider_config::{ProviderProfile, ProviderProtocol};
use thiserror::Error;

mod requests;
mod responses;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelMessage {
    pub role: MessageRole,
    pub content: String,
    pub tool_call_id: Option<String>,
}

impl ModelMessage {
    #[cfg(test)]
    pub fn new(role: MessageRole, content: &str) -> Self {
        Self {
            role,
            content: content.to_owned(),
            tool_call_id: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelReply {
    pub text: String,
    pub tool_calls: Vec<ModelToolCall>,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider profile is invalid: {0}")]
    InvalidProfile(#[source] anyhow::Error),
    #[error("provider request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("provider returned HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("provider response did not include a candidate")]
    EmptyResponse,
    #[error("provider returned invalid tool arguments: {0}")]
    InvalidToolArguments(#[source] serde_json::Error),
}

pub async fn complete(
    http: &reqwest::Client,
    profile: &ProviderProfile,
    messages: &[ModelMessage],
    tools: &[ToolDefinition],
) -> Result<ModelReply, ProviderError> {
    profile.validate().map_err(ProviderError::InvalidProfile)?;
    let api_key = profile.api_key().map_err(ProviderError::InvalidProfile)?;
    match profile.protocol {
        ProviderProtocol::OpenAiChat => {
            requests::complete_openai(http, profile, messages, tools, api_key.as_deref()).await
        }
        ProviderProtocol::AnthropicMessages => {
            requests::complete_anthropic(http, profile, messages, tools, api_key.as_deref()).await
        }
        ProviderProtocol::GeminiGenerateContent => {
            requests::complete_gemini(http, profile, messages, tools, api_key.as_deref()).await
        }
    }
}

#[cfg(test)]
#[path = "provider_client_tests.rs"]
mod tests;
