mod harness;

use harness::{run_cli, run_cli_with_home, unused_port, wait_for_cli_ping, DaemonProcess};
use std::time::Duration;
use tokio::time::sleep;

#[derive(serde::Serialize)]
struct TestSkillManifest {
    skill_id: String,
    name: String,
    description: String,
    version: String,
    tool_ids: Vec<String>,
}

#[derive(serde::Serialize)]
struct TestSkillBundle {
    manifest_json: String,
    publisher_key: String,
    signature: String,
}

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
async fn cli_installs_self_signed_skill_bundle() {
    use ed25519_dalek::{Signer as _, SigningKey};

    let port = unused_port();
    let mut daemon = DaemonProcess::start(port);
    let host = format!("http://127.0.0.1:{port}");
    let fixture_dir = tempfile::tempdir().expect("create fixture directory");
    let bundle_path = fixture_dir.path().join("portable.skill.json");
    let manifest_json = serde_json::to_string(&TestSkillManifest {
        skill_id: "portable-research".to_owned(),
        name: "Portable research".to_owned(),
        description: "Self-signed portable research skill".to_owned(),
        version: "1.0.0".to_owned(),
        tool_ids: vec!["memory.recall".to_owned()],
    })
    .expect("serialize manifest");
    let signing_key = SigningKey::from_bytes(&[9; 32]);
    let bundle = TestSkillBundle {
        signature: hex::encode(signing_key.sign(manifest_json.as_bytes()).to_bytes()),
        publisher_key: hex::encode(signing_key.verifying_key().as_bytes()),
        manifest_json,
    };
    std::fs::write(
        &bundle_path,
        serde_json::to_vec(&bundle).expect("serialize bundle"),
    )
    .expect("write bundle");

    let _ = wait_for_cli_ping(&host).await;
    let installed = run_cli(&[
        "--host",
        &host,
        "skills",
        "install",
        bundle_path.to_str().expect("bundle path"),
    ]);
    let skills = run_cli(&["--host", &host, "skills", "list"]);
    daemon.assert_running();

    assert!(
        installed.contains("installed\tportable-research"),
        "install output: {installed}"
    );
    assert!(
        skills.contains("portable-research") && skills.contains("signed=true"),
        "skills output: {skills}"
    );
}

#[tokio::test]
async fn cli_exports_and_imports_complete_steward_home() {
    let port = unused_port();
    let daemon = DaemonProcess::start(port);
    let host = format!("http://127.0.0.1:{port}");
    let home = daemon.workdir().join(".steward");
    let archive = daemon.workdir().join("portable.steward.zip");
    let _ = wait_for_cli_ping(&host).await;
    let remembered = run_cli(&[
        "--host",
        &host,
        "memory",
        "remember",
        "portable session memory",
    ]);
    assert!(remembered.contains("remembered"), "remember: {remembered}");
    let archive_text = archive.to_str().expect("archive path");
    let exported = run_cli_with_home(
        &["--host", &host, "data", "export", archive_text],
        Some(&home),
    );
    assert!(exported.contains("exported"), "export: {exported}");

    let workdir = daemon.into_workdir();
    std::fs::remove_dir_all(&home).expect("remove original Steward home");
    let imported = run_cli_with_home(
        &["--host", &host, "data", "import", archive_text],
        Some(&home),
    );
    assert!(imported.contains("imported"), "import: {imported}");

    let mut restored_daemon = DaemonProcess::start_in_workdir(port, workdir, &[]);
    let _ = wait_for_cli_ping(&host).await;
    let recalled = run_cli(&["--host", &host, "memory", "recall", "portable session"]);
    restored_daemon.assert_running();
    assert!(
        recalled.contains("portable session memory"),
        "recall after import: {recalled}"
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
