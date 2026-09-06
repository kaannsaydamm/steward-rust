//! DAP conformance against a mock adapter (Task 10.3 acceptance).

use super::dap::*;
use std::sync::Arc;
use async_trait::async_trait;
use serde_json::Value;
use anyhow::Result as AnyhowResult;
use parking_lot::Mutex;
use serde_json::json;

#[derive(Default)]
struct MockAdapter {
    calls: Mutex<Vec<(String, Value)>>,
}

#[async_trait]
impl DapTransport for MockAdapter {
    async fn request(&self, command: &str, args: Value) -> AnyhowResult<Value> {
        self.calls.lock().push((command.to_owned(), args.clone()));
        match command {
            "setBreakpoints" => Ok(json!({"breakpoints": [
                {"verified": true, "line": 9}
            ]})),
            "stackTrace" => Ok(json!({"stackFrames": [
                {"id": 1, "name": "main", "source": {"path": "/repo/src/main.rs"}, "line": 10},
                {"id": 2, "name": "run_kernel", "source": {"path": "/repo/src/kernel.rs"}, "line": 42}
            ]})),
            "scopes" => Ok(json!({"scopes": [{"name": "Locals", "variablesReference": 7}]})),
            "variables" => Ok(json!({"variables": [
                {"name": "state", "value": "Running", "type": "ProcessStatus"},
                {"name": "attempts", "value": "3", "type": "u32"}
            ]})),
            "evaluate" => Ok(json!({"result": "42", "type": "u32"})),
            _ => Ok(json!({})),
        }
    }
}

#[tokio::test]
async fn launch_requires_policy_grant() {
    let manager = DapManager::default();
    let result = manager.launch("s1", "/repo/app", Arc::new(MockAdapter::default()), false).await;
    assert!(result.is_err());
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("launch must be refused without effect grant"),
    };
    assert!(error.to_string().contains("effect not granted"));
}

#[tokio::test]
async fn launch_initialize_then_launch_commands() {
    let adapter = Arc::new(MockAdapter::default());
    let manager = DapManager::default();
    let session = manager.launch("s1", "/repo/app", adapter.clone(), true).await.unwrap();
    let calls = adapter.calls.lock();
    assert_eq!(calls[0].0, "initialize");
    assert_eq!(calls[1].0, "launch");
    assert_eq!(calls[1].1.get("program").and_then(Value::as_str), Some("/repo/app"));
    assert_eq!(manager.session_count(), 1);
    let _ = session;
}

#[tokio::test]
async fn breakpoints_roundtrip_with_verification() {
    let adapter = Arc::new(MockAdapter::default());
    let manager = DapManager::default();
    let session = manager.launch("s1", "/repo/app", adapter.clone(), true).await.unwrap();
    let breakpoints = session.set_breakpoints("/repo/src/main.rs", &[10]).await.unwrap();
    assert_eq!(breakpoints.len(), 1);
    assert!(breakpoints[0].verified);
    assert_eq!(breakpoints[0].line, 10, "0-based +1");
    let calls = adapter.calls.lock();
    assert_eq!(calls[2].0, "setBreakpoints");
}

#[tokio::test]
async fn stack_frames_normalize_to_one_based_lines() {
    let adapter = Arc::new(MockAdapter::default());
    let manager = DapManager::default();
    let session = manager.launch("s1", "/repo/app", adapter.clone(), true).await.unwrap();
    let frames = session.stack_trace(1).await.unwrap();
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].name, "main");
    assert_eq!(frames[0].file, "/repo/src/main.rs");
    assert_eq!(frames[1].line, 42);
}

#[tokio::test]
async fn variables_resolved_through_scope_reference() {
    let adapter = Arc::new(MockAdapter::default());
    let manager = DapManager::default();
    let session = manager.launch("s1", "/repo/app", adapter.clone(), true).await.unwrap();
    let variables = session.variables(1).await.unwrap();
    assert_eq!(variables.len(), 2);
    assert_eq!(variables[0].name, "state");
    assert_eq!(variables[0].type_name.as_deref(), Some("ProcessStatus"));
    let calls = adapter.calls.lock();
    assert_eq!(calls[2].0, "scopes");
    assert_eq!(calls[3].0, "variables");
    assert_eq!(calls[3].1.get("variablesReference").and_then(Value::as_u64), Some(7));
}

#[tokio::test]
async fn evaluate_returns_result_string() {
    let adapter = Arc::new(MockAdapter::default());
    let manager = DapManager::default();
    let session = manager.launch("s1", "/repo/app", adapter.clone(), true).await.unwrap();
    let result = session.evaluate(1, "attempts * 14").await.unwrap();
    assert_eq!(result, "42");
}

#[tokio::test]
async fn terminate_disconnects_and_releases() {
    let adapter = Arc::new(MockAdapter::default());
    let manager = DapManager::default();
    let session = manager.launch("s1", "/repo/app", adapter.clone(), true).await.unwrap();
    session.terminate().await.unwrap();
    let calls = adapter.calls.lock();
    assert_eq!(calls.last().map(|(command, _)| command.as_str()), Some("disconnect"));
}

#[tokio::test]
async fn released_sessions_are_counted() {
    let manager = DapManager::default();
    let _session = manager.launch("s1", "/a", Arc::new(MockAdapter::default()), true).await.unwrap();
    assert!(manager.release("s1"));
    assert_eq!(manager.session_count(), 0);
    assert!(!manager.release("s1"));
}
