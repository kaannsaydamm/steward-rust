//! Store tests: durability contracts for threads, runs, events, checkpoints.

use super::*;
use rusqlite::Connection;

fn migrated() -> Connection {
    let mut connection = Connection::open_in_memory().expect("open memory db");
    crate::db_migrator::migrate(&mut connection, std::env::temp_dir().as_path())
        .expect("run migrations");
    connection
}

fn thread(id: &str) -> Thread {
    Thread {
        thread_id: id.to_owned(),
        workspace_id: Some("ws_1".into()),
        title: format!("thread {id}"),
        status: "active".into(),
        created_at_ms: 1,
        updated_at_ms: 1,
        metadata: serde_json::json!({}),
    }
}

fn run(id: &str, thread_id: &str) -> Run {
    Run {
        run_id: id.to_owned(),
        thread_id: thread_id.to_owned(),
        parent_run_id: None,
        source_kind: "chat".into(),
        status: "pending".into(),
        execution_plan: None,
        success_policy: serde_json::json!({"kind": "AgentDeclared"}),
        budget: serde_json::json!({"max_model_calls": 8}),
        created_at_ms: 2,
        started_at_ms: None,
        completed_at_ms: None,
    }
}

#[test]
fn thread_create_get_archive_roundtrip() {
    let connection = migrated();
    create_thread(&connection, &thread("thr_1")).expect("create");
    let loaded = get_thread(&connection, "thr_1")
        .expect("get")
        .expect("present");
    assert_eq!(loaded.title, "thread thr_1");
    assert_eq!(loaded.status, "active");

    archive_thread(&connection, "thr_1").expect("archive");
    let archived = get_thread(&connection, "thr_1").expect("get").unwrap();
    assert_eq!(archived.status, "archived");

    assert!(get_thread(&connection, "missing").expect("get").is_none());
}

#[test]
fn thread_list_orders_by_recency() {
    let connection = migrated();
    let mut older = thread("thr_old");
    older.updated_at_ms = 1;
    let mut newer = thread("thr_new");
    newer.updated_at_ms = 99;
    create_thread(&connection, &older).unwrap();
    create_thread(&connection, &newer).unwrap();

    let listed = list_threads(&connection, 10).unwrap();
    assert_eq!(listed[0].thread_id, "thr_new");
}

#[test]
fn run_create_transition_and_foreign_key() {
    let connection = migrated();
    create_thread(&connection, &thread("thr_1")).unwrap();
    create_run(&connection, &run("run_1", "thr_1")).expect("create run");

    transition_run(&connection, "run_1", "running").unwrap();
    let running = get_run(&connection, "run_1").unwrap().unwrap();
    assert_eq!(running.status, "running");
    assert!(running.started_at_ms.is_some(), "running sets started_at");

    transition_run(&connection, "run_1", "succeeded").unwrap();
    let done = get_run(&connection, "run_1").unwrap().unwrap();
    assert_eq!(done.status, "succeeded");
    assert!(done.completed_at_ms.is_some());

    // Invalid status names are rejected.
    assert!(transition_run(&connection, "run_1", "flying").is_err());
    // Unknown run ids are rejected.
    assert!(transition_run(&connection, "run_missing", "running").is_err());
    // Missing thread violates FK.
    assert!(create_run(&connection, &run("run_2", "thr_missing")).is_err());
}

#[test]
fn run_status_terminal_records_completion_once() {
    let connection = migrated();
    create_thread(&connection, &thread("thr_1")).unwrap();
    create_run(&connection, &run("run_1", "thr_1")).unwrap();
    transition_run(&connection, "run_1", "running").unwrap();
    transition_run(&connection, "run_1", "cancelled").unwrap();
    let first = get_run(&connection, "run_1")
        .unwrap()
        .unwrap()
        .completed_at_ms;
    transition_run(&connection, "run_1", "cancelled").unwrap();
    let again = get_run(&connection, "run_1")
        .unwrap()
        .unwrap()
        .completed_at_ms;
    assert_eq!(first, again, "completed_at must not be overwritten");
}

#[test]
fn event_sequence_monotonic_without_gaps() {
    let connection = migrated();
    create_thread(&connection, &thread("thr_1")).unwrap();
    create_run(&connection, &run("run_1", "thr_1")).unwrap();

    for index in 0..5 {
        let event = append_event(
            &connection,
            "run_1",
            "ToolCompleted",
            &serde_json::json!(index),
        )
        .expect("append");
        assert_eq!(event.sequence, index + 1, "sequence is gapless");
    }

    let after_3 = events_after(&connection, "run_1", 3).unwrap();
    assert_eq!(after_3.len(), 2);
    assert_eq!(after_3[0].sequence, 4);
    assert_eq!(after_3[1].sequence, 5);
}

#[test]
fn event_sequences_are_isolated_per_run() {
    let connection = migrated();
    create_thread(&connection, &thread("thr_1")).unwrap();
    create_run(&connection, &run("run_a", "thr_1")).unwrap();
    create_run(&connection, &run("run_b", "thr_1")).unwrap();

    let a = append_event(&connection, "run_a", "RunStarted", &serde_json::json!({})).unwrap();
    let b = append_event(&connection, "run_b", "RunStarted", &serde_json::json!({})).unwrap();
    assert_eq!((a.sequence, b.sequence), (1, 1), "per-run sequences");
}

#[test]
fn checkpoint_save_latest_load_roundtrip() {
    let connection = migrated();
    create_thread(&connection, &thread("thr_1")).unwrap();
    create_run(&connection, &run("run_1", "thr_1")).unwrap();

    let parent = Checkpoint {
        checkpoint_id: "chk_1".into(),
        run_id: "run_1".into(),
        sequence: 1,
        parent_checkpoint_id: None,
        state: serde_json::json!({"turn": 1}),
        created_at_ms: 10,
    };
    save_checkpoint(&connection, &parent).unwrap();

    let child = Checkpoint {
        checkpoint_id: "chk_2".into(),
        run_id: "run_1".into(),
        sequence: 2,
        parent_checkpoint_id: Some("chk_1".into()),
        state: serde_json::json!({"turn": 2}),
        created_at_ms: 20,
    };
    save_checkpoint(&connection, &child).unwrap();

    let latest = latest_checkpoint(&connection, "run_1").unwrap().unwrap();
    assert_eq!(latest.checkpoint_id, "chk_2");
    assert_eq!(latest.parent_checkpoint_id.as_deref(), Some("chk_1"));

    let loaded = load_checkpoint(&connection, "chk_1").unwrap().unwrap();
    assert_eq!(loaded.state, serde_json::json!({"turn": 1}));
    assert!(load_checkpoint(&connection, "chk_missing")
        .unwrap()
        .is_none());
}

#[test]
fn run_list_for_thread_orders_chronologically() {
    let connection = migrated();
    create_thread(&connection, &thread("thr_1")).unwrap();
    let mut first = run("run_1", "thr_1");
    first.created_at_ms = 5;
    let mut second = run("run_2", "thr_1");
    second.created_at_ms = 6;
    create_run(&connection, &second).unwrap();
    create_run(&connection, &first).unwrap();

    let listed = list_runs_for_thread(&connection, "thr_1").unwrap();
    assert_eq!(
        listed.iter().map(|r| r.run_id.as_str()).collect::<Vec<_>>(),
        vec!["run_1", "run_2"]
    );
}
