//! `steward-workspace`: workspace roots, leases, process manager, git
//! worktrees, sandbox backends (Omega §24).

pub mod lease;
pub mod process;
pub mod worktree;

pub use lease::{WorkspaceLease, WorkspaceMode, WorkspaceRegistry};
pub use process::{ProcessManager, ProcessSpec, ProcessStatus, ManagedProcess};
pub use worktree::WorktreeManager;
