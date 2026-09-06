//! Durable stores for the Omega run substrate (Phase 3, Tasks 3.1-3.3).
//!
//! All stores operate on a `rusqlite::Connection`; the daemon shares one
//! connection behind a mutex, so store functions are synchronous. Event
//! sequence assignment happens inside the same transaction as the insert
//! (§49 invariant: monotonic per run, no gaps/duplicates).

use anyhow::{Context as _, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Public clock for sibling modules composing store rows (fork/replay).
pub fn now_ms_pub() -> i64 {
    now_ms()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Thread {
    pub thread_id: String,
    pub workspace_id: Option<String>,
    pub title: String,
    pub status: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub metadata: Value,
}

pub fn create_thread(connection: &Connection, thread: &Thread) -> Result<()> {
    connection
        .execute(
            "INSERT INTO threads (thread_id, workspace_id, title, status, created_at_ms, updated_at_ms, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                thread.thread_id,
                thread.workspace_id,
                thread.title,
                thread.status,
                thread.created_at_ms,
                thread.updated_at_ms,
                thread.metadata.to_string(),
            ],
        )
        .context("inserting thread")?;
    Ok(())
}

pub fn get_thread(connection: &Connection, thread_id: &str) -> Result<Option<Thread>> {
    let thread = connection
        .query_row(
            "SELECT thread_id, workspace_id, title, status, created_at_ms, updated_at_ms, metadata_json
             FROM threads WHERE thread_id = ?1",
            [thread_id],
            |row| {
                Ok(Thread {
                    thread_id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    title: row.get(2)?,
                    status: row.get(3)?,
                    created_at_ms: row.get(4)?,
                    updated_at_ms: row.get(5)?,
                    metadata: serde_json::from_str(&row.get::<_, String>(6)?)
                        .unwrap_or(Value::Null),
                })
            },
        )
        .optional()
        .context("loading thread")?;
    Ok(thread)
}

pub fn list_threads(connection: &Connection, limit: u32) -> Result<Vec<Thread>> {
    let mut statement = connection
        .prepare(
            "SELECT thread_id, workspace_id, title, status, created_at_ms, updated_at_ms, metadata_json
             FROM threads ORDER BY updated_at_ms DESC LIMIT ?1",
        )
        .context("preparing thread list")?;
    let rows = statement
        .query_map([limit], map_thread_row)
        .context("listing threads")?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .context("iterating threads")
}

pub fn archive_thread(connection: &Connection, thread_id: &str) -> Result<()> {
    connection
        .execute(
            "UPDATE threads SET status = 'archived', updated_at_ms = ?2 WHERE thread_id = ?1",
            params![thread_id, now_ms()],
        )
        .context("archiving thread")?;
    Ok(())
}

fn map_thread_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Thread> {
    Ok(Thread {
        thread_id: row.get(0)?,
        workspace_id: row.get(1)?,
        title: row.get(2)?,
        status: row.get(3)?,
        created_at_ms: row.get(4)?,
        updated_at_ms: row.get(5)?,
        metadata: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or(Value::Null),
    })
}

// ------------------------------------------------------------------- runs ---

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    pub run_id: String,
    pub thread_id: String,
    pub parent_run_id: Option<String>,
    pub source_kind: String,
    pub status: String,
    pub execution_plan: Option<Value>,
    pub success_policy: Value,
    pub budget: Value,
    pub created_at_ms: i64,
    pub started_at_ms: Option<i64>,
    pub completed_at_ms: Option<i64>,
}

pub fn create_run(connection: &Connection, run: &Run) -> Result<()> {
    connection
        .execute(
            "INSERT INTO runs (run_id, thread_id, parent_run_id, source_kind, status, execution_plan_json,
                               success_policy_json, budget_json, created_at_ms, started_at_ms, completed_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                run.run_id,
                run.thread_id,
                run.parent_run_id,
                run.source_kind,
                run.status,
                run.execution_plan.as_ref().map(Value::to_string),
                run.success_policy.to_string(),
                run.budget.to_string(),
                run.created_at_ms,
                run.started_at_ms,
                run.completed_at_ms,
            ],
        )
        .context("inserting run")?;
    Ok(())
}

pub fn get_run(connection: &Connection, run_id: &str) -> Result<Option<Run>> {
    let run = connection
        .query_row(
            "SELECT run_id, thread_id, parent_run_id, source_kind, status, execution_plan_json,
                    success_policy_json, budget_json, created_at_ms, started_at_ms, completed_at_ms
             FROM runs WHERE run_id = ?1",
            [run_id],
            |row| {
                Ok(Run {
                    run_id: row.get(0)?,
                    thread_id: row.get(1)?,
                    parent_run_id: row.get(2)?,
                    source_kind: row.get(3)?,
                    status: row.get(4)?,
                    execution_plan: row
                        .get::<_, Option<String>>(5)?
                        .and_then(|json| serde_json::from_str(&json).ok()),
                    success_policy: serde_json::from_str(&row.get::<_, String>(6)?)
                        .unwrap_or(Value::Null),
                    budget: serde_json::from_str(&row.get::<_, String>(7)?)
                        .unwrap_or(Value::Null),
                    created_at_ms: row.get(8)?,
                    started_at_ms: row.get(9)?,
                    completed_at_ms: row.get(10)?,
                })
            },
        )
        .optional()
        .context("loading run")?;
    Ok(run)
}

pub fn list_runs_for_thread(connection: &Connection, thread_id: &str) -> Result<Vec<Run>> {
    let mut statement = connection
        .prepare(
            "SELECT run_id, thread_id, parent_run_id, source_kind, status, execution_plan_json,
                    success_policy_json, budget_json, created_at_ms, started_at_ms, completed_at_ms
             FROM runs WHERE thread_id = ?1 ORDER BY created_at_ms",
        )
        .context("preparing run list")?;
    let rows = statement
        .query_map([thread_id], |row| {
            Ok(Run {
                run_id: row.get(0)?,
                thread_id: row.get(1)?,
                parent_run_id: row.get(2)?,
                source_kind: row.get(3)?,
                status: row.get(4)?,
                execution_plan: row
                    .get::<_, Option<String>>(5)?
                    .and_then(|json| serde_json::from_str(&json).ok()),
                success_policy: serde_json::from_str(&row.get::<_, String>(6)?)
                    .unwrap_or(Value::Null),
                budget: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or(Value::Null),
                created_at_ms: row.get(8)?,
                started_at_ms: row.get(9)?,
                completed_at_ms: row.get(10)?,
            })
        })
        .context("listing runs")?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .context("iterating runs")
}

/// Transitions a run's status; unknown statuses are rejected so the state
/// machine (§13.1) cannot be bypassed by typo.
pub fn transition_run(connection: &Connection, run_id: &str, status: &str) -> Result<()> {
    const ALLOWED: &[&str] = &[
        "pending", "ready", "claimed", "running", "waiting", "succeeded", "failed_retryable",
        "failed_terminal", "cancelled", "unknown_effect",
    ];
    anyhow::ensure!(
        ALLOWED.contains(&status),
        "invalid run status '{status}'"
    );
    let completed = if matches!(status, "succeeded" | "failed_terminal" | "cancelled") {
        Some(now_ms())
    } else {
        None
    };
    let updated = connection
        .execute(
            "UPDATE runs SET status = ?2,
                 completed_at_ms = COALESCE(?3, completed_at_ms),
                 started_at_ms = COALESCE(started_at_ms, CASE WHEN ?2 = 'running' THEN ?4 ELSE NULL END)
             WHERE run_id = ?1",
            params![run_id, status, completed, now_ms()],
        )
        .context("transitioning run")?;
    anyhow::ensure!(updated == 1, "run '{run_id}' not found");
    Ok(())
}

// ----------------------------------------------------------------- events ---

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunEvent {
    pub run_id: String,
    pub sequence: i64,
    pub event_type: String,
    pub payload: Value,
    pub timestamp_ms: i64,
}

/// Appends an event with a transactionally assigned monotonic sequence.
/// The connection must not already be inside a transaction.
pub fn append_event(connection: &Connection, run_id: &str, event_type: &str, payload: &Value) -> Result<RunEvent> {
    let tx = connection.unchecked_transaction().context("opening event tx")?;
    let next: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM run_events WHERE run_id = ?1",
            [run_id],
            |row| row.get(0),
        )
        .context("reading next sequence")?;
    let event = RunEvent {
        run_id: run_id.to_owned(),
        sequence: next,
        event_type: event_type.to_owned(),
        payload: payload.clone(),
        timestamp_ms: now_ms(),
    };
    tx.execute(
        "INSERT INTO run_events (run_id, sequence, event_type, payload_json, timestamp_ms)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![run_id, next, event_type, payload.to_string(), event.timestamp_ms],
    )
    .context("inserting event")?;
    tx.commit().context("committing event")?;
    Ok(event)
}

/// Events strictly after `after_sequence` in ascending order (reconnect).
pub fn events_after(connection: &Connection, run_id: &str, after_sequence: i64) -> Result<Vec<RunEvent>> {
    let mut statement = connection
        .prepare(
            "SELECT run_id, sequence, event_type, payload_json, timestamp_ms
             FROM run_events WHERE run_id = ?1 AND sequence > ?2 ORDER BY sequence",
        )
        .context("preparing event query")?;
    let rows = statement
        .query_map(params![run_id, after_sequence], |row| {
            Ok(RunEvent {
                run_id: row.get(0)?,
                sequence: row.get(1)?,
                event_type: row.get(2)?,
                payload: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or(Value::Null),
                timestamp_ms: row.get(4)?,
            })
        })
        .context("querying events")?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .context("iterating events")
}

// ------------------------------------------------------------ checkpoints ---

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub checkpoint_id: String,
    pub run_id: String,
    pub sequence: i64,
    pub parent_checkpoint_id: Option<String>,
    pub state: Value,
    pub created_at_ms: i64,
}

pub fn save_checkpoint(connection: &Connection, checkpoint: &Checkpoint) -> Result<()> {
    connection
        .execute(
            "INSERT INTO checkpoints (checkpoint_id, run_id, sequence, parent_checkpoint_id, state_blob, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                checkpoint.checkpoint_id,
                checkpoint.run_id,
                checkpoint.sequence,
                checkpoint.parent_checkpoint_id,
                checkpoint.state.to_string(),
                checkpoint.created_at_ms,
            ],
        )
        .context("inserting checkpoint")?;
    Ok(())
}

pub fn latest_checkpoint(connection: &Connection, run_id: &str) -> Result<Option<Checkpoint>> {
    let checkpoint = connection
        .query_row(
            "SELECT checkpoint_id, run_id, sequence, parent_checkpoint_id, state_blob, created_at_ms
             FROM checkpoints WHERE run_id = ?1 ORDER BY sequence DESC LIMIT 1",
            [run_id],
            map_checkpoint_row,
        )
        .optional()
        .context("loading latest checkpoint")?;
    Ok(checkpoint)
}

pub fn load_checkpoint(connection: &Connection, checkpoint_id: &str) -> Result<Option<Checkpoint>> {
    let checkpoint = connection
        .query_row(
            "SELECT checkpoint_id, run_id, sequence, parent_checkpoint_id, state_blob, created_at_ms
             FROM checkpoints WHERE checkpoint_id = ?1",
            [checkpoint_id],
            map_checkpoint_row,
        )
        .optional()
        .context("loading checkpoint")?;
    Ok(checkpoint)
}

fn map_checkpoint_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Checkpoint> {
    Ok(Checkpoint {
        checkpoint_id: row.get(0)?,
        run_id: row.get(1)?,
        sequence: row.get(2)?,
        parent_checkpoint_id: row.get(3)?,
        state: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or(Value::Null),
        created_at_ms: row.get(5)?,
    })
}

#[cfg(test)]
#[path = "stores_tests.rs"]
mod stores_tests;
