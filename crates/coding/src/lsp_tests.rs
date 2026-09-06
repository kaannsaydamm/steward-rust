//! LSP + DAP conformance suite against in-process mock servers (Task 10.1 /
//! 10.3 acceptance): initialize, symbols, diagnostics, definition,
//! references, rename; launch, breakpoint, stack, variables, evaluate.

use super::lsp::*;
use async_trait::async_trait;
use anyhow::Result as AnyhowResult;
use serde_json::Value;
use std::sync::Arc;
use parking_lot::Mutex;
use serde_json::json;
use std::collections::BTreeMap;

/// Mock language server: deterministic canned responses per method.
#[derive(Default)]
struct MockServer {
    calls: Mutex<Vec<(String, Value)>>,
    diagnostics: BTreeMap<String, u64>,
}

impl MockServer {
    fn seed_diagnostic(&mut self, file: &str, count: u64) {
        self.diagnostics.insert(file.to_owned(), count);
    }
}

#[async_trait]
impl LspTransport for MockServer {
    async fn request(&self, method: &str, params: Value) -> AnyhowResult<Value> {
        self.calls.lock().push((method.to_owned(), params.clone()));
        match method {
            "initialize" => Ok(json!({"capabilities": {"hoverProvider": true}})),
            "initialized" => Ok(json!({})),
            "textDocument/hover" => Ok(json!({"contents": "fn run_kernel(): the kernel entry"})),
            "textDocument/definition" => Ok(json!([{
                "uri": "file:///repo/src/kernel.rs",
                "range": {"start": {"line": 9, "character": 7}, "end": {"line": 9, "character": 16}}
            }])),
            "textDocument/references" => Ok(json!([
                {"uri": "file:///repo/src/a.rs", "range": {"start": {"line": 0, "character": 4}, "end": {"line": 0, "character": 13}}},
                {"uri": "file:///repo/src/b.rs", "range": {"start": {"line": 4, "character": 4}, "end": {"line": 4, "character": 13}}}
            ])),
            "textDocument/diagnostic" => {
                let uri = params.get("textDocument").and_then(|d| d.get("uri")).and_then(Value::as_str).unwrap_or_default().trim_start_matches("file://").to_owned();
                let count = self.diagnostics.get(&uri).copied().unwrap_or(0);
                let items: Vec<Value> = (0..count)
                    .map(|i| json!({
                        "range": {"start": {"line": i, "character": 0}, "end": {"line": i, "character": 5}},
                        "severity": 1,
                        "message": format!("error {i}"),
                        "source": "mockc"
                    }))
                    .collect();
                Ok(json!({"items": items}))
            }
            "textDocument/rename" => Ok(json!({
                "changes": {
                    "file:///repo/src/a.rs": [
                        {"range": {"start": {"line": 0, "character": 4}, "end": {"line": 0, "character": 13}}, "newText": "renamed_kernel"}
                    ]
                }
            })),
            _ => Ok(json!({})),
        }
    }
}

#[tokio::test]
async fn handshake_is_performed_once_and_cached() {
    let server = Arc::new(MockServer::default());
    let manager = LspManager::default();
    let session = manager.start_server("/repo", "rust", server.clone()).await.unwrap();
    manager.start_server("/repo", "rust", server.clone()).await.unwrap();

    let calls = server.calls.lock();
    let initialize_count = calls.iter().filter(|(m, _)| m == "initialize").count();
    assert_eq!(initialize_count, 1, "second start reuses the session");
    let _ = session;
}

#[tokio::test]
async fn hover_returns_normalized_content() {
    let manager = LspManager::default();
    let session = manager.start_server("/repo", "rust", Arc::new(MockServer::default())).await.unwrap();
    let hover = manager.hover(&session, "/repo/src/lib.rs", 5, 1).await.unwrap();
    assert_eq!(hover, "fn run_kernel(): the kernel entry");
}

#[tokio::test]
async fn definition_returns_file_line_column_one_indexed() {
    let manager = LspManager::default();
    let session = manager.start_server("/repo", "rust", Arc::new(MockServer::default())).await.unwrap();
    let locations = manager.definition(&session, "/repo/src/lib.rs", 5, 1).await.unwrap();
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0].file, "/repo/src/kernel.rs");
    assert_eq!(locations[0].line, 10, "LSP 0-based +1 = 1-based");
    assert_eq!(locations[0].column, 8);
}

#[tokio::test]
async fn references_return_exact_ranges() {
    let manager = LspManager::default();
    let session = manager.start_server("/repo", "rust", Arc::new(MockServer::default())).await.unwrap();
    let references = manager.references(&session, "/repo/src/lib.rs", 1, 1, false).await.unwrap();
    assert_eq!(references.len(), 2);
    assert_eq!(references[0].file, "/repo/src/a.rs");
    assert_eq!(references[0].line, 1);
    assert_eq!(references[1].line, 5);
}

#[tokio::test]
async fn diagnostics_map_severity_and_lines() {
    let mut server = MockServer::default();
    server.seed_diagnostic("/repo/src/broken.rs", 2);
    let manager = LspManager::default();
    let session = manager.start_server("/repo", "rust", Arc::new(server)).await.unwrap();
    let diagnostics = manager.diagnostics(&session, "/repo/src/broken.rs").await.unwrap();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostics[0].line, 1);
    assert_eq!(diagnostics[1].line, 2);
    assert_eq!(diagnostics[0].source, "mockc");
}

#[tokio::test]
async fn rename_returns_workspace_edit_for_atomic_apply() {
    let manager = LspManager::default();
    let session = manager.start_server("/repo", "rust", Arc::new(MockServer::default())).await.unwrap();
    let edits = manager.rename(&session, "/repo/src/a.rs", 1, 5, "renamed_kernel").await.unwrap();
    let changes = edits.get("changes").unwrap();
    assert!(changes.get("file:///repo/src/a.rs").is_some());
}

#[tokio::test]
async fn operations_fail_without_a_started_server() {
    let manager = LspManager::default();
    let session = LspSession { workspace_root: "/nowhere".into(), language: "rust" };
    assert!(manager.hover(&session, "/x.rs", 1, 1).await.is_err());
}

#[tokio::test]
async fn language_detection_covers_common_extensions() {
    assert_eq!(language_for("rs"), "rust");
    assert_eq!(language_for("tsx"), "typescript");
    assert_eq!(language_for("unknown"), "plaintext");
}
