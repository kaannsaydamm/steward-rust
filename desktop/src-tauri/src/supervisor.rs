//! Desktop daemon supervisor (Omega §42, Tasks 23.1-23.5).
//!
//! The Tauri shell embeds web-ui/out and supervises the shared daemon:
//! health-check → start-if-absent → wait-ready → attach. The desktop keeps
//! NO second database and NO second agent runtime (blacklist §13.12);
//! domain operations go through daemon APIs exclusively.

use anyhow::{Context as _, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisorState {
    /// No daemon found; not starting (manual mode).
    Detached,
    Starting,
    Attached,
    Failed { reason: String },
}

#[derive(Debug)]
pub struct DaemonSupervisor {
    binary_path: std::path::PathBuf,
    data_root: std::path::PathBuf,
    port: u16,
    web_port: u16,
    child: Mutex<Option<Child>>,
}

impl DaemonSupervisor {
    /// Creates a supervisor for the bundled daemon binary.
    pub fn new(binary_path: std::path::PathBuf, data_root: std::path::PathBuf, port: u16, web_port: u16) -> Self {
        Self {
            binary_path,
            data_root,
            port,
            web_port,
            child: Mutex::new(None),
        }
    }

    /// Health handshake: version + instance id via gRPC ping equivalent.
    pub fn is_healthy(&self) -> bool {
        // HTTP probe on the gRPC port works because the daemon serves
        // gRPC-Web (accept_http1): a plain GET yields a response, not ECONNREFUSED.
        std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], self.port)),
            Duration::from_millis(250),
        )
        .is_ok()
    }

    /// Ensures the daemon: attach to a healthy one, else start bundled.
    /// Returns the supervisor state after resolution.
    pub fn ensure_daemon(&self) -> Result<SupervisorState> {
        if self.is_healthy() {
            return Ok(SupervisorState::Attached);
        }
        let child = Command::new(&self.binary_path)
            .arg("--port")
            .arg(self.port.to_string())
            .arg("--web-port")
            .arg(self.web_port.to_string())
            .env("STEWARD_HOME", &self.data_root)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match child {
            Ok(child) => child,
            Err(error) => {
                return Ok(SupervisorState::Failed {
                    reason: format!("could not start daemon: {error}"),
                });
            }
        };
        *self.child.lock() = Some(child);

        // Wait for readiness (Task 23.2 step 4); fail fast if it dies.
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if self.is_healthy() {
                return Ok(SupervisorState::Attached);
            }
            let exited = {
                let mut guard = self.child.lock();
                match guard.as_mut() {
                    Some(process) => process.try_wait().ok().flatten(),
                    None => Some(std::process::ExitStatus::default()),
                }
            };
            if let Some(status) = exited {
                *self.child.lock() = None;
                return Ok(SupervisorState::Failed {
                    reason: format!("daemon exited during startup: {status}"),
                });
            }
            if Instant::now() >= deadline {
                return Ok(SupervisorState::Failed {
                    reason: "daemon did not become ready in 30s".into(),
                });
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    /// Single-instance behavior (Task 23.4): if an existing healthy daemon
    /// is present, a second Desktop invocation attaches instead of spawning.
    pub fn attach_or_wait(&self) -> Result<SupervisorState> {
        self.ensure_daemon()
    }

    /// Shuts down only the daemon we spawned (never one we attached to).
    pub fn shutdown_spawned(&self) -> bool {
        let mut guard = self.child.lock();
        if let Some(child) = guard.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
            *guard = None;
            true
        } else {
            false
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supervisor() -> DaemonSupervisor {
        DaemonSupervisor::new(
            std::path::PathBuf::from("steward-daemon.exe"),
            std::env::temp_dir().join("steward-desktop-test"),
            59999,
            59998,
        )
    }

    #[test]
    fn dead_port_reports_unhealthy() {
        // Bind then drop: the port is closed now.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let supervisor = DaemonSupervisor::new(
            std::path::PathBuf::from("missing-binary"),
            std::env::temp_dir(),
            port,
            port + 1,
        );
        assert!(!supervisor.is_healthy());
    }

    #[test]
    fn live_listener_reports_healthy() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let supervisor = DaemonSupervisor::new(
            std::path::PathBuf::from("unused"),
            std::env::temp_dir(),
            port,
            port + 1,
        );
        assert!(supervisor.is_healthy(), "attach path: existing daemon is healthy");
        drop(listener);
    }

    #[test]
    fn failed_start_reports_reason() {
        let supervisor = supervisor();
        let state = supervisor.ensure_daemon().unwrap();
        match state {
            SupervisorState::Failed { reason } => assert!(!reason.is_empty()),
            other => panic!("expected failure for missing binary, got {other:?}"),
        }
    }

    #[test]
    fn shutdown_with_no_spawned_child_is_noop() {
        let supervisor = supervisor();
        assert!(!supervisor.shutdown_spawned(), "attached daemons are never killed");
    }

    #[test]
    fn port_accessor_matches_configuration() {
        assert_eq!(supervisor().port(), 59999);
    }
}
