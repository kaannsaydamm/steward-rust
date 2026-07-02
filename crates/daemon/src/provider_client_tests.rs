use super::{complete, MessageRole, ModelMessage};
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use steward_core::provider_config::{ProviderProfile, ProviderProtocol};

#[tokio::test]
async fn openai_reply_is_parsed_when_server_returns_text() {
    let captured = Arc::new(Mutex::new(None));
    let (base_url, server) = mock_server(
        captured.clone(),
        json!({"choices":[{"message":{"content":"Hello from model"}}]}),
    )
    .await;
    let profile = profile(ProviderProtocol::OpenAiChat, &base_url);

    let reply = complete(
        &reqwest::Client::new(),
        &profile,
        &[ModelMessage::new(MessageRole::User, "hello")],
        &[],
    )
    .await
    .expect("model response");

    assert_eq!(reply.text, "Hello from model");
    assert_eq!(captured_model(&captured), "test-model");
    server.abort();
}

#[tokio::test]
async fn anthropic_reply_is_parsed_when_server_returns_content_blocks() {
    let captured = Arc::new(Mutex::new(None));
    let (base_url, server) = mock_server(
        captured,
        json!({"content":[{"type":"text","text":"Anthropic answer"}]}),
    )
    .await;
    let profile = profile(ProviderProtocol::AnthropicMessages, &base_url);

    let reply = complete(
        &reqwest::Client::new(),
        &profile,
        &[ModelMessage::new(MessageRole::User, "hello")],
        &[],
    )
    .await
    .expect("model response");

    assert_eq!(reply.text, "Anthropic answer");
    server.abort();
}

#[tokio::test]
async fn gemini_reply_is_parsed_when_server_returns_candidate_parts() {
    let captured = Arc::new(Mutex::new(None));
    let (base_url, server) = mock_server(
        captured,
        json!({"candidates":[{"content":{"parts":[{"text":"Gemini answer"}]}}]}),
    )
    .await;
    let profile = profile(ProviderProtocol::GeminiGenerateContent, &base_url);

    let reply = complete(
        &reqwest::Client::new(),
        &profile,
        &[ModelMessage::new(MessageRole::User, "hello")],
        &[],
    )
    .await
    .expect("model response");

    assert_eq!(reply.text, "Gemini answer");
    server.abort();
}

fn profile(protocol: ProviderProtocol, base_url: &str) -> ProviderProfile {
    ProviderProfile {
        profile_id: "test".to_owned(),
        provider_id: "test".to_owned(),
        display_name: "Test".to_owned(),
        protocol,
        base_url: base_url.to_owned(),
        model: "test-model".to_owned(),
        api_key_env: None,
    }
}

async fn mock_server(
    captured: Arc<Mutex<Option<Value>>>,
    response: Value,
) -> (String, tokio::task::JoinHandle<()>) {
    async fn handler(
        State((captured, response)): State<(Arc<Mutex<Option<Value>>>, Value)>,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        if let Ok(mut slot) = captured.lock() {
            *slot = Some(body);
        }
        Json(response)
    }

    let app = Router::new()
        .route("/v1/chat/completions", post(handler))
        .route("/v1/messages", post(handler))
        .route("/v1/models/test-model:generateContent", post(handler))
        .with_state((captured, response));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock provider");
    let address = listener.local_addr().expect("mock provider address");
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("serve mock provider");
    });
    (format!("http://{address}/v1"), server)
}

fn captured_model(captured: &Arc<Mutex<Option<Value>>>) -> String {
    captured
        .lock()
        .expect("captured request lock")
        .as_ref()
        .and_then(|body| body.get("model"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
