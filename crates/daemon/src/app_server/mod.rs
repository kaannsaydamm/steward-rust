//! App Server: the Wire v2 WebSocket endpoint (`/api/v2/wire`).
//!
//! Loopback-only by default; every connection must pass an origin check and
//! present the install/session auth token before the Hello handshake.

pub mod compat_v1;

use anyhow::{Context as _, Result};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use steward_wire::envelope::{AppResponse, ClientEnvelope, ServerEnvelope};
use steward_wire::version::{Hello, HANDSHAKE};
use tokio::sync::Mutex;

/// Shared app-server session state: in-memory event log per run so
/// reconnecting clients can replay from their cursor (§34.3).
#[derive(Default)]
pub struct AppState {
    /// Run id -> ordered events (bounded ring per run is a later refinement).
    pub events: Mutex<std::collections::HashMap<String, Vec<steward_wire::envelope::RunEventV2>>>,
    /// Install/session token required on the Hello frame.
    pub auth_token: String,
}

/// Builds the app-server router mounted under `/api/v2`.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/wire", get(ws_upgrade))
        .with_state(state)
}

async fn ws_upgrade(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    if !origin_allowed(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    upgrade
        .on_upgrade(move |socket| connection(state, socket))
        .into_response()
}

/// Strict loopback origin check: only http(s) origins whose host resolves to
/// loopback are accepted (guards cross-site WebSocket hijacking).
fn origin_allowed(headers: &HeaderMap) -> bool {
    let origin = match headers.get(axum::http::header::ORIGIN) {
        Some(value) => match value.to_str() {
            Ok(value) => value,
            Err(_) => return false,
        },
        // Non-browser clients (CLI/TUI) may omit Origin.
        None => return true,
    };
    if let Ok(url) = url::Url::parse(origin) {
        if let Some(host) = url.host_str() {
            return matches!(host, "localhost" | "127.0.0.1" | "[::1]");
        }
    }
    false
}

async fn connection(state: Arc<AppState>, mut socket: WebSocket) {
    let mut hello_done = false;
    let mut last_sequence: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();

    while let Some(message) = socket.recv().await {
        let Ok(Message::Binary(bytes)) = message else {
            continue;
        };
        let Ok(envelope) = serde_json::from_slice::<ClientEnvelope>(&bytes) else {
            let _ = send(
                &mut socket,
                &ServerEnvelope::Error(steward_wire::envelope::AppError {
                    request_id: None,
                    code: "ProtocolMismatch".into(),
                    message: "unparseable client envelope".into(),
                    remediation: None,
                    retryable: false,
                }),
            )
            .await;
            continue;
        };

        match envelope {
            ClientEnvelope::Hello(hello) => {
                if let Some(expected) = Some(&state.auth_token) {
                    match &hello.auth_token {
                        Some(token) if token == expected => {}
                        _ => {
                            let _ = send(
                                &mut socket,
                                &ServerEnvelope::Error(steward_wire::envelope::AppError {
                                    request_id: None,
                                    code: "Authentication".into(),
                                    message: "missing or invalid auth token".into(),
                                    remediation: None,
                                    retryable: false,
                                }),
                            )
                            .await;
                            let _ = socket.close().await;
                            return;
                        }
                    }
                }
                let server_hello = Hello {
                    protocol_version: HANDSHAKE,
                    instance_id: "steward-daemon".into(),
                    auth_token: None,
                    capabilities: vec!["runs".into(), "events".into()],
                };
                if server_hello.negotiate(&hello).is_err() {
                    let _ = send(
                        &mut socket,
                        &ServerEnvelope::Error(steward_wire::envelope::AppError {
                            request_id: None,
                            code: "ProtocolMismatch".into(),
                            message: format!(
                                "unsupported protocol version {}.{}, expected {}.{}",
                                hello.protocol_version.major,
                                hello.protocol_version.minor,
                                HANDSHAKE.major,
                                HANDSHAKE.minor
                            ),
                            remediation: None,
                            retryable: false,
                        }),
                    )
                    .await;
                    let _ = socket.close().await;
                    return;
                }
                hello_done = true;
                let _ = send(&mut socket, &ServerEnvelope::Hello(server_hello)).await;
            }
            ClientEnvelope::Ping => {
                let _ = send(&mut socket, &ServerEnvelope::Pong).await;
            }
            ClientEnvelope::Ack(ack) => {
                last_sequence.insert(ack.run_id.clone(), ack.last_sequence);
            }
            ClientEnvelope::Command(command) if hello_done => {
                handle_command(&state, &mut socket, command, &mut last_sequence).await;
            }
            ClientEnvelope::Command(_) => {
                let _ = send(
                    &mut socket,
                    &ServerEnvelope::Error(steward_wire::envelope::AppError {
                        request_id: None,
                        code: "Authentication".into(),
                        message: "commands require a completed handshake".into(),
                        remediation: None,
                        retryable: false,
                    }),
                )
                .await;
            }
        }
    }
}

async fn handle_command(
    state: &Arc<AppState>,
    socket: &mut WebSocket,
    command: steward_wire::envelope::AppCommand,
    last_sequence: &mut std::collections::HashMap<String, u64>,
) {
    use steward_wire::envelope::AppCommand;
    match command {
        AppCommand::StartRun {
            thread_id, message, ..
        } => {
            // Snapshot-only placeholder run; the kernel adapter (Phase 4)
            // replaces this with real TurnEngine execution.
            let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());
            let event = steward_wire::envelope::RunEventV2 {
                run_id: run_id.clone(),
                sequence: 1,
                timestamp_ms: now_ms(),
                event_type: "RunCreated".into(),
                payload_json: serde_json::json!({
                    "thread_id": thread_id,
                    "message": message,
                })
                .to_string(),
            };
            state
                .events
                .lock()
                .await
                .entry(run_id.clone())
                .or_default()
                .push(event.clone());
            let _ = send(
                socket,
                &ServerEnvelope::Response(AppResponse {
                    request_id: String::new(),
                    ok: true,
                    run_id: Some(run_id.clone()),
                    error: None,
                }),
            )
            .await;
            let _ = send(socket, &ServerEnvelope::Event(event)).await;
        }
        AppCommand::CancelRun { .. } | AppCommand::Signal { .. } => {
            let _ = send(
                socket,
                &ServerEnvelope::Response(AppResponse {
                    request_id: String::new(),
                    ok: false,
                    run_id: None,
                    error: Some("not yet wired to the kernel; Phase 4".into()),
                }),
            )
            .await;
        }
        AppCommand::ResolveApproval { .. } => {
            let _ = send(
                socket,
                &ServerEnvelope::Response(AppResponse {
                    request_id: String::new(),
                    ok: false,
                    run_id: None,
                    error: Some("approvals move to the tools runtime; Phase 7".into()),
                }),
            )
            .await;
        }
    }
}

async fn send(socket: &mut WebSocket, envelope: &ServerEnvelope) -> Result<()> {
    let bytes = serde_json::to_vec(envelope).context("encoding server envelope")?;
    socket
        .send(Message::Binary(bytes))
        .await
        .context("sending over websocket")
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
pub(crate) mod tests;
