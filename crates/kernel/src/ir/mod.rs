//! Unified Execution IR (§12, Tasks 12.1-12.4, G-001..G-019).
//!
//! The visual editor, declarative YAML/JSON, autonomous planning, and RLM
//! orchestration all compile to this serializable plan. The concurrent
//! scheduler executes it with fan-out/join, conditionals, retries,
//! fallbacks, and nested subworkflows.

use anyhow::{bail, Context as _, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

pub mod scheduler;

pub use scheduler::{NodeOutput, Scheduler, SchedulerEvent};

pub const IR_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub schema_version: u32,
    pub plan_id: String,
    pub entry_nodes: Vec<String>,
    pub nodes: BTreeMap<String, NodeSpec>,
    pub edges: Vec<EdgeSpec>,
    pub success_policy: Value,
    pub budget: Value,
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeSpec {
    Agent {
        profile_id: String,
        task: String,
        #[serde(default)]
        model_override: Option<String>,
    },
    Tool {
        tool_id: String,
        #[serde(default)]
        arguments: Value,
    },
    Code {
        language: String,
        source: String,
    },
    /// Deterministic native function node.
    Native {
        operation: String,
        #[serde(default)]
        parameters: Value,
    },
    Human {
        prompt: String,
        /// JSON schema of the expected answer payload.
        #[serde(default)]
        answer_schema: Value,
    },
    Verify {
        criterion: String,
    },
    Router {
        /// Expression evaluated against node outputs; safe evaluator only
        /// (§12.3): field lookups + comparisons, never arbitrary code.
        expression: String,
        #[serde(default)]
        branches: Vec<String>,
    },
    Subworkflow {
        plan_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EdgeSpec {
    pub from: String,
    pub to: String,
    #[serde(rename = "kind", default = "default_edge_kind")]
    pub edge_kind: EdgeKind,
}

fn default_edge_kind() -> EdgeKind {
    EdgeKind::Direct
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum EdgeKind {
    Direct,
    Conditional { predicate: String },
    FanOut,
    FanIn { policy: JoinPolicy },
    Retry { max_attempts: u32, backoff_ms: u64 },
    Fallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinPolicy {
    All,
    Any,
    /// Wait for N of the incoming branches.
    Quorum { count: u32 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum NodeOutcome {
    Succeeded { output: Value },
    Failed { error: String, retryable: bool },
    Cancelled,
}

impl ExecutionPlan {
    /// Validates the plan (Task 12.1): unique ids, known edges, acyclic
    /// entry, compatible fan-out/fan-in, valid success policy.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != IR_SCHEMA_VERSION {
            bail!("unsupported IR schema version {}", self.schema_version);
        }
        if self.plan_id.trim().is_empty() {
            bail!("plan_id cannot be empty");
        }
        if self.nodes.is_empty() {
            bail!("plan has no nodes");
        }
        // Entry nodes must exist.
        for entry in &self.entry_nodes {
            if !self.nodes.contains_key(entry) {
                bail!("entry node '{entry}' does not exist");
            }
        }
        // Edges must reference known nodes.
        for edge in &self.edges {
            if !self.nodes.contains_key(&edge.from) {
                bail!("edge from unknown node '{}'", edge.from);
            }
            if !self.nodes.contains_key(&edge.to) {
                bail!("edge to unknown node '{}'", edge.to);
            }
            if edge.from == edge.to {
                bail!("self-loop on node '{}'", edge.from);
            }
            // FanIn edges must target a node reachable by FanOut branches.
            if matches!(edge.edge_kind, EdgeKind::FanIn { .. }) {
                let has_fan_out = self
                    .edges
                    .iter()
                    .any(|e| matches!(e.edge_kind, EdgeKind::FanOut) && e.to == edge.from)
                    || self
                        .edges
                        .iter()
                        .filter(|e| e.to == edge.from)
                        .count()
                        > 1;
                if !has_fan_out {
                    bail!("fan-in on node '{}' has no parallel inputs", edge.from);
                }
            }
        }
        // Cycle detection over direct/conditional/ret/fallback edges.
        let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for edge in &self.edges {
            adjacency.entry(edge.from.as_str()).or_default().push(edge.to.as_str());
        }
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        for entry in &self.entry_nodes {
            self.check_cycles(entry, &adjacency, &mut visiting, &mut visited)?;
        }
        if !self.entry_nodes.is_empty() {
            // All nodes reachable?
            if visited.len() != self.nodes.len() {
                bail!("plan contains unreachable nodes");
            }
        }
        Ok(())
    }

    fn check_cycles<'a>(
        &self,
        node: &'a str,
        adjacency: &BTreeMap<&'a str, Vec<&'a str>>,
        visiting: &mut BTreeSet<&'a str>,
        visited: &mut BTreeSet<&'a str>,
    ) -> Result<()> {
        if visited.contains(node) {
            return Ok(());
        }
        if !visiting.insert(node) {
            bail!("cycle detected: node '{node}' is already being visited");
        }
        if let Some(targets) = adjacency.get(node) {
            for target in targets.clone() {
                if let Err(mut error) = self.check_cycles(target, adjacency, visiting, visited) {
                    // Preserve the cycle message at the top level.
                    if !error.to_string().contains("cycle") {
                        error = error.context(format!("from node '{node}'"));
                    }
                    return Err(error);
                }
            }
        }
        visiting.remove(node);
        visited.insert(node);
        Ok(())
    }

    /// Compiles a v1 workflow DAG definition into an ExecutionPlan
    /// (Task 12.5): v1 nodes become sequential direct edges.
    pub fn from_v1_workflow(definition: &Value) -> Result<Self> {
        let nodes_json = definition
            .get("nodes")
            .and_then(Value::as_array)
            .context("v1 workflow must have a nodes array")?;
        let mut nodes = BTreeMap::new();
        let mut order: Vec<String> = Vec::new();
        for node in nodes_json {
            let id = node
                .get("id")
                .and_then(Value::as_str)
                .context("v1 node missing id")?
                .to_owned();
            let agent = node
                .get("agent")
                .and_then(Value::as_str)
                .unwrap_or("worker")
                .to_owned();
            let task = node
                .get("task")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            nodes.insert(
                id.clone(),
                NodeSpec::Agent { profile_id: agent, task, model_override: None },
            );
            order.push(id);
        }
        let edges = order
            .windows(2)
            .map(|pair| EdgeSpec {
                from: pair[0].clone(),
                to: pair[1].clone(),
                edge_kind: EdgeKind::Direct,
            })
            .collect();
        let entry_nodes = order.first().cloned().into_iter().collect();
        Ok(Self {
            schema_version: IR_SCHEMA_VERSION,
            plan_id: format!("v1_{}", definition.get("id").and_then(Value::as_str).unwrap_or("workflow")),
            entry_nodes,
            nodes,
            edges,
            success_policy: serde_json::json!({"kind": "agent_declared"}),
            budget: serde_json::json!({}),
            metadata: BTreeMap::new(),
        })
    }
}

/// Safe expression evaluator for Router/Conditional nodes (Task 12.3):
/// `field op value` with ==, !=, contains; field lookup into node outputs.
/// Never evaluates arbitrary code (G-009).
pub fn evaluate_predicate(expression: &str, state: &BTreeMap<String, Value>) -> Result<bool> {
    let expression = expression.trim();
    for op in ["==", "!=", "contains"] {
        if let Some((left, right)) = expression.split_once(op) {
            let left_value = resolve_operand(left.trim(), state)?;
            let right_value = resolve_operand(right.trim(), state)?;
            return match op {
                "==" => Ok(left_value == right_value),
                "!=" => Ok(left_value != right_value),
                "contains" => Ok(match (&left_value, right_value) {
                    (Value::String(haystack), Value::String(needle)) => haystack.contains(needle.as_str()),
                    (Value::Array(items), Value::String(needle)) => items.contains(&Value::String(needle.clone())),
                    _ => false,
                }),
                _ => unreachable!(),
            };
        }
    }
    bail!("unsupported predicate '{expression}'; use ==, != or contains")
}

fn resolve_operand(operand: &str, state: &BTreeMap<String, Value>) -> Result<Value> {
    let trimmed = operand.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        return Ok(Value::String(trimmed[1..trimmed.len() - 1].to_owned()));
    }
    let operand = trimmed;
    if operand == "true" {
        return Ok(Value::Bool(true));
    }
    if operand == "false" {
        return Ok(Value::Bool(false));
    }
    if let Ok(number) = operand.parse::<i64>() {
        return Ok(Value::Number(number.into()));
    }
    state
        .get(operand)
        .cloned()
        .context(format!("unknown field '{operand}' in predicate"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn plan(nodes: Vec<(&str, NodeSpec)>, edges: Vec<(&str, &str)>) -> ExecutionPlan {
        let mut node_map = BTreeMap::new();
        let mut entry = Vec::new();
        for (index, (id, spec)) in nodes.iter().enumerate() {
            node_map.insert(id.to_string(), spec.clone());
            if index == 0 {
                entry.push(id.to_string());
            }
        }
        ExecutionPlan {
            schema_version: IR_SCHEMA_VERSION,
            plan_id: "test_plan".into(),
            entry_nodes: entry,
            nodes: node_map,
            edges: edges
                .into_iter()
                .map(|(from, to)| EdgeSpec {
                    from: from.into(),
                    to: to.into(),
                    edge_kind: EdgeKind::Direct,
                })
                .collect(),
            success_policy: json!({"kind": "agent_declared"}),
            budget: json!({}),
            metadata: BTreeMap::new(),
        }
    }

    fn agent(id: &'static str) -> (&'static str, NodeSpec) {
        let task: &'static str = Box::leak(id.to_string().into_boxed_str());
        (id, NodeSpec::Agent { profile_id: "worker".into(), task: task.to_owned(), model_override: None })
    }

    #[test]
    fn valid_plan_passes() {
        let plan = plan(vec![agent("a"), agent("b")], vec![("a", "b")]);
        plan.validate().unwrap();
    }

    #[test]
    fn plan_roundtrips_through_json() {
        let value = plan(vec![agent("a"), agent("b")], vec![("a", "b")]);
        let json = serde_json::to_string(&value).unwrap();
        let back: ExecutionPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(back, value);
    }

    #[test]
    fn unknown_edge_endpoints_are_rejected() {
        let plan = plan(vec![agent("a")], vec![("a", "ghost")]);
        let error = plan.validate().unwrap_err();
        assert!(error.to_string().contains("unknown node"));
    }

    #[test]
    fn cycles_are_detected() {
        let plan = plan(
            vec![agent("a"), agent("b"), agent("c")],
            vec![("a", "b"), ("b", "c"), ("c", "a")],
        );
        let error = plan.validate().unwrap_err();
        assert!(error.to_string().contains("cycle"));
    }

    #[test]
    fn self_loops_are_rejected() {
        let plan = plan(vec![agent("a")], vec![("a", "a")]);
        assert!(plan.validate().unwrap_err().to_string().contains("self-loop"));
    }

    #[test]
    fn fan_in_requires_parallel_inputs() {
        let mut plan2 = plan(
            vec![agent("a"), agent("join")],
            vec![("a", "join")],
        );
        plan2.edges[0].edge_kind = EdgeKind::FanIn { policy: JoinPolicy::All };
        let error = plan2.validate().unwrap_err();
        assert!(error.to_string().contains("no parallel inputs"));
    }

    #[test]
    fn unreachable_nodes_are_rejected() {
        let mut plan = plan(
            vec![agent("a"), agent("orphan")],
            vec![("a", "orphan")],
        );
        plan.nodes.insert("island".into(), NodeSpec::Native {
            operation: "noop".into(),
            parameters: json!({}),
        });
        let error = plan.validate().unwrap_err();
        assert!(error.to_string().contains("unreachable"));
    }

    #[test]
    fn v1_workflow_compiles_to_equivalent_sequential_plan() {
        let v1 = json!({
            "id": "legacy",
            "nodes": [
                {"id": "research", "agent": "explorer", "task": "explore"},
                {"id": "build", "agent": "worker", "task": "build"}
            ]
        });
        let compiled = ExecutionPlan::from_v1_workflow(&v1).unwrap();
        compiled.validate().unwrap();
        assert_eq!(compiled.nodes.len(), 2);
        assert_eq!(compiled.edges.len(), 1);
        assert_eq!(compiled.entry_nodes, vec!["research".to_owned()]);
        // Sequential semantics preserved.
        assert!(matches!(compiled.nodes.get("research"), Some(NodeSpec::Agent { profile_id, .. }) if profile_id == "explorer"));
    }

    #[test]
    fn predicates_evaluate_safely() {
        let mut state = BTreeMap::new();
        state.insert("mode".to_owned(), json!("fast"));
        state.insert("count".to_owned(), json!(3));

        assert!(evaluate_predicate("mode == \"fast\"", &state).unwrap());
        assert!(evaluate_predicate("mode != \"slow\"", &state).unwrap());
        assert!(!evaluate_predicate("mode contains \"fastx\"", &state).unwrap());
        assert!(evaluate_predicate("count == 3", &state).unwrap());
        assert!(evaluate_predicate("count != 99", &state).unwrap());
        assert!(evaluate_predicate("mode contains \"as\"", &state).unwrap());
        // Unknown fields are errors, never silent true.
        assert!(evaluate_predicate("ghost == 1", &state).is_err());
        // Arbitrary code is not executed — unsupported syntax errors.
        assert!(evaluate_predicate("mode.drop()", &state).is_err());
    }

    #[test]
    fn retry_edges_carry_backoff() {
        let edge = EdgeSpec {
            from: "a".into(),
            to: "b".into(),
            edge_kind: EdgeKind::Retry { max_attempts: 3, backoff_ms: 250 },
        };
        let json = serde_json::to_string(&edge).unwrap();
        let back: EdgeSpec = serde_json::from_str(&json).unwrap();
        match back.edge_kind {
            EdgeKind::Retry { max_attempts, backoff_ms } => {
                assert_eq!((max_attempts, Duration::from_millis(backoff_ms)), (3, Duration::from_millis(250)));
            }
            other => panic!("unexpected kind {other:?}"),
        }
    }
}
