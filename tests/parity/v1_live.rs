//! Phase 0 v1 baseline parity evals against the live provider.
//!
//! Skips unless `STEWARD_TEST_BASE_URL` + `STEWARD_TEST_API_KEY` are set.
//! The key travels only through the environment; it is never written to
//! fixtures, code, or output.

#[path = "../harness.rs"]
mod harness;

use harness::{unused_port, DaemonProcess};
use steward_core::pb::steward_service_client::StewardServiceClient;
use steward_core::pb::{
    ChatEventKind, ChatRequest, PingRequest, SaveProviderProfileRequest, ProviderProfileInfo,
    ProviderProtocol,
};
use std::time::Duration;
use tokio::time::sleep;

fn live_config() -> Option<(String, String, String)> {
    let base_url = std::env::var("STEWARD_TEST_BASE_URL").ok()?;
    let api_key = std::env::var("STEWARD_TEST_API_KEY").ok()?;
    if base_url.trim().is_empty() || api_key.trim().is_empty() {
        return None;
    }
    let model = std::env::var("STEWARD_TEST_MODEL").unwrap_or_else(|_| "glm-5.3-flash".into());
    Some((base_url, api_key, model))
}

async fn connect_daemon() -> (StewardServiceClient<tonic::transport::Channel>, DaemonProcess) {
    let port = unused_port();
    let daemon = DaemonProcess::start(port);
    let addr = format!("http://127.0.0.1:{port}");
    let mut client = None;
    for _ in 0..60 {
        if let Ok(c) = StewardServiceClient::connect(addr.clone()).await {
            client = Some(c);
            break;
        }
        sleep(Duration::from_millis(500)).await;
    }
    let mut client = client.expect("daemon never accepted gRPC connections");
    client
        .ping(tonic::Request::new(PingRequest {}))
        .await
        .expect("ping daemon");
    (client, daemon)
}

async fn save_live_profile(
    client: &mut StewardServiceClient<tonic::transport::Channel>,
    base_url: &str,
    api_key: &str,
    model: &str,
) {
    client
        .save_provider_profile(tonic::Request::new(SaveProviderProfileRequest {
            profile: Some(ProviderProfileInfo {
                profile_id: "live-test".into(),
                provider_id: "bankofai".into(),
                display_name: "Bank of AI live test".into(),
                protocol: ProviderProtocol::OpenaiChat as i32,
                base_url: base_url.to_owned(),
                model: model.to_owned(),
                api_key_env: "STEWARD_TEST_API_KEY".into(),
                api_key: api_key.to_owned(),
                ..ProviderProfileInfo::default()
            }),
            activate: true,
        }))
        .await
        .expect("save provider profile");
}

async fn chat_collect(
    client: &mut StewardServiceClient<tonic::transport::Channel>,
    message: &str,
    allow_tools: bool,
) -> Vec<steward_core::pb::ChatEvent> {
    let request = ChatRequest {
        message: message.to_owned(),
        working_directory: std::env::current_dir()
            .expect("cwd")
            .to_string_lossy()
            .to_string(),
        allow_tools,
        ..ChatRequest::default()
    };
    let mut stream = client
        .chat(tonic::Request::new(request))
        .await
        .expect("chat rpc")
        .into_inner();
    let mut events = Vec::new();
    while let Ok(Some(event)) = stream.message().await {
        events.push(event);
    }
    events
}

#[tokio::test]
async fn v1_live_plain_completion_completes_with_done() {
    let Some((base_url, api_key, model)) = live_config() else {
        eprintln!("skipping: STEWARD_TEST_BASE_URL/STEWARD_TEST_API_KEY not set");
        return;
    };
    let (mut client, _daemon) = connect_daemon().await;
    save_live_profile(&mut client, &base_url, &api_key, &model).await;

    let events = chat_collect(&mut client, "Reply with the single word: pong", false).await;
    assert!(
        events.iter().any(|e| e.kind == ChatEventKind::Done as i32),
        "missing Done event; kinds: {:?}",
        events.iter().map(|e| e.kind).collect::<Vec<_>>()
    );
    let text: String = events
        .iter()
        .filter(|e| e.kind == ChatEventKind::Text as i32)
        .map(|e| e.content.clone())
        .collect();
    assert!(
        text.to_lowercase().contains("pong"),
        "expected 'pong' in reply, got: {text}"
    );
}

#[tokio::test]
async fn v1_live_session_persists_after_run() {
    let Some((base_url, api_key, model)) = live_config() else {
        eprintln!("skipping: STEWARD_TEST_BASE_URL/STEWARD_TEST_API_KEY not set");
        return;
    };
    let (mut client, _daemon) = connect_daemon().await;
    save_live_profile(&mut client, &base_url, &api_key, &model).await;

    let events = chat_collect(&mut client, "Say: persistent", false).await;
    assert!(events.iter().any(|e| e.kind == ChatEventKind::Done as i32));
    let session_id = events
        .first()
        .map(|e| e.session_id.clone())
        .expect("session id present on events");
    assert!(!session_id.is_empty());

    let sessions = client
        .list_chat_sessions(tonic::Request::new(
            steward_core::pb::ListChatSessionsRequest::default(),
        ))
        .await
        .expect("list sessions")
        .into_inner();
    assert!(
        sessions
            .sessions
            .iter()
            .any(|s| s.session_id == session_id),
        "session {session_id} missing after completion"
    );
}

#[tokio::test]
async fn v1_live_tool_roundtrip_runs_tool() {
    let Some((base_url, api_key, model)) = live_config() else {
        eprintln!("skipping: STEWARD_TEST_BASE_URL/STEWARD_TEST_API_KEY not set");
        return;
    };
    let (mut client, _daemon) = connect_daemon().await;
    save_live_profile(&mut client, &base_url, &api_key, &model).await;

    // fs.read is disabled by default; the v1 loop only exposes enabled,
    // non-approval tools. Enable it through the public registry RPC.
    client
        .set_tool_enabled(tonic::Request::new(
            steward_core::pb::SetToolEnabledRequest {
                tool_id: "fs.read".into(),
                enabled: true,
            },
        ))
        .await
        .expect("enable fs.read");

    let events = chat_collect(
        &mut client,
        "Call the fs__read tool with path Cargo.toml, then say done.",
        true,
    )
    .await;
    let started = events
        .iter()
        .any(|e| e.kind == ChatEventKind::ToolStart as i32);
    let finished = events.iter().any(|e| e.kind == ChatEventKind::Done as i32);
    assert!(
        started && finished,
        "expected tool start + done; kinds: {:?}",
        events.iter().map(|e| e.kind).collect::<Vec<_>>()
    );
}
