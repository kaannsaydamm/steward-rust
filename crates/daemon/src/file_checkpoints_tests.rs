use super::{create_schema, list, record, rollback};
use rusqlite::Connection;

fn setup() -> Connection {
    let connection = Connection::open_in_memory().expect("open database");
    create_schema(&connection).expect("checkpoint schema");
    connection
}

#[test]
fn rollback_restores_the_previous_content() {
    let connection = setup();
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().unwrap();
    std::fs::write(temp.path().join("notes.txt"), "new content").expect("write current");
    let checkpoint =
        record(&connection, root, "notes.txt", Some(b"old content")).expect("record checkpoint");

    let output = rollback(&connection, checkpoint).expect("rollback");

    assert!(output.starts_with("restored"));
    let restored = std::fs::read_to_string(temp.path().join("notes.txt")).expect("read back");
    assert_eq!(restored, "old content");
}

#[test]
fn rollback_deletes_files_that_did_not_exist_before() {
    let connection = setup();
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().unwrap();
    std::fs::write(temp.path().join("created.txt"), "fresh").expect("write current");
    let checkpoint = record(&connection, root, "created.txt", None).expect("record checkpoint");

    let output = rollback(&connection, checkpoint).expect("rollback");

    assert!(output.starts_with("removed"));
    assert!(!temp.path().join("created.txt").exists());
}

#[test]
fn rollback_is_itself_reversible() {
    let connection = setup();
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().unwrap();
    std::fs::write(temp.path().join("notes.txt"), "version 2").expect("write current");
    let first = record(&connection, root, "notes.txt", Some(b"version 1")).expect("record");

    rollback(&connection, first).expect("first rollback");
    // The rollback recorded "version 2" as a new checkpoint; rolling that back returns us
    // to the state before the first rollback.
    let checkpoints = list(&connection, 10).expect("list checkpoints");
    let undo = checkpoints[0].checkpoint_id;
    rollback(&connection, undo).expect("undo rollback");

    let content = std::fs::read_to_string(temp.path().join("notes.txt")).expect("read back");
    assert_eq!(content, "version 2");
}

#[test]
fn rollback_of_unknown_checkpoint_fails() {
    let connection = setup();

    let error = rollback(&connection, 999).expect_err("unknown checkpoint");

    assert!(error.to_string().contains("not found"));
}

#[test]
fn list_reports_newest_first_with_sizes() {
    let connection = setup();
    record(&connection, "/root", "a.txt", Some(b"aaaa")).expect("first");
    record(&connection, "/root", "b.txt", None).expect("second");

    let checkpoints = list(&connection, 10).expect("list");

    assert_eq!(checkpoints.len(), 2);
    assert_eq!(checkpoints[0].path, "b.txt");
    assert!(!checkpoints[0].existed_before);
    assert_eq!(checkpoints[1].previous_bytes, 4);
    assert!(checkpoints[1].existed_before);
}
