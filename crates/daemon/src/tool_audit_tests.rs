use super::{list_recent, record, InvocationStatus, NewInvocation};
use crate::tool_registry;
use rusqlite::Connection;

#[test]
fn records_policy_and_terminal_invocation_outcomes() {
    let connection = Connection::open_in_memory().expect("open audit database");
    tool_registry::initialize(&connection).expect("initialize registry");

    let pending_id = record(
        &connection,
        NewInvocation {
            tool_id: "memory.store",
            approved: false,
            input_json: "{\"content\":\"lesson\"}",
            status: InvocationStatus::PendingApproval,
            output: "",
            error: "approval required",
        },
    )
    .expect("record pending invocation");
    let success_id = record(
        &connection,
        NewInvocation {
            tool_id: "memory.recall",
            approved: false,
            input_json: "{\"query\":\"lesson\"}",
            status: InvocationStatus::Succeeded,
            output: "lesson recalled",
            error: "",
        },
    )
    .expect("record successful invocation");

    let records = list_recent(&connection, 10).expect("list invocation audit");

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].invocation_id, success_id);
    assert_eq!(records[0].status, InvocationStatus::Succeeded);
    assert_eq!(records[1].invocation_id, pending_id);
    assert_eq!(records[1].status, InvocationStatus::PendingApproval);
}
