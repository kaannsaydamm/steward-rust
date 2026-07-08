use crate::provider_client::{self, MessageRole, ModelMessage};
use crate::{agent_runtime, session_store, MySteward};
use std::pin::Pin;
use steward_core::pb::{
    ChatEvent, ChatMessageInfo, ChatRequest, ChatSession, ChatSessionSummary,
    CompactChatSessionRequest, CompactChatSessionResponse, DeleteChatSessionRequest,
    DeleteChatSessionResponse, GetChatSessionRequest, ListChatSessionsRequest,
    ListChatSessionsResponse,
};
use steward_core::provider_config::ProviderSettings;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

const COMPACT_INSTRUCTION: &str = "Summarize the following conversation transcript for use as \
a replacement context. Preserve: concrete facts and figures, decisions made and their reasons, \
code or file paths that were discussed, unresolved questions, and the user's goals. Be concise \
but do not drop anything a future turn would need. Reply with the summary only.";

pub type ChatStream =
    Pin<Box<dyn tokio_stream::Stream<Item = Result<ChatEvent, Status>> + Send + 'static>>;

pub async fn start(
    steward: &MySteward,
    request: Request<ChatRequest>,
) -> Result<Response<ChatStream>, Status> {
    let (sender, receiver) = tokio::sync::mpsc::channel(64);
    let steward = steward.clone();
    tokio::spawn(async move {
        if let Err(error) = agent_runtime::run(&steward, request.into_inner(), sender.clone()).await
        {
            let _ = sender
                .send(Err(Status::failed_precondition(format!("{error:#}"))))
                .await;
        }
    });
    Ok(Response::new(Box::pin(ReceiverStream::new(receiver))))
}

pub async fn list_sessions(
    steward: &MySteward,
    request: Request<ListChatSessionsRequest>,
) -> Result<Response<ListChatSessionsResponse>, Status> {
    let limit = usize::try_from(request.into_inner().limit.max(1)).unwrap_or(50);
    let connection = database(steward).map_err(internal)?;
    let sessions = session_store::list_sessions(&connection, limit)
        .map_err(internal)?
        .into_iter()
        .map(summary)
        .collect();
    Ok(Response::new(ListChatSessionsResponse { sessions }))
}

pub async fn get_session(
    steward: &MySteward,
    request: Request<GetChatSessionRequest>,
) -> Result<Response<ChatSession>, Status> {
    let session_id = request.into_inner().session_id;
    let connection = database(steward).map_err(internal)?;
    let session = session_store::get_session(&connection, &session_id)
        .map_err(internal)?
        .ok_or_else(|| Status::not_found(format!("chat session '{session_id}' not found")))?;
    Ok(Response::new(ChatSession {
        summary: Some(summary(session.summary)),
        messages: session
            .messages
            .into_iter()
            .map(|message| ChatMessageInfo {
                message_id: message.message_id,
                role: message.role,
                content: message.content,
                tool_name: message.tool_name,
                tool_call_id: message.tool_call_id,
                created_at: message.created_at,
            })
            .collect(),
    }))
}

pub async fn compact_session(
    steward: &MySteward,
    request: Request<CompactChatSessionRequest>,
) -> Result<Response<CompactChatSessionResponse>, Status> {
    let session_id = request.into_inner().session_id;
    // Snapshot the transcript first and release the DB lock before the (slow) model call.
    let (transcript, message_count) = {
        let connection = database(steward).map_err(internal)?;
        let session = session_store::get_session(&connection, &session_id)
            .map_err(internal)?
            .ok_or_else(|| Status::not_found(format!("chat session '{session_id}' not found")))?;
        let transcript = session
            .messages
            .iter()
            .map(|message| {
                let speaker = if message.tool_name.is_empty() {
                    message.role.clone()
                } else {
                    format!("tool {}", message.tool_name)
                };
                format!("[{speaker}] {}", message.content)
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        (transcript, session.messages.len())
    };
    if message_count < 2 {
        return Err(Status::failed_precondition(
            "session is too short to compact",
        ));
    }
    let settings = ProviderSettings::load(&steward.provider_path).map_err(internal)?;
    let profile = settings
        .active()
        .map_err(|error| Status::failed_precondition(format!("{error:#}")))?;
    let messages = vec![
        ModelMessage::new(MessageRole::System, COMPACT_INSTRUCTION),
        ModelMessage::new(MessageRole::User, &transcript),
    ];
    let reply = provider_client::complete(&steward.http, profile, &messages, &[])
        .await
        .map_err(|error| Status::unavailable(format!("summarization failed: {error:#}")))?;
    if reply.text.trim().is_empty() {
        return Err(Status::internal("model returned an empty summary"));
    }
    let removed = {
        let connection = database(steward).map_err(internal)?;
        session_store::replace_with_summary(&connection, &session_id, &reply.text)
            .map_err(internal)?
    };
    Ok(Response::new(CompactChatSessionResponse {
        removed_messages: i64::try_from(removed).unwrap_or(i64::MAX),
        summary: reply.text,
    }))
}

pub async fn delete_session(
    steward: &MySteward,
    request: Request<DeleteChatSessionRequest>,
) -> Result<Response<DeleteChatSessionResponse>, Status> {
    let connection = database(steward).map_err(internal)?;
    let deleted = session_store::delete_session(&connection, &request.into_inner().session_id)
        .map_err(internal)?;
    Ok(Response::new(DeleteChatSessionResponse { deleted }))
}

fn summary(value: session_store::SessionSummary) -> ChatSessionSummary {
    ChatSessionSummary {
        session_id: value.session_id,
        title: value.title,
        provider_profile: value.provider_profile,
        model: value.model,
        created_at: value.created_at,
        updated_at: value.updated_at,
    }
}

fn database(
    steward: &MySteward,
) -> anyhow::Result<std::sync::MutexGuard<'_, rusqlite::Connection>> {
    steward
        .db
        .lock()
        .map_err(|_| anyhow::anyhow!("Database lock failed"))
}

fn internal(error: anyhow::Error) -> Status {
    Status::internal(format!("{error:#}"))
}
