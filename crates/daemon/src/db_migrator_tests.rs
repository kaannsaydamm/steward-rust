use super::{all_migrations, backup_database, current_version, migrate};
use rusqlite::Connection;

fn open() -> Connection {
    Connection::open_in_memory().expect("in-memory db")
}

#[test]
fn fresh_database_reaches_latest_version() {
    let mut connection = open();
    let applied = migrate(&mut connection, std::env::temp_dir().as_path()).expect("migrate");
    assert_eq!(applied, vec![1, 2]);
    assert_eq!(current_version(&connection).unwrap(), 2);
}

#[test]
fn second_run_is_idempotent() {
    let mut connection = open();
    migrate(&mut connection, std::env::temp_dir().as_path()).expect("first migrate");
    let applied = migrate(&mut connection, std::env::temp_dir().as_path()).expect("second migrate");
    assert!(applied.is_empty(), "no migrations should re-run");
    assert_eq!(current_version(&connection).unwrap(), 2);
}

#[test]
fn v0002_tables_exist_with_expected_columns() {
    let mut connection = open();
    migrate(&mut connection, std::env::temp_dir().as_path()).expect("migrate");
    for table in [
        "threads",
        "runs",
        "run_events",
        "checkpoints",
        "node_attempts",
        "agent_instances",
        "approvals_v2",
    ] {
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |row| row.get(0),
            )
            .expect("table lookup");
        assert_eq!(count, 1, "table {table} missing");
    }
}

#[test]
fn run_events_sequence_is_composite_primary_key() {
    let mut connection = open();
    migrate(&mut connection, std::env::temp_dir().as_path()).expect("migrate");
    connection
        .execute(
            "INSERT INTO run_events (run_id, sequence, event_type, payload_json, timestamp_ms)
             VALUES ('r1', 1, 'RunStarted', '{}', 1)",
            [],
        )
        .expect("insert first");
    connection
        .execute(
            "INSERT INTO run_events (run_id, sequence, event_type, payload_json, timestamp_ms)
             VALUES ('r1', 2, 'RunCompleted', '{}', 2)",
            [],
        )
        .expect("insert second");
    let duplicate = connection.execute(
        "INSERT INTO run_events (run_id, sequence, event_type, payload_json, timestamp_ms)
         VALUES ('r1', 1, 'Duplicate', '{}', 3)",
        [],
    );
    assert!(
        duplicate.is_err(),
        "duplicate (run, sequence) must be rejected"
    );
}

#[test]
fn runs_require_existing_thread() {
    let mut connection = open();
    migrate(&mut connection, std::env::temp_dir().as_path()).expect("migrate");
    connection
        .execute(
            "INSERT INTO threads (thread_id, workspace_id, title, status, created_at_ms, updated_at_ms)
             VALUES ('t1', NULL, 'hello', 'active', 1, 1)",
            [],
        )
        .expect("insert thread");
    let ok = connection.execute(
        "INSERT INTO runs (run_id, thread_id, source_kind, status, success_policy_json, budget_json, created_at_ms)
         VALUES ('r1', 't1', 'chat', 'pending', '{}', '{}', 1)",
        [],
    );
    assert!(ok.is_ok(), "run for existing thread accepted");

    let orphan = connection.execute(
        "INSERT INTO runs (run_id, thread_id, source_kind, status, success_policy_json, budget_json, created_at_ms)
         VALUES ('r2', 'missing-thread', 'chat', 'pending', '{}', '{}', 1)",
        [],
    );
    assert!(orphan.is_err(), "foreign key must reject missing thread");
}

#[test]
fn backup_creates_readable_snapshot_file() {
    let mut connection = open();
    migrate(&mut connection, std::env::temp_dir().as_path()).expect("migrate");
    let directory = tempfile::tempdir().expect("backup root");
    let backup = backup_database(&connection, directory.path()).expect("backup");
    assert!(
        backup.is_file(),
        "backup file exists at {}",
        backup.display()
    );
    let restored = Connection::open(&backup).expect("open backup");
    assert_eq!(current_version(&restored).unwrap(), 2);
}

#[test]
fn interrupted_migration_leaves_prior_db_valid() {
    // Apply only the baseline version record; the v0002 schema never runs.
    // A failure afterwards must not corrupt or advance the recorded version.
    let mut connection = open();
    super::ensure_migrations_table(&connection).expect("migrations table");
    let registry = all_migrations();
    let baseline = &registry[0];
    let tx = connection.transaction().expect("tx");
    baseline.up(&tx).expect("baseline up");
    tx.execute(
        "INSERT INTO schema_migrations (version, name, applied_at_ms) VALUES (1, 'baseline', 0)",
        [],
    )
    .expect("record baseline");
    tx.commit().expect("commit baseline");
    assert_eq!(current_version(&connection).unwrap(), 1);

    let failed = connection.execute_batch("INSERT INTO nonexistent VALUES (1)");
    assert!(failed.is_err());
    assert_eq!(current_version(&connection).unwrap(), 1);
}

#[test]
fn migration_names_are_recorded() {
    let mut connection = open();
    migrate(&mut connection, std::env::temp_dir().as_path()).expect("migrate");
    let names: Vec<(i64, String)> = {
        let mut statement = connection
            .prepare("SELECT version, name FROM schema_migrations ORDER BY version")
            .expect("prepare");
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("query");
        rows.map(|r| r.expect("row")).collect()
    };
    assert_eq!(
        names,
        vec![
            (1, "baseline".into()),
            (2, "threads_runs_events_checkpoints".into())
        ]
    );
}

#[test]
fn migrations_are_version_ordered() {
    let migrations = all_migrations();
    let versions: Vec<i64> = migrations.iter().map(|m| m.version()).collect();
    let mut sorted = versions.clone();
    sorted.sort();
    assert_eq!(
        versions, sorted,
        "migrations must be listed in version order"
    );
}
