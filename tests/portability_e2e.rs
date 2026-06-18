mod harness;

use harness::{run_cli, run_cli_with_home, unused_port, wait_for_cli_ping, DaemonProcess};

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
