use rusqlite::Connection;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;
use reqwest;

#[test]
fn test_db_sqlite_vec_loaded() {
    let conn = Connection::open("test_e2e_steward.db").unwrap();
    
    unsafe {
        conn.load_extension_enable().unwrap();
        conn.load_extension("sqlite_vec0", None).unwrap();
        conn.load_extension_disable().unwrap();
    }
    
    let mut stmt = conn.prepare("SELECT vec_version()").unwrap();
    let version: String = stmt.query_row([], |row| row.get(0)).unwrap();
    
    assert!(!version.is_empty());
    println!("Loaded sqlite-vec version: {}", version);
}

#[tokio::test]
async fn test_daemon_and_cli_interaction() {
    // Start daemon
    let mut daemon_child = Command::new("cargo")
        .args(&["run", "--bin", "steward-daemon"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start daemon");

    // Give it a moment to boot
    sleep(Duration::from_secs(2)).await;

    // Run CLI
    let cli_output = Command::new("cargo")
        .args(&["run", "--bin", "steward-cli"])
        .output()
        .expect("Failed to run CLI");

    // The E2E tests expect assert!(output.status.success());
    assert!(cli_output.status.success());
    let stdout = String::from_utf8_lossy(&cli_output.stdout);
    assert!(stdout.contains("RESPONSE="));

    daemon_child.kill().unwrap();
}
