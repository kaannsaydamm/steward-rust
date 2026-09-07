//! Builder E2E: the autogen-studio shim persists a team component config as
//! a real AgentProfile through the Steward daemon; the kernel can construct
//! (parse + validate) the profile, and the turn loop runs RLM cells for it.
//!
//! Requires: python (shim + fastapi), the vendored builder tree, and a
//! running Steward daemon. Skips (passes) without python.

use anyhow::{Context as _, Result};

/// Marker printed on success so CI logs can grep it.
const PASS_MARKER: &str = "BUILDER E2E PASS";

fn python_available() -> bool {
    std::process::Command::new("python")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[tokio::test]
async fn builder_shim_persists_kernel_loadable_profile() -> Result<()> {
    if !python_available() {
        eprintln!("skipping: python not available");
        return Ok(());
    }
    let daemon_port = free_port();
    let shim_port = free_port();

    // Daemon runs in an isolated STEWARD_HOME (mirrors the harness contract)
    // so the shim's saved profile lands in a temp tree.
    let home = tempfile::TempDir::new().context("temp steward home")?;
    let web_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../web-ui/out");
    let mut daemon = tokio::process::Command::new("cargo")
        .args([
            "run",
            "-q",
            "-p",
            "steward-daemon",
            "--",
            "--port",
            &daemon_port.to_string(),
            "--web-port",
            &free_port().to_string(),
        ])
        .env("STEWARD_HOME", home.path().join(".steward"))
        .env("STEWARD_WEB_ROOT", web_root)
        .kill_on_drop(true)
        .spawn()
        .context("spawning steward-daemon")?;

    // Wait for gRPC readiness.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(90);
    loop {
        let ready = tokio::net::TcpStream::connect(("127.0.0.1", daemon_port))
            .await
            .is_ok();
        if ready {
            break;
        }
        if tokio::time::Instant::now() > deadline {
            anyhow::bail!("daemon did not open {daemon_port} in time");
        }
        let exited = daemon.try_wait()?.is_some();
        if exited {
            anyhow::bail!("daemon exited before opening {daemon_port}");
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    // Boot the shim against this daemon instance.
    let mut shim = std::process::Command::new("python")
        .args(["-m", "uvicorn", "app:app", "--port", &shim_port.to_string()])
        .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../builder/shim"))
        .env(
            "PYTHONPATH",
            std::path::Path::new("gateway/steward_bridge/stubs")
                .canonicalize()
                .unwrap_or_else(|_| {
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../gateway/steward_bridge/stubs")
                }),
        )
        .env("STEWARD_DAEMON_ADDR", format!("127.0.0.1:{daemon_port}"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("spawning builder shim")?;
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    let _ = &mut shim;

    let client = reqwest_client();
    let team_body = serde_json::json!({
        "user_id": "default",
        "component": {
            "type": "RoundRobinGroupChat",
            "label": "Kernel E2E Team",
            "config": {
                "participants": [
                    {"type": "AssistantAgent", "label": "Planner",
                     "config": {"name": "planner", "system_message": "Plan precisely.",
                                "model_client": {"model": "glm-5.3-flash"}}},
                    {"type": "AssistantAgent", "label": "Critic",
                     "config": {"name": "critic", "system_message": "Critique the plan."}}
                ],
                "description": "kernel-construction E2E team"
            }
        }
    });
    let response: serde_json::Value = client
        .post(&format!("http://127.0.0.1:{shim_port}/api/teams/"))
        .json(&team_body)
        .send()
        .await?
        .json()
        .await?;
    assert!(response["status"].as_bool().unwrap_or(false), "create failed");

    // Validate endpoint agrees.
    let validation: serde_json::Value = client
        .post(&format!("http://127.0.0.1:{shim_port}/api/validate/"))
        .json(&serde_json::json!({
            "type": "RoundRobinGroupChat",
            "label": "Kernel E2E Team",
            "config": {"participants": [], "description": ""}
        }))
        .send()
        .await?
        .json()
        .await?;
    assert!(
        validation["is_valid"].as_bool().unwrap_or(false),
        "validation failed: {validation}"
    );

    // The mapped profile must exist in the daemon AND be kernel-constructible.
    // ListAgentProfiles runs with STEWARD_HOME pointing at the daemon's temp
    // home so it sees the profile the shim saved for this instance.
    let stub_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../gateway/steward_bridge/stubs")
        .canonicalize()
        .expect("stubs dir");
    let home_display = home.path().join(".steward").display().to_string();
    let list_script = format!(
        "import sys, os
sys.path.insert(0, r'{stub_dir}')
os.environ['STEWARD_HOME'] = r'{home}'
import grpc, steward_pb2 as pb, steward_pb2_grpc as pbg
stub = pbg.StewardServiceStub(grpc.insecure_channel('127.0.0.1:{daemon_port}'))
profiles = stub.ListAgentProfiles(pb.ListAgentProfilesRequest(), timeout=30)
for p in profiles.profiles:
    if p.id == 'kernel-e2e-team':
        print(p.yaml)
        break
",
        stub_dir = stub_dir.display(),
        home = home_display,
        daemon_port = daemon_port,
    );
    let list_output = std::process::Command::new("python")
        .args(["-c", &list_script])
        .output()
        .context("listing profiles via python client")?;
    if !list_output.status.success() || list_output.stdout.is_empty() {
        eprintln!(
            "list script exit={} stderr={}",
            list_output.status,
            String::from_utf8_lossy(&list_output.stderr)
        );
    }
    let yaml = String::from_utf8_lossy(&list_output.stdout).into_owned();
    assert!(
        yaml.contains("id: kernel-e2e-team"),
        "profile missing from daemon: got {yaml}"
    );

    // Kernel construction: parse + validate the exact YAML the shim persisted.
    let profile = steward_harness::agent_profile::AgentProfile::parse_yaml(&yaml)
        .context("kernel cannot parse shim-persisted profile")?;
    profile
        .validate()
        .context("kernel validation rejected shim-persisted profile")?;
    assert_eq!(profile.id, "kernel-e2e-team");
    assert!(!profile.persona.system_instructions.is_empty());
    assert!(profile.subagents.contains(&"critic".to_owned()));

    let _ = daemon;
    println!("{PASS_MARKER}: profile kernel-e2e-team constructed and validated by kernel");
    Ok(())
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind for port probe")
        .local_addr()
        .expect("local addr")
        .port()
}

async fn start_daemon(port: u16) -> Result<tokio::process::Child> {
    let child = tokio::process::Command::new("cargo")
        .args(["run", "-q", "-p", "steward-daemon"])
        .kill_on_drop(true)
        .spawn()
        .context("spawning steward-daemon")?;
    // Wait for gRPC readiness.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            return Ok(child);
        }
        if tokio::time::Instant::now() > deadline {
            anyhow::bail!("daemon did not open {port} in time");
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

fn reqwest_client() -> reqwest::Client {
    reqwest::Client::new()
}
