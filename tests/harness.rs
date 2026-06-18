use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Once;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

static BUILD_BINARIES: Once = Once::new();

pub struct DaemonProcess {
    child: Child,
    workdir: TempDir,
}

impl DaemonProcess {
    #[allow(dead_code)]
    pub fn start(port: u16) -> Self {
        Self::start_with_args(port, &[])
    }

    #[allow(dead_code)]
    pub fn start_with_args(port: u16, extra_args: &[&str]) -> Self {
        let workdir = tempfile::tempdir().expect("failed to create daemon temp directory");
        Self::start_in_workdir(port, workdir, extra_args)
    }

    pub fn start_in_workdir(port: u16, workdir: TempDir, extra_args: &[&str]) -> Self {
        ensure_binaries_built();
        let mut command = Command::new(daemon_path());
        command
            .arg("--port")
            .arg(port.to_string())
            .args(extra_args)
            .env("STEWARD_HOME", workdir.path().join(".steward"))
            .current_dir(workdir.path())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = command.spawn().expect("failed to start daemon");
        Self { child, workdir }
    }

    #[allow(dead_code)]
    pub fn into_workdir(mut self) -> TempDir {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let replacement = tempfile::tempdir().expect("failed to create replacement tempdir");
        std::mem::replace(&mut self.workdir, replacement)
    }

    #[allow(dead_code)]
    pub fn workdir(&self) -> &Path {
        self.workdir.path()
    }

    pub fn assert_running(&mut self) {
        if let Ok(Some(status)) = self.child.try_wait() {
            panic!(
                "daemon exited early with status: {status}\n{}",
                self.drain_output()
            );
        }
    }

    fn drain_output(&mut self) -> String {
        let mut output = String::new();
        if let Some(stdout) = &mut self.child.stdout {
            let mut stdout_text = String::new();
            let _ = stdout.read_to_string(&mut stdout_text);
            if !stdout_text.is_empty() {
                output.push_str("stdout:\n");
                output.push_str(&stdout_text);
            }
        }
        if let Some(stderr) = &mut self.child.stderr {
            let mut stderr_text = String::new();
            let _ = stderr.read_to_string(&mut stderr_text);
            if !stderr_text.is_empty() {
                output.push_str("stderr:\n");
                output.push_str(&stderr_text);
            }
        }
        output
    }
}

impl Drop for DaemonProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub async fn wait_for_cli_ping(host: &str) -> String {
    ensure_binaries_built();
    let mut last_output = String::new();
    for _ in 0..20 {
        let output = Command::new(cli_path())
            .arg("--host")
            .arg(host)
            .arg("ping")
            .output()
            .expect("failed to execute CLI");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        last_output = format!("{stdout}{stderr}");
        if output.status.success() && last_output.contains("daemon: OK") {
            return last_output;
        }
        sleep(Duration::from_millis(150)).await;
    }
    last_output
}

pub fn run_cli(args: &[&str]) -> String {
    run_cli_with_home(args, None)
}

pub fn run_cli_with_home(args: &[&str], steward_home: Option<&Path>) -> String {
    run_cli_with_status(args, steward_home, true)
}

pub fn run_cli_with_status(
    args: &[&str],
    steward_home: Option<&Path>,
    expect_success: bool,
) -> String {
    let mut command = Command::new(cli_path());
    command.args(args);
    if let Some(home) = steward_home {
        command.env("STEWARD_HOME", home);
    }
    let output = command.output().expect("failed to execute CLI");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    assert_eq!(
        output.status.success(),
        expect_success,
        "unexpected CLI status: {combined}"
    );
    combined
}

pub fn unused_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind ephemeral port");
    listener
        .local_addr()
        .expect("failed to read ephemeral port")
        .port()
}

fn daemon_path() -> PathBuf {
    binary_path("steward-daemon")
}

fn cli_path() -> PathBuf {
    binary_path("steward-cli")
}

pub fn binary_path(name: &str) -> PathBuf {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/debug")
        .join(exe)
}

fn ensure_binaries_built() {
    BUILD_BINARIES.call_once(|| {
        let status = Command::new("cargo")
            .args(["build", "-p", "steward-cli", "-p", "steward-daemon"])
            .current_dir(workspace_dir())
            .status()
            .expect("failed to build steward binaries");
        assert!(status.success(), "failed to build steward binaries");
        let fixture_status = Command::new("cargo")
            .args([
                "build",
                "-p",
                "steward-e2e-tests",
                "--bin",
                "steward-mcp-fixture",
            ])
            .current_dir(workspace_dir())
            .status()
            .expect("failed to build MCP fixture");
        assert!(fixture_status.success(), "failed to build MCP fixture");
    });
}

fn workspace_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}
