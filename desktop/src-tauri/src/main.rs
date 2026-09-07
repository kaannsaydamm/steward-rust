//! Steward Desktop: Tauri shell over the shared daemon (Omega §42).
//!
//! The window loads the same static WebUI the daemon serves; the supervisor
//! guarantees a daemon is running before the window opens. Domain logic
//! lives in the daemon only — the shell owns no runtime state.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use steward_desktop::{DaemonSupervisor, SupervisorState};

fn main() {
    let data_root = steward_core::storage::root().unwrap_or_default();
    let supervisor = DaemonSupervisor::new(
        std::path::PathBuf::from("steward-daemon"),
        data_root,
        50051,
        3000,
    );
    let state = supervisor
        .ensure_daemon()
        .unwrap_or(SupervisorState::Detached);
    eprintln!("daemon supervisor: {state:?}");

    match state {
        SupervisorState::Failed { reason } => {
            eprintln!("fatal: {reason}");
            std::process::exit(1);
        }
        // Detached/Attached/Starting all proceed: the WebUI itself reports
        // daemon status and reconnects live.
        _ => {}
    }

    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    // Only kill a daemon WE spawned (attached daemons belong to their own owner).
    supervisor.shutdown_spawned();
}
