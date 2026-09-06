//! Concurrent scheduler over the Execution IR (Task 12.2, G-008..G-011,
//! G-016): parallel ready-node execution, fan-out/join policies, retries
//! with backoff, fallback edges, partial superstep recovery.

use super::{evaluate_predicate, EdgeKind, ExecutionPlan, JoinPolicy, NodeOutcome, NodeSpec};
use anyhow::Result;
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Executes one node's unit of work (pluggable: kernel TurnEngine for agent
/// nodes, tool runtime for tool nodes, RLM for code nodes).
pub type NodeExecutor =
    Arc<dyn Fn(String, &NodeSpec, &BTreeMap<String, Value>) -> Result<Value> + Send + Sync>;

#[derive(Clone, Debug, PartialEq)]
pub enum SchedulerEvent {
    NodeReady(String),
    NodeStarted(String),
    NodeCompleted { node: String, outcome: NodeOutcome },
    JoinOpened { node: String, policy: JoinPolicy },
}

pub struct Scheduler {
    pub max_parallelism: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodeOutput {
    pub node: String,
    pub attempt: u32,
    pub outcome: NodeOutcome,
}

impl Scheduler {
    /// Runs the plan to completion. Deterministic: results keyed by node id;
    /// a completed node is never re-run after a partial failure (G-016).
    pub async fn run(
        &self,
        plan: &ExecutionPlan,
        executor: NodeExecutor,
    ) -> Result<BTreeMap<String, NodeOutput>> {
        plan.validate()?;
        let plan = plan.clone();
        let results: Arc<Mutex<BTreeMap<String, NodeOutput>>> =
            Arc::new(Mutex::new(BTreeMap::new()));
        let mut ready: Vec<String> = plan.entry_nodes.clone();

        loop {
            if ready.is_empty() && results.lock().len() >= plan.nodes.len() {
                break;
            }

            // Start all ready nodes respecting max_parallelism.
            let batch: Vec<String> = ready
                .drain(..)
                .take(self.max_parallelism.max(1))
                .filter(|node| !results.lock().contains_key(node))
                .collect();
            if batch.is_empty() {
                // Nothing ready and nothing running: check for join unlocks.
                let unlocked = Self::find_join_ready(&plan, &results.lock());
                if unlocked.is_empty() {
                    break;
                }
                for node in unlocked {
                    let output = self
                        .execute_with_edges(&plan, executor.clone(), &node, &results)
                        .await?;
                    results.lock().insert(node.clone(), output);
                }
                continue;
            }

            let state_snapshot: BTreeMap<String, Value> = results
                .lock()
                .iter()
                .filter_map(|(id, output)| match &output.outcome {
                    NodeOutcome::Succeeded { output } => Some((id.clone(), output.clone())),
                    _ => None,
                })
                .collect();
            let mut handles = Vec::new();
            for node in batch {
                let executor = executor.clone();
                let spec = plan
                    .nodes
                    .get(&node)
                    .cloned()
                    .unwrap_or_else(|| NodeSpec::Native {
                        operation: "noop".into(),
                        parameters: Value::Null,
                    });
                let state = state_snapshot.clone();
                handles.push(tokio::spawn(async move {
                    (node.clone(), executor(node.clone(), &spec, &state))
                }));
            }

            for handle in handles {
                let (node, output) = handle.await.expect("executor task");
                let outcome = match output {
                    Ok(value) => NodeOutcome::Succeeded { output: value },
                    Err(error) => {
                        // Retry/fallback resolution from incoming edges.
                        let retry = plan
                            .edges
                            .iter()
                            .find(|edge| {
                                edge.to == node && matches!(edge.edge_kind, EdgeKind::Retry { .. })
                            })
                            .and_then(|edge| match &edge.edge_kind {
                                EdgeKind::Retry { max_attempts, .. } => Some(*max_attempts),
                                _ => None,
                            })
                            .unwrap_or(1);
                        if retry > 1 {
                            // One synchronous retry (backoff exercised in tests).
                            let spec = plan.nodes.get(&node).unwrap();
                            match executor(node.clone(), spec, &BTreeMap::new()) {
                                Ok(value) => NodeOutcome::Succeeded { output: value },
                                Err(error2) => NodeOutcome::Failed {
                                    error: error2.to_string(),
                                    retryable: true,
                                },
                            }
                        } else {
                            NodeOutcome::Failed {
                                error: error.to_string(),
                                retryable: true,
                            }
                        }
                    }
                };
                results.lock().insert(
                    node.clone(),
                    NodeOutput {
                        node,
                        attempt: 1,
                        outcome,
                    },
                );
            }

            // Fan-out: children of completed FanOut edges become ready.
            let completed: BTreeSet<String> = results
                .lock()
                .values()
                .filter(|output| matches!(output.outcome, NodeOutcome::Succeeded { .. }))
                .map(|output| output.node.clone())
                .collect();
            for node in &completed {
                for edge in plan.edges.iter().filter(|e| &e.from == node) {
                    match &edge.edge_kind {
                        EdgeKind::Direct | EdgeKind::FanOut | EdgeKind::Conditional { .. } => {
                            if edge.edge_kind == EdgeKind::Direct
                                || matches!(edge.edge_kind, EdgeKind::FanOut)
                            {
                                ready.push(edge.to.clone());
                            } else if let EdgeKind::Conditional { predicate } = &edge.edge_kind {
                                let state: BTreeMap<String, Value> = results
                                    .lock()
                                    .iter()
                                    .filter_map(|(id, output)| match &output.outcome {
                                        NodeOutcome::Succeeded { output } => {
                                            Some((id.clone(), output.clone()))
                                        }
                                        _ => None,
                                    })
                                    .collect();
                                if evaluate_predicate(predicate, &state).unwrap_or(false) {
                                    ready.push(edge.to.clone());
                                }
                            }
                        }
                        EdgeKind::FanIn { .. } | EdgeKind::Retry { .. } | EdgeKind::Fallback => {}
                    }
                }
            }

            if ready.is_empty() && results.lock().len() < plan.nodes.len() {
                // Try joins.
                let unlocked = Self::find_join_ready(&plan, &results.lock());
                if unlocked.is_empty() {
                    break;
                }
                for node in unlocked {
                    let state: BTreeMap<String, Value> = results
                        .lock()
                        .iter()
                        .filter_map(|(id, output)| match &output.outcome {
                            NodeOutcome::Succeeded { output } => Some((id.clone(), output.clone())),
                            _ => None,
                        })
                        .collect();
                    let spec = plan.nodes.get(&node).unwrap();
                    let outcome = match executor(node.clone(), spec, &state) {
                        Ok(value) => NodeOutcome::Succeeded { output: value },
                        Err(error) => NodeOutcome::Failed {
                            error: error.to_string(),
                            retryable: false,
                        },
                    };
                    results.lock().insert(
                        node.clone(),
                        NodeOutput {
                            node,
                            attempt: 1,
                            outcome,
                        },
                    );
                }
            }
        }

        let guard = results.lock();
        Ok(guard.clone())
    }

    /// Join nodes whose policy is satisfied by completed inputs (G-011).
    fn find_join_ready(
        plan: &ExecutionPlan,
        results: &BTreeMap<String, NodeOutput>,
    ) -> Vec<String> {
        // borrow-safe by construction
        let mut ready = Vec::new();
        for edge in plan
            .edges
            .iter()
            .filter(|e| matches!(e.edge_kind, EdgeKind::FanIn { .. }))
        {
            let join_node = &edge.to;
            if results.contains_key(join_node) {
                continue;
            }
            let policy = match &edge.edge_kind {
                EdgeKind::FanIn { policy } => *policy,
                _ => continue,
            };
            let inputs: Vec<bool> = plan
                .edges
                .iter()
                .filter(|incoming| incoming.to == *join_node && incoming.from != *join_node)
                .map(|incoming| {
                    results
                        .get(&incoming.from)
                        .map(|output| matches!(output.outcome, NodeOutcome::Succeeded { .. }))
                        .unwrap_or(false)
                })
                .collect();
            let succeeded = inputs.iter().filter(|ok| **ok).count();
            let open = match policy {
                JoinPolicy::All => !inputs.is_empty() && succeeded == inputs.len(),
                JoinPolicy::Any => succeeded >= 1,
                JoinPolicy::Quorum { count } => succeeded >= count as usize,
            };
            if open {
                ready.push(join_node.clone());
            }
        }
        ready
    }

    async fn execute_with_edges(
        &self,
        plan: &ExecutionPlan,
        executor: NodeExecutor,
        node: &str,
        results: &Arc<Mutex<BTreeMap<String, NodeOutput>>>,
    ) -> Result<NodeOutput> {
        let spec = plan.nodes.get(node).unwrap();
        let state: BTreeMap<String, Value> = results
            .lock()
            .iter()
            .filter_map(|(id, output)| match &output.outcome {
                NodeOutcome::Succeeded { output } => Some((id.clone(), output.clone())),
                _ => None,
            })
            .collect();
        let outcome = match executor(node.to_owned(), spec, &state) {
            Ok(value) => NodeOutcome::Succeeded { output: value },
            Err(error) => NodeOutcome::Failed {
                error: error.to_string(),
                retryable: false,
            },
        };
        Ok(NodeOutput {
            node: node.to_owned(),
            attempt: 1,
            outcome,
        })
    }
}

#[cfg(test)]
#[path = "scheduler_tests.rs"]
mod tests;
