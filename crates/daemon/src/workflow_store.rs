use crate::state::WorkflowState;
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use steward_core::pb::{WorkflowEvent, WorkflowStatus};

pub fn create_schema(db: &Connection) -> Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS workflow_runs (
            workflow_id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            phase INTEGER NOT NULL,
            mode INTEGER NOT NULL,
            overall_progress REAL NOT NULL,
            current_agent TEXT NOT NULL DEFAULT '',
            status_message TEXT NOT NULL,
            requires_approval INTEGER NOT NULL DEFAULT 0,
            approved INTEGER NOT NULL DEFAULT 0,
            cancelled INTEGER NOT NULL DEFAULT 0,
            created_at REAL NOT NULL,
            updated_at REAL NOT NULL
        );
        CREATE TABLE IF NOT EXISTS workflow_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            workflow_id TEXT NOT NULL,
            phase INTEGER NOT NULL,
            agent_id TEXT NOT NULL,
            message TEXT NOT NULL,
            detail TEXT NOT NULL,
            progress REAL NOT NULL,
            requires_approval INTEGER NOT NULL DEFAULT 0,
            created_at REAL NOT NULL,
            FOREIGN KEY(workflow_id) REFERENCES workflow_runs(workflow_id)
        );
        CREATE INDEX IF NOT EXISTS idx_workflow_events_workflow_id_id
            ON workflow_events(workflow_id, id);",
    )
    .context("creating workflow persistence schema")?;
    Ok(())
}

pub fn load_workflows(db: &Connection) -> Result<HashMap<String, WorkflowState>> {
    let mut statement = db
        .prepare(
            "SELECT workflow_id, title, phase, mode, overall_progress, current_agent,
                    status_message, requires_approval, approved, cancelled
             FROM workflow_runs
             ORDER BY updated_at DESC",
        )
        .context("preparing workflow load query")?;

    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
                row.get::<_, f32>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, bool>(7)?,
                row.get::<_, bool>(8)?,
                row.get::<_, bool>(9)?,
            ))
        })
        .context("querying persisted workflows")?;

    let mut workflows = HashMap::new();
    for row in rows {
        let (
            workflow_id,
            title,
            phase,
            mode,
            overall_progress,
            current_agent,
            status_message,
            requires_approval,
            approved,
            cancelled,
        ) = row.context("reading persisted workflow row")?;
        let events = load_recent_events(db, &workflow_id)?;
        let status = WorkflowStatus {
            workflow_id: workflow_id.clone(),
            title,
            phase,
            mode,
            overall_progress,
            current_agent,
            status_message,
            recent_events: events.clone(),
            requires_approval,
            pending_approval: None,
        };
        workflows.insert(
            workflow_id,
            WorkflowState {
                status,
                events,
                cancelled,
                approved,
                mode,
            },
        );
    }
    Ok(workflows)
}

pub fn upsert_workflow(db: &Connection, state: &WorkflowState) -> Result<()> {
    let now = unix_seconds();
    db.execute(
        "INSERT INTO workflow_runs (
            workflow_id, title, phase, mode, overall_progress, current_agent,
            status_message, requires_approval, approved, cancelled, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
        ON CONFLICT(workflow_id) DO UPDATE SET
            title = excluded.title,
            phase = excluded.phase,
            mode = excluded.mode,
            overall_progress = excluded.overall_progress,
            current_agent = excluded.current_agent,
            status_message = excluded.status_message,
            requires_approval = excluded.requires_approval,
            approved = excluded.approved,
            cancelled = excluded.cancelled,
            updated_at = excluded.updated_at",
        params![
            state.status.workflow_id,
            state.status.title,
            state.status.phase,
            state.status.mode,
            state.status.overall_progress,
            state.status.current_agent,
            state.status.status_message,
            state.status.requires_approval,
            state.approved,
            state.cancelled,
            now,
        ],
    )
    .with_context(|| format!("persisting workflow {}", state.status.workflow_id))?;
    Ok(())
}

pub fn insert_event(db: &Connection, event: &WorkflowEvent) -> Result<()> {
    db.execute(
        "INSERT INTO workflow_events (
            workflow_id, phase, agent_id, message, detail, progress,
            requires_approval, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            event.workflow_id,
            event.phase,
            event.agent_id,
            event.message,
            event.detail,
            event.progress,
            event.requires_approval,
            unix_seconds(),
        ],
    )
    .with_context(|| format!("persisting workflow event {}", event.workflow_id))?;
    Ok(())
}

fn load_recent_events(db: &Connection, workflow_id: &str) -> Result<Vec<WorkflowEvent>> {
    let mut statement = db
        .prepare(
            "SELECT workflow_id, phase, agent_id, message, detail, progress, requires_approval
             FROM (
                SELECT workflow_id, phase, agent_id, message, detail, progress, requires_approval, id
                FROM workflow_events
                WHERE workflow_id = ?1
                ORDER BY id DESC
                LIMIT 20
             )
             ORDER BY id ASC",
        )
        .context("preparing workflow event load query")?;
    let events = statement
        .query_map([workflow_id], |row| {
            Ok(WorkflowEvent {
                workflow_id: row.get(0)?,
                phase: row.get(1)?,
                agent_id: row.get(2)?,
                message: row.get(3)?,
                detail: row.get(4)?,
                progress: row.get(5)?,
                requires_approval: row.get(6)?,
                approval: None,
            })
        })
        .with_context(|| format!("querying events for workflow {workflow_id}"))?;

    let mut loaded = Vec::new();
    for event in events {
        loaded.push(event.context("reading persisted workflow event")?);
    }
    Ok(loaded)
}

fn unix_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}
