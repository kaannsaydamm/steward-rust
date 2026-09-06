//! Async task registry (Task 11.3, K-007/K-008): parent continues while the
//! child runs; status/result/cancel APIs survive daemon restarts when the
//! executor is resumable (durable rows land in `agent_instances`).

use crate::agents::scheduler::{AgentStatus, SpawnMode, SpawnRequest};
use anyhow::Result;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AsyncTask {
    pub task_id: String,
    pub request: SpawnRequest,
    pub status: AgentStatus,
    /// Final result text when Succeeded.
    #[serde(default)]
    pub result: Option<String>,
    /// Failure reason when Failed/Cancelled.
    #[serde(default)]
    pub error: Option<String>,
    pub created_at_ms: i64,
    pub completed_at_ms: Option<i64>,
}

pub struct AsyncTaskRegistry {
    tasks: Mutex<BTreeMap<String, AsyncTask>>,
    next_id: Mutex<u64>,
}

impl Default for AsyncTaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncTaskRegistry {
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(BTreeMap::new()),
            next_id: Mutex::new(0),
        }
    }

    pub fn register(&self, request: SpawnRequest) -> AsyncTask {
        let mut next = self.next_id.lock();
        *next += 1;
        let task = AsyncTask {
            task_id: format!("task_{:04}", *next),
            request,
            status: AgentStatus::Queued,
            result: None,
            error: None,
            created_at_ms: now_ms(),
            completed_at_ms: None,
        };
        self.tasks.lock().insert(task.task_id.clone(), task.clone());
        task
    }

    pub fn mark_running(&self, task_id: &str) -> Result<()> {
        let mut tasks = self.tasks.lock();
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("task not found"))?;
        task.status = AgentStatus::Running;
        Ok(())
    }

    pub fn complete(&self, task_id: &str, result: &str) -> Result<()> {
        let mut tasks = self.tasks.lock();
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("task not found"))?;
        if matches!(
            task.status,
            AgentStatus::Succeeded | AgentStatus::Failed | AgentStatus::Cancelled
        ) {
            anyhow::bail!("task already terminal");
        }
        task.status = AgentStatus::Succeeded;
        task.result = Some(result.to_owned());
        task.completed_at_ms = Some(now_ms());
        Ok(())
    }

    pub fn fail(&self, task_id: &str, error: &str) -> Result<()> {
        let mut tasks = self.tasks.lock();
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("task not found"))?;
        task.status = AgentStatus::Failed;
        task.error = Some(error.to_owned());
        task.completed_at_ms = Some(now_ms());
        Ok(())
    }

    pub fn cancel(&self, task_id: &str) -> Result<bool> {
        let mut tasks = self.tasks.lock();
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("task not found"))?;
        if matches!(
            task.status,
            AgentStatus::Succeeded | AgentStatus::Failed | AgentStatus::Cancelled
        ) {
            return Ok(false);
        }
        task.status = AgentStatus::Cancelled;
        task.completed_at_ms = Some(now_ms());
        Ok(true)
    }

    pub fn get(&self, task_id: &str) -> Option<AsyncTask> {
        self.tasks.lock().get(task_id).cloned()
    }

    /// Restart recovery: tasks still marked Running after a daemon restart
    /// are marked Failed("interrupted") — never silently resumed as running
    /// (§12.11 honest-state semantics).
    pub fn reconcile_after_restart(&self) -> usize {
        let mut tasks = self.tasks.lock();
        let mut reconciled = 0;
        for task in tasks.values_mut() {
            if task.status == AgentStatus::Running {
                task.status = AgentStatus::Failed;
                task.error = Some("interrupted by daemon restart".into());
                task.completed_at_ms = Some(now_ms());
                reconciled += 1;
            }
        }
        reconciled
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
    use crate::agents::scheduler::SpawnMode;

    fn spawn_request() -> SpawnRequest {
        SpawnRequest {
            parent_instance_id: None,
            profile_id: "explorer".into(),
            task: "background scan".into(),
            mode: SpawnMode::Async,
            budget: Default::default(),
        }
    }

    #[tokio::test]
    async fn parent_continues_child_runs_to_completion() {
        let registry = AsyncTaskRegistry::new();
        let task = registry.register(spawn_request());
        registry.mark_running(&task.task_id).unwrap();
        // Simulate work.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        registry.complete(&task.task_id, "scan done").unwrap();

        let finished = registry.get(&task.task_id).unwrap();
        assert_eq!(finished.status, AgentStatus::Succeeded);
        assert_eq!(finished.result.as_deref(), Some("scan done"));
        assert!(finished.completed_at_ms.is_some());
    }

    #[tokio::test]
    async fn cancel_prevents_completion() {
        let registry = AsyncTaskRegistry::new();
        let task = registry.register(spawn_request());
        registry.mark_running(&task.task_id).unwrap();
        assert!(registry.cancel(&task.task_id).unwrap());

        // Completing after cancel is rejected.
        assert!(registry.complete(&task.task_id, "late").is_err());
        let cancelled = registry.get(&task.task_id).unwrap();
        assert_eq!(cancelled.status, AgentStatus::Cancelled);
    }

    #[tokio::test]
    async fn restart_marks_running_tasks_interrupted() {
        let registry = AsyncTaskRegistry::new();
        let task_a = registry.register(spawn_request());
        let task_b = registry.register(spawn_request());
        registry.mark_running(&task_a.task_id).unwrap();
        registry.complete(&task_b.task_id, "done").unwrap();

        let reconciled = registry.reconcile_after_restart();
        assert_eq!(reconciled, 1);

        let a = registry.get(&task_a.task_id).unwrap();
        assert_eq!(a.status, AgentStatus::Failed);
        assert_eq!(a.error.as_deref(), Some("interrupted by daemon restart"));
        // Completed tasks are untouched.
        assert_eq!(
            registry.get(&task_b.task_id).unwrap().status,
            AgentStatus::Succeeded
        );
    }
}
