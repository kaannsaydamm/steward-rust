use crate::client;
use anyhow::{Context as _, Result};
use steward_core::pb::{
    ActivateProviderProfileRequest, ChatEvent, ChatRequest, ChatSession, ChatSessionSummary,
    CompactChatSessionRequest, CompactChatSessionResponse, DeleteChatSessionRequest,
    DeleteProviderProfileRequest, GetChatSessionRequest, ListChatSessionsRequest,
    ListProviderCatalogRequest, ListProviderProfilesRequest, ProviderCatalogEntry,
    ProviderProfileInfo, SaveProviderProfileRequest,
};
use tokio::sync::mpsc::UnboundedSender;
use tonic::Request;

pub async fn provider_catalog(host: &str) -> Result<Vec<ProviderCatalogEntry>> {
    let mut rpc = client::connect(host).await?;
    Ok(rpc
        .list_provider_catalog(Request::new(ListProviderCatalogRequest {}))
        .await
        .context("calling ListProviderCatalog")?
        .into_inner()
        .providers)
}

pub async fn provider_profiles(host: &str) -> Result<Vec<ProviderProfileInfo>> {
    let mut rpc = client::connect(host).await?;
    Ok(rpc
        .list_provider_profiles(Request::new(ListProviderProfilesRequest {}))
        .await
        .context("calling ListProviderProfiles")?
        .into_inner()
        .profiles)
}

pub async fn save_provider(
    host: &str,
    profile: ProviderProfileInfo,
    activate: bool,
) -> Result<ProviderProfileInfo> {
    let mut rpc = client::connect(host).await?;
    Ok(rpc
        .save_provider_profile(Request::new(SaveProviderProfileRequest {
            profile: Some(profile),
            activate,
        }))
        .await
        .context("calling SaveProviderProfile")?
        .into_inner())
}

pub async fn activate_provider(host: &str, profile_id: &str) -> Result<ProviderProfileInfo> {
    let mut rpc = client::connect(host).await?;
    Ok(rpc
        .activate_provider_profile(Request::new(ActivateProviderProfileRequest {
            profile_id: profile_id.to_owned(),
        }))
        .await
        .context("calling ActivateProviderProfile")?
        .into_inner())
}

pub async fn delete_provider(host: &str, profile_id: &str) -> Result<bool> {
    let mut rpc = client::connect(host).await?;
    Ok(rpc
        .delete_provider_profile(Request::new(DeleteProviderProfileRequest {
            profile_id: profile_id.to_owned(),
        }))
        .await
        .context("calling DeleteProviderProfile")?
        .into_inner()
        .deleted)
}

pub async fn chat(
    host: &str,
    session_id: &str,
    message: &str,
    allow_tools: bool,
) -> Result<Vec<ChatEvent>> {
    let mut rpc = client::connect(host).await?;
    let mut stream = rpc
        .chat(Request::new(ChatRequest {
            session_id: session_id.to_owned(),
            message: message.to_owned(),
            working_directory: std::env::current_dir()?.display().to_string(),
            allow_tools,
        }))
        .await
        .context("calling Chat")?
        .into_inner();
    let mut events = Vec::new();
    while let Some(event) = stream.message().await.context("reading Chat stream")? {
        events.push(event);
    }
    Ok(events)
}

pub async fn stream_chat(
    host: &str,
    session_id: &str,
    message: &str,
    allow_tools: bool,
    sender: UnboundedSender<Result<ChatEvent, String>>,
) -> Result<()> {
    let mut rpc = client::connect(host).await?;
    let mut stream = rpc
        .chat(Request::new(ChatRequest {
            session_id: session_id.to_owned(),
            message: message.to_owned(),
            working_directory: std::env::current_dir()?.display().to_string(),
            allow_tools,
        }))
        .await
        .context("calling Chat")?
        .into_inner();
    while let Some(event) = stream.message().await.context("reading Chat stream")? {
        if sender.send(Ok(event)).is_err() {
            break;
        }
    }
    Ok(())
}

pub async fn sessions(host: &str, limit: i32) -> Result<Vec<ChatSessionSummary>> {
    let mut rpc = client::connect(host).await?;
    Ok(rpc
        .list_chat_sessions(Request::new(ListChatSessionsRequest { limit }))
        .await
        .context("calling ListChatSessions")?
        .into_inner()
        .sessions)
}

pub async fn session(host: &str, session_id: &str) -> Result<ChatSession> {
    let mut rpc = client::connect(host).await?;
    Ok(rpc
        .get_chat_session(Request::new(GetChatSessionRequest {
            session_id: session_id.to_owned(),
        }))
        .await
        .context("calling GetChatSession")?
        .into_inner())
}

pub async fn compact_session(host: &str, session_id: &str) -> Result<CompactChatSessionResponse> {
    let mut rpc = client::connect(host).await?;
    Ok(rpc
        .compact_chat_session(Request::new(CompactChatSessionRequest {
            session_id: session_id.to_owned(),
        }))
        .await
        .context("calling CompactChatSession")?
        .into_inner())
}

pub async fn delete_session(host: &str, session_id: &str) -> Result<bool> {
    let mut rpc = client::connect(host).await?;
    Ok(rpc
        .delete_chat_session(Request::new(DeleteChatSessionRequest {
            session_id: session_id.to_owned(),
        }))
        .await
        .context("calling DeleteChatSession")?
        .into_inner()
        .deleted)
}
