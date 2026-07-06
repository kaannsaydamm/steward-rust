use super::{initialize, list_skills, list_tools, set_tool_enabled, RiskLevel, ToolRuntime};
use rusqlite::Connection;

#[test]
fn initialize_seeds_deterministic_tools_and_skills() {
    let connection = Connection::open_in_memory().expect("open registry database");

    initialize(&connection).expect("initialize registry");
    initialize(&connection).expect("initialize registry twice");
    let tools = list_tools(&connection).expect("list tools");
    let skills = list_skills(&connection).expect("list skills");

    assert_eq!(tools.len(), 8);
    assert_eq!(skills.len(), 3);
    assert_eq!(tools[0].id, "fs.read");
    assert_eq!(skills[0].id, "codebase-research");
    assert_eq!(
        skills[0].tool_ids,
        vec!["fs.search", "fs.read", "memory.recall"]
    );
}

#[test]
fn process_execution_is_disabled_and_requires_approval() {
    let connection = Connection::open_in_memory().expect("open registry database");
    initialize(&connection).expect("initialize registry");

    let tool = list_tools(&connection)
        .expect("list tools")
        .into_iter()
        .find(|tool| tool.id == "process.exec")
        .expect("process execution tool");

    assert_eq!(tool.runtime, ToolRuntime::Process);
    assert_eq!(tool.risk, RiskLevel::High);
    assert!(!tool.enabled);
    assert!(tool.requires_approval);
}

#[test]
fn skill_tool_relationship_rejects_unknown_tools() {
    let connection = Connection::open_in_memory().expect("open registry database");
    initialize(&connection).expect("initialize registry");

    let result = connection.execute(
        "INSERT INTO skill_tools (skill_id, tool_id, position, required)
         VALUES ('codebase-research', 'missing.tool', 99, 1)",
        [],
    );

    assert!(result.is_err());
}

#[test]
fn reseeding_preserves_operator_enablement() {
    let connection = Connection::open_in_memory().expect("open registry database");
    initialize(&connection).expect("initialize registry");
    connection
        .execute(
            "UPDATE tools SET enabled = 1 WHERE tool_id = 'process.exec'",
            [],
        )
        .expect("enable process tool");

    initialize(&connection).expect("reseed registry");
    let tool = list_tools(&connection)
        .expect("list tools")
        .into_iter()
        .find(|tool| tool.id == "process.exec")
        .expect("process execution tool");

    assert!(tool.enabled);
}

#[test]
fn set_tool_enabled_updates_and_returns_the_tool() {
    let connection = Connection::open_in_memory().expect("open registry database");
    initialize(&connection).expect("initialize registry");

    let updated = set_tool_enabled(&connection, "fs.read", true).expect("enable fs.read");

    assert!(updated.enabled);
    let persisted = list_tools(&connection)
        .expect("list tools")
        .into_iter()
        .find(|tool| tool.id == "fs.read")
        .expect("fs.read tool");
    assert!(persisted.enabled);
}

#[test]
fn set_tool_enabled_fails_for_unknown_tool() {
    let connection = Connection::open_in_memory().expect("open registry database");
    initialize(&connection).expect("initialize registry");

    let error = set_tool_enabled(&connection, "missing.tool", true).expect_err("unknown tool");

    assert!(error.to_string().contains("does not exist"));
}

#[test]
fn schema_rejects_unknown_runtime_codes() {
    let connection = Connection::open_in_memory().expect("open registry database");
    initialize(&connection).expect("initialize registry");

    let result = connection.execute(
        "INSERT INTO tools
         (tool_id, name, description, runtime, risk_level, enabled, requires_approval)
         VALUES ('invalid', 'Invalid', 'Invalid runtime', 'native', 'low', 1, 0)",
        [],
    );

    assert!(result.is_err());
}
