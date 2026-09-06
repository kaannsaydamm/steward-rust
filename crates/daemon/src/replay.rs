//! Fork/replay semantics (Task 3.4).
//!
//! Forking a run creates a new RunId linked to the parent via `parent_run_id`
//! and a fresh run row cloned from the source. The source run and its
//! checkpoints stay immutable. Replay walks recorded events; external side
//! effects are NOT re-executed unless the caller opts into `live` mode.

use super::stores;
use anyhow::{Context as _, Result};
use rusqlite::Connection;
use serde_json::json;
use steward_core::ids::RunId;

/// Forks a run at (or from the start of) its latest checkpoint. Returns the
/// new run id. Source rows are never mutated.
pub fn fork_run(connection: &Connection, source_run_id: &str) -> Result<RunId> {
    let source = stores::get_run(connection, source_run_id)
        .context("loading source run")?
        .context("source run does not exist")?;
    let new_run = RunId::generate();
    let forked = stores::Run {
        run_id: new_run.to_string(),
        thread_id: source.thread_id,
        parent_run_id: Some(source.run_id),
        source_kind: source.source_kind,
        status: "pending".into(),
        execution_plan: source.execution_plan,
        success_policy: source.success_policy,
        budget: source.budget,
        created_at_ms: stores::now_ms_pub(),
        started_at_ms: None,
        completed_at_ms: None,
    };
    stores::create_run(connection, &forked).context("creating forked run")?;

    if let Some(checkpoint) = stores::latest_checkpoint(connection, source_run_id)? {
        // Record the fork point as the first event on the new run.
        stores::append_event(
            connection,
            new_run.as_str(),
            "RunCreated",
            &json!({
                "forked_from": source_run_id,
                "fork_point_checkpoint": checkpoint.checkpoint_id,
            }),
        )?;
    }
    Ok(new_run)
}

/// Replays a run's recorded events. In `Dry` mode (default) this is a pure
/// read — the default and only mode until live replay exists (Phase 13).
pub fn replay_events(
    connection: &Connection,
    run_id: &str,
) -> Result<Vec<stores::RunEvent>> {
    stores::events_after(connection, run_id, 0)
        .context("loading events for replay")
}

#[cfg(test)]
mod tests {
    use super::super::stores;
    use super::*;
    use rusqlite::Connection;

    fn setup() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::db_migrator::migrate(&mut connection, std::env::temp_dir().as_path()).unwrap();
        connection
    }

    fn seed_run(connection: &Connection) -> String {
        stores::create_thread(
            connection,
            &stores::Thread {
                thread_id: "thr_1".into(),
                workspace_id: None,
                title: "t".into(),
                status: "active".into(),
                created_at_ms: 1,
                updated_at_ms: 1,
                metadata: serde_json::json!({}),
            },
        )
        .unwrap();
        stores::create_run(
            connection,
            &stores::Run {
                run_id: "run_src".into(),
                thread_id: "thr_1".into(),
                parent_run_id: None,
                source_kind: "chat".into(),
                status: "succeeded".into(),
                execution_plan: None,
                success_policy: serde_json::json!({}),
                budget: serde_json::json!({}),
                created_at_ms: 2,
                started_at_ms: Some(2),
                completed_at_ms: Some(3),
            },
        )
        .unwrap();
        stores::append_event(connection, "run_src", "RunStarted", &serde_json::json!({})).unwrap();
        stores::append_event(connection, "run_src", "RunCompleted", &serde_json::json!({})).unwrap();
        stores::save_checkpoint(
            connection,
            &stores::Checkpoint {
                checkpoint_id: "chk_src".into(),
                run_id: "run_src".into(),
                sequence: 1,
                parent_checkpoint_id: None,
                state: serde_json::json!({"turn": 2}),
                created_at_ms: 5,
            },
        )
        .unwrap();
        "run_src".into()
    }

    #[test]
    fn fork_creates_new_run_with_parent_linkage() {
        let connection = setup();
        let source = seed_run(&connection);
        let forked = fork_run(&connection, &source).unwrap();
        assert_ne!(forked.as_str(), source);

        let new_run = stores::get_run(&connection, forked.as_str()).unwrap().unwrap();
        assert_eq!(new_run.parent_run_id.as_deref(), Some(source.as_str()));
        assert_eq!(new_run.status, "pending");
        assert!(new_run.started_at_ms.is_none(), "fork starts clean");

        // Source untouched.
        let original = stores::get_run(&connection, &source).unwrap().unwrap();
        assert_eq!(original.status, "succeeded");
        assert_eq!(original.completed_at_ms, Some(3));
    }

    #[test]
    fn fork_records_checkpoint_lineage_event() {
        let connection = setup();
        let source = seed_run(&connection);
        let forked = fork_run(&connection, &source).unwrap();
        let events = stores::events_after(&connection, forked.as_str(), 0).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "RunCreated");
        assert!(events[0].payload["fork_point_checkpoint"].is_string());
    }

    #[test]
    fn source_checkpoints_are_immutable_after_fork() {
        let connection = setup();
        let source = seed_run(&connection);
        let before = stores::load_checkpoint(&connection, "chk_src").unwrap().unwrap();
        let _fork = fork_run(&connection, &source).unwrap();
        let after = stores::load_checkpoint(&connection, "chk_src").unwrap().unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn replay_returns_all_recorded_events() {
        let connection = setup();
        let source = seed_run(&connection);
        let events = replay_events(&connection, &source).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "RunStarted");
        assert_eq!(events[1].event_type, "RunCompleted");
    }

    #[test]
    fn fork_of_missing_run_fails() {
        let connection = setup();
        assert!(fork_run(&connection, "run_missing").is_err());
    }
}
