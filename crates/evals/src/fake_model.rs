//! Deterministic scripted model for tests: replies come from a queue, never
//! from the network. `complete` returns the next scripted reply and records the
//! exact request history so tests can assert what the caller sent.

use anyhow::Result;
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// One queued model response: either text, a set of tool calls, or an error
/// message. Token usage is fixed per reply so cost fixtures are stable.
#[derive(Clone, Debug)]
pub struct ScriptedReply {
    pub text: String,
    pub tool_calls: Vec<ScriptedToolCall>,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptedToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// Normalized model request the harness records on every call. Mirrors the
/// v1 provider client request shape so the same fixtures drive both loops.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordedRequest {
    pub messages: Vec<RecordedMessage>,
    pub tools: Vec<RecordedTool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecordedMessage {
    pub role: &'static str,
    pub content: String,
    pub tool_call_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecordedTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Clone, Debug, Default)]
struct ScriptedState {
    replies: Vec<ScriptedReply>,
    requests: Vec<RecordedRequest>,
}

/// Thread-safe scripted model. Clone shares the same queue/history so the
/// holder can inspect requests after driving an agent run.
#[derive(Clone, Default)]
pub struct ScriptedModel {
    state: Arc<Mutex<ScriptedState>>,
}

impl ScriptedModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues a plain-text reply.
    pub fn text(mut self, text: &str) -> Self {
        self.push(ScriptedReply {
            text: text.to_owned(),
            tool_calls: Vec::new(),
            prompt_tokens: 10,
            completion_tokens: 5,
            error: None,
        });
        self
    }

    /// Queues a tool-call reply followed by a terminal text reply.
    pub fn tool_then_text(self, call: ScriptedToolCall, final_text: &str) -> Self {
        self.tool(vec![call]).text(final_text)
    }

    /// Queues a reply that requests tool calls.
    pub fn tool(mut self, calls: Vec<ScriptedToolCall>) -> Self {
        self.push(ScriptedReply {
            text: String::new(),
            tool_calls: calls,
            prompt_tokens: 20,
            completion_tokens: 8,
            error: None,
        });
        self
    }

    /// Queues a provider error.
    pub fn error(mut self, message: &str) -> Self {
        self.push(ScriptedReply {
            text: String::new(),
            tool_calls: Vec::new(),
            prompt_tokens: 0,
            completion_tokens: 0,
            error: Some(message.to_owned()),
        });
        self
    }

    fn push(&mut self, reply: ScriptedReply) {
        self.state
            .lock()
            .expect("scripted model lock")
            .replies
            .push(reply);
    }

    /// Next scripted reply. Exhausting the queue is a test-authoring bug.
    pub fn complete(
        &self,
        messages: &[RecordedMessage],
        tools: &[RecordedTool],
    ) -> Result<ScriptedReply> {
        let mut state = self.state.lock().expect("scripted model lock");
        state.requests.push(RecordedRequest {
            messages: messages.to_vec(),
            tools: tools.to_vec(),
        });
        let reply = state
            .replies
            .pop_front_hole()
            .ok_or_else(|| anyhow::anyhow!("scripted model exhausted; queue more replies"))?;
        match reply.error {
            Some(message) => Err(anyhow::anyhow!(message)),
            None => Ok(reply),
        }
    }

    /// Full request history in call order.
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.state
            .lock()
            .expect("scripted model lock")
            .requests
            .clone()
    }

    /// Number of queued replies still pending.
    pub fn remaining(&self) -> usize {
        self.state
            .lock()
            .expect("scripted model lock")
            .replies
            .len()
    }
}

trait PopFront {
    fn pop_front_hole(&mut self) -> Option<ScriptedReply>;
}

impl PopFront for Vec<ScriptedReply> {
    fn pop_front_hole(&mut self) -> Option<ScriptedReply> {
        if self.is_empty() {
            None
        } else {
            Some(self.remove(0))
        }
    }
}

/// Builds a tool call with a fixed id so serialization is deterministic.
pub fn call(name: &str, arguments: Value) -> ScriptedToolCall {
    ScriptedToolCall {
        id: format!("call_{name}"),
        name: name.to_owned(),
        arguments,
    }
}
