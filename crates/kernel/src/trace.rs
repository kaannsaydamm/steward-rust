//! Trace/span model (§38, Task 21.1, §13.9): canonical hierarchy
//! RunSpan -> AgentSpan -> TurnSpan -> {Context,Routing,Generation,Tool,
//! Guardrail}Span, plus WorkflowNodeSpan/CodeRuntimeSpan/ApprovalSpan/
//! VerificationSpan. Every generation carries tokens/route/manifest id;
//! every tool carries effects/approval/duration/redacted digest.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanKind {
    Run,
    Agent,
    Turn,
    ContextBuild,
    Routing,
    Generation,
    Tool,
    Guardrail,
    Subagent,
    WorkflowNode,
    CodeRuntime,
    Approval,
    Verification,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub run_id: String,
    pub kind: SpanKind,
    pub name: String,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    /// Structured attributes per §13.9.
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub status: SpanStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanStatus {
    Running,
    Ok,
    Error,
    Cancelled,
}

/// Attribute keys (stable schema; the UI reads these).
pub mod attrs {
    pub const MODEL_ROUTE: &str = "model.route";
    pub const MODEL_PROVIDER: &str = "model.provider";
    pub const PROMPT_TOKENS: &str = "tokens.prompt";
    pub const COMPLETION_TOKENS: &str = "tokens.completion";
    pub const CACHE_READ_TOKENS: &str = "tokens.cache_read";
    pub const COST_MICROUSD: &str = "cost.microusd";
    pub const CONTEXT_MANIFEST_ID: &str = "context.manifest_id";
    pub const ROUTE_RATIONALE: &str = "routing.rationale";
    pub const TOOL_ID: &str = "tool.id";
    pub const TOOL_EFFECTS: &str = "tool.effects";
    pub const TOOL_APPROVAL: &str = "tool.approval_decision";
    pub const TOOL_ARGS_DIGEST: &str = "tool.args_digest";
    pub const VERIFIER_OUTCOME: &str = "verification.outcome";
}

/// In-memory span collector (persisted to trace_store in the daemon).
#[derive(Debug, Default)]
pub struct TraceCollector {
    by_id: BTreeMap<String, Span>,
    order: Vec<String>,
}

impl TraceCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&mut self, span: Span) {
        self.order.push(span.span_id.clone());
        self.by_id.insert(span.span_id.clone(), span);
    }

    pub fn finish(
        &mut self,
        span_id: &str,
        status: SpanStatus,
        extra_attributes: Option<BTreeMap<String, serde_json::Value>>,
    ) -> Option<Span> {
        let span = self.by_id.get_mut(span_id)?;
        span.ended_at_ms = Some(now_ms());
        span.status = status;
        if let Some(extra) = extra_attributes {
            span.attributes.extend(extra);
        }
        Some(span.clone())
    }

    /// Reconstructs the canonical tree (Task 21.1 acceptance: Run -> Agent ->
    /// Turn -> Context/Model/Tool hierarchy reconstructed from spans).
    pub fn tree(&self) -> Vec<(usize, Span)> {
        let mut depth_of: BTreeMap<String, usize> = BTreeMap::new();
        let mut output = Vec::new();
        for id in &self.order {
            let span = &self.by_id[id];
            let depth = match &span.parent_span_id {
                Some(parent) => depth_of.get(parent).copied().unwrap_or(0) + 1,
                None => 0,
            };
            depth_of.insert(span.span_id.clone(), depth);
            output.push((depth, span.clone()));
        }
        output
    }

    pub fn span(&self, span_id: &str) -> Option<&Span> {
        self.by_id.get(span_id)
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}

/// Builders for canonical spans with required attributes.
pub fn generation_span(
    span_id: &str,
    parent: &str,
    run_id: &str,
    route: &str,
    provider: &str,
    manifest_id: &str,
    rationale: &str,
) -> Span {
    let mut attributes = BTreeMap::new();
    attributes.insert(attrs::MODEL_ROUTE.to_owned(), serde_json::json!(route));
    attributes.insert(
        attrs::MODEL_PROVIDER.to_owned(),
        serde_json::json!(provider),
    );
    attributes.insert(
        attrs::CONTEXT_MANIFEST_ID.to_owned(),
        serde_json::json!(manifest_id),
    );
    attributes.insert(
        attrs::ROUTE_RATIONALE.to_owned(),
        serde_json::json!(rationale),
    );
    Span {
        span_id: span_id.to_owned(),
        parent_span_id: Some(parent.to_owned()),
        run_id: run_id.to_owned(),
        kind: SpanKind::Generation,
        name: "generation".into(),
        started_at_ms: now_ms(),
        ended_at_ms: None,
        attributes,
        status: SpanStatus::Running,
    }
}

pub fn tool_span(
    span_id: &str,
    parent: &str,
    run_id: &str,
    tool_id: &str,
    effects: &[&str],
    approval: &str,
    args_digest: &str,
) -> Span {
    let mut attributes = BTreeMap::new();
    attributes.insert(attrs::TOOL_ID.to_owned(), serde_json::json!(tool_id));
    attributes.insert(attrs::TOOL_EFFECTS.to_owned(), serde_json::json!(effects));
    attributes.insert(attrs::TOOL_APPROVAL.to_owned(), serde_json::json!(approval));
    attributes.insert(
        attrs::TOOL_ARGS_DIGEST.to_owned(),
        serde_json::json!(args_digest),
    );
    Span {
        span_id: span_id.to_owned(),
        parent_span_id: Some(parent.to_owned()),
        run_id: run_id.to_owned(),
        kind: SpanKind::Tool,
        name: format!("tool:{tool_id}"),
        started_at_ms: now_ms(),
        ended_at_ms: None,
        attributes,
        status: SpanStatus::Running,
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_tree_reconstructs_hierarchy() {
        let mut collector = TraceCollector::new();
        collector.start(Span {
            span_id: "run".into(),
            parent_span_id: None,
            run_id: "r1".into(),
            kind: SpanKind::Run,
            name: "run".into(),
            started_at_ms: 1,
            ended_at_ms: None,
            attributes: BTreeMap::new(),
            status: SpanStatus::Running,
        });
        collector.start(Span {
            span_id: "agent".into(),
            parent_span_id: Some("run".into()),
            run_id: "r1".into(),
            kind: SpanKind::Agent,
            name: "main agent".into(),
            started_at_ms: 2,
            ended_at_ms: None,
            attributes: BTreeMap::new(),
            status: SpanStatus::Running,
        });
        collector.start(Span {
            span_id: "turn".into(),
            parent_span_id: Some("agent".into()),
            run_id: "r1".into(),
            kind: SpanKind::Turn,
            name: "turn 1".into(),
            started_at_ms: 3,
            ended_at_ms: None,
            attributes: BTreeMap::new(),
            status: SpanStatus::Running,
        });
        collector.start(generation_span(
            "gen",
            "turn",
            "r1",
            "auto/coding",
            "openai",
            "mf_1",
            "task_affinity score=0.8",
        ));

        let tree = collector.tree();
        let names: Vec<(&str, usize)> = tree
            .iter()
            .map(|(depth, span)| (span.span_id.as_str(), *depth))
            .collect();
        assert_eq!(
            names,
            vec![("run", 0), ("agent", 1), ("turn", 2), ("gen", 3)]
        );
    }

    #[test]
    fn generation_span_carries_required_attributes() {
        let span = generation_span("g", "t", "r", "route", "prov", "mf", "why");
        assert_eq!(
            span.attributes
                .get(attrs::MODEL_ROUTE)
                .and_then(|v| v.as_str()),
            Some("route")
        );
        assert_eq!(
            span.attributes
                .get(attrs::CONTEXT_MANIFEST_ID)
                .and_then(|v| v.as_str()),
            Some("mf")
        );
        assert_eq!(
            span.attributes
                .get(attrs::ROUTE_RATIONALE)
                .and_then(|v| v.as_str()),
            Some("why")
        );
    }

    #[test]
    fn tool_span_carries_effects_and_approval_and_digest() {
        let span = tool_span(
            "t",
            "turn",
            "r",
            "fs.write",
            &["filesystem.write"],
            "granted:apr_1",
            "abc123",
        );
        assert_eq!(
            span.attributes
                .get(attrs::TOOL_EFFECTS)
                .and_then(|v| v.as_array())
                .map(|a| a.len()),
            Some(1)
        );
        assert_eq!(
            span.attributes
                .get(attrs::TOOL_APPROVAL)
                .and_then(|v| v.as_str()),
            Some("granted:apr_1")
        );
        assert_eq!(
            span.attributes
                .get(attrs::TOOL_ARGS_DIGEST)
                .and_then(|v| v.as_str()),
            Some("abc123")
        );
    }

    #[test]
    fn finish_records_outcome_and_latency_window() {
        let mut collector = TraceCollector::new();
        let span = generation_span("g", "t", "r", "route", "prov", "mf", "why");
        collector.start(span);
        let mut extra = BTreeMap::new();
        extra.insert(attrs::PROMPT_TOKENS.to_owned(), json!(100));
        extra.insert(attrs::COMPLETION_TOKENS.to_owned(), json!(50));
        let finished = collector.finish("g", SpanStatus::Ok, Some(extra)).unwrap();
        assert!(finished.ended_at_ms.is_some());
        assert_eq!(
            finished
                .attributes
                .get(attrs::PROMPT_TOKENS)
                .and_then(|v| v.as_u64()),
            Some(100)
        );
    }

    #[test]
    fn spans_serialize_for_persistence() {
        let span = tool_span(
            "t",
            "turn",
            "r",
            "fs.read",
            &["filesystem.read"],
            "granted",
            "digest",
        );
        let json = serde_json::to_string(&span).unwrap();
        let back: Span = serde_json::from_str(&json).unwrap();
        assert_eq!(back, span);
    }
}
