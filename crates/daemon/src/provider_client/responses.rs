use super::{ModelReply, ModelToolCall, ProviderError};
use serde_json::{json, Value};

pub(super) fn parse_openai(value: Value) -> Result<ModelReply, ProviderError> {
    let message = value
        .pointer("/choices/0/message")
        .ok_or(ProviderError::EmptyResponse)?;
    let text = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let tool_calls = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(|values| parse_openai_tools(values))
        .transpose()?
        .unwrap_or_default();
    Ok(ModelReply {
        text,
        tool_calls,
        prompt_tokens: token(&value, "/usage/prompt_tokens"),
        completion_tokens: token(&value, "/usage/completion_tokens"),
    })
}

fn parse_openai_tools(values: &[Value]) -> Result<Vec<ModelToolCall>, ProviderError> {
    values
        .iter()
        .map(|value| {
            let raw = value
                .pointer("/function/arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}");
            Ok(ModelToolCall {
                id: string(value, "/id"),
                name: string(value, "/function/name"),
                arguments: serde_json::from_str(raw)
                    .map_err(ProviderError::InvalidToolArguments)?,
            })
        })
        .collect()
}

pub(super) fn parse_anthropic(value: Value) -> Result<ModelReply, ProviderError> {
    let content = value
        .get("content")
        .and_then(Value::as_array)
        .ok_or(ProviderError::EmptyResponse)?;
    let text = content
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("");
    let tool_calls = content
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
        .map(|block| ModelToolCall {
            id: string(block, "/id"),
            name: string(block, "/name"),
            arguments: block.get("input").cloned().unwrap_or_else(|| json!({})),
        })
        .collect();
    Ok(ModelReply {
        text,
        tool_calls,
        prompt_tokens: token(&value, "/usage/input_tokens"),
        completion_tokens: token(&value, "/usage/output_tokens"),
    })
}

pub(super) fn parse_gemini(value: Value) -> Result<ModelReply, ProviderError> {
    let parts = value
        .pointer("/candidates/0/content/parts")
        .and_then(Value::as_array)
        .ok_or(ProviderError::EmptyResponse)?;
    let text = parts
        .iter()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("");
    let tool_calls = parts
        .iter()
        .filter_map(|part| part.get("functionCall"))
        .map(|call| ModelToolCall {
            id: String::new(),
            name: string(call, "/name"),
            arguments: call.get("args").cloned().unwrap_or_else(|| json!({})),
        })
        .collect();
    Ok(ModelReply {
        text,
        tool_calls,
        prompt_tokens: token(&value, "/usageMetadata/promptTokenCount"),
        completion_tokens: token(&value, "/usageMetadata/candidatesTokenCount"),
    })
}

fn string(value: &Value, pointer: &str) -> String {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn token(value: &Value, pointer: &str) -> u64 {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_default()
}
