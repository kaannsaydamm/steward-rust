use std::process::Command;
use std::time::Duration;
use std::thread;
use tempfile::TempDir;

const DAEMON_BIN: &str = env!("CARGO_BIN_EXE_steward-daemon");
const CLI_BIN: &str = env!("CARGO_BIN_EXE_steward-cli");

// F1: Daemon Lifecycle
#[test]
fn test_daemon_lifecycle() {
    let mut child = Command::new(DAEMON_BIN)
        .arg("start")
        .spawn()
        .expect("Failed to start daemon");
        
    thread::sleep(Duration::from_millis(500));
    
    // Attempt to stop daemon using cli
    let status = Command::new(CLI_BIN)
        .arg("stop")
        .status()
        .expect("Failed to execute stop command");
        
    assert!(status.success(), "Daemon stop command failed");
    
    // Ensure daemon process exited
    let exit_status = child.wait().expect("Failed to wait on daemon");
    assert!(exit_status.success(), "Daemon did not exit cleanly");
}

// F2: CLI Ping
#[test]
fn test_cli_ping() {
    let mut daemon = Command::new(DAEMON_BIN)
        .arg("start")
        .spawn()
        .expect("Failed to start daemon");

    thread::sleep(Duration::from_millis(500));

    let output = Command::new(CLI_BIN)
        .arg("ping")
        .output()
        .expect("Failed to execute ping");

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Stop daemon
    let _ = daemon.kill();
    let _ = daemon.wait();

    assert!(output.status.success(), "CLI ping failed");
    assert!(stdout.contains("PONG"), "Ping did not return PONG");
}

// F3: SQLite DB Init
#[test]
fn test_sqlite_db_init() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("steward.db");
    
    let status = Command::new(DAEMON_BIN)
        .arg("init")
        .arg("--db-path")
        .arg(&db_path)
        .status()
        .expect("Failed to initialize daemon");

    assert!(status.success(), "Daemon init failed");
    assert!(db_path.exists(), "Database file was not created");
}

// F4: Plugin Sandbox
#[test]
fn test_plugin_sandbox() {
    let mut daemon = Command::new(DAEMON_BIN)
        .arg("start")
        .spawn()
        .expect("Failed to start daemon");

    thread::sleep(Duration::from_millis(500));

    // Execute plugin load command
    let output = Command::new(CLI_BIN)
        .arg("plugin")
        .arg("load")
        .arg("test_plugin.wasm")
        .output()
        .expect("Failed to execute plugin load");
        
    let _ = daemon.kill();
    let _ = daemon.wait();

    assert!(output.status.success(), "Plugin load command failed");
}
