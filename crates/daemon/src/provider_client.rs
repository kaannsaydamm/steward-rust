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

pub async fn list_models(
    http: &reqwest::Client,
    protocol: ProviderProtocol,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<String>, ProviderError> {
    let base = base_url.trim_end_matches('/');
    let (url, header_name) = match protocol {
        ProviderProtocol::OpenAiChat => (format!("{base}/models"), reqwest::header::AUTHORIZATION),
        ProviderProtocol::AnthropicMessages => {
            let path = if base.ends_with("/v1") {
                "models"
            } else {
                "v1/models"
            };
            (
                format!("{base}/{path}"),
                reqwest::header::HeaderName::from_static("x-api-key"),
            )
        }
        ProviderProtocol::GeminiGenerateContent => {
            let path = if base.contains("v1beta") || base.ends_with("/v1") {
                "models"
            } else {
                "v1beta/models"
            };
            (
                format!("{base}/{path}"),
                reqwest::header::HeaderName::from_static("x-goog-api-key"),
            )
        }
    };
    let mut request = http.get(&url);
    if protocol == ProviderProtocol::AnthropicMessages {
        request = request.header("anthropic-version", "2023-06-01");
    }
    if let Some(key) = api_key {
        let header_value = match protocol {
            ProviderProtocol::OpenAiChat => format!("Bearer {key}"),
            ProviderProtocol::AnthropicMessages | ProviderProtocol::GeminiGenerateContent => {
                key.to_owned()
            }
        };
        let header_value =
            reqwest::header::HeaderValue::from_str(&header_value).map_err(|error| {
                ProviderError::InvalidProfile(anyhow::anyhow!("invalid api key value: {error}"))
            })?;
        request = request.header(header_name, header_value);
    }
    let response = request.send().await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await?.chars().take(2048).collect();
        return Err(ProviderError::Http {
            status: status.as_u16(),
            body,
        });
    }
    let payload: Value = response.json().await?;
    Ok(parse_model_ids(protocol, &payload))
}

fn parse_model_ids(protocol: ProviderProtocol, payload: &Value) -> Vec<String> {
    match protocol {
        ProviderProtocol::OpenAiChat | ProviderProtocol::AnthropicMessages => payload["data"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item["id"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        ProviderProtocol::GeminiGenerateContent => payload["models"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item["name"].as_str())
                    .map(|name| name.trim_start_matches("models/").to_owned())
                    .collect()
            })
            .unwrap_or_default(),
    }
}

#[cfg(test)]
#[path = "provider_client_tests.rs"]
mod tests;
