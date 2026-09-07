//! Ported from PrimeIntellect-ai/prime-agent@844e85545af6858dcb3d6cfe42bbfcf2ca0be4e5
//! packages/coding-agent/src/core/rlm-runtime.ts (MIT). Modified for Steward:
//! RLM runs execute as kernel `RlmRun` actions routed to the CodeRuntime
//! sidecar (persistent REPL keyed by session); child agents spawn through the
//! kernel's `RlmChildSpawner` port so the daemon can wire the harness
//! scheduler without a kernel→harness dependency cycle.

use crate::budget::{Budget, BudgetCheck};
use crate::code_runtime::{CodeRuntime, SidecarRequest};
use anyhow::{bail, Context as _, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// One RLM invocation: the model emits `RlmRun { code, session }` like a tool
/// and the executor feeds `cell_source` to the session's persistent REPL.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RlmRunRequest {
    /// Prompt/task the RLM run is solving (context for observations).
    pub prompt: String,
    /// Free-form kwargs carried with the run (prime `kwargs`).
    #[serde(default)]
    pub kwargs: Value,
    /// The code cell to execute in the persistent REPL.
    pub cell_source: String,
}

/// Lifecycle state of a child agent spawned from inside the REPL.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChildState {
    Running,
    Completed,
    Error,
}

/// A registered child agent of an RLM run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChildAgent {
    pub id: String,
    pub profile_id: String,
    pub task: String,
    pub state: ChildState,
    /// Result payload after the child settles (prime: results as values).
    #[serde(default)]
    pub result: Option<Value>,
}

/// Port for spawning children. Implemented in the daemon against the harness
/// scheduler (`SpawnMode::Async`); the kernel runtime only sees this trait.
pub trait RlmChildSpawner: Send + Sync {
    fn spawn(&self, run_id: &str, profile_id: &str, task: &str) -> Result<String>;
    /// Polls a child; settles registry state. Returns (state, result).
    fn poll(&self, child_id: &str) -> Result<(ChildState, Option<Value>)>;
}

/// Termination knobs for one RLM run (prime: wall-clock + max failures).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RlmLimits {
    pub max_wall_clock_ms: u64,
    pub max_failures: u32,
}

impl Default for RlmLimits {
    fn default() -> Self {
        Self {
            max_wall_clock_ms: 120_000,
            max_failures: 3,
        }
    }
}

/// Counters enforcing per-run budgets inside the RLM loop.
#[derive(Debug, Default)]
struct RunCounters {
    cells: AtomicU32,
    failures: AtomicU32,
    spawned_agents: AtomicU32,
    wall_clock_ms: AtomicU64,
}

/// One RLM run: registry of children + counters + persistent REPL binding.
pub struct RlmRun {
    pub run_id: String,
    pub session_id: String,
    pub request: RlmRunRequest,
    pub limits: RlmLimits,
    budget: Budget,
    counters: RunCounters,
    started_at: Instant,
    children: Mutex<BTreeMap<String, ChildAgent>>,
    spawner: Option<Arc<dyn RlmChildSpawner>>,
    runtime: Arc<CodeRuntime>,
}

impl RlmRun {
    /// Starts a run: registers the REPL session (idempotent — reuses an
    /// existing session so REPL state persists across cells).
    pub fn start(
        run_id: String,
        session_id: String,
        request: RlmRunRequest,
        budget: Budget,
        limits: RlmLimits,
        runtime: Arc<CodeRuntime>,
    ) -> Self {
        if let Ok(crate::code_runtime::SidecarResponse::Error { message }) =
            runtime.handle(SidecarRequest::CreateSession {
                session_id: session_id.clone(),
                language: "python".to_owned(),
                working_dir: String::new(),
            })
        {
            eprintln!("rlm: session create note: {message}");
        }
        Self {
            run_id,
            session_id,
            request,
            limits,
            budget,
            counters: RunCounters::default(),
            started_at: Instant::now(),
            children: Mutex::new(BTreeMap::new()),
            spawner: None,
            runtime,
        }
    }

    pub fn set_spawner(&mut self, spawner: Arc<dyn RlmChildSpawner>) {
        self.spawner = Some(spawner);
    }

    /// Whether the run must stop: budget exhaustion, wall-clock, or failures.
    pub fn should_terminate(&self) -> Option<&'static str> {
        let elapsed = self.started_at.elapsed().as_millis() as u64;
        self.counters
            .wall_clock_ms
            .store(elapsed, Ordering::Relaxed);
        if elapsed >= self.limits.max_wall_clock_ms {
            return Some("wall_clock");
        }
        if self.counters.failures.load(Ordering::Relaxed) >= self.limits.max_failures {
            return Some("max_failures");
        }
        match self.budget.check(
            0,
            self.counters.cells.load(Ordering::Relaxed),
            self.counters.spawned_agents.load(Ordering::Relaxed),
            self.counters.failures.load(Ordering::Relaxed),
            0,
            0,
            0,
            elapsed,
        ) {
            BudgetCheck::Ok => None,
            BudgetCheck::Exhausted(dimension) => Some(dimension),
        }
    }

    /// Executes one cell in the persistent REPL (`rlm.run` observation).
    pub fn execute_cell(&self) -> Result<(String, Value)> {
        if let Some(reason) = self.should_terminate() {
            bail!("rlm run terminated: {reason}");
        }
        let cell_id = format!(
            "{}-{}",
            self.run_id,
            self.counters.cells.load(Ordering::Relaxed)
        );
        let response = self.runtime.handle(SidecarRequest::ExecuteCell {
            session_id: self.session_id.clone(),
            cell_id: cell_id.clone(),
            source: self.request.cell_source.clone(),
        });
        self.counters.cells.fetch_add(1, Ordering::Relaxed);
        match response {
            Ok(crate::code_runtime::SidecarResponse::CellCompleted {
                output, variables, ..
            }) => Ok((output, variables)),
            Ok(crate::code_runtime::SidecarResponse::Error { message }) => {
                self.counters.failures.fetch_add(1, Ordering::Relaxed);
                bail!("rlm cell failed: {message}");
            }
            Err(error) => {
                // Transport/handler-level failures count against max_failures.
                self.counters.failures.fetch_add(1, Ordering::Relaxed);
                Err(error).context("rlm cell execution")
            }
            other => bail!("unexpected sidecar response: {other:?}"),
        }
    }

    /// Spawns a child agent (the `rlm()` callable inside the REPL).
    pub fn spawn_child(&mut self, profile_id: &str, task: &str) -> Result<String> {
        if let Some(reason) = self.should_terminate() {
            bail!("rlm run terminated: {reason}");
        }
        let child_id = match &self.spawner {
            Some(spawner) => spawner.spawn(&self.run_id, profile_id, task)?,
            None => {
                // Without a spawner the child completes inline as a value.
                format!(
                    "{}-inline-{}",
                    self.run_id,
                    self.counters.spawned_agents.load(Ordering::Relaxed)
                )
            }
        };
        self.counters.spawned_agents.fetch_add(1, Ordering::Relaxed);
        self.children.lock().insert(
            child_id.clone(),
            ChildAgent {
                id: child_id.clone(),
                profile_id: profile_id.to_owned(),
                task: task.to_owned(),
                state: ChildState::Running,
                result: None,
            },
        );
        Ok(child_id)
    }

    /// Polls every running child, settling registry state from the spawner.
    pub fn poll_children(&mut self) {
        let ids: Vec<String> = {
            let children = self.children.lock();
            children
                .values()
                .filter(|child| child.state == ChildState::Running)
                .map(|child| child.id.clone())
                .collect()
        };
        for id in ids {
            let Some(spawner) = &self.spawner else {
                continue;
            };
            if let Ok((state, result)) = spawner.poll(&id) {
                if let Some(child) = self.children.lock().get_mut(&id) {
                    child.state = state;
                    child.result = result;
                    if state == ChildState::Error {
                        self.counters.failures.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
    }

    pub fn children(&self) -> Vec<ChildAgent> {
        self.children.lock().values().cloned().collect()
    }

    /// Destroys the persistent REPL session (release resources on termination).
    pub fn shutdown(&self) -> Result<()> {
        self.runtime
            .handle(SidecarRequest::Destroy {
                session_id: self.session_id.clone(),
            })
            .map(|_| ())
    }
}

/// Serializes the `rlm.run` observation event payload for the trace.
pub fn observation_event(run: &RlmRun, output: &str, variables: &Value) -> Value {
    json!({
        "kind": "rlm.run",
        "run_id": run.run_id,
        "session_id": run.session_id,
        "prompt": run.request.prompt,
        "output": output,
        "variables": variables,
        "children": run.children(),
        "elapsed_ms": run.counters.wall_clock_ms.load(Ordering::Relaxed),
    })
}

#[cfg(test)]
mod rlm_runtime_tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::AtomicUsize;

    /// In-memory spawner: children settle after N polls.
    struct ScriptedSpawner {
        settle_after: AtomicUsize,
        polls: AtomicUsize,
        next_id: AtomicU32,
    }

    impl ScriptedSpawner {
        fn new(settle_after: usize) -> Self {
            Self {
                settle_after: AtomicUsize::new(settle_after),
                polls: AtomicUsize::new(0),
                next_id: AtomicU32::new(0),
            }
        }
    }

    impl RlmChildSpawner for ScriptedSpawner {
        fn spawn(&self, _run_id: &str, profile_id: &str, _task: &str) -> Result<String> {
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            Ok(format!("child-{profile_id}-{id}"))
        }
        fn poll(&self, _child_id: &str) -> Result<(ChildState, Option<Value>)> {
            let polls = self.polls.fetch_add(1, Ordering::Relaxed);
            if polls >= self.settle_after.load(Ordering::Relaxed) {
                Ok((ChildState::Completed, Some(json!({"answer": 42}))))
            } else {
                Ok((ChildState::Running, None))
            }
        }
    }

    fn make_run(budget: Budget, limits: RlmLimits) -> RlmRun {
        RlmRun::start(
            "run-1".to_owned(),
            "session-rlm-1".to_owned(),
            RlmRunRequest {
                prompt: "compute the answer".to_owned(),
                kwargs: json!({}),
                cell_source: "print(6*7)".to_owned(),
            },
            budget,
            limits,
            Arc::new(CodeRuntime::new()),
        )
    }

    #[test]
    fn cell_execution_persists_across_cells_and_counts() {
        let run = make_run(Budget::default(), RlmLimits::default());
        let (output, _) = run.execute_cell().expect("first cell");
        assert!(!output.is_empty());
        let _ = run.execute_cell().expect("second cell");
        assert_eq!(run.counters.cells.load(Ordering::Relaxed), 2);
        // Persistent REPL: destroying then reusing the session is a fresh run
        // contract — the same session must still be alive here.
        assert!(run.shutdown().is_ok());
    }

    #[test]
    fn wall_clock_limit_terminates_run() {
        let limits = RlmLimits {
            max_wall_clock_ms: 0, // immediately exhausted
            max_failures: 3,
        };
        let run = make_run(Budget::default(), limits);
        let reason = run.should_terminate().expect("must terminate");
        assert_eq!(reason, "wall_clock");
        assert!(run.execute_cell().is_err());
    }

    #[test]
    fn max_failures_terminates_run() {
        let limits = RlmLimits {
            max_wall_clock_ms: 1_000_000,
            max_failures: 1,
        };
        let mut run = make_run(Budget::default(), limits);
        // Force one failure: an unknown session id errors in the sidecar.
        run.session_id = "missing-session".to_owned();
        assert!(run.execute_cell().is_err(), "erroring cell must fail");
        let reason = run.should_terminate().expect("failures must exhaust");
        assert_eq!(reason, "max_failures");
    }

    #[test]
    fn budget_exhaustion_terminates_run() {
        let budget = Budget {
            max_tool_calls: Some(1),
            ..Budget::default()
        };
        let run = make_run(budget, RlmLimits::default());
        let _ = run.execute_cell().expect("first cell ok");
        let reason = run.should_terminate().expect("tool budget exhausted");
        assert_eq!(reason, "max_tool_calls");
    }

    #[test]
    fn children_register_and_settle_through_spawner() {
        let mut run = make_run(Budget::default(), RlmLimits::default());
        run.set_spawner(Arc::new(ScriptedSpawner::new(1)));
        let child_id = run
            .spawn_child("worker", "summarize the page")
            .expect("spawn");
        run.poll_children();
        let children = run.children();
        assert_eq!(children.len(), 1);
        let child = &children[0];
        assert_eq!(child.id, child_id);
        // After the settle threshold, the child completes with its result.
        run.poll_children();
        let child = &run.children()[0];
        assert_eq!(child.state, ChildState::Completed);
        assert_eq!(child.result, Some(json!({"answer": 42})));
    }

    #[test]
    fn observation_event_carries_run_identity() {
        let run = make_run(Budget::default(), RlmLimits::default());
        let event = observation_event(&run, "42", &json!({"x": 1}));
        assert_eq!(event["kind"], "rlm.run");
        assert_eq!(event["run_id"], "run-1");
        assert_eq!(event["session_id"], "session-rlm-1");
        assert_eq!(event["output"], "42");
    }
}
