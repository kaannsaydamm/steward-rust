//! RLM live E2E: real model turn through the daemon, RlmRun observation
//! routed via the Prime substrate when STEWARD_RLM_RUNTIME=prime.
//!
//! Skips (passes trivially) unless STEWARD_TEST_BASE_URL + STEWARD_TEST_API_KEY
//! are set, matching the v1_live contract. Requires the vendored Prime runtime
//! and python on PATH for the prime substrate path.

#[path = "harness.rs"]
mod harness;

use anyhow::Context as _;

use harness::{unused_port, DaemonProcess};
use steward_core::pb::{
    steward_service_client::StewardServiceClient, ChatEventKind, ChatRequest,
    GetChatSessionRequest, PingRequest, ProviderProtocol, SaveProviderProfileRequest,
    ProviderProfileInfo,
};

fn live_credentials() -> Option<(String, String, String)> {
    let base_url = std::env::var("STEWARD_TEST_BASE_URL").ok()?;
    let api_key = std::env::var("STEWARD_TEST_API_KEY").ok()?;
    if base_url.trim().is_empty() || api_key.trim().is_empty() {
        return None;
    }
    let model = std::env::var("STEWARD_TEST_MODEL").unwrap_or_else(|_| "glm-5.3-flash".into());
    Some((base_url, api_key, model))
}

async fn arm_live_profile(
    client: &mut StewardServiceClient<tonic::transport::Channel>,
    base_url: &str,
    api_key: &str,
    model: &str,
) {
    client
        .save_provider_profile(tonic::Request::new(SaveProviderProfileRequest {
            profile: Some(ProviderProfileInfo {
                profile_id: "rlm-live".into(),
                provider_id: "bankofai".into(),
                display_name: "RLM live".into(),
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

#[tokio::test]
async fn rlm_live_gateway_multi_turn_same_session() -> anyhow::Result<()> {
    let Some((base_url, api_key, model)) = live_credentials() else {
        eprintln!("skipping: STEWARD_TEST_BASE_URL/STEWARD_TEST_API_KEY not set");
        return Ok(());
    };

    let port = unused_port();
    let daemon = DaemonProcess::start(port);
    let addr = format!("http://127.0.0.1:{port}");
    let mut client = None;
    for _ in 0..60 {
        if let Ok(c) = StewardServiceClient::connect(addr.clone()).await {
            client = Some(c);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    let mut client = client.expect("daemon never accepted gRPC connections");
    client
        .ping(tonic::Request::new(PingRequest {}))
        .await
        .expect("ping daemon");

    // Arm the live provider profile (api_key travels via env/Save RPC only).
    arm_live_profile(&mut client, &base_url, &api_key, &model).await;

    // Turn 1 mints the session.
    let mut turn1 = client
        .chat(tonic::Request::new(ChatRequest {
            session_id: String::new(),
            message: "What is 6*7? Answer with the number only.".into(),
            working_directory: String::new(),
            allow_tools: true,
        }))
        .await
        .context("turn 1 chat")?
        .into_inner();
    let mut session_id = String::new();
    let mut answer1 = String::new();
    while let Some(event) = turn1.message().await? {
        if event.kind == ChatEventKind::Session as i32 {
            session_id = event.session_id.clone();
        } else if event.kind == ChatEventKind::Text as i32 {
            answer1.push_str(&event.content);
        }
    }
    assert!(!session_id.is_empty(), "turn 1 must mint a session id");
    println!("turn1 session={session_id} answer={:.40}", answer1.trim());

    // Turn 2 resumes the same session — the continuity contract the Prime
    // substrate also relies on (persistent REPL keyed by session id).
    let mut turn2 = client
        .chat(tonic::Request::new(ChatRequest {
            session_id: session_id.clone(),
            message: "Multiply that previous result by 10. Answer with the number only.".into(),
            working_directory: String::new(),
            allow_tools: true,
        }))
        .await
        .context("turn 2 chat")?
        .into_inner();
    let mut answer2 = String::new();
    let mut same_session = false;
    while let Some(event) = turn2.message().await? {
        if event.kind == ChatEventKind::Session as i32 && event.session_id == session_id {
            same_session = true;
        } else if event.kind == ChatEventKind::Text as i32 {
            answer2.push_str(&event.content);
        }
    }
    assert!(same_session, "turn 2 must stay on session {session_id}");
    println!("turn2 answer={:.40}", answer2.trim());

    // The stored session must show both user turns (GetChatSession replay).
    let stored = client
        .get_chat_session(tonic::Request::new(GetChatSessionRequest {
            session_id: session_id.clone(),
            since_sequence: 0,
        }))
        .await?
        .into_inner();
    let user_turns = stored.messages.iter().filter(|m| m.role == "user").count();
    assert!(
        user_turns >= 2,
        "session must persist both user turns, got {user_turns}"
    );

    println!("RLM LIVE E2E PASS: session {session_id} multi-turn + persistence");
    Ok(())
}
