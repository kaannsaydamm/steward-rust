use crate::state::{self, WorkflowState};
use crate::workflow_events::{cancelled_event, workflow_event};
use crate::workflow_logs;
use crate::workflow_store;
use anyhow::{anyhow, Result};
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use steward_core::pb::{AgentLogEntry, StartWorkflowRequest, WorkflowEvent};
use tokio::sync::mpsc;
use tonic::Status;

type EventSender = mpsc::Sender<std::result::Result<WorkflowEvent, Status>>;

#[derive(Clone)]
pub struct WorkflowRuntime {
    db: Arc<Mutex<Connection>>,
    workflows: Arc<tokio::sync::Mutex<HashMap<String, WorkflowState>>>,
    agent_logs: Arc<tokio::sync::Mutex<HashMap<String, Vec<AgentLogEntry>>>>,
}

impl WorkflowRuntime {
    pub fn new(
        db: Arc<Mutex<Connection>>,
        workflows: Arc<tokio::sync::Mutex<HashMap<String, WorkflowState>>>,
        agent_logs: Arc<tokio::sync::Mutex<HashMap<String, Vec<AgentLogEntry>>>>,
    ) -> Self {
        Self {
            db,
            workflows,
            agent_logs,
        }
    }

    pub async fn start(
        &self,
        req: StartWorkflowRequest,
        tx: EventSender,
    ) -> std::result::Result<(), Status> {
        let workflow_id = uuid::Uuid::new_v4().to_string();
        let title = req.title;
        let description = req.description;
        let initial_state = state::new_workflow_state(&workflow_id, &title);

        self.workflows
            .lock()
            .await
            .insert(workflow_id.clone(), initial_state.clone());
        self.persist_workflow_state(&initial_state)
            .map_err(|error| Status::internal(error.to_string()))?;
        self.spawn_runner(workflow_id, title, description, Some(tx), 0);
        Ok(())
    }

    pub fn resume_persisted(&self) {
        let runtime = self.clone();
        tokio::spawn(async move {
            let snapshots = {
                let workflows = runtime.workflows.lock().await;
                workflows
                    .values()
                    .filter(|state| !state.cancelled && state.status.phase < 10)
                    .cloned()
                    .collect::<Vec<_>>()
            };

            for snapshot in snapshots {
                let phase = snapshot.status.phase;
                runtime.spawn_runner(
                    snapshot.status.workflow_id.clone(),
                    snapshot.status.title.clone(),
                    snapshot.status.title.clone(),
                    None,
                    phase,
                );
            }
        });
    }

    pub fn persist_workflow_state(&self, state: &WorkflowState) -> Result<()> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow!("Database lock failed"))?;
        workflow_store::upsert_workflow(&db, state)
    }

    fn spawn_runner(
        &self,
        workflow_id: String,
        title: String,
        description: String,
        tx: Option<EventSender>,
        current_phase: i32,
    ) {
        let runtime = self.clone();
        tokio::spawn(async move {
            runtime
                .run_workflow(workflow_id, title, description, tx, current_phase)
                .await;
        });
    }

    async fn run_workflow(
        &self,
        workflow_id: String,
        title: String,
        description: String,
        tx: Option<EventSender>,
        current_phase: i32,
    ) {
        if current_phase == 7 {
            if !self.wait_for_approval(&workflow_id).await {
                return;
            }
            self.copy_approved_mode(&workflow_id).await;
        }

        let total = state::WORKFLOW_PHASES.len() as f32;
        for (index, (phase, phase_name, agent_id)) in state::WORKFLOW_PHASES.iter().enumerate() {
            if *phase <= current_phase {
                continue;
            }
            if self.is_cancelled(&workflow_id).await {
                self.emit_cancelled(&workflow_id, tx.as_ref()).await;
                break;
            }

            let event = workflow_event(
                &workflow_id,
                &title,
                &description,
                *phase,
                phase_name,
                agent_id,
                ((index + 1) as f32 / total) * 100.0,
            );

            if let Some(sender) = tx.as_ref() {
                let _ = sender.send(Ok(event.clone())).await;
            }

            self.apply_event(&workflow_id, &event).await;
            self.persist_workflow_event(&event);
            workflow_logs::log_agent_phase(
                &self.agent_logs,
                &workflow_id,
                &title,
                phase_name,
                agent_id,
            )
            .await;

            if *phase == 7 {
                if !self.wait_for_approval(&workflow_id).await {
                    break;
                }
                self.copy_approved_mode(&workflow_id).await;
            } else {
                tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            }
        }
    }

    async fn apply_event(&self, workflow_id: &str, event: &WorkflowEvent) {
        let state_snapshot = {
            let mut workflows = self.workflows.lock().await;
            if let Some(state) = workflows.get_mut(workflow_id) {
                state.status.phase = event.phase;
                state.status.overall_progress = event.progress;
                state.status.current_agent = event.agent_id.clone();
                state.status.status_message = event.message.clone();
                state::push_recent_event(state, event.clone());
                if event.phase == 7 {
                    state.status.requires_approval = true;
                    state.status.pending_approval = event.approval.clone();
                }
                Some(state.clone())
            } else {
                None
            }
        };
        if let Some(state) = state_snapshot {
            self.persist_workflow_state_background(&state);
        }
    }

    async fn wait_for_approval(&self, workflow_id: &str) -> bool {
        for _ in 0..600 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let decision = {
                let workflows = self.workflows.lock().await;
                workflows
                    .get(workflow_id)
                    .map(|state| (state.approved, state.cancelled))
                    .unwrap_or((false, true))
            };
            if decision.1 {
                return false;
            }
            if decision.0 {
                return true;
            }
        }
        false
    }

    async fn copy_approved_mode(&self, workflow_id: &str) {
        let state_snapshot = {
            let mut workflows = self.workflows.lock().await;
            if let Some(state) = workflows.get_mut(workflow_id) {
                state.status.mode = state.mode;
                Some(state.clone())
            } else {
                None
            }
        };
        if let Some(state) = state_snapshot {
            self.persist_workflow_state_background(&state);
        }
    }

    async fn is_cancelled(&self, workflow_id: &str) -> bool {
        let workflows = self.workflows.lock().await;
        workflows
            .get(workflow_id)
            .map(|state| state.cancelled)
            .unwrap_or(true)
    }

    async fn emit_cancelled(&self, workflow_id: &str, tx: Option<&EventSender>) {
        let event = cancelled_event(workflow_id);
        self.persist_workflow_event(&event);
        if let Some(sender) = tx {
            let _ = sender.send(Ok(event)).await;
        }
    }

    fn persist_workflow_state_background(&self, state: &WorkflowState) {
        if let Err(error) = self.persist_workflow_state(state) {
            log::error!("failed to persist workflow state: {error}");
        }
    }

    fn persist_workflow_event(&self, event: &WorkflowEvent) {
        let result = self
            .db
            .lock()
            .map_err(|_| anyhow!("Database lock failed"))
            .and_then(|db| workflow_store::insert_event(&db, event));
        if let Err(error) = result {
            log::error!("failed to persist workflow event: {error}");
        }
    }
}
