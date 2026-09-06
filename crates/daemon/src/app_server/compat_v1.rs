//! v1 -> v2 compatibility adapters (Task 2.4).
//!
//! Existing v1 RPC semantics (chat streaming, provider profiles, tools,
//! sessions) keep working untouched; this module maps a small, growing set of
//! v1 requests into Wire v2 envelopes so new clients can drive the same
//! daemon behavior before the full v2 services land.

use steward_wire::envelope::{AppCommand, AppResponse, ClientEnvelope, RunEventV2, ServerEnvelope};

/// Result of adapting a v1 request: zero or more client envelopes to send.
pub fn adapt_chat_start(
    thread_id: &str,
    message: &str,
    workspace_id: Option<&str>,
) -> Vec<ClientEnvelope> {
    vec![ClientEnvelope::Command(AppCommand::StartRun {
        thread_id: thread_id.to_owned(),
        workspace_id: workspace_id.map(str::to_owned),
        message: message.to_owned(),
        budget: Default::default(),
    })]
}

/// Maps a v1 ChatEvent kind/content pair into v2 run events.
pub fn chat_event_to_v2(run_id: &str, sequence: u64, kind: &str, content: &str) -> ServerEnvelope {
    ServerEnvelope::Event(RunEventV2 {
        run_id: run_id.to_owned(),
        sequence,
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64,
        event_type: kind.to_owned(),
        payload_json: serde_json::json!({ "content": content }).to_string(),
    })
}

/// Standard response for adapted commands.
pub fn ok_response(run_id: Option<String>) -> ServerEnvelope {
    ServerEnvelope::Response(AppResponse {
        request_id: String::new(),
        ok: true,
        run_id,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_start_adapts_to_start_run_command() {
        let envelopes = adapt_chat_start("thr_1", "hello", Some("ws_1"));
        assert_eq!(envelopes.len(), 1);
        match &envelopes[0] {
            ClientEnvelope::Command(AppCommand::StartRun {
                thread_id,
                message,
                workspace_id,
                ..
            }) => {
                assert_eq!(thread_id, "thr_1");
                assert_eq!(message, "hello");
                assert_eq!(workspace_id.as_deref(), Some("ws_1"));
            }
            other => panic!("unexpected envelope: {other:?}"),
        }
    }

    #[test]
    fn chat_events_map_to_ordered_v2_events() {
        let text = chat_event_to_v2("run_1", 3, "Text", "hello");
        let done = chat_event_to_v2("run_1", 4, "Done", "hello");
        match (text, done) {
            (ServerEnvelope::Event(a), ServerEnvelope::Event(b)) => {
                assert_eq!(a.sequence, 3);
                assert_eq!(b.sequence, 4);
                assert_eq!(a.run_id, "run_1");
            }
            _ => panic!("expected events"),
        }
    }

    #[test]
    fn ok_response_carries_run_id() {
        match ok_response(Some("run_9".into())) {
            ServerEnvelope::Response(response) => {
                assert!(response.ok);
                assert_eq!(response.run_id.as_deref(), Some("run_9"));
            }
            _ => panic!("expected response"),
        }
    }
}
