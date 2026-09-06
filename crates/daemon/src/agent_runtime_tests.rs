use super::run;
use crate::maintenance::RetentionConfig;
use crate::MySteward;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use steward_core::pb::{ChatEventKind, ChatRequest};
use steward_core::provider_config::{ProviderProfile, ProviderProtocol, ProviderSettings};

#[tokio::test]
async fn chat_persists_model_reply_when_provider_succeeds() {
    let (base_url, provider) = provider_server().await;
    let temp = tempfile::tempdir().expect("temporary directory");
    let db_path = temp.path().join("steward.db");
    let steward = MySteward::new(
        db_path.to_str().expect("database path"),
        RetentionConfig::default(),
    )
    .expect("steward runtime");
    let mut settings = ProviderSettings::default();
    settings
        .upsert(ProviderProfile {
            profile_id: "test".to_owned(),
            provider_id: "custom".to_owned(),
            display_name: "Test".to_owned(),
            protocol: ProviderProtocol::OpenAiChat,
            base_url,
            model: "test-model".to_owned(),
            api_key_env: None,
            secret_ref: None,
            legacy_api_key: None,
        })
        .expect("profile");
    settings.activate("test").expect("activate profile");
    settings
        .save(&steward.provider_path)
        .expect("provider settings");
    let (sender, mut receiver) = tokio::sync::mpsc::channel(16);

    let result = run(
        &steward,
        ChatRequest {
            session_id: String::new(),
            message: "hello".to_owned(),
            working_directory: temp.path().display().to_string(),
            allow_tools: false,
        },
        sender,
    )
    .await
    .expect("chat succeeds");
    drop(result);
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event.expect("chat event"));
    }

    assert!(events.iter().any(|event| {
        event.kind == ChatEventKind::Text as i32 && event.content == "Model answer"
    }));
    assert_eq!(
        events.last().map(|event| event.kind),
        Some(ChatEventKind::Done as i32)
    );
    provider.abort();
}

async fn provider_server() -> (String, tokio::task::JoinHandle<()>) {
    async fn handler(Json(_body): Json<Value>) -> Json<Value> {
        Json(json!({"choices":[{"message":{"content":"Model answer"}}]}))
    }
    let app = Router::new().route("/v1/chat/completions", post(handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind provider");
    let address = listener.local_addr().expect("provider address");
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve provider");
    });
    (format!("http://{address}/v1"), task)
}
