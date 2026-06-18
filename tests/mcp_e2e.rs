mod harness;

use harness::{
    binary_path, run_cli, run_cli_with_status, unused_port, wait_for_cli_ping, DaemonProcess,
};

#[tokio::test]
async fn cli_manages_mcp_lifecycle_and_invokes_discovered_tool() {
    let port = unused_port();
    let mut daemon = DaemonProcess::start(port);
    let host = format!("http://127.0.0.1:{port}");
    let fixture = binary_path("steward-mcp-fixture");
    let fixture = fixture.to_str().expect("fixture path");
    let _ = wait_for_cli_ping(&host).await;
    let added = run_cli(&[
        "--host",
        &host,
        "mcp",
        "add",
        "fixture",
        "--name",
        "Fixture MCP",
        "--",
        fixture,
    ]);
    assert!(added.contains("registered\tfixture"), "add: {added}");
    let started = run_cli(&["--host", &host, "mcp", "start", "fixture"]);
    assert!(
        started.contains("running") && started.contains("tools=1"),
        "start: {started}"
    );
    let tools = run_cli(&["--host", &host, "tools", "list"]);
    let tool_id = tools
        .lines()
        .find_map(|line| {
            line.starts_with("mcp.fixture.")
                .then(|| line.split('\t').next())
        })
        .flatten()
        .expect("discovered MCP tool");
    let invoked = run_cli(&[
        "--host",
        &host,
        "tools",
        "invoke",
        tool_id,
        "--approve",
        "--arg",
        "text=hello-mcp",
    ]);
    assert!(invoked.contains("hello-mcp"), "invoke: {invoked}");
    let stopped = run_cli(&["--host", &host, "mcp", "stop", "fixture"]);
    assert!(stopped.contains("stopped"), "stop: {stopped}");
    daemon.assert_running();
}

#[tokio::test]
async fn mcp_configuration_survives_daemon_restart_as_stopped() {
    let port = unused_port();
    let daemon = DaemonProcess::start(port);
    let host = format!("http://127.0.0.1:{port}");
    let fixture = binary_path("steward-mcp-fixture");
    let fixture = fixture.to_str().expect("fixture path");
    let _ = wait_for_cli_ping(&host).await;
    let _ = run_cli(&[
        "--host",
        &host,
        "mcp",
        "add",
        "fixture",
        "--name",
        "Fixture MCP",
        "--",
        fixture,
    ]);

    let workdir = daemon.into_workdir();
    let mut restarted = DaemonProcess::start_in_workdir(port, workdir, &[]);
    let _ = wait_for_cli_ping(&host).await;
    let adapters = run_cli(&["--host", &host, "mcp", "list"]);
    restarted.assert_running();

    assert!(
        adapters.contains("fixture\tstatus=stopped"),
        "adapters after restart: {adapters}"
    );
}

#[tokio::test]
async fn failed_mcp_start_is_reported_without_enabling_tools() {
    let port = unused_port();
    let mut daemon = DaemonProcess::start(port);
    let host = format!("http://127.0.0.1:{port}");
    let _ = wait_for_cli_ping(&host).await;
    let _ = run_cli(&[
        "--host",
        &host,
        "mcp",
        "add",
        "broken",
        "--name",
        "Broken MCP",
        "--",
        "definitely-missing-steward-mcp-server",
    ]);

    let failure = run_cli_with_status(&["--host", &host, "mcp", "start", "broken"], None, false);
    let adapters = run_cli(&["--host", &host, "mcp", "list"]);
    let tools = run_cli(&["--host", &host, "tools", "list"]);
    daemon.assert_running();

    assert!(
        failure.contains("starting MCP adapter 'broken'"),
        "failure: {failure}"
    );
    assert!(
        adapters.contains("broken\tstatus=failed"),
        "adapters: {adapters}"
    );
    assert!(!tools.contains("mcp.broken."), "tools: {tools}");
}
