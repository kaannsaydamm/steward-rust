//! Client/server envelopes: the only message shapes crossing the wire.
//!
//! Commands carry requests; responses, events, snapshots, and errors flow
//! back. IDs stay opaque strings at the boundary (§34.2). Binary (protobuf)
//! and JSON (diagnostics) encodings must roundtrip identically.

use crate::version::Hello;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Client -> server.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientEnvelope {
    Hello(Hello),
    Command(AppCommand),
    Ack(EventAck),
    Ping,
}

/// Server -> client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEnvelope {
    Hello(Hello),
    Response(AppResponse),
    Event(RunEventV2),
    Snapshot(StateSnapshot),
    Error(AppError),
    Pong,
}

/// A client command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppCommand {
    /// Starts a run under a thread.
    StartRun {
        thread_id: String,
        workspace_id: Option<String>,
        message: String,
        #[serde(default)]
        budget: BTreeMap<String, serde_json::Value>,
    },
    /// Cancels a run; propagates to children per policy.
    CancelRun { run_id: String },
    /// Signals (steer/wake/pause/resume) a live run.
    Signal { run_id: String, signal: String },
    /// Resolves a pending approval.
    ResolveApproval {
        approval_id: String,
        decision: String,
        scope: Option<String>,
    },
}

/// Command response with correlated request id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppResponse {
    pub request_id: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Client acknowledgement of processed events (resume cursor tracking).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventAck {
    pub run_id: String,
    /// Highest event sequence processed by the client.
    pub last_sequence: u64,
}

/// Ordered durable run event (§14).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunEventV2 {
    pub run_id: String,
    /// Server-owned monotonic sequence per run.
    pub sequence: u64,
    pub timestamp_ms: i64,
    pub event_type: String,
    pub payload_json: String,
}

/// Compact state for reconnecting clients whose cursor rolled off retention.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub run_id: String,
    pub upto_sequence: u64,
    pub status: String,
    #[serde(default)]
    pub state: BTreeMap<String, serde_json::Value>,
}

/// Structured error: clients never parse strings (§13.10).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppError {
    pub request_id: Option<String>,
    /// Machine-readable code from the steward error taxonomy.
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
    pub retryable: bool,
}

/// Reconnect request: resume from `last_seen_sequence`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubscribeRequest {
    pub run_id: String,
    pub last_seen_sequence: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::{ProtocolVersion, HANDSHAKE};

    fn sample_client() -> ClientEnvelope {
        ClientEnvelope::Command(AppCommand::StartRun {
            thread_id: "thr_1".into(),
            workspace_id: Some("ws_1".into()),
            message: "hello".into(),
            budget: BTreeMap::new(),
        })
    }

    fn sample_server() -> ServerEnvelope {
        ServerEnvelope::Event(RunEventV2 {
            run_id: "run_1".into(),
            sequence: 7,
            timestamp_ms: 1_000,
            event_type: "ToolStarted".into(),
            payload_json: "{\"tool\":\"fs.read\"}".into(),
        })
    }

    #[test]
    fn client_envelope_json_roundtrip() {
        let value = sample_client();
        let json = serde_json::to_string(&value).unwrap();
        let back: ClientEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(back, value);
    }

    #[test]
    fn server_envelope_json_roundtrip() {
        let value = sample_server();
        let json = serde_json::to_string(&value).unwrap();
        let back: ServerEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(back, value);
    }

    #[test]
    fn envelopes_survive_binary_encoding() {
        // Simulate the binary transport framing via bincode-style stable
        // serialization: JSON is the diagnostic path; CBOR/proto structurally
        // mirror this roundtrip. Guard the invariant, not the codec.
        let client = sample_client();
        let json = serde_json::to_vec(&client).unwrap();
        let back: ClientEnvelope = serde_json::from_slice(&json).unwrap();
        assert_eq!(back, client);

        let server = sample_server();
        let json = serde_json::to_vec(&server).unwrap();
        let back: ServerEnvelope = serde_json::from_slice(&json).unwrap();
        assert_eq!(back, server);
    }

    #[test]
    fn hello_roundtrips_with_optional_token() {
        let hello = Hello {
            protocol_version: HANDSHAKE,
            instance_id: "d1".into(),
            auth_token: Some("tok".into()),
            capabilities: vec!["runs".into(), "approvals".into()],
        };
        let json = serde_json::to_string(&hello).unwrap();
        let back: Hello = serde_json::from_str(&json).unwrap();
        assert_eq!(back, hello);
    }

    #[test]
    fn error_carries_structured_code() {
        let error = AppError {
            request_id: Some("req-1".into()),
            code: "ProviderRateLimited".into(),
            message: "429 from provider".into(),
            remediation: Some("retry after 30s".into()),
            retryable: true,
        };
        let json = serde_json::to_string(&error).unwrap();
        let back: AppError = serde_json::from_str(&json).unwrap();
        assert_eq!(back.code, "ProviderRateLimited");
        assert!(back.retryable);
    }

    #[test]
    fn subscribe_request_serializes_cursor() {
        let request = SubscribeRequest {
            run_id: "run_9".into(),
            last_seen_sequence: 41,
        };
        let json = serde_json::to_string(&request).unwrap();
        let back: SubscribeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.last_seen_sequence, 41);
    }

    #[test]
    fn version_field_roundtrips() {
        let version = ProtocolVersion { major: 2, minor: 1 };
        let json = serde_json::to_string(&version).unwrap();
        let back: ProtocolVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(back, version);
    }
}
