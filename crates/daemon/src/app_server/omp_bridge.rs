//! OpenAI-compatible chat-completions endpoint backed by the Steward daemon.
//!
//! Adapter for the vendored OMP CLI (`vendor/omp`): its `openai-completions`
//! provider accepts any OpenAI-shaped `baseUrl`, so pointing it at
//! `http://127.0.0.1:<wire-port>/omp/v1` routes every model call through the
//! Steward daemon (single source of truth). The HTTP surface is additive —
//! no upstream OMP file is modified; the adapter is a config-level seam
//! (`models.yml` provider `baseUrl`) per docs/OSS_PORT_MAP.md O3/O4.
//!
//! Request: OpenAI chat-completions shape (non-streaming handled; streaming
//! returns SSE deltas). Response: OpenAI shape with `choices[0].message`.

use crate::agent_runtime;
use crate::MySteward;
use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::Json;
use futures_util::stream::Stream;
use serde::Deserialize;
use steward_core::pb::ChatEventKind;
use serde_json::{json, Value};
use std::convert::Infallible;
use tonic::Request;
use steward_core::pb::ChatRequest;

#[derive(Deserialize, Clone)]
pub struct OpenAiMessage {
    pub role: String,
    pub content: Value,
}

#[derive(Deserialize, Clone)]
pub struct OpenAiChatRequest {
    pub model: String,
    pub messages: Vec<OpenAiMessage>,
    #[serde(default)]
    pub stream: bool,
}

fn content_to_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

async fn run_turn(
    steward: &MySteward,
    model: String,
    messages: Vec<OpenAiMessage>,
    sender: agent_runtime::EventSender,
) -> anyhow::Result<String> {
    let last_user = messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| content_to_text(&m.content))
        .unwrap_or_default();
    // Session continuity: reuse the last steward session id per connection via
    // the empty-session mint (Chat handles persistence); OMP manages its own
    // thread-of-thought above this endpoint.
    let request = ChatRequest {
        session_id: String::new(),
        message: last_user,
        working_directory: String::new(),
        allow_tools: true,
    };
    let _ = model; // model routing is daemon-owned (ProviderSettings)
    let result = agent_runtime::run(steward, request, sender).await?;
    Ok(result.final_text)
}

/// POST /omp/v1/chat/completions
pub async fn chat_completions(
    State(steward): State<MySteward>,
    body: String,
) -> Result<axum::response::Response, (axum::http::StatusCode, String)> {
    tracing::info!("omp bridge body len={} head={}", body.len(), &body.chars().take(120).collect::<String>());
    let request: OpenAiChatRequest = serde_json::from_str(&body)
        .map_err(|error| (axum::http::StatusCode::BAD_REQUEST, format!("bad json: {error}")))?;
    if !request.stream {
        // Non-streaming: buffer the full answer, return a completion object.
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<
            std::result::Result<steward_core::pb::ChatEvent, tonic::Status>,
        >(64);
        let steward_clone = steward.clone();
        let messages = request.messages.clone();
        tokio::spawn(async move {
            let _ = run_turn(&steward_clone, request.model.clone(), messages, sender).await;
        });
        let mut answer = String::new();
        while let Some(result) = receiver.recv().await {
            let Ok(event) = result else { continue };
            if ChatEventKind::try_from(event.kind) == Ok(ChatEventKind::Text) {
                answer.push_str(&event.content);
            }
        }
        let completion = json!({
            "id": format!("chatcmpl-steward-{}", uuid::Uuid::new_v4().simple()),
            "object": "chat.completion",
            "model": "steward",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": answer},
                "finish_reason": "stop"
            }]
        });
        return Ok(axum::Json(completion).into_response());
    }
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<
        std::result::Result<steward_core::pb::ChatEvent, tonic::Status>,
    >(64);
    let steward_clone = steward.clone();
    let model = request.model.clone();
    let messages = request.messages.clone();
    tokio::spawn(async move {
        if let Err(error) = run_turn(&steward_clone, model, messages, sender.clone()).await {
            let _ = sender.send(Ok(steward_core::pb::ChatEvent {
                kind: steward_core::pb::ChatEventKind::Text as i32,
                content: format!("[error: {error}"),
                ..Default::default()
            }));
        }
    });

    // SSE stream mapping daemon events to OpenAI delta chunks (futures channel).
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<Event, Infallible>>();
    tokio::spawn(async move {
        let id = format!("chatcmpl-steward-{}", uuid::Uuid::new_v4().simple());
        let _ = tx.send(Ok(Event::default().data(json!({
            "id": id, "object": "chat.completion.chunk",
            "model": "steward",
            "choices": [{"index": 0, "delta": {"role": "assistant"}, "finish_reason": null}]
        }).to_string())));
        while let Some(result) = receiver.recv().await {
            let Ok(event) = result else { continue };
            match ChatEventKind::try_from(event.kind) {
                Ok(ChatEventKind::Text) => {
                    let _ = tx.send(Ok(Event::default().data(json!({
                        "id": id, "object": "chat.completion.chunk", "model": "steward",
                        "choices": [{"index": 0, "delta": {"content": event.content}, "finish_reason": null}]
                    }).to_string())));
                }
                Ok(ChatEventKind::ToolStart) => {
                    let _ = tx.send(Ok(Event::default().data(json!({
                        "id": id, "object": "chat.completion.chunk", "model": "steward",
                        "choices": [{"index": 0, "delta": {"content": format!("\n[tool: {}] ", event.tool_name)}, "finish_reason": null}]
                    }).to_string())));
                }
                _ => {}
            }
        }
        // Terminal chunks: OpenAI clients require a finish_reason=stop delta
        // and OMP additionally expects the [DONE] sentinel before close.
        let _ = tx.send(Ok(Event::default().data(json!({
            "id": id, "object": "chat.completion.chunk", "model": "steward",
            "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]
        }).to_string())));
        let _ = tx.send(Ok(Event::default().data("[DONE]")));
    });
    let rx = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
    Ok(Sse::new(rx).into_response())
}
