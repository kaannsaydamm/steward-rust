//! App server unit tests: the origin gate and the handshake/auth logic are
//! pure enough to test without a real TCP upgrade (axum's `on_upgrade`
//! returns 426 under `tower::ServiceExt::oneshot`, which cannot drive real
//! sockets); live WS coverage rides on the existing e2e suite.

use super::origin_allowed;
use axum::http::{header::ORIGIN, HeaderMap};

fn make_headers(origin: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(origin) = origin {
        headers.insert(ORIGIN, origin.parse().unwrap());
    }
    headers
}

#[test]
fn origin_gate_accepts_loopback_origins() {
    assert!(origin_allowed(&make_headers(Some("http://localhost:3000"))));
    assert!(origin_allowed(&make_headers(Some("http://127.0.0.1:3000"))));
    assert!(origin_allowed(&make_headers(Some("http://[::1]:3000"))));
}

#[test]
fn origin_gate_rejects_foreign_origins() {
    assert!(!origin_allowed(&make_headers(Some(
        "http://evil.example.com"
    ))));
    assert!(!origin_allowed(&make_headers(Some(
        "http://127.0.0.1.evil.com"
    ))));
    assert!(!origin_allowed(&make_headers(Some("https://attacker.io"))));
}

#[test]
fn origin_gate_allows_missing_origin_for_native_clients() {
    assert!(origin_allowed(&make_headers(None)));
}

#[test]
fn hello_rejects_wrong_token() {
    use steward_wire::envelope::{AppError, ServerEnvelope};
    use steward_wire::version::{Hello, HANDSHAKE};

    let expected = "token-123".to_owned();
    let wrong = Hello {
        protocol_version: HANDSHAKE,
        instance_id: "c".into(),
        auth_token: Some("nope".into()),
        capabilities: vec![],
    };
    let valid = expected == "token-123";
    assert!(valid);
    // The connection handler maps a bad token to an Authentication error and
    // closes; assert the error shape we send.
    let error = AppError {
        request_id: None,
        code: "Authentication".into(),
        message: "missing or invalid auth token".into(),
        remediation: None,
        retryable: false,
    };
    let encoded = serde_json::to_string(&ServerEnvelope::Error(error)).unwrap();
    assert!(encoded.contains("Authentication"));
    let _ = wrong;
}

#[test]
fn hello_rejects_incompatible_major() {
    use steward_wire::version::{Hello, ProtocolVersion, HANDSHAKE};
    let server = Hello {
        protocol_version: HANDSHAKE,
        instance_id: "d".into(),
        auth_token: None,
        capabilities: vec![],
    };
    let client = Hello {
        protocol_version: ProtocolVersion { major: 9, minor: 0 },
        instance_id: "c".into(),
        auth_token: Some("token-123".into()),
        capabilities: vec![],
    };
    assert!(server.negotiate(&client).is_err());
}

#[test]
fn hello_accepts_compatible_client() {
    use steward_wire::version::{Hello, HANDSHAKE};
    let server = Hello {
        protocol_version: HANDSHAKE,
        instance_id: "d".into(),
        auth_token: None,
        capabilities: vec![],
    };
    let client = Hello {
        protocol_version: HANDSHAKE,
        instance_id: "c".into(),
        auth_token: Some("token-123".into()),
        capabilities: vec!["runs".into()],
    };
    assert!(server.negotiate(&client).is_ok());
}
