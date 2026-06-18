use super::{AdapterConfig, DiscoveredTool};
use crate::tool_registry::ToolRuntime;
use crate::{mcp_registry, tool_registry};
use rusqlite::Connection;

fn config() -> AdapterConfig {
    AdapterConfig {
        id: "fixture".to_owned(),
        name: "Fixture MCP".to_owned(),
        command: "fixture-server".to_owned(),
        args: vec!["--stdio".to_owned()],
        cwd: Some("C:/workspace".to_owned()),
    }
}

#[test]
fn registered_adapter_configuration_survives_reload() {
    let connection = Connection::open_in_memory().expect("open database");
    tool_registry::initialize(&connection).expect("initialize registry");

    mcp_registry::register(&connection, &config()).expect("register adapter");
    let adapters = mcp_registry::list(&connection).expect("list adapters");

    assert_eq!(adapters, vec![config()]);
}

#[test]
fn removing_adapter_deletes_only_its_discovered_tools() {
    let connection = Connection::open_in_memory().expect("open database");
    tool_registry::initialize(&connection).expect("initialize registry");
    mcp_registry::register(&connection, &config()).expect("register adapter");
    let discovered = vec![DiscoveredTool {
        name: "echo".to_owned(),
        description: "Echo input".to_owned(),
        input_schema_json: r#"{"type":"object"}"#.to_owned(),
    }];
    let tool_ids = mcp_registry::replace_tools(&connection, "fixture", &discovered)
        .expect("store discovered tools");

    let removed = mcp_registry::remove(&connection, "fixture").expect("remove adapter");
    let tools = tool_registry::list_tools(&connection).expect("list tools");

    assert!(removed);
    assert_eq!(tool_ids.len(), 1);
    assert!(tools.iter().all(|tool| tool.id != tool_ids[0]));
    assert!(tools.iter().any(|tool| tool.id == "memory.recall"));
}

#[test]
fn adapters_can_publish_tools_with_the_same_remote_name() {
    let connection = Connection::open_in_memory().expect("open database");
    tool_registry::initialize(&connection).expect("initialize registry");
    let first = config();
    let mut second = config();
    second.id = "fixture-two".to_owned();
    second.name = "Fixture MCP Two".to_owned();
    mcp_registry::register(&connection, &first).expect("register first adapter");
    mcp_registry::register(&connection, &second).expect("register second adapter");
    let tools = [DiscoveredTool {
        name: "echo".to_owned(),
        description: "Echo input".to_owned(),
        input_schema_json: r#"{"type":"object"}"#.to_owned(),
    }];

    let first_ids =
        mcp_registry::replace_tools(&connection, &first.id, &tools).expect("store first tools");
    let second_ids =
        mcp_registry::replace_tools(&connection, &second.id, &tools).expect("store second tools");

    assert_ne!(first_ids, second_ids);
    assert_eq!(
        tool_registry::list_tools(&connection)
            .expect("list tools")
            .into_iter()
            .filter(|tool| tool.runtime == ToolRuntime::Mcp)
            .count(),
        2
    );
}
