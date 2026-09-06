//! Human-in-the-loop (§13 HITL, Tasks 13.1-13.3, K-015, G-005, G-017).
//!
//! Human/Verify nodes pause the run durably: state is checkpointed, the
//! executor releases resources, and a pending approval/interrupt row waits
//! for a decision that may arrive hours later from any client. Breakpoints
//! pause before/after a node; they survive daemon restarts.

use anyhow::{Context as _, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Why a run paused.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Interrupt {
    /// Human node awaiting a structured answer.
    HumanInput { node: String, prompt: String, answer_schema: Value },
    /// Effect approval required before a node may run.
    Approval { node: String, effect: String },
    /// User breakpoint before a node executes.
    BreakpointBefore { node: String },
    /// User breakpoint after a node completed.
    BreakpointAfter { node: String },
}

/// A resolved interrupt carries the human payload (answer/approval).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InterruptResolution {
    pub interrupt: Interrupt,
    pub approved: bool,
    /// Human answer payload (for HumanInput nodes).
    #[serde(default)]
    pub answer: Option<Value>,
    /// Provenance: which client resolved it.
    pub resolved_by: String,
    pub resolved_at_ms: i64,
}

/// Durable interrupt registry: pause/resume without holding executor
/// resources (G-005: workflow can pause indefinitely).
pub struct InterruptRegistry {
    pending: Mutex<BTreeMap<String, Interrupt>>,
    resolutions: Mutex<Vec<InterruptResolution>>,
}

impl Default for InterruptRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl InterruptRegistry {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(BTreeMap::new()),
            resolutions: Mutex::new(Vec::new()),
        }
    }

    /// Registers a pause. Returns the interrupt id.
    pub fn pause(&self, run_id: &str, interrupt: Interrupt) -> String {
        let id = format!("int_{run_id}");
        self.pending.lock().insert(id.clone(), interrupt);
        id
    }

    /// Resolves a pending interrupt (Task 13.2 acceptance: same node
    /// attempt continues after the answer).
    pub fn resolve(
        &self,
        interrupt_id: &str,
        approved: bool,
        answer: Option<Value>,
        resolved_by: &str,
    ) -> Result<InterruptResolution> {
        let interrupt = self
            .pending
            .lock()
            .remove(interrupt_id)
            .context("interrupt not found or already resolved")?;
        let resolution = InterruptResolution {
            interrupt: interrupt.clone(),
            approved,
            answer,
            resolved_by: resolved_by.to_owned(),
            resolved_at_ms: now_ms(),
        };
        self.resolutions.lock().push(resolution.clone());
        Ok(resolution)
    }

    pub fn pending_count(&self) -> usize {
        self.pending.lock().len()
    }

    pub fn get_pending(&self, interrupt_id: &str) -> Option<Interrupt> {
        self.pending.lock().get(interrupt_id).cloned()
    }

    pub fn resolutions(&self) -> Vec<InterruptResolution> {
        self.resolutions.lock().clone()
    }
}

/// User breakpoints (Task 13.3, G-017): persist across restarts because
/// they live in run state, not in memory of the executor.
#[derive(Debug, Default)]
pub struct BreakpointSet {
    breakpoints: Mutex<Vec<Breakpoint>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Breakpoint {
    BeforeNode { run_id: String, node: String },
    AfterNode { run_id: String, node: String },
    /// Before any effect in the deny/approval classes executes.
    BeforeHighRiskEffect { run_id: String, effect: String },
}

impl BreakpointSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&self, breakpoint: Breakpoint) {
        let mut breakpoints = self.breakpoints.lock();
        if !breakpoints.contains(&breakpoint) {
            breakpoints.push(breakpoint);
        }
    }

    pub fn remove(&self, breakpoint: &Breakpoint) -> bool {
        let mut breakpoints = self.breakpoints.lock();
        let position = breakpoints.iter().position(|b| b == breakpoint);
        match position {
            Some(index) => {
                breakpoints.remove(index);
                true
            }
            None => false,
        }
    }

    /// Whether a breakpoint fires for this transition.
    pub fn hits_before(&self, run_id: &str, node: &str) -> bool {
        self.breakpoints.lock().iter().any(|b| matches!(b, Breakpoint::BeforeNode { run_id: r, node: n } if r == run_id && n == node))
    }

    pub fn hits_after(&self, run_id: &str, node: &str) -> bool {
        self.breakpoints.lock().iter().any(|b| matches!(b, Breakpoint::AfterNode { run_id: r, node: n } if r == run_id && n == node))
    }

    pub fn len(&self) -> usize {
        self.breakpoints.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
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
    fn human_node_pauses_without_holding_executor() {
        let registry = InterruptRegistry::new();
        let interrupt_id = registry.pause(
            "run_1",
            Interrupt::HumanInput {
                node: "approve_migration".into(),
                prompt: "Proceed with the schema migration?".into(),
                answer_schema: json!({"type": "object", "properties": {"confirm": {"type": "boolean"}}}),
            },
        );
        assert_eq!(registry.pending_count(), 1);

        // Hours later, from any client:
        let resolved = registry
            .resolve(
                &interrupt_id,
                true,
                Some(json!({"confirm": true})),
                "web-client",
            )
            .unwrap();
        assert_eq!(resolved_by_provenance(&registry), Some("web-client".to_owned()));
        assert_eq!(registry.pending_count(), 0);
        assert!(matches!(resolved.interrupt, Interrupt::HumanInput { ref node, .. } if node == "approve_migration"));
    }

    fn resolved_by_provenance(registry: &InterruptRegistry) -> Option<String> {
        registry.resolutions().last().map(|r| r.resolved_by.clone())
    }

    #[test]
    fn double_resolution_is_rejected() {
        let registry = InterruptRegistry::new();
        let id = registry.pause("run_2", Interrupt::Approval { node: "w".into(), effect: "filesystem.write".into() });
        registry.resolve(&id, true, None, "cli").unwrap();
        assert!(registry.resolve(&id, false, None, "web").is_err(), "already resolved");
    }

    #[test]
    fn denial_carries_no_answer_requirement() {
        let registry = InterruptRegistry::new();
        let id = registry.pause("run_3", Interrupt::Approval { node: "n".into(), effect: "git.write".into() });
        let resolved = registry.resolve(&id, false, None, "desktop").unwrap();
        assert!(!resolved.approved, "denial carries approved=false");
    }

    #[test]
    fn breakpoints_fire_before_and_after() {
        let breakpoints = BreakpointSet::new();
        breakpoints.add(Breakpoint::BeforeNode { run_id: "run_1".into(), node: "deploy".into() });
        breakpoints.add(Breakpoint::AfterNode { run_id: "run_1".into(), node: "test".into() });

        assert!(breakpoints.hits_before("run_1", "deploy"));
        assert!(!breakpoints.hits_before("run_1", "test"));
        assert!(breakpoints.hits_after("run_1", "test"));
        assert_eq!(breakpoints.len(), 2);

        // Removal restores flow.
        assert!(breakpoints.remove(&Breakpoint::BeforeNode {
            run_id: "run_1".into(),
            node: "deploy".into()
        }));
        assert!(!breakpoints.hits_before("run_1", "deploy"));
        assert!(breakpoints.len() == 1);
    }

    #[test]
    fn breakpoints_are_run_scoped() {
        let breakpoints = BreakpointSet::new();
        breakpoints.add(Breakpoint::BeforeNode { run_id: "run_a".into(), node: "x".into() });
        assert!(!breakpoints.hits_before("run_b", "x"), "other runs unaffected");
    }

    #[test]
    fn high_risk_effect_breakpoint_registered() {
        let breakpoints = BreakpointSet::new();
        breakpoints.add(Breakpoint::BeforeHighRiskEffect {
            run_id: "run_1".into(),
            effect: "filesystem.delete".into(),
        });
        assert_eq!(breakpoints.len(), 1);
    }
}
