//! Steward Desktop: Tauri shell over the shared daemon (Omega §42).
//!
//! This binary is the supervisor library + entry point; the Tauri runtime
//! wraps it with tray/dialogs/notifications once platform signing is
//! configured (§44). Domain logic lives in the daemon only.

pub mod supervisor;

pub use supervisor::{DaemonSupervisor, SupervisorState};

fn main() {
    // Placeholder entry: the Tauri build wires this into tauri::Builder.
    // The supervisor logic is fully testable without the GUI runtime.
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--selfcheck") {
        let supervisor = DaemonSupervisor::new(
            std::path::PathBuf::from("steward-daemon"),
            steward_core::storage::root().unwrap_or_default(),
            50051,
            3000,
        );
        let state = supervisor.ensure_daemon().unwrap_or(SupervisorState::Detached);
        println!("{state:?}");
    }
}
