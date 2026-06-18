use super::{list, register, start};
use crate::mcp_registry::{self, AdapterConfig, DiscoveredTool};
use crate::mcp_runtime::AdapterState;
use crate::tool_registry;
use crate::MySteward;

fn missing_config() -> AdapterConfig {
    AdapterConfig {
        id: "broken".to_owned(),
        name: "Broken MCP".to_owned(),
        command: "definitely-missing-steward-mcp-server".to_owned(),
        args: Vec::new(),
        cwd: None,
    }
}

fn steward() -> (tempfile::TempDir, MySteward) {
    let temp = tempfile::tempdir().expect("create temp directory");
    let database = temp.path().join("steward.db");
    let steward =
        MySteward::new(database.to_str().expect("database path")).expect("create steward");
    (temp, steward)
}

#[tokio::test]
async fn reregistering_failed_adapter_clears_runtime_error() {
    let (_temp, steward) = steward();
    let config = missing_config();
    register(&steward, config.clone())
        .await
        .expect("register adapter");
    start(&steward, &config.id)
        .await
        .expect_err("start must fail");

    register(&steward, config)
        .await
        .expect("register adapter again");
    let adapters = list(&steward).await.expect("list adapters");

    assert_eq!(adapters[0].status.state, AdapterState::Stopped);
}

#[tokio::test]
async fn failed_start_disables_previously_discovered_tools() {
    let (_temp, steward) = steward();
    let config = missing_config();
    register(&steward, config.clone())
        .await
        .expect("register adapter");
    {
        let connection = steward.db.lock().expect("lock database");
        mcp_registry::replace_tools(
            &connection,
            &config.id,
            &[DiscoveredTool {
                name: "old-tool".to_owned(),
                description: "Previously discovered tool".to_owned(),
                input_schema_json: r#"{"type":"object"}"#.to_owned(),
            }],
        )
        .expect("seed discovered tool");
    }

    start(&steward, &config.id)
        .await
        .expect_err("start must fail");
    let tools = {
        let connection = steward.db.lock().expect("lock database");
        tool_registry::list_tools(&connection).expect("list tools")
    };

    let old_tool = tools
        .into_iter()
        .find(|tool| tool.id.starts_with("mcp.broken."))
        .expect("old MCP tool");
    assert!(!old_tool.enabled);
}
