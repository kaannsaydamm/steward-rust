use super::{
    append_message, create_schema, create_session, get_session, list_sessions, replace_with_summary,
};
use rusqlite::Connection;

#[test]
fn session_round_trip_preserves_messages_in_order() {
    let connection = Connection::open_in_memory().expect("in-memory database");
    create_schema(&connection).expect("session schema");
    let session =
        create_session(&connection, "profile-a", "model-a", "First task").expect("create session");
    append_message(&connection, &session.session_id, "user", "hello", "", "")
        .expect("user message");
    append_message(
        &connection,
        &session.session_id,
        "assistant",
        "world",
        "",
        "",
    )
    .expect("assistant message");

    let loaded = get_session(&connection, &session.session_id)
        .expect("get session")
        .expect("session exists");

    assert_eq!(loaded.messages.len(), 2);
    assert_eq!(loaded.messages[0].content, "hello");
    assert_eq!(loaded.messages[1].content, "world");
}

#[test]
fn compaction_replaces_all_messages_with_one_system_summary() {
    let connection = Connection::open_in_memory().expect("in-memory database");
    create_schema(&connection).expect("session schema");
    let session = create_session(&connection, "profile", "model", "Task").expect("create session");
    append_message(&connection, &session.session_id, "user", "hello", "", "").expect("user");
    append_message(
        &connection,
        &session.session_id,
        "assistant",
        "world",
        "",
        "",
    )
    .expect("assistant");

    let removed = replace_with_summary(&connection, &session.session_id, "greeted each other")
        .expect("compact session");

    assert_eq!(removed, 2);
    let loaded = get_session(&connection, &session.session_id)
        .expect("get session")
        .expect("session exists");
    assert_eq!(loaded.messages.len(), 1);
    assert_eq!(loaded.messages[0].role, "system");
    assert!(loaded.messages[0].content.contains("greeted each other"));
}

#[test]
fn compaction_fails_on_an_empty_session() {
    let connection = Connection::open_in_memory().expect("in-memory database");
    create_schema(&connection).expect("session schema");
    let session = create_session(&connection, "profile", "model", "Task").expect("create session");

    let error = replace_with_summary(&connection, &session.session_id, "summary")
        .expect_err("empty session cannot be compacted");

    assert!(error.to_string().contains("no messages"));
}

#[test]
fn listing_sessions_returns_most_recent_first() {
    let connection = Connection::open_in_memory().expect("in-memory database");
    create_schema(&connection).expect("session schema");
    let first = create_session(&connection, "profile", "model", "First").expect("first");
    let second = create_session(&connection, "profile", "model", "Second").expect("second");
    append_message(&connection, &first.session_id, "user", "update", "", "").expect("touch first");

    let sessions = list_sessions(&connection, 10).expect("list sessions");

    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].session_id, first.session_id);
    assert_eq!(sessions[1].session_id, second.session_id);
}
