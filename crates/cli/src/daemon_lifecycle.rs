use crate::client;
use anyhow::{Context as _, Result};
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tokio::time::{sleep, timeout, Duration};

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub async fn ensure_running(host: &str, auto_start: bool, web_port: u16) -> Result<()> {
    if ping_with_timeout(host).await.is_ok() {
        return Ok(());
    }
    if !auto_start {
        return Ok(());
    }
    let Some(port) = local_port(host) else {
        return Ok(());
    };

    let log_path = spawn_daemon(port, web_port)
        .with_context(|| format!("starting local steward daemon on port {port}"))?;
    wait_until_ready(host)
        .await
        .with_context(|| format!("daemon log: {}", log_path.display()))
}

fn spawn_daemon(port: u16, web_port: u16) -> Result<PathBuf> {
    let root = steward_core::storage::root().context("resolving ~/.steward data root")?;
    let log_dir = root.join("logs");
    fs::create_dir_all(&log_dir)
        .with_context(|| format!("creating daemon log directory {}", log_dir.display()))?;
    let log_path = log_dir.join("daemon.log");
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("opening daemon log {}", log_path.display()))?;
    let stderr = stdout.try_clone()?;
    let mut command = Command::new(daemon_path());
    command
        .arg("--port")
        .arg(port.to_string())
        .arg("--web-port")
        .arg(web_port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    apply_hidden_window(&mut command);
    command.spawn()?;
    Ok(log_path)
}

pub async fn ensure_web_ready(port: u16) -> Result<()> {
    for _ in 0..30 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            return Ok(());
        }
        sleep(Duration::from_millis(150)).await;
    }
    anyhow::bail!("Web UI did not become ready at {}", web_url(port))
}

pub(crate) fn web_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

async fn wait_until_ready(host: &str) -> Result<()> {
    let mut last_error = None;
    for _ in 0..30 {
        match ping_with_timeout(host).await {
            Ok(_) => return Ok(()),
            Err(error) => last_error = Some(error),
        }
        sleep(Duration::from_millis(150)).await;
    }
    if let Some(error) = last_error {
        Err(error).context("local daemon did not become ready")
    } else {
        anyhow::bail!("local daemon did not become ready")
    }
}

async fn ping_with_timeout(host: &str) -> Result<String> {
    timeout(Duration::from_millis(500), client::ping(host))
        .await
        .context("timed out while checking daemon readiness")?
}

pub(crate) fn daemon_path() -> PathBuf {
    let exe_name = if cfg!(windows) {
        "steward-daemon.exe"
    } else {
        "steward-daemon"
    };
    if let Ok(current) = std::env::current_exe() {
        if let Some(parent) = current.parent() {
            return parent.join(exe_name);
        }
    }
    PathBuf::from(exe_name)
}

fn local_port(host: &str) -> Option<u16> {
    let host = host.strip_prefix("http://")?;
    let host = host.strip_suffix('/').unwrap_or(host);
    let (name, port) = host.rsplit_once(':')?;
    if matches!(name, "127.0.0.1" | "localhost" | "[::1]") {
        port.parse().ok()
    } else {
        None
    }
}

#[cfg(windows)]
fn apply_hidden_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn apply_hidden_window(_command: &mut Command) {}

#[cfg(test)]
#[path = "daemon_lifecycle_tests.rs"]
mod tests;
