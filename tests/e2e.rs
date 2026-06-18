mod harness;

use harness::{run_cli, unused_port, wait_for_cli_ping, DaemonProcess};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn daemon_stays_running_when_started_on_requested_port() {
    let mut daemon = DaemonProcess::start(unused_port());

    sleep(Duration::from_millis(500)).await;

    daemon.assert_running();
}

#[tokio::test]
async fn cli_ping_reports_ok_when_daemon_is_reachable() {
    let port = unused_port();
    let mut daemon = DaemonProcess::start(port);
    let host = format!("http://127.0.0.1:{port}");

    let output = wait_for_cli_ping(&host).await;
    daemon.assert_running();

    assert!(
        output.contains("daemon: OK"),
        "expected successful ping output, got: {output}"
    );
}

#[tokio::test]
async fn cli_status_reports_operator_counts() {
    let port = unused_port();
    let mut daemon = DaemonProcess::start_with_args(port, &["--dream-now"]);
    let host = format!("http://127.0.0.1:{port}");

    let _ = wait_for_cli_ping(&host).await;
    let status = run_cli(&["--host", &host, "status"]);
    daemon.assert_running();

    assert!(status.contains("daemon: OK"), "status output: {status}");
    assert!(status.contains("agents: 4"), "status output: {status}");
    assert!(status.contains("workflows: 0"), "status output: {status}");
    assert!(status.contains("memories:"), "status output: {status}");
    assert!(status.contains("dreams:"), "status output: {status}");
}

#[tokio::test]
async fn cli_doctor_reports_runtime_health() {
    let port = unused_port();
    let mut daemon = DaemonProcess::start(port);
    let host = format!("http://127.0.0.1:{port}");

    let _ = wait_for_cli_ping(&host).await;
    let report = run_cli(&["--no-auto-start", "--host", &host, "doctor"]);
    daemon.assert_running();

    assert!(report.contains("[pass] cli:"), "doctor output: {report}");
    assert!(
        report.contains(&format!("[pass] endpoint: local port {port}")),
        "doctor output: {report}"
    );
    assert!(
        report.contains("[pass] daemon binary:"),
        "doctor output: {report}"
    );
    assert!(
        report.contains("[pass] daemon: OK"),
        "doctor output: {report}"
    );
    assert!(report.contains("summary:"), "doctor output: {report}");
}

#[tokio::test]
async fn cli_lists_governed_tools_and_skills() {
    let port = unused_port();
    let mut daemon = DaemonProcess::start(port);
    let host = format!("http://127.0.0.1:{port}");

    let _ = wait_for_cli_ping(&host).await;
    let tools = run_cli(&["--host", &host, "tools", "list"]);
    let skills = run_cli(&["--host", &host, "skills", "list"]);
    daemon.assert_running();

    assert!(tools.contains("fs.read"), "tools output: {tools}");
    assert!(
        tools.contains("process.exec")
            && tools.contains("enabled=false")
            && tools.contains("approval=true"),
        "tools output: {tools}"
    );
    assert!(
        skills.contains("codebase-research") && skills.contains("fs.search,fs.read,memory.recall"),
        "skills output: {skills}"
    );
}

#[tokio::test]
async fn cli_invokes_tools_with_policy_and_audit() {
    let port = unused_port();
    let mut daemon = DaemonProcess::start(port);
    let host = format!("http://127.0.0.1:{port}");
    let content = "policy invocation memory";

    let _ = wait_for_cli_ping(&host).await;
    let pending = run_cli(&[
        "--host",
        &host,
        "tools",
        "invoke",
        "memory.store",
        "--arg",
        &format!("content={content}"),
    ]);
    assert!(pending.contains("pending_approval"), "pending: {pending}");

    let stored = run_cli(&[
        "--host",
        &host,
        "tools",
        "invoke",
        "memory.store",
        "--approve",
        "--arg",
        &format!("content={content}"),
    ]);
    assert!(stored.contains("succeeded"), "stored: {stored}");

    let recalled = run_cli(&[
        "--host",
        &host,
        "tools",
        "invoke",
        "memory.recall",
        "--arg",
        "query=policy invocation",
    ]);
    assert!(recalled.contains(content), "recalled: {recalled}");

    let denied = run_cli(&[
        "--host",
        &host,
        "tools",
        "invoke",
        "process.exec",
        "--approve",
    ]);
    assert!(denied.contains("denied"), "denied: {denied}");

    let history = run_cli(&["--host", &host, "tools", "history"]);
    daemon.assert_running();
    assert!(history.contains("memory.store"), "history: {history}");
    assert!(history.contains("process.exec"), "history: {history}");
}

#[tokio::test]
async fn cli_memory_commands_store_and_recall_entries() {
    let port = unused_port();
    let mut daemon = DaemonProcess::start(port);
    let host = format!("http://127.0.0.1:{port}");

    let _ = wait_for_cli_ping(&host).await;
    let store = run_cli(&[
        "--host",
        &host,
        "memory",
        "remember",
        "operator memory smoke",
    ]);
    daemon.assert_running();
    assert!(store.contains("remembered"), "store output: {store}");

    let recall = run_cli(&["--host", &host, "memory", "recall", "operator"]);
    assert!(
        recall.contains("operator memory smoke"),
        "recall output: {recall}"
    );
}

#[tokio::test]
async fn daemon_dream_now_writes_nightly_memory_file() {
    let port = unused_port();
    let mut daemon = DaemonProcess::start_with_args(port, &["--dream-now"]);
    let host = format!("http://127.0.0.1:{port}");

    let _ = wait_for_cli_ping(&host).await;
    daemon.assert_running();

    let dream_dir = daemon.workdir().join(".steward/memory/nightly");
    let entries = std::fs::read_dir(&dream_dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", dream_dir.display()))
        .collect::<Result<Vec<_>, _>>()
        .expect("failed to collect dream files");
    assert!(
        entries.iter().any(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "md")
        }),
        "expected markdown dream file in {}",
        dream_dir.display()
    );
}
