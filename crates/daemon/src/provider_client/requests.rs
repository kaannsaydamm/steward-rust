use super::responses::{parse_anthropic, parse_gemini, parse_openai};
use super::{MessageRole, ModelMessage, ModelReply, ProviderError, ToolDefinition};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};
use steward_core::provider_config::ProviderProfile;

pub(super) async fn complete_openai(
    http: &reqwest::Client,
    profile: &ProviderProfile,
    messages: &[ModelMessage],
    tools: &[ToolDefinition],
    api_key: Option<&str>,
) -> Result<ModelReply, ProviderError> {
    let body = json!({
        "model": profile.model,
        "messages": messages,
        "tools": tools.iter().map(openai_tool).collect::<Vec<_>>()
    });
    let response = checked_json(
        http.post(join_url(&profile.base_url, "chat/completions"))
            .headers(common_headers(api_key, AuthStyle::Bearer)?)
            .json(&body)
            .send()
            .await?,
    )
    .await?;
    parse_openai(response)
}

pub(super) async fn complete_anthropic(
    http: &reqwest::Client,
    profile: &ProviderProfile,
    messages: &[ModelMessage],
    tools: &[ToolDefinition],
    api_key: Option<&str>,
) -> Result<ModelReply, ProviderError> {
    let path = if profile.base_url.trim_end_matches('/').ends_with("/v1") {
        "messages"
    } else {
        "v1/messages"
    };
    let system = messages
        .iter()
        .filter(|message| message.role == MessageRole::System)
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let body = json!({
        "model": profile.model,
        "max_tokens": 8192,
        "system": system,
        "messages": messages.iter().filter(|message| message.role != MessageRole::System).map(anthropic_message).collect::<Vec<_>>(),
        "tools": tools.iter().map(anthropic_tool).collect::<Vec<_>>()
    });
    let response = checked_json(
        http.post(join_url(&profile.base_url, path))
            .headers(common_headers(api_key, AuthStyle::Anthropic)?)
            .json(&body)
            .send()
            .await?,
    )
    .await?;
    parse_anthropic(response)
}

pub(super) async fn complete_gemini(
    http: &reqwest::Client,
    profile: &ProviderProfile,
    messages: &[ModelMessage],
    tools: &[ToolDefinition],
    api_key: Option<&str>,
) -> Result<ModelReply, ProviderError> {
    let version = if profile.base_url.trim_end_matches('/').ends_with("/v1") {
        ""
    } else {
        "v1beta/"
    };
    let path = format!("{version}models/{}:generateContent", profile.model);
    let body = json!({
        "contents": messages.iter().filter(|message| message.role != MessageRole::System).map(gemini_message).collect::<Vec<_>>(),
        "systemInstruction": {"parts": messages.iter().filter(|message| message.role == MessageRole::System).map(|message| json!({"text": message.content})).collect::<Vec<_>>()},
        "tools": [{"functionDeclarations": tools.iter().map(gemini_tool).collect::<Vec<_>>()}]
    });
    let response = checked_json(
        http.post(join_url(&profile.base_url, &path))
            .headers(common_headers(api_key, AuthStyle::Gemini)?)
            .json(&body)
            .send()
            .await?,
    )
    .await?;
    parse_gemini(response)
}

#[derive(Clone, Copy)]
enum AuthStyle {
    Bearer,
    Anthropic,
    Gemini,
}

fn common_headers(api_key: Option<&str>, style: AuthStyle) -> Result<HeaderMap, ProviderError> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if let Some(key) = api_key {
        let value = secret_header(key)?;
        match style {
            AuthStyle::Bearer => {
                headers.insert(AUTHORIZATION, secret_header(&format!("Bearer {key}"))?)
            }
            AuthStyle::Anthropic => {
                headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
                headers.insert("x-api-key", value)
            }
            AuthStyle::Gemini => headers.insert("x-goog-api-key", value),
        };
    }
    Ok(headers)
}

fn secret_header(value: &str) -> Result<HeaderValue, ProviderError> {
    HeaderValue::from_str(value).map_err(|error| {
        ProviderError::InvalidProfile(anyhow::Error::new(error).context("invalid API key"))
    })
}

async fn checked_json(response: reqwest::Response) -> Result<Value, ProviderError> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await?.chars().take(2048).collect();
        return Err(ProviderError::Http {
            status: status.as_u16(),
            body,
        });
    }
    response.json().await.map_err(ProviderError::Request)
}

fn join_url(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

fn openai_tool(tool: &ToolDefinition) -> Value {
    json!({"type":"function","function":{"name":tool.name,"description":tool.description,"parameters":tool.input_schema}})
}

fn anthropic_tool(tool: &ToolDefinition) -> Value {
    json!({"name":tool.name,"description":tool.description,"input_schema":tool.input_schema})
}

fn gemini_tool(tool: &ToolDefinition) -> Value {
    json!({"name":tool.name,"description":tool.description,"parameters":tool.input_schema})
}

fn anthropic_message(message: &ModelMessage) -> Value {
    let role = if message.role == MessageRole::Assistant {
        "assistant"
    } else {
        "user"
    };
    json!({"role":role,"content":message.content})
}

fn gemini_message(message: &ModelMessage) -> Value {
    let role = if message.role == MessageRole::Assistant {
        "model"
    } else {
        "user"
    };
    json!({"role":role,"parts":[{"text":message.content}]})
}
