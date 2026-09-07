//! Steward Desktop supervisor library.

pub mod menu;
pub mod supervisor;
pub mod tray;

pub use supervisor::{DaemonSupervisor, SupervisorState};
