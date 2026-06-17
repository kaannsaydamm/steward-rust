mod harness;

use harness::{run_cli, unused_port, wait_for_cli_ping, DaemonProcess};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn workflow_list_survives_daemon_restart() {
    let workdir = tempfile::tempdir().expect("failed to create shared daemon workdir");
    let first_port = unused_port();
    let first_host = format!("http://127.0.0.1:{first_port}");
    let daemon = DaemonProcess::start_in_workdir(first_port, workdir, &[]);

    let _ = wait_for_cli_ping(&first_host).await;
    let start = run_cli(&[
        "--host",
        &first_host,
        "workflow",
        "start",
        "--description",
        "restart durability smoke",
        "persisted workflow",
    ]);
    let workflow_id = start
        .lines()
        .find_map(|line| line.split_once('\t').map(|(id, _)| id.to_owned()))
        .expect("workflow start output did not include an id");
    let workdir = daemon.into_workdir();

    let second_port = unused_port();
    let second_host = format!("http://127.0.0.1:{second_port}");
    let mut daemon = DaemonProcess::start_in_workdir(second_port, workdir, &[]);
    let _ = wait_for_cli_ping(&second_host).await;

    let list = run_cli(&["--host", &second_host, "workflow", "list"]);
    daemon.assert_running();

    assert!(
        list.contains(&workflow_id),
        "expected persisted workflow id {workflow_id}; list output: {list}"
    );
    assert!(
        list.contains("persisted workflow"),
        "expected persisted workflow title; list output: {list}"
    );
}

#[tokio::test]
async fn workflow_awaiting_approval_resumes_after_restart() {
    let workdir = tempfile::tempdir().expect("failed to create shared daemon workdir");
    let first_port = unused_port();
    let first_host = format!("http://127.0.0.1:{first_port}");
    let daemon = DaemonProcess::start_in_workdir(first_port, workdir, &[]);

    let _ = wait_for_cli_ping(&first_host).await;
    let start = run_cli(&[
        "--host",
        &first_host,
        "workflow",
        "start",
        "--description",
        "restart resume smoke",
        "resumable workflow",
    ]);
    let workflow_id = workflow_id_from_start(&start);
    let workdir = daemon.into_workdir();

    let second_port = unused_port();
    let second_host = format!("http://127.0.0.1:{second_port}");
    let mut daemon = DaemonProcess::start_in_workdir(second_port, workdir, &[]);
    let _ = wait_for_cli_ping(&second_host).await;

    let approval = run_cli(&["--host", &second_host, "workflow", "approve", &workflow_id]);
    assert!(approval.contains("approved"), "approval output: {approval}");

    let mut last_list = String::new();
    for _ in 0..20 {
        last_list = run_cli(&["--host", &second_host, "workflow", "list"]);
        if last_list.contains(&workflow_id)
            && last_list.contains("phase=10")
            && last_list.contains("mode=2")
        {
            daemon.assert_running();
            return;
        }
        sleep(Duration::from_millis(500)).await;
    }

    panic!("workflow did not resume to completion; last list output: {last_list}");
}

fn workflow_id_from_start(start: &str) -> String {
    start
        .lines()
        .find_map(|line| line.split_once('\t').map(|(id, _)| id.to_owned()))
        .expect("workflow start output did not include an id")
}

#[tokio::test]
async fn workflow_continues_after_start_stream_disconnects() {
    let port = unused_port();
    let mut daemon = DaemonProcess::start(port);
    let host = format!("http://127.0.0.1:{port}");

    let _ = wait_for_cli_ping(&host).await;
    let start = run_cli(&[
        "--host",
        &host,
        "workflow",
        "start",
        "--description",
        "stream disconnect smoke",
        "stream resilient workflow",
    ]);
    let workflow_id = workflow_id_from_start(&start);
    let approval = run_cli(&["--host", &host, "workflow", "approve", &workflow_id]);
    assert!(approval.contains("approved"), "approval output: {approval}");

    let mut last_status = String::new();
    for _ in 0..20 {
        last_status = run_cli(&["--host", &host, "workflow", "status", &workflow_id]);
        if last_status.contains("phase\t10") && last_status.contains("progress\t100%") {
            daemon.assert_running();
            return;
        }
        sleep(Duration::from_millis(500)).await;
    }

    panic!("workflow did not continue after stream disconnect; last status: {last_status}");
}
